//! In-memory decoder and simulation smoke checks using `local-fixtures`.
//! The BLP, MDX and TGA fixture inputs are embedded at compile time.
//!
//! Run on a target that reports `process::exit`, such as `wasm32-wasip1`.
//! The program ORs the check results to produce the exit code.
//! Zero means all checks passed. Failure values are 1 (BLP), 2 (MDX), 3 (TGA)
//! and 4 (simulation). A combined nonzero result can be ambiguous.

use pcx_blp::Header;
use pcx_mdl::Mdlx;
use pcx_sim::{Command, FX_ONE, Input, Rng, World, step};

/// Return 1 if the embedded BLP fails to decode to the expected shape.
fn check_blp() -> i32 {
    let blp: &[u8] = include_bytes!("../tests/fixtures/SelectionCircleLarge.blp");
    let ok = match pcx_blp::decode(blp) {
        Ok((Header { width, height, .. }, rgba)) => rgba_shape(width, height, rgba.len()),
        Err(_) => false,
    };
    if ok { 0 } else { 1 }
}

/// Return 2 if the embedded MDX fails to parse or lacks the expected records.
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

/// Return 3 if the embedded TGA fails the decode check.
fn check_tga() -> i32 {
    let tga: &[u8] = include_bytes!("../tests/fixtures/misc.tga");
    let ok = pcx_tga::decode(tga)
        .map(|(h, rgba)| rgba_shape(u32::from(h.width), u32::from(h.height), rgba.len()))
        .unwrap_or(false);
    if ok { 0 } else { 3 }
}

/// Return 4 if the two simulation runs produce different results.
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

fn rgba_shape(width: u32, height: u32, actual: usize) -> bool {
    width > 0
        && height > 0
        && usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .and_then(|(w, h)| w.checked_mul(h))
            .and_then(|pixels| pixels.checked_mul(4))
            == Some(actual)
}

fn main() {
    let mut code = 0;
    code |= check_blp();
    code |= check_mdx();
    code |= check_tga();
    code |= check_sim();
    std::process::exit(code);
}
