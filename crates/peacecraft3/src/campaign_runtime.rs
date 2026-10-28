//! Application pacing for the published campaign VM. Simulation time is tick based.
use crate::{
    browser_configuration::SelectedMapSources,
    campaign_camera::CampaignCamera,
    campaign_startup::CampaignStartupSet,
    game_input::GameInput,
    map_binding::BindingInputs,
    map_requests::{MapRequestSet, MapRequests},
    map_script_session::{MapScriptSession, TimerSessionError},
};
use bevy::prelude::*;
use pcx_play::{
    map_admission::ParsedMapContext, map_startup_host::MapStartupHost,
    playercolor_host::PlayerScoreSnapshot, scripts::AssetIdentity,
};
use pcx_script::Value;
use std::{
    num::NonZeroU32,
    sync::{Arc, Weak},
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
pub enum CampaignRuntimeError {
    #[error("published campaign has no admitted timer clock")]
    MissingClock,
    #[error("campaign logical tick counter exhausted")]
    TickOverflow,
    #[error("campaign pacing duration exhausted")]
    ClockOverflow,
    #[error("one script delivery requested conflicting campaign destinations")]
    ConflictingDestinations,
    #[error(transparent)]
    Timer(#[from] TimerSessionError),
}

#[derive(Debug, thiserror::Error)]
pub enum CampaignStepError {
    #[error("no published campaign session to step")]
    MissingSession,
    #[error("requested {requested} ticks exceeds the per-update budget {limit}")]
    TickBudget { requested: usize, limit: usize },
    #[error("explicit ticks require the existing frame-time debt to be drained first")]
    PendingFrameTime,
    #[error("campaign stopped after {advanced} committed ticks: {reason}")]
    RuntimeStopped { advanced: u64, reason: String },
}

/// Application scheduling policy, independent of original asset values.
#[derive(Resource)]
pub struct CampaignRuntime {
    step: Duration,
    max_ticks_per_update: usize,
    max_expiries_per_tick: usize,
    session: Option<Weak<()>>,
    origin: f64,
    pending: Duration,
    speed: NonZeroU32,
    tick: u64,
    pub paused: bool,
    /// Application view suspension, independent of an explicit simulation pause.
    pub view_paused: bool,
    pub last_error: Option<CampaignRuntimeError>,
}

impl CampaignRuntime {
    pub fn new(step: Duration, max_ticks_per_update: usize, max_expiries_per_tick: usize) -> Self {
        assert!(!step.is_zero(), "campaign timestep must be positive");
        assert!(
            max_ticks_per_update > 0,
            "campaign update budget must be positive"
        );
        assert!(
            max_expiries_per_tick > 0,
            "campaign timer budget must be positive"
        );
        Self {
            step,
            max_ticks_per_update,
            max_expiries_per_tick,
            session: None,
            origin: 0.0,
            pending: Duration::ZERO,
            speed: NonZeroU32::MIN,
            tick: 0,
            paused: false,
            view_paused: false,
            last_error: None,
        }
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn pending(&self) -> Duration {
        self.pending
    }

    /// Wall-clock pacing only: ticks, commands and random draws retain their order.
    pub fn set_speed(&mut self, speed: NonZeroU32) {
        self.speed = speed;
    }

    pub fn speed(&self) -> NonZeroU32 {
        self.speed
    }

    fn synchronize_script_pause(&mut self, session: &mut MapScriptSession<MapStartupHost>) -> bool {
        let Some(host) = session.host() else {
            return false;
        };
        let objects = host.objects();
        if objects.game_paused() != Some(true) {
            return false;
        }
        self.pending = Duration::ZERO;
        match objects.timer_game_time() {
            Some(now) => self.origin = now - self.step.as_secs_f64() * self.tick as f64,
            None => self.last_error = Some(CampaignRuntimeError::MissingClock),
        }
        true
    }

    /// An error stops this session; repeating an update cannot replay partial effects.
    pub fn advance(&mut self, session: &mut MapScriptSession<MapStartupHost>, elapsed: Duration) {
        if !self.prepare(session, elapsed) {
            return;
        }
        let Some(pending) = elapsed
            .checked_mul(self.speed.get())
            .and_then(|scaled| self.pending.checked_add(scaled))
        else {
            self.last_error = Some(CampaignRuntimeError::ClockOverflow);
            return;
        };
        self.pending = pending;
        for _ in 0..self.max_ticks_per_update {
            if self.pending < self.step || !self.advance_one_tick(session) {
                break;
            }
            self.pending -= self.step;
            if self.synchronize_script_pause(session) {
                break;
            }
        }
    }

    /// Advance bounded simulation ticks without wall time or speed scaling.
    /// First bind samples vision and returns zero. Pauses retain their ordinary
    /// event delivery. Presentation clocks are not advanced. Refused pacing
    /// inputs leave the runtime usable; execution faults remain sticky.
    /// A failed batch reports fully completed ticks. Its failing tick may have
    /// partial effects; the sticky fault forbids retrying even with zero progress.
    pub fn advance_ticks(
        &mut self,
        session: &mut MapScriptSession<MapStartupHost>,
        ticks: usize,
    ) -> Result<u64, CampaignStepError> {
        if ticks > self.max_ticks_per_update {
            return Err(CampaignStepError::TickBudget {
                requested: ticks,
                limit: self.max_ticks_per_update,
            });
        }
        let witness = session
            .session_witness()
            .ok_or(CampaignStepError::MissingSession)?;
        if self
            .session
            .as_ref()
            .is_some_and(|old| Weak::ptr_eq(old, &witness))
            && !self.pending.is_zero()
        {
            return Err(CampaignStepError::PendingFrameTime);
        }
        let ready = self.prepare(session, Duration::ZERO);
        let start = self.tick;
        if ready {
            for _ in 0..ticks {
                if !self.advance_one_tick(session) || self.synchronize_script_pause(session) {
                    break;
                }
            }
        }
        let advanced = self.tick - start;
        match &self.last_error {
            Some(error) => Err(CampaignStepError::RuntimeStopped {
                advanced,
                reason: error.to_string(),
            }),
            None => Ok(advanced),
        }
    }

    fn prepare(
        &mut self,
        session: &mut MapScriptSession<MapStartupHost>,
        elapsed: Duration,
    ) -> bool {
        let Some(witness) = session.session_witness() else {
            self.session = None;
            self.pending = Duration::ZERO;
            return false;
        };
        if !self
            .session
            .as_ref()
            .is_some_and(|old| Weak::ptr_eq(old, &witness))
        {
            self.session = Some(witness);
            self.pending = Duration::ZERO;
            self.tick = 0;
            self.last_error = None;
            match session
                .host()
                .and_then(|host| host.objects().timer_game_time())
            {
                Some(origin) => self.origin = origin,
                None => self.last_error = Some(CampaignRuntimeError::MissingClock),
            }
            // Loading and the previous session's frame debt do not advance this session.
            if self.last_error.is_none()
                && let Err(error) = session.sample_admitted_vision()
            {
                self.last_error = Some(error.into());
            }
            return false;
        }
        if let Some(host) = session.host() {
            host.objects_mut().advance_volume_group_clock(elapsed);
        }
        if self.synchronize_script_pause(session) {
            if self.last_error.is_none()
                && let Err(error) = session.advance_queued_events(65536)
            {
                self.last_error = Some(error.into());
            }
            if self.last_error.is_none()
                && let Err(error) = session.sample_admitted_vision()
            {
                self.last_error = Some(error.into());
            }
            // This frame's wall interval belongs to the paused state even if a UI callback resumes it.
            return false;
        }
        if self.paused || self.view_paused || self.last_error.is_some() {
            return false;
        }
        true
    }

    fn advance_one_tick(&mut self, session: &mut MapScriptSession<MapStartupHost>) -> bool {
        let Some(tick) = self.tick.checked_add(1) else {
            self.last_error = Some(CampaignRuntimeError::TickOverflow);
            return false;
        };
        let until = self.origin + self.step.as_secs_f64() * tick as f64;
        match pcx_play::campaign_host::advance_tick(session, until, self.max_expiries_per_tick) {
            Ok(_) => {
                self.tick = tick;
                true
            }
            Err(error) => {
                self.last_error = Some(error.into());
                false
            }
        }
    }
}

pub fn install(app: &mut App, runtime: CampaignRuntime) {
    crate::game_input::install(app);
    // Time<Real> is sampled before startup. Bind on the following frame so loading is excluded.
    app.insert_resource(runtime)
        .init_resource::<CampaignExitResult>()
        .init_resource::<CampaignTransition>()
        .add_message::<CampaignTransitionRequest>()
        .add_systems(
            Update,
            (
                advance_campaign
                    .in_set(CampaignAdvanceSet)
                    .before(CampaignStartupSet),
                take_game_exit
                    .in_set(CampaignExitSet)
                    .after(advance_campaign),
                apply_transition
                    .in_set(CampaignTransitionSet)
                    .after(CampaignExitSet)
                    .before(MapRequestSet::Deliver),
                finish_transition.after(CampaignStartupSet),
            ),
        );
}

/// Ends the session for executed exits. Systems ordered `.after(CampaignExitSet)`
/// observe the session's `CampaignExitResult` snapshot in the same update.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CampaignExitSet;

/// Orders the simulation step ahead of presentation that reads shared vision.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CampaignAdvanceSet;

fn advance_campaign(
    time: Res<Time<Real>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    input: Res<GameInput>,
    mut runtime: ResMut<CampaignRuntime>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    if session.session_witness().is_some()
        && keys.is_some_and(|keys| keys.just_pressed(KeyCode::F9))
        && input.ctrl.held()
        && input.shift.held()
    {
        let speed = match runtime.speed().get() {
            1 => 2,
            2 => 4,
            4 => 8,
            _ => 1,
        };
        runtime.set_speed(NonZeroU32::new(speed).expect("positive application pacing"));
        info!("Campaign simulation speed {speed}x (fixed ticks, Ctrl+Shift+F9 to cycle)");
    }
    // Real time only paces ticks. Time<Virtual>'s frame clamp would discard load debt.
    runtime.advance(&mut session, time.delta());
}

/// Local player's game result, from the `PLAYER_GAME_RESULT_*` globals the script loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalGameResult {
    Victory,
    Defeat,
    Tie,
    Neutral,
}

const GAME_RESULT_GLOBALS: [(LocalGameResult, &str); 4] = [
    (LocalGameResult::Victory, "PLAYER_GAME_RESULT_VICTORY"),
    (LocalGameResult::Defeat, "PLAYER_GAME_RESULT_DEFEAT"),
    (LocalGameResult::Tie, "PLAYER_GAME_RESULT_TIE"),
    (LocalGameResult::Neutral, "PLAYER_GAME_RESULT_NEUTRAL"),
];

/// State of one campaign exit, captured before its VM is dropped.
pub struct CampaignExitSnapshot {
    /// Any executed exit requesting scores. Choosing any-true over other
    /// combinations is host policy; the native's multi-call behavior is not verified.
    pub score_screen: bool,
    /// Pending script selection, captured before dropping the VM.
    pub campaign_menu_index: Option<i32>,
    /// Verbatim destination of the executed `ChangeLevel` request, if any.
    pub next_level: Option<String>,
    /// Simulation clock; `None` when the timer is not admitted.
    pub game_time: Option<f64>,
    /// `None` when the local player has no admitted removal record.
    pub local_result: Option<LocalGameResult>,
    /// Score counters of the local player. `None` when scores are not admitted or no
    /// local player is selected. Each counter's value is `None` until its source is verified.
    pub local_scores: Option<PlayerScoreSnapshot>,
    /// `RACE_PREF_*` constant of the local player's executed `SetPlayerRacePreference`.
    /// `Ok(None)` when none ran; `Err` when the constant is not uniquely identifiable.
    pub local_race: Result<Option<String>, String>,
    /// Map context of the exiting session, for resolving its assets.
    pub context: Arc<ParsedMapContext>,
}

/// Single-slot handoff for the score presentation. A later exit replaces an
/// unconsumed snapshot.
#[derive(Resource, Default)]
pub struct CampaignExitResult(Option<CampaignExitSnapshot>);

impl CampaignExitResult {
    pub fn take_campaign_menu_index(&mut self) -> Option<i32> {
        self.0.as_mut()?.campaign_menu_index.take()
    }

    pub fn take(&mut self) -> Option<CampaignExitSnapshot> {
        self.0.take()
    }
}

/// Ends the campaign session for executed `EndGame` or `ChangeLevel` requests. Ordered after
/// `advance_campaign`, the only place queued callbacks are delivered, so a request
/// recorded by a delivered callback is taken in the same frame. Requests come from
/// the active host only, so a cleared or replaced session cannot deliver them.
fn take_game_exit(
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut exit: ResMut<CampaignExitResult>,
    view: Option<ResMut<CampaignCamera>>,
    mut runtime: ResMut<CampaignRuntime>,
) {
    let Some(context) = session.context().cloned() else {
        return;
    };
    let globals = GAME_RESULT_GLOBALS.map(|(result, name)| (result, session.global(name)));
    let Some(host) = session.host() else {
        return;
    };
    let requests = host.objects_mut().take_game_exit_requests();
    if requests.is_empty() {
        return;
    }
    let next_level = requests[0].next_level.clone();
    if requests
        .iter()
        .any(|request| request.next_level != next_level)
    {
        runtime.last_error = Some(CampaignRuntimeError::ConflictingDestinations);
        return;
    }
    let campaign_menu_index = host.objects_mut().take_campaign_menu_index();
    let objects = host.objects();
    let local_result = objects
        .local_player_number()
        .and_then(|number| objects.player_identity(number))
        .and_then(|player| objects.player_removal_state()?.get(player))
        .and_then(|record| {
            globals.iter().find_map(|(result, value)| {
                matches!(value, Some(Value::Handle(id)) if *id == record.game_result)
                    .then_some(*result)
            })
        });
    let local_number = objects.local_player_number();
    exit.0 = Some(CampaignExitSnapshot {
        score_screen: requests.iter().any(|request| request.score_screen),
        campaign_menu_index,
        next_level,
        game_time: objects.timer_game_time(),
        local_result,
        local_scores: local_number.and_then(|number| objects.player_score_snapshot(number)),
        local_race: objects
            .local_race_preference_constant()
            .map_err(|error| error.to_string()),
        context,
    });
    session.clear();
    if let Some(mut view) = view {
        view.suspend();
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CampaignTransitionSet;

/// An authored continuation. It names the exiting
/// session's context so a late press cannot replace a different admitted map.
#[derive(Message, Clone)]
pub struct CampaignTransitionRequest {
    pub next_level: String,
    pub context: Arc<ParsedMapContext>,
}

#[derive(Debug, thiserror::Error)]
pub enum CampaignTransitionError {
    #[error("continued score screen is not the admitted session")]
    StaleSession,
    #[error("continued score screen has no selected map bindings")]
    MissingBindings,
    #[error("selected map sources no longer hold the binding's reader")]
    MissingReader,
    #[error("binding revision counter exhausted")]
    RevisionOverflow,
    #[error("resolving next level {member}: {source}")]
    Resolve {
        member: String,
        #[source]
        source: pcx_assets::Error,
    },
    #[error("binding next level {member}: {reason}")]
    Binding { member: String, reason: String },
}

/// Diagnostics for the authored score-screen continuation. A refused request
/// leaves the previous binding, admission and selected reader unchanged.
#[derive(Resource, Default)]
pub struct CampaignTransition {
    pub last_next_level: Option<String>,
    pub last_error: Option<CampaignTransitionError>,
    pending: Option<(CampaignTransitionRequest, Arc<ParsedMapContext>)>,
}

/// Rebind an authored next map through the retained source generation.
///
/// The request is accepted only while its exit context is still the admitted
/// map. The next member is read once within the retained map/input ceilings,
/// and its winning source fixes the archive identity; edition and version are
/// retained from the admitted binding. Success publishes one new binding and
/// admission atomically. Failure leaves both unchanged.
pub fn transition_to_next_level(
    requests: &mut MapRequests,
    sources: &mut SelectedMapSources,
    request: &CampaignTransitionRequest,
) -> Result<(), CampaignTransitionError> {
    let admitted = requests
        .admission
        .shared_context()
        .ok_or(CampaignTransitionError::StaleSession)?;
    if !Arc::ptr_eq(&admitted, &request.context) {
        return Err(CampaignTransitionError::StaleSession);
    }
    let bindings = requests
        .bindings
        .as_ref()
        .ok_or(CampaignTransitionError::MissingBindings)?;
    let previous_resolver = sources
        .for_receipts(&bindings.receipts)
        .ok_or(CampaignTransitionError::MissingReader)?;
    let recipe = sources.campaign_for_receipts(&bindings.receipts);
    let (resolver, catalog_identity, catalog_sources) = if let Some(recipe) = &recipe {
        let prepare = || -> Result<_, String> {
            let base = recipe.base_resolver().map_err(|error| error.to_string())?;
            let map = base
                .resolve_bounded(
                    &request.next_level,
                    bindings.limits.max_map_file_bytes as u64,
                )
                .map_err(|error| error.to_string())?;
            let limits = bindings.limits;
            let reader = recipe
                .open_map(
                    &map.bytes,
                    &request.next_level,
                    pcx_mpq::checked::ReadBudget {
                        max_archive_bytes: limits.max_map_file_bytes,
                        max_archive_table_bytes: limits.admission.scripts.max_archive_table_bytes,
                        max_raw_packed_bytes: limits.admission.scripts.max_raw_packed_bytes,
                        max_decoded_bytes: limits.admission.info_bytes,
                    },
                )
                .map_err(|error| error.to_string())?;
            let selected = reader
                .resolver
                .resolve_bounded(&request.next_level, limits.max_map_file_bytes as u64)
                .map_err(|error| error.to_string())?;
            if selected.bytes != map.bytes {
                return Err(
                    "selected campaign namespace changes the map used to select its data".into(),
                );
            }
            let mut catalog_sources = Vec::new();
            for path in reader.resolver.paths() {
                let source = reader
                    .resolver
                    .source_for(path.as_str())
                    .map_err(|error| error.to_string())?;
                if !catalog_sources.contains(source) {
                    catalog_sources.push(source.clone());
                }
            }
            let identity = AssetIdentity {
                edition: bindings.catalog_identity.edition.clone(),
                version: recipe.rules().origin.version.clone(),
                source: format!("{}::{}", recipe.dir().display(), reader.selection.prefix),
            };
            Ok((Arc::new(reader.resolver), identity, catalog_sources))
        };
        prepare().map_err(|reason| CampaignTransitionError::Binding {
            member: request.next_level.clone(),
            reason,
        })?
    } else {
        (
            previous_resolver,
            bindings.catalog_identity.clone(),
            bindings.catalog_sources.clone(),
        )
    };
    let (map_path, map_source) = {
        let source = resolver.source_for(&request.next_level).map_err(|source| {
            CampaignTransitionError::Resolve {
                member: request.next_level.clone(),
                source,
            }
        })?;
        (request.next_level.clone(), source.clone())
    };
    let map_identity = AssetIdentity {
        edition: bindings.map_identity.edition.clone(),
        version: bindings.map_identity.version.clone(),
        source: map_source.origin.clone(),
    };
    let revision = bindings
        .revision
        .checked_add(1)
        .ok_or(CampaignTransitionError::RevisionOverflow)?;
    let common_path = bindings.receipts.common.path.as_str().to_owned();
    let common_label = bindings.common.label.clone();
    let common_identity = bindings.common.identity.clone();
    let common_source = resolver
        .source_for(&common_path)
        .map_err(|source| CampaignTransitionError::Resolve {
            member: common_path.clone(),
            source,
        })?
        .clone();
    let blizzard_path = bindings.receipts.blizzard.path.as_str().to_owned();
    let blizzard_label = bindings.blizzard.label.clone();
    let blizzard_identity = bindings.blizzard.identity.clone();
    let blizzard_source = resolver
        .source_for(&blizzard_path)
        .map_err(|source| CampaignTransitionError::Resolve {
            member: blizzard_path.clone(),
            source,
        })?
        .clone();
    let catalog_label = bindings.catalog_label.clone();
    let required_catalog_tables = bindings.required_catalog_tables.clone();
    let required_fields = bindings.required_fields.clone();
    let limits = bindings.limits;
    let label = request.next_level.clone();
    let input = BindingInputs {
        map: crate::map_binding::IdentifiedMember {
            path: &map_path,
            label: &label,
            identity: &map_identity,
            source: &map_source,
        },
        common: crate::map_binding::IdentifiedMember {
            path: &common_path,
            label: &common_label,
            identity: &common_identity,
            source: &common_source,
        },
        blizzard: crate::map_binding::IdentifiedMember {
            path: &blizzard_path,
            label: &blizzard_label,
            identity: &blizzard_identity,
            source: &blizzard_source,
        },
        catalog_identity: &catalog_identity,
        catalog_label: &catalog_label,
        catalog_sources: &catalog_sources,
        required_catalog_tables: &required_catalog_tables,
        required_fields: &required_fields,
        revision,
    };
    let outcome = requests.replace_selected_with_campaign(sources, resolver, recipe, input, limits);
    match &outcome.result {
        Ok(()) => Ok(()),
        Err(error) => Err(CampaignTransitionError::Binding {
            member: request.next_level.clone(),
            reason: error.to_string(),
        }),
    }
}

fn apply_transition(
    mut continuation: MessageReader<CampaignTransitionRequest>,
    mut state: ResMut<CampaignTransition>,
    mut requests: ResMut<MapRequests>,
    mut sources: ResMut<SelectedMapSources>,
    startup: Option<Res<crate::campaign_startup::CampaignStartup>>,
) {
    for request in continuation.read() {
        let mut candidate = request.clone();
        if let Some((original, admitted)) = &state.pending
            && Arc::ptr_eq(&original.context, &request.context)
            && original.next_level == request.next_level
            && requests
                .admission
                .shared_context()
                .is_some_and(|current| Arc::ptr_eq(&current, admitted))
        {
            // A failed startup keeps its score screen; another press retries that binding.
            if startup
                .as_ref()
                .is_none_or(|startup| startup.last_error.is_none())
            {
                continue;
            }
            candidate.context = Arc::clone(admitted);
        }
        state.last_next_level = Some(request.next_level.clone());
        match transition_to_next_level(&mut requests, &mut sources, &candidate) {
            Ok(()) => {
                state.last_error = None;
                state.pending = Some((
                    request.clone(),
                    requests
                        .admission
                        .shared_context()
                        .expect("successful admission"),
                ));
            }
            Err(error) => {
                warn!("campaign transition refused: {error}");
                state.last_error = Some(error);
            }
        }
    }
}

fn finish_transition(
    session: Res<MapScriptSession<MapStartupHost>>,
    requests: Res<MapRequests>,
    mut state: ResMut<CampaignTransition>,
    mut score: Option<ResMut<crate::campaign_score::ScoreScreenState>>,
) {
    let Some((original, admitted)) = &state.pending else {
        return;
    };
    let Some(current) = session.context() else {
        return;
    };
    // A different successful map launch also retires the old retry screen.
    if !Arc::ptr_eq(current, admitted)
        && !requests
            .admission
            .shared_context()
            .is_some_and(|context| Arc::ptr_eq(&context, current))
    {
        return;
    }
    if let Some(score) = score.as_mut()
        && score
            .0
            .as_ref()
            .is_some_and(|screen| Arc::ptr_eq(screen.context(), &original.context))
    {
        score.0 = None;
    }
    state.pending = None;
}
