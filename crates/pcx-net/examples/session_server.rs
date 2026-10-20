//! Local session host.
//!
//! cargo run -p pcx-net --features session-server --example session_server -- [ADDRESS]
//! cargo run -p pcx-net --features session-server --example session_server -- --print-identity
//! cargo run -p pcx-net --features session-server --example session_server -- --map FILE
//!
//! Serving prints `PeaceCraft session listening on ...` on stdout. `--print-identity`
//! prints `session identity <hex>` on stdout and binds nothing. Diagnostics go to stderr.
//! `--map` refuses the map with its reasons: map play is not wired to the production startup.
#[cfg(not(target_arch = "wasm32"))]
use std::{
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

#[cfg(not(target_arch = "wasm32"))]
use pcx_net::{
    host::{Host, local_world, session_identity},
    map_session,
};

#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_ADDRESS: &str = "127.0.0.1:8932";
#[cfg(not(target_arch = "wasm32"))]
const SERVICE_INTERVAL: Duration = Duration::from_millis(1);
#[cfg(not(target_arch = "wasm32"))]
const SESSION_SEED: u64 = 1;
#[cfg(not(target_arch = "wasm32"))]
const USAGE: &str = "usage: session_server [ADDRESS] | --print-identity | --map FILE";
/// Exit code for a map this host refuses as unsupported.
#[cfg(not(target_arch = "wasm32"))]
const EXIT_UNSUPPORTED_MAP: u8 = 3;

#[cfg(not(target_arch = "wasm32"))]
enum Command {
    Help,
    PrintIdentity,
    Serve { address: String },
    Map { path: PathBuf },
}

/// Usage errors exit 2; runtime failures exit 1.
#[cfg(not(target_arch = "wasm32"))]
enum Failure {
    Usage(String),
    Runtime(String),
}

#[cfg(not(target_arch = "wasm32"))]
fn parse(mut args: impl Iterator<Item = String>) -> Result<Command, Failure> {
    let mut address = None;
    let mut map = None;
    let mut print_identity = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "--print-identity" => print_identity = true,
            "--map" => {
                let path = args
                    .next()
                    .ok_or_else(|| Failure::Usage("--map needs a file".into()))?;
                map = Some(PathBuf::from(path));
            }
            flag if flag.starts_with('-') => {
                return Err(Failure::Usage(format!("unknown option {flag}")));
            }
            _ if address.is_none() => address = Some(arg),
            _ => return Err(Failure::Usage("only one ADDRESS is accepted".into())),
        }
    }
    match (print_identity, map, address) {
        (true, None, None) => Ok(Command::PrintIdentity),
        (true, _, _) => Err(Failure::Usage(
            "--print-identity takes no other arguments".into(),
        )),
        (false, Some(path), None) => Ok(Command::Map { path }),
        (false, Some(_), Some(_)) => Err(Failure::Usage(
            "--map does not take an ADDRESS; it refuses before listening".into(),
        )),
        (false, None, address) => Ok(Command::Serve {
            address: address.unwrap_or_else(|| DEFAULT_ADDRESS.into()),
        }),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn refuse_map(path: &std::path::Path) -> Result<ExitCode, Failure> {
    let bytes =
        std::fs::read(path).map_err(|e| Failure::Runtime(format!("{}: {e}", path.display())))?;
    let label = path.display().to_string();
    let reasons = map_session::assess(&bytes, &label)
        .map_err(|e| Failure::Runtime(format!("{label}: {e}")))?;
    eprintln!("error: {label} is not playable by this host:");
    for reason in &reasons {
        eprintln!("  - {reason}");
    }
    Ok(ExitCode::from(EXIT_UNSUPPORTED_MAP))
}

#[cfg(not(target_arch = "wasm32"))]
fn serve(address: &str) -> Result<(), Failure> {
    let mut host = Host::bind(address, local_world(), SESSION_SEED)
        .map_err(|e| Failure::Runtime(format!("{address}: {e}")))?;
    println!(
        "PeaceCraft session listening on ws://{address}; local movement simulation, not a complete Warcraft match"
    );
    loop {
        host.poll(Instant::now())
            .map_err(|e| Failure::Runtime(e.to_string()))?;
        std::thread::sleep(SERVICE_INTERVAL);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run(args: impl Iterator<Item = String>) -> Result<ExitCode, Failure> {
    match parse(args)? {
        Command::Help => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        Command::PrintIdentity => {
            println!(
                "session identity {}",
                session_identity(&local_world(), SESSION_SEED).to_hex()
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Map { path } => refuse_map(&path),
        Command::Serve { address } => {
            serve(&address)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> ExitCode {
    match run(std::env::args().skip(1)) {
        Ok(code) => code,
        Err(Failure::Usage(message)) => {
            eprintln!("error: {message}\n{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Runtime(message)) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() -> std::process::ExitCode {
    eprintln!("error: the TCP/WebSocket session server requires a native target");
    std::process::ExitCode::FAILURE
}
