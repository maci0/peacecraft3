use super::*;
use crate::playercolor_host::GuardBudget;

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn guarded() -> (Vm, HandleId, HandleId) {
    let extra = format!(
        "{}\n{}\n{}\n{}",
        super::super::enumeration::TRIGGERS,
        super::super::activity::ORDERS,
        DECLARATIONS,
        r#"
native RemoveGuardPosition takes unit u returns nothing
native RemoveUnit takes unit u returns nothing
native SetUnitOwner takes unit u, player p, boolean color returns nothing
globals
constant mapcontrol MAP_CONTROL_COMPUTER=ConvertMapControl(91)
constant mapcontrol MAP_CONTROL_USER=ConvertMapControl(-17)
endglobals
"#
    );
    let mut fields = armed("1", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "8",
    ));
    let mut h = position_host_admitting(1000, 16, MAP, &extra, &fields, |h| h);
    let (registry, activity) = super::super::activity::rules();
    let mut native = crate::unit_guard_rules::tests::fixture(registry.id("beta").unwrap());
    native.image_sha256 = registry.binding().image_sha256;
    h = h
        .with_locations()
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
        .unwrap()
        .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 8 })
        .unwrap()
        .with_order_registry(registry)
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.,
        mana_per_intelligence: 15.,
        life_regen_per_strength: 0.,
        mana_regen_per_intelligence: 0.,
    })
    .unwrap();
    let mut combat = rules();
    combat.guard_distance = 8.;
    combat.max_guard_distance = 16.;
    combat.guard_return_time = 1.;
    h.admit_combat(combat.clone(), "alpha").unwrap();
    for (number, control) in [(4, 91), (5, -17)] {
        let p = h.native("Player", &[Value::Int(number)]).unwrap();
        let c = h
            .native("ConvertMapControl", &[Value::Int(control)])
            .unwrap();
        h.native("SetPlayerController", &[p, c]).unwrap();
    }
    h = h
        .with_unit_guards(native, combat, GuardBudget { max_units: 2 })
        .unwrap();
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = |v| match v {
        Value::Handle(id) => id,
        _ => panic!("unit"),
    };
    let (a, b) = (id(vm.global("A")), id(vm.global("B")));
    (vm, a, b)
}
fn attack(vm: &mut Vm, a: HandleId, b: HandleId) {
    assert_eq!(
        vm.host()
            .inner
            .native(
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
}
#[test]
fn managed_creation_returns_home_but_release_allows_the_authored_pursuit() {
    let (mut vm, a, b) = guarded();
    let home = vm.host().inner.guard_state().unwrap().get(a).unwrap().home;
    assert_eq!(
        home,
        vm.host().inner.executed_unit(a).unwrap().position.unwrap()
    );
    assert!(
        vm.host().inner.guard_state().unwrap().get(b).is_none(),
        "user-owned unit is not computer guard"
    );
    attack(&mut vm, a, b);
    step(&mut vm, 4.);
    let h = &mut vm.host().inner;
    let definition = &h.executed_unit(a).unwrap().definition;
    let FieldValue::Cell(collision) = definition.fields[b"G001"].value.as_ref().unwrap() else {
        panic!("synthetic collision source cell");
    };
    let collision = collision.parse::<f32>().unwrap();
    assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), home) <= collision);
    h.native("RemoveGuardPosition", &[Value::Handle(a)])
        .unwrap();
    assert!(h.guard_state().unwrap().get(a).unwrap().released);
    attack(&mut vm, a, b);
    step(&mut vm, 7.);
    let h = &mut vm.host().inner;
    assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), home) > 8.);
    assert_eq!(
        h.executed_unit(a).unwrap().combat.as_ref().unwrap().target,
        Some(b)
    );
}
#[test]
fn release_cancels_current_guard_ai_order_and_repeated_release_stops_new_order() {
    let (mut vm, a, b) = guarded();
    attack(&mut vm, a, b);
    step(&mut vm, 0.5);
    let h = &mut vm.host().inner;
    assert!(
        h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    h.native("RemoveGuardPosition", &[Value::Handle(a)])
        .unwrap();
    assert!(
        !h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    assert!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target
            .is_none()
    );
    attack(&mut vm, a, b);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.executed_unit(a).unwrap().combat.as_ref().unwrap().target,
        Some(b)
    );
    h.native("RemoveGuardPosition", &[Value::Handle(a)])
        .unwrap();
    assert!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target
            .is_none()
    );
    assert!(h.guard_state().unwrap().get(a).unwrap().released);
}
#[test]
fn ownership_reenrolls_at_current_position_and_removal_releases_record() {
    let (mut vm, a, b) = guarded();
    attack(&mut vm, a, b);
    step(&mut vm, 2.5);
    let h = &mut vm.host().inner;
    let user = h.native("Player", &[Value::Int(5)]).unwrap();
    let cpu = h.native("Player", &[Value::Int(4)]).unwrap();
    h.native(
        "SetUnitOwner",
        &[Value::Handle(a), user, Value::Bool(false)],
    )
    .unwrap();
    assert!(h.guard_state().unwrap().get(a).is_none());
    assert!(
        !h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving(),
        "ownership cancels old guard-owned home path"
    );
    let position = h.executed_unit(a).unwrap().position.unwrap();
    h.native("SetUnitOwner", &[Value::Handle(a), cpu, Value::Bool(false)])
        .unwrap();
    assert_eq!(h.guard_state().unwrap().get(a).unwrap().home, position);
    h.native("RemoveUnit", &[Value::Handle(a)]).unwrap();
    assert!(h.guard_state().unwrap().get(a).is_none());
}
#[test]
fn invalid_guard_calls_leave_current_activity_and_membership_unchanged() {
    let (mut vm, a, b) = guarded();
    attack(&mut vm, a, b);
    step(&mut vm, 0.5);
    let h = &mut vm.host().inner;
    let guard = *h.guard_state().unwrap().get(a).unwrap();
    let activity = h.executed_unit(a).unwrap().activity.clone();
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    for args in [
        vec![],
        vec![Value::Int(1)],
        vec![player],
        vec![Value::Handle(a), Value::Bool(true)],
    ] {
        assert!(h.native("RemoveGuardPosition", &args).is_err());
    }
    assert_eq!(*h.guard_state().unwrap().get(a).unwrap(), guard);
    assert_eq!(h.executed_unit(a).unwrap().activity, activity);
    h.native("RemoveGuardPosition", &[Value::Null]).unwrap();
    assert_eq!(*h.guard_state().unwrap().get(a).unwrap(), guard);
}
