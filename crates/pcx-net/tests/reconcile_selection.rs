use pcx_net::Client;
use pcx_net::rollback::{Correction, RollbackLimits, SnapReason};
use pcx_sim::{Command, FX_ONE, Input, PlayerInput, World};

fn commands(tick: u64, commands: Vec<Command>) -> Input {
    Input {
        tick,
        commands: vec![PlayerInput {
            player: 0,
            commands,
        }],
    }
}

fn two_units() -> (World, u32, u32) {
    let mut world = World::empty();
    let first = world.spawn(0, 0, 0);
    let second = world.spawn(0, 4 * FX_ONE, 0);
    world.tick = 1;
    (world, first, second)
}

#[test]
fn older_snapshot_replays_unacknowledged_select_instead_of_copying_flags() {
    let (world, first, second) = two_units();
    let mut client = Client::from_world(world.clone(), 7);
    client.acknowledge(0);
    client.predict(commands(1, vec![Command::Select { x: 0, y: 0 }]));
    assert_eq!(client.predicted().tick, 2);
    assert!(client.predicted().unit(first).unwrap().selected);
    assert!(!client.predicted().unit(second).unwrap().selected);
    let report = client.reconcile_detailed(world);
    assert_eq!(report.predicted_tick, 2);
    assert_eq!(
        report.correction,
        Correction::RolledBack {
            tick: 1,
            resimulated: 1
        }
    );
    assert!(client.predicted().unit(first).unwrap().selected);
    assert!(!client.predicted().unit(second).unwrap().selected);
    assert_eq!(client.predicted().tick, 2);
}

#[test]
fn same_tick_authority_replaces_selection_and_target_without_replay() {
    let (world, first, second) = two_units();
    let mut client = Client::from_world(world, 7);
    client.acknowledge(0);
    client.predict(commands(1, vec![Command::Select { x: 0, y: 0 }]));
    let mut snapshot = client.predicted().clone();
    snapshot.units.iter_mut().for_each(|unit| {
        unit.selected = true;
        unit.move_target = Some((8, 8));
    });
    snapshot.units.swap(0, 1);
    let order_before = client
        .predicted()
        .units
        .iter()
        .map(|unit| unit.id)
        .collect::<Vec<_>>();
    let report = client.reconcile_detailed(snapshot.clone());
    assert_eq!(report.correction, Correction::Aligned);
    assert_eq!(report.predicted_tick, 2);
    assert_eq!(client.predicted().tick, 2);
    assert_eq!(client.predicted(), &snapshot);
    assert_ne!(
        client
            .predicted()
            .units
            .iter()
            .map(|unit| unit.id)
            .collect::<Vec<_>>(),
        order_before
    );
    assert!(client.predicted().unit(first).unwrap().selected);
    assert!(client.predicted().unit(second).unwrap().selected);
    assert_eq!(
        client.predicted().unit(first).unwrap().move_target,
        Some((8, 8))
    );
}

#[test]
fn select_then_move_replays_in_command_order_for_the_addressed_unit() {
    let (world, first, second) = two_units();
    let mut client = Client::from_world(world.clone(), 7);
    client.acknowledge(0);
    client.predict(commands(
        1,
        vec![
            Command::Select {
                x: 4 * FX_ONE,
                y: 0,
            },
            Command::Move {
                x: 9 * FX_ONE,
                y: 0,
            },
        ],
    ));
    let after_predict = client.predicted().clone();
    assert!(after_predict.unit(second).unwrap().selected);
    assert!(!after_predict.unit(first).unwrap().selected);
    assert!(after_predict.unit(second).unwrap().x > 4 * FX_ONE);
    assert_eq!(after_predict.unit(first).unwrap().x, 0);
    let report = client.reconcile_detailed(world);
    assert_eq!(
        report.correction,
        Correction::RolledBack {
            tick: 1,
            resimulated: 1
        }
    );
    assert_eq!(client.predicted(), &after_predict);
    assert_eq!(
        client
            .predicted()
            .units
            .iter()
            .map(|unit| unit.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
}

#[test]
fn acknowledged_snapshot_is_stale_and_keeps_the_newer_selection() {
    let (world, _first, second) = two_units();
    let mut client = Client::from_world(world.clone(), 7);
    client.acknowledge(0);
    client.predict(commands(1, vec![Command::Select { x: 0, y: 0 }]));
    client.acknowledge(1);
    let report = client.reconcile_detailed(world);
    assert_eq!(report.correction, Correction::Stale { tick: 1 });
    assert_eq!(report.predicted_tick, 2);
    assert!(!client.predicted().unit(second).unwrap().selected);
    assert_eq!(client.pending_inputs(), 0);
    assert_eq!(client.consensus_tick(), 1);
}

#[test]
fn snapshot_behind_consensus_does_not_rewrite_selection() {
    let (world, _first, second) = two_units();
    let mut client = Client::from_world(world.clone(), 7);
    client.acknowledge(0);
    client.predict(commands(1, vec![Command::Select { x: 0, y: 0 }]));
    let current = client.predicted().clone();
    let _ = client.reconcile_detailed(current.clone());
    assert_eq!(client.consensus_tick(), 2);
    let report = client.reconcile_detailed(world);
    assert_eq!(report.correction, Correction::Stale { tick: 1 });
    assert!(!client.predicted().unit(second).unwrap().selected);
    assert_eq!(client.predicted(), &current);
}

#[test]
fn resim_budget_snaps_to_the_snapshot_instead_of_a_partial_replay() {
    let (world, _first, second) = two_units();
    let mut client = Client::with_limits(
        world.clone(),
        7,
        RollbackLimits {
            history_ticks: 8,
            max_resim_ticks: 1,
        },
    );
    client.acknowledge(0);
    client.predict(commands(1, vec![Command::Select { x: 0, y: 0 }]));
    client.predict(commands(
        2,
        vec![Command::Select {
            x: 4 * FX_ONE,
            y: 0,
        }],
    ));
    client.predict(commands(3, vec![Command::Stop]));
    assert_eq!(client.predicted().tick, 4);
    assert!(client.predicted().unit(second).unwrap().selected);
    let report = client.reconcile_detailed(world.clone());
    assert_eq!(
        report.correction,
        Correction::Snapped {
            tick: 1,
            reason: SnapReason::ResimBudget
        }
    );
    assert_eq!(client.predicted(), &world);
    assert!(client.predicted().unit(second).unwrap().selected);
    assert_eq!(client.pending_inputs(), 0);
    assert!(client.history().len() <= 8);
}

#[test]
fn missing_ack_and_empty_history_snap_without_replaying_selection() {
    let (world, _first, second) = two_units();
    let mut unconfirmed = Client::from_world(world.clone(), 7);
    unconfirmed.predict(commands(1, vec![Command::Select { x: 0, y: 0 }]));
    let report = unconfirmed.reconcile_detailed(world.clone());
    assert_eq!(
        report.correction,
        Correction::Snapped {
            tick: 1,
            reason: SnapReason::Unconfirmed
        }
    );
    assert_eq!(unconfirmed.predicted(), &world);
    assert!(unconfirmed.predicted().unit(second).unwrap().selected);
    assert_eq!(unconfirmed.pending_inputs(), 0);

    let mut no_history = Client::with_limits(
        world.clone(),
        7,
        RollbackLimits {
            history_ticks: 0,
            max_resim_ticks: 8,
        },
    );
    no_history.acknowledge(0);
    no_history.predict(commands(1, vec![Command::Select { x: 0, y: 0 }]));
    let report = no_history.reconcile_detailed(world.clone());
    assert_eq!(
        report.correction,
        Correction::Snapped {
            tick: 1,
            reason: SnapReason::OutsideWindow
        }
    );
    assert_eq!(no_history.predicted(), &world);
    assert_eq!(no_history.pending_inputs(), 0);
}
