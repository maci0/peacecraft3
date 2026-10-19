//! Application pacing for the published campaign VM. Simulation time is tick based.
use crate::{
    campaign_startup::CampaignStartupSet,
    map_script_session::{MapScriptSession, TimerSessionError},
};
use bevy::prelude::*;
use pcx_play::map_startup_host::MapStartupHost;
use std::{sync::Weak, time::Duration};

#[derive(Debug, thiserror::Error)]
pub enum CampaignRuntimeError {
    #[error("published campaign has no admitted timer clock")]
    MissingClock,
    #[error("campaign logical tick counter exhausted")]
    TickOverflow,
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
        if self.paused || self.view_paused || self.last_error.is_some() {
            return;
        }
        self.pending = self.pending.saturating_add(elapsed);
        for _ in 0..self.max_ticks_per_update {
            if self.pending < self.step {
                break;
            }
            let Some(tick) = self.tick.checked_add(1) else {
                self.last_error = Some(CampaignRuntimeError::TickOverflow);
                break;
            };
            let until = self.origin + self.step.as_secs_f64() * tick as f64;
            match session.advance_timers(until, self.max_expiries_per_tick) {
                Ok(_) => {
                    self.tick = tick;
                    self.pending -= self.step;
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
        .add_systems(Update, advance_campaign.before(CampaignStartupSet));
}

fn advance_campaign(
    time: Res<Time<Real>>,
    mut runtime: ResMut<CampaignRuntime>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    // Real time only paces ticks. Time<Virtual>'s frame clamp would discard load debt.
    runtime.advance(&mut session, time.delta());
}
