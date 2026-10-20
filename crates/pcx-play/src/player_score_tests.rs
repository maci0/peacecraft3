use super::*;

fn fields() -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed("64", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "8",
    ));
    fields
}

fn refusal(h: &mut ObjectHost, player: HandleId, counter: &Value) -> String {
    h.native("GetPlayerScore", &[Value::Handle(player), counter.clone()])
        .expect_err("counters without a verified source refuse")
}

#[test]
fn admitted_snapshot_lists_selected_counters_without_values() {
    let (mut vm, _, _) = open(&fields());
    let h = &mut vm.host().inner;
    let snapshot = h.player_score_snapshot(4).unwrap();
    assert_eq!(snapshot.player, 4);
    let mut names: Vec<_> = snapshot.counters.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        ["PLAYER_SCORE_UNITS_KILLED", "PLAYER_SCORE_UNITS_TRAINED"]
    );
    assert!(snapshot.counters.iter().all(|c| c.value.is_none()));
}

#[test]
fn every_selected_counter_refuses_before_and_after_a_combat_death() {
    let (mut vm, a, b) = open(&fields());
    let killed = vm_global(&vm, "PLAYER_SCORE_UNITS_KILLED");
    let trained = vm_global(&vm, "PLAYER_SCORE_UNITS_TRAINED");
    let attacker = vm.host().inner.executed_unit(a).unwrap().owner;
    let h = &mut vm.host().inner;
    assert!(refusal(h, attacker, &killed).contains("PLAYER_SCORE_UNITS_KILLED"));
    assert!(refusal(h, attacker, &trained).contains("PLAYER_SCORE_UNITS_TRAINED"));

    let start = h.timer_game_time().unwrap();
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[Value::Handle(a), Value::Str("alpha".into()), Value::Handle(b)],
        )
        .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, start + 12.0);
    let h = &mut vm.host().inner;
    assert!(h.executed_unit(b).unwrap().combat.as_ref().unwrap().dead);
    assert!(refusal(h, attacker, &killed).contains("PLAYER_SCORE_UNITS_KILLED"));
    assert!(
        h.player_score_snapshot(4)
            .unwrap()
            .counters
            .iter()
            .all(|c| c.value.is_none())
    );
}

#[test]
fn non_player_handle_is_refused_as_a_wrong_family() {
    let (mut vm, a, _) = open(&fields());
    let killed = vm_global(&vm, "PLAYER_SCORE_UNITS_KILLED");
    let h = &mut vm.host().inner;
    assert!(
        h.native("GetPlayerScore", &[Value::Handle(a), killed])
            .is_err()
    );
}
