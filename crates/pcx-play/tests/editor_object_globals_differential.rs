//! Original-asset differential: the derived object-global names match the
//! declarations in the compiled `war3map.j` of every shipped map. This is the
//! evidence behind `pcx_play::editor_object_globals`. Opt-in and read-only.

use pcx_map::{Doodads, Regions, Units};
use pcx_play::editor_object_globals::{object_global_name, region_global_name, sanitize_identifier};
use pcx_trig::{extract_trig_file, legacy_dir, shipped_maps};
use std::collections::BTreeSet;

fn env(name: &str) -> String {
    std::env::var(format!("PEACECRAFT3_TRIGGER_{name}"))
        .unwrap_or_else(|_| panic!("set PEACECRAFT3_TRIGGER_{name}"))
}

fn declared_globals(jass: &str, kind: &str, prefix: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for line in jass.split(['\n', '\r']) {
        let t = line.trim();
        if t.starts_with(kind)
            && t.contains(prefix)
            && let Some(token) = t.split_whitespace().find(|tok| tok.starts_with(prefix))
        {
            out.insert(token.to_string());
        }
    }
    out
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR (or the default local 1.29b tree) and PEACECRAFT3_TRIGGER_EDITION/VERSION"]
fn derived_object_globals_match_the_shipped_war3map_j_declarations() {
    let dir = legacy_dir().expect("set PEACECRAFT3_LEGACY_DIR to a 1.29b install");
    let _ = (env("EDITION"), env("VERSION"));
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let maps = shipped_maps(&mut set).unwrap();
    let mut checked = 0usize;
    for map in &maps {
        let Ok(j_bytes) = extract_trig_file(&mut set, map, "war3map.j") else {
            continue;
        };
        let Ok(units_bytes) = extract_trig_file(&mut set, map, "war3mapUnits.doo") else {
            continue;
        };
        let Ok(regions_bytes) = extract_trig_file(&mut set, map, "war3map.w3r") else {
            continue;
        };
        let Ok(doodads_bytes) = extract_trig_file(&mut set, map, "war3map.doo") else {
            continue;
        };
        let j = String::from_utf8_lossy(&j_bytes);
        let units = Units::parse(&units_bytes).unwrap();
        let regions = Regions::parse(&regions_bytes).unwrap();
        let doodads = Doodads::from_bytes(doodads_bytes).unwrap();

        let declared_units = declared_globals(&j, "unit", "gg_unit_");
        let declared_rcts = declared_globals(&j, "rect", "gg_rct_");
        let declared_dests = declared_globals(&j, "destructable", "gg_dest_");

        let candidate_units: BTreeSet<String> = units
            .instances
            .iter()
            .map(|u| object_global_name("gg_unit_", &u.type_id, u.creation_number))
            .collect();
        let candidate_rcts: BTreeSet<String> = regions
            .regions
            .iter()
            .filter_map(|r| region_global_name(&r.name))
            .collect();
        let candidate_dests: BTreeSet<String> = doodads
            .instances()
            .iter()
            .map(|d| object_global_name("gg_dest_", &d.type_id, d.creation_number))
            .collect();

        assert!(
            declared_units.is_subset(&candidate_units),
            "{map}: declared unit globals not derivable from records: {:?}",
            declared_units.difference(&candidate_units).collect::<Vec<_>>()
        );
        assert!(
            declared_rcts.is_subset(&candidate_rcts),
            "{map}: declared region globals not derivable from records: {:?}",
            declared_rcts.difference(&candidate_rcts).collect::<Vec<_>>()
        );
        assert!(
            declared_dests.is_subset(&candidate_dests),
            "{map}: declared destructable globals not derivable from records: {:?}",
            declared_dests.difference(&candidate_dests).collect::<Vec<_>>()
        );

        // Region name sanitization matches byte-for-byte for the declared set.
        for name in &declared_rcts {
            let region = name.strip_prefix("gg_rct_").expect("prefix");
            assert!(
                regions.regions.iter().any(|r| {
                    sanitize_identifier(&String::from_utf8_lossy(&r.name)) == region
                }),
                "{map}: region {region} has no record whose sanitized name matches"
            );
        }
        checked += 1;
    }
    assert!(checked >= 44, "expected all 44 campaign maps, inspected {checked}");
    println!("object-global differential: {checked} maps, unit/region/destructable declarations match record-derived names");
}
