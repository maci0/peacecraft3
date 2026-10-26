use super::{profile, state::*};

#[test]
fn seed_budget_and_profile_refusal_are_transactional() {
    let policy = profile::fixture();
    assert_eq!(seeded(&policy, 17, 0), Err(StateError::Budget));
    let mut state = Some(seeded(&policy, 17, 3).unwrap());
    assert_eq!(state.as_ref().unwrap().secondary_words().len(), 3);
    let before = state.clone();
    let mut other = policy.clone();
    other.key[0] ^= 1;
    assert_eq!(integer(&other, &mut state, 0, 1), Err(StateError::Profile));
    assert_eq!(state, before);
    assert_eq!(state, Some(seeded(&policy, 17, 3).unwrap()));
}

#[test]
fn equal_bounds_need_no_state_and_do_not_consume_it() {
    let policy = profile::fixture();
    let mut missing = None;
    assert_eq!(integer(&policy, &mut missing, 37, 37).unwrap(), 37);
    assert_eq!(
        real(&policy, &mut missing, -0.0, 0.0).unwrap().to_bits(),
        (-0.0f32).to_bits()
    );
    assert_eq!(
        integer(&policy, &mut missing, 0, 1),
        Err(StateError::Uninitialized)
    );
    let mut state = Some(seeded(&policy, 17, 3).unwrap());
    let before = state.clone();
    integer(&policy, &mut state, 5, 5).unwrap();
    real(&policy, &mut state, 1.0, 1.0001).unwrap();
    assert_eq!(state, before);
}

#[test]
fn full_signed_distance_still_consumes_one_draw() {
    let policy = profile::fixture();
    let mut state = Some(seeded(&policy, 17, 3).unwrap());
    let mut control = state.clone();
    assert_eq!(
        integer(&policy, &mut state, i32::MIN, i32::MAX).unwrap(),
        i32::MIN
    );
    integer(&policy, &mut control, 0, 1).unwrap();
    assert_eq!(state, control);
    let secondary: Vec<_> = state.as_ref().unwrap().secondary_words().collect();
    real(&policy, &mut state, -2.0, 3.0).unwrap();
    assert_eq!(
        state
            .as_ref()
            .unwrap()
            .secondary_words()
            .collect::<Vec<_>>(),
        secondary
    );
}

#[test]
fn refused_real_inputs_preserve_all_state() {
    let policy = profile::fixture();
    let mut state = Some(seeded(&policy, -1, 3).unwrap());
    let before = state.clone();
    for low in [f32::NAN, f32::INFINITY, f32::from_bits(1), -f32::MAX] {
        assert!(real(&policy, &mut state, low, f32::MAX).is_err());
        assert_eq!(state, before);
    }
}

#[test]
fn tolerance_equality_consumes_a_draw_while_strictly_below_does_not() {
    let policy = profile::fixture();
    let mut state = Some(seeded(&policy, 17, 3).unwrap());
    let before = state.clone();
    let tolerance = f32::from_bits(policy.real_tolerance);
    real(&policy, &mut state, 0.0, tolerance / 2.0).unwrap();
    assert_eq!(state, before);
    real(&policy, &mut state, 0.0, tolerance).unwrap();
    let mut control = before;
    integer(&policy, &mut control, 0, 1).unwrap();
    assert_eq!(state, control);
}

#[test]
fn secondary_die_consumes_only_selected_stream_including_one_sided_die() {
    let policy = profile::fixture();
    let mut state = Some(seeded(&policy, 17, 3).unwrap());
    let before = state.clone().unwrap();
    assert_eq!(secondary_die(&policy, &mut state, 1, 1), Ok(0));
    let after = state.as_ref().unwrap();
    assert_eq!(after.primary_word(), before.primary_word());
    let old: Vec<_> = before.secondary_words().collect();
    let new: Vec<_> = after.secondary_words().collect();
    assert_eq!(old[0], new[0]);
    assert_ne!(old[1], new[1]);
    assert_eq!(old[2], new[2]);
    let mut control = Some(before);
    secondary_die(&policy, &mut control, 1, 91).unwrap();
    assert_eq!(state, control);
    assert!(secondary_die(&policy, &mut state, 1, 91).unwrap() < 91);
}

#[test]
fn secondary_die_refusals_preserve_all_streams() {
    let policy = profile::fixture();
    let mut state = Some(seeded(&policy, -8, 3).unwrap());
    let before = state.clone();
    assert_eq!(
        secondary_die(&policy, &mut state, 3, 2),
        Err(StateError::SecondaryDie)
    );
    assert_eq!(
        secondary_die(&policy, &mut state, 0, 0),
        Err(StateError::SecondaryDie)
    );
    assert_eq!(state, before);
    let mut other = policy.clone();
    other.key[0] ^= 1;
    assert_eq!(
        secondary_die(&other, &mut state, 0, 2),
        Err(StateError::Profile)
    );
    assert_eq!(state, before);
    let mut missing = None;
    assert_eq!(
        secondary_die(&policy, &mut missing, 0, 2),
        Err(StateError::Uninitialized)
    );
    assert_eq!(missing, None);
}
