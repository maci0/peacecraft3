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

/// Human04 with the original data: the rally command is offered to a unit that trains and to
/// no worker, and a hero's items trade slots, drop on the ground and move to another hero
/// through the source ranges.
#[test]
#[ignore = "requires PCX_WAR3_EXE and PCX_NATIVE_CHAPTER_* selecting the unchanged Human04 on roc"]
fn selected_chapter_offers_rally_and_moves_items_between_slots_ground_and_heroes() {
    use pcx_play::playercolor_host::{ItemCommand, ItemDrop};
    use pcx_play::standard_command::Kind as Standard;
    let mut app = crate::native_campaign::tests::started_app();
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let objects = session.host().unwrap().objects_mut();
    let local = objects.local_player_number().unwrap();
    let find = |objects: &ObjectHost, code: &[u8; 4]| {
        objects
            .executed_units()
            .find(|(_, record)| {
                &record.definition.rawcode == code
                    && objects.player_settings(record.owner).unwrap().number == local
            })
            .map(|(id, _)| id)
            .unwrap_or_else(|| panic!("no own {}", String::from_utf8_lossy(code)))
    };
    let id = |objects: &mut ObjectHost, name: &str| match objects
        .native("OrderId", &[Value::Str(name.into())])
    {
        Ok(Value::Int(id)) => id,
        other => panic!("order {name}: {other:?}"),
    };
    let rally = id(objects, "setrally");
    let barracks = objects
        .executed_units()
        .find(|(_, record)| &record.definition.rawcode == b"hbar")
        .map(|(id, _)| id)
        .expect("a barracks");
    let peasant = find(objects, b"hpea");
    let arthas = find(objects, b"Hart");
    assert_eq!(
        objects
            .standard_command_order(barracks, Standard::Rally)
            .unwrap(),
        Some(rally)
    );
    assert_eq!(
        objects
            .standard_command_order(peasant, Standard::Rally)
            .unwrap(),
        None
    );
    assert_eq!(
        objects
            .standard_command_order(arthas, Standard::Rally)
            .unwrap(),
        None
    );

    // Items: two in Arthas' first slots, a second hero beside him.
    let owner = objects.executed_unit(arthas).unwrap().owner;
    let [x, y] = objects.executed_unit(arthas).unwrap().position.unwrap();
    let create_item = |objects: &mut ObjectHost, code: &[u8; 4]| {
        let Ok(Value::Handle(item)) = objects.native(
            "CreateItem",
            &[
                Value::Int(i32::from_be_bytes(*code)),
                Value::Real(x),
                Value::Real(y),
            ],
        ) else {
            panic!("item {}", String::from_utf8_lossy(code))
        };
        item
    };
    let (first, second) = (create_item(objects, b"phea"), create_item(objects, b"rat6"));
    for item in [first, second] {
        assert_eq!(
            objects.native("UnitAddItem", &[Value::Handle(arthas), Value::Handle(item)]),
            Ok(Value::Bool(true))
        );
    }
    let slot = |objects: &mut ObjectHost, unit, slot| {
        objects
            .native("UnitItemInSlot", &[Value::Handle(unit), Value::Int(slot)])
            .unwrap()
    };
    assert_eq!(slot(objects, arthas, 0), Value::Handle(first));
    assert_eq!(
        objects
            .player_move_item(local, arthas, 0, ItemDrop::Slot(1))
            .unwrap(),
        ItemCommand::Done
    );
    assert_eq!(slot(objects, arthas, 0), Value::Handle(second));
    assert_eq!(slot(objects, arthas, 1), Value::Handle(first));

    // The source drop range (DropItemRange) decides between dropping now and walking.
    let near = [x + 1.0, y];
    assert_eq!(
        objects
            .player_move_item(local, arthas, 1, ItemDrop::Ground(near))
            .unwrap(),
        ItemCommand::Done
    );
    let dropped = objects.executed_item(first).unwrap();
    assert_eq!(
        (dropped.held_by, dropped.x, dropped.y),
        (None, near[0], near[1])
    );
    assert_eq!(
        objects
            .player_move_item(local, arthas, 0, ItemDrop::Ground([x + 5000.0, y]))
            .unwrap(),
        ItemCommand::Walking
    );
    assert_eq!(slot(objects, arthas, 0), Value::Handle(second));

    // A peasant has no inventory; another hero beside Arthas takes the item at once.
    let created = |objects: &mut ObjectHost, code: &[u8; 4]| {
        let Ok(Value::Handle(unit)) = objects.native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*code)),
                Value::Real(x),
                Value::Real(y),
                Value::Real(0.0),
            ],
        ) else {
            panic!("unit {}", String::from_utf8_lossy(code))
        };
        unit
    };
    assert_eq!(
        objects
            .player_move_item(local, arthas, 0, ItemDrop::Unit(peasant))
            .unwrap(),
        ItemCommand::Refused("that unit cannot carry items")
    );
    let jaina = created(objects, b"Hjai");
    assert_eq!(
        objects
            .player_move_item(local, arthas, 0, ItemDrop::Unit(jaina))
            .unwrap(),
        ItemCommand::Done
    );
    assert_eq!(slot(objects, arthas, 0), Value::Null);
    assert_eq!(slot(objects, jaina, 0), Value::Handle(second));
}

/// A rig showing the synthetic inventory of its first unit, selected: slots 0, 1 and 3 hold
/// items and three slots are usable. The synthetic layout's slot keys are the letters A to D.
fn rig_with_inventory<M>(
    systems: impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, M>,
) -> Rig {
    let mut rig = Rig::new(&row(), SCREEN);
    rig.app.add_systems(Update, systems.after(sync_queue_flag));
    let unit = rig.units[0];
    rig.select(&[unit]);
    rig.app
        .insert_resource(crate::campaign_inventory::slot_input_tests::shown(
            unit,
            3,
            [true, true, false, true],
        ));
    rig
}

fn used(rig: &Rig) -> Option<(HandleId, usize)> {
    rig.app
        .world()
        .resource::<crate::campaign_inventory::CampaignInventory>()
        .use_request
}

/// The item use a key press asks for on a fresh rig, after `before` set up the case.
fn use_after_tap(key: KeyCode, before: impl FnOnce(&mut Rig)) -> Option<(HandleId, usize)> {
    let mut rig = rig_with_inventory(inventory_hotkey);
    before(&mut rig);
    rig.frame();
    tap(&mut rig, key);
    used(&rig)
}

#[test]
fn a_slot_key_requests_the_item_in_its_slot() {
    let unit = Rig::new(&row(), SCREEN).units[0];
    assert_eq!(use_after_tap(KeyCode::KeyA, |_| {}), Some((unit, 0)));
    assert_eq!(use_after_tap(KeyCode::KeyB, |_| {}), Some((unit, 1)));
    // Shift is not a gate.
    assert_eq!(
        use_after_tap(KeyCode::KeyB, |rig| rig.hold(KeyCode::ShiftLeft, true)),
        Some((unit, 1))
    );
}

#[test]
fn empty_and_unusable_slots_and_other_keys_request_nothing() {
    // Slot 2 is empty, slot 3 holds an item past the unit's capacity.
    for key in [
        KeyCode::KeyC,
        KeyCode::KeyD,
        KeyCode::KeyQ,
        KeyCode::Numpad7,
    ] {
        assert_eq!(use_after_tap(key, |_| {}), None, "{key:?}");
    }
}

#[test]
fn slot_keys_ignore_ctrl_alt_and_closed_input() {
    for modifier in [
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::AltLeft,
        KeyCode::AltRight,
    ] {
        let held = use_after_tap(KeyCode::KeyA, |rig| rig.hold(modifier, true));
        assert_eq!(held, None, "{modifier:?}");
    }
    let closed = use_after_tap(KeyCode::KeyA, |rig| {
        rig.app
            .world_mut()
            .resource_mut::<CampaignCamera>()
            .set_menu_open(true);
    });
    assert_eq!(closed, None);
}

fn release_after(at: crate::campaign_inventory::ReleaseAt) -> (Option<String>, bool) {
    let mut rig = rig_with_inventory(release_inventory_drag);
    let unit = rig.units[0];
    rig.app
        .world_mut()
        .resource_mut::<crate::campaign_inventory::CampaignInventory>()
        .release = Some(crate::campaign_inventory::Release { unit, slot: 0, at });
    rig.frame();
    let pending = rig
        .app
        .world()
        .resource::<crate::campaign_inventory::CampaignInventory>()
        .release
        .is_some();
    (rig.error(), pending)
}

#[test]
fn a_release_reaches_the_host_from_a_slot_or_the_world_and_is_consumed() {
    use crate::campaign_inventory::ReleaseAt;
    // The synthetic session admits no inventory, so the host's refusal is what is shown.
    let in_world = Vec2::new(SCREEN[0] / 2.0, SCREEN[1] / 2.0);
    for at in [ReleaseAt::Slot(1), ReleaseAt::Window(in_world)] {
        let (error, pending) = release_after(at);
        assert!(!pending, "{at:?}");
        assert!(
            error.is_some_and(|error| error.contains("inventory")),
            "{at:?}"
        );
    }
}

#[test]
fn a_release_outside_the_world_view_does_nothing() {
    let (error, pending) = release_after(crate::campaign_inventory::ReleaseAt::Window(
        Vec2::splat(-5.0),
    ));
    assert_eq!((error, pending), (None, false));
}

#[test]
fn a_release_while_input_is_closed_is_discarded() {
    let mut rig = rig_with_inventory(release_inventory_drag);
    let unit = rig.units[0];
    rig.app
        .world_mut()
        .resource_mut::<CampaignCamera>()
        .set_menu_open(true);
    rig.app
        .world_mut()
        .resource_mut::<crate::campaign_inventory::CampaignInventory>()
        .release = Some(crate::campaign_inventory::Release {
        unit,
        slot: 0,
        at: crate::campaign_inventory::ReleaseAt::Slot(1),
    });
    rig.frame();
    assert_eq!(rig.error(), None);
    assert!(
        rig.app
            .world()
            .resource::<crate::campaign_inventory::CampaignInventory>()
            .release
            .is_none()
    );
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
