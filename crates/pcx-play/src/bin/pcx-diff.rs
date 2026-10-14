//! Compare canonical traces or minimize fresh paired synthetic `.rpl` runs.
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
    if args.get(1).is_some_and(|arg| arg == "minimize") {
        match minimize(&args[2..]) {
            Ok(summary) => println!("{summary}"),
            Err(error) => {
                eprintln!("pcx-diff: {error}");
                std::process::exit(2);
            }
        }
        return;
    }
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

#[cfg(not(target_arch = "wasm32"))]
fn minimize(args: &[String]) -> Result<String, String> {
    use pcx_play::minimize::{MinimizationConfig, minimize_differential_seed};
    if args.len() < 5 {
        return Err("usage: pcx-diff minimize <left.rpl> <right.rpl> <original-seed> <max-oracles> <max-inspections> [candidate-seed ...]".into());
    }
    let read = |path: &str| {
        let source = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        pcx_play::Rpl::parse(&source).map_err(|e| format!("{path}: {e}"))
    };
    let mut left = read(&args[0])?;
    let mut right = read(&args[1])?;
    let original = args[2]
        .parse::<u64>()
        .map_err(|_| "invalid original seed")?;
    let max_oracles = args[3]
        .parse::<usize>()
        .map_err(|_| "invalid oracle budget")?;
    let max_inspections = args[4]
        .parse::<usize>()
        .map_err(|_| "invalid inspection budget")?;
    let candidates = args[5..]
        .iter()
        .map(|arg| {
            arg.parse::<u64>()
                .map_err(|_| format!("invalid candidate seed {arg:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let config = MinimizationConfig::new(original, candidates, max_oracles)
        .with_max_inspections(max_inspections);
    let result = minimize_differential_seed(config, |seed| {
        left.config.seed = seed;
        right.config.seed = seed;
        let a = left
            .run()
            .map_err(|e| std::io::Error::other(format!("{}: {e}", args[0])))?;
        let b = right
            .run()
            .map_err(|e| std::io::Error::other(format!("{}: {e}", args[1])))?;
        Ok::<_, std::io::Error>((a, b))
    })
    .map_err(|e| e.to_string())?;
    Ok(format!(
        "minimum-seed={} original-seed={} oracle-evaluations={} inspections={} status={:?}\n{}",
        result.minimum_seed,
        result.original_seed,
        result.oracle_evaluations,
        result.inspections,
        result.status,
        result.minimum_divergence,
    ))
}
