//! Local session host and its CLI, following The CLI Spec v0.3 (see `schema`).
//!
//! session_server [ADDRESS]                    serve the harness world (default command)
//! session_server identity                     print the session identity
//! session_server check-map --map FILE         refuse a map with its reasons
//! session_server schema                       print the offline CLI description (JSON)
//!
//! Global option: `--format json|text` (default text). Data goes to stdout. Diagnostics go
//! to stderr. In JSON mode an error is one JSON envelope line on stderr, last.
//! Legacy aliases kept from earlier builds: `--print-identity` for `identity`, and `--map FILE`
//! for `check-map`.
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
use serde_json::{Value, json};

#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_ADDRESS: &str = "127.0.0.1:8932";
#[cfg(not(target_arch = "wasm32"))]
const SERVICE_INTERVAL: Duration = Duration::from_millis(1);
#[cfg(not(target_arch = "wasm32"))]
const SESSION_SEED: u64 = 1;
#[cfg(not(target_arch = "wasm32"))]
const USAGE: &str = "usage: session_server [--format json|text] [ADDRESS | identity | check-map --map FILE | schema]";

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

#[cfg(not(target_arch = "wasm32"))]
enum Invocation {
    Help,
    Schema,
    Identity,
    CheckMap { path: PathBuf },
    Serve { address: String },
}

/// Each failure kind has one exit code in `schema`.
#[cfg(not(target_arch = "wasm32"))]
enum Failure {
    Usage(String),
    Runtime(String),
    UnsupportedMap { label: String, reasons: Vec<String> },
}

#[cfg(not(target_arch = "wasm32"))]
impl Failure {
    fn kind(&self) -> &'static str {
        match self {
            Failure::Usage(_) => "usage",
            Failure::Runtime(_) => "runtime_failed",
            Failure::UnsupportedMap { .. } => "unsupported_map",
        }
    }

    fn exit_code(&self) -> u8 {
        match self {
            Failure::Usage(_) => 2,
            Failure::Runtime(_) => 1,
            Failure::UnsupportedMap { .. } => 3,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Returns the `--format` seen before any error, so a usage error is reported in that format.
fn parse(args: impl Iterator<Item = String>) -> (Format, Result<Invocation, Failure>) {
    let mut format = Format::Text;
    match parse_args(args, &mut format) {
        Ok(invocation) => (format, Ok(invocation)),
        Err(failure) => (format, Err(failure)),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_args(
    args: impl Iterator<Item = String>,
    format: &mut Format,
) -> Result<Invocation, Failure> {
    let mut command: Option<&'static str> = None;
    let mut address = None;
    let mut map = None;
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Invocation::Help),
            "--format" => {
                *format = match args.next().as_deref() {
                    Some("json") => Format::Json,
                    Some("text") => Format::Text,
                    _ => return Err(Failure::Usage("--format takes json or text".into())),
                };
            }
            "--print-identity" => set_command(&mut command, "identity")?,
            "--map" => {
                let path = args
                    .next()
                    .ok_or_else(|| Failure::Usage("--map needs a file".into()))?;
                set_command(&mut command, "check-map")?;
                map = Some(PathBuf::from(path));
            }
            "schema" | "identity" | "check-map" | "serve"
                if command.is_none() && address.is_none() =>
            {
                command = Some(match arg.as_str() {
                    "schema" => "schema",
                    "identity" => "identity",
                    "check-map" => "check-map",
                    _ => "serve",
                });
            }
            flag if flag.starts_with('-') => {
                return Err(Failure::Usage(format!("unknown option {flag}")));
            }
            _ if address.is_none() && command.is_none_or(|c| c == "serve") => {
                set_command(&mut command, "serve")?;
                address = Some(arg);
            }
            _ => return Err(Failure::Usage(format!("unexpected argument {arg}"))),
        }
    }
    let invocation = match command.unwrap_or("serve") {
        "schema" => no_extra(Invocation::Schema, &address, &map, "schema")?,
        "identity" => no_extra(Invocation::Identity, &address, &map, "identity")?,
        "check-map" => {
            if address.is_some() {
                return Err(Failure::Usage(
                    "check-map takes --map FILE, not an ADDRESS".into(),
                ));
            }
            let path = map.ok_or_else(|| Failure::Usage("check-map needs --map FILE".into()))?;
            Invocation::CheckMap { path }
        }
        _ => Invocation::Serve {
            address: address.unwrap_or_else(|| DEFAULT_ADDRESS.into()),
        },
    };
    Ok(invocation)
}

#[cfg(not(target_arch = "wasm32"))]
fn set_command(command: &mut Option<&'static str>, next: &'static str) -> Result<(), Failure> {
    match *command {
        Some(current) if current != next => {
            Err(Failure::Usage(format!("{next} conflicts with {current}")))
        }
        _ => {
            *command = Some(next);
            Ok(())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn no_extra(
    invocation: Invocation,
    address: &Option<String>,
    map: &Option<PathBuf>,
    name: &str,
) -> Result<Invocation, Failure> {
    if address.is_some() || map.is_some() {
        return Err(Failure::Usage(format!("{name} takes no other arguments")));
    }
    Ok(invocation)
}

#[cfg(not(target_arch = "wasm32"))]
fn schema() -> Value {
    json!({
        "clispec": "0.3",
        "name": "session_server",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Local PeaceCraft session host. The harness world is not a complete Warcraft match.",
        "output": { "tty": "text", "piped": "text" },
        "global_args": [{
            "name": "--format",
            "type": "string",
            "enum": ["json", "text"],
            "default": "text"
        }],
        "commands": [
            {
                "name": "schema",
                "description": "Describe this CLI offline. Always JSON.",
                "effects": "read_only",
                "output_kind": "data",
                "cardinality": "single",
                "args": [],
                "stdout_schema": { "type": "object", "required": ["clispec", "commands", "errors"] },
                "errors": ["usage"]
            },
            {
                "name": "identity",
                "description": "Print the identity of the harness session: seed and initial world.",
                "effects": "read_only",
                "output_kind": "data",
                "cardinality": "single",
                "args": [],
                "stdout_schema": {
                    "type": "object",
                    "properties": { "identity": { "type": "string", "pattern": "^[0-9a-f]{64}$" } },
                    "required": ["identity"]
                },
                "errors": ["usage"]
            },
            {
                "name": "check-map",
                "description": "Assess a map file. Exits 3 with its reasons: no map is playable yet.",
                "effects": "read_only",
                "output_kind": "data",
                "cardinality": "single",
                "args": [{ "name": "--map", "type": "path", "required": true }],
                "stdout_schema": { "type": "object" },
                "errors": ["usage", "runtime_failed", "unsupported_map"]
            },
            {
                "name": "serve",
                "description": "Listen for peers on ADDRESS and serve the harness world until stopped.",
                "effects": "non_idempotent",
                "output_kind": "data",
                "cardinality": "single",
                "args": [{ "name": "ADDRESS", "type": "string", "required": false }],
                "stdout_schema": {
                    "type": "object",
                    "properties": { "listening": { "type": "string" } },
                    "required": ["listening"]
                },
                "errors": ["usage", "runtime_failed"]
            }
        ],
        "errors": [
            { "kind": "usage", "exit_code": 2, "retryable": false,
              "description": "Invalid arguments. Nothing was run." },
            { "kind": "runtime_failed", "exit_code": 1, "retryable": true,
              "description": "Bind, read or I/O failure. Check the address or file, then retry." },
            { "kind": "unsupported_map", "exit_code": 3, "retryable": false,
              "description": "The map cannot run as a session yet. details.reasons lists why." }
        ]
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn emit_failure(format: Format, failure: &Failure) {
    match format {
        Format::Text => {
            match failure {
                Failure::Usage(message) | Failure::Runtime(message) => {
                    eprintln!("error: {message}");
                }
                Failure::UnsupportedMap { label, reasons } => {
                    eprintln!("error: {label} is not playable by this host:");
                    for reason in reasons {
                        eprintln!("  - {reason}");
                    }
                }
            }
            if matches!(failure, Failure::Usage(_)) {
                eprintln!("{USAGE}");
            }
        }
        Format::Json => {
            let (message, details) = match failure {
                Failure::Usage(message) | Failure::Runtime(message) => (message.clone(), None),
                Failure::UnsupportedMap { label, reasons } => (
                    format!("{label} is not playable by this host"),
                    Some(json!({ "reasons": reasons })),
                ),
            };
            let mut error = json!({ "kind": failure.kind(), "message": message });
            if let (Some(details), Value::Object(fields)) = (details, &mut error) {
                fields.insert("details".into(), details);
            }
            eprintln!("{}", json!({ "error": error }));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn check_map(path: &std::path::Path) -> Result<(), Failure> {
    let label = path.display().to_string();
    let bytes = std::fs::read(path).map_err(|e| Failure::Runtime(format!("{label}: {e}")))?;
    let reasons = map_session::assess(&bytes, &label)
        .map_err(|e| Failure::Runtime(format!("{label}: {e}")))?;
    Err(Failure::UnsupportedMap {
        label,
        reasons: reasons.iter().map(ToString::to_string).collect(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn serve(format: Format, address: &str) -> Result<(), Failure> {
    let mut host = Host::bind(address, local_world(), SESSION_SEED)
        .map_err(|e| Failure::Runtime(format!("{address}: {e}")))?;
    let listening = host
        .local_addr()
        .map_err(|e| Failure::Runtime(format!("{address}: {e}")))?;
    let line = format!("ws://{listening}");
    match format {
        Format::Text => println!(
            "PeaceCraft session listening on {line}; local movement simulation, not a complete Warcraft match"
        ),
        Format::Json => println!("{}", json!({ "listening": line })),
    }
    loop {
        host.poll(Instant::now())
            .map_err(|e| Failure::Runtime(e.to_string()))?;
        std::thread::sleep(SERVICE_INTERVAL);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn execute(format: Format, invocation: Invocation) -> Result<(), Failure> {
    match invocation {
        Invocation::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Invocation::Schema => {
            println!("{}", schema());
            Ok(())
        }
        Invocation::Identity => {
            let identity = session_identity(&local_world(), SESSION_SEED).to_hex();
            match format {
                Format::Text => println!("session identity {identity}"),
                Format::Json => println!("{}", json!({ "identity": identity })),
            }
            Ok(())
        }
        Invocation::CheckMap { path } => check_map(&path),
        Invocation::Serve { address } => serve(format, &address),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> ExitCode {
    let (format, invocation) = parse(std::env::args().skip(1));
    let result = invocation.and_then(|invocation| execute(format, invocation));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            emit_failure(format, &failure);
            ExitCode::from(failure.exit_code())
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() -> std::process::ExitCode {
    eprintln!("error: the TCP/WebSocket session server requires a native target");
    std::process::ExitCode::FAILURE
}
