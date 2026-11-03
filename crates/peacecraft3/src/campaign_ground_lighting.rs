//! Selected terrain day/night lighting for the admitted ground draw.
use bevy::prelude::*;
use pcx_play::{day_night::Sun, map_startup_host::MapStartupHost, menu_lighting::Parameters};
use peacecraft3::{map_ground::GroundPublication, map_script_session::MapScriptSession};
use std::sync::{Arc, Weak};

#[derive(Resource, Default)]
pub(crate) struct TerrainLighting {
    context: Option<Weak<pcx_play::map_admission::ParsedMapContext>>,
    parameters: Option<Result<Parameters, String>>,
    /// Last material and source sun successfully submitted to the renderer.
    pub last: Option<(AssetId<pcx_gfx::GroundMaterial>, Sun)>,
    pub last_error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<TerrainLighting>()
        .add_systems(PostUpdate, project);
}

fn project(
    mut state: ResMut<TerrainLighting>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    ground: Res<GroundPublication>,
    draws: Query<&MeshMaterial3d<pcx_gfx::GroundMaterial>>,
    mut materials: ResMut<Assets<pcx_gfx::GroundMaterial>>,
    launcher: Option<Res<crate::launch_gui::Launcher>>,
) {
    let Some(context) = session.context().cloned() else {
        return;
    };
    if !ground
        .context
        .as_ref()
        .is_some_and(|source| Arc::ptr_eq(source, &context))
    {
        return;
    }
    let Some(handle) = ground
        .entity
        .and_then(|entity| draws.get(entity).ok())
        .map(|draw| &draw.0)
    else {
        return;
    };
    if state
        .context
        .as_ref()
        .is_none_or(|old| !Weak::ptr_eq(old, &Arc::downgrade(&context)))
    {
        *state = TerrainLighting {
            context: Some(Arc::downgrade(&context)),
            ..default()
        };
    }
    let result = (|| -> Result<(), String> {
        if state.parameters.is_none() {
            let install = launcher
                .as_deref()
                .and_then(crate::launch_gui::selected_install);
            state.parameters = Some(crate::menu_lighting::parameters(install.as_deref()));
        }
        let objects = session
            .host()
            .ok_or("terrain lighting has no active host")?
            .objects();
        let [terrain, _] = objects
            .day_night_state()
            .and_then(|day| day.selected())
            .ok_or("terrain day/night light is not selected")?;
        let fraction = objects
            .day_fraction()
            .ok_or("terrain day fraction is unavailable")?;
        let sun = terrain.sun(fraction)?;
        if materials.get(handle).is_none() {
            return Err("terrain lighting material ownership lost".into());
        }
        if state.last == Some((handle.id(), sun)) {
            return Ok(());
        }
        let parameters = state
            .parameters
            .as_ref()
            .expect("loaded above")
            .as_ref()
            .map_err(Clone::clone)?;
        let (diffuse, ambient) = crate::menu_lighting::sun_channels(&sun, parameters);
        let [x, y, z] = sun.direction;
        materials
            .get_mut(handle)
            .ok_or("terrain lighting material ownership lost")?
            .set_directional_light(-Vec3::new(x, z, -y), diffuse, ambient)?;
        state.last = Some((handle.id(), sun));
        Ok(())
    })();
    let error = result.err();
    if error != state.last_error {
        if let Some(error) = &error {
            warn!("terrain source lighting: {error}");
        }
        state.last_error = error;
    }
}
