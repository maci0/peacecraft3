//! `pcx-diff <left.trace> <right.trace>`: compare two play-test traces.
//!
//! Prints `unchanged` and exits 0 when the traces agree, one
//! `diverge tick=... field=... left=... right=...` line and exits 1 when they
//! do not, and a usage or parse message on stderr with exit 2 otherwise.

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        run();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: pcx-diff <left.trace> <right.trace>");
        std::process::exit(2);
    }
    let left = load(&args[1]);
    let right = load(&args[2]);
    match pcx_play::first_divergence(&left, &right) {
        None => {
            println!("unchanged");
        }
        Some(divergence) => {
            println!("{divergence}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load(path: &str) -> pcx_play::Trace {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("pcx-diff: cannot read {path}: {error}");
            std::process::exit(2);
        }
    };
    match pcx_play::Trace::decode(&bytes) {
        Ok(trace) => trace,
        Err(error) => {
            eprintln!("pcx-diff: {path} is not a canonical trace: {error}");
            std::process::exit(2);
        }
    }
}
