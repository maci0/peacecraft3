//! Admission and fixtures shared by the production-authority tests. Every input is an explicit
//! local file, read from the environment; nothing is searched for or defaulted.
#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode};
use pcx_net::production::ProductionAuthority;

/// Environment inputs, as the headless startup gate reads them.
pub struct Inputs {
    pub install: PathBuf,
    pub map: PathBuf,
    pub mode: GameMode,
}

pub fn inputs() -> Inputs {
    let env = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| panic!("set {name}; see the test module comment"))
    };
    let addon = match env("PCX_STARTUP_ADDON").as_str() {
        "roc" => Addon::NoAddon,
        "tft" => Addon::Tft,
        other => panic!("PCX_STARTUP_ADDON must be roc or tft, not {other}"),
    };
    Inputs {
        install: PathBuf::from(env("PCX_STARTUP_INSTALL")),
        map: PathBuf::from(env("PCX_STARTUP_MAP")),
        mode: GameMode {
            build: Build::V1_29b,
            addon,
            assets: AssetMode::Vanilla,
            behaviour: Behaviour::Vanilla,
        },
    }
}

/// Admits the selected map with `seed` and wraps it as the production authority.
pub fn admit(seed: i32) -> ProductionAuthority {
    let inputs = inputs();
    let bytes = std::fs::read(&inputs.map).expect("read map");
    let label = inputs.map.display().to_string();
    let startup =
        pcx_play::headless_startup::start(&inputs.install, inputs.mode, &label, &bytes, seed, None)
            .unwrap_or_else(|error| panic!("admission: {error}"));
    ProductionAuthority::new(startup).unwrap_or_else(|error| panic!("roster: {error}"))
}
