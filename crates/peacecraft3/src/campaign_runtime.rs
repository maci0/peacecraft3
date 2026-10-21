//! Application pacing for the published campaign VM. Simulation time is tick based.
use crate::{
    campaign_camera::CampaignCamera,
    campaign_startup::CampaignStartupSet,
    map_script_session::{MapScriptSession, TimerSessionError},
};
use bevy::prelude::*;
use pcx_play::{
    map_admission::ParsedMapContext, map_startup_host::MapStartupHost,
    playercolor_host::PlayerScoreSnapshot,
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
    #[error(transparent)]
    Timer(#[from] TimerSessionError),
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
        let Some(witness) = session.session_witness() else {
            self.session = None;
            self.pending = Duration::ZERO;
            return;
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
            return;
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
            // This frame's wall interval belongs to the paused state even if a UI callback resumes it.
            return;
        }
        if self.paused || self.view_paused || self.last_error.is_some() {
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
            if self.pending < self.step {
                break;
            }
            let Some(tick) = self.tick.checked_add(1) else {
                self.last_error = Some(CampaignRuntimeError::TickOverflow);
                break;
            };
            let until = self.origin + self.step.as_secs_f64() * tick as f64;
            let result =
                pcx_play::campaign_host::advance_tick(session, until, self.max_expiries_per_tick);
            match result {
                Ok(_) => {
                    self.tick = tick;
                    self.pending -= self.step;
                    if self.synchronize_script_pause(session) {
                        break;
                    }
                }
                Err(error) => {
                    self.last_error = Some(error.into());
                    break;
                }
            }
        }
    }
}

pub fn install(app: &mut App, runtime: CampaignRuntime) {
    // Time<Real> is sampled before startup. Bind on the following frame so loading is excluded.
    app.insert_resource(runtime)
        .init_resource::<CampaignExitResult>()
        .add_systems(
            Update,
            (
                advance_campaign.before(CampaignStartupSet),
                take_game_exit
                    .in_set(CampaignExitSet)
                    .after(advance_campaign),
            ),
        );
}

/// Ends the session for executed exits. Systems ordered `.after(CampaignExitSet)`
/// observe the session's `CampaignExitResult` snapshot in the same update.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CampaignExitSet;

fn advance_campaign(
    time: Res<Time<Real>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut runtime: ResMut<CampaignRuntime>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    if session.session_witness().is_some()
        && keys.is_some_and(|keys| {
            keys.just_pressed(KeyCode::F9)
                && keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
                && keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
        })
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
    /// Any executed `EndGame(true)` in the exit. Choosing any-true over other
    /// combinations is host policy; the native's multi-call behavior is not verified.
    pub score_screen: bool,
    /// Simulation clock; `None` when the timer is not admitted.
    pub game_time: Option<f64>,
    /// `None` when the local player has no admitted removal record.
    pub local_result: Option<LocalGameResult>,
    /// Score counters of the local player. `None` when scores are not admitted or no
    /// local player is selected. Each counter's value is `None` until its source is verified.
    pub local_scores: Option<PlayerScoreSnapshot>,
    /// Map context of the exiting session, for resolving its assets.
    pub context: Arc<ParsedMapContext>,
}

/// Single-slot handoff for the score presentation. A later exit replaces an
/// unconsumed snapshot.
#[derive(Resource, Default)]
pub struct CampaignExitResult(Option<CampaignExitSnapshot>);

impl CampaignExitResult {
    pub fn take(&mut self) -> Option<CampaignExitSnapshot> {
        self.0.take()
    }
}

/// Ends the campaign session for executed `EndGame` requests. Ordered after
/// `advance_campaign`, the only place queued callbacks are delivered, so a request
/// recorded by a delivered callback is taken in the same frame. Requests come from
/// the active host only, so a cleared or replaced session cannot deliver them.
fn take_game_exit(
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut exit: ResMut<CampaignExitResult>,
    view: Option<ResMut<CampaignCamera>>,
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
        game_time: objects.timer_game_time(),
        local_result,
        local_scores: local_number.and_then(|number| objects.player_score_snapshot(number)),
        context,
    });
    session.clear();
    if let Some(mut view) = view {
        view.suspend();
    }
}
