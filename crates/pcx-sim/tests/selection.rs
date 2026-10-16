use pcx_sim::{Command, FX_ONE, Input, PlayerInput, Rng, World, step};

fn apply(world: &mut World, commands: Vec<Command>) {
    step(
        world,
        &Input {
            tick: world.tick,
            commands: vec![PlayerInput {
                player: 0,
                commands,
            }],
        },
        &mut Rng::new(3),
    );
}

#[test]
fn selected_move_preserves_unselected_and_foreign_orders() {
    let mut world = World::empty();
    let selected = world.spawn(0, 0, 0);
    let other = world.spawn(0, 5 * FX_ONE, 0);
    let foreign = world.spawn(1, 0, 0);
    apply(
        &mut world,
        vec![
            Command::Select { x: 0, y: 0 },
            Command::Move {
                x: 9 * FX_ONE,
                y: 0,
            },
        ],
    );
    assert!(world.unit(selected).unwrap().selected);
    assert!(!world.unit(other).unwrap().selected);
    assert!(world.unit(foreign).unwrap().selected);
    assert_eq!(
        world.unit(selected).unwrap().move_target,
        Some((9 * FX_ONE, 0))
    );
    assert_eq!(world.unit(other).unwrap().move_target, None);
    assert_eq!(world.unit(foreign).unwrap().move_target, None);
}

#[test]
fn stop_applies_to_the_new_selection_without_erasing_an_older_order() {
    let mut world = World::empty();
    let near = world.spawn(0, 0, 0);
    let far = world.spawn(0, 5 * FX_ONE, 0);
    apply(
        &mut world,
        vec![Command::Move {
            x: 20 * FX_ONE,
            y: 0,
        }],
    );
    apply(
        &mut world,
        vec![
            Command::Select { x: 0, y: 0 },
            Command::Move {
                x: 30 * FX_ONE,
                y: 0,
            },
            Command::Stop,
        ],
    );
    assert_eq!(world.unit(near).unwrap().move_target, None);
    assert_eq!(world.unit(far).unwrap().move_target, Some((20 * FX_ONE, 0)));
    assert!(!world.unit(far).unwrap().selected);
}

#[test]
fn selection_keeps_the_existing_strict_axis_boundary() {
    let mut world = World::empty();
    let inside = world.spawn(0, FX_ONE - 1, FX_ONE - 1);
    let edge = world.spawn(0, FX_ONE, 0);
    apply(&mut world, vec![Command::Select { x: 0, y: 0 }]);
    assert!(world.unit(inside).unwrap().selected);
    assert!(!world.unit(edge).unwrap().selected);
}
