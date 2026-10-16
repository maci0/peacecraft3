use pcx_net::Peer;
use pcx_net::session::Session;
use pcx_sim::{FX_ONE, World};

#[test]
fn disconnect_stops_unselected_owned_units_at_the_next_tick() {
    let mut world = World::empty();
    let owned = world.spawn(0, 0, 0);
    let foreign = world.spawn(1, 0, 0);
    let outside_session_slots = world.spawn(200, 0, 0);
    for unit in &mut world.units {
        unit.selected = false;
        unit.move_target = Some((10 * FX_ONE, 0));
    }
    let mut session = Session::new(world, 7);
    session.join(Peer(17), 0).unwrap();
    session.leave(Peer(17));
    assert_eq!(session.world().tick, 0);
    assert_eq!(
        session.world().unit(owned).unwrap().move_target,
        Some((10 * FX_ONE, 0))
    );
    session.advance();
    assert_eq!(session.world().tick, 1);
    let unit = session.world().unit(owned).unwrap();
    assert_eq!(
        unit.x, 0,
        "disconnect must stop before another movement tick"
    );
    assert_eq!(unit.move_target, None);
    assert!(
        !unit.selected,
        "server stop must preserve the player's selection"
    );
    for id in [foreign, outside_session_slots] {
        let unit = session.world().unit(id).unwrap();
        assert!(unit.x > 0);
        assert!(unit.move_target.is_some());
        assert!(!unit.selected);
    }
}

#[test]
fn a_rejoined_player_can_issue_new_orders_after_the_pending_disconnect_stop() {
    use pcx_net::{Datagram, encode, encode_input_payload};
    use pcx_sim::Command;

    let mut world = World::empty();
    let unit = world.spawn(0, 0, 0);
    world.units[0].selected = false;
    world.units[0].move_target = Some((10 * FX_ONE, 0));
    let mut session = Session::new(world, 7);
    session.join(Peer(17), 0).unwrap();
    session.leave(Peer(17));
    session.join(Peer(18), 0).unwrap();
    session
        .receive(
            Peer(18),
            &encode(&Datagram::Input {
                tick: 0,
                player: 0,
                payload: encode_input_payload(&[
                    Command::Select { x: 0, y: 0 },
                    Command::Move {
                        x: 20 * FX_ONE,
                        y: 0,
                    },
                ]),
            }),
        )
        .unwrap();
    session.advance();
    let unit = session.world().unit(unit).unwrap();
    assert!(unit.selected);
    assert!(unit.x > 0);
    assert_eq!(unit.move_target, Some((20 * FX_ONE, 0)));
}

#[test]
fn leaving_an_unknown_peer_does_not_stop_a_connected_player() {
    let mut world = World::empty();
    let unit = world.spawn(0, 0, 0);
    world.units[0].selected = false;
    world.units[0].move_target = Some((10 * FX_ONE, 0));
    let mut session = Session::new(world, 7);
    session.join(Peer(17), 0).unwrap();
    session.leave(Peer(18));
    session.advance();
    assert!(session.world().unit(unit).unwrap().x > 0);
    assert_eq!(
        session.world().unit(unit).unwrap().move_target,
        Some((10 * FX_ONE, 0))
    );
}
