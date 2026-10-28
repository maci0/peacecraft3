//! Fixed-work asset loading and decode workloads.
//!
//! Gate metrics are user-mode instructions (read by `tools/perf/assets.py` through
//! `perf stat`), allocation calls and requested bytes, and an output digest. The
//! measured window opens after inputs are read from the local install and closes
//! before reporting. Elapsed time is report-only. Inputs are read from the
//! install at run time and are never copied into the repository.
mod alloc;
mod blp;
mod digest;
mod input;
mod map;
mod mdx;
mod mpq;
mod resolver;
mod tables;
mod terrain;
mod window;

use std::{path::PathBuf, process::ExitCode, time::Instant};

use digest::Fnv;

pub type Failure = String;

/// Folds decoded results into the output digest. Results stay alive until it runs,
/// after the window closes, so hashing never inflates the measured instructions.
pub type Fold = Box<dyn FnOnce(&mut Fnv)>;

/// What a workload reports besides allocations: fixed-work counters and the
/// retained decoded results.
pub struct Outcome {
    pub work: Vec<(&'static str, u64)>,
    pub fold: Fold,
}

/// Inputs prepared outside the window and the work executed inside it. `run` is
/// `FnMut` so captured inputs are dropped with `Prepared`, outside the window.
pub struct Prepared {
    pub input: Fnv,
    pub run: Box<dyn FnMut() -> Result<Outcome, Failure>>,
}

pub struct Context {
    pub install: PathBuf,
}

pub struct Workload {
    pub name: &'static str,
    pub summary: &'static str,
    pub prepare: fn(&Context) -> Result<Prepared, Failure>,
}

fn workloads() -> Vec<Workload> {
    let mut all = Vec::new();
    all.extend(mpq::WORKLOADS);
    all.extend(blp::WORKLOADS);
    all.extend(mdx::WORKLOADS);
    all.extend(tables::WORKLOADS);
    all.extend(map::WORKLOADS);
    all.extend(resolver::WORKLOADS);
    all.extend(terrain::WORKLOADS);
    all
}

const USAGE: &str = "usage: peacecraft-assets-perf <command>
  list [--format json]                  workload names and summaries
  run <workload> --install DIR [--control CTL --ack ACK] [--timing]
  paths --install DIR <substring>...    union listfile entries containing every substring
  --help
Install may also come from PEACECRAFT3_LEGACY_DIR. stdout carries one JSON
document, stderr diagnostics. Exit: 0 ok, 1 workload failure, 2 usage.";

fn usage_error(message: &str) -> ExitCode {
    eprintln!("error: {message}\n{USAGE}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        return usage_error("missing command");
    };
    let rest: Vec<String> = args.collect();
    match command.as_str() {
        "--help" | "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        "list" => list(),
        "run" => run(&rest),
        "paths" => paths(&rest),
        other => usage_error(&format!("unknown command {other:?}")),
    }
}

fn list() -> ExitCode {
    let entries: Vec<String> = workloads()
        .iter()
        .map(|w| format!("{{\"name\":\"{}\",\"summary\":\"{}\"}}", w.name, w.summary))
        .collect();
    println!("{{\"workloads\":[{}]}}", entries.join(","));
    ExitCode::SUCCESS
}

struct Options {
    positional: Vec<String>,
    install: Option<PathBuf>,
    control: Option<PathBuf>,
    ack: Option<PathBuf>,
    timing: bool,
}

fn options(args: &[String]) -> Result<Options, String> {
    let mut parsed = Options {
        positional: Vec::new(),
        install: std::env::var_os("PEACECRAFT3_LEGACY_DIR").map(PathBuf::from),
        control: None,
        ack: None,
        timing: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--install" => parsed.install = Some(value("--install")?),
            "--control" => parsed.control = Some(value("--control")?),
            "--ack" => parsed.ack = Some(value("--ack")?),
            "--timing" => parsed.timing = true,
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}")),
            _ => parsed.positional.push(arg.clone()),
        }
    }
    Ok(parsed)
}

fn run(args: &[String]) -> ExitCode {
    let options = match options(args) {
        Ok(options) => options,
        Err(message) => return usage_error(&message),
    };
    let [name] = options.positional.as_slice() else {
        return usage_error("run takes one workload name");
    };
    let Some(install) = options.install else {
        return usage_error("--install or PEACECRAFT3_LEGACY_DIR is required");
    };
    let all = workloads();
    let Some(workload) = all.iter().find(|w| w.name == name) else {
        return usage_error(&format!("unknown workload {name:?}; see `list`"));
    };
    let result = (|| -> Result<String, Failure> {
        let mut window = window::Window::open(options.control.as_deref(), options.ack.as_deref())?;
        let mut prepared = (workload.prepare)(&Context { install })?;
        let start = options.timing.then(Instant::now);
        window.enable()?;
        alloc::start();
        let outcome = (prepared.run)();
        let counts = alloc::stop();
        window.disable()?;
        let elapsed = start.map(|start| start.elapsed().as_nanos());
        let outcome = outcome?;
        let mut output = Fnv::default();
        (outcome.fold)(&mut output);
        let work: Vec<String> = outcome
            .work
            .iter()
            .map(|(key, value)| format!("\"{key}\":{value}"))
            .collect();
        Ok(format!(
            "{{\"workload\":\"{}\",\"input_digest\":\"{:016x}\",\"output_digest\":\"{:016x}\",\"work\":{{{}}},\"allocations\":{},\"reallocations\":{},\"allocated_bytes\":{},\"elapsed_ns_report_only\":{}}}",
            workload.name,
            prepared.input.finish(),
            output.finish(),
            work.join(","),
            counts.allocations,
            counts.reallocations,
            counts.bytes,
            elapsed.map_or("null".into(), |v| v.to_string()),
        ))
    })();
    match result {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {name}: {message}");
            ExitCode::from(1)
        }
    }
}

fn paths(args: &[String]) -> ExitCode {
    let options = match options(args) {
        Ok(options) => options,
        Err(message) => return usage_error(&message),
    };
    let Some(install) = options.install else {
        return usage_error("--install or PEACECRAFT3_LEGACY_DIR is required");
    };
    let needles: Vec<String> = options
        .positional
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let result = input::open_set(&install)
        .and_then(|mut set| set.file_list().map_err(|e| format!("union listfile: {e}")));
    match result {
        Ok(listed) => {
            for path in listed {
                let low = path.to_ascii_lowercase();
                if needles.iter().all(|n| low.contains(n.as_str())) {
                    println!("{path}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
    }
}
