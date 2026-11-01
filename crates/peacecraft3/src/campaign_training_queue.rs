//! Shared training-queue presentation for the selected producer.
//!
//! The host keeps one FIFO per producer with per-entry costs, food and
//! source-derived refunds. This panel reads that queue fresh every frame and
//! paints the canonical order: the active head first with its remaining-time
//! veil, then the pending entries. Every entry is a cancel button that shares
//! the host's FIFO-slot cancellation with the input path; there is no script
//! order for it (the public `"cancel"` string converts to zero).
//!
//! Presentation rules, kept honest:
//! - Icon art, tooltips, costs and per-entry refunds resolve the same unit
//!   catalog and map overrides the enqueue path uses. A removed producer
//!   errors the queue read, so the panel hides instead of showing a stale
//!   queue. An owner change errors the same way, so no other owner's queue is
//!   presented as this producer's. Producer death clears the jobs host-side
//!   with refunds and no cancel events; the panel then hides on its empty
//!   queue.
//! - A non-empty queue with no active head is the paid head waiting: food
//!   capacity when the producer runs, otherwise the producer's pause. The
//!   host only leaves a pending head for those two gates.
//! - A pending head that starts in the same callback it was paid in is read
//!   fresh like every other frame, so no extra input is needed to present it.
//! - The strip height and the entry size are read at runtime from the shipped
//!   info-panel frame definitions (`SimpleBuildQueueBackdrop` height and the
//!   info icon template). The shared FDF parser covers the glue subset and
//!   does not keep named texture blocks, so this module scans those records
//!   with its own bounded depth walk. Exact strip offsets still need a
//!   rendered comparison; the left-anchored bottom strip is a recreation
//!   choice, and the remaining-time veil is recreation chrome until the
//!   authored statusbar art is compared.
use bevy::{ecs::system::SystemParam, prelude::*, ui_widgets::Button};
use pcx_play::{
    map_startup_host::MapStartupHost,
    playercolor_host::{ExecutedUnit, ObjectHost, TrainingQueueEntry},
};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources, map_binding::AssetReceipt,
    map_requests::MapRequests, map_script_session::MapScriptSession,
};
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

const CONSOLE_FDF: &str = r"UI\FrameDef\UI\ConsoleUI.fdf";
const SIMPLE_PANEL_FDF: &str = r"UI\FrameDef\UI\SimpleInfoPanel.fdf";
const BUILDING_DETAIL_FDF: &str = r"UI\FrameDef\UI\InfoPanelBuildingDetail.fdf";
const SKIN: &str = r"UI\war3skins.txt";
/// Application read/cache ceilings, not authored icon sizes or slot counts.
const MAX_FDF_BYTES: u64 = 1 << 20;
const MAX_IMAGES: usize = 64;
const MAX_RETAINED: usize = 32 << 20;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TrainingQueueSet;

/// Runtime geometry: the authored strip height and entry size in frame units,
// plus the logical viewport they are percentages of. `repeat` is the
// command-texture sampler addressing the executable selects.
#[derive(Clone, Copy, Debug, PartialEq)]
struct QueueGeometry {
    strip: f32,
    icon: f32,
    viewport: [f32; 2],
    repeat: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct QueueView {
    producer: HandleId,
    capacity: usize,
    kinds: Vec<(i32, bool)>,
}

/// The remaining-time veil over the active entry, as a width of its button.
struct QueueFill {
    entity: Entity,
    full: f32,
}

#[derive(Resource, Default)]
pub(crate) struct TrainingQueue {
    witness: Option<Weak<()>>,
    geometry: Option<QueueGeometry>,
    console: Option<String>,
    attempted: bool,
    parent: Option<Entity>,
    root: Option<Entity>,
    view: Option<QueueView>,
    fills: Vec<QueueFill>,
    images: HashMap<String, Handle<Image>>,
    failed: HashMap<String, String>,
    retained: usize,
    receipts: Vec<AssetReceipt>,
    pub last_error: Option<String>,
}

impl TrainingQueue {
    fn reset(
        &mut self,
        commands: &mut Commands,
        images: &mut Assets<Image>,
        witness: Option<Weak<()>>,
    ) {
        if let Some(root) = self.root.take() {
            commands.entity(root).try_despawn();
        }
        for image in self.images.values() {
            images.remove(image.id());
        }
        *self = TrainingQueue {
            witness,
            ..Default::default()
        };
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<TrainingQueue>().add_systems(
        Update,
        sync
            .in_set(TrainingQueueSet)
            .after(crate::hud_render::HudRenderSet)
            .after(crate::campaign_command_card::CommandCardSet)
            .before(peacecraft3::campaign_presentation::CampaignPresentationSet),
    );
}

#[derive(SystemParam)]
struct Inputs<'w, 's> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    camera: Res<'w, peacecraft3::campaign_camera::CampaignCamera>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
    frames: Query<'w, 's, (Entity, &'static crate::hud_render::HudSourceAttachment)>,
}

/// One `Height`/`Width` number from a named texture or backdrop block. The
/// walk starts at the block's header line and reads records until its braces
/// balance, so nested blocks cannot leak their numbers into the wrong frame.
fn block_number(text: &str, block: &str, key: &str) -> Result<f32, String> {
    let header = format!("\"{block}\"");
    let mut depth = None::<i32>;
    for raw in text.lines() {
        let line = match raw.find("//") {
            Some(index) => &raw[..index],
            None => raw,
        }
        .trim();
        if depth.is_none() {
            if line.contains(&header) && line.contains('{') {
                let opened = line.matches('{').count() as i32 - line.matches('}').count() as i32;
                if opened <= 0 {
                    return Err(format!("{block} has no body"));
                }
                depth = Some(opened);
            }
            continue;
        }
        let current = depth.expect("queue block depth");
        let open = line.matches('{').count() as i32;
        let close = line.matches('}').count() as i32;
        if current + open - close <= 0 {
            return Err(format!("{block} has no {key}"));
        }
        depth = Some(current + open - close);
        if open > 0 {
            continue;
        }
        let mut parts = line.split_whitespace();
        if parts.next() == Some(key) {
            let Some(value) = parts.next() else {
                continue;
            };
            let number: f32 = value
                .trim_end_matches(',')
                .parse()
                .map_err(|_| format!("{block} {key} is not a number"))?;
            if !number.is_finite() || number <= 0.0 {
                return Err(format!("{block} {key} is not positive"));
            }
            return Ok(number);
        }
    }
    Err(format!("{block} is missing"))
}

/// Authored strip and entry sizes from the shipped info-panel definitions.
fn read_geometry(
    reader: &pcx_assets::Resolver,
    viewport: [f32; 2],
    repeat: bool,
    receipts: &mut Vec<AssetReceipt>,
) -> Result<QueueGeometry, String> {
    use crate::campaign_hud::upper_buttons::read;
    let mut aggregate = String::new();
    for member in [SIMPLE_PANEL_FDF, BUILDING_DETAIL_FDF] {
        let bytes = read(reader, member, MAX_FDF_BYTES, receipts)?;
        aggregate.push_str(
            &String::from_utf8(bytes).map_err(|_| format!("{member} is not UTF-8"))?,
        );
        aggregate.push('\n');
    }
    let strip = block_number(&aggregate, "SimpleBuildQueueBackdrop", "Height")
        .or_else(|_| block_number(&aggregate, "BuildQueueBackdrop", "Height"))?;
    let icon = block_number(&aggregate, "InfoPanelIconTemplate", "Width").and_then(|width| {
        let height = block_number(&aggregate, "InfoPanelIconTemplate", "Height")?;
        if (width - height).abs() > 1e-6 {
            return Err("info icon template is not square".to_string());
        }
        Ok(width)
    })?;
    if strip > viewport[1] || icon > viewport[0] || icon > viewport[1] || icon > strip {
        return Err("queue geometry exceeds its viewport".to_string());
    }
    Ok(QueueGeometry {
        strip,
        icon,
        viewport,
        repeat,
    })
}

fn image(
    reader: &pcx_assets::Resolver,
    state: &mut TrainingQueue,
    path: &str,
    repeat: bool,
    images: &mut Assets<Image>,
) -> Result<Handle<Image>, String> {
    if let Some(image) = state.images.get(path) {
        return Ok(image.clone());
    }
    if let Some(error) = state.failed.get(path) {
        return Err(error.clone());
    }
    if state.images.len() + state.failed.len() >= MAX_IMAGES {
        return Err("training queue image cache budget exceeded".into());
    }
    let result: Result<Handle<Image>, String> = (|| {
        use crate::campaign_hud::upper_buttons::read;
        let bytes = read(reader, path, 4 << 20, &mut state.receipts)?;
        let remaining = MAX_RETAINED
            .checked_sub(state.retained)
            .ok_or("training queue image aggregate budget")?;
        let texture = pcx_gfx::load_image_bounded(&bytes, path, remaining.min(16 << 20))
            .map_err(|error| error.to_string())?;
        let retained = state
            .retained
            .checked_add(texture.rgba_bytes().ok_or("training queue image size")?)
            .filter(|bytes| *bytes <= MAX_RETAINED)
            .ok_or("training queue image aggregate budget")?;
        let mut decoded = pcx_gfx::viewer::rgba_image(&texture);
        decoded.sampler = bevy::image::ImageSampler::Descriptor(
            crate::campaign_hud::upper_buttons::image_sampler(repeat),
        );
        let handle = images.add(decoded);
        state.images.insert(path.into(), handle.clone());
        state.retained = retained;
        Ok(handle)
    })();
    if let Err(error) = &result {
        state.failed.insert(path.into(), error.clone());
    }
    result
}

/// The entry's authored tip with its resolved cost, per-entry refund and
/// queue state. Costs and refunds come from the queue read, not from a
/// second catalog pass.
fn entry_tooltip(entry: &TrainingQueueEntry, slot: usize, capacity: usize) -> (String, String) {
    let name = pcx_objects::fourcc_text(entry.kind.to_be_bytes());
    let mut tip = format!(
        "Cost: {} gold, {} lumber, {} food, {} sec.|nRefund on cancel: {} gold, {} lumber.|nQueue slot {} of {}.",
        entry.gold,
        entry.lumber,
        entry.food,
        entry.duration,
        entry.refund_gold,
        entry.refund_lumber,
        slot + 1,
        capacity
    );
    if entry.active {
        tip.push_str("|nTraining — click to cancel this entry and refund it.");
    } else {
        tip.push_str("|nQueued — click to cancel this entry and refund it.");
    }
    (name, tip)
}

/// The elapsed fraction of the active head: one minus the remaining share of
/// its authored duration. The host stretches the deadline across pauses, so
/// a paused head holds its fraction without extra handling here.
fn elapsed_fraction(entry: &TrainingQueueEntry, now: f64) -> Result<f32, String> {
    if !entry.active {
        return Ok(0.0);
    }
    let (finish, _) = entry
        .finish
        .zip(entry.through)
        .ok_or("active training entry has no deadline")?;
    if entry.duration <= 0 {
        return Err("training duration must be positive".into());
    }
    if !finish.is_finite() || !now.is_finite() {
        return Err("training clock is not finite".into());
    }
    let fraction = 1.0 - ((finish - now) / f64::from(entry.duration)) as f32;
    Ok(fraction.clamp(0.0, 1.0))
}

fn display_of(visible: bool) -> Display {
    if visible {
        Display::Flex
    } else {
        Display::None
    }
}

fn root_node(geometry: &QueueGeometry, len: usize, visible: bool) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(0.0),
        bottom: Val::Percent(0.0),
        width: Val::Percent(len as f32 * geometry.icon / geometry.viewport[0] * 100.0),
        height: Val::Percent(geometry.strip / geometry.viewport[1] * 100.0),
        flex_direction: FlexDirection::Row,
        display: display_of(visible),
        ..Default::default()
    }
}

fn hide(state: &mut TrainingQueue, nodes: &mut Query<&mut Node>) {
    if let Some(root) = state.root
        && let Ok(mut node) = nodes.get_mut(root)
        && node.display != Display::None
    {
        node.display = Display::None;
    }
}

/// The skin-resolved console frame mounted for this binding. The console name
/// comes from the same skin lookup the command card uses.
fn console_parent(
    inputs: &Inputs,
    console: &str,
    bindings: &peacecraft3::map_requests::MapBindings,
) -> Result<Entity, String> {
    let mut parents = inputs.frames.iter().filter(|(_, frame)| {
        frame.frame == console
            && frame.source_member == CONSOLE_FDF
            && frame.receipt_path == bindings.receipts.map.path.as_str()
            && frame.binding_revision == bindings.revision
    });
    let Some((parent, _)) = parents.next() else {
        return Err("training queue awaits its mounted console parent".into());
    };
    if parents.next().is_some() {
        return Err("training queue console parent is ambiguous".into());
    }
    Ok(parent)
}

fn sync(
    mut commands: Commands,
    mut state: ResMut<TrainingQueue>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    inputs: Inputs,
    mut images: ResMut<Assets<Image>>,
    mut nodes: Query<&mut Node>,
) {
    let witness = session.session_witness();
    if !match (&state.witness, &witness) {
        (Some(old), Some(new)) => Weak::ptr_eq(old, new),
        (None, None) => true,
        _ => false,
    } {
        state.reset(&mut commands, &mut images, witness.clone());
    }
    let (Some(witness), Some(context)) = (witness, session.context().cloned()) else {
        return;
    };
    let Some(bindings) = inputs.requests.bindings.as_ref().filter(|bindings| {
        inputs
            .requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
            && inputs
                .requests
                .admission
                .shared_context()
                .is_some_and(|admitted| Arc::ptr_eq(&admitted, &context))
    }) else {
        return;
    };
    let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
        return;
    };
    let Some(host) = session.host() else { return };
    if !state.attempted {
        state.attempted = true;
        let result = (|| {
            let install = inputs
                .launcher
                .as_deref()
                .and_then(crate::launch_gui::selected_install);
            let (viewport, repeat, console) = crate::menu_lighting::with_client_image(
                install.as_deref(),
                |label, image| {
                    let identity = pcx_play::campaign_inputs::client_identity(image, label);
                    let rules =
                        pcx_play::command_layout::Rules::reviewed_legacy(image, identity.clone())?;
                    let skin_text = crate::campaign_hud::upper_buttons::text(
                        &reader,
                        SKIN,
                        &mut state.receipts,
                    )?;
                    let skin = crate::menu_skin::SkinTable::parse(&skin_text)
                        .map_err(|e| e.to_string())?;
                    let sections = crate::campaign_hud::race_sections(
                        &skin,
                        host.objects()
                            .local_race_preference_constant()
                            .map_err(|e| e.to_string())?,
                    )?;
                    let console = crate::campaign_hud::upper_buttons::skin_file(
                        &skin,
                        &sections,
                        &rules.console_key,
                    )?;
                    Ok::<_, String>((rules.viewport, rules.icon_repeat, console))
                },
            )?;
            let geometry = read_geometry(&reader, viewport, repeat, &mut state.receipts)?;
            crate::campaign_hud::upper_buttons::text(&reader, CONSOLE_FDF, &mut state.receipts)?;
            Ok::<_, String>((geometry, console))
        })();
        match result {
            Ok((geometry, console)) => {
                state.geometry = Some(geometry);
                state.console = Some(console);
                state.last_error = None;
            }
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        }
    }
    if state.last_error.is_some() {
        hide(&mut state, &mut nodes);
        return;
    }
    let (Some(geometry), Some(console)) = (state.geometry, state.console.clone()) else {
        return;
    };
    let parent = match console_parent(&inputs, &console, bindings) {
        Ok(parent) => parent,
        Err(error) => {
            state.last_error = Some(error);
            hide(&mut state, &mut nodes);
            return;
        }
    };
    if state.parent != Some(parent) || state.root.is_none_or(|root| nodes.get(root).is_err()) {
        if let Some(root) = state.root.take() {
            commands.entity(root).try_despawn();
        }
        let root = commands
            .spawn((
                root_node(&geometry, 0, false),
                peacecraft3::campaign_presentation::CampaignInterface,
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(parent).add_child(root);
        state.parent = Some(parent);
        state.root = Some(root);
        state.view = None;
        state.fills.clear();
    }
    let root = state.root.expect("queue root");
    let objects = host.objects();
    let active = match objects.active_unit() {
        Ok(active) => active,
        Err(error) => {
            state.last_error = Some(error);
            hide(&mut state, &mut nodes);
            return;
        }
    };
    let Some(producer) = active else {
        state.last_error = None;
        hide(&mut state, &mut nodes);
        return;
    };
    let record = match objects.executed_unit(producer) {
        Ok(record) => record,
        Err(error) => {
            state.last_error = Some(error.to_string());
            hide(&mut state, &mut nodes);
            return;
        }
    };
    if objects
        .player_settings(record.owner)
        .map_err(|e| e.to_string())
        .is_ok_and(|player| Some(player.number) != objects.local_player_number())
    {
        state.last_error = None;
        hide(&mut state, &mut nodes);
        return;
    }
    let details = match objects.unit_training_details(producer) {
        Ok(details) => details,
        Err(error) => {
            state.last_error = Some(error.to_string());
            hide(&mut state, &mut nodes);
            return;
        }
    };
    if details.is_empty() {
        state.last_error = None;
        hide(&mut state, &mut nodes);
        return;
    }
    let capacity = match objects.training_queue_capacity() {
        Ok(capacity) => capacity,
        Err(error) => {
            state.last_error = Some(error.to_string());
            hide(&mut state, &mut nodes);
            return;
        }
    };
    let now = match objects.timer_game_time().filter(|now| now.is_finite()) {
        Some(now) => now,
        None => {
            state.last_error = Some("training clock is not admitted".into());
            hide(&mut state, &mut nodes);
            return;
        }
    };
    state.last_error = None;
    let view = QueueView {
        producer,
        capacity,
        kinds: details
            .iter()
            .map(|entry| (entry.kind, entry.active))
            .collect(),
    };
    if state.view.as_ref() != Some(&view) {
        if let Err(error) = paint(
            &mut commands,
            &mut state,
            objects,
            &reader,
            &mut images,
            root,
            &witness,
            &geometry,
            &view,
            &details,
            producer_paused(&record),
            now,
        ) {
            state.last_error = Some(error);
            hide(&mut state, &mut nodes);
            return;
        }
        state.view = Some(view);
    }
    if nodes.get(root).map_or(true, |node| {
        node.display != display_of(inputs.camera.active())
    }) {
        commands
            .entity(root)
            .insert(root_node(&geometry, details.len(), inputs.camera.active()));
    }
    for entry in &details {
        if !entry.active {
            continue;
        }
        let fraction = match elapsed_fraction(entry, now) {
            Ok(fraction) => fraction,
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        };
        // The active head is always the first FIFO entry, so the single fill
        // tracks slot zero.
        if let Some(fill) = state.fills.first() {
            let width = Val::Percent(fill.full * (1.0 - fraction));
            if let Ok(mut node) = nodes.get_mut(fill.entity)
                && node.width != width
            {
                node.width = width;
            }
        }
    }
}

fn producer_paused(record: &ExecutedUnit) -> bool {
    record.paused_since.is_some()
}

fn paint(
    commands: &mut Commands,
    state: &mut TrainingQueue,
    objects: &ObjectHost,
    reader: &pcx_assets::Resolver,
    images: &mut Assets<Image>,
    root: Entity,
    witness: &Weak<()>,
    geometry: &QueueGeometry,
    view: &QueueView,
    details: &[TrainingQueueEntry],
    paused: bool,
    now: f64,
) -> Result<(), String> {
    commands.entity(root).despawn_children();
    state.fills.clear();
    let blocked = details.iter().all(|entry| !entry.active);
    for (slot, entry) in details.iter().enumerate() {
        let kind = entry.kind.to_be_bytes();
        let definition = objects
            .unit_catalog()
            .map_err(|e| e.to_string())?
            .resolve(kind)
            .map_err(|e| format!("training queue unit {kind:?}: {e:?}"))?;
        let command = pcx_play::training_command::Command::read(&definition)
            .map_err(|e| format!("training queue unit {kind:?}: {e}"))?;
        pcx_assets::AssetPath::new(&command.art).map_err(|e| e.to_string())?;
        let image = image(reader, state, &command.art, geometry.repeat, images)?;
        let (name, mut tip) = match command.tooltip {
            Ok(tip) => (tip.title, tip.description),
            Err(_) => (
                pcx_objects::fourcc_text(kind),
                "The authored tooltip is not resolved.".to_string(),
            ),
        };
        let (cost_name, cost_tip) = entry_tooltip(entry, slot, view.capacity);
        if name.is_empty() {
            tip = format!("{cost_name}|n{cost_tip}");
        } else {
            tip.push_str(&format!("|n|n{cost_tip}"));
        }
        if blocked {
            tip.push_str(if paused {
                "|nPaused — the paid head resumes with its remaining time."
            } else {
                "|nWaiting for food — the paid head starts when capacity frees."
            });
        }
        let left = slot as f32 * geometry.icon / geometry.viewport[0] * 100.0;
        let width = geometry.icon / geometry.viewport[0] * 100.0;
        let height = geometry.icon / geometry.viewport[1] * 100.0;
        let button = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(left),
                    bottom: Val::Percent(0.0),
                    width: Val::Percent(width),
                    height: Val::Percent(height),
                    ..Default::default()
                },
                ImageNode::new(image).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
                Button,
                bevy::picking::hover::Hovered::default(),
                crate::campaign_command_card::ButtonOrder {
                    unit: view.producer,
                    kind: crate::campaign_command_card::Kind::TrainCancel(slot),
                    session: witness.clone(),
                    order: None,
                },
                crate::campaign_inventory::ControlTooltip { name, ubertip: tip },
                crate::campaign_inventory::TooltipAnchor {
                    rect: pcx_play::inventory_layout::Rect {
                        x: slot as f32 * geometry.icon,
                        y: 0.0,
                        width: geometry.icon,
                        height: geometry.icon,
                    },
                    viewport: geometry.viewport,
                },
            ))
            .id();
        commands.entity(root).add_child(button);
        if entry.active {
            let remaining = 1.0 - elapsed_fraction(entry, now)?;
            let veil = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: Val::Percent(0.0),
                        top: Val::Percent(0.0),
                        width: Val::Percent(width * remaining),
                        height: Val::Percent(100.0),
                        ..Default::default()
                    },
                    // Recreation chrome: the authored statusbar art still
                    // needs a rendered comparison.
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(button).add_child(veil);
            state.fills.push(QueueFill {
                entity: veil,
                full: width,
            });
        }
    }
    Ok(())
}
