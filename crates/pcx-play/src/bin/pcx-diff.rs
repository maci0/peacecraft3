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
    let divergence = match (&left, &right) {
        (Loaded::Units(left), Loaded::Units(right)) => {
            pcx_play::first_divergence(left, right).map(|d| d.to_string())
        }
        (Loaded::Script(left), Loaded::Script(right)) => {
            pcx_play::script_trace::first_script_divergence(left, right).map(|d| d.to_string())
        }
        _ => {
            eprintln!("pcx-diff: cannot compare unit-state and script-execution trace schemas");
            std::process::exit(2);
        }
    };
    match divergence {
        None => println!("unchanged"),
        Some(divergence) => {
            println!("{divergence}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
enum Loaded {
    Units(pcx_play::Trace),
    Script(pcx_play::campaign::ChapterTrace),
}

#[cfg(not(target_arch = "wasm32"))]
fn load(path: &str) -> Loaded {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("pcx-diff: cannot read {path}: {error}");
            std::process::exit(2);
        }
    };
    let decoded = if bytes.starts_with(b"pcx-script-trace ") {
        pcx_play::campaign::ChapterTrace::decode(&bytes).map(Loaded::Script)
    } else {
        pcx_play::Trace::decode(&bytes)
            .map(Loaded::Units)
            .map_err(|e| e.to_string())
    };
    match decoded {
        Ok(trace) => trace,
        Err(error) => {
            eprintln!("pcx-diff: {path} is not a canonical trace: {error}");
            std::process::exit(2);
        }
    }
}
