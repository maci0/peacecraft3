use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_script::{Host, Prelude};
use std::sync::Arc;
const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative SetMapMusic takes string name, boolean random, integer index returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
fn host(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "synthetic:music",
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 8,
                admission_steps: 100,
                expression_depth: 16,
                expression_work: 100,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
fn set(h: &mut ObjectHost, name: &str, random: bool, index: i32) -> Result<Value, String> {
    h.native(
        "SetMapMusic",
        &[
            Value::Str(name.into()),
            Value::Bool(random),
            Value::Int(index),
        ],
    )
}
#[test]
fn exact_selection_replaces_only_after_executed_calls_without_defaults_or_track_expansion() {
    let mut h = host(COMMON, "");
    assert!(h.map_music_selection().is_none());
    assert!(set(&mut h, "Unresolved", true, 53).is_err());
    let mut h = h.with_map_music_selection(64).unwrap();
    assert!(h.map_music_selection().is_none());
    for (name, random, index) in [
        ("Unresolved;Selected", true, 53),
        ("音楽", false, -7),
        ("", false, i32::MIN),
        ("Other", true, i32::MAX),
    ] {
        assert_eq!(set(&mut h, name, random, index).unwrap(), Value::Null);
        assert_eq!(
            h.map_music_selection(),
            Some(&MapMusicSelection {
                name: name.into(),
                random,
                index
            })
        );
        assert_eq!(h.inner().handles(), 0);
    }
}
#[test]
fn wrong_arguments_nuls_and_byte_limit_fail_atomically() {
    let mut h = host(COMMON, "").with_map_music_selection(4).unwrap();
    set(&mut h, "Good", false, 27).unwrap();
    for args in [
        vec![],
        vec![Value::Null, Value::Bool(false), Value::Int(1)],
        vec![Value::Str("Bad".into()), Value::Int(1), Value::Int(1)],
        vec![
            Value::Str("Bad".into()),
            Value::Bool(true),
            Value::Real(1.0),
        ],
        vec![
            Value::Str("Bad".into()),
            Value::Bool(true),
            Value::Int(1),
            Value::Int(2),
        ],
    ] {
        assert!(
            h.native("SetMapMusic", &args)
                .unwrap_err()
                .contains("SetMapMusic")
        );
        assert_eq!(h.map_music_selection().unwrap().name, "Good");
    }
    assert!(set(&mut h, "12345", true, 4).is_err());
    assert!(set(&mut h, "音楽", true, 4).is_err());
    assert!(set(&mut h, "A\0B", true, 4).is_err());
    assert_eq!(
        h.map_music_selection(),
        Some(&MapMusicSelection {
            name: "Good".into(),
            random: false,
            index: 27
        })
    );
    let mut zero = host(COMMON, "").with_map_music_selection(0).unwrap();
    assert!(set(&mut zero, "A", false, 1).is_err());
    assert!(zero.map_music_selection().is_none());
    assert_eq!(set(&mut zero, "", false, 1).unwrap(), Value::Null);
}
#[test]
fn admission_refuses_duplicates_overrides_signature_changes_and_platform_overflow() {
    assert!(
        host(COMMON, "")
            .with_map_music_selection(8)
            .unwrap()
            .with_map_music_selection(16)
            .is_err()
    );
    assert!(
        host(COMMON, "")
            .with_map_music_selection((isize::MAX as usize) + 1)
            .is_err()
    );
    for bad in [
        COMMON.replace("native SetMapMusic", "constant native SetMapMusic"),
        COMMON.replace("boolean random", "integer random"),
        COMMON.replace(
            "integer index returns nothing",
            "integer index returns integer",
        ),
    ] {
        assert!(host(&bad, "").with_map_music_selection(8).is_err());
    }
    assert!(host(COMMON,"function SetMapMusic takes string name, boolean random, integer index returns nothing\nendfunction\n").with_map_music_selection(8).is_err());
}
#[test]
fn retained_program_keeps_executed_selection_after_later_failure() {
    let common = format!("{COMMON}native Next takes nothing returns nothing\n");
    let mut vm=host(&common,"function main takes nothing returns nothing\ncall SetMapMusic(\"AuthoredToken\",true,71)\ncall Next()\nendfunction\n").with_map_music_selection(64).unwrap().into_vm();
    vm.init_globals().unwrap();
    assert!(
        vm.call("main", &[])
            .unwrap_err()
            .to_string()
            .contains("Next")
    );
    assert_eq!(
        vm.host().map_music_selection(),
        Some(&MapMusicSelection {
            name: "AuthoredToken".into(),
            random: true,
            index: 71
        })
    );
    vm.init_globals().unwrap();
    assert_eq!(vm.host().map_music_selection().unwrap().index, 71);
    assert_eq!(vm.host().inner().handles(), 1);
}
