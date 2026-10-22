use super::*;
use crate::playercolor_host::{ActiveBuff, BuffTraits};

fn buff(rawcode: [u8; 4], traits: BuffTraits) -> ActiveBuff {
    ActiveBuff::new(rawcode, traits)
}

fn positive(rawcode: [u8; 4]) -> ActiveBuff {
    buff(rawcode, BuffTraits { positive: true, ..Default::default() })
}

fn negative(rawcode: [u8; 4]) -> ActiveBuff {
    buff(rawcode, BuffTraits { negative: true, ..Default::default() })
}

#[test]
fn unit_remove_buffs_maps_bool_arguments_to_polarity() {
    let (mut vm, a, b) = open_clocked(&armed("64", "normal"), rules());
    let h = &mut vm.host().inner;
    h.apply_unit_buff(a, positive(*b"BHab")).unwrap();
    h.apply_unit_buff(b, negative(*b"Bcor")).unwrap();

    assert_eq!(
        h.native("UnitRemoveBuffs", &[Value::Handle(a), Value::Bool(true), Value::Bool(false)])
            .unwrap(),
        Value::Null
    );
    assert!(h.active_unit_buffs(a).unwrap().is_empty());

    assert_eq!(
        h.native("UnitRemoveBuffs", &[Value::Handle(b), Value::Bool(true), Value::Bool(false)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(h.active_unit_buffs(b).unwrap(), vec![negative(*b"Bcor")]);

    assert_eq!(
        h.native("UnitRemoveBuffs", &[Value::Handle(b), Value::Bool(false), Value::Bool(true)])
            .unwrap(),
        Value::Null
    );
    assert!(h.active_unit_buffs(b).unwrap().is_empty());
}

#[test]
fn unit_remove_buffs_ex_respects_extra_filters() {
    let (mut vm, a, _) = open_clocked(&armed("64", "normal"), rules());
    let h = &mut vm.host().inner;
    let timed = buff(*b"BTLf", BuffTraits { positive: true, timed_life: true, ..Default::default() });
    let aura = buff(*b"BHab", BuffTraits { positive: true, aura: true, ..Default::default() });
    h.apply_unit_buff(a, timed).unwrap();
    h.apply_unit_buff(a, aura).unwrap();

    let remove_positive_without_life = |h: &mut crate::playercolor_host::ObjectHost,
                                        timed_life: bool,
                                        aura: bool| {
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
    assert_eq!(h.active_unit_buffs(a).unwrap(), vec![timed, aura]);

    assert_eq!(remove_positive_without_life(h, false, true), Value::Null);
    assert_eq!(h.active_unit_buffs(a).unwrap(), vec![timed]);
}

#[test]
fn unit_remove_buffs_null_removed_and_wrong_arity_are_no_writes() {
    let (mut vm, a, b) = open_clocked(&armed("64", "normal"), rules());
    let h = &mut vm.host().inner;
    h.apply_unit_buff(a, positive(*b"BHab")).unwrap();
    h.apply_unit_buff(b, negative(*b"Bcor")).unwrap();

    assert_eq!(
        h.native("UnitRemoveBuffs", &[Value::Null, Value::Bool(true), Value::Bool(true)]).unwrap(),
        Value::Null
    );
    assert!(h.native("UnitRemoveBuffs", &[Value::Handle(a), Value::Bool(true)]).is_err());
    assert!(h.native("UnitRemoveBuffs", &[Value::Handle(a), Value::Int(1), Value::Bool(true)]).is_err());
    assert_eq!(h.active_unit_buffs(a).unwrap(), vec![positive(*b"BHab")]);
    assert_eq!(h.active_unit_buffs(b).unwrap(), vec![negative(*b"Bcor")]);

    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    assert!(h.native("UnitRemoveBuffs", &[player, Value::Bool(true), Value::Bool(true)]).is_err());
    assert_eq!(h.active_unit_buffs(b).unwrap(), vec![negative(*b"Bcor")]);
}
