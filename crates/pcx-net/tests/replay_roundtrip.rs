//! Records real sessions and plays them back through the public replay API.

use std::sync::Arc;

use pcx_map::Pathing;
use pcx_net::replay::{ReplayError, play};
use pcx_net::session::{Session, SessionError};
use pcx_net::{Datagram, Peer, encode, encode_input_payload, encode_world_payload};
use pcx_sim::{AdmittedMovement, Command, FX_ONE, MovementProfile, World, admit_movement};
use sha1::{Digest, Sha1};

const MAP: [u8; 20] = [0xC3; 20];
const SEED: u64 = 0x1234_5678;

fn session() -> Session {
    let mut world = World::empty();
    world.spawn(0, 0, 0);
    world.spawn(0, FX_ONE, 0);
    world.spawn(1, 0, FX_ONE);
    let mut s = Session::new(world, SEED);
    s.join(Peer(10), 0).unwrap();
    s.join(Peer(11), 1).unwrap();
    s
}

fn order(s: &mut Session, peer: Peer, tick: u64, player: u8, commands: &[Command]) {
    let bytes = encode(&Datagram::Input {
        tick,
        player,
        payload: encode_input_payload(commands),
    });
    s.receive(peer, &bytes).unwrap();
}

/// Ticks 0..6 with orders, a select, and a disconnect that stops player 1's units.
fn recorded_session() -> (Session, Vec<u8>) {
    let mut s = session();
    s.start_recording(MAP).unwrap();
    order(
        &mut s,
        Peer(10),
        0,
        0,
        &[Command::Move {
            x: 2 * FX_ONE,
            y: 0,
        }],
    );
    order(
        &mut s,
        Peer(11),
        0,
        1,
        &[Command::Move { x: 0, y: -FX_ONE }],
    );
    s.advance();
    order(
        &mut s,
        Peer(10),
        1,
        0,
        &[Command::Select { x: 0, y: 0 }, Command::Stop],
    );
    s.advance();
    order(
        &mut s,
        Peer(11),
        2,
        1,
        &[Command::Move {
            x: FX_ONE,
            y: FX_ONE,
        }],
    );
    s.advance();
    s.leave(Peer(11));
    for _ in 0..3 {
        s.advance();
    }
    let bytes = s.stop_recording().expect("session was recording");
    (s, bytes)
}

#[test]
fn playback_reproduces_the_live_world_including_a_disconnect_stop() {
    let (live, bytes) = recorded_session();
    assert!(
        live.world().units[2].move_target.is_none(),
        "the disconnect stop must have cleared player 1's move"
    );
    let replay = play(&bytes, None).unwrap();
    assert_eq!(replay.map_sha1, MAP);
    assert_eq!(replay.seed, SEED);
    assert_eq!(&replay.world, live.world());
    assert_eq!(
        encode_world_payload(&replay.world),
        encode_world_payload(live.world())
    );
}

#[test]
fn recording_is_refused_after_the_first_tick_and_when_already_running() {
    let mut s = session();
    s.start_recording(MAP).unwrap();
    assert!(matches!(
        s.start_recording(MAP),
        Err(SessionError::Replay(_))
    ));
    s.advance();
    assert!(s.stop_recording().is_some());
    assert!(
        s.stop_recording().is_none(),
        "stopping twice reports that nothing was recorded"
    );
    assert!(matches!(
        s.start_recording(MAP),
        Err(SessionError::Replay(_))
    ));
}

#[test]
fn every_truncation_is_refused_without_panicking() {
    let (_, bytes) = recorded_session();
    for n in 0..bytes.len() {
        assert!(
            play(&bytes[..n], None).is_err(),
            "prefix of {n} bytes passed"
        );
    }
}

fn reseal(bytes: &mut [u8]) {
    let split = bytes.len() - 40;
    let digest: [u8; 20] = Sha1::digest(&bytes[..split]).into();
    bytes[split..split + 20].copy_from_slice(&digest);
}

#[test]
fn an_unsealed_edit_is_corrupt_and_a_resealed_header_edit_is_its_own_error() {
    let (_, bytes) = recorded_session();
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(play(&trailing, None), Err(ReplayError::Corrupt)));
    let mut magic = bytes.clone();
    magic[0] ^= 0xFF;
    reseal(&mut magic);
    assert!(matches!(play(&magic, None), Err(ReplayError::BadMagic)));
    let mut version = bytes.clone();
    version[8] = 2;
    reseal(&mut version);
    assert!(matches!(
        play(&version, None),
        Err(ReplayError::UnsupportedVersion { found: 2, .. })
    ));
    let mut body_tail = bytes;
    let split = body_tail.len() - 40;
    body_tail.insert(split, 0);
    reseal(&mut body_tail);
    assert!(matches!(
        play(&body_tail, None),
        Err(ReplayError::Trailing { remaining: 1 })
    ));
}

#[test]
fn a_flipped_byte_is_always_refused() {
    let (_, bytes) = recorded_session();
    for at in 0..bytes.len() {
        let mut flipped = bytes.clone();
        flipped[at] ^= 0xA5;
        assert!(
            play(&flipped, None).is_err(),
            "byte {at} changed the replay"
        );
    }
}

/// A one-row raster whose cell 2 is blocked for the admitted movement.
fn admitted(cells: Vec<u8>) -> Arc<AdmittedMovement> {
    let width = cells.len() as u32;
    let bound = cells.len();
    let pathing = Pathing {
        version: 0,
        width,
        height: 1,
        cells,
    };
    let profile = MovementProfile {
        origin_x: 0,
        origin_y: 0,
        terrain_tile_fx: FX_ONE,
        blocked_bits: 0x02,
    };
    admit_movement(&pathing, width, 1, profile, bound).unwrap()
}

#[test]
fn admitted_movement_replays_only_with_the_same_admission() {
    let movement = admitted(vec![0, 0, 0x02, 0, 0, 0, 0, 0]);
    let mut world = World::empty();
    world.spawn(0, 0, 0);
    world.movement = Some(movement.clone());
    let mut s = Session::new(world, SEED);
    s.join(Peer(10), 0).unwrap();
    s.start_recording(MAP).unwrap();
    order(
        &mut s,
        Peer(10),
        0,
        0,
        &[Command::Move {
            x: 5 * FX_ONE,
            y: 0,
        }],
    );
    for _ in 0..60 {
        s.advance();
    }
    let bytes = s.stop_recording().expect("session was recording");
    assert!(
        s.world().units[0].x < 5 * FX_ONE,
        "the blocked cell must stop the unit before its target"
    );
    let replay = play(&bytes, Some(movement)).unwrap();
    assert_eq!(&replay.world, s.world());
    assert!(matches!(play(&bytes, None), Err(ReplayError::Divergence)));
}
