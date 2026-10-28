use super::*;

const EVENTS: &str = r#"
constant native GetPlayerStructureCount takes player p, boolean includeIncomplete returns integer
constant native GetConstructingStructure takes nothing returns unit
constant native GetConstructedStructure takes nothing returns unit
constant native GetCancelledStructure takes nothing returns unit
constant native GetTrainedUnit takes nothing returns unit
constant native GetTrainedUnitType takes nothing returns integer
constant native SetPlayerTechMaxAllowed takes player p, integer kind, integer maximum returns nothing
constant native GetPlayerTechMaxAllowed takes player p, integer kind returns integer
constant native SetPlayerTechResearched takes player p, integer kind, integer level returns nothing
constant native GetPlayerTechResearched takes player p, integer kind, boolean specific returns boolean
constant native GetPlayerTechCount takes player p, integer kind, boolean specific returns integer
globals
constant unitevent EVENT_UNIT_CHANGE_OWNER=ConvertUnitEvent(266)
constant playerunitevent EVENT_PLAYER_UNIT_CHANGE_OWNER=ConvertPlayerUnitEvent(267)
constant real bj_UNIT_FACING=0.0
constant playerunitevent EVENT_PLAYER_UNIT_CONSTRUCT_START=ConvertPlayerUnitEvent(226)
constant playerunitevent EVENT_PLAYER_UNIT_CONSTRUCT_CANCEL=ConvertPlayerUnitEvent(227)
constant playerunitevent EVENT_PLAYER_UNIT_CONSTRUCT_FINISH=ConvertPlayerUnitEvent(228)
constant unitevent EVENT_UNIT_CONSTRUCT_CANCEL=ConvertUnitEvent(164)
constant unitevent EVENT_UNIT_CONSTRUCT_FINISH=ConvertUnitEvent(165)
constant unitevent EVENT_UNIT_TRAIN_START=ConvertUnitEvent(78)
constant unitevent EVENT_UNIT_TRAIN_FINISH=ConvertUnitEvent(79)
constant unitevent EVENT_UNIT_TRAIN_CANCEL=ConvertUnitEvent(80)
constant playerunitevent EVENT_PLAYER_UNIT_TRAIN_START=ConvertPlayerUnitEvent(178)
constant playerunitevent EVENT_PLAYER_UNIT_TRAIN_FINISH=ConvertPlayerUnitEvent(179)
constant playerunitevent EVENT_PLAYER_UNIT_TRAIN_CANCEL=ConvertPlayerUnitEvent(180)
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
    fixture_with_capacity(overrides, 4)
}

fn fixture_with_capacity(
    overrides: &[([u8; 4], ObjectField)],
    capacity: usize,
) -> (Vm, HandleId, HandleId) {
    fixture_with_training(
        overrides,
        capacity,
        0.37,
        true,
        super::super::super::activity::rules(),
        false,
        EVENTS,
    )
}

fn fixture_with_training(
    overrides: &[([u8; 4], ObjectField)],
    capacity: usize,
    refund_rate: f32,
    admit_numeric: bool,
    orders: (
        std::sync::Arc<crate::order_registry::Registry>,
        crate::unit_activity::Rules,
    ),
    heroes: bool,
    script: &str,
) -> (Vm, HandleId, HandleId) {
    fixture_with_training_configured(
        overrides,
        capacity,
        refund_rate,
        admit_numeric,
        orders,
        heroes,
        (script, |host| host),
    )
}

fn fixture_with_training_configured(
    overrides: &[([u8; 4], ObjectField)],
    capacity: usize,
    refund_rate: f32,
    admit_numeric: bool,
    orders: (
        std::sync::Arc<crate::order_registry::Registry>,
        crate::unit_activity::Rules,
    ),
    heroes: bool,
    source: (&str, impl FnOnce(ObjectHost) -> ObjectHost),
) -> (Vm, HandleId, HandleId) {
    let (script, configure) = source;
    let mut fields = vec![
        profile_field(b"T001", "Trains", "unitList", "u001"),
        profile_field(b"T002", "Requires", "techList", ""),
        requirement_amount(""),
        profile_field(b"T009", "Upgrade", "unitList", ""),
        profile_field(b"T010", "DependencyOr", "unitList", ""),
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
    let (mut vm, actor, other) = guarded_configured_with_orders(
        (8., 16., 1.),
        None,
        (&fields, script),
        capacity,
        orders,
        |h| {
            let mut resources = crate::player_resources::fixture::rules();
            let (image, binding, budget) = crate::script_numeric::tests::fixture();
            let numeric =
                crate::script_numeric::NumericPolicy::read(&image, binding, budget, 64).unwrap();
            resources.image_sha256 = *numeric.image_sha256();
            let mut training = crate::training_rules::Rules::fixture(3, refund_rate);
            training.image_sha256 = resources.image_sha256;
            training.ai_production = Ok(crate::campaign_production_rules::Rules::fixture(
                resources.image_sha256,
                5,
            ));
            resources.training = Ok(training);
            resources.unit_cost = Ok(crate::player_unit_cost::Policy::fixture(17, b'J', b'Q', 3));
            let construction = crate::construction_rules::Rules {
                origin: resources.origin.clone(),
                common_origin: resources.origin.clone(),
                image_sha256: resources.image_sha256,
                common_sha256: [7; 32],
                profile_sources: vec!["synthetic construction".into()],
                initial_life_fraction: 0.23,
                human_build_ability: *b"ZYX9",
                refund_rate: 0.37,
                damage_refund_penalty: true,
                life_drain_rate: 4.25,
            };
            resources = resources.with_construction_rules(Ok(construction));
            let h = if admit_numeric {
                h.with_script_numeric(numeric)
            } else {
                h
            };
            let h = h
                .with_player_state_requests(crate::playercolor_host::PlayerStateRequestBudget {
                    max_records: 16,
                })
                .unwrap()
                .with_player_resources(resources)
                .unwrap()
                .with_player_tech_limits(crate::playercolor_host::TechLimitBudget {
                    max_records: 8,
                })
                .unwrap()
                .with_player_tech_researched(crate::playercolor_host::TechLimitBudget {
                    max_records: 8,
                })
                .unwrap();
            configure(if heroes { admit_fixture_heroes(h) } else { h })
        },
    );
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

fn requirement_amount(text: &str) -> ([u8; 4], ObjectField) {
    let mut field = profile_field(b"T008", "Requiresamount", "intList", text);
    field.1.metadata.as_mut().unwrap().kind = FieldKind::IntList;
    field
}

#[test]
fn unit_technology_queries_follow_owner_liveness_and_removal_retirement() {
    let (mut vm, actor, producer) = fixture();
    step(&mut vm, 1.);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
        .unwrap();
    let count = |h: &mut ObjectHost, owner, code| {
        h.native(
            "GetPlayerTechCount",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(code)),
                Value::Bool(false),
            ],
        )
        .unwrap()
    };
    assert_eq!(count(h, owner, *b"u001"), Value::Int(1));
    h.native("ShowUnit", &[Value::Handle(actor), Value::Bool(false)])
        .unwrap();
    h.native("PauseUnit", &[Value::Handle(actor), Value::Bool(true)])
        .unwrap();
    assert_eq!(count(h, owner, *b"u001"), Value::Int(1));
    let Value::Handle(other) = h.native("Player", &[Value::Int(5)]).unwrap() else {
        panic!("owner")
    };
    h.native(
        "SetUnitOwner",
        &[
            Value::Handle(actor),
            Value::Handle(other),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(count(h, owner, *b"u001"), Value::Int(0));
    assert_eq!(count(h, other, *b"u001"), Value::Int(1));
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    assert_eq!(count(h, other, *b"u001"), Value::Int(0));
    h.native("RemoveUnit", &[Value::Handle(producer)]).unwrap();
    assert_eq!(count(h, owner, *b"b001"), Value::Int(1));
    h.native("RemoveUnit", &[Value::Handle(producer)]).unwrap();
    assert_eq!(count(h, owner, *b"b001"), Value::Int(1));
    step(&mut vm, 2.);
    assert_eq!(count(&mut vm.host().inner, owner, *b"b001"), Value::Int(0));
}

#[test]
fn unit_technology_relationships_reach_public_queries_on_the_canonical_arena() {
    let (mut vm, actor, _) = fixture_with(&[
        profile_field(b"T009", "Upgrade", "unitList", "d001"),
        profile_field(b"T010", "DependencyOr", "unitList", "e001"),
    ]);
    step(&mut vm, 1.);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
        .unwrap();
    for (code, x) in [(*b"d001", 64.), (*b"e001", 80.)] {
        h.native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(code)),
                Value::Real(x),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap();
    }
    for (specific, expected) in [(false, 3), (true, 2)] {
        let args = [
            Value::Handle(owner),
            Value::Int(i32::from_be_bytes(*b"u001")),
            Value::Bool(specific),
        ];
        assert_eq!(
            h.native("GetPlayerTechCount", &args).unwrap(),
            Value::Int(expected)
        );
        assert_eq!(
            h.native("GetPlayerTechResearched", &args).unwrap(),
            Value::Bool(true)
        );
    }
}

#[test]
fn unit_prerequisites_count_canonical_providers_before_spending() {
    let (mut vm, actor, _) = fixture_with(&[
        profile_field(b"T002", "Requires", "techList", "d001"),
        requirement_amount("2"),
    ]);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
        .unwrap();
    h.set_ai_replacement_count(4, 4).unwrap();
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    for x in [64., 80.] {
        h.fill_ai_guard_posts(4).unwrap();
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
        h.native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"d001")),
                Value::Real(x),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap();
    }
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    assert_eq!(
        h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), false)
            .unwrap(),
        1
    );
}

#[test]
fn upgrade_prerequisite_waits_without_spending_then_trains_after_research() {
    for amount in ["", "1"] {
        let (mut vm, actor, producer) = fixture_with(&[
            profile_field(b"T002", "Requires", "techList", "Qtec"),
            requirement_amount(amount),
        ]);
        let h = &mut vm.host().inner;
        let owner = h.executed_unit(actor).unwrap().owner;
        let ctx = Arc::clone(h.player_context().unwrap());
        h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
            .unwrap();
        h.set_ai_replacement_count(4, 4).unwrap();
        h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
        for _ in 0..2 {
            h.fill_ai_guard_posts(4).unwrap();
            assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
            assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 3);
            assert!(h.next_training_completion(100.).is_none());
            assert!(h.take_fired_event().is_none());
        }
        h.native(
            "SetPlayerTechResearched",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"Qtec")),
                Value::Int(1),
            ],
        )
        .unwrap();
        let trigger = h.native("CreateTrigger", &[]).unwrap();
        let event = vm.global("EVENT_PLAYER_UNIT_TRAIN_FINISH");
        let h = &mut vm.host().inner;
        h.native(
            "TriggerRegisterPlayerUnitEvent",
            &[trigger, Value::Handle(owner), event, Value::Null],
        )
        .unwrap();
        h.fill_ai_guard_posts(4).unwrap();
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 6);
        step(&mut vm, 9.);
        let h = &mut vm.host().inner;
        assert_eq!(
            h.ai_unit_count(4, i32::from_be_bytes(*b"u001"), true)
                .unwrap(),
            1
        );
        let unit = h
            .executed_units()
            .find(|(_, r)| {
                r.definition.rawcode == *b"u001" && r.combat.as_ref().is_some_and(|c| !c.dead)
            })
            .map(|(id, _)| id)
            .unwrap();
        observe_event(h, producer, Some(unit));
        assert!(h.take_fired_event().is_none());
    }
}

#[test]
fn malformed_or_unqualified_requirements_preserve_resources_and_pending_jobs() {
    for (requires, amount) in [
        ("Qtec", "0"),
        ("Qtec", "-1"),
        ("Qtec", "1.5"),
        ("Qtec", "2147483648"),
        ("Qtec", "1,2"),
        ("Qtec,Qtec", "1"),
        ("Qtec,", ""),
        ("Qte", ""),
        ("Qtec,Rbadextra", "1,1"),
    ] {
        let (mut vm, actor, _) = fixture_with(&[
            profile_field(b"T002", "Requires", "techList", requires),
            requirement_amount(amount),
        ]);
        let h = &mut vm.host().inner;
        let ctx = Arc::clone(h.player_context().unwrap());
        h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
            .unwrap();
        let owner = h.executed_unit(actor).unwrap().owner;
        h.set_ai_replacement_count(4, 4).unwrap();
        h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
        assert!(h.fill_ai_guard_posts(4).is_err(), "{requires}/{amount}");
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 3);
        assert!(h.next_training_completion(100.).is_none());
        assert!(h.take_fired_event().is_none());
    }
}

#[test]
fn prerequisite_uses_its_declared_amount_and_the_actual_producer_owner() {
    for (amount, researcher) in [("1", 5), ("2", 4)] {
        let (mut vm, actor, _) = fixture_with(&[
            profile_field(b"T002", "Requires", "techList", "Qtec"),
            requirement_amount(amount),
        ]);
        let h = &mut vm.host().inner;
        let ctx = Arc::clone(h.player_context().unwrap());
        h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
            .unwrap();
        let owner = h.executed_unit(actor).unwrap().owner;
        let researcher = h.native("Player", &[Value::Int(researcher)]).unwrap();
        h.native(
            "SetPlayerTechResearched",
            &[
                researcher,
                Value::Int(i32::from_be_bytes(*b"Qtec")),
                Value::Int(1),
            ],
        )
        .unwrap();
        h.set_ai_replacement_count(4, 4).unwrap();
        h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
        h.fill_ai_guard_posts(4).unwrap();
        assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
        assert!(h.next_training_completion(100.).is_none());
        assert!(h.take_fired_event().is_none());
    }
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

#[test]
fn player_orders_share_a_paid_fifo_with_one_active_food_reservation() {
    let (mut vm, actor, producer) = fixture_with_capacity(
        &[column(
            b"T003",
            SlkId::UnitBalance,
            "goldcost",
            FieldKind::Int,
            "13",
        )],
        8,
    );
    let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
    let start = vm.global("EVENT_PLAYER_UNIT_TRAIN_START");
    let finish = vm.global("EVENT_PLAYER_UNIT_TRAIN_FINISH");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
        .unwrap();

    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), gold, Value::Int(90)],
    )
    .unwrap();
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    for event in [start, finish] {
        h.native(
            "TriggerRegisterPlayerUnitEvent",
            &[trigger.clone(), Value::Handle(owner), event, Value::Null],
        )
        .unwrap();
    }
    let order = [
        Value::Handle(producer),
        Value::Int(i32::from_be_bytes(*b"u001")),
    ];
    for _ in 0..3 {
        assert_eq!(
            h.native("IssueImmediateOrderById", &order).unwrap(),
            Value::Bool(true)
        );
    }
    assert_eq!(
        h.native("IssueImmediateOrderById", &order).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 51);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 62);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 9);
    assert_eq!(h.unit_training_queue(producer).unwrap().len(), 3);
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(producer)])
            .unwrap(),
        Value::Int(0)
    );
    observe_event(h, producer, None);
    assert_eq!(
        h.native(
            "GetPlayerTechCount",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Bool(false)
            ]
        )
        .unwrap(),
        Value::Int(1)
    );
    step(&mut vm, 9.);
    let h = &mut vm.host().inner;
    assert_eq!(h.unit_training_queue(producer).unwrap().len(), 2);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 12);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 51);
    let mut finished = 0;
    let mut started = 0;
    while let Some(event) = h.take_fired_event() {
        assert_eq!(event.subject.unit, Some(producer));
        assert_eq!(
            event.subject.trained_type,
            Some(i32::from_be_bytes(*b"u001"))
        );
        if event.subject.trained.is_some() {
            finished += 1;
        } else {
            started += 1;
        }
    }
    assert_eq!((finished, started), (1, 1));
    step(&mut vm, 18.);
    step(&mut vm, 27.);
    let h = &mut vm.host().inner;
    assert_eq!(h.unit_training_queue(producer).unwrap().len(), 0);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 15);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 51);
}

#[test]
fn paid_pending_head_waits_for_food_without_another_money_debit() {
    let (mut vm, actor, producer) = fixture_with_capacity(
        &[column(
            b"T003",
            SlkId::UnitBalance,
            "goldcost",
            FieldKind::Int,
            "13",
        )],
        8,
    );
    let cap = vm.global("PLAYER_STATE_RESOURCE_FOOD_CAP");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let order = [
        Value::Handle(producer),
        Value::Int(i32::from_be_bytes(*b"u001")),
    ];
    for _ in 0..2 {
        assert_eq!(
            h.native("IssueImmediateOrderById", &order).unwrap(),
            Value::Bool(true)
        );
    }
    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), cap.clone(), Value::Int(9)],
    )
    .unwrap();
    step(&mut vm, 9.);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.unit_training_queue(producer).unwrap(),
        vec![(i32::from_be_bytes(*b"u001"), None)]
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 9);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 64);
    step(&mut vm, 18.);
    let h = &mut vm.host().inner;
    assert_eq!(h.unit_training_queue(producer).unwrap()[0].1, None);
    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), cap, Value::Int(40)],
    )
    .unwrap();
    step(&mut vm, 19.);
    let h = &mut vm.host().inner;
    assert!(h.unit_training_queue(producer).unwrap()[0].1.is_some());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 12);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 64);
    step(&mut vm, 28.);
    let h = &mut vm.host().inner;
    assert!(h.unit_training_queue(producer).unwrap().is_empty());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 12);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 64);
}

#[test]
fn cancelling_pending_and_active_entries_refunds_once_and_preserves_the_head_rules() {
    let (mut vm, actor, producer) = fixture_with_capacity(
        &[column(
            b"T003",
            SlkId::UnitBalance,
            "goldcost",
            FieldKind::Int,
            "13",
        )],
        8,
    );
    let start = vm.global("EVENT_PLAYER_UNIT_TRAIN_START");
    let cancel = vm.global("EVENT_PLAYER_UNIT_TRAIN_CANCEL");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    for event in [start, cancel] {
        h.native(
            "TriggerRegisterPlayerUnitEvent",
            &[trigger.clone(), Value::Handle(owner), event, Value::Null],
        )
        .unwrap();
    }
    for _ in 0..3 {
        assert_eq!(
            h.native(
                "IssueImmediateOrderById",
                &[
                    Value::Handle(producer),
                    Value::Int(i32::from_be_bytes(*b"u001"))
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    observe_event(h, producer, None);
    let original_head = h.unit_training_queue(producer).unwrap()[0];
    assert!(!h.cancel_unit_training(producer, 3).unwrap());
    assert!(h.cancel_unit_training(producer, 2).unwrap());
    assert_eq!(h.unit_training_queue(producer).unwrap()[0], original_head);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 55);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 64);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 9);
    observe_event(h, producer, None);
    step(&mut vm, 2.);
    let h = &mut vm.host().inner;
    assert!(h.cancel_unit_training(producer, 0).unwrap());
    assert_eq!(h.unit_training_queue(producer).unwrap()[0].1, Some(11.));
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 59);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 66);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 9);
    let mut events = 0;
    while let Some(event) = h.take_fired_event() {
        assert_eq!(event.subject.unit, Some(producer));
        assert_eq!(
            event.subject.trained_type,
            Some(i32::from_be_bytes(*b"u001"))
        );
        events += 1;
    }
    assert_eq!(events, 2);
    step(&mut vm, 11.);
    let h = &mut vm.host().inner;
    assert!(h.unit_training_queue(producer).unwrap().is_empty());
    assert!(!h.cancel_unit_training(producer, 0).unwrap());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 59);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 9);
}

#[test]
fn producer_death_refunds_each_entry_and_releases_food_without_cancel_events() {
    let (mut vm, actor, producer) = fixture_with_capacity(
        &[column(
            b"T003",
            SlkId::UnitBalance,
            "goldcost",
            FieldKind::Int,
            "13",
        )],
        8,
    );
    let cancel = vm.global("EVENT_PLAYER_UNIT_TRAIN_CANCEL");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[trigger, Value::Handle(owner), cancel, Value::Null],
    )
    .unwrap();
    for _ in 0..3 {
        assert_eq!(
            h.native(
                "IssueImmediateOrderById",
                &[
                    Value::Handle(producer),
                    Value::Int(i32::from_be_bytes(*b"u001"))
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    h.native("KillUnit", &[Value::Handle(producer)]).unwrap();
    assert!(h.unit_training_queue(producer).unwrap().is_empty());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 63);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 68);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 3);
    assert!(h.take_fired_event().is_none());
    h.native("KillUnit", &[Value::Handle(producer)]).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 63);
    step(&mut vm, 30.);
    assert_eq!(
        bank(&vm.host().inner, owner, "PLAYER_STATE_RESOURCE_GOLD"),
        63
    );
}

#[test]
fn missing_refund_math_refuses_cancel_and_death_before_world_changes() {
    let (mut vm, actor, producer) = fixture_with_training(
        &[],
        8,
        0.37,
        false,
        super::super::super::activity::rules(),
        false,
        EVENTS,
    );
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    assert_eq!(
        h.native(
            "IssueImmediateOrderById",
            &[
                Value::Handle(producer),
                Value::Int(i32::from_be_bytes(*b"u001"))
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let queue = h.unit_training_queue(producer).unwrap();
    let life = h.executed_unit(producer).unwrap().vitals.unwrap().life;
    assert!(
        h.cancel_unit_training(producer, 0)
            .unwrap_err()
            .to_string()
            .contains("numeric")
    );
    assert!(
        h.native("KillUnit", &[Value::Handle(producer)])
            .unwrap_err()
            .contains("numeric")
    );
    assert_eq!(h.unit_training_queue(producer).unwrap(), queue);
    assert_eq!(
        h.executed_unit(producer).unwrap().vitals.unwrap().life,
        life
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 9);
}

#[test]
fn cancelling_head_with_blocked_food_leaves_the_paid_successor_pending() {
    let (mut vm, actor, producer) = fixture_with_capacity(
        &[column(
            b"T003",
            SlkId::UnitBalance,
            "goldcost",
            FieldKind::Int,
            "13",
        )],
        8,
    );
    let cap = vm.global("PLAYER_STATE_RESOURCE_FOOD_CAP");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    for _ in 0..2 {
        assert_eq!(
            h.native(
                "IssueImmediateOrderById",
                &[
                    Value::Handle(producer),
                    Value::Int(i32::from_be_bytes(*b"u001"))
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), cap, Value::Int(6)],
    )
    .unwrap();
    assert!(h.cancel_unit_training(producer, 0).unwrap());
    assert_eq!(
        h.unit_training_queue(producer).unwrap(),
        vec![(i32::from_be_bytes(*b"u001"), None)]
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 6);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 68);
    step(&mut vm, 12.);
    assert_eq!(
        vm.host().inner.unit_training_queue(producer).unwrap()[0].1,
        None
    );
}

#[test]
fn source_classified_hero_training_refuses_before_payment_or_queue_changes() {
    let (mut vm, actor, producer) =
        fixture_with(&[profile_field(b"T001", "Trains", "unitList", "J001")]);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let error = h
        .native(
            "IssueImmediateOrderById",
            &[
                Value::Handle(producer),
                Value::Int(i32::from_be_bytes(*b"J001")),
            ],
        )
        .unwrap_err();
    assert!(error.contains("hero training"), "{error}");
    assert!(h.unit_training_queue(producer).unwrap().is_empty());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 6);
}

#[test]
fn a_trained_unit_moves_to_the_producer_rally() {
    let (mut vm, actor, producer) = fixture_with_training(
        &[],
        4,
        0.37,
        true,
        super::super::super::activity::rules_with_setrally(),
        false,
        EVENTS,
    );
    let rally = [48., 16.];
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(producer),
                Value::Str("setrally".into()),
                Value::Real(rally[0]),
                Value::Real(rally[1]),
            ],
        )
        .unwrap(),
        Value::Bool(true)
    );
    h.fill_ai_guard_posts(4).unwrap();
    h.set_ai_replacement_count(4, 4).unwrap();
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    // Fixture build time is 9. One second later the trainee's route is the rally.
    step(&mut vm, 10.);
    let h = &mut vm.host().inner;
    let unit = h
        .executed_units()
        .find(|(id, record)| *id != actor && record.definition.rawcode == *b"u001")
        .unwrap()
        .0;
    assert_eq!(
        h.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        Some(rally)
    );
    assert_eq!(
        h.executed_unit(producer)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .rally,
        Some(rally)
    );
    // This fixture's guard-post trip has finished by t=47. The explicit rally
    // move settles on the rally instead of continuing to the post.
    step(&mut vm, 47.);
    let h = &mut vm.host().inner;
    assert_eq!(h.executed_unit(unit).unwrap().position.unwrap(), rally);
    assert!(
        !h.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
}

const CACHE: &str = r#"
type gamecache extends agent
native InitGameCache takes string filename returns gamecache
native StoreUnit takes gamecache cache, string mission, string key, unit u returns boolean
native RestoreUnit takes gamecache cache, string mission, string key, player p, real x, real y, real facing returns unit
"#;

fn empty_ability_list() -> ([u8; 4], ObjectField) {
    (
        *b"uabi",
        super::super::super::ability_field("abilList", "_"),
    )
}

fn admit_fixture_heroes(h: ObjectHost) -> ObjectHost {
    let (rules, _) = super::super::hero_rules();
    let origin = crate::scripts::AssetIdentity {
        source: super::super::super::source().install,
        edition: "fixture".into(),
        version: "fixture".into(),
    };
    let tables = pcx_objects::editor::defaults::DefaultTables::from_bytes(
        pcx_objects::editor::defaults::Identity {
            install: origin.source.clone(),
            edition_version: origin.version.clone(),
        },
        &[("Units\\AbilityMetaData.slk", b"ID;PWXL;N;E\r\nB;Y3;X9\r\nC;X1;Y1;K\"ID\"\r\nC;X2;Y1;K\"field\"\r\nC;X3;Y1;K\"slk\"\r\nC;X4;Y1;K\"index\"\r\nC;X5;Y1;K\"type\"\r\nC;X6;Y1;K\"category\"\r\nC;X7;Y1;K\"repeat\"\r\nC;X8;Y1;K\"appendIndex\"\r\nC;X9;Y1;K\"data\"\r\nC;X1;Y2;K\"Mpx1\"\r\nC;X2;Y2;K\"Buttonpos\"\r\nC;X3;Y2;K\"Profile\"\r\nC;X4;Y2;K\"0\"\r\nC;X5;Y2;K\"int\"\r\nC;X6;Y2;K\"art\"\r\nC;X7;Y2;K\"0\"\r\nC;X8;Y2;K\"0\"\r\nC;X9;Y2;K\"\"\r\nC;X1;Y3;K\"Mpy1\"\r\nC;X2;Y3;K\"Buttonpos\"\r\nC;X3;Y3;K\"Profile\"\r\nC;X4;Y3;K\"1\"\r\nC;X5;Y3;K\"int\"\r\nC;X6;Y3;K\"art\"\r\nC;X7;Y3;K\"0\"\r\nC;X8;Y3;K\"0\"\r\nC;X9;Y3;K\"\"\r\nE\r\n"), ("Units\\AbilityData.slk", b"ID;PWXL;N;E\r\nB;Y2;X2\r\nC;X1;Y1;K\"alias\"\r\nC;X2;K\"race\"\r\nC;X1;Y2;K\"ZYX9\"\r\nC;X2;K\"fixture\"\r\nE\r\n")],
        &[("Units\\HumanAbilityFunc.txt", b"[ZYX9]\nArt=fixture/menu.blp\nButtonpos=3,1\nHotkey=W\nTip=Source menu\nUbertip=Source description\n")],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 3,
            max_asset_bytes: 4 << 10,
            max_aggregate_input_bytes: 4 << 10,
        },
    )
    .unwrap();
    h.with_hero_skills(
        tables,
        origin,
        Default::default(),
        rules,
        crate::playercolor_host::HeroBudget {
            abilities_per_unit: 8,
            overlay_objects: 0,
            modifications_per_ability: 0,
        },
    )
    .unwrap()
}

#[test]
fn a_restored_producer_keeps_its_rally_and_trains_toward_it() {
    let script = format!("{EVENTS}\n{CACHE}");
    let (mut vm, actor, producer) = fixture_with_training(
        &[empty_ability_list()],
        4,
        0.37,
        true,
        super::super::super::activity::rules_with_setrally(),
        true,
        &script,
    );
    let rally = [48., 16.];
    let store = std::sync::Arc::new(crate::game_cache::tests::Memory::default());
    let h = &mut vm.host().inner;
    h.admit_game_caches(
        store,
        crate::game_cache::Budget {
            caches: 4,
            entries: 16,
            text_bytes: 4096,
            storage_bytes: 8192,
            handle_bytes: 256,
        },
    )
    .unwrap();
    let owner = h.executed_unit(producer).unwrap().owner;
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(producer),
                Value::Str("setrally".into()),
                Value::Real(rally[0]),
                Value::Real(rally[1]),
            ],
        )
        .unwrap(),
        Value::Bool(true)
    );
    let cache = h
        .native("InitGameCache", &[Value::Str("campaign".into())])
        .unwrap();
    assert_eq!(
        h.native(
            "StoreUnit",
            &[
                cache.clone(),
                Value::Str("mission".into()),
                Value::Str("hall".into()),
                Value::Handle(producer),
            ],
        )
        .unwrap(),
        Value::Bool(true)
    );
    h.native("RemoveUnit", &[Value::Handle(producer)]).unwrap();
    let Value::Handle(restored) = h
        .native(
            "RestoreUnit",
            &[
                cache,
                Value::Str("mission".into()),
                Value::Str("hall".into()),
                Value::Handle(owner),
                Value::Real(112.),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("restored producer");
    };
    let activity = h
        .executed_unit(restored)
        .unwrap()
        .activity
        .as_ref()
        .unwrap();
    assert_eq!(activity.rally, Some(rally));
    assert!(!activity.moving());
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(restored)])
            .unwrap(),
        Value::Int(0)
    );
    h.fill_ai_guard_posts(4).unwrap();
    h.set_ai_replacement_count(4, 4).unwrap();
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    h.fill_ai_guard_posts(4).unwrap();
    step(&mut vm, 10.);
    let h = &mut vm.host().inner;
    let unit = h
        .executed_units()
        .find(|(id, record)| {
            *id != actor && *id != restored && record.definition.rawcode == *b"u001"
        })
        .unwrap()
        .0;
    assert_eq!(
        h.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        Some(rally)
    );
    assert_eq!(
        h.executed_unit(restored)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .rally,
        Some(rally)
    );
}

#[path = "unit_construction_tests.rs"]
mod construction;

struct AiProductionCatalog(Arc<dyn pcx_sim::world::ObjectCatalog + Send + Sync>);
impl pcx_sim::world::ObjectCatalog for AiProductionCatalog {
    fn resolve(&self, code: [u8; 4]) -> Result<Definition, pcx_sim::world::ResolutionProblem> {
        let mut definition = self.0.resolve(code)?;
        if code != *b"b001" {
            definition.fields.get_mut(b"T001").unwrap().value = Ok(FieldValue::Cell("".into()));
            if let Some(field) = definition.fields.get_mut(b"T011") {
                field.value = Ok(FieldValue::Cell("".into()));
            }
        }
        definition.fields.get_mut(b"F001").unwrap().value = Ok(FieldValue::Cell(
            if code == *b"b001" { "1" } else { "0" }.into(),
        ));
        Ok(definition)
    }
}
fn ai_production_fixture() -> (Vm, HandleId, HandleId) {
    ai_production_fixture_with(&[])
}
fn ai_production_fixture_with(overrides: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    fixture_with_training_configured(
        overrides,
        4,
        0.37,
        true,
        super::super::super::activity::rules(),
        false,
        (EVENTS, |mut host| {
            let catalog = host.unit_catalog().unwrap();
            host.construction_test_catalog(Arc::new(AiProductionCatalog(catalog)));
            host
        }),
    )
}

#[test]
fn ai_production_uses_paid_canonical_queue_and_preserves_partial_acceptance() {
    let (mut vm, actor, producer) = ai_production_fixture();
    let start = vm.global("EVENT_PLAYER_UNIT_TRAIN_START");
    let finish = vm.global("EVENT_PLAYER_UNIT_TRAIN_FINISH");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    for event in [start, finish] {
        h.native(
            "TriggerRegisterPlayerUnitEvent",
            &[trigger.clone(), Value::Handle(owner), event, Value::Null],
        )
        .unwrap();
    }
    let kind = i32::from_be_bytes(*b"u001");
    let handles = h.inner().handles();
    assert!(!h.set_ai_produce(4, 3, kind, -1).unwrap());
    assert_eq!(h.inner().handles(), handles);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 8);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 68);
    assert_eq!(h.ai_unit_count(4, kind, false).unwrap(), 3);
    assert_eq!(h.ai_unit_count(4, kind, true).unwrap(), 1);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 9);
    observe_event(h, producer, None);
    step(&mut vm, 9.);
    let h = &mut vm.host().inner;
    let trained = h
        .executed_units()
        .find(|(id, r)| *id != actor && r.definition.rawcode == *b"u001")
        .unwrap()
        .0;
    assert_eq!(h.executed_unit(trained).unwrap().owner, owner);
    // Completion and activation publish two events; both refer to the same producer.
    let mut events = 0;
    while let Some(event) = h.take_fired_event() {
        h.begin_fired_event(&event).unwrap();
        assert_eq!(
            h.native("GetTriggerUnit", &[]).unwrap(),
            Value::Handle(producer)
        );
        h.finish_fired_event();
        events += 1;
    }
    assert_eq!(events, 2);
    assert_eq!(h.ai_unit_count(4, kind, true).unwrap(), 2);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 8);
}

#[test]
fn ai_production_rejects_invalid_quantity_and_unsupported_requests_without_spending() {
    let (mut vm, actor, _) = ai_production_fixture();
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let kind = i32::from_be_bytes(*b"u001");
    for quantity in [-1, 0, 6] {
        assert!(!h.set_ai_produce(4, quantity, kind, -1).unwrap());
    }
    assert!(h.set_ai_produce(4, 1, kind, 0).is_err());
    assert!(
        !h.set_ai_produce(4, 1, i32::from_be_bytes(*b"b001"), -1)
            .unwrap(),
        "a structure with no builder answers false"
    );
    assert!(
        h.set_ai_produce(4, 1, i32::from_be_bytes(*b"J001"), -1)
            .is_err()
    );
    assert!(h.set_ai_produce(3, 1, kind, -1).is_err());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    assert!(h.next_training_completion(100.).is_none());
}

#[test]
fn ai_production_skips_paused_dead_and_foreign_producers() {
    let (mut vm, actor, producer) = ai_production_fixture();
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let kind = i32::from_be_bytes(*b"u001");
    h.native("PauseUnit", &[Value::Handle(producer), Value::Bool(true)])
        .unwrap();
    assert!(!h.set_ai_produce(4, 1, kind, -1).unwrap());
    h.native("PauseUnit", &[Value::Handle(producer), Value::Bool(false)])
        .unwrap();
    let other = h.native("Player", &[Value::Int(3)]).unwrap();
    let controller = h
        .native("GetPlayerController", &[Value::Handle(owner)])
        .unwrap();
    h.native("SetPlayerController", &[other.clone(), controller])
        .unwrap();
    h.native(
        "SetUnitOwner",
        &[Value::Handle(producer), other, Value::Bool(false)],
    )
    .unwrap();
    assert!(!h.set_ai_produce(4, 1, kind, -1).unwrap());
    h.native(
        "SetUnitOwner",
        &[
            Value::Handle(producer),
            Value::Handle(owner),
            Value::Bool(false),
        ],
    )
    .unwrap();
    h.native("KillUnit", &[Value::Handle(producer)]).unwrap();
    assert!(!h.set_ai_produce(4, 1, kind, -1).unwrap());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
}

#[test]
fn ai_production_full_queue_preserves_accepted_prefix_and_tech_food_refusals() {
    let (mut vm, actor, _) = ai_production_fixture_with(&[column(
        b"T003",
        SlkId::UnitBalance,
        "goldcost",
        FieldKind::Int,
        "10",
    )]);
    let food = vm.global("PLAYER_STATE_RESOURCE_FOOD_CAP");
    let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let kind = i32::from_be_bytes(*b"u001");
    h.native(
        "SetPlayerTechMaxAllowed",
        &[Value::Handle(owner), Value::Int(kind), Value::Int(0)],
    )
    .unwrap();
    assert!(!h.set_ai_produce(4, 1, kind, -1).unwrap());
    h.native(
        "SetPlayerTechMaxAllowed",
        &[Value::Handle(owner), Value::Int(kind), Value::Int(9)],
    )
    .unwrap();
    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), food.clone(), Value::Int(6)],
    )
    .unwrap();
    assert!(!h.set_ai_produce(4, 1, kind, -1).unwrap());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), food, Value::Int(40)],
    )
    .unwrap();
    h.native(
        "SetPlayerState",
        &[Value::Handle(owner), gold, Value::Int(1000)],
    )
    .unwrap();
    assert!(!h.set_ai_produce(4, 4, kind, -1).unwrap());
    assert_eq!(h.ai_unit_count(4, kind, false).unwrap(), 4);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 70);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"), 9);
}

#[test]
fn intermediate_optional_routing_cannot_starve_later_guard_training_projection() {
    let run = |mandatory_first| {
        let (mut vm, actor, producer) = fixture();
        let start = vm.global("EVENT_PLAYER_UNIT_TRAIN_START");
        let finish = vm.global("EVENT_PLAYER_UNIT_TRAIN_FINISH");
        let host = &mut vm.host().inner;
        let owner = host.executed_unit(actor).unwrap().owner;
        let trigger = host.native("CreateTrigger", &[]).unwrap();
        for event in [start, finish] {
            host.native(
                "TriggerRegisterPlayerUnitEvent",
                &[trigger.clone(), Value::Handle(owner), event, Value::Null],
            )
            .unwrap();
        }
        host.set_ai_replacement_count(4, 4).unwrap();
        host.native(
            "IssuePointOrder",
            &[
                Value::Handle(producer),
                Value::Str("gamma".into()),
                Value::Real(16.),
                Value::Real(16.),
            ],
        )
        .unwrap();
        host.native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"d001")),
                Value::Real(80.),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap();
        host.next_timer_expiry(10., &mut 1000).unwrap();
        let mut work = 128;
        if mandatory_first {
            host.advance_unit_activity_mandatory(10., &mut work)
                .unwrap();
            assert!(work > 0, "intermediate optional search must wait");
        } else {
            host.advance_unit_activity(10., &mut work).unwrap();
            assert_eq!(
                work, 0,
                "the previous schedule spends the shared allowance early"
            );
        }
        let after_first = work;
        // The intervening AI wake removes a post occupant and enqueues actual paid
        // guard replacement training; no constructor or ready-made replacement.
        host.native("KillUnit", &[Value::Handle(actor)]).unwrap();
        host.fill_ai_guard_posts(4).unwrap();
        let paid = [
            bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
            bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
        ];
        assert_eq!(paid, [49, 74]);
        observe_event(host, producer, None);
        host.next_timer_expiry(10.125, &mut 1000).unwrap();
        let later = host.advance_unit_activity_mandatory(10.125, &mut work);
        if !mandatory_first {
            assert!(
                later
                    .unwrap_err()
                    .to_string()
                    .contains("training work budget")
            );
            return (after_first, work, None);
        }
        later.unwrap();
        assert_eq!(host.unit_training_queue(producer).unwrap().len(), 1);
        let before_optional = work;
        host.flush_unit_activity_routes(10.125, &mut work).unwrap();
        assert!(work <= before_optional);
        let after_optional = work;
        host.flush_unit_activity_routes(10.125, &mut work).unwrap();
        assert_eq!(work, after_optional, "deferred witnesses are consumed once");
        host.fill_ai_guard_posts(4).unwrap();
        assert_eq!(
            [
                bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
                bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER")
            ],
            paid
        );
        step(&mut vm, 19.);
        let host = &mut vm.host().inner;
        let unit = host
            .executed_units()
            .find_map(|(unit, record)| {
                (unit != actor && record.definition.rawcode == *b"u001").then_some(unit)
            })
            .expect("canonical training creates the replacement");
        assert!(host.unit_training_queue(producer).unwrap().is_empty());
        assert_eq!(
            [
                bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
                bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER")
            ],
            paid
        );
        observe_event(host, producer, Some(unit));
        assert!(
            host.take_fired_event().is_none(),
            "START and FINISH each occur once"
        );
        (
            after_first,
            after_optional,
            host.executed_unit(unit).unwrap().position,
        )
    };
    run(false);
    assert_eq!(
        run(true),
        run(true),
        "shared work, paid completion and placement are deterministic"
    );
}

#[test]
fn mandatory_training_still_refuses_a_genuinely_undersized_shared_allowance() {
    let (mut vm, actor, producer) = fixture();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(actor).unwrap().owner;
    host.set_ai_replacement_count(4, 4).unwrap();
    host.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    host.fill_ai_guard_posts(4).unwrap();
    let queue = host.unit_training_queue(producer).unwrap();
    let handles = host.inner().handles();
    host.next_timer_expiry(1., &mut 1000).unwrap();
    let error = host
        .advance_unit_activity_mandatory(1., &mut 0)
        .unwrap_err();
    assert!(error.to_string().contains("training work budget"));
    assert_eq!(host.unit_training_queue(producer).unwrap(), queue);
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 74);
}

#[test]
fn deferred_route_witness_does_not_override_an_intervening_manual_stop() {
    let (mut vm, actor, producer) = fixture();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(actor).unwrap().owner;
    host.native(
        "IssuePointOrder",
        &[
            Value::Handle(producer),
            Value::Str("gamma".into()),
            Value::Real(16.),
            Value::Real(16.),
        ],
    )
    .unwrap();
    host.native(
        "CreateUnit",
        &[
            Value::Handle(owner),
            Value::Int(i32::from_be_bytes(*b"d001")),
            Value::Real(80.),
            Value::Real(16.),
            Value::Real(0.),
        ],
    )
    .unwrap();
    host.next_timer_expiry(10., &mut 1000).unwrap();
    let mut work = 128;
    host.advance_unit_activity_mandatory(10., &mut work)
        .unwrap();
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(producer), Value::Int(744)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let before = work;
    host.flush_unit_activity_routes(10., &mut work).unwrap();
    assert_eq!(
        work, before,
        "stale motion witness must not start another search"
    );
    assert_eq!(
        host.native("GetUnitCurrentOrder", &[Value::Handle(producer)])
            .unwrap(),
        Value::Int(0)
    );
    assert!(
        !host
            .executed_unit(producer)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
}

#[test]
fn repeated_subwake_witnesses_are_bounded_and_a_later_fault_discards_them() {
    let (mut vm, actor, producer) = fixture();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(actor).unwrap().owner;
    host.native(
        "IssuePointOrder",
        &[
            Value::Handle(producer),
            Value::Str("gamma".into()),
            Value::Real(16.),
            Value::Real(16.),
        ],
    )
    .unwrap();
    host.native(
        "CreateUnit",
        &[
            Value::Handle(owner),
            Value::Int(i32::from_be_bytes(*b"d001")),
            Value::Real(80.),
            Value::Real(16.),
            Value::Real(0.),
        ],
    )
    .unwrap();
    let mut work = 128;
    for tick in 80..=88 {
        let now = f64::from(tick) / 8.;
        host.next_timer_expiry(now, &mut 1000).unwrap();
        host.advance_unit_activity_mandatory(now, &mut work)
            .unwrap();
        assert_eq!(
            host.deferred_unit_activity_route_count(),
            1,
            "repeated queued units replace their witness rather than accumulate"
        );
        assert!(host.deferred_unit_activity_route_count() <= host.inner().handles());
    }
    host.set_ai_replacement_count(4, 4).unwrap();
    host.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    host.fill_ai_guard_posts(4).unwrap();
    let queue = host.unit_training_queue(producer).unwrap();
    host.next_timer_expiry(11.125, &mut 1000).unwrap();
    assert!(
        host.advance_unit_activity_mandatory(11.125, &mut 0)
            .unwrap_err()
            .to_string()
            .contains("training work budget")
    );
    assert_eq!(
        host.deferred_unit_activity_route_count(),
        0,
        "failed projection must not leave optional work for a later retry"
    );
    assert_eq!(
        host.unit_training_queue(producer).unwrap(),
        queue,
        "cleanup discards witnesses, not committed training effects"
    );
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
}

#[test]
fn guard_posts_fill_with_the_contexts_initial_replacement_limit_when_no_script_sets_one() {
    let (mut vm, actor, _) = fixture_with(&[
        profile_field(b"T002", "Requires", "techList", "d001"),
        requirement_amount("2"),
    ]);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
        .unwrap();
    assert_eq!(h.campaign_replacement_count(owner).unwrap(), None);
    assert_eq!(h.campaign_mode_policy().unwrap().default_replacement, 5);
    h.native("KillUnit", &[Value::Handle(actor)]).unwrap();
    // No `SetReplacementCount` ran: the policy's initializer value bounds the fill, no refusal.
    h.fill_ai_guard_posts(4).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
}

const RESEARCHED: [u8; 4] = *b"R001";
const GATED: [u8; 4] = *b"R002";

/// `R001` has two levels (gold 50 + 25 n, lumber 20 + 10 n, seconds 7 + 3 n) and needs a
/// `u001` for level 2; `R002` needs a `d001` nobody owns. Only `b001` researches them.
fn research_fixture() -> (Vm, HandleId, HandleId) {
    let (mut vm, actor, producer) = ai_production_fixture_with(&[profile_field(
        b"T011",
        "Researches",
        "upgradeList",
        "R001,R002",
    )]);
    step(&mut vm, 1.);
    let h = &mut vm.host().inner;
    let ctx = Arc::clone(h.player_context().unwrap());
    let rules = crate::upgrade_research::fixture_with_research(
        &ctx.sources.catalog,
        vec![
            (
                i32::from_be_bytes(RESEARCHED),
                2,
                [50, 25, 20, 10, 7, 3],
                vec![vec![], vec![*b"u001"]],
            ),
            (
                i32::from_be_bytes(GATED),
                1,
                [5, 0, 5, 0, 4, 0],
                vec![vec![*b"d001"]],
            ),
        ],
    );
    h.admit_upgrade_research(&ctx, rules).unwrap();
    (vm, actor, producer)
}

fn tech_level(h: &mut ObjectHost, owner: HandleId, upgrade: [u8; 4]) -> i32 {
    match h
        .native(
            "GetPlayerTechCount",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(upgrade)),
                Value::Bool(true),
            ],
        )
        .unwrap()
    {
        Value::Int(level) => level,
        other => panic!("{other:?}"),
    }
}

fn fund(vm: &mut Vm, owner: HandleId, gold: i32, lumber: i32) {
    for (state, value) in [
        ("PLAYER_STATE_RESOURCE_GOLD", gold),
        ("PLAYER_STATE_RESOURCE_LUMBER", lumber),
    ] {
        let state = vm.global(state);
        vm.host()
            .inner
            .native(
                "SetPlayerState",
                &[Value::Handle(owner), state, Value::Int(value)],
            )
            .unwrap();
    }
}

#[test]
fn ai_research_pays_the_level_cost_runs_the_level_time_and_raises_the_level() {
    let (mut vm, actor, producer) = research_fixture();
    let owner = vm.host().inner.executed_unit(actor).unwrap().owner;
    fund(&mut vm, owner, 100, 100);
    let h = &mut vm.host().inner;
    let upgrade = i32::from_be_bytes(RESEARCHED);
    assert_eq!(h.ai_upgrade_level(owner, upgrade).unwrap(), 0);
    assert_eq!(h.ai_upgrade_cost(owner, upgrade, false).unwrap(), 50);
    assert_eq!(h.ai_upgrade_cost(owner, upgrade, true).unwrap(), 20);
    h.set_ai_upgrade(4, upgrade).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 50);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 80);
    // Queued or running research counts as a level for the AI, and prices the next one.
    assert_eq!(h.ai_upgrade_level(owner, upgrade).unwrap(), 1);
    assert_eq!(h.ai_upgrade_cost(owner, upgrade, false).unwrap(), 75);
    assert_eq!(h.ai_upgrade_cost(owner, upgrade, true).unwrap(), 30);
    assert_eq!(tech_level(h, owner, RESEARCHED), 0);
    assert_eq!(h.unit_training_queue(producer).unwrap().len(), 1);
    assert!(h.set_ai_upgrade(4, upgrade).is_err(), "already running");
    step(&mut vm, 1. + 6.);
    let h = &mut vm.host().inner;
    assert_eq!(tech_level(h, owner, RESEARCHED), 0, "7 s of research");
    step(&mut vm, 1. + 7.5);
    let h = &mut vm.host().inner;
    assert_eq!(tech_level(h, owner, RESEARCHED), 1);
    assert_eq!(h.ai_upgrade_level(owner, upgrade).unwrap(), 1);
    assert!(h.unit_training_queue(producer).unwrap().is_empty());
    fund(&mut vm, owner, 100, 100);
    let h = &mut vm.host().inner;
    h.set_ai_upgrade(4, upgrade).unwrap();
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 25);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 70);
    step(&mut vm, 1. + 7.5 + 10.5);
    let h = &mut vm.host().inner;
    assert_eq!(tech_level(h, owner, RESEARCHED), 2, "10 s for level 2");
    assert!(
        h.set_ai_upgrade(4, upgrade).is_err(),
        "a level beyond the upgrade's levels is refused loudly"
    );
}

#[test]
fn ai_research_waits_for_funds_requirements_a_researcher_and_an_idle_queue() {
    let (mut vm, actor, producer) = research_fixture();
    let owner = vm.host().inner.executed_unit(actor).unwrap().owner;
    let upgrade = i32::from_be_bytes(RESEARCHED);
    fund(&mut vm, owner, 49, 80);
    let h = &mut vm.host().inner;
    h.set_ai_upgrade(4, upgrade).unwrap();
    assert!(h.unit_training_queue(producer).unwrap().is_empty(), "gold");
    fund(&mut vm, owner, 90, 19);
    let h = &mut vm.host().inner;
    h.set_ai_upgrade(4, upgrade).unwrap();
    assert!(
        h.unit_training_queue(producer).unwrap().is_empty(),
        "lumber"
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    fund(&mut vm, owner, 90, 80);
    let h = &mut vm.host().inner;
    h.set_ai_upgrade(4, i32::from_be_bytes(GATED)).unwrap();
    assert!(
        h.unit_training_queue(producer).unwrap().is_empty(),
        "requires d001"
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    // A queued unit keeps the building busy.
    assert!(
        h.set_ai_produce(4, 1, i32::from_be_bytes(*b"u001"), -1)
            .unwrap()
    );
    let gold = bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD");
    h.set_ai_upgrade(4, upgrade).unwrap();
    assert_eq!(h.unit_training_queue(producer).unwrap().len(), 1);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), gold);
    // An upgrade no building lists, and one the rules do not know.
    assert!(h.set_ai_upgrade(4, i32::from_be_bytes(*b"R009")).is_err());
    assert!(h.set_ai_upgrade(3, upgrade).is_err(), "inactive player");
}

#[test]
fn ai_research_refunds_on_cancel_and_leaves_the_level_unchanged() {
    let (mut vm, actor, producer) = research_fixture();
    let owner = vm.host().inner.executed_unit(actor).unwrap().owner;
    fund(&mut vm, owner, 100, 100);
    let h = &mut vm.host().inner;
    let upgrade = i32::from_be_bytes(RESEARCHED);
    h.set_ai_upgrade(4, upgrade).unwrap();
    assert!(h.cancel_unit_training(producer, 0).unwrap());
    let gold = bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD");
    assert!(
        gold > 50 && gold < 100,
        "refund, not the full price: {gold}"
    );
    assert!(h.unit_training_queue(producer).unwrap().is_empty());
    assert_eq!(h.ai_upgrade_level(owner, upgrade).unwrap(), 0);
    step(&mut vm, 20.);
    assert_eq!(tech_level(&mut vm.host().inner, owner, RESEARCHED), 0);
}

/// The second fixture gives every unit a `Builds` list with `b001`.
#[test]
fn ai_structure_requests_answer_false_without_a_builder_and_refuse_placement_with_one() {
    let structure = i32::from_be_bytes(*b"b001");
    let (mut vm, actor, producer) = ai_production_fixture();
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    assert!(!h.set_ai_produce(4, 1, structure, -1).unwrap());
    assert!(!h.set_ai_produce(4, 3, structure, -1).unwrap());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    assert!(h.unit_training_queue(producer).unwrap().is_empty());

    let (mut vm, actor, _) =
        ai_production_fixture_with(&[profile_field(b"T012", "Builds", "unitList", "b001")]);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(actor).unwrap().owner;
    let error = h.set_ai_produce(4, 1, structure, -1).unwrap_err();
    assert!(
        error.to_string().contains("placement is not admitted"),
        "{error}"
    );
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    // A unit that upgrades into the kind also routes the request (the executable's fallback).
    let (mut vm, actor, _) =
        ai_production_fixture_with(&[profile_field(b"T009", "Upgrade", "unitList", "b001")]);
    let h = &mut vm.host().inner;
    let _ = actor;
    let error = h.set_ai_produce(4, 1, structure, -1).unwrap_err();
    assert!(
        error.to_string().contains("upgrade is not admitted"),
        "{error}"
    );
    let (mut vm, actor, _) =
        ai_production_fixture_with(&[profile_field(b"T012", "Builds", "unitList", "b001")]);
    let h = &mut vm.host().inner;
    let _ = actor;
    // Dead units are not builders.
    for unit in h.executed_units().map(|(id, _)| id).collect::<Vec<_>>() {
        h.native("KillUnit", &[Value::Handle(unit)]).unwrap();
    }
    assert!(!h.set_ai_produce(4, 1, structure, -1).unwrap());
}
