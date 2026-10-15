use super::{ColorHost, Failure, NATIVE};
use pcx_script::{HandleId, Host, Prelude, Value};
use std::sync::Arc;
const COMMON: &str = "type playercolor extends handle\ntype widget extends handle\ntype unit extends widget\ntype specialcolor extends playercolor\nconstant native ConvertPlayerColor takes integer i returns playercolor\nglobals\nconstant playercolor FIRST = ConvertPlayerColor(19)\nconstant playercolor SECOND = ConvertPlayerColor(-5)\nconstant playercolor ALIAS = ConvertPlayerColor(19)\nendglobals\n";
fn program(common: &str, map: &str) -> Arc<pcx_script::resolve::Program> {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    Arc::new(pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap())
}
fn host(max: usize) -> ColorHost {
    ColorHost::checked(program(COMMON, ""), "synthetic:edition:member:hash", max).unwrap()
}
fn id(value: Value) -> HandleId {
    if let Value::Handle(id) = value {
        id
    } else {
        panic!("expected handle")
    }
}
#[test]
fn domain_is_authored_sparse_and_conversion_is_interned_typed_identity() {
    let mut h = host(8);
    assert_eq!(h.domain().collect::<Vec<_>>(), vec![-5, 19]);
    let a = id(h.convert(&[Value::Int(19)]).unwrap());
    let b = id(h.convert(&[Value::Int(-5)]).unwrap());
    assert_ne!(a, b);
    assert_eq!(h.convert(&[Value::Int(19)]).unwrap(), Value::Handle(a));
    assert_eq!(h.player_color(a).unwrap(), 19);
    assert_eq!(h.kind(a).unwrap(), "playercolor");
    assert_eq!(h.handles(), 2);
    assert!(h.convert(&[Value::Int(0)]).is_err());
    assert!(
        pcx_script::value::binary(
            pcx_script::ast::BinOp::Eq,
            &Value::Handle(a),
            &Value::Int(19)
        )
        .is_err()
    );
}
#[test]
fn interleaved_unit_and_color_identities_share_one_collision_free_arena() {
    let mut h = host(8);
    let unit = h.reserve_identity("unit").unwrap();
    let color = id(h.convert(&[Value::Int(19)]).unwrap());
    let widget = h.reserve_identity("widget").unwrap();
    assert_ne!(unit, color);
    assert_ne!(color, widget);
    assert_ne!(unit, widget);
    assert_eq!(h.kind(unit).unwrap(), "unit");
    assert_eq!(h.kind(widget).unwrap(), "widget");
    assert_eq!(
        h.player_color(unit).unwrap_err().failure,
        Failure::WrongFamily
    );
    assert_eq!(h.convert(&[Value::Int(19)]).unwrap(), Value::Handle(color));
    for ty in ["playercolor", "specialcolor", "absent", "integer"] {
        assert!(h.reserve_identity(ty).is_err());
    }
    assert_eq!(h.handles(), 3);
}
#[test]
fn capacity_failure_preserves_existing_identity_and_never_recycles_tokens() {
    let mut h = host(2);
    let unit = h.reserve_identity("unit").unwrap();
    let color = id(h.convert(&[Value::Int(19)]).unwrap());
    assert_eq!(
        h.convert(&[Value::Int(-5)]).unwrap_err().failure,
        Failure::Capacity
    );
    assert_eq!(h.handles(), 2);
    assert_eq!(h.kind(unit).unwrap(), "unit");
    assert_eq!(h.convert(&[Value::Int(19)]).unwrap(), Value::Handle(color));
    assert!(ColorHost::checked(program(COMMON, ""), "synthetic", 1).is_err());
}
#[test]
fn wrong_args_and_outside_domain_refuse_without_minting_values() {
    let mut h = host(8);
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Real(19.)],
        vec![Value::Int(19), Value::Int(19)],
    ] {
        assert_eq!(h.convert(&args).unwrap_err().failure, Failure::Arguments);
    }
    let e = h.convert(&[Value::Int(18)]).unwrap_err();
    assert_eq!(e.failure, Failure::OutsideDomain(18));
    assert_eq!(e.source_context, "synthetic:edition:member:hash");
    assert_eq!(e.declaration_source, "synthetic:common");
    assert!(e.declaration_span.end > e.declaration_span.start);
    assert_eq!(h.handles(), 0);
}
#[test]
fn declarations_and_unsupported_constant_expression_shapes_are_admitted_explicitly() {
    for common in [
        COMMON.replace("takes integer i", "takes nothing"),
        COMMON.replace("returns playercolor", "returns integer"),
        COMMON.replace("ConvertPlayerColor(19)", "null"),
        COMMON.replace("constant playercolor FIRST", "playercolor FIRST"),
    ] {
        assert!(ColorHost::checked(program(&common, ""), "synthetic", 8).is_err());
    }
    let mut h = ColorHost::checked(
        program(
            &COMMON.replace("ConvertPlayerColor(19)", "ConvertPlayerColor(($13))"),
            "",
        ),
        "synthetic",
        8,
    )
    .unwrap();
    assert!(h.convert(&[Value::Int(19)]).is_ok());
}
#[test]
fn map_constants_cannot_expand_the_common_authored_domain() {
    let map = "globals\nconstant playercolor EXTRA = ConvertPlayerColor(91)\nendglobals\n";
    let h = ColorHost::checked(program(COMMON, map), "synthetic", 8).unwrap();
    assert_eq!(h.domain().collect::<Vec<_>>(), vec![-5, 19]);
    let mut vm = h.into_vm();
    let e = vm.init_globals().unwrap_err();
    assert!(e.to_string().contains("91"));
    assert_eq!(vm.host().handles(), 2);
}
#[test]
fn compiled_vm_runs_authored_color_globals_then_reports_next_native_and_caches_failure() {
    let map = "native NextNative takes nothing returns integer\nglobals\ninteger BLOCKER = NextNative()\nendglobals\n";
    let h = ColorHost::checked(program(COMMON, map), "synthetic", 8).unwrap();
    let mut vm = h.into_vm();
    let first = vm.init_globals().unwrap_err();
    assert!(first.to_string().contains("NextNative"));
    assert_eq!(vm.global("FIRST"), vm.global("ALIAS"));
    assert_ne!(vm.global("FIRST"), vm.global("SECOND"));
    assert!(matches!(vm.global("FIRST"), Value::Handle(_)));
    assert_eq!(vm.host().handles(), 2);
    let again = vm.init_globals().unwrap_err();
    assert_eq!(first, again);
    assert_eq!(vm.host().handles(), 2);
}
#[test]
fn undeclared_or_ambiguous_native_and_script_function_override_do_not_install_conversion() {
    let empty = program("", "");
    assert!(ColorHost::checked(empty, "synthetic", 8).is_err());
    let common = pcx_script::parse_script(COMMON).unwrap();
    let other =
        pcx_script::parse_script("native ConvertPlayerColor takes real i returns playercolor\n")
            .unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let p = pcx_script::resolve_with(
        &map,
        &[
            Prelude::new("synthetic:common", &common),
            Prelude::new("synthetic:other", &other),
        ],
    )
    .unwrap();
    assert!(ColorHost::checked(Arc::new(p), "synthetic", 8).is_err());
    let map = "function ConvertPlayerColor takes integer i returns playercolor\nreturn null\nendfunction\n";
    assert!(ColorHost::checked(program(COMMON, map), "synthetic", 8).is_err());
}
#[test]
fn host_dispatch_keeps_other_natives_explicitly_unsupported() {
    let mut h = host(8);
    assert!(h.native("CreateUnit", &[]).is_err());
    assert_eq!(h.handles(), 0);
    assert!(h.native(NATIVE, &[Value::Int(19)]).is_ok());
}
