//! Selection rules beyond a plain click: Shift toggles a unit, Tab and Shift+Tab cycle the
//! active subgroup, and a group-panel button selects, removes or switches subgroup.
//! The pure rules are in [`pcx_play::selection_click`] and [`pcx_play::selection_subgroups`];
//! evidence is in `docs/re-notes/input-selection-rules.md`.
use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::PlayerCommand,
    playercolor_host::ObjectHost,
    selection_box::Candidate,
    selection_click::{Toggle, toggle},
};
use pcx_script::HandleId;
use peacecraft3::{game_input::GameInput, map_script_session::MapScriptSession};

use crate::campaign_input::{CampaignInput, record_command};

/// Keyboard modifiers read when a selection click lands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
}

impl Modifiers {
    pub fn read(input: &GameInput) -> Self {
        Self {
            shift: input.shift.held(),
            ctrl: input.ctrl.held(),
        }
    }
}

pub(crate) fn install(app: &mut App) {
    app.add_systems(
        Update,
        subgroup_keys.before(crate::campaign_command_card::CommandCardSet),
    );
}

/// Tab selects the next subgroup, Shift+Tab the previous one; both wrap.
fn subgroup_keys(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    game_input: Res<GameInput>,
    mut input: ResMut<CampaignInput>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
) {
    let Some(keys) = keys else { return };
    if !game_input.open() || !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let Some(host) = session.host() else { return };
    let forward = !Modifiers::read(&game_input).shift;
    if let Err(error) = host.objects_mut().cycle_subgroup(forward) {
        input.last_error = Some(error);
    }
}

fn candidates(objects: &ObjectHost, ids: &[HandleId]) -> Result<Vec<Candidate>, String> {
    ids.iter()
        .filter_map(|&id| objects.box_candidate(id).transpose())
        .collect()
}

fn set_selection(
    objects: &mut ObjectHost,
    input: &mut CampaignInput,
    units: Vec<HandleId>,
    replace: bool,
) {
    objects.select_units(&units, replace);
    let clock = objects.timer_game_time();
    record_command(input, clock, PlayerCommand::Select { units, replace });
}

/// A left click on a unit. Shift toggles it into or out of the selection; without the selection
/// cap (client rules not admitted) Shift only adds, as before the rules were read.
pub(crate) fn click_unit(
    objects: &mut ObjectHost,
    input: &mut CampaignInput,
    unit: HandleId,
    additive: bool,
    cap: Option<usize>,
) -> Result<(), String> {
    match (additive, cap) {
        (true, Some(cap)) => {
            let current = candidates(objects, objects.selection())?;
            let Some(clicked) = objects.box_candidate(unit)? else {
                return Ok(());
            };
            match toggle(&current, clicked, cap).map_err(|e| e.to_string())? {
                Toggle::Select(units) => set_selection(objects, input, units, true),
                Toggle::Full => {}
            }
        }
        (additive, _) => set_selection(objects, input, vec![unit], !additive),
    }
    Ok(())
}

/// A click on a group-panel button for `unit`. Ctrl selects the units of `unit`'s subgroup.
/// Shift is a world click with Shift. Otherwise a unit of the active subgroup is selected alone
/// and a unit of another subgroup makes its subgroup active.
pub(crate) fn click_group_button(
    objects: &mut ObjectHost,
    input: &mut CampaignInput,
    unit: HandleId,
    modifiers: Modifiers,
    cap: Option<usize>,
) -> Result<(), String> {
    if modifiers.ctrl {
        let units = objects.subgroup_units_of(unit)?;
        if !units.is_empty() {
            set_selection(objects, input, units, true);
        }
        return Ok(());
    }
    if modifiers.shift {
        return click_unit(objects, input, unit, true, cap);
    }
    let groups = objects.subgroups()?;
    let active = objects.active_subgroup()?;
    let in_active = active.is_some_and(|index| groups[index].units.contains(&unit));
    if in_active {
        click_unit(objects, input, unit, false, cap)
    } else {
        objects.activate_subgroup_of(unit)?;
        Ok(())
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "test-fixtures"))]
mod tests {
    use super::*;
    use crate::{
        campaign_input::box_select_tests::{Rig, SCREEN, Spec},
        campaign_startup_cases::box_select_cases::{FOOTMAN, HALL, PEON},
    };
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    };

    /// Two peons, two footmen and an own hall, all in view of the test camera.
    fn mixed() -> Vec<Spec> {
        vec![
            (0, PEON, [24.0, 40.0]),
            (0, PEON, [40.0, 40.0]),
            (0, FOOTMAN, [56.0, 40.0]),
            (0, FOOTMAN, [72.0, 40.0]),
            (0, HALL, [56.0, 76.0]),
        ]
    }

    fn shift_click(rig: &mut Rig, game: [f32; 2]) {
        let at = rig.screen(game);
        rig.hold(KeyCode::ShiftLeft, true);
        rig.click(at);
        rig.hold(KeyCode::ShiftLeft, false);
    }

    fn tab(rig: &mut Rig, shift: bool) {
        rig.hold(KeyCode::ShiftLeft, shift);
        let window = rig.window;
        rig.app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Tab,
            logical_key: Key::Tab,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
        rig.frame();
        rig.hold(KeyCode::ShiftLeft, false);
        rig.app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Tab,
            logical_key: Key::Tab,
            state: ButtonState::Released,
            text: None,
            repeat: false,
            window,
        });
        rig.frame();
    }

    fn active(rig: &mut Rig) -> Option<HandleId> {
        rig.app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .host()
            .unwrap()
            .objects()
            .active_unit()
            .unwrap()
    }

    /// Runs a group-button click on the synthetic session, as the panel's system would.
    fn group_click(rig: &mut Rig, unit: HandleId, modifiers: Modifiers) {
        rig.app.world_mut().resource_scope(
            |world, mut session: Mut<MapScriptSession<MapStartupHost>>| {
                let mut input = world.resource_mut::<CampaignInput>();
                click_group_button(
                    session.host().unwrap().objects_mut(),
                    &mut input,
                    unit,
                    modifiers,
                    Some(12),
                )
                .unwrap();
            },
        );
    }

    #[test]
    fn shift_click_adds_a_unit_and_a_second_shift_click_removes_it() {
        let mut rig = Rig::new(&mixed(), SCREEN);
        let at = rig.screen([24.0, 40.0]);
        rig.click(at);
        shift_click(&mut rig, [40.0, 40.0]);
        assert_eq!(rig.selection(), [rig.units[0], rig.units[1]]);
        shift_click(&mut rig, [24.0, 40.0]);
        assert_eq!(rig.selection(), [rig.units[1]]);
        shift_click(&mut rig, [40.0, 40.0]);
        assert!(rig.selection().is_empty());
        assert_eq!(rig.selects_journaled(), 4);
    }

    #[test]
    fn shift_click_on_a_building_replaces_the_unit_selection() {
        let mut rig = Rig::new(&mixed(), SCREEN);
        rig.select(&[rig.units[0], rig.units[1]]);
        shift_click(&mut rig, [56.0, 76.0]);
        assert_eq!(rig.selection(), [rig.units[4]]);
        assert_eq!(rig.error(), None);
    }

    #[test]
    fn ctrl_click_selects_every_own_unit_of_that_type_on_screen() {
        let mut rig = Rig::new(&mixed(), SCREEN);
        let at = rig.screen([56.0, 40.0]);
        rig.hold(KeyCode::ControlLeft, true);
        rig.click(at);
        rig.hold(KeyCode::ControlLeft, false);
        assert_eq!(rig.selection(), [rig.units[2], rig.units[3]]);
    }

    #[test]
    fn a_click_on_empty_ground_keeps_the_selection() {
        let mut rig = Rig::new(&mixed(), SCREEN);
        rig.select(&[rig.units[0]]);
        let at = rig.screen([110.0, 20.0]);
        rig.click(at);
        assert_eq!(rig.selection(), [rig.units[0]]);
        assert_eq!(rig.selects_journaled(), 0);
    }

    #[test]
    fn the_cap_read_from_the_client_limits_ctrl_click_and_shift_click() {
        let peons: Vec<Spec> = (0..14)
            .map(|i| {
                let row = (i / 7) as f32;
                (0, PEON, [22.0 + 14.0 * (i % 7) as f32, 30.0 + 20.0 * row])
            })
            .collect();
        let mut rig = Rig::new(&peons, SCREEN);
        let at = rig.screen([22.0, 30.0]);
        rig.hold(KeyCode::ControlLeft, true);
        rig.click(at);
        rig.hold(KeyCode::ControlLeft, false);
        assert_eq!(rig.selection(), rig.units[..12]);
        assert!(rig.error().is_some_and(|e| e.contains("more than 12")));
        // The thirteenth peon is not taken by a Shift click on a full selection.
        shift_click(&mut rig, [22.0 + 14.0 * 5.0, 50.0]);
        assert_eq!(rig.selection(), rig.units[..12]);
        // Removing one makes room for it.
        shift_click(&mut rig, [22.0, 30.0]);
        shift_click(&mut rig, [22.0 + 14.0 * 5.0, 50.0]);
        assert_eq!(rig.selection().len(), 12);
        assert_eq!(rig.selection().last(), Some(&rig.units[12]));
    }

    #[test]
    fn tab_and_shift_tab_cycle_subgroups_and_wrap() {
        let mut rig = Rig::new(&mixed(), SCREEN);
        rig.app.add_systems(Update, subgroup_keys);
        let [peon, _, foot, _, hall] = [0, 1, 2, 3, 4].map(|i| rig.units[i]);
        rig.select(&[peon, hall, foot]);
        // Priorities 6, 3, 1: footmen, hall, peons.
        assert_eq!(active(&mut rig), Some(foot));
        tab(&mut rig, false);
        assert_eq!(active(&mut rig), Some(hall));
        tab(&mut rig, false);
        assert_eq!(active(&mut rig), Some(peon));
        tab(&mut rig, false);
        assert_eq!(active(&mut rig), Some(foot));
        tab(&mut rig, true);
        assert_eq!(active(&mut rig), Some(peon));
        tab(&mut rig, true);
        assert_eq!(active(&mut rig), Some(hall));
        // The selection itself never changes.
        assert_eq!(rig.selection(), [peon, hall, foot]);
        // A fresh selection starts again at the first subgroup.
        rig.select(&[peon, foot]);
        assert_eq!(active(&mut rig), Some(foot));
    }

    #[test]
    fn tab_with_one_subgroup_changes_nothing() {
        let mut rig = Rig::new(&mixed(), SCREEN);
        rig.app.add_systems(Update, subgroup_keys);
        rig.select(&[rig.units[0], rig.units[1]]);
        tab(&mut rig, false);
        assert_eq!(active(&mut rig), Some(rig.units[0]));
        tab(&mut rig, true);
        assert_eq!(active(&mut rig), Some(rig.units[0]));
    }

    #[test]
    fn group_button_clicks_select_switch_subgroup_or_remove() {
        let mut rig = Rig::new(&mixed(), SCREEN);
        let [peon, _, foot, foot2, _] = [0, 1, 2, 3, 4].map(|i| rig.units[i]);
        rig.select(&[peon, foot, foot2]);
        let none = Modifiers::default();
        // A unit of another subgroup activates that subgroup and keeps the selection.
        group_click(&mut rig, peon, none);
        assert_eq!(active(&mut rig), Some(peon));
        assert_eq!(rig.selection(), [peon, foot, foot2]);
        // A unit of the active subgroup is selected alone.
        group_click(&mut rig, peon, none);
        assert_eq!(rig.selection(), [peon]);
        rig.select(&[peon, foot, foot2]);
        group_click(&mut rig, foot2, none);
        assert_eq!(rig.selection(), [foot2]);
        // Shift removes the unit.
        rig.select(&[peon, foot, foot2]);
        group_click(
            &mut rig,
            foot,
            Modifiers {
                shift: true,
                ctrl: false,
            },
        );
        assert_eq!(rig.selection(), [peon, foot2]);
        // Ctrl takes the unit's subgroup out of the selection.
        rig.select(&[peon, foot, foot2]);
        group_click(
            &mut rig,
            foot2,
            Modifiers {
                shift: false,
                ctrl: true,
            },
        );
        assert_eq!(rig.selection(), [foot, foot2]);
    }
}
