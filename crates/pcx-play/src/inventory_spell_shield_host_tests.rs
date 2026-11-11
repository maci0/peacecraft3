//! A held Spell Shield (`ANss`) where a hosted spell would reach it. The rule under test is the
//! one decoded in `docs/re-notes/passive-held-items.md`, "`ANss`".
use super::*;

/// A unit of `raw` for `player`.
fn unit_for_player(h: &mut ObjectHost, raw: &[u8; 4], player: i32) -> HandleId {
    let owner = h.native("Player", &[Value::Int(player)]).unwrap();
    let Value::Handle(id) = h
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*raw)),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit handle")
    };
    id
}

#[test]
fn an_enemys_targeted_spell_on_a_shield_holder_is_refused_with_the_decoded_reason() {
    let Fixture {
        mut h,
        hero,
        ordinary,
    } = open();
    let enemy = unit_for_player(&mut h, b"H001", 5);
    // Without a shield nothing is consulted.
    assert_eq!(h.refuse_spell_shield_consulted(enemy, hero, "Aslo"), Ok(()));
    let shield = item(&mut h, b"spsh", 0.0);
    assert_eq!(add(&mut h, hero, shield), Ok(Value::Bool(true)));
    let error = h
        .refuse_spell_shield_consulted(enemy, hero, "Aslo")
        .unwrap_err();
    for part in [
        "Aslo",
        "ANss",
        "cooldown 40 s",
        "0xc71310",
        "+0x1d8",
        "+0x380",
    ] {
        assert!(error.contains(part), "{part} missing from {error}");
    }
    // The client's enemy test fails first for a caster of the holder's own side.
    assert_eq!(
        h.refuse_spell_shield_consulted(ordinary, hero, "Aslo"),
        Ok(())
    );
    // The shield protects its holder only.
    assert_eq!(
        h.refuse_spell_shield_consulted(enemy, ordinary, "Aslo"),
        Ok(())
    );
}

#[test]
fn two_shields_on_one_holder_are_refused_where_the_holder_is_read() {
    let Fixture { mut h, hero, .. } = open();
    let enemy = unit_for_player(&mut h, b"H001", 5);
    let first = item(&mut h, b"spsh", 0.0);
    let second = item(&mut h, b"spsh", 4.0);
    assert_eq!(add(&mut h, hero, first), Ok(Value::Bool(true)));
    assert_eq!(add(&mut h, hero, second), Ok(Value::Bool(true)));
    let error = h
        .refuse_spell_shield_consulted(enemy, hero, "Aslo")
        .unwrap_err();
    assert!(error.contains("two Spell Shields"), "{error}");
}

#[test]
fn an_item_with_two_shield_abilities_refuses_the_pickup() {
    let Fixture { mut h, hero, .. } = open();
    let both = item(&mut h, b"spss", 0.0);
    let error = add(&mut h, hero, both).unwrap_err();
    assert!(error.contains("two spell shield abilities"), "{error}");
}
