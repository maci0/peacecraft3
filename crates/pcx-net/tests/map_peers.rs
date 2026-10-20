//! Native peers on an admitted, unchanged local map over loopback WebSockets.
//!
//! Ignored by default: the tests read the selected Legacy install and two local map
//! files, and each admission reads the whole map. Run them with:
//!
//! PCX_SESSION_INSTALL=<install dir> PCX_SESSION_MAP=<map file> \
//!   PCX_SESSION_OTHER_MAP=<different map file> \
//!   cargo test -p pcx-net --features session-server --test map_peers -- --ignored
#![cfg(not(target_arch = "wasm32"))]

mod support;

use std::path::{Path, PathBuf};

use pcx_net::{
    map_session::{self, MapSession},
    session_client::{ClientError, SessionClient},
};
use pcx_sim::{AdmittedMovement, CellQuery, Command, Input, PlayerInput, Rng, Unit, World, step};
use support::{Server, TOKEN_LEFT, TOKEN_RIGHT, owned, snapshot, until};

const SEED: u64 = 1;
const LEFT_OWNER: u8 = 0;
const RIGHT_OWNER: u8 = 1;
/// Pathing cells searched along each axis for a blocked cell.
const SCAN_CELLS: i64 = 8;
/// Consecutive unchanged snapshots that count as a stop.
const STALL_SNAPSHOTS: usize = 30;
const DIRECTIONS: [(i64, i64); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

fn gate(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("set {name} to a selected local file; see the module comment"))
}

fn admit_file(install: &Path, map: &Path, seed: u64) -> MapSession {
    let bytes = std::fs::read(map).expect("read map");
    map_session::admit(install, &bytes, &map.display().to_string(), seed).expect("admit map")
}

/// Steps `initial` through the recorded inputs up to tick `until`, the local reference.
fn replay(initial: &World, seed: u64, inputs: &[(u64, u8, Vec<Command>)], until: u64) -> World {
    let mut world = initial.clone();
    let mut rng = Rng::new(seed);
    while world.tick < until {
        let tick = world.tick;
        let commands = inputs
            .iter()
            .filter(|(at, _, _)| *at == tick)
            .map(|(_, player, commands)| PlayerInput {
                player: *player,
                commands: commands.clone(),
            })
            .collect();
        step(&mut world, &Input { tick, commands }, &mut rng);
    }
    world
}

fn point(x: i64, y: i64) -> Option<(i32, i32)> {
    Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
}

fn blocked(movement: &AdmittedMovement, x: i64, y: i64) -> bool {
    point(x, y).is_some_and(|(x, y)| {
        matches!(
            movement.locate(x, y),
            CellQuery::Inside { blocked: true, .. }
        )
    })
}

/// The first blocked point reached by whole-cell steps from `unit`, with its direction.
///
/// Whole-cell steps keep each sample in the cell `k` cells away, so the sample is inside
/// that cell.
fn barrier(movement: &AdmittedMovement, unit: &Unit) -> Option<((i32, i32), (i64, i64))> {
    let cell = i64::from(movement.cell_fx());
    DIRECTIONS.iter().find_map(|&(dx, dy)| {
        (1..=SCAN_CELLS).find_map(|k| {
            let (x, y) = (
                i64::from(unit.x) + dx * k * cell,
                i64::from(unit.y) + dy * k * cell,
            );
            blocked(movement, x, y)
                .then(|| point(x, y))
                .flatten()
                .map(|at| (at, (dx, dy)))
        })
    })
}

/// The point one cell from `unit` in the first direction whose neighbour is open.
fn open_neighbour(movement: &AdmittedMovement, unit: &Unit) -> Option<(i32, i32)> {
    let cell = i64::from(movement.cell_fx());
    DIRECTIONS.iter().find_map(|&(dx, dy)| {
        let (x, y) = (i64::from(unit.x) + dx * cell, i64::from(unit.y) + dy * cell);
        point(x, y).filter(|&(x, y)| {
            matches!(
                movement.locate(x, y),
                CellQuery::Inside { blocked: false, .. }
            )
        })
    })
}

/// The snapshot for `at`, from what `history` already holds or from the rest of the stream.
fn snapshot_at(history: &[World], peer: &mut SessionClient, at: u64) -> World {
    if let Some(world) = history.iter().find(|world| world.tick == at) {
        return world.clone();
    }
    let world = until(peer, |world| world.tick >= at)
        .pop()
        .expect("snapshot");
    assert_eq!(world.tick, at, "stream skipped tick {at}");
    world
}

/// Reads snapshots until the unit arrives or stops moving for `STALL_SNAPSHOTS` ticks.
fn settled(peer: &mut SessionClient, owner: u8) -> Vec<World> {
    let mut first = None;
    let mut last = None;
    let mut moved = false;
    let mut unchanged = 0;
    until(peer, |world| {
        let unit = owned(world, owner);
        let position = (unit.x, unit.y);
        let start = *first.get_or_insert(position);
        moved |= position != start;
        unchanged = if last == Some(position) {
            unchanged + 1
        } else {
            0
        };
        last = Some(position);
        moved && (unit.move_target.is_none() || unchanged >= STALL_SNAPSHOTS)
    })
}

#[test]
#[ignore = "requires PCX_SESSION_INSTALL, PCX_SESSION_MAP and PCX_SESSION_OTHER_MAP selected local files"]
fn peers_refuse_a_different_map_or_seed_before_play() {
    let install = gate("PCX_SESSION_INSTALL");
    let map = gate("PCX_SESSION_MAP");
    let other = gate("PCX_SESSION_OTHER_MAP");
    let host = admit_file(&install, &map, SEED);
    let host_hex = host.identity.to_hex();
    let server = Server::start(host.world.clone(), host.seed, Some(host.identity.clone()));

    let wrong_map = admit_file(&install, &other, SEED).identity;
    let wrong_seed = admit_file(&install, &map, SEED + 1).identity;
    for (token, expected) in [(TOKEN_LEFT, &wrong_map), (TOKEN_RIGHT, &wrong_seed)] {
        match SessionClient::connect_admitted(&server.address(), token, expected) {
            Err(ClientError::Identity { host, .. }) => {
                assert_eq!(host.as_deref(), Some(host_hex.as_str()));
            }
            Err(other) => panic!("refused for the wrong reason: {other}"),
            Ok(_) => panic!("a peer admitted a different map or seed"),
        }
    }

    let peer = SessionClient::connect_admitted(&server.address(), TOKEN_LEFT, &host.identity)
        .expect("matching peer joins");
    assert_eq!(peer.identity(), Some(&host.identity));
}

#[test]
#[ignore = "requires PCX_SESSION_INSTALL and PCX_SESSION_MAP selected local files"]
fn two_peers_on_one_admitted_map_share_pathed_authority() {
    let install = gate("PCX_SESSION_INSTALL");
    let map = gate("PCX_SESSION_MAP");
    let host = admit_file(&install, &map, SEED);
    // Each peer admits the same inputs itself; its own world is the reference for replay.
    let local = admit_file(&install, &map, SEED);
    assert_eq!(
        host.identity, local.identity,
        "identical inputs gave different identities"
    );
    assert_eq!(host.report, local.report);

    let server = Server::start(host.world.clone(), SEED, Some(host.identity.clone()));
    let mut left = SessionClient::connect_admitted(&server.address(), TOKEN_LEFT, &host.identity)
        .expect("left joins");
    let mut right = SessionClient::connect_admitted(&server.address(), TOKEN_RIGHT, &host.identity)
        .expect("right joins");
    assert_eq!((left.player(), right.player()), (LEFT_OWNER, RIGHT_OWNER));

    // Before any command, the host's state is what the peer's own admission replays to.
    let first = snapshot(&mut left);
    assert_eq!(
        replay(&local.world, SEED, &[], first.tick).units,
        first.units,
        "initial authority differs from the local admission"
    );

    let movement = local.world.movement.clone().expect("admitted movement");
    let left_start = owned(&first, LEFT_OWNER).clone();
    let right_start = owned(&first, RIGHT_OWNER).clone();
    // Owner 1 is blocked: its barrier is nearer, so the settle takes fewer host ticks.
    // Owner 0 has an open neighbour to walk to.
    let (barrier_at, barrier_dir) = barrier(&movement, &right_start).unwrap_or_else(|| {
        panic!("no blocked pathing cell within {SCAN_CELLS} cells of owner {RIGHT_OWNER}'s unit; choose another map")
    });
    let open_at = open_neighbour(&movement, &left_start)
        .unwrap_or_else(|| panic!("no open pathing cell beside owner {LEFT_OWNER}'s unit"));

    let right_order = vec![Command::Move {
        x: barrier_at.0,
        y: barrier_at.1,
    }];
    let left_order = vec![Command::Move {
        x: open_at.0,
        y: open_at.1,
    }];
    let right_tick = right.submit(&right_order).expect("right submits");
    let left_tick = left.submit(&left_order).expect("left submits");
    let inputs = [
        (right_tick, RIGHT_OWNER, right_order),
        (left_tick, LEFT_OWNER, left_order),
    ];

    // Owner 1 stops short of the barrier and never enters a blocked cell on the way.
    let right_history = settled(&mut right, RIGHT_OWNER);
    let right_end = owned(right_history.last().expect("right history"), RIGHT_OWNER).clone();
    assert!(
        right_history.iter().all(|world| {
            let unit = owned(world, RIGHT_OWNER);
            !blocked(&movement, i64::from(unit.x), i64::from(unit.y))
        }),
        "a unit entered a blocked pathing cell"
    );
    let (dx, dy) = barrier_dir;
    let short_of_barrier = (dx > 0 && right_end.x < barrier_at.0)
        || (dx < 0 && right_end.x > barrier_at.0)
        || (dy > 0 && right_end.y < barrier_at.1)
        || (dy < 0 && right_end.y > barrier_at.1);
    assert!(
        short_of_barrier,
        "owner 1 reached the blocked cell at {barrier_at:?}"
    );

    // Owner 0 walks to its open target; both peers must then hold the same authority.
    let left_history = settled(&mut left, LEFT_OWNER);
    let left_end = owned(left_history.last().expect("left history"), LEFT_OWNER);
    assert_ne!(
        (left_end.x, left_end.y),
        (left_start.x, left_start.y),
        "owner 0 did not move"
    );

    let at = left_history
        .last()
        .expect("left history")
        .tick
        .max(right_history.last().expect("right history").tick);
    let left_world = snapshot_at(&left_history, &mut left, at);
    let right_world = snapshot_at(&right_history, &mut right, at);
    assert_eq!(
        left_world, right_world,
        "peers hold different authority at tick {at}"
    );
    let reference = replay(&local.world, SEED, &inputs, at);
    assert_eq!(
        reference.units, left_world.units,
        "authority at tick {at} is not the local replay of the accepted commands"
    );
}
