use super::*;

const MAP: &str = "globals\nplayer P = null\nunit U = null\nendglobals\nfunction setup takes nothing returns nothing\nset P = Player(4)\nset U = CreateUnit(P, 'hfoo', 1.5, -2.25, 90.0)\nendfunction\n";

fn open_extra(extra: &str) -> VmHost {
    let common = script_common_with("widget", true, extra);
    let ctx = context(300, &common, MAP, "unit-catalog");
    let (catalog, hits, seen) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let host = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&shared), source(), limits(4))
        .unwrap();
    VmHost {
        vm: Arc::clone(&ctx.scripts).owned_vm(host),
        hits,
        seen,
        catalog: shared,
    }
}

const RESERVE: &str = "native SetReservedLocalHeroButtons takes integer reserved returns nothing\n";

#[test]
fn reserved_local_hero_buttons_stores_the_integer_unclamped() {
    let mut s = open_extra(RESERVE);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    assert_eq!(host.reserved_local_hero_buttons(), None);
    for value in [2, 0, -3, i32::MAX] {
        assert_eq!(
            host.native("SetReservedLocalHeroButtons", &[Value::Int(value)])
                .unwrap(),
            Value::Null
        );
        assert_eq!(host.reserved_local_hero_buttons(), Some(value));
    }
}

#[test]
fn reserved_local_hero_buttons_refuses_other_argument_shapes_and_keeps_the_value() {
    let mut s = open_extra(RESERVE);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    host.native("SetReservedLocalHeroButtons", &[Value::Int(2)])
        .unwrap();
    for args in [
        &[][..],
        &[Value::Real(1.)][..],
        &[Value::Int(1), Value::Int(1)][..],
    ] {
        assert!(host.native("SetReservedLocalHeroButtons", args).is_err());
    }
    assert_eq!(host.reserved_local_hero_buttons(), Some(2));
}

#[test]
fn reserved_local_hero_buttons_refuses_a_declaration_with_another_signature() {
    let common = script_common_with(
        "widget",
        true,
        "native SetReservedLocalHeroButtons takes real reserved returns nothing\n",
    );
    let ctx = context(300, &common, MAP, "unit-catalog");
    let (catalog, ..) = counting();
    let shared: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let admitted = object(&ctx, 16)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(shared, source(), limits(4));
    assert!(admitted.is_err());
}

#[test]
fn a_program_without_the_declaration_leaves_the_native_unsupported() {
    let mut s = open_extra("");
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    let error = host
        .native("SetReservedLocalHeroButtons", &[Value::Int(2)])
        .unwrap_err();
    assert!(error.contains("unsupported"), "{error}");
    assert_eq!(host.reserved_local_hero_buttons(), None);
}
