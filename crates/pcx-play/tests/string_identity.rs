use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{HandleId, Host, SourceMap, Stage, Value};
use std::{cell::RefCell, num::NonZeroU64};

const WTS: &[u8] = b"STRING 0\r\n{\r\n\r\n}\r\nSTRING 7\r\n{\r\nUnrelated table entry\r\n}\r\nSTRING 9\r\n{\r\n\xff\r\n}\r\n";

struct FixtureHost {
    substitution: Option<String>,
    tags: RefCell<Vec<String>>,
}
impl FixtureHost {
    fn new(substitution: Option<&str>) -> Self {
        Self {
            substitution: substitution.map(str::to_owned),
            tags: RefCell::new(Vec::new()),
        }
    }
}
impl Host for FixtureHost {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match (name, args) {
            ("Make", []) => Ok(Value::Handle(HandleId::new(NonZeroU64::new(99).unwrap()))),
            _ => Err(format!("unsupported fixture native {name}")),
        }
    }
    fn code_tag(&self, tag: &str) -> Option<String> {
        self.tags.borrow_mut().push(tag.to_owned());
        self.substitution.clone()
    }
}
fn scripts(source: &str) -> MapScripts {
    MapScripts::compile(
        AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture-1".into(),
            source: "caller in-memory inputs".into(),
        },
        ScriptAsset {
            label: "synthetic/common.j",
            bytes: b"// synthetic prelude\n",
        },
        ScriptAsset {
            label: "synthetic/Blizzard.j",
            bytes: b"// synthetic prelude\n",
        },
        ScriptAsset {
            label: "synthetic/war3map.j",
            bytes: source.as_bytes(),
        },
        Some(ScriptAsset {
            label: "synthetic/war3map.wts",
            bytes: WTS,
        }),
    )
    .unwrap()
}

#[test]
fn authored_tag_parts_cannot_fabricate_a_table_reference() {
    for tag in ["id", "識別"] {
        let source = format!(
            "function Tagged takes nothing returns string\nreturn \"TRIGSTR_^^{tag}^^7\"\nendfunction\n"
        );
        let tree = pcx_script::parse(&source).unwrap();
        let printed = tree.to_source();
        assert!(printed.contains(&format!("TRIGSTR_^^{tag}^^7")));
        let scripts = scripts(&source);
        let mut vm = scripts.vm(FixtureHost::new(Some("x")));
        assert_eq!(
            vm.call("Tagged", &[]).unwrap(),
            Value::Str("TRIGSTR_x7".into())
        );
        assert_eq!(vm.host().inner.tags.borrow().as_slice(), [tag]);
        assert_eq!(scripts.identity.source, "caller in-memory inputs");
        assert_eq!(
            scripts
                .digests
                .iter()
                .map(|d| d.label.as_str())
                .collect::<Vec<_>>(),
            [
                "synthetic/common.j",
                "synthetic/Blizzard.j",
                "synthetic/war3map.j",
                "synthetic/war3map.wts"
            ]
        );
    }
}

#[test]
fn unrelated_table_entry_cannot_bypass_missing_tag_refusal() {
    let source =
        "function Tagged takes nothing returns string\nreturn \"TRIGSTR_^^id^^7\"\nendfunction\n";
    let scripts = scripts(source);
    let mut vm = scripts.vm(FixtureHost::new(None));
    let error = vm
        .call("Tagged", &[])
        .expect_err("missing authored tag must not return unrelated table entry 7");
    assert_eq!(error.stage, Stage::Run);
    assert!(error.message.contains("no substitution"));
    assert!(error.message.contains("^id^"));
    assert_eq!(
        &source[error.span.start as usize..error.span.end as usize],
        "\"TRIGSTR_^^id^^7\""
    );
    assert_eq!(error.locate(&SourceMap::new(source)), (2, 8));
    assert_eq!(vm.error_source(), Some("the map script"));
    assert!(
        scripts
            .diagnostic(&vm, &error)
            .starts_with("synthetic/war3map.j:2:8")
    );
    assert_eq!(vm.host().inner.tags.borrow().as_slice(), ["id"]);
}

#[test]
fn null_empty_unicode_and_requested_table_failures_stay_distinct() {
    let source = "function Empty takes nothing returns string\nreturn \"\"\nendfunction\nfunction TableEmpty takes nothing returns string\nreturn \"TRIGSTR_0\"\nendfunction\nfunction Absent takes nothing returns string\nreturn null\nendfunction\nfunction Unicode takes nothing returns string\nreturn \"漢字|n|cffaabbccText|r\"\nendfunction\nfunction Missing takes nothing returns string\nreturn \"TRIGSTR_8\"\nendfunction\nfunction BadEncoding takes nothing returns string\nreturn \"TRIGSTR_9\"\nendfunction\n";
    let scripts = scripts(source);
    let mut vm = scripts.vm(FixtureHost::new(None));
    for name in ["Empty", "TableEmpty"] {
        assert_eq!(vm.call(name, &[]).unwrap(), Value::Str(String::new()));
    }
    assert_eq!(vm.call("Absent", &[]).unwrap(), Value::Null);
    assert_eq!(
        vm.call("Unicode", &[]).unwrap(),
        Value::Str("漢字|n|cffaabbccText|r".into())
    );
    let missing = vm.call("Missing", &[]).unwrap_err();
    assert!(missing.message.contains("TRIGSTR_8 is not in this map"));
    let encoding = vm.call("BadEncoding", &[]).unwrap_err();
    assert!(
        encoding
            .message
            .contains("synthetic/war3map.wts: WTS entry 9 is not UTF-8 at body byte 0")
    );
    assert!(
        scripts
            .diagnostic(&vm, &encoding)
            .starts_with("synthetic/war3map.j:")
    );
    assert!(vm.host().inner.tags.borrow().is_empty());
}

#[test]
fn string_code_and_caller_handle_values_keep_distinct_kinds() {
    let source = "type widget extends handle\nnative Make takes nothing returns widget\nglobals\nstring array strings\ncode array routines\nwidget array objects\nendglobals\nfunction Marker takes nothing returns nothing\nendfunction\nfunction Fill takes nothing returns boolean\nset strings[1] = \"漢字|n\"\nset routines[2] = function Marker\nset objects[3] = Make()\nreturn strings[1] != null and routines[2] == function Marker and objects[3] != null\nendfunction\nfunction Mismatch takes nothing returns boolean\nreturn strings[1] == routines[2]\nendfunction\n";
    let scripts = scripts(source);
    let mut vm = scripts.vm(FixtureHost::new(None));
    assert_eq!(vm.call("Fill", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.global_at("strings", 1), Value::Str("漢字|n".into()));
    assert!(matches!(vm.global_at("routines", 2), Value::Code(_)));
    assert_eq!(
        vm.global_at("objects", 3),
        Value::Handle(HandleId::new(NonZeroU64::new(99).unwrap()))
    );
    assert_eq!(vm.global_at("objects", 3).as_int(), None);
    let mismatch = vm.call("Mismatch", &[]).unwrap_err();
    assert!(mismatch.message.contains("cannot compare string and code"));
    assert_eq!(vm.global_at("strings", 0), Value::Null);
}
