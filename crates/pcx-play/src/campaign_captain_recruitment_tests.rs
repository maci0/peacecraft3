use super::*;

fn alias_fixture() -> (Vm, HandleId, HandleId) {
    let mut policy = crate::campaign_captain_policy::CaptainPolicy::fixture();
    policy.defenders.alternates[0] = (*b"z007", *b"u001");
    let (mut vm, alternate, exact, _) = defender_fixture_with_policy(0, policy);
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(alternate).unwrap().owner;
    h.native("RemoveUnit", &[Value::Handle(exact)]).unwrap();
    let exact = create(h, owner, *b"z007", 48.);
    (vm, alternate, exact)
}

#[test]
fn zero_request_finishes_exact_count_before_visiting_existing_alternate() {
    let (mut vm, alternate, exact) = alias_fixture();
    let h = &mut vm.host().inner;
    h.fixture_captain_member(4, false, alternate);
    let before = h.executed_unit(exact).unwrap().clone();
    assert!(
        h.add_ai_defenders(4, 0, i32::from_be_bytes(*b"z007"))
            .unwrap()
    );
    assert_eq!(h.fixture_captain_members(4, false), &[alternate]);
    assert_eq!(h.executed_unit(exact).unwrap(), &before);
    // An existing exact member still decrements zero into a negative request.
    h.fixture_captain_member(4, false, exact);
    assert!(
        !h.add_ai_defenders(4, 0, i32::from_be_bytes(*b"z007"))
            .unwrap()
    );
}

#[test]
fn recruitment_prefers_exact_within_each_health_pass_and_retries_after_death() {
    for (exact_healthy, alternate_healthy, expect_exact) in [
        (true, true, true),
        (false, true, false),
        (true, false, true),
        (false, false, true),
    ] {
        let (mut vm, alternate, exact) = alias_fixture();
        let h = &mut vm.host().inner;
        if !exact_healthy {
            h.set_test_life(exact, 1.);
        }
        if !alternate_healthy {
            h.set_test_life(alternate, 1.);
        }
        let expected = if expect_exact { exact } else { alternate };
        let other = if expect_exact { alternate } else { exact };
        let before = h.executed_unit(other).unwrap().clone();
        let kind = i32::from_be_bytes(*b"z007");
        assert!(h.add_ai_defenders(4, 1, kind).unwrap());
        assert_eq!(h.fixture_captain_members(4, false), &[expected]);
        assert_eq!(h.executed_unit(other).unwrap(), &before);
        assert!(h.add_ai_defenders(4, 1, kind).unwrap());
        assert_eq!(h.fixture_captain_members(4, false), &[expected]);
        h.native("KillUnit", &[Value::Handle(expected)]).unwrap();
        assert!(h.fixture_captain_members(4, false).is_empty());
        assert!(h.add_ai_defenders(4, 1, kind).unwrap());
        assert_eq!(h.fixture_captain_members(4, false), &[other]);
    }
}

#[test]
fn assault_reuses_exact_priority_and_keeps_truthful_count_and_readiness() {
    let (mut vm, alternate, exact) = assault_fixture();
    let h = &mut vm.host().inner;
    let policy = h.campaign_captain_policy().unwrap();
    let (requested, alternate_code) = policy.defenders.alternates[0];
    let owner = h.executed_unit(alternate).unwrap().owner;
    h.native("RemoveUnit", &[Value::Handle(alternate)]).unwrap();
    h.native("RemoveUnit", &[Value::Handle(exact)]).unwrap();
    let alternate = create(h, owner, alternate_code, 16.);
    let exact = create(h, owner, requested, 48.);
    h.init_ai_assault(4).unwrap();
    let kind = i32::from_be_bytes(requested);
    assert!(h.add_ai_assault(4, 1, kind).unwrap());
    assert_eq!(h.fixture_captain_members(4, true), &[exact]);
    assert_eq!(h.campaign_captain_group_size(owner).unwrap(), Some(1));
    let full_readiness = h
        .campaign_captain_policy()
        .unwrap()
        .assault
        .as_ref()
        .unwrap()
        .percentage as i32;
    assert_eq!(h.ai_captain_readiness(4).unwrap(), full_readiness);
    assert!(h.ai_captain_is_full(4).unwrap());
    assert!(!h.add_ai_assault(4, 3, kind).unwrap());
    assert_eq!(h.campaign_captain_group_size(owner).unwrap(), Some(2));
    assert!(!h.ai_captain_is_full(4).unwrap());
    h.native("RemoveUnit", &[Value::Handle(exact)]).unwrap();
    assert_eq!(h.fixture_captain_members(4, true), &[alternate]);
}

#[test]
fn unsupported_exact_candidate_refuses_before_publishing_an_alternate() {
    let (mut vm, alternate, exact) = alias_fixture();
    let h = &mut vm.host().inner;
    h.test_set_summoned(exact, true);
    let before = h.executed_unit(alternate).unwrap().clone();
    assert!(
        h.add_ai_defenders(4, 1, i32::from_be_bytes(*b"z007"))
            .is_err()
    );
    assert!(h.fixture_captain_members(4, false).is_empty());
    assert_eq!(h.executed_unit(alternate).unwrap(), &before);
}

fn create(h: &mut ObjectHost, owner: HandleId, kind: [u8; 4], x: f32) -> HandleId {
    let Value::Handle(unit) = h
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(kind)),
                Value::Real(x),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    else {
        panic!("unit");
    };
    unit
}

#[test]
fn inactive_defend_defaults_allow_guard_return_and_recruitment_but_active_defend_refuses() {
    for recruit in [false, true] {
        let fields = [(*b"uabi", ability_field("abilList", "Qord"))];
        let tables = order_tables(
            ORDER_HEADERS,
            &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "0", "0"],
            "",
            "1",
        );
        let (mut vm, a, _) = guarded_configured(
            8.,
            16.,
            1.,
            None,
            (
                &fields,
                "native SetUnitPosition takes unit u, real x, real y returns nothing\nnative SetPlayerAbilityAvailable takes player p, integer ability, boolean available returns nothing\n",
            ),
            |h| {
                h.with_player_tech_researched(TechLimitBudget { max_records: 2 })
                    .unwrap()
                    .with_defend_ability(&tables, "Qord")
                    .unwrap()
                    .with_player_abilities(16)
                    .unwrap()
            },
        );
        let h = &mut vm.host().inner;
        let context = Arc::clone(h.player_context().unwrap());
        let image = h.guard_state().unwrap().native_rules().image_sha256;
        let mut gathering = crate::campaign_gathering_policy::GatheringPolicy::fixture();
        gathering.image_sha256 = image;
        h.admit_campaign_gathering(&context, gathering).unwrap();
        let mut captains = crate::campaign_captain_policy::CaptainPolicy::fixture();
        captains.image_sha256 = image;
        h.admit_campaign_captains(&context, captains).unwrap();
        let mut mode = crate::campaign_mode_policy::CampaignModePolicy::fixture();
        mode.image_sha256 = image;
        h.admit_campaign_mode(&context, mode).unwrap();
        let mut targeting = crate::campaign_targeting_policy::TargetingPolicy::fixture();
        targeting.image_sha256 = image;
        h.admit_campaign_targeting(&context, targeting).unwrap();
        h.activate_campaign_ai_player(4).unwrap();
        h.create_ai_captains(4).unwrap();
        let owner = h.executed_unit(a).unwrap().owner;
        assert_eq!(
            h.executed_unit(a).unwrap().admitted_order,
            Some((*b"Qcls", false))
        );
        h.native(
            "SetPlayerTechResearched",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"Qtec")),
                Value::Int(1),
            ],
        )
        .unwrap();
        h.native(
            "SetUnitPosition",
            &[Value::Handle(a), Value::Real(80.), Value::Real(16.)],
        )
        .unwrap();
        assert_eq!(
            h.native(
                "IssueImmediateOrder",
                &[Value::Handle(a), Value::Str("engage".into())]
            )
            .unwrap(),
            Value::Bool(true)
        );
        let active = h.executed_unit(a).unwrap().clone();
        let attempt = |h: &mut ObjectHost| {
            if recruit {
                h.add_ai_defenders(4, 1, i32::from_be_bytes(*b"u001"))
                    .map(|accepted| assert!(accepted))
            } else {
                h.return_ai_guard_posts(4)
            }
        };
        assert!(attempt(h).is_err());
        assert_eq!(h.executed_unit(a).unwrap(), &active);
        assert!(h.fixture_captain_members(4, false).is_empty());
        assert_eq!(
            h.native(
                "IssueImmediateOrder",
                &[Value::Handle(a), Value::Str("release".into())]
            )
            .unwrap(),
            Value::Bool(true)
        );
        attempt(h).unwrap();
        assert_eq!(
            h.executed_unit(a).unwrap().admitted_order,
            Some((*b"Qcls", false))
        );
        assert!(
            h.executed_unit(a)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .moving()
        );
        if recruit {
            assert_eq!(h.fixture_captain_members(4, false), &[a]);
        }
    }
}
