//! Executed `EndGame` and `ChangeLevel` requests. The host records each request and
//! returns to the calling VM, which keeps running; the consumer decides when the
//! session ends. This scheduling choice does not claim unobserved native
//! `ChangeLevel` teardown. Score-screen presentation is not implemented: the
//! request carries the flag only.
use super::{Error, Failure, ObjectHost};
use crate::{campaign_progress::Progress, game_cache::Storage};
use pcx_script::Value;
use std::sync::Arc;

const END_GAME: &str = "EndGame";
const MISSION: &str = "SetMissionAvailable";
const NEXT_LEVEL: &str = "SetNextLevel";
const CHANGE_LEVEL: &str = "ChangeLevel";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameExitRequest {
    /// The script's `doScoreScreen` argument.
    pub score_screen: bool,
    /// `ChangeLevel`'s verbatim target, or `None` when this request came from `EndGame`.
    pub next_level: Option<String>,
}

pub(super) struct GameExits {
    admitted: [bool; 4],
    max_pending: usize,
    requests: Vec<GameExitRequest>,
    progress: Option<(Progress, Arc<dyn Storage>)>,
    next_level: Option<String>,
}

impl ObjectHost {
    /// Validates declared exit, mission-availability, next-map and change-level natives
    /// against the selected source. Missing declarations stay unsupported; progress
    /// storage is attached separately.
    pub fn with_game_exit(mut self, max_pending: usize) -> Result<Self, Error> {
        if self.game_exit.is_some() {
            return Err(self
                .inner
                .error(END_GAME, Failure::ClockState("game exit already admitted")));
        }
        std::alloc::Layout::array::<GameExitRequest>(max_pending)
            .map_err(|_| self.inner.error(END_GAME, Failure::Capacity))?;
        let mut admitted = [false; 4];
        for (index, (name, args)) in [
            (END_GAME, &["boolean"][..]),
            (MISSION, &["integer", "integer", "boolean"][..]),
            (NEXT_LEVEL, &["string"][..]),
            (CHANGE_LEVEL, &["string", "boolean"][..]),
        ]
        .into_iter()
        .enumerate()
        {
            if self.inner.program.natives().get(name).is_none()
                && self.inner.program.natives().ambiguous(name).is_none()
            {
                continue;
            }
            Self::contract_in(&self.inner, name, args, "nothing", false, None, false)?;
            let source = &self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked native")
                .source;
            if self
                .inner
                .program
                .natives()
                .get("CreateTimer")
                .is_none_or(|n| &n.source != source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
            admitted[index] = true;
        }
        if !admitted.iter().any(|value| *value) {
            return Ok(self);
        }
        self.game_exit = Some(GameExits {
            admitted,
            max_pending,
            requests: Vec::new(),
            progress: None,
            next_level: None,
        });
        Ok(self)
    }

    pub fn game_exit_admitted(&self) -> bool {
        self.game_exit
            .as_ref()
            .is_some_and(|state| state.admitted[0])
    }

    /// Explicit availability only; unknown fresh-profile defaults remain unknown.
    pub fn campaign_progress(&self) -> Option<&Progress> {
        self.game_exit
            .as_ref()?
            .progress
            .as_ref()
            .map(|(progress, _)| progress)
    }

    /// Verbatim source next-map request. This does not itself end or replace a session.
    pub fn next_level(&self) -> Option<&str> {
        self.game_exit.as_ref()?.next_level.as_deref()
    }

    /// Attach a bounded progress section before script execution. Saves must be atomic.
    pub fn admit_campaign_progress_storage(
        &mut self,
        storage: Arc<dyn Storage>,
    ) -> Result<(), Error> {
        let Some(state) = self.game_exit.as_ref().filter(|s| s.admitted[1]) else {
            return Ok(());
        };
        let fail = |detail| self.inner.error(MISSION, Failure::CampaignProgress(detail));
        if state.progress.is_some() {
            return Err(fail("progress storage already admitted".into()));
        }
        let profile = match storage
            .load(crate::campaign_progress::MAX_BYTES)
            .map_err(fail)?
        {
            Some(bytes) => Progress::decode(&bytes).map_err(fail)?,
            None => Progress::default(),
        };
        self.game_exit.as_mut().expect("checked admission").progress = Some((profile, storage));
        Ok(())
    }

    /// Requests recorded since the last take, in call order.
    pub fn game_exit_requests(&self) -> &[GameExitRequest] {
        self.game_exit
            .as_ref()
            .map_or(&[], |exits| exits.requests.as_slice())
    }

    pub fn take_game_exit_requests(&mut self) -> Vec<GameExitRequest> {
        self.game_exit
            .as_mut()
            .map_or_else(Vec::new, |exits| std::mem::take(&mut exits.requests))
    }

    pub(super) fn game_exit_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.game_exit.as_ref()?;
        if name == MISSION && state.admitted[1] {
            return Some(self.set_mission_available(args));
        }
        if name == NEXT_LEVEL && state.admitted[2] {
            return Some(self.set_next_level(args));
        }
        if name == CHANGE_LEVEL && state.admitted[3] {
            return Some(self.change_level(args));
        }
        if name != END_GAME || !state.admitted[0] {
            return None;
        }
        let [Value::Bool(score_screen)] = args else {
            return Some(Err(self.inner.error(
                END_GAME,
                Failure::ClockState("EndGame expects one boolean"),
            )));
        };
        let exits = self.game_exit.as_mut().expect("admitted game exit");
        if exits.requests.len() >= exits.max_pending {
            return Some(Err(self
                .inner
                .error(END_GAME, Failure::Budget("game exit requests"))));
        }
        if exits.requests.try_reserve(1).is_err() {
            return Some(Err(self.inner.error(END_GAME, Failure::Capacity)));
        }
        exits.requests.push(GameExitRequest {
            score_screen: *score_screen,
            next_level: None,
        });
        // The calling thread continues after EndGame, as in the original native.
        Some(Ok(Value::Null))
    }

    fn set_mission_available(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |detail| self.inner.error(MISSION, Failure::CampaignProgress(detail));
        let [
            Value::Int(campaign),
            Value::Int(mission),
            Value::Bool(available),
        ] = args
        else {
            return Err(fail("campaign, mission and availability required".into()));
        };
        let (profile, storage) = self
            .game_exit
            .as_mut()
            .and_then(|s| s.progress.as_mut())
            .ok_or_else(|| fail("progress storage is not admitted".into()))?;
        if profile.mission_available(*campaign, *mission) == Some(*available) {
            return Ok(Value::Null);
        }
        let mut next = profile.clone();
        next.set(*campaign, *mission, *available).map_err(fail)?;
        let bytes = next.encode().map_err(fail)?;
        storage.save(&bytes).map_err(fail)?;
        *profile = next;
        Ok(Value::Null)
    }

    fn change_level(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [Value::Str(map), Value::Bool(score_screen)] = args else {
            return Err(self.inner.error(
                CHANGE_LEVEL,
                Failure::ClockState("ChangeLevel expects a map name and boolean"),
            ));
        };
        let next_level = self.bounded_map(CHANGE_LEVEL, map)?;
        let exits = self.game_exit.as_mut().expect("admitted change level");
        if exits.requests.len() >= exits.max_pending {
            return Err(self
                .inner
                .error(CHANGE_LEVEL, Failure::Budget("game exit requests")));
        }
        if exits.requests.try_reserve(1).is_err() {
            return Err(self.inner.error(CHANGE_LEVEL, Failure::Capacity));
        }
        exits.requests.push(GameExitRequest {
            score_screen: *score_screen,
            next_level: Some(next_level),
        });
        // The host returns to the VM caller after queueing the request.
        Ok(Value::Null)
    }

    fn set_next_level(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [Value::Str(map)] = args else {
            return Err(self
                .inner
                .error(NEXT_LEVEL, Failure::ClockState("one map name required")));
        };
        let value = self.bounded_map(NEXT_LEVEL, map)?;
        self.game_exit
            .as_mut()
            .expect("admitted next level")
            .next_level = (!value.is_empty()).then_some(value);
        Ok(Value::Null)
    }

    fn bounded_map(&self, name: &str, map: &str) -> Result<String, Error> {
        let fail = |detail| self.inner.error(name, Failure::CampaignProgress(detail));
        if map.len() > crate::campaign_progress::MAX_BYTES || map.contains('\0') {
            return Err(fail("next-map name byte policy or embedded NUL".into()));
        }
        let mut value = String::new();
        value
            .try_reserve_exact(map.len())
            .map_err(|e| fail(e.to_string()))?;
        value.push_str(map);
        Ok(value)
    }
}

#[cfg(test)]
#[path = "game_exit_tests.rs"]
mod tests;
