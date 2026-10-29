use super::*;
use crate::playercolor_host::{ActiveBuff, BuffTraits};

fn buff(rawcode: [u8; 4], traits: BuffTraits) -> ActiveBuff {
    ActiveBuff::new(rawcode, traits)
}

fn positive(rawcode: [u8; 4]) -> ActiveBuff {
    buff(
        rawcode,
        BuffTraits {
            positive: true,
            ..Default::default()
        },
    )
}

fn negative(rawcode: [u8; 4]) -> ActiveBuff {
    buff(
        rawcode,
        BuffTraits {
            negative: true,
            ..Default::default()
        },
    )
}

#[cfg(feature = "test-fixtures")]
#[test]
fn placement_diagnostic_reports_live_buff_ids() {
    use crate::ai_placement::PlacementEventKind;
    let (mut vm, a, _) = open_clocked(&armed("64", "normal"), rules());
    let h = &mut vm.host().inner;
    assert!(h.diagnostic_ai_buffs(a).unwrap().is_empty());
    h.attach_unit_buff(a, negative(*b"Bxyz")).unwrap();
    assert_eq!(h.diagnostic_ai_buffs(a).unwrap(), vec![*b"Bxyz"]);
    assert!(h.detach_unit_buff(a, *b"Bxyz"));
    assert!(h.diagnostic_ai_buffs(a).unwrap().is_empty());
    assert!(
        h.placement_events(a)
            .unwrap()
            .all(|event| event.game_time == h.timer_game_time().unwrap())
    );
    assert_eq!(
        h.placement_events(a)
            .unwrap()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        [
            PlacementEventKind::BuffApplied(*b"Bxyz"),
            PlacementEventKind::BuffRemoved(*b"Bxyz")
        ]
    );
}

#[test]
fn unit_remove_buffs_maps_bool_arguments_to_polarity() {
    let (mut vm, a, b) = open_clocked(&armed("64", "normal"), rules());
    let h = &mut vm.host().inner;
    h.attach_unit_buff(a, positive(*b"BHab")).unwrap();
    h.attach_unit_buff(b, negative(*b"Bcor")).unwrap();
    assert!(h.has_unqualified_motion_buff(a));
    assert!(h.has_unqualified_motion_buff(b));

    assert_eq!(
        h.native(
            "UnitRemoveBuffs",
            &[Value::Handle(a), Value::Bool(true), Value::Bool(false)]
        )
        .unwrap(),
        Value::Null
    );
    assert!(h.unit_buffs(a).unwrap().is_empty());

    assert_eq!(
        h.native(
            "UnitRemoveBuffs",
            &[Value::Handle(b), Value::Bool(true), Value::Bool(false)]
        )
        .unwrap(),
        Value::Null
    );
    assert_eq!(h.unit_buffs(b).unwrap(), vec![negative(*b"Bcor")]);

    assert_eq!(
        h.native(
            "UnitRemoveBuffs",
            &[Value::Handle(b), Value::Bool(false), Value::Bool(true)]
        )
        .unwrap(),
        Value::Null
    );
    assert!(h.unit_buffs(b).unwrap().is_empty());
}

#[test]
fn unit_remove_buffs_ex_respects_extra_filters() {
    let (mut vm, a, _) = open_clocked(&armed("64", "normal"), rules());
    let h = &mut vm.host().inner;
    let timed = buff(
        *b"BTLf",
        BuffTraits {
            positive: true,
            timed_life: true,
            ..Default::default()
        },
    );
    let aura = buff(
        *b"BHab",
        BuffTraits {
            positive: true,
            aura: true,
            ..Default::default()
        },
    );
    h.attach_unit_buff(a, timed).unwrap();
    h.attach_unit_buff(a, aura).unwrap();

    let remove_positive_without_life =
        |h: &mut crate::playercolor_host::ObjectHost, timed_life: bool, aura: bool| {
            h.native(
                "UnitRemoveBuffsEx",
                &[
                    Value::Handle(a),
                    Value::Bool(true),
                    Value::Bool(false),
                    Value::Bool(false),
                    Value::Bool(false),
                    Value::Bool(timed_life),
                    Value::Bool(aura),
                    Value::Bool(false),
                ],
            )
            .unwrap()
        };

    assert_eq!(remove_positive_without_life(h, false, false), Value::Null);
    assert_eq!(h.unit_buffs(a).unwrap(), vec![timed, aura]);

    assert_eq!(remove_positive_without_life(h, false, true), Value::Null);
    assert_eq!(h.unit_buffs(a).unwrap(), vec![timed]);
}

#[test]
fn unit_remove_buffs_null_removed_and_wrong_arity_are_no_writes() {
    let (mut vm, a, b) = open_clocked(&armed("64", "normal"), rules());
    let h = &mut vm.host().inner;
    h.attach_unit_buff(a, positive(*b"BHab")).unwrap();
    h.attach_unit_buff(b, negative(*b"Bcor")).unwrap();

    assert_eq!(
        h.native(
            "UnitRemoveBuffs",
            &[Value::Null, Value::Bool(true), Value::Bool(true)]
        )
        .unwrap(),
        Value::Null
    );
    assert!(
        h.native("UnitRemoveBuffs", &[Value::Handle(a), Value::Bool(true)])
            .is_err()
    );
    assert!(
        h.native(
            "UnitRemoveBuffs",
            &[Value::Handle(a), Value::Int(1), Value::Bool(true)]
        )
        .is_err()
    );
    assert_eq!(h.unit_buffs(a).unwrap(), vec![positive(*b"BHab")]);
    assert_eq!(h.unit_buffs(b).unwrap(), vec![negative(*b"Bcor")]);

    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    assert!(
        h.native(
            "UnitRemoveBuffs",
            &[player, Value::Bool(true), Value::Bool(true)]
        )
        .is_err()
    );
    assert_eq!(h.unit_buffs(b).unwrap(), vec![negative(*b"Bcor")]);
}

#[test]
fn attach_unit_buff_refuses_removed_units_and_captures_per_unit() {
    let (mut vm, a, b) = open_clocked(&armed("64", "normal"), rules());
    let h = &mut vm.host().inner;
    h.attach_unit_buff(a, positive(*b"BHab")).unwrap();

    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    assert_eq!(h.unit_buffs(b).unwrap(), Vec::<ActiveBuff>::new());
    assert!(h.attach_unit_buff(b, negative(*b"Bcor")).is_err());
    assert_eq!(h.unit_buffs(b).unwrap(), Vec::<ActiveBuff>::new());
    assert_eq!(h.unit_buffs(a).unwrap(), vec![positive(*b"BHab")]);

    let cap = crate::playercolor_host::MAX_BUFFS_PER_UNIT;
    let holder = h;
    for _ in 0..(cap - 1) {
        holder.attach_unit_buff(a, positive(*b"BHba")).unwrap();
    }
    assert!(holder.attach_unit_buff(a, positive(*b"BHbb")).is_err());
}
