//! Explicit local input check; summaries only, no extracted assets written.

use pcx_map::Map;
use pcx_mpq::{MpqSet, fnv1a64};
use pcx_tera::{WaterBudget, WaterData, WaterSelection};

#[test]
#[ignore = "requires PEACECRAFT3_WATER_INSTALL, PEACECRAFT3_WATER_EDITION and PEACECRAFT3_WATER_MAP"]
fn local_water_inputs() -> Result<(), Box<dyn std::error::Error>> {
    let required = |name| std::env::var(name).map_err(|_| format!("set {name} explicitly"));
    let install = required("PEACECRAFT3_WATER_INSTALL")?;
    let edition = required("PEACECRAFT3_WATER_EDITION")?;
    let map_source = required("PEACECRAFT3_WATER_MAP")?;
    let mut set = MpqSet::open_wc3(&install)?;
    println!("install={install:?} edition={edition:?} map={map_source:?}");

    for member in ["TerrainArt\\Water.slk", "TerrainArt\\Terrain.slk"] {
        let source = set
            .locate(member)
            .map(|(index, _)| set.members()[index].path().display().to_string())
            .ok_or_else(|| format!("{install}: missing {member}"))?;
        let bytes = set.extract(member)?;
        let table = pcx_slq::Table::parse(&bytes)?;
        println!(
            "{source}: {member}: bytes={} fnv1a64={:016x} rows={} columns={}",
            bytes.len(),
            fnv1a64(&bytes),
            table.row_count(),
            table.column_count()
        );
        if member == "TerrainArt\\Water.slk" {
            for column in [
                "waterID", "height", "texFile", "numTex", "texRate", "Smin_A", "Dmax_B",
            ] {
                assert!(
                    table.column_of(column).is_some(),
                    "{member}: missing {column}"
                );
                let token = format!("\"{column}\"");
                let offset = bytes
                    .windows(token.len())
                    .position(|part| part == token.as_bytes())
                    .ok_or_else(|| format!("{member}: header token {token} missing"))?;
                println!("  header={column} token_offset=0x{offset:x}");
            }
        }
    }

    for member in ["Scripts\\common.j", "Scripts\\Blizzard.j"] {
        let bytes = set.extract(member)?;
        println!(
            "{member}: bytes={} fnv1a64={:016x}",
            bytes.len(),
            fnv1a64(&bytes)
        );
        for token in [
            "SetWaterBaseColor",
            "SetWaterDeforms",
            "PATHING_TYPE_FLOATABILITY",
        ] {
            let offsets: Vec<_> = bytes
                .windows(token.len())
                .enumerate()
                .filter_map(|(at, part)| (part == token.as_bytes()).then_some(at))
                .collect();
            println!("  token={token} offsets={offsets:x?}");
        }
    }

    let misc = set.extract("UI\\MiscData.txt")?;
    println!(
        "UI\\MiscData.txt: bytes={} fnv1a64={:016x}",
        misc.len(),
        fnv1a64(&misc)
    );
    for token in [
        "[Water]",
        "MinDepth=",
        "DeepLevel=",
        "MaxDepth=",
        "DeepLevelPathing=",
        "WavesDepth=",
    ] {
        let offset = misc
            .windows(token.len())
            .position(|part| part == token.as_bytes())
            .ok_or_else(|| format!("UI\\MiscData.txt: missing {token}"))?;
        println!("  token={token} offset=0x{offset:x}");
    }

    let map_bytes = set.extract(&map_source)?;
    let map = Map::parse(&map_bytes)?;
    let terrain = map.terrain()?;
    let water = WaterData::new(&terrain, &edition, &map_source)?;
    println!(
        "war3map.w3e: version={} bytes={} fnv1a64={:016x} corners={} size={}x{}",
        terrain.version(),
        terrain.raw().len(),
        fnv1a64(terrain.raw()),
        terrain.corner_count(),
        terrain.width(),
        terrain.height()
    );
    let mut flags = [0usize; 8];
    let mut position_bits = [0usize; 16];
    let mut last_end = None;
    for cy in 0..=terrain.height() {
        for cx in 0..=terrain.width() {
            let raw = water
                .raw_corner(cx, cy)
                .ok_or("missing original corner record")?;
            if let Some(end) = last_end {
                assert_eq!(raw.byte_offset, end);
            } else {
                println!(
                    "  first_corner_offset=0x{:x} second_word=+2 slot_flags=+4",
                    raw.byte_offset
                );
            }
            last_end = Some(raw.byte_offset + raw.record.len());
            assert_eq!(
                u16::from_le_bytes([raw.record[2], raw.record[3]]),
                raw.corner.position_word
            );
            assert_eq!(raw.record[4] >> 4, raw.corner.corner_flags);
            assert_eq!(raw.record[5], raw.corner.byte5);
            assert_eq!(raw.record[6], raw.corner.byte6);
            for (bit, count) in flags.iter_mut().enumerate() {
                *count += usize::from(raw.record[4] & (1 << bit) != 0);
            }
            for (bit, count) in position_bits.iter_mut().enumerate() {
                *count += usize::from(raw.corner.position_word & (1 << bit) != 0);
            }
            assert!(water.water_present(cx, cy).is_some());
        }
    }
    assert_eq!(last_end, Some(terrain.raw().len()));
    let water_slk = set.extract("TerrainArt\\Water.slk")?;
    let water_table = pcx_slq::Table::parse(&water_slk)?;
    let profile = pcx_tera::WaterProfile::open(
        &water_table,
        "TerrainArt\\Water.slk",
        WaterSelection::Tileset,
        &terrain,
    )?;
    let mut level_min = f32::MAX;
    let mut level_max = f32::MIN;
    let mut wet_corners = 0u64;
    for cy in 0..=terrain.height() {
        for cx in 0..=terrain.width() {
            let level = water
                .water_level(cx, cy, &profile)?
                .ok_or("in-range corner returned no water level")?;
            level_min = level_min.min(level);
            level_max = level_max.max(level);
            if water.water_present(cx, cy) == Some(true) {
                wet_corners += 1;
            }
        }
    }
    let mesh = water.mesh(
        &profile,
        &WaterBudget {
            max_vertices: u64::try_from(terrain.corner_count())?
                .checked_mul(4)
                .ok_or("water vertex budget overflows")?,
            max_indices: u64::from(terrain.width())
                .checked_mul(u64::from(terrain.height()))
                .and_then(|tiles| tiles.checked_mul(6))
                .ok_or("water index budget overflows")?,
        },
    )?;
    println!(
        "  water_id={} wet_corners={wet_corners} wet_tiles={} vertices={} level_min={level_min} level_max={level_max}",
        profile.water_id(),
        mesh.wet_tiles,
        mesh.positions.len()
    );
    println!("  raw +4 bit set counts (not meanings): {flags:?}");
    println!("  raw +2 word bit set counts (not meanings): {position_bits:?}");
    if let Ok(script) = map.extract("war3map.j") {
        println!(
            "war3map.j: bytes={} fnv1a64={:016x}",
            script.len(),
            fnv1a64(&script)
        );
        for token in ["SetWaterBaseColor", "SetWaterDeforms"] {
            let count = script
                .windows(token.len())
                .filter(|part| *part == token.as_bytes())
                .count();
            println!("  token={token} occurrences={count}");
        }
    } else {
        println!("war3map.j: not available in the selected map; no script inference");
    }
    Ok(())
}
