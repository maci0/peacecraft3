//! `GetPlayerScore` admission for the selected `common.j` score counters.
//!
//! The counters are the selected program's `PLAYER_SCORE_*` constants, read at admission.
//! No counter has a verified source yet, so every counter refuses and every snapshot value
//! is `None`. A counter is supported only after its source event is established against
//! the selected executable (see `docs/re-notes/player-score-counters.md`).

use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::Value;

const GET: &str = "GetPlayerScore";
const CONVERT: &str = "ConvertPlayerScore";
const COUNTER_KIND: &str = "playerscore";

/// One selected score constant. `value` is `None` because no counter is derived yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerScoreCounter {
    pub name: String,
    pub ordinal: i32,
    pub value: Option<i32>,
}

/// Score counters for one player number, in selected-constant order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerScoreSnapshot {
    pub player: i32,
    pub counters: Vec<PlayerScoreCounter>,
}

impl ObjectHost {
    /// Validates `GetPlayerScore`, its `ConvertPlayerScore` domain and each selected
    /// `PLAYER_SCORE_*` constant's ordinal in that domain.
    pub fn with_player_scores(mut self) -> Result<Self, Error> {
        if self.player_scores.is_some() {
            return Err(self
                .inner
                .error(GET, Failure::PlayerScore("player scores already admitted".into())));
        }
        Self::contract(&self.inner, GET, &["player", "playerscore"], "integer", true, None)?;
        let domain = self
            .inner
            .conversions()
            .find(|c| c.name == CONVERT && c.kind == COUNTER_KIND)
            .map(|c| c.domain.clone())
            .ok_or_else(|| self.inner.error(CONVERT, Failure::Signature))?;
        let names: Vec<String> = self
            .inner
            .program
            .globals()
            .iter()
            .filter(|g| g.is_constant && !g.array && g.ty.kind.name() == COUNTER_KIND)
            .map(|g| g.name.clone())
            .collect();
        let mut counters = Vec::with_capacity(names.len());
        for name in names {
            let ordinal = self.source_constant_enumerand(&name, COUNTER_KIND)?;
            if !domain.contains(&ordinal) {
                return Err(self
                    .inner
                    .error(GET, Failure::PlayerScore(format!("{name} is outside the score domain"))));
            }
            counters.push(PlayerScoreCounter {
                name,
                ordinal,
                value: None,
            });
        }
        self.player_scores = Some(counters);
        Ok(self)
    }

    /// Counters of an admitted program for one player number. `None` before admission.
    pub fn player_score_snapshot(&self, number: i32) -> Option<PlayerScoreSnapshot> {
        self.player_scores.as_ref().map(|counters| PlayerScoreSnapshot {
            player: number,
            counters: counters.clone(),
        })
    }

    pub(super) fn player_score_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != GET || self.player_scores.is_none() {
            return None;
        }
        Some(self.get_player_score(args))
    }

    fn get_player_score(&self, args: &[Value]) -> Result<Value, Error> {
        let [Value::Handle(player), Value::Handle(score)] = args else {
            return Err(self
                .inner
                .error(GET, Failure::PlayerScore("expected a player and a score".into())));
        };
        if !matches!(self.inner.entry(*player)?, Entry::Player { .. }) {
            return Err(self.inner.error(GET, Failure::WrongFamily));
        }
        let ordinal = self.inner.symbolic_value(*score, COUNTER_KIND)?;
        let counters = self.player_scores.as_ref().expect("dispatch requires admission");
        let counter = counters
            .iter()
            .find(|c| c.ordinal == ordinal)
            .ok_or_else(|| {
                self.inner
                    .error(GET, Failure::PlayerScore("score is not a selected constant".into()))
            })?;
        // Refuse rather than report a zero for a counter with no verified source.
        Err(self.inner.error(
            GET,
            Failure::PlayerScore(format!("{} has no verified source", counter.name)),
        ))
    }
}
