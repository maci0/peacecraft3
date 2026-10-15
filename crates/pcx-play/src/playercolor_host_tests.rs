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

mod enums {
    use super::super::{EnumHost, Failure, Limits, NATIVE};
    use pcx_script::{Host, Prelude, Value};
    use std::sync::Arc;
    const SOURCE: &str = "synthetic:edition:common:hash";
    const COMMON: &str = "type playercolor extends handle\ntype race extends handle\ntype specialrace extends race\ntype widget extends handle\ntype unit extends widget\nconstant native ConvertPlayerColor takes integer i returns playercolor\nconstant native ConvertRace takes integer i returns race\nconstant native AliasRace takes integer i returns race\nconstant native AcquireUnit takes integer i returns unit\nglobals\nconstant playercolor FIRST_COLOR = ConvertPlayerColor(19)\nconstant playercolor SECOND_COLOR = ConvertPlayerColor(-5)\nconstant race FIRST_RACE = ConvertRace(19)\nconstant race SECOND_RACE = ConvertRace(64)\nconstant race ALIAS_RACE = AliasRace(19)\nconstant race EXTRA_RACE = AliasRace(777)\nconstant unit OBJECT = AcquireUnit(19)\nendglobals\n";
    fn program(common: &str, map: &str) -> Arc<pcx_script::resolve::Program> {
        let common = pcx_script::parse_script(common).unwrap();
        let map = pcx_script::parse_script(map).unwrap();
        Arc::new(
            pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
        )
    }
    fn limits() -> Limits {
        Limits {
            max_handles: 32,
            admission_steps: 1024,
            expression_depth: 32,
            expression_work: 1024,
        }
    }
    fn host() -> EnumHost {
        EnumHost::checked_selected(
            program(COMMON, ""),
            SOURCE,
            &[NATIVE, "ConvertRace", "AliasRace"],
            limits(),
        )
        .unwrap()
    }
    fn id(v: Value) -> pcx_script::HandleId {
        if let Value::Handle(id) = v {
            id
        } else {
            panic!("not a handle")
        }
    }
    fn expression_common(expression: &str) -> String {
        format!(
            "type race extends handle\nconstant native ConvertRace takes integer i returns race\nglobals\nconstant race A = ConvertRace({expression})\nendglobals\n"
        )
    }
    #[test]
    fn family_and_enumerand_identity_is_shared_across_native_aliases_but_never_across_types() {
        let mut h = host();
        let unit = h.reserve_identity("unit").unwrap();
        let color = id(h.convert(&[Value::Int(19)]).unwrap());
        let race = id(h.convert_for("ConvertRace", &[Value::Int(19)]).unwrap());
        let alias = id(h.convert_for("AliasRace", &[Value::Int(19)]).unwrap());
        assert_eq!(race, alias);
        assert_ne!(color, race);
        assert_ne!(unit, color);
        assert_ne!(unit, race);
        assert_eq!(h.symbolic_value(race, "race").unwrap(), 19);
        assert_eq!(h.player_color(color).unwrap(), 19);
        assert_eq!(
            h.player_color(race).unwrap_err().failure,
            Failure::WrongFamily
        );
        assert_eq!(
            h.symbolic_value(unit, "race").unwrap_err().failure,
            Failure::WrongFamily
        );
        assert_eq!(h.handles(), 3);
        for ty in ["race", "specialrace", "playercolor"] {
            assert!(matches!(h.reserve_identity(ty),Err(e) if e.failure==Failure::Type(ty.into())));
        }
        assert_eq!(h.handles(), 3);
    }
    #[test]
    fn per_native_sparse_domains_do_not_expand_from_alias_cache_or_bitmask_combinations() {
        let mut h = host();
        h.convert_for("AliasRace", &[Value::Int(777)]).unwrap();
        assert_eq!(
            h.convert_for("ConvertRace", &[Value::Int(777)])
                .unwrap_err()
                .failure,
            Failure::OutsideDomain(777)
        );
        assert_eq!(
            h.convert_for("ConvertRace", &[Value::Int(19 + 64)])
                .unwrap_err()
                .failure,
            Failure::OutsideDomain(83)
        );
        assert_eq!(
            h.convert_for("ConvertRace", &[Value::Int(20)])
                .unwrap_err()
                .failure,
            Failure::OutsideDomain(20)
        );
        assert_eq!(h.handles(), 1);
    }
    #[test]
    fn closed_integer_arithmetic_uses_existing_vm_wrapping_division_remainder_and_unary_semantics()
    {
        for (expr, expected) in [
            ("2147483647+1", i32::MIN),
            ("$FFFFFFFF", -1),
            ("(65536*65536)-1", -1),
            ("-2147483648 / -1", i32::MIN),
            ("-7 % 3", -1),
            ("+(2*3)", 6),
        ] {
            let mut h = EnumHost::checked_selected(
                program(&expression_common(expr), ""),
                SOURCE,
                &["ConvertRace"],
                limits(),
            )
            .unwrap();
            let handle = id(h
                .convert_for("ConvertRace", &[Value::Int(expected)])
                .unwrap());
            assert_eq!(
                h.symbolic_value(handle, "race").unwrap(),
                expected,
                "{expr}"
            );
            let mut vm = h.into_vm();
            vm.init_globals().unwrap();
            assert_eq!(vm.global("A"), Value::Handle(handle));
        }
        for expr in ["1 / 0", "1 % 0", "1 == 1"] {
            assert!(
                matches!(EnumHost::checked_selected(program(&expression_common(expr),""),SOURCE,&["ConvertRace"],limits()),Err(e) if matches!(e.failure,Failure::Expression(_)))
            );
        }
    }
    #[test]
    fn expression_depth_and_aggregate_work_are_separate_checked_admission_policies() {
        let p = program(&expression_common("((1+2))"), "");
        let mut l = limits();
        l.expression_depth = 3;
        let e = EnumHost::checked_selected(Arc::clone(&p), SOURCE, &["ConvertRace"], l)
            .err()
            .unwrap();
        assert_eq!(e.failure, Failure::Budget("expression depth"));
        assert_eq!(e.native, "ConvertRace");
        assert_eq!(e.initializer.as_ref().unwrap().0, "A");
        l.expression_depth = 4;
        l.expression_work = 4;
        assert!(
            matches!(EnumHost::checked_selected(Arc::clone(&p),SOURCE,&["ConvertRace"],l),Err(e) if e.failure==Failure::Budget("expression work"))
        );
        l.expression_work = 5;
        assert!(EnumHost::checked_selected(p, SOURCE, &["ConvertRace"], l).is_ok());
        let common = expression_common("1")
            .replace("endglobals", "constant race B = ConvertRace(2)\nendglobals");
        l.expression_work = 1;
        assert!(
            matches!(EnumHost::checked_selected(program(&common,""),SOURCE,&["ConvertRace"],l),Err(e) if e.failure==Failure::Budget("expression work"))
        );
    }
    #[test]
    fn source_injection_and_duplicate_declaration_origins_cannot_change_the_admitted_domain() {
        let map = "globals\nconstant race FORGED = ConvertRace(999)\nendglobals\n";
        let mut h =
            EnumHost::checked_selected(program(COMMON, map), SOURCE, &["ConvertRace"], limits())
                .unwrap();
        assert_eq!(
            h.convert_for("ConvertRace", &[Value::Int(999)])
                .unwrap_err()
                .failure,
            Failure::OutsideDomain(999)
        );
        let map = "constant native ConvertRace takes integer i returns race\n";
        assert!(
            matches!(EnumHost::checked_selected(program(COMMON,map),SOURCE,&["ConvertRace"],limits()),Err(e) if e.failure==Failure::Source)
        );
    }
    #[test]
    fn closed_domain_admission_does_not_execute_identifiers_functions_or_other_natives() {
        for expr in ["UnknownInteger()", "UNKNOWN_VALUE"] {
            let common=expression_common(expr).replacen("globals\n","constant native UnknownInteger takes nothing returns integer\nglobals\nconstant integer UNKNOWN_VALUE = 9\n",1);
            assert!(
                matches!(EnumHost::checked_selected(program(&common,""),SOURCE,&["ConvertRace"],limits()),Err(e) if matches!(e.failure,Failure::Expression(_)))
            );
        }
        let mut h = host();
        assert!(h.native("AcquireUnit", &[Value::Int(19)]).is_err());
        assert_eq!(h.handles(), 0);
    }
    #[test]
    fn nominal_identity_capacity_counts_shared_aliases_once_and_preserves_existing_handles_at_capacity()
     {
        let common = "type race extends handle\nconstant native ConvertRace takes integer i returns race\nconstant native AliasRace takes integer i returns race\nglobals\nconstant race A = ConvertRace(8)\nconstant race B = AliasRace(8)\nendglobals\n";
        let mut l = limits();
        l.max_handles = 1;
        let mut h = EnumHost::checked_selected(
            program(common, ""),
            SOURCE,
            &["ConvertRace", "AliasRace"],
            l,
        )
        .unwrap();
        let first = h.convert_for("ConvertRace", &[Value::Int(8)]).unwrap();
        assert_eq!(first, h.convert_for("AliasRace", &[Value::Int(8)]).unwrap());
        assert_eq!(h.handles(), 1);
        assert!(h.reserve_identity("handle").is_err());
        assert_eq!(first, h.convert_for("AliasRace", &[Value::Int(8)]).unwrap());
        let families = "type first extends handle\ntype second extends handle\nconstant native First takes integer i returns first\nconstant native Second takes integer i returns second\nglobals\nconstant first A = First(8)\nconstant second B = Second(8)\nendglobals\n";
        assert!(
            matches!(EnumHost::checked_selected(program(families,""),SOURCE,&["First","Second"],l),Err(e) if e.failure==Failure::Capacity)
        );
        let mut l = limits();
        l.max_handles = 5;
        let mut h = EnumHost::checked_selected(
            program(COMMON, ""),
            SOURCE,
            &[NATIVE, "ConvertRace", "AliasRace"],
            l,
        )
        .unwrap();
        let unit = h.reserve_identity("unit").unwrap();
        h.convert(&[Value::Int(19)]).unwrap();
        h.convert(&[Value::Int(-5)]).unwrap();
        h.convert_for("ConvertRace", &[Value::Int(19)]).unwrap();
        h.convert_for("ConvertRace", &[Value::Int(64)]).unwrap();
        let error = h.convert_for("AliasRace", &[Value::Int(777)]).unwrap_err();
        assert_eq!(error.failure, Failure::Capacity);
        assert_eq!(error.native, "AliasRace");
        assert_eq!(error.declaration_source, "synthetic:common");
        assert_eq!(h.kind(unit).unwrap(), "unit");
        assert_eq!(h.handles(), 5);
        assert!(h.convert_for("AliasRace", &[Value::Int(19)]).is_ok());
    }
    #[test]
    fn selected_declarations_must_be_constant_scalar_integer_to_named_handle_types_without_function_overrides()
     {
        for common in [
            COMMON.replace("constant native ConvertRace", "native ConvertRace"),
            COMMON.replace("ConvertRace takes integer i", "ConvertRace takes real i"),
            COMMON.replace("ConvertRace takes integer i", "ConvertRace takes nothing"),
            COMMON.replace(
                "ConvertRace takes integer i returns race",
                "ConvertRace takes integer i returns integer",
            ),
        ] {
            assert!(
                matches!(EnumHost::checked_selected(program(&common,""),SOURCE,&["ConvertRace"],limits()),Err(e) if e.failure==Failure::Signature)
            );
        }
        let map = "function ConvertRace takes integer i returns race\nreturn null\nendfunction\n";
        assert!(
            EnumHost::checked_selected(program(COMMON, map), SOURCE, &["ConvertRace"], limits())
                .is_err()
        );
        assert!(
            EnumHost::checked_selected(program(COMMON, ""), SOURCE, &["AbsentNative"], limits())
                .is_err()
        );
        let mut h = EnumHost::checked_selected(program(COMMON, ""), SOURCE, &[], limits()).unwrap();
        assert!(h.native(NATIVE, &[Value::Int(19)]).is_err());
        assert_eq!(h.handles(), 0);
    }
    #[test]
    fn compiled_vm_initializes_all_selected_symbolic_globals_and_refuses_the_next_object_native() {
        let mut vm = host().into_vm();
        let error = vm.init_globals().unwrap_err();
        assert!(error.to_string().contains("AcquireUnit"));
        assert_eq!(vm.global("FIRST_RACE"), vm.global("ALIAS_RACE"));
        assert_ne!(vm.global("FIRST_RACE"), vm.global("FIRST_COLOR"));
        assert_eq!(vm.host().handles(), 5);
        assert_eq!(error, vm.init_globals().unwrap_err());
        assert_eq!(vm.host().handles(), 5);
        assert_eq!(vm.global("OBJECT"), Value::Null);
    }

    #[test]
    fn color_compatibility_never_interprets_another_named_family_as_playercolor() {
        let common = expression_common("19").replace("ConvertRace", NATIVE);
        let p = program(&common, "");
        assert!(matches!(
            EnumHost::checked(Arc::clone(&p), SOURCE, 32),
            Err(e) if e.failure == Failure::Signature
        ));
        let mut h = EnumHost::checked_selected(p, SOURCE, &[NATIVE], limits()).unwrap();
        let race = id(h.convert_for(NATIVE, &[Value::Int(19)]).unwrap());
        assert_eq!(h.symbolic_value(race, "race").unwrap(), 19);
        assert_eq!(h.domain().count(), 0);
        assert_eq!(
            h.player_color(race).unwrap_err().failure,
            Failure::Signature
        );
        assert_eq!(
            h.convert(&[Value::Int(19)]).unwrap_err().failure,
            Failure::Signature
        );
    }

    #[test]
    fn admission_counts_duplicate_names_and_unrelated_globals_before_filtering() {
        let common = expression_common("19").replace(
            "endglobals",
            "integer UNRELATED = 7\ninteger ALSO_UNRELATED = 8\nendglobals",
        );
        let p = program(&common, "");
        let names = ["ConvertRace", "ConvertRace"];
        let exact = names.len() + p.globals().len();
        for steps in 0..exact {
            let mut l = limits();
            l.admission_steps = steps;
            assert!(
                matches!(
                    EnumHost::checked_selected(Arc::clone(&p), SOURCE, &names, l),
                    Err(e) if e.failure == Failure::Budget("admission steps")
                ),
                "steps={steps}"
            );
        }
        let mut l = limits();
        l.admission_steps = exact;
        assert!(EnumHost::checked_selected(Arc::clone(&p), SOURCE, &names, l).is_ok());
        l.admission_steps = 0;
        assert!(EnumHost::checked_selected(p, SOURCE, &[], l).is_ok());
    }

    #[test]
    fn admission_counts_a_separate_global_scan_for_each_selected_alias() {
        let common = expression_common("19")
            .replacen(
                "globals\n",
                "constant native AliasRace takes integer i returns race\nglobals\n",
                1,
            )
            .replace("endglobals", "constant race B = AliasRace(19)\nendglobals");
        let p = program(&common, "");
        let names = ["ConvertRace", "AliasRace"];
        let mut l = limits();
        l.admission_steps = names.len() + names.len() * p.globals().len() - 1;
        assert!(matches!(
            EnumHost::checked_selected(Arc::clone(&p), SOURCE, &names, l),
            Err(e) if e.failure == Failure::Budget("admission steps")
        ));
        l.admission_steps += 1;
        assert!(EnumHost::checked_selected(p, SOURCE, &names, l).is_ok());
    }
}
