//! Native async preload progress for the loading consumer.
//! Each frame warms a bounded batch of still-pending requests; cancellation
//! and cache release stay explicit so no wall-clock delay is invented from
//! the authored timeout. Loose overrides stay live and bypass retention.
use crate::map_script_session::MapScriptSession;
use bevy::prelude::*;
use pcx_play::map_startup_host::MapStartupHost;
use std::sync::Weak;

// Application work budget per frame, unrelated to authored preload timeouts.
const POLL_BATCH: usize = 8;

#[derive(Resource, Default)]
pub struct PreloadPublication {
    session: Option<Weak<()>>,
    pub queued: usize,
    pub loaded: usize,
    pub pending: usize,
    pub loaded_bytes: u64,
    pub cancelled: usize,
    pub generated: Vec<String>,
    pub last_error: Option<String>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<PreloadPublication>().add_systems(
        Update,
        project.after(crate::campaign_presentation::CampaignPresentationSet),
    );
}

fn project(
    session: Option<ResMut<MapScriptSession<MapStartupHost>>>,
    mut state: ResMut<PreloadPublication>,
) {
    let Some(mut session) = session else {
        *state = PreloadPublication::default();
        return;
    };
    let witness = session.session_witness();
    let same = state
        .session
        .as_ref()
        .zip(witness.as_ref())
        .is_some_and(|(a, b)| Weak::ptr_eq(a, b));
    if !same {
        *state = PreloadPublication::default();
        state.session = witness;
        state.last_error = None;
    }
    if state.session.is_none() {
        return;
    }
    let Some(host) = session.host() else {
        state.last_error = Some("preload publication has no host".into());
        return;
    };
    // Bounded per-frame warming: native async progress without blocking loads.
    if let Err(error) = host.preload_poll(POLL_BATCH) {
        // No admitted preloader is not an error; other failures are retained.
        if error != "Preload: not admitted" {
            state.last_error = Some(error);
            return;
        }
    }
    let Some(report) = host.preload_report() else {
        state.last_error = None;
        return;
    };
    state.queued = report.queued;
    state.loaded = report.loaded;
    state.pending = report.pending;
    state.loaded_bytes = report.loaded_bytes;
    state.cancelled = report.cancelled;
    state.generated = report.generated.keys().cloned().collect();
    state.last_error = None;
}

/// Explicit cancellation for session teardown; the warmed prefix is retained.
pub fn cancel_session_preloads(session: &mut MapScriptSession<MapStartupHost>) -> usize {
    session
        .host()
        .map_or(0, |host| host.preload_cancel_pending())
}
