use super::*;
use crate::scripts::AssetIdentity;
use crate::unit_placement::{
    Footprint, Movement, Placement, StationaryKey, StationaryStamp, tests::placement,
};
use pcx_map::Pathing;
use pcx_sim::world::Source;

const OBJECT: u32 = 0x3000;
const REGION: u32 = 0x2000;
const REMOVAL: u32 = 0;
const ACTIVE: u32 = 0x100;
const REGISTERED: u32 = 0x200;
const MOVING: u32 = 0x40;
/// Pathing masks of the placement test rules: ground, air.
const GROUND: u32 = 8;
const AIR: u32 = 4;

fn id(n: u64) -> HandleId {
    HandleId::new(std::num::NonZeroU64::new(n).unwrap())
}

fn link(identity: u64, class: u32) -> ChainLink {
    ChainLink { identity, class }
}

fn source(map: &str) -> Source {
    Source {
        install: "synthetic".into(),
        edition: "test".into(),
        map: map.into(),
    }
}

/// The placement test grid: 32 by 32 cells of 4 units from the origin.
pub(crate) fn pool() -> PoolOwner {
    PoolOwner::new(
        SpatialRules::synthetic(EntryRules::synthetic(7)),
        Geometry {
            origin: [0.; 2],
            cell_size: 4.,
            width: 32,
            height: 32,
        },
        0.,
        0.25,
    )
    .unwrap()
}

fn tracked(map: &str) -> Placement {
    let mut p = placement(source(map), 10000);
    p.attach_spatial(pool());
    p
}

fn chain(p: &Placement, cell: [usize; 2]) -> Vec<ChainLink> {
    p.spatial().unwrap().chain(cell).unwrap().collect()
}

fn footprint_loader(cells: Vec<[i32; 2]>) -> impl FnMut(&str) -> Result<Footprint, String> {
    move |_| {
        Ok(Footprint {
            origin: AssetIdentity {
                source: "synthetic".into(),
                edition: "test".into(),
                version: "test".into(),
            },
            cells: cells.clone(),
        })
    }
}

#[test]
fn a_unit_registers_moves_and_retires_in_the_simulation_order() {
    let mut p = tracked("unit order");
    let stamp = p.prepare([32., 40.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    // Radius 2 at (32, 40): native cells [9, 11) by [7, 9), axes transposed.
    for cell in [[9, 7], [10, 7], [9, 8], [10, 8]] {
        assert_eq!(chain(&p, cell), [link(1, OBJECT)]);
    }
    assert!(chain(&p, [11, 7]).is_empty());
    let pool = p.spatial().unwrap();
    assert_eq!(pool.live().sequence(), 1);
    assert_eq!(
        pool.live().entry_state(1),
        Some((ACTIVE | GROUND, 0)),
        "a movement entry is active, carries the ground mask, and has no registration bit"
    );

    let moved = p.prepare_motion(id(1), [64., 40.], 2.).unwrap().unwrap();
    p.commit(id(1), moved);
    assert_eq!(
        chain(&p, [9, 7]),
        [link(1, REMOVAL), link(1, OBJECT)],
        "the cell the unit left holds the removal link above the old object link"
    );
    // Radius 2 at (64, 40): native cells [9, 11) by [15, 17).
    assert_eq!(chain(&p, [9, 15]), [link(1, OBJECT)]);
    assert_eq!(p.spatial().unwrap().live().sequence(), 2);

    // A commit that leaves the stamp where it is registers nothing.
    let same = p.prepare_motion(id(1), [64., 40.], 2.).unwrap().unwrap();
    p.commit(id(1), same);
    assert_eq!(p.spatial().unwrap().live().sequence(), 2);

    p.hide(id(1)).unwrap();
    assert_eq!(chain(&p, [9, 15]), [link(1, REMOVAL), link(1, OBJECT)]);
    let pool = p.spatial().unwrap();
    assert_eq!(pool.live().sequence(), 3);
    assert_eq!(pool.live().entry_state(1), None);
    assert!(pool.refusal().is_none());
}

#[test]
fn units_of_different_movement_classes_share_one_ordered_chain() {
    let mut p = tracked("interleave");
    let ground = p.prepare([32., 40.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), ground);
    let air = p
        .prepare_class([32., 40.], Some(2.), "_", 0., Movement::Air)
        .unwrap();
    p.commit(id(2), air);
    // The newest registration heads the chain.
    assert_eq!(chain(&p, [9, 7]), [link(2, OBJECT), link(1, OBJECT)]);
    let live = p.spatial().unwrap().live();
    assert_eq!(live.sequence(), 2);
    assert_eq!(live.entry_state(2), Some((ACTIVE | AIR, 0)));
}

#[test]
fn a_shown_unit_registers_a_new_entry_after_it_was_hidden() {
    let mut p = tracked("show");
    let stamp = p.prepare([32., 40.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    p.hide(id(1)).unwrap();
    let again = p
        .prepare_show(id(1), [32., 40.], Some(2.), "_", 0.)
        .unwrap();
    p.commit(id(1), again);
    assert_eq!(
        chain(&p, [9, 7]),
        [link(1, OBJECT), link(1, REMOVAL), link(1, OBJECT)]
    );
    assert_eq!(p.spatial().unwrap().live().sequence(), 3);
    assert!(p.spatial().unwrap().refusal().is_none());
}

#[test]
fn a_structure_footprint_is_a_blocking_texture_entry_that_hide_deactivates() {
    let mut p = placement(source("structure"), 10000)
        .with_footprint_loader(footprint_loader(vec![[0, 0], [1, 0], [0, 1]]));
    p.attach_spatial(pool());
    let stamp = p.prepare([32., 32.], None, "farm.tga", 0.).unwrap();
    p.commit(id(5), stamp);
    // Pathing cells (8, 8), (9, 8), (8, 9) become native cells [8, 8], [8, 9], [9, 8].
    for cell in [[8, 8], [8, 9], [9, 8]] {
        assert_eq!(chain(&p, cell), [link(5, OBJECT)]);
    }
    assert!(chain(&p, [9, 9]).is_empty());
    let live = p.spatial().unwrap().live();
    assert_eq!(
        live.sequence(),
        2,
        "two row runs, two ordered registrations"
    );
    assert_eq!(live.entry_state(5), Some((ACTIVE | GROUND, REGISTERED)));
    assert!(live.blocked([8, 8], 0, GROUND).unwrap());
    assert!(!live.blocked([8, 8], 0, AIR).unwrap());

    p.hide(id(5)).unwrap();
    let live = p.spatial().unwrap().live();
    assert_eq!(
        live.entry_state(5),
        Some((GROUND, REGISTERED)),
        "hiding clears only the active flag"
    );
    assert!(!live.blocked([8, 8], 0, GROUND).unwrap());
    assert_eq!(chain(&p, [8, 8]), [link(5, REMOVAL), link(5, OBJECT)]);

    let again = p
        .prepare_show(id(5), [32., 32.], None, "farm.tga", 0.)
        .unwrap();
    p.commit(id(5), again);
    let live = p.spatial().unwrap().live();
    assert!(live.blocked([8, 8], 0, GROUND).unwrap());
    assert!(p.spatial().unwrap().refusal().is_none());
}

#[test]
fn stationary_layers_swap_their_cells_when_the_object_dies() {
    let mut p = placement(source("stationary"), 10000)
        .with_stationary(
            Pathing {
                version: 0,
                width: 32,
                height: 32,
                cells: vec![0; 1024],
            },
            vec![StationaryStamp {
                key: StationaryKey::Placed(3),
                live: vec![100],
                dead: vec![200],
                mask: 0x10,
                active: true,
            }],
        )
        .unwrap();
    let mut owner = pool();
    owner.texture(Texture::Placed(3), 0x10, &[100]);
    p.attach_spatial(owner);
    let placed = PLACED_TAG | 3;
    // Pathing cell 100 is native cell [3, 4]; cell 200 is [6, 8].
    assert_eq!(chain(&p, [3, 4]), [link(placed, OBJECT)]);

    assert!(p.set_placed_active(3, false));
    assert_eq!(
        chain(&p, [3, 4]),
        [link(placed, REMOVAL), link(placed, OBJECT)]
    );
    assert_eq!(chain(&p, [6, 8]), [link(placed, OBJECT)]);

    assert!(p.set_placed_active(3, true));
    assert_eq!(
        chain(&p, [6, 8]),
        [link(placed, REMOVAL), link(placed, OBJECT)]
    );
    assert_eq!(chain(&p, [3, 4])[0], link(placed, OBJECT));
    assert!(p.spatial().unwrap().refusal().is_none());
}

#[test]
fn region_rectangles_register_once_then_extend_the_entry() {
    let mut owner = pool();
    owner.region_added(id(9), [-4., 2., 8., 10.]);
    owner.region_added(id(9), [14., 26., 26., 34.]);
    owner.region_added(id(10), [0., 0., 3., 3.]);
    // World y selects native axis 0: [0, 3) by [0, 3), then [6, 9) by [3, 7).
    assert_eq!(
        owner.chain([0, 0]).unwrap().collect::<Vec<_>>(),
        [link(10, REGION), link(9, REGION)]
    );
    assert_eq!(
        owner.chain([6, 3]).unwrap().collect::<Vec<_>>(),
        [link(9, REGION)]
    );
    assert!(owner.chain([9, 3]).unwrap().next().is_none());
    assert_eq!(owner.live().sequence(), 3);
    assert_eq!(owner.live().entry_state(9), Some((0, REGISTERED)));
    assert!(owner.refusal().is_none());
}

#[test]
fn a_refused_operation_stops_the_stream_and_names_the_first_failure() {
    let mut owner = pool();
    owner.region_added(id(9), [f32::NAN, 0., 1., 1.]);
    let refusal = *owner.refusal().unwrap();
    assert_eq!(refusal.identity, 9);
    assert_eq!(refusal.error, ChainError::Coordinates);
    owner.region_added(id(10), [0., 0., 3., 3.]);
    assert_eq!(owner.refusal(), Some(&refusal));
    assert_eq!(owner.live().sequence(), 0);
    assert!(owner.chain([0, 0]).unwrap().next().is_none());
}

#[test]
fn a_mover_that_becomes_a_texture_owner_is_refused() {
    let mut p =
        placement(source("morph"), 10000).with_footprint_loader(footprint_loader(vec![[0, 0]]));
    p.attach_spatial(pool());
    let mover = p.prepare([32., 40.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), mover);
    let structure = p.prepare([32., 40.], None, "farm.tga", 0.).unwrap();
    p.commit(id(1), structure);
    let refusal = p.spatial().unwrap().refusal().unwrap();
    assert_eq!(refusal.error, ChainError::DuplicateEntry(1));
    assert_eq!(refusal.operation, "register texture");
}

#[test]
fn walking_state_follows_commanded_velocity_and_refuses_other_contributors() {
    let mut p = tracked("velocity");
    let stamp = p.prepare([32., 40.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    let pool = p.spatial_mut().unwrap();
    pool.command_velocity(id(1), [3., 0.], 3., None);
    assert_eq!(pool.live().entry_state(1).unwrap().1 & MOVING, MOVING);
    assert_eq!(pool.moving_units().collect::<Vec<_>>(), [id(1)]);
    // An unchanged command is not reapplied.
    let before = pool.live().entry_state(1);
    pool.command_velocity(id(1), [3., 0.], 3., None);
    assert_eq!(pool.live().entry_state(1), before);
    pool.command_velocity(id(1), [0., 0.], 0., None);
    assert_eq!(pool.live().entry_state(1).unwrap().1 & MOVING, 0);
    assert_eq!(pool.moving_units().count(), 0);
    // A unit the pool does not track is ignored.
    pool.command_velocity(id(7), [3., 0.], 3., Some("sleep"));
    assert!(pool.refusal().is_none());
    pool.command_velocity(id(1), [3., 0.], 3., Some("active buff"));
    let refusal = pool.refusal().unwrap();
    assert_eq!(refusal.identity, 1);
    assert_eq!(
        refusal.error,
        ChainError::MovementContributor {
            identity: 1,
            contributor: "active buff"
        }
    );
}

#[test]
fn cleanup_runs_every_due_deadline_and_recycles_removal_links() {
    let mut p = tracked("cleanup");
    let stamp = p.prepare([32., 40.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    let moved = p.prepare_motion(id(1), [64., 40.], 2.).unwrap().unwrap();
    p.commit(id(1), moved);
    let pool = p.spatial_mut().unwrap();
    pool.cleanup(0.2);
    assert_eq!(
        pool.chain([9, 7]).unwrap().count(),
        2,
        "no deadline is due before 0.25"
    );
    pool.cleanup(0.6);
    assert!(
        pool.chain([9, 7]).unwrap().next().is_none(),
        "the removal link suppresses the old object link and both are recycled"
    );
    assert_eq!(pool.chain([9, 15]).unwrap().count(), 1);
    assert_eq!(pool.live().next_cleanup(), 0.75);
}

#[test]
fn untracked_placements_hold_no_owner() {
    let mut p = placement(source("untracked"), 10000);
    let stamp = p.prepare([32., 40.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    p.hide(id(1)).unwrap();
    assert!(p.spatial().is_none());
}
