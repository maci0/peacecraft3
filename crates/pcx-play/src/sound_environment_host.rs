//! `NewSoundEnvironment` as the native runs it. A call inside the window after the last
//! applied change only buffers the name and schedules a callback; otherwise the record
//! is computed at once. A callback computes the buffered name. Provider state decides
//! what the computed request is for. Nothing here delivers a request: no backend
//! acknowledges one, so the outcomes never claim a delivery.
//!
//! Times are game seconds from `timer_game_time`. Without a game clock the call is
//! refused rather than timed from zero.
use super::{Error, Failure, ObjectHost};
use crate::sound_environment::{Request, SoundEnvironment, SoundEnvironmentInput, Window, apply};
use pcx_script::Value;

const NATIVE: &str = "NewSoundEnvironment";
/// The executable's `1000.0` scale (`0xe78350`) converts the window from ms to seconds.
const MS_PER_SECOND: f64 = 1000.0;

/// Identity of a 3D provider, as the audio owner reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderId(pub u64);

/// Provider state reported by the audio owner. `Unqualified` is the state before any
/// report: nothing is claimed about the provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderState {
    Unqualified,
    Closed,
    Open(ProviderId),
}

/// Whether a room call is pending on the native's room gate. The gate byte at
/// `0x1140aa4` is not decoded, so a pending room write is never reported as sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomGate {
    Unresolved,
    NotApplicable,
}

/// Outcome of the most recent call or callback. No variant reports a backend receipt,
/// because no backend in this build acknowledges requests.
#[derive(Clone, Debug, PartialEq)]
pub enum Delivery {
    /// A callback is queued and computes the buffered name at `fire_at` game seconds.
    Scheduled { fire_at: f64 },
    /// The name matches no record. The native sets nothing.
    NoRecord,
    /// Computed. The provider state is unqualified, so nothing is requested.
    Validated(Request),
    /// The provider is closed. The native skips its provider calls.
    ProviderClosed(Request),
    /// The provider reported open. The request is ready for a backend and is not sent.
    Prepared {
        provider: ProviderId,
        request: Request,
        room_gate: RoomGate,
    },
}

struct Entry {
    environment: SoundEnvironment,
    request: Request,
}

pub(super) struct State {
    entries: Vec<Entry>,
    window: Window,
    provider: ProviderState,
    /// Game time of the last applied change; `None` until one occurs.
    last_change: Option<f64>,
    /// Name the next callback computes. Set while a callback is pending or in the window.
    buffered: String,
    /// The native's pending flag: set while at least one callback is queued.
    pending: bool,
    /// Callback times, ascending.
    fires: Vec<f64>,
    selected: Option<usize>,
    delivery: Option<Delivery>,
}

impl State {
    fn window_seconds(&self) -> f64 {
        f64::from(self.window.ms) / MS_PER_SECOND
    }

    fn index_of(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.environment.name == name)
    }

    /// Computes the named record now and returns what the provider state makes of it.
    fn outcome(&mut self, name: &str) -> Delivery {
        let Some(index) = self.index_of(name) else {
            return Delivery::NoRecord;
        };
        self.selected = Some(index);
        let request = self.entries[index].request.clone();
        match self.provider {
            ProviderState::Unqualified => Delivery::Validated(request),
            ProviderState::Closed => Delivery::ProviderClosed(request),
            ProviderState::Open(provider) => Delivery::Prepared {
                provider,
                room_gate: match request.room {
                    Some(_) => RoomGate::Unresolved,
                    None => RoomGate::NotApplicable,
                },
                request,
            },
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
            .is_some_and(|last| now <= last + self.window_seconds());
        if self.pending || in_window {
            self.buffered = name.to_owned();
        }
        if in_window {
            let fire_at = now + self.window_seconds();
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
    /// Admits the native. Every record is computed against the executable tables here, so
    /// a record that cannot be written refuses the admission.
    pub fn with_sound_environments(mut self, input: SoundEnvironmentInput) -> Result<Self, Error> {
        let SoundEnvironmentInput {
            records: environments,
            tables,
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
            let request = apply(&environment, &tables).map_err(|_| {
                self.inner.error(
                    NATIVE,
                    Failure::SoundState("sound environment record cannot be computed"),
                )
            })?;
            entries.push(Entry {
                environment,
                request,
            });
        }
        self.sound_environments = Some(State {
            entries,
            window: tables.window,
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

    /// Runs callbacks due by the current game time. Called once per unit-activity tick
    /// so a scheduled callback runs on time when no script call follows.
    pub(super) fn advance_sound_environments(&mut self) {
        let Some(now) = self.timer_game_time() else {
            return;
        };
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
        self.sound_environments.as_ref()?;
        let [Value::Str(text)] = args else {
            return Some(Err(self
                .inner
                .error(name, Failure::SoundState("expected one environment name"))));
        };
        let Some(now) = self.timer_game_time() else {
            return Some(Err(self.inner.error(
                name,
                Failure::SoundState("no game clock to time the environment window"),
            )));
        };
        let state = self.sound_environments.as_mut()?;
        state.fire_due(now);
        state.call(text, now);
        Some(Ok(Value::Null))
    }
}

#[cfg(test)]
#[path = "sound_environment_host_tests.rs"]
mod tests;
