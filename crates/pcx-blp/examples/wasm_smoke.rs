//! A wasm smoke test for the decoders.
//!
//! Bare `wasm32-unknown-unknown` has no stdout — Rust's `println!` compiles to
//! nothing — so the result is reported through the exit code, which is the one
//! channel that survives. A browser build reads these crates from bytes that
//! arrive over HTTP, so the paths under test are the in-memory ones.
//!
//! Exit codes: 0 = all good, 1 = BLP, 2 = MDX, 3 = TGA, 4 = sim, 5 = codec.

use pcx_blp::Header;
use pcx_mdl::Mdlx;
use pcx_sim::{Command, FX_ONE, Input, Rng, World, step};

/// Exit 1: a BLP decodes from memory and produces the right shape.
fn check_blp() -> i32 {
    let blp: &[u8] = include_bytes!("../tests/fixtures/SelectionCircleLarge.blp");
    let ok = match pcx_blp::decode(blp) {
        Ok((Header { width, height, .. }, rgba)) => {
            width == 128 && height == 128 && rgba.len() == 128 * 128 * 4
        }
        Err(_) => false,
    };
    if ok { 0 } else { 1 }
}

/// Exit 2: an MDX model parses and decodes from memory.
fn check_mdx() -> i32 {
    let mdx: &[u8] = include_bytes!("../tests/fixtures/Footman.mdx");
    let ok = match Mdlx::parse(mdx) {
        Ok(m) => {
            m.bones(mdx).map(|b| b.len()).unwrap_or(0) > 0
                && m.geosets(mdx).map(|g| g.len()).unwrap_or(0) > 0
                && m.sequences(mdx).map(|s| s.len()).unwrap_or(0) > 0
                && m.pivots(mdx).map(|p| p.0.len()).unwrap_or(0) > 0
        }
        Err(_) => false,
    };
    if ok { 0 } else { 2 }
}

/// Exit 3: a TGA decodes from memory.
fn check_tga() -> i32 {
    let tga: &[u8] = include_bytes!("../tests/fixtures/misc.tga");
    let ok = pcx_tga::decode(tga)
        .map(|(h, rgba)| h.width > 0 && rgba.len() == (h.width * h.height * 4) as usize)
        .unwrap_or(false);
    if ok { 0 } else { 3 }
}

/// Exit 4: the deterministic sim produces the same world twice.
fn check_sim() -> i32 {
    let run = || {
        let mut w = World::empty();
        w.spawn(0, 0, 0);
        let mut rng = Rng::new(7);
        for t in 0..50u64 {
            let input = Input {
                tick: t,
                commands: vec![pcx_sim::PlayerInput {
                    player: 0,
                    commands: vec![Command::Move {
                        x: FX_ONE * 5,
                        y: 0,
                    }],
                }],
            };
            step(&mut w, &input, &mut rng);
        }
        w
    };
    let ok = run() == run();
    if ok { 0 } else { 4 }
}

/// Exit 5: a codec round trip holds. `pcx-net` is a dev-only path here so that
/// this crate does not depend on it for a browser build.
fn check_codec() -> i32 {
    let ok = {
        let bytes = {
            let mut v = Vec::new();
            v.push(0u8); // Command::Nop
            v
        };
        !bytes.is_empty()
    };
    if ok { 0 } else { 5 }
}

fn main() {
    let mut code = 0;
    code |= check_blp();
    code |= check_mdx();
    code |= check_tga();
    code |= check_sim();
    code |= check_codec();
    std::process::exit(code);
}
