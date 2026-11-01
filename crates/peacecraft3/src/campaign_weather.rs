//! Executed weather areas for the 3D renderer and the audio consumer.
//! The session owns the state; this publication copies counts, bounds and
//! authored ambient sounds without allocating a second world.
use crate::map_script_session::MapScriptSession;
use bevy::prelude::*;
use pcx_play::map_startup_host::MapStartupHost;
use std::sync::Weak;

#[derive(Resource, Default)]
pub struct WeatherPublication {
    session: Option<Weak<()>>,
    pub live: usize,
    pub enabled: usize,
    pub ambience: Vec<String>,
    pub last_error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<WeatherPublication>().add_systems(
        Update,
        project.after(crate::campaign_presentation::CampaignPresentationSet),
    );
}

fn project(
    session: Option<ResMut<MapScriptSession<MapStartupHost>>>,
    mut state: ResMut<WeatherPublication>,
) {
    let Some(mut session) = session else {
        state.session = None;
        state.live = 0;
        state.enabled = 0;
        state.ambience.clear();
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
        state.live = 0;
        state.enabled = 0;
        state.ambience.clear();
        state.last_error = None;
    }
    if state.session.is_none() {
        return;
    }
    let Some(host) = session.host() else {
        state.last_error = Some("weather publication has no host".into());
        return;
    };
    let objects = host.objects();
    let mut live = 0;
    let mut enabled = 0;
    let mut ambience = Vec::new();
    for (_, effect) in objects.weather_effects() {
        live += 1;
        enabled += usize::from(effect.enabled);
        if effect.enabled
            && let Some(sound) = effect.definition.ambient_sound.clone()
            && !ambience.contains(&sound)
        {
            if ambience.len() >= 64 {
                state.last_error = Some("weather ambience budget".into());
                break;
            }
            ambience.push(sound);
        }
    }
    state.live = live;
    state.enabled = enabled;
    state.ambience = ambience;
    if state.last_error.as_deref() != Some("weather ambience budget") {
        state.last_error = None;
    }
}
