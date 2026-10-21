use super::*;
use crate::standard_command::Kind;

#[test]
fn following_ignores_enemy_acquisition_until_a_stop_releases_the_order() {
    let mut fields = armed("64", "normal");
    fields.retain(|(id, _)| id != b"W005");
    fields.push(column(
        b"W005",
        SlkId::UnitWeapons,
        "acquire",
        FieldKind::Real,
        "200",
    ));
    fields.push(column(
        b"F001",
        SlkId::UnitBalance,
        "isbldg",
        FieldKind::Bool,
        "0",
    ));
    let (mut vm, a, b) = open(&fields);
    let life = |vm: &mut Vm, unit| vm.host().inner.unit_vitals(unit).unwrap().unwrap().life;
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(b), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("gamma".into()),
                    Value::Handle(b)
                ]
            )
            .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 10.0);
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .followed_unit(),
        Some(b)
    );
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        None
    );
    let stop = vm
        .host()
        .inner
        .standard_command_order(a, Kind::Stop)
        .unwrap()
        .unwrap();
    vm.host()
        .inner
        .native(
            "IssueImmediateOrderById",
            &[Value::Handle(a), Value::Int(stop)],
        )
        .unwrap();
    step(&mut vm, 10.5);
    assert!(life(&mut vm, b) < 20.0);
}

#[test]
fn source_bound_standard_identifiers_reach_motion_hold_stop_patrol_and_attack() {
    let (mut vm, [unit, _, enemy, _]) = holy_open("", LEARNED, NOTHING);
    let host = &mut vm.host().inner;
    let order = |host: &ObjectHost, kind| host.standard_command_order(unit, kind).unwrap().unwrap();
    let movement = order(host, Kind::Move);
    assert_eq!(
        host.native(
            "IssuePointOrderById",
            &[
                Value::Handle(unit),
                Value::Int(movement),
                Value::Real(32.),
                Value::Real(32.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    let hold = order(host, Kind::Hold);
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(unit), Value::Int(hold)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .held()
    );
    let stop = order(host, Kind::Stop);
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(unit), Value::Int(stop)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert!(
        !host
            .executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .held()
    );
    let patrol = order(host, Kind::Patrol);
    assert_eq!(
        host.native(
            "IssuePointOrderById",
            &[
                Value::Handle(unit),
                Value::Int(patrol),
                Value::Real(32.),
                Value::Real(32.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        host.native("GetUnitCurrentOrder", &[Value::Handle(unit)])
            .unwrap(),
        Value::Int(patrol)
    );
    let attack = order(host, Kind::Attack);
    assert_eq!(
        host.native(
            "IssueTargetOrderById",
            &[
                Value::Handle(unit),
                Value::Int(attack),
                Value::Handle(enemy)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(enemy)
    );
    host.native("KillUnit", &[Value::Handle(unit)]).unwrap();
    for kind in Kind::ALL {
        assert_eq!(host.standard_command_order(unit, kind).unwrap(), None);
    }
}
