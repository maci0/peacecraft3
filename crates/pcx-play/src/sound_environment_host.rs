//! `NewSoundEnvironment` as the native runs it. A call inside the window after the last
//! applied change only buffers the name and schedules a callback; otherwise the record
//! is applied at once. A callback applies the buffered name. Provider state decides
//! what the apply sends. Times are game seconds from `timer_game_time`.
use super::{Error, Failure, ObjectHost};
use crate::sound_environment::{Applied, SoundEnvironment, SoundEnvironmentInput, apply};
use pcx_script::Value;

const NATIVE: &str = "NewSoundEnvironment";
const MS_PER_SECOND: f64 = 1000.0;

/// Identity of a 3D provider as the audio owner reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderId(pub u64);

/// Provider state supplied by the audio owner. `Unqualified` is the state before any
/// report: nothing is claimed about the provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderState {
    Unqualified,
    Closed,
    Open(ProviderId),
}

/// Outcome of the most recent apply or schedule.
#[derive(Clone, Debug, PartialEq)]
pub enum Delivery {
    /// A callback is queued and applies the buffered name at `fire_at` game seconds.
    Scheduled { fire_at: f64 },
    /// The name matches no record. The native sets nothing.
    NoRecord,
    /// Values computed. The provider state is unqualified, so nothing is sent.
    Validated(Applied),
    /// The provider is closed. The native skips every provider call.
    ProviderClosed(Applied),
    /// Sent to the open provider.
    Delivered {
        provider: ProviderId,
        values: Applied,
    },
}

struct Entry {
    environment: SoundEnvironment,
    values: Applied,
}

pub(super) struct State {
    entries: Vec<Entry>,
    window: f64,
    provider: ProviderState,
    /// Game time of the last applied change; `None` until one occurs.
    last_change: Option<f64>,
    /// Name the next callback applies. Set while a callback is pending or in the window.
    buffered: String,
    /// The native's pending flag: set while at least one callback is queued.
    pending: bool,
    /// Callback times, ascending.
    fires: Vec<f64>,
    selected: Option<usize>,
    delivery: Option<Delivery>,
}

impl State {
    fn index_of(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.environment.name == name)
    }

    /// Applies the named record now and returns what the provider state allows.
    fn outcome(&mut self, name: &str) -> Delivery {
        let Some(index) = self.index_of(name) else {
            return Delivery::NoRecord;
        };
        self.selected = Some(index);
        let values = self.entries[index].values.clone();
        match self.provider {
            ProviderState::Unqualified => Delivery::Validated(values),
            ProviderState::Closed => Delivery::ProviderClosed(values),
            ProviderState::Open(provider) => Delivery::Delivered { provider, values },
        }
    }

    /// Runs callbacks due at or before `now`, each at its own game time.
    fn fire_due(&mut self, now: f64) {
        while self.fires.first().is_some_and(|at| *at <= now) {
            let at = self.fires.remove(0);
            self.pending = false;
            self.last_change = Some(at);
            let name = self.buffered.clone();
            self.delivery = Some(self.outcome(&name));
        }
    }

    fn call(&mut self, name: &str, now: f64) {
        let in_window = self
            .last_change
            .is_some_and(|last| now <= last + self.window);
        if self.pending || in_window {
            self.buffered = name.to_owned();
        }
        if in_window {
            let fire_at = now + self.window;
            let at = self.fires.partition_point(|known| *known <= fire_at);
            self.fires.insert(at, fire_at);
            self.pending = true;
            self.delivery = Some(Delivery::Scheduled { fire_at });
        } else {
            self.last_change = Some(now);
            self.delivery = Some(self.outcome(name));
        }
    }
}

impl ObjectHost {
    /// Admits the native. Every record is applied against `tables` here, so a value that
    /// cannot be written (fractional int cell, missing row) refuses the admission. The
    /// window is the native's `0x1388` ms (5 s) constant.
    pub fn with_sound_environments(mut self, input: SoundEnvironmentInput) -> Result<Self, Error> {
        let SoundEnvironmentInput {
            records: environments,
            tables,
            window_ms,
        } = input;
        if self.sound_environments.is_some() {
            return Err(self.inner.error(
                NATIVE,
                Failure::SoundState("sound environments already admitted"),
            ));
        }
        if environments.is_empty() {
            return Err(self
                .inner
                .error(NATIVE, Failure::SoundState("no sound environment records")));
        }
        Self::contract(&self.inner, NATIVE, &["string"], "nothing", false, None)?;
        let mut entries = Vec::with_capacity(environments.len());
        for environment in environments {
            let values = apply(&environment, &tables).map_err(|_| {
                self.inner.error(
                    NATIVE,
                    Failure::SoundState("sound environment record cannot be applied"),
                )
            })?;
            entries.push(Entry {
                environment,
                values,
            });
        }
        self.sound_environments = Some(State {
            entries,
            window: f64::from(window_ms) / MS_PER_SECOND,
            provider: ProviderState::Unqualified,
            last_change: None,
            buffered: String::new(),
            pending: false,
            fires: Vec::new(),
            selected: None,
            delivery: None,
        });
        Ok(self)
    }

    /// Provider state from the audio owner. It applies to later outcomes and callbacks.
    pub fn set_sound_environment_provider(&mut self, provider: ProviderState) {
        if let Some(state) = self.sound_environments.as_mut() {
            state.provider = provider;
        }
    }

    /// The record the last successful name lookup selected.
    pub fn sound_environment(&self) -> Option<&SoundEnvironment> {
        let state = self.sound_environments.as_ref()?;
        state
            .entries
            .get(state.selected?)
            .map(|entry| &entry.environment)
    }

    /// Outcome of the last call or callback. `None` until one occurs.
    pub fn sound_environment_delivery(&self) -> Option<&Delivery> {
        self.sound_environments.as_ref()?.delivery.as_ref()
    }

    /// Values the native sent to an open provider. Any other outcome yields `None`.
    pub fn sound_environment_applied(&self) -> Option<&Applied> {
        match self.sound_environment_delivery()? {
            Delivery::Delivered { values, .. } => Some(values),
            _ => None,
        }
    }

    /// Runs callbacks due by the current game time. Called once per simulation tick so
    /// a scheduled apply lands on time even when no script call follows.
    pub(super) fn advance_sound_environments(&mut self) {
        let now = self.timer_game_time().unwrap_or(0.0);
        if let Some(state) = self.sound_environments.as_mut() {
            state.fire_due(now);
        }
    }

    pub(super) fn sound_environment_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE {
            return None;
        }
        let now = self.timer_game_time().unwrap_or(0.0);
        let state = self.sound_environments.as_mut()?;
        let [Value::Str(text)] = args else {
            return Some(Err(self
                .inner
                .error(name, Failure::SoundState("expected one environment name"))));
        };
        state.fire_due(now);
        state.call(text, now);
        Some(Ok(Value::Null))
    }
}

#[cfg(test)]
#[path = "sound_environment_host_tests.rs"]
mod tests;
