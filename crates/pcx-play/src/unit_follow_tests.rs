use super::*;

fn opened(structure: bool) -> Vm {
    opened_value(FieldValue::Cell(if structure { "1" } else { "0" }.into()))
}

fn opened_value(value: FieldValue) -> Vm {
    let mut field = geometry_field(Ok(value));
    let meta = field.metadata.as_mut().unwrap();
    meta.fourcc = *b"F001";
    meta.field = "isbldg".into();
    meta.kind = FieldKind::Bool;
    meta.declared = "bool".into();
    meta.source = FieldSource::Column {
        slk: SlkId::UnitBalance,
        column: "isbldg".into(),
    };
    let mut vm = open_with_fields(&[(*b"F001", field)]);
    for name in ["A", "B"] {
        let id = unit(&vm, name);
        vm.host()
            .inner
            .native("SetUnitPathing", &[Value::Handle(id), Value::Bool(false)])
            .unwrap();
    }
    vm
}

fn current(vm: &mut Vm, actor: HandleId) -> Value {
    vm.host()
        .inner
        .native("GetUnitCurrentOrder", &[Value::Handle(actor)])
        .unwrap()
}

fn follow(vm: &mut Vm, actor: HandleId, target: HandleId, numeric: bool) -> Value {
    let order = if numeric {
        vm.host()
            .inner
            .native("OrderId", &[Value::Str("gamma".into())])
            .unwrap()
    } else {
        Value::Str("gamma".into())
    };
    vm.host()
        .inner
        .native(
            if numeric {
                "IssueTargetOrderById"
            } else {
                "IssueTargetOrder"
            },
            &[Value::Handle(actor), order, Value::Handle(target)],
        )
        .unwrap()
}

fn target(vm: &mut Vm, actor: HandleId) -> Option<HandleId> {
    vm.host()
        .inner
        .executed_unit(actor)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .followed_unit()
}

#[test]
fn follow_retains_its_target_at_source_range_and_resumes_when_the_target_moves() {
    for numeric in [false, true] {
        let mut vm = opened(false);
        let a = unit(&vm, "A");
        let b = unit(&vm, "B");
        assert_eq!(
            vm.host()
                .inner
                .executed_unit(a)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .follow_range(),
            None
        );
        assert_eq!(follow(&mut vm, a, b, numeric), Value::Bool(true));
        assert_eq!(
            vm.host()
                .inner
                .executed_unit(a)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .follow_range(),
            Some(12.0)
        );
        tick(&mut vm, 2.0);
        assert_eq!(position(&mut vm, a), [32.0, 16.0]);
        tick(&mut vm, 6.5);
        assert_eq!(position(&mut vm, a), [68.0, 16.0]);
        tick(&mut vm, 8.0);
        assert_eq!(position(&mut vm, a), [68.0, 16.0]);
        assert_eq!(target(&mut vm, a), Some(b));
        assert_eq!(current(&mut vm, a), Value::Int(1631));
        vm.host()
            .inner
            .native(
                "SetUnitPosition",
                &[Value::Handle(b), Value::Real(100.0), Value::Real(16.0)],
            )
            .unwrap();
        tick(&mut vm, 9.0);
        assert_eq!(position(&mut vm, a), [76.0, 16.0]);
        vm.host()
            .inner
            .native("RemoveUnit", &[Value::Handle(b)])
            .unwrap();
        tick(&mut vm, 10.0);
        assert_eq!(
            vm.host()
                .inner
                .executed_unit(a)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .follow_range(),
            None
        );
        assert_eq!(target(&mut vm, a), None);
        assert_eq!(position(&mut vm, a), [76.0, 16.0]);
        assert_eq!(current(&mut vm, a), Value::Int(0));
    }
}

#[test]
fn zero_speed_follow_restart_waits_for_a_positive_speed_without_losing_the_target() {
    const ZERO_MIN: &str =
        "[Misc]\nMinUnitSpeed=0\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300\n";
    let mut vm = open_move_speed_with(ZERO_MIN, "10");
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    vm.host()
        .inner
        .native("SetUnitPathing", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
    tick(&mut vm, 1.0);
    let before = position(&mut vm, a);
    let expected_order = vm
        .host()
        .inner
        .native("OrderId", &[Value::Str("gamma".into())])
        .unwrap();
    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(a), Value::Real(0.0)])
        .unwrap();
    vm.host()
        .inner
        .native(
            "SetUnitPosition",
            &[Value::Handle(b), Value::Real(100.0), Value::Real(16.0)],
        )
        .unwrap();
    tick(&mut vm, 2.0);
    assert_eq!(position(&mut vm, a), before);
    assert_eq!(target(&mut vm, a), Some(b));
    assert_eq!(current(&mut vm, a), expected_order);
    assert_eq!(move_speed(&mut vm, a), 0.0);

    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(a), Value::Real(10.0)])
        .unwrap();
    tick(&mut vm, 3.0);
    tick(&mut vm, 4.0);
    assert_ne!(position(&mut vm, a), before);
    assert_eq!(target(&mut vm, a), Some(b));
    assert_eq!(current(&mut vm, a), expected_order);
    assert_eq!(move_speed(&mut vm, a), 10.0);
}

#[test]
fn structure_range_and_pause_preserve_the_follow_order_without_catchup() {
    let mut vm = opened(true);
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
    tick(&mut vm, 2.0);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    tick(&mut vm, 10.0);
    assert_eq!(position(&mut vm, a), [32.0, 16.0]);
    assert_eq!(target(&mut vm, a), Some(b));
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    tick(&mut vm, 15.5);
    assert_eq!(position(&mut vm, a), [76.0, 16.0]);
    assert_eq!(target(&mut vm, a), Some(b));
}

#[test]
fn rejected_follow_does_not_replace_motion_and_a_new_order_cancels_follow() {
    let mut vm = opened(false);
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    vm.call("move", &[]).unwrap();
    let before = vm.host().inner.executed_unit(a).unwrap().activity.clone();
    assert_eq!(follow(&mut vm, a, a, false), Value::Bool(false));
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().activity, before);
    assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
    vm.call("hold", &[]).unwrap();
    assert_eq!(target(&mut vm, a), None);
    assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
    vm.call("stop", &[]).unwrap();
    assert_eq!(target(&mut vm, a), None);
    assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
    vm.call("move", &[]).unwrap();
    assert_eq!(target(&mut vm, a), None);
}

#[test]
fn follow_profile_overrides_are_source_bound_and_invalid_values_refuse() {
    let (_, base) = rules();
    let stock = b"[Misc]\nFollowRange=19\nStructureFollowRange=7\n";
    let edited = base
        .clone()
        .with_follow_profiles(
            &[("stock", stock), ("map", b"[Misc]\nFollowRange=5\n")],
            4096,
        )
        .unwrap();
    let parsed = edited.follow_rules().unwrap();
    assert_eq!((parsed.range, parsed.structure_range), (5.0, 7.0));
    assert_eq!(parsed.sources[0].1, crate::neutral_player::sha256(stock));
    for invalid in [
        "[Misc]\nFollowRange=-1\nStructureFollowRange=2",
        "[Misc]\nFollowRange=NaN\nStructureFollowRange=2",
        "[Misc]\nFollowRange=1",
        "[Misc]\nFollowRange=1,2\nStructureFollowRange=2",
        "[Misc]\nFollowRange=1\nFollowRange=2\nStructureFollowRange=3",
    ] {
        assert!(
            base.clone()
                .with_follow_profiles(&[("invalid", invalid.as_bytes())], 4096)
                .is_err()
        );
    }
}

#[test]
fn relocation_into_range_stops_the_old_route_and_projection_work_stays_bounded() {
    let mut vm = opened(false);
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
    vm.host()
        .inner
        .native(
            "SetUnitPosition",
            &[Value::Handle(a), Value::Real(72.0), Value::Real(16.0)],
        )
        .unwrap();
    tick(&mut vm, 1.0);
    assert_eq!(position(&mut vm, a), [72.0, 16.0]);
    assert_eq!(target(&mut vm, a), Some(b));
    assert!(
        !vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    let host = &mut vm.host().inner;
    assert!(host.next_timer_expiry(2.0, &mut 1000).unwrap().is_none());
    let mut spans = Vec::new();
    assert!(
        host.advance_unit_activity_spans(2.0, &mut 0, &mut spans)
            .is_err()
    );
    assert!(spans.is_empty());
}

#[test]
fn stationary_target_routes_and_projection_chunks_preserve_the_same_endpoint() {
    for pathing in [false, true] {
        let mut whole = opened(false);
        let a = unit(&whole, "A");
        let b = unit(&whole, "B");
        whole
            .host()
            .inner
            .native("SetUnitPathing", &[Value::Handle(a), Value::Bool(pathing)])
            .unwrap();
        assert_eq!(follow(&mut whole, a, b, false), Value::Bool(true));
        tick(&mut whole, 8.0);
        let mut split = opened(false);
        let x = unit(&split, "A");
        let y = unit(&split, "B");
        split
            .host()
            .inner
            .native("SetUnitPathing", &[Value::Handle(x), Value::Bool(pathing)])
            .unwrap();
        assert_eq!(follow(&mut split, x, y, true), Value::Bool(true));
        for half in 1..=16 {
            tick(&mut split, f64::from(half) * 0.5);
        }
        assert_eq!(position(&mut whole, a), [68.0, 16.0]);
        assert_eq!(position(&mut split, x), position(&mut whole, a));
        assert_eq!(target(&mut whole, a), Some(b));
        assert_eq!(target(&mut split, x), Some(y));
    }
}

#[test]
fn structure_native_and_follow_share_typed_boolean_source_classification() {
    for (value, expected) in [
        (FieldValue::Cell("0".into()), false),
        (FieldValue::Cell("1".into()), true),
        (FieldValue::Override(pcx_objects::Value::Int(0)), false),
        (FieldValue::Override(pcx_objects::Value::Int(1)), true),
    ] {
        let mut vm = opened_value(value);
        let a = unit(&vm, "A");
        let b = unit(&vm, "B");
        let kind = vm.global("UNIT_TYPE_STRUCTURE");
        assert_eq!(
            vm.host()
                .inner
                .native("IsUnitType", &[Value::Handle(b), kind])
                .unwrap(),
            Value::Bool(expected)
        );
        assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
        tick(&mut vm, 8.0);
        assert_eq!(
            position(&mut vm, a),
            [if expected { 76.0 } else { 68.0 }, 16.0]
        );
    }
    let mut invalid = opened_value(FieldValue::Override(pcx_objects::Value::Int(2)));
    let unit = unit(&invalid, "B");
    let kind = invalid.global("UNIT_TYPE_STRUCTURE");
    assert!(
        invalid
            .host()
            .inner
            .native("IsUnitType", &[Value::Handle(unit), kind])
            .is_err()
    );
}

#[test]
fn blocked_follow_keeps_stand_animation_running_and_recovers_when_unblocked() {
    let mut vm = opened(false);
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    vm.host()
        .inner
        .native("SetUnitPathing", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    let mut east = None;
    for at in [[26.0, 16.0], [10.0, 16.0], [16.0, 26.0], [16.0, 10.0]] {
        let Value::Handle(blocker) = position_create(&mut vm.host().inner, at[0], at[1]).unwrap()
        else {
            panic!("blocker")
        };
        if at[0] > 16.0 {
            east = Some(blocker);
        }
    }
    assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
    tick(&mut vm, 1.0);
    let stop = position(&mut vm, a);
    let started = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .animation
        .as_ref()
        .unwrap()
        .started;
    for time in [2.0, 3.0, 4.0] {
        tick(&mut vm, time);
        assert_eq!(position(&mut vm, a), stop);
        let record = vm.host().inner.executed_unit(a).unwrap();
        assert!(!record.activity.as_ref().unwrap().moving());
        assert_eq!(record.activity.as_ref().unwrap().followed_unit(), Some(b));
        assert_eq!(
            record.animation.as_ref().unwrap().started,
            started,
            "blocked retry restarted the standing pose"
        );
    }
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(east.unwrap())])
        .unwrap();
    tick(&mut vm, 5.0);
    assert_eq!(
        position(&mut vm, a),
        stop,
        "the post-projection search grants no elapsed travel"
    );
    tick(&mut vm, 6.0);
    assert!(position(&mut vm, a)[0] > stop[0]);
}

#[test]
fn deferred_follow_refresh_does_not_credit_the_wait_as_travel() {
    let mut vm = opened(false);
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    assert_eq!(follow(&mut vm, a, b, false), Value::Bool(true));
    tick(&mut vm, 8.0);
    let before = position(&mut vm, a);
    vm.host()
        .inner
        .native("SetUnitPathing", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    vm.host()
        .inner
        .native(
            "SetUnitPosition",
            &[Value::Handle(b), Value::Real(100.0), Value::Real(16.0)],
        )
        .unwrap();
    let host = &mut vm.host().inner;
    host.next_timer_expiry(9.0, &mut 1000).unwrap();
    host.advance_unit_activity(9.0, &mut 1).unwrap();
    assert_eq!(position(&mut vm, a), before);
    assert_eq!(target(&mut vm, a), Some(b));
    tick(&mut vm, 10.0);
    assert_eq!(position(&mut vm, a), before);
    let speed = move_speed(&mut vm, a);
    tick(&mut vm, 11.0);
    let after = position(&mut vm, a);
    assert!(after[0] > before[0]);
    assert!(
        after[0] - before[0] <= speed,
        "only one moving interval is credited"
    );
}
