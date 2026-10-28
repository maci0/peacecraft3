//! Right click and Shift through the real input systems on the synthetic published
//! session. The session admits no order rules, so these tests cover what the input layer
//! decides itself: Shift as the queue flag, Shift keeping targeting armed, and silent
//! right clicks for units that have no order. Order resolution and queue behaviour are
//! tested on the order host in `pcx-play`; the chapter test below runs both on Human01.
use bevy::input::ButtonState;
use pcx_play::standard_command::Kind as Standard;

use super::box_select_tests::{Rig, SCREEN, row};
use super::*;

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
    rig.app
        .world()
        .resource::<CampaignInput>()
        .command
        .is_some()
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

/// Human04 with the original data: units of the chapter resolve a smart order from their
/// own abilities and unit data, and Shift queues behind a running order.
#[test]
#[ignore = "requires PCX_WAR3_EXE and PCX_NATIVE_CHAPTER_* selecting the unchanged Human04 on roc"]
fn selected_chapter_resolves_smart_orders_from_unit_data_and_queues_with_shift() {
    use pcx_play::playercolor_host::{OrderForm, SmartRefusal, SmartTarget};
    let mut app = crate::native_campaign::tests::started_app();
    let start = {
        let seed = app
            .world()
            .resource::<peacecraft3::campaign_startup::CampaignSeed>()
            .prepared
            .unwrap();
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        pcx_play::player_command_log::StartIdentity::from_context(seed, session.context().unwrap())
    };
    app.world_mut()
        .resource_mut::<CampaignInput>()
        .commands
        .begin(start);
    app.world_mut()
        .resource_scope(|world, mut input: Mut<CampaignInput>| {
            let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
            let objects = session.host().unwrap().objects_mut();
            objects
                .native("EnableUserControl", &[Value::Bool(true)])
                .unwrap();
            let id = |objects: &mut ObjectHost, name: &str| match objects
                .native("OrderId", &[Value::Str(name.into())])
            {
                Ok(Value::Int(id)) => id,
                other => panic!("order {name}: {other:?}"),
            };
            let [move_id, attack_id, rally_id, harvest_id] =
                ["move", "attack", "setrally", "harvest"].map(|name| id(objects, name));
            let local = objects.local_player_number().unwrap();
            let find = |objects: &ObjectHost, code: &[u8; 4], own: Option<bool>| {
                objects
                    .executed_units()
                    .find(|(_, record)| {
                        &record.definition.rawcode == code
                            && own.is_none_or(|own| {
                                (objects.player_settings(record.owner).unwrap().number == local)
                                    == own
                            })
                    })
                    .map(|(id, _)| id)
                    .unwrap_or_else(|| panic!("no {}", String::from_utf8_lossy(code)))
            };
            let arthas = find(objects, b"Hart", Some(true));
            let peasant = find(objects, b"hpea", Some(true));
            let barracks = find(objects, b"hbar", None);
            let mine = find(objects, b"ngol", None);
            let enemy = find(objects, b"uaco", None);
            let point = [100.0, 200.0];
            let mut resolve = |unit, target| objects.resolve_smart_order(unit, target);
            assert_eq!(
                resolve(arthas, SmartTarget::Ground(point)),
                Ok(OrderForm::Point {
                    order: move_id,
                    at: point
                })
            );
            assert_eq!(
                resolve(barracks, SmartTarget::Ground(point)),
                Ok(OrderForm::Point {
                    order: rally_id,
                    at: point
                })
            );
            assert!(matches!(
                resolve(barracks, SmartTarget::Unit(peasant)),
                Err(SmartRefusal::NotSimulated(_))
            ));
            assert_eq!(
                resolve(arthas, SmartTarget::Unit(enemy)),
                Ok(OrderForm::Target {
                    order: attack_id,
                    target: enemy
                })
            );
            assert_eq!(
                resolve(peasant, SmartTarget::Unit(arthas)),
                Ok(OrderForm::Target {
                    order: move_id,
                    target: arthas
                })
            );
            assert_eq!(
                resolve(arthas, SmartTarget::Unit(arthas)),
                Err(SmartRefusal::SelfTarget)
            );
            assert_eq!(
                resolve(peasant, SmartTarget::Unit(mine)),
                Ok(OrderForm::Target {
                    order: harvest_id,
                    target: mine
                })
            );
            assert_eq!(
                resolve(arthas, SmartTarget::Unit(mine)),
                Ok(OrderForm::Target {
                    order: move_id,
                    target: mine
                })
            );
            let destructables: Vec<_> = objects
                .destructables_with_handles()
                .map(|(handle, _, _)| handle)
                .collect();
            let tree = destructables
                .into_iter()
                .find(|&tree| {
                    matches!(
                        objects.resolve_smart_order(
                            peasant,
                            SmartTarget::Destructable { target: tree, at: point }
                        ),
                        Ok(OrderForm::Target { order, .. }) if order == harvest_id
                    )
                })
                .expect("a harvestable tree");
            assert_eq!(
                objects.resolve_smart_order(
                    arthas,
                    SmartTarget::Destructable {
                        target: tree,
                        at: point
                    }
                ),
                Ok(OrderForm::Point {
                    order: move_id,
                    at: point
                })
            );

            // The resolved harvest order is one the harvest runtime takes.
            objects
                .native("PauseUnit", &[Value::Handle(peasant), Value::Bool(false)])
                .unwrap();
            // Human04 starts the player without a town hall, so the runtime has no depot to
            // work a mine from and refuses with that reason; the input layer shows it as the
            // order error.
            let harvest = OrderForm::Target {
                order: harvest_id,
                target: mine,
            };
            let refused = objects.issue_order_form(peasant, harvest).unwrap_err();
            assert!(refused.contains("no gold-accepting depot"), "{refused}");

            // Shift: the first order runs, the second waits, an unflagged one replaces both.
            objects
                .native("PauseUnit", &[Value::Handle(arthas), Value::Bool(false)])
                .unwrap();
            objects.select_units(&[arthas], true);
            let there = [point[0] + 50.0, point[1]];
            input.queue_orders = true;
            issue_smart_orders(objects, &mut input, SmartTarget::Ground(point));
            issue_smart_orders(objects, &mut input, SmartTarget::Ground(there));
            assert_eq!(
                objects.queued_orders(arthas),
                [OrderForm::Point {
                    order: move_id,
                    at: there
                }]
            );
            assert_eq!(
                objects.native("GetUnitCurrentOrder", &[Value::Handle(arthas)]),
                Ok(Value::Int(move_id))
            );
            let queued: Vec<_> = input
                .commands
                .entries()
                .iter()
                .filter(|entry| matches!(entry.command, PlayerCommand::Queued(_)))
                .collect();
            assert_eq!(queued.len(), 2, "{:?}", input.last_error);
            input.queue_orders = false;
            issue_smart_orders(objects, &mut input, SmartTarget::Ground(point));
            assert!(objects.queued_orders(arthas).is_empty());
            assert_eq!(input.last_error, None);
        });
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
