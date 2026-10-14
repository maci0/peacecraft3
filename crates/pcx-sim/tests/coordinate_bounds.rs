use pcx_sim::{Command, FX_ONE, Input, PlayerInput, Rng, World, step};

fn command(world: &mut World, command: Command) {
    step(
        world,
        &Input {
            tick: world.tick,
            commands: vec![PlayerInput {
                player: 0,
                commands: vec![command],
            }],
        },
        &mut Rng::new(1),
    );
}

#[test]
fn opposite_coordinate_extremes_move_towards_the_target() {
    let mut world = World::empty();
    let id = world.spawn(0, i32::MIN, i32::MAX);
    command(
        &mut world,
        Command::Move {
            x: i32::MAX,
            y: i32::MIN,
        },
    );
    let unit = world.unit(id).unwrap();
    assert_eq!(
        (unit.x, unit.y),
        (i32::MIN + FX_ONE / 10, i32::MAX - FX_ONE / 10)
    );
}

#[test]
fn minimum_signed_distance_is_represented_without_abs_overflow() {
    let mut world = World::empty();
    let id = world.spawn(0, 0, 0);
    command(
        &mut world,
        Command::Move {
            x: i32::MIN,
            y: i32::MAX,
        },
    );
    let unit = world.unit(id).unwrap();
    assert_eq!((unit.x, unit.y), (-FX_ONE / 10, FX_ONE / 10));
}

#[test]
fn movement_lands_at_both_representable_boundaries() {
    let mut world = World::empty();
    let id = world.spawn(0, i32::MIN + 3, i32::MAX - 3);
    command(
        &mut world,
        Command::Move {
            x: i32::MIN,
            y: i32::MAX,
        },
    );
    let unit = world.unit(id).unwrap();
    assert_eq!((unit.x, unit.y), (i32::MIN, i32::MAX));
    command(&mut world, Command::Nop);
    assert!(world.unit(id).unwrap().move_target.is_none());
}

#[test]
fn distant_selection_does_not_wrap_into_the_selection_radius() {
    for (position, selection) in [(i32::MIN, i32::MAX), (i32::MIN, 0)] {
        let mut world = World::empty();
        let id = world.spawn(0, position, position);
        world.units[0].move_target = Some((position + 1, position + 1));
        command(
            &mut world,
            Command::Select {
                x: selection,
                y: selection,
            },
        );
        let unit = world.unit(id).unwrap();
        assert_eq!(unit.move_target, Some((position + 1, position + 1)));
        assert_eq!((unit.x, unit.y), (position + 1, position + 1));
    }
}
