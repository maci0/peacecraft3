use super::*;
use crate::standard_command::Kind;

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
