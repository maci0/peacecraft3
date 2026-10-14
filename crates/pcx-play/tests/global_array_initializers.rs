//! Mirrors the existing local-array refusal contract; no native admission/order rule is inferred.
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{Host, Stage, Value};
const DECL: &str = "native Mark takes integer value returns integer\n";
const OTHER: &str = "function Other takes nothing returns nothing\nendfunction\n";
const MAIN: &str = "function main takes nothing returns nothing\ncall Mark(19)\nendfunction\n";
const INVALID: &str = "globals\ninteger prefix = Mark(3)\ninteger array bad = Mark(7)\ninteger suffix = Mark(11)\nendglobals\n";
#[derive(Default)]
struct Recording(Vec<i32>);
impl Host for Recording {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        assert_eq!(name, "Mark");
        let [Value::Int(value)] = args else {
            panic!("explicit marker input")
        };
        self.0.push(*value);
        Ok(Value::Int(*value))
    }
}
fn compile(common: &str, bj: &str, map: &str) -> MapScripts {
    MapScripts::compile(
        AssetIdentity {
            edition: "synthetic".into(),
            version: "caller-fixture".into(),
            source: "supplied-memory".into(),
        },
        ScriptAsset {
            label: "caller/common.j",
            bytes: common.as_bytes(),
        },
        ScriptAsset {
            label: "caller/Blizzard.j",
            bytes: bj.as_bytes(),
        },
        ScriptAsset {
            label: "caller/map.j",
            bytes: map.as_bytes(),
        },
        None,
    )
    .unwrap()
}
#[test]
fn global_array_initializer_refuses_before_expression_and_remains_sticky_in_every_source() {
    for where_ in 0..3 {
        let common = if where_ == 0 {
            format!("{DECL}{INVALID}")
        } else {
            DECL.into()
        };
        let bj = if where_ == 1 { INVALID } else { OTHER };
        let map = if where_ == 2 {
            format!("{INVALID}{MAIN}")
        } else {
            MAIN.into()
        };
        let scripts = compile(&common, bj, &map);
        let declaration = scripts
            .program()
            .globals()
            .iter()
            .find(|g| g.name == "bad")
            .unwrap();
        assert!(declaration.array);
        assert!(declaration.init.is_some());
        let span = declaration.span;
        let owner = declaration.source.clone();
        let mut vm = scripts.vm(Recording::default());
        let error = vm.init_globals().unwrap_err();
        assert_eq!(error.stage, Stage::Run);
        assert_eq!(error.span, span);
        assert_eq!(vm.error_source(), Some(owner.as_str()));
        assert!(error.message.contains("declared `array`"));
        assert!(error.message.contains("no initialiser"));
        let label = match where_ {
            0 => "caller/common.j:",
            1 => "caller/Blizzard.j:",
            _ => "caller/map.j:",
        };
        assert!(scripts.diagnostic(&vm, &error).starts_with(label));
        assert_eq!(vm.global("prefix"), Value::Int(3));
        assert_eq!(vm.host().inner.0, [3]);
        assert_eq!(vm.init_globals().unwrap_err(), error);
        assert_eq!(vm.call("main", &[]).unwrap_err(), error);
        assert_eq!(vm.host().inner.0, [3]);
    }
}
#[test]
fn valid_uninitialized_array_remains_indexable_and_successful_init_runs_once() {
    let common =
        format!("{DECL}globals\ninteger seed = Mark(3)\ninteger array buffer\nendglobals\n");
    let map = "function main takes nothing returns nothing\nset buffer[5] = 23\nendfunction\n";
    let scripts = compile(&common, OTHER, map);
    let mut vm = scripts.vm(Recording::default());
    vm.init_globals().unwrap();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global_at("buffer", 5), Value::Int(23));
    assert_eq!(vm.host().inner.0, [3]);
}
#[test]
fn existing_local_array_initializer_refusal_still_precedes_marker_call() {
    let map = "function main takes nothing returns nothing\nlocal integer array bad = Mark(7)\nendfunction\n";
    let scripts = compile(DECL, OTHER, map);
    let mut vm = scripts.vm(Recording::default());
    let e = vm.call("main", &[]).unwrap_err();
    assert!(e.message.contains("no initialiser"));
    assert!(vm.host().inner.0.is_empty());
    assert!(scripts.diagnostic(&vm, &e).starts_with("caller/map.j:2:"));
}
