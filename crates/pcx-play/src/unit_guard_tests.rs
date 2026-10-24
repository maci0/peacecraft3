use super::*;
use crate::playercolor_host::GuardBudget;

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn guarded() -> (Vm, HandleId, HandleId) {
    guarded_with_limits(8., 16., 1.)
}

fn guarded_with_limits(distance: f32, maximum: f32, timeout: f32) -> (Vm, HandleId, HandleId) {
    guarded_with_flee(distance, maximum, timeout, None)
}

fn guarded_with_flee(
    distance: f32,
    maximum: f32,
    timeout: f32,
    flee: Option<(
        crate::campaign_flee_policy::FleePolicy,
        crate::script_numeric::NumericPolicy,
    )>,
) -> (Vm, HandleId, HandleId) {
    guarded_configured(distance, maximum, timeout, flee, (&[], ""), |host| host)
}

fn guarded_configured(
    distance: f32,
    maximum: f32,
    timeout: f32,
    flee: Option<(
        crate::campaign_flee_policy::FleePolicy,
        crate::script_numeric::NumericPolicy,
    )>,
    source: (&[([u8; 4], ObjectField)], &str),
    configure: impl FnOnce(ObjectHost) -> ObjectHost,
) -> (Vm, HandleId, HandleId) {
    let extra = format!(
        "{}\n{}\n{}\n{}\n{}",
        super::super::enumeration::TRIGGERS,
        super::super::activity::ORDERS,
        DECLARATIONS,
        r#"
native RemoveGuardPosition takes unit u returns nothing
native RemoveUnit takes unit u returns nothing
native SetUnitOwner takes unit u, player p, boolean color returns nothing
native UnitApplyTimedLife takes unit u, integer buffid, real duration returns nothing
type unittype extends handle
constant native ConvertUnitType takes integer n returns unittype
globals
constant unittype UNIT_TYPE_PEON=ConvertUnitType(16)
constant mapcontrol MAP_CONTROL_COMPUTER=ConvertMapControl(91)
constant mapcontrol MAP_CONTROL_USER=ConvertMapControl(-17)
endglobals
"#,
        source.1
    );
    let mut fields = armed("1", "normal");
    fields.extend(super::super::activity::gathering::source_fields(false));
    let speed = if flee.is_some() { "100" } else { "8" };
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        speed,
    ));
    if flee.is_some() {
        fields.push(column(
            b"C001",
            SlkId::UnitData,
            "canFlee",
            FieldKind::Bool,
            "1",
        ));
    }
    fields.extend_from_slice(source.0);
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
    combat.guard_distance = distance;
    combat.max_guard_distance = maximum;
    combat.guard_return_time = timeout;
    h.admit_combat(combat.clone(), "alpha").unwrap();
    h.set_neutral_aggressive(9);
    for (number, control) in [(4, 91), (5, -17)] {
        let p = h.native("Player", &[Value::Int(number)]).unwrap();
        let c = h
            .native("ConvertMapControl", &[Value::Int(control)])
            .unwrap();
        h.native("SetPlayerController", &[p, c]).unwrap();
    }
    let (policy, numeric) = match flee {
        Some((policy, numeric)) => (Some(policy), Some(numeric)),
        None => (None, None),
    };
    if let Some(numeric) = numeric {
        h = h.with_script_numeric(numeric);
    }
    h = h
        .with_unit_guards(native, combat, GuardBudget { max_units: 2 })
        .unwrap();
    let flee = policy.is_some();
    if let Some(policy) = policy {
        let context = Arc::clone(h.player_context().unwrap());
        let mut targeting = crate::campaign_targeting_policy::TargetingPolicy::fixture();
        targeting.image_sha256 = policy.image_sha256;
        h.admit_campaign_targeting(&context, targeting).unwrap();
        h.admit_campaign_flee(&context, policy).unwrap();
    }
    let h = configure(h);
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    if flee {
        vm.host().inner.activate_campaign_ai_player(4).unwrap();
    }
    let id = |v| match v {
        Value::Handle(id) => id,
        _ => panic!("unit"),
    };
    let (a, b) = (id(vm.global("A")), id(vm.global("B")));
    (vm, a, b)
}

#[path = "unit_training_tests.rs"]
mod training;
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

fn defender_fixture(quantity: i32) -> (Vm, HandleId, HandleId, bool) {
    defender_fixture_with_policy(
        quantity,
        crate::campaign_captain_policy::CaptainPolicy::fixture(),
    )
}

fn defender_fixture_with_policy(
    quantity: i32,
    policy: crate::campaign_captain_policy::CaptainPolicy,
) -> (Vm, HandleId, HandleId, bool) {
    let (mut vm, a, b) = guarded_with_limits(128., 256., 1.);
    let h = &mut vm.host().inner;
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_campaign_gathering(
        &context,
        crate::campaign_gathering_policy::GatheringPolicy::fixture(),
    )
    .unwrap();
    h.admit_campaign_captains(&context, policy).unwrap();
    h.activate_campaign_ai_player(4).unwrap();
    h.create_ai_captains(4).unwrap();
    let kind = i32::from_be_bytes(h.executed_unit(a).unwrap().definition.rawcode);
    let result = h.add_ai_defenders(4, quantity, kind).unwrap();
    (vm, a, b, result)
}

#[test]
fn defenders_keep_partial_and_negative_recruits_without_touching_attack_captain() {
    for (quantity, expected) in [(0, true), (1, true), (2, false), (-1, false)] {
        let (mut vm, a, _, result) = defender_fixture(quantity);
        let h = &mut vm.host().inner;
        assert_eq!(result, expected);
        assert!(h.fixture_captain_members(4, true).is_empty());
        let expected_members = if quantity == 0 { vec![] } else { vec![a] };
        assert_eq!(h.fixture_captain_members(4, false), expected_members);
        let kind = i32::from_be_bytes(h.executed_unit(a).unwrap().definition.rawcode);
        let before = h.executed_unit(a).unwrap().clone();
        assert_eq!(h.add_ai_defenders(4, quantity, kind).unwrap(), expected);
        assert_eq!(h.executed_unit(a).unwrap(), &before);
        if quantity != 0 {
            assert!(!h.add_ai_defenders(4, 0, kind).unwrap());
        }
        assert!(
            !h.add_ai_defenders(4, 1, i32::from_be_bytes(*b"zzzz"))
                .unwrap()
        );
        assert!(h.add_ai_defenders(5, 1, kind).is_err());
    }
}

#[test]
fn defender_routes_home_changes_combat_and_retirement_share_the_same_unit() {
    let (mut vm, a, b, success) = defender_fixture(1);
    assert!(success);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(a).unwrap().owner;
    let second = i32::from(h.campaign_captain_policy().unwrap().second_selector_mask);
    h.set_ai_captain_home(4, second, [16., 80.]).unwrap();
    assert!(h.captain_route_owned(a));
    assert_eq!(h.guard_state().unwrap().get(a).unwrap().home, [16., 80.]);
    step(&mut vm, 1.);
    assert!(vm.host().inner.executed_unit(a).unwrap().position.unwrap()[1] > 16.);
    attack(&mut vm, a, b);
    assert!(!vm.host().inner.captain_route_owned(a));
    step(&mut vm, 1.25);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.campaign_captain_in_combat(owner, true).unwrap(),
        Some(false)
    );
    assert_eq!(
        h.campaign_captain_in_combat(owner, false).unwrap(),
        Some(true)
    );
    let explicit = h.executed_unit(a).unwrap().clone();
    h.deactivate_campaign_ai_player(4).unwrap();
    assert_eq!(h.executed_unit(a).unwrap(), &explicit);
    assert_eq!(h.campaign_captain_in_combat(owner, false).unwrap(), None);

    let (mut vm, a, _, _) = defender_fixture(1);
    let h = &mut vm.host().inner;
    let second = i32::from(h.campaign_captain_policy().unwrap().second_selector_mask);
    h.set_ai_captain_home(4, second, [16., 80.]).unwrap();
    h.deactivate_campaign_ai_player(4).unwrap();
    assert!(
        !h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
}

#[test]
fn defender_aliases_are_directed_and_failures_preserve_routes_and_membership() {
    let (mut vm, a, _, _) = defender_fixture(0);
    let rawcode = vm.host().inner.executed_unit(a).unwrap().definition.rawcode;
    let mut policy = crate::campaign_captain_policy::CaptainPolicy::fixture();
    policy.defenders.alternates[0] = (*b"z001", rawcode);
    let (mut vm, a, _, _) = defender_fixture_with_policy(0, policy);
    let h = &mut vm.host().inner;
    assert!(
        h.add_ai_defenders(4, 1, i32::from_be_bytes(*b"z001"))
            .unwrap()
    );
    assert_eq!(h.fixture_captain_members(4, false), &[a]);
    h.native("RemoveUnit", &[Value::Handle(a)]).unwrap();
    assert!(h.fixture_captain_members(4, false).is_empty());

    let (mut vm, a, _, _) = defender_fixture(0);
    let h = &mut vm.host().inner;
    let kind = i32::from_be_bytes(h.executed_unit(a).unwrap().definition.rawcode);
    h.test_set_summoned(a, true);
    let before = h.executed_unit(a).unwrap().clone();
    assert!(h.add_ai_defenders(4, 1, kind).is_err());
    assert_eq!(h.executed_unit(a).unwrap(), &before);
    assert!(h.fixture_captain_members(4, false).is_empty());
}

#[test]
fn defenders_prefer_healthy_candidates_and_canonical_lifecycle_detaches_them() {
    let (mut vm, a, b, _) = defender_fixture(0);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(a).unwrap().owner;
    let kind = i32::from_be_bytes(h.executed_unit(a).unwrap().definition.rawcode);
    h.native(
        "SetUnitOwner",
        &[Value::Handle(b), Value::Handle(owner), Value::Bool(false)],
    )
    .unwrap();
    h.set_test_life(a, 1.);
    assert!(h.add_ai_defenders(4, 1, kind).unwrap());
    assert_eq!(h.fixture_captain_members(4, false), &[b]);
    let foreign = h.native("Player", &[Value::Int(5)]).unwrap();
    h.native(
        "SetUnitOwner",
        &[Value::Handle(b), foreign, Value::Bool(false)],
    )
    .unwrap();
    assert!(h.fixture_captain_members(4, false).is_empty());
    assert!(h.add_ai_defenders(4, 1, kind).unwrap());
    assert_eq!(h.fixture_captain_members(4, false), &[a]);
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    assert!(h.fixture_captain_members(4, false).is_empty());
}

#[test]
fn defenders_use_nonstock_count_steps_and_preflight_the_whole_selected_batch() {
    let mut policy = crate::campaign_captain_policy::CaptainPolicy::fixture();
    policy.defenders.count_step = -2;
    let (mut vm, a, _, result) = defender_fixture_with_policy(2, policy);
    assert!(result);
    assert_eq!(vm.host().inner.fixture_captain_members(4, false), &[a]);

    let (mut vm, a, b, _) = defender_fixture(0);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(a).unwrap().owner;
    let kind = i32::from_be_bytes(h.executed_unit(a).unwrap().definition.rawcode);
    h.native(
        "SetUnitOwner",
        &[Value::Handle(b), Value::Handle(owner), Value::Bool(false)],
    )
    .unwrap();
    h.test_set_summoned(b, true);
    let before = h.executed_unit(a).unwrap().clone();
    assert!(h.add_ai_defenders(4, 2, kind).is_err());
    assert_eq!(h.executed_unit(a).unwrap(), &before);
    assert!(h.fixture_captain_members(4, false).is_empty());
    assert!(h.add_ai_defenders(4, 1, kind).unwrap());
    assert_eq!(h.fixture_captain_members(4, false), &[a]);
}

#[test]
fn ignored_units_follow_guard_release_and_canonical_lifecycle() {
    let (mut vm, a, b) = guarded();
    let h = &mut vm.host().inner;
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_campaign_mode(
        &context,
        crate::campaign_mode_policy::CampaignModePolicy::fixture(),
    )
    .unwrap();
    h.admit_campaign_targeting(
        &context,
        crate::campaign_targeting_policy::TargetingPolicy::fixture(),
    )
    .unwrap();
    h.activate_campaign_ai_player(4).unwrap();
    let kind = i32::from_be_bytes(h.executed_unit(a).unwrap().definition.rawcode);
    let handles = h.inner().handles();
    let before = h.executed_unit(a).unwrap().clone();
    assert_eq!(h.ai_unit_count(4, kind, false).unwrap(), 1);
    assert_eq!(h.ai_ignored_units(4, kind).unwrap(), 0);
    assert_eq!(h.ai_ignored_units(4, 0).unwrap(), 0);
    assert!(h.ai_ignored_units(5, kind).is_err());
    assert_eq!(h.executed_unit(a).unwrap(), &before);
    assert_eq!(h.inner().handles(), handles);
    h.native("RemoveGuardPosition", &[Value::Handle(a)])
        .unwrap();
    assert_eq!(h.ai_ignored_units(4, kind).unwrap(), 1);
    h.native("RemoveGuardPosition", &[Value::Handle(a)])
        .unwrap();
    assert_eq!(h.ai_ignored_units(4, kind).unwrap(), 1);
    let other_owner = h.executed_unit(b).unwrap().owner;
    h.native(
        "SetUnitOwner",
        &[
            Value::Handle(a),
            Value::Handle(other_owner),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(h.ai_ignored_units(4, kind).unwrap(), 0);
    h.activate_campaign_ai_player(5).unwrap();
    assert_eq!(h.ai_ignored_units(5, kind).unwrap(), 2);
    h.kill_unit(a, None).unwrap();
    assert_eq!(h.ai_ignored_units(5, kind).unwrap(), 1);
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    assert_eq!(h.ai_ignored_units(5, kind).unwrap(), 0);
}

#[test]
fn false_timed_life_stops_then_guard_refresh_returns_home() {
    let (mut vm, a, _) = guarded();
    let h = &mut vm.host().inner;
    let context = Arc::clone(h.player_context().unwrap());
    let policy = crate::campaign_group_timed_life_policy::GroupTimedLifePolicy::fixture();
    let key = i32::from_be_bytes(policy.timed_life_factory);
    h.admit_campaign_group_timed_life(&context, policy).unwrap();
    h.activate_campaign_ai_player(4).unwrap();
    h.set_ai_group_timed_life(4, false).unwrap();
    let home = h.guard_state().unwrap().get(a).unwrap().home;
    assert!(h.steer_unit(a, Some([64., 16.]), "test").unwrap());
    step(&mut vm, 2.);
    let h = &mut vm.host().inner;
    assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), home) > 8.);
    assert!(!h.guard_state().unwrap().get(a).unwrap().returning);
    h.native(
        "UnitApplyTimedLife",
        &[Value::Handle(a), Value::Int(key), Value::Real(10.)],
    )
    .unwrap();
    assert_eq!(
        h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        None
    );
    step(&mut vm, 2.0 + 1.0 / 64.0);
    let h = &vm.host().inner;
    assert!(h.guard_state().unwrap().get(a).unwrap().returning);
    assert_eq!(
        h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        Some(home)
    );
}

#[test]
fn false_timed_life_refuses_when_combat_will_skip_the_guard() {
    for paused in [false, true] {
        let (mut vm, a, _) = guarded();
        let h = &mut vm.host().inner;
        let context = Arc::clone(h.player_context().unwrap());
        let policy = crate::campaign_group_timed_life_policy::GroupTimedLifePolicy::fixture();
        let key = i32::from_be_bytes(policy.timed_life_factory);
        h.admit_campaign_group_timed_life(&context, policy).unwrap();
        h.activate_campaign_ai_player(4).unwrap();
        h.set_ai_group_timed_life(4, false).unwrap();
        if paused {
            h.native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
                .unwrap();
        } else {
            h.test_disarm_unit(a);
        }
        let guard = *h.guard_state().unwrap().get(a).unwrap();
        let destination = h
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination();
        let refusal = h
            .native(
                "UnitApplyTimedLife",
                &[Value::Handle(a), Value::Int(key), Value::Real(1.)],
            )
            .unwrap_err();
        assert!(refusal.to_string().contains("guard combat step"));
        assert_eq!(*h.guard_state().unwrap().get(a).unwrap(), guard);
        assert_eq!(
            h.executed_unit(a)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .destination(),
            destination
        );
        h.advance_unit_expirations(1.).unwrap();
        assert!(!h.executed_unit(a).unwrap().combat.as_ref().unwrap().dead);
    }
}

#[test]
fn creep_maximum_tether_does_not_wait_for_the_chase_timeout_or_tether_other_computers() {
    for creep in [false, true] {
        let (mut vm, a, b) = guarded_with_limits(3., 7., 100.);
        if creep {
            vm.host().inner.set_neutral_aggressive(4);
        }
        let home = vm.host().inner.guard_state().unwrap().get(a).unwrap().home;
        attack(&mut vm, a, b);
        step(&mut vm, 1.);
        let h = &vm.host().inner;
        assert_eq!(h.guard_state().unwrap().get(a).unwrap().returning, creep);
        let combat = h.executed_unit(a).unwrap().combat.as_ref().unwrap();
        if creep {
            assert_eq!(
                h.executed_unit(a)
                    .unwrap()
                    .activity
                    .as_ref()
                    .unwrap()
                    .destination(),
                Some(home)
            );
            assert_eq!(combat.target, None);
        } else {
            assert_eq!(combat.target, Some(b));
            assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), home) > 7.);
        }
    }
}
#[test]
fn a_blocked_patrol_keeps_its_endpoint_and_guard_release_cancels_it() {
    let (mut vm, a, b) = guarded();
    let h = &mut vm.host().inner;
    let home = h.executed_unit(a).unwrap().position.unwrap();
    let occupied = h.executed_unit(b).unwrap().position.unwrap();
    let patrol = h.native("OrderId", &[Value::Str("delta".into())]).unwrap();
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("delta".into()),
                Value::Real(occupied[0]),
                Value::Real(occupied[1])
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 10.0);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        patrol
    );
    assert!(!h.guard_state().unwrap().get(a).unwrap().returning);
    assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), home) > 8.0);
    h.native("KillUnit", &[Value::Handle(b)]).unwrap();
    step(&mut vm, 12.0);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination,
        Some(home)
    );
    h.native("RemoveGuardPosition", &[Value::Handle(a)])
        .unwrap();
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(0)
    );
    let stopped = h.executed_unit(a).unwrap().position;
    step(&mut vm, 20.0);
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().position, stopped);
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

#[test]
fn guard_return_cancels_an_active_campaign_flee_route() {
    let flee = crate::campaign_flee_policy::tests::route_fixture();
    let (mut vm, a, b) = guarded_with_flee(3., 7., 0.2, Some(flee));
    let home = vm.host().inner.guard_state().unwrap().get(a).unwrap().home;
    let state = vm.global("UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(a), state, Value::Real(20.0)],
        )
        .unwrap();
    vm.host().inner.combat_damage_unit(b, a, 1.0).unwrap();
    assert!(vm.host().inner.campaign_flee_motion_owns(a));
    let away = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .destination()
        .unwrap();
    assert_ne!(Some(away), Some(home));
    step(&mut vm, 0.2);
    let h = &vm.host().inner;
    assert!(!h.campaign_flee_motion_owns(a));
    assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), home) <= 8.);
}

#[test]
fn guard_wait_keeps_an_active_campaign_flee_route() {
    let flee = crate::campaign_flee_policy::tests::route_fixture();
    let (mut vm, a, b) = guarded_with_flee(3., 7., 0.2, Some(flee));
    let state = vm.global("UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(a), state, Value::Real(20.0)],
        )
        .unwrap();
    vm.host().inner.combat_damage_unit(b, a, 1.0).unwrap();
    assert!(vm.host().inner.campaign_flee_motion_owns(a));
    let home = vm.host().inner.guard_state().unwrap().get(a).unwrap().home;
    let away = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .destination()
        .unwrap();
    vm.host().inner.test_set_guard_returning(a, true);
    vm.host().inner.test_advance_combat().unwrap();
    let h = &vm.host().inner;
    assert!(h.campaign_flee_motion_owns(a));
    assert_ne!(Some(away), Some(home));
    assert_eq!(
        h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        Some(away)
    );
}
