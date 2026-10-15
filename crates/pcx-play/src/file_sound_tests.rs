use super::*;
use crate::label_sounds::SoundBank;
use crate::neutral_player::sha256;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;
fn origin() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "test".into(),
        source: "synthetic:PCM".into(),
    }
}
fn wave() -> Vec<u8> {
    let mut d = b"RIFF\0\0\0\0WAVEfmt ".to_vec();
    d.extend(16u32.to_le_bytes());
    for n in [1u16, 2] {
        d.extend(n.to_le_bytes());
    }
    d.extend(17300u32.to_le_bytes());
    d.extend(69200u32.to_le_bytes());
    d.extend(4u16.to_le_bytes());
    d.extend(16u16.to_le_bytes());
    d.extend(b"data");
    d.extend(28u32.to_le_bytes());
    d.extend([0x12; 28]);
    let n = (d.len() - 8) as u32;
    d[4..8].copy_from_slice(&n.to_le_bytes());
    d
}
fn budget() -> Budget {
    Budget {
        resources: 4,
        resource_bytes: 4096,
        total_bytes: 8192,
        metadata_bytes: 4096,
    }
}
fn input(d: &[u8]) -> SoundInput<'_> {
    SoundInput {
        member: r"Media\a.wav",
        asset: ScriptAsset {
            label: "synthetic:archive::Media\\a.wav",
            bytes: d,
        },
        origin: origin(),
    }
}
fn assets() -> Assets {
    Assets::read(&[input(&wave())], budget()).unwrap()
}
#[test]
fn complete_pcm_and_exact_filename_provenance_are_retained_without_a_table() {
    let d = wave();
    let a = Assets::read(&[input(&d)], budget()).unwrap();
    let [r] = a.resources() else {
        panic!("one resource")
    };
    assert_eq!(r.member(), r"Media\a.wav");
    assert_eq!(r.bytes(), d);
    assert_eq!(r.sha256(), sha256(&d));
    assert_eq!(r.origin(), &origin());
    assert_eq!(r.source_label(), "synthetic:archive::Media\\a.wav");
    assert_eq!(
        (
            r.pcm().channels,
            r.pcm().sample_rate,
            r.pcm().bits_per_sample,
            r.pcm().frames()
        ),
        (2, 17300, 16, 7)
    );
}
#[test]
fn whole_input_preflight_refuses_duplicates_identity_and_budget_before_decode() {
    let d = wave();
    let bad = b"invalid";
    let mut later = input(&d);
    later.member = r"Media\b.wav";
    let mut n = budget();
    n.total_bytes = d.len() + bad.len() - 1;
    assert!(matches!(
        Assets::read(&[input(bad), later], n).unwrap_err().problem,
        Problem::Budget
    ));
    assert!(matches!(
        Assets::read(&[input(bad), input(&d)], budget())
            .unwrap_err()
            .problem,
        Problem::Duplicate
    ));
    let mut i = input(&d);
    i.origin.edition.clear();
    assert!(matches!(
        Assets::read(&[i], budget()).unwrap_err().problem,
        Problem::Identity
    ));
    let mut i = input(&d);
    i.member = "A\0B";
    assert!(matches!(
        Assets::read(&[i], budget()).unwrap_err().problem,
        Problem::Identity
    ));
    for case in 0..5 {
        let mut n = budget();
        match case {
            0 => n.resources = 0,
            1 => n.resource_bytes = d.len() - 1,
            2 => n.total_bytes = d.len() - 1,
            3 => n.metadata_bytes = 1,
            _ => n.resources = usize::MAX,
        };
        assert!(matches!(
            Assets::read(&[input(&d)], n).unwrap_err().problem,
            Problem::Budget
        ));
    }
    assert!(matches!(
        Assets::read(&[input(bad)], budget()).unwrap_err().problem,
        Problem::Wave(_)
    ));
    let mut n = budget();
    n.resource_bytes = d.len();
    n.total_bytes = d.len();
    assert!(Assets::read(&[input(&d)], n).is_ok());
}
const COMMON: &str = "type agent extends handle\ntype sound extends agent\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateMIDISound takes string label, integer a, integer b returns sound\nnative CreateSound takes string file, boolean repeating, boolean spatial, boolean stop, integer a, integer b, string eax returns sound\nnative StartSound takes sound s returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
fn enums(common: &str, map: &str, max_handles: usize) -> EnumHost {
    let c = pcx_script::parse_script(common).unwrap();
    let m = pcx_script::parse_script(map).unwrap();
    let p =
        Arc::new(pcx_script::resolve_with(&m, &[Prelude::new("synthetic:common", &c)]).unwrap());
    EnumHost::checked_selected(
        p,
        "synthetic:files",
        &["ConvertPlayerColor"],
        Limits {
            max_handles,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap()
}
fn host(common: &str, map: &str, max_handles: usize) -> ObjectHost {
    ObjectHost::checked(enums(common, map, max_handles)).unwrap()
}
fn args() -> Vec<Value> {
    vec![
        Value::Str(r"Media\a.wav".into()),
        Value::Bool(true),
        Value::Bool(false),
        Value::Bool(true),
        Value::Int(-7),
        Value::Int(91),
        Value::Str("AuthoredProfile".into()),
    ]
}
#[test]
fn direct_creations_share_pcm_but_have_distinct_arena_ids_and_exact_creation_inputs() {
    let mut h = host(COMMON, "", 8);
    assert!(h.native("CreateSound", &args()).is_err());
    let mut h = h
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_sound_starts(4)
        .unwrap();
    let Value::Handle(a) = h.native("CreateSound", &args()).unwrap() else {
        panic!("sound")
    };
    let mut other = args();
    other[1] = Value::Bool(false);
    other[2] = Value::Bool(true);
    other[3] = Value::Bool(false);
    other[4] = Value::Int(0);
    other[5] = Value::Int(i32::MAX);
    other[6] = Value::Str("".into());
    let Value::Handle(b) = h.native("CreateSound", &other).unwrap() else {
        panic!("sound")
    };
    assert_ne!(a, b);
    let s = h.file_sound_state(a).unwrap();
    assert_eq!(
        (
            s.looping,
            s.is_3d,
            s.stop_when_out_of_range,
            s.fade_in_rate,
            s.fade_out_rate,
            s.eax_setting
        ),
        (true, false, true, -7, 91, "AuthoredProfile")
    );
    let t = h.file_sound_state(b).unwrap();
    assert_eq!(
        (
            t.looping,
            t.is_3d,
            t.stop_when_out_of_range,
            t.fade_in_rate,
            t.fade_out_rate,
            t.eax_setting
        ),
        (false, true, false, 0, i32::MAX, "")
    );
    assert!(std::ptr::eq(s.resource, t.resource));
    assert_eq!(h.inner().kind(a).unwrap(), "sound");
    assert_eq!(h.sound_resource(a).unwrap().pcm().frames(), 7);
    assert!(h.sound_state(a).is_err());
    for id in [b, a, b] {
        assert_eq!(
            h.native("StartSound", &[Value::Handle(id)]).unwrap(),
            Value::Null
        );
    }
    assert_eq!(h.take_sound_starts(), [b, a, b]);
    assert_eq!(h.inner().handles(), 2);
}
#[test]
fn missing_case_changed_paths_bad_types_and_eax_budget_never_issue_ids() {
    let mut h = host(COMMON, "", 8).with_file_sounds(assets(), 15).unwrap();
    for text in [r"Media\missing.wav", r"media\a.wav"] {
        let mut a = args();
        a[0] = Value::Str(text.into());
        let e = h.native("CreateSound", &a).unwrap_err();
        assert!(e.contains(text));
    }
    let mut long = args();
    long[0] = Value::Str("x".repeat(4097));
    assert!(h.native("CreateSound", &long).is_err());
    for at in 0..7 {
        let mut a = args();
        a[at] = Value::Null;
        assert!(h.native("CreateSound", &a).is_err());
    }
    for text in ["x".repeat(16), "A\0B".into()] {
        let mut a = args();
        a[6] = Value::Str(text);
        assert!(h.native("CreateSound", &a).is_err());
    }
    assert_eq!(h.inner().handles(), 0);
    assert!(h.pending_sound_starts().is_empty());
    let mut h = host(COMMON, "", 1).with_file_sounds(assets(), 64).unwrap();
    let first = h.native("CreateSound", &args()).unwrap();
    assert!(h.native("CreateSound", &args()).is_err());
    assert_eq!(h.inner().handles(), 1);
    let Value::Handle(id) = first else {
        panic!("sound")
    };
    assert_eq!(
        h.file_sound_state(id).unwrap().eax_setting,
        "AuthoredProfile"
    );
}
#[test]
fn file_admission_contract_platform_and_reserved_identity_checks_are_explicit() {
    assert!(
        host(COMMON, "", 8)
            .with_file_sounds(assets(), 64)
            .unwrap()
            .with_file_sounds(assets(), 64)
            .is_err()
    );
    assert!(
        host(COMMON, "", 8)
            .with_file_sounds(assets(), (isize::MAX as usize) + 1)
            .is_err()
    );
    for c in [
        COMMON.replace("native CreateSound", "constant native CreateSound"),
        COMMON.replace("boolean spatial", "integer spatial"),
        COMMON.replace("string eax returns sound", "string eax returns timer"),
        COMMON.replace("type sound extends agent", "type sound extends handle"),
    ] {
        assert!(host(&c, "", 8).with_file_sounds(assets(), 64).is_err());
    }
    assert!(host(COMMON,"function CreateSound takes string f, boolean l, boolean d, boolean s, integer a, integer b, string eax returns sound\nreturn null\nendfunction\n",8).with_file_sounds(assets(),64).is_err());
    let mut e = enums(COMMON, "", 8);
    let fake = e.reserve_identity("sound").unwrap();
    let mut h = ObjectHost::checked(e)
        .unwrap()
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_sound_starts(2)
        .unwrap();
    assert!(h.file_sound_state(fake).is_err());
    assert!(h.sound_resource(fake).is_err());
    assert!(h.native("StartSound", &[Value::Handle(fake)]).is_err());
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(h.native("StartSound", &[timer]).is_err());
    assert!(h.pending_sound_starts().is_empty());
}
fn bank() -> SoundBank {
    let table=b"ID;PWXL;N;E\nC;X1;Y1;K\"SoundName\"\nC;X2;Y1;K\"DirectoryBase\"\nC;X3;Y1;K\"FileNames\"\nC;X1;Y2;K\"Breeze\"\nC;X2;Y2;K\"Media\"\nC;X3;Y2;K\"a.wav\"\nE\n";
    let table = std::str::from_utf8(table)
        .unwrap()
        .replace('\n', "\r\n")
        .into_bytes();
    let d = wave();
    SoundBank::read(
        ScriptAsset {
            label: "synthetic:slk",
            bytes: &table,
        },
        origin(),
        "SoundName",
        &[crate::label_sounds::SoundInput {
            label: "Breeze",
            member: r"Media\a.wav",
            asset: ScriptAsset {
                label: "synthetic:PCM",
                bytes: &d,
            },
            origin: origin(),
        }],
        crate::label_sounds::Budget {
            table_bytes: 4096,
            coordinates: pcx_slq::CoordinateLimits {
                max_row: 16,
                max_column: 8,
            },
            sounds: 4,
            resource_bytes: 4096,
            total_resource_bytes: 8192,
            metadata_bytes: 4096,
        },
    )
    .unwrap()
}
#[test]
fn label_and_file_objects_share_the_same_start_queue_and_preserve_label_api() {
    let mut h = host(COMMON, "", 8)
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_label_sounds(bank())
        .unwrap()
        .with_sound_starts(4)
        .unwrap();
    let Value::Handle(file) = h.native("CreateSound", &args()).unwrap() else {
        panic!("file")
    };
    let Value::Handle(label) = h
        .native(
            "CreateMIDISound",
            &[Value::Str("Breeze".into()), Value::Int(3), Value::Int(5)],
        )
        .unwrap()
    else {
        panic!("label")
    };
    assert_ne!(file, label);
    assert_eq!(h.sound_state(label).unwrap().resource.label(), "Breeze");
    assert_eq!(h.sound_state(label).unwrap().resource.row(), 2);
    assert!(h.file_sound_state(label).is_err());
    assert_eq!(
        h.sound_resource(file).unwrap().bytes(),
        h.sound_resource(label).unwrap().bytes()
    );
    for id in [file, label, file] {
        h.native("StartSound", &[Value::Handle(id)]).unwrap();
    }
    assert_eq!(h.take_sound_starts(), [file, label, file]);
}
#[test]
fn retained_vm_keeps_file_sound_state_and_start_requests_after_later_failure() {
    let c = format!("{COMMON}native Next takes nothing returns nothing\n");
    let map = "globals\nsound voice=null\nendglobals\nfunction main takes nothing returns nothing\nset voice=CreateSound(\"Media\\\\a.wav\",true,false,true,-7,91,\"AuthoredProfile\")\ncall StartSound(voice)\ncall Next()\nendfunction\n";
    let mut vm = host(&c, map, 8)
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_sound_starts(4)
        .unwrap()
        .into_vm();
    vm.init_globals().unwrap();
    assert!(
        vm.call("main", &[])
            .unwrap_err()
            .to_string()
            .contains("Next")
    );
    let Value::Handle(id) = vm.global("voice") else {
        panic!("retained sound")
    };
    assert_eq!(
        vm.host()
            .file_sound_state(id)
            .unwrap()
            .resource
            .pcm()
            .frames(),
        7
    );
    assert_eq!(vm.host().pending_sound_starts(), [id]);
    let count = vm.host().inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner().handles(), count);
    assert_eq!(vm.host().pending_sound_starts(), [id]);
}
