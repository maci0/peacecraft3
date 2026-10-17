//! Wire round-trip and rewind keep one movement admission.
use std::sync::Arc;

use pcx_map::Pathing;
use pcx_net::rollback::Correction;
use pcx_net::{Client, Server, decode_world_payload, encode_world_payload};
use pcx_sim::{Command, Input, MOVE_SPEED, MovementProfile, PlayerInput, World, admit_movement};

fn raster(cells: Vec<u8>) -> Pathing {
    let width = cells.len() as u32;
    Pathing {
        version: 0,
        width,
        height: 1,
        cells,
    }
}

fn admitted(cells: Vec<u8>, tile: i32, mask: u8) -> Arc<pcx_sim::AdmittedMovement> {
    let bound = cells.len();
    let width = cells.len() as u32;
    admit_movement(
        &raster(cells),
        width,
        1,
        MovementProfile {
            origin_x: 0,
            origin_y: 0,
            terrain_tile_fx: tile,
            blocked_bits: mask,
        },
        bound,
    )
    .unwrap()
}

fn order(tick: u64, x: i32) -> Input {
    Input {
        tick,
        commands: vec![PlayerInput {
            player: 0,
            commands: vec![Command::Move { x, y: 0 }],
        }],
    }
}

fn step_both(server: &mut Server, client: &mut Client, tick: u64, x: i32) {
    let input = order(tick, x);
    server.accept(input.clone()).unwrap();
    assert!(server.advance());
    client.predict(input);
}

#[test]
fn a_decoded_snapshot_reconciles_onto_the_same_wall() {
    let movement = admitted(vec![0, 0, 0x02, 0], 8_000, 0x02);
    let mut world = World::empty();
    world.movement = Some(Arc::clone(&movement));
    let id = world.spawn(0, 100, 0);
    let mut server = Server::new(world.clone(), 3);
    let mut client = Client::from_world(world, 3);
    step_both(&mut server, &mut client, 0, 40_000);
    step_both(&mut server, &mut client, 1, 40_000);
    assert_eq!(server.world().unit(id).unwrap().x, 100 + 2 * MOVE_SPEED);

    let bytes = encode_world_payload(server.world());
    let decoded = decode_world_payload(server.world().tick, &bytes).unwrap();
    assert!(decoded.movement.is_none());
    assert_eq!(
        decoded.unit(id).unwrap().x,
        server.world().unit(id).unwrap().x
    );
    assert_eq!(encode_world_payload(&decoded), bytes);

    let (bound, divergence) = client.reconcile(decoded);
    assert_eq!(divergence, 0);
    assert!(Arc::ptr_eq(bound.movement.as_ref().unwrap(), &movement));
    assert!(Arc::ptr_eq(
        client.predicted().movement.as_ref().unwrap(),
        &movement
    ));

    step_both(&mut server, &mut client, 2, 40_000);
    step_both(&mut server, &mut client, 3, 40_000);
    let client_x = client.predicted().unit(id).unwrap().x;
    let server_x = server.world().unit(id).unwrap().x;
    assert_eq!(client_x, server_x);
    assert_eq!(client_x, 15_999);
    assert!(client_x < 16_000);
}

#[test]
fn a_different_admission_is_not_stepped() {
    let movement = admitted(vec![0, 0, 0x02, 0], 2_000, 0x02);
    let foreign = admitted(vec![0, 0, 0x02, 0], 2_000, 0x04);
    let mut world = World::empty();
    world.movement = Some(movement);
    let id = world.spawn(0, 100, 0);
    let mut client = Client::from_world(world, 1);
    let mut snapshot = client.predicted().clone();
    snapshot.movement = Some(foreign);
    let report = client.reconcile_detailed(snapshot);
    assert_eq!(report.correction, Correction::AdmissionMismatch { tick: 0 });
    assert_eq!(client.predicted().unit(id).unwrap().x, 100);
    client.predict(order(0, 10_000));
    assert_eq!(client.predicted().unit(id).unwrap().x, 3_999);
}

#[test]
fn unadmitted_bytes_and_rewind_keep_the_harness_step() {
    let mut world = World::empty();
    let id = world.spawn(0, 100, 0);
    let mut server = Server::new(world, 1);
    for tick in 0..4 {
        server.accept(order(tick, 40_000)).unwrap();
        assert!(server.advance());
    }
    let restored = server.snapshot(2).unwrap().unit(id).unwrap().x;
    assert!(server.resync_to(2));
    assert!(server.world().movement.is_none());
    assert_eq!(server.world().unit(id).unwrap().x, restored);
    assert_eq!(restored, 100 + 2 * MOVE_SPEED);
    let bytes = encode_world_payload(server.world());
    let decoded = decode_world_payload(server.world().tick, &bytes).unwrap();
    assert!(decoded.movement.is_none());
    assert_eq!(decoded.unit(id).unwrap().x, restored);
    assert_eq!(encode_world_payload(&decoded), bytes);
}

#[test]
fn admitted_rewind_retains_the_shared_raster() {
    let movement = admitted(vec![0, 0, 0x02, 0], 8_000, 0x02);
    let mut world = World::empty();
    world.movement = Some(Arc::clone(&movement));
    let id = world.spawn(0, 100, 0);
    let mut server = Server::new(world, 1);
    for tick in 0..3 {
        server.accept(order(tick, 40_000)).unwrap();
        assert!(server.advance());
    }
    assert_eq!(server.world().unit(id).unwrap().x, 15_999);
    assert!(server.resync_to(1));
    assert!(Arc::ptr_eq(
        server.world().movement.as_ref().unwrap(),
        &movement
    ));
    assert_eq!(server.world().unit(id).unwrap().x, 100 + MOVE_SPEED);
    server.accept(order(1, 40_000)).unwrap();
    assert!(server.advance());
    server.accept(order(2, 40_000)).unwrap();
    assert!(server.advance());
    assert_eq!(server.world().unit(id).unwrap().x, 15_999);
    assert!(Arc::ptr_eq(
        server.world().movement.as_ref().unwrap(),
        &movement
    ));
}
