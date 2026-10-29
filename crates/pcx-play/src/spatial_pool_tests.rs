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

#[test]
fn town_search_operands_follow_selected_instructions() {
    let threshold = [0xc7, 0x45, 0xf8, 17, 0, 0, 0];
    let work = [0xbb, 29, 0, 0, 0];
    let increment = [0x83, 0xc0, 7];
    let merge = [0xc7, 0x45, 0xe0, 11, 0, 0, 0];
    assert_eq!(
        decode_town_operands(&threshold, &work, &increment, &merge),
        Ok((17, 29, 7, 11))
    );
    assert!(decode_town_operands(&threshold, &work, &[0x83, 0xc1, 7], &merge).is_err());
}

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

#[test]
fn texture_masks_follow_selected_instructions_and_green_pixels() {
    let plain = [
        0xc7, 0x45, 0xe4, 1, 0, 0, 0, 0x50, 0xc7, 0x45, 0xe8, 2, 0, 0, 0, 0xc7, 0x45, 0xec, 4, 0,
        0, 0,
    ];
    let green = [
        0xc7, 0x45, 0xe0, 1, 0, 0, 0, 0x50, 0xc7, 0x45, 0xe4, 2, 0, 0, 0, 0xc7, 0x45, 0xe8, 4, 0,
        0, 0, 0xc7, 0x45, 0xec, 8, 0, 0, 0,
    ];
    let masks = TextureMasks::from_instructions(&plain, &green).unwrap();
    assert_eq!(masks.for_pixels(&[1, 4, 0]), &[1, 2, 4]);
    assert_eq!(masks.for_pixels(&[1, 8, 4]), &[1, 2, 4, 8]);
    let mut invalid = green;
    invalid[3] = 2;
    assert!(TextureMasks::from_instructions(&plain, &invalid).is_err());
}

#[test]
fn texture_pixels_create_separate_ordered_entries_and_restore_them() {
    let mut pool = pool();
    let pixels = [(100, 1 | 4), (101, 4), (102, 8)];
    pool.set_texture_pixels(5, &pixels);
    let entries = pool.texture_entries[&5].clone();
    assert_eq!(
        entries.iter().map(|entry| entry.1).collect::<Vec<_>>(),
        [1, 2, 4, 8]
    );
    let links: Vec<_> = pool.chain([3, 4]).unwrap().collect();
    assert_eq!(
        links,
        [link(entries[2].0, OBJECT), link(entries[0].0, OBJECT)]
    );
    assert!(pool.live().blocked([3, 4], 0, 4).unwrap());
    pool.hide(id(5));
    assert!(!pool.live().blocked([3, 4], 0, 4).unwrap());
    pool.set_texture_pixels(5, &pixels);
    assert!(pool.live().blocked([3, 4], 0, 4).unwrap());
    assert!(pool.refusal().is_none());
}

#[test]
fn unknown_stationary_pixels_stop_the_live_pool() {
    let mut pool = pool();
    pool.stationary_active(Texture::Placed(12), true);
    assert_eq!(
        pool.refusal().unwrap().error,
        ChainError::MissingTexturePixels(PLACED_TAG | 12)
    );
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
fn a_unit_without_movement_uses_its_own_entry_mask() {
    let mut p = tracked("no movement");
    let stamp = p
        .prepare_class([32., 40.], Some(2.), "_", 0., Movement::NoMovement)
        .unwrap();
    p.commit(id(1), stamp);
    assert_eq!(
        p.spatial().unwrap().live().entry_state(1),
        Some((ACTIVE, 0))
    );
    assert_eq!(Movement::from_token("none"), Ok(Movement::NoMovement));
    assert_eq!(Movement::from_token("foot"), Ok(Movement::Ground));
}

#[test]
fn movement_classes_use_the_selected_entry_masks() {
    for (movement, expected) in [
        (Movement::Ground, 8),
        (Movement::NoMovement, 0),
        (Movement::Air, 4),
        (Movement::Float, 2),
        (Movement::Amphibious, 1),
    ] {
        let mut owner = pool();
        owner.commit(
            id(1),
            Stamp {
                active: true,
                movement,
                cells: &[0],
                footprint: &[],
                native_footprint: None,
            },
        );
        assert_eq!(owner.live.entry_state(1), Some((ACTIVE | expected, 0)));
    }
}

#[test]
fn movement_mask_changes_preserve_links_and_state() {
    let mut owner = pool();
    let rect = CellRect {
        lower: [2, 3],
        upper: [4, 5],
    };
    owner.set_mover(1, 8, rect);
    owner.live.update_state(1, ACTIVE | 8, MOVING).unwrap();
    owner.set_mover(1, 4, rect);
    assert_eq!(owner.live.entry_state(1), Some((ACTIVE | 4, MOVING)));
    assert_eq!(
        owner.chain([2, 3]).unwrap().collect::<Vec<_>>(),
        [link(1, OBJECT)]
    );
    assert_eq!(owner.live.sequence(), 1);
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
fn a_structure_without_retained_texture_pixels_is_refused() {
    let mut p = placement(source("structure"), 10000)
        .with_footprint_loader(footprint_loader(vec![[0, 0], [1, 0], [0, 1]]));
    p.attach_spatial(pool());
    let stamp = p.prepare([32., 32.], None, "farm.tga", 0.).unwrap();
    p.commit(id(5), stamp);
    assert_eq!(
        p.spatial().unwrap().refusal().unwrap().error,
        ChainError::MissingTexturePixels(5)
    );
    assert!(chain(&p, [8, 8]).is_empty());
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
    owner.stationary_pixels(Texture::Placed(3), vec![(100, 4)], vec![(200, 4)], true);
    let placed = owner.texture_entries[&(PLACED_TAG | 3)][2].0;
    p.attach_spatial(owner);
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
    pool.command_velocity(id(1), [3., 0.], 0., None);
    assert_eq!(pool.live().entry_state(1).unwrap().1 & MOVING, 0);
    pool.command_velocity(id(1), [3., 0.], 3., None);
    assert_eq!(pool.live().entry_state(1).unwrap().1 & MOVING, MOVING);
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
fn walking_velocity_uses_the_selected_scale() {
    let mut p = tracked("velocity scale");
    let stamp = p.prepare([32., 40.], Some(2.), "_", 0.).unwrap();
    p.commit(id(1), stamp);
    let pool = p.spatial_mut().unwrap();
    pool.rules.movement_scale = 0.25_f32.to_bits();
    pool.command_velocity(id(1), [4., 0.], 4., None);
    assert_eq!(pool.live().entry_state(1).unwrap().1 & MOVING, 0);
    pool.command_velocity(id(1), [8., 0.], 8., None);
    assert_eq!(pool.live().entry_state(1).unwrap().1 & MOVING, MOVING);
    assert!(pool.refusal().is_none());
}

#[test]
fn movement_scale_requires_matching_source_operands() {
    let immediate = (0_u32.wrapping_sub(2 << 23)).to_le_bytes();
    let mut x = [0_u8; 5];
    x[0] = 0x05;
    x[1..].copy_from_slice(&immediate);
    let mut y = [0_u8; 6];
    y[..2].copy_from_slice(&[0x81, 0xc1]);
    y[2..].copy_from_slice(&immediate);
    assert_eq!(movement_scale_operands(&x, &y, &x), Ok(0.25_f32.to_bits()));
    y[2] ^= 1;
    assert!(movement_scale_operands(&x, &y, &x).is_err());
    assert!(movement_scale_operands(&[], &y, &x).is_err());
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
