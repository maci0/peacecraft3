//! Internal canonical arrival/lifecycle fixtures; these do not exercise public point-order travel.
use super::*;
use crate::playercolor_host::BuildUnavailable;

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
        true,
        (
            &format!(
                "{EVENTS}\nnative SetUnitPosition takes unit u, real x, real y returns nothing\n"
            ),
            |mut host| {
                let catalog = Arc::new(PublicBuildCatalog(host.unit_catalog().unwrap()));
                host.construction_test_catalog(catalog);
                host.construction_test_footprint_loader(|path| {
                    assert_eq!(path, "build.tga");
                    Ok(crate::unit_placement::Footprint {
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
