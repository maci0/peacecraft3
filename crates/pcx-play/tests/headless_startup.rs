//! Headless production startup on a selected install and local map. Ignored by default: the
//! gate needs selected local files.
//!
//! PCX_STARTUP_INSTALL=<install dir> PCX_STARTUP_MAP=<map file> PCX_STARTUP_ADDON=roc|tft \
//!   PCX_STARTUP_SEED=<i32> cargo test -p pcx-play --test headless_startup -- --ignored
use std::path::PathBuf;

use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode};
use pcx_play::headless_startup::{Startup, start};
use pcx_script::HandleId;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}; see the test module comment"))
}

fn selected_mode() -> (GameMode, bool) {
    let (addon, expansion) = match env("PCX_STARTUP_ADDON").as_str() {
        "roc" => (Addon::NoAddon, false),
        "tft" => (Addon::Tft, true),
        other => panic!("PCX_STARTUP_ADDON must be roc or tft, not {other}"),
    };
    let mode = GameMode {
        build: Build::V1_29b,
        addon,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    };
    (mode, expansion)
}

type ExecutedUnitState = Vec<(HandleId, i32, u32, u32)>;

fn executed(startup: &Startup) -> ExecutedUnitState {
    startup
        .host
        .objects()
        .executed_units()
        .map(|(id, unit)| (id, unit.executed_owner, unit.x.to_bits(), unit.y.to_bits()))
        .collect()
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP, PCX_STARTUP_ADDON and PCX_STARTUP_SEED"]
fn selected_map_starts_from_the_same_inputs_with_the_same_executed_state() {
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_STARTUP_MAP"));
    let seed: i32 = env("PCX_STARTUP_SEED")
        .parse()
        .expect("PCX_STARTUP_SEED is an i32");
    let (mode, expansion) = selected_mode();
    let bytes = std::fs::read(&map).expect("read map");
    let label = map.display().to_string();

    let first = start(&install, mode, &label, &bytes, seed, expansion)
        .unwrap_or_else(|error| panic!("first start: {error}"));
    let second = start(&install, mode, &label, &bytes, seed, expansion)
        .unwrap_or_else(|error| panic!("second start: {error}"));
    assert_eq!(first.context.sources.map, second.context.sources.map);
    let units = executed(&first);
    assert_eq!(
        units,
        executed(&second),
        "two starts from the same inputs diverged"
    );
    eprintln!("executed units after startup: {}", units.len());
}
