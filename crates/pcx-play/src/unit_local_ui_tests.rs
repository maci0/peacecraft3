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

const ALT_ICON: &str = "native SetAltMinimapIcon takes string iconPath returns nothing\n";

#[test]
fn alt_minimap_icon_keeps_the_last_accepted_path() {
    let mut s = open_extra(ALT_ICON);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    assert_eq!(host.alt_minimap_icon(), None);
    host.native("SetAltMinimapIcon", &[Value::Str("UI\\Minimap\\A".into())])
        .unwrap();
    assert_eq!(host.alt_minimap_icon(), Some("UI\\Minimap\\A"));
    host.native("SetAltMinimapIcon", &[Value::Str("UI\\Minimap\\B".into())])
        .unwrap();
    assert_eq!(host.alt_minimap_icon(), Some("UI\\Minimap\\B"));
}

#[test]
fn alt_minimap_icon_ignores_a_null_or_empty_path_instead_of_clearing() {
    let mut s = open_extra(ALT_ICON);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    host.native("SetAltMinimapIcon", &[Value::Null]).unwrap();
    host.native("SetAltMinimapIcon", &[Value::Str("".into())])
        .unwrap();
    assert_eq!(host.alt_minimap_icon(), None);
    host.native("SetAltMinimapIcon", &[Value::Str("x".into())])
        .unwrap();
    host.native("SetAltMinimapIcon", &[Value::Null]).unwrap();
    host.native("SetAltMinimapIcon", &[Value::Str("".into())])
        .unwrap();
    assert_eq!(host.alt_minimap_icon(), Some("x"));
}

#[test]
fn alt_minimap_icon_refuses_trigger_strings_and_bad_arguments_without_a_write() {
    let mut s = open_extra(ALT_ICON);
    s.vm.init_globals().unwrap();
    let host = &mut s.vm.host().inner;
    host.native("SetAltMinimapIcon", &[Value::Str("keep".into())])
        .unwrap();
    let trigger = host
        .native("SetAltMinimapIcon", &[Value::Str("trigstr_12".into())])
        .unwrap_err();
    assert!(trigger.contains("map string table"), "{trigger}");
    for args in [&[][..], &[Value::Int(1)][..]] {
        assert!(host.native("SetAltMinimapIcon", args).is_err());
    }
    assert_eq!(host.alt_minimap_icon(), Some("keep"));
}
