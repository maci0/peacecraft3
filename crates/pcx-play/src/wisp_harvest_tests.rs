//! Wisp Harvest (`Awha`) on synthetic ability rows: 4.75 lumber every 3 seconds within a reach of
//! 30 are fixture values (`harvest_tables`), not the install's 5, 8 and 900.
use super::*;

const STOP_ORDER_TEXT: &str = "beta";

/// A worker-model unit that lists only `Awha`, and a tree `tree_x` away on the same row.
fn wisp_fixture(tree_x: f32) -> (Vm, HandleId, HandleId) {
    let (mut vm, tree) = open_harvest_with(
        harvest_tables(),
        tree_catalog(),
        placed_tree_at(tree_x, 32.0),
        harvest_fields(),
    );
    let wisp = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    gold_role(&mut vm, wisp, "Awha", false, None);
    (vm, wisp, tree)
}

fn lumber_gathered(vm: &mut Vm) -> i32 {
    player_bank(vm, "PLAYER_STATE_LUMBER_GATHERED")
}

fn distance_to(vm: &mut Vm, unit: HandleId, point: [f32; 2]) -> f32 {
    let at = position(vm, unit);
    ((at[0] - point[0]).powi(2) + (at[1] - point[1]).powi(2)).sqrt()
}

#[test]
fn a_wisp_in_reach_credits_truncated_dataa_lumber_every_dur_without_cargo_depot_or_damage() {
    let (mut vm, wisp, tree) = wisp_fixture(52.0);
    let (bank, gathered) = (lumber_bank(&mut vm), lumber_gathered(&mut vm));
    assert_eq!(
        explicit_tree_order(&mut vm, wisp, tree).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(current_order(&mut vm, wisp), 902);
    assert!(vm.host().inner.harvest_runs(wisp));
    tick(&mut vm, 2.9);
    assert_eq!(lumber_bank(&mut vm), bank, "no credit before the interval");
    tick(&mut vm, 3.0);
    assert_eq!(lumber_bank(&mut vm), bank + 4, "4.75 truncates to 4");
    assert_eq!(lumber_gathered(&mut vm), gathered + 4);
    tick(&mut vm, 5.9);
    assert_eq!(lumber_bank(&mut vm), bank + 4);
    tick(&mut vm, 6.0);
    assert_eq!(lumber_bank(&mut vm), bank + 8);
    assert_eq!(lumber_gathered(&mut vm), gathered + 8);
    assert_eq!(tree_life(&mut vm, tree), 80.0, "wisps do not damage trees");
    assert_eq!(vm.host().inner.harvest_cargo(wisp), None);
    assert_eq!(position(&mut vm, wisp), [32.0, 32.0]);
    assert_eq!(current_order(&mut vm, wisp), 902);
}

#[test]
fn a_wisp_beyond_rng_flies_to_the_tree_and_the_interval_starts_on_arrival() {
    let (mut vm, wisp, tree) = wisp_fixture(110.0);
    let bank = lumber_bank(&mut vm);
    assert_eq!(
        explicit_tree_order(&mut vm, wisp, tree).unwrap(),
        Value::Bool(true)
    );
    let mut arrived_at = None;
    let mut credited_at = None;
    for step in 1..=400 {
        let now = f64::from(step) * 0.25;
        tick(&mut vm, now);
        let in_reach = distance_to(&mut vm, wisp, [110.0, 32.0]) <= 30.0;
        if arrived_at.is_none() && in_reach {
            arrived_at = Some(now);
        }
        if lumber_bank(&mut vm) > bank {
            assert!(
                arrived_at.is_some(),
                "no credit before the wisp is in reach (step {step})"
            );
            credited_at = Some(now);
            break;
        }
    }
    let (arrived, credited) = (
        arrived_at.expect("the wisp arrived"),
        credited_at.expect("a credit"),
    );
    assert!(
        credited >= arrived + 3.0 - 1e-6,
        "first credit {credited} comes a full interval after arrival {arrived}"
    );
    assert_eq!(lumber_bank(&mut vm), bank + 4);
}

#[test]
fn another_order_ends_a_wisp_harvest() {
    let (mut vm, wisp, tree) = wisp_fixture(52.0);
    let bank = lumber_bank(&mut vm);
    explicit_tree_order(&mut vm, wisp, tree).unwrap();
    tick(&mut vm, 3.0);
    assert_eq!(lumber_bank(&mut vm), bank + 4);
    vm.host()
        .inner
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(wisp), Value::Str(STOP_ORDER_TEXT.into())],
        )
        .unwrap();
    assert!(!vm.host().inner.harvest_runs(wisp));
    assert_eq!(current_order(&mut vm, wisp), 0);
    tick(&mut vm, 9.0);
    assert_eq!(lumber_bank(&mut vm), bank + 4);
}

#[test]
fn a_wisp_idles_when_its_tree_dies_with_no_other_tree_within_rng() {
    let (mut vm, wisp, tree) = wisp_fixture(52.0);
    let bank = lumber_bank(&mut vm);
    explicit_tree_order(&mut vm, wisp, tree).unwrap();
    tick(&mut vm, 3.0);
    assert_eq!(lumber_bank(&mut vm), bank + 4);
    {
        let h = &mut vm.host().inner;
        let plan = h
            .prepare_destructable_damage(tree, 1000.0)
            .unwrap()
            .expect("the tree takes damage");
        h.commit_destructable_damage(plan);
    }
    tick(&mut vm, 3.5);
    assert!(!vm.host().inner.harvest_runs(wisp));
    assert_eq!(current_order(&mut vm, wisp), 0);
    tick(&mut vm, 9.0);
    assert_eq!(lumber_bank(&mut vm), bank + 4);
}

#[test]
fn a_wisp_takes_the_nearest_other_tree_within_rng_when_its_tree_dies() {
    let mut second = placed_tree_at(62.0, 32.0);
    second.creation.origin =
        crate::playercolor_host::DestructableOrigin::Placed { creation_number: 2 };
    second.source.creation_number = 2;
    let (mut vm, tree) = open_harvest_model(
        harvest_tables(),
        tree_catalog(),
        vec![placed_tree_at(52.0, 32.0), second],
        harvest_fields(),
        crate::playercolor_host::harvest_runtime::Budget::new(4, 64),
        ("", MAP),
        |host| host,
        true,
    );
    let other = vm
        .host()
        .inner
        .destructables_with_handles()
        .map(|(handle, ..)| handle)
        .find(|handle| *handle != tree)
        .expect("a second tree");
    let wisp = create_unit(&mut vm, *b"hpea", 32.0, 32.0);
    gold_role(&mut vm, wisp, "Awha", false, None);
    let bank = lumber_bank(&mut vm);
    explicit_tree_order(&mut vm, wisp, tree).unwrap();
    tick(&mut vm, 3.0);
    assert_eq!(lumber_bank(&mut vm), bank + 4);
    {
        let h = &mut vm.host().inner;
        let plan = h
            .prepare_destructable_damage(tree, 1000.0)
            .unwrap()
            .expect("the tree takes damage");
        h.commit_destructable_damage(plan);
    }
    tick(&mut vm, 3.5);
    assert!(
        vm.host().inner.harvest_runs(wisp),
        "the wisp took the new tree"
    );
    assert_eq!(current_order(&mut vm, wisp), 902);
    for step in 8..=24 {
        tick(&mut vm, f64::from(step) * 0.5);
    }
    assert!(
        lumber_bank(&mut vm) > bank + 4,
        "credits resume on the new tree"
    );
    assert_eq!(tree_life(&mut vm, other), 80.0);
}

#[test]
fn wisp_autoharvestlumber_refuses_and_a_dead_tree_target_is_not_accepted() {
    let (mut vm, wisp, tree) = wisp_fixture(52.0);
    let error = vm
        .host()
        .inner
        .native(
            "IssueImmediateOrder",
            &[Value::Handle(wisp), Value::Str("autoharvestlumber".into())],
        )
        .unwrap_err();
    assert!(error.contains("not decoded"), "{error}");
    assert!(!vm.host().inner.harvest_runs(wisp));
    {
        let h = &mut vm.host().inner;
        let plan = h
            .prepare_destructable_damage(tree, 1000.0)
            .unwrap()
            .expect("the tree takes damage");
        h.commit_destructable_damage(plan);
    }
    assert_eq!(
        explicit_tree_order(&mut vm, wisp, tree).unwrap(),
        Value::Bool(false)
    );
}
