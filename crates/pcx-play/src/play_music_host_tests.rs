use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_script::{Host, Prelude};
use std::sync::Arc;
const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative PlayMusic takes string musicName returns nothing\nnative PlayMusicEx takes string musicName, integer frommsecs, integer fadeinmsecs returns nothing\nnative StopMusic takes boolean fadeOut returns nothing\nnative ResumeMusic takes nothing returns nothing\nnative PlayThematicMusicEx takes string musicFileName, integer frommsecs returns nothing\nnative SetMusicVolume takes integer volume returns nothing\nnative SetMusicPlayPosition takes integer millisecs returns nothing\nnative SetThematicMusicPlayPosition takes integer millisecs returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
fn host(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:play-music", &common)]).unwrap(),
    );
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "synthetic:play-music",
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 8,
                admission_steps: 200,
                expression_depth: 16,
                expression_work: 200,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
fn refused(name: &str, args: &[Value]) -> String {
    host(COMMON, "")
        .with_play_music(64)
        .unwrap()
        .native(name, args)
        .unwrap_err()
}
#[test]
fn unadmitted_music_natives_stay_unsupported() {
    let mut h = host(COMMON, "");
    assert!(h.play_music_request().is_none());
    assert!(h.stop_music_fade_out().is_none());
    assert!(h.resume_music_requests().is_none());
    assert!(h.music_volume_request().is_none());
    assert!(h.music_play_position_request().is_none());
    assert!(h.thematic_music_ex_request().is_none());
    assert!(h.thematic_play_position_request().is_none());
    for (name, args) in [
        ("PlayMusic", vec![Value::Str("Music".into())]),
        (
            "PlayMusicEx",
            vec![Value::Str("Music".into()), Value::Int(11), Value::Int(22)],
        ),
        ("StopMusic", vec![Value::Bool(true)]),
        ("ResumeMusic", vec![]),
        (
            "PlayThematicMusicEx",
            vec![Value::Str("Theme.mp3".into()), Value::Int(7)],
        ),
        ("SetMusicVolume", vec![Value::Int(80)]),
        ("SetMusicPlayPosition", vec![Value::Int(1234)]),
        ("SetThematicMusicPlayPosition", vec![Value::Int(9)]),
    ] {
        assert!(
            h.native(name, &args)
                .unwrap_err()
                .contains("unsupported by this host"),
            "{name} must stay unsupported before admission"
        );
    }
}
#[test]
fn admission_ok_with_exact_declarations_then_retains_and_refuses_loudly() {
    let mut h = host(COMMON, "").with_play_music(64).unwrap();
    assert!(h.play_music_request().is_none());
    assert!(h.resume_music_requests().is_some_and(|count| count == 0));
    let err = h
        .native("PlayMusic", &[Value::Str("Music".into())])
        .unwrap_err();
    assert!(err.contains("PlayMusic"), "unexpected refusal: {err}");
    assert!(err.contains("unqualified"), "unexpected refusal: {err}");
    assert!(
        !err.contains("unsupported by this host"),
        "unexpected refusal: {err}"
    );
    assert_eq!(
        h.play_music_request(),
        Some(&PlayMusicRequest {
            name: "Music".into(),
            from_msecs: None,
            fadein_msecs: None
        })
    );
    let err = h
        .native(
            "PlayMusicEx",
            &[Value::Str("Other".into()), Value::Int(11), Value::Int(22)],
        )
        .unwrap_err();
    assert!(err.contains("PlayMusicEx"), "unexpected refusal: {err}");
    assert!(err.contains("unqualified"), "unexpected refusal: {err}");
    assert_eq!(
        h.play_music_request(),
        Some(&PlayMusicRequest {
            name: "Other".into(),
            from_msecs: Some(11),
            fadein_msecs: Some(22)
        })
    );
    for (name, args) in [
        ("StopMusic", vec![Value::Bool(true)]),
        ("ResumeMusic", vec![]),
        (
            "PlayThematicMusicEx",
            vec![Value::Str("Theme.mp3".into()), Value::Int(7)],
        ),
        ("SetMusicVolume", vec![Value::Int(80)]),
        ("SetMusicPlayPosition", vec![Value::Int(1234)]),
        ("SetThematicMusicPlayPosition", vec![Value::Int(9)]),
    ] {
        let err = h.native(name, &args).unwrap_err();
        assert!(err.contains(name), "unexpected refusal: {err}");
        assert!(err.contains("unqualified"), "unexpected refusal: {err}");
    }
    assert_eq!(h.stop_music_fade_out(), Some(true));
    assert_eq!(h.resume_music_requests(), Some(1));
    assert_eq!(h.music_volume_request(), Some(80));
    assert_eq!(h.music_play_position_request(), Some(1234));
    assert_eq!(
        h.thematic_music_ex_request(),
        Some(&ThematicMusicExRequest {
            path: "Theme.mp3".into(),
            from_msecs: 7
        })
    );
    assert_eq!(h.thematic_play_position_request(), Some(9));
    assert_eq!(h.inner().handles(), 0);
}
#[test]
fn wrong_arguments_fail_atomically_without_retaining() {
    let mut h = host(COMMON, "").with_play_music(64).unwrap();
    for (name, args) in [
        ("PlayMusic", vec![]),
        ("PlayMusic", vec![Value::Null]),
        ("PlayMusic", vec![Value::Str("Bad".into()), Value::Int(1)]),
        ("PlayMusicEx", vec![Value::Str("Bad".into()), Value::Int(1)]),
        (
            "PlayMusicEx",
            vec![Value::Str("Bad".into()), Value::Int(1), Value::Real(2.0)],
        ),
        ("StopMusic", vec![]),
        ("StopMusic", vec![Value::Int(1)]),
        ("ResumeMusic", vec![Value::Int(1)]),
        ("PlayThematicMusicEx", vec![Value::Str("Bad".into())]),
        ("SetMusicVolume", vec![]),
        ("SetMusicVolume", vec![Value::Str("Bad".into())]),
        ("SetMusicPlayPosition", vec![Value::Real(1.0)]),
        ("SetThematicMusicPlayPosition", vec![Value::Null]),
    ] {
        let err = h.native(name, &args).unwrap_err();
        assert!(err.contains(name), "unexpected error: {err}");
        assert!(
            !err.contains("unqualified"),
            "argument errors must precede the playback refusal: {err}"
        );
    }
    assert!(h.play_music_request().is_none());
    assert!(h.stop_music_fade_out().is_none());
    assert_eq!(h.resume_music_requests(), Some(0));
    assert!(h.music_volume_request().is_none());
    assert!(h.music_play_position_request().is_none());
    assert!(h.thematic_music_ex_request().is_none());
    assert!(h.thematic_play_position_request().is_none());
}
#[test]
fn name_budget_nuls_and_empty_names() {
    let mut h = host(COMMON, "").with_play_music(4).unwrap();
    assert!(refused("PlayMusic", &[Value::Str("Good".into())]).contains("unqualified"));
    assert_eq!(
        host(COMMON, "")
            .with_play_music(4)
            .unwrap()
            .play_music_request(),
        None,
        "a fresh host retains nothing"
    );
    for args in [
        vec![Value::Str("12345".into()), Value::Int(1), Value::Int(2)],
        vec![Value::Str("A\0B".into()), Value::Int(1), Value::Int(2)],
    ] {
        let err = h.native("PlayMusicEx", &args).unwrap_err();
        assert!(err.contains("PlayMusicEx"), "unexpected error: {err}");
    }
    assert!(
        h.native(
            "PlayMusicEx",
            &[Value::Str("".into()), Value::Int(1), Value::Int(2)]
        )
        .unwrap_err()
        .contains("unqualified")
    );
    assert_eq!(
        h.play_music_request(),
        Some(&PlayMusicRequest {
            name: "".into(),
            from_msecs: Some(1),
            fadein_msecs: Some(2)
        })
    );
    let mut zero = host(COMMON, "").with_play_music(0).unwrap();
    assert!(
        zero.native("PlayMusic", &[Value::Str("A".into())])
            .unwrap_err()
            .contains("PlayMusic")
    );
    assert!(zero.play_music_request().is_none());
    assert!(
        zero.native("PlayMusic", &[Value::Str("".into())])
            .unwrap_err()
            .contains("unqualified")
    );
}
#[test]
fn admission_refuses_duplicates_overflow_signature_and_source() {
    assert!(
        host(COMMON, "")
            .with_play_music(8)
            .unwrap()
            .with_play_music(16)
            .is_err()
    );
    assert!(
        host(COMMON, "")
            .with_play_music((isize::MAX as usize) + 1)
            .is_err()
    );
    for bad in [
        COMMON.replace("native PlayMusic takes", "constant native PlayMusic takes"),
        COMMON.replace("takes boolean fadeOut", "takes integer fadeOut"),
        COMMON.replace(
            "native ResumeMusic takes nothing returns nothing",
            "native ResumeMusic takes nothing returns boolean",
        ),
        COMMON.replace(", integer fadeinmsecs returns nothing", " returns nothing"),
        COMMON.replace(
            "native StopMusic takes boolean fadeOut returns nothing\n",
            "",
        ),
    ] {
        assert!(
            matches!(host(&bad, "").with_play_music(8), Err(e) if e.failure == Failure::Signature),
            "bad declaration must fail the contract"
        );
    }
    assert!(
        host(
            COMMON,
            "function PlayMusic takes string musicName returns nothing\nendfunction\n"
        )
        .with_play_music(8)
        .is_err()
    );
    let common = COMMON.replace(
        "native PlayMusicEx takes string musicName, integer frommsecs, integer fadeinmsecs returns nothing\n",
        "",
    );
    let map = "native PlayMusicEx takes string musicName, integer frommsecs, integer fadeinmsecs returns nothing\n";
    assert!(
        matches!(host(&common, map).with_play_music(8), Err(e) if e.failure == Failure::Source)
    );
}
#[test]
fn skips_admission_silently_when_no_music_natives_declared() {
    const NO_MUSIC: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";
    let mut h = host(NO_MUSIC, "");
    assert!(h.play_music_request().is_none());
    assert!(h.stop_music_fade_out().is_none());
    assert!(h.resume_music_requests().is_none());
    assert!(h.music_volume_request().is_none());
    assert!(h.music_play_position_request().is_none());
    assert!(h.thematic_music_ex_request().is_none());
    assert!(h.thematic_play_position_request().is_none());
    assert!(
        h.native("PlayMusic", &[Value::Str("Music".into())])
            .unwrap_err()
            .contains("unsupported by this host"),
        "undeclared music natives must stay unsupported before admission"
    );
    let mut h = h.with_play_music(64).unwrap();
    assert!(h.play_music_request().is_none());
    assert!(h.stop_music_fade_out().is_none());
    assert!(h.resume_music_requests().is_some_and(|count| count == 0));
    assert!(h.music_volume_request().is_none());
    assert!(h.music_play_position_request().is_none());
    assert!(h.thematic_music_ex_request().is_none());
    assert!(h.thematic_play_position_request().is_none());
    // The skip installs the same dispatch surface as full admission: the call
    // is retained and refused as unqualified playback, not as unsupported.
    let err = h
        .native("PlayMusic", &[Value::Str("Music".into())])
        .unwrap_err();
    assert!(err.contains("PlayMusic"), "unexpected refusal: {err}");
    assert!(err.contains("unqualified"), "unexpected refusal: {err}");
    assert!(
        !err.contains("unsupported by this host"),
        "unexpected refusal: {err}"
    );
}
#[test]
fn missing_create_timer_with_music_natives_is_a_source_failure() {
    // `ObjectHost::checked` already requires `CreateTimer`, so a prelude
    // without it can only reach admission with the timer declared elsewhere;
    // the foreign source still fails the single-source playback surface.
    let common = COMMON.replace("native CreateTimer takes nothing returns timer\n", "");
    let map = "native CreateTimer takes nothing returns timer\n";
    assert!(
        matches!(host(&common, map).with_play_music(8), Err(e) if e.failure == Failure::Source)
    );
}

#[test]
fn thematic_public_vm_calls_interleave_without_accepting_regular_music() {
    let common = COMMON.replacen("globals", "native SetMapMusic takes string name, boolean random, integer index returns nothing\nnative PlayThematicMusic takes string path returns nothing\nnative EndThematicMusic takes nothing returns nothing\nglobals", 1);
    let map = "function main takes nothing returns nothing\ncall PlayThematicMusicEx(\"theme.mp3\",9580)\ncall PlayThematicMusic(\"theme.mp3\")\ncall SetThematicMusicPlayPosition(2401)\ncall EndThematicMusic()\ncall SetThematicMusicPlayPosition(-200)\ncall PlayThematicMusicEx(\"theme.mp3\",3007)\nendfunction\n";
    let mut vm = host(&common, map)
        .with_map_music_selection(64)
        .unwrap()
        .with_play_music(64)
        .unwrap()
        .with_thematic_music_rules(super::super::music::thematic_rules::tests::rules())
        .unwrap()
        .into_vm();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let request = vm.host().thematic_music_request().unwrap();
    assert_eq!(request.path, "theme.mp3");
    assert_eq!(
        request.position,
        Some(std::time::Duration::from_millis(3007))
    );
    assert_eq!(request.revision, 4);
    assert_eq!(request.position_revision, 4);
    for (name, args) in [
        ("SetMusicVolume", vec![Value::Int(91)]),
        ("SetMusicPlayPosition", vec![Value::Int(931)]),
        ("PlayMusic", vec![Value::Str("regular.mp3".into())]),
    ] {
        assert!(
            vm.host()
                .native(name, &args)
                .unwrap_err()
                .contains("unqualified")
        );
    }
    assert_eq!(vm.host().thematic_music_request().unwrap().revision, 4);
}

#[test]
fn thematic_shared_surface_refuses_foreign_map_music_declarations() {
    let map = "native SetMapMusic takes string name, boolean random, integer index returns nothing\nnative PlayThematicMusic takes string path returns nothing\nnative EndThematicMusic takes nothing returns nothing\n";
    let host = host(COMMON, map)
        .with_map_music_selection(64)
        .unwrap()
        .with_play_music(64)
        .unwrap();
    assert!(
        matches!(host.with_thematic_music_rules(super::super::music::thematic_rules::tests::rules()), Err(error) if error.failure == Failure::Source)
    );
}

#[test]
#[ignore = "requires selected PCX_THEMATIC_IMAGE, PCX_THEMATIC_COMMON and PCX_THEMATIC_BLIZZARD"]
fn thematic_selected_blizzard_offset_helpers_reach_the_shared_request() {
    let read = |key| {
        String::from_utf8(bounded_music_input(std::env::var_os(key).unwrap(), 4 << 20)).unwrap()
    };
    let mut common = pcx_script::parse_script(&read("PCX_THEMATIC_COMMON")).unwrap();
    let mut blizzard = pcx_script::parse_script(&read("PCX_THEMATIC_BLIZZARD")).unwrap();
    // Execute the unchanged helper bodies, without unrelated prelude startup.
    for item in &mut common.items {
        if let pcx_script::ast::Item::Globals(globals, _) = item {
            globals.retain(|global| global.ty.kind.name() == "playercolor");
        }
    }
    blizzard.items.retain(|item| matches!(item, pcx_script::ast::Item::Function(function)
        if ["PlayThematicMusicExBJ", "SetThematicMusicOffsetBJ", "EndThematicMusicBJ"].contains(&function.name.as_str())));
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("Scripts\\common.j", &common),
                Prelude::new("Scripts\\Blizzard.j", &blizzard),
            ],
        )
        .unwrap(),
    );
    let enum_host = EnumHost::checked_selected(
        program,
        "selected:thematic-helper",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 128,
            admission_steps: 20_000,
            expression_depth: 64,
            expression_work: 20_000,
        },
    )
    .unwrap();
    let image = bounded_music_input(std::env::var_os("PCX_THEMATIC_IMAGE").unwrap(), 32 << 20);
    let rules = crate::playercolor_host::ThematicMusicRules::read(&image).unwrap();
    let mut vm = ObjectHost::checked(enum_host)
        .unwrap()
        .with_map_music_selection(64)
        .unwrap()
        .with_play_music(64)
        .unwrap()
        .with_thematic_music_rules(rules)
        .unwrap()
        .into_vm();
    vm.call(
        "PlayThematicMusicExBJ",
        &[Value::Str("synthetic-theme.mp3".into()), Value::Real(3.125)],
    )
    .unwrap();
    assert_eq!(
        vm.host().thematic_music_request().unwrap().position,
        Some(std::time::Duration::from_millis(3125))
    );
    vm.call("SetThematicMusicOffsetBJ", &[Value::Real(0.875)])
        .unwrap();
    assert_eq!(
        vm.host().thematic_music_request().unwrap().position,
        Some(std::time::Duration::from_millis(875))
    );
    vm.call("EndThematicMusicBJ", &[]).unwrap();
    assert!(vm.host().thematic_music_request().is_none());
}

fn bounded_music_input(path: impl AsRef<std::path::Path>, limit: u64) -> Vec<u8> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(
        bytes.len() as u64 <= limit,
        "local music input exceeds budget"
    );
    bytes
}

#[test]
fn regular_volume_public_vm_updates_current_and_future_gain_without_changing_thematic() {
    let common = COMMON.replacen("globals", "native SetMapMusic takes string name, boolean random, integer index returns nothing\nnative PlayThematicMusic takes string path returns nothing\nnative EndThematicMusic takes nothing returns nothing\nglobals", 1);
    let mut vm = host(&common, "function main takes nothing returns nothing\ncall PlayThematicMusic(\"theme.mp3\")\ncall SetMusicVolume(3)\nendfunction\n")
        .with_map_music_selection(64).unwrap()
        .with_play_music(64).unwrap()
        .with_thematic_music_rules(super::super::music::thematic_rules::tests::rules()).unwrap()
        .with_regular_music_volume_rules(Ok(volume_rules::tests::rules())).unwrap()
        .into_vm();
    vm.init_globals().unwrap();
    let initial = vm.host().regular_music_volume_request().unwrap().clone();
    assert_eq!((initial.level, initial.revision), (2, 0));
    vm.call("main", &[]).unwrap();
    let before = vm.host().thematic_music_request().unwrap().clone();
    let updated = vm.host().regular_music_volume_request().unwrap().clone();
    assert_eq!((updated.level, updated.revision), (3, 1));
    assert_eq!(updated.rules.gain(updated.level, 1.0, 1.0).unwrap(), 0.25);
    assert_eq!(
        vm.host()
            .native("SetMusicVolume", &[Value::Int(4)])
            .unwrap(),
        Value::Null
    );
    let updated = vm.host().regular_music_volume_request().unwrap();
    assert_eq!((updated.level, updated.revision), (4, 2));
    assert_eq!(vm.host().thematic_music_request(), Some(&before));
    assert!(
        vm.host()
            .native("PlayMusic", &[Value::Str("unsupported.mp3".into())])
            .unwrap_err()
            .contains("unqualified")
    );
}

#[test]
fn regular_volume_missing_backend_and_rejected_calls_preserve_accepted_state() {
    let mut missing = host(COMMON, "")
        .with_play_music(64)
        .unwrap()
        .with_regular_music_volume_rules(Err(
            "selected backend module absent in source generation 9".into(),
        ))
        .unwrap();
    assert!(missing.regular_music_volume_request().is_none());
    assert!(
        missing
            .native("SetMusicVolume", &[Value::Int(3)])
            .unwrap_err()
            .contains("source generation 9")
    );
    assert!(missing.music_volume_request().is_none());
    let mut h = host(COMMON, "")
        .with_play_music(64)
        .unwrap()
        .with_regular_music_volume_rules(Ok(volume_rules::tests::rules()))
        .unwrap();
    h.native("SetMusicVolume", &[Value::Int(3)]).unwrap();
    for args in [
        vec![],
        vec![Value::Real(1.)],
        vec![Value::Int(-1)],
        vec![Value::Int(5)],
        vec![Value::Int(1), Value::Int(2)],
    ] {
        assert!(h.native("SetMusicVolume", &args).is_err());
        let current = h.regular_music_volume_request().unwrap();
        assert_eq!((current.level, current.revision), (3, 1));
        assert_eq!(h.music_volume_request(), Some(3));
    }
    let no_music = format!(
        "{}globals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n",
        COMMON.split("native PlayMusic takes").next().unwrap()
    );
    let mut undeclared = host(&no_music, "")
        .with_play_music(64)
        .unwrap()
        .with_regular_music_volume_rules(Ok(volume_rules::tests::rules()))
        .unwrap();
    assert!(
        undeclared
            .native("SetMusicVolume", &[Value::Int(4)])
            .is_err()
    );
    h.play_music
        .as_mut()
        .unwrap()
        .volume_request
        .as_mut()
        .unwrap()
        .revision = u64::MAX;
    assert!(h.native("SetMusicVolume", &[Value::Int(2)]).is_err());
    assert_eq!(h.regular_music_volume_request().unwrap().level, 3);
    assert_eq!(h.music_volume_request(), Some(3));
    let next = host(COMMON, "")
        .with_play_music(64)
        .unwrap()
        .with_regular_music_volume_rules(Ok(volume_rules::tests::rules()))
        .unwrap();
    assert_eq!(next.regular_music_volume_request().unwrap().level, 2);
    assert_eq!(next.regular_music_volume_request().unwrap().revision, 0);
    assert!(
        host(COMMON, "")
            .with_regular_music_volume_rules(Ok(volume_rules::tests::rules()))
            .is_err()
    );
    assert!(
        next.with_regular_music_volume_rules(Ok(volume_rules::tests::rules()))
            .is_err()
    );
}

#[test]
#[ignore = "requires PCX_THEMATIC_COMMON, PCX_THEMATIC_BLIZZARD, PCX_WAR3_EXE and PCX_MUSIC_BACKEND local selected sources"]
fn regular_volume_selected_blizzard_percent_helper_reaches_backend_qualified_state() {
    let read = |key| {
        String::from_utf8(bounded_music_input(std::env::var_os(key).unwrap(), 4 << 20)).unwrap()
    };
    let mut common = pcx_script::parse_script(&read("PCX_THEMATIC_COMMON")).unwrap();
    let mut blizzard = pcx_script::parse_script(&read("PCX_THEMATIC_BLIZZARD")).unwrap();
    for item in &mut common.items {
        if let pcx_script::ast::Item::Globals(globals, _) = item {
            globals.retain(|global| global.ty.kind.name() == "playercolor");
        }
    }
    blizzard.items.retain(|item| {
        matches!(item, pcx_script::ast::Item::Function(function)
        if ["SetMusicVolumeBJ","PercentToInt"].contains(&function.name.as_str()))
    });
    let program = Arc::new(
        pcx_script::resolve_with(
            &pcx_script::parse_script("").unwrap(),
            &[
                Prelude::new("Scripts\\common.j", &common),
                Prelude::new("Scripts\\Blizzard.j", &blizzard),
            ],
        )
        .unwrap(),
    );
    let enums = EnumHost::checked_selected(
        program,
        "selected:regular-volume",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 128,
            admission_steps: 20_000,
            expression_depth: 64,
            expression_work: 20_000,
        },
    )
    .unwrap();
    let image = bounded_music_input(std::env::var_os("PCX_WAR3_EXE").unwrap(), 32 << 20);
    let backend_path = std::env::var_os("PCX_MUSIC_BACKEND").unwrap();
    let backend = bounded_music_input(&backend_path, 4 << 20);
    let rules = volume_rules::Rules::read(
        &image,
        &backend,
        crate::scripts::AssetIdentity {
            source: backend_path.to_string_lossy().into_owned(),
            edition: "Legacy".into(),
            version: "3.0.0.0".into(),
        },
    )
    .unwrap();
    let maximum = rules.maximum_level();
    let mut vm = ObjectHost::checked(enums)
        .unwrap()
        .with_play_music(64)
        .unwrap()
        .with_regular_music_volume_rules(Ok(rules))
        .unwrap()
        .into_vm();
    for (percent, level) in [(100.0, maximum), (0.0, 0), (200.0, maximum), (-5.0, 0)] {
        vm.call("SetMusicVolumeBJ", &[Value::Real(percent)])
            .unwrap();
        assert_eq!(
            vm.host().regular_music_volume_request().unwrap().level,
            level
        );
    }
}

#[test]
fn regular_journal_vm_retains_order_and_controller_consumes_acknowledged_prefix() {
    use super::controller::{Controller, Effect, Operation};
    let rules = Arc::new(rules::Rules::fixture());
    let mut vm = host(COMMON, "function main takes nothing returns nothing\ncall PlayMusicEx(\"a.ogg\",12,0)\ncall StopMusic(false)\ncall SetMusicPlayPosition(421)\ncall ResumeMusic()\ncall StopMusic(true)\nendfunction\n")
        .with_play_music(64).unwrap()
        .with_regular_music_journal(Arc::clone(&rules), 5).unwrap()
        .into_vm();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.host().stop_music_fade_out(), Some(true));
    assert_eq!(vm.host().resume_music_requests(), Some(1));
    assert_eq!(vm.host().music_play_position_request(), Some(421));
    assert_eq!(vm.host().play_music_request().unwrap().from_msecs, Some(12));
    let commands = vm.host().regular_music_commands().unwrap().clone();
    assert_eq!(
        commands.iter().map(|c| c.sequence).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    assert_eq!(commands[2].operation, Operation::Seek(421));
    let mut controller = Controller::new(rules, 0x1234);
    let mut effects = Vec::new();
    for command in &commands {
        let resolved =
            matches!(command.operation, Operation::Play { .. }).then(|| vec!["a.ogg".into()]);
        effects.extend(
            controller
                .apply(command.operation.clone(), resolved, 100, &mut 100)
                .unwrap(),
        );
        vm.host()
            .acknowledge_regular_music(command.sequence)
            .unwrap();
    }
    assert!(vm.host().regular_music_commands().unwrap().is_empty());
    let voice = controller.regular().unwrap();
    assert_eq!(voice.start_ms, 421);
    assert!(voice.playing);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Start(_)))
            .count(),
        1
    );
    assert!(effects.contains(&Effect::Pause(voice.serial)));
    assert!(effects.contains(&Effect::Resume(voice.serial)));
    assert!(controller.pump(107, &mut 100).unwrap().is_empty());
    controller.pump(145, &mut 100).unwrap();
    assert!(!controller.regular().unwrap().playing);
}

#[test]
fn regular_journal_full_queue_refuses_without_losing_or_consuming_commands() {
    let mut h = host(COMMON, "")
        .with_play_music(64)
        .unwrap()
        .with_regular_music_journal(Arc::new(rules::Rules::fixture()), 1)
        .unwrap();
    h.native("StopMusic", &[Value::Bool(false)]).unwrap();
    assert!(
        h.native("ResumeMusic", &[])
            .unwrap_err()
            .contains("ordered music commands")
    );
    assert_eq!(h.regular_music_commands().unwrap().len(), 1);
    assert!(h.acknowledge_regular_music(2).is_err());
    assert_eq!(h.regular_music_commands().unwrap()[0].sequence, 1);
    h.acknowledge_regular_music(1).unwrap();
    h.acknowledge_regular_music(1).unwrap();
    h.native("ResumeMusic", &[]).unwrap();
    assert_eq!(h.regular_music_commands().unwrap()[0].sequence, 2);
    assert!(h.acknowledge_regular_music(0).is_err());
    assert_eq!(h.regular_music_commands().unwrap()[0].sequence, 2);
}

#[test]
fn regular_journal_invalid_commands_preserve_queue_and_plain_defaults_are_source_bound() {
    let mut h = host(COMMON, "")
        .with_play_music(64)
        .unwrap()
        .with_regular_music_journal(Arc::new(rules::Rules::fixture()), 8)
        .unwrap();
    for args in [
        vec![Value::Str("a".into()), Value::Int(-1), Value::Int(0)],
        vec![Value::Str("a".into()), Value::Int(0), Value::Int(-1)],
        vec![Value::Str("a\0".into()), Value::Int(0), Value::Int(0)],
        vec![
            Value::Str("x".repeat(65).into()),
            Value::Int(0),
            Value::Int(0),
        ],
        vec![Value::Null, Value::Int(0), Value::Int(0)],
    ] {
        assert!(h.native("PlayMusicEx", &args).is_err());
        assert!(h.regular_music_commands().unwrap().is_empty());
    }
    assert!(h.native("SetMusicPlayPosition", &[Value::Int(-1)]).is_err());
    assert!(h.regular_music_commands().unwrap().is_empty());
    h.native("PlayMusic", &[Value::Str("a".into())]).unwrap();
    assert_eq!(h.regular_music_commands().unwrap()[0].sequence, 1);
    assert_eq!(
        h.regular_music_commands().unwrap()[0].operation,
        controller::Operation::Play {
            name: "a".into(),
            position_ms: 23,
            fade_ms: 23,
        }
    );
}

#[test]
fn regular_journal_failed_controller_admission_leaves_pending_prefix_for_retry() {
    use super::controller::Controller;
    let rules = Arc::new(rules::Rules::fixture());
    let mut h = host(COMMON, "")
        .with_play_music(64)
        .unwrap()
        .with_regular_music_journal(Arc::clone(&rules), 1)
        .unwrap();
    h.native(
        "PlayMusicEx",
        &[Value::Str("a".into()), Value::Int(0), Value::Int(0)],
    )
    .unwrap();
    let command = h.regular_music_commands().unwrap()[0].clone();
    let mut c = Controller::new(rules, 77);
    assert!(
        c.apply(command.operation.clone(), None, 0, &mut 100)
            .is_err()
    );
    assert!(c.regular().is_none());
    assert_eq!(h.regular_music_commands().unwrap().len(), 1);
    c.apply(command.operation, Some(vec!["a.ogg".into()]), 0, &mut 100)
        .unwrap();
    h.acknowledge_regular_music(command.sequence).unwrap();
    assert!(h.regular_music_commands().unwrap().is_empty());
    assert_eq!(c.regular().unwrap().path, "a.ogg");
}
