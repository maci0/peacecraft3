use super::*;
use crate::campaign_captain_policy::CaptainPolicy;
use crate::campaign_gathering_policy::GatheringPolicy;
use crate::campaign_group_timed_life_policy::GroupTimedLifePolicy;

fn opened() -> (Vm, HandleId, HandleId, i32) {
    opened_with_acquire(None)
}

fn opened_with_acquire(acquire: Option<&str>) -> (Vm, HandleId, HandleId, i32) {
    let mut fields = armed("80", "normal");
    if let Some(acquire) = acquire {
        fields
            .iter_mut()
            .find(|(id, _)| id == b"W005")
            .unwrap()
            .1
            .value = Ok(FieldValue::Cell(acquire.into()));
    }
    fields.extend(super::super::activity::profile_fields(
        FieldValue::Cell("100".into()),
        Some("foot"),
        None,
    ));
    fields.extend(super::super::activity::gathering::source_fields(false));
    let (mut vm, unit, other) = open_fixture(&fields, None, rules(), false, None, None);
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_gathering(&context, GatheringPolicy::fixture())
        .unwrap();
    let captain = CaptainPolicy::fixture();
    host.admit_campaign_captains(&context, captain.clone())
        .unwrap();
    host.admit_campaign_group_timed_life(&context, GroupTimedLifePolicy::fixture())
        .unwrap();
    assert!(host.activate_campaign_ai_player(4).unwrap());
    host.create_ai_captains(4).unwrap();
    host.set_ai_captain_home(4, i32::from(captain.first_selector_mask), [112., 16.])
        .unwrap();
    (
        vm,
        unit,
        other,
        i32::from_be_bytes(GroupTimedLifePolicy::fixture().timed_life_factory),
    )
}

fn apply(host: &mut ObjectHost, unit: HandleId, key: i32, duration: f32) -> Result<Value, String> {
    host.native(
        "UnitApplyTimedLife",
        &[Value::Handle(unit), Value::Int(key), Value::Real(duration)],
    )
}

#[test]
fn captain_combat_is_published_from_members_not_other_owned_fighters() {
    let (mut vm, unit, target, key) = opened();
    let owner = vm.host().inner.executed_unit(unit).unwrap().owner;
    for attack in [false, true] {
        assert_eq!(
            vm.host()
                .inner
                .campaign_captain_in_combat(owner, attack)
                .unwrap(),
            Some(false)
        );
    }
    vm.host()
        .inner
        .native(
            "IssueTargetOrder",
            &[
                Value::Handle(unit),
                Value::Str("alpha".into()),
                Value::Handle(target),
            ],
        )
        .unwrap();
    step(&mut vm, 0.1);
    assert_eq!(
        vm.host()
            .inner
            .campaign_captain_in_combat(owner, true)
            .unwrap(),
        Some(false)
    );
    let host = &mut vm.host().inner;
    host.set_ai_group_timed_life(4, true).unwrap();
    apply(host, unit, key, 10.).unwrap();
    host.native(
        "IssueTargetOrder",
        &[
            Value::Handle(unit),
            Value::Str("alpha".into()),
            Value::Handle(target),
        ],
    )
    .unwrap();
    assert_eq!(
        host.campaign_captain_in_combat(owner, true).unwrap(),
        Some(false)
    );
    let life = host.executed_unit(target).unwrap().vitals.unwrap().life;
    step(&mut vm, 2.);
    let host = &mut vm.host().inner;
    assert!(host.executed_unit(target).unwrap().vitals.unwrap().life < life);
    assert_eq!(
        host.campaign_captain_in_combat(owner, true).unwrap(),
        Some(true)
    );
    assert_eq!(
        host.campaign_captain_in_combat(owner, false).unwrap(),
        Some(false)
    );
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
    host.fixture_captain_member(4, false, unit);
    step(&mut vm, 2.1);
    let host = &mut vm.host().inner;
    assert_eq!(
        host.campaign_captain_in_combat(owner, true).unwrap(),
        Some(false)
    );
    assert_eq!(
        host.campaign_captain_in_combat(owner, false).unwrap(),
        Some(true)
    );
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(0));
    let other_owner = host.executed_unit(target).unwrap().owner;
    host.activate_campaign_ai_player(5).unwrap();
    host.create_ai_captains(5).unwrap();
    assert_eq!(
        host.campaign_captain_in_combat(other_owner, false).unwrap(),
        Some(false)
    );
    host.native("KillUnit", &[Value::Handle(target)]).unwrap();
    step(&mut vm, 2.2);
    let host = &mut vm.host().inner;
    assert_eq!(
        host.campaign_captain_in_combat(owner, false).unwrap(),
        Some(false)
    );
    host.deactivate_campaign_ai_player(4).unwrap();
    assert_eq!(host.campaign_captain_in_combat(owner, true).unwrap(), None);
    assert_eq!(
        host.campaign_captain_in_combat(other_owner, true).unwrap(),
        Some(false)
    );
}

#[test]
fn removing_a_captain_combat_target_invalidates_the_query_reference() {
    let (mut vm, unit, target, key) = opened();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(unit).unwrap().owner;
    host.set_ai_group_timed_life(4, true).unwrap();
    apply(host, unit, key, 10.).unwrap();
    assert_eq!(
        host.native(
            "IssueTargetOrder",
            &[
                Value::Handle(unit),
                Value::Str("alpha".into()),
                Value::Handle(target)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 0.25);
    let host = &mut vm.host().inner;
    assert_eq!(
        host.campaign_captain_in_combat(owner, true).unwrap(),
        Some(true)
    );
    host.native("RemoveUnit", &[Value::Handle(target)]).unwrap();
    assert_eq!(
        host.campaign_captain_in_combat(owner, true).unwrap(),
        Some(false)
    );
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
}

#[test]
fn true_membership_tracks_shared_expiry_and_home_route_without_minting_handles() {
    let (mut vm, unit, other, key) = opened();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(unit).unwrap().owner;
    let handles = host.inner().handles();
    let kind = i32::from_be_bytes(host.executed_unit(unit).unwrap().definition.rawcode);
    assert_eq!(host.ai_ignored_units(4, kind).unwrap(), 1);
    assert_eq!(host.campaign_group_timed_life_enabled(owner).unwrap(), None);
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(0));
    assert_eq!(host.campaign_captain_is_empty(owner).unwrap(), Some(true));
    host.set_ai_group_timed_life(4, true).unwrap();
    apply(host, unit, key, 1.).unwrap();
    assert_eq!(host.ai_ignored_units(4, kind).unwrap(), 0);
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
    assert_eq!(host.campaign_captain_is_empty(owner).unwrap(), Some(false));
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        Some([112., 16.])
    );
    assert_eq!(host.inner().handles(), handles);
    assert!(apply(host, unit, key, 1.).is_err());
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
    host.set_ai_captain_home(
        4,
        i32::from(host.campaign_captain_policy().unwrap().first_selector_mask),
        [96., 16.],
    )
    .unwrap();
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        Some([96., 16.])
    );
    apply(host, other, key, 1.).unwrap();
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
    host.advance_unit_expirations(1.).unwrap();
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(0));
    assert_eq!(host.campaign_captain_is_empty(owner).unwrap(), Some(true));
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[test]
fn false_flag_without_guard_refuses_before_stopping_or_scheduling() {
    let (mut vm, unit, _, key) = opened();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(unit).unwrap().owner;
    host.set_ai_group_timed_life(4, false).unwrap();
    let wrong_family = apply(host, owner, key, 1.).unwrap_err();
    assert!(
        wrong_family.contains("UnitApplyTimedLife"),
        "{wrong_family}"
    );
    assert!(host.steer_unit(unit, Some([112., 16.]), "test").unwrap());
    let before = host
        .executed_unit(unit)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .destination();
    assert_eq!(before, Some([112., 16.]));
    assert!(apply(host, unit, key ^ 1, 1.).is_err());
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        before
    );
    let refusal = apply(host, unit, key, 1.).unwrap_err();
    assert!(refusal.contains("unit guard"), "{refusal}");
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        before
    );
    host.advance_unit_expirations(1.).unwrap();
    assert!(
        !host
            .executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(0));
    assert_eq!(host.campaign_captain_is_empty(owner).unwrap(), Some(true));
}

#[test]
fn captain_route_keeps_combat_acquisition_out_until_arrival() {
    let (mut vm, unit, other, key) = opened_with_acquire(Some("256"));
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(unit).unwrap().owner;
    host.set_ai_group_timed_life(4, true).unwrap();
    apply(host, unit, key, 5.).unwrap();
    assert!(host.captain_route_owned(unit));
    step(&mut vm, 0.25);
    let host = &vm.host().inner;
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        None
    );
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        Some([112., 16.])
    );
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
    step(&mut vm, 2.);
    let host = &vm.host().inner;
    assert!(!host.captain_route_owned(unit));
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(other)
    );
}

#[test]
fn transfer_and_context_retirement_clear_membership_but_not_the_unit_deadline() {
    let (mut vm, unit, _, key) = opened();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(unit).unwrap().owner;
    host.set_ai_group_timed_life(4, true).unwrap();
    apply(host, unit, key, 1.).unwrap();
    let next = host.player_identity(5).unwrap();
    host.native(
        "SetUnitOwner",
        &[Value::Handle(unit), Value::Handle(next), Value::Bool(false)],
    )
    .unwrap();
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(0));
    host.deactivate_campaign_ai_player(4).unwrap();
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), None);
    host.advance_unit_expirations(1.).unwrap();
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[test]
fn context_retirement_stops_its_captain_route_but_preserves_expiry() {
    let (mut vm, unit, _, key) = opened();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(unit).unwrap().owner;
    host.set_ai_group_timed_life(4, true).unwrap();
    apply(host, unit, key, 1.).unwrap();
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
    host.deactivate_campaign_ai_player(4).unwrap();
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), None);
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        None
    );
    host.advance_unit_expirations(1.).unwrap();
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[test]
fn a_later_explicit_route_keeps_membership_without_captain_route_ownership() {
    let (mut vm, unit, _, key) = opened();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(unit).unwrap().owner;
    host.set_ai_group_timed_life(4, true).unwrap();
    apply(host, unit, key, 1.).unwrap();
    assert!(host.captain_route_owned(unit));
    assert!(host.steer_unit(unit, Some([64., 16.]), "test").unwrap());
    assert!(!host.captain_route_owned(unit));
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(1));
    host.deactivate_campaign_ai_player(4).unwrap();
    assert_eq!(
        host.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .destination(),
        Some([64., 16.])
    );
    host.advance_unit_expirations(1.).unwrap();
    assert!(
        host.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[test]
fn inactive_unit_expiry_and_removed_member_do_not_leave_captain_counts() {
    let (mut vm, unit, other, key) = opened();
    let host = &mut vm.host().inner;
    let owner = host.executed_unit(unit).unwrap().owner;
    host.set_ai_group_timed_life(4, true).unwrap();
    apply(host, unit, key, 1.).unwrap();
    host.native("RemoveUnit", &[Value::Handle(unit)]).unwrap();
    assert_eq!(host.campaign_captain_group_size(owner).unwrap(), Some(0));
    host.deactivate_campaign_ai_player(4).unwrap();
    apply(host, other, key, 1.).unwrap();
    host.advance_unit_expirations(1.).unwrap();
    assert!(
        host.executed_unit(other)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[test]
fn campaign_mode_reaches_existing_and_later_units_without_a_second_unit_store() {
    use crate::campaign_mode_policy::CampaignModePolicy;
    let (mut vm, unit, other, _) = opened();
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_mode(&context, CampaignModePolicy::fixture())
        .unwrap();
    assert_eq!(host.campaign_unit_mode(unit).unwrap(), None);
    host.set_ai_campaign_mode(4).unwrap();
    assert_eq!(host.campaign_unit_mode(unit).unwrap(), Some(true));
    assert_eq!(host.campaign_unit_mode(other).unwrap(), None);
    let later = create_for(&mut vm, unit, *b"u001", 80., 16.);
    let host = &mut vm.host().inner;
    assert_eq!(host.campaign_unit_mode(later).unwrap(), Some(true));
    let owner = host.executed_unit(other).unwrap().owner;
    host.native(
        "SetUnitOwner",
        &[
            Value::Handle(later),
            Value::Handle(owner),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(host.campaign_unit_mode(later).unwrap(), None);
    host.deactivate_campaign_ai_player(4).unwrap();
    assert_eq!(host.campaign_unit_mode(unit).unwrap(), None);
}
