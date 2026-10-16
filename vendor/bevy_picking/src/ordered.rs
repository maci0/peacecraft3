//! Event-local backend resolution feeding the existing pointer event state machine.
use crate::{
    backend::{HitData, PointerHits, ray::RayMap},
    events::pointer_events_step,
    hover::{PointerCaptureMap, resolve_groups},
    pointer::{Location, PointerAction, PointerInput},
};
use bevy_camera::{Camera, RenderTarget};
use bevy_ecs::{entity::EntityHashMap, message::MessageCursor, prelude::*, system::SystemId};
use bevy_platform::collections::{HashMap, HashSet};
use bevy_transform::prelude::GlobalTransform;
use bevy_window::PrimaryWindow;

/// Opts the configured, registered picking backends into ordered input resolution.
/// Every active backend must register its event-local resolver before using this marker.
#[derive(Resource, Default)]
pub struct OrderedPointerInput;

/// An event location together with rays built by the normal camera path.
#[derive(Clone)]
pub struct InputHitRequest {
    /// Pointer associated with this individual input.
    pub pointer: crate::pointer::PointerId,
    /// This input's own location, independent of final frame pointer state.
    pub location: Location,
    /// Rays derived from that same location.
    pub rays: RayMap,
}
/// Registered normal-backend resolvers; no entity action callbacks are stored here.
#[derive(Resource, Default)]
pub struct InputHitResolvers {
    resolvers: Vec<SystemId<In<InputHitRequest>, Vec<PointerHits>>>,
}
impl InputHitResolvers {
    /// Register the backend's shared hit-at-location system once at plugin setup.
    pub fn register(&mut self, resolver: SystemId<In<InputHitRequest>, Vec<PointerHits>>) {
        if !self.resolvers.contains(&resolver) {
            self.resolvers.push(resolver);
        }
    }
}

pub(crate) struct DispatchStep {
    pub frame: bool,
    pub ordered: bool,
    pub inputs: Vec<PointerInput>,
    pub hits: EntityHashMap<HitData>,
    pub locations: HashMap<crate::pointer::PointerId, Location>,
}
/// Persistent cursor and registered systems owned by the normal dispatcher.
#[derive(Default)]
pub struct PointerEventDriver {
    cursor: MessageCursor<PointerInput>,
    step: Option<SystemId<In<DispatchStep>>>,
    rays: Option<SystemId<In<(crate::pointer::PointerId, Location)>, RayMap>>,
    refresh: Option<[SystemId; 3]>,
    hover: crate::hover::HoverMap,
    locations: HashMap<crate::pointer::PointerId, Location>,
    presses: HashMap<crate::pointer::PointerId, (Entity, crate::pointer::PointerPressState)>,
}
fn event_rays(
    In((pointer, location)): In<(crate::pointer::PointerId, Location)>,
    primary: Query<Entity, With<PrimaryWindow>>,
    cameras: Query<(Entity, &Camera, &RenderTarget, &GlobalTransform)>,
) -> RayMap {
    RayMap::for_location(pointer, &location, &primary, &cameras)
}
fn resolve(
    world: &mut World,
    rays: SystemId<In<(crate::pointer::PointerId, Location)>, RayMap>,
    resolvers: &[SystemId<In<InputHitRequest>, Vec<PointerHits>>],
    pointer: crate::pointer::PointerId,
    location: &Location,
) -> EntityHashMap<HitData> {
    let rays = world
        .run_system_with(rays, (pointer, location.clone()))
        .expect("event ray system registered");
    let request = InputHitRequest {
        pointer,
        location: location.clone(),
        rays,
    };
    let mut groups = Vec::new();
    for resolver in resolvers {
        groups.extend(
            world
                .run_system_with(*resolver, request.clone())
                .expect("event hit resolver registered"),
        );
    }
    let mut hits = resolve_groups(groups, |entity| {
        world.get::<crate::Pickable>(entity).cloned()
    });
    hits.retain(|entity, _| world.get_entity(*entity).is_ok());
    if let Some((entity, hit)) = world.resource::<PointerCaptureMap>().get(&pointer) {
        let (entity, hit) = (entity, hit.clone());
        if world.get_entity(entity).is_ok() {
            hits.clear();
            hits.insert(entity, hit);
        }
    }
    hits
}
fn publish_hover(world: &mut World, hover: &crate::hover::HoverMap, refresh: &[SystemId; 3]) {
    let previous = world.resource::<crate::hover::HoverMap>().0.clone();
    world.resource_mut::<crate::hover::PreviousHoverMap>().0 = previous;
    world.resource_mut::<crate::hover::HoverMap>().0 = hover.0.clone();
    for system in refresh {
        world
            .run_system(*system)
            .expect("hover component system registered");
    }
}
/// Dispatches input through registered normal hit resolvers and one existing state machine.
/// Observer commands flush between inputs; this never replays a schedule or layout system.
pub fn pointer_events(world: &mut World, mut driver: Local<PointerEventDriver>) {
    let inputs: Vec<_> = driver
        .cursor
        .read(world.resource::<Messages<PointerInput>>())
        .cloned()
        .collect();
    let step = *driver
        .step
        .get_or_insert_with(|| world.register_system(pointer_events_step));
    if !world.contains_resource::<OrderedPointerInput>() {
        world
            .run_system_with(
                step,
                DispatchStep {
                    frame: true,
                    ordered: false,
                    inputs,
                    hits: Default::default(),
                    locations: Default::default(),
                },
            )
            .expect("pointer dispatcher registered");
        return;
    }
    let rays = *driver
        .rays
        .get_or_insert_with(|| world.register_system(event_rays));
    let refresh = *driver.refresh.get_or_insert_with(|| {
        [
            world.register_system(crate::hover::update_interactions),
            world.register_system(crate::hover::update_is_hovered),
            world.register_system(crate::hover::update_is_directly_hovered),
        ]
    });
    let resolvers = world.resource::<InputHitResolvers>().resolvers.clone();
    // Backend frame hover is final-position-only. Preserve our last delivered map instead.
    world.resource_mut::<crate::hover::HoverMap>().0 = driver.hover.0.clone();
    let snapshots = {
        let mut query = world.query::<(
            Entity,
            &crate::pointer::PointerId,
            &crate::pointer::PointerLocation,
            &crate::pointer::PointerPressState,
        )>();
        query
            .iter(world)
            .map(|(entity, id, location, pressed)| (entity, *id, location.clone(), pressed.clone()))
            .collect::<Vec<_>>()
    };
    let active: HashSet<_> = snapshots.iter().map(|(_, id, _, _)| *id).collect();
    driver.locations.retain(|id, _| active.contains(id));
    driver.presses.retain(|id, _| active.contains(id));
    for (entity, id, _, _) in &snapshots {
        let pressed = driver
            .presses
            .get(id)
            .filter(|(previous_entity, _)| previous_entity == entity)
            .map(|(_, pressed)| pressed.clone())
            .unwrap_or_default();
        if let Some(mut state) = world.get_mut::<crate::pointer::PointerPressState>(*entity) {
            *state = pressed;
        }
    }
    let mut canceled = HashSet::new();
    for input in inputs {
        if canceled.contains(&input.pointer_id) {
            continue;
        }
        if let Some(entity) = world
            .resource::<crate::pointer::PointerMap>()
            .get_entity(input.pointer_id)
        {
            if let Some(mut location) = world.get_mut::<crate::pointer::PointerLocation>(entity) {
                location.location = Some(input.location.clone());
            }
            if let Some(mut pressed) = world.get_mut::<crate::pointer::PointerPressState>(entity) {
                pressed.apply_action(input.action);
            }
        }
        let hits = resolve(world, rays, &resolvers, input.pointer_id, &input.location);
        driver
            .locations
            .insert(input.pointer_id, input.location.clone());
        driver.hover.insert(input.pointer_id, hits.clone());
        publish_hover(world, &driver.hover, &refresh);
        let release = matches!(
            input.action,
            PointerAction::Release(_) | PointerAction::Cancel
        );
        let cancel = matches!(input.action, PointerAction::Cancel);
        let pointer = input.pointer_id;
        world
            .run_system_with(
                step,
                DispatchStep {
                    frame: true,
                    ordered: true,
                    inputs: vec![input],
                    hits,
                    locations: driver.locations.clone(),
                },
            )
            .expect("pointer dispatcher registered");
        if release {
            world.resource_mut::<PointerCaptureMap>().release(pointer);
        }
        if cancel {
            canceled.insert(pointer);
        }
    }
    // Restore the real final input components after observer-visible event-local publication.
    for (entity, id, location, mut pressed) in snapshots {
        if canceled.contains(&id) {
            pressed = crate::pointer::PointerPressState::default();
        }
        if let Some(mut current) = world.get_mut::<crate::pointer::PointerLocation>(entity) {
            *current = location;
        }
        if let Some(mut current) = world.get_mut::<crate::pointer::PointerPressState>(entity) {
            *current = pressed.clone();
        }
        driver.presses.insert(id, (entity, pressed));
    }
    // Synchronize final live hover (including touch's absent location) without another input dispatch.
    let final_pointers = {
        let mut query =
            world.query::<(&crate::pointer::PointerId, &crate::pointer::PointerLocation)>();
        query
            .iter(world)
            .map(|(id, loc)| (*id, loc.location.clone()))
            .collect::<Vec<_>>()
    };
    driver.hover.0.clear();
    for (pointer, location) in final_pointers {
        if let Some(location) = location {
            if canceled.contains(&pointer) {
                continue;
            }
            let hits = resolve(world, rays, &resolvers, pointer, &location);
            driver.hover.insert(pointer, hits);
        }
    }
    publish_hover(world, &driver.hover, &refresh);
    world
        .run_system_with(
            step,
            DispatchStep {
                frame: true,
                ordered: true,
                inputs: Vec::new(),
                hits: Default::default(),
                locations: driver.locations.clone(),
            },
        )
        .expect("pointer dispatcher registered");
}
