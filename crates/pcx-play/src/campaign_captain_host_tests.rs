use super::*;
use crate::campaign_captain_policy::CaptainPolicy;
use crate::campaign_gathering_policy::GatheringPolicy;

fn opened() -> Vm {
    open_with_fields(&super::gathering::source_fields(true))
}

fn admit(vm: &mut Vm) -> (HandleId, CaptainPolicy) {
    let a = unit(vm, "A");
    let owner = vm.host().inner.executed_unit(a).unwrap().owner;
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    host.admit_campaign_gathering(&context, GatheringPolicy::fixture())
        .unwrap();
    let policy = CaptainPolicy::fixture();
    let mut mismatch = policy.clone();
    mismatch.image_sha256[0] ^= 1;
    assert!(host.admit_campaign_captains(&context, mismatch).is_err());
    assert!(host.campaign_captain_policy().is_none());
    host.admit_campaign_captains(&context, policy.clone())
        .unwrap();
    assert!(
        host.admit_campaign_captains(&context, policy.clone())
            .is_err()
    );
    assert!(host.activate_campaign_ai_player(4).unwrap());
    (owner, policy)
}

#[test]
fn malformed_selector_admission_preserves_the_unadmitted_host() {
    for (first, second) in [(0, 2), (1, 0), (3, 2), (1, 1)] {
        let mut vm = opened();
        let host = &mut vm.host().inner;
        let context = Arc::clone(host.player_context().unwrap());
        let mut policy = CaptainPolicy::fixture();
        policy.first_selector_mask = first;
        policy.second_selector_mask = second;
        let handles = host.inner().handles();
        assert!(host.admit_campaign_captains(&context, policy).is_err());
        assert!(host.campaign_captain_policy().is_none());
        assert_eq!(host.inner().handles(), handles);
    }
    let mut vm = opened();
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    let mut policy = CaptainPolicy::fixture();
    policy.first_selector_mask = 0x80;
    host.admit_campaign_captains(&context, policy).unwrap();
}

#[test]
fn creation_before_stop_uses_canonical_anchor_and_setter_changes_selected_homes() {
    let mut vm = opened();
    let owner = admit(&mut vm).0;
    let policy = vm.host().inner.campaign_captain_policy().unwrap().clone();
    let host = &mut vm.host().inner;
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .enabled,
        None
    );
    assert!(!host.campaign_captain_state(owner).unwrap().unwrap().created);
    assert!(host.campaign_captain_in_combat(owner, true).is_err());
    let handles = host.inner().handles();
    // Source selector bits with no selected slot do not require captain creation.
    host.set_ai_captain_home(4, 0, [48., 64.]).unwrap();
    let high_only = !(policy.first_selector_mask | policy.second_selector_mask) as i32;
    host.set_ai_captain_home(4, high_only, [48., 64.]).unwrap();
    assert!(
        host.set_ai_captain_home(4, policy.first_selector_mask as i32, [48., 64.])
            .is_err()
    );
    host.create_ai_captains(4).unwrap();
    assert_eq!(host.inner().handles(), handles);
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .enabled,
        None
    );
    assert_eq!(
        host.campaign_captain_state(owner).unwrap().unwrap().homes,
        [Some([80., 16.]); 2]
    );
    host.set_ai_captain_home(4, policy.first_selector_mask as i32, [48., 64.])
        .unwrap();
    assert_eq!(
        host.campaign_captain_state(owner).unwrap().unwrap().homes,
        [Some([48., 64.]), Some([80., 16.])]
    );
    let both = (policy.first_selector_mask | policy.second_selector_mask) as i32;
    host.set_ai_captain_home(4, both, [32., 40.]).unwrap();
    assert_eq!(
        host.campaign_captain_state(owner).unwrap().unwrap().homes,
        [Some([32., 40.]); 2]
    );
    assert!(host.create_ai_captains(4).is_err());
}

#[test]
fn empty_anchor_creates_absent_targets_and_selected_set_refuses_atomically() {
    let mut vm = opened();
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    let (owner, policy) = admit(&mut vm);
    let host = &mut vm.host().inner;
    host.native("RemoveUnit", &[Value::Handle(a)]).unwrap();
    host.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    host.create_ai_captains(4).unwrap();
    let empty = host.campaign_captain_state(owner).unwrap().unwrap();
    assert!(empty.created);
    assert_eq!(empty.homes, [None; 2]);
    assert!(host.campaign_captain_in_combat(owner, false).is_err());
    let both = (policy.first_selector_mask | policy.second_selector_mask) as i32;
    assert!(host.set_ai_captain_home(4, both, [24., 32.]).is_err());
    assert_eq!(host.campaign_captain_state(owner).unwrap().unwrap(), empty);
    assert!(host.set_ai_captain_home(4, 0, [24., 32.]).is_ok());
    assert!(host.set_ai_captain_home(4, 0, [f32::NAN, 32.]).is_err());
    assert_eq!(host.campaign_captain_state(owner).unwrap().unwrap(), empty);
}

#[test]
fn initial_home_samples_the_current_anchor_without_changing_gathering_flags() {
    let mut vm = opened();
    let b = unit(&vm, "B");
    let (owner, _) = admit(&mut vm);
    let host = &mut vm.host().inner;
    host.native(
        "SetUnitPosition",
        &[Value::Handle(b), Value::Real(112.), Value::Real(16.)],
    )
    .unwrap();
    host.create_ai_captains(4).unwrap();
    assert_eq!(
        host.campaign_captain_state(owner).unwrap().unwrap().homes,
        [Some([112., 16.]); 2]
    );
    assert_eq!(
        host.campaign_gathering_state(owner)
            .unwrap()
            .unwrap()
            .enabled,
        None
    );
}

#[test]
fn unsupported_candidate_refuses_before_creating_any_captain_target() {
    let mut vm = opened();
    let b = unit(&vm, "B");
    let (owner, _) = admit(&mut vm);
    let host = &mut vm.host().inner;
    host.native("PauseUnit", &[Value::Handle(b), Value::Bool(true)])
        .unwrap();
    let before = host.campaign_captain_state(owner).unwrap().unwrap();
    assert!(host.create_ai_captains(4).is_err());
    assert_eq!(host.campaign_captain_state(owner).unwrap().unwrap(), before);
}
