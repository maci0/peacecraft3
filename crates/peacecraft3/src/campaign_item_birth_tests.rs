//! Ground-item publication keeps unknown visibility refused and does not show a hidden newborn.
use super::{ActorQueries, DrawAssets, TeamPalette, UnitActors, project_ground_items};
use bevy::prelude::*;
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource, ObjectKind, SlkId};
use pcx_play::playercolor_host::ExecutedItem;
use pcx_script::HandleId;
use pcx_sim::world::{Definition, FieldValue, ObjectField, Source as ItemSource};
use std::collections::BTreeMap;
use std::num::NonZeroU64;

#[derive(Resource)]
struct Birth {
    items: Vec<(HandleId, ExecutedItem)>,
    reader: pcx_assets::Resolver,
    terrain: pcx_map::Terrain,
    palette: TeamPalette,
    parameters: pcx_play::menu_lighting::Parameters,
}

fn definition() -> Definition {
    let mut definition = Definition {
        rawcode: *b"I001",
        base_rawcode: "I001".into(),
        kind: ObjectKind::Item,
        fields: BTreeMap::new(),
        modifications: Vec::new(),
    };
    for (id, column, value, kind) in [
        (*b"f001", "file", "Models/Synthetic.mdl", FieldKind::Token),
        (*b"f002", "scale", "1", FieldKind::Real),
        (*b"f003", "colorR", "1", FieldKind::Int),
        (*b"f004", "colorG", "1", FieldKind::Int),
        (*b"f005", "colorB", "1", FieldKind::Int),
    ] {
        definition.fields.insert(
            id,
            ObjectField {
                metadata: Some(FieldDef {
                    fourcc: id,
                    from: SlkId::UnitMetaData,
                    field: column.into(),
                    slk: "ItemData".into(),
                    index: -1,
                    declared: if kind == FieldKind::Token {
                        "model"
                    } else {
                        "real"
                    }
                    .into(),
                    kind,
                    category: "art".into(),
                    source: FieldSource::Column {
                        slk: SlkId::ItemData,
                        column: column.into(),
                    },
                    measured: true,
                }),
                value: Ok(FieldValue::Cell(value.into())),
            },
        );
    }
    definition
}

fn item(id: u64, shown: Option<bool>) -> (HandleId, ExecutedItem) {
    (
        HandleId::new(NonZeroU64::new(id).unwrap()),
        ExecutedItem {
            definition: definition(),
            source: ItemSource {
                install: "synthetic".into(),
                edition: "test".into(),
                map: "fixture".into(),
            },
            x: 64.,
            y: 64.,
            held_by: None,
            shown,
            invulnerable: None,
            droppable: None,
            drop_on_death: None,
            claimed: false,
            consumed: false,
        },
    )
}

fn widen_extents(bytes: &mut [u8]) {
    let mut at = 4usize;
    while at + 8 <= bytes.len() {
        let tag = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        if tag == b"MODL" && size >= 0x158 + 24 {
            for (index, value) in [-1f32, -1., -1., 1., 1., 1.].into_iter().enumerate() {
                let offset = at + 8 + 0x158 + index * 4;
                bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        at += 8 + size;
    }
}

fn project(
    mut commands: Commands,
    mut state: ResMut<UnitActors>,
    mut assets: DrawAssets,
    mut queries: ActorQueries,
    birth: Res<Birth>,
) {
    project_ground_items(
        &mut commands,
        &mut state,
        &mut assets,
        &mut queries,
        &super::doodads::Input {
            reader: &birth.reader,
            terrain: &birth.terrain,
            palette: &birth.palette,
            parameters: &birth.parameters,
            now: 0.1,
            sun: None,
        },
        "test",
        birth.items.iter().map(|(id, item)| (*id, item)),
    );
}

#[test]
fn hidden_ground_items_publish_no_root_and_unknown_visibility_still_refuses() {
    let (mut bytes, _) = crate::mainmenu_scene::particle_tests::synthetic(|_, _| {});
    widen_extents(&mut bytes);
    let fixture = crate::campaign_startup_cases::fixture("");
    let reader = fixture.resolver_with_assets(BTreeMap::from([
        (
            "Models/Synthetic.mdx".into(),
            crate::binding::Reply::Bytes(bytes),
        ),
        (
            "Synthetic/Shared.blp".into(),
            crate::binding::Reply::Bytes(super::sky::synthetic_sky::sky_texture()),
        ),
        (
            "Other.blp".into(),
            crate::binding::Reply::Bytes(super::sky::synthetic_sky::sky_texture()),
        ),
    ]));
    let mut app = App::new();
    app.init_resource::<UnitActors>()
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<Image>>()
        .init_resource::<Assets<crate::menu_material::MenuMaterial>>()
        .init_resource::<Assets<crate::mainmenu_scene::ParticleMaterial>>()
        .insert_resource(Birth {
            items: vec![item(1, None)],
            reader,
            terrain: pcx_map::Terrain::from_bytes(crate::fixtures::terrain_bytes(2, 2)).unwrap(),
            palette: TeamPalette {
                prefixes: ["unused-colour".into(), "unused-glow".into()],
                count: 1,
            },
            parameters: crate::menu_lighting::tests::parameters(),
        })
        .add_systems(Update, project);
    let unknown = HandleId::new(NonZeroU64::new(1).unwrap());
    let hidden = HandleId::new(NonZeroU64::new(2).unwrap());
    let shown = HandleId::new(NonZeroU64::new(3).unwrap());
    app.update();
    {
        let state = app.world().resource::<UnitActors>();
        assert_eq!(
            state.last_error.as_deref(),
            Some("item visibility rules missing")
        );
        assert!(!state.item_animations.contains_key(&unknown));
        assert!(!state.actors.contains_key(&unknown));
    }
    app.world_mut().resource_mut::<Birth>().items = vec![item(2, Some(false))];
    app.world_mut().resource_mut::<UnitActors>().last_error = None;
    app.update();
    {
        let state = app.world().resource::<UnitActors>();
        assert!(state.last_error.is_none(), "{:?}", state.last_error);
        assert!(state.item_animations.contains_key(&hidden));
        assert!(!state.actors.contains_key(&hidden));
    }
    let hidden_roots = {
        let mut query = app.world_mut().query::<&Visibility>();
        query.iter(app.world()).count()
    };
    assert_eq!(hidden_roots, 0);
    app.world_mut().resource_mut::<Birth>().items = vec![item(3, Some(true))];
    app.update();
    let root = {
        let state = app.world().resource::<UnitActors>();
        assert!(state.last_error.is_none(), "{:?}", state.last_error);
        state.actors[&shown].root
    };
    assert_eq!(
        *app.world().get::<Visibility>(root).unwrap(),
        Visibility::Inherited
    );
}
