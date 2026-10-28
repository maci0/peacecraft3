//! The in-game cursor: the local player's race cursor model (`Cursor` in
//! `UI\war3skins.txt`) drawn at the pointer in place of the system cursor, with the
//! sequence and tint the client image gives each pointer state.
//!
//! [`pcx_play::cursor_rules`] reads which mode shows which sequence and tint. This module
//! classifies the pointer (edge band, interface, armed command, unit under it), poses the
//! model, and hides the system cursor while the model is drawn. The model is posed by
//! [`crate::campaign_score_model`] and its real-time clock
//! ([`crate::campaign_score_model::animate_backgrounds`]). Evidence labels and the states
//! that are not established are in `docs/re-notes/input-cursor.md`.
use std::sync::{Arc, Weak};

use bevy::{
    camera::{
        ScalingMode,
        visibility::RenderLayers,
    },
    core_pipeline::tonemapping::{DebandDither, Tonemapping},
    ecs::system::SystemParam,
    picking::{hover::HoverMap, pointer::PointerId},
    prelude::*,
    ui_widgets::Button,
    window::{CursorOptions, PrimaryWindow},
};
use pcx_play::{
    cursor_rules::{CursorRules, Look, Pointer},
    map_startup_host::MapStartupHost,
    playercolor_host::{ObjectHost, Relation, VisionCapability},
    scripts::AssetIdentity,
    standard_command::Kind as StandardKind,
};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};

use crate::{
    campaign_command_card::Kind,
    campaign_hud::upper_buttons::{skin_file, text},
    campaign_input::{CampaignInput, CampaignInputSet, Picking, ground_point},
    campaign_score_model::{self, AuthoredBackground, ScoreBackgroundState},
    fdf::Rect,
    mainmenu::FRAME_H,
    menu_material::MenuMaterial,
};

const SKIN: &str = r"UI\war3skins.txt";
/// The skin key that names the race cursor model.
const CURSOR_KEY: &str = "Cursor";
/// The cursor draws above every other camera, the loading screen included.
const CAMERA_ORDER: isize = 2000;
/// Render layer of the cursor model, apart from the menu, HUD and loading layers.
const CURSOR_LAYER: usize = 11;
/// The model has no depth of its own worth keeping: its z span is a few thousandths.
const DEPTH_RANGE: f32 = 1.0;

/// A pointer state the cursor could not show as the original does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CursorDiagnostic {
    /// The model has no sequence of this name, so mode `mode` shows the normal cursor.
    MissingSequence { mode: u8, name: String },
    /// The original's cursor for this situation was not established; the normal cursor
    /// is shown.
    Unestablished(Situation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Situation {
    /// A building waits to be placed. The client's build mode sets no cursor mode in the
    /// handlers read, and the live capture did not cover it.
    BuildPlacement,
}

/// What a waiting command asks of the cursor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Armed {
    #[default]
    Nothing,
    /// Attack: units under the pointer colour the cursor by relation.
    Attack,
    /// Any other command waiting for a target (spells, move, patrol): the plain target
    /// cursor.
    Target,
    Build,
}

impl Armed {
    fn of(kind: Kind) -> Self {
        match kind {
            Kind::Standard(StandardKind::Attack) => Self::Attack,
            Kind::TargetUnit
            | Kind::Standard(
                StandardKind::Move
                | StandardKind::Patrol
                | StandardKind::AttackGround
                | StandardKind::Rally,
            ) => Self::Target,
            Kind::Standard(StandardKind::Stop | StandardKind::Hold)
            | Kind::Immediate
            | Kind::Train(_)
            | Kind::BuildMenu
            | Kind::Build(_)
            | Kind::BuildCancel
            | Kind::CancelConstruction
            | Kind::SkillMenu
            | Kind::SkillCancel
            | Kind::Learn(_)
            | Kind::DisplayOnly => Self::Nothing,
        }
    }
}

/// The unit under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Hovered {
    pub relation: Relation,
    /// The unit is part of the local selection.
    pub selected: bool,
}

/// Everything the pointer state depends on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Facts {
    /// Edge directions the pointer is in (`pcx_play::camera_input::edge`).
    pub edges: u8,
    /// The pointer is over the interface or outside the world view, or input is closed.
    pub over_interface: bool,
    pub armed: Armed,
    pub hover: Option<Hovered>,
}

/// The pointer state of `facts`, and the situation that fell back to the normal cursor.
///
/// An edge band wins over everything: the client locks the scroll cursor while an edge
/// scroll is held, including over the interface.
pub(crate) fn pointer(facts: Facts) -> (Pointer, Option<Situation>) {
    if facts.edges != 0 {
        return (Pointer::Scroll(facts.edges), None);
    }
    match facts.armed {
        Armed::Build => (Pointer::Normal, Some(Situation::BuildPlacement)),
        _ if facts.over_interface => (Pointer::Normal, None),
        Armed::Attack => (
            Pointer::Target(facts.hover.filter(|h| !h.selected).map(|h| h.relation)),
            None,
        ),
        Armed::Target => (Pointer::Target(None), None),
        Armed::Nothing => (
            facts
                .hover
                .map_or(Pointer::Normal, |h| Pointer::Hover(h.relation)),
            None,
        ),
    }
}

/// Cursor position in the model camera's units: `FRAME_H` tall, centred on the window.
fn model_position(cursor: Vec2, window: Vec2) -> Vec2 {
    let scale = FRAME_H / window.y;
    Vec2::new(
        (cursor.x - window.x / 2.0) * scale,
        (window.y / 2.0 - cursor.y) * scale,
    )
}

/// The model and the sequences it carries.
struct Model {
    root: Entity,
    camera: Entity,
    /// Sequence name and its interval, in model order.
    sequences: Vec<(String, [u32; 2])>,
    /// What spawning the model created, released when the session ends.
    handles: campaign_score_model::ScoreSceneHandles,
}

#[derive(Resource, Default)]
pub(crate) struct CampaignCursor {
    witness: Option<Weak<()>>,
    attempted: bool,
    rules: Option<CursorRules>,
    model: Option<Model>,
    /// The last pointer state, and the mode and tint applied for it.
    pointer: Option<Pointer>,
    applied: Option<(u8, [u8; 3])>,
    facts: Facts,
    /// Where the pointer is in the model camera's units; `None` outside the window.
    position: Option<Vec2>,
    pub diagnostics: Vec<CursorDiagnostic>,
    pub last_error: Option<String>,
}

impl CampaignCursor {
    fn note(&mut self, diagnostic: CursorDiagnostic) {
        if !self.diagnostics.contains(&diagnostic) {
            warn!("cursor: {diagnostic:?}");
            self.diagnostics.push(diagnostic);
        }
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CampaignCursorSet;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignCursor>().add_systems(
        Update,
        (load, classify, present)
            .chain()
            .in_set(CampaignCursorSet)
            .after(CampaignCameraSet)
            .after(CampaignInputSet),
    );
}

#[derive(SystemParam)]
struct Inputs<'w> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    camera: Res<'w, CampaignCamera>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
}

#[derive(SystemParam)]
struct Stores<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<MenuMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}

fn same_session(a: Option<&Weak<()>>, b: Option<&Weak<()>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// The `Cursor` model path of the race the local player picked.
fn model_path(
    reader: &pcx_assets::Resolver,
    race: Option<String>,
) -> Result<String, String> {
    let mut receipts = Vec::new();
    let skin = crate::menu_skin::SkinTable::parse(&text(reader, SKIN, &mut receipts)?)
        .map_err(|e| e.to_string())?;
    let sections = crate::campaign_hud::race_sections(&skin, race)?;
    skin_file(&skin, &sections, CURSOR_KEY)
}

/// A cursor model read and checked against the rules.
struct Prepared {
    plan: campaign_score_model::BackgroundPlan,
    /// Sequence name and its interval, in model order.
    sequences: Vec<(String, [u32; 2])>,
    /// Modes whose sequence the model lacks.
    diagnostics: Vec<CursorDiagnostic>,
}

/// Reads the model and checks every sequence the rules ask for against it.
fn prepare(
    reader: &pcx_assets::Resolver,
    path: &str,
    rules: &CursorRules,
) -> Result<Prepared, String> {
    let plan = campaign_score_model::prepare_sequence(
        reader,
        &AuthoredBackground {
            frame: CURSOR_KEY.into(),
            model: path.into(),
            // The model is placed by its root; its authored origin is the pointer.
            rect: Rect {
                left: 0.0,
                bottom: 0.0,
                width: 0.0,
                height: 0.0,
            },
            unlit: true,
        },
        None,
    )?;
    let mut plan = plan;
    // The client fills the item-icon slot with the carried item's icon. The recreation has
    // no carried item, so the slot is transparent and HoldItem is never selected.
    for texture in plan.runtime.textures.iter().filter(|t| t.path.is_empty()) {
        if texture.slot != rules.item_icon_slot {
            return Err(format!(
                "{}: texture slot {} has no path and is not the item icon",
                plan.source, texture.slot
            ));
        }
        if !plan.textures.iter().any(|t| t.source.is_empty()) {
            plan.textures.push(pcx_gfx::LoadedTexture {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
                mipmaps: Vec::new(),
                source: String::new(),
            });
        }
    }
    let sequences: Vec<(String, [u32; 2])> = plan
        .mdl
        .sequences(&plan.bytes)
        .map_err(|e| format!("{}: {e}", plan.source))?
        .iter()
        .map(|s| (s.name.clone(), [s.interval_start, s.interval_end]))
        .collect();
    let normal = rules.look(Pointer::Normal).sequence;
    if !sequences.iter().any(|(name, _)| name == normal) {
        return Err(format!("{}: no {normal} sequence", plan.source));
    }
    let mut diagnostics = Vec::new();
    for (mode, name) in rules.modes() {
        match sequences.iter().find(|(have, _)| have == name) {
            Some((_, interval)) => {
                campaign_score_model::interval_plans(&plan.runtime, *interval, &plan.source)?;
            }
            None => diagnostics.push(CursorDiagnostic::MissingSequence {
                mode,
                name: name.to_owned(),
            }),
        }
    }
    Ok(Prepared {
        plan,
        sequences,
        diagnostics,
    })
}

/// Reads the rules and the model once per session, and spawns the cursor camera and model.
fn load(
    mut commands: Commands,
    mut cursor: ResMut<CampaignCursor>,
    inputs: Inputs,
    mut stores: Stores,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let witness = session.session_witness();
    if !same_session(cursor.witness.as_ref(), witness.as_ref()) {
        if let Some(model) = cursor.model.take() {
            commands.entity(model.root).try_despawn();
            commands.entity(model.camera).try_despawn();
            campaign_score_model::release_handles(
                &model.handles,
                &mut stores.meshes,
                &mut stores.materials,
                &mut stores.images,
            );
        }
        *cursor = CampaignCursor {
            witness: witness.clone(),
            rules: cursor.rules.take(),
            ..default()
        };
    }
    if cursor.attempted || !inputs.camera.active() {
        return;
    }
    let (Some(_), Some(context)) = (witness, session.context().cloned()) else {
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
    cursor.attempted = true;
    let race = host
        .objects()
        .local_race_preference_constant()
        .map_err(|error| error.to_string());
    let install = inputs
        .launcher
        .as_deref()
        .and_then(crate::launch_gui::selected_install);
    let built = (|| {
        let rules = match cursor.rules.take() {
            Some(rules) => rules,
            None => crate::menu_lighting::with_client_image(install.as_deref(), |source, image| {
                CursorRules::read(
                    image,
                    AssetIdentity {
                        source: source.into(),
                        edition: "selected campaign client".into(),
                        version: "reviewed Legacy 1.29.2.9231 cursor profile".into(),
                    },
                )
            })?,
        };
        let path = model_path(&reader, race?)?;
        let prepared = prepare(&reader, &path, &rules).map_err(|e| format!("{path}: {e}"));
        Ok::<_, String>((rules, prepared?))
    })();
    let (
        rules,
        Prepared {
            plan,
            sequences,
            diagnostics,
        },
    ) = match built {
        Ok(built) => built,
        Err(error) => {
            warn!("cursor unavailable, the system cursor stays: {error}");
            cursor.last_error = Some(error);
            return;
        }
    };
    for diagnostic in diagnostics {
        cursor.note(diagnostic);
    }
    let root = commands
        .spawn((
            Transform::default(),
            Visibility::Hidden,
            Name::new("Cursor model"),
        ))
        .id();
    let Stores {
        meshes,
        materials,
        images,
    } = &mut stores;
    let spawned = campaign_score_model::spawn_backgrounds_on(
        &mut commands,
        crate::mainmenu_scene::GeosetAssets {
            meshes: &mut *meshes,
            materials: &mut *materials,
            images: &mut *images,
        },
        root,
        vec![plan],
        CURSOR_LAYER,
    );
    let handles = match spawned {
        Ok(handles) => handles,
        Err(error) => {
            commands.entity(root).try_despawn();
            cursor.last_error = Some(error);
            return;
        }
    };
    let camera = commands
        .spawn((
            Camera3d::default(),
            Projection::Orthographic(OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical {
                    viewport_height: FRAME_H,
                },
                near: -DEPTH_RANGE,
                far: DEPTH_RANGE,
                ..OrthographicProjection::default_3d()
            }),
            Camera {
                order: CAMERA_ORDER,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            Tonemapping::None,
            DebandDither::Disabled,
            RenderLayers::layer(CURSOR_LAYER),
            Name::new("Cursor camera"),
        ))
        .id();
    cursor.rules = Some(rules);
    cursor.model = Some(Model {
        root,
        camera,
        sequences,
        handles,
    });
}

/// An entity the interface draws: a transparent container does not count.
type InterfaceHit = Or<(
    With<ImageNode>,
    With<Text>,
    With<Button>,
    With<BackgroundColor>,
)>;

#[derive(SystemParam)]
struct Hover<'w, 's> {
    map: Res<'w, HoverMap>,
    interface: Query<'w, 's, (), InterfaceHit>,
}

impl Hover<'_, '_> {
    fn over_interface(&self) -> bool {
        self.map
            .get(&PointerId::Mouse)
            .is_some_and(|hits| hits.keys().any(|&entity| self.interface.contains(entity)))
    }
}

/// The unit under `point`, by the click picker's rule: the nearest visible live unit whose
/// selection circle covers the ground point.
fn hovered_unit(
    objects: &ObjectHost,
    circles: &pcx_objects::editor::selection::SelectionCircles,
    point: [f32; 2],
) -> Result<Option<HandleId>, String> {
    let gate = match objects.vision_capability() {
        VisionCapability::Unadmitted => None,
        VisionCapability::Unavailable(reason) => return Err(reason.to_string()),
        VisionCapability::Ready { fog, mask } => {
            let local = objects
                .local_player_settings()
                .ok_or("shared vision needs a local player")?;
            Some((local.identity, fog, mask))
        }
    };
    let mut best: Option<(HandleId, f32)> = None;
    for (id, unit) in objects.executed_units() {
        let Some([x, y]) = unit.position else {
            continue;
        };
        if !unit.shown || unit.combat.as_ref().is_some_and(|c| c.dead) {
            continue;
        }
        if let Some((viewer, fog, mask)) = gate
            && !matches!(
                objects.vision_presented(viewer, [x, y], fog, mask),
                Ok(Some(pcx_play::playercolor_host::VisionState::Visible))
            )
        {
            continue;
        }
        let Ok(scale) = crate::campaign_actors::scalar(&unit.definition, "scale") else {
            continue;
        };
        let distance = ((x - point[0]).powi(2) + (y - point[1]).powi(2)).sqrt();
        if distance <= scale * circles.scale_factor * 0.5
            && best.is_none_or(|(_, nearest)| distance < nearest)
        {
            best = Some((id, distance));
        }
    }
    Ok(best.map(|(id, _)| id))
}

fn classify(
    mut cursor: ResMut<CampaignCursor>,
    input: Res<CampaignInput>,
    camera_input: Res<crate::campaign_camera_input::CameraInput>,
    picking: Picking,
    hover: Hover,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    cursor.position = None;
    cursor.facts = Facts::default();
    let Ok(window) = picking.windows.single() else {
        return;
    };
    let Some(at) = window.cursor_position().filter(|_| window.focused) else {
        return;
    };
    cursor.position = Some(model_position(at, window.size()));
    let witness = session.session_witness();
    let armed = if input.build.is_some() {
        Armed::Build
    } else {
        input.armed_kind(witness).map_or(Armed::Nothing, Armed::of)
    };
    let in_world = picking.game_input.open()
        && picking
            .cameras
            .single()
            .ok()
            .and_then(|(camera, ..)| camera.logical_viewport_rect())
            .is_some_and(|rect| rect.contains(at))
        && !hover.over_interface();
    let hovered = in_world
        .then(|| hovered(&mut cursor, &input, &picking, &mut session, at))
        .flatten();
    cursor.facts = Facts {
        edges: camera_input.edges,
        over_interface: !in_world,
        armed,
        hover: hovered,
    };
}

/// The unit under the pointer and the local player's relation to it. Failures are kept in
/// `last_error` and read as no unit.
fn hovered(
    cursor: &mut CampaignCursor,
    input: &CampaignInput,
    picking: &Picking,
    session: &mut MapScriptSession<MapStartupHost>,
    at: Vec2,
) -> Option<Hovered> {
    let (camera, transform, projection) = picking.cameras.single().ok()?;
    let circles = input.circles.as_ref()?;
    let terrain = &picking.ground.context.as_ref()?.terrain;
    let ray = camera.viewport_to_world(transform, at).ok()?;
    let point = ground_point(ray, terrain, projection.far())?;
    let objects = session.host()?.objects();
    let found = hovered_unit(objects, circles, point)
        .map_err(|error| cursor.last_error = Some(error))
        .ok()??;
    Some(Hovered {
        relation: objects.local_relation(found)?,
        selected: objects.selection().contains(&found),
    })
}

/// Poses the model for `look`: its sequence, and the tint on every geoset material.
fn apply(
    look: Look<'_>,
    sequences: &[(String, [u32; 2])],
    state: &mut ScoreBackgroundState,
    materials: &mut Assets<MenuMaterial>,
) -> Result<(), CursorDiagnostic> {
    let missing = || CursorDiagnostic::MissingSequence {
        mode: look.mode,
        name: look.sequence.to_owned(),
    };
    let (_, interval) = sequences
        .iter()
        .find(|(name, _)| name == look.sequence)
        .ok_or_else(missing)?;
    if state.interval != *interval {
        campaign_score_model::set_interval(state, *interval).map_err(|_| missing())?;
    }
    let [r, g, b] = look.tint;
    for (_, _, handle) in &state.bound {
        let Some(material) = materials.get(handle) else {
            continue;
        };
        let tint = Color::srgb_u8(r, g, b).with_alpha(material.base.base_color.alpha());
        if material.base.base_color != tint
            && let Some(mut material) = materials.get_mut(handle)
        {
            material.base.base_color = tint;
        }
    }
    Ok(())
}

fn present(
    mut cursor: ResMut<CampaignCursor>,
    camera: Res<CampaignCamera>,
    mut transforms: Query<(&mut Transform, &mut Visibility)>,
    mut states: Query<(&ChildOf, &mut ScoreBackgroundState)>,
    mut materials: ResMut<Assets<MenuMaterial>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let shown = cursor.model.is_some() && camera.active() && cursor.position.is_some();
    // The system cursor returns whenever the model is not drawn.
    for mut options in &mut windows {
        if options.visible == shown {
            options.visible = !shown;
        }
    }
    let Some(root) = cursor.model.as_ref().map(|model| model.root) else {
        return;
    };
    if let Ok((mut transform, mut visibility)) = transforms.get_mut(root) {
        let wanted = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if let Some(at) = cursor.position {
            transform.translation = at.extend(0.0);
        }
    }
    if !shown {
        return;
    }
    let (wanted, situation) = pointer(cursor.facts);
    if let Some(situation) = situation {
        cursor.note(CursorDiagnostic::Unestablished(situation));
    }
    let (Some(model), Some(rules)) = (cursor.model.as_ref(), cursor.rules.as_ref()) else {
        return;
    };
    let look = rules.look(wanted);
    if cursor.pointer == Some(wanted) && cursor.applied == Some((look.mode, look.tint)) {
        return;
    }
    let normal = rules.look(Pointer::Normal);
    let Some((_, mut state)) = states.iter_mut().find(|(parent, _)| parent.parent() == root)
    else {
        return;
    };
    let mut notes = Vec::new();
    if let Err(missing) = apply(look, &model.sequences, &mut state, &mut materials) {
        // The mode's sequence is missing: the normal cursor stands in, and it is reported.
        notes.push(missing);
        if let Err(normal_missing) = apply(normal, &model.sequences, &mut state, &mut materials) {
            notes.push(normal_missing);
        }
    }
    let applied = (look.mode, look.tint);
    for note in notes {
        cursor.note(note);
    }
    cursor.pointer = Some(wanted);
    cursor.applied = Some(applied);
}

#[cfg(test)]
#[path = "campaign_cursor_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "campaign_cursor_capture_tests.rs"]
mod capture_tests;
