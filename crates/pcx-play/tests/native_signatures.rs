//! Supplied synthetic declarations exercise existing conversion dispatch, not stock/native coverage.
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::natives::{NativeCall, NativeImpl};
use pcx_script::{Host, NullHost, Value};
fn compile(common: &str, map: &str) -> MapScripts {
    MapScripts::compile(
        AssetIdentity {
            edition: "synthetic supplied edition".into(),
            version: "synthetic-v1".into(),
            source: "caller-memory".into(),
        },
        ScriptAsset {
            label: "caller/archive/common.j",
            bytes: common.as_bytes(),
        },
        ScriptAsset {
            label: "caller/archive/Blizzard.j",
            bytes: b"function Other takes nothing returns nothing\nendfunction\n",
        },
        ScriptAsset {
            label: "caller/map/war3map.j",
            bytes: map.as_bytes(),
        },
        None,
    )
    .unwrap()
}
struct RefusingHost(usize);
impl Host for RefusingHost {
    fn native(&mut self, _: &str, _: &[Value]) -> Result<Value, String> {
        self.0 += 1;
        Err("synthetic host refused".into())
    }
}
#[test]
fn incompatible_argument_count_type_and_result_signatures_refuse_before_intrinsic_or_host_dispatch()
{
    for (declaration, call) in [
        ("native I2R takes nothing returns real\n", "I2R()"),
        (
            "native I2R takes integer a, integer b returns real\n",
            "I2R(17,19)",
        ),
        ("native I2R takes real value returns real\n", "I2R(17)"),
        (
            "native I2R takes integer value returns boolean\n",
            "I2R(17)",
        ),
        ("native R2I takes nothing returns integer\n", "R2I()"),
        (
            "native R2I takes integer value returns integer\n",
            "R2I(17.5)",
        ),
        ("native R2I takes real value returns real\n", "R2I(17.5)"),
    ] {
        let map = format!("function main takes nothing returns real\nreturn {call}\nendfunction\n");
        let scripts = compile(declaration, &map);
        let name = if call.starts_with("I2R") {
            "I2R"
        } else {
            "R2I"
        };
        assert_eq!(
            scripts
                .program()
                .unimplemented_natives()
                .collect::<Vec<_>>(),
            vec![(name, 1)]
        );
        let n = match scripts.program().natives().call(name) {
            Some(NativeCall::SignatureMismatch(n, _)) => n,
            other => panic!("wrong coverage {other:?}"),
        };
        assert_eq!(n.source, "caller/archive/common.j");
        assert_eq!(n.span.start, 0);
        assert_eq!(
            &declaration[n.span.start as usize..n.span.end as usize],
            declaration.trim_end()
        );
        let signature = pcx_script::natives::signature_of(&n.name, &n.params, &n.ret);
        let mut vm = scripts.vm(RefusingHost(0));
        let error = vm.call("main", &[]).unwrap_err();
        assert!(error.message.contains(&signature));
        assert!(error.message.contains("incompatible"));
        assert!(error.message.contains("caller/archive/common.j:0.."));
        let diagnostic = scripts.diagnostic(&vm, &error);
        assert!(diagnostic.starts_with("caller/map/war3map.j:"));
        assert!(diagnostic.contains(&signature), "{diagnostic}");
        assert_eq!(
            &map[error.span.start as usize..error.span.end as usize],
            call
        );
        assert_eq!(vm.host().inner.0, 0);
    }
}
#[test]
fn exact_supplied_contracts_keep_both_existing_conversion_results() {
    for (declaration, call, returns, implementation, expected) in [
        (
            "constant native I2R takes integer authoredName returns real\n",
            "I2R(-17)",
            "real",
            NativeImpl::IntToReal,
            Value::Real(-17.0),
        ),
        (
            "native R2I takes real authoredName returns integer\n",
            "R2I(-17.75)",
            "integer",
            NativeImpl::RealToInt,
            Value::Int(-17),
        ),
    ] {
        let map =
            format!("function main takes nothing returns {returns}\nreturn {call}\nendfunction\n");
        let scripts = compile(declaration, &map);
        let name = call.split('(').next().unwrap();
        match scripts.program().natives().call(name) {
            Some(NativeCall::Implemented(n, which)) => {
                assert_eq!(which, implementation);
                assert_eq!(n.params[0].name.as_str(), "authoredName");
                assert_eq!(n.is_constant, name == "I2R");
            }
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(scripts.vm(NullHost).call("main", &[]).unwrap(), expected);
    }
}
#[test]
fn unknown_and_case_distinct_names_still_use_the_existing_loud_host_refusal() {
    for name in ["SuppliedOnly", "i2r"] {
        let declaration = format!("native {name} takes integer a returns real\n");
        let map =
            format!("function main takes nothing returns real\nreturn {name}(17)\nendfunction\n");
        let scripts = compile(&declaration, &map);
        assert!(matches!(
            scripts.program().natives().call(name),
            Some(NativeCall::Unimplemented(_))
        ));
        let mut vm = scripts.vm(RefusingHost(0));
        let e = vm.call("main", &[]).unwrap_err();
        assert!(e.message.contains("synthetic host refused"));
        assert!(e.message.contains("caller/archive/common.j"));
        assert_eq!(vm.host().inner.0, 1);
    }
}
#[test]
fn call_arity_against_the_authored_declaration_still_refuses_during_compile() {
    let result = MapScripts::compile(
        AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture".into(),
            source: "supplied".into(),
        },
        ScriptAsset {
            label: "declared-source",
            bytes: b"native I2R takes integer x returns real\n",
        },
        ScriptAsset {
            label: "other-source",
            bytes: b"function Other takes nothing returns nothing\nendfunction\n",
        },
        ScriptAsset {
            label: "map-source",
            bytes: b"function main takes nothing returns real\nreturn I2R()\nendfunction\n",
        },
        None,
    );
    let error = match result {
        Err(e) => e,
        Ok(_) => panic!("arity accepted"),
    };
    assert!(error.contains("map-source"));
    assert!(error.contains("declared-source"));
    assert!(error.contains("takes 1 argument, 0 given"));
}
#[test]
fn duplicate_equal_declarations_and_mismatch_coverage_retain_authored_source_metadata() {
    let first = pcx_script::parse("native I2R takes nothing returns real\n").unwrap();
    let second =
        pcx_script::parse("// separate supplied source\nnative I2R takes nothing returns real\n")
            .unwrap();
    let mut table = pcx_script::natives::NativeTable::new();
    table.insert(first.natives().next().unwrap(), "first-source");
    table.insert(second.natives().next().unwrap(), "second-source");
    let n = match table.call("I2R") {
        Some(NativeCall::SignatureMismatch(n, NativeImpl::IntToReal)) => n,
        other => panic!("unexpected {other:?}"),
    };
    assert_eq!(n.source, "first-source");
    assert_eq!(n.also_declared_in[0].0, "second-source");
    assert_eq!(
        n.also_declared_in[0].1,
        second.natives().next().unwrap().span
    );
}
#[test]
fn ambiguous_declarations_remain_ambiguous_and_never_intrinsic_dispatch() {
    let a = pcx_script::parse("native I2R takes integer x returns real\n").unwrap();
    let b = pcx_script::parse("native I2R takes integer x returns integer\n").unwrap();
    let mut t = pcx_script::natives::NativeTable::new();
    t.insert(a.natives().next().unwrap(), "a");
    t.insert(b.natives().next().unwrap(), "b");
    assert!(t.call("I2R").is_none());
    assert_eq!(t.ambiguous("I2R").unwrap().at.len(), 2);
}
#[test]
fn caller_array_markers_do_not_disappear_from_typed_mismatch_metadata() {
    let tree = pcx_script::parse("native I2R takes integer x returns real\n").unwrap();
    let mut declaration = tree.natives().next().unwrap().clone();
    declaration.params[0].ty.array = true;
    let mut table = pcx_script::natives::NativeTable::new();
    table.insert(&declaration, "explicit caller AST");
    let n = match table.call("I2R") {
        Some(NativeCall::SignatureMismatch(n, _)) => n,
        other => panic!("unexpected {other:?}"),
    };
    assert!(n.params[0].ty.array);
    assert_eq!(n.params[0].ty.span, declaration.params[0].ty.span);
}
