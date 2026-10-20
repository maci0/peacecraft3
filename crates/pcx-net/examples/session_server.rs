//! Local session host: cargo run -p pcx-net --features session-server --example session_server
#[cfg(not(target_arch = "wasm32"))]
use std::time::{Duration, Instant};

#[cfg(not(target_arch = "wasm32"))]
use pcx_net::host::{Host, local_world};

#[cfg(not(target_arch = "wasm32"))]
const SERVICE_INTERVAL: Duration = Duration::from_millis(1);

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8932".into());
    let mut host = Host::bind(&address, local_world(), 1)?;
    println!(
        "PeaceCraft session listening on ws://{address}; local movement simulation, not a complete Warcraft match"
    );
    loop {
        host.poll(Instant::now())?;
        std::thread::sleep(SERVICE_INTERVAL);
    }
}

#[cfg(target_arch = "wasm32")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "the TCP/WebSocket session server requires a native target",
    )
    .into())
}
