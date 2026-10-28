//! Command-card hotkeys through the real input systems on the synthetic published session.
//! Key messages are injected; there is no host window, keyboard or audio. The session admits
//! no order rules, so these tests cover the dispatch the input layer owns: which button a key
//! reaches, the Ctrl and Alt gate, the closed-input gate and the cancel codes. Order issue
//! for each command is tested on the order host in `pcx-play`.
use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyboardInput},
};
use pcx_play::command_hotkey::{Hotkey, parse_ranked};

use super::box_select_tests::{Rig, SCREEN, row};
use super::*;
use crate::campaign_command_card::{CommandCard, Kind};

fn key_of(text: &str) -> Option<Hotkey> {
    parse_ranked(text).unwrap()[0]
}

/// A rig with its first unit selected and a card showing the skill-menu pair for it.
fn rig_with_skill_pair() -> Rig {
    let mut rig = Rig::new(&row(), SCREEN);
    rig.app
        .add_systems(Update, command_hotkey.after(sync_queue_flag));
    let unit = rig.units[0];
    rig.select(&[unit]);
    rig.app
        .world_mut()
        .resource_mut::<CommandCard>()
        .show_buttons(
            unit,
            &[
                (Kind::SkillMenu, key_of("O")),
                (Kind::SkillCancel, key_of("27")),
            ],
        );
    rig
}

fn tap(rig: &mut Rig, key_code: KeyCode) {
    let window = rig.window;
    for state in [ButtonState::Pressed, ButtonState::Released] {
        rig.app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window,
        });
        rig.frame();
    }
}

fn skills_open(rig: &Rig) -> bool {
    rig.app.world().resource::<CommandCard>().skill_page_open()
}

#[test]
fn a_hotkey_presses_its_button_and_escape_presses_the_cancel_code() {
    let mut rig = rig_with_skill_pair();
    assert!(!skills_open(&rig));
    tap(&mut rig, KeyCode::KeyO);
    assert!(skills_open(&rig));
    // The unrelated key does nothing.
    tap(&mut rig, KeyCode::KeyQ);
    assert!(skills_open(&rig));
    // The stored cancel code 27 is Escape.
    tap(&mut rig, KeyCode::Escape);
    assert!(!skills_open(&rig));
}

#[test]
fn the_other_stored_cancel_code_also_answers_to_escape() {
    let mut rig = rig_with_skill_pair();
    let unit = rig.units[0];
    rig.app
        .world_mut()
        .resource_mut::<CommandCard>()
        .show_buttons(
            unit,
            &[
                (Kind::SkillMenu, key_of("O")),
                (Kind::SkillCancel, key_of("512")),
            ],
        );
    tap(&mut rig, KeyCode::KeyO);
    assert!(skills_open(&rig));
    tap(&mut rig, KeyCode::Escape);
    assert!(!skills_open(&rig));
}

#[test]
fn a_key_with_ctrl_or_alt_held_is_ignored_by_the_card() {
    for modifier in [
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::AltLeft,
        KeyCode::AltRight,
    ] {
        let mut rig = rig_with_skill_pair();
        rig.hold(modifier, true);
        rig.frame();
        tap(&mut rig, KeyCode::KeyO);
        assert!(!skills_open(&rig), "{modifier:?}");
        rig.hold(modifier, false);
        rig.frame();
        tap(&mut rig, KeyCode::KeyO);
        assert!(skills_open(&rig), "{modifier:?}");
    }
}

#[test]
fn shift_does_not_stop_the_card() {
    let mut rig = rig_with_skill_pair();
    rig.hold(KeyCode::ShiftLeft, true);
    rig.frame();
    tap(&mut rig, KeyCode::KeyO);
    assert!(skills_open(&rig));
}

#[test]
fn keys_do_nothing_while_game_input_is_closed() {
    let mut rig = rig_with_skill_pair();
    rig.app
        .world_mut()
        .resource_mut::<CampaignCamera>()
        .set_menu_open(true);
    rig.frame();
    tap(&mut rig, KeyCode::KeyO);
    assert!(!skills_open(&rig));
}

#[test]
fn the_card_of_another_unit_is_not_reached() {
    let mut rig = rig_with_skill_pair();
    let other = rig.units[1];
    rig.select(&[other]);
    tap(&mut rig, KeyCode::KeyO);
    assert!(!skills_open(&rig));
}

#[test]
fn a_button_without_a_hotkey_is_not_reached() {
    let mut rig = Rig::new(&row(), SCREEN);
    rig.app
        .add_systems(Update, command_hotkey.after(sync_queue_flag));
    let unit = rig.units[0];
    rig.select(&[unit]);
    rig.app
        .world_mut()
        .resource_mut::<CommandCard>()
        .show_buttons(unit, &[(Kind::SkillMenu, None)]);
    for key in [KeyCode::KeyO, KeyCode::Escape, KeyCode::Digit0] {
        tap(&mut rig, key);
    }
    assert!(!skills_open(&rig));
    assert_eq!(rig.error(), None);
}
