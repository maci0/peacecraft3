//! Terrain mesh, ground-slot selection and height sampling from fixed maps.
use pcx_map::{Map, Terrain};
use pcx_tera::{
    GroundTiles, TerrainMesh, TileCatalog, cliff::TileCensus, tile_texture_selectors,
    world::sample_world_z,
};

use crate::{
    Context, Failure, Outcome, Prepared, Workload,
    digest::Fnv,
    input::{self, CHECKED_MAPS},
    map::BUDGET,
};

pub const WORKLOADS: [Workload; 2] = [
    Workload {
        name: "terrain-mesh-v1",
        summary: "TerrainMesh::build, manifold report and digest for four maps",
        prepare: mesh,
    },
    Workload {
        name: "terrain-ground-v1",
        summary: "tile catalog, ground slots, per-tile selectors, census and height sampling",
        prepare: ground,
    },
];

const CATALOG: &str = "TerrainArt\\Terrain.slk";

/// World-space sample points per axis for the height lattice.
const HEIGHT_SAMPLES: u32 = 128;

fn terrains(ctx: &Context, input_digest: &mut Fnv) -> Result<Vec<(String, Terrain)>, Failure> {
    let maps = input::read_maps(&ctx.install, &CHECKED_MAPS)?;
    let mut out = Vec::new();
    for (name, bytes) in &maps {
        let map = Map::parse(bytes).map_err(|e| format!("{name}: {e}"))?;
        let raw = map
            .extract_checked(name, "war3map.w3e", BUDGET)
            .map_err(|e| format!("{name}: {e}"))?
            .ok_or_else(|| format!("{name}: no war3map.w3e"))?;
        input_digest.text(name);
        input_digest.bytes(&raw);
        out.push((
            name.clone(),
            Terrain::from_bytes(raw).map_err(|e| format!("{name}: {e}"))?,
        ));
    }
    Ok(out)
}

fn mesh(ctx: &Context) -> Result<Prepared, Failure> {
    let mut input_digest = Fnv::default();
    let terrains = terrains(ctx, &mut input_digest)?;
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut built = Vec::new();
            for (_, terrain) in &terrains {
                let mesh = TerrainMesh::build(terrain);
                let manifold = mesh.manifold();
                built.push((mesh, manifold));
            }
            let vertices: u64 = built.iter().map(|(m, _)| m.vertex_count() as u64).sum();
            Ok(Outcome {
                work: vec![("maps", built.len() as u64), ("vertices", vertices)],
                fold: Box::new(move |out| {
                    for (mesh, manifold) in &built {
                        out.debug(mesh);
                        out.debug(manifold);
                        out.u64(mesh.digest());
                    }
                }),
            })
        }),
    })
}

fn ground(ctx: &Context) -> Result<Prepared, Failure> {
    let mut input_digest = Fnv::default();
    let terrains = terrains(ctx, &mut input_digest)?;
    let mut set = input::open_set_with_codec(&ctx.install)?;
    let catalog_bytes = set
        .extract(CATALOG)
        .map_err(|e| format!("{CATALOG}: {e}"))?;
    input_digest.bytes(&catalog_bytes);
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let catalog =
                TileCatalog::from_terrain_slk(&catalog_bytes).map_err(|e| e.to_string())?;
            let mut selectors: Vec<[u16; 4]> = Vec::new();
            let mut paths = Vec::new();
            let mut heights: Vec<Option<f32>> = Vec::new();
            let mut summaries = Vec::new();
            for (name, terrain) in &terrains {
                paths.push(catalog.resolve_terrain(terrain));
                let tiles = GroundTiles::of(terrain);
                let extended: Vec<bool> = (0..terrain.slots().len()).map(|i| i % 2 == 0).collect();
                for ty in 0..terrain.height() {
                    for tx in 0..terrain.width() {
                        selectors.push(
                            tile_texture_selectors(terrain, &extended, tx, ty)
                                .map_err(|e| format!("{name}: {e}"))?,
                        );
                    }
                }
                let (width, height) = (
                    terrain.width() as f32 * 128.0,
                    terrain.height() as f32 * 128.0,
                );
                let (origin_x, origin_y) = terrain.origin();
                for iy in 0..HEIGHT_SAMPLES {
                    for ix in 0..HEIGHT_SAMPLES {
                        let x = origin_x + width * ix as f32 / HEIGHT_SAMPLES as f32;
                        let y = origin_y + height * iy as f32 / HEIGHT_SAMPLES as f32;
                        heights.push(sample_world_z(terrain, x, y));
                    }
                }
                summaries.push((tiles.used_indices(terrain), TileCensus::of(terrain)));
            }
            Ok(Outcome {
                work: vec![
                    ("tiles", selectors.len() as u64),
                    ("height_samples", heights.len() as u64),
                ],
                fold: Box::new(move |out| {
                    out.debug(&catalog);
                    out.debug(&paths);
                    out.debug(&summaries);
                    selectors
                        .iter()
                        .flatten()
                        .for_each(|s| out.u32(u32::from(*s)));
                    heights
                        .iter()
                        .for_each(|h| out.u32(h.map_or(u32::MAX, f32::to_bits)));
                }),
            })
        }),
    })
}
