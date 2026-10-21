//! Learned command icons under the receipt-bound, authored console parent.
use crate::campaign_hud::upper_buttons::{image_sampler, read, skin_file, text};
use bevy::{
    ecs::system::SystemParam, picking::events::PointerPress, prelude::*, ui_widgets::Button,
};
use pcx_play::{
    ability_profile, command_layout::Rules, map_startup_host::MapStartupHost,
    playercolor_host::ObjectHost,
};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources,
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_presentation::{CampaignInterface, CampaignPresentationSet},
    map_binding::AssetReceipt,
    map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

const SKIN: &str = r"UI\war3skins.txt";
const CONSOLE: &str = r"UI\FrameDef\UI\ConsoleUI.fdf";
// Application read/cache ceilings, not authored icon dimensions or slot counts.
const MAX_IMAGES: usize = 64;
const MAX_RETAINED: usize = 32 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    TargetUnit,
    Immediate,
}

#[derive(Component, Clone)]
pub(crate) struct ButtonOrder {
    pub unit: HandleId,
    pub kind: Kind,
    pub session: Weak<()>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Icon {
    kind: Kind,
    data: ability_profile::Button,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct View {
    unit: HandleId,
    icons: Vec<Icon>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Header {
    unit: HandleId,
    owner: HandleId,
    local: i32,
    shown: bool,
    dead: Option<bool>,
}
struct Signature {
    header: Header,
    abilities: Vec<([u8; 4], i32, bool)>,
}

fn header(objects: &ObjectHost) -> Result<Option<Header>, String> {
    let Some(&unit) = objects.selection().first() else {
        return Ok(None);
    };
    let record = objects
        .executed_unit(unit)
        .map_err(|error| error.to_string())?;
    let local = objects
        .local_player_number()
        .ok_or("command card has no local player")?;
    if objects
        .player_settings(record.owner)
        .map_err(|error| error.to_string())?
        .number
        != local
    {
        return Ok(None);
    }
    Ok(Some(Header {
        unit,
        owner: record.owner,
        local,
        shown: record.shown,
        dead: record.combat.as_ref().map(|combat| combat.dead),
    }))
}

fn unchanged(objects: &ObjectHost, previous: Option<&Signature>) -> Result<bool, String> {
    let current = header(objects)?;
    let (Some(current), Some(previous)) = (current, previous) else {
        return Ok(current.is_none() && previous.is_none());
    };
    if current != previous.header {
        return Ok(false);
    }
    let record = objects
        .executed_unit(current.unit)
        .map_err(|error| error.to_string())?;
    let mut old = previous.abilities.iter();
    for (ability, rank) in record
        .abilities
        .iter()
        .flat_map(|abilities| abilities.levels())
    {
        let available = objects
            .unit_ability_available(current.unit, ability)
            .map_err(|error| error.to_string())?;
        if old.next() != Some(&(ability, rank, available)) {
            return Ok(false);
        }
    }
    Ok(old.next().is_none())
}

fn snapshot(objects: &ObjectHost) -> Result<Option<Signature>, String> {
    let Some(header) = header(objects)? else {
        return Ok(None);
    };
    let record = objects
        .executed_unit(header.unit)
        .map_err(|error| error.to_string())?;
    let abilities = record
        .abilities
        .iter()
        .flat_map(|abilities| abilities.levels())
        .map(|(ability, rank)| {
            objects
                .unit_ability_available(header.unit, ability)
                .map(|available| (ability, rank, available))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(Signature { header, abilities }))
}

struct Source {
    rules: Rules,
    console: String,
    images: HashMap<String, Handle<Image>>,
    failed: HashMap<String, String>,
    retained: usize,
    receipts: Vec<AssetReceipt>,
}

#[derive(Resource, Default)]
pub(crate) struct CommandCard {
    witness: Option<Weak<()>>,
    source: Option<Source>,
    attempted: bool,
    root: Option<Entity>,
    parent: Option<Entity>,
    view: Option<View>,
    signature: Option<Signature>,
    pending_pointer: bool,
    pub last_error: Option<String>,
}

impl CommandCard {
    pub fn receipt_count(&self) -> usize {
        self.source
            .as_ref()
            .map_or(0, |source| source.receipts.len())
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CommandCardSet;

#[derive(SystemParam)]
struct Inputs<'w, 's> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    camera: Res<'w, CampaignCamera>,
    prepared: Res<'w, crate::hud_render::HudPrepare>,
    launcher: Option<Res<'w, crate::launch_gui::Launcher>>,
    frames: Query<'w, 's, (Entity, &'static crate::hud_render::HudSourceAttachment)>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CommandCard>()
        .add_observer(capture_press)
        .add_systems(
            Update,
            (capture, project)
                .chain()
                .in_set(CommandCardSet)
                .after(CampaignCameraSet)
                .after(crate::campaign_hud::CampaignHudSet)
                .before(CampaignPresentationSet),
        );
}

fn capture_press(
    event: On<PointerPress>,
    buttons: Query<(), With<ButtonOrder>>,
    mut card: ResMut<CommandCard>,
    presentation: Option<Res<peacecraft3::campaign_presentation::CampaignPresentation>>,
) {
    if buttons.contains(event.entity) && crate::button_actions::input_allowed(presentation) {
        card.pending_pointer = true;
    }
}

fn capture(mut card: ResMut<CommandCard>, mut hud: ResMut<crate::campaign_hud::CampaignHud>) {
    hud.pointer_consumed |= std::mem::take(&mut card.pending_pointer);
}

fn wanted(objects: &ObjectHost) -> Result<Option<View>, String> {
    let Some(&unit) = objects.selection().first() else {
        return Ok(None);
    };
    let record = objects
        .executed_unit(unit)
        .map_err(|error| error.to_string())?;
    if objects
        .player_settings(record.owner)
        .map_err(|error| error.to_string())?
        .number
        != objects
            .local_player_number()
            .ok_or("command card has no local player")?
    {
        return Ok(None);
    }
    let mut icons = Vec::new();
    for (kind, command) in [
        (Kind::TargetUnit, objects.holy_light_command(unit)),
        (Kind::Immediate, objects.water_elemental_command(unit)),
    ] {
        if let Some(command) = command.map_err(|error| error.to_string())? {
            let data = objects
                .ability_button(unit, command.ability)
                .map_err(|error| error.to_string())?
                .ok_or("command has no learned presentation")?;
            pcx_assets::AssetPath::new(&data.art).map_err(|error| error.to_string())?;
            if icons
                .iter()
                .any(|icon: &Icon| icon.data.position == data.position)
            {
                return Err("implemented commands collide at the same authored Buttonpos".into());
            }
            icons.push(Icon { kind, data });
        }
    }
    Ok(Some(View { unit, icons }))
}

fn reset(
    commands: &mut Commands,
    card: &mut CommandCard,
    images: &mut Assets<Image>,
    witness: Option<Weak<()>>,
) {
    if let Some(root) = card.root {
        commands.entity(root).try_despawn();
    }
    if let Some(source) = card.source.take() {
        for image in source.images.values() {
            images.remove(image.id());
        }
    }
    *card = CommandCard {
        witness,
        ..default()
    };
}

fn image(
    reader: &pcx_assets::Resolver,
    source: &mut Source,
    path: &str,
    images: &mut Assets<Image>,
) -> Result<Handle<Image>, String> {
    if let Some(image) = source.images.get(path) {
        return Ok(image.clone());
    }
    if let Some(error) = source.failed.get(path) {
        return Err(error.clone());
    }
    if source.images.len() + source.failed.len() >= MAX_IMAGES {
        return Err("command image cache budget exceeded".into());
    }
    let result: Result<Handle<Image>, String> = (|| {
        let mut receipts = Vec::new();
        let bytes = read(reader, path, 4 << 20, &mut receipts)?;
        let remaining = MAX_RETAINED
            .checked_sub(source.retained)
            .ok_or("command image aggregate budget")?;
        let texture = pcx_gfx::load_image_bounded(&bytes, path, remaining.min(16 << 20))
            .map_err(|error| error.to_string())?;
        let retained = source
            .retained
            .checked_add(texture.rgba_bytes().ok_or("command image size")?)
            .filter(|bytes| *bytes <= MAX_RETAINED)
            .ok_or("command image aggregate budget")?;
        let mut decoded = pcx_gfx::viewer::rgba_image(&texture);
        decoded.sampler =
            bevy::image::ImageSampler::Descriptor(image_sampler(source.rules.icon_repeat));
        let handle = images.add(decoded);
        source.images.insert(path.into(), handle.clone());
        source.retained = retained;
        source.receipts.extend(receipts);
        Ok(handle)
    })();
    if let Err(error) = &result {
        source.failed.insert(path.into(), error.clone());
    }
    result
}

fn paint(
    commands: &mut Commands,
    root: Entity,
    source: &Source,
    view: &View,
    witness: &Weak<()>,
) -> Result<(), String> {
    let mut nodes = Vec::new();
    for icon in &view.icons {
        let rect = source
            .rules
            .slot(icon.data.position[0], icon.data.position[1])?;
        let image = source
            .images
            .get(&icon.data.art)
            .ok_or("command icon is not decoded")?
            .clone();
        nodes.push((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(rect.x / source.rules.viewport[0] * 100.),
                bottom: Val::Percent(rect.y / source.rules.viewport[1] * 100.),
                width: Val::Percent(rect.width / source.rules.viewport[0] * 100.),
                height: Val::Percent(rect.height / source.rules.viewport[1] * 100.),
                ..default()
            },
            image,
            icon.kind,
        ));
    }
    commands.entity(root).despawn_children();
    for (node, image, kind) in nodes {
        commands.entity(root).with_children(|parent| {
            parent.spawn((
                node,
                ImageNode::new(image),
                Button,
                ButtonOrder {
                    unit: view.unit,
                    kind,
                    session: witness.clone(),
                },
            ));
        });
    }
    Ok(())
}

fn root_node(display: Display) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.),
        height: Val::Percent(100.),
        display,
        ..default()
    }
}

fn project(
    mut commands: Commands,
    mut card: ResMut<CommandCard>,
    inputs: Inputs,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut images: ResMut<Assets<Image>>,
    nodes: Query<&Node>,
) {
    let witness = session.session_witness();
    if !same_session(card.witness.as_ref(), witness.as_ref()) {
        reset(&mut commands, &mut card, &mut images, witness.clone());
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
    if !inputs.prepared.mounted_for(&context, bindings.revision) {
        return;
    }
    let Some(reader) = inputs.sources.for_receipts(&bindings.receipts) else {
        return;
    };
    let Some(host) = session.host() else { return };
    if !card.attempted {
        card.attempted = true;
        let result = (|| {
            let install = inputs
                .launcher
                .as_deref()
                .and_then(crate::launch_gui::selected_install);
            let rules =
                crate::menu_lighting::with_client_image(install.as_deref(), |label, image| {
                    Rules::reviewed_legacy(
                        image,
                        pcx_play::campaign_inputs::client_identity(image, label),
                    )
                })?;
            let mut receipts = Vec::new();
            let skin = crate::menu_skin::SkinTable::parse(&text(&reader, SKIN, &mut receipts)?)
                .map_err(|error| error.to_string())?;
            let sections = crate::campaign_hud::race_sections(
                &skin,
                host.objects()
                    .local_race_preference_constant()
                    .map_err(|error| error.to_string())?,
            )?;
            let console = skin_file(&skin, &sections, &rules.console_key)?;
            text(&reader, CONSOLE, &mut receipts)?;
            Ok::<_, String>(Source {
                rules,
                console,
                receipts,
                images: HashMap::new(),
                failed: HashMap::new(),
                retained: 0,
            })
        })();
        match result {
            Ok(source) => card.source = Some(source),
            Err(error) => {
                card.last_error = Some(error);
                return;
            }
        }
    }
    let Some(mut source) = card.source.take() else {
        return;
    };
    let result = (|| {
        let mut parents = inputs
            .frames
            .iter()
            .filter(|(_, frame)| {
                frame.frame == source.console
                    && frame.source_member == CONSOLE
                    && frame.binding_revision == bindings.revision
                    && frame.receipt_path == bindings.receipts.map.path.as_str()
            })
            .map(|(entity, _)| entity);
        let Some(parent) = parents.next() else {
            return Err("command card awaits its mounted console parent".into());
        };
        if parents.next().is_some() {
            return Err("command card console parent is ambiguous".into());
        }
        let mut rebuilt = false;
        if card.parent != Some(parent) || card.root.is_none_or(|root| nodes.get(root).is_err()) {
            if let Some(root) = card.root {
                commands.entity(root).try_despawn();
            }
            let root = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        ..default()
                    },
                    CampaignInterface,
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(parent).add_child(root);
            card.parent = Some(parent);
            card.root = Some(root);
            rebuilt = true;
        }
        // Tables are immutable in a published session; avoid rebuilding metadata every frame.
        let changed = !unchanged(host.objects(), card.signature.as_ref())?;
        if changed {
            card.view = wanted(host.objects())?;
            card.signature = snapshot(host.objects())?;
        }
        let root = card.root.expect("command root");
        let visible = inputs.camera.active()
            && card
                .view
                .as_ref()
                .is_some_and(|view| !view.icons.is_empty());
        let display = if visible {
            Display::Flex
        } else {
            Display::None
        };
        if nodes.get(root).map_or(true, |node| node.display != display) {
            commands.entity(root).insert(root_node(display));
        }
        if rebuilt || changed || card.last_error.is_some() {
            if let Some(view) = &card.view {
                for icon in &view.icons {
                    image(&reader, &mut source, &icon.data.art, &mut images)?;
                }
                paint(&mut commands, root, &source, view, &witness)?;
            } else {
                commands.entity(root).despawn_children();
            }
        }
        Ok::<_, String>(())
    })();
    if result.is_err()
        && let Some(root) = card.root
        && nodes
            .get(root)
            .map_or(true, |node| node.display != Display::None)
    {
        commands.entity(root).insert(root_node(Display::None));
    }
    card.last_error = result.err();
    card.source = Some(source);
}

fn same_session(a: Option<&Weak<()>>, b: Option<&Weak<()>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::NormalizedRenderTarget,
        ecs::system::RunSystemOnce,
        picking::{
            backend::HitData,
            events::Pointer,
            pointer::{Location, PointerButton, PointerId},
        },
    };
    use pcx_play::scripts::AssetIdentity;

    fn selected_icons() -> (Source, View, pcx_assets::Resolver) {
        use pcx_assets::{Addon, ArchiveLayer, AssetMode, Behaviour, Build, GameMode, Resolver};
        use pcx_objects::editor::defaults::{DefaultTables, Family};
        let dir = std::path::PathBuf::from(
            std::env::var_os("PCX_NATIVE_CHAPTER_INSTALL").expect("set PCX_NATIVE_CHAPTER_INSTALL"),
        );
        let path = std::env::var("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE");
        let bytes = std::fs::read(&path).unwrap();
        let identity = pcx_play::campaign_inputs::client_identity(&bytes, &path);
        let rules = Rules::reviewed_legacy(&bytes, identity.clone()).unwrap();
        let tables = DefaultTables::open(&dir, identity.version).unwrap();
        let mut icons = Vec::new();
        for (ability, kind) in [(*b"AHhb", Kind::TargetUnit), (*b"AHwe", Kind::Immediate)] {
            let row = pcx_objects::fourcc_text(ability);
            let object = tables.object(Family::Ability, &row).unwrap();
            let art = object.profile("Art");
            let [art] = art.as_slice() else {
                panic!("{row}: expected one source Art")
            };
            let position = object.profile("Buttonpos");
            let [position] = position.as_slice() else {
                panic!("{row}: expected one source Buttonpos")
            };
            let position: Vec<usize> = position
                .text
                .split(',')
                .map(|v| v.trim().parse().unwrap())
                .collect();
            icons.push(Icon {
                kind,
                data: ability_profile::Button {
                    ability,
                    rank: 1,
                    art: art.text.to_owned(),
                    position: position.try_into().unwrap(),
                },
            });
        }
        let reader = Resolver::new(
            GameMode {
                build: Build::V1_29b,
                addon: Addon::NoAddon,
                assets: AssetMode::Vanilla,
                behaviour: Behaviour::Vanilla,
            },
            None,
            vec![ArchiveLayer {
                priority: 0,
                source: Box::new(pcx_play::install_source::InstallSource::open(&dir).unwrap()),
            }],
        )
        .unwrap();
        (
            Source {
                rules,
                console: "synthetic component parent".into(),
                images: HashMap::new(),
                failed: HashMap::new(),
                retained: 0,
                receipts: Vec::new(),
            },
            View {
                unit: HandleId::new(std::num::NonZeroU64::new(7).unwrap()),
                icons,
            },
            reader,
        )
    }

    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL and PCX_WAR3_EXE; reads local original icons"]
    fn selected_original_icons_decode_once_and_missing_assets_are_cached() {
        let (mut source, view, reader) = selected_icons();
        let mut images = Assets::<Image>::default();
        for icon in &view.icons {
            let handle = image(&reader, &mut source, &icon.data.art, &mut images).unwrap();
            let receipt_count = source.receipts.len();
            let retained = source.retained;
            let decoded = images.get(&handle).unwrap();
            assert!(decoded.width() > 0 && decoded.height() > 0);
            assert!(decoded.data.as_ref().is_some_and(|data| !data.is_empty()));
            assert_eq!(
                image(&reader, &mut source, &icon.data.art, &mut images)
                    .unwrap()
                    .id(),
                handle.id()
            );
            assert_eq!(source.receipts.len(), receipt_count);
            assert_eq!(source.retained, retained);
            assert!(
                source
                    .receipts
                    .iter()
                    .any(|receipt| receipt.path
                        == pcx_assets::AssetPath::new(&icon.data.art).unwrap())
            );
        }
        assert_eq!(images.len(), view.icons.len());
        assert!(source.retained > 0 && source.retained <= MAX_RETAINED);
        let missing = "PCX-Fixture-Missing-Command-Icon.blp";
        let first = image(&reader, &mut source, missing, &mut images).unwrap_err();
        assert_eq!(
            image(&reader, &mut source, missing, &mut images).unwrap_err(),
            first
        );
        assert_eq!(source.failed.len(), 1);
    }

    #[test]
    #[ignore = "requires live desktop, selected install/executable, and PCX_COMMAND_CARD_ABILITY=AHhb|AHwe"]
    fn selected_original_command_icon_native_component() {
        let (mut source, mut view, reader) = selected_icons();
        let ability = std::env::var("PCX_COMMAND_CARD_ABILITY").expect("choose a fixture ability");
        assert!(matches!(ability.as_str(), "AHhb" | "AHwe"));
        view.icons
            .retain(|icon| icon.data.ability.as_slice() == ability.as_bytes());
        assert_eq!(view.icons.len(), 1);
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "PeaceCraft command card fixture".into(),
                        resolution: (1280_u32, 960_u32).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(bevy::winit::WinitPlugin {
                    run_on_any_thread: true,
                })
                .disable::<bevy::audio::AudioPlugin>(),
        );
        app.world_mut().spawn(Camera2d);
        {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            for icon in &view.icons {
                image(&reader, &mut source, &icon.data.art, &mut images).unwrap();
            }
        }
        let root = app.world_mut().spawn(root_node(Display::Flex)).id();
        let session = Arc::new(());
        let mut queue = bevy::ecs::world::CommandQueue::default();
        paint(
            &mut Commands::new(&mut queue, app.world()),
            root,
            &source,
            &view,
            &Arc::downgrade(&session),
        )
        .unwrap();
        queue.apply(app.world_mut());
        // Watchdog only; screenshots and exact geometry checks establish the visual result.
        app.add_systems(Update, |time: Res<Time<Real>>| {
            assert!(time.elapsed_secs() < 45.)
        });
        assert!(matches!(app.run(), AppExit::Success));
    }

    #[test]
    #[ignore = "requires PCX_WAR3_EXE for the selected command geometry; images are synthetic"]
    fn selected_client_card_preflights_replacement_and_releases_only_owned_assets() {
        let path = std::env::var("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE");
        let rules = Rules::reviewed_legacy(
            &std::fs::read(&path).unwrap(),
            AssetIdentity {
                source: path,
                edition: "selected local client".into(),
                version: "reviewed image".into(),
            },
        )
        .unwrap();
        let mut images = Assets::<Image>::default();
        let owned = images.add(Image::default());
        let unrelated = images.add(Image::default());
        let source = Source {
            rules,
            console: "synthetic parent".into(),
            images: HashMap::from([("fixture".into(), owned.clone())]),
            failed: HashMap::new(),
            retained: 0,
            receipts: Vec::new(),
        };
        let mut world = World::new();
        let root = world.spawn(Node::default()).id();
        let session = Arc::new(());
        let witness = Arc::downgrade(&session);
        let mut view = View {
            unit: HandleId::new(std::num::NonZeroU64::new(7).unwrap()),
            icons: vec![Icon {
                kind: Kind::TargetUnit,
                data: ability_profile::Button {
                    ability: *b"TEST",
                    rank: 1,
                    art: "fixture".into(),
                    position: [0, 0],
                },
            }],
        };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        paint(
            &mut Commands::new(&mut queue, &world),
            root,
            &source,
            &view,
            &witness,
        )
        .unwrap();
        queue.apply(&mut world);
        let child = world.get::<Children>(root).unwrap()[0];
        assert_eq!(world.get::<ButtonOrder>(child).unwrap().unit, view.unit);
        let rect = source.rules.slot(0, 0).unwrap();
        assert_eq!(
            world.get::<Node>(child).unwrap().width,
            Val::Percent(rect.width / source.rules.viewport[0] * 100.)
        );
        view.icons.push(Icon {
            kind: Kind::Immediate,
            data: ability_profile::Button {
                ability: *b"FAIL",
                rank: 1,
                art: "fixture".into(),
                position: [source.rules.columns, 0],
            },
        });
        assert!(
            paint(
                &mut Commands::new(&mut queue, &world),
                root,
                &source,
                &view,
                &witness
            )
            .is_err()
        );
        queue.apply(&mut world);
        assert_eq!(world.get::<Children>(root).unwrap()[0], child);
        assert_eq!(world.get::<Children>(root).unwrap().len(), 1);
        let mut card = CommandCard {
            root: Some(root),
            source: Some(source),
            witness: Some(witness),
            ..default()
        };
        reset(
            &mut Commands::new(&mut queue, &world),
            &mut card,
            &mut images,
            None,
        );
        queue.apply(&mut world);
        assert!(world.get_entity(root).is_err());
        assert!(world.get_entity(child).is_err());
        assert!(images.get(&owned).is_none());
        assert!(images.get(&unrelated).is_some());
        assert!(card.root.is_none() && card.source.is_none() && card.witness.is_none());
    }

    #[test]
    fn session_identity_is_not_interchangeable_with_another_live_session() {
        let first = Arc::new(());
        let second = Arc::new(());
        let first_witness = Arc::downgrade(&first);
        assert!(same_session(
            Some(&first_witness),
            Some(&Arc::downgrade(&first))
        ));
        assert!(!same_session(
            Some(&first_witness),
            Some(&Arc::downgrade(&second))
        ));
        assert!(!same_session(Some(&first_witness), None));
        drop(first);
        assert!(!same_session(
            Some(&first_witness),
            Some(&Arc::downgrade(&second))
        ));
        assert!(same_session(None, None));
    }

    #[test]
    fn command_press_consumes_world_pointer_once_without_consuming_unrelated_presses() {
        let mut app = App::new();
        app.init_resource::<CommandCard>()
            .init_resource::<crate::campaign_hud::CampaignHud>()
            .add_observer(capture_press);
        let session = Arc::new(());
        let button = app
            .world_mut()
            .spawn(ButtonOrder {
                unit: HandleId::new(std::num::NonZeroU64::new(7).unwrap()),
                kind: Kind::TargetUnit,
                session: Arc::downgrade(&session),
            })
            .id();
        let unrelated = app.world_mut().spawn_empty().id();
        let press = |entity| PointerPress {
            entity,
            pointer: Pointer::new(
                PointerId::Mouse,
                Location {
                    target: NormalizedRenderTarget::None {
                        width: 1,
                        height: 1,
                    },
                    position: Vec2::ZERO,
                },
            ),
            button: PointerButton::Primary,
            hit: HitData::new(entity, 0.0, None, None),
            count: 1,
        };
        app.world_mut().trigger(press(unrelated));
        app.world_mut().run_system_once(capture).unwrap();
        assert!(
            !app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
        app.world_mut().trigger(press(button));
        app.world_mut().run_system_once(capture).unwrap();
        assert!(
            app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
        app.world_mut()
            .resource_mut::<crate::campaign_hud::CampaignHud>()
            .pointer_consumed = false;
        app.world_mut().run_system_once(capture).unwrap();
        assert!(
            !app.world()
                .resource::<crate::campaign_hud::CampaignHud>()
                .pointer_consumed
        );
    }
}
