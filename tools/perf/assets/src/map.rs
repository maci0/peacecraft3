//! W3M open and member parsing, through the checked and unchecked paths.
use pcx_map::{
    Map, doodads::Doodads, info::MapInfo, pathing::Pathing, regions::Regions, shadows::Shadows,
    units::Units,
};
use pcx_mpq::checked::ReadBudget;
use pcx_objects::ObjectFile;

use crate::{
    Context, Failure, Outcome, Prepared, Workload,
    digest::Fnv,
    input::{self, CHECKED_MAPS},
};

pub const WORKLOADS: [Workload; 2] = [
    Workload {
        name: "map-load-v1",
        summary: "Map::parse, extract_checked and parse every gameplay member of four maps",
        prepare: load,
    },
    Workload {
        name: "map-unchecked-v1",
        summary: "Map::terrain, shadows, doodads, info, units, regions and content_files",
        prepare: unchecked,
    },
];

pub const BUDGET: ReadBudget = ReadBudget {
    max_archive_bytes: 256 << 20,
    max_archive_table_bytes: 64 << 20,
    max_raw_packed_bytes: 64 << 20,
    max_decoded_bytes: 64 << 20,
};

const OBJECT_MEMBERS: [&str; 7] = [
    "war3map.w3u",
    "war3map.w3t",
    "war3map.w3a",
    "war3map.w3b",
    "war3map.w3d",
    "war3map.w3h",
    "war3map.w3q",
];

fn read(map: &Map<'_>, name: &str, member: &'static str) -> Result<Option<Vec<u8>>, Failure> {
    map.extract_checked(name, member, BUDGET)
        .map_err(|e| format!("{name}: {e}"))
}

/// Parsed members of one map; `Debug` is the digest surface.
#[derive(Debug, Default)]
struct Members {
    info: Option<MapInfo>,
    terrain: Option<pcx_map::Terrain>,
    shadows: Option<Shadows>,
    doodads: Option<Doodads>,
    pathing: Option<Pathing>,
    units: Option<Units>,
    regions: Option<Regions>,
    objects: Vec<(&'static str, usize, usize, usize)>,
}

fn load_one(name: &str, bytes: &[u8]) -> Result<Members, Failure> {
    let fail = |member: &str, e: &dyn std::fmt::Display| format!("{name}: {member}: {e}");
    let map = Map::parse(bytes).map_err(|e| fail("container", &e))?;
    let mut out = Members::default();
    if let Some(raw) = read(&map, name, "war3map.w3i")? {
        out.info = Some(MapInfo::parse(&raw).map_err(|e| fail("w3i", &e))?);
    }
    if let Some(raw) = read(&map, name, "war3map.w3e")? {
        out.terrain = Some(pcx_map::Terrain::from_bytes(raw).map_err(|e| fail("w3e", &e))?);
    }
    if let (Some(terrain), Some(raw)) = (&out.terrain, read(&map, name, "war3map.shd")?) {
        out.shadows = Some(
            Shadows::from_bytes(raw, terrain.width(), terrain.height())
                .map_err(|e| fail("shd", &e))?,
        );
    }
    if let Some(raw) = read(&map, name, "war3map.doo")? {
        out.doodads = Some(Doodads::from_bytes(raw).map_err(|e| fail("doo", &e))?);
    }
    if let Some(raw) = read(&map, name, "war3map.wpm")? {
        out.pathing = Some(Pathing::parse(&raw).map_err(|e| fail("wpm", &e))?);
    }
    if let Some(raw) = read(&map, name, "war3mapUnits.doo")? {
        out.units = Some(Units::parse(&raw).map_err(|e| fail("units", &e))?);
    }
    if let Some(raw) = read(&map, name, "war3map.w3r")? {
        out.regions = Some(Regions::parse(&raw).map_err(|e| fail("w3r", &e))?);
    }
    for member in OBJECT_MEMBERS {
        if let Some(raw) = read(&map, name, member)? {
            let file = ObjectFile::parse(member, &raw).map_err(|e| fail(member, &e))?;
            let wire = file.records().map(|r| r.wire_len()).sum();
            out.objects
                .push((member, file.base().len(), file.custom().len(), wire));
        }
    }
    Ok(out)
}

fn load(ctx: &Context) -> Result<Prepared, Failure> {
    let maps = input::read_maps(&ctx.install, &CHECKED_MAPS)?;
    let mut input_digest = Fnv::default();
    for (name, bytes) in &maps {
        input_digest.text(name);
        input_digest.bytes(bytes);
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut all = Vec::new();
            for (name, bytes) in &maps {
                all.push(load_one(name, bytes)?);
            }
            let corners: u64 = all
                .iter()
                .filter_map(|m| m.terrain.as_ref())
                .map(|t| t.corner_count() as u64)
                .sum();
            let doodads: u64 = all
                .iter()
                .filter_map(|m| m.doodads.as_ref())
                .map(|d| d.instances().len() as u64)
                .sum();
            Ok(Outcome {
                work: vec![
                    ("maps", all.len() as u64),
                    ("corners", corners),
                    ("doodads", doodads),
                ],
                fold: Box::new(move |out| out.debug(&all)),
            })
        }),
    })
}

fn unchecked(ctx: &Context) -> Result<Prepared, Failure> {
    let maps = input::read_maps(&ctx.install, &CHECKED_MAPS)?;
    let mut input_digest = Fnv::default();
    for (name, bytes) in &maps {
        input_digest.text(name);
        input_digest.bytes(bytes);
    }
    Ok(Prepared {
        input: input_digest,
        run: Box::new(move || {
            let mut all = Vec::new();
            for (name, bytes) in &maps {
                let fail =
                    |member: &str, e: &dyn std::fmt::Display| format!("{name}: {member}: {e}");
                let map = Map::parse(bytes).map_err(|e| fail("container", &e))?;
                let files = map.content_files();
                let terrain = map.terrain().map_err(|e| fail("terrain", &e))?;
                let shadows = map.shadows().map_err(|e| fail("shadows", &e))?;
                let doodads = map.doodads().map_err(|e| fail("doodads", &e))?;
                let info = map.info().map_err(|e| fail("info", &e))?;
                let units = map.units().map_err(|e| fail("units", &e))?;
                let regions = map.regions().map_err(|e| fail("regions", &e))?;
                all.push((files, terrain, shadows, doodads, info, units, regions));
            }
            Ok(Outcome {
                work: vec![("maps", all.len() as u64)],
                fold: Box::new(move |out| out.debug(&all)),
            })
        }),
    })
}
