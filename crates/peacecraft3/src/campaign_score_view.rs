//! Campaign score screen view. Spawns the selected `ScoreScreen.fdf` chrome through the
//! glue helpers and fills labels from `GlobalStrings` keys. Score rows are not drawn: their
//! counters are not qualified yet. Refusals go to diagnostics until source-authored refusal
//! UI exists; no game copy is invented for them.
use crate::campaign_score_model::{self, ScoreSceneHandles};
use crate::glue_menu::{self, GlueButton, GlueLabel};
use crate::mainmenu::{FRAME_H, FRAME_W};
use crate::menu_material::MenuMaterial;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use peacecraft3::campaign_score::{ActionOutcome, ScoreScreen, ScoreScreenState, press};
use peacecraft3::campaign_score_assets::ScoreScreenAssets;
use peacecraft3::campaign_score_layout::ROOT_FRAME;
use std::collections::HashSet;

/// The frame the score layout is authored against, as the glue screens use it.
const VIEWPORT: (f32, f32) = (FRAME_W, FRAME_H);

#[derive(Component)]
struct ScoreScreenRoot;

/// The score source owns this label; the normal menu cache must not fill it.
#[derive(Component)]
pub(crate) struct ScoreLabel(bool);

#[derive(Component)]
struct ScoreFonts(Vec<Handle<Font>>);

#[derive(SystemParam)]
struct ScoreStores<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<MenuMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    fonts: ResMut<'w, Assets<Font>>,
}

#[derive(SystemParam)]
struct ScoreTextAssets<'w, 's> {
    fonts: ResMut<'w, Assets<Font>>,
    nodes: Query<'w, 's, &'static mut Node>,
}

type ScoreRoots<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static crate::glue_menu::ArtSource>,
        Option<&'static ScoreSceneHandles>,
        Option<&'static ScoreFonts>,
    ),
    With<ScoreScreenRoot>,
>;

pub fn install(app: &mut App) {
    app.add_systems(
        Update,
        (
            rebuild.after(peacecraft3::campaign_score_assets::load_assets),
            handle_buttons
                .before(crate::button_actions::clear)
                .before(peacecraft3::campaign_runtime::CampaignTransitionSet),
            fill_labels,
        )
            .chain(),
    )
    .add_systems(Update, campaign_score_model::animate_backgrounds);
}

/// Every entity under the score screen roots, excluding the roots themselves.
fn owned_entities(
    roots: &Query<Entity, With<ScoreScreenRoot>>,
    children: &Query<&Children>,
) -> HashSet<Entity> {
    let mut owned = HashSet::new();
    let mut pending: Vec<Entity> = roots.iter().collect();
    while let Some(entity) = pending.pop() {
        if let Ok(kids) = children.get(entity) {
            for kid in kids.iter() {
                if owned.insert(kid) {
                    pending.push(kid);
                }
            }
        }
    }
    owned
}

fn rebuild(
    mut commands: Commands,
    state: Res<ScoreScreenState>,
    assets: Res<ScoreScreenAssets>,
    mut glue: ResMut<crate::glue_menu::GlueImages>,
    stores: ScoreStores,
    roots: ScoreRoots,
) {
    let ScoreStores {
        mut meshes,
        mut materials,
        mut images,
        mut fonts,
    } = stores;
    if !(state.is_changed() || assets.is_changed()) {
        return;
    }
    // Teardown: the root's children go with it; its handles and decodes are released here.
    for (entity, source, handles, faces) in &roots {
        if let Some(source) = source {
            glue.forget_source(&source.0);
        }
        if let Some(handles) = handles {
            campaign_score_model::release_handles(
                handles,
                &mut meshes,
                &mut materials,
                &mut images,
            );
        }
        if let Some(faces) = faces {
            for font in &faces.0 {
                fonts.remove(font.id());
            }
        }
        commands.entity(entity).despawn();
    }
    let Some(ScoreScreen::Refused {
        missing, context, ..
    }) = &state.0
    else {
        return;
    };
    for input in missing.iter() {
        warn!("score screen refused: {input}");
    }
    let Some(layout) = assets.layout_for(context) else {
        return;
    };
    let mut root = commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ScoreScreenRoot,
    ));
    let source = assets.source_for(context);
    if let Some(source) = &source {
        root.insert(crate::glue_menu::ArtSource(source.clone()));
    }
    let root = root.id();
    match glue_menu::control_items_for(layout.fdf(), ROOT_FRAME, VIEWPORT, |name| {
        if name == "ScoreScreenReplaySavedButton" {
            // No recording can reach the source's replay-saved state yet.
            return Ok(false);
        }
        if layout.fdf().frame(name).is_some_and(|f| f.kind == "TEXT")
            && layout.fdf().text_value(name).is_some()
            && let Err(error) = layout.rect(name, VIEWPORT)
        {
            warn!("score label {name} geometry refused: {error:?}");
            return Ok(false);
        }
        Ok(true)
    }) {
        Ok((items, unsupported)) => {
            for frame in unsupported {
                warn!("score screen control without a verified view: {frame}");
            }
            glue_menu::spawn(&mut commands, root, &items);
            commands.queue(move |world: &mut World| {
                let mut pending = vec![root];
                while let Some(entity) = pending.pop() {
                    if let Some(children) = world.get::<Children>(entity) {
                        pending.extend(children.iter());
                    }
                    let button = world.get::<GlueButton>(entity).is_some();
                    let label = world.get::<GlueLabel>(entity).is_some();
                    let mut node = world.entity_mut(entity);
                    node.remove::<crate::menu_screens::ScreenPosition>();
                    if button {
                        node.insert(crate::menu_screens::CampaignAction);
                    }
                    if label {
                        node.insert(ScoreLabel(false));
                    }
                }
            });
        }
        Err(e) => warn!("score screen layout refused: {e:?}"),
    }
    // The three authored backgrounds. A failed read or check refuses the backgrounds only.
    if let Some(source) = source {
        match campaign_score_model::authored_backgrounds(layout, VIEWPORT).and_then(|found| {
            found
                .iter()
                .map(|background| campaign_score_model::prepare(&source, background))
                .collect::<Result<Vec<_>, String>>()
        }) {
            Ok(plans) => {
                // Depth runs from the models' own extents, as the glue panel camera does.
                let (low, high) = plans
                    .iter()
                    .fold((f32::MAX, f32::MIN), |(low, high), plan| {
                        (
                            low.min(plan.model.minimum_extent[2]),
                            high.max(plan.model.maximum_extent[2]),
                        )
                    });
                commands.spawn((
                    Camera3d::default(),
                    Projection::Orthographic(OrthographicProjection {
                        scaling_mode: bevy::camera::ScalingMode::Fixed {
                            width: FRAME_W,
                            height: FRAME_H,
                        },
                        near: -high,
                        far: -low,
                        ..OrthographicProjection::default_3d()
                    }),
                    Transform::from_xyz(FRAME_W / 2.0, FRAME_H / 2.0, 0.0),
                    Camera {
                        order: 0,
                        clear_color: ClearColorConfig::None,
                        ..default()
                    },
                    bevy::core_pipeline::tonemapping::Tonemapping::None,
                    bevy::core_pipeline::tonemapping::DebandDither::Disabled,
                    RenderLayers::layer(campaign_score_model::SCORE_RENDER_LAYER),
                    ChildOf(root),
                ));
                match campaign_score_model::spawn_backgrounds(
                    &mut commands,
                    crate::mainmenu_scene::GeosetAssets {
                        meshes: &mut meshes,
                        materials: &mut materials,
                        images: &mut images,
                    },
                    root,
                    plans,
                ) {
                    Ok(handles) => {
                        commands.entity(root).insert(handles);
                    }
                    Err(e) => warn!("score backgrounds refused: {e}"),
                }
            }
            Err(e) => warn!("score backgrounds refused: {e}"),
        }
    }
}

/// Score controls consume their own presses; next-map admission precedes dismissal.
fn handle_buttons(
    activations: Res<crate::button_actions::ButtonActions>,
    buttons: Query<&GlueButton>,
    roots: Query<Entity, With<ScoreScreenRoot>>,
    children: Query<&Children>,
    mut state: ResMut<ScoreScreenState>,
    mut continuation: MessageWriter<peacecraft3::campaign_runtime::CampaignTransitionRequest>,
) {
    let owned = owned_entities(&roots, &children);
    for entity in &activations.entities {
        if !owned.contains(entity) {
            continue;
        }
        let Ok(button) = buttons.get(*entity) else {
            continue;
        };
        match press(&mut state, &button.frame) {
            Some(ActionOutcome::Continue) => {
                if let Some(request) = state.continuation() {
                    continuation.write(request);
                }
            }
            Some(ActionOutcome::ReplayUnsupported) => {
                warn!("score screen: Save Replay refused, no replay writer exists");
            }
            _ => {}
        }
    }
}

/// Writes labels that belong to the score screen. A key that does not resolve is logged
/// with its frame and key, and the label is left unchanged rather than filled with a guess.
fn fill_labels(
    mut commands: Commands,
    state: Res<ScoreScreenState>,
    assets: Res<ScoreScreenAssets>,
    roots: Query<Entity, With<ScoreScreenRoot>>,
    children: Query<&Children>,
    labels: Query<(Entity, &GlueLabel, &ChildOf, &ScoreLabel)>,
    text_assets: ScoreTextAssets,
) {
    if !labels.iter().any(|(_, _, _, label)| !label.0) {
        return;
    }
    let Ok(root) = roots.single() else {
        return;
    };
    let ScoreTextAssets {
        mut fonts,
        mut nodes,
    } = text_assets;
    let Some(ScoreScreen::Refused { context, .. }) = &state.0 else {
        return;
    };
    let Some(layout) = assets.layout_for(context) else {
        return;
    };
    let owned = owned_entities(&roots, &children);
    let pending: Vec<_> = labels
        .iter()
        .filter(|(entity, _, _, styled)| !styled.0 && owned.contains(entity))
        .collect();
    if pending.is_empty() {
        return;
    }
    let Some(source) = assets.source_for(context) else {
        return;
    };
    let skin = source
        .resolve_bounded(crate::mainmenu::SKIN_TABLE, 4 << 20)
        .map_err(|e| e.to_string())
        .and_then(|asset| String::from_utf8(asset.bytes).map_err(|e| e.to_string()));
    let mut faces = std::collections::BTreeMap::new();
    let mut remaining_font_bytes = 32 << 20;
    for (entity, label, parent, _) in pending {
        let result = (|| {
            let skin = skin.as_ref().map_err(Clone::clone)?;
            let value = layout.label(&label.frame).map_err(|e| format!("{e:?}"))?;
            let style = crate::menu_text::render_style(layout.fdf(), &label.frame)?;
            let binding =
                crate::mainmenu::menu_font_file(skin, &style.font).map_err(|e| format!("{e:?}"))?;
            let font = match faces.entry(binding.file.clone()) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    let bytes = source
                        .resolve_bounded(&binding.file, remaining_font_bytes)
                        .map_err(|e| e.to_string())?
                        .bytes;
                    remaining_font_bytes -= bytes.len() as u64;
                    if !crate::hud_render::font_registers(&bytes) {
                        return Err(format!("{}: font has no readable face", binding.file));
                    }
                    entry.insert(fonts.add(Font::from_bytes(bytes)))
                }
            };
            let replacement = crate::campaign_script_dialogs::label(
                &mut commands,
                parent.parent(),
                &value,
                &style,
                font,
            )?;
            commands.entity(replacement).insert((
                Name::new(label.frame.clone()),
                GlueLabel {
                    frame: label.frame.clone(),
                    button: label.button,
                },
                ScoreLabel(true),
            ));
            if let Ok(mut node) = nodes.get_mut(parent.parent()) {
                node.justify_content = match style.justify_v {
                    crate::menu_text::JustifyV::Top => JustifyContent::FlexStart,
                    crate::menu_text::JustifyV::Middle => JustifyContent::Center,
                    crate::menu_text::JustifyV::Bottom => JustifyContent::FlexEnd,
                };
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            warn!("score label {} refused: {error}", label.frame);
        }
        // The normal menu's FDF/font cache does not own this label.
        commands.entity(entity).despawn();
    }
    commands
        .entity(root)
        .insert(ScoreFonts(faces.into_values().collect()));
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod selected_tests {
    use super::*;
    use pcx_play::map_startup_host::MapStartupHost;
    use pcx_script::Value;
    use peacecraft3::{
        campaign_runtime::{CampaignRuntime, CampaignTransition},
        campaign_startup::CampaignStartup,
        map_requests::MapRequests,
        map_script_session::MapScriptSession,
    };
    use std::{sync::Arc, time::Duration};

    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL, MAP, ADDON and PCX_NATIVE_NEXT_CHAPTER_MAP"]
    fn selected_victory_continue_admits_and_starts_the_next_chapter() {
        check_selected_continue(0);
    }

    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL, MAP, ADDON and PCX_NATIVE_NEXT_CHAPTER_MAP"]
    fn selected_victory_continue_runs_the_next_chapters_opening_minute() {
        check_selected_continue(60);
    }

    fn check_selected_continue(seconds: usize) {
        let next = std::env::var("PCX_NATIVE_NEXT_CHAPTER_MAP")
            .expect("set PCX_NATIVE_NEXT_CHAPTER_MAP to the next selected campaign member");
        let mut app = crate::native_campaign::tests::started_app();
        app.insert_resource(crate::launch_gui::Launcher::test_source(
            "/missing-install-must-not-replace-the-admitted-source".into(),
            std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref() == Ok("roc"),
        ));
        peacecraft3::campaign_runtime::install(
            &mut app,
            CampaignRuntime::new(Duration::from_secs_f64(1.0 / 64.0), 8, 65536),
        );
        peacecraft3::campaign_score::install(&mut app);
        peacecraft3::campaign_score_assets::install(&mut app);
        app.init_resource::<crate::glue_menu::GlueImages>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<MenuMaterial>>()
            .init_resource::<Assets<Image>>();
        install(&mut app);
        let recipe = {
            let bindings = app
                .world()
                .resource::<MapRequests>()
                .bindings
                .as_ref()
                .unwrap();
            app.world()
                .resource::<peacecraft3::browser_configuration::SelectedMapSources>()
                .campaign_for_receipts(&bindings.receipts)
                .expect("native configuration pins a campaign recipe")
        };
        let old = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let context = Arc::clone(session.context().unwrap());
            let program = context.scripts.program();
            // Explicit victory stimulus exercises the selected BJ, not mission completion.
            session
                .call_function(
                    program.function_id("SetNextLevelBJ").unwrap(),
                    &[Value::Str(next.clone())],
                )
                .unwrap();
            let player = session
                .global("udg_AAAP_Arthas")
                .expect("select Human01, whose authored victory names this player");
            session
                .call_function(
                    program.function_id("CustomVictoryBJ").unwrap(),
                    &[player, Value::Bool(true), Value::Bool(true)],
                )
                .unwrap();
            session
                .call_function(program.function_id("CustomVictoryOkBJ").unwrap(), &[])
                .unwrap();
            context
        };
        app.update();
        let request = app
            .world()
            .resource::<ScoreScreenState>()
            .continuation()
            .expect("authored next-map request");
        assert_eq!(request.next_level, next);
        assert!(Arc::ptr_eq(&request.context, &old));
        let button = app
            .world_mut()
            .query::<(Entity, &GlueButton)>()
            .iter(app.world())
            .find(|(_, button)| button.frame == "ScoreScreenOkButton")
            .map(|(entity, _)| entity)
            .expect("selected score FDF must spawn Continue");
        app.world_mut()
            .trigger(bevy::ui_widgets::Activate { entity: button });
        app.update();
        assert!(
            app.world()
                .resource::<CampaignTransition>()
                .last_error
                .is_none(),
            "transition: {:?}",
            app.world().resource::<CampaignTransition>().last_error
        );
        let requests = app.world().resource::<MapRequests>();
        let admitted = requests.admission.shared_context().unwrap();
        assert!(!Arc::ptr_eq(&admitted, &old));
        let bindings = requests.bindings.as_ref().unwrap();
        let next_recipe = app
            .world()
            .resource::<peacecraft3::browser_configuration::SelectedMapSources>()
            .campaign_for_receipts(&bindings.receipts)
            .expect("Continue retains the admitted campaign generation");
        assert!(Arc::ptr_eq(&recipe, &next_recipe));
        let limits = bindings.limits;
        let selected = recipe
            .open_map(
                &bindings.catalog_map_bytes,
                &next,
                pcx_mpq::checked::ReadBudget {
                    max_archive_bytes: limits.max_map_file_bytes,
                    max_archive_table_bytes: limits.admission.scripts.max_archive_table_bytes,
                    max_raw_packed_bytes: limits.admission.scripts.max_raw_packed_bytes,
                    max_decoded_bytes: limits.admission.info_bytes,
                },
            )
            .unwrap();
        let ability = bindings
            .receipts
            .catalog
            .iter()
            .find(|receipt| {
                receipt
                    .path
                    .as_str()
                    .eq_ignore_ascii_case("Units/AbilityData.slk")
            })
            .expect("selected ability table receipt");
        assert!(
            ability.provenance.locator.to_ascii_lowercase().ends_with(
                &format!("{}\\Units\\AbilityData.slk", selected.selection.prefix)
                    .to_ascii_lowercase()
            ),
            "{}",
            ability.provenance.locator
        );
        assert_eq!(
            requests.bindings.as_ref().unwrap().receipts.map.path,
            pcx_assets::AssetPath::new(&next).unwrap()
        );
        assert!(
            app.world()
                .resource::<CampaignStartup>()
                .last_error
                .is_none(),
            "next startup: {:?}",
            app.world().resource::<CampaignStartup>().last_error
        );
        assert!(app.world().resource::<ScoreScreenState>().0.is_none());
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        assert!(
            session
                .context()
                .is_some_and(|ctx| Arc::ptr_eq(ctx, &admitted))
        );
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        for tick in 1..=seconds * 64 {
            let at = tick as f64 / 64.0;
            pcx_play::campaign_host::advance_tick(&mut session, at, 65536)
                .unwrap_or_else(|error| panic!("next chapter at {at:.6}s: {error}"));
        }
        if seconds > 0 {
            assert_eq!(
                session.host().unwrap().objects().timer_game_time(),
                Some(seconds as f64),
                "the script must actually advance through the requested opening interval"
            );
        }
    }
}

/// Production rebuild over two real admitted maps: a failing replacement removes the earlier
/// screen's roots and every asset they owned, and leaves unrelated assets in place.
#[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
mod lifecycle_tests {
    use super::*;
    use crate::binding::Reply;
    use crate::campaign_score_model::{ScoreSceneHandles, lifecycle_tests::model};
    use crate::campaign_startup_cases::script_timer_cases::game_exit::{
        QUIT_SCRIPT, click_quit, publish_with_assets, score_app,
    };
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::PrimitiveTopology;
    use std::collections::BTreeMap;

    const SCREEN: &str = "Frame \"FRAME\" \"ScoreScreenFrame\" {\n\tWidth 0.8,\n\tHeight 0.6,\n\
        Frame \"GLUEBUTTON\" \"ScoreScreenOkButton\" {\n\tWidth 0.2,\n\tHeight 0.04,\n\
        \tSetPoint BOTTOMLEFT,\"ScoreScreenFrame\",BOTTOMLEFT,0.1,0.1,\n}\n}\n\
        Frame \"MODEL\" \"ScoreScreenBackground\" {\n\tBackgroundArt \"UI\\Glues\\Test\\Test.mdl\",\n\
        \tWidth 0.3,\n\tHeight 0.4,\n\tSetPoint BOTTOMLEFT,\"ScoreScreenFrame\",BOTTOMLEFT,0.1,0.2,\n\
        \tLayerStyle \"NOSHADING\",\n}\n";

    /// One 1x1 uncompressed 32-bit TGA.
    fn tga() -> Vec<u8> {
        let mut bytes = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 32, 0];
        bytes.extend_from_slice(&[0, 0, 255, 255]);
        bytes
    }

    /// Selected score UI with one authored background: its model and texture are present.
    fn assets_with_background() -> BTreeMap<String, Reply> {
        BTreeMap::from([
            (
                r"UI\FrameDef\Glue\ScoreScreen.fdf".to_owned(),
                Reply::Bytes(SCREEN.as_bytes().to_vec()),
            ),
            (
                r"UI\FrameDef\GlobalStrings.fdf".to_owned(),
                Reply::Bytes(b"OK_WHITE \"OK\",\n".to_vec()),
            ),
            (r"UI\Glues\Test\Test.mdx".to_owned(), Reply::Bytes(model())),
            (r"Textures\glow.tga".to_owned(), Reply::Bytes(tga())),
        ])
    }

    fn fresh_mesh() -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD,
        )
    }

    #[test]
    fn a_failing_replacement_removes_the_previous_roots_and_their_owned_assets() {
        let mut app = score_app(assets_with_background());
        let unrelated_entity = app.world_mut().spawn_empty().id();
        app.init_resource::<crate::glue_menu::GlueImages>()
            .init_resource::<crate::button_actions::ButtonActions>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<MenuMaterial>>()
            .init_resource::<Assets<Image>>();
        app.init_resource::<Assets<Font>>();
        install(&mut app);
        app.update();
        assert!(app.world().get_entity(unrelated_entity).is_ok());
        click_quit(&mut app);
        app.update();
        let Some(ScoreScreen::Refused { context, .. }) =
            &app.world().resource::<ScoreScreenState>().0
        else {
            panic!("quit did not request a score screen");
        };
        assert!(
            app.world()
                .resource::<ScoreScreenAssets>()
                .layout_for(context)
                .is_some(),
            "selected score layout was not loaded"
        );
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ScoreScreenRoot>>()
                .iter(app.world())
                .count(),
            1,
            "one score root is present"
        );
        let mut buttons = app.world_mut().query::<(
            &GlueButton,
            Option<&crate::menu_screens::CampaignAction>,
            Option<&crate::menu_screens::ScreenPosition>,
        )>();
        let (button, owned_action, generic_position) = buttons.single(app.world()).unwrap();
        assert_eq!(button.frame, "ScoreScreenOkButton");
        assert!(
            owned_action.is_some(),
            "score presses bypass menu navigation"
        );
        assert!(generic_position.is_none(), "the score FDF owns placement");

        let (root, owned) = {
            let mut roots = app
                .world_mut()
                .query_filtered::<(Entity, &ScoreSceneHandles), With<ScoreScreenRoot>>();
            let (root, handles) = roots.single(app.world()).expect("one visible score root");
            (root, handles.clone())
        };
        app.update();
        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ScoreScreenRoot>>()
                .single(app.world())
                .unwrap(),
            root,
            "an idle score frame must not rebuild its controls"
        );
        let owned_font = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(Vec::new()));
        app.world_mut()
            .entity_mut(root)
            .insert(ScoreFonts(vec![owned_font.clone()]));
        let unrelated_font = app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::from_bytes(Vec::new()));
        assert!(
            !owned.meshes.is_empty() && !owned.materials.is_empty() && !owned.images.is_empty()
        );
        let unrelated = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(fresh_mesh());

        // Replacement B has the same script but no selected score UI files, so its refusal
        // finds no layout.
        publish_with_assets(&mut app, QUIT_SCRIPT, 2, BTreeMap::new(), |_| {});
        app.update();
        click_quit(&mut app);
        app.update();
        assert!(app.world().get_entity(unrelated_entity).is_ok());

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<ScoreScreenRoot>>()
                .iter(app.world())
                .count(),
            0,
            "A's root is not visible under B"
        );
        assert!(app.world().get_entity(root).is_err());
        assert!(
            app.world()
                .resource::<Assets<Font>>()
                .get(&owned_font)
                .is_none()
        );
        assert!(
            app.world()
                .resource::<Assets<Font>>()
                .get(&unrelated_font)
                .is_some()
        );
        for mesh in &owned.meshes {
            assert!(app.world().resource::<Assets<Mesh>>().get(mesh).is_none());
        }
        for material in &owned.materials {
            assert!(
                app.world()
                    .resource::<Assets<MenuMaterial>>()
                    .get(material)
                    .is_none()
            );
        }
        for image in &owned.images {
            assert!(app.world().resource::<Assets<Image>>().get(image).is_none());
        }
        assert!(
            app.world()
                .resource::<Assets<Mesh>>()
                .get(&unrelated)
                .is_some()
        );
    }
}
