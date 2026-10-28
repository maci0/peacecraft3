//! Application state of the campaign loading screen, from the mission launch to
//! the first playable frame. The screen's content comes from
//! `pcx_play::campaign_loading`; this module owns when it is shown, how its bar
//! advances and when the held simulation is released. See
//! `docs/re-notes/campaign-loading-screen.md`.
use crate::{
    campaign_camera::{CampaignCamera, CampaignCameraSet},
    campaign_startup::{CampaignHostSupply, CampaignStartup, CampaignStartupSet, StartupError},
    map_requests::{MapRequestSet, MapRequests},
    map_script_session::MapScriptSession,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use pcx_assets::Resolver;
use pcx_map::info::MapInfo;
use pcx_play::{
    campaign_loading::{
        Progress, ProgressRules, Screen, ScreenTable, ScreenText, Selection, StringTable,
        TABLE_PATH, resolve_text, select,
    },
    map_startup_host::MapStartupHost,
};

use std::sync::{Arc, Weak};

/// Upper bound on the table read through the selected resolver.
const TABLE_BYTES: u64 = 4 << 20;
/// Recreation choice: the bar advances in equal shares at these load stages.
/// The original's own stage weights are code immediates that are not attributed
/// to one load path.
const STAGES: usize = 4;
/// Recreation fail-safe: ends a screen whose load never completes.
const CANCEL_KEY: KeyCode = KeyCode::Escape;

/// What the screen is built from. The resolver is the one the table was read
/// through, so the frames, models and textures come from the same sources.
pub struct LoadingRequest {
    pub screen: Screen,
    pub text: ScreenText,
    pub resolver: Arc<Resolver>,
    pub rules: ProgressRules,
}

impl LoadingRequest {
    /// `Ok(None)` when the header selects the melee panel, which is not drawn.
    pub fn read(
        resolver: Arc<Resolver>,
        info: &MapInfo,
        strings: Option<&StringTable>,
        rules: ProgressRules,
    ) -> Result<Option<Self>, String> {
        let asset = resolver
            .resolve_bounded(TABLE_PATH, TABLE_BYTES)
            .map_err(|e| format!("{TABLE_PATH}: {e}"))?;
        let table = std::str::from_utf8(&asset.bytes)
            .map_err(|_| format!("{TABLE_PATH}: not UTF-8 text"))
            .and_then(|text| ScreenTable::parse(text).map_err(|e| e.to_string()))?;
        let fields = info.loading_screen();
        let screen = match select(&fields, &table).map_err(|e| e.to_string())? {
            Selection::Campaign(screen) => screen,
            Selection::Melee => return Ok(None),
        };
        let text = resolve_text(&fields, strings).map_err(|e| e.to_string())?;
        Ok(Some(Self {
            screen,
            text,
            resolver,
            rules,
        }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// The bar fills while the map loads.
    Loading,
    /// Loaded: the bar is full and a key or pointer press starts the game.
    AwaitKey,
}

/// What the running application shows about the load in progress.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Signals {
    /// The launched map's bindings are published.
    pub admitted: bool,
    /// Its host inputs are prepared.
    pub prepared: bool,
    /// Its script session is running.
    pub published: bool,
    /// Its view is active.
    pub ready: bool,
    /// A stage refused; the error is reported by that stage.
    pub failed: bool,
}

#[derive(Resource, Default)]
pub struct CampaignLoading {
    phase: Phase,
    request: Option<Arc<LoadingRequest>>,
    progress: Progress,
    reached: usize,
    /// The bindings revision and session at the launch, which a new load must replace.
    revision: Option<u64>,
    session: Option<Weak<()>>,
    /// Failure already on display at the launch.
    outcome: Option<(String, Option<u64>)>,
    generation: u64,
}

impl CampaignLoading {
    /// Start the screen for a launched mission. A screen already shown is replaced.
    pub fn begin(
        &mut self,
        request: LoadingRequest,
        requests: &MapRequests,
        session: Option<Weak<()>>,
    ) {
        *self = Self {
            phase: Phase::Loading,
            request: Some(Arc::new(request)),
            revision: requests.bindings.as_ref().map(|b| b.revision),
            session,
            outcome: requests
                .last_outcome
                .as_ref()
                .filter(|o| o.result.is_err())
                .map(|o| (o.label.clone(), o.binding_revision)),
            generation: self.generation + 1,
            ..Self::default()
        };
    }

    /// Drop the screen without starting the game.
    pub fn cancel(&mut self) {
        self.phase = Phase::Idle;
        self.request = None;
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The screen is up: the menu and the held game sit behind it.
    pub fn shown(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// The simulation waits until the screen ends, as the original's load hold does.
    pub fn holds_simulation(&self) -> bool {
        self.shown()
    }

    pub fn request(&self) -> Option<&Arc<LoadingRequest>> {
        self.request.as_ref()
    }

    /// Changes whenever a new screen begins, so a view can tell screens apart.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Bar position in `0.0..=1.0`.
    pub fn fraction(&self) -> f32 {
        self.request
            .as_ref()
            .map_or(0.0, |r| self.progress.fraction(&r.rules))
    }

    /// One frame: report newly reached stages, ease the bar, and end the load
    /// when the view is ready.
    pub fn observe(&mut self, signals: Signals) {
        let Some(request) = self.request.clone() else {
            return;
        };
        if self.phase != Phase::Loading {
            return;
        }
        if signals.failed {
            self.cancel();
            return;
        }
        let reached = [
            signals.admitted,
            signals.prepared,
            signals.published,
            signals.ready,
        ];
        while self.reached < STAGES && reached[self.reached] {
            self.progress.report(1.0 / STAGES as f32);
            self.reached += 1;
        }
        if signals.ready {
            self.progress.finish(&request.rules);
            self.phase = Phase::AwaitKey;
        } else {
            self.progress.step(&request.rules);
        }
    }

    /// A key or pointer press while the loaded screen waits ends it.
    pub fn press(&mut self) -> bool {
        let ends = self.phase == Phase::AwaitKey;
        if ends {
            self.cancel();
        }
        ends
    }

    fn fresh_outcome_failed(&self, requests: &MapRequests) -> bool {
        requests.last_outcome.as_ref().is_some_and(|outcome| {
            outcome.result.is_err()
                && self.outcome.as_ref() != Some(&(outcome.label.clone(), outcome.binding_revision))
        })
    }
}

/// The load's observable state, read from the published resources.
fn signals(
    loading: &CampaignLoading,
    requests: &MapRequests,
    supply: &CampaignHostSupply,
    startup: &CampaignStartup,
    witness: Option<Weak<()>>,
    view_active: bool,
) -> Signals {
    let revision = requests.bindings.as_ref().map(|b| b.revision);
    let admitted = revision.is_some_and(|r| Some(r) != loading.revision)
        && requests
            .last_outcome
            .as_ref()
            .is_some_and(|o| o.result.is_ok() && o.binding_revision == revision);
    let prepared = admitted && revision == Some(supply.revision);
    let published = prepared
        && witness.as_ref().is_some_and(|w| {
            loading
                .session
                .as_ref()
                .is_none_or(|old| !Weak::ptr_eq(old, w))
        });
    let startup_failed = admitted
        && match &startup.last_error {
            Some(StartupError::Preparation { binding, .. }) => Some(*binding) == revision,
            Some(_) => prepared,
            None => false,
        };
    Signals {
        admitted,
        prepared,
        published,
        ready: published && view_active,
        failed: loading.fresh_outcome_failed(requests) || startup_failed,
    }
}

/// What the load is observed through.
#[derive(SystemParam)]
struct LoadState<'w> {
    requests: Res<'w, MapRequests>,
    supply: Res<'w, CampaignHostSupply>,
    startup: Res<'w, CampaignStartup>,
    session: Res<'w, MapScriptSession<MapStartupHost>>,
    camera: Option<Res<'w, CampaignCamera>>,
}

#[derive(SystemParam)]
struct Presses<'w> {
    keys: Option<ResMut<'w, ButtonInput<KeyCode>>>,
    mouse: Option<ResMut<'w, ButtonInput<MouseButton>>>,
}

fn advance_loading(mut loading: ResMut<CampaignLoading>, state: LoadState, presses: Presses) {
    let LoadState {
        requests,
        supply,
        startup,
        session,
        camera,
    } = state;
    let Presses { keys, mouse } = presses;
    if !loading.shown() {
        return;
    }
    let observed = signals(
        &loading,
        &requests,
        &supply,
        &startup,
        session.session_witness(),
        camera.is_some_and(|c| c.active()),
    );
    // Only a press made while the loaded screen was already waiting counts.
    let waiting = loading.phase() == Phase::AwaitKey;
    loading.observe(observed);
    let mut pressed = false;
    if let Some(mut keys) = keys
        && keys.get_just_pressed().next().is_some()
    {
        // Recreation fail-safe, not in the original: a load that never completes
        // would otherwise leave the screen up for good.
        if loading.phase() == Phase::Loading && keys.just_pressed(CANCEL_KEY) {
            loading.cancel();
        }
        pressed |= waiting;
        keys.clear();
    }
    if let Some(mut mouse) = mouse
        && mouse.get_just_pressed().next().is_some()
    {
        pressed |= waiting;
        mouse.clear();
    }
    // Presses are consumed while the screen is up so they do not reach the menu
    // or the game behind it.
    if pressed {
        loading.press();
    }
}

/// The loading state is current for the frame once this set has run.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampaignLoadingSet;

pub fn install(app: &mut App) {
    app.init_resource::<CampaignLoading>().add_systems(
        Update,
        advance_loading
            .in_set(CampaignLoadingSet)
            .after(MapRequestSet::Deliver)
            .after(CampaignStartupSet)
            .after(CampaignCameraSet),
    );
}

#[cfg(test)]
#[path = "campaign_loading_tests.rs"]
mod tests;
