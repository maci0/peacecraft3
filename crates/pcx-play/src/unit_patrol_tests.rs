use super::*;

fn patrol(vm: &mut Vm, a: HandleId, goal: [f32; 2]) -> Value {
    vm.host()
        .inner
        .native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("delta".into()),
                Value::Real(goal[0]),
                Value::Real(goal[1]),
            ],
        )
        .unwrap()
}

fn current(vm: &mut Vm, a: HandleId) -> Value {
    vm.host()
        .inner
        .native("GetUnitCurrentOrder", &[Value::Handle(a)])
        .unwrap()
}

fn free(vm: &mut Vm, a: HandleId) {
    vm.host()
        .inner
        .native("SetUnitPathing", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
}

#[test]
fn patrol_identity_follows_changed_source_comparison_and_registry_operands() {
    for id in [-707, 2122, 98765] {
        let (registry, rules) = rules_with_patrol(id);
        assert_eq!(registry.id("delta"), Some(id));
        assert_eq!(rules.patrol, id);
        assert_ne!(rules.patrol, rules.movement);
    }
}

#[test]
fn patrol_repeats_both_endpoints_and_spans_keep_the_unspent_time_across_turns() {
    let mut vm = open();
    let a = unit(&vm, "A");
    free(&mut vm, a);
    let start = position(&mut vm, a);
    let end = [48.0, start[1]];
    let leg = f64::from(end[0] - start[0]) / f64::from(move_speed(&mut vm, a));
    assert_eq!(patrol(&mut vm, a, end), Value::Bool(true));
    let expected_order = vm
        .host()
        .inner
        .native("OrderId", &[Value::Str("delta".into())])
        .unwrap();
    let until = 2.5 * leg;
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(until, &mut 1000).unwrap().is_none());
    let mut spans = Vec::new();
    h.advance_unit_activity_spans(until, &mut 65536, &mut spans)
        .unwrap();
    assert_eq!(position(&mut vm, a), [(start[0] + end[0]) / 2.0, start[1]]);
    assert_eq!(current(&mut vm, a), expected_order);
    assert_eq!(spans.len(), 3);
    assert_eq!((spans[0].from, spans[0].to), (start, end));
    assert_eq!((spans[1].from, spans[1].to), (end, start));
    assert_eq!(spans[0].start, 0.0);
    assert_eq!(spans.last().unwrap().end, until);
    for pair in spans.windows(2) {
        assert_eq!(pair[0].to, pair[1].from);
        assert_eq!(pair[0].end, pair[1].start);
    }
    let mut split = open();
    let other = unit(&split, "A");
    free(&mut split, other);
    assert_eq!(patrol(&mut split, other, end), Value::Bool(true));
    for quarter in 1..=10 {
        tick(&mut split, leg * f64::from(quarter) / 4.0);
    }
    assert_eq!(position(&mut split, other), position(&mut vm, a));
    assert_eq!(current(&mut split, other), expected_order);
}

#[test]
fn patrol_accepts_each_point_form_and_pause_retains_the_leg_without_catchup() {
    for name in [
        "IssuePointOrder",
        "IssuePointOrderById",
        "IssuePointOrderLoc",
        "IssuePointOrderByIdLoc",
    ] {
        let mut vm = open();
        let a = unit(&vm, "A");
        free(&mut vm, a);
        let start = position(&mut vm, a);
        let end = [48.0, start[1]];
        let id = vm
            .host()
            .inner
            .native("OrderId", &[Value::Str("delta".into())])
            .unwrap();
        let location = vm
            .host()
            .inner
            .native("Location", &[Value::Real(end[0]), Value::Real(end[1])])
            .unwrap();
        let order = if name.contains("ById") {
            id.clone()
        } else {
            Value::Str("delta".into())
        };
        let mut args = vec![Value::Handle(a), order];
        if name.ends_with("Loc") {
            args.push(location);
        } else {
            args.extend([Value::Real(end[0]), Value::Real(end[1])]);
        }
        assert_eq!(
            vm.host().inner.native(name, &args).unwrap(),
            Value::Bool(true)
        );
        tick(&mut vm, 1.0);
        let before = position(&mut vm, a);
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
            .unwrap();
        tick(&mut vm, 20.0);
        assert_eq!(position(&mut vm, a), before);
        assert_eq!(current(&mut vm, a), id);
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
            .unwrap();
        tick(&mut vm, 21.0);
        assert_eq!(position(&mut vm, a)[0] - before[0], move_speed(&mut vm, a));
        assert_eq!(current(&mut vm, a), id);
    }
}

#[test]
fn replacement_and_refused_orders_preserve_the_right_patrol_state() {
    let mut vm = open();
    let a = unit(&vm, "A");
    free(&mut vm, a);
    assert_eq!(patrol(&mut vm, a, [48.0, 16.0]), Value::Bool(true));
    tick(&mut vm, 1.0);
    let before = vm.host().inner.executed_unit(a).unwrap().activity.clone();
    assert!(
        vm.host()
            .inner
            .native(
                "IssuePointOrder",
                &[
                    Value::Handle(a),
                    Value::Str("delta".into()),
                    Value::Real(f32::NAN),
                    Value::Real(16.0)
                ]
            )
            .is_err()
    );
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().activity, before);
    assert_eq!(vm.call("stop", &[]).unwrap(), Value::Bool(true));
    let stopped = position(&mut vm, a);
    tick(&mut vm, 30.0);
    assert_eq!(position(&mut vm, a), stopped);
    assert_eq!(current(&mut vm, a), Value::Int(0));
    assert_eq!(patrol(&mut vm, a, [48.0, 16.0]), Value::Bool(true));
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 60.0);
    assert_eq!(position(&mut vm, a), [72.0, 16.0]);
    assert_eq!(current(&mut vm, a), Value::Int(0));
}

#[test]
fn blocked_patrol_keeps_its_intent_at_zero_speed_and_resumes_when_positive() {
    const ZERO_MIN: &str =
        "[Misc]\nMinUnitSpeed=0\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300\n";
    let mut vm = open_move_speed_with(ZERO_MIN, "10");
    let a = unit(&vm, "A");
    let start = position(&mut vm, a);
    let end = [48.0, start[1]];
    let expected_order = vm
        .host()
        .inner
        .native("OrderId", &[Value::Str("delta".into())])
        .unwrap();
    assert_eq!(patrol(&mut vm, a, end), Value::Bool(true));
    tick(&mut vm, 1.0);
    let stopped = position(&mut vm, a);
    assert_ne!(stopped, start);

    let blockers = {
        let h = &mut vm.host().inner;
        [
            [stopped[0] + 8.0, stopped[1]],
            [stopped[0] - 8.0, stopped[1]],
            [stopped[0], stopped[1] + 8.0],
            [stopped[0], stopped[1] - 8.0],
        ]
        .map(|at| position_create(h, at[0], at[1]).unwrap())
    };
    tick(&mut vm, 2.0);
    let blocked = vm.host().inner.executed_unit(a).unwrap().activity.clone();
    assert!(blocked.as_ref().is_some_and(|s| !s.moving()));
    assert_eq!(position(&mut vm, a), stopped);
    assert_eq!(current(&mut vm, a), expected_order);

    for blocker in blockers {
        let Value::Handle(blocker) = blocker else {
            panic!("blocker unit");
        };
        vm.host()
            .inner
            .native("RemoveUnit", &[Value::Handle(blocker)])
            .unwrap();
    }
    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(a), Value::Real(0.0)])
        .unwrap();
    tick(&mut vm, 3.0);
    let held = vm.host().inner.executed_unit(a).unwrap().activity.clone();
    assert!(
        held.as_ref()
            .is_some_and(|s| !s.moving() && s.destination().is_none())
    );
    assert_eq!(position(&mut vm, a), stopped);
    assert_eq!(move_speed(&mut vm, a), 0.0);
    assert_eq!(current(&mut vm, a), expected_order);

    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(a), Value::Real(10.0)])
        .unwrap();
    tick(&mut vm, 4.0);
    let resumed = vm.host().inner.executed_unit(a).unwrap().activity.clone();
    assert!(
        resumed
            .as_ref()
            .is_some_and(|s| s.moving() && s.destination() == Some(end))
    );
    tick(&mut vm, 5.0);
    assert_ne!(position(&mut vm, a), stopped);
    assert_ne!(move_speed(&mut vm, a), 0.0);
    assert_eq!(current(&mut vm, a), expected_order);
}

#[test]
fn coincident_patrol_endpoints_stand_and_many_cycles_refuse_bounded_work() {
    let mut vm = open();
    let a = unit(&vm, "A");
    free(&mut vm, a);
    let start = position(&mut vm, a);
    assert_eq!(patrol(&mut vm, a, start), Value::Bool(true));
    tick(&mut vm, 100.0);
    assert_eq!(position(&mut vm, a), start);
    assert_ne!(current(&mut vm, a), Value::Int(0));
    let end = [start[0] + 1.0, start[1]];
    assert_eq!(patrol(&mut vm, a, end), Value::Bool(true));
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(200.0, &mut 1000).unwrap().is_none());
    let mut spans = Vec::new();
    assert!(
        h.advance_unit_activity_spans(200.0, &mut 8, &mut spans)
            .is_err()
    );
    assert!(
        spans.is_empty(),
        "a failed projection must truncate its appended spans"
    );
}
