//! Native peers over real loopback WebSockets against the session host.
//!
//! The host runs on its own thread, exactly as the example server does. Every
//! wait is bounded by a message count and every socket has a read timeout, so a
//! silent host fails these tests rather than hanging them.
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
    Datagram, encode_input_payload,
    host::{Host, local_world},
    session_client::{ClientError, Received, SessionClient},
};
use pcx_sim::{Command, FX_ONE, MOVE_SPEED, Unit, World};

const TOKEN_LEFT: &str = "0123456789abcdef0123456789abcdef";
const TOKEN_RIGHT: &str = "fedcba9876543210fedcba9876543210";
const MAX_MESSAGES: usize = 400;
const LEFT_TARGET: i32 = FX_ONE;
const RIGHT_TARGET: i32 = 3 * FX_ONE;
// Far enough that no test reaches it before the disconnect it exercises.
const FAR_TARGET: i32 = 15_000 * FX_ONE;
const LEFT_OWNER: u8 = 0;
const RIGHT_OWNER: u8 = 1;
const RIGHT_START_X: i32 = 2 * FX_ONE;

struct Server {
    address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl Server {
    fn start() -> Self {
        let mut host = Host::bind("127.0.0.1:0", local_world(), 1).expect("bind loopback host");
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
            stop,
            thread: Some(thread),
        }
    }

    fn join(&self, token: &str) -> SessionClient {
        SessionClient::connect(&self.address.to_string(), token).expect("join")
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

fn snapshot(peer: &mut SessionClient) -> World {
    match peer.receive().expect("snapshot") {
        Received::Snapshot(world) => world,
        Received::Rejected(refusal) => panic!("unexpected refusal: {refusal}"),
    }
}

/// Reads snapshots until one satisfies `done`.
fn until(peer: &mut SessionClient, mut done: impl FnMut(&World) -> bool) -> Vec<World> {
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

fn owned(world: &World, owner: u8) -> &Unit {
    world
        .units
        .iter()
        .find(|unit| unit.owner == owner)
        .expect("owner has a unit")
}

#[test]
fn reconnect_token_is_validated_before_any_socket_is_opened() {
    assert!(matches!(
        SessionClient::connect("127.0.0.1:9", "not-a-token"),
        Err(ClientError::Token)
    ));
}

#[test]
fn two_peers_see_identical_authority_and_each_unit_moves_exactly_to_its_target() {
    let server = Server::start();
    let mut left = server.join(TOKEN_LEFT);
    let mut right = server.join(TOKEN_RIGHT);
    assert_eq!((left.player(), right.player()), (LEFT_OWNER, RIGHT_OWNER));

    // Commands are submitted from the newest snapshot, so sync each peer first.
    snapshot(&mut left);
    snapshot(&mut right);
    left.submit(&[Command::Move {
        x: LEFT_TARGET,
        y: 0,
    }])
    .expect("left submits");
    right
        .submit(&[Command::Move {
            x: RIGHT_TARGET,
            y: 0,
        }])
        .expect("right submits");

    // Both units cover the same distance at the same step, so they arrive together.
    let arrived = |world: &World| {
        [(LEFT_OWNER, LEFT_TARGET), (RIGHT_OWNER, RIGHT_TARGET)]
            .iter()
            .all(|&(owner, target)| {
                owned(world, owner).x == target && owned(world, owner).move_target.is_none()
            })
    };
    let history = until(&mut left, arrived);
    let last = history.last().expect("left saw arrival");

    // One snapshot per tick, and each step is the sim's fixed step or its remainder.
    let mut moved = false;
    for pair in history.windows(2) {
        assert_eq!(pair[1].tick, pair[0].tick + 1, "a tick was skipped");
        for owner in [LEFT_OWNER, RIGHT_OWNER] {
            let step = owned(&pair[1], owner).x - owned(&pair[0], owner).x;
            assert!(
                (0..=MOVE_SPEED).contains(&step),
                "step {step} is outside the sim's clamp"
            );
            moved |= step == MOVE_SPEED;
        }
    }
    assert!(moved, "no unit took a full step");

    // The other peer receives identical authority at the same tick.
    let right_seen = until(&mut right, |world| world.tick >= last.tick);
    assert_eq!(
        right_seen.last(),
        Some(last),
        "peers diverged at tick {}",
        last.tick
    );
}

#[test]
fn a_forged_player_is_refused_and_the_other_slot_gains_no_order() {
    let server = Server::start();
    let mut left = server.join(TOKEN_LEFT);
    let mut right = server.join(TOKEN_RIGHT);
    snapshot(&mut left);

    // Identity is checked before the tick, so tick 0 is only a placeholder.
    left.send_datagram(&Datagram::Input {
        tick: 0,
        player: RIGHT_OWNER,
        payload: encode_input_payload(&[Command::Move {
            x: RIGHT_TARGET,
            y: 0,
        }]),
    })
    .expect("forged packet sent");

    let mut refusal = None;
    for _ in 0..MAX_MESSAGES {
        match left.receive().expect("refusal") {
            Received::Rejected(text) => {
                refusal = Some(text);
                break;
            }
            Received::Snapshot(_) => {}
        }
    }
    assert_eq!(
        refusal.as_deref(),
        Some(r#"{"error":"peer is not assigned to that player"}"#)
    );

    let later = until(&mut right, |world| world.tick >= 10);
    for world in &later {
        let unit = owned(world, RIGHT_OWNER);
        assert_eq!(unit.x, RIGHT_START_X, "forged order moved the other slot");
        assert!(unit.move_target.is_none(), "forged order set a target");
    }
}

#[test]
fn disconnect_stops_the_departed_units_order_and_reconnect_reclaims_the_slot() {
    let server = Server::start();
    let mut left = server.join(TOKEN_LEFT);
    let mut right = server.join(TOKEN_RIGHT);
    snapshot(&mut left);
    left.submit(&[Command::Move {
        x: FAR_TARGET,
        y: 0,
    }])
    .expect("left submits");
    until(&mut right, |world| owned(world, LEFT_OWNER).x > 0);

    drop(left);
    let stopped = until(&mut right, |world| {
        owned(world, LEFT_OWNER).move_target.is_none()
    });
    let frozen = owned(stopped.last().expect("stop observed"), LEFT_OWNER).x;
    assert!(
        frozen > 0 && frozen < FAR_TARGET,
        "stop did not land mid-move: {frozen}"
    );

    let after = until(&mut right, |world| {
        world.tick >= stopped.last().unwrap().tick + 8
    });
    for world in &after {
        assert_eq!(
            owned(world, LEFT_OWNER).x,
            frozen,
            "departed unit kept moving"
        );
    }

    let mut again = server.join(TOKEN_LEFT);
    assert_eq!(
        again.player(),
        LEFT_OWNER,
        "reconnect did not reclaim the slot"
    );
    let first = snapshot(&mut again);
    assert!(
        first.tick >= after.last().unwrap().tick,
        "reconnect rewound authority"
    );
    assert_eq!(owned(&first, LEFT_OWNER).x, frozen);
}
