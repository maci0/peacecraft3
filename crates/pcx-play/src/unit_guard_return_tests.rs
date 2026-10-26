use super::*;

fn fixture() -> (Vm, HandleId, HandleId) {
    fixture_with(None)
}

fn fixture_with(problem: Option<&str>) -> (Vm, HandleId, HandleId) {
    let (mut vm, a, b) = guarded_configured(
        8.,
        16.,
        1.,
        None,
        (
            &[],
            "native SetUnitPosition takes unit u, real x, real y returns nothing\n",
        ),
        |h| h,
    );
    let h = &mut vm.host().inner;
    let ctx = Arc::clone(h.player_context().unwrap());
    let mut image = h.guard_state().unwrap().native_rules().image_sha256;
    if problem == Some("identity") {
        image[0] ^= 1;
    }
    let mut mode = crate::campaign_mode_policy::CampaignModePolicy::fixture();
    mode.image_sha256 = image;
    if problem == Some("order") {
        mode.guard_posts.return_order += 1;
    }
    let mut classifier = crate::campaign_targeting_policy::TargetingPolicy::fixture();
    classifier.image_sha256 = image;
    h.admit_campaign_mode(&ctx, mode).unwrap();
    h.admit_campaign_targeting(&ctx, classifier).unwrap();
    h.activate_campaign_ai_player(4).unwrap();
    (vm, a, b)
}

fn reported_order(h: &mut ObjectHost, unit: HandleId) -> Value {
    h.native("GetUnitCurrentOrder", &[Value::Handle(unit)])
        .unwrap()
}

fn displace(h: &mut ObjectHost, unit: HandleId) {
    h.native(
        "SetUnitPosition",
        &[Value::Handle(unit), Value::Real(48.), Value::Real(16.)],
    )
    .unwrap();
}

#[test]
fn return_guard_posts_routes_idle_and_held_guards_without_creating_units() {
    for held in [false, true] {
        let (mut vm, a, b) = fixture();
        let h = &mut vm.host().inner;
        let home = h.guard_state().unwrap().get(a).unwrap().home;
        displace(h, a);
        if held {
            assert_eq!(
                h.native(
                    "IssueImmediateOrder",
                    &[Value::Handle(a), Value::Str("alpha".into())]
                )
                .unwrap(),
                Value::Bool(true)
            );
            assert!(
                h.executed_unit(a)
                    .unwrap()
                    .activity
                    .as_ref()
                    .unwrap()
                    .held()
            );
        }
        assert_eq!(reported_order(h, a), Value::Int(0));
        let handles = h.inner().handles();
        let other = h.executed_unit(b).unwrap().clone();
        h.return_ai_guard_posts(4).unwrap();
        assert_eq!(
            h.executed_unit(a)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .destination(),
            Some(home)
        );
        assert!(
            !h.executed_unit(a)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .held()
        );
        let order = h.campaign_mode_policy().unwrap().guard_posts.return_order;
        assert_eq!(
            h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
                .unwrap(),
            Value::Int(order)
        );
        let accepted = h.executed_unit(a).unwrap().clone();
        h.return_ai_guard_posts(4).unwrap();
        assert_eq!(h.executed_unit(a).unwrap(), &accepted);
        assert_eq!(h.executed_unit(b).unwrap(), &other);
        assert_eq!(h.inner().handles(), handles);
        step(&mut vm, 5.);
        let h = &mut vm.host().inner;
        assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), home) <= 1.);
        assert_eq!(reported_order(h, a), Value::Int(0));
    }
}

#[test]
fn return_guard_posts_preserves_active_move_and_attack_point_orders() {
    for order in ["gamma", "alpha"] {
        let (mut vm, a, _) = fixture();
        let h = &mut vm.host().inner;
        displace(h, a);
        assert_eq!(
            h.native(
                "IssuePointOrder",
                &[
                    Value::Handle(a),
                    Value::Str(order.into()),
                    Value::Real(80.),
                    Value::Real(16.)
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
        let accepted = h.executed_unit(a).unwrap().clone();
        assert_ne!(reported_order(h, a), Value::Int(0));
        h.return_ai_guard_posts(4).unwrap();
        assert_eq!(h.executed_unit(a).unwrap(), &accepted);
    }
}

#[test]
fn return_guard_posts_does_not_reclaim_released_dead_removed_or_foreign_units() {
    for action in ["release", "death", "remove", "transfer"] {
        let (mut vm, a, b) = fixture();
        let h = &mut vm.host().inner;
        displace(h, a);
        match action {
            "release" => {
                h.native("RemoveGuardPosition", &[Value::Handle(a)])
                    .unwrap();
            }
            "death" => h.kill_unit(a, None).unwrap(),
            "remove" => {
                h.native("RemoveUnit", &[Value::Handle(a)]).unwrap();
            }
            "transfer" => {
                let other = h.executed_unit(b).unwrap().owner;
                h.native(
                    "SetUnitOwner",
                    &[Value::Handle(a), Value::Handle(other), Value::Bool(false)],
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let records: Vec<_> = h.executed_units().map(|(id, r)| (id, r.clone())).collect();
        h.return_ai_guard_posts(4).unwrap();
        assert_eq!(
            h.executed_units()
                .map(|(id, r)| (id, r.clone()))
                .collect::<Vec<_>>(),
            records
        );
    }
}

#[test]
fn return_guard_posts_refusals_preserve_the_unit_and_allow_a_later_retry() {
    for problem in ["work", "identity", "order", "paused"] {
        let (mut vm, a, _) = fixture_with(Some(problem));
        let h = &mut vm.host().inner;
        displace(h, a);
        match problem {
            "work" => h.test_set_route_work(0),
            "identity" | "order" => {}
            "paused" => {
                h.native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let record = h.executed_unit(a).unwrap().clone();
        let handles = h.inner().handles();
        assert!(
            h.return_ai_guard_posts(4)
                .unwrap_err()
                .to_string()
                .contains("ReturnGuardPosts")
        );
        assert_eq!(h.executed_unit(a).unwrap(), &record);
        assert_eq!(h.inner().handles(), handles);
        assert!(h.return_ai_guard_posts(5).is_err());
        if problem == "work" {
            h.test_set_route_work(65536);
            h.return_ai_guard_posts(4).unwrap();
            assert!(
                h.executed_unit(a)
                    .unwrap()
                    .activity
                    .as_ref()
                    .unwrap()
                    .moving()
            );
        }
    }
}

#[test]
fn explicit_move_completion_and_hold_do_not_become_automatic_guard_returns() {
    for hold in [false, true] {
        let (mut vm, a, b) = fixture();
        let h = &mut vm.host().inner;
        let home = h.guard_state().unwrap().get(a).unwrap().home;
        h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
        displace(h, a);
        let destination = if hold {
            assert_eq!(
                h.native(
                    "IssueImmediateOrder",
                    &[Value::Handle(a), Value::Str("alpha".into())]
                )
                .unwrap(),
                Value::Bool(true)
            );
            [48., 16.]
        } else {
            assert_eq!(
                h.native(
                    "IssuePointOrder",
                    &[
                        Value::Handle(a),
                        Value::Str("gamma".into()),
                        Value::Real(80.),
                        Value::Real(16.)
                    ]
                )
                .unwrap(),
                Value::Bool(true)
            );
            [80., 16.]
        };
        step(&mut vm, 6.);
        let h = &mut vm.host().inner;
        assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), destination) <= 1.);
        assert_eq!(reported_order(h, a), Value::Int(0));
        h.return_ai_guard_posts(4).unwrap();
        assert_eq!(
            h.executed_unit(a)
                .unwrap()
                .activity
                .as_ref()
                .unwrap()
                .destination(),
            Some(home)
        );
        step(&mut vm, 16.);
        assert!(
            distance(
                vm.host().inner.executed_unit(a).unwrap().position.unwrap(),
                home
            ) <= 1.
        );
    }
}

#[test]
fn hidden_return_guard_moves_without_showing_and_can_reveal_midroute_or_at_home() {
    for reveal_at in [0.5, 5.0] {
        let (mut vm, a, b) = fixture();
        let h = &mut vm.host().inner;
        let home = h.guard_state().unwrap().get(a).unwrap().home;
        displace(h, a);
        h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
            .unwrap();
        let start = h.executed_unit(a).unwrap().position.unwrap();
        let control = return_guard_control_state(h, b);
        h.return_ai_guard_posts(4).unwrap();
        let order = h.campaign_mode_policy().unwrap().guard_posts.return_order;
        assert_eq!(reported_order(h, a), Value::Int(order));
        assert!(!h.executed_unit(a).unwrap().shown);
        step(&mut vm, reveal_at);
        let h = &mut vm.host().inner;
        let position = h.executed_unit(a).unwrap().position.unwrap();
        assert!(distance(position, home) < distance(start, home));
        assert!(!h.executed_unit(a).unwrap().shown);
        assert_eq!(return_guard_control_state(h, b), control);
        // A motion commit must retain inactive collision, or this reveal refuses.
        h.native("ShowUnit", &[Value::Handle(a), Value::Bool(true)])
            .unwrap();
        assert!(h.executed_unit(a).unwrap().shown);
        assert_eq!(h.executed_unit(a).unwrap().position, Some(position));
        step(&mut vm, 6.0);
        let h = &mut vm.host().inner;
        assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), home) <= 1.0);
        assert_eq!(reported_order(h, a), Value::Int(0));
        assert_eq!(return_guard_control_state(h, b), control);
    }
}

#[test]
fn return_guard_motion_preserves_visible_collision_and_hidden_nonoccupancy() {
    for hidden in [false, true] {
        let (mut vm, a, b) = fixture();
        let h = &mut vm.host().inner;
        displace(h, a);
        if hidden {
            h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
                .unwrap();
        }
        h.return_ai_guard_posts(4).unwrap();
        step(&mut vm, 5.0);
        let h = &mut vm.host().inner;
        let at = h.executed_unit(a).unwrap().position.unwrap();
        let other = h.executed_unit(b).unwrap().owner;
        let kind = i32::from_be_bytes(h.executed_unit(a).unwrap().definition.rawcode);
        let created = h
            .native(
                "CreateUnit",
                &[
                    Value::Handle(other),
                    Value::Int(kind),
                    Value::Real(at[0]),
                    Value::Real(at[1]),
                    Value::Real(0.0),
                ],
            )
            .unwrap();
        let Value::Handle(created) = created else {
            panic!("created control");
        };
        let position = h.executed_unit(created).unwrap().position.unwrap();
        if hidden {
            assert_eq!(position, at, "hidden motion must not republish collision");
        } else {
            assert_ne!(position, at, "visible motion must retain collision");
        }
    }
}

#[test]
fn hidden_idle_units_are_not_scheduled_without_an_explicit_guard_return() {
    let (mut vm, a, b) = fixture();
    let h = &mut vm.host().inner;
    displace(h, a);
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    let before = h.executed_unit(a).unwrap().position;
    let control = return_guard_control_state(h, b);
    step(&mut vm, 5.0);
    let h = &mut vm.host().inner;
    assert_eq!(h.executed_unit(a).unwrap().position, before);
    assert!(!h.executed_unit(a).unwrap().shown);
    assert_eq!(reported_order(h, a), Value::Int(0));
    assert_eq!(return_guard_control_state(h, b), control);
}

fn return_guard_control_state(
    h: &mut ObjectHost,
    unit: HandleId,
) -> (Option<[f32; 2]>, HandleId, bool, f32, Value) {
    let record = h.executed_unit(unit).unwrap();
    let state = (
        record.position,
        record.owner,
        record.shown,
        record.vitals.unwrap().life,
    );
    (state.0, state.1, state.2, state.3, reported_order(h, unit))
}

#[test]
fn hidden_return_guard_pause_refusal_is_atomic_and_unpause_allows_retry() {
    let (mut vm, a, _) = fixture();
    let h = &mut vm.host().inner;
    displace(h, a);
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    h.native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    let before = h.executed_unit(a).unwrap().clone();
    let error = h.return_ai_guard_posts(4).unwrap_err().to_string();
    assert!(error.contains("paused_since Some") && error.contains("shown false"));
    assert_eq!(h.executed_unit(a).unwrap(), &before);
    h.native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    h.return_ai_guard_posts(4).unwrap();
    assert!(
        h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    assert!(!h.executed_unit(a).unwrap().shown);
}

#[test]
fn hidden_return_guard_does_not_reclaim_dead_removed_or_transferred_members() {
    for action in ["death", "remove", "transfer"] {
        let (mut vm, a, b) = fixture();
        let h = &mut vm.host().inner;
        displace(h, a);
        h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
            .unwrap();
        h.return_ai_guard_posts(4).unwrap();
        match action {
            "death" => h.kill_unit(a, None).unwrap(),
            "remove" => {
                h.native("RemoveUnit", &[Value::Handle(a)]).unwrap();
            }
            "transfer" => {
                let owner = h.executed_unit(b).unwrap().owner;
                h.native(
                    "SetUnitOwner",
                    &[Value::Handle(a), Value::Handle(owner), Value::Bool(false)],
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let records: Vec<_> = h.executed_units().map(|(id, r)| (id, r.clone())).collect();
        h.return_ai_guard_posts(4).unwrap();
        assert_eq!(
            h.executed_units()
                .map(|(id, r)| (id, r.clone()))
                .collect::<Vec<_>>(),
            records
        );
    }
}

#[test]
fn hidden_return_guard_reveal_then_manual_move_supersedes_accepted_home_route() {
    let (mut vm, a, b) = fixture();
    let h = &mut vm.host().inner;
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    displace(h, a);
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    h.return_ai_guard_posts(4).unwrap();
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("gamma".into()),
                Value::Real(80.),
                Value::Real(16.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let accepted = h.executed_unit(a).unwrap().clone();
    h.return_ai_guard_posts(4).unwrap();
    assert_eq!(h.executed_unit(a).unwrap(), &accepted);
    step(&mut vm, 6.0);
    let h = &mut vm.host().inner;
    assert!(distance(h.executed_unit(a).unwrap().position.unwrap(), [80., 16.]) <= 1.);
    assert_eq!(reported_order(h, a), Value::Int(0));
}
