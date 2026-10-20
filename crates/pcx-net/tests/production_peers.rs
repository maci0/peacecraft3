//! Peers over loopback WebSockets against the production authority on a real admitted map.
//! The host runs on its own thread and builds its authority there, so the startup never crosses
//! threads. Ignored by default: the gate needs the selected install and map.
//!
//! PCX_STARTUP_INSTALL=<install dir> PCX_STARTUP_MAP=<map file> PCX_STARTUP_ADDON=roc|tft \
//!   cargo test -p pcx-net --features session-server --test production_peers -- --ignored
#![cfg(not(target_arch = "wasm32"))]

#[path = "support/production.rs"]
mod production;

use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use pcx_net::host::Host;
use pcx_net::session::{SessionAuthority, SessionIdentity};
use pcx_net::session_client::{ClientError, Received, SessionClient};
use pcx_net::{Datagram, encode_input_payload};

use pcx_sim::Command;
use production::admit;

const TOKEN_A: &str = "0123456789abcdef0123456789abcdef";
const TOKEN_B: &str = "fedcba9876543210fedcba9876543210";
const MAX_MESSAGES: usize = 2_000;

/// A production host on its own thread, built from the admission inputs there.
struct Server {
    address: String,
    identity: SessionIdentity,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl Server {
    fn start(seed: i32) -> Self {
        let (ready, announced) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let authority = admit(seed);
            let identity = authority.identity();
            let mut host = Host::with_authority("127.0.0.1:0", authority)?;
            let address = host.local_addr()?.to_string();
            let _ = ready.send((address, identity));
            while !flag.load(Ordering::Relaxed) {
                host.poll(Instant::now())?;
                thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        });
        let (address, identity) = announced.recv().expect("production host started");
        Self {
            address,
            identity,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .expect("host thread panicked")
                .expect("host poll failed");
        }
    }
}

/// The tick of the snapshot the host sends next. A refusal in its place fails the caller.
fn next_snapshot_tick(client: &mut SessionClient) -> u64 {
    match client.receive().expect("session message") {
        Received::Snapshot(world) => world.tick,
        Received::Rejected(text) => panic!("unexpected refusal while waiting: {text}"),
    }
}

/// The first refusal the host sends, after any snapshots.
fn next_refusal(client: &mut SessionClient) -> String {
    for _ in 0..MAX_MESSAGES {
        if let Received::Rejected(text) = client.receive().expect("session message") {
            return text;
        }
    }
    panic!("no refusal within {MAX_MESSAGES} messages");
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP and PCX_STARTUP_ADDON selected local files"]
fn only_the_roster_slot_joins_with_the_admitted_identity() {
    let server = Server::start(1);
    let roster: Vec<u8> = admit(1).roster().collect();
    let slot = roster[0];

    let mut wrong = server.identity.clone();
    wrong.0[0] ^= 1;
    assert!(matches!(
        SessionClient::connect(&server.address, TOKEN_A, &wrong),
        Err(ClientError::Identity { .. })
    ));

    let mut first = SessionClient::connect(&server.address, TOKEN_A, &server.identity)
        .expect("the roster slot joins with the admitted identity");
    assert_eq!(first.player(), slot);
    assert!(
        SessionClient::connect(&server.address, TOKEN_B, &server.identity).is_err(),
        "a second token has no roster slot on this map"
    );

    // A batch for a tick that already ran is refused at receipt, over the wire.
    first
        .send_datagram(&Datagram::Input {
            tick: 0,
            player: slot,
            payload: encode_input_payload(&[Command::Nop]),
        })
        .expect("stale batch sent");
    let refusal = next_refusal(&mut first);
    assert!(refusal.contains("already run"), "{refusal}");
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP and PCX_STARTUP_ADDON selected local files"]
fn reconnect_reclaims_the_slot_and_resync_does_not_rewind() {
    let server = Server::start(1);
    let slot = admit(1).roster().next().expect("roster");

    let mut first =
        SessionClient::connect(&server.address, TOKEN_A, &server.identity).expect("join");
    let joined_tick = next_snapshot_tick(&mut first);
    drop(first);

    let mut again =
        SessionClient::connect(&server.address, TOKEN_A, &server.identity).expect("reconnect");
    assert_eq!(
        again.player(),
        slot,
        "the reconnect reclaims the roster slot"
    );
    again.resync().expect("resync sent");
    let mut last = 0;
    for _ in 0..MAX_MESSAGES {
        match again.receive().expect("message") {
            Received::Snapshot(world) if world.tick >= joined_tick => {
                last = world.tick;
                break;
            }
            Received::Snapshot(_) => continue,
            Received::Rejected(text) => panic!("refused: {text}"),
        }
    }
    assert!(
        last >= joined_tick,
        "resync rewound authority: {last} < {joined_tick}"
    );
}
