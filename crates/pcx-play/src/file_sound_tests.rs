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

const MPEG: &[u8] = include_bytes!("../tests/fixtures/synthetic-mp3/mono-xing.mp3");
fn mp3_budget() -> Mp3Budget {
    Mp3Budget {
        max_input_allocation_bytes: MPEG.len() + crate::mp3_frame::DECODER_RING_BYTES,
        max_decoder_steps: 512,
        max_packets: 128,
        max_frames: 200_000,
        max_samples: 400_000,
    }
}
pub(crate) fn mixed_assets() -> Assets {
    let pcm = wave();
    let mut encoded = input(MPEG);
    encoded.member = r"Media\tone.bin";
    Assets::read_encoded(
        &[input(&pcm), encoded],
        Budget {
            resources: 2,
            resource_bytes: MPEG.len().max(pcm.len()),
            total_bytes: MPEG.len() + pcm.len(),
            ..budget()
        },
        mp3_budget(),
    )
    .unwrap()
}

pub(crate) const PROPERTY_DECLARATIONS: &str =
    "native SetSoundChannel takes sound s, integer c returns nothing
native SetSoundVolume takes sound s, integer v returns nothing
native SetSoundPitch takes sound s, real p returns nothing
native SetSoundDistanceCutoff takes sound s, real c returns nothing
native SetSoundDistances takes sound s, real a, real b returns nothing
native SetSoundConeAngles takes sound s, real a, real b, integer v returns nothing
native SetSoundConeOrientation takes sound s, real x, real y, real z returns nothing
native SetSoundPosition takes sound s, real x, real y, real z returns nothing
native SetSoundVelocity takes sound s, real x, real y, real z returns nothing
";

fn property_host(map: &str) -> ObjectHost {
    host(&format!("{COMMON}{PROPERTY_DECLARATIONS}"), map, 16)
        .with_file_sounds(mixed_assets(), 64)
        .unwrap()
        .with_label_sounds(bank())
        .unwrap()
        .with_sound_properties()
        .unwrap()
}

const STACKED_DECLARATIONS: &str =
    "native RegisterStackedSound takes sound s, boolean positional, real w, real h returns nothing
native UnregisterStackedSound takes sound s, boolean positional, real w, real h returns nothing
";

fn stacked_host(map: &str, limit: usize) -> ObjectHost {
    let common = format!(
        "{COMMON}{PROPERTY_DECLARATIONS}{STACKED_DECLARATIONS}native Next takes nothing returns nothing\n"
    );
    host(&common, map, 16)
        .with_file_sounds(mixed_assets(), 64)
        .unwrap()
        .with_label_sounds(bank())
        .unwrap()
        .with_sound_properties()
        .unwrap()
        .with_stacked_sounds(limit)
        .unwrap()
}

#[test]
fn stacked_sound_requests_keep_order_duplicates_and_position_snapshots() {
    use crate::playercolor_host::StackedSoundAction;
    let mut host = stacked_host("", 4);
    let mut creation = args();
    creation[0] = Value::Str(r"Media\tone.bin".into());
    creation[2] = Value::Bool(true);
    let sound = host.native("CreateSound", &creation).unwrap();
    let Value::Handle(id) = sound else {
        panic!("sound");
    };
    host.native(
        "SetSoundPosition",
        &[sound.clone(), Value::Int(3), Value::Int(-4), Value::Int(5)],
    )
    .unwrap();
    let call = [
        sound.clone(),
        Value::Bool(true),
        Value::Real(13.5),
        Value::Int(29),
    ];
    host.native("RegisterStackedSound", &call).unwrap();
    host.native("RegisterStackedSound", &call).unwrap();
    host.native(
        "SetSoundPosition",
        &[sound.clone(), Value::Int(17), Value::Int(31), Value::Int(7)],
    )
    .unwrap();
    host.native("UnregisterStackedSound", &call).unwrap();
    let requests = host.pending_stacked_sound_requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0], requests[1]);
    assert_eq!(requests[0].action, StackedSoundAction::Register);
    assert_eq!(requests[0].sound, id);
    assert!(requests[0].by_position);
    assert_eq!(
        (requests[0].rect_width, requests[0].rect_height),
        (13.5, 29.0)
    );
    assert_eq!(requests[0].position, Some([3.0, -4.0, 5.0]));
    assert_eq!(requests[2].action, StackedSoundAction::Unregister);
    assert_eq!(requests[2].position, Some([17.0, 31.0, 7.0]));
    let issued = host.inner().handles();
    let transferred = host.take_stacked_sound_requests();
    assert_eq!(transferred.len(), 3);
    assert!(host.pending_stacked_sound_requests().is_empty());
    assert_eq!(host.inner().handles(), issued);
    assert_eq!(host.file_mp3_state(id).unwrap().resource.bytes(), MPEG);
    host.native("RegisterStackedSound", &call).unwrap();
    assert_eq!(
        host.pending_stacked_sound_requests()[0].position,
        Some([17.0, 31.0, 7.0])
    );
}

#[test]
fn stacked_sound_limits_and_invalid_calls_leave_pending_work_unchanged() {
    let mut host = stacked_host("", 1);
    let sound = host.native("CreateSound", &args()).unwrap();
    let call = [
        sound.clone(),
        Value::Bool(false),
        Value::Int(0),
        Value::Int(7),
    ];
    let missing = host
        .native(
            "RegisterStackedSound",
            &[
                sound.clone(),
                Value::Bool(true),
                Value::Int(1),
                Value::Int(2),
            ],
        )
        .unwrap_err();
    assert!(missing.contains("explicit position"), "{missing}");
    assert!(host.pending_stacked_sound_requests().is_empty());
    let timer = host.native("CreateTimer", &[]).unwrap();
    for args in [
        vec![
            sound.clone(),
            Value::Bool(false),
            Value::Real(f32::INFINITY),
            Value::Int(2),
        ],
        vec![
            sound.clone(),
            Value::Bool(false),
            Value::Int(-1),
            Value::Int(2),
        ],
        vec![sound.clone(), Value::Int(0), Value::Int(1), Value::Int(2)],
        vec![timer, Value::Bool(false), Value::Int(1), Value::Int(2)],
        vec![
            Value::Null,
            Value::Bool(false),
            Value::Int(1),
            Value::Int(2),
        ],
    ] {
        assert!(host.native("RegisterStackedSound", &args).is_err());
        assert!(host.pending_stacked_sound_requests().is_empty());
    }
    host.native("RegisterStackedSound", &call).unwrap();
    let before = host.pending_stacked_sound_requests().to_vec();
    let error = host.native("UnregisterStackedSound", &call).unwrap_err();
    assert!(error.contains("pending stacked sound requests"), "{error}");
    assert_eq!(host.pending_stacked_sound_requests(), before);
    assert_eq!(host.pending_stacked_sound_requests()[0].position, None);
    host.take_stacked_sound_requests();
    host.native("UnregisterStackedSound", &call).unwrap();
    let mut zero = stacked_host("", 0);
    let s = zero.native("CreateSound", &args()).unwrap();
    assert!(
        zero.native(
            "RegisterStackedSound",
            &[s, Value::Bool(false), Value::Int(0), Value::Int(0)]
        )
        .is_err()
    );
    assert!(zero.pending_stacked_sound_requests().is_empty());
}

#[test]
fn stacked_sound_admission_checks_layout_resources_and_both_signatures() {
    let common = format!("{COMMON}{STACKED_DECLARATIONS}");
    assert!(host(&common, "", 8).with_stacked_sounds(1).is_err());
    assert!(
        host(&common, "", 8)
            .with_file_sounds(assets(), 64)
            .unwrap()
            .with_stacked_sounds(usize::MAX)
            .is_err()
    );
    let mut disabled = host(&common, "", 8).with_file_sounds(assets(), 64).unwrap();
    let sound = disabled.native("CreateSound", &args()).unwrap();
    assert!(
        disabled
            .native(
                "RegisterStackedSound",
                &[sound, Value::Bool(false), Value::Int(1), Value::Int(2)]
            )
            .unwrap_err()
            .contains("unsupported")
    );
    let Err(error) = stacked_host("", 1).with_stacked_sounds(1) else {
        panic!("admitted twice");
    };
    assert!(error.to_string().contains("admitted once"));
    for common in [
        common.replace(
            "native UnregisterStackedSound",
            "constant native UnregisterStackedSound",
        ),
        common.replace(
            "boolean positional, real w, real h returns nothing",
            "boolean positional, real w, real h returns sound",
        ),
    ] {
        assert!(
            host(&common, "", 8)
                .with_file_sounds(assets(), 64)
                .unwrap()
                .with_stacked_sounds(1)
                .is_err()
        );
    }
}

#[test]
fn stacked_sound_requests_execute_in_the_same_vm_without_minting_new_handles() {
    use crate::playercolor_host::StackedSoundAction;
    let script = "globals\nsound sceneSound=null\nendglobals\nfunction main takes nothing returns nothing\nset sceneSound=CreateSound(\"Media\\\\a.wav\",true,true,true,-7,91,\"AuthoredProfile\")\ncall SetSoundPosition(sceneSound,3.0,-7.5,11.0)\ncall RegisterStackedSound(sceneSound,true,19.0,27.0)\ncall UnregisterStackedSound(sceneSound,true,19.0,27.0)\ncall Next()\nendfunction\n";
    let mut vm = stacked_host(script, 2).into_vm();
    assert!(
        vm.call("main", &[])
            .unwrap_err()
            .to_string()
            .contains("Next")
    );
    let Value::Handle(id) = vm.global("sceneSound") else {
        panic!("sound");
    };
    let requests = vm.host().take_stacked_sound_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].action, StackedSoundAction::Register);
    assert_eq!(requests[1].action, StackedSoundAction::Unregister);
    assert_eq!(requests[0].sound, id);
    assert_eq!(requests[1].sound, id);
    assert_eq!(requests[0].position, Some([3.0, -7.5, 11.0]));
    assert_eq!(
        vm.host().file_sound_state(id).unwrap().resource.bytes(),
        wave()
    );
}

#[test]
fn sound_properties_preserve_pcm_and_mpeg_identity_and_raw_assignments() {
    use crate::playercolor_host::SoundProperties;
    let mut host = property_host("");
    for member in [r"Media\a.wav", r"Media\tone.bin"] {
        let mut creation = args();
        creation[0] = Value::Str(member.into());
        creation[2] = Value::Bool(true);
        let Value::Handle(id) = host.native("CreateSound", &creation).unwrap() else {
            panic!("sound handle");
        };
        assert_eq!(
            host.sound_properties(id).unwrap(),
            &SoundProperties::default()
        );
        for (name, tail) in [
            ("SetSoundChannel", vec![Value::Int(-5)]),
            ("SetSoundVolume", vec![Value::Int(233)]),
            ("SetSoundPitch", vec![Value::Real(-0.25)]),
            ("SetSoundDistanceCutoff", vec![Value::Real(937.5)]),
            ("SetSoundDistances", vec![Value::Int(19), Value::Real(81.5)]),
            (
                "SetSoundConeAngles",
                vec![Value::Real(13.5), Value::Real(95.0), Value::Int(-11)],
            ),
            (
                "SetSoundConeOrientation",
                vec![Value::Int(1), Value::Int(-2), Value::Int(3)],
            ),
            (
                "SetSoundPosition",
                vec![Value::Real(-0.0), Value::Real(f32::MAX), Value::Real(-9.25)],
            ),
            (
                "SetSoundVelocity",
                vec![Value::Real(-7.5), Value::Int(0), Value::Real(0.25)],
            ),
        ] {
            let mut call = vec![Value::Handle(id)];
            call.extend(tail);
            assert_eq!(host.native(name, &call).unwrap(), Value::Null);
        }
        assert_eq!(
            *host.sound_properties(id).unwrap(),
            SoundProperties {
                channel: Some(-5),
                volume: Some(233),
                pitch: Some(-0.25),
                distance_cutoff: Some(937.5),
                distances: Some([19.0, 81.5]),
                cone_angles: Some((13.5, 95.0, -11)),
                cone_orientation: Some([1.0, -2.0, 3.0]),
                position: Some([-0.0, f32::MAX, -9.25]),
                velocity: Some([-7.5, 0.0, 0.25]),
                attached_unit: None,
            }
        );
        assert_eq!(
            host.sound_properties(id).unwrap().position.unwrap()[0].to_bits(),
            (-0.0_f32).to_bits()
        );
        let mut changed = creation.clone();
        changed[2] = Value::Bool(false);
        let Value::Handle(other) = host.native("CreateSound", &changed).unwrap() else {
            panic!("sound");
        };
        assert_ne!(id, other);
        assert_eq!(
            *host.sound_properties(other).unwrap(),
            SoundProperties::default()
        );
        assert!(
            host.native(
                "SetSoundPosition",
                &[
                    Value::Handle(other),
                    Value::Int(0),
                    Value::Int(0),
                    Value::Int(0)
                ]
            )
            .unwrap_err()
            .contains("explicit 3D")
        );
        assert_eq!(host.inner().kind(id).unwrap(), "sound");
        if member.ends_with(".bin") {
            assert_eq!(host.file_mp3_state(id).unwrap().resource.bytes(), MPEG);
        } else {
            assert_eq!(host.file_sound_state(id).unwrap().resource.bytes(), wave());
        }
    }
}

#[test]
fn sound_property_errors_do_not_mutate_state_or_cross_handle_families() {
    let mut host = property_host("");
    let mut creation = args();
    creation[2] = Value::Bool(true);
    let sound = host.native("CreateSound", &creation).unwrap();
    let Value::Handle(id) = sound else {
        panic!("sound");
    };
    host.native(
        "SetSoundPosition",
        &[sound.clone(), Value::Int(4), Value::Int(5), Value::Int(6)],
    )
    .unwrap();
    let before = *host.sound_properties(id).unwrap();
    for (name, tail) in [
        (
            "SetSoundPosition",
            vec![Value::Int(1), Value::Real(f32::NAN), Value::Int(3)],
        ),
        (
            "SetSoundVelocity",
            vec![Value::Real(f32::INFINITY), Value::Int(1), Value::Int(0)],
        ),
        (
            "SetSoundConeAngles",
            vec![Value::Int(1), Value::Int(2), Value::Real(3.0)],
        ),
        ("SetSoundVolume", vec![Value::Real(7.0)]),
        ("SetSoundDistances", vec![Value::Int(1)]),
        ("SetSoundPitch", vec![Value::Str("not a real".into())]),
    ] {
        let mut call = vec![sound.clone()];
        call.extend(tail);
        let error = host.native(name, &call).unwrap_err();
        assert!(error.contains("synthetic:common"), "{error}");
        assert_eq!(*host.sound_properties(id).unwrap(), before);
    }
    let timer = host.native("CreateTimer", &[]).unwrap();
    assert!(
        host.native("SetSoundVolume", &[timer, Value::Int(1)])
            .unwrap_err()
            .contains("another family")
    );
    assert!(
        host.native("SetSoundPitch", &[Value::Null, Value::Int(1)])
            .is_err()
    );
    let label = host
        .native(
            "CreateMIDISound",
            &[Value::Str("Breeze".into()), Value::Int(1), Value::Int(2)],
        )
        .unwrap();
    host.native("SetSoundVolume", &[label.clone(), Value::Int(29)])
        .unwrap();
    assert!(
        host.native(
            "SetSoundPosition",
            &[label.clone(), Value::Int(1), Value::Int(2), Value::Int(3)]
        )
        .unwrap_err()
        .contains("explicit 3D creation required")
    );
    let Value::Handle(label) = label else {
        panic!("label");
    };
    assert_eq!(host.sound_properties(label).unwrap().volume, Some(29));
    assert_eq!(host.sound_properties(label).unwrap().position, None);
    assert_eq!(*host.sound_properties(id).unwrap(), before);
}

#[test]
fn sound_property_contracts_and_admission_are_checked() {
    let common = format!("{COMMON}{PROPERTY_DECLARATIONS}");
    assert!(host(&common, "", 8).with_sound_properties().is_err());
    let mut unadmitted = host(&common, "", 8).with_file_sounds(assets(), 64).unwrap();
    let sound = unadmitted.native("CreateSound", &args()).unwrap();
    assert!(
        unadmitted
            .native("SetSoundVolume", &[sound, Value::Int(31)])
            .unwrap_err()
            .contains("unsupported")
    );
    let Err(error) = property_host("").with_sound_properties() else {
        panic!("second admission");
    };
    assert!(error.to_string().contains("admitted once"));
    for bad in [
        common.replace("native SetSoundPitch", "constant native SetSoundPitch"),
        common.replace(
            "real x, real y, real z returns nothing",
            "real x, real y, real z returns sound",
        ),
        common.replace(
            "sound s, integer v returns nothing",
            "sound s, real v returns nothing",
        ),
    ] {
        assert!(
            host(&bad, "", 8)
                .with_file_sounds(assets(), 64)
                .unwrap()
                .with_sound_properties()
                .is_err()
        );
    }
}

#[test]
fn sound_properties_run_in_actual_vm_and_survive_the_next_refusal() {
    let map = "globals\nsound sceneSound=null\nendglobals\nfunction main takes nothing returns nothing\nset sceneSound=CreateSound(\"Media\\\\a.wav\",true,true,true,-7,91,\"AuthoredProfile\")\ncall SetSoundPosition(sceneSound,7.5,-3.0,11.0)\ncall SetSoundVolume(sceneSound,220)\ncall SetSoundPitch(sceneSound,0.75)\ncall Next()\nendfunction\n";
    let common =
        format!("{COMMON}{PROPERTY_DECLARATIONS}native Next takes nothing returns nothing\n");
    let mut vm = host(&common, map, 8)
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_sound_properties()
        .unwrap()
        .into_vm();
    assert!(
        vm.call("main", &[])
            .unwrap_err()
            .to_string()
            .contains("Next")
    );
    let Value::Handle(id) = vm.global("sceneSound") else {
        panic!("sound");
    };
    let properties = *vm.host().sound_properties(id).unwrap();
    assert_eq!(properties.position, Some([7.5, -3.0, 11.0]));
    assert_eq!(properties.volume, Some(220));
    assert_eq!(properties.pitch, Some(0.75));
    assert_eq!(properties.velocity, None);
    assert_eq!(
        vm.host().file_sound_state(id).unwrap().eax_setting,
        "AuthoredProfile"
    );
}

#[test]
fn encoded_bank_preserves_pcm_and_mp3_sources_without_extension_inference() {
    let legacy = Assets::read(
        &[input(MPEG)],
        Budget {
            resource_bytes: MPEG.len(),
            total_bytes: MPEG.len(),
            ..budget()
        },
    )
    .unwrap_err();
    assert!(matches!(legacy.problem, Problem::Wave(_)));
    let assets = mixed_assets();
    let [pcm] = assets.resources() else {
        panic!("one PCM")
    };
    let [mp3] = assets.mp3_resources() else {
        panic!("one MP3")
    };
    assert_eq!(pcm.bytes(), wave());
    assert_eq!(mp3.member(), r"Media\tone.bin");
    assert_eq!(mp3.bytes(), MPEG);
    assert_eq!(mp3.sha256(), sha256(MPEG));
    assert_eq!(mp3.origin(), &origin());
    assert_eq!(mp3.source_label(), input(MPEG).asset.label);
    assert_eq!(mp3.frames().channels, 1);
    assert!(mp3.frames().decoded_frames > 0);
}

#[test]
fn encoded_preflight_and_decode_limits_keep_source_context() {
    assert!(matches!(
        Assets::read_encoded(&[input(b"invalid"), input(MPEG)], budget(), mp3_budget())
            .unwrap_err()
            .problem,
        Problem::Duplicate
    ));
    let mut later = input(MPEG);
    later.member = "later";
    let error = Assets::read_encoded(
        &[input(b"invalid"), later],
        Budget {
            total_bytes: 0,
            ..budget()
        },
        mp3_budget(),
    )
    .unwrap_err();
    assert!(matches!(error.problem, Problem::Budget));
    let error = Assets::read_encoded(
        &[input(MPEG)],
        Budget {
            resource_bytes: MPEG.len(),
            total_bytes: MPEG.len(),
            ..budget()
        },
        Mp3Budget {
            max_frames: 0,
            ..mp3_budget()
        },
    )
    .unwrap_err();
    assert_eq!(error.source_context, input(MPEG).asset.label);
    assert!(matches!(
        error.problem,
        Problem::Mp3(crate::mp3_frame::DecodeError::FrameLimit { .. })
    ));
}

#[test]
fn mixed_host_keeps_creation_inputs_duration_and_start_identity() {
    let mut host = host(COMMON, "", 8)
        .with_file_sounds(mixed_assets(), 64)
        .unwrap()
        .with_sound_starts(3)
        .unwrap()
        .with_sound_duration()
        .unwrap();
    let pcm = host.native("CreateSound", &args()).unwrap();
    let mut call = args();
    call[0] = Value::Str(r"Media\tone.bin".into());
    let mp3 = host.native("CreateSound", &call).unwrap();
    let Value::Handle(id) = mp3 else {
        panic!("MP3 sound")
    };
    let Value::Handle(pcm_id) = pcm else {
        panic!("PCM sound")
    };
    assert_ne!(id, pcm_id);
    host.native("SetSoundDuration", &[Value::Handle(id), Value::Int(-11)])
        .unwrap();
    let state = host.file_mp3_state(id).unwrap();
    assert_eq!(state.resource.bytes(), MPEG);
    assert_eq!(
        (
            state.duration,
            state.looping,
            state.is_3d,
            state.stop_when_out_of_range,
            state.fade_in_rate,
            state.fade_out_rate,
            state.eax_setting
        ),
        (Some(-11), true, false, true, -7, 91, "AuthoredProfile")
    );
    assert!(host.sound_resource(id).is_err());
    assert!(host.file_sound_state(id).is_err());
    assert!(host.file_mp3_state(pcm_id).is_err());
    for handle in [pcm_id, id, id] {
        host.native("StartSound", &[Value::Handle(handle)]).unwrap();
    }
    assert_eq!(host.take_sound_starts(), [pcm_id, id, id]);
    assert_eq!(host.sound_resource(pcm_id).unwrap().bytes(), wave());
}

#[test]
#[ignore = "requires PCX_MP3_ARCHIVE, MEMBER, EDITION, VERSION and EXPECTED_SHA256"]
fn original_mp3_is_admitted_from_one_explicit_local_archive() {
    let archive_path = std::env::var("PCX_MP3_ARCHIVE").expect("PCX_MP3_ARCHIVE");
    let member = std::env::var("PCX_MP3_MEMBER").expect("PCX_MP3_MEMBER");
    let expected = std::env::var("PCX_MP3_EXPECTED_SHA256").expect("PCX_MP3_EXPECTED_SHA256");
    let origin = AssetIdentity {
        source: archive_path.clone(),
        edition: std::env::var("PCX_MP3_EDITION").expect("PCX_MP3_EDITION"),
        version: std::env::var("PCX_MP3_VERSION").expect("PCX_MP3_VERSION"),
    };
    let mut archive = pcx_mpq::MpqArchive::open(&archive_path).unwrap();
    // Test resource policy; neither packed nor decoded bytes can exceed it.
    let limit = 8 * 1024 * 1024;
    let bytes = archive
        .extract_bounded(
            &member,
            pcx_mpq::ExtractionBudget {
                max_raw_packed_bytes: limit,
                max_decoded_bytes: limit,
            },
            None,
        )
        .unwrap();
    let hash = sha256(&bytes);
    let actual: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(actual, expected);
    let files = Assets::read_encoded(
        &[SoundInput {
            member: &member,
            asset: ScriptAsset {
                label: &archive_path,
                bytes: &bytes,
            },
            origin,
        }],
        Budget {
            resources: 1,
            resource_bytes: limit,
            total_bytes: limit,
            metadata_bytes: 4096,
        },
        Mp3Budget {
            max_input_allocation_bytes: limit + crate::mp3_frame::DECODER_RING_BYTES,
            max_decoder_steps: 40_000,
            max_packets: 20_000,
            max_frames: 5_000_000,
            max_samples: 10_000_000,
        },
    )
    .unwrap();
    let report = files.mp3_resources()[0].frames();
    assert_eq!(report.encoded_sha256, hash);
    assert_eq!(files.mp3_resources()[0].bytes(), bytes);
    assert!(report.decoded_frames > 0);
    eprintln!("source={archive_path} member={member} encoded_sha256={actual} report={report:?}");
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
const COMMON: &str = "type agent extends handle\ntype sound extends agent\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateMIDISound takes string label, integer a, integer b returns sound\nnative CreateSound takes string file, boolean repeating, boolean spatial, boolean stop, integer a, integer b, string eax returns sound\nnative StartSound takes sound s returns nothing\nnative SetSoundDuration takes sound s, integer duration returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
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

#[test]
fn authored_duration_is_unset_then_retained_per_label_and_file_identity() {
    let mut h = host(COMMON, "", 8)
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_label_sounds(bank())
        .unwrap()
        .with_sound_duration()
        .unwrap();
    let Value::Handle(a) = h.native("CreateSound", &args()).unwrap() else {
        panic!("file")
    };
    let Value::Handle(b) = h
        .native(
            "CreateMIDISound",
            &[Value::Str("Breeze".into()), Value::Int(3), Value::Int(5)],
        )
        .unwrap()
    else {
        panic!("label")
    };
    assert_eq!(h.sound_duration(a).unwrap(), None);
    assert_eq!(h.sound_duration(b).unwrap(), None);
    let hash = h.sound_resource(a).unwrap().sha256();
    let count = h.inner().handles();
    for duration in [-1, 0, 539, i32::MAX] {
        assert_eq!(
            h.native(
                "SetSoundDuration",
                &[Value::Handle(a), Value::Int(duration)]
            )
            .unwrap(),
            Value::Null
        );
        assert_eq!(h.file_sound_state(a).unwrap().duration, Some(duration));
        assert_eq!(h.sound_duration(b).unwrap(), None);
        assert_eq!(h.sound_resource(a).unwrap().sha256(), hash);
        assert_eq!(h.inner().handles(), count);
    }
    h.native("SetSoundDuration", &[Value::Handle(b), Value::Int(43)])
        .unwrap();
    assert_eq!(h.sound_state(b).unwrap().duration, Some(43));
    assert_eq!(h.sound_state(b).unwrap().resource.label(), "Breeze");
    assert_eq!(h.sound_resource(a).unwrap().pcm().frames(), 7);
}
#[test]
fn duration_refuses_invalid_sound_arguments_without_changing_prior_state() {
    let mut e = enums(COMMON, "", 8);
    let fake = e.reserve_identity("sound").unwrap();
    let mut h = ObjectHost::checked(e)
        .unwrap()
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_sound_duration()
        .unwrap();
    let Value::Handle(id) = h.native("CreateSound", &args()).unwrap() else {
        panic!("sound")
    };
    h.native("SetSoundDuration", &[Value::Handle(id), Value::Int(73)])
        .unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let count = h.inner().handles();
    let wide = pcx_script::HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    for args in [
        vec![Value::Null, Value::Int(1)],
        vec![Value::Handle(id), Value::Real(1.0)],
        vec![Value::Handle(fake), Value::Int(1)],
        vec![timer, Value::Int(1)],
        vec![Value::Handle(wide), Value::Int(1)],
        vec![Value::Handle(id)],
    ] {
        assert!(h.native("SetSoundDuration", &args).is_err());
        assert_eq!(h.sound_duration(id).unwrap(), Some(73));
        assert_eq!(h.inner().handles(), count);
    }
    assert!(h.sound_duration(fake).is_err());
    assert!(h.sound_duration(wide).is_err());
}
#[test]
fn duration_requires_admission_and_actual_vm_execution_preserves_its_last_assignment() {
    assert!(host(COMMON, "", 8).with_sound_duration().is_err());
    let mut h = host(COMMON, "", 8).with_file_sounds(assets(), 64).unwrap();
    let id = h.native("CreateSound", &args()).unwrap();
    assert!(h.native("SetSoundDuration", &[id, Value::Int(71)]).is_err());
    assert!(
        h.with_sound_duration()
            .unwrap()
            .with_sound_duration()
            .is_err()
    );
    for c in [
        COMMON.replace(
            "native SetSoundDuration",
            "constant native SetSoundDuration",
        ),
        COMMON.replace(
            "integer duration returns nothing",
            "real duration returns nothing",
        ),
        COMMON.replace(
            "integer duration returns nothing",
            "integer duration returns sound",
        ),
    ] {
        assert!(
            host(&c, "", 8)
                .with_file_sounds(assets(), 64)
                .unwrap()
                .with_sound_duration()
                .is_err()
        );
    }
    assert!(
        host(
            COMMON,
            "function SetSoundDuration takes sound s, integer d returns nothing\nendfunction\n",
            8
        )
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_sound_duration()
        .is_err()
    );
    let c = format!("{COMMON}native Next takes nothing returns nothing\n");
    let map = "globals\nsound voice=null\nendglobals\nfunction main takes nothing returns nothing\nset voice=CreateSound(\"Media\\\\a.wav\",true,false,true,-7,91,\"AuthoredProfile\")\ncall SetSoundDuration(voice,98)\ncall SetSoundDuration(voice,103)\ncall Next()\nendfunction\n";
    let mut vm = host(&c, map, 8)
        .with_file_sounds(assets(), 64)
        .unwrap()
        .with_sound_duration()
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
    assert_eq!(vm.host().file_sound_state(id).unwrap().duration, Some(103));
    let count = vm.host().inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner().handles(), count);
    assert_eq!(vm.host().sound_duration(id).unwrap(), Some(103));
}

fn loaded_file(member: &str, bytes: &[u8], budget: Budget) -> Result<Assets, String> {
    let mut value = input(bytes);
    value.member = member;
    Assets::read_encoded(&[value], budget, mp3_budget()).map_err(|e| e.to_string())
}

#[test]
fn incremental_file_loading_keeps_old_resources_and_cached_reads() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut h = host(COMMON, "", 16)
        .with_file_sounds(mixed_assets(), 64)
        .unwrap();
    let old_pcm = h.file_sound_assets().unwrap().resources()[0]
        .bytes()
        .as_ptr();
    let old_mp3 = h.file_sound_assets().unwrap().mp3_resources()[0]
        .bytes()
        .as_ptr();
    let counts = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&counts);
    let limit = Budget {
        resources: 4,
        resource_bytes: 8192,
        total_bytes: 16384,
        ..budget()
    };
    h.admit_file_sound_loader(limit, move |member, remaining| {
        calls.fetch_add(1, Ordering::SeqCst);
        assert!(remaining.resources <= 2);
        let pcm = wave();
        loaded_file(
            member,
            if member.ends_with(".bin") { MPEG } else { &pcm },
            remaining,
        )
    })
    .unwrap();
    let mut ids = Vec::new();
    for member in [
        r"Media\a.wav",
        r"Media\tone.bin",
        "new.wav",
        "new.bin",
        "new.wav",
    ] {
        let mut a = args();
        a[0] = Value::Str(member.into());
        let Value::Handle(id) = h.native("CreateSound", &a).unwrap() else {
            panic!("sound")
        };
        ids.push(id);
    }
    assert_eq!(counts.load(Ordering::SeqCst), 2);
    assert_ne!(ids[2], ids[4]);
    assert_eq!(h.file_sound_state(ids[0]).unwrap().resource.bytes(), wave());
    assert_eq!(h.file_mp3_state(ids[1]).unwrap().resource.bytes(), MPEG);
    assert_eq!(
        h.file_sound_state(ids[0])
            .unwrap()
            .resource
            .bytes()
            .as_ptr(),
        old_pcm
    );
    assert_eq!(
        h.file_mp3_state(ids[1]).unwrap().resource.bytes().as_ptr(),
        old_mp3
    );
    assert_eq!(
        h.file_sound_state(ids[2])
            .unwrap()
            .resource
            .bytes()
            .as_ptr(),
        h.file_sound_state(ids[4])
            .unwrap()
            .resource
            .bytes()
            .as_ptr()
    );
    assert_eq!(
        h.file_mp3_state(ids[3]).unwrap().resource.sha256(),
        sha256(MPEG)
    );
    let mut a = args();
    a[0] = Value::Str("over.wav".into());
    assert!(h.native("CreateSound", &a).is_err());
    assert_eq!(
        counts.load(Ordering::SeqCst),
        2,
        "full bank refuses before source work"
    );
}

#[test]
fn incremental_file_loading_refuses_arguments_and_wrong_bindings_before_mutation() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut h = host(COMMON, "", 8)
        .with_file_sounds(Assets::read(&[], budget()).unwrap(), 64)
        .unwrap();
    let counts = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&counts);
    h.admit_file_sound_loader(budget(), move |member, remaining| {
        calls.fetch_add(1, Ordering::SeqCst);
        match member {
            "missing" => Err("qualified source missing".into()),
            "wrong" => loaded_file("another", &wave(), remaining),
            "many" => Ok(mixed_assets()),
            "malformed" => loaded_file(member, b"not audio", remaining),
            _ => loaded_file(member, &wave(), remaining),
        }
    })
    .unwrap();
    let before = h.inner().handles();
    for member in ["missing", "wrong", "many", "malformed"] {
        let mut a = args();
        a[0] = Value::Str(member.into());
        assert!(h.native("CreateSound", &a).is_err());
        assert!(h.file_sound_assets().unwrap().resources().is_empty());
        assert!(h.file_sound_assets().unwrap().mp3_resources().is_empty());
        assert_eq!(h.inner().handles(), before);
    }
    assert_eq!(counts.load(Ordering::SeqCst), 4);
    for index in 0..4 {
        let mut a = args();
        match index {
            0 => a[1] = Value::Int(1),
            1 => a[6] = Value::Str("\0".into()),
            2 => a[0] = Value::Str("\0".into()),
            _ => a[0] = Value::Str("x".repeat(budget().metadata_bytes + 1)),
        }
        assert!(h.native("CreateSound", &a).is_err());
    }
    assert_eq!(counts.load(Ordering::SeqCst), 4);
    assert!(h.native("CreateSound", &args()).is_ok());
    assert_eq!(h.inner().handles(), before + 1);
}

#[test]
fn incremental_file_bank_checks_combined_budgets_and_duplicate_names() {
    let pcm = wave();
    let mut next = input(&pcm);
    next.member = "second";
    let metadata = |i: &SoundInput<'_>| {
        [
            i.member,
            i.asset.label,
            &i.origin.source,
            &i.origin.edition,
            &i.origin.version,
        ]
        .iter()
        .map(|s| s.len())
        .sum::<usize>()
    };
    let exact = Budget {
        resources: 2,
        resource_bytes: pcm.len(),
        total_bytes: pcm.len() * 2,
        metadata_bytes: metadata(&input(&pcm)) + metadata(&next),
    };
    for limit in [
        Budget {
            resources: 1,
            ..exact
        },
        Budget {
            resource_bytes: pcm.len() - 1,
            ..exact
        },
        Budget {
            total_bytes: exact.total_bytes - 1,
            ..exact
        },
        Budget {
            metadata_bytes: exact.metadata_bytes - 1,
            ..exact
        },
    ] {
        let mut bank = assets();
        let pointer = bank.resources()[0].bytes().as_ptr();
        let extra = Assets::read(
            &[SoundInput {
                member: next.member,
                asset: next.asset,
                origin: next.origin.clone(),
            }],
            budget(),
        )
        .unwrap();
        assert!(bank.append(extra, limit).is_err());
        assert_eq!(bank.resources().len(), 1);
        assert_eq!(bank.resources()[0].bytes().as_ptr(), pointer);
    }
    let mut bank = assets();
    assert!(matches!(
        bank.append(assets(), budget()).unwrap_err().problem,
        Problem::Duplicate
    ));
    bank.append(loaded_file("second", &pcm, budget()).unwrap(), exact)
        .unwrap();
    let left = bank.remaining_budget(exact).unwrap();
    assert_eq!(
        (left.resources, left.total_bytes, left.metadata_bytes),
        (0, 0, 0)
    );
}

#[test]
fn incremental_file_loader_requires_admission_and_preserves_it_on_rejection() {
    let mut bare = host(COMMON, "", 8);
    assert!(
        bare.admit_file_sound_loader(budget(), |_, _| unreachable!())
            .is_err()
    );
    let mut h = bare.with_file_sounds(assets(), 64).unwrap();
    assert!(
        h.admit_file_sound_loader(
            Budget {
                metadata_bytes: (isize::MAX as usize) + 1,
                ..budget()
            },
            |_, _| unreachable!()
        )
        .is_err()
    );
    h.admit_file_sound_loader(budget(), |member, remaining| {
        loaded_file(member, &wave(), remaining)
    })
    .unwrap();
    assert!(
        h.admit_file_sound_loader(budget(), |_, _| unreachable!())
            .is_err()
    );
    assert!(h.native("CreateSound", &args()).is_ok());
    let mut a = args();
    a[0] = Value::Str("second".into());
    assert!(h.native("CreateSound", &a).is_ok());
}
