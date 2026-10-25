//! Explicit original-asset census: lowers every shipped map's `war3map.wtg`
//! from a local install and prints each map's outcome. Opt-in; it needs the
//! install and its TriggerData identity, and no map is ever written out.

use pcx_trig::catalog::Catalog;
use pcx_trig::lower::lower;
use pcx_trig::{Wct, Wtg, extract_trig_file, legacy_dir, map_trig_files, shipped_maps};

fn env(name: &str) -> String {
    std::env::var(format!("PEACECRAFT3_TRIGGER_{name}"))
        .unwrap_or_else(|_| panic!("set PEACECRAFT3_TRIGGER_{name}"))
}

#[test]
#[ignore = "requires the selected Legacy install; prints Prologue02 point-order call sites only"]
fn prologue02_point_order_callers() {
    let dir = legacy_dir().expect("selected Legacy install");
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let bytes = extract_trig_file(&mut set, r"Maps\Campaign\Prologue02.w3m", "war3map.j").unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let mut function = String::new();
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("function ") {
            function = rest.split_whitespace().next().unwrap_or(rest).to_string();
        }
        if trimmed.contains("IssuePointOrder")
            || trimmed.contains("SetUnitRally")
            || trimmed.contains("setrally")
        {
            println!("{} {function}: {trimmed}", number + 1);
        }
    }
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
        let wct_bytes = extract_trig_file(&mut set, &map, "war3map.wct").unwrap();
        let wct = Wct::parse(&wct_bytes).unwrap();
        match lower(&parsed, &wct, &catalog) {
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
