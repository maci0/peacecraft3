//! Draw canonical destructables through the same checked model pipeline as units.
use super::*;
use bevy::camera::primitives::Aabb;
use pcx_play::{
    playercolor_host::{DestructableCreation, DestructableVisual, ObjectHost},
    unit_model::{Animation, Model},
};

#[derive(Default)]
pub(super) struct Draw {
    pub(super) instances: BTreeMap<HandleId, Instance>,
    models: BTreeMap<(i32, i32), Result<Arc<Prepared>, String>>,
    bytes: usize,
    failed: BTreeSet<HandleId>,
}
struct Prepared {
    profile: Arc<DestructableVisual>,
    model: Arc<Model>,
    bounds: Aabb,
}
pub(super) struct Instance {
    pub(super) profile: Arc<DestructableVisual>,
    pub(super) animation: Animation,
    pub(super) pose: Transform,
    pub(super) face: f32,
    dead: bool,
}

fn load(
    objects: &ObjectHost,
    creation: DestructableCreation,
    input: &doodads::Input<'_>,
    edition: &str,
    remaining: &mut usize,
) -> Result<Arc<Prepared>, String> {
    let variation =
        u32::try_from(creation.variation).map_err(|_| "negative destructable variation")?;
    let profile = Arc::new(
        objects
            .destructable_catalog()
            .ok_or("destructable catalog is absent")?
            .visual(
                creation.objectid,
                variation,
                input.reader.paths().map(|p| p.as_str()),
            )?,
    );
    let asset = input
        .reader
        .resolve_bounded(&profile.member, (*remaining).min(16 << 20) as u64)
        .map_err(|e| e.to_string())?;
    *remaining = remaining
        .checked_sub(asset.bytes.len())
        .ok_or("destructable model byte budget")?;
    let (_, origin) = pcx_play::campaign_assets::identity(&asset, edition, 16 << 10)?;
    let model = Arc::new(Model::read(
        origin,
        profile.member.clone(),
        asset.bytes.into(),
        16 << 20,
    )?);
    let data = pcx_mdl::Mdlx::parse(&model.bytes)
        .map_err(|e| e.to_string())?
        .model(&model.bytes)
        .map_err(|e| e.to_string())?;
    let min = Vec3::from_array(data.minimum_extent);
    let max = Vec3::from_array(data.maximum_extent);
    if !min.is_finite() || !max.is_finite() || min.cmpgt(max).any() {
        return Err(format!(
            "{}: invalid destructable model bounds",
            profile.member
        ));
    }
    Ok(Arc::new(Prepared {
        profile,
        model,
        bounds: Aabb::from_min_max(min, max),
    }))
}

fn pose(
    objects: &ObjectHost,
    id: HandleId,
    creation: DestructableCreation,
    terrain: &pcx_map::Terrain,
) -> Result<(Transform, f32), String> {
    let (position, angle, scale) = if let Some(source) = objects.placed_source(id) {
        (source.position, source.angle, source.scale)
    } else {
        let z = creation
            .z
            .or_else(|| pcx_tera::world::sample_world_z(terrain, creation.x, creation.y))
            .ok_or("destructable outside terrain")?;
        (
            [creation.x, creation.y, z],
            creation.face.to_radians(),
            [creation.scale; 3],
        )
    };
    Ok((doodads::pose(position, angle, scale)?, angle.to_degrees()))
}

pub(super) fn project(
    commands: &mut Commands,
    state: &mut UnitActors,
    assets: &mut DrawAssets,
    queries: &mut ActorQueries,
    input: &doodads::Input<'_>,
    objects: &ObjectHost,
    edition: &str,
) {
    let Ok((_, camera, projection, view, ..)) = queries.scene.single() else {
        return;
    };
    let Some(size) = view
        .physical_viewport_size()
        .filter(|size| size.min_element() > 0)
    else {
        return;
    };
    let mut projection = projection.clone();
    projection.update(size.x as f32, size.y as f32);
    let frustum = projection.compute_frustum(&GlobalTransform::from(*camera));
    let mut draw = std::mem::take(&mut state.destructables);
    for (id, creation, current) in objects.destructables_with_handles() {
        if draw.failed.contains(&id) {
            continue;
        }
        let result: Result<(), String> = (|| {
            let key = (creation.objectid, creation.variation);
            if !draw.models.contains_key(&key) {
                if draw.models.len() >= 512 {
                    return Err("destructable model count budget".into());
                }
                let remaining = &mut (128usize << 20).saturating_sub(draw.bytes);
                let before = *remaining;
                let model = load(objects, creation, input, edition, remaining);
                draw.bytes += before - *remaining;
                draw.models.insert(key, model);
            }
            let prepared = draw.models[&key].as_ref().map_err(Clone::clone)?.clone();
            let (pose, face) = pose(objects, id, creation, input.terrain)?;
            if current.hidden
                || !frustum.intersects_obb(&prepared.bounds, &pose.compute_affine(), true, true)
            {
                retire(commands, state, id);
                return Ok(());
            }
            if let std::collections::btree_map::Entry::Vacant(entry) = draw.instances.entry(id) {
                // Placed models start with map initialization; scripted creation is observed on this frame.
                let created = if objects.placed_source(id).is_some() {
                    0.0
                } else {
                    input.now
                };
                let mut animation = Animation::new(Arc::clone(&prepared.model), created)?;
                if current.dead {
                    animation.select("death", created, None)?;
                }
                entry.insert(Instance {
                    profile: Arc::clone(&prepared.profile),
                    animation,
                    pose,
                    face,
                    dead: current.dead,
                });
            }
            let instance = draw.instances.get_mut(&id).expect("inserted destructable");
            instance.pose = pose;
            instance.face = face;
            if instance.dead != current.dead {
                instance.animation.select(
                    if current.dead { "death" } else { "stand" },
                    input.now,
                    None,
                )?;
                instance.dead = current.dead;
            }
            instance.animation.advance(input.now)?;
            let visual = Visual::Destructable(instance);
            if !state.actors.contains_key(&id) {
                let (actor, bytes, vertices) = Actor::spawn(
                    commands,
                    assets,
                    ActorSource {
                        reader: input.reader,
                        palette: input.palette,
                        visual,
                        terrain: input.terrain,
                        color: 0,
                        now: input.now,
                        channel_scale: input.parameters.channel_scale,
                        cutout: input.parameters.cutout_reference,
                        particle_parameters: Some(input.parameters),
                        available_images: (512usize << 20).saturating_sub(state.image_bytes),
                        available_vertices: 2_000_000usize.saturating_sub(state.vertices),
                    },
                )?;
                commands
                    .entity(actor.root)
                    .insert(Name::new("Canonical destructable"));
                state.image_bytes += bytes;
                state.vertices += vertices;
                state.actors.insert(id, actor);
            }
            state.actors.get_mut(&id).expect("inserted actor").update(
                visual,
                input.now,
                assets,
                (&mut queries.visibility, &mut queries.transforms),
                input.terrain,
                input.sun.map(|sun| (sun, input.parameters)),
            )?;
            Ok(())
        })();
        if let Err(error) = result {
            state.last_error = Some(format!("destructable {id:?}: {error}"));
            draw.failed.insert(id);
            retire(commands, state, id);
        }
    }
    state.destructables = draw;
}

pub(super) fn advance_particles(
    state: &mut UnitActors,
    assets: &mut DrawAssets,
    queries: &mut ActorQueries,
    now: f64,
    parameters: &pcx_play::menu_lighting::Parameters,
) {
    let Ok((_, camera, ..)) = queries.scene.single() else {
        return;
    };
    for (id, instance) in &state.destructables.instances {
        if let Some(actor) = state.actors.get_mut(id) {
            let local = Transform::from_rotation(
                actor.world_transform.rotation.inverse() * camera.rotation,
            );
            if let Err(error) = actor.advance_particles(
                &instance.animation,
                now,
                &local,
                assets,
                &mut queries.visibility,
                parameters,
            ) {
                state.last_error = Some(format!("destructable {id:?}: {error}"));
            }
        }
    }
}
