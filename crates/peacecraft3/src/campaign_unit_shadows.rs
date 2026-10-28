//! Ground shadows under live units and buildings from their UnitUI shadow images.
//!
//! `unitShadow` places a soft image from `shadowW`, `shadowH`, `shadowX` and
//! `shadowY`; `buildingShadow` stamps a cell mask at the building (see
//! [`pcx_play::unit_shadow`]). The baked `war3map.shd` mask carries neither.
//! Recreation choices: each image is clipped onto admitted ground triangles like
//! a selection circle and drawn unlit with the colour of `FogOfWar/ShadowImageColor`
//! (its alpha multiplies the image alpha), overlapping shadows blend instead of
//! taking the maximum, and no shadow is drawn for hidden or dead units, under fog
//! or at a flyer's height offset. See `docs/re-notes/campaign-unit-shadows.md`.
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

use bevy::{camera::visibility::RenderLayers, mesh::VertexAttributeValues, prelude::*};
use pcx_play::{
    map_startup_host::MapStartupHost,
    unit_shadow::{self, Footprint, Shadows},
};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources, campaign_camera::CampaignCameraSet,
    map_ground::GroundPublication, map_requests::MapRequests, map_script_session::MapScriptSession,
};

const MISC_DATA: &str = "UI\\MiscData.txt";
/// Read limits for one shadow image and for `MiscData.txt`; not authored values.
const MAX_TEXTURE_BYTES: usize = 1 << 20;
const MAX_MISC_DATA_BYTES: usize = 1 << 20;
/// Pulls the coplanar shadow in front of its terrain, behind ubersplats (32).
const DEPTH_BIAS: f32 = 16.0;
/// World units of one `war3map.shd` cell: four cells per terrain tile edge.
const SHADOW_CELL: f32 = pcx_tera::world::TILE_XY / pcx_map::shadows::SHADOW_CELLS_PER_TILE as f32;

#[derive(Component)]
pub(crate) struct UnitShadow;

/// The two images one definition can name, drawn independently.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Layer {
    Unit,
    Building,
}

struct Loaded {
    material: Handle<StandardMaterial>,
    texels: [u32; 2],
}

type Texture = Result<Loaded, String>;
type Placement = (Transform, Footprint, Handle<StandardMaterial>);

struct Drawn {
    entity: Entity,
    placement: Placement,
    mesh: Handle<Mesh>,
}

#[derive(Resource, Default)]
pub(crate) struct CampaignUnitShadows {
    session: Option<Weak<()>>,
    /// Images per unit, keyed by its definition revision.
    shadows: BTreeMap<HandleId, (u64, Arc<Shadows>)>,
    tint: Option<Result<[u8; 4], String>>,
    textures: BTreeMap<String, Texture>,
    drawn: BTreeMap<(HandleId, Layer), Drawn>,
    pub last_error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignUnitShadows>()
        .add_systems(Update, draw.after(CampaignCameraSet));
}

fn load_tint(reader: &pcx_assets::Resolver) -> Result<[u8; 4], String> {
    let raw = reader
        .resolve_bounded(MISC_DATA, MAX_MISC_DATA_BYTES as u64)
        .map_err(|e| format!("{MISC_DATA}: {e}"))?;
    pcx_objects::editor::fog_boundary::ShadowColor::read(&raw.bytes, MAX_MISC_DATA_BYTES)
        .map(|color| color.0)
}

/// Loads an image keeping only its alpha: the shadow colour is `tint`'s.
fn load_texture(
    reader: &pcx_assets::Resolver,
    path: &str,
    [r, g, b, a]: [u8; 4],
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
) -> Texture {
    let raw = reader
        .resolve_bounded(path, MAX_TEXTURE_BYTES as u64)
        .map_err(|e| format!("{path}: {e}"))?;
    let mut texture = pcx_gfx::load_image_bounded(&raw.bytes, path, MAX_TEXTURE_BYTES)
        .map_err(|e| e.to_string())?;
    for level in std::iter::once(&mut texture.rgba).chain(&mut texture.mipmaps) {
        for pixel in level.as_chunks_mut::<4>().0 {
            pixel[..3].fill(u8::MAX);
        }
    }
    let texels = [texture.width, texture.height];
    let image = images.add(pcx_gfx::viewer::rgba_image(&texture));
    Ok(Loaded {
        material: materials.add(StandardMaterial {
            base_color: Color::srgba_u8(r, g, b, a),
            base_color_texture: Some(image),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            depth_bias: DEPTH_BIAS,
            cull_mode: None,
            ..default()
        }),
        texels,
    })
}

/// Rewrites the UVs of a mesh projected from `footprint`'s enclosing square so
/// they address the footprint's image instead.
fn remap_uvs(mesh: &mut Mesh, footprint: &Footprint) -> Result<(), String> {
    let ([cx, cy], side) = footprint.enclosing_square();
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0)
    else {
        return Err("projected shadow has no UVs".into());
    };
    for uv in uvs {
        // Inverts the projection's square UVs: U grows east, V grows south.
        let world = [
            cx - side / 2.0 + uv[0] * side,
            cy + side / 2.0 - uv[1] * side,
        ];
        *uv = footprint.uv(world);
    }
    Ok(())
}

fn draw(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    ground: Res<GroundPublication>,
    (requests, sources): (Res<MapRequests>, Res<SelectedMapSources>),
    mut state: ResMut<CampaignUnitShadows>,
    assets: crate::campaign_selection::DrawAssets,
) {
    let crate::campaign_selection::DrawAssets {
        mut meshes,
        mut materials,
        mut images,
    } = assets;
    let witness = session.session_witness();
    if !match (&state.session, &witness) {
        (Some(old), Some(new)) => Weak::ptr_eq(old, new),
        (None, None) => true,
        _ => false,
    } {
        for drawn in std::mem::take(&mut state.drawn).into_values() {
            commands.entity(drawn.entity).despawn();
            meshes.remove(drawn.mesh.id());
        }
        *state = CampaignUnitShadows {
            session: witness,
            ..default()
        };
        return;
    }
    let Some(context) = ground.context.clone().filter(|ground| {
        session
            .context()
            .is_some_and(|active| Arc::ptr_eq(ground, active))
    }) else {
        return;
    };
    let reader = crate::campaign_selection::session_reader(&requests, &sources, &session);
    let Some(host) = session.host() else {
        return;
    };
    let state = &mut *state;
    let mut wanted = Vec::new();
    for (unit_id, unit) in host.objects().executed_units() {
        if !unit.shown || unit.combat.as_ref().is_some_and(|c| c.dead) {
            continue;
        }
        let shadows = match state.shadows.get(&unit_id) {
            Some((revision, shadows)) if *revision == unit.definition_revision => shadows.clone(),
            _ => match unit_shadow::read(&unit.definition) {
                Ok(shadows) => {
                    let shadows = Arc::new(shadows);
                    state
                        .shadows
                        .insert(unit_id, (unit.definition_revision, shadows.clone()));
                    shadows
                }
                Err(error) => {
                    state.last_error = Some(format!("shadow for unit {unit_id:?}: {error}"));
                    continue;
                }
            },
        };
        if let Some(position) = unit.position
            && *shadows != Shadows::default()
        {
            wanted.push((unit_id, shadows, position));
        }
    }
    state
        .shadows
        .retain(|unit, _| host.objects().executed_unit(*unit).is_ok());
    let mut placed: BTreeMap<(HandleId, Layer), Placement> = BTreeMap::new();
    if !wanted.is_empty() {
        let Some(reader) = reader.as_ref() else {
            state.last_error =
                Some("unit shadows have no resolver for the published map receipts".into());
            return;
        };
        let tint = state.tint.get_or_insert_with(|| load_tint(reader)).clone();
        let tint = match tint {
            Ok(tint) => tint,
            Err(error) => {
                state.last_error = Some(error);
                return;
            }
        };
        for (unit, shadows, position) in wanted {
            let layers = [
                shadows
                    .unit
                    .as_ref()
                    .map(|image| (Layer::Unit, &image.texture, Some(image))),
                shadows
                    .building
                    .as_ref()
                    .map(|path| (Layer::Building, path, None)),
            ];
            for (layer, path, image) in layers.into_iter().flatten() {
                if !state.textures.contains_key(path) {
                    let texture = load_texture(reader, path, tint, &mut images, &mut materials);
                    state.textures.insert(path.clone(), texture);
                }
                let placement = (|| -> Result<_, String> {
                    let loaded = state.textures[path].as_ref().map_err(Clone::clone)?;
                    let footprint = match image {
                        Some(image) => image.footprint(position),
                        None => {
                            unit_shadow::building_footprint(position, loaded.texels, SHADOW_CELL)
                        }
                    };
                    let ([cx, cy], side) = footprint.enclosing_square();
                    let [x, y] = position;
                    let z = pcx_tera::world::sample_world_z(&context.terrain, x, y)
                        .ok_or("unit position outside admitted terrain")?;
                    let transform =
                        Transform::from_xyz(cx, z, -cy).with_scale(Vec3::new(side, 1.0, side));
                    Ok((transform, footprint, loaded.material.clone()))
                })();
                match placement {
                    Ok(placement) => {
                        placed.insert((unit, layer), placement);
                    }
                    Err(error) => {
                        state.last_error = Some(format!("shadow for unit {unit:?}: {error}"));
                    }
                }
            }
        }
    }
    state.drawn.retain(|key, drawn| {
        let keep = placed.get(key).is_some_and(|next| next == &drawn.placement);
        if keep {
            placed.remove(key);
        } else {
            commands.entity(drawn.entity).despawn();
            meshes.remove(drawn.mesh.id());
        }
        keep
    });
    for (key, placement) in placed {
        let (transform, footprint, material) = &placement;
        let mesh = crate::campaign_selection::projection::project(&context.terrain, transform, 0.0)
            .and_then(|mut mesh| remap_uvs(&mut mesh, footprint).map(|()| mesh));
        match mesh {
            Ok(mesh) => {
                let mesh = meshes.add(mesh);
                let entity = commands
                    .spawn((
                        UnitShadow,
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        *transform,
                        RenderLayers::layer(peacecraft3::campaign_camera::RENDER_LAYER),
                    ))
                    .id();
                state.drawn.insert(
                    key,
                    Drawn {
                        entity,
                        placement,
                        mesh,
                    },
                );
            }
            Err(error) => state.last_error = Some(format!("shadow for unit {:?}: {error}", key.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcx_map::Terrain;

    fn flat_terrain(width: u32, height: u32) -> Terrain {
        let mut raw = b"W3E!".to_vec();
        raw.extend(11u32.to_le_bytes());
        raw.push(0);
        raw.extend(0u32.to_le_bytes());
        raw.extend(1u32.to_le_bytes());
        raw.extend(b"Ldrt");
        raw.extend(1u32.to_le_bytes());
        raw.extend(b"CLdi");
        raw.extend((width + 1).to_le_bytes());
        raw.extend((height + 1).to_le_bytes());
        raw.extend(32f32.to_le_bytes());
        raw.extend((-48f32).to_le_bytes());
        for _ in 0..(width + 1) * (height + 1) {
            raw.extend(8192u16.to_le_bytes());
            raw.extend([0; 5]);
        }
        Terrain::from_bytes(raw).unwrap()
    }

    /// Every projected vertex addresses the image point that lies under it.
    fn assert_uvs_follow_world(footprint: &Footprint, terrain: &Terrain) {
        let ([cx, cy], side) = footprint.enclosing_square();
        let transform = Transform::from_xyz(cx, 0.0, -cy).with_scale(Vec3::new(side, 1.0, side));
        let mut mesh =
            crate::campaign_selection::projection::project(terrain, &transform, 0.0).unwrap();
        remap_uvs(&mut mesh, footprint).unwrap();
        let positions = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .unwrap()
            .as_float3()
            .unwrap();
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("UV format");
        };
        assert!(!positions.is_empty());
        for (position, uv) in positions.iter().zip(uvs) {
            let world = transform.transform_point(Vec3::from_array(*position));
            let expected = footprint.uv([world.x, -world.z]);
            assert!(
                (expected[0] - uv[0]).abs() < 1e-4 && (expected[1] - uv[1]).abs() < 1e-4,
                "vertex {world:?} has uv {uv:?}, expected {expected:?}"
            );
        }
    }

    #[test]
    fn projected_vertices_address_the_unit_image_rectangle_inside_its_square() {
        let terrain = flat_terrain(4, 4);
        let image = unit_shadow::UnitImage {
            texture: String::new(),
            width: 150.0,
            height: 90.0,
            origin_x: 30.0,
            origin_y: 70.0,
        };
        assert_uvs_follow_world(&image.footprint([200.0, 120.0]), &terrain);
    }

    #[test]
    fn projected_vertices_address_the_building_mask_with_south_first_rows() {
        let terrain = flat_terrain(4, 4);
        let footprint = unit_shadow::building_footprint([180.0, 150.0], [9, 9], SHADOW_CELL);
        assert_uvs_follow_world(&footprint, &terrain);
        // The terrain's south-west quarter of the footprint reads the first rows.
        let ([cx, cy], side) = footprint.enclosing_square();
        let transform = Transform::from_xyz(cx, 0.0, -cy).with_scale(Vec3::new(side, 1.0, side));
        let mut mesh =
            crate::campaign_selection::projection::project(&terrain, &transform, 0.0).unwrap();
        remap_uvs(&mut mesh, &footprint).unwrap();
        let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0)
        else {
            panic!("UV format");
        };
        let south_most = uvs.iter().map(|uv| uv[1]).fold(f32::MAX, f32::min);
        assert!(south_most.abs() < 1e-4, "{south_most}");
    }
}
