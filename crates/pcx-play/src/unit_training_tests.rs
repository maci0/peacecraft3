use super::*;

const EVENTS: &str = r#"
constant native GetTrainedUnit takes nothing returns unit
constant native GetTrainedUnitType takes nothing returns integer
constant native SetPlayerTechMaxAllowed takes player p, integer kind, integer maximum returns nothing
constant native GetPlayerTechMaxAllowed takes player p, integer kind returns integer
globals
constant unitevent EVENT_UNIT_TRAIN_START=ConvertUnitEvent(78)
constant unitevent EVENT_UNIT_TRAIN_FINISH=ConvertUnitEvent(79)
constant playerunitevent EVENT_PLAYER_UNIT_TRAIN_START=ConvertPlayerUnitEvent(178)
constant playerunitevent EVENT_PLAYER_UNIT_TRAIN_FINISH=ConvertPlayerUnitEvent(179)
constant playerstate PLAYER_STATE_RESOURCE_HERO_TOKENS=ConvertPlayerState(17)
endglobals
"#;

fn profile_field(id: &[u8; 4], key: &str, declared: &str, text: &str) -> ([u8; 4], ObjectField) {
    let (id, mut field) = column(id, SlkId::UnitBalance, key, FieldKind::Token, text);
    let metadata = field.metadata.as_mut().unwrap();
    metadata.declared = declared.into();
    metadata.source = FieldSource::Profile {
        key: key.to_ascii_lowercase(),
        index: -1,
        slots: 1,
    };
    (id, field)
}

fn fixture() -> (Vm, HandleId, HandleId) {
    fixture_with(&[])
}

fn fixture_with(overrides: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    let mut fields = vec![
        profile_field(b"T001", "Trains", "unitList", "u001"),
        profile_field(b"T002", "Requires", "techList", ""),
        column(
            b"T003",
            SlkId::UnitBalance,
            "goldcost",
            FieldKind::Int,
            "41",
        ),
        column(
            b"T004",
            SlkId::UnitBalance,
            "lumbercost",
            FieldKind::Int,
            "6",
        ),
        column(b"T005", SlkId::UnitBalance, "bldtm", FieldKind::Int, "9"),
        column(b"T006", SlkId::UnitBalance, "fused", FieldKind::Int, "3"),
        column(b"T007", SlkId::UnitBalance, "fmade", FieldKind::Int, "0"),
    ];
    fields.extend_from_slice(overrides);
    for (_, field) in &mut fields {
        if let Some(metadata) = &mut field.metadata
            && metadata.kind == FieldKind::Int
        {
            metadata.declared = "int".into();
        }
    }
    let (mut vm, actor, other) = guarded_configured(8., 16., 1., None, (&fields, EVENTS), |h| {
        let mut resources = crate::player_resources::fixture::rules();
        resources.unit_cost = Ok(crate::player_unit_cost::Policy::fixture(17, b'J', b'Q', 3));
        h.with_player_state_requests(crate::playercolor_host::PlayerStateRequestBudget {
            max_records: 16,
        })
        .unwrap()
        .with_player_resources(resources)
        .unwrap()
        .with_player_tech_limits(crate::playercolor_host::TechLimitBudget { max_records: 8 })
        .unwrap()
    });
    let h = &mut vm.host().inner;
    h.native("RemoveUnit", &[Value::Handle(other)]).unwrap();
    let owner = h.executed_unit(actor).unwrap().owner;
    let Value::Handle(producer) = h
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"b001")),
                Value::Real(112.),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("producer")
    };
    h.native("RemoveGuardPosition", &[Value::Handle(producer)])
        .unwrap();
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_campaign_mode(
        &ctx,
        crate::campaign_mode_policy::CampaignModePolicy::fixture(),
    )
    .unwrap();
    h.admit_campaign_targeting(
        &ctx,
        crate::campaign_targeting_policy::TargetingPolicy::fixture(),
    )
    .unwrap();
    h.activate_campaign_ai_player(4).unwrap();
    for (name, value) in [
        ("PLAYER_STATE_RESOURCE_GOLD", 90),
        ("PLAYER_STATE_RESOURCE_LUMBER", 80),
        ("PLAYER_STATE_RESOURCE_FOOD_CAP", 40),
        ("PLAYER_STATE_RESOURCE_FOOD_USED", 6),
    ] {
        let global = vm.global(name);
        vm.host()
            .inner
            .native(
                "SetPlayerState",
                &[Value::Handle(owner), global, Value::Int(value)],
            )
            .unwrap();
    }
    (vm, actor, producer)
}

fn bank(h: &ObjectHost, owner: HandleId, name: &str) -> i32 {
    h.player_resource_value(owner, name).unwrap().unwrap()
}

fn observe_event(h: &mut ObjectHost, producer: HandleId, unit: Option<HandleId>) {
    let event = h.take_fired_event().expect("training event");
    h.begin_fired_event(&event)
        .unwrap()
        .expect("enabled trigger");
    assert_eq!(
        h.native("GetTriggerUnit", &[]).unwrap(),
        Value::Handle(producer)
    );
    assert_eq!(
        h.native("GetTrainedUnit", &[]).unwrap(),
        unit.map_or(Value::Null, Value::Handle)
    );
    assert_eq!(
        h.native("GetTrainedUnitType", &[]).unwrap(),
        Value::Int(i32::from_be_bytes(*b"u001"))
    );
    h.finish_fired_event();
    assert!(h.take_fired_event().is_none());
}

#[test]
fn guard_training_reserves_cost_food_and_counts_then_creates_and_routes_once() {
    let (mut vm, actor, producer) = fixture();
    let start = vm.global("EVENT_PLAYER_UNIT_TRAIN_START");
    let finish = vm.global("EVENT_PLAYER_UNIT_TRAIN_FINISH");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    h.fill_ai_guard_posts(4).unwrap(); // Occupied posts do not require a guessed limit.
    h.set_ai_replacement_count(4, 4).unwrap();
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    for event in [start, finish] {
        h.native(
            "TriggerRegisterPlayerUnitEvent",
            &[trigger.clone(), Value::Handle(owner), event, Value::Null],
        )
        .unwrap();
    }
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    let handles = h.inner().handles();
    let kind = i32::from_be_bytes(*b"u001");
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(h.inner().handles(), handles);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 74);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 6);
    assert_eq!(h.ai_unit_count(4, kind, false).unwrap(), 1);
    assert_eq!(h.ai_unit_count(4, kind, true).unwrap(), 0);
    assert_eq!(h.ai_player_unit_type_count(Some(owner), kind).unwrap(), 1);
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(producer)])
            .unwrap(),
        Value::Int(0)
    );
    observe_event(h, producer, None);
    for _ in 0..5 {
        h.fill_ai_guard_posts(4).unwrap();
    }
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    step(&mut vm, 8.);
    assert_eq!(vm.host().inner.ai_unit_count(4, kind, true).unwrap(), 0);
    step(&mut vm, 9.);
    let h = &mut vm.host().inner;
    let unit = h
        .executed_units()
        .find(|(id, r)| *id != actor && r.definition.rawcode == *b"u001")
        .unwrap()
        .0;
    assert_ne!(unit, actor);
    assert!(!h.executed_unit(unit).unwrap().summoned);
    assert_eq!(h.ai_unit_count(4, kind, true).unwrap(), 1);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 6);
    assert_eq!(h.guard_state().unwrap().get(unit).unwrap().home, [16., 16.]);
    assert!(
        h.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    let attack = h.native("OrderId", &[Value::Str("alpha".into())]).unwrap();
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(unit)])
            .unwrap(),
        attack
    );
    observe_event(h, producer, Some(unit));
    step(&mut vm, 30.);
    let h = &mut vm.host().inner;
    assert_eq!(h.executed_unit(unit).unwrap().position.unwrap(), [16., 16.]);
    h.native("KillUnit", &[Value::Handle(unit)]).unwrap();
    h.fill_ai_guard_posts(4).unwrap(); // Nonstock initial 3 + two bindings exceeds limit 4.
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    assert_eq!(h.ai_unit_count(4, kind, false).unwrap(), 0);
}

#[test]
fn released_posts_and_insufficient_resources_do_not_queue_or_spend() {
    for release in [false, true] {
        let (mut vm, actor, _) = fixture();
        let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
        let h = &mut vm.host().inner;
        let owner = h.executed_unit(actor).unwrap().owner;
        h.set_ai_replacement_count(4, 4).unwrap();
        if release {
            h.native("RemoveGuardPosition", &[Value::Handle(actor)])
                .unwrap();
        } else {
            h.native(
                "SetPlayerState",
                &[Value::Handle(owner), gold, Value::Int(40)],
            )
            .unwrap();
        }
        h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
        let before = bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD");
        h.fill_ai_guard_posts(4).unwrap();
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), before);
        assert_eq!(
            h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), false)
                .unwrap(),
            0
        );
        assert!(h.take_fired_event().is_none());
        step(&mut vm, 20.);
        assert_eq!(
            vm.host()
                .inner
                .ai_unit_count(4, i32::from_be_bytes(*b"u001"), false)
                .unwrap(),
            0
        );
    }
}

#[test]
fn failed_completion_keeps_the_job_and_retry_cannot_duplicate_a_unit() {
    let (mut vm, actor, _) = fixture();
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    h.set_ai_replacement_count(4, 4).unwrap();
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    h.test_set_route_work(0);
    let handles = h.inner().handles();
    h.next_timer_expiry(9., &mut 100).unwrap();
    assert!(h.advance_unit_activity(9., &mut 65536).is_err());
    assert_eq!(h.inner().handles(), handles);
    assert_eq!(
        h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), true)
            .unwrap(),
        0
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 6);
    h.test_set_route_work(65536);
    h.advance_unit_activity(9., &mut 65536).unwrap();
    let count = h.executed_units().count();
    h.advance_unit_activity(9., &mut 65536).unwrap();
    assert_eq!(h.executed_units().count(), count);
    assert_eq!(
        h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), true)
            .unwrap(),
        1
    );
}

#[test]
fn source_release_counter_is_a_signed_limit_and_food_shortage_is_not_a_free_unit() {
    let (mut vm, actor, _) = fixture();
    let cap = vm.global("PLAYER_STATE_RESOURCE_FOOD_CAP");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    h.set_ai_replacement_count(4, 909).unwrap();
    h.native("RemoveGuardPosition", &[Value::Handle(actor)])
        .unwrap();
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    h.set_ai_replacement_count(4, 910).unwrap();
    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), cap.clone(), Value::Int(3)],
    )
    .unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), cap, Value::Int(6)],
    )
    .unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    step(&mut vm, 9.);
    assert_eq!(
        vm.host()
            .inner
            .ai_unit_count(4, i32::from_be_bytes(*b"u001"), true)
            .unwrap(),
        1
    );
}

#[test]
fn pausing_a_producer_retains_reserved_food_and_remaining_training_time() {
    let (mut vm, actor, producer) = fixture();
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    h.set_ai_replacement_count(4, 4).unwrap();
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    step(&mut vm, 4.);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(producer), Value::Bool(true)])
        .unwrap();
    step(&mut vm, 9.);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), true)
            .unwrap(),
        0
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 6);
    assert!(h.next_training_completion(100.).is_none());
    h.native("PauseUnit", &[Value::Handle(producer), Value::Bool(false)])
        .unwrap();
    assert_eq!(h.next_training_completion(100.), Some(14.));
    step(&mut vm, 14.);
    assert_eq!(
        vm.host()
            .inner
            .ai_unit_count(4, i32::from_be_bytes(*b"u001"), true)
            .unwrap(),
        1
    );
}

#[test]
fn production_finishes_even_when_the_trained_unit_cannot_start_motion() {
    let speed = column(b"S001", SlkId::UnitBalance, "spd", FieldKind::Real, "0");
    let (mut vm, actor, _) = fixture_with(&[speed]);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    h.set_ai_replacement_count(4, 4).unwrap();
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    step(&mut vm, 9.);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), true)
            .unwrap(),
        1
    );
    let (new, record) = h
        .executed_units()
        .find(|(id, r)| *id != actor && r.definition.rawcode == *b"u001")
        .unwrap();
    assert!(!record.activity.as_ref().unwrap().moving());
    assert_eq!(h.guard_state().unwrap().get(new).unwrap().home, [16., 16.]);
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
}

#[test]
fn authored_unit_limits_prevent_requests_before_any_resource_debit() {
    let (mut vm, actor, _) = fixture();
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let kind = i32::from_be_bytes(*b"u001");
    h.set_ai_replacement_count(4, 4).unwrap();
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    h.native(
        "SetPlayerTechMaxAllowed",
        &[Value::Handle(owner), Value::Int(kind), Value::Int(0)],
    )
    .unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    assert_eq!(h.ai_unit_count(4, kind, false).unwrap(), 0);
    h.native(
        "SetPlayerTechMaxAllowed",
        &[Value::Handle(owner), Value::Int(kind), Value::Int(1)],
    )
    .unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    assert_eq!(h.ai_unit_count(4, kind, false).unwrap(), 1);
    step(&mut vm, 9.);
    assert_eq!(vm.host().inner.ai_unit_count(4, kind, true).unwrap(), 1);
}

#[test]
fn malformed_train_profiles_and_unsupported_prerequisites_preserve_the_world() {
    let mut foreign = profile_field(b"T001", "Trains", "unitList", "u001");
    foreign.1.metadata.as_mut().unwrap().from = SlkId::ItemData;
    let mut indexed = profile_field(b"T001", "Trains", "unitList", "u001");
    indexed.1.metadata.as_mut().unwrap().index = 0;
    let mut malformed = profile_field(b"T001", "Trains", "unitList", "u001");
    malformed.1.value = Ok(FieldValue::Override(pcx_objects::Value::Str(vec![0xff])));
    for replacement in [
        foreign,
        indexed,
        malformed,
        profile_field(b"T001", "Trains", "abilityList", "u001"),
        profile_field(b"T002", "Requires", "techList", "R001"),
    ] {
        let (mut vm, actor, _) = fixture_with(&[replacement]);
        let h = &mut vm.host().inner;
        let owner = h.executed_unit(actor).unwrap().owner;
        h.set_ai_replacement_count(4, 4).unwrap();
        h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
        let handles = h.inner().handles();
        assert!(h.fill_ai_guard_posts(4).is_err());
        assert_eq!(h.inner().handles(), handles);
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 3);
        assert_eq!(
            h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), false)
                .unwrap(),
            0
        );
        assert!(h.take_fired_event().is_none());
    }
}

#[test]
fn training_deadline_does_not_drift_with_timer_partition() {
    for divided in [false, true] {
        let (mut vm, actor, _) = fixture();
        let h = &mut vm.host().inner;
        h.set_ai_replacement_count(4, 4).unwrap();
        h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
        h.fill_ai_guard_posts(4).unwrap();
        let steps = if divided { 90 } else { 1 };
        for step in 1..=steps {
            let now = 9. * f64::from(step) / f64::from(steps);
            h.next_timer_expiry(now, &mut 100).unwrap();
            h.advance_unit_activity(now, &mut 65536).unwrap();
        }
        assert_eq!(
            h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), true)
                .unwrap(),
            1
        );
        assert!(h.next_training_completion(10.).is_none());
    }
}
