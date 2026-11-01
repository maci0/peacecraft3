//! Executed 3D terrain fog for the scene camera.
//! Script overrides win over authored map settings; absent overrides read as
//! `None` and leave the authored fog untouched. Fog-of-war masking stays in
//! the existing minimap/vision consumer; this pass drives distance fog only.
use crate::map_script_session::MapScriptSession;
use bevy::{
    pbr::{DistanceFog, FogFalloff},
    prelude::*,
};
use pcx_play::map_startup_host::MapStartupHost;
use std::sync::Weak;

#[derive(Resource, Default)]
pub struct FogPublication {
    session: Option<Weak<()>>,
    pub style: Option<i32>,
    pub applied: bool,
    pub last_error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<FogPublication>().add_systems(
        Update,
        project.after(crate::campaign_presentation::CampaignPresentationSet),
    );
}

fn project(
    session: Option<ResMut<MapScriptSession<MapStartupHost>>>,
    mut state: ResMut<FogPublication>,
    mut cameras: Query<&mut DistanceFog>,
) {
    let Some(mut session) = session else {
        state.session = None;
        state.style = None;
        state.applied = false;
        state.last_error = None;
        return;
    };
    let witness = session.session_witness();
    let same = state
        .session
        .as_ref()
        .zip(witness.as_ref())
        .is_some_and(|(a, b)| Weak::ptr_eq(a, b));
    if !same {
        state.session = witness;
        state.style = None;
        state.applied = false;
        state.last_error = None;
    }
    if state.session.is_none() {
        return;
    }
    let Some(host) = session.host() else {
        state.last_error = Some("fog publication has no host".into());
        return;
    };
    let fog = host.objects().terrain_fog_3d();
    state.style = fog.map(|f| f.style);
    let Some(fog) = fog else {
        state.applied = false;
        state.last_error = None;
        return;
    };
    if !(fog.z_start.is_finite()
        && fog.z_end.is_finite()
        && fog.z_end > fog.z_start
        && fog.density.is_finite()
        && fog.density >= 0.0
        && fog.color.iter().all(|c| c.is_finite()))
    {
        state.last_error = Some("executed terrain fog is non-finite".into());
        return;
    }
    let color = Color::srgb(
        fog.color[0].clamp(0.0, 1.0),
        fog.color[1].clamp(0.0, 1.0),
        fog.color[2].clamp(0.0, 1.0),
    );
    // Style follows the selected script value: 0 is linear, 1 is
    // exponential with the authored density; other styles refuse loudly.
    let falloff = match fog.style {
        0 => FogFalloff::Linear {
            start: fog.z_start,
            end: fog.z_end,
        },
        1 => FogFalloff::Exponential {
            density: fog.density,
        },
        style => {
            state.last_error = Some(format!("unsupported terrain fog style {style}"));
            return;
        }
    };
    let mut applied = false;
    for mut distance in &mut cameras {
        *distance = DistanceFog {
            color,
            falloff,
            directional_light_color: Color::NONE,
            directional_light_exponent: 0.0,
        };
        applied = true;
    }
    state.applied = applied;
    state.last_error = None;
}
