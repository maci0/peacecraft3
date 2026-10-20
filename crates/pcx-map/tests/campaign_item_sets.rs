//! Original-input gate: the placed-unit drop sets must match the drop calls the
//! same map's script generates for them. Requires two locally extracted files
//! from one campaign map, neither committed:
//!
//! - `PEACECRAFT3_CAMPAIGN_UNITS`: that map's `war3mapUnits.doo`
//! - `PEACECRAFT3_CAMPAIGN_SCRIPT`: that map's generated `war3map.j`
//!
//! A swapped pair order (chance read as the rawcode, or the reverse) fails the
//! multiset comparison, because the rawcodes never equal the small chances.
use pcx_map::Units;
use std::path::PathBuf;

fn required(name: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(name).unwrap_or_else(|| panic!("set {name}")))
}

/// `RandomDistAddItem( 'pman', 100 )` in generated script text.
fn script_drop_entries(script: &str) -> Vec<(u32, u32)> {
    let mut entries = Vec::new();
    for line in script.lines() {
        let Some(at) = line.find("RandomDistAddItem(") else {
            continue;
        };
        let args = &line[at + "RandomDistAddItem(".len()..];
        let (code, rest) = args.split_once(',').expect("rawcode argument");
        let code = code.trim().trim_matches('\'').as_bytes();
        let code: [u8; 4] = code.try_into().expect("four-character rawcode");
        let chance = rest.split(')').next().expect("chance argument").trim();
        entries.push((
            u32::from_le_bytes(code),
            chance.parse().expect("integer chance"),
        ));
    }
    entries.sort_unstable();
    entries
}

#[test]
#[ignore = "requires PEACECRAFT3_CAMPAIGN_UNITS and PEACECRAFT3_CAMPAIGN_SCRIPT from one local campaign map"]
fn placed_drop_sets_match_generated_drop_calls() {
    let units = std::fs::read(required("PEACECRAFT3_CAMPAIGN_UNITS")).unwrap();
    let script = std::fs::read_to_string(required("PEACECRAFT3_CAMPAIGN_SCRIPT")).unwrap();
    let parsed = Units::parse(&units).unwrap();
    let mut placed: Vec<(u32, u32)> = parsed
        .instances
        .iter()
        .flat_map(|unit| unit.item_sets.iter().flatten())
        .map(|&[code, chance]| (code, chance))
        .collect();
    placed.sort_unstable();
    let generated = script_drop_entries(&script);
    assert!(
        !generated.is_empty(),
        "script has no RandomDistAddItem calls"
    );
    assert_eq!(placed, generated);
}
