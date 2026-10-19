use super::*;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;
fn origin() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "test".into(),
        source: "synthetic:archive".into(),
    }
}
fn chunk(tag: &[u8; 4], data: &[u8], out: &mut Vec<u8>) {
    out.extend(tag);
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(data);
    if !data.len().is_multiple_of(2) {
        out.push(99);
    }
}
fn riff(chunks: &[([u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut d = b"RIFF\0\0\0\0WAVE".to_vec();
    for (tag, b) in chunks {
        chunk(tag, b, &mut d);
    }
    let n = (d.len() - 8) as u32;
    d[4..8].copy_from_slice(&n.to_le_bytes());
    d
}
fn fmt(channels: u16, rate: u32, bits: u16) -> Vec<u8> {
    let align = channels * (bits / 8);
    let mut d = Vec::new();
    d.extend(1u16.to_le_bytes());
    d.extend(channels.to_le_bytes());
    d.extend(rate.to_le_bytes());
    d.extend((rate * u32::from(align)).to_le_bytes());
    d.extend(align.to_le_bytes());
    d.extend(bits.to_le_bytes());
    d
}
fn wave() -> Vec<u8> {
    riff(&[(*b"fmt ", fmt(2, 17300, 16)), (*b"data", vec![0x12; 28])])
}
fn table(name: &str, files: &str) -> Vec<u8> {
    let rows = [
        vec![
            "Selector",
            "DirectoryBase",
            "FileNames",
            "Volume",
            "Pitch",
            "Flags",
        ],
        vec![name, r"Media\Loop", files, "71", "0.875", "SYNTHETIC_FLAG"],
    ];
    let mut s = "ID;PWXL;N;E\n".to_string();
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.iter().enumerate() {
            s += &format!("C;X{};Y{};K\"{}\"\n", x + 1, y + 1, c.replace('"', "\"\""));
        }
    }
    s += "E\n";
    s.replace('\n', "\r\n").into_bytes()
}
fn limits() -> Budget {
    Budget {
        table_bytes: 4096,
        coordinates: CoordinateLimits {
            max_row: 100,
            max_column: 32,
        },
        sounds: 4,
        resource_bytes: 4096,
        total_resource_bytes: 8192,
        metadata_bytes: 4096,
    }
}
fn source<'a>(bytes: &'a [u8]) -> SoundInput<'a> {
    SoundInput {
        label: "Breeze",
        member: r"Media\Loop\sample.wav",
        asset: ScriptAsset {
            label: "synthetic:wave",
            bytes,
        },
        origin: origin(),
    }
}
fn bank() -> SoundBank {
    SoundBank::read(
        ScriptAsset {
            label: "synthetic:slk",
            bytes: &table("Breeze", "sample.wav"),
        },
        origin(),
        "Selector",
        &[source(&wave())],
        limits(),
    )
    .unwrap()
}
#[test]
fn pcm_dimensions_sample_ranges_and_unknown_chunks_are_source_derived() {
    for (channels, rate, bits, frames) in [(1, 12000, 8, 3), (2, 17300, 16, 7)] {
        let payload = vec![0x28; channels as usize * (bits as usize / 8) * frames];
        let bytes = riff(&[
            (*b"JUNK", vec![1, 2, 3]),
            (*b"data", payload.clone()),
            (*b"fmt ", fmt(channels, rate, bits)),
        ]);
        let p = PcmWave::inspect(&bytes).unwrap();
        assert_eq!(
            (p.channels, p.sample_rate, p.bits_per_sample, p.frames()),
            (channels, rate, bits, frames)
        );
        assert_eq!(&bytes[p.data_range()], payload);
    }
}
#[test]
fn malformed_wave_sizes_formats_rates_and_alignment_are_not_accepted() {
    let mut bad = Vec::new();
    bad.push(b"RIFF".to_vec());
    let mut d = wave();
    d[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    bad.push(d);
    let mut d = wave();
    d[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    bad.push(d);
    bad.push(riff(&[(*b"fmt ", fmt(2, 17300, 16))]));
    bad.push(riff(&[(*b"data", vec![0; 4])]));
    bad.push(riff(&[
        (*b"fmt ", fmt(2, 17300, 16)),
        (*b"fmt ", fmt(2, 17300, 16)),
        (*b"data", vec![0; 4]),
    ]));
    bad.push(riff(&[
        (*b"fmt ", fmt(2, 17300, 16)),
        (*b"data", vec![0; 4]),
        (*b"data", vec![0; 4]),
    ]));
    bad.push(riff(&[(*b"fmt ", vec![0; 15]), (*b"data", vec![0; 4])]));
    for (offset, value) in [(0, 85u32), (2, 3), (4, 0), (8, 1), (12, 1), (14, 24)] {
        let mut f = fmt(2, 17300, 16);
        if [4, 8].contains(&offset) {
            f[offset..offset + 4].copy_from_slice(&value.to_le_bytes())
        } else {
            f[offset..offset + 2].copy_from_slice(&(value as u16).to_le_bytes())
        }
        bad.push(riff(&[(*b"fmt ", f), (*b"data", vec![0; 4])]));
    }
    bad.push(riff(&[
        (*b"fmt ", fmt(2, 17300, 16)),
        (*b"data", vec![0; 3]),
    ]));
    let mut d = riff(&[(*b"fmt ", fmt(1, 12000, 8)), (*b"data", vec![0; 3])]);
    d.pop();
    let n = (d.len() - 8) as u32;
    d[4..8].copy_from_slice(&n.to_le_bytes());
    bad.push(d);
    for d in bad {
        assert!(PcmWave::inspect(&d).is_err());
    }
}
#[test]
fn source_table_metadata_and_complete_pcm_resource_are_retained() {
    let bank = bank();
    let sound = &bank.sounds()[0];
    assert_eq!(sound.label(), "Breeze");
    assert_eq!(sound.member(), r"Media\Loop\sample.wav");
    assert_eq!(sound.bytes(), wave());
    assert_eq!(sound.sha256(), sha256(&wave()));
    assert_eq!(sound.pcm().frames(), 7);
    assert_eq!(sound.origin(), &origin());
    assert_eq!(sound.source_label(), "synthetic:wave");
    assert_eq!(bank.table().row(sound.row()).by_name("Volume"), Some("71"));
    assert_eq!(
        bank.table().row(sound.row()).by_name("Pitch"),
        Some("0.875")
    );
    assert_eq!(bank.sha256(), sha256(&table("Breeze", "sample.wav")));
    assert_eq!(bank.origin(), &origin());
    assert_eq!(bank.source_label(), "synthetic:slk");
}
#[test]
fn whole_input_preflight_and_exact_resource_boundaries_precede_parsing() {
    let bytes = wave();
    let data = table("Breeze", "sample.wav");
    let mut b = limits();
    b.resource_bytes = bytes.len() - 1;
    assert!(matches!(
        SoundBank::read(
            ScriptAsset {
                label: "bad table",
                bytes: b"broken"
            },
            origin(),
            "Selector",
            &[source(&bytes)],
            b
        )
        .unwrap_err()
        .problem,
        Problem::Budget
    ));
    b = limits();
    b.resource_bytes = bytes.len();
    b.total_resource_bytes = bytes.len();
    b.table_bytes = data.len();
    assert!(
        SoundBank::read(
            ScriptAsset {
                label: "slk",
                bytes: &data
            },
            origin(),
            "Selector",
            &[source(&bytes)],
            b
        )
        .is_ok()
    );
    for which in 0..6 {
        let mut b = limits();
        match which {
            0 => b.table_bytes = data.len() - 1,
            1 => b.sounds = 0,
            2 => b.resource_bytes = bytes.len() - 1,
            3 => b.total_resource_bytes = bytes.len() - 1,
            4 => b.metadata_bytes = 0,
            _ => b.coordinates.max_row = 1,
        }
        assert!(
            SoundBank::read(
                ScriptAsset {
                    label: "slk",
                    bytes: &data
                },
                origin(),
                "Selector",
                &[source(&bytes)],
                b
            )
            .is_err()
        );
    }
    for which in 0..5 {
        let mut b = limits();
        match which {
            0 => b.table_bytes = usize::MAX,
            1 => b.sounds = usize::MAX,
            2 => b.resource_bytes = usize::MAX,
            3 => b.total_resource_bytes = usize::MAX,
            _ => b.metadata_bytes = usize::MAX,
        }
        assert!(matches!(
            SoundBank::read(
                ScriptAsset {
                    label: "slk",
                    bytes: &data
                },
                origin(),
                "Selector",
                &[],
                b
            )
            .unwrap_err()
            .problem,
            Problem::Budget
        ));
    }
}
#[test]
fn unqualified_ambiguous_missing_or_multi_file_bindings_refuse() {
    let bytes = wave();
    let data = table("Breeze", "sample.wav");
    assert!(matches!(
        SoundBank::read(
            ScriptAsset {
                label: "slk",
                bytes: &data
            },
            origin(),
            "Selector",
            &[source(&bytes), source(&bytes)],
            limits()
        )
        .unwrap_err()
        .problem,
        Problem::Ambiguous
    ));
    for files in ["sample.wav,other.wav", "_", ""] {
        assert!(
            SoundBank::read(
                ScriptAsset {
                    label: "slk",
                    bytes: &table("Breeze", files)
                },
                origin(),
                "Selector",
                &[source(&bytes)],
                limits()
            )
            .is_err()
        );
    }
    for column in ["Missing", "DirectoryBase"] {
        assert!(
            SoundBank::read(
                ScriptAsset {
                    label: "slk",
                    bytes: &data
                },
                origin(),
                column,
                &[source(&bytes)],
                limits()
            )
            .is_err()
        );
    }
    assert!(
        SoundBank::read(
            ScriptAsset {
                label: "slk",
                bytes: &table("Other", "sample.wav")
            },
            origin(),
            "Selector",
            &[source(&bytes)],
            limits()
        )
        .is_err()
    );
    let mut input = source(&bytes);
    input.member = r"Other\sample.wav";
    assert!(matches!(
        SoundBank::read(
            ScriptAsset {
                label: "slk",
                bytes: &data
            },
            origin(),
            "Selector",
            &[input],
            limits()
        )
        .unwrap_err()
        .problem,
        Problem::Member
    ));
    let mut input = source(&bytes);
    input.origin.version.clear();
    assert!(
        SoundBank::read(
            ScriptAsset {
                label: "slk",
                bytes: &data
            },
            origin(),
            "Selector",
            &[input],
            limits()
        )
        .is_err()
    );
    let original = String::from_utf8(data.clone()).unwrap();
    let malformed =
        original.strip_suffix("E\r\n").unwrap().to_owned() + "C;X1;Y3;K\"Breeze\"\r\nE\r\n";
    assert!(
        SoundBank::read(
            ScriptAsset {
                label: "slk",
                bytes: malformed.as_bytes()
            },
            origin(),
            "Selector",
            &[source(&bytes)],
            limits()
        )
        .is_err()
    );
}
#[test]
fn advisory_dimensions_remain_metadata_while_duplicate_values_refuse() {
    let original = String::from_utf8(table("Breeze", "sample.wav")).unwrap();
    let advisory = original.replacen("ID;PWXL;N;E\r\n", "ID;PWXL;N;E\r\nB;X99;Y999\r\n", 1);
    let bank = SoundBank::read(
        ScriptAsset {
            label: "advisory table",
            bytes: advisory.as_bytes(),
        },
        origin(),
        "Selector",
        &[source(&wave())],
        limits(),
    )
    .unwrap();
    assert_eq!(bank.table().row_count(), 2);
    assert_eq!(bank.table().declared().unwrap().rows, 999);
    assert!(
        bank.table()
            .anomalies()
            .iter()
            .any(|a| a.kind == pcx_slq::AnomalyKind::DeclaredSizeMismatch)
    );
    let duplicate =
        original.strip_suffix("E\r\n").unwrap().to_owned() + "C;X4;Y2;K\"999\"\r\nE\r\n";
    assert!(matches!(
        SoundBank::read(
            ScriptAsset {
                label: "duplicate table",
                bytes: duplicate.as_bytes()
            },
            origin(),
            "Selector",
            &[source(&wave())],
            limits()
        )
        .unwrap_err()
        .problem,
        Problem::Ambiguous
    ));
}
const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype sound extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateMIDISound takes string label, integer a, integer b returns sound\nnative StartSound takes sound soundHandle returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
fn enums(common: &str, map: &str, max_handles: usize) -> EnumHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let p = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    EnumHost::checked_selected(
        p,
        "synthetic:sounds",
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
fn create(h: &mut ObjectHost, label: &str, a: i32, b: i32) -> Result<Value, String> {
    h.native(
        "CreateMIDISound",
        &[Value::Str(label.into()), Value::Int(a), Value::Int(b)],
    )
}
#[test]
fn real_sound_objects_share_the_arena_and_preserve_exact_authored_rates() {
    let mut disabled = host(COMMON, "", 4);
    assert!(create(&mut disabled, "Breeze", 45, -7).is_err());
    let mut h = disabled.with_label_sounds(bank()).unwrap();
    let a = create(&mut h, "Breeze", 45, -7).unwrap();
    let b = create(&mut h, "Breeze", 0, 91).unwrap();
    assert_ne!(a, b);
    let (Value::Handle(a), Value::Handle(b)) = (a, b) else {
        panic!("sound handles")
    };
    let sa = h.sound_state(a).unwrap();
    assert_eq!(
        (sa.identity, sa.fade_in_rate, sa.fade_out_rate),
        (a, 45, -7)
    );
    assert!(std::ptr::eq(
        sa.resource,
        h.sound_state(b).unwrap().resource
    ));
    assert_eq!(h.inner().kind(a).unwrap(), "sound");
    let timer = h.native("CreateTimer", &[]).unwrap();
    let color = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    assert_ne!(timer, Value::Handle(a));
    assert_ne!(color, Value::Handle(a));
    assert_eq!(h.inner().handles(), 4);
    assert!(create(&mut h, "Breeze", 1, 2).is_err());
    assert_eq!(h.inner().handles(), 4);
    assert_eq!(h.sound_state(a).unwrap().fade_out_rate, -7);
    assert!(h.with_label_sounds(bank()).is_err());
}
#[test]
fn unknown_labels_wrong_families_and_bad_arguments_do_not_allocate() {
    let mut h = host(COMMON, "", 8).with_label_sounds(bank()).unwrap();
    assert!(
        create(&mut h, "unbound", 1, 2)
            .unwrap_err()
            .contains("unbound")
    );
    assert!(create(&mut h, &"x".repeat(4097), 1, 2).is_err());
    for args in [
        vec![],
        vec![Value::Null, Value::Int(1), Value::Int(2)],
        vec![Value::Str("Breeze".into()), Value::Real(1.0), Value::Int(2)],
    ] {
        assert!(h.native("CreateMIDISound", &args).is_err());
    }
    assert_eq!(h.inner().handles(), 0);
    let Value::Handle(timer) = h.native("CreateTimer", &[]).unwrap() else {
        panic!("timer")
    };
    assert!(h.sound_state(timer).is_err());
    let mut e = enums(COMMON, "", 4);
    let reserved = e.reserve_identity("sound").unwrap();
    let h = ObjectHost::checked(e)
        .unwrap()
        .with_label_sounds(bank())
        .unwrap();
    assert!(h.sound_state(reserved).is_err());
}
#[test]
fn exact_sound_signatures_family_ancestry_and_script_overrides_are_required() {
    for common in [
        COMMON.replace("native CreateMIDISound", "constant native CreateMIDISound"),
        COMMON.replace("integer b returns sound", "integer b returns timer"),
        COMMON.replace("integer a, integer b", "real a, integer b"),
        COMMON.replace("type sound extends agent", "type sound extends handle"),
    ] {
        assert!(host(&common, "", 4).with_label_sounds(bank()).is_err());
    }
    assert!(host(COMMON,"function CreateMIDISound takes string s, integer a, integer b returns sound\nreturn null\nendfunction\n",4).with_label_sounds(bank()).is_err());
}
#[test]
fn owned_vm_retains_created_sound_resources_after_entrypoint_failure() {
    let common = format!("{COMMON}native Next takes nothing returns nothing\n");
    let mut vm=host(&common,"globals\nsound ambient=null\nendglobals\nfunction main takes nothing returns nothing\nset ambient=CreateMIDISound(\"Breeze\",45,91)\ncall Next()\nendfunction\n",4).with_label_sounds(bank()).unwrap().into_vm();
    vm.init_globals().unwrap();
    assert!(
        vm.call("main", &[])
            .unwrap_err()
            .to_string()
            .contains("Next")
    );
    let Value::Handle(id) = vm.global("ambient") else {
        panic!("retained sound")
    };
    assert_eq!(
        vm.host().sound_state(id).unwrap().resource.pcm().frames(),
        7
    );
    let handles = vm.host().inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner().handles(), handles);
}
#[test]
fn bounded_sound_starts_preserve_order_repeats_and_resource_ownership() {
    let mut h = host(COMMON, "", 8)
        .with_label_sounds(bank())
        .unwrap()
        .with_sound_starts(3)
        .unwrap();
    let a = create(&mut h, "Breeze", 45, -7).unwrap();
    let b = create(&mut h, "Breeze", 8, 9).unwrap();
    let (Value::Handle(a), Value::Handle(b)) = (a, b) else {
        panic!("sound identities")
    };
    let handles = h.inner().handles();
    for id in [b, a, b] {
        assert_eq!(
            h.native("StartSound", &[Value::Handle(id)]).unwrap(),
            Value::Null
        );
    }
    assert_eq!(h.pending_sound_starts(), &[b, a, b]);
    assert!(
        h.native("StartSound", &[Value::Handle(a)])
            .unwrap_err()
            .contains("pending sound starts")
    );
    assert_eq!(h.pending_sound_starts(), &[b, a, b]);
    assert_eq!(h.inner().handles(), handles);
    let pending = h.take_sound_starts();
    assert_eq!(pending, [b, a, b]);
    assert!(h.pending_sound_starts().is_empty());
    assert_eq!(
        h.sound_state(pending[0]).unwrap().resource.pcm().frames(),
        7
    );
    assert_eq!(h.sound_state(a).unwrap().fade_in_rate, 45);
    assert_eq!(
        h.native("StartSound", &[Value::Handle(a)]).unwrap(),
        Value::Null
    );
    assert_eq!(h.take_sound_starts(), [a]);
    assert!(h.take_sound_starts().is_empty());
}
#[test]
fn bad_start_arguments_and_wrong_or_reserved_families_never_enqueue() {
    let mut e = enums(COMMON, "", 8);
    let fake = e.reserve_identity("sound").unwrap();
    let mut h = ObjectHost::checked(e)
        .unwrap()
        .with_label_sounds(bank())
        .unwrap()
        .with_sound_starts(8)
        .unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let color = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Int(7)],
        vec![Value::Handle(fake)],
        vec![timer],
        vec![color],
        vec![Value::Null, Value::Null],
    ] {
        let error = h.native("StartSound", &args).unwrap_err();
        assert!(error.contains("StartSound"));
        assert!(h.pending_sound_starts().is_empty());
    }
}
#[test]
fn start_admission_requires_bank_exact_contract_and_valid_platform_budget() {
    assert!(host(COMMON, "", 8).with_sound_starts(3).is_err());
    let mut disabled = host(COMMON, "", 8).with_label_sounds(bank()).unwrap();
    let sound = create(&mut disabled, "Breeze", 1, 2).unwrap();
    assert!(
        disabled
            .native("StartSound", std::slice::from_ref(&sound))
            .is_err()
    );
    assert!(disabled.pending_sound_starts().is_empty());
    let mut zero = disabled.with_sound_starts(0).unwrap();
    assert!(zero.native("StartSound", &[sound]).is_err());
    assert!(zero.pending_sound_starts().is_empty());
    assert!(zero.with_sound_starts(4).is_err());
    let platform_overflow = (isize::MAX as usize / std::mem::size_of::<pcx_script::HandleId>()) + 1;
    assert!(
        host(COMMON, "", 8)
            .with_label_sounds(bank())
            .unwrap()
            .with_sound_starts(platform_overflow)
            .is_err()
    );
    for bad in [
        COMMON.replace("native StartSound", "constant native StartSound"),
        COMMON.replace(
            "takes sound soundHandle returns nothing",
            "takes timer soundHandle returns nothing",
        ),
        COMMON.replace(
            "takes sound soundHandle returns nothing",
            "takes sound soundHandle returns integer",
        ),
    ] {
        assert!(
            host(&bad, "", 8)
                .with_label_sounds(bank())
                .unwrap()
                .with_sound_starts(2)
                .is_err()
        );
    }
    assert!(
        host(
            COMMON,
            "function StartSound takes sound s returns nothing\nendfunction\n",
            8
        )
        .with_label_sounds(bank())
        .unwrap()
        .with_sound_starts(2)
        .is_err()
    );
}
#[test]
fn retained_vm_start_requests_survive_later_failure_without_reinitialization() {
    let common = format!("{COMMON}native Next takes nothing returns nothing\n");
    let mut vm=host(&common,"globals\nsound ambient=null\nendglobals\nfunction main takes nothing returns nothing\nset ambient=CreateMIDISound(\"Breeze\",45,91)\ncall StartSound(ambient)\ncall Next()\nendfunction\n",4).with_label_sounds(bank()).unwrap().with_sound_starts(3).unwrap().into_vm();
    vm.init_globals().unwrap();
    assert!(
        vm.call("main", &[])
            .unwrap_err()
            .to_string()
            .contains("Next")
    );
    let Value::Handle(id) = vm.global("ambient") else {
        panic!("retained sound")
    };
    assert_eq!(vm.host().pending_sound_starts(), [id]);
    let handles = vm.host().inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner().handles(), handles);
    assert_eq!(vm.host().pending_sound_starts(), [id]);
    assert_eq!(
        vm.host().sound_state(id).unwrap().resource.pcm().frames(),
        7
    );
}

#[path = "label_constructor_tests.rs"]
mod constructor;

fn named_bank(label: &str) -> SoundBank {
    let bytes = wave();
    let mut input = source(&bytes);
    input.label = label;
    SoundBank::read(
        ScriptAsset {
            label: "synthetic:slk",
            bytes: &table(label, "sample.wav"),
        },
        origin(),
        "Selector",
        &[input],
        limits(),
    )
    .unwrap()
}
fn loading_budget() -> LoadBudget {
    let mut inputs = limits();
    inputs.sounds = 8;
    LoadBudget {
        banks: 8,
        total_table_bytes: 32768,
        inputs,
    }
}
fn loading_host() -> ObjectHost {
    let common = COMMON.replacen("globals\n", "native CreateSoundFromLabel takes string s, boolean a, boolean b, boolean c, integer x, integer y returns sound\nglobals\n", 1);
    host(&common, "", 64)
        .with_label_sounds(bank())
        .unwrap()
        .with_label_sound_creation(vec![bank()])
        .unwrap()
}
fn create_label(h: &mut ObjectHost, label: &str) -> Result<Value, String> {
    h.native(
        "CreateSoundFromLabel",
        &[
            Value::Str(label.into()),
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(true),
            Value::Int(-7),
            Value::Int(91),
        ],
    )
}
fn handle(value: Value) -> pcx_script::HandleId {
    let Value::Handle(id) = value else {
        panic!("handle required")
    };
    id
}
#[test]
fn executed_label_loading_appends_without_changing_old_indices_or_buffers() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut h = loading_host();
    let first = handle(create(&mut h, "Breeze", 3, 5).unwrap());
    let original = h.sound_resource(first).unwrap().bytes().as_ptr();
    h.admit_label_sound_loader(loading_budget(), move |label, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(named_bank(label))
    })
    .unwrap();
    let one = handle(create(&mut h, "One", 45, 91).unwrap());
    let two = handle(create_label(&mut h, "Two").unwrap());
    let again = handle(create(&mut h, "One", -1, 8).unwrap());
    assert_ne!(one, again);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(h.sound_state(one).unwrap().resource.label(), "One");
    assert_eq!(h.sound_state(two).unwrap().resource.label(), "Two");
    assert_eq!(h.sound_state(again).unwrap().fade_in_rate, -1);
    assert_eq!(h.sound_resource(first).unwrap().bytes().as_ptr(), original);
    assert_eq!(h.sound_state(first).unwrap().resource.label(), "Breeze");
    assert!(h.sound_state(two).unwrap().creation_flags.unwrap().looping);
    assert_eq!(h.sound_resource_bytes().unwrap(), wave().len() * 4);
}
#[test]
fn label_loader_rejects_wrong_bindings_and_arguments_before_allocating() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut h = loading_host();
    h.admit_label_sound_loader(loading_budget(), move |_, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(bank())
    })
    .unwrap();
    let before = handle(h.native("CreateTimer", &[]).unwrap());
    for label in ["", "\0", "   "] {
        assert!(create_label(&mut h, label).is_err());
    }
    assert!(
        h.native(
            "CreateMIDISound",
            &[Value::Int(0), Value::Int(0), Value::Int(0)]
        )
        .is_err()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(
        create_label(&mut h, "Other")
            .unwrap_err()
            .contains("one matching label")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let after = handle(h.native("CreateTimer", &[]).unwrap());
    assert_eq!(after.token(), before.token() + 1);
    assert!(
        h.admit_label_sound_loader(loading_budget(), |_, _| Ok(bank()))
            .is_err()
    );
    let id = handle(create(&mut h, "Breeze", 0, 0).unwrap());
    assert_eq!(h.sound_state(id).unwrap().resource.label(), "Breeze");
}
#[test]
fn label_loading_enforces_cumulative_input_limits_before_source_work() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    for kind in 0..6 {
        let mut budget = loading_budget();
        let mut h = loading_host();
        match kind {
            0 => budget.banks = 1,
            1 => budget.total_table_bytes = 0,
            2 => budget.inputs.sounds = 1,
            3 => budget.inputs.resource_bytes = wave().len() - 1,
            4 => budget.inputs.total_resource_bytes = wave().len() * 2 - 1,
            _ => budget.inputs.metadata_bytes = 1,
        }
        assert!(
            h.admit_label_sound_loader(budget, |_, _| Ok(bank()))
                .is_err(),
            "{kind}"
        );
    }
    let mut h = loading_host();
    let mut budget = loading_budget();
    budget.inputs.sounds = 2;
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    h.admit_label_sound_loader(budget, move |_, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(bank())
    })
    .unwrap();
    assert!(create_label(&mut h, "Other").is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(create_label(&mut h, "Breeze").is_ok());
    let mut h = loading_host();
    let mut budget = loading_budget();
    budget.inputs.total_resource_bytes = wave().len() * 2 + 1;
    h.admit_label_sound_loader(budget, |label, _| Ok(named_bank(label)))
        .unwrap();
    assert!(create_label(&mut h, "Other").is_err());
    assert_eq!(h.sound_resource_bytes().unwrap(), wave().len() * 2);
}
