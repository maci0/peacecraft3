//! Score screen request consumer. Each exit snapshot is taken once: an executed
//! `EndGame(true)` becomes a score screen request that keeps the exiting session's
//! context, so its selected assets can be resolved. Score rows are not derived from the
//! simulation yet, so the request is refused with the missing inputs named. No rows or
//! placeholder values are produced.
use crate::campaign_runtime::{
    CampaignExitResult, CampaignTransitionRequest, CampaignTransitionSet, LocalGameResult,
};
use bevy::prelude::*;
use pcx_play::map_admission::ParsedMapContext;
use std::sync::Arc;

/// Inputs the score presentation still needs from the selected source and runtime.
pub const MISSING_INPUTS: [&str; 2] = [
    "score rows: per-player score values are not derived from the simulation",
    "replay: no replay writer exists, so Save Replay is refused",
];

#[derive(Clone)]
pub enum ScoreScreen {
    Refused {
        missing: &'static [&'static str],
        local_result: Option<LocalGameResult>,
        game_time: Option<f64>,
        /// Context of the exiting session. Its selected resolver supplies the score UI.
        context: Arc<ParsedMapContext>,
        next_level: Option<String>,
    },
}

/// The request from the most recent exit. `None` when that exit did not request a score screen.
#[derive(Resource, Default)]
pub struct ScoreScreenState(pub Option<ScoreScreen>);

impl ScoreScreen {
    pub fn context(&self) -> &Arc<ParsedMapContext> {
        match self {
            Self::Refused { context, .. } => context,
        }
    }
}

impl ScoreScreenState {
    pub fn continuation(&self) -> Option<CampaignTransitionRequest> {
        let ScoreScreen::Refused {
            next_level: Some(next_level),
            context,
            ..
        } = self.0.as_ref()?
        else {
            return None;
        };
        Some(CampaignTransitionRequest {
            next_level: next_level.clone(),
            context: Arc::clone(context),
        })
    }
}

/// Buttons on the score screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreAction {
    Ok,
    SaveReplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionOutcome {
    /// Keep the presentation until the selected next map is admitted.
    Continue,
    /// Close an exit presentation with no authored next-map request.
    Dismiss,
    /// No replay writer exists, so Save Replay is refused rather than reported as saved.
    ReplayUnsupported,
}

/// OK continues an authored next-map request or dismisses an ordinary exit.
/// Save Replay refuses and keeps the screen. Other frames and absent screens do nothing.
pub fn press(state: &mut ScoreScreenState, frame: &str) -> Option<ActionOutcome> {
    let action = match frame {
        "ScoreScreenOkButton" => ScoreAction::Ok,
        "ScoreScreenSaveReplayButton" => ScoreAction::SaveReplay,
        _ => return None,
    };
    state.0.as_ref()?;
    if action == ScoreAction::Ok
        && matches!(
            state.0.as_ref(),
            Some(ScoreScreen::Refused {
                next_level: Some(_),
                ..
            })
        )
    {
        return Some(ActionOutcome::Continue);
    }
    let result = outcome(action);
    if result == ActionOutcome::Dismiss {
        state.0 = None;
    }
    Some(result)
}

pub fn outcome(action: ScoreAction) -> ActionOutcome {
    match action {
        ScoreAction::Ok => ActionOutcome::Dismiss,
        ScoreAction::SaveReplay => ActionOutcome::ReplayUnsupported,
    }
}

pub fn consume(
    mut exit: ResMut<CampaignExitResult>,
    mut state: ResMut<ScoreScreenState>,
    mut continuation: MessageWriter<CampaignTransitionRequest>,
) {
    let Some(snapshot) = exit.take() else {
        return;
    };
    state.0 = if snapshot.score_screen {
        Some(ScoreScreen::Refused {
            missing: &MISSING_INPUTS,
            local_result: snapshot.local_result,
            game_time: snapshot.game_time,
            context: snapshot.context,
            next_level: snapshot.next_level,
        })
    } else {
        if let Some(next_level) = snapshot.next_level {
            continuation.write(CampaignTransitionRequest {
                next_level,
                context: snapshot.context,
            });
        }
        None
    };
}

/// Runs after the exit set, so a request taken in the same update that recorded it is seen.
pub fn install(app: &mut App) {
    app.init_resource::<ScoreScreenState>().add_systems(
        Update,
        consume
            .after(crate::campaign_runtime::CampaignExitSet)
            .before(CampaignTransitionSet),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_replay_is_refused_and_ok_only_dismisses() {
        assert_eq!(
            outcome(ScoreAction::SaveReplay),
            ActionOutcome::ReplayUnsupported
        );
        assert_eq!(outcome(ScoreAction::Ok), ActionOutcome::Dismiss);
    }

    #[test]
    fn every_missing_input_is_named_once() {
        let mut seen = std::collections::BTreeSet::new();
        for input in MISSING_INPUTS {
            assert!(seen.insert(input), "duplicate missing input {input}");
        }
        assert_eq!(seen.len(), 2);
    }
}
