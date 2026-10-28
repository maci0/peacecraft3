use super::super::{EnumHost, Limits};
use super::*;
use pcx_objects::editor::camera::GameCamera;
use pcx_objects::editor::misc::DayCycle;
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

const COMMON: &str = "type agent extends handle
type timer extends agent
type timerdialog extends agent
type widget extends agent
type unit extends widget
type location extends agent
type sound extends agent
type fgamestate extends handle
type playercolor extends handle
type volumegroup extends handle
type camerafield extends handle
type camerasetup extends handle
constant native ConvertFGameState takes integer i returns fgamestate
constant native ConvertPlayerColor takes integer i returns playercolor
constant native ConvertVolumeGroup takes integer i returns volumegroup
constant native ConvertCameraField takes integer i returns camerafield
native CreateTimer takes nothing returns timer
native CreateTimerDialog takes timer t returns timerdialog
native DestroyTimerDialog takes timerdialog d returns nothing
native TimerDialogSetTitle takes timerdialog d, string title returns nothing
native TimerDialogSetTitleColor takes timerdialog d, integer r, integer g, integer b, integer a returns nothing
native TimerDialogSetTimeColor takes timerdialog d, integer r, integer g, integer b, integer a returns nothing
native TimerDialogSetSpeed takes timerdialog d, real speed returns nothing
native TimerDialogDisplay takes timerdialog d, boolean display returns nothing
native IsTimerDialogDisplayed takes timerdialog d returns boolean
native TimerDialogSetRealTimeRemaining takes timerdialog d, real time returns nothing
native DestroyTimer takes timer t returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
native PauseTimer takes timer t returns nothing
native ResumeTimer takes timer t returns nothing
constant native GetFloatGameState takes fgamestate s returns real
native SetFloatGameState takes fgamestate s, real v returns nothing
native SuspendTimeOfDay takes boolean b returns nothing
native SetTimeOfDayScale takes real r returns nothing
native GetTimeOfDayScale takes nothing returns real
native SetSkyModel takes string f returns nothing
native SetTerrainFogEx takes integer style, real a, real b, real c, real r, real g, real b2 returns nothing
native ResetTerrainFog takes nothing returns nothing
native CreateSound takes string file, boolean looping, boolean is3d, boolean stop, integer a, integer b, string eax returns sound
native VolumeGroupSetVolume takes volumegroup g, real s returns nothing
native VolumeGroupReset takes nothing returns nothing
native AddIndicator takes widget w, integer r, integer g, integer b, integer a returns nothing
native PingMinimap takes real x, real y, real d returns nothing
native SetCinematicScene takes integer portrait, playercolor color, string speaker, string text, real a, real b returns nothing
native EndCinematicScene takes nothing returns nothing
native CreateCameraSetup takes nothing returns camerasetup
native CameraSetupSetField takes camerasetup s, camerafield f, real v, real d returns nothing
native CameraSetupSetDestPosition takes camerasetup s, real x, real y, real d returns nothing
native CameraSetupApplyForceDuration takes camerasetup s, boolean pan, real d returns nothing
native PanCameraToTimed takes real x, real y, real d returns nothing
native ResetToGameCamera takes real d returns nothing
constant native GetCameraTargetPositionLoc takes nothing returns location
constant native GetCameraTargetPositionX takes nothing returns real
constant native GetCameraTargetPositionY takes nothing returns real
constant native GetCameraField takes camerafield f returns real
globals
constant fgamestate CLOCK = ConvertFGameState(2)
constant fgamestate OTHER = ConvertFGameState(1)
constant playercolor RED = ConvertPlayerColor(0)
constant volumegroup MUSIC = ConvertVolumeGroup(5)
constant camerafield CAMERA_FIELD_TARGET_DISTANCE = ConvertCameraField(0)
constant camerafield CAMERA_FIELD_FARZ = ConvertCameraField(1)
constant camerafield CAMERA_FIELD_ANGLE_OF_ATTACK = ConvertCameraField(2)
constant camerafield CAMERA_FIELD_FIELD_OF_VIEW = ConvertCameraField(3)
constant camerafield CAMERA_FIELD_ROLL = ConvertCameraField(4)
constant camerafield CAMERA_FIELD_ROTATION = ConvertCameraField(5)
constant camerafield CAMERA_FIELD_ZOFFSET = ConvertCameraField(6)
constant integer bj_CAMERA_DEFAULT_ROLL = 0
endglobals
";

fn host() -> ObjectHost {
    let common = pcx_script::parse_script(COMMON).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:state", &common)]).unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:state",
        &[
            "ConvertFGameState",
            "ConvertPlayerColor",
            "ConvertVolumeGroup",
            "ConvertCameraField",
        ],
        Limits {
            max_handles: 64,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    ObjectHost::checked(inner)
        .unwrap()
        .with_startup_clock(crate::startup_clock::tests::input(9), "CLOCK", 64, 16, 1000)
        .unwrap()
        .with_timer_runtime(100.0)
        .unwrap()
        .with_environment()
        .unwrap()
        .with_volume_groups()
        .unwrap()
        .with_indicators(2)
        .unwrap()
        .with_cinematic_scenes()
        .unwrap()
        .with_camera_setups(8)
        .unwrap()
}

fn advance(h: &mut ObjectHost, until: f64) {
    assert!(h.next_timer_expiry(until, &mut 8).unwrap().is_none());
}

#[test]
fn time_of_day_runs_on_the_source_rate_and_honours_suspend_and_scale() {
    let mut h = host();
    h.admit_time_of_day(&DayCycle {
        day_length: 480.0,
        day_hours: 24.0,
        sources: vec!["synthetic".into()],
    })
    .unwrap();
    let clock = h.native("ConvertFGameState", &[Value::Int(2)]).unwrap();
    let hour = |h: &mut ObjectHost| h.native("GetFloatGameState", std::slice::from_ref(&clock));
    assert_eq!(hour(&mut h).unwrap(), Value::Real(9.0));
    assert_eq!(
        h.native("GetTimeOfDayScale", &[]).unwrap(),
        Value::Real(3.0)
    );
    h.native("SetTimeOfDayScale", &[Value::Real(1.0)]).unwrap();
    h.native("SetFloatGameState", &[clock.clone(), Value::Real(23.0)])
        .unwrap();
    // 24 hours per 480 s: 40 s is two hours, wrapping past midnight.
    advance(&mut h, 140.0);
    assert_eq!(hour(&mut h).unwrap(), Value::Real(1.0));
    h.native("SuspendTimeOfDay", &[Value::Bool(true)]).unwrap();
    advance(&mut h, 200.0);
    assert_eq!(hour(&mut h).unwrap(), Value::Real(1.0));
    h.native("SuspendTimeOfDay", &[Value::Bool(false)]).unwrap();
    advance(&mut h, 220.0);
    assert_eq!(hour(&mut h).unwrap(), Value::Real(2.0));
    let other = h.native("ConvertFGameState", &[Value::Int(1)]).unwrap();
    assert!(
        h.native("SetFloatGameState", &[other, Value::Real(1.0)])
            .is_err()
    );
}

#[test]
fn paused_timers_keep_their_remaining_time_until_resumed() {
    let mut h = host();
    let timer = h.native("CreateTimer", &[]).unwrap();
    h.native(
        "TimerStart",
        &[
            timer.clone(),
            Value::Real(10.0),
            Value::Bool(false),
            Value::Null,
        ],
    )
    .unwrap();
    advance(&mut h, 104.0);
    h.native("PauseTimer", std::slice::from_ref(&timer))
        .unwrap();
    advance(&mut h, 150.0);
    let remaining =
        |h: &mut ObjectHost| h.native("TimerGetRemaining", std::slice::from_ref(&timer));
    assert_eq!(remaining(&mut h).unwrap(), Value::Real(6.0));
    h.native("ResumeTimer", std::slice::from_ref(&timer))
        .unwrap();
    assert!(h.next_timer_expiry(155.0, &mut 8).unwrap().is_none());
    let expiry = h.next_timer_expiry(157.0, &mut 8).unwrap().expect("expiry");
    assert_eq!(h.timer_game_time(), Some(156.0));
    h.finish_timer_expiry(expiry).unwrap();
    h.native("PauseTimer", &[Value::Null]).unwrap();
}

#[test]
fn presentation_records_keep_executed_arguments_for_their_consumers() {
    let mut h = host();
    assert_eq!(h.environment(), Some(&Environment::default()));
    h.native("SetSkyModel", &[Value::Str("Sky.mdl".into())])
        .unwrap();
    h.native(
        "SetTerrainFogEx",
        &[
            Value::Int(0),
            Value::Real(1.0),
            Value::Real(2.0),
            Value::Real(0.5),
            Value::Real(0.1),
            Value::Real(0.2),
            Value::Real(0.3),
        ],
    )
    .unwrap();
    let state = h.environment().unwrap();
    assert_eq!(state.sky_model, Some(Some("Sky.mdl".into())));
    assert_eq!(state.terrain_fog.unwrap().color, [0.1, 0.2, 0.3]);
    h.native("SetSkyModel", &[Value::Null]).unwrap();
    h.native("ResetTerrainFog", &[]).unwrap();
    assert_eq!(h.environment().unwrap().sky_model, Some(None));
    assert_eq!(h.environment().unwrap().terrain_fog, None);

    let music = h.native("ConvertVolumeGroup", &[Value::Int(5)]).unwrap();
    h.native("VolumeGroupSetVolume", &[music, Value::Real(0.25)])
        .unwrap();
    assert_eq!(h.volume_group_scales().unwrap().get(&5), Some(&0.25));
    h.native("VolumeGroupReset", &[]).unwrap();
    assert!(h.volume_group_scales().unwrap().is_empty());

    for x in [1.0, 2.0, 3.0] {
        h.native(
            "PingMinimap",
            &[Value::Real(x), Value::Real(0.0), Value::Real(1.0)],
        )
        .unwrap();
    }
    let pings: Vec<f32> = h.minimap_pings().iter().map(|p| p.x).collect();
    assert_eq!(pings, [2.0, 3.0], "bounded to the newest two");
    h.native(
        "AddIndicator",
        &[
            Value::Null,
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
        ],
    )
    .unwrap();
    assert!(h.indicators().is_empty());

    let red = h.native("ConvertPlayerColor", &[Value::Int(0)]).unwrap();
    h.native(
        "SetCinematicScene",
        &[
            Value::Int(i32::from_be_bytes(*b"Hart")),
            red,
            Value::Str("Arthas".into()),
            Value::Str("Line".into()),
            Value::Real(4.0),
            Value::Real(3.5),
        ],
    )
    .unwrap();
    let scene = h.cinematic_scene().unwrap();
    assert_eq!((scene.speaker.as_str(), scene.started), ("Arthas", 100.0));
    h.native("EndCinematicScene", &[]).unwrap();
    assert!(h.cinematic_scene().is_none());
}

#[test]
fn camera_ramps_toward_setups_and_game_fields_over_their_durations() {
    let mut h = host();
    h.admit_camera_view(&GameCamera {
        angle_of_attack: 304.0,
        field_of_view: 70.0,
        rotation: 90.0,
        distance: 1650.0,
        far_z: 5000.0,
        height: 0.0,
        forward_rate: 3000.0,
        strafe_rate: 3000.0,
    })
    .unwrap();
    assert_eq!(h.camera_target(), None);
    assert!(h.native("GetCameraTargetPositionLoc", &[]).is_err());
    h.native(
        "PanCameraToTimed",
        &[Value::Real(0.0), Value::Real(0.0), Value::Real(0.0)],
    )
    .unwrap();
    h.native(
        "PanCameraToTimed",
        &[Value::Real(100.0), Value::Real(0.0), Value::Real(2.0)],
    )
    .unwrap();
    advance(&mut h, 101.0);
    assert_eq!(h.camera_target(), Some([50.0, 0.0]));
    let distance = h.native("ConvertCameraField", &[Value::Int(0)]).unwrap();
    let Value::Handle(distance_id) = distance else {
        panic!("field")
    };
    assert_eq!(h.camera_field(distance_id).unwrap(), Some(1650.0));
    let setup = h.native("CreateCameraSetup", &[]).unwrap();
    h.native(
        "CameraSetupSetField",
        &[
            setup.clone(),
            distance,
            Value::Real(2650.0),
            Value::Real(0.0),
        ],
    )
    .unwrap();
    h.native(
        "CameraSetupApplyForceDuration",
        &[setup, Value::Bool(false), Value::Real(4.0)],
    )
    .unwrap();
    advance(&mut h, 103.0);
    assert_eq!(h.camera_field(distance_id).unwrap(), Some(2150.0));
    h.native("ResetToGameCamera", &[Value::Real(0.0)]).unwrap();
    assert_eq!(h.camera_field(distance_id).unwrap(), Some(1650.0));
    h.scroll_camera([7.0, 8.0]).unwrap();
    assert_eq!(h.camera_target(), Some([7.0, 8.0]));
    assert_eq!(h.camera_scroll_rates(), Some([3000.0, 3000.0]));
}

#[test]
fn player_zoom_eases_the_live_fields_and_becomes_the_game_camera() {
    let mut h = host();
    let game = GameCamera {
        angle_of_attack: 304.0,
        field_of_view: 70.0,
        rotation: 90.0,
        distance: 1650.0,
        far_z: 5000.0,
        height: 0.0,
        forward_rate: 3000.0,
        strafe_rate: 3000.0,
    };
    let pose = crate::camera_input::Pose {
        distance: 1600.0,
        angle_of_attack: 311.0,
        rotation: 20.0,
    };
    assert!(h.ease_player_camera(pose, 1.0).is_err(), "no camera yet");
    h.admit_camera_view(&game).unwrap();
    let field = |h: &mut ObjectHost, index: i32| match h
        .native("ConvertCameraField", &[Value::Int(index)])
        .unwrap()
    {
        Value::Handle(id) => id,
        other => panic!("{other:?}"),
    };
    let (distance, attack, rotation) = (field(&mut h, 0), field(&mut h, 2), field(&mut h, 5));
    let live = |h: &ObjectHost| {
        [distance, attack, rotation].map(|id| h.camera_field(id).unwrap().unwrap())
    };
    let start = h.timer_game_time().unwrap();
    h.ease_player_camera(pose, 1.0).unwrap();
    assert_eq!(live(&h), [1650.0, 304.0, 90.0]);
    advance(&mut h, start + 0.5);
    assert_eq!(live(&h), [1625.0, 307.5, 55.0]);
    advance(&mut h, start + 2.0);
    assert_eq!(live(&h), [1600.0, 311.0, 20.0]);
    // A script camera replaces the live fields; resetting returns to the player's pose.
    h.native(
        "PanCameraToTimed",
        &[Value::Real(0.0), Value::Real(0.0), Value::Real(0.0)],
    )
    .unwrap();
    let setup = h.native("CreateCameraSetup", &[]).unwrap();
    h.native(
        "CameraSetupSetField",
        &[
            setup.clone(),
            Value::Handle(distance),
            Value::Real(900.0),
            Value::Real(0.0),
        ],
    )
    .unwrap();
    h.native(
        "CameraSetupApplyForceDuration",
        &[setup, Value::Bool(false), Value::Real(0.0)],
    )
    .unwrap();
    assert_eq!(live(&h)[0], 900.0);
    h.native("ResetToGameCamera", &[Value::Real(0.0)]).unwrap();
    assert_eq!(live(&h), [1600.0, 311.0, 20.0]);
    for bad in [f32::NAN, f32::INFINITY] {
        assert!(h.ease_player_camera(pose, bad).is_err());
        let mut broken = pose;
        broken.distance = bad;
        assert!(h.ease_player_camera(broken, 1.0).is_err());
    }
}

#[test]
fn camera_getters_report_live_target_and_fields_with_angles_in_radians() {
    let mut h = host();
    h.admit_camera_view(&GameCamera {
        angle_of_attack: 304.0,
        field_of_view: 70.0,
        rotation: 90.0,
        distance: 1650.0,
        far_z: 5000.0,
        height: 0.0,
        forward_rate: 3000.0,
        strafe_rate: 3000.0,
    })
    .unwrap();
    for name in ["GetCameraTargetPositionX", "GetCameraTargetPositionY"] {
        assert!(h.native(name, &[]).is_err(), "unset target must refuse");
    }
    h.native(
        "PanCameraToTimed",
        &[Value::Real(12.5), Value::Real(-3.0), Value::Real(0.0)],
    )
    .unwrap();
    assert_eq!(
        h.native("GetCameraTargetPositionX", &[]).unwrap(),
        Value::Real(12.5)
    );
    assert_eq!(
        h.native("GetCameraTargetPositionY", &[]).unwrap(),
        Value::Real(-3.0)
    );
    let field = |h: &mut ObjectHost, index: i32| {
        let field = h
            .native("ConvertCameraField", &[Value::Int(index)])
            .unwrap();
        h.native("GetCameraField", &[field])
    };
    assert_eq!(field(&mut h, 0).unwrap(), Value::Real(1650.0));
    assert_eq!(field(&mut h, 6).unwrap(), Value::Real(0.0));
    assert_eq!(field(&mut h, 2).unwrap(), Value::Real(304f32.to_radians()));
    assert_eq!(field(&mut h, 3).unwrap(), Value::Real(70f32.to_radians()));
    assert_eq!(field(&mut h, 4).unwrap(), Value::Real(0.0));
    assert_eq!(field(&mut h, 5).unwrap(), Value::Real(90f32.to_radians()));
    assert_eq!(field(&mut h, 1).unwrap(), Value::Real(5000.0));
}

#[test]
fn ai_commands_queue_per_player_and_refuse_an_unordered_read() {
    let mut h = host();
    assert_eq!(h.ai_commands_waiting(4), 0);
    assert!(h.ai_last_command(4, "GetLastCommand").is_err());
    h.push_ai_command(4, 7, 9).unwrap();
    assert_eq!(h.ai_commands_waiting(4), 1);
    assert_eq!(h.ai_commands_waiting(5), 0);
    assert_eq!(h.ai_last_command(4, "GetLastCommand").unwrap(), (7, 9));
    h.pop_ai_command(4).unwrap();
    assert_eq!(h.ai_commands_waiting(4), 0);
    h.push_ai_command(4, 1, 2).unwrap();
    h.push_ai_command(4, 3, 4).unwrap();
    let error = h.ai_last_command(4, "GetLastData").unwrap_err().to_string();
    assert!(
        error.contains("which one is last is not established"),
        "{error}"
    );
    assert!(h.pop_ai_command(4).is_err());
    assert_eq!(h.ai_commands_waiting(4), 2, "a refused pop removes nothing");
    for _ in 2..64 {
        h.push_ai_command(4, 0, 0).unwrap();
    }
    assert!(h.push_ai_command(4, 0, 0).is_err());
}

#[test]
fn timer_dialogs_retain_executed_state_and_refuse_unestablished_uses() {
    let mut h = host().with_timer_dialogs().unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let Value::Handle(dialog) = h
        .native("CreateTimerDialog", std::slice::from_ref(&timer))
        .unwrap()
    else {
        panic!("dialog");
    };
    let call = |h: &mut ObjectHost, name: &str, tail: &[Value]| {
        let mut args = vec![Value::Handle(dialog)];
        args.extend_from_slice(tail);
        h.native(name, &args)
    };
    assert_eq!(
        call(&mut h, "IsTimerDialogDisplayed", &[]).unwrap(),
        Value::Bool(false)
    );
    call(
        &mut h,
        "TimerDialogSetTitle",
        &[Value::Str("Reinforcements".into())],
    )
    .unwrap();
    call(&mut h, "TimerDialogDisplay", &[Value::Bool(true)]).unwrap();
    call(&mut h, "TimerDialogSetSpeed", &[Value::Real(2.5)]).unwrap();
    call(
        &mut h,
        "TimerDialogSetRealTimeRemaining",
        &[Value::Real(31.0)],
    )
    .unwrap();
    let color = [Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)];
    call(&mut h, "TimerDialogSetTitleColor", &color).unwrap();
    call(&mut h, "TimerDialogSetTimeColor", &color).unwrap();
    assert_eq!(
        call(&mut h, "IsTimerDialogDisplayed", &[]).unwrap(),
        Value::Bool(true)
    );
    let live: Vec<_> = h.timer_dialogs().collect();
    assert_eq!(live.len(), 1);
    let (_, state) = live[0];
    assert_eq!(state.title.as_deref(), Some("Reinforcements"));
    assert_eq!(
        (state.speed, state.real_time_remaining),
        (Some(2.5), Some(31.0))
    );
    assert_eq!(state.title_color, Some([1, 2, 3, 4]));
    assert!(matches!(timer, Value::Handle(t) if t == state.timer));
    // Unestablished uses refuse; a bad argument leaves the state unchanged.
    assert!(h.native("CreateTimerDialog", &[Value::Null]).is_err());
    assert!(call(&mut h, "TimerDialogSetSpeed", &[Value::Real(f32::NAN)]).is_err());
    assert!(
        h.native("TimerDialogDisplay", &[Value::Null, Value::Bool(true)])
            .is_err()
    );
    call(&mut h, "DestroyTimerDialog", &[]).unwrap();
    assert_eq!(h.timer_dialogs().count(), 0);
    assert!(call(&mut h, "DestroyTimerDialog", &[]).is_err());
    assert!(call(&mut h, "IsTimerDialogDisplayed", &[]).is_err());
}
