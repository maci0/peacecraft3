use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_script::{Host, Prelude};
use std::sync::Arc;

const COMMON: &str = "
type agent extends handle
type timer extends agent
type blendmode extends handle
type texmapflags extends handle
constant native ConvertBlendMode takes integer n returns blendmode
constant native ConvertTexMapFlags takes integer n returns texmapflags
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native EnableUserUI takes boolean enabled returns nothing
native EnableUserControl takes boolean enabled returns nothing
native ShowInterface takes boolean show, real duration returns nothing
native SetCineFilterTexture takes string path returns nothing
native SetCineFilterBlendMode takes blendmode b returns nothing
native SetCineFilterTexMapFlags takes texmapflags f returns nothing
native SetCineFilterStartUV takes real a, real b, real c, real d returns nothing
native SetCineFilterEndUV takes real a, real b, real c, real d returns nothing
native SetCineFilterStartColor takes integer r, integer g, integer b, integer a returns nothing
native SetCineFilterEndColor takes integer r, integer g, integer b, integer a returns nothing
native SetCineFilterDuration takes real duration returns nothing
native DisplayCineFilter takes boolean show returns nothing
globals
constant blendmode BLEND_MODE_BLEND=ConvertBlendMode(73)
constant blendmode OTHER=ConvertBlendMode(74)
constant texmapflags TEXMAP_FLAG_NONE=ConvertTexMapFlags(21)
endglobals
";
const SETUP: &str = "
function setup takes nothing returns nothing
call EnableUserUI(false)
call SetCineFilterTexture(\"synthetic.tga\")
call SetCineFilterBlendMode(BLEND_MODE_BLEND)
call SetCineFilterTexMapFlags(TEXMAP_FLAG_NONE)
call SetCineFilterStartUV(1.0,3.0,-1.0,2.0)
call SetCineFilterEndUV(0.0,0.0,1.0,1.0)
call SetCineFilterStartColor(-1,256,257,0)
call SetCineFilterEndColor(0,128,255,255)
call SetCineFilterDuration(0.0)
call DisplayCineFilter(true)
endfunction
";
fn host(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "synthetic:filter",
            &["ConvertBlendMode", "ConvertTexMapFlags"],
            Limits {
                max_handles: 32,
                admission_steps: 1024,
                expression_depth: 16,
                expression_work: 1024,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn actual_vm_publishes_explicit_filter_fields_without_constructor_defaults() {
    let mut h = host(COMMON, SETUP);
    h.admit_presentation(64).unwrap();
    assert_eq!(h.presentation().unwrap().ui_enabled, None);
    assert!(h.native("DisplayCineFilter", &[Value::Bool(true)]).is_err());
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let state = vm.host().presentation().unwrap();
    assert_eq!(state.ui_enabled, Some(false));
    let filter = state.filter.as_ref().unwrap();
    assert_eq!(filter.texture, "synthetic.tga");
    assert_eq!(filter.start_uv, [-1., 2., 1., 3.]);
    assert_eq!(filter.start_color, [255, 0, 1, 0]);
    assert_eq!(
        filter.sample(None).unwrap().color,
        [0., 128. / 255., 1., 1.]
    );
    let old = filter.clone();
    let other = vm.global("OTHER");
    vm.host()
        .native("SetCineFilterBlendMode", &[other])
        .unwrap();
    assert!(
        vm.host()
            .native("DisplayCineFilter", &[Value::Bool(true)])
            .is_err()
    );
    assert_eq!(
        vm.host().presentation().unwrap().filter.as_ref(),
        Some(&old)
    );
    vm.host()
        .native("DisplayCineFilter", &[Value::Bool(false)])
        .unwrap();
    assert!(vm.host().presentation().unwrap().filter.is_none());
    assert_eq!(vm.host().presentation().unwrap().ui_enabled, Some(false));
    vm.host()
        .native("EnableUserUI", &[Value::Bool(true)])
        .unwrap();
    assert_eq!(vm.host().presentation().unwrap().ui_enabled, Some(true));
}
#[test]
fn timed_filter_refuses_without_clock_and_invalid_setters_preserve_the_previous_draft() {
    let mut h = host(COMMON, SETUP);
    h.admit_presentation(64).unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    for args in [
        vec![Value::Real(f32::NAN)],
        vec![Value::Real(-1.)],
        vec![Value::Int(1)],
    ] {
        assert!(vm.host().native("SetCineFilterDuration", &args).is_err());
    }
    assert!(
        vm.host()
            .native("SetCineFilterTexture", &[Value::Str("x".repeat(65))])
            .is_err()
    );
    assert!(
        vm.host()
            .native("SetCineFilterTexture", &[Value::Str("bad\0path".into())])
            .is_err()
    );
    vm.host()
        .native("DisplayCineFilter", &[Value::Bool(true)])
        .unwrap();
    assert_eq!(
        vm.host()
            .presentation()
            .unwrap()
            .filter
            .as_ref()
            .unwrap()
            .duration,
        0.
    );
    vm.host()
        .native("SetCineFilterDuration", &[Value::Real(2.)])
        .unwrap();
    assert!(
        vm.host()
            .native("DisplayCineFilter", &[Value::Bool(true)])
            .is_err()
    );
    assert_eq!(
        vm.host()
            .presentation()
            .unwrap()
            .filter
            .as_ref()
            .unwrap()
            .sequence,
        2
    );
    assert!(vm.host().native("EnableUserUI", &[Value::Int(0)]).is_err());
}
#[test]
fn filter_samples_depend_on_logical_time_not_update_chunking() {
    let filter = CineFilter {
        sequence: 1,
        texture: "synthetic".into(),
        start_uv: [0.; 4],
        end_uv: [1.; 4],
        start_color: [0; 4],
        end_color: [255; 4],
        duration: 2.,
        started_at: Some(10.),
    };
    let expected = filter.sample(Some(11.)).unwrap();
    for times in [&[10., 11.][..], &[10., 10.1, 10.9, 11.], &[11.][..]] {
        for &time in times {
            let _ = filter.sample(Some(time)).unwrap();
        }
        assert_eq!(filter.sample(times.last().copied()).unwrap(), expected);
    }
    assert_eq!(expected.color, [0.5; 4]);
    assert_eq!(filter.sample(Some(30.)).unwrap().color, [1.; 4]);
    assert!(filter.sample(None).is_err());
    assert!(filter.sample(Some(9.)).is_err());
    assert!(filter.sample(Some(f64::NAN)).is_err());
}
#[test]
fn omitted_and_foreign_natives_cannot_use_the_presentation_dispatch() {
    let common = COMMON.replace(
        "native EnableUserUI takes boolean enabled returns nothing\n",
        "",
    );
    let mut h = host(&common, "");
    h.admit_presentation(64).unwrap();
    assert!(h.native("EnableUserUI", &[Value::Bool(false)]).is_err());
    let mut h = host(
        &common,
        "native EnableUserUI takes boolean enabled returns nothing\n",
    );
    assert!(h.admit_presentation(64).is_err());
    let mut h = host(
        &COMMON.replace(
            "native EnableUserUI takes boolean enabled",
            "native EnableUserUI takes integer enabled",
        ),
        "",
    );
    assert!(h.admit_presentation(64).is_err());
}

#[test]
fn published_timed_filter_advances_on_the_host_timer_clock() {
    let timed = "native TimerStart takes timer t, real timeout, boolean periodic, code f returns nothing\nnative TimerGetElapsed takes timer t returns real\nnative TimerGetRemaining takes timer t returns real\nnative TimerGetTimeout takes timer t returns real\nnative GetExpiredTimer takes nothing returns timer\n";
    let mut h = host(&format!("{COMMON}{timed}"), SETUP)
        .with_timer_runtime(20.)
        .unwrap();
    h.admit_presentation(64).unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let h = vm.host();
    h.native("SetCineFilterDuration", &[Value::Real(2.)])
        .unwrap();
    h.native("DisplayCineFilter", &[Value::Bool(true)]).unwrap();
    assert!(h.next_timer_expiry(21., &mut 1).unwrap().is_none());
    let f = h.presentation().unwrap().filter.as_ref().unwrap();
    assert_eq!(f.started_at, Some(20.));
    assert_eq!(f.sample(h.timer_game_time()).unwrap().color[3], 0.5);
}

#[test]
fn immediate_interface_changes_control_without_changing_the_ui_flag_or_clock() {
    let mut h = host(
        COMMON,
        "function run takes nothing returns nothing\ncall ShowInterface(false,0)\ncall EnableUserControl(true)\nendfunction",
    );
    h.admit_presentation(64).unwrap();
    assert_eq!(h.presentation().unwrap().interface_visible, None);
    assert_eq!(h.presentation().unwrap().control_enabled, None);
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("run", &[]).unwrap();
    let state = vm.host().presentation().unwrap();
    assert_eq!(state.interface_visible, Some(false));
    assert_eq!(state.control_enabled, Some(true));
    assert_eq!(state.ui_enabled, None);
    vm.host()
        .native("EnableUserUI", &[Value::Bool(false)])
        .unwrap();
    vm.host()
        .native("EnableUserControl", &[Value::Bool(false)])
        .unwrap();
    vm.host()
        .native("ShowInterface", &[Value::Bool(true), Value::Real(0.)])
        .unwrap();
    let state = vm.host().presentation().unwrap();
    assert_eq!(state.interface_visible, Some(true));
    assert_eq!(state.control_enabled, Some(true));
    assert_eq!(state.ui_enabled, Some(false));
    assert_eq!(vm.host().timer_game_time(), None);
}
#[test]
fn unsupported_interface_fades_and_bad_arguments_preserve_the_visible_state() {
    let mut h = host(COMMON, "");
    h.admit_presentation(64).unwrap();
    h.native("ShowInterface", &[Value::Bool(false), Value::Real(0.)])
        .unwrap();
    for duration in [1., -1., f32::INFINITY, f32::NAN] {
        assert!(
            h.native("ShowInterface", &[Value::Bool(true), Value::Real(duration)])
                .is_err()
        );
        assert_eq!(h.presentation().unwrap().interface_visible, Some(false));
        assert_eq!(h.presentation().unwrap().control_enabled, Some(false));
    }
    for (name, args) in [
        ("ShowInterface", vec![Value::Bool(true), Value::Int(0)]),
        ("EnableUserControl", vec![Value::Int(1)]),
        ("ShowInterface", vec![]),
    ] {
        assert!(h.native(name, &args).is_err());
    }
    assert_eq!(h.presentation().unwrap().interface_visible, Some(false));
}
#[test]
fn optional_interface_and_control_contracts_do_not_admit_foreign_or_missing_calls() {
    for declaration in [
        "native EnableUserControl takes boolean enabled returns nothing\n",
        "native ShowInterface takes boolean show, real duration returns nothing\n",
    ] {
        let common = COMMON.replace(declaration, "");
        let mut h = host(&common, "");
        h.admit_presentation(64).unwrap();
        let name = declaration.split_whitespace().nth(1).unwrap();
        assert!(h.native(name, &[]).is_err());
        let mut h = host(&common, declaration);
        assert!(h.admit_presentation(64).is_err());
        let mut h = host(
            &COMMON.replace(declaration, &format!("constant {declaration}")),
            "",
        );
        assert!(h.admit_presentation(64).is_err());
    }
}

#[test]
fn disabled_occlusion_requires_an_explicit_renderer_capability_and_never_enables_a_pass() {
    let common = format!("{COMMON}\nnative EnableOcclusion takes boolean flag returns nothing\n");
    let mut h = host(&common, "");
    assert!(h.native("EnableOcclusion", &[Value::Bool(false)]).is_err());
    h.admit_no_occlusion_pass().unwrap();
    let handles = h.inner().handles();
    assert_eq!(
        h.native("EnableOcclusion", &[Value::Bool(false)]).unwrap(),
        Value::Null
    );
    assert!(
        h.native("EnableOcclusion", &[Value::Bool(true)])
            .unwrap_err()
            .contains("renderer")
    );
    for args in [
        vec![],
        vec![Value::Int(0)],
        vec![Value::Bool(false), Value::Bool(false)],
    ] {
        assert!(h.native("EnableOcclusion", &args).is_err());
    }
    assert_eq!(h.inner().handles(), handles);
    assert!(h.presentation().is_none());
    assert_eq!(h.timer_game_time(), None);
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    assert_eq!(
        vm.host()
            .native("EnableOcclusion", &[Value::Bool(false)])
            .unwrap(),
        Value::Null
    );
}
#[test]
fn occlusion_capability_checks_optional_native_source_and_signature() {
    let mut h = host(COMMON, "");
    h.admit_no_occlusion_pass().unwrap();
    assert!(h.native("EnableOcclusion", &[Value::Bool(false)]).is_err());
    for declaration in [
        "constant native EnableOcclusion takes boolean flag returns nothing",
        "native EnableOcclusion takes integer flag returns nothing",
        "native EnableOcclusion takes boolean flag returns integer",
    ] {
        let mut h = host(&format!("{COMMON}\n{declaration}"), "");
        assert!(h.admit_no_occlusion_pass().is_err());
        assert!(h.native("EnableOcclusion", &[Value::Bool(false)]).is_err());
    }
    let mut h = host(
        COMMON,
        "native EnableOcclusion takes boolean flag returns nothing",
    );
    assert!(h.admit_no_occlusion_pass().is_err());
}
