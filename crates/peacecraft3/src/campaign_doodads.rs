//! Draw the session's canonical decorative instances and selected animation clips.
use super::{Actor, ActorQueries, ActorSource, DrawAssets, TeamPalette, UnitActors, Visual};
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct Doodads {
    pub(super) actors: BTreeMap<u32, Actor>,
    failed: BTreeSet<u32>,
}

pub(super) fn transform(
    row: &pcx_play::campaign_placement::PlacedDoodad,
) -> Result<Transform, String> {
    let source = &row.source;
    pose(source.position, source.angle, source.scale)
}

pub(super) fn pose(position: [f32; 3], angle: f32, scale: [f32; 3]) -> Result<Transform, String> {
    let [x, y, z] = position;
    let pose = Transform::from_translation(Vec3::new(x, z, -y))
        .with_rotation(
            Quat::from_rotation_y(angle) * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        )
        .with_scale(Vec3::from_array(scale));
    if !pose.is_finite() || scale.iter().any(|s| *s <= 0.) {
        return Err("nonfinite or mirrored placed doodad pose".into());
    }
    Ok(pose)
}

pub(super) struct Input<'a> {
    pub reader: &'a pcx_assets::Resolver,
    pub terrain: &'a pcx_map::Terrain,
    pub palette: &'a TeamPalette,
    pub parameters: &'a pcx_play::menu_lighting::Parameters,
    pub now: f64,
    pub sun: Option<&'a pcx_play::day_night::Sun>,
}

pub(super) fn project<'a>(
    commands: &mut Commands,
    state: &mut UnitActors,
    assets: &mut DrawAssets,
    queries: &mut ActorQueries,
    input: Input,
    rows: impl Iterator<Item = &'a pcx_play::campaign_placement::PlacedDoodad>,
) {
    for row in rows {
        let id = row.source.creation_number;
        if state.doodads.failed.contains(&id) {
            continue;
        }
        let appearance = match &row.appearance {
            Ok(value) => value,
            Err(error) => {
                state.last_error = Some(format!("doodad {id}: {error}"));
                state.doodads.failed.insert(id);
                continue;
            }
        };
        let visual = Visual::Doodad(row, appearance);
        if !state.doodads.actors.contains_key(&id) {
            match Actor::spawn(
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
            ) {
                Ok((actor, bytes, vertices)) => {
                    state.image_bytes += bytes;
                    state.vertices += vertices;
                    state.doodads.actors.insert(id, actor);
                }
                Err(error) => {
                    state.last_error = Some(format!("doodad {id}: {error}"));
                    state.doodads.failed.insert(id);
                    continue;
                }
            }
        }
        if let Some(actor) = state.doodads.actors.get_mut(&id) {
            if let Ok(mut visibility) = queries.visibility.get_mut(actor.root) {
                *visibility = if row.shown {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
            if let Err(error) = actor.update(
                visual,
                input.now,
                assets,
                (&mut queries.visibility, &mut queries.transforms),
                input.terrain,
                input.sun.map(|sun| (sun, input.parameters)),
            ) {
                state.last_error = Some(format!("doodad {id}: {error}"));
            }
        }
    }
}

/// Runs after the projection's deferred particle entities have been published.
pub(super) fn advance_particles<'a>(
    state: &mut UnitActors,
    assets: &mut DrawAssets,
    queries: &mut ActorQueries,
    rows: impl Iterator<Item = &'a pcx_play::campaign_placement::PlacedDoodad>,
    now: f64,
    parameters: &pcx_play::menu_lighting::Parameters,
) {
    for row in rows {
        let id = row.source.creation_number;
        let Ok(appearance) = &row.appearance else {
            continue;
        };
        if let Some(actor) = state.doodads.actors.get_mut(&id)
            && let Ok((_, camera, ..)) = queries.scene.single()
        {
            let local_camera = Transform::from_rotation(
                actor.world_transform.rotation.inverse() * camera.rotation,
            );
            if let Err(error) = actor.advance_particles(
                &appearance.animation,
                now,
                &local_camera,
                assets,
                &mut queries.visibility,
                parameters,
            ) {
                state.last_error = Some(format!("doodad {id}: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_three_axis_pose_is_applied_without_grounding_or_extra_default_scale() {
        let row = pcx_play::campaign_placement::PlacedDoodad {
            source: pcx_map::Doodad {
                type_id: *b"Zx91",
                variation: 2,
                position: [37., -91., 153.],
                angle: std::f32::consts::FRAC_PI_2,
                scale: [0.75, 1.25, 2.],
                flags: 2,
                life: 255,
                creation_number: 71,
            },
            shown: true,
            point_selection: true,
            appearance: Err("model not needed by pose projection".into()),
        };
        let pose = transform(&row).unwrap();
        assert_eq!(pose.translation, Vec3::new(37., 153., 91.));
        assert_eq!(pose.scale, Vec3::new(0.75, 1.25, 2.));
        assert!((pose.rotation * Vec3::X + Vec3::Z).length() < 1e-5);
        assert!((pose.rotation * Vec3::Z - Vec3::Y).length() < 1e-5);
        let mut invalid = row;
        invalid.source.scale[1] = -1.;
        assert!(transform(&invalid).is_err());
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
#[path = "campaign_doodad_particle_tests.rs"]
mod particle_tests;
