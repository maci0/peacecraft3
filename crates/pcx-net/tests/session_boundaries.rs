use pcx_net::{
    Datagram, Peer, encode, encode_input_payload,
    session::{MAX_COMMANDS_PER_TICK, MAX_PACKET_BYTES, Session},
};
use pcx_sim::{Command, FX_ONE, MAX_INPUT_LEAD, World};

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
