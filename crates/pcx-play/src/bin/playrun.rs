//! Run a local synthetic .rpl, assert outcomes and compare canonical traces.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    match execute() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err((code, message)) => {
            eprintln!("playrun: {message}");
            std::process::ExitCode::from(code)
        }
    }
}
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn execute() -> Result<(), (u8, String)> {
    use pcx_play::{Rpl, Trace, first_divergence};
    let usage = "usage: playrun <scenario.rpl> <output.trace> [--runs <count>] [--baseline <baseline.trace>] [--max-work <count>]";
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or((2, usage.into()))?;
    let output = args.next().ok_or((2, usage.into()))?;
    let mut runs = 1usize;
    let mut baseline = None;
    let mut max_work = None;
    while let Some(option) = args.next() {
        let value = args.next().ok_or((2, usage.into()))?;
        match option.as_str() {
            "--runs" => {
                runs = value
                    .parse()
                    .map_err(|_| (2, format!("invalid run count {value:?}")))?
            }
            "--baseline" => {
                if baseline.replace(value).is_some() {
                    return Err((2, "duplicate baseline".into()));
                }
            }
            "--max-work" => {
                max_work = Some(
                    value
                        .parse::<u64>()
                        .map_err(|_| (2, "invalid max work".into()))?,
                )
            }
            _ => return Err((2, format!("unknown option {option:?}; {usage}"))),
        }
    }
    if runs == 0 {
        return Err((2, "run count must be positive".into()));
    }
    let same_file = |left: &str, right: &str| {
        left == right
            || match (std::fs::canonicalize(left), std::fs::canonicalize(right)) {
                (Ok(left), Ok(right)) => left == right,
                _ => false,
            }
    };
    if same_file(&path, &output)
        || baseline
            .as_deref()
            .is_some_and(|baseline| same_file(baseline, &output))
    {
        return Err((2, "output must differ from scenario and baseline".into()));
    }
    let source = std::fs::read_to_string(&path).map_err(|e| (2, format!("{path}: {e}")))?;
    let rpl = Rpl::parse(&source).map_err(|e| (2, format!("{path}:{e}")))?;
    // A conservative, reproducible harness work estimate; never a timing gate.
    let work = rpl
        .scenario
        .ticks
        .checked_mul(rpl.scenario.spawns.len() as u64 + 1)
        .and_then(|w| w.checked_add(rpl.scenario.script.len() as u64))
        .and_then(|w| w.checked_mul(runs as u64))
        .ok_or((2, "work estimate overflow".into()))?;
    if max_work.is_some_and(|max| work > max) {
        return Err((
            1,
            format!("work {work} exceeds limit {}", max_work.unwrap()),
        ));
    }
    let expected = if let Some(path) = &baseline {
        let bytes = std::fs::read(path).map_err(|e| (2, format!("{path}: {e}")))?;
        Some(Trace::decode(&bytes).map_err(|e| (2, format!("{path}: {e}")))?)
    } else {
        None
    };
    let first = rpl.run().map_err(|e| (1, format!("{path}: {e}")))?;
    let bytes = first.encode();
    for index in 1..runs {
        let next = rpl
            .run()
            .map_err(|e| (1, format!("{path} run {}: {e}", index + 1)))?;
        if bytes != next.encode() {
            return Err((
                1,
                format!(
                    "run {}: {}",
                    index + 1,
                    first_divergence(&first, &next)
                        .map(|d| d.to_string())
                        .unwrap_or_else(|| "canonical bytes diverged".into())
                ),
            ));
        }
    }
    // Retain the candidate trace even on baseline mismatch for diagnosis.
    std::fs::write(&output, bytes).map_err(|e| (2, format!("{output}: {e}")))?;
    if let Some(expected) = expected
        && let Some(divergence) = first_divergence(&expected, &first)
    {
        return Err((1, divergence.to_string()));
    }
    println!(
        "passed scenario={} runs={runs} ticks={} work={work} hash={:016x}",
        first.header.scenario,
        first.header.tick_count,
        first.canonical_hash()
    );
    Ok(())
}
