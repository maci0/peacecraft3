//! Right click and Shift through the real input systems on the synthetic published
//! session. The session admits no order rules, so these tests cover what the input layer
//! decides itself: Shift as the queue flag, Shift keeping targeting armed, and silent
//! right clicks for units that have no order. Order resolution and queue behaviour are
//! tested on the order host in `pcx-play`; the chapter test below runs both on Human01.
use bevy::input::ButtonState;
use pcx_play::standard_command::Kind as Standard;

use super::*;
use super::box_select_tests::{Rig, SCREEN, row};

fn arm(rig: &mut Rig, kind: crate::campaign_command_card::Kind, order: i32) {
    let session = rig
        .app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    rig.app.world_mut().resource_mut::<CampaignInput>().command = Some(ArmedCommand {
        order,
        kind,
        session,
    });
}

fn armed(rig: &Rig) -> bool {
    rig.app.world().resource::<CampaignInput>().command.is_some()
}

fn right_click(rig: &mut Rig, at: [f32; 2]) {
    rig.cursor(at);
    rig.button(MouseButton::Right, ButtonState::Pressed);
    rig.frame();
    rig.button(MouseButton::Right, ButtonState::Released);
    rig.frame();
}

fn queueing(rig: &Rig) -> bool {
    rig.app.world().resource::<CampaignInput>().queue_orders
}

#[test]
fn the_queue_flag_is_shift_held_while_input_is_open() {
    let mut rig = Rig::new(&row(), SCREEN);
    assert!(!queueing(&rig));
    rig.hold(KeyCode::ShiftLeft, true);
    rig.frame();
    assert!(queueing(&rig));
    rig.hold(KeyCode::ShiftLeft, false);
    rig.frame();
    assert!(!queueing(&rig));
    rig.hold(KeyCode::ShiftRight, true);
    rig.frame();
    assert!(queueing(&rig));
}

#[test]
fn shift_keeps_an_armed_standard_command_armed_after_its_click() {
    let mut rig = Rig::new(&row(), SCREEN);
    let first = rig.units[0];
    rig.select(&[first]);
    let ground = rig.screen([60.0, 60.0]);
    let move_card = crate::campaign_command_card::Kind::Standard(Standard::Move);
    for (shift, still_armed) in [(false, false), (true, true)] {
        arm(&mut rig, move_card, 1);
        rig.hold(KeyCode::ShiftLeft, shift);
        rig.frame();
        rig.click(ground);
        rig.hold(KeyCode::ShiftLeft, false);
        assert_eq!(armed(&rig), still_armed, "shift {shift}");
    }
    // A right click cancels even with Shift held.
    arm(&mut rig, move_card, 1);
    rig.hold(KeyCode::ShiftLeft, true);
    rig.frame();
    right_click(&mut rig, ground);
    rig.hold(KeyCode::ShiftLeft, false);
    assert!(!armed(&rig));
}

#[test]
fn shift_keeps_a_unit_targeted_command_armed_after_its_click() {
    let mut rig = Rig::new(&row(), SCREEN);
    let first = rig.units[0];
    rig.select(&[first]);
    let target = rig.screen([40.0, 40.0]);
    let card = crate::campaign_command_card::Kind::TargetUnit;
    for (shift, still_armed) in [(false, false), (true, true)] {
        arm(&mut rig, card, 3);
        rig.hold(KeyCode::ShiftLeft, shift);
        rig.frame();
        rig.click(target);
        rig.hold(KeyCode::ShiftLeft, false);
        assert_eq!(armed(&rig), still_armed, "shift {shift}");
    }
}

#[test]
fn a_right_click_on_units_with_no_order_is_silent() {
    let mut rig = Rig::new(&row(), SCREEN);
    let (peon, hall) = (rig.units[0], rig.units[5]);
    rig.select(&[peon, hall]);
    for game in [[60.0, 60.0], [40.0, 40.0], [24.0, 40.0], [100.0, 76.0]] {
        let at = rig.screen(game);
        right_click(&mut rig, at);
        rig.hold(KeyCode::ShiftLeft, true);
        right_click(&mut rig, at);
        rig.hold(KeyCode::ShiftLeft, false);
    }
    assert_eq!(rig.error(), None);
    assert_eq!(rig.selection(), [peon, hall]);
    assert!(
        rig.app
            .world()
            .resource::<CampaignInput>()
            .commands
            .entries()
            .is_empty()
    );
}
