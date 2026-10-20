//! Local session host.
//!
//! cargo run -p pcx-net --features session-server --example session_server -- [ADDRESS] [--install DIR --map FILE]
//!
//! Without `--install` and `--map` the host runs the harness world with no map identity.
//! With them, it admits the map from the install directory before listening.
#[cfg(not(target_arch = "wasm32"))]
use std::{
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

#[cfg(not(target_arch = "wasm32"))]
use pcx_net::{
    host::{Host, local_world},
    map_session,
};

#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_ADDRESS: &str = "127.0.0.1:8932";
#[cfg(not(target_arch = "wasm32"))]
const SERVICE_INTERVAL: Duration = Duration::from_millis(1);
#[cfg(not(target_arch = "wasm32"))]
const SESSION_SEED: u64 = 1;
#[cfg(not(target_arch = "wasm32"))]
const USAGE: &str = "usage: session_server [ADDRESS] [--install DIR --map FILE]";

#[cfg(not(target_arch = "wasm32"))]
struct Serve {
    address: String,
    map: Option<(PathBuf, PathBuf)>,
}

#[cfg(not(target_arch = "wasm32"))]
enum Command {
    Help,
    Serve(Serve),
}

#[cfg(not(target_arch = "wasm32"))]
fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut address = None;
    let mut install = None;
    let mut map = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "--install" => {
                install = Some(PathBuf::from(
                    args.next().ok_or("--install needs a directory")?,
                ))
            }
            "--map" => map = Some(PathBuf::from(args.next().ok_or("--map needs a file")?)),
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            _ if address.is_none() => address = Some(arg),
            _ => return Err("only one ADDRESS is accepted".into()),
        }
    }
    let map = match (install, map) {
        (None, None) => None,
        (Some(install), Some(map)) => Some((install, map)),
        _ => return Err("--install and --map must be given together".into()),
    };
    Ok(Command::Serve(Serve {
        address: address.unwrap_or_else(|| DEFAULT_ADDRESS.into()),
        map,
    }))
}

#[cfg(not(target_arch = "wasm32"))]
fn serve(options: Serve) -> Result<(), String> {
    let (world, identity, mode) = match &options.map {
        None => (
            local_world(),
            None,
            "local movement simulation, not a complete Warcraft match".to_string(),
        ),
        Some((install, map_path)) => {
            let bytes =
                std::fs::read(map_path).map_err(|e| format!("{}: {e}", map_path.display()))?;
            let label = map_path.display().to_string();
            let session = map_session::admit(install, &bytes, &label, SESSION_SEED)
                .map_err(|e| format!("{label}: {e}"))?;
            eprintln!(
                "admitted {label}: {} units, excluded {:?}",
                session.report.units, session.report.excluded
            );
            println!("session identity {}", session.identity.to_hex());
            (
                session.world,
                Some(session.identity),
                format!("admitted map {label}; not a complete Warcraft match"),
            )
        }
    };
    let mut host = Host::bind(&options.address, world, SESSION_SEED, identity)
        .map_err(|e| format!("{}: {e}", options.address))?;
    println!(
        "PeaceCraft session listening on ws://{}; {mode}",
        options.address
    );
    loop {
        host.poll(Instant::now()).map_err(|e| e.to_string())?;
        std::thread::sleep(SERVICE_INTERVAL);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> ExitCode {
    match parse(std::env::args().skip(1)) {
        Ok(Command::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}\n{USAGE}");
            ExitCode::from(2)
        }
        Ok(Command::Serve(options)) => match serve(options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("error: {message}");
                ExitCode::FAILURE
            }
        },
    }
}

#[cfg(target_arch = "wasm32")]
fn main() -> std::process::ExitCode {
    eprintln!("error: the TCP/WebSocket session server requires a native target");
    std::process::ExitCode::FAILURE
}
