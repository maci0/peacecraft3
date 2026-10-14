use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{NullHost, Value};

fn compile(
    common: ScriptAsset<'_>,
    blizzard: ScriptAsset<'_>,
    map: ScriptAsset<'_>,
) -> Result<MapScripts, String> {
    MapScripts::compile(
        AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture".into(),
            source: "memory-only".into(),
        },
        common,
        blizzard,
        map,
        None,
    )
}
fn asset<'a>(label: &'a str, bytes: &'a [u8]) -> ScriptAsset<'a> {
    ScriptAsset { label, bytes }
}

#[test]
fn common_type_failure_uses_its_own_label_and_line() {
    let error = compile(
        asset(
            "caller/common.j",
            b"// synthetic source\n\n\n\ntype Broken extends MissingParent\n",
        ),
        asset("caller/Blizzard.j", b"\n"),
        asset(
            "caller/map.j",
            b"function main takes nothing returns nothing\nendfunction\n",
        ),
    )
    .err()
    .expect("missing parent must fail");
    assert!(error.starts_with("caller/common.j:5:"), "{error}");
    assert!(error.contains("MissingParent"), "{error}");
}
#[test]
fn blizzard_type_failure_uses_its_own_label_and_line() {
    let error = compile(
        asset("caller/common.j", b"\n"),
        asset(
            "caller/Blizzard.j",
            b"// separate source\n\ntype Broken extends MissingParent\n",
        ),
        asset(
            "caller/map.j",
            b"function main takes nothing returns nothing\nendfunction\n",
        ),
    )
    .err()
    .expect("missing parent must fail");
    assert!(error.starts_with("caller/Blizzard.j:3:"), "{error}");
    assert!(error.contains("MissingParent"), "{error}");
}
#[test]
fn main_failure_keeps_its_map_label_and_line() {
    let error = compile(
        asset("caller/common.j", b"// unrelated prelude\n\n\n"),
        asset("caller/Blizzard.j", b"\n"),
        asset(
            "caller/map.j",
            b"function main takes nothing returns nothing\ncall MissingFunction()\nendfunction\n",
        ),
    )
    .err()
    .expect("missing function must fail");
    assert!(error.starts_with("caller/map.j:2:"), "{error}");
    assert!(error.contains("MissingFunction"), "{error}");
}
#[test]
fn prelude_labels_cannot_alias_the_vm_main_source() {
    for reserved_common in [true, false] {
        let common_label = if reserved_common {
            "the map script"
        } else {
            "caller/common.j"
        };
        let blizzard_label = if reserved_common {
            "caller/Blizzard.j"
        } else {
            "the map script"
        };
        let error = compile(
            asset(common_label, b"\n"),
            asset(blizzard_label, b"\n"),
            asset(
                "caller/map.j",
                b"function main takes nothing returns nothing\nendfunction\n",
            ),
        )
        .err()
        .expect("ambiguous runtime source identity must fail");
        assert!(error.contains("source identity"), "{error}");
        assert!(error.contains("the map script"), "{error}");
    }
}
#[test]
fn main_label_can_match_its_vm_identity_and_valid_preludes_still_run() {
    let scripts = compile(
        asset("caller/common.j", b"\n"),
        asset(
            "caller/Blizzard.j",
            b"function Body takes nothing returns integer\nreturn 7\nendfunction\n",
        ),
        asset(
            "the map script",
            b"function main takes nothing returns integer\nreturn Body()\nendfunction\n",
        ),
    )
    .unwrap();
    let mut vm = scripts.vm(NullHost);
    assert_eq!(vm.call("main", &[]).unwrap(), Value::Int(7));
    assert_eq!(scripts.digests.len(), 3);
}
