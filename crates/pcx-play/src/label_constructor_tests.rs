use super::{COMMON, bank, enums, host, limits, origin, table, wave};
use crate::label_sounds::{SoundBank, SoundInput};
use crate::playercolor_host::{LabelSoundFlags, ObjectHost};
use crate::scripts::ScriptAsset;
use pcx_script::{Host, Value};
const CREATE: &str = "native CreateSoundFromLabel takes string label, boolean looped, boolean spatial, boolean stop, integer a, integer b returns sound";
fn common() -> String {
    format!(
        "{COMMON}\n{CREATE}\nnative SetSoundDuration takes sound s, integer n returns nothing\n"
    )
}
fn ui_bank() -> SoundBank {
    named_bank("Breeze")
}
fn named_bank(label: &str) -> SoundBank {
    let mut bytes = wave();
    *bytes.last_mut().unwrap() ^= 1;
    SoundBank::read(
        ScriptAsset {
            label: "synthetic:ui-table",
            bytes: &table(label, "ui.wav"),
        },
        origin(),
        "Selector",
        &[SoundInput {
            label,
            member: r"Media\Loop\ui.wav",
            asset: ScriptAsset {
                label: "synthetic:ui-wave",
                bytes: &bytes,
            },
            origin: origin(),
        }],
        limits(),
    )
    .unwrap()
}
fn create(h: &mut ObjectHost, flags: [bool; 3], rates: [i32; 2]) -> Result<Value, String> {
    h.native(
        "CreateSoundFromLabel",
        &[
            Value::Str("Breeze".into()),
            Value::Bool(flags[0]),
            Value::Bool(flags[1]),
            Value::Bool(flags[2]),
            Value::Int(rates[0]),
            Value::Int(rates[1]),
        ],
    )
}
fn id(v: &Value) -> pcx_script::HandleId {
    let Value::Handle(id) = v else {
        panic!("sound");
    };
    *id
}
#[test]
fn owned_vm_label_creation_keeps_flags_source_and_shared_arena_without_midi_fallback() {
    let c = common();
    let map = "globals\nsound A = null\nsound B = null\nsound Midi = null\nendglobals\nfunction main takes nothing returns nothing\nset Midi = CreateMIDISound(\"Breeze\",-3,9)\nset A = CreateSoundFromLabel(\"Breeze\",true,false,true,-7,0)\nset B = CreateSoundFromLabel(\"Breeze\",false,true,false,5,-9)\ncall SetSoundDuration(A,37)\ncall StartSound(A)\nendfunction\n";
    let h = host(&c, map, 8)
        .with_label_sounds(bank())
        .unwrap()
        .with_label_sound_creation(vec![named_bank("Remote"), ui_bank()])
        .unwrap()
        .with_sound_duration()
        .unwrap()
        .with_sound_starts(1)
        .unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let initial_handles = vm.host().inner().handles();
    vm.call("main", &[]).unwrap();
    let (a, b, m) = (
        id(&vm.global("A")),
        id(&vm.global("B")),
        id(&vm.global("Midi")),
    );
    assert_ne!(a, b);
    assert_ne!(a, m);
    let h = vm.host();
    let sa = h.sound_state(a).unwrap();
    let sb = h.sound_state(b).unwrap();
    let sm = h.sound_state(m).unwrap();
    assert_eq!(
        sa.creation_flags,
        Some(LabelSoundFlags {
            looping: true,
            is_3d: false,
            stop_when_out_of_range: true
        })
    );
    assert_eq!(
        sb.creation_flags,
        Some(LabelSoundFlags {
            looping: false,
            is_3d: true,
            stop_when_out_of_range: false
        })
    );
    assert_eq!(sm.creation_flags, None);
    assert_eq!(
        (sa.fade_in_rate, sa.fade_out_rate, sa.duration),
        (-7, 0, Some(37))
    );
    assert_eq!((sb.fade_in_rate, sb.fade_out_rate), (5, -9));
    assert!(std::ptr::eq(sa.resource, sb.resource));
    assert!(!std::ptr::eq(sa.resource, sm.resource));
    assert_eq!(sa.bank.source_label(), "synthetic:ui-table");
    assert_eq!(sm.bank.source_label(), "synthetic:slk");
    assert_eq!(sa.resource.member(), r"Media\Loop\ui.wav");
    assert_ne!(sa.resource.sha256(), sm.resource.sha256());
    assert!(h.sound_properties(a).unwrap().volume.is_none());
    assert!(h.sound_properties(a).unwrap().pitch.is_none());
    assert_eq!(h.pending_sound_starts(), &[a]);
    assert_eq!(h.inner().handles(), initial_handles + 3);
}
#[test]
fn label_only_admission_reuses_sound_consumers_and_refusals_are_atomic() {
    let c = common();
    assert!(
        host(&c, "", 4)
            .with_label_sound_creation(Vec::new())
            .is_err()
    );
    assert!(
        host(&c, "", 4)
            .with_label_sound_creation(vec![ui_bank(), ui_bank()])
            .is_err()
    );
    let mut disabled = host(&c, "", 4).with_label_sounds(bank()).unwrap();
    assert!(
        create(&mut disabled, [false; 3], [0; 2])
            .unwrap_err()
            .contains("unsupported")
    );
    let mut h = host(&c, "", 4)
        .with_label_sound_creation(vec![ui_bank()])
        .unwrap()
        .with_sound_duration()
        .unwrap()
        .with_sound_starts(1)
        .unwrap();
    let a = create(&mut h, [false; 3], [0; 2]).unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let before = h.inner().handles();
    for args in [
        vec![],
        vec![Value::Null],
        vec![
            Value::Str("Breeze".into()),
            Value::Int(0),
            Value::Bool(false),
            Value::Bool(false),
            Value::Int(0),
            Value::Int(0),
        ],
        vec![
            Value::Str("missing".into()),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
            Value::Int(0),
            Value::Int(0),
        ],
        vec![
            Value::Str("x".repeat(4097)),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
            Value::Int(0),
            Value::Int(0),
        ],
    ] {
        assert!(h.native("CreateSoundFromLabel", &args).is_err());
        assert_eq!(h.inner().handles(), before);
    }
    assert!(
        h.native(
            "CreateMIDISound",
            &[Value::Str("Breeze".into()), Value::Int(0), Value::Int(0)]
        )
        .is_err()
    );
    assert!(h.sound_state(id(&timer)).is_err());
    h.native("SetSoundDuration", &[a.clone(), Value::Int(-5)])
        .unwrap();
    h.native("StartSound", std::slice::from_ref(&a)).unwrap();
    assert_eq!(h.sound_duration(id(&a)).unwrap(), Some(-5));
    assert_eq!(h.pending_sound_starts(), &[id(&a)]);
    while h.inner().handles() < 4 {
        h.native("CreateTimer", &[]).unwrap();
    }
    assert!(create(&mut h, [true; 3], [3, -2]).is_err());
    assert_eq!(h.inner().handles(), 4);
    assert_eq!(
        h.sound_state(id(&a)).unwrap().creation_flags,
        Some(LabelSoundFlags {
            looping: false,
            is_3d: false,
            stop_when_out_of_range: false
        })
    );
    assert!(h.with_label_sound_creation(vec![ui_bank()]).is_err());
    let mut e = enums(&c, "", 4);
    let reserved = e.reserve_identity("sound").unwrap();
    let h = ObjectHost::checked(e)
        .unwrap()
        .with_label_sound_creation(vec![ui_bank()])
        .unwrap();
    assert!(h.sound_resource(reserved).is_err());
}
#[test]
fn label_constructor_requires_loaded_contract_and_source_ancestry() {
    let c = common();
    for c in [
        c.replace(CREATE, ""),
        c.replace(CREATE, &format!("constant {CREATE}")),
        c.replace("boolean looped", "integer looped"),
        c.replace("integer b returns sound", "integer b returns timer"),
        c.replace("type sound extends agent", "type sound extends handle"),
    ] {
        assert!(
            host(&c, "", 4)
                .with_label_sound_creation(vec![ui_bank()])
                .is_err()
        );
    }
    let moved = c.replace(CREATE, "");
    assert!(
        host(&moved, CREATE, 4)
            .with_label_sound_creation(vec![ui_bank()])
            .is_err()
    );
    assert!(host(&c,"function CreateSoundFromLabel takes string label, boolean looped, boolean spatial, boolean stop, integer a, integer b returns sound\nreturn null\nendfunction\n",4).with_label_sound_creation(vec![ui_bank()]).is_err());
    let moved = c.replace("type sound extends agent", "");
    assert!(
        host(&moved, "type sound extends agent", 4)
            .with_label_sound_creation(vec![ui_bank()])
            .is_err()
    );
    let c = c.replace(
        "type sound extends agent",
        "type cue extends agent\ntype sound extends cue",
    );
    host(&c, "", 4)
        .with_label_sound_creation(vec![ui_bank()])
        .unwrap();
}

#[test]
fn label_spatial_assignments_require_the_executed_creation_flag() {
    let mut c = common();
    for (name, tail) in [
        ("SetSoundChannel", "integer v"),
        ("SetSoundVolume", "integer v"),
        ("SetSoundPitch", "real v"),
        ("SetSoundDistanceCutoff", "real v"),
        ("SetSoundDistances", "real a, real b"),
        ("SetSoundConeAngles", "real a, real b, integer v"),
        ("SetSoundConeOrientation", "real x, real y, real z"),
        ("SetSoundPosition", "real x, real y, real z"),
        ("SetSoundVelocity", "real x, real y, real z"),
    ] {
        c.push_str(&format!(
            "native {name} takes sound s, {tail} returns nothing\n"
        ));
    }
    let mut h = host(&c, "", 8)
        .with_label_sound_creation(vec![ui_bank()])
        .unwrap()
        .with_sound_properties()
        .unwrap();
    let spatial = create(&mut h, [false, true, false], [0; 2]).unwrap();
    let flat = create(&mut h, [false; 3], [0; 2]).unwrap();
    for sound in [&spatial, &flat] {
        let result = h.native(
            "SetSoundPosition",
            &[
                sound.clone(),
                Value::Real(1.5),
                Value::Real(-3.0),
                Value::Real(0.0),
            ],
        );
        assert_eq!(result.is_ok(), sound == &spatial);
    }
    assert_eq!(
        h.sound_properties(id(&spatial)).unwrap().position,
        Some([1.5, -3.0, 0.0])
    );
    assert_eq!(h.sound_properties(id(&flat)).unwrap().position, None);
    assert_eq!(
        h.sound_state(id(&spatial))
            .unwrap()
            .bank
            .table()
            .data_rows()
            .next()
            .unwrap()
            .by_name("Volume"),
        Some("71")
    );
}
