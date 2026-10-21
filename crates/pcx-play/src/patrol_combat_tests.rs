use super::*;

#[test]
fn patrol_acquires_then_resumes_its_leg_and_continues_both_directions_after_combat() {
    let mut fields = armed("8", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("128".into()));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    let start = h.executed_unit(a).unwrap().position.unwrap();
    let end = [96.0, 16.0];
    let alliance = h.native("ConvertAllianceType", &[Value::Int(0)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            Value::Handle(h.executed_unit(b).unwrap().owner),
            Value::Handle(h.executed_unit(a).unwrap().owner),
            alliance,
            Value::Bool(true),
        ],
    )
    .unwrap();
    let patrol = h.native("OrderId", &[Value::Str("delta".into())]).unwrap();
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("delta".into()),
                Value::Real(end[0]),
                Value::Real(end[1])
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 0.5);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.executed_unit(a).unwrap().combat.as_ref().unwrap().target,
        Some(b)
    );
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        patrol
    );
    h.native("KillUnit", &[Value::Handle(b)]).unwrap();
    step(&mut vm, 1.0);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination,
        Some(end)
    );
    let p = h.executed_unit(a).unwrap().position.unwrap();
    let Value::Real(speed) = h.native("GetUnitMoveSpeed", &[Value::Handle(a)]).unwrap() else {
        panic!("source speed");
    };
    let leg = f64::from(end[0] - start[0]) / f64::from(speed);
    let mid_return =
        1.0 + f64::from((end[0] - p[0]).hypot(end[1] - p[1])) / f64::from(speed) + leg / 2.0;
    step(&mut vm, mid_return);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination,
        Some(start)
    );
    step(&mut vm, mid_return + leg);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination,
        Some(end)
    );
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        patrol
    );
}

#[test]
fn an_explicit_target_order_replaces_patrol_without_resuming_the_old_endpoint() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    let h = &mut vm.host().inner;
    h.native(
        "IssuePointOrder",
        &[
            Value::Handle(a),
            Value::Str("delta".into()),
            Value::Real(96.0),
            Value::Real(16.0),
        ],
    )
    .unwrap();
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Handle(b)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let attack = h.native("OrderId", &[Value::Str("alpha".into())]).unwrap();
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        attack
    );
    h.native("KillUnit", &[Value::Handle(b)]).unwrap();
    step(&mut vm, 10.0);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination,
        None
    );
}
