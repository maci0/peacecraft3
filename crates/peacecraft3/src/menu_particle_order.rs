//! Ordered submission seam only. No particle meshes, images, clocks or source defaults.
use bevy::{
    core_pipeline::core_3d::Transparent3d,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_phase::{SortedRenderPhase, ViewSortedRenderPhases, sort_phase_system},
        sync_world::{MainEntity, MainEntityHashMap},
        view::ExtractedView,
    },
};
use std::{collections::HashSet, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceKind {
    Geometry,
    Particle,
}

/// Every field identifies an authored source record, except the explicit draw-kind tie convention.
#[derive(Clone, Copy, Debug)]
pub struct SourceOrder {
    pub owner: MainEntity,
    pub priority_plane: i32,
    pub kind: SourceKind,
    pub ordinal: u32,
    pub sort_far_z: bool,
}

/// Source batches on their actual camera; each owner retains its own phase slots.
#[derive(Component, Clone, Debug, ExtractComponent)]
#[extract_app(RenderApp)]
pub struct SourceOrderView {
    pub owners: Arc<[MainEntity]>,
    pub source: Arc<str>,
    pub orders: Arc<MainEntityHashMap<SourceOrder>>,
    pub max_phase_items: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderFailure {
    Budget,
    Allocation,
    DuplicateOrdinal,
    DuplicateEntity,
}

/// Reorders only this owner's existing phase slots. Untagged items keep their slots and get no invented plane.
///
/// Authored `SortFarZ` batches sort far-to-near by their actual submitted
/// distance instead of refusing: the CPU mesh already sorts quads within each
/// emitter (`menu_particles::sort_pieces`), and this orders the emitter batches
/// themselves. Non-flagged items keep the explicit record order. Distances are
/// Bevy's submitted values, not invented planes.
pub fn order_phase(
    phase: &mut SortedRenderPhase<Transparent3d>,
    owner: MainEntity,
    orders: &MainEntityHashMap<SourceOrder>,
    max_phase_items: usize,
) -> Result<usize, OrderFailure> {
    if phase.items.len() > max_phase_items || orders.len() > max_phase_items {
        return Err(OrderFailure::Budget);
    }
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(phase.items.len())
        .map_err(|_| OrderFailure::Allocation)?;
    for (slot, (_, item)) in phase.items.iter().enumerate() {
        let Some(order) = orders
            .get(&item.entity.1)
            .filter(|order| order.owner == owner)
        else {
            continue;
        };
        selected.push((slot, item.entity.1, *order, item.distance));
    }
    selected.sort_by(|a, b| {
        a.2.priority_plane.cmp(&b.2.priority_plane).then_with(|| {
            match (a.2.sort_far_z, b.2.sort_far_z) {
                // Far-to-near on the submitted distance; ties keep record order.
                (true, true) => {
                    b.3.total_cmp(&a.3)
                        .then_with(|| a.2.kind.cmp(&b.2.kind))
                        .then_with(|| a.2.ordinal.cmp(&b.2.ordinal))
                }
                (true, false) | (false, true) => {
                    a.2.kind
                        .cmp(&b.2.kind)
                        .then_with(|| a.2.ordinal.cmp(&b.2.ordinal))
                }
                (false, false) => {
                    a.2.kind
                        .cmp(&b.2.kind)
                        .then_with(|| a.2.ordinal.cmp(&b.2.ordinal))
                }
            }
        })
    });
    let mut seen = std::collections::HashSet::new();
    for (_, _, order, _) in &selected {
        // `SourceKind` carries no `Hash`, so the check keys on its explicit
        // declaration order instead.
        if !seen.insert((order.kind as u8, order.ordinal)) {
            return Err(OrderFailure::DuplicateOrdinal);
        }
    }
    // `selected` already carries the recreation order: signed priority, then
    // geometry/PRE2 class and declaration order, with authored `SortFarZ`
    // batches far-to-near on their submitted distance.
    let mut slots = Vec::new();
    slots
        .try_reserve_exact(selected.len())
        .map_err(|_| OrderFailure::Allocation)?;
    slots.extend(selected.iter().map(|(slot, _, _, _)| *slot));
    slots.sort_unstable();
    let mut ranks = MainEntityHashMap::default();
    ranks
        .try_reserve(phase.items.len())
        .map_err(|_| OrderFailure::Allocation)?;
    for (slot, (_, item)) in phase.items.iter().enumerate() {
        if ranks.insert(item.entity.1, slot).is_some() {
            return Err(OrderFailure::DuplicateEntity);
        }
    }
    for (slot, (_, entity, _, _)) in slots.into_iter().zip(selected.iter()) {
        ranks.insert(*entity, slot);
    }
    phase
        .items
        .sort_unstable_by_key(|_, item| ranks[&item.entity.1]);
    Ok(selected.len())
}

pub struct MenuParticleOrderPlugin;
impl Plugin for MenuParticleOrderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<SourceOrderView>::default());
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                order_backgrounds
                    .in_set(RenderSystems::PhaseSort)
                    .after(sort_phase_system::<Transparent3d>),
            );
        }
    }
}

fn order_backgrounds(
    views: Query<(&ExtractedView, &SourceOrderView)>,
    mut phases: ResMut<ViewSortedRenderPhases<Transparent3d>>,
    mut reported: Local<HashSet<MainEntity>>,
) {
    reported.retain(|owner| views.iter().any(|(_, view)| view.owners.contains(owner)));
    for (view, source) in &views {
        if let Some(phase) = phases.get_mut(&view.retained_view_entity) {
            order_view(phase, source, &mut reported);
        }
    }
}

/// Orders every owner's slots of one view's phase. An owner with no item in the phase
/// reorders nothing, so only owners present in it need a pass; a refused budget or a
/// repeated entity fails every owner, which is reported for each.
fn order_view(
    phase: &mut SortedRenderPhase<Transparent3d>,
    source: &SourceOrderView,
    reported: &mut HashSet<MainEntity>,
) {
    let mut present = HashSet::new();
    let mut repeated = false;
    for item in phase.items.values() {
        repeated |= !present.insert(item.entity.1);
    }
    let all_owners = phase.items.len() > source.max_phase_items
        || source.orders.len() > source.max_phase_items
        || repeated;
    let item_owners: HashSet<MainEntity> = present
        .iter()
        .filter_map(|entity| source.orders.get(entity))
        .map(|order| order.owner)
        .collect();
    for &owner in source.owners.iter() {
        if !all_owners && !item_owners.contains(&owner) {
            continue;
        }
        if let Err(failure) = order_phase(phase, owner, &source.orders, source.max_phase_items) {
            // Keep geometry; a refused particle batch must never fall back to Bevy's distance-only submission.
            phase.items.retain(|_, item| {
                !source
                    .orders
                    .get(&item.entity.1)
                    .is_some_and(|order| order.owner == owner && order.kind == SourceKind::Particle)
            });
            if reported.insert(owner) {
                error!("{}: PRE2 submission disabled: {:?}", source.source, failure);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::core_pipeline::core_3d::TransparentSortingInfo3d;
    use bevy::render::{
        render_phase::{DrawFunctionId, PhaseItemExtraIndex},
        render_resource::CachedRenderPipelineId,
    };
    fn entities() -> (World, Vec<MainEntity>) {
        let mut world = World::new();
        let ids = (0..8).map(|_| world.spawn_empty().id().into()).collect();
        (world, ids)
    }
    fn item(id: MainEntity, distance: f32) -> Transparent3d {
        Transparent3d {
            sorting_info: TransparentSortingInfo3d::Sorted {
                mesh_center: Vec3::new(0., 0., distance),
                depth_bias: 0.,
            },
            distance,
            pipeline: CachedRenderPipelineId::INVALID,
            entity: (Entity::PLACEHOLDER, id),
            draw_function: DrawFunctionId(0),
            batch_range: 0..1,
            extra_index: PhaseItemExtraIndex::None,
            indexed: true,
        }
    }
    fn add(phase: &mut SortedRenderPhase<Transparent3d>, ids: &[MainEntity]) {
        for (i, id) in ids.iter().enumerate() {
            phase.add_retained(item(*id, i as f32));
        }
    }
    fn key(owner: MainEntity, priority: i32, kind: SourceKind, ordinal: u32) -> SourceOrder {
        SourceOrder {
            owner,
            priority_plane: priority,
            kind,
            ordinal,
            sort_far_z: false,
        }
    }
    fn ids(phase: &SortedRenderPhase<Transparent3d>) -> Vec<MainEntity> {
        phase.items.values().map(|i| i.entity.1).collect()
    }
    #[test]
    fn source_signed_planes_override_distance_without_changing_geometry_or_other_slots() {
        let (_world, e) = entities();
        let mut phase = SortedRenderPhase::default();
        add(&mut phase, &e[..5]);
        let orders = MainEntityHashMap::from_iter([
            (e[0], key(e[7], i32::MAX, SourceKind::Particle, 9)),
            (e[2], key(e[7], i32::MIN, SourceKind::Geometry, 3)),
            (e[4], key(e[7], 0, SourceKind::Particle, 1)),
        ]);
        let before: Vec<_> = phase
            .items
            .values()
            .map(|i| (i.entity.1, i.distance, i.sorting_info))
            .collect();
        assert_eq!(order_phase(&mut phase, e[7], &orders, 5), Ok(3));
        assert_eq!(ids(&phase), vec![e[2], e[1], e[4], e[3], e[0]]);
        for (entity, distance, _) in before {
            assert_eq!(
                phase
                    .items
                    .get(&(Entity::PLACEHOLDER, entity))
                    .unwrap()
                    .distance,
                distance
            );
        }
    }
    #[test]
    fn view_pass_matches_ordering_every_owner_including_absent_ones() {
        let (_world, e) = entities();
        let owners = [e[6], e[7]];
        let build = || {
            let mut phase = SortedRenderPhase::default();
            add(&mut phase, &e[..5]);
            phase
        };
        let orders = MainEntityHashMap::from_iter([
            (e[0], key(e[7], i32::MAX, SourceKind::Particle, 9)),
            (e[2], key(e[7], i32::MIN, SourceKind::Geometry, 3)),
            (e[4], key(e[7], 0, SourceKind::Particle, 1)),
        ]);
        for max_phase_items in [8, 2] {
            let source = SourceOrderView {
                owners: owners.as_slice().into(),
                source: "test".into(),
                orders: Arc::new(orders.clone()),
                max_phase_items,
            };
            let mut expected = build();
            for owner in owners {
                if order_phase(&mut expected, owner, &orders, max_phase_items).is_err() {
                    expected.items.retain(|_, item| {
                        !orders.get(&item.entity.1).is_some_and(|order| {
                            order.owner == owner && order.kind == SourceKind::Particle
                        })
                    });
                }
            }
            let mut actual = build();
            order_view(&mut actual, &source, &mut HashSet::new());
            assert_eq!(ids(&actual), ids(&expected), "budget {max_phase_items}");
        }
    }
    #[test]
    fn ties_use_explicit_record_order_and_other_owners_stay_untouched() {
        let (_world, e) = entities();
        let mut phase = SortedRenderPhase::default();
        add(&mut phase, &e[..4]);
        let orders = MainEntityHashMap::from_iter([
            (e[0], key(e[7], 0, SourceKind::Particle, 2)),
            (e[1], key(e[7], 0, SourceKind::Geometry, 9)),
            (e[2], key(e[7], 0, SourceKind::Particle, 1)),
            (e[3], key(e[6], -99, SourceKind::Geometry, 0)),
        ]);
        order_phase(&mut phase, e[7], &orders, 4).unwrap();
        assert_eq!(ids(&phase), vec![e[1], e[2], e[0], e[3]]);
    }
    #[test]
    fn budget_and_duplicate_source_records_refuse_transactionally() {
        let (_world, e) = entities();
        let mut phase = SortedRenderPhase::default();
        add(&mut phase, &e[..2]);
        let before = ids(&phase);
        let mut orders = MainEntityHashMap::from_iter([
            (e[0], key(e[7], 0, SourceKind::Particle, 1)),
            (e[1], key(e[7], 0, SourceKind::Particle, 2)),
        ]);
        assert_eq!(
            order_phase(&mut phase, e[7], &orders, 1),
            Err(OrderFailure::Budget)
        );
        assert_eq!(ids(&phase), before);
        orders.get_mut(&e[1]).unwrap().ordinal = 1;
        assert_eq!(
            order_phase(&mut phase, e[7], &orders, 2),
            Err(OrderFailure::DuplicateOrdinal)
        );
        assert_eq!(ids(&phase), before);
    }
    #[test]
    fn flagged_batches_order_far_to_near_on_submitted_distance() {
        let (_world, e) = entities();
        let mut phase = SortedRenderPhase::default();
        // Distances are the slot indices: e[0] near (0.0), e[1] far (1.0).
        add(&mut phase, &e[..2]);
        let mut orders = MainEntityHashMap::from_iter([
            (e[0], key(e[7], 0, SourceKind::Particle, 1)),
            (e[1], key(e[7], 0, SourceKind::Particle, 2)),
        ]);
        for entity in [e[0], e[1]] {
            orders.get_mut(&entity).unwrap().sort_far_z = true;
        }
        assert_eq!(order_phase(&mut phase, e[7], &orders, 2), Ok(2));
        // Declaration ordinals say e[0] first; the authored depth policy wins.
        assert_eq!(ids(&phase), vec![e[1], e[0]]);
    }
}
