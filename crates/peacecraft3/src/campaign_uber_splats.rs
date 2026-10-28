//! Ground ubersplats under units whose UnitUI `uberSplat` names one.
//!
//! The selected `Splats\UberSplatData.slk` supplies the texture, blend mode,
//! `Scale`, the `Start`/`Middle`/`End` colours and the `BirthTime`/`Decay` fades
//! (see [`pcx_play::uber_splat`]). A unit present when play begins shows its
//! `Middle` colour; one that appears later fades in from `Start`; one that dies or
//! is removed keeps its splat, fading to `End` over `Decay`.
//! Recreation choices: `Scale` is read as the half-width of a square centred on
//! the unit, the square is clipped onto admitted ground triangles like a
//! selection circle and drawn unlit, only blend mode 0 (alpha blend) is
//! admitted, colour channels clamp to 0..=255, and fade times are the game
//! clock at the first frame that sees the change. `PauseTime` is not applied.
//! See `docs/re-notes/campaign-ubersplats.md`.
use std::{
    collections::BTreeMap,
    sync::{Arc, Weak},
};

use bevy::{camera::visibility::RenderLayers, prelude::*};
use pcx_play::{
    map_startup_host::MapStartupHost,
    uber_splat::{Catalog, Presence, Shade, Sighting, Tracker},
};
use pcx_script::HandleId;
use peacecraft3::{
    browser_configuration::SelectedMapSources, campaign_camera::CampaignCameraSet,
    map_ground::GroundPublication, map_requests::MapRequests, map_script_session::MapScriptSession,
};

const TABLE: &str = "Splats/UberSplatData.slk";
/// Read limits for the splat table; not authored values.
const MAX_TABLE_BYTES: usize = 1 << 20;
const MAX_TABLE_ROWS: u32 = 4096;
/// Identity text limit, as for other selected campaign tables.
const MAX_IDENTITY_BYTES: usize = 16 << 10;
/// Pulls the coplanar splat in front of its terrain, behind selection circles.
const DEPTH_BIAS: f32 = 32.0;
/// The admitted `BlendMode`: straight alpha blending.
const BLEND_ALPHA: i32 = 0;
/// One byte channel at full strength, the unit of the authored colour columns.
const CHANNEL_MAX: f32 = 255.0;

#[derive(Component)]
pub(crate) struct UberSplat;

type Material = Result<Handle<StandardMaterial>, String>;

/// Authored channels as a colour. Rows may author a channel above 255 (shipped
/// `HFS1` writes 256); how the original treats that is not established, so this
/// recreation clamps each channel to a byte range.
fn color(shade: Shade) -> Color {
    let [r, g, b, a] = shade.map(|channel| channel.clamp(0.0, CHANNEL_MAX) / CHANNEL_MAX);
    Color::srgba(r, g, b, a)
}

struct Drawn {
    entity: Entity,
    transform: Transform,
    /// This splat's own copy of its row's material, so each fades independently.
    material: Handle<StandardMaterial>,
    mesh: Handle<Mesh>,
    color: Color,
}

#[derive(Resource, Default)]
pub(crate) struct CampaignUberSplats {
    session: Option<Weak<()>>,
    catalog: Option<Result<Arc<Catalog>, String>>,
    /// Splat per unit, keyed by its definition revision.
    ids: BTreeMap<HandleId, (u64, Option<[u8; 4]>)>,
    lifecycles: Tracker<HandleId>,
    /// Texture material per splat row, shared as the template for each unit's copy.
    materials: BTreeMap<[u8; 4], Material>,
    drawn: BTreeMap<HandleId, Drawn>,
    pub last_error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignUberSplats>()
        .add_systems(Update, draw.after(CampaignCameraSet));
}

fn load_catalog(reader: &pcx_assets::Resolver, edition: &str) -> Result<Arc<Catalog>, String> {
    let raw = reader
        .resolve_bounded(TABLE, MAX_TABLE_BYTES as u64)
        .map_err(|e| format!("{TABLE}: {e}"))?;
    let (_, origin) = pcx_play::campaign_assets::identity(&raw, edition, MAX_IDENTITY_BYTES)?;
    Catalog::read(&raw.bytes, origin, MAX_TABLE_BYTES, MAX_TABLE_ROWS).map(Arc::new)
}

fn retire(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    drawn: Drawn,
) {
    commands.entity(drawn.entity).despawn();
    meshes.remove(drawn.mesh.id());
    materials.remove(drawn.material.id());
}

fn draw(
    mut commands: Commands,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    ground: Res<GroundPublication>,
    (requests, sources): (Res<MapRequests>, Res<SelectedMapSources>),
    mut state: ResMut<CampaignUberSplats>,
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
            retire(&mut commands, &mut meshes, &mut materials, drawn);
        }
        *state = CampaignUberSplats {
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
    let Some(now) = host.objects().timer_game_time() else {
        return;
    };
    let state = &mut *state;
    let mut sightings = BTreeMap::new();
    for (unit_id, unit) in host.objects().executed_units() {
        let presence = if unit.combat.as_ref().is_some_and(|c| c.dead) {
            Presence::Dead
        } else if unit.shown {
            Presence::Alive
        } else {
            Presence::Hidden
        };
        let id = match state.ids.get(&unit_id) {
            Some((revision, id)) if *revision == unit.definition_revision => *id,
            _ => match pcx_play::uber_splat::splat_id(&unit.definition) {
                Ok(id) => {
                    state.ids.insert(unit_id, (unit.definition_revision, id));
                    id
                }
                Err(error) => {
                    state.last_error = Some(format!("ubersplat for unit {unit_id:?}: {error}"));
                    continue;
                }
            },
        };
        if let (Some(id), Some(position)) = (id, unit.position) {
            sightings.insert(
                unit_id,
                Sighting {
                    id,
                    position,
                    presence,
                },
            );
        }
    }
    state
        .ids
        .retain(|unit, _| host.objects().executed_unit(*unit).is_ok());
    state.lifecycles.observe(now, &sightings);
    if state.catalog.is_none() && !sightings.is_empty() {
        let Some(reader) = reader.as_ref() else {
            state.last_error =
                Some("ubersplats have no resolver for the published map receipts".into());
            return;
        };
        state.catalog = Some(load_catalog(reader, &context.sources.map.edition));
    }
    let catalog = match &state.catalog {
        Some(Ok(catalog)) => catalog.clone(),
        Some(Err(error)) => {
            state.last_error = Some(error.clone());
            return;
        }
        None => return,
    };
    let mut wanted = BTreeMap::new();
    for (unit, result) in state.lifecycles.placed(&catalog, now) {
        let placement = result.and_then(|placed| {
            if placed.splat.blend_mode != BLEND_ALPHA {
                return Err(format!(
                    "unsupported ubersplat BlendMode {}",
                    placed.splat.blend_mode
                ));
            }
            let [x, y] = placed.position;
            let z = pcx_tera::world::sample_world_z(&context.terrain, x, y)
                .ok_or("unit position outside admitted terrain")?;
            let size = 2.0 * placed.splat.scale;
            let transform = Transform::from_xyz(x, z, -y).with_scale(Vec3::new(size, 1.0, size));
            Ok((placed, transform))
        });
        match placement {
            Ok(placement) => {
                wanted.insert(unit, placement);
            }
            Err(error) => state.last_error = Some(format!("ubersplat for unit {unit:?}: {error}")),
        }
    }
    let moved: Vec<HandleId> = state
        .drawn
        .iter()
        .filter(|(unit, drawn)| {
            wanted
                .get(unit)
                .is_none_or(|(_, transform)| *transform != drawn.transform)
        })
        .map(|(unit, _)| *unit)
        .collect();
    for unit in moved {
        if let Some(drawn) = state.drawn.remove(&unit) {
            retire(&mut commands, &mut meshes, &mut materials, drawn);
        }
    }
    for (unit, (placed, transform)) in wanted {
        let next = color(placed.shade);
        if let Some(drawn) = state.drawn.get_mut(&unit) {
            if drawn.color != next {
                if let Some(mut material) = materials.get_mut(&drawn.material) {
                    material.base_color = next;
                }
                drawn.color = next;
            }
            continue;
        }
        let spawned = (|| -> Result<Drawn, String> {
            let template = match state.materials.get(&placed.id) {
                Some(material) => material.clone(),
                None => {
                    let reader = reader
                        .as_ref()
                        .ok_or("ubersplats have no resolver for the published map receipts")?;
                    // The colour is set on each unit's copy below, not on this template.
                    let material = crate::campaign_selection::load_material(
                        reader,
                        &(placed.splat.texture.clone(), [u8::MAX; 4]),
                        &mut images,
                        &mut materials,
                        DEPTH_BIAS,
                    );
                    state.materials.insert(placed.id, material.clone());
                    material
                }
            }?;
            let mut own = materials
                .get(&template)
                .cloned()
                .ok_or("ubersplat template material was removed")?;
            own.base_color = next;
            let mesh =
                crate::campaign_selection::projection::project(&context.terrain, &transform, 0.0)?;
            let mesh = meshes.add(mesh);
            let material = materials.add(own);
            let entity = commands
                .spawn((
                    UberSplat,
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    transform,
                    RenderLayers::layer(peacecraft3::campaign_camera::RENDER_LAYER),
                ))
                .id();
            Ok(Drawn {
                entity,
                transform,
                material,
                mesh,
                color: next,
            })
        })();
        match spawned {
            Ok(drawn) => {
                state.drawn.insert(unit, drawn);
            }
            Err(error) => state.last_error = Some(format!("ubersplat for unit {unit:?}: {error}")),
        }
    }
}
