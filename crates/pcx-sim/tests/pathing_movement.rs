use pcx_map::grid_alignment::terrain_pathing_alignment;
use pcx_map::{GridAlignmentError, Pathing};
use pcx_sim::pathing::{MAX_CELLS_PER_AXIS, MAX_SEGMENT_CHECKS};
use pcx_sim::{
    AdmitError, CellQuery, Command, Input, MOVE_SPEED, MovementProfile, PlayerInput, Rng, World,
    admit_aligned_movement, admit_movement, step,
};

fn raster(width: u32, height: u32, cells: Vec<u8>) -> Pathing {
    Pathing {
        version: 0,
        width,
        height,
        cells,
    }
}

fn profile(origin: (i32, i32), tile: i32, mask: u8) -> MovementProfile {
    MovementProfile {
        origin_x: origin.0,
        origin_y: origin.1,
        terrain_tile_fx: tile,
        blocked_bits: mask,
    }
}

fn admit_len(
    width: u32,
    height: u32,
    cells: Vec<u8>,
    origin: (i32, i32),
    tile: i32,
    mask: u8,
) -> std::sync::Arc<pcx_sim::AdmittedMovement> {
    let bound = cells.len();
    admit_movement(
        &raster(width, height, cells),
        width,
        height,
        profile(origin, tile, mask),
        bound,
    )
    .unwrap()
}

fn order(world: &mut World, command: Command) {
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
fn unadmitted_step_crosses_the_cell_an_admitted_step_stops_before() {
    let mut open = World::empty();
    let mut blocked = World::empty();
    let id_open = open.spawn(0, 100, 0);
    let id_blocked = blocked.spawn(0, 100, 0);
    blocked.movement = Some(admit_len(4, 1, vec![0, 0, 0x02, 0], (0, 0), 2_000, 0x02));
    let command = Command::Move { x: 10_000, y: 0 };
    order(&mut open, command.clone());
    order(&mut blocked, command);
    assert_eq!(open.unit(id_open).unwrap().x, 100 + MOVE_SPEED);
    let stopped = blocked.unit(id_blocked).unwrap();
    assert_eq!((stopped.x, stopped.y), (3_999, 0));
    assert!(stopped.move_target.is_some());
    assert!(matches!(
        blocked
            .movement
            .as_ref()
            .unwrap()
            .locate(stopped.x, stopped.y),
        CellQuery::Inside {
            x: 1,
            blocked: false,
            ..
        }
    ));
}

#[test]
fn a_unit_already_in_a_blocked_cell_does_not_advance() {
    let mut world = World::empty();
    world.movement = Some(admit_len(4, 1, vec![0, 0, 0x02, 0], (0, 0), 2_000, 0x02));
    let id = world.spawn(0, 4_000, 0);
    order(&mut world, Command::Move { x: 10_000, y: 0 });
    assert_eq!(world.unit(id).unwrap().x, 4_000);
    assert!(matches!(
        world.movement.as_ref().unwrap().locate(4_000, 0),
        CellQuery::Inside {
            x: 2,
            blocked: true,
            ..
        }
    ));
}

#[test]
fn a_long_move_cannot_tunnel_through_a_blocked_cell() {
    let mut world = World::empty();
    let id = world.spawn(0, 100, 0);
    world.movement = Some(admit_len(4, 1, vec![0, 0, 0x02, 0], (0, 0), 8_000, 0x02));
    for _ in 0..8 {
        let before = world.unit(id).unwrap().clone();
        order(&mut world, Command::Move { x: 40_000, y: 0 });
        let after = world.unit(id).unwrap();
        assert!(after.x < 16_000);
        assert!(after.x >= before.x);
        assert!(matches!(
            world.movement.as_ref().unwrap().locate(after.x, after.y),
            CellQuery::Inside { blocked: false, .. }
        ));
    }
    let unit = world.unit(id).unwrap();
    assert_eq!((unit.x, unit.y), (15_999, 0));
    assert_eq!(unit.move_target, Some((40_000, 0)));
}

#[test]
fn diagonal_step_stops_at_the_blocked_cell_and_passes_a_side_neighbor() {
    let mut blocked = World::empty();
    let mut side = World::empty();
    let wall = {
        let mut cells = vec![0u8; 16];
        cells[5] = 0x02;
        cells
    };
    let neighbor = {
        let mut cells = vec![0u8; 16];
        cells[1] = 0x02;
        cells
    };
    blocked.movement = Some(admit_len(4, 4, wall, (0, 0), 4_000, 0x02));
    side.movement = Some(admit_len(4, 4, neighbor, (0, 0), 4_000, 0x02));
    let blocked_id = blocked.spawn(0, 500, 500);
    let side_id = side.spawn(0, 3_500, 3_500);
    order(
        &mut blocked,
        Command::Move {
            x: 20_000,
            y: 20_000,
        },
    );
    order(&mut side, Command::Move { x: 4_500, y: 4_500 });
    assert_eq!(
        (
            blocked.unit(blocked_id).unwrap().x,
            blocked.unit(blocked_id).unwrap().y
        ),
        (3_999, 3_999)
    );
    assert_eq!(
        (side.unit(side_id).unwrap().x, side.unit(side_id).unwrap().y),
        (4_500, 4_500)
    );
}

#[test]
fn bits_outside_the_admitted_mask_do_not_block() {
    let mut world = World::empty();
    let id = world.spawn(0, 100, 0);
    world.movement = Some(admit_len(
        4,
        1,
        vec![0x00, 0x80, 0x02, 0x01],
        (0, 0),
        2_000,
        0x02,
    ));
    order(&mut world, Command::Move { x: 10_000, y: 0 });
    assert_eq!(world.unit(id).unwrap().x, 3_999);
}

#[test]
fn negative_coordinates_edges_and_extremes_are_explicit() {
    let movement = admit_len(4, 1, vec![0, 0, 0, 0], (0, 0), 2_000, 0x02);
    assert!(matches!(movement.locate(-1, 0), CellQuery::NegativeX));
    assert!(matches!(movement.locate(0, -1), CellQuery::NegativeY));
    assert!(matches!(movement.locate(-4, -4), CellQuery::NegativeX));
    assert!(matches!(movement.locate(i32::MIN, 0), CellQuery::NegativeX));
    assert!(matches!(movement.locate(8_000, 0), CellQuery::PastEdgeX));
    assert!(matches!(movement.locate(0, 2_000), CellQuery::PastEdgeY));
    assert!(matches!(
        movement.locate(i32::MAX, i32::MAX),
        CellQuery::PastEdgeX
    ));
    assert!(matches!(
        movement.locate(1_999, 0),
        CellQuery::Inside { x: 0, y: 0, .. }
    ));
    assert!(matches!(
        movement.locate(2_000, 0),
        CellQuery::Inside { x: 1, y: 0, .. }
    ));

    let mut outside = World::empty();
    outside.movement = Some(movement.clone());
    let id = outside.spawn(0, -10, 0);
    order(&mut outside, Command::Move { x: 5_000, y: 0 });
    assert_eq!(outside.unit(id).unwrap().x, -10);

    let mut left = World::empty();
    left.movement = Some(movement.clone());
    let id = left.spawn(0, 100, 0);
    order(&mut left, Command::Move { x: -10_000, y: 0 });
    assert_eq!(left.unit(id).unwrap().x, 0);
    order(&mut left, Command::Nop);
    assert_eq!(left.unit(id).unwrap().x, 0);

    let mut right = World::empty();
    right.movement = Some(movement);
    let id = right.spawn(0, 7_000, 0);
    order(&mut right, Command::Move { x: 20_000, y: 0 });
    assert_eq!(right.unit(id).unwrap().x, 7_999);
}

#[test]
fn negative_origin_and_derived_scale_place_the_blocked_cell() {
    let movement = admit_movement(
        &raster(4, 2, vec![0, 0, 0x02, 0, 0, 0, 0, 0]),
        2,
        1,
        profile((-8_000, -8_000), 4_000, 0x02),
        8,
    )
    .unwrap();
    assert_eq!(movement.alignment().uniform_scale, 2);
    assert_eq!(movement.cell_fx(), 2_000);
    let mut world = World::empty();
    world.movement = Some(movement);
    let id = world.spawn(0, -7_000, -8_000);
    order(&mut world, Command::Move { x: 0, y: -8_000 });
    assert_eq!(world.unit(id).unwrap().x, -4_001);
}

#[test]
fn a_clear_admission_matches_the_harness_step() {
    let mut harness = World::empty();
    let mut admitted = World::empty();
    let harness_id = harness.spawn(0, 1_000, 1_000);
    let admitted_id = admitted.spawn(0, 1_000, 1_000);
    admitted.movement = Some(admit_len(6, 2, vec![0; 12], (0, 0), 4_000, 0x02));
    let mut rng_a = Rng::new(9);
    let mut rng_b = Rng::new(9);
    for _ in 0..6 {
        let input = Input {
            tick: harness.tick,
            commands: vec![PlayerInput {
                player: 0,
                commands: vec![Command::Move {
                    x: 12_000,
                    y: 1_000,
                }],
            }],
        };
        step(&mut harness, &input, &mut rng_a);
        step(&mut admitted, &input, &mut rng_b);
        assert_eq!(
            harness.unit(harness_id).unwrap().x,
            admitted.unit(admitted_id).unwrap().x
        );
        assert_eq!(
            harness.unit(harness_id).unwrap().y,
            admitted.unit(admitted_id).unwrap().y
        );
    }
    assert_eq!(harness.unit(harness_id).unwrap().x, 12_000);
    assert!(harness.unit(harness_id).unwrap().move_target.is_none());
    assert_eq!(rng_a.next_u64(), rng_b.next_u64());
}

#[test]
fn repeated_admission_is_deterministic_and_bounded() {
    let movement = admit_len(4, 1, vec![0, 0, 0x02, 0], (0, 0), 2_000, 0x02);
    let first = movement.clamp_step(100, 0, MOVE_SPEED, 0);
    let second = movement.clamp_step(100, 0, MOVE_SPEED, 0);
    assert_eq!(first, second);
    assert_eq!((first.x, first.y), (3_999, 0));
    assert_eq!(first.checks, 6);
    assert!(first.checks <= MAX_SEGMENT_CHECKS);

    let diagonal = {
        let mut cells = vec![0u8; 16];
        cells[5] = 0x02;
        admit_len(4, 4, cells, (0, 0), 4_000, 0x02)
    };
    let diagonal_step = diagonal.clamp_step(500, 500, MOVE_SPEED, MOVE_SPEED);
    assert_eq!((diagonal_step.x, diagonal_step.y), (3_999, 3_999));
    assert_eq!(diagonal_step.checks, 5);

    let side = {
        let mut cells = vec![0u8; 16];
        cells[1] = 0x02;
        admit_len(4, 4, cells, (0, 0), 4_000, 0x02)
    };
    let side_step = side.clamp_step(3_500, 3_500, 1_000, 1_000);
    assert_eq!((side_step.x, side_step.y), (4_500, 4_500));
    assert_eq!(side_step.checks, 4);

    let wide = admit_len(20, 1, vec![0; 20], (0, 0), 2_000, 0x02);
    let refused = wide.clamp_step(0, 0, 40_000, 0);
    assert_eq!((refused.x, refused.y), (0, 0));
    assert!(refused.checks <= MAX_SEGMENT_CHECKS);
    let ordinary = wide.clamp_step(0, 0, MOVE_SPEED, 0);
    assert_eq!(ordinary.x, MOVE_SPEED);
    assert!(i64::from(MOVE_SPEED) / 2_000 + 2 <= i64::from(MAX_CELLS_PER_AXIS));

    let long = admit_len(4, 1, vec![0, 0, 0x02, 0], (0, 0), 8_000, 0x02);
    let hit = long.clamp_step(13_206, 0, MOVE_SPEED, 0);
    assert_eq!(hit.x, 15_999);
    assert_eq!(hit.checks, 3);
}

#[test]
fn the_same_seed_and_input_match() {
    let movement = admit_len(4, 1, vec![0, 0, 0x02, 0], (0, 0), 8_000, 0x02);
    let mut a = World::empty();
    a.movement = Some(movement.clone());
    a.spawn(0, 100, 0);
    let mut b = a.clone();
    let mut rng_a = Rng::new(9);
    let mut rng_b = Rng::new(9);
    for _ in 0..12 {
        let input = Input {
            tick: a.tick,
            commands: vec![PlayerInput {
                player: 0,
                commands: vec![Command::Move { x: 40_000, y: 4 }],
            }],
        };
        step(&mut a, &input, &mut rng_a);
        step(&mut b, &input, &mut rng_b);
        assert_eq!(a, b);
    }
    assert_eq!(rng_a.next_u64(), rng_b.next_u64());
}

#[test]
fn stop_still_clears_an_admitted_order() {
    let mut world = World::empty();
    world.movement = Some(admit_len(4, 1, vec![0; 4], (0, 0), 4_000, 0x02));
    let id = world.spawn(0, 100, 0);
    order(&mut world, Command::Move { x: 3_000, y: 0 });
    let moved = world.unit(id).unwrap().x;
    order(&mut world, Command::Stop);
    let unit = world.unit(id).unwrap();
    assert_eq!(unit.x, moved);
    assert!(unit.move_target.is_none());
}

#[test]
fn admission_refuses_unknown_masks_and_bad_grids() {
    let open = profile((0, 0), 2_000, 0x02);
    assert!(matches!(
        admit_movement(&raster(1, 1, vec![0]), 1, 1, profile((0, 0), 2_000, 0), 1),
        Err(AdmitError::EmptyMask)
    ));
    assert!(matches!(
        admit_movement(&raster(1, 1, vec![0]), 1, 1, profile((0, 0), 0, 0x02), 1),
        Err(AdmitError::NonPositiveTile(0))
    ));
    assert!(matches!(
        admit_movement(&raster(1, 1, vec![0]), 1, 1, profile((0, 0), -5, 0x02), 1),
        Err(AdmitError::NonPositiveTile(-5))
    ));
    assert!(matches!(
        admit_movement(&raster(3, 2, vec![0; 6]), 2, 2, open, 6),
        Err(AdmitError::Alignment(
            GridAlignmentError::NonDivisibleWidth { .. }
        ))
    ));
    assert!(matches!(
        admit_movement(&raster(4, 1, vec![0; 4]), 2, 1, open, 4),
        Err(AdmitError::Alignment(
            GridAlignmentError::ScaleMismatch { .. }
        ))
    ));
    assert!(matches!(
        admit_movement(&raster(0, 1, vec![]), 1, 1, open, 0),
        Err(AdmitError::Alignment(GridAlignmentError::ZeroPathingWidth))
    ));
    assert!(matches!(
        admit_movement(&raster(2, 2, vec![0; 4]), 1, 1, profile((0, 0), 5, 0x02), 4),
        Err(AdmitError::TileScale { .. })
    ));
    assert!(matches!(
        admit_movement(&raster(1, 1, vec![0]), 1, 1, profile((0, 0), 1, 0x02), 1),
        Err(AdmitError::StepTooFine { .. })
    ));
    assert!(matches!(
        admit_movement(&raster(2, 2, vec![0; 3]), 2, 2, open, 4),
        Err(AdmitError::RasterBytes { .. })
    ));
    assert!(matches!(
        admit_movement(&raster(65_536, 65_536, vec![]), 65_536, 65_536, open, 0),
        Err(AdmitError::RasterOverflow { .. })
    ));
    assert!(matches!(
        admit_movement(&raster(3_000_000_000, 3_000_000_000, vec![]), 1, 1, open, 0),
        Err(AdmitError::ScaleOverflow(_))
    ));
    assert!(matches!(
        admit_movement(
            &raster(4, 2, vec![0; 8]),
            2,
            1,
            profile((0, 0), 4_000, 0x02),
            7
        ),
        Err(AdmitError::Capacity { bytes: 8, bound: 7 })
    ));
}

#[test]
fn supplied_alignment_must_match_the_raster() {
    let cells = vec![0, 0, 0x02, 0, 0, 0, 0, 0];
    let alignment = terrain_pathing_alignment(2, 1, 4, 2).unwrap();
    let admitted = admit_aligned_movement(
        &raster(4, 2, cells),
        &alignment,
        profile((-8_000, -8_000), 4_000, 0x02),
        8,
    )
    .unwrap();
    assert_eq!(admitted.cell_fx(), 2_000);
    assert_eq!(admitted.alignment().uniform_scale, 2);
    let mut tampered = alignment;
    tampered.uniform_scale = 1;
    assert!(matches!(
        admit_aligned_movement(
            &raster(4, 2, vec![0; 8]),
            &tampered,
            profile((0, 0), 4_000, 0x02),
            8
        ),
        Err(AdmitError::AlignmentMismatch)
    ));
}

#[test]
fn a_fixed_point_sum_outside_i32_stays_put() {
    let movement = admit_len(4, 1, vec![0; 4], (0, 0), 8_000, 0x02);
    let stayed = movement.clamp_step(i32::MAX - 10, 0, MOVE_SPEED, 0);
    assert_eq!((stayed.x, stayed.y), (i32::MAX - 10, 0));
    assert_eq!(stayed.checks, 0);
}
