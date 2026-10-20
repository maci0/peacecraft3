use pcx_net::rollback::{Correction, RollbackLimits};
use pcx_net::{
    Client, Datagram, Peer, decode, decode_world_payload, encode, encode_input_payload,
    encode_world_payload,
    session::{MAX_COMMANDS_PER_TICK, MAX_PACKET_BYTES, Session, SessionError},
};
use pcx_sim::{Command, FX_ONE, Input, MAX_INPUT_LEAD, MOVE_SPEED, PlayerInput, World};

fn order(tick: u64, player: u8, x: i32) -> Vec<u8> {
    encode(&Datagram::Input {
        tick,
        player,
        payload: encode_input_payload(&[Command::Move { x, y: 0 }]),
    })
}

fn session() -> Session {
    let mut world = World::empty();
    world.spawn(0, 0, 0);
    world.spawn(1, 0, 0);
    let mut s = Session::new(world, 1);
    s.join(Peer(10), 0).unwrap();
    s.join(Peer(11), 1).unwrap();
    s
}

#[test]
fn unauthorized_peer_cannot_receive_or_mutate_authority() {
    let mut s = session();
    let before_tick = s.world().tick;
    let before = s.world().clone();
    assert!(matches!(
        s.receive(Peer(999), &order(0, 0, FX_ONE)),
        Err(pcx_net::session::SessionError::Identity)
    ));
    assert_eq!(s.world().tick, before_tick);
    assert_eq!(s.world(), &before);
}

#[test]
fn spoofed_player_is_identity_error_and_does_not_store_input() {
    let mut s = session();
    assert!(matches!(
        s.receive(Peer(10), &order(0, 1, FX_ONE)),
        Err(pcx_net::session::SessionError::Identity)
    ));
    // A subsequent legitimate submission for the same tick/player must still work.
    s.receive(Peer(10), &order(0, 0, FX_ONE)).unwrap();
    let snapshot_before = s.snapshot();
    s.advance();
    assert!(!snapshot_before.is_empty());
}

#[test]
fn tick_rewind_is_rejected() {
    let mut s = session();
    s.receive(Peer(10), &order(2, 0, FX_ONE)).unwrap();
    s.advance();
    s.advance();
    assert!(matches!(
        s.receive(Peer(10), &order(1, 0, FX_ONE)),
        Err(pcx_net::session::SessionError::Tick(_))
    ));
}

#[test]
fn tick_beyond_horizon_is_rejected() {
    let mut s = session();
    assert!(matches!(
        s.receive(Peer(10), &order(MAX_INPUT_LEAD + 1, 0, FX_ONE)),
        Err(pcx_net::session::SessionError::Tick(_))
    ));
}

#[test]
fn oversized_and_malformed_packets_are_rejected() {
    let mut s = session();
    let mut big = vec![0u8; MAX_PACKET_BYTES + 1];
    big[0] = 1;
    assert!(matches!(
        s.receive(Peer(10), &big),
        Err(pcx_net::session::SessionError::Packet)
    ));
    let bad_kind = vec![99u8; 9];
    assert!(matches!(
        s.receive(Peer(10), &bad_kind),
        Err(pcx_net::session::SessionError::Packet)
    ));
}

#[test]
fn coordinate_overflow_is_rejected() {
    let mut s = session();
    assert!(matches!(
        s.receive(Peer(10), &order(0, 0, i32::MIN)),
        Err(pcx_net::session::SessionError::Packet)
    ));
}

#[test]
fn command_count_overflow_is_rejected() {
    let mut s = session();
    let many = vec![Command::Nop; MAX_COMMANDS_PER_TICK + 1];
    let bytes = encode(&Datagram::Input {
        tick: 0,
        player: 0,
        payload: encode_input_payload(&many),
    });
    assert!(matches!(
        s.receive(Peer(10), &bytes),
        Err(pcx_net::session::SessionError::Packet)
    ));
}

#[test]
fn duplicates_are_rejected() {
    let mut s = session();
    s.receive(Peer(10), &order(0, 0, FX_ONE)).unwrap();
    assert!(matches!(
        s.receive(Peer(10), &order(0, 0, -FX_ONE)),
        Err(pcx_net::session::SessionError::Duplicate)
    ));
}

#[test]
fn resync_does_not_rewind_or_mutate_inputs() {
    let mut s = session();
    s.receive(Peer(10), &order(1, 0, FX_ONE)).unwrap();
    let before = s.world().clone();
    let snap = s
        .receive(Peer(10), &encode(&Datagram::Resync { from_tick: 0 }))
        .unwrap()
        .unwrap();
    assert_eq!(snap, s.snapshot());
    assert_eq!(s.world(), &before);
}

#[test]
fn left_peer_cannot_receive_again() {
    let mut s = session();
    s.leave(Peer(10));
    assert!(matches!(
        s.receive(Peer(10), &order(0, 0, FX_ONE)),
        Err(pcx_net::session::SessionError::Identity)
    ));
    s.advance();
    s.join(Peer(12), 0).unwrap();
    s.receive(Peer(12), &order(s.world().tick, 0, FX_ONE))
        .unwrap();
}

#[test]
fn join_rejects_duplicate_peer_or_player() {
    let mut s = session();
    assert!(matches!(
        s.join(Peer(10), 2),
        Err(pcx_net::session::SessionError::Identity)
    ));
    assert!(matches!(
        s.join(Peer(12), 0),
        Err(pcx_net::session::SessionError::Identity)
    ));
    assert!(matches!(
        s.join(Peer(12), 100),
        Err(pcx_net::session::SessionError::Identity)
    ));
}

#[test]
fn advance_applies_both_players_authoritatively() {
    let mut s = session();
    s.receive(Peer(11), &order(0, 1, -FX_ONE)).unwrap();
    s.receive(Peer(10), &order(0, 0, FX_ONE)).unwrap();
    s.advance();
    assert_eq!(s.world().units[0].x, FX_ONE / 10);
    assert_eq!(s.world().units[1].x, -FX_ONE / 10);
}

/// Split-mix seed for `pcx_sim::Rng` inside the session. Not the original JASS RNG.
const HARNESS_SEED: u64 = 11;
const LOGICAL_TICKS: u64 = 3;
const RESIM_BUDGET: usize = 2;

struct TickOrder {
    tick: u64,
    player: u8,
    commands: Vec<Command>,
}

fn script() -> Vec<TickOrder> {
    vec![
        TickOrder {
            tick: 0,
            player: 0,
            commands: vec![
                Command::Select { x: 0, y: 0 },
                Command::Move {
                    x: FX_ONE * 10,
                    y: 0,
                },
            ],
        },
        TickOrder {
            tick: 0,
            player: 1,
            commands: vec![Command::Move {
                x: 0,
                y: FX_ONE * 8,
            }],
        },
        TickOrder {
            tick: 1,
            player: 0,
            commands: vec![Command::Stop],
        },
        TickOrder {
            tick: 2,
            player: 0,
            commands: vec![Command::Move {
                x: FX_ONE * 10,
                y: 0,
            }],
        },
    ]
}

fn initial_world() -> World {
    let mut world = World::empty();
    world.spawn(0, 0, 0);
    world.spawn(0, FX_ONE * 3, 0);
    world.spawn(1, 0, FX_ONE);
    world
}

fn open_session() -> Session {
    let mut session = Session::new(initial_world(), HARNESS_SEED);
    session.join(Peer(10), 0).unwrap();
    session.join(Peer(11), 1).unwrap();
    session
}

fn peer_for(player: u8) -> Peer {
    if player == 0 {
        Peer(10)
    } else {
        Peer(11)
    }
}

fn deliver(session: &mut Session, order: &TickOrder) {
    let bytes = encode(&Datagram::Input {
        tick: order.tick,
        player: order.player,
        payload: encode_input_payload(&order.commands),
    });
    session.receive(peer_for(order.player), &bytes).unwrap();
}

fn orders_for(tick: u64) -> Vec<TickOrder> {
    script().into_iter().filter(|order| order.tick == tick).collect()
}

fn input_for(tick: u64) -> Input {
    Input {
        tick,
        commands: orders_for(tick)
            .into_iter()
            .map(|order| PlayerInput {
                player: order.player,
                commands: order.commands,
            })
            .collect(),
    }
}

fn advance_all(session: &mut Session) -> Vec<Vec<u8>> {
    (0..LOGICAL_TICKS).map(|_| session.advance()).collect()
}

fn just_in_time(reverse_players: bool) -> Vec<Vec<u8>> {
    let mut session = open_session();
    let mut snaps = Vec::new();
    for tick in 0..LOGICAL_TICKS {
        let mut batch = orders_for(tick);
        if reverse_players {
            batch.reverse();
        }
        for order in &batch {
            deliver(&mut session, order);
        }
        snaps.push(session.advance());
    }
    snaps
}

fn lookahead_then_advance() -> Vec<Vec<u8>> {
    let mut session = open_session();
    for tick in (0..LOGICAL_TICKS).rev() {
        let mut batch = orders_for(tick);
        batch.reverse();
        for order in &batch {
            deliver(&mut session, order);
        }
    }
    advance_all(&mut session)
}

fn injected_resync_and_duplicate() -> Vec<Vec<u8>> {
    let mut session = open_session();
    let mut snaps = Vec::new();
    for tick in 0..LOGICAL_TICKS {
        for order in orders_for(tick) {
            let resync = session
                .receive(
                    peer_for(order.player),
                    &encode(&Datagram::Resync { from_tick: 0 }),
                )
                .unwrap()
                .unwrap();
            assert_eq!(resync, session.snapshot());
            deliver(&mut session, &order);
            let again = encode(&Datagram::Input {
                tick: order.tick,
                player: order.player,
                payload: encode_input_payload(&order.commands),
            });
            assert!(matches!(
                session.receive(peer_for(order.player), &again),
                Err(SessionError::Duplicate)
            ));
        }
        snaps.push(session.advance());
    }
    snaps
}

fn snapshot_world(bytes: &[u8]) -> World {
    match decode(bytes).unwrap() {
        Datagram::Snapshot { tick, payload } => decode_world_payload(tick, &payload).unwrap(),
        other => panic!("advance returns a snapshot, got {other:?}"),
    }
}

fn payload(bytes: &[u8]) -> Vec<u8> {
    match decode(bytes).unwrap() {
        Datagram::Snapshot { payload, .. } => payload,
        other => panic!("advance returns a snapshot, got {other:?}"),
    }
}

#[test]
fn tick_keyed_orders_match_across_arrival_lookahead_and_bounded_work() {
    let reference = just_in_time(false);
    assert_eq!(reference, just_in_time(true));
    assert_eq!(reference, lookahead_then_advance());
    assert_eq!(reference, injected_resync_and_duplicate());
    assert_ne!(reference[0], reference[1]);
    assert_ne!(reference[1], reference[2]);

    let after_select_then_move = snapshot_world(&reference[0]);
    assert_eq!(after_select_then_move.unit(1).unwrap().x, MOVE_SPEED);
    assert_eq!(after_select_then_move.unit(1).unwrap().selected, true);
    assert_eq!(after_select_then_move.unit(2).unwrap().x, FX_ONE * 3);
    assert_eq!(after_select_then_move.unit(2).unwrap().selected, false);
    assert_eq!(
        after_select_then_move.unit(3).unwrap().y,
        FX_ONE + MOVE_SPEED
    );
    let stopped = snapshot_world(&reference[1]);
    assert_eq!(stopped.unit(1).unwrap().x, MOVE_SPEED);
    assert_eq!(stopped.unit(1).unwrap().move_target, None);
    let moved_again = snapshot_world(&reference[2]);
    assert_eq!(moved_again.unit(1).unwrap().x, MOVE_SPEED * 2);
    assert_eq!(moved_again.tick, LOGICAL_TICKS);

    let mut client = Client::with_limits(
        initial_world(),
        HARNESS_SEED,
        RollbackLimits {
            history_ticks: 4,
            max_resim_ticks: RESIM_BUDGET,
        },
    );
    for (index, snap) in reference.iter().enumerate() {
        let tick = index as u64;
        client.predict(input_for(tick));
        let world = snapshot_world(snap);
        let report = client.reconcile_detailed(world);
        assert_eq!(report.divergence, 0);
        assert_eq!(report.correction, Correction::Aligned);
        assert_eq!(encode_world_payload(client.predicted()), payload(snap));
        client.acknowledge(tick);
    }
    let stale = client.reconcile_detailed(snapshot_world(&reference[0]));
    assert_eq!(stale.correction, Correction::Stale { tick: 1 });
    assert_eq!(encode_world_payload(client.predicted()), payload(&reference[2]));

    let mut ahead = Client::with_limits(
        initial_world(),
        HARNESS_SEED,
        RollbackLimits {
            history_ticks: 4,
            max_resim_ticks: RESIM_BUDGET,
        },
    );
    ahead.predict(input_for(0));
    ahead.predict(input_for(1));
    ahead.acknowledge(0);
    let replay = ahead.reconcile_detailed(snapshot_world(&reference[0]));
    assert_eq!(
        replay.correction,
        Correction::RolledBack {
            tick: 1,
            resimulated: 1
        }
    );
    assert_eq!(encode_world_payload(ahead.predicted()), payload(&reference[1]));
}

#[test]
fn reversed_player_commands_and_late_or_dropped_packets_diverge() {
    let reference = just_in_time(false);

    let mut reversed = open_session();
    deliver(
        &mut reversed,
        &TickOrder {
            tick: 0,
            player: 0,
            commands: vec![
                Command::Move {
                    x: FX_ONE * 10,
                    y: 0,
                },
                Command::Select { x: 0, y: 0 },
            ],
        },
    );
    for order in orders_for(0).into_iter().filter(|order| order.player != 0) {
        deliver(&mut reversed, &order);
    }
    let first = reversed.advance();
    assert_ne!(first, reference[0]);
    let world = snapshot_world(&first);
    assert_eq!(world.unit(1).unwrap().x, 0);
    assert_eq!(world.unit(2).unwrap().x, FX_ONE * 3 + MOVE_SPEED);

    let mut late = open_session();
    for tick in 0..2 {
        for order in orders_for(tick) {
            deliver(&mut late, &order);
        }
        assert_eq!(late.advance(), reference[tick as usize]);
    }
    let without_tick_2 = late.advance();
    assert_ne!(without_tick_2, reference[2]);
    let mut due = orders_for(2);
    let tick_2 = due.remove(0);
    assert!(matches!(
        late.receive(
            peer_for(tick_2.player),
            &encode(&Datagram::Input {
                tick: tick_2.tick,
                player: tick_2.player,
                payload: encode_input_payload(&tick_2.commands),
            }),
        ),
        Err(SessionError::Tick(_))
    ));
    assert_eq!(late.snapshot(), without_tick_2);

    let mut dropped = open_session();
    for order in orders_for(0).into_iter().filter(|order| order.player == 0) {
        deliver(&mut dropped, &order);
    }
    let missing_player = dropped.advance();
    assert_ne!(missing_player, reference[0]);
    assert_eq!(snapshot_world(&missing_player).unit(3).unwrap().y, FX_ONE);
}
