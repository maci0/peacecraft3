//! Explicit original-asset census: lowers every shipped map's `war3map.wtg`
//! from a local install and prints each map's outcome. Opt-in; it needs the
//! install and its TriggerData identity, and no map is ever written out.

use pcx_trig::catalog::Catalog;
use pcx_trig::lower::lower;
use pcx_trig::{Wtg, extract_trig_file, legacy_dir, map_trig_files, shipped_maps};

fn env(name: &str) -> String {
    std::env::var(format!("PEACECRAFT3_TRIGGER_{name}"))
        .unwrap_or_else(|_| panic!("set PEACECRAFT3_TRIGGER_{name}"))
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR (or the default local 1.29b tree) and PEACECRAFT3_TRIGGER_EDITION/VERSION"]
fn shipped_maps_lower_or_report_their_refusal() {
    let dir = legacy_dir().expect("set PEACECRAFT3_LEGACY_DIR to a 1.29b install");
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let data = set.extract("UI\\TriggerData.txt").unwrap();
    let catalog = Catalog::parse(
        "UI\\TriggerData.txt",
        &env("EDITION"),
        &env("VERSION"),
        &String::from_utf8_lossy(&data),
    )
    .unwrap();
    let arity = catalog.arity().unwrap();
    let (mut lowered, mut refused) = (0usize, 0usize);
    for map in shipped_maps(&mut set).unwrap() {
        let files = map_trig_files(&mut set, &map).unwrap();
        let Some(wtg_name) = files.iter().find(|f| f.eq_ignore_ascii_case("war3map.wtg")) else {
            println!("{map}: no war3map.wtg");
            continue;
        };
        let bytes = extract_trig_file(&mut set, &map, wtg_name).unwrap();
        let parsed = Wtg::parse(&bytes, &arity).unwrap();
        match lower(&parsed, &catalog) {
            Ok(jass) => {
                lowered += 1;
                println!("{map}: lowered, {} bytes of JASS", jass.len());
            }
            Err(error) => {
                refused += 1;
                println!("{map}: refused: {error}");
            }
        }
    }
    println!("census: {lowered} lowered, {refused} refused");
}
