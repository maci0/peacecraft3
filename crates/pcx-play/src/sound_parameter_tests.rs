use super::*;
use crate::file_sounds;
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
fn rows() -> Vec<Vec<&'static str>> {
    vec![
        vec![
            "Selector",
            "Volume",
            "Pitch",
            "PitchVariance",
            "Priority",
            "Channel",
            "MinDistance",
            "MaxDistance",
            "DistanceCutoff",
            "Flags",
            "EAXFlags",
            "DirectoryBase",
            "FileNames",
        ],
        vec![
            "Voice",
            "83",
            "0.875",
            "0.125",
            "-3",
            "17",
            "25.5",
            "900.25",
            "2700",
            "OPAQUE,SPATIAL",
            "ProfileA",
            "Other",
            "one.wav,two.wav",
        ],
        vec![
            "Other",
            "-14",
            "1.25",
            "0",
            "92",
            "-1",
            "0",
            "601",
            "1900.5",
            "LOOP_DECLARATION",
            "ProfileB",
            "Elsewhere",
            "_",
        ],
    ]
}
fn table(rows: &[Vec<&str>]) -> Vec<u8> {
    let mut s = "ID;PWXL;N;E\r\n".to_owned();
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.iter().enumerate() {
            s += &format!(
                "C;X{};Y{};K\"{}\"\r\n",
                x + 1,
                y + 1,
                c.replace('"', "\"\"")
            );
        }
    }
    s += "E\r\n";
    s.into_bytes()
}
fn budget() -> Budget {
    Budget {
        table_bytes: 8192,
        coordinates: CoordinateLimits {
            max_row: 100,
            max_column: 32,
        },
        definitions: 4,
        metadata_bytes: 256,
    }
}
fn read(data: &[u8], labels: &[&str], b: Budget) -> Result<Bank, Error> {
    Bank::read(
        ScriptAsset {
            label: "synthetic:slk",
            bytes: data,
        },
        origin(),
        "Selector",
        labels,
        b,
    )
}
fn bank() -> Bank {
    read(&table(&rows()), &["Voice", "Other"], budget()).unwrap()
}
#[test]
fn exact_selected_parameters_and_provenance_need_no_pcm_or_filename_choice() {
    let d = table(&rows());
    let bank = read(&d, &["Voice", "Other"], budget()).unwrap();
    let a = bank.get(0).unwrap();
    assert_eq!(
        (a.label, a.row, a.flags, a.eax_flags),
        ("Voice", 2, "OPAQUE,SPATIAL", "ProfileA")
    );
    assert_eq!(
        *a.values,
        Values {
            volume: 83,
            pitch: 0.875,
            pitch_variance: 0.125,
            priority: -3,
            channel: 17,
            min_distance: 25.5,
            max_distance: 900.25,
            distance_cutoff: 2700.0
        }
    );
    assert_eq!(bank.get(1).unwrap().values.volume, -14);
    assert!(bank.get(2).is_none());
    assert_eq!(
        bank.table().by_name(2, "FileNames"),
        Some("one.wav,two.wav")
    );
    assert_eq!(bank.origin(), &origin());
    assert_eq!(bank.source_label(), "synthetic:slk");
    assert_eq!(bank.sha256(), sha256(&d));
}
#[test]
fn preflight_budgets_identity_and_duplicates_precede_table_decode() {
    for which in 0..7 {
        let mut b = budget();
        match which {
            0 => b.table_bytes = 0,
            1 => b.definitions = 0,
            2 => b.metadata_bytes = 0,
            3 => b.table_bytes = usize::MAX,
            4 => b.definitions = usize::MAX,
            5 => b.metadata_bytes = usize::MAX,
            _ => b.coordinates.max_row = 1,
        }
        assert!(read(b"invalid", &["Voice"], b).is_err());
    }
    for labels in [&["Voice", "Voice"][..], &[""][..], &["X\0Y"][..]] {
        assert!(read(b"invalid", labels, budget()).is_err());
    }
    let mut o = origin();
    o.version.clear();
    assert!(matches!(
        Bank::read(
            ScriptAsset {
                label: "slk",
                bytes: b"invalid"
            },
            o,
            "Selector",
            &[],
            budget()
        )
        .unwrap_err()
        .problem,
        Problem::Identity
    ));
    let d = table(&rows());
    let mut b = budget();
    b.table_bytes = d.len();
    b.definitions = 2;
    assert!(read(&d, &["Voice", "Other"], b).is_ok());
}
#[test]
fn selected_numeric_fields_reject_missing_overflow_and_nonfinite_without_defaults() {
    for (column, bad) in [
        (1, "2147483648"),
        (2, "NaN"),
        (3, "inf"),
        (4, "1.5"),
        (5, "_"),
        (6, "1e100"),
        (7, "-inf"),
        (8, ""),
    ] {
        let mut r = rows();
        r[1][column] = bad;
        assert!(matches!(
            read(&table(&r), &["Voice"], budget()).unwrap_err().problem,
            Problem::Parameter { row: 2, .. }
        ));
    }
    let mut r = rows();
    r[1].remove(8);
    assert!(read(&table(&r), &["Voice"], budget()).is_err());
    let mut r = rows();
    r[2][1] = "malformed unselected row";
    assert!(read(&table(&r), &["Voice"], budget()).is_ok());
}
#[test]
fn duplicate_headers_rows_cells_and_trailing_data_are_ambiguous() {
    let mut r = rows();
    r[0][2] = "Volume";
    assert!(read(&table(&r), &["Voice"], budget()).is_err());
    let mut r = rows();
    r[2][0] = "Voice";
    assert!(matches!(
        read(&table(&r), &["Voice"], budget()).unwrap_err().problem,
        Problem::Ambiguous
    ));
    assert!(matches!(
        read(&table(&rows()), &["Missing"], budget())
            .unwrap_err()
            .problem,
        Problem::Missing
    ));
    let original = String::from_utf8(table(&rows())).unwrap();
    let duplicate = original.strip_suffix("E\r\n").unwrap().to_owned() + "C;X2;Y2;K\"0\"\r\nE\r\n";
    assert!(matches!(
        read(duplicate.as_bytes(), &["Voice"], budget())
            .unwrap_err()
            .problem,
        Problem::Ambiguous
    ));
    let trailing = original + "C;X2;Y2;K\"0\"\r\n";
    assert!(read(trailing.as_bytes(), &["Voice"], budget()).is_err());
}
const COMMON: &str = "type agent extends handle\ntype sound extends agent\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateSound takes string f, boolean repeating, boolean spatial, boolean stop, integer a, integer b, string eax returns sound\nnative CreateMIDISound takes string s, integer a, integer b returns sound\nnative SetSoundParamsFromLabel takes sound s, string label returns nothing\nnative StartSound takes sound s returns nothing\nnative SetSoundDuration takes sound s, integer duration returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
fn enums(common: &str, map: &str) -> EnumHost {
    let c = pcx_script::parse_script(common).unwrap();
    let m = pcx_script::parse_script(map).unwrap();
    let p =
        Arc::new(pcx_script::resolve_with(&m, &[Prelude::new("synthetic:common", &c)]).unwrap());
    EnumHost::checked_selected(
        p,
        "synthetic:parameters",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 16,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap()
}
fn pcm() -> Vec<u8> {
    let mut d = b"RIFF\x26\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0".to_vec();
    d.extend(11000u32.to_le_bytes());
    d.extend(22000u32.to_le_bytes());
    d.extend([2, 0, 16, 0]);
    d.extend(b"data\x02\0\0\0\x15\x19");
    d
}
fn assets() -> file_sounds::Assets {
    file_sounds::Assets::read(
        &[file_sounds::SoundInput {
            member: "existing.wav",
            asset: ScriptAsset {
                label: "synthetic:PCM",
                bytes: &pcm(),
            },
            origin: origin(),
        }],
        file_sounds::Budget {
            resources: 1,
            resource_bytes: 1024,
            total_bytes: 1024,
            metadata_bytes: 256,
        },
    )
    .unwrap()
}
fn host(common: &str, map: &str) -> ObjectHost {
    ObjectHost::checked(enums(common, map))
        .unwrap()
        .with_file_sounds(assets(), 32)
        .unwrap()
}
fn create(h: &mut ObjectHost) -> pcx_script::HandleId {
    let Value::Handle(id) = h
        .native(
            "CreateSound",
            &[
                Value::Str("existing.wav".into()),
                Value::Bool(true),
                Value::Bool(false),
                Value::Bool(true),
                Value::Int(-7),
                Value::Int(91),
                Value::Str("CreationEAX".into()),
            ],
        )
        .unwrap()
    else {
        panic!("sound")
    };
    id
}
#[test]
fn parameter_assignment_changes_only_the_selected_identity_not_resource_or_creation_inputs() {
    let mut h = host(COMMON, "")
        .with_sound_parameters(bank())
        .unwrap()
        .with_sound_starts(4)
        .unwrap();
    let a = create(&mut h);
    let b = create(&mut h);
    assert!(h.sound_parameters(a).unwrap().is_none());
    let before = h.sound_resource(a).unwrap() as *const _;
    for label in ["Voice", "Other", "Voice"] {
        h.native(
            "SetSoundParamsFromLabel",
            &[Value::Handle(a), Value::Str(label.into())],
        )
        .unwrap();
        assert_eq!(h.sound_parameters(a).unwrap().unwrap().label, label);
        assert!(h.sound_parameters(b).unwrap().is_none());
        assert_eq!(h.sound_resource(a).unwrap() as *const _, before);
    }
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
        (true, false, true, -7, 91, "CreationEAX")
    );
    assert_eq!(h.inner().handles(), 2);
    for id in [b, a, a] {
        h.native("StartSound", &[Value::Handle(id)]).unwrap();
    }
    assert_eq!(h.take_sound_starts(), [b, a, a]);
    assert_eq!(
        h.sound_parameter_bank()
            .unwrap()
            .get(0)
            .unwrap()
            .values
            .volume,
        83
    );
}
#[test]
fn failed_assignments_are_atomic_and_reserved_or_wrong_family_objects_refuse() {
    let mut e = enums(COMMON, "");
    let fake = e.reserve_identity("sound").unwrap();
    let mut h = ObjectHost::checked(e)
        .unwrap()
        .with_file_sounds(assets(), 32)
        .unwrap()
        .with_sound_parameters(bank())
        .unwrap();
    let a = create(&mut h);
    h.native(
        "SetSoundParamsFromLabel",
        &[Value::Handle(a), Value::Str("Voice".into())],
    )
    .unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    for args in [
        vec![Value::Handle(a), Value::Str("Missing".into())],
        vec![Value::Handle(a), Value::Str("x".repeat(257))],
        vec![Value::Handle(fake), Value::Str("Voice".into())],
        vec![timer, Value::Str("Voice".into())],
        vec![Value::Null, Value::Str("Voice".into())],
        vec![Value::Handle(a), Value::Int(1)],
    ] {
        assert!(h.native("SetSoundParamsFromLabel", &args).is_err());
        assert_eq!(h.sound_parameters(a).unwrap().unwrap().label, "Voice");
        assert_eq!(h.inner().handles(), 3);
    }
}
#[test]
fn contract_and_explicit_admission_do_not_turn_unsupported_calls_into_noops() {
    let mut h = host(COMMON, "");
    let a = create(&mut h);
    assert!(
        h.native(
            "SetSoundParamsFromLabel",
            &[Value::Handle(a), Value::Str("Voice".into())]
        )
        .is_err()
    );
    assert!(
        ObjectHost::checked(enums(COMMON, ""))
            .unwrap()
            .with_sound_parameters(bank())
            .is_err()
    );
    assert!(
        h.with_sound_parameters(bank())
            .unwrap()
            .with_sound_parameters(bank())
            .is_err()
    );
    for c in [
        COMMON.replace(
            "native SetSoundParamsFromLabel",
            "constant native SetSoundParamsFromLabel",
        ),
        COMMON.replace(
            "string label returns nothing",
            "integer label returns nothing",
        ),
        COMMON.replace("string label returns nothing", "string label returns sound"),
    ] {
        assert!(host(&c, "").with_sound_parameters(bank()).is_err());
    }
    assert!(host(COMMON,"function SetSoundParamsFromLabel takes sound s, string label returns nothing\nendfunction\n").with_sound_parameters(bank()).is_err());
}
#[test]
fn actual_vm_retains_parameter_assignment_at_a_later_failure() {
    let c = format!("{COMMON}native Next takes nothing returns nothing\n");
    let map = "globals\nsound voice=null\nendglobals\nfunction main takes nothing returns nothing\nset voice=CreateSound(\"existing.wav\",true,false,true,-7,91,\"CreationEAX\")\ncall SetSoundParamsFromLabel(voice,\"Voice\")\ncall StartSound(voice)\ncall Next()\nendfunction\n";
    let mut vm = host(&c, map)
        .with_sound_parameters(bank())
        .unwrap()
        .with_sound_starts(2)
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
        panic!("sound")
    };
    assert_eq!(
        vm.host()
            .sound_parameters(id)
            .unwrap()
            .unwrap()
            .values
            .volume,
        83
    );
    assert_eq!(vm.host().pending_sound_starts(), [id]);
    let count = vm.host().inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner().handles(), count);
}
#[test]
fn label_and_direct_objects_use_the_same_parameter_bank_without_replacing_pcm() {
    let mut r = rows();
    r[1][11] = "Media";
    r[1][12] = "single.wav";
    let table = table(&r);
    let pcm = pcm();
    let label_bank = crate::label_sounds::SoundBank::read(
        ScriptAsset {
            label: "synthetic:creation-table",
            bytes: &table,
        },
        origin(),
        "Selector",
        &[crate::label_sounds::SoundInput {
            label: "Voice",
            member: r"Media\single.wav",
            asset: ScriptAsset {
                label: "synthetic:label-PCM",
                bytes: &pcm,
            },
            origin: origin(),
        }],
        crate::label_sounds::Budget {
            table_bytes: 8192,
            coordinates: CoordinateLimits {
                max_row: 100,
                max_column: 32,
            },
            sounds: 1,
            resource_bytes: 1024,
            total_resource_bytes: 1024,
            metadata_bytes: 1024,
        },
    )
    .unwrap();
    let mut h = host(COMMON, "")
        .with_label_sounds(label_bank)
        .unwrap()
        .with_sound_parameters(bank())
        .unwrap()
        .with_sound_duration()
        .unwrap();
    let Value::Handle(id) = h
        .native(
            "CreateMIDISound",
            &[Value::Str("Voice".into()), Value::Int(6), Value::Int(9)],
        )
        .unwrap()
    else {
        panic!("sound")
    };
    let direct = create(&mut h);
    for i in [id, direct] {
        h.native("SetSoundDuration", &[Value::Handle(i), Value::Int(673)])
            .unwrap();
        h.native(
            "SetSoundParamsFromLabel",
            &[Value::Handle(i), Value::Str("Other".into())],
        )
        .unwrap();
        assert_eq!(h.sound_parameters(i).unwrap().unwrap().values.volume, -14);
        assert_eq!(h.sound_duration(i).unwrap(), Some(673));
    }
    let s = h.sound_state(id).unwrap();
    assert_eq!(
        (
            s.resource.label(),
            s.resource.member(),
            s.fade_in_rate,
            s.fade_out_rate
        ),
        ("Voice", r"Media\single.wav", 6, 9)
    );
    assert_eq!(
        h.file_sound_state(direct).unwrap().resource.member(),
        "existing.wav"
    );
    let wide = pcx_script::HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    assert!(h.sound_parameters(wide).is_err());
    assert!(
        h.native(
            "SetSoundParamsFromLabel",
            &[Value::Handle(wide), Value::Str("Other".into())]
        )
        .is_err()
    );
}

fn banks_budget() -> BanksBudget {
    BanksBudget {
        banks: 4,
        table_bytes: 8192,
        total_table_bytes: 32768,
        coordinates: CoordinateLimits {
            max_row: 100,
            max_column: 32,
        },
        definitions: 8,
        metadata_bytes: 2048,
    }
}
fn input<'a>(
    data: &'a [u8],
    source: &'a str,
    origin: &'a AssetIdentity,
    labels: &'a [&'a str],
    profile: Profile,
) -> Input<'a> {
    Input {
        asset: ScriptAsset {
            label: source,
            bytes: data,
        },
        origin,
        label_column: "Selector",
        labels,
        profile,
    }
}
fn dialogue_rows() -> Vec<Vec<&'static str>> {
    vec![
        vec![
            "Selector",
            "DirectoryBase",
            "FileNames",
            "Volume",
            "Flags",
            "version",
        ],
        vec![
            "Speech",
            "Voices\\",
            "a.mp3,b.mp3",
            "101",
            "OPAQUE,DIALOGUE",
            "-9",
        ],
    ]
}
fn mixed_banks() -> Banks {
    let full = table(&rows());
    let dialogue = table(&dialogue_rows());
    let o = origin();
    Banks::read(
        &[
            input(&full, "synthetic:full", &o, &["Voice"], Profile::Full),
            input(
                &dialogue,
                "synthetic:dialogue",
                &o,
                &["Speech"],
                Profile::Dialogue,
            ),
        ],
        banks_budget(),
    )
    .unwrap()
}
#[test]
fn multi_bank_rows_and_lookup_columns_preserve_the_actual_assignment_source() {
    let first = table(&rows());
    let mut r = rows();
    r[1][0] = "Second";
    r[1][1] = "211";
    r[0].push("UnrelatedLookup");
    r[1].push("Voice");
    r[2].push("Voice");
    let second = table(&r);
    let mut first_origin = origin();
    first_origin.source = "synthetic:first".into();
    let mut second_origin = origin();
    second_origin.source = "synthetic:second".into();
    let banks = Banks::read(
        &[
            input(
                &first,
                "synthetic:first-table",
                &first_origin,
                &["Voice"],
                Profile::Full,
            ),
            input(
                &second,
                "synthetic:second-table",
                &second_origin,
                &["Second"],
                Profile::Full,
            ),
        ],
        banks_budget(),
    )
    .unwrap();
    let mut h = host(COMMON, "").with_sound_parameter_banks(banks).unwrap();
    assert!(h.sound_parameter_bank().is_none());
    assert_eq!(h.sound_parameter_banks().unwrap().sources().count(), 2);
    let id = create(&mut h);
    let resource = h.sound_resource(id).unwrap() as *const _;
    for (label, volume, source, sha, origin) in [
        (
            "Voice",
            83,
            "synthetic:first-table",
            sha256(&first),
            &first_origin,
        ),
        (
            "Second",
            211,
            "synthetic:second-table",
            sha256(&second),
            &second_origin,
        ),
        (
            "Voice",
            83,
            "synthetic:first-table",
            sha256(&first),
            &first_origin,
        ),
    ] {
        h.native(
            "SetSoundParamsFromLabel",
            &[Value::Handle(id), Value::Str(label.into())],
        )
        .unwrap();
        let assignment = h.sound_parameter_assignment(id).unwrap().unwrap();
        assert_eq!(assignment.source.source_label, source);
        assert_eq!(assignment.source.label_column, "Selector");
        assert_eq!(assignment.source.sha256, sha);
        assert_eq!(assignment.source.origin, origin);
        let ParameterValues::Full(p) = assignment.parameters else {
            panic!("full")
        };
        assert_eq!((p.row, p.values.volume), (2, volume));
        assert_eq!(
            assignment
                .source
                .table
                .by_name(p.row, "Volume")
                .unwrap()
                .parse::<i32>()
                .unwrap(),
            volume
        );
        assert_eq!(
            h.sound_parameters(id).unwrap().unwrap().values.volume,
            volume
        );
        assert_eq!(h.sound_resource(id).unwrap() as *const _, resource);
    }
    assert!(
        h.native(
            "SetSoundParamsFromLabel",
            &[Value::Handle(id), Value::Str("Unadmitted".into())]
        )
        .is_err()
    );
    assert_eq!(
        h.sound_parameter_assignment(id)
            .unwrap()
            .unwrap()
            .source
            .source_label,
        "synthetic:first-table"
    );
    assert_eq!(h.inner().handles(), 1);
}
#[test]
fn aggregate_preflight_and_cross_bank_duplicates_precede_all_table_parsing() {
    let o = origin();
    let inputs = [
        input(b"bad-a", "first", &o, &["A"], Profile::Full),
        input(b"bad-b", "second", &o, &["B"], Profile::Dialogue),
    ];
    for which in 0..8 {
        let mut b = banks_budget();
        match which {
            0 => b.banks = 1,
            1 => b.definitions = 1,
            2 => b.total_table_bytes = 5,
            3 => {
                b.metadata_bytes =
                    preflight(inputs[0].asset, &o, "Selector", &["A"], budget()).unwrap()
            }
            4 => b.banks = usize::MAX,
            5 => b.definitions = usize::MAX,
            6 => b.total_table_bytes = usize::MAX,
            _ => b.metadata_bytes = usize::MAX,
        };
        assert!(matches!(
            Banks::read(&inputs, b).unwrap_err().problem,
            Problem::Budget
        ));
    }
    let duplicates = [
        input(b"invalid-a", "first", &o, &["Same"], Profile::Full),
        input(b"invalid-b", "second", &o, &["Same"], Profile::Dialogue),
    ];
    assert!(matches!(
        Banks::read(&duplicates, banks_budget())
            .unwrap_err()
            .problem,
        Problem::Ambiguous
    ));
    let d = table(&rows());
    let equal = [
        input(&d, "one", &o, &["Voice"], Profile::Full),
        input(&d, "two", &o, &["Voice"], Profile::Full),
    ];
    assert!(matches!(
        Banks::read(&equal, banks_budget()).unwrap_err().problem,
        Problem::Ambiguous
    ));
}
#[test]
fn dialogue_has_only_sourced_values_and_full_getter_refuses_its_typed_profile() {
    let mut h = host(COMMON, "")
        .with_sound_parameter_banks(mixed_banks())
        .unwrap();
    let id = create(&mut h);
    let untouched = create(&mut h);
    assert!(h.sound_parameter_assignment(id).unwrap().is_none());
    assert!(h.sound_parameters(id).unwrap().is_none());
    let resource = h.sound_resource(id).unwrap() as *const _;
    h.native(
        "SetSoundParamsFromLabel",
        &[Value::Handle(id), Value::Str("Speech".into())],
    )
    .unwrap();
    let assignment = h.sound_parameter_assignment(id).unwrap().unwrap();
    assert_eq!(assignment.source.profile, Profile::Dialogue);
    assert_eq!(assignment.source.source_label, "synthetic:dialogue");
    for field in [
        "Pitch",
        "PitchVariance",
        "Priority",
        "Channel",
        "MinDistance",
        "MaxDistance",
        "DistanceCutoff",
        "EAXFlags",
    ] {
        assert!(assignment.source.table.column_of(field).is_none());
    }
    let ParameterValues::Dialogue(p) = assignment.parameters else {
        panic!("dialogue")
    };
    assert_eq!(
        *p.values,
        DialogueValues {
            volume: 101,
            version: -9
        }
    );
    assert_eq!(
        (p.row, p.flags, p.directory_base, p.file_names),
        (2, "OPAQUE,DIALOGUE", "Voices\\", "a.mp3,b.mp3")
    );
    assert!(matches!(
        h.sound_parameters(id).unwrap_err().failure,
        crate::playercolor_host::Failure::UnsupportedSoundParameterProfile(Profile::Dialogue)
    ));
    assert!(h.sound_parameter_bank().is_none());
    assert!(h.sound_parameter_assignment(untouched).unwrap().is_none());
    assert_eq!(h.sound_resource(id).unwrap() as *const _, resource);
    let before = h.file_sound_state(id).unwrap();
    assert_eq!(
        (
            before.looping,
            before.is_3d,
            before.stop_when_out_of_range,
            before.fade_in_rate,
            before.fade_out_rate,
            before.eax_setting
        ),
        (true, false, true, -7, 91, "CreationEAX")
    );
    assert!(
        h.native(
            "SetSoundParamsFromLabel",
            &[Value::Handle(id), Value::Str("Missing".into())]
        )
        .is_err()
    );
    assert!(matches!(
        h.sound_parameter_assignment(id)
            .unwrap()
            .unwrap()
            .parameters,
        ParameterValues::Dialogue(_)
    ));
    h.native(
        "SetSoundParamsFromLabel",
        &[Value::Handle(id), Value::Str("Voice".into())],
    )
    .unwrap();
    assert_eq!(h.sound_parameters(id).unwrap().unwrap().label, "Voice");
}
#[test]
fn dialogue_schema_refuses_missing_or_invalid_numbers_without_coercing_opaque_text() {
    let o = origin();
    for (column, bad) in [(3, "_"), (3, ""), (3, "2147483648"), (5, "NaN"), (5, "1.5")] {
        let mut r = dialogue_rows();
        r[1][column] = bad;
        let d = table(&r);
        assert!(matches!(
            Banks::read(
                &[input(&d, "dialogue", &o, &["Speech"], Profile::Dialogue)],
                banks_budget()
            )
            .unwrap_err()
            .problem,
            Problem::Parameter { row: 2, .. }
        ));
    }
    for flags in ["_", ""] {
        let mut r = dialogue_rows();
        r[1][4] = flags;
        let d = table(&r);
        let b = Banks::read(
            &[input(
                &d,
                "opaque-dialogue",
                &o,
                &["Speech"],
                Profile::Dialogue,
            )],
            banks_budget(),
        )
        .unwrap();
        let a = b.get(b.find("Speech").unwrap()).unwrap();
        let ParameterValues::Dialogue(p) = a.parameters else {
            panic!("dialogue")
        };
        assert_eq!(p.flags, flags);
    }
    let mut r = dialogue_rows();
    r[1].pop();
    let d = table(&r);
    assert!(matches!(
        Banks::read(
            &[input(&d, "absent-cell", &o, &["Speech"], Profile::Dialogue)],
            banks_budget()
        )
        .unwrap_err()
        .problem,
        Problem::Parameter {
            field: "version",
            ..
        }
    ));
    let mut r = dialogue_rows();
    r[0].pop();
    r[1].pop();
    let d = table(&r);
    assert!(matches!(
        Banks::read(
            &[input(
                &d,
                "absent-header",
                &o,
                &["Speech"],
                Profile::Dialogue
            )],
            banks_budget()
        )
        .unwrap_err()
        .problem,
        Problem::Ambiguous
    ));
    let mut r = dialogue_rows();
    r[0].push("Pitch");
    r[1].push("_");
    let d = table(&r);
    assert!(matches!(
        Banks::read(
            &[input(
                &d,
                "mixed-schema",
                &o,
                &["Speech"],
                Profile::Dialogue
            )],
            banks_budget()
        )
        .unwrap_err()
        .problem,
        Problem::Parameter {
            row: 1,
            field: "Pitch"
        }
    ));
    let full = table(&rows());
    assert!(matches!(
        Banks::read(
            &[input(
                &full,
                "full-is-not-dialogue",
                &o,
                &["Voice"],
                Profile::Dialogue
            )],
            banks_budget()
        )
        .unwrap_err()
        .problem,
        Problem::Ambiguous | Problem::Parameter { .. }
    ));
    let d = table(&dialogue_rows());
    assert!(
        Banks::read(
            &[input(
                &d,
                "dialogue-is-not-full",
                &o,
                &["Speech"],
                Profile::Full
            )],
            banks_budget()
        )
        .is_err()
    );
}
#[test]
fn late_bad_bank_cannot_install_or_change_a_prior_assignment() {
    let mut h = host(COMMON, "").with_sound_parameters(bank()).unwrap();
    let id = create(&mut h);
    h.native(
        "SetSoundParamsFromLabel",
        &[Value::Handle(id), Value::Str("Other".into())],
    )
    .unwrap();
    let o = origin();
    let first = table(&rows());
    let mut r = dialogue_rows();
    r[1][3] = "not-an-integer";
    let late = table(&r);
    assert!(matches!(
        Banks::read(
            &[
                input(&first, "valid-first", &o, &["Voice"], Profile::Full),
                input(&late, "bad-late", &o, &["Speech"], Profile::Dialogue)
            ],
            banks_budget()
        )
        .unwrap_err()
        .problem,
        Problem::Parameter {
            field: "Volume",
            ..
        }
    ));
    assert_eq!(h.sound_parameters(id).unwrap().unwrap().label, "Other");
    assert_eq!(
        h.sound_parameter_bank().unwrap().source_label(),
        "synthetic:slk"
    );
    assert_eq!(h.inner().handles(), 1);
    assert!(h.with_sound_parameter_banks(mixed_banks()).is_err());
}
#[test]
fn actual_vm_keeps_dialogue_receipt_and_typed_full_refusal_at_next_native_failure() {
    let c = format!("{COMMON}native Next takes nothing returns nothing\n");
    let map = "globals\nsound speech=null\nendglobals\nfunction main takes nothing returns nothing\nset speech=CreateSound(\"existing.wav\",false,false,false,3,5,\"Creation\")\ncall SetSoundParamsFromLabel(speech,\"Speech\")\ncall Next()\nendfunction\n";
    let mut vm = host(&c, map)
        .with_sound_parameter_banks(mixed_banks())
        .unwrap()
        .into_vm();
    vm.init_globals().unwrap();
    assert!(
        vm.call("main", &[])
            .unwrap_err()
            .to_string()
            .contains("Next")
    );
    let Value::Handle(id) = vm.global("speech") else {
        panic!("sound")
    };
    assert!(matches!(
        vm.host().sound_parameters(id).unwrap_err().failure,
        crate::playercolor_host::Failure::UnsupportedSoundParameterProfile(Profile::Dialogue)
    ));
    let a = vm.host().sound_parameter_assignment(id).unwrap().unwrap();
    assert_eq!(a.source.source_label, "synthetic:dialogue");
    let ParameterValues::Dialogue(p) = a.parameters else {
        panic!("dialogue")
    };
    assert_eq!(p.values.volume, 101);
    let handles = vm.host().inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner().handles(), handles);
}
