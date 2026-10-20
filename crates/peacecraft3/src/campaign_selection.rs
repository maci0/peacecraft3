//! Selection circles under the local player's selected units.
//!
//! `UI\MiscData.txt` `[SelectionCircle]` (read by [`crate::campaign_input`])
//! supplies everything: the circle size is the unit's UnitUI `scale` times
//! `ScaleFactor`, the texture is the first band whose `Size##` holds that size
//! (the last band takes the rest), and the ARGB tint is `ColorFriend`,
//! `ColorNeutral` or `ColorEnemy` by the local player's relation to the owner.
//! Recreation choices, unqualified against the original: the size is the
//! circle's diameter, the circle is a flat alpha-blended quad at the ground
//! height under the unit (slopes can cut through it), and the texture name takes
//! a `.blp` suffix.
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

use bevy::{camera::visibility::RenderLayers, ecs::system::SystemParam, prelude::*};
use pcx_play::{map_startup_host::MapStartupHost, playercolor_host::Relation};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources, campaign_camera::CampaignCameraSet,
    map_ground::GroundPublication, map_requests::MapRequests, map_script_session::MapScriptSession,
};

const TEXTURE_SUFFIX: &str = ".blp";
/// Read and decode limit for one circle texture.
const MAX_TEXTURE_BYTES: usize = 4 << 20;
/// Pulls the coplanar circle in front of the terrain it lies on.
const DEPTH_BIAS: f32 = 64.0;

#[derive(Component)]
struct SelectionCircle(HandleId);

#[derive(Resource, Default)]
pub(crate) struct CampaignSelection {
    session: Option<Weak<()>>,
    quad: Option<Handle<Mesh>>,
    materials: BTreeMap<(String, [u8; 4]), Handle<StandardMaterial>>,
    pub last_error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignSelection>()
        .add_systems(Update, draw.after(CampaignCameraSet));
}

/// Texture and ARGB tint for a circle of `size` seen with `relation`.
fn circle_look(
    circles: &pcx_objects::editor::selection::SelectionCircles,
    size: f32,
    relation: Relation,
) -> Option<(String, [u8; 4])> {
    let band = circles
        .bands
        .iter()
        .find(|band| band.max_size.is_none_or(|max| size <= max))?;
    let colour = match relation {
        Relation::Friend => circles.friend,
        Relation::Neutral => circles.neutral,
        Relation::Enemy => circles.enemy,
    };
    Some((format!("{}{TEXTURE_SUFFIX}", band.texture), colour))
}

#[derive(SystemParam)]
struct Inputs<'w> {
    input: Res<'w, crate::campaign_input::CampaignInput>,
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    ground: Res<'w, GroundPublication>,
}

#[derive(SystemParam)]
struct DrawAssets<'w> {
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}

type Drawn = (
    Entity,
    &'static SelectionCircle,
    &'static mut Transform,
    &'static mut MeshMaterial3d<StandardMaterial>,
);

fn draw(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    inputs: Inputs,
    mut state: ResMut<CampaignSelection>,
    mut assets: DrawAssets,
    mut drawn: Query<Drawn>,
) {
    let Inputs {
        input,
        requests,
        sources,
        ground,
    } = inputs;
    let DrawAssets {
        meshes,
        materials,
        images,
    } = &mut assets;
    let witness = session.session_witness();
    if !match (&state.session, &witness) {
        (Some(old), Some(new)) => Weak::ptr_eq(old, new),
        (None, None) => true,
        _ => false,
    } {
        state.session = witness;
        state.materials.clear();
        state.last_error = None;
        for (entity, ..) in &drawn {
            commands.entity(entity).despawn();
        }
        return;
    }
    let wanted = wanted_circles(&mut session, &input, &ground);
    let quad = state
        .quad
        .get_or_insert_with(|| meshes.add(Plane3d::default().mesh().size(1.0, 1.0)))
        .clone();
    let reader = requests
        .bindings
        .as_ref()
        .filter(|bindings| {
            requests
                .admission
                .shared_context()
                .as_ref()
                .zip(session.context())
                .is_some_and(|(admitted, active)| Arc::ptr_eq(admitted, active))
                && requests
                    .published_receipts
                    .as_ref()
                    .is_some_and(|published| Arc::ptr_eq(published, &bindings.receipts))
        })
        .and_then(|bindings| sources.for_receipts(&bindings.receipts));
    let mut placed = BTreeMap::new();
    for (unit, transform, look) in wanted {
        let material = match state.materials.get(&look) {
            Some(material) => material.clone(),
            None => {
                let Some(reader) = reader.as_ref() else {
                    continue;
                };
                match load_material(reader, &look, images, materials) {
                    Ok(material) => {
                        state.materials.insert(look.clone(), material.clone());
                        material
                    }
                    Err(error) => {
                        state.last_error = Some(error);
                        continue;
                    }
                }
            }
        };
        placed.insert(unit, (transform, material));
    }
    for (entity, circle, mut transform, mut material) in &mut drawn {
        match placed.remove(&circle.0) {
            Some((next, handle)) => {
                *transform = next;
                material.0 = handle;
            }
            None => commands.entity(entity).despawn(),
        }
    }
    for (unit, (transform, material)) in placed {
        commands.spawn((
            SelectionCircle(unit),
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            transform,
            RenderLayers::layer(peacecraft3::campaign_camera::RENDER_LAYER),
        ));
    }
}

type Wanted = Vec<(HandleId, Transform, (String, [u8; 4]))>;

fn wanted_circles(
    session: &mut MapScriptSession<MapStartupHost>,
    input: &crate::campaign_input::CampaignInput,
    ground: &GroundPublication,
) -> Wanted {
    if !ground
        .context
        .as_ref()
        .zip(session.context())
        .is_some_and(|(ground, active)| Arc::ptr_eq(ground, active))
    {
        return Vec::new();
    }
    let (Some(circles), Some(context), Some(host)) = (
        input.circles.as_ref(),
        ground.context.as_ref(),
        session.host(),
    ) else {
        return Vec::new();
    };
    let objects = host.objects();
    objects
        .selection()
        .iter()
        .filter_map(|&id| {
            let unit = objects.executed_unit(id).ok()?;
            if !unit.shown || unit.combat.as_ref().is_some_and(|c| c.dead) {
                return None;
            }
            let [x, y] = unit.position?;
            let z = pcx_tera::world::sample_world_z(&context.terrain, x, y)?;
            let scale = crate::campaign_actors::scalar(&unit.definition, "scale").ok()?;
            let size = scale * circles.scale_factor;
            let look = circle_look(circles, size, objects.local_relation(id)?)?;
            let transform = Transform::from_xyz(x, z, -y).with_scale(Vec3::new(size, 1.0, size));
            transform.is_finite().then_some((id, transform, look))
        })
        .collect()
}

fn load_material(
    reader: &pcx_assets::Resolver,
    (path, [a, r, g, b]): &(String, [u8; 4]),
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Result<Handle<StandardMaterial>, String> {
    let raw = reader
        .resolve_bounded(path, MAX_TEXTURE_BYTES as u64)
        .map_err(|e| format!("{path}: {e}"))?;
    let texture = pcx_gfx::load_image_bounded(&raw.bytes, path, MAX_TEXTURE_BYTES)
        .map_err(|e| e.to_string())?;
    let image = images.add(pcx_gfx::viewer::rgba_image(&texture));
    Ok(materials.add(StandardMaterial {
        base_color: Color::srgba_u8(*r, *g, *b, *a),
        base_color_texture: Some(image),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        depth_bias: DEPTH_BIAS,
        cull_mode: None,
        ..default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_objects::editor::selection::{CircleBand, SelectionCircles};

    #[test]
    fn clearing_session_discards_materials_from_the_previous_source() {
        let witness = std::sync::Arc::new(());
        let mut state = CampaignSelection {
            session: Some(std::sync::Arc::downgrade(&witness)),
            last_error: Some("old source".into()),
            ..default()
        };
        state
            .materials
            .insert(("old.blp".into(), [0; 4]), Handle::default());
        let mut app = App::new();
        app.insert_resource(state)
            .init_resource::<MapScriptSession<MapStartupHost>>()
            .init_resource::<crate::campaign_input::CampaignInput>()
            .init_resource::<MapRequests>()
            .init_resource::<SelectedMapSources>()
            .init_resource::<GroundPublication>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Image>>()
            .add_systems(Update, draw);
        app.update();
        let state = app.world().resource::<CampaignSelection>();
        assert!(state.session.is_none());
        assert!(state.materials.is_empty());
        assert!(state.last_error.is_none());
    }

    #[test]
    fn size_bands_and_relations_pick_texture_and_tint() {
        let band = |max_size, texture: &str| CircleBand {
            max_size,
            texture: texture.into(),
            dotted: String::new(),
        };
        let circles = SelectionCircles {
            scale_factor: 100.0,
            bands: vec![band(Some(100.0), "small"), band(None, "large")],
            friend: [1, 2, 3, 4],
            neutral: [5, 6, 7, 8],
            enemy: [9, 10, 11, 12],
        };
        assert_eq!(
            circle_look(&circles, 100.0, Relation::Friend),
            Some(("small.blp".into(), [1, 2, 3, 4]))
        );
        assert_eq!(
            circle_look(&circles, 100.5, Relation::Enemy),
            Some(("large.blp".into(), [9, 10, 11, 12]))
        );
        assert_eq!(
            circle_look(&circles, 10.0, Relation::Neutral).unwrap().1,
            [5, 6, 7, 8]
        );
    }
}
