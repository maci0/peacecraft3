//! Host-thread and peer helpers shared by the loopback and admitted-map session tests.
#![cfg(not(target_arch = "wasm32"))]

use std::{
    io,
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use pcx_net::{
    host::{Host, session_identity},
    session::SessionIdentity,
    session_client::{Received, SessionClient},
};
use pcx_sim::{Unit, World};

pub const TOKEN_LEFT: &str = "0123456789abcdef0123456789abcdef";
pub const TOKEN_RIGHT: &str = "fedcba9876543210fedcba9876543210";
/// Bounds every wait by message count (50 s of host ticks at 25 ms); a silent host fails the test instead of hanging it.
pub const MAX_MESSAGES: usize = 2_000;

pub struct Server {
    address: SocketAddr,
    identity: SessionIdentity,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl Server {
    /// Runs the host poll loop on its own thread until dropped.
    pub fn start(world: World, seed: u64) -> Self {
        let identity = session_identity(&world, seed);
        let mut host = Host::bind("127.0.0.1:0", world, seed).expect("bind loopback host");
        let address = host.local_addr().expect("host address");
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                host.poll(Instant::now())?;
                thread::sleep(Duration::from_millis(1));
            }
            Ok(())
        });
        Self {
            address,
            identity,
            stop,
            thread: Some(thread),
        }
    }

    pub fn address(&self) -> String {
        self.address.to_string()
    }

    /// The identity a peer must verify before play; derived from the host's own world and seed.
    pub fn identity(&self) -> &SessionIdentity {
        &self.identity
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

pub fn snapshot(peer: &mut SessionClient) -> World {
    match peer.receive().expect("snapshot") {
        Received::Snapshot(world) => world,
        Received::Rejected(refusal) => panic!("unexpected refusal: {refusal}"),
    }
}

/// Reads snapshots until one satisfies `done`; returns every snapshot read, the last matching one included.
pub fn until(peer: &mut SessionClient, mut done: impl FnMut(&World) -> bool) -> Vec<World> {
    let mut seen = Vec::new();
    for _ in 0..MAX_MESSAGES {
        let world = snapshot(peer);
        let finished = done(&world);
        seen.push(world);
        if finished {
            return seen;
        }
    }
    panic!("condition not reached within {MAX_MESSAGES} snapshots");
}

pub fn owned(world: &World, owner: u8) -> &Unit {
    world
        .units
        .iter()
        .find(|unit| unit.owner == owner)
        .expect("owner has a unit")
}
