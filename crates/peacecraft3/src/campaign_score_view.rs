//! Campaign score screen view. Spawns the selected `ScoreScreen.fdf` chrome through the
//! glue helpers and fills labels from `GlobalStrings` keys. Score rows are not drawn: their
//! counters are not qualified yet. Refusals go to diagnostics until source-authored refusal
//! UI exists; no game copy is invented for them.
use crate::campaign_score_model::{self, ScoreSceneHandles};
use crate::glue_menu::{self, GlueButton, GlueLabel};
use crate::mainmenu::{FRAME_H, FRAME_W};
use crate::menu_material::MenuMaterial;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use peacecraft3::campaign_score::{ActionOutcome, ScoreScreen, ScoreScreenState, press};
use peacecraft3::campaign_score_assets::ScoreScreenAssets;
use peacecraft3::campaign_score_layout::ROOT_FRAME;
use std::collections::HashSet;

/// The frame the score layout is authored against, as the glue screens use it.
const VIEWPORT: (f32, f32) = (FRAME_W, FRAME_H);

#[derive(Component)]
struct ScoreScreenRoot;

pub fn install(app: &mut App) {
    app.add_systems(
        Update,
        (
            rebuild.after(peacecraft3::campaign_score_assets::load_assets),
            handle_buttons.before(crate::button_actions::clear),
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<MenuMaterial>>,
    mut images: ResMut<Assets<Image>>,
    roots: Query<
        (
            Entity,
            Option<&crate::glue_menu::ArtSource>,
            Option<&ScoreSceneHandles>,
        ),
        With<ScoreScreenRoot>,
    >,
) {
    if !(state.is_changed() || assets.is_changed()) {
        return;
    }
    // Teardown: the root's children go with it; its handles and decodes are released here.
    for (entity, source, handles) in &roots {
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
    match glue_menu::control_items_for(layout.fdf(), ROOT_FRAME, VIEWPORT, |_| Ok(true)) {
        Ok((items, unsupported)) => {
            for frame in unsupported {
                warn!("score screen control without a verified view: {frame}");
            }
            glue_menu::spawn(&mut commands, root, &items);
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

/// Applies the score buttons pressed this frame. OK dismisses the presentation; Save Replay
/// is refused, since no replay writer exists. Presses on buttons outside this screen are
/// ignored.
fn handle_buttons(
    activations: Res<crate::button_actions::ButtonActions>,
    buttons: Query<&GlueButton>,
    roots: Query<Entity, With<ScoreScreenRoot>>,
    children: Query<&Children>,
    mut state: ResMut<ScoreScreenState>,
) {
    let owned = owned_entities(&roots, &children);
    for entity in &activations.entities {
        if !owned.contains(entity) {
            continue;
        }
        let Ok(button) = buttons.get(*entity) else {
            continue;
        };
        if let Some(ActionOutcome::ReplayUnsupported) = press(&mut state, &button.frame) {
            warn!("score screen: Save Replay refused, no replay writer exists");
        }
    }
}

/// Writes labels that belong to the score screen. A key that does not resolve is logged
/// with its frame and key, and the label is left unchanged rather than filled with a guess.
fn fill_labels(
    state: Res<ScoreScreenState>,
    assets: Res<ScoreScreenAssets>,
    roots: Query<Entity, With<ScoreScreenRoot>>,
    children: Query<&Children>,
    mut labels: Query<(Entity, &GlueLabel, &mut Text)>,
) {
    let Some(ScoreScreen::Refused { context, .. }) = &state.0 else {
        return;
    };
    let Some(layout) = assets.layout_for(context) else {
        return;
    };
    let owned = owned_entities(&roots, &children);
    for (entity, label, mut text) in &mut labels {
        if !owned.contains(&entity) {
            continue;
        }
        match layout.label(&label.frame) {
            Ok(value) => {
                if text.0 != value {
                    text.0 = value;
                }
            }
            Err(e) => warn_once!("score label {} unresolved: {e:?}", label.frame),
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

    const SCREEN: &str = "Frame \"FRAME\" \"ScoreScreenFrame\" {\n\tWidth 0.8,\n\tHeight 0.6,\n}\n\
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
        install(&mut app);
        app.update();
        assert!(app.world().get_entity(unrelated_entity).is_ok());
        click_quit(&mut app);
        app.update();

        let (root, owned) = {
            let mut roots = app
                .world_mut()
                .query_filtered::<(Entity, &ScoreSceneHandles), With<ScoreScreenRoot>>();
            let (root, handles) = roots.single(app.world()).expect("one visible score root");
            (root, handles.clone())
        };
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
