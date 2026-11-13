//! Internal canonical arrival/lifecycle fixtures; these do not exercise public point-order travel.
use super::*;
use crate::playercolor_host::BuildUnavailable;
use pcx_script::RealOps;

fn fixture() -> (Vm, HandleId, HandleId) {
    fixture_with(&[])
}
fn fixture_with(overrides: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    let mut fields = vec![column(
        b"F001",
        SlkId::UnitBalance,
        "isbldg",
        FieldKind::Bool,
        "1",
    )];
    fields.push(column(
        b"T006",
        SlkId::UnitBalance,
        "fused",
        FieldKind::Int,
        "0",
    ));
    fields.extend_from_slice(overrides);
    let (mut vm, worker, producer) = super::fixture_with(&fields);
    let h = &mut vm.host().inner;
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(
        &context,
        crate::upgrade_research::fixture(&context.sources.catalog),
    )
    .unwrap();
    (vm, worker, producer)
}
fn register(vm: &mut Vm, owner: HandleId, event: &str) {
    let event = vm.global(event);
    let h = &mut vm.host().inner;
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    h.native(
        "TriggerRegisterPlayerUnitEvent",
        &[trigger, Value::Handle(owner), event, Value::Null],
    )
    .unwrap();
}
fn tech(h: &mut ObjectHost, owner: HandleId) -> i32 {
    let Value::Int(count) = h
        .native(
            "GetPlayerTechCount",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"b001")),
                Value::Bool(true),
            ],
        )
        .unwrap()
    else {
        panic!()
    };
    count
}
fn site(vm: &mut Vm, worker: HandleId) -> HandleId {
    vm.host()
        .inner
        .begin_unit_construction(worker, i32::from_be_bytes(*b"b001"), [80., 16.])
        .unwrap()
        .unwrap()
}
fn observe(h: &mut ObjectHost, site: HandleId, response: &str) -> f32 {
    let event = h.take_fired_event().expect("construction event");
    h.begin_fired_event(&event).unwrap().expect("enabled");
    assert_eq!(
        h.native("GetTriggerUnit", &[]).unwrap(),
        Value::Handle(site)
    );
    assert_eq!(h.native(response, &[]).unwrap(), Value::Handle(site));
    let life = h.executed_unit(site).unwrap().vitals.unwrap().life;
    h.finish_fired_event();
    life
}

#[test]
fn start_callbacks_see_full_life_then_current_maximum_is_scaled_once() {
    let (mut vm, worker, _) = fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    let site = site(&mut vm, worker);
    let h = &mut vm.host().inner;
    let max = h.executed_unit(site).unwrap().vitals.unwrap().max_life;
    assert_eq!(observe(h, site, "GetConstructingStructure"), max);
    assert_eq!(h.executed_unit(site).unwrap().vitals.unwrap().life, max);
    let event = h.take_fired_event().unwrap();
    h.begin_fired_event(&event).unwrap();
    assert_eq!(
        h.native("GetConstructedStructure", &[]).unwrap(),
        Value::Null
    );
    // Simulate a canonical maximum mutation inside the final START action.
    h.set_test_maximum(site, Some(max * 2.), None);
    h.finish_fired_event();
    assert!((h.executed_unit(site).unwrap().vitals.unwrap().life - max * 2. * 0.23).abs() < 0.001);
    assert_eq!(h.construction_request_revision(worker).unwrap(), 1);
}

#[test]
fn incomplete_site_counts_and_training_are_suppressed_until_completion() {
    let (mut vm, worker, _) = fixture_with(&[column(
        b"T007",
        SlkId::UnitBalance,
        "fmade",
        FieldKind::Int,
        "7",
    )]);
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    let before = tech(&mut vm.host().inner, owner);
    let food = bank(&vm.host().inner, owner, "PLAYER_STATE_RESOURCE_FOOD_CAP");
    let site = site(&mut vm, worker);
    let h = &mut vm.host().inner;
    assert_eq!(tech(h, owner), before);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_CAP"), food);
    assert_eq!(
        h.native(
            "IssueImmediateOrderById",
            &[
                Value::Handle(site),
                Value::Int(i32::from_be_bytes(*b"u001"))
            ]
        )
        .unwrap(),
        Value::Bool(false)
    );
    step(&mut vm, 9.);
    let h = &mut vm.host().inner;
    h.advance_unit_construction(site, 9.).unwrap();
    assert!(h.unit_construction(site).unwrap().is_none());
    assert_eq!(tech(h, owner), before + 1);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_FOOD_CAP"), food + 7);
    observe(h, site, "GetConstructedStructure");
    assert!(h.take_fired_event().is_none());
}

#[test]
fn participation_pause_resume_keeps_one_site_and_spends_once() {
    let (mut vm, worker, _) = fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    let site = site(&mut vm, worker);
    step(&mut vm, 2.);
    vm.host().inner.advance_unit_construction(site, 2.).unwrap();
    vm.host().inner.set_construction_worker(site, None).unwrap();
    let life = vm
        .host()
        .inner
        .executed_unit(site)
        .unwrap()
        .vitals
        .unwrap()
        .life;
    step(&mut vm, 7.);
    vm.host().inner.advance_unit_construction(site, 7.).unwrap();
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(site)
            .unwrap()
            .vitals
            .unwrap()
            .life,
        life
    );
    assert_eq!(
        vm.host()
            .inner
            .unit_construction(site)
            .unwrap()
            .unwrap()
            .elapsed,
        2.
    );
    vm.host()
        .inner
        .set_construction_worker(site, Some(worker))
        .unwrap();
    step(&mut vm, 14.);
    let h = &mut vm.host().inner;
    h.advance_unit_construction(site, 14.).unwrap();
    assert!(h.unit_construction(site).unwrap().is_none());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 49);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 74);
}

#[test]
fn cancel_refunds_source_rate_delivers_zero_life_then_retires_once() {
    let (mut vm, worker, _) = fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_CANCEL");
    let site = site(&mut vm, worker);
    let h = &mut vm.host().inner;
    assert!(h.cancel_unit_construction(site).unwrap());
    assert!(!h.cancel_unit_construction(site).unwrap());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 64);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 76);
    assert_eq!(observe(h, site, "GetCancelledStructure"), 0.);
    assert!(h.executed_unit(site).is_err());
    assert!(!h.cancel_unit_construction(site).unwrap());
}

#[test]
fn removed_or_cancelled_site_in_start_action_is_not_reinitialized() {
    for cancel in [false, true] {
        let (mut vm, worker, _) = fixture();
        let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
        register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
        let site = site(&mut vm, worker);
        let h = &mut vm.host().inner;
        let event = h.take_fired_event().unwrap();
        h.begin_fired_event(&event).unwrap();
        if cancel {
            assert!(h.cancel_unit_construction(site).unwrap());
        } else {
            h.native("RemoveUnit", &[Value::Handle(site)]).unwrap();
        }
        h.finish_fired_event();
        assert!(h.executed_unit(site).is_err());
        assert!(h.unit_construction_order(worker).unwrap().is_none());
    }
}

#[test]
fn paused_worker_freezes_work_and_damaged_cancel_refuses_before_refund() {
    let (mut vm, worker, _) = fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    let site = site(&mut vm, worker);
    step(&mut vm, 2.);
    vm.host().inner.advance_unit_construction(site, 2.).unwrap();
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(worker), Value::Bool(true)])
        .unwrap();
    step(&mut vm, 7.);
    vm.host().inner.advance_unit_construction(site, 7.).unwrap();
    assert_eq!(
        vm.host()
            .inner
            .unit_construction(site)
            .unwrap()
            .unwrap()
            .elapsed,
        2.
    );
    let h = &mut vm.host().inner;
    let life = h.executed_unit(site).unwrap().vitals.unwrap().life;
    h.set_test_life(site, life - 0.25);
    let before = bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD");
    assert!(h.cancel_unit_construction(site).is_err());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), before);
    assert!(h.unit_construction(site).unwrap().is_some());
    assert!(h.cancel_unit_construction(owner).is_err());
}

#[test]
fn invalid_source_duration_refuses_before_spending_or_publishing_site() {
    let (mut vm, worker, _) = fixture_with(&[column(
        b"T005",
        SlkId::UnitBalance,
        "bldtm",
        FieldKind::Int,
        "0",
    )]);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(worker).unwrap().owner;
    let before = h.executed_units().count();
    assert!(
        h.begin_unit_construction(worker, i32::from_be_bytes(*b"b001"), [80., 16.])
            .is_err()
    );
    assert_eq!(h.executed_units().count(), before);
    assert_eq!(h.construction_request_revision(worker).unwrap(), 0);
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), 90);
    assert!(h.take_fired_event().is_none());
}

#[test]
fn build_options_read_authored_list_and_distinguish_unready_from_malformed() {
    let builds = profile_field(b"C001", "Builds", "unitList", "b001,b001");
    let (mut vm, worker, _) = fixture_with(&[builds]);
    let host = &mut vm.host().inner;
    let options = host.unit_build_options(worker).unwrap();
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].kind, i32::from_be_bytes(*b"b001"));
    assert_eq!(options[0].unavailable, None);
    host.native("PauseUnit", &[Value::Handle(worker), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        host.unit_build_options(worker).unwrap()[0].unavailable,
        Some(BuildUnavailable::Worker)
    );

    let builds = profile_field(b"C001", "Builds", "unitList", "broken");
    let (mut vm, worker, _) = fixture_with(&[builds]);
    assert!(vm.host().inner.unit_build_options(worker).is_err());
}

struct PublicBuildCatalog(Arc<dyn ObjectCatalog + Send + Sync>);
impl ObjectCatalog for PublicBuildCatalog {
    fn resolve(&self, code: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let mut definition = self.0.resolve(code)?;
        if code == *b"b001" {
            definition.fields.get_mut(b"M001").unwrap().value = Ok(FieldValue::Cell("_".into()));
            definition.fields.get_mut(b"P001").unwrap().value =
                Ok(FieldValue::Cell("build.tga".into()));
        } else {
            definition.fields.get_mut(b"F001").unwrap().value = Ok(FieldValue::Cell("0".into()));
        }
        Ok(definition)
    }
}

fn public_fixture() -> (Vm, HandleId, HandleId) {
    public_fixture_with(&[])
}

fn public_fixture_with(overrides: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    public_fixture_configured(overrides, |host| host)
}

fn public_fixture_configured(
    overrides: &[([u8; 4], ObjectField)],
    configure: impl FnOnce(ObjectHost) -> ObjectHost,
) -> (Vm, HandleId, HandleId) {
    public_fixture_admitted(overrides, true, configure)
}

fn public_fixture_admitted(
    overrides: &[([u8; 4], ObjectField)],
    heroes: bool,
    configure: impl FnOnce(ObjectHost) -> ObjectHost,
) -> (Vm, HandleId, HandleId) {
    let mut fields = vec![
        profile_field(b"C001", "Builds", "unitList", "b001"),
        column(
            b"C002",
            SlkId::UnitData,
            "race",
            FieldKind::Token,
            "fixture",
        ),
        column(
            b"C004",
            SlkId::UnitBalance,
            "preventPlace",
            FieldKind::Token,
            "unbuildable",
        ),
        column(
            b"C005",
            SlkId::UnitBalance,
            "requirePlace",
            FieldKind::Token,
            "_",
        ),
        column(
            b"C006",
            SlkId::UnitData,
            "requireWaterRadius",
            FieldKind::Real,
            "0",
        ),
        column(b"F001", SlkId::UnitBalance, "isbldg", FieldKind::Bool, "1"),
        column(b"T006", SlkId::UnitBalance, "fused", FieldKind::Int, "0"),
        empty_ability_list(),
    ];
    fields.extend_from_slice(overrides);
    let (mut vm, worker, producer) = super::fixture_with_training_configured(
        &fields,
        8,
        0.37,
        true,
        super::super::super::super::activity::rules_with_repair(),
        heroes,
        (
            &format!(
                "{EVENTS}\nnative SetUnitPosition takes unit u, real x, real y returns nothing\n\
                native GetBJPlayerNeutralExtra takes nothing returns integer\n\
                native GetBJPlayerNeutralVictim takes nothing returns integer\n\
                native GetBJMaxPlayers takes nothing returns integer\n\
                native GetPlayerNeutralAggressive takes nothing returns integer\n\
                native GetPlayerNeutralPassive takes nothing returns integer\n\
                type limitop extends eventid\n\
                constant native ConvertLimitOp takes integer value returns limitop\n\
                native TriggerRegisterPlayerStateEvent takes trigger t, player p, playerstate s, limitop op, real limit returns event\n\
                constant native GetEventPlayerState takes nothing returns playerstate\n\
                globals\n\
                constant limitop LESS_THAN=ConvertLimitOp(0)\n\
                constant limitop LESS_THAN_OR_EQUAL=ConvertLimitOp(1)\n\
                constant limitop EQUAL=ConvertLimitOp(2)\n\
                constant limitop GREATER_THAN_OR_EQUAL=ConvertLimitOp(3)\n\
                constant limitop GREATER_THAN=ConvertLimitOp(4)\n\
                constant limitop NOT_EQUAL=ConvertLimitOp(5)\n\
                constant playerevent EVENT_PLAYER_STATE_LIMIT=ConvertPlayerEvent(47)\n\
                endglobals\n"
            ),
            |mut host| {
                let catalog = Arc::new(PublicBuildCatalog(host.unit_catalog().unwrap()));
                host.construction_test_catalog(catalog);
                host.construction_test_footprint_loader(|path| {
                    assert_eq!(path, "build.tga");
                    Ok(crate::unit_placement::Footprint {
                        dimensions: [2, 2],
                        origin: crate::scripts::AssetIdentity {
                            source: "synthetic build footprint".into(),
                            edition: "fixture".into(),
                            version: "nonstock".into(),
                        },
                        cells: vec![[-1, -1], [0, -1], [-1, 0], [0, 0]],
                    })
                });
                configure(host)
            },
        ),
    );
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_upgrade_research(
        &context,
        crate::upgrade_research::fixture(&context.sources.catalog),
    )
    .unwrap();
    host.native("RemoveGuardPosition", &[Value::Handle(worker)])
        .unwrap();
    (vm, worker, producer)
}

#[test]
fn the_build_order_native_makes_the_same_request_and_reports_acceptance() {
    let (mut vm, worker, _) = public_fixture();
    let host = &mut vm.host().inner;
    let revision = host.construction_request_revision(worker).unwrap();
    let args = [
        Value::Handle(worker),
        Value::Int(i32::from_be_bytes(*b"b001")),
        Value::Real(80.),
        Value::Real(16.),
    ];
    assert_eq!(
        host.native("IssueBuildOrderById", &args).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        host.construction_request_revision(worker).unwrap(),
        revision + 1
    );
    assert_eq!(
        host.unit_construction_order(worker).unwrap().unwrap().site,
        None
    );
    // The second request cannot pay: a refused request reports false and changes nothing.
    let owner = host.executed_unit(worker).unwrap().owner;
    let gold = host
        .player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
        .unwrap();
    assert_eq!(gold, Some(49));
    assert_eq!(
        host.native("IssueBuildOrderById", &args).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        gold
    );
}

#[test]
fn public_point_order_reserves_once_arrives_and_delivers_real_construction() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    let host = &mut vm.host().inner;
    let revision = host.construction_request_revision(worker).unwrap();
    assert_eq!(
        host.native(
            "IssuePointOrderById",
            &[
                Value::Handle(worker),
                Value::Int(i32::from_be_bytes(*b"b001")),
                Value::Real(80.),
                Value::Real(16.)
            ]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        host.construction_request_revision(worker).unwrap(),
        revision + 1
    );
    assert_eq!(
        host.unit_construction_order(worker).unwrap().unwrap().site,
        None
    );
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(49)
    );
    step(&mut vm, 10.);
    let host = &mut vm.host().inner;
    let site = host
        .unit_construction_order(worker)
        .unwrap()
        .unwrap()
        .site
        .unwrap();
    assert_eq!(
        observe(host, site, "GetConstructingStructure"),
        host.executed_unit(site).unwrap().vitals.unwrap().max_life
    );
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(49)
    );
    step(&mut vm, 20.);
    let host = &mut vm.host().inner;
    assert!(host.unit_construction(site).unwrap().is_none());
    observe(host, site, "GetConstructedStructure");
}

#[test]
fn unattended_build_releases_worker_and_finishes_from_source_duration() {
    let (mut vm, worker, _) = public_fixture_with(&[column(
        b"C002",
        SlkId::UnitData,
        "race",
        FieldKind::Token,
        "other-fixture",
    )]);
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    let host = &mut vm.host().inner;
    assert_eq!(
        host.unit_build_menu_ability(worker).unwrap(),
        Some(*b"ZYX8")
    );
    assert_eq!(
        host.native(
            "IssueBuildOrderById",
            &[
                Value::Handle(worker),
                Value::Int(i32::from_be_bytes(*b"b001")),
                Value::Real(80.),
                Value::Real(16.),
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 10.);
    let host = &mut vm.host().inner;
    let site = host
        .executed_units()
        .find(|(_, record)| record.definition.rawcode == *b"b001" && record.construction.is_some())
        .unwrap()
        .0;
    assert!(host.unit_construction_order(worker).unwrap().is_none());
    assert_eq!(
        host.native("GetUnitCurrentOrder", &[Value::Handle(worker)])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(host.unit_construction(site).unwrap().unwrap().worker, None);
    observe(host, site, "GetConstructingStructure");
    step(&mut vm, 20.);
    let host = &mut vm.host().inner;
    assert!(host.unit_construction(site).unwrap().is_none());
    assert!(host.executed_unit(worker).unwrap().vitals.unwrap().life > 0.);
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(49)
    );
    observe(host, site, "GetConstructedStructure");
}

#[test]
fn unattended_site_survives_worker_removal_and_cancel_refunds_once() {
    let (mut vm, worker, _) = public_fixture_with(&[column(
        b"C002",
        SlkId::UnitData,
        "race",
        FieldKind::Token,
        "other-fixture",
    )]);
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueBuildOrderById",
                &[
                    Value::Handle(worker),
                    Value::Int(i32::from_be_bytes(*b"b001")),
                    Value::Real(80.),
                    Value::Real(16.),
                ]
            )
            .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 10.);
    let host = &mut vm.host().inner;
    let site = host
        .executed_units()
        .find(|(_, record)| record.definition.rawcode == *b"b001" && record.construction.is_some())
        .unwrap()
        .0;
    host.native("RemoveUnit", &[Value::Handle(worker)]).unwrap();
    assert!(host.unit_construction(site).unwrap().is_some());
    assert!(host.cancel_unit_construction(site).unwrap());
    assert!(!host.cancel_unit_construction(site).unwrap());
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(64)
    );
}

fn point_build(host: &mut ObjectHost, worker: HandleId) {
    assert_eq!(
        host.native(
            "IssuePointOrderById",
            &[
                Value::Handle(worker),
                Value::Int(i32::from_be_bytes(*b"b001")),
                Value::Real(80.),
                Value::Real(16.)
            ]
        )
        .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn public_stop_before_arrival_refunds_without_site_events() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    let host = &mut vm.host().inner;
    point_build(host, worker);
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(49)
    );
    step(&mut vm, 1.);
    let host = &mut vm.host().inner;
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(worker), Value::Int(744)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(90)
    );
    assert!(host.unit_construction_order(worker).unwrap().is_none());
    step(&mut vm, 12.);
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn public_stop_and_source_repair_resume_the_same_paid_site() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    point_build(&mut vm.host().inner, worker);
    step(&mut vm, 10.);
    let host = &mut vm.host().inner;
    let site = host
        .unit_construction_order(worker)
        .unwrap()
        .unwrap()
        .site
        .unwrap();
    assert_eq!(
        host.native("GetUnitCurrentOrder", &[Value::Handle(worker)])
            .unwrap(),
        Value::Int(9001)
    );
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(worker), Value::Int(744)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let stopped = host.unit_construction(site).unwrap().unwrap().elapsed;
    step(&mut vm, 15.);
    let host = &mut vm.host().inner;
    assert_eq!(
        host.unit_construction(site).unwrap().unwrap().elapsed,
        stopped
    );
    assert_eq!(
        host.native(
            "IssueTargetOrder",
            &[
                Value::Handle(worker),
                Value::Str("repair".into()),
                Value::Handle(site)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 25.);
    let host = &mut vm.host().inner;
    assert!(
        host.unit_construction(site).unwrap().is_none(),
        "site {:?}, order {:?}, position {:?}",
        host.unit_construction(site).unwrap(),
        host.unit_construction_order(worker).unwrap(),
        host.executed_unit(worker).unwrap().position
    );
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(49)
    );
}

#[test]
fn public_kill_before_arrival_releases_the_reservation_without_site_events() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    point_build(&mut vm.host().inner, worker);
    step(&mut vm, 1.);
    let host = &mut vm.host().inner;
    host.native("KillUnit", &[Value::Handle(worker)]).unwrap();
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(90)
    );
    assert!(host.unit_construction_order(worker).unwrap().is_none());
    step(&mut vm, 12.);
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn implicit_build_menu_uses_unranked_source_profiles_without_inventing_an_ability_rank() {
    let (mut vm, worker, _) = public_fixture();
    let menu = vm
        .host()
        .inner
        .unit_build_menu_command(worker)
        .unwrap()
        .unwrap();
    assert_eq!(menu.ability, *b"ZYX9");
    assert_eq!(menu.art, "fixture/menu.blp");
    assert_eq!(menu.position, [3, 1]);
    assert_eq!(menu.hotkey, Some('W'));
    assert_eq!(
        menu.tooltip.unwrap(),
        crate::ability_profile::Tooltip {
            title: "Source menu".into(),
            description: "Source description".into(),
        }
    );
    assert!(
        vm.host()
            .inner
            .ability_button(worker, *b"ZYX9")
            .unwrap()
            .is_none()
    );
}

#[test]
fn public_build_refusals_leave_bank_revision_and_units_unchanged() {
    let (mut vm, worker, _) = public_fixture();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(worker).unwrap().owner;
    let before = host.executed_units().count();
    for point in [[-1., 16.], [112., 16.]] {
        assert_eq!(
            host.preview_unit_construction(worker, i32::from_be_bytes(*b"b001"), point)
                .unwrap(),
            crate::playercolor_host::ConstructionPreview::Blocked
        );
        assert_eq!(
            host.native(
                "IssuePointOrderById",
                &[
                    Value::Handle(worker),
                    Value::Int(i32::from_be_bytes(*b"b001")),
                    Value::Real(point[0]),
                    Value::Real(point[1])
                ]
            )
            .unwrap(),
            Value::Bool(false)
        );
    }
    assert_eq!(host.construction_request_revision(worker).unwrap(), 0);
    assert_eq!(host.executed_units().count(), before);
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(90)
    );
    assert!(
        host.preview_unit_construction(worker, i32::from_be_bytes(*b"b001"), [f32::NAN, 0.])
            .is_err()
    );
}

#[test]
fn placement_changed_during_travel_releases_paid_request_without_start_event() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    let host = &mut vm.host().inner;
    point_build(host, worker);
    host.native(
        "CreateUnit",
        &[
            Value::Handle(owner),
            Value::Int(i32::from_be_bytes(*b"b001")),
            Value::Real(80.),
            Value::Real(16.),
            Value::Real(0.),
        ],
    )
    .unwrap();
    step(&mut vm, 12.);
    let host = &mut vm.host().inner;
    assert!(host.unit_construction_order(worker).unwrap().is_none());
    assert_eq!(
        host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
            .unwrap(),
        Some(90)
    );
    assert!(host.take_fired_event().is_none());
}

#[test]
fn pending_requests_reserve_production_limits_without_providing_technology() {
    let (mut vm, worker, _) = public_fixture();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(worker).unwrap().owner;
    host.set_ai_replacement_count(4, 4).unwrap();
    let Value::Handle(other) = host
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(16.),
                Value::Real(48.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!()
    };
    host.native(
        "SetPlayerTechMaxAllowed",
        &[
            Value::Handle(owner),
            Value::Int(i32::from_be_bytes(*b"b001")),
            Value::Int(2),
        ],
    )
    .unwrap();
    point_build(host, worker);
    assert_eq!(
        host.unit_build_options(other).unwrap()[0].unavailable,
        Some(BuildUnavailable::Limit)
    );
    assert_eq!(tech(host, owner), 1);
    host.native(
        "IssueImmediateOrderById",
        &[Value::Handle(worker), Value::Int(744)],
    )
    .unwrap();
    assert_eq!(host.unit_build_options(other).unwrap()[0].unavailable, None);
}

#[test]
fn preview_and_commit_share_source_capability_refusals_before_any_reservation() {
    for field in [
        column(b"T005", SlkId::UnitBalance, "bldtm", FieldKind::Int, "0"),
        column(b"T006", SlkId::UnitBalance, "fused", FieldKind::Int, "2"),
    ] {
        let (mut vm, worker, _) = public_fixture_with(&[field]);
        let host = &mut vm.host().inner;
        let owner = host.executed_unit(worker).unwrap().owner;
        assert!(
            host.preview_unit_construction(worker, i32::from_be_bytes(*b"b001"), [80., 16.])
                .is_err()
        );
        assert!(
            host.native(
                "IssuePointOrderById",
                &[
                    Value::Handle(worker),
                    Value::Int(i32::from_be_bytes(*b"b001")),
                    Value::Real(80.),
                    Value::Real(16.)
                ]
            )
            .is_err()
        );
        assert_eq!(host.construction_request_revision(worker).unwrap(), 0);
        assert_eq!(
            host.player_resource_value(owner, "PLAYER_STATE_RESOURCE_GOLD")
                .unwrap(),
            Some(90)
        );
    }
}

#[test]
fn damage_then_life_restoration_cannot_enter_the_undamaged_refund_path() {
    let (mut vm, worker, _) = fixture();
    let site = site(&mut vm, worker);
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(worker).unwrap().owner;
    let life = host.executed_unit(site).unwrap().vitals.unwrap().life;
    host.damage_unit(worker, site, 1.).unwrap();
    host.set_test_life(site, life);
    assert_eq!(host.executed_unit(site).unwrap().vitals.unwrap().life, life);
    let before = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    assert!(host.cancel_unit_construction(site).is_err());
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), before);
}

#[test]
fn optional_build_menu_discovery_accepts_nonbuilders_without_construction_capability() {
    let (mut vm, worker, _) = fixture();
    let host = &mut vm.host().inner;
    host.clear_construction_test_player_admission();
    assert_eq!(host.unit_build_menu_command(worker).unwrap(), None);
    assert!(host.unit_build_options(worker).is_err());

    for empty in ["", "_", "-", " , _ , - "] {
        let (mut vm, worker, _) =
            fixture_with(&[profile_field(b"C001", "Builds", "unitList", empty)]);
        vm.host().inner.clear_construction_test_player_admission();
        assert_eq!(
            vm.host().inner.unit_build_menu_ability(worker).unwrap(),
            None
        );
    }
}

#[test]
fn optional_build_menu_discovery_retains_malformed_builder_errors_without_capability() {
    let (mut vm, worker, _) =
        fixture_with(&[profile_field(b"C001", "Builds", "unitList", "broken")]);
    vm.host().inner.clear_construction_test_player_admission();
    assert!(vm.host().inner.unit_build_menu_command(worker).is_err());
    let (mut vm, worker, _) = public_fixture();
    vm.host().inner.clear_construction_test_player_admission();
    assert_eq!(
        vm.host().inner.unit_build_menu_command(worker).unwrap(),
        None
    );
    assert!(vm.host().inner.unit_build_options(worker).is_err());
}

#[test]
fn optional_build_menu_discovery_omits_other_source_worker_styles() {
    let (mut vm, worker, _) = public_fixture_with(&[column(
        b"C002",
        SlkId::UnitData,
        "race",
        FieldKind::Token,
        "other-style",
    )]);
    let host = &mut vm.host().inner;
    assert_eq!(host.unit_build_menu_command(worker).unwrap(), None);
    assert!(
        host.preview_unit_construction(worker, i32::from_be_bytes(*b"b001"), [80., 16.])
            .is_err()
    );
}

#[test]
fn campaign_targeting_peon_channel_band_tracks_public_construction_participation() {
    let (mut vm, worker, producer) = public_fixture_with(&[column(
        b"F002",
        SlkId::UnitBalance,
        "type",
        FieldKind::Token,
        "peon",
    )]);
    let host = &mut vm.host().inner;
    let difficulty = host
        .campaign_targeting_policy()
        .unwrap()
        .optional_level_gate;
    host.test_set_game_difficulty(i32::from(difficulty));
    let choices = [(producer, 1.), (worker, 20.)];
    assert_eq!(
        host.test_campaign_acquisition(&choices).unwrap(),
        Some(producer)
    );
    point_build(host, worker);
    assert_eq!(
        host.test_campaign_acquisition(&choices).unwrap(),
        Some(producer)
    );
    step(&mut vm, 10.);
    let host = &mut vm.host().inner;
    let site = host
        .unit_construction_order(worker)
        .unwrap()
        .unwrap()
        .site
        .unwrap();
    assert_eq!(
        host.test_campaign_acquisition(&choices).unwrap(),
        Some(worker)
    );
    host.native(
        "IssueImmediateOrderById",
        &[Value::Handle(worker), Value::Int(744)],
    )
    .unwrap();
    assert_eq!(
        host.test_campaign_acquisition(&choices).unwrap(),
        Some(producer)
    );
    host.native(
        "IssueTargetOrder",
        &[
            Value::Handle(worker),
            Value::Str("repair".into()),
            Value::Handle(site),
        ],
    )
    .unwrap();
    step(&mut vm, 10.25);
    let host = &mut vm.host().inner;
    assert_eq!(
        host.test_campaign_acquisition(&choices).unwrap(),
        Some(worker)
    );
    host.cancel_unit_construction(site).unwrap();
    assert_eq!(
        host.test_campaign_acquisition(&choices).unwrap(),
        Some(producer)
    );
}

#[test]
fn campaign_targeting_completed_construction_drops_peon_channel_priority() {
    let (mut vm, worker, producer) = public_fixture_with(&[column(
        b"F002",
        SlkId::UnitBalance,
        "type",
        FieldKind::Token,
        "peon",
    )]);
    let host = &mut vm.host().inner;
    host.test_set_game_difficulty(i32::from(
        host.campaign_targeting_policy()
            .unwrap()
            .optional_level_gate,
    ));
    point_build(host, worker);
    step(&mut vm, 20.);
    let host = &mut vm.host().inner;
    assert!(host.unit_construction_order(worker).unwrap().is_none());
    assert_eq!(
        host.test_campaign_acquisition(&[(producer, 1.), (worker, 20.)])
            .unwrap(),
        Some(producer)
    );
}

fn structures(host: &mut ObjectHost, owner: HandleId, include_incomplete: bool) -> i32 {
    let Value::Int(count) = host
        .native(
            "GetPlayerStructureCount",
            &[Value::Handle(owner), Value::Bool(include_incomplete)],
        )
        .unwrap()
    else {
        panic!("structure count")
    };
    count
}

#[test]
fn player_structure_count_observes_start_completion_owner_death_and_removal() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    assert_eq!(structures(&mut vm.host().inner, owner, false), 1);
    assert_eq!(structures(&mut vm.host().inner, owner, true), 1);
    point_build(&mut vm.host().inner, worker);
    assert_eq!(
        structures(&mut vm.host().inner, owner, true),
        1,
        "a paid reservation is not a structure before arrival"
    );
    assert_eq!(structures(&mut vm.host().inner, owner, false), 1);
    step(&mut vm, 10.);
    let host = &mut vm.host().inner;
    let site = host
        .unit_construction_order(worker)
        .unwrap()
        .unwrap()
        .site
        .unwrap();
    assert_eq!(structures(host, owner, false), 1);
    assert_eq!(structures(host, owner, true), 2);
    observe(host, site, "GetConstructingStructure");
    step(&mut vm, 20.);
    let host = &mut vm.host().inner;
    assert!(host.unit_construction(site).unwrap().is_none());
    assert_eq!(structures(host, owner, false), 2);
    assert_eq!(structures(host, owner, true), 2);
    observe(host, site, "GetConstructedStructure");
    // Presentation does not change the canonical ownership/completion count.
    host.native("ShowUnit", &[Value::Handle(site), Value::Bool(false)])
        .unwrap();
    assert_eq!(structures(host, owner, true), 2);
    let Value::Handle(other) = host.native("Player", &[Value::Int(5)]).unwrap() else {
        panic!()
    };
    host.native(
        "SetUnitOwner",
        &[
            Value::Handle(site),
            Value::Handle(other),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(structures(host, owner, true), 1);
    assert_eq!(structures(host, other, false), 1);
    host.native("KillUnit", &[Value::Handle(site)]).unwrap();
    assert_eq!(structures(host, other, true), 0);
    host.native("RemoveUnit", &[Value::Handle(site)]).unwrap();
    assert_eq!(structures(host, other, true), 0);
    assert_eq!(
        host.native("GetPlayerStructureCount", &[Value::Null, Value::Bool(true)])
            .unwrap(),
        Value::Int(0)
    );
    assert!(
        host.native(
            "GetPlayerStructureCount",
            &[Value::Handle(worker), Value::Bool(true)]
        )
        .is_err()
    );
    assert!(
        host.native(
            "GetPlayerStructureCount",
            &[Value::Handle(owner), Value::Int(1)]
        )
        .is_err()
    );
}

#[test]
fn player_structure_count_cancel_removes_incomplete_site_once() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_CANCEL");
    point_build(&mut vm.host().inner, worker);
    step(&mut vm, 10.);
    let host = &mut vm.host().inner;
    let site = host
        .unit_construction_order(worker)
        .unwrap()
        .unwrap()
        .site
        .unwrap();
    observe(host, site, "GetConstructingStructure");
    assert_eq!(structures(host, owner, false), 1);
    assert_eq!(structures(host, owner, true), 2);
    assert!(host.cancel_unit_construction(site).unwrap());
    assert_eq!(structures(host, owner, false), 1);
    assert_eq!(structures(host, owner, true), 1);
    observe(host, site, "GetCancelledStructure");
    assert_eq!(structures(host, owner, true), 1);
}

#[test]
fn player_structure_count_uses_typed_source_building_override() {
    for (building, expected) in [(0, 0), (1, 2)] {
        let (id, mut field) = column(b"F001", SlkId::UnitBalance, "isbldg", FieldKind::Bool, "0");
        field.value = Ok(FieldValue::Override(pcx_objects::Value::Int(building)));
        let (mut vm, worker, _) = fixture_with(&[(id, field)]);
        let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
        assert_eq!(structures(&mut vm.host().inner, owner, false), expected);
        assert_eq!(structures(&mut vm.host().inner, owner, true), expected);
    }
}

#[test]
fn player_structure_count_pending_stop_does_not_publish_a_site() {
    let (mut vm, worker, _) = public_fixture();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(worker).unwrap().owner;
    point_build(host, worker);
    assert_eq!(structures(host, owner, false), 1);
    assert_eq!(structures(host, owner, true), 1);
    assert_eq!(
        host.unit_construction_order(worker).unwrap().unwrap().site,
        None
    );
    host.native(
        "IssueImmediateOrderById",
        &[Value::Handle(worker), Value::Int(744)],
    )
    .unwrap();
    assert_eq!(host.unit_construction_order(worker).unwrap(), None);
    assert_eq!(structures(host, owner, false), 1);
    assert_eq!(structures(host, owner, true), 1);
}

#[test]
fn blocked_build_request_pays_mandatory_construction_before_optional_replanning() {
    let run = || {
        let (mut vm, worker, blocker) = public_fixture();
        let host = &mut vm.host().inner;
        point_build(host, worker);
        let revision = host.construction_request_revision(worker).unwrap();
        host.native(
            "SetUnitPosition",
            &[Value::Handle(blocker), Value::Real(48.), Value::Real(16.)],
        )
        .unwrap();
        host.next_timer_expiry(10., &mut 1000).unwrap();
        // Enough for mandatory movement/construction; the optional obstructed
        // route search exhausts the remainder and must defer without a fault.
        let mut work = 128;
        host.advance_unit_activity(10., &mut work).unwrap();
        assert_eq!(work, 0, "the optional search consumes the remaining work");
        let stopped = host.executed_unit(worker).unwrap().position.unwrap();
        let order = host.unit_construction_order(worker).unwrap().unwrap();
        assert_eq!(order.revision, revision);
        assert_eq!(order.site, None);
        assert!(stopped[0] > 16. && stopped[0] < 48.);
        assert!(
            host.executed_unit(worker)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .moving()
        );
        (stopped, work, host.unit_construction_order(worker).unwrap())
    };
    assert_eq!(
        run(),
        run(),
        "work consumption and pending state are deterministic"
    );
}

#[test]
fn bounded_pending_routes_preserve_partial_retries_and_finish_after_obstruction_clears() {
    let run = |allowance| {
        let (mut vm, worker, blocker) = public_fixture();
        let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
        register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
        register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
        let host = &mut vm.host().inner;
        point_build(host, worker);
        let paid = [
            bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
            bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
        ];
        let order = host.unit_construction_order(worker).unwrap();
        host.native(
            "SetUnitPosition",
            &[Value::Handle(blocker), Value::Real(48.), Value::Real(16.)],
        )
        .unwrap();
        host.next_timer_expiry(10., &mut 1000).unwrap();
        host.advance_unit_activity(10., &mut 32).unwrap();
        let stopped = host.executed_unit(worker).unwrap().position;
        let mut counts = Vec::new();
        for now in 11..15 {
            host.next_timer_expiry(f64::from(now), &mut 1000).unwrap();
            let mut work = allowance;
            host.advance_unit_activity(f64::from(now), &mut work)
                .unwrap();
            assert_eq!(host.executed_unit(worker).unwrap().position, stopped);
            assert_eq!(host.unit_construction_order(worker).unwrap(), order);
            assert!(
                host.executed_unit(worker)
                    .unwrap()
                    .activity
                    .as_ref()
                    .unwrap()
                    .moving()
            );
            assert_eq!(
                [
                    bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
                    bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER")
                ],
                paid
            );
            assert!(host.take_fired_event().is_none());
            assert_eq!(
                work, 0,
                "bounded optional work is spent, not refused by whole-segment preflight"
            );
            counts.push(allowance - work);
        }
        host.native("RemoveUnit", &[Value::Handle(blocker)])
            .unwrap();
        step(&mut vm, 25.);
        let host = &mut vm.host().inner;
        let site = host
            .unit_construction_order(worker)
            .unwrap()
            .unwrap()
            .site
            .unwrap();
        observe(host, site, "GetConstructingStructure");
        assert!(host.take_fired_event().is_none(), "START occurs once");
        step(&mut vm, 40.);
        let host = &mut vm.host().inner;
        assert!(host.unit_construction(site).unwrap().is_none());
        assert!(host.unit_construction_order(worker).unwrap().is_none());
        observe(host, site, "GetConstructedStructure");
        assert!(host.take_fired_event().is_none(), "FINISH occurs once");
        assert_eq!(
            [
                bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
                bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER")
            ],
            paid
        );
        assert_eq!(
            host.executed_units()
                .filter(|(_, r)| r.definition.rawcode == *b"b001")
                .count(),
            1
        );
        (stopped, counts, host.executed_unit(site).unwrap().position)
    };
    for allowance in [18, 20] {
        assert_eq!(
            run(allowance),
            run(allowance),
            "work and canonical completion are deterministic"
        );
    }
}

#[test]
fn construction_partial_path_end_retries_without_waiting_for_a_new_player_order() {
    let (mut vm, worker, blocker) = public_fixture();
    let host = &mut vm.host().inner;
    point_build(host, worker);
    host.native(
        "SetUnitPosition",
        &[Value::Handle(blocker), Value::Real(48.), Value::Real(16.)],
    )
    .unwrap();
    for now in 10..40 {
        host.next_timer_expiry(f64::from(now), &mut 1000).unwrap();
        host.advance_unit_activity(f64::from(now), &mut 128)
            .unwrap();
    }
    assert!(host.unit_construction_order(worker).unwrap().is_none());
    let sites: Vec<_> = host
        .executed_units()
        .filter(|(u, r)| *u != blocker && r.definition.rawcode == *b"b001")
        .collect();
    assert_eq!(
        sites.len(),
        1,
        "partial detour must be followed by another bounded route attempt"
    );
    assert!(sites[0].1.construction.is_none());
}

#[test]
fn partial_construction_retries_retire_on_manual_order_death_or_removal() {
    for action in ["stop", "move", "kill", "remove"] {
        let (mut vm, worker, blocker) = public_fixture();
        let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
        register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_START");
        let host = &mut vm.host().inner;
        let before = [
            bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
            bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
        ];
        point_build(host, worker);
        host.native(
            "SetUnitPosition",
            &[Value::Handle(blocker), Value::Real(48.), Value::Real(16.)],
        )
        .unwrap();
        host.next_timer_expiry(10., &mut 1000).unwrap();
        host.advance_unit_activity(10., &mut 32).unwrap();
        match action {
            "stop" => {
                host.native(
                    "IssueImmediateOrderById",
                    &[Value::Handle(worker), Value::Int(744)],
                )
                .unwrap();
            }
            "move" => {
                host.native(
                    "IssuePointOrder",
                    &[
                        Value::Handle(worker),
                        Value::Str("gamma".into()),
                        Value::Real(16.),
                        Value::Real(48.),
                    ],
                )
                .unwrap();
            }
            "kill" => {
                host.native("KillUnit", &[Value::Handle(worker)]).unwrap();
            }
            "remove" => {
                host.native("RemoveUnit", &[Value::Handle(worker)]).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            [
                bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
                bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER")
            ],
            before
        );
        host.native("RemoveUnit", &[Value::Handle(blocker)])
            .unwrap();
        step(&mut vm, 40.);
        let host = &mut vm.host().inner;
        assert_eq!(
            [
                bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
                bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER")
            ],
            before
        );
        assert!(
            host.executed_units()
                .all(|(_, r)| r.definition.rawcode != *b"b001")
        );
        assert!(
            host.take_fired_event().is_none(),
            "retired request must not later publish START"
        );
    }
}

#[test]
fn construction_partial_route_retry_reselects_the_authored_walk_sequence() {
    let model = crate::unit_model::tests::model();
    let walk = model
        .sequences
        .iter()
        .position(|sequence| sequence.name.eq_ignore_ascii_case("walk"))
        .expect("synthetic authored Walk");
    let (mut vm, worker, blocker) = public_fixture_configured(&[], move |mut host| {
        host.admit_unit_models(move |_| Ok(Some(Arc::clone(&model))))
            .unwrap();
        host
    });
    let host = &mut vm.host().inner;
    point_build(host, worker);
    host.native(
        "SetUnitPosition",
        &[Value::Handle(blocker), Value::Real(48.), Value::Real(16.)],
    )
    .unwrap();
    let mut resumed = false;
    for now in 10..=14 {
        let now = f64::from(now);
        host.next_timer_expiry(now, &mut 1000).unwrap();
        host.advance_unit_activity(now, &mut 128).unwrap();
        let animation = host
            .executed_unit(worker)
            .unwrap()
            .animation
            .as_ref()
            .unwrap();
        if now > 10. && animation.started == now {
            assert_eq!(animation.sequence, Some(walk));
            resumed = true;
        }
    }
    assert!(
        resumed,
        "a new nonzero route after the partial endpoint must restart authored Walk"
    );
}

const LINKED_GOLD: i32 = 31337;

struct LinkedMineCatalog(Arc<dyn ObjectCatalog + Send + Sync>);

impl ObjectCatalog for LinkedMineCatalog {
    fn resolve(&self, code: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let mut definition = if code == *b"m001" {
            let mut built = PublicBuildCatalog(Arc::clone(&self.0)).resolve(*b"b001")?;
            built.rawcode = *b"m001";
            built
        } else {
            PublicBuildCatalog(Arc::clone(&self.0)).resolve(code)?
        };
        let buff = if matches!(&code, b"b001" | b"m001") {
            "resource"
        } else {
            "_"
        };
        let (_, field) = column(b"BTYP", SlkId::UnitData, "buffType", FieldKind::Token, buff);
        definition.fields.insert(*b"BTYP", field);
        Ok(definition)
    }
}

fn linked_resource_tables() -> Arc<pcx_objects::editor::defaults::DefaultTables> {
    let data = b"ID;PWXL;N;E\r\nC;X1;Y1;K\"alias\"\r\nC;X2;Y1;K\"code\"\r\nC;X3;Y1;K\"levels\"\r\nC;X4;Y1;K\"DataA1\"\r\nC;X5;Y1;K\"DataB1\"\r\nC;X1;Y2;K\"Zfix\"\r\nC;X2;Y2;K\"Zfix\"\r\nC;X3;Y2;K\"1\"\r\nC;X4;Y2;K\"0\"\r\nC;X5;Y2;K\"0\"\r\nE\r\n";
    Arc::new(
        pcx_objects::editor::defaults::DefaultTables::from_bytes(
            pcx_objects::editor::defaults::Identity {
                install: "synthetic linked mine".into(),
                edition_version: "fixture".into(),
            },
            &[(
                pcx_objects::editor::SlkId::AbilityData.path(),
                data.as_slice(),
            )],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 1,
                max_asset_bytes: 65536,
                max_aggregate_input_bytes: 131072,
            },
        )
        .expect("synthetic resource tables"),
    )
}

fn linked_mine_fixture() -> (Vm, HandleId, HandleId) {
    let script = format!(
        "{EVENTS}\nnative SetUnitPosition takes unit u, real x, real y returns nothing\nnative SetResourceAmount takes unit whichUnit, integer amount returns nothing\nnative AddResourceAmount takes unit whichUnit, integer amount returns nothing\nnative GetResourceAmount takes unit whichUnit returns integer\nnative ShowUnit takes unit u, boolean shown returns nothing\nconstant native IsUnitHidden takes unit u returns boolean\n"
    );
    let (mut vm, worker, producer) = super::fixture_with_training_configured(
        &[
            profile_field(b"C001", "Builds", "unitList", "b001"),
            column(
                b"C002",
                SlkId::UnitData,
                "race",
                FieldKind::Token,
                "fixture",
            ),
            column(
                b"C004",
                SlkId::UnitBalance,
                "preventPlace",
                FieldKind::Token,
                "unbuildable",
            ),
            column(
                b"C005",
                SlkId::UnitBalance,
                "requirePlace",
                FieldKind::Token,
                "_",
            ),
            column(
                b"C006",
                SlkId::UnitData,
                "requireWaterRadius",
                FieldKind::Real,
                "0",
            ),
            column(b"F001", SlkId::UnitBalance, "isbldg", FieldKind::Bool, "1"),
            column(b"T006", SlkId::UnitBalance, "fused", FieldKind::Int, "0"),
            empty_ability_list(),
        ],
        8,
        0.37,
        true,
        super::super::super::super::activity::rules_with_repair(),
        true,
        (&script, |mut host| {
            let catalog = Arc::new(LinkedMineCatalog(host.unit_catalog().unwrap()));
            host.construction_test_catalog(catalog);
            host.construction_test_footprint_loader(|path| {
                assert_eq!(path, "build.tga");
                Ok(crate::unit_placement::Footprint {
                    dimensions: [2, 2],
                    origin: crate::scripts::AssetIdentity {
                        source: "synthetic build footprint".into(),
                        edition: "fixture".into(),
                        version: "nonstock".into(),
                    },
                    cells: vec![[-1, -1], [0, -1], [-1, 0], [0, 0]],
                })
            });
            host.with_resource_amounts(linked_resource_tables())
                .expect("resource amounts")
        }),
    );
    let h = &mut vm.host().inner;
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_upgrade_research(
        &context,
        crate::upgrade_research::fixture(&context.sources.catalog),
    )
    .unwrap();
    h.native("RemoveGuardPosition", &[Value::Handle(worker)])
        .unwrap();
    let _ = producer;
    (vm, worker, producer)
}

fn resource_amount(h: &mut ObjectHost, unit: HandleId) -> i32 {
    let Value::Int(amount) = h
        .native("GetResourceAmount", &[Value::Handle(unit)])
        .unwrap()
    else {
        panic!("resource amount")
    };
    amount
}

fn hidden(h: &mut ObjectHost, unit: HandleId) -> bool {
    let Value::Bool(hidden) = h.native("IsUnitHidden", &[Value::Handle(unit)]).unwrap() else {
        panic!("hidden")
    };
    hidden
}

fn create_linked_mine(vm: &mut Vm, owner: HandleId, at: [f32; 2]) -> HandleId {
    let Value::Handle(mine) = vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"m001")),
                Value::Real(at[0]),
                Value::Real(at[1]),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("mine")
    };
    vm.host()
        .inner
        .native(
            "SetResourceAmount",
            &[Value::Handle(mine), Value::Int(LINKED_GOLD)],
        )
        .unwrap();
    mine
}

#[test]
fn linked_mine_construction_hides_the_source_copies_gold_and_restores_it() {
    let (mut vm, worker, _) = linked_mine_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    let mine = create_linked_mine(&mut vm, owner, [80., 16.]);
    let kind = i32::from_be_bytes(*b"b001");
    let before = bank(&vm.host().inner, owner, "PLAYER_STATE_RESOURCE_GOLD");
    let position = vm
        .host()
        .inner
        .executed_unit(mine)
        .unwrap()
        .position
        .unwrap();
    assert!(
        vm.host()
            .inner
            .request_linked_mine_construction(worker, kind, mine)
            .unwrap()
    );
    let h = &mut vm.host().inner;
    assert!(!hidden(h, mine));
    assert_eq!(resource_amount(h, mine), LINKED_GOLD);
    assert!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD") < before);
    let order = h.unit_construction_order(worker).unwrap().unwrap();
    assert_eq!(order.position, position);
    assert!(order.site.is_none());
    let site = h
        .begin_unit_construction(worker, kind, position)
        .unwrap()
        .unwrap();
    assert!(hidden(h, mine));
    assert_eq!(resource_amount(h, mine), LINKED_GOLD);
    assert_eq!(resource_amount(h, site), LINKED_GOLD);
    assert_eq!(h.executed_unit(site).unwrap().position, Some(position));
    step(&mut vm, 9.);
    vm.host().inner.advance_unit_construction(site, 9.).unwrap();
    assert!(vm.host().inner.unit_construction(site).unwrap().is_none());
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(site)])
        .unwrap();
    let h = &mut vm.host().inner;
    assert!(!hidden(h, mine));
    assert_eq!(resource_amount(h, mine), LINKED_GOLD);
}

#[test]
fn begin_linked_mine_construction_answers_none_when_the_source_is_already_hidden() {
    let (mut vm, worker, _) = linked_mine_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    let mine = create_linked_mine(&mut vm, owner, [80., 16.]);
    let kind = i32::from_be_bytes(*b"b001");
    let position = vm
        .host()
        .inner
        .executed_unit(mine)
        .unwrap()
        .position
        .unwrap();
    assert!(
        vm.host()
            .inner
            .request_linked_mine_construction(worker, kind, mine)
            .unwrap()
    );
    vm.host()
        .inner
        .native("ShowUnit", &[Value::Handle(mine), Value::Bool(false)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .begin_unit_construction(worker, kind, position)
            .unwrap(),
        None
    );
    assert!(
        vm.host()
            .inner
            .unit_construction_order(worker)
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_pending_build_order_refuses_the_linked_mine_instead_of_replacing_it() {
    let (mut vm, worker, _) = linked_mine_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    let kind = i32::from_be_bytes(*b"b001");
    assert_eq!(
        vm.host().inner.native(
            "IssueBuildOrderById",
            &[
                Value::Handle(worker),
                Value::Int(kind),
                Value::Real(80.),
                Value::Real(16.),
            ],
        ),
        Ok(Value::Bool(true))
    );
    let mine = create_linked_mine(&mut vm, owner, [32., 48.]);
    let gold = bank(&vm.host().inner, owner, "PLAYER_STATE_RESOURCE_GOLD");
    let error = vm
        .host()
        .inner
        .request_linked_mine_construction(worker, kind, mine)
        .unwrap_err()
        .to_string();
    assert!(error.contains("admitted build order"), "{error}");
    let h = &mut vm.host().inner;
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), gold);
    assert!(!hidden(h, mine));
    assert_eq!(
        h.unit_construction_order(worker).unwrap().unwrap().position,
        [80., 16.]
    );
}

#[test]
fn set_produce_skips_a_busy_builder_and_answers_false_when_every_builder_is_busy() {
    let (mut vm, worker, producer) = linked_mine_fixture();
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(worker).unwrap().owner;
    let kind = i32::from_be_bytes(*b"b001");
    h.native("RemoveUnit", &[Value::Handle(producer)]).unwrap();
    assert_eq!(
        h.native(
            "IssueBuildOrderById",
            &[
                Value::Handle(worker),
                Value::Int(kind),
                Value::Real(80.),
                Value::Real(16.),
            ],
        ),
        Ok(Value::Bool(true))
    );
    let gold = bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD");
    let order = h.unit_construction_order(worker).unwrap();
    assert!(!h.set_ai_produce(4, 1, kind, -1).unwrap());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), gold);
    assert_eq!(h.unit_construction_order(worker).unwrap(), order);

    let rawcode = h.executed_unit(worker).unwrap().definition.rawcode;
    let Value::Handle(other) = h
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(rawcode)),
                Value::Real(48.),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("second builder")
    };
    h.native("RemoveGuardPosition", &[Value::Handle(other)])
        .unwrap();
    let error = h.set_ai_produce(4, 1, kind, -1).unwrap_err().to_string();
    assert!(
        error.contains(&format!("by {other:?}")) && error.contains("AI placement rules missing"),
        "{error}"
    );
    assert_eq!(h.unit_construction_order(worker).unwrap(), order);
    assert!(h.unit_construction_order(other).unwrap().is_none());
    assert_eq!(bank(h, owner, "PLAYER_STATE_RESOURCE_GOLD"), gold);
}

#[test]
fn a_linked_mine_keeps_the_captured_town_center() {
    let (mut vm, worker, producer) = linked_mine_fixture();
    let h = &mut vm.host().inner;
    h.native("RemoveUnit", &[Value::Handle(producer)]).unwrap();
    let owner = h.executed_unit(worker).unwrap().owner;
    let mine_at = [80., 16.];
    let mine = create_linked_mine(&mut vm, owner, mine_at);
    let center = [64., 32.];
    let h = &mut vm.host().inner;
    h.remember_town_mine(4, worker, center, mine, mine_at);
    assert_eq!(h.ai_live_town_center(4, owner).unwrap(), center);
    let kind = i32::from_be_bytes(*b"b001");
    assert!(
        h.request_linked_mine_construction(worker, kind, mine)
            .unwrap()
    );
    assert!(
        h.begin_unit_construction(worker, kind, mine_at)
            .unwrap()
            .is_some()
    );
    assert_eq!(h.ai_live_town_center(4, owner).unwrap(), center);
    let _extra = create_linked_mine(&mut vm, owner, [16., 80.]);
    let error = vm.host().inner.ai_live_town_center(4, owner).unwrap_err();
    assert!(
        error.contains("haunted-mine link is not qualified"),
        "{error}"
    );
}

#[test]
fn linked_mine_cancel_and_depletion_refuse_without_inventing_a_teardown() {
    let (mut vm, worker, _) = linked_mine_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    let mine = create_linked_mine(&mut vm, owner, [80., 16.]);
    let kind = i32::from_be_bytes(*b"b001");
    let position = vm
        .host()
        .inner
        .executed_unit(mine)
        .unwrap()
        .position
        .unwrap();
    vm.host()
        .inner
        .request_linked_mine_construction(worker, kind, mine)
        .unwrap();
    let site = vm
        .host()
        .inner
        .begin_unit_construction(worker, kind, position)
        .unwrap()
        .unwrap();
    let error = vm
        .host()
        .inner
        .cancel_unit_construction(site)
        .unwrap_err()
        .to_string();
    assert!(error.contains("cancellation"), "{error}");
    assert!(hidden(&mut vm.host().inner, mine));
    assert!(vm.host().inner.unit_construction(site).unwrap().is_some());
    step(&mut vm, 9.);
    vm.host().inner.advance_unit_construction(site, 9.).unwrap();
    vm.host()
        .inner
        .native("SetResourceAmount", &[Value::Handle(site), Value::Int(0)])
        .unwrap();
    let error = vm
        .host()
        .inner
        .native("KillUnit", &[Value::Handle(site)])
        .unwrap_err()
        .to_string();
    assert!(error.contains("depletion"), "{error}");
    let h = &mut vm.host().inner;
    assert!(hidden(h, mine));
    assert_eq!(resource_amount(h, mine), LINKED_GOLD);
    assert!(
        h.executed_unit(site)
            .unwrap()
            .combat
            .as_ref()
            .is_none_or(|combat| !combat.dead)
    );
}

#[test]
fn killed_unattended_site_has_no_completion_wake_or_finish_benefits() {
    let (mut vm, worker, _) = public_fixture_with(&[column(
        b"C002",
        SlkId::UnitData,
        "race",
        FieldKind::Token,
        "other-fixture",
    )]);
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    point_build(&mut vm.host().inner, worker);
    let site = site(&mut vm, worker);
    let host = &mut vm.host().inner;
    assert!(host.next_construction_completion(100.).is_some());
    let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    host.native("KillUnit", &[Value::Handle(site)]).unwrap();
    assert_eq!(host.next_construction_completion(100.), None);
    assert!(!host.set_construction_worker(site, Some(worker)).unwrap());
    assert!(!host.cancel_unit_construction(site).unwrap());
    step(&mut vm, 20.);
    let host = &mut vm.host().inner;
    assert_eq!(host.next_construction_completion(100.), None);
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
    assert!(host.take_fired_event().is_none());
}

#[test]
fn unattended_site_owner_transfer_still_refuses_before_mutation() {
    let (mut vm, worker, _) = public_fixture_with(&[column(
        b"C002",
        SlkId::UnitData,
        "race",
        FieldKind::Token,
        "other-fixture",
    )]);
    point_build(&mut vm.host().inner, worker);
    let site = site(&mut vm, worker);
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(site).unwrap().owner;
    let other = host.native("Player", &[Value::Int(5)]).unwrap();
    let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    let before = host.unit_construction(site).unwrap();
    assert!(
        host.native(
            "SetUnitOwner",
            &[Value::Handle(site), other, Value::Bool(false),]
        )
        .is_err()
    );
    assert_eq!(host.executed_unit(site).unwrap().owner, owner);
    assert_eq!(host.unit_construction(site).unwrap(), before);
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
    step(&mut vm, 9.);
    assert!(vm.host().inner.unit_construction(site).unwrap().is_none());
}

#[test]
fn human_site_transfer_revalidates_the_old_worker_and_resumes_paid_work() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    let Value::Handle(other) = vm.host().inner.native("Player", &[Value::Int(5)]).unwrap() else {
        panic!("new owner")
    };
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    register(&mut vm, other, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    let site = site(&mut vm, worker);
    step(&mut vm, 2.);
    let host = &mut vm.host().inner;
    let paid = [owner, other].map(|player| {
        ["PLAYER_STATE_RESOURCE_GOLD", "PLAYER_STATE_RESOURCE_LUMBER"]
            .map(|resource| bank(host, player, resource))
    });
    let before = host.unit_construction(site).unwrap().unwrap();
    let life = host.executed_unit(site).unwrap().vitals.unwrap().life;
    host.native(
        "SetUnitOwner",
        &[
            Value::Handle(site),
            Value::Handle(other),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(host.executed_unit(site).unwrap().owner, other);
    assert_eq!(host.executed_unit(site).unwrap().vitals.unwrap().life, life);
    assert_eq!(host.unit_construction(site).unwrap(), Some(before));
    assert_eq!(
        host.unit_construction_order(worker).unwrap().unwrap().site,
        Some(site)
    );
    assert_eq!(host.next_construction_completion(100.), None);
    step(&mut vm, 3.);
    let host = &mut vm.host().inner;
    assert_eq!(host.unit_construction(site).unwrap().unwrap().worker, None);
    assert_eq!(
        host.unit_construction(site).unwrap().unwrap().elapsed,
        before.elapsed
    );
    assert_eq!(host.executed_unit(site).unwrap().vitals.unwrap().life, life);
    assert!(host.unit_construction_order(worker).unwrap().is_none());
    assert_eq!(
        host.native(
            "IssueTargetOrder",
            &[
                Value::Handle(worker),
                Value::Str("repair".into()),
                Value::Handle(site)
            ]
        )
        .unwrap(),
        Value::Bool(false)
    );
    host.native(
        "SetUnitOwner",
        &[
            Value::Handle(worker),
            Value::Handle(other),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(
        host.native(
            "IssueTargetOrder",
            &[
                Value::Handle(worker),
                Value::Str("repair".into()),
                Value::Handle(site)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    for tick in 1..=6400 {
        step(&mut vm, 3. + f64::from(tick) / 64.);
        if vm
            .host()
            .inner
            .unit_construction(site)
            .unwrap()
            .unwrap()
            .worker
            == Some(worker)
        {
            break;
        }
    }
    let host = &mut vm.host().inner;
    assert_eq!(
        host.unit_construction(site).unwrap().unwrap().worker,
        Some(worker)
    );
    assert!(host.take_fired_event().is_none());
    let until = host.timer_game_time().unwrap() + before.duration;
    step(&mut vm, until);
    let host = &mut vm.host().inner;
    assert!(host.unit_construction(site).unwrap().is_none());
    let event = host.take_fired_event().expect("new owner's completion");
    host.begin_fired_event(&event).unwrap().expect("enabled");
    assert_eq!(
        host.native("GetTriggerPlayer", &[]).unwrap(),
        Value::Handle(other)
    );
    assert_eq!(
        host.native("GetConstructedStructure", &[]).unwrap(),
        Value::Handle(site)
    );
    host.finish_fired_event();
    assert!(host.take_fired_event().is_none());
    assert_eq!(
        [owner, other].map(|player| {
            ["PLAYER_STATE_RESOURCE_GOLD", "PLAYER_STATE_RESOURCE_LUMBER"]
                .map(|resource| bank(host, player, resource))
        }),
        paid
    );
}

#[test]
fn transferred_human_site_death_clears_the_old_worker_without_refund_or_finish() {
    let (mut vm, worker, _) = public_fixture();
    let owner = vm.host().inner.executed_unit(worker).unwrap().owner;
    let site = site(&mut vm, worker);
    let Value::Handle(other) = vm.host().inner.native("Player", &[Value::Int(5)]).unwrap() else {
        panic!("new owner")
    };
    register(&mut vm, other, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    let host = &mut vm.host().inner;
    let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    host.native(
        "SetUnitOwner",
        &[
            Value::Handle(site),
            Value::Handle(other),
            Value::Bool(false),
        ],
    )
    .unwrap();
    host.native("KillUnit", &[Value::Handle(site)]).unwrap();
    assert!(host.unit_construction_order(worker).unwrap().is_none());
    assert_eq!(host.next_construction_completion(100.), None);
    step(&mut vm, 20.);
    let host = &mut vm.host().inner;
    assert!(host.take_fired_event().is_none());
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
}

#[test]
fn repair_arrival_cannot_replace_an_existing_worker_or_attach_to_a_dead_site() {
    let (mut vm, worker, other) = public_fixture();
    let site = site(&mut vm, worker);
    let host = &mut vm.host().inner;
    assert!(!host.set_construction_worker(site, Some(other)).unwrap());
    assert_eq!(
        host.unit_construction(site).unwrap().unwrap().worker,
        Some(worker)
    );
    host.native("KillUnit", &[Value::Handle(site)]).unwrap();
    assert!(!host.set_construction_worker(site, Some(worker)).unwrap());
    assert_eq!(host.unit_construction(site).unwrap().unwrap().worker, None);
    assert_eq!(host.next_construction_completion(100.), None);
}

#[test]
fn worker_owner_transfer_releases_pending_cost_or_stops_paid_work() {
    for pending in [true, false] {
        let (mut vm, worker, _) = public_fixture();
        let original_owner = vm.host().inner.executed_unit(worker).unwrap().owner;
        let original_gold = bank(
            &vm.host().inner,
            original_owner,
            "PLAYER_STATE_RESOURCE_GOLD",
        );
        let site = if pending {
            None
        } else {
            Some(site(&mut vm, worker))
        };
        let host = &mut vm.host().inner;
        if pending {
            point_build(host, worker);
        }
        let owner = host.executed_unit(worker).unwrap().owner;
        let other = host.native("Player", &[Value::Int(5)]).unwrap();
        let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
        host.native(
            "SetUnitOwner",
            &[Value::Handle(worker), other, Value::Bool(false)],
        )
        .unwrap();
        assert!(host.unit_construction_order(worker).unwrap().is_none());
        assert_eq!(
            bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
            if pending { original_gold } else { paid }
        );
        if let Some(site) = site {
            let state = host.unit_construction(site).unwrap().unwrap();
            assert_eq!(state.worker, None);
            step(&mut vm, 20.);
            assert_eq!(
                vm.host()
                    .inner
                    .unit_construction(site)
                    .unwrap()
                    .unwrap()
                    .elapsed,
                state.elapsed
            );
        } else {
            step(&mut vm, 20.);
            assert!(
                vm.host()
                    .inner
                    .executed_units()
                    .all(|(_, record)| record.construction.is_none())
            );
        }
    }
}

#[test]
fn competing_public_repair_arrivals_keep_the_first_participant_and_spend_once() {
    let (mut vm, worker, _) = public_fixture();
    let site = site(&mut vm, worker);
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(worker).unwrap().owner;
    assert!(host.set_construction_worker(site, None).unwrap());
    let Value::Handle(other) = host
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(16.),
                Value::Real(80.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("second worker")
    };
    host.native("RemoveGuardPosition", &[Value::Handle(other)])
        .unwrap();
    let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    for candidate in [worker, other] {
        assert_eq!(
            host.native(
                "IssueTargetOrder",
                &[
                    Value::Handle(candidate),
                    Value::Str("repair".into()),
                    Value::Handle(site),
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    for tick in 1..=6400 {
        step(&mut vm, f64::from(tick) / 64.);
        if vm
            .host()
            .inner
            .unit_construction_order(other)
            .unwrap()
            .is_none()
        {
            break;
        }
    }
    let host = &mut vm.host().inner;
    assert_eq!(
        host.unit_construction(site).unwrap().unwrap().worker,
        Some(worker)
    );
    assert!(host.unit_construction_order(other).unwrap().is_none());
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
    let until =
        host.timer_game_time().unwrap() + host.unit_construction(site).unwrap().unwrap().duration;
    step(&mut vm, until);
    let host = &mut vm.host().inner;
    assert!(host.unit_construction(site).unwrap().is_none());
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
}

fn assistant_fixture(
    ratio: &str,
    rate: &str,
    overlay: pcx_objects::editor::Overlay,
) -> (Vm, HandleId, HandleId) {
    assistant_fixture_configured(ratio, rate, overlay, |host| host)
}

fn assistant_fixture_configured(
    ratio: &str,
    rate: &str,
    overlay: pcx_objects::editor::Overlay,
    configure: impl FnOnce(ObjectHost) -> ObjectHost,
) -> (Vm, HandleId, HandleId) {
    struct AssistantCatalog(Arc<dyn ObjectCatalog + Send + Sync>);
    impl ObjectCatalog for AssistantCatalog {
        fn resolve(&self, code: [u8; 4]) -> Result<Definition, ResolutionProblem> {
            let mut definition = self
                .0
                .resolve(if code == *b"w001" { *b"u001" } else { code })?;
            if code == *b"w001" {
                definition.rawcode = code;
                definition.fields.get_mut(b"uabi").unwrap().value =
                    Ok(FieldValue::Cell("R008".into()));
            }
            Ok(definition)
        }
    }
    fn table(headers: &[&str], rows: &[Vec<String>]) -> Vec<u8> {
        let mut text = String::from("ID;PWXL;N;E\r\n");
        for (y, row) in std::iter::once(headers.iter().map(|cell| (*cell).to_owned()).collect())
            .chain(rows.iter().cloned())
            .enumerate()
        {
            for (x, cell) in row.iter().enumerate() {
                text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, cell));
            }
        }
        text.push_str("E\r\n");
        text.into_bytes()
    }
    let data = table(
        &[
            "alias", "code", "levels", "race", "DataA1", "DataB1", "DataC1", "DataD1",
        ],
        &[
            ["ZYX9", "ZYX9", "1", "fixture", "0", "0", "0", "0"]
                .map(str::to_owned)
                .to_vec(),
            ["ZYX8", "ZYX8", "1", "other-fixture", "0", "0", "0", "0"]
                .map(str::to_owned)
                .to_vec(),
            ["R007", "ZYX7", "1", "fixture", ".11", "3.25", ratio, rate]
                .map(str::to_owned)
                .to_vec(),
            ["R008", "ZYX7", "1", "fixture", ".11", "3.25", "0", ".75"]
                .map(str::to_owned)
                .to_vec(),
        ],
    );
    let metadata = table(
        &["ID", "field", "slk", "index", "type", "data", "category"],
        &[
            ["alev", "levels", "AbilityData", "-1", "int", "0", "data"]
                .map(str::to_owned)
                .to_vec(),
            ["Rep3", "Data", "AbilityData", "-1", "unreal", "3", "data"]
                .map(str::to_owned)
                .to_vec(),
            ["Rep4", "Data", "AbilityData", "-1", "unreal", "4", "data"]
                .map(str::to_owned)
                .to_vec(),
        ],
    );
    let mut abilities = empty_ability_list();
    abilities.1.value = Ok(FieldValue::Cell("R007".into()));
    public_fixture_admitted(
        &[
            abilities,
            column(b"T005", SlkId::UnitBalance, "bldtm", FieldKind::Int, "37"),
            column(b"REPT", SlkId::UnitBalance, "reptm", FieldKind::Int, "83"),
        ],
        false,
        |mut host| {
            host.construction_test_catalog(Arc::new(AssistantCatalog(
                host.unit_catalog().unwrap(),
            )));
            let origin = crate::scripts::AssetIdentity {
                source: super::super::super::super::source().install,
                edition: "fixture".into(),
                version: "assistant nonstock".into(),
            };
            let tables = pcx_objects::editor::defaults::DefaultTables::from_bytes(
                pcx_objects::editor::defaults::Identity {
                    install: origin.source.clone(),
                    edition_version: origin.version.clone(),
                },
                &[
                    (SlkId::AbilityData.path(), &data),
                    (SlkId::AbilityMetaData.path(), &metadata),
                ],
                &[],
                pcx_objects::editor::EditorReadLimits {
                    max_listed_paths: 2,
                    max_asset_bytes: 16384,
                    max_aggregate_input_bytes: 32768,
                },
            )
            .unwrap();
            let (rules, _) = super::super::super::hero_rules();
            configure(
                host.with_hero_skills(
                    tables,
                    origin,
                    overlay,
                    rules,
                    crate::playercolor_host::HeroBudget {
                        abilities_per_unit: 8,
                        overlay_objects: 4,
                        modifications_per_ability: 8,
                    },
                )
                .unwrap(),
            )
        },
    )
}

fn assistant_worker(host: &mut ObjectHost, owner: HandleId) -> HandleId {
    let Value::Handle(worker) = host
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(16.),
                Value::Real(80.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("assistant")
    };
    host.native("RemoveGuardPosition", &[Value::Handle(worker)])
        .unwrap();
    worker
}

fn publish_construction_session(
    vm: &mut Vm,
) -> crate::map_script_session::MapScriptSession<crate::map_startup_host::MapStartupHost> {
    use crate::map_script_session::MapScriptSession;
    use crate::map_startup_host::MapStartupHost;
    use crate::neutral_player::{SelectedMapNeutralHost, tests as neutral};
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    let placeholder = ObjectHost::checked(
        EnumHost::checked_selected(
            context.scripts.shared_program(),
            &context.map_digest.label,
            &[],
            Limits {
                max_handles: 256,
                admission_steps: 1000,
                expression_depth: 64,
                expression_work: 4096,
            },
        )
        .unwrap(),
    )
    .unwrap();
    let host = std::mem::replace(host, placeholder);
    let world =
        SelectedMapNeutralHost::try_new(context.clone(), neutral::slots_rule(301, -3, 9), host)
            .unwrap();
    let world =
        SelectedMapNeutralHost::try_new(context.clone(), neutral::extra_rule(301, -3, 9), world)
            .unwrap();
    let world =
        SelectedMapNeutralHost::try_new(context.clone(), neutral::victim_rule(301, -3, 9), world)
            .unwrap();
    let world = SelectedMapNeutralHost::try_new(
        context.clone(),
        neutral::bj_max_players_rule(301, -3, 9),
        world,
    )
    .unwrap();
    let world =
        SelectedMapNeutralHost::try_new(context.clone(), neutral::aggressive_rules(301), world)
            .unwrap();
    let world = SelectedMapNeutralHost::try_new(context.clone(), neutral::rules(), world).unwrap();
    let mut session = MapScriptSession::<MapStartupHost>::default();
    session
        .replace(
            context,
            |_| Ok::<_, std::convert::Infallible>(MapStartupHost::timer_session_fixture(world)),
            &[],
        )
        .unwrap();
    session
}

#[test]
fn published_session_finishes_fractional_free_assistance_at_large_clocks() {
    for start in [10000., 1_099_511_627_776.] {
        let (mut vm, primary, _) = assistant_fixture("0", ".3", Default::default());
        let owner = vm.host().inner.executed_unit(primary).unwrap().owner;
        register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
        assert!(
            vm.host()
                .inner
                .next_timer_expiry(start, &mut 8)
                .unwrap()
                .is_none()
        );
        let site = site(&mut vm, primary);
        let host = &mut vm.host().inner;
        let helper = assistant_worker(host, owner);
        assert!(host.set_construction_worker(site, Some(helper)).unwrap());
        let paid_bank = [
            bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
            bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
        ];
        let mut session = publish_construction_session(&mut vm);
        assert_eq!(session.advance_timers(start + 40., 8).unwrap(), 0);
        assert_eq!(session.advance_queued_events(1).unwrap(), 1);
        assert_eq!(session.advance_timers(start + 41., 8).unwrap(), 0);
        assert_eq!(session.advance_queued_events(1).unwrap(), 0);
        let objects = session.host().unwrap().objects_mut();
        assert_eq!(objects.timer_game_time(), Some(start + 41.));
        assert_eq!(
            [
                bank(objects, owner, "PLAYER_STATE_RESOURCE_GOLD"),
                bank(objects, owner, "PLAYER_STATE_RESOURCE_LUMBER")
            ],
            paid_bank
        );
        assert!(objects.unit_construction(site).unwrap().is_none());
        for worker in [primary, helper] {
            assert!(objects.unit_construction_order(worker).unwrap().is_none());
        }
    }
}

#[test]
fn public_repair_enrolls_multiple_assistants_and_completion_clears_every_order() {
    let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
    let owner = vm.host().inner.executed_unit(primary).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    let site = site(&mut vm, primary);
    let host = &mut vm.host().inner;
    let helpers = [assistant_worker(host, owner), assistant_worker(host, owner)];
    let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    for helper in helpers {
        assert_eq!(
            host.native(
                "IssueTargetOrder",
                &[
                    Value::Handle(helper),
                    Value::Str("repair".into()),
                    Value::Handle(site)
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    for tick in 1..=6400 {
        step(&mut vm, f64::from(tick) / 64.);
        let host = &vm.host().inner;
        if helpers.iter().all(|helper| {
            host.executed_unit(*helper)
                .unwrap()
                .construction_request
                .is_none()
        }) {
            break;
        }
    }
    let host = &mut vm.host().inner;
    assert_eq!(
        host.unit_construction(site).unwrap().unwrap().worker,
        Some(primary)
    );
    for helper in helpers {
        assert_eq!(
            host.unit_construction_order(helper).unwrap().unwrap().site,
            Some(site)
        );
    }
    let now = host.timer_game_time().unwrap();
    let elapsed = host.unit_construction(site).unwrap().unwrap().elapsed;
    step(&mut vm, now + 1.);
    let host = &mut vm.host().inner;
    assert!((host.unit_construction(site).unwrap().unwrap().elapsed - elapsed - 2.).abs() < 0.0001);
    assert!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD") < paid);
    step(&mut vm, now + 40.);
    let host = &mut vm.host().inner;
    assert!(host.unit_construction(site).unwrap().is_none());
    for worker in [primary, helpers[0], helpers[1]] {
        assert!(host.unit_construction_order(worker).unwrap().is_none());
    }
    observe(host, site, "GetConstructedStructure");
    assert!(host.take_fired_event().is_none());
}

#[test]
fn assistant_stop_death_removal_and_transfer_remove_only_the_departing_contribution() {
    for action in [
        "IssueImmediateOrderById",
        "KillUnit",
        "RemoveUnit",
        "SetUnitOwner",
    ] {
        let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
        let site = site(&mut vm, primary);
        let host = &mut vm.host().inner;
        let owner = host.executed_unit(primary).unwrap().owner;
        let a = assistant_worker(host, owner);
        let b = assistant_worker(host, owner);
        assert!(host.set_construction_worker(site, Some(a)).unwrap());
        assert!(host.set_construction_worker(site, Some(b)).unwrap());
        let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
        let args = match action {
            "IssueImmediateOrderById" => vec![Value::Handle(a), Value::Int(744)],
            "SetUnitOwner" => vec![
                Value::Handle(a),
                host.native("Player", &[Value::Int(5)]).unwrap(),
                Value::Bool(false),
            ],
            _ => vec![Value::Handle(a)],
        };
        host.native(action, &args).unwrap();
        assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
        assert_eq!(
            host.unit_construction(site).unwrap().unwrap().worker,
            Some(primary)
        );
        assert_eq!(
            host.unit_construction_order(b).unwrap().unwrap().site,
            Some(site)
        );
        step(&mut vm, 1.);
        let host = &mut vm.host().inner;
        assert!(
            (host.unit_construction(site).unwrap().unwrap().elapsed - 1.5).abs() < 0.0001,
            "{action}"
        );
        assert_eq!(
            host.unit_construction_order(b).unwrap().unwrap().site,
            Some(site)
        );
    }
}

#[test]
fn surviving_assistant_reclaims_a_dead_primary_without_repaying_construction() {
    let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
    let site = site(&mut vm, primary);
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(primary).unwrap().owner;
    let helper = assistant_worker(host, owner);
    assert!(host.set_construction_worker(site, Some(helper)).unwrap());
    let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    host.native("KillUnit", &[Value::Handle(primary)]).unwrap();
    assert_eq!(
        host.unit_construction_order(helper).unwrap().unwrap().site,
        Some(site)
    );
    step(&mut vm, 1.);
    let host = &mut vm.host().inner;
    assert_eq!(
        host.unit_construction(site).unwrap().unwrap().worker,
        Some(helper)
    );
    assert_eq!(host.unit_construction(site).unwrap().unwrap().elapsed, 1.);
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
    host.native(
        "IssueImmediateOrderById",
        &[Value::Handle(helper), Value::Int(744)],
    )
    .unwrap();
    step(&mut vm, 3.);
    let host = &mut vm.host().inner;
    assert_eq!(host.unit_construction(site).unwrap().unwrap().elapsed, 1.);
    assert!(host.unit_construction_order(helper).unwrap().is_none());
}

#[test]
fn depleted_banks_stop_assistants_and_restoring_funding_does_not_reenroll_them() {
    let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
    let site = site(&mut vm, primary);
    let owner = vm.host().inner.executed_unit(primary).unwrap().owner;
    let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
    let wood = vm.global("PLAYER_STATE_RESOURCE_LUMBER");
    let host = &mut vm.host().inner;
    let helper = assistant_worker(host, owner);
    assert!(host.set_construction_worker(site, Some(helper)).unwrap());
    for resource in [gold.clone(), wood.clone()] {
        host.native(
            "SetPlayerState",
            &[Value::Handle(owner), resource, Value::Int(0)],
        )
        .unwrap();
    }
    step(&mut vm, 1.);
    let host = &mut vm.host().inner;
    assert_eq!(host.unit_construction(site).unwrap().unwrap().elapsed, 1.);
    assert!(host.unit_construction_order(helper).unwrap().is_none());
    for resource in [gold, wood] {
        host.native(
            "SetPlayerState",
            &[Value::Handle(owner), resource, Value::Int(1000)],
        )
        .unwrap();
    }
    step(&mut vm, 2.);
    let host = &mut vm.host().inner;
    assert_eq!(host.unit_construction(site).unwrap().unwrap().elapsed, 2.);
    assert!(host.unit_construction_order(helper).unwrap().is_none());
}

#[test]
fn pooled_funding_exhaustion_preserves_an_authored_zero_cost_assistant() {
    let (mut vm, primary, _) = assistant_fixture("1", ".5", Default::default());
    let site = site(&mut vm, primary);
    let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
    let wood = vm.global("PLAYER_STATE_RESOURCE_LUMBER");
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(primary).unwrap().owner;
    let paid = assistant_worker(host, owner);
    let Value::Handle(free) = host
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"w001")),
                Value::Real(16.),
                Value::Real(80.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("zero-cost assistant")
    };
    host.native("RemoveGuardPosition", &[Value::Handle(free)])
        .unwrap();
    assert!(host.set_construction_worker(site, Some(paid)).unwrap());
    assert!(host.set_construction_worker(site, Some(free)).unwrap());
    for resource in [gold, wood] {
        host.native(
            "SetPlayerState",
            &[Value::Handle(owner), resource, Value::Int(3)],
        )
        .unwrap();
    }
    let (image, binding, budget) = crate::script_numeric::tests::fixture();
    let numeric = crate::script_numeric::NumericPolicy::read(&image, binding, budget, 64).unwrap();
    let Value::Real(duration) = numeric
        .binary(pcx_script::ast::BinOp::Div, 37., 2.25)
        .unwrap()
    else {
        panic!()
    };
    let Value::Real(period) = numeric
        .binary(pcx_script::ast::BinOp::Div, duration, 41.)
        .unwrap()
    else {
        panic!()
    };
    let until = host.next_construction_completion(100.).unwrap() + f64::from(period * 3.) + 0.01;
    assert!(host.next_timer_expiry(until, &mut 100).unwrap().is_none());
    host.advance_unit_construction(site, until).unwrap();
    assert!(host.unit_construction_order(paid).unwrap().is_none());
    assert_eq!(
        host.unit_construction_order(free).unwrap().unwrap().site,
        Some(site)
    );
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), 0);
    assert!(bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER") >= 0);
}

#[test]
fn source_powerbuild_profiles_and_overrides_control_rates_and_integer_cost_deadlines() {
    let mut overlay = pcx_objects::editor::Overlay::default();
    overlay.mods.insert(
        "R007".into(),
        vec![pcx_objects::Mod {
            field: *b"Rep4",
            value_type: pcx_objects::ValueType::Unreal,
            selectors: Some([1, 4]),
            value: pcx_objects::Value::Real(1.25),
            end_word: 0,
        }],
    );
    let (mut vm, primary, _) = assistant_fixture(".25", ".5", overlay);
    let site = site(&mut vm, primary);
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(primary).unwrap().owner;
    let helper = assistant_worker(host, owner);
    let before = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    assert!(host.set_construction_worker(site, Some(helper)).unwrap());
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), before - 3);
    step(&mut vm, 1.);
    let host = &mut vm.host().inner;
    assert!((host.unit_construction(site).unwrap().unwrap().elapsed - 2.25).abs() < 0.0001);
    // bldtm=37, reptm=83 and repair DataB=3.25 are deliberately distinct.
    // This fixture deliberately supplies a nonstock reciprocal table, rather than IEEE division.
    let (image, binding, budget) = crate::script_numeric::tests::fixture();
    let numeric = crate::script_numeric::NumericPolicy::read(&image, binding, budget, 64).unwrap();
    let Value::Real(duration) = numeric
        .binary(pcx_script::ast::BinOp::Div, 37., 2.25)
        .unwrap()
    else {
        panic!("source division")
    };
    let Value::Real(period) = numeric
        .binary(pcx_script::ast::BinOp::Div, duration, 41. * 0.25)
        .unwrap()
    else {
        panic!("source division")
    };
    let Value::Real(period) = numeric
        .binary(pcx_script::ast::BinOp::Mul, period, 3.)
        .unwrap()
    else {
        panic!("source debit")
    };
    let period = f64::from(period);
    assert!(
        (host.next_construction_completion(100.).unwrap() - period).abs() < 0.001,
        "next={:?}, duration={:?}",
        host.next_construction_completion(100.),
        host.unit_construction(site).unwrap()
    );
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), before - 3);
    step(&mut vm, period + 0.01);
    assert_eq!(
        bank(&vm.host().inner, owner, "PLAYER_STATE_RESOURCE_GOLD"),
        before - 6
    );
}

#[test]
fn malformed_assistant_profiles_refuse_before_order_or_resource_changes() {
    for (ratio, rate) in [("broken", ".5"), (".25", "nan"), ("-.1", ".5"), ("_", ".5")] {
        let (mut vm, primary, _) = assistant_fixture(ratio, rate, Default::default());
        let site = site(&mut vm, primary);
        let host = &mut vm.host().inner;
        let owner = host.executed_unit(primary).unwrap().owner;
        let helper = assistant_worker(host, owner);
        let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
        let revision = host.construction_request_revision(helper).unwrap();
        if ratio == "_" {
            assert_eq!(
                host.hero_ability_level_cell_including_marker(
                    *b"R007",
                    "DataC",
                    1,
                    FieldKind::Unreal
                )
                .unwrap(),
                "_"
            );
            assert!(
                host.hero_ability_level_cell(*b"R007", "DataC", 1, FieldKind::Unreal)
                    .is_err()
            );
        }
        assert!(
            host.native(
                "IssueTargetOrder",
                &[
                    Value::Handle(helper),
                    Value::Str("repair".into()),
                    Value::Handle(site)
                ]
            )
            .is_err()
        );
        assert_eq!(
            host.construction_request_revision(helper).unwrap(),
            revision
        );
        assert!(host.unit_construction_order(helper).unwrap().is_none());
        assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
    }
}

#[test]
fn ranked_reader_preserves_rank_validation_before_missing_source() {
    let (mut vm, _, _) = fixture();
    let host = &vm.host().inner;
    for rank in [0, -1] {
        assert_eq!(
            host.hero_ability_level_cell(*b"R007", "DataC", rank, FieldKind::Unreal)
                .unwrap_err(),
            "ability rank must be positive"
        );
        assert_eq!(
            host.hero_ability_level_cell_including_marker(
                *b"R007",
                "DataC",
                rank,
                FieldKind::Unreal
            )
            .unwrap_err(),
            "ability rank must be positive"
        );
        assert_eq!(
            host.unit_ability_level_cell(*b"R007", "DataC", rank, FieldKind::Unreal)
                .unwrap_err(),
            "ability rank must be positive"
        );
    }
}

#[test]
fn transferring_an_assisted_site_revalidates_all_old_owner_participants_without_refund() {
    let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
    let site = site(&mut vm, primary);
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(primary).unwrap().owner;
    let helpers = [assistant_worker(host, owner), assistant_worker(host, owner)];
    for helper in helpers {
        assert!(host.set_construction_worker(site, Some(helper)).unwrap());
    }
    step(&mut vm, 1.);
    let host = &mut vm.host().inner;
    let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    let life = host.executed_unit(site).unwrap().vitals.unwrap().life;
    let other = host.native("Player", &[Value::Int(5)]).unwrap();
    host.native(
        "SetUnitOwner",
        &[Value::Handle(site), other, Value::Bool(false)],
    )
    .unwrap();
    for worker in [primary, helpers[0], helpers[1]] {
        assert_eq!(
            host.unit_construction_order(worker).unwrap().unwrap().site,
            Some(site)
        );
    }
    assert_eq!(host.executed_unit(site).unwrap().vitals.unwrap().life, life);
    step(&mut vm, 2.);
    let host = &mut vm.host().inner;
    for worker in [primary, helpers[0], helpers[1]] {
        assert!(host.unit_construction_order(worker).unwrap().is_none());
    }
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
    assert_eq!(host.next_construction_completion(100.), None);
    assert_eq!(host.executed_unit(site).unwrap().vitals.unwrap().life, life);
}

#[test]
fn assisted_site_death_clears_enrolled_and_arriving_helpers_and_all_charge_wakes() {
    let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
    let owner = vm.host().inner.executed_unit(primary).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    let site = site(&mut vm, primary);
    let host = &mut vm.host().inner;
    let enrolled = assistant_worker(host, owner);
    let arriving = assistant_worker(host, owner);
    assert!(host.set_construction_worker(site, Some(enrolled)).unwrap());
    assert_eq!(
        host.native(
            "IssueTargetOrder",
            &[
                Value::Handle(arriving),
                Value::Str("repair".into()),
                Value::Handle(site)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert!(
        host.executed_unit(arriving)
            .unwrap()
            .construction_request
            .is_some()
    );
    let paid = bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD");
    host.native("KillUnit", &[Value::Handle(site)]).unwrap();
    assert_eq!(host.next_construction_completion(100.), None);
    for worker in [primary, enrolled, arriving] {
        assert!(host.unit_construction_order(worker).unwrap().is_none());
    }
    assert!(
        !host
            .executed_unit(arriving)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    step(&mut vm, 100.);
    let host = &mut vm.host().inner;
    assert!(host.take_fired_event().is_none());
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), paid);
}

#[test]
fn membership_stop_debits_only_distant_resource_and_preserves_near_deadline_in_session() {
    let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
    let owner = vm.host().inner.executed_unit(primary).unwrap().owner;
    register(&mut vm, owner, "EVENT_PLAYER_UNIT_CONSTRUCT_FINISH");
    let site = site(&mut vm, primary);
    let host = &mut vm.host().inner;
    let helpers = [assistant_worker(host, owner), assistant_worker(host, owner)];
    for worker in helpers {
        assert!(host.set_construction_worker(site, Some(worker)).unwrap());
    }
    let mut session = publish_construction_session(&mut vm);
    assert_eq!(session.advance_timers(0.25, 16).unwrap(), 0);
    let host = session.host().unwrap().objects_mut();
    let before = host.construction_test_assistant_deadlines(site);
    assert!(
        before[0].unwrap() - 0.25 <= 7. && before[1].unwrap() - 0.25 > 7.,
        "{before:?}"
    );
    let banks = [
        bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
        bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
    ];
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(helpers[0]), Value::Int(744)]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), banks[0]);
    assert_eq!(
        bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
        banks[1] - 3
    );
    let after = host.construction_test_assistant_deadlines(site);
    assert_eq!(after[0], before[0]);
    assert!(after[1].unwrap() > 0.25 && after[1] != before[1]);
    assert!(host.unit_construction_order(helpers[0]).unwrap().is_none());
    assert_eq!(session.advance_timers(80., 256).unwrap(), 0);
    assert_eq!(session.advance_queued_events(1).unwrap(), 1);
    assert_eq!(session.advance_queued_events(1).unwrap(), 0);
    let host = session.host().unwrap().objects_mut();
    assert!(host.unit_construction(site).unwrap().is_none());
    for worker in [primary, helpers[1]] {
        assert!(host.unit_construction_order(worker).unwrap().is_none());
    }
}

fn producer_assistant_fixture() -> (Vm, HandleId, HandleId, HandleId, HandleId) {
    struct QueueCostCatalog(Arc<dyn ObjectCatalog + Send + Sync>);
    impl ObjectCatalog for QueueCostCatalog {
        fn resolve(&self, code: [u8; 4]) -> Result<Definition, ResolutionProblem> {
            let mut definition = self.0.resolve(code)?;
            if code == *b"u001" {
                definition.fields.get_mut(b"T004").unwrap().value =
                    Ok(FieldValue::Cell("9".into()));
            }
            Ok(definition)
        }
    }
    let (mut vm, primary, _) =
        assistant_fixture_configured(".25", ".5", Default::default(), |mut host| {
            host.construction_test_catalog(Arc::new(QueueCostCatalog(
                host.unit_catalog().unwrap(),
            )));
            host.with_player_state_events(8).unwrap()
        });
    let site = site(&mut vm, primary);
    let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
    let wood = vm.global("PLAYER_STATE_RESOURCE_LUMBER");
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(primary).unwrap().owner;
    let a = assistant_worker(host, owner);
    let b = assistant_worker(host, owner);
    for worker in [a, b] {
        assert!(host.set_construction_worker(site, Some(worker)).unwrap());
    }
    assert_eq!(
        host.native(
            "IssueImmediateOrderById",
            &[Value::Handle(a), Value::Int(i32::from_be_bytes(*b"u001"))]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(host.unit_training_queue(a).unwrap().len(), 1);
    assert!(host.unit_construction_order(a).unwrap().is_some());
    for state in [gold, wood] {
        host.native(
            "SetPlayerState",
            &[Value::Handle(owner), state, Value::Int(20)],
        )
        .unwrap();
    }
    (vm, primary, site, a, b)
}

#[test]
fn producer_assistant_loss_composes_refund_debit_food_and_projected_event_snapshots() {
    for action in ["KillUnit", "RemoveUnit"] {
        let (mut vm, primary, site, a, b) = producer_assistant_fixture();
        let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
        let wood = vm.global("PLAYER_STATE_RESOURCE_LUMBER");
        let op = vm.global("GREATER_THAN");
        let host = &mut vm.host().inner;
        let owner = host.executed_unit(primary).unwrap().owner;
        let food = bank(host, owner, "PLAYER_STATE_RESOURCE_FOOD_USED");
        for state in [gold.clone(), wood.clone()] {
            let trigger = host.native("CreateTrigger", &[]).unwrap();
            host.native(
                "TriggerRegisterPlayerStateEvent",
                &[
                    trigger,
                    Value::Handle(owner),
                    state,
                    op.clone(),
                    Value::Real(0.),
                ],
            )
            .unwrap();
        }
        host.native(action, &[Value::Handle(a)]).unwrap();
        assert_eq!(
            bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
            35,
            "{action}"
        );
        assert_eq!(
            bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
            20,
            "refund3 then debit3: {action}"
        );
        assert_eq!(
            bank(host, owner, "PLAYER_STATE_RESOURCE_FOOD_USED"),
            food - 3,
            "{action}"
        );
        assert!(host.unit_training_queue(a).unwrap().is_empty());
        assert_eq!(
            host.unit_construction(site).unwrap().unwrap().worker,
            Some(primary)
        );
        assert_eq!(
            host.unit_construction_order(b).unwrap().unwrap().site,
            Some(site)
        );
        let mut events = Vec::new();
        while let Some(event) = host.take_fired_event() {
            host.begin_fired_event(&event).unwrap().unwrap();
            events.push(host.native("GetEventPlayerState", &[]).unwrap());
            host.finish_fired_event();
        }
        assert_eq!(events, vec![gold, wood.clone(), wood], "{action}");
    }
}

#[test]
fn projected_event_reservation_overflow_keeps_banks_orders_liveness_and_snapshots() {
    let (mut vm, primary, site, a, _) = producer_assistant_fixture();
    let wood = vm.global("PLAYER_STATE_RESOURCE_LUMBER");
    let op = vm.global("GREATER_THAN");
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(primary).unwrap().owner;
    let trigger = host.native("CreateTrigger", &[]).unwrap();
    host.native(
        "TriggerRegisterPlayerStateEvent",
        &[
            trigger,
            Value::Handle(owner),
            wood.clone(),
            op,
            Value::Real(0.),
        ],
    )
    .unwrap();
    let Value::Handle(state) = wood else { panic!() };
    assert!(
        host.construction_test_refund_event_overflow(a, owner)
            .is_err()
    );
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), 20);
    assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 20);
    assert!(!host.executed_unit(a).unwrap().combat.as_ref().unwrap().dead);
    assert_eq!(host.unit_training_queue(a).unwrap().len(), 1);
    assert_eq!(
        host.unit_construction_order(a).unwrap().unwrap().site,
        Some(site)
    );
    assert!(host.take_fired_event().is_none());
    // A later ordinary change still sees the original registration snapshot.
    host.native(
        "SetPlayerState",
        &[Value::Handle(owner), Value::Handle(state), Value::Int(21)],
    )
    .unwrap();
    assert!(host.take_fired_event().is_some());
}

#[test]
fn membership_boundary_at_source_horizon_retains_both_pending_debits() {
    let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
    let site = site(&mut vm, primary);
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(primary).unwrap().owner;
    let a = assistant_worker(host, owner);
    let b = assistant_worker(host, owner);
    for worker in [a, b] {
        assert!(host.set_construction_worker(site, Some(worker)).unwrap());
    }
    let now = host.construction_test_assistant_deadlines(site)[1].unwrap() - 7.;
    assert!(now > 0.);
    step(&mut vm, now);
    let host = &mut vm.host().inner;
    let before = host.construction_test_assistant_deadlines(site);
    assert_eq!(before[1].unwrap() - now, 7.);
    let banks = [
        bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
        bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
    ];
    host.native(
        "IssueImmediateOrderById",
        &[Value::Handle(a), Value::Int(744)],
    )
    .unwrap();
    assert_eq!(host.construction_test_assistant_deadlines(site), before);
    assert_eq!(
        [
            bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
            bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER")
        ],
        banks
    );
}

#[test]
fn unaffordable_member_loss_stops_paid_cohort_preserves_free_work_without_refunds() {
    for action in [
        "IssueImmediateOrderById",
        "KillUnit",
        "RemoveUnit",
        "SetUnitOwner",
    ] {
        let (mut vm, primary, _) = assistant_fixture(".25", ".5", Default::default());
        let site = site(&mut vm, primary);
        let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
        let wood = vm.global("PLAYER_STATE_RESOURCE_LUMBER");
        let host = &mut vm.host().inner;
        let owner = host.executed_unit(primary).unwrap().owner;
        let a = assistant_worker(host, owner);
        let b = assistant_worker(host, owner);
        let Value::Handle(free) = host
            .native(
                "CreateUnit",
                &[
                    Value::Handle(owner),
                    Value::Int(i32::from_be_bytes(*b"w001")),
                    Value::Real(16.),
                    Value::Real(80.),
                    Value::Real(0.),
                ],
            )
            .unwrap()
        else {
            panic!()
        };
        host.native("RemoveGuardPosition", &[Value::Handle(free)])
            .unwrap();
        for worker in [a, b, free] {
            assert!(host.set_construction_worker(site, Some(worker)).unwrap());
        }
        for state in [gold, wood] {
            host.native(
                "SetPlayerState",
                &[Value::Handle(owner), state, Value::Int(0)],
            )
            .unwrap();
        }
        let args = match action {
            "IssueImmediateOrderById" => vec![Value::Handle(a), Value::Int(744)],
            "SetUnitOwner" => vec![
                Value::Handle(a),
                host.native("Player", &[Value::Int(5)]).unwrap(),
                Value::Bool(false),
            ],
            _ => vec![Value::Handle(a)],
        };
        host.native(action, &args).unwrap();
        assert_eq!(
            bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"),
            0,
            "{action}"
        );
        assert_eq!(
            bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"),
            0,
            "{action}"
        );
        assert!(
            host.unit_construction_order(b).unwrap().is_none(),
            "{action}"
        );
        assert_eq!(
            host.unit_construction_order(free).unwrap().unwrap().site,
            Some(site)
        );
        assert_eq!(
            host.unit_construction(site).unwrap().unwrap().worker,
            Some(primary)
        );
        step(&mut vm, 1.);
        assert_eq!(
            vm.host()
                .inner
                .unit_construction(site)
                .unwrap()
                .unwrap()
                .elapsed,
            1.75,
            "{action}"
        );
    }
}

#[test]
fn primary_only_loss_has_no_assistant_charge_or_resource_requirement() {
    for action in [
        "IssueImmediateOrderById",
        "KillUnit",
        "RemoveUnit",
        "SetUnitOwner",
    ] {
        let (mut vm, primary, _) = assistant_fixture("0", ".5", Default::default());
        let site = site(&mut vm, primary);
        let gold = vm.global("PLAYER_STATE_RESOURCE_GOLD");
        let wood = vm.global("PLAYER_STATE_RESOURCE_LUMBER");
        let host = &mut vm.host().inner;
        let owner = host.executed_unit(primary).unwrap().owner;
        for state in [gold, wood] {
            host.native(
                "SetPlayerState",
                &[Value::Handle(owner), state, Value::Int(0)],
            )
            .unwrap();
        }
        let args = match action {
            "IssueImmediateOrderById" => vec![Value::Handle(primary), Value::Int(744)],
            "SetUnitOwner" => vec![
                Value::Handle(primary),
                host.native("Player", &[Value::Int(5)]).unwrap(),
                Value::Bool(false),
            ],
            _ => vec![Value::Handle(primary)],
        };
        host.native(action, &args).unwrap();
        assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_GOLD"), 0);
        assert_eq!(bank(host, owner, "PLAYER_STATE_RESOURCE_LUMBER"), 0);
        assert_eq!(host.unit_construction(site).unwrap().unwrap().worker, None);
    }
}
