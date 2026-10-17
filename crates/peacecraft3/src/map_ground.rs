//! Admitted-map ground draw.
//!
//! [`install`] registers the ground material and a publisher that runs after
//! [`crate::map_requests::MapRequestSet::Deliver`]. The publisher reads the
//! admitted terrain only. It does not start a script VM, and parsed map data
//! is not a playable session.
//!
//! The source reader is [`crate::browser_configuration::SelectedMapSources`]
//! for the admitted binding receipts. Native `replace_selected` and a file drop
//! of a resolver-backed binding publish that pair. A binding that did not retain
//! its resolver, or a refused candidate, leaves the previous reader in place.
//! With no published reader the publisher records
//! [`MapGroundError::MissingSourceReader`] and does not draw.
//! The SLK read limit and retained/transient texture budgets come from
//! [`GroundSupply`]. No limit, archive, or tileset is filled in here.

use std::sync::{Arc, Weak};

use bevy::prelude::*;
use bevy::render::{render_resource::WgpuLimits, renderer::RenderDevice};
use pcx_gfx::{
    GroundDraw, GroundError, GroundMaterial, GroundMaterialPlugin, GroundMemory, SelectedGroundBlp,
    spawn_admitted_ground,
};
use pcx_map::Terrain;
use pcx_play::map_admission::ParsedMapContext;
use pcx_tera::TileCatalog;

use crate::browser_configuration::SelectedMapSources;
use crate::map_binding::{AssetReceipt, BindingReceipts, receipt};
use crate::map_requests::{MapRequestSet, MapRequests};

const TERRAIN_SLK: &str = "TerrainArt\\Terrain.slk";

struct AttemptKey {
    context: Weak<ParsedMapContext>,
    revision: u64,
}

/// Last admitted ground draw. `entity` is absent until a publication succeeds.
#[derive(Resource, Default)]
pub struct GroundPublication {
    attempted: Option<AttemptKey>,
    pub entity: Option<Entity>,
    /// Source context of the mounted draw; a refused replacement keeps it.
    pub context: Option<Arc<ParsedMapContext>>,
    /// Catalog and BLP winners for the mounted draw, excluding their contents.
    pub receipts: Vec<AssetReceipt>,
    pub last_error: Option<MapGroundError>,
}

/// Caller budgets for the admitted binding revision. `Default` sets no limit.
#[derive(Resource, Default)]
pub struct GroundSupply {
    pub revision: u64,
    pub read_limit: Option<u64>,
    pub memory: Option<GroundMemory>,
}

/// Owned BLP bytes for one terrain slot, in slot order.
#[derive(Debug)]
pub struct SlotBlp {
    pub fourcc: [u8; 4],
    pub source: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MapGroundError {
    #[error("admitted map has no published source reader for its binding receipts")]
    MissingSourceReader,
    #[error("binding revision {binding} has no ground read limit or texture budget")]
    MissingBudgets { binding: u64 },
    #[error("ground budgets are for binding revision {supply}; the admitted revision is {binding}")]
    InputRevision { supply: u64, binding: u64 },
    #[error("{path}: terrain catalog: {detail}")]
    Catalog { path: String, detail: String },
    #[error("terrain slot {fourcc:?} has no Terrain.slk row")]
    MissingCatalog { fourcc: [u8; 4] },
    #[error("{path}: {detail}")]
    Read { path: String, detail: String },
    #[error("ground slot list allocation failed")]
    Allocation,
    #[error(transparent)]
    Ground(#[from] GroundError),
}

/// Registers the ground material and the admitted-map publisher.
pub fn install(app: &mut App) {
    app.add_plugins(GroundMaterialPlugin);
    register_publisher(app);
}

fn register_publisher(app: &mut App) {
    app.init_resource::<GroundPublication>()
        .init_resource::<GroundSupply>()
        .init_resource::<SelectedMapSources>()
        .add_systems(
            Update,
            publish_admitted_ground.after(MapRequestSet::Deliver),
        );
}

fn require_reader(
    sources: &SelectedMapSources,
    receipts: &Arc<BindingReceipts>,
) -> Result<Arc<pcx_assets::Resolver>, MapGroundError> {
    sources
        .for_receipts(receipts)
        .ok_or(MapGroundError::MissingSourceReader)
}

fn require_budget(
    supply: &GroundSupply,
    binding: u64,
) -> Result<(u64, GroundMemory), MapGroundError> {
    if supply.revision != binding {
        return Err(MapGroundError::InputRevision {
            supply: supply.revision,
            binding,
        });
    }
    match (supply.read_limit, supply.memory) {
        (Some(read_limit), Some(memory)) => Ok((read_limit, memory)),
        _ => Err(MapGroundError::MissingBudgets { binding }),
    }
}

type GroundAssets<'w> = (
    ResMut<'w, Assets<Mesh>>,
    ResMut<'w, Assets<Image>>,
    ResMut<'w, Assets<GroundMaterial>>,
);

fn publish_admitted_ground(
    mut commands: Commands,
    requests: Res<MapRequests>,
    sources: Res<SelectedMapSources>,
    supply: Res<GroundSupply>,
    mut publication: ResMut<GroundPublication>,
    device: Option<Res<RenderDevice>>,
    mut assets: GroundAssets<'_>,
) {
    let Some(outcome) = requests.last_outcome.as_ref() else {
        return;
    };
    if outcome.result.is_err() {
        return;
    }
    let Some(bindings) = requests.bindings.as_ref() else {
        return;
    };
    let Some(context) = requests.admission.shared_context() else {
        return;
    };
    if outcome.binding_revision != Some(bindings.revision)
        || outcome.identity.as_ref() != Some(&context.sources.map)
        || context.sources.map.edition != bindings.map_identity.edition
        || context.sources.map.version != bindings.map_identity.version
        || !requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
    {
        return;
    }
    let key = AttemptKey {
        context: Arc::downgrade(&context),
        revision: bindings.revision,
    };
    if publication.attempted.as_ref().is_some_and(|previous| {
        previous.revision == key.revision && Weak::ptr_eq(&previous.context, &key.context)
    }) {
        return;
    }
    let (read_limit, memory) = match require_budget(&supply, bindings.revision) {
        Ok(budget) => budget,
        Err(error) => {
            publication.last_error = Some(error);
            return;
        }
    };
    let reader = match require_reader(&sources, &bindings.receipts) {
        Ok(reader) => reader,
        Err(error) => {
            publication.last_error = Some(error);
            return;
        }
    };
    let texture_limits = device.as_ref().map(|device| device.limits());
    let result = prepare_draw(
        &context.terrain,
        reader.as_ref(),
        read_limit,
        memory,
        texture_limits.as_ref(),
        (&mut assets.0, &mut assets.1, &mut assets.2),
    );
    publication.attempted = Some(key);
    match result {
        Ok((draw, receipts)) => {
            let entity = entity_from_draw(&mut commands, &draw);
            replace_entity(&mut commands, &mut publication, entity);
            publication.context = Some(context);
            publication.receipts = receipts;
            publication.last_error = None;
        }
        Err(error) => {
            publication.last_error = Some(error);
        }
    }
}

fn prepare_draw(
    terrain: &Terrain,
    reader: &pcx_assets::Resolver,
    read_limit: u64,
    memory: GroundMemory,
    texture_limits: Option<&WgpuLimits>,
    assets: (
        &mut Assets<Mesh>,
        &mut Assets<Image>,
        &mut Assets<GroundMaterial>,
    ),
) -> Result<(GroundDraw, Vec<AssetReceipt>), MapGroundError> {
    let (meshes, images, materials) = assets;
    let slk = reader
        .resolve_bounded(TERRAIN_SLK, read_limit)
        .map_err(|error| MapGroundError::Read {
            path: TERRAIN_SLK.to_owned(),
            detail: error.to_string(),
        })?;
    let catalog =
        TileCatalog::from_terrain_slk(&slk.bytes).map_err(|error| MapGroundError::Catalog {
            path: TERRAIN_SLK.to_owned(),
            detail: error.to_string(),
        })?;
    let mut receipts = Vec::new();
    receipts
        .try_reserve_exact(terrain.slots().len() + 1)
        .map_err(|_| MapGroundError::Allocation)?;
    receipts.push(receipt(&slk));
    let slots = load_slot_blps(terrain, &catalog, |path| {
        let asset = reader
            .resolve_bounded(path, read_limit)
            .map_err(|error| error.to_string())?;
        receipts.push(receipt(&asset));
        Ok(asset.bytes)
    })?;
    let draw = spawn_admitted(
        terrain,
        &slots,
        memory,
        texture_limits,
        meshes,
        images,
        materials,
    )?;
    Ok((draw, receipts))
}

fn entity_from_draw(commands: &mut Commands, draw: &GroundDraw) -> Option<Entity> {
    let mesh = draw.mesh.clone()?;
    let material = draw.material.clone()?;
    Some(
        commands
            .spawn((Mesh3d(mesh), MeshMaterial3d(material)))
            .id(),
    )
}

fn replace_entity(
    commands: &mut Commands,
    publication: &mut GroundPublication,
    next: Option<Entity>,
) {
    if let Some(previous) = publication.entity.take() {
        commands.entity(previous).despawn();
    }
    publication.entity = next;
}

/// Resolve each terrain slot through the catalog and the caller's reader.
///
/// A missing catalog row is an error. Slot 0 is not substituted.
pub fn load_slot_blps(
    terrain: &Terrain,
    catalog: &TileCatalog,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<SlotBlp>, MapGroundError> {
    if terrain.slots().len() > pcx_gfx::ground::MAX_GROUND_SLOTS {
        return Err(GroundError::TooManySlots {
            count: terrain.slots().len(),
        }
        .into());
    }
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(terrain.slots().len())
        .map_err(|_| MapGroundError::Allocation)?;
    for fourcc in terrain.slots() {
        let source = catalog
            .blp_path(*fourcc)
            .ok_or(MapGroundError::MissingCatalog { fourcc: *fourcc })?;
        let bytes = read(&source).map_err(|detail| MapGroundError::Read {
            path: source.clone(),
            detail,
        })?;
        slots.push(SlotBlp {
            fourcc: *fourcc,
            source,
            bytes,
        });
    }
    Ok(slots)
}

/// Decode and publish the ground draw. `memory` is the caller's retained and
/// transient budget, separate from menu image budgets.
pub fn spawn_admitted(
    terrain: &Terrain,
    slots: &[SlotBlp],
    memory: GroundMemory,
    texture_limits: Option<&WgpuLimits>,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<GroundMaterial>,
) -> Result<GroundDraw, MapGroundError> {
    let mut borrowed = Vec::new();
    borrowed
        .try_reserve_exact(slots.len())
        .map_err(|_| MapGroundError::Allocation)?;
    for slot in slots {
        borrowed.push(SelectedGroundBlp {
            fourcc: slot.fourcc,
            source: &slot.source,
            bytes: &slot.bytes,
        });
    }
    Ok(spawn_admitted_ground(
        terrain,
        &borrowed,
        memory,
        texture_limits,
        meshes,
        images,
        materials,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> TileCatalog {
        let text = "ID;PWXL;N;E\r\nC;Y1;X1;K\"tileID\"\r\nC;X2;K\"dir\"\r\nC;X3;K\"file\"\r\nC;Y2;X1;K\"Ldrt\"\r\nC;X2;K\"TerrainArt\\LordaeronSummer\"\r\nC;X3;K\"LordaeronSummer\"\r\nE\r\n";
        TileCatalog::from_terrain_slk(text.as_bytes()).unwrap()
    }

    fn terrain(nibble: u8) -> Terrain {
        let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
        bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
        bytes.push(0);
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(1u32.to_le_bytes());
        bytes.extend_from_slice(b"Ldrt");
        bytes.extend(0u32.to_le_bytes());
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(0.0f32.to_le_bytes());
        bytes.extend(0.0f32.to_le_bytes());
        for _ in 0..4 {
            bytes.extend(8192u16.to_le_bytes());
            bytes.extend(0u16.to_le_bytes());
            bytes.push(nibble);
            bytes.push(0);
            bytes.push(0);
        }
        Terrain::from_bytes(bytes).unwrap()
    }

    fn sheet() -> Vec<u8> {
        let payload_at = 0x9Cu32 + 256 * 4;
        let mut bytes = vec![0u8; payload_at as usize + 16];
        bytes[0..4].copy_from_slice(b"BLP1");
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes());
        bytes[0x0C..0x10].copy_from_slice(&4u32.to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&4u32.to_le_bytes());
        bytes[0x1C..0x20].copy_from_slice(&payload_at.to_le_bytes());
        bytes[0x5C..0x60].copy_from_slice(&16u32.to_le_bytes());
        for index in 0..256 {
            let at = 0x9C + index * 4;
            let entry: [u8; 4] = if index == 0 {
                [0, 255, 0, 255]
            } else {
                [0, 0, 0, 255]
            };
            bytes[at..at + 4].copy_from_slice(&entry);
        }
        bytes
    }

    fn sample_receipts() -> Arc<BindingReceipts> {
        use crate::map_binding::AssetReceipt;
        use pcx_assets::{AssetPath, LayerKind, Provenance, SourceInfo};
        let source = SourceInfo {
            id: "fixture".into(),
            kind: LayerKind::Mpq,
            origin: "fixture-origin".into(),
            version: Some("fixture".into()),
        };
        let receipt = AssetReceipt {
            path: AssetPath::new("fixture.blp").unwrap(),
            provenance: Provenance {
                source,
                archive_priority: Some(1),
                locator: "fixture.blp".into(),
                byte_length: 1,
            },
            hash: 1,
        };
        Arc::new(BindingReceipts {
            map: receipt.clone(),
            common: receipt.clone(),
            blizzard: receipt,
            catalog: Vec::new(),
            missing_catalog: Vec::new(),
            units: None,
            items: None,
        })
    }

    #[test]
    fn the_hook_resolves_catalog_bytes_and_spawns_the_ground_draw() {
        let terrain = terrain(0);
        let slots = load_slot_blps(&terrain, &catalog(), |path| {
            assert_eq!(path, "TerrainArt\\LordaeronSummer\\LordaeronSummer.blp");
            Ok(sheet())
        })
        .unwrap();
        let mut meshes = Assets::default();
        let mut images = Assets::default();
        let mut materials = Assets::default();
        let draw = spawn_admitted(
            &terrain,
            &slots,
            GroundMemory {
                retained_bytes: 64,
                transient_bytes: 256,
            },
            None,
            &mut meshes,
            &mut images,
            &mut materials,
        )
        .unwrap();
        let mut world = World::new();
        let entity = entity_from_draw(&mut world.commands(), &draw).unwrap();
        world.flush();
        assert!(world.get::<Mesh3d>(entity).is_some());
        assert!(
            world
                .get::<MeshMaterial3d<GroundMaterial>>(entity)
                .is_some()
        );
    }

    #[test]
    fn a_missing_catalog_row_or_failed_budget_publishes_nothing() {
        let missing = {
            let mut bytes = pcx_map::TERRAIN_MAGIC.to_vec();
            bytes.extend(pcx_map::TERRAIN_VERSION.to_le_bytes());
            bytes.push(0);
            bytes.extend(0u32.to_le_bytes());
            bytes.extend(1u32.to_le_bytes());
            bytes.extend_from_slice(b"ZZZZ");
            bytes.extend(0u32.to_le_bytes());
            bytes.extend(2u32.to_le_bytes());
            bytes.extend(2u32.to_le_bytes());
            bytes.extend(0.0f32.to_le_bytes());
            bytes.extend(0.0f32.to_le_bytes());
            for _ in 0..4 {
                bytes.extend(8192u16.to_le_bytes());
                bytes.extend(0u16.to_le_bytes());
                bytes.extend_from_slice(&[0, 0, 0]);
            }
            Terrain::from_bytes(bytes).unwrap()
        };
        let error = load_slot_blps(&missing, &catalog(), |_| Ok(sheet())).unwrap_err();
        assert_eq!(error, MapGroundError::MissingCatalog { fourcc: *b"ZZZZ" });

        let terrain = terrain(0);
        let slots = load_slot_blps(&terrain, &catalog(), |_| Ok(sheet())).unwrap();
        let mut meshes = Assets::default();
        let mut images = Assets::default();
        let mut materials = Assets::default();
        let error = spawn_admitted(
            &terrain,
            &slots,
            GroundMemory {
                retained_bytes: 63,
                transient_bytes: 256,
            },
            None,
            &mut meshes,
            &mut images,
            &mut materials,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            MapGroundError::Ground(GroundError::RetainedBytes {
                needed: 64,
                limit: 63,
                ..
            })
        ));
        assert!(images.iter().next().is_none());
        assert!(meshes.iter().next().is_none());
        assert!(materials.iter().next().is_none());
        assert!(
            entity_from_draw(
                &mut World::new().commands(),
                &GroundDraw {
                    mesh: None,
                    material: None
                }
            )
            .is_none()
        );
    }

    #[test]
    fn budgets_and_a_missing_reader_are_refused() {
        let error = require_budget(&GroundSupply::default(), 7).unwrap_err();
        assert_eq!(
            error,
            MapGroundError::InputRevision {
                supply: 0,
                binding: 7
            }
        );
        let partial = GroundSupply {
            revision: 7,
            read_limit: Some(32),
            memory: None,
        };
        assert_eq!(
            require_budget(&partial, 7).unwrap_err(),
            MapGroundError::MissingBudgets { binding: 7 }
        );
        let sources = SelectedMapSources::default();
        assert!(matches!(
            require_reader(&sources, &sample_receipts()),
            Err(MapGroundError::MissingSourceReader)
        ));
    }

    #[test]
    fn the_publisher_draws_nothing_without_an_admitted_map() {
        let mut app = App::new();
        crate::map_requests::install(&mut app);
        register_publisher(&mut app);
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<Image>>();
        app.init_resource::<Assets<GroundMaterial>>();
        app.update();
        let publication = app.world().resource::<GroundPublication>();
        assert!(publication.entity.is_none());
        assert!(publication.last_error.is_none());
    }
}
