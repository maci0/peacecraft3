//! Control groups on the digit keys for the local player.
//!
//! A digit selects its group, Ctrl+digit defines it from the selection, and
//! Shift+digit adds the selection to it. A second press of the same digit within
//! the double-press window also centres the camera on the selection's primary
//! unit. The digit-to-group binding, the cap, the window and the refusal texts
//! all come from the client and its string tables; see
//! `docs/re-notes/input-control-groups.md`. Groups follow the game input gate.
use std::{collections::BTreeMap, sync::Weak};

use bevy::prelude::*;
use pcx_play::{
    control_group_rules::ControlGroupRules,
    control_groups::{ControlGroups, Group, Refusal, Roster, Standing},
    map_startup_host::MapStartupHost,
    playercolor_host::ObjectHost,
};
use pcx_script::HandleId;

use crate::{
    campaign_camera::scroll_to, campaign_error_line::ErrorLine, game_input::GameInput,
    map_script_session::MapScriptSession,
};

/// Digit keys in the order of the digits they type; the client binds the top-row
/// digits only, not the keypad.
const DIGIT_KEYS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// Shift+digit adds the selection to the group.
///
/// The client's key handler builds the union of the group and the selection and
/// redefines the group from it (executable evidence); a live capture saw the group
/// gain the selection and the selection stay as it was
/// (`docs/re-notes/input-live-capture.md`). Change only this function if the
/// behaviour is ever shown to differ.
fn shift_digit(
    groups: &mut ControlGroups,
    group: Group,
    selection: &[HandleId],
    roster: &impl Roster,
    cap: usize,
) -> Result<(), Refusal> {
    groups.extend(group, selection, roster, cap)
}

/// Refusal texts from the `[Errors]` section of `Units\CommandStrings.txt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ErrorTexts {
    pub invalid: String,
    pub empty: String,
    pub non_unit: String,
}

impl ErrorTexts {
    fn of(&self, refusal: Refusal) -> &str {
        match refusal {
            Refusal::ForeignUnits => &self.invalid,
            Refusal::EmptySelection => &self.empty,
            Refusal::NotUnits => &self.non_unit,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Source {
    pub rules: ControlGroupRules,
    pub texts: ErrorTexts,
}

/// Sent when a digit replaced the selection, so the application can record it.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct ControlGroupSelected {
    pub units: Vec<HandleId>,
}

/// Sent on the second press of a digit within the window: the camera moves to
/// the primary unit of the selection.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ControlGroupCentred {
    pub position: [f32; 2],
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlGroupSet;

#[derive(Resource, Default)]
pub struct CampaignControlGroups {
    session: Option<Weak<()>>,
    source: Option<Result<Source, String>>,
    players: BTreeMap<i32, ControlGroups>,
    pub last_error: Option<String>,
}

impl CampaignControlGroups {
    /// True until the loader has supplied the rules or a refusal for this session.
    pub fn needs_source(&self) -> bool {
        self.source.is_none()
    }

    pub fn admit(&mut self, source: Result<Source, String>) {
        if let Err(error) = &source {
            self.last_error = Some(error.clone());
        }
        self.source = Some(source);
    }

    pub fn groups(&self, player: i32) -> Option<&ControlGroups> {
        self.players.get(&player)
    }
}

pub fn install(app: &mut App) {
    crate::game_input::install(app);
    app.init_resource::<CampaignControlGroups>()
        .init_resource::<ErrorLine>()
        .add_message::<ControlGroupSelected>()
        .add_message::<ControlGroupCentred>()
        .add_systems(
            Update,
            (control_group_keys, centre_camera)
                .chain()
                .in_set(ControlGroupSet),
        );
}

/// Moves the camera to where a control group press asked, within the camera bounds.
fn centre_camera(
    mut requests: MessageReader<ControlGroupCentred>,
    mut state: ResMut<CampaignControlGroups>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let Some(request) = requests.read().last().copied() else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    if let Err(error) = scroll_to(host.objects_mut(), request.position) {
        state.last_error = Some(error);
    }
}

struct HostRoster<'a> {
    objects: &'a ObjectHost,
    local: i32,
}

impl Roster for HostRoster<'_> {
    fn standing(&self, handle: HandleId) -> Standing {
        let Ok(unit) = self.objects.executed_unit(handle) else {
            return if self.objects.executed_item(handle).is_ok() {
                Standing::NotUnit
            } else {
                Standing::Gone
            };
        };
        if unit.combat.as_ref().is_some_and(|combat| combat.dead) {
            return Standing::Gone;
        }
        let owned = self
            .objects
            .player_settings(unit.owner)
            .is_ok_and(|player| player.number == self.local);
        match (owned, unit.shown) {
            (false, _) => Standing::Foreign,
            (true, true) => Standing::Own,
            (true, false) => Standing::Hidden,
        }
    }
}

fn same_session(a: Option<&Weak<()>>, b: Option<&Weak<()>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Weak::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// What a group key tells the rest of the application.
#[derive(bevy::ecs::system::SystemParam)]
struct Outputs<'w> {
    selected: MessageWriter<'w, ControlGroupSelected>,
    centred: MessageWriter<'w, ControlGroupCentred>,
    errors: ResMut<'w, ErrorLine>,
}

fn control_group_keys(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    input: Res<GameInput>,
    time: Option<Res<Time<Real>>>,
    mut state: ResMut<CampaignControlGroups>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut out: Outputs,
) {
    let witness = session.session_witness();
    if !same_session(state.session.as_ref(), witness.as_ref()) {
        *state = CampaignControlGroups {
            session: witness,
            ..default()
        };
        out.errors.clear();
        return;
    }
    let (Some(keys), Some(time)) = (keys, time) else {
        return;
    };
    let digits: Vec<usize> = DIGIT_KEYS
        .iter()
        .enumerate()
        .filter(|(_, key)| keys.just_pressed(**key))
        .map(|(digit, _)| digit)
        .collect();
    if digits.is_empty() || !input.open() {
        return;
    }
    let state = &mut *state;
    let Some(Ok(source)) = &state.source else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects_mut();
    let Some(local) = objects.local_player_number() else {
        return;
    };
    let groups = state.players.entry(local).or_default();
    for digit in digits {
        let Some(group) = Group::new(source.rules.digit_groups[digit]) else {
            continue;
        };
        let cap = source.rules.selection_cap;
        let selection = objects.selection().to_vec();
        let roster = HostRoster {
            objects: &*objects,
            local,
        };
        let outcome = if input.ctrl.held() {
            groups.define(group, &selection, &roster, cap)
        } else if input.shift.held() {
            shift_digit(groups, group, &selection, &roster, cap)
        } else {
            let units = groups.recall(group, &roster, cap);
            if !units.is_empty() {
                objects.select_units(&units, true);
                out.selected.write(ControlGroupSelected { units });
                let repeated = groups.press(
                    group,
                    time.elapsed_secs(),
                    source.rules.double_press_seconds,
                );
                if repeated {
                    let position = objects
                        .selection()
                        .first()
                        .and_then(|&unit| objects.executed_unit(unit).ok())
                        .and_then(|unit| unit.position);
                    if let Some(position) = position {
                        out.centred.write(ControlGroupCentred { position });
                    }
                }
            }
            continue;
        };
        if let Err(refusal) = outcome {
            out.errors
                .post(source.texts.of(refusal), time.elapsed_secs_f64());
        }
    }
}
