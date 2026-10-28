//! Production projection with real PRE2 simulation and synthetic source members.
use super::*;
use crate::campaign_actors::CampaignSceneCamera;
use bevy::mesh::VertexAttributeValues;
use pcx_play::{
    campaign_placement::{DoodadAppearance, PlacedDoodad},
    scripts::AssetIdentity,
    unit_model::{Animation, Model},
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

use crate::campaign_actors::sky::synthetic_sky as art;
#[derive(Resource)]
struct Source {
    row: PlacedDoodad,
    reader: pcx_assets::Resolver,
    terrain: pcx_map::Terrain,
    palette: TeamPalette,
    parameters: pcx_play::menu_lighting::Parameters,
    now: f64,
}
fn draw(
    mut commands: Commands,
    mut state: ResMut<UnitActors>,
    mut assets: DrawAssets,
    mut queries: ActorQueries,
    source: Res<Source>,
) {
    project(
        &mut commands,
        &mut state,
        &mut assets,
        &mut queries,
        Input {
            reader: &source.reader,
            terrain: &source.terrain,
            palette: &source.palette,
            parameters: &source.parameters,
            now: source.now,
            sun: None,
            view: None,
        },
        std::iter::once(&source.row),
    );
}
fn advance(
    mut state: ResMut<UnitActors>,
    mut assets: DrawAssets,
    mut queries: ActorQueries,
    source: Res<Source>,
) {
    advance_particles(
        &mut state,
        &mut assets,
        &mut queries,
        std::iter::once(&source.row),
        source.now,
        &source.parameters,
    );
}
fn app() -> App {
    let (mut bytes, _) = crate::mainmenu_scene::particle_tests::synthetic(|_, _| {});
    let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
    if parsed.payload(&bytes, pcx_mdl::tag::MODL).is_none() {
        bytes.extend(b"MODL");
        bytes.extend(372u32.to_le_bytes());
        bytes.extend([0; 372]);
    }
    let model = Arc::new(
        Model::read(
            AssetIdentity {
                edition: "synthetic".into(),
                version: "nonstock".into(),
                source: "deferred-doodad".into(),
            },
            "synthetic-particles.mdx".into(),
            bytes.into(),
            1 << 20,
        )
        .unwrap(),
    );
    let row = PlacedDoodad {
        source: pcx_map::Doodad {
            type_id: *b"Zx91",
            variation: 0,
            position: [0.; 3],
            angle: 0.,
            scale: [1.; 3],
            flags: 2,
            life: 255,
            creation_number: 71,
        },
        shown: true,
        point_selection: true,
        appearance: Ok(DoodadAppearance {
            tint: [255.; 3],
            animation: Animation::new(model, 0.).unwrap(),
        }),
    };
    let fixture = crate::campaign_startup_cases::fixture("");
    let reader = fixture.resolver_with_assets(BTreeMap::from([
        (
            "Synthetic/Shared.blp".into(),
            crate::binding::Reply::Bytes(art::sky_texture()),
        ),
        (
            "Other.blp".into(),
            crate::binding::Reply::Bytes(art::sky_texture()),
        ),
    ]));
    let terrain = pcx_map::Terrain::from_bytes(crate::fixtures::terrain_bytes(2, 2)).unwrap();
    let mut app = App::new();
    app.init_resource::<UnitActors>()
        .init_resource::<crate::campaign_actors::DecodedTextures>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<crate::menu_material::MenuMaterial>>()
        .init_resource::<Assets<crate::mainmenu_scene::ParticleMaterial>>()
        .insert_resource(Source {
            row,
            reader,
            terrain,
            palette: TeamPalette {
                prefixes: ["unused-colour".into(), "unused-glow".into()],
                count: 1,
            },
            parameters: crate::menu_lighting::tests::parameters(),
            now: 0.1,
        })
        .add_systems(Update, (draw, advance).chain());
    app.world_mut().spawn((
        CampaignSceneCamera,
        Camera3d::default(),
        Transform::default(),
        Projection::default(),
    ));
    app
}
#[test]
fn deferred_spawn_publishes_before_particle_advance_then_catches_up() {
    let mut app = app();
    app.update();
    let state = app.world().resource::<UnitActors>();
    assert!(state.last_error.is_none(), "{:?}", state.last_error);
    assert_eq!(
        state.doodads.actors[&71].particle_clock,
        Duration::from_secs_f64(0.1)
    );
    assert!(
        app.world()
            .resource::<Assets<Mesh>>()
            .iter()
            .any(|(_, mesh)| match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                Some(VertexAttributeValues::Float32x3(vertices)) =>
                    vertices.iter().flatten().any(|v| *v != 0.),
                _ => false,
            }),
        "the source emission must create actual particle vertices"
    );
    app.world_mut().resource_mut::<Source>().now = 0.2;
    app.update();
    let state = app.world().resource::<UnitActors>();
    assert!(state.last_error.is_none(), "{:?}", state.last_error);
    assert_eq!(
        state.doodads.actors[&71].particle_clock,
        Duration::from_secs_f64(0.2)
    );
    let entity = app
        .world_mut()
        .query_filtered::<Entity, With<MeshMaterial3d<crate::mainmenu_scene::ParticleMaterial>>>()
        .iter(app.world())
        .next()
        .unwrap();
    app.world_mut().despawn(entity);
    app.world_mut().resource_mut::<Source>().now = 0.3;
    app.update();
    let state = app.world().resource::<UnitActors>();
    assert!(
        state
            .last_error
            .as_deref()
            .is_some_and(|e| e.contains("ownership was lost")),
        "real loss must still refuse"
    );
    assert_eq!(
        state.doodads.actors[&71].particle_clock,
        Duration::from_secs_f64(0.2)
    );
}

#[test]
fn hidden_doodad_is_hidden_on_publication_and_can_be_shown_later() {
    let mut app = app();
    app.world_mut().resource_mut::<Source>().row.shown = false;
    app.update();
    let root = app.world().resource::<UnitActors>().doodads.actors[&71].root;
    assert_eq!(
        *app.world().get::<Visibility>(root).unwrap(),
        Visibility::Hidden
    );
    assert!(app.world().resource::<UnitActors>().last_error.is_none());
    app.world_mut().resource_mut::<Source>().row.shown = true;
    app.world_mut().resource_mut::<Source>().now = 0.2;
    app.update();
    assert_eq!(
        *app.world().get::<Visibility>(root).unwrap(),
        Visibility::Inherited
    );
    assert!(app.world().resource::<UnitActors>().last_error.is_none());
}
