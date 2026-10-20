//! Score screen request consumer. Each exit snapshot is taken once: an executed
//! `EndGame(true)` becomes a score screen request that keeps the exiting session's
//! context, so its selected assets can be resolved. Score rows are not derived from the
//! simulation yet, so the request is refused with the missing inputs named. No rows or
//! placeholder values are produced.
use crate::campaign_runtime::{CampaignExitResult, LocalGameResult};
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
    },
}

/// The request from the most recent exit. `None` when that exit did not request a score screen.
#[derive(Resource, Default)]
pub struct ScoreScreenState(pub Option<ScoreScreen>);

/// Buttons on the score screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreAction {
    Ok,
    SaveReplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionOutcome {
    /// Leave the screen. The source action behind OK is not established, so this only
    /// closes the presentation.
    Dismiss,
    /// No replay writer exists, so Save Replay is refused rather than reported as saved.
    ReplayUnsupported,
}

/// Applies a press of the score button with `frame` name. OK dismisses the request;
/// Save Replay is refused and keeps it. Frames that are not score buttons, and presses with
/// no pending request, change nothing and return `None`.
pub fn press(state: &mut ScoreScreenState, frame: &str) -> Option<ActionOutcome> {
    let action = match frame {
        "ScoreScreenOkButton" => ScoreAction::Ok,
        "ScoreScreenSaveReplayButton" => ScoreAction::SaveReplay,
        _ => return None,
    };
    state.0.as_ref()?;
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

pub fn consume(mut exit: ResMut<CampaignExitResult>, mut state: ResMut<ScoreScreenState>) {
    let Some(snapshot) = exit.take() else {
        return;
    };
    state.0 = if snapshot.score_screen {
        Some(ScoreScreen::Refused {
            missing: &MISSING_INPUTS,
            local_result: snapshot.local_result,
            game_time: snapshot.game_time,
            context: snapshot.context,
        })
    } else {
        None
    };
}

/// Runs after the exit set, so a request taken in the same update that recorded it is seen.
pub fn install(app: &mut App) {
    app.init_resource::<ScoreScreenState>().add_systems(
        Update,
        consume.after(crate::campaign_runtime::CampaignExitSet),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_replay_is_refused_and_ok_only_dismisses() {
        assert_eq!(outcome(ScoreAction::SaveReplay), ActionOutcome::ReplayUnsupported);
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
