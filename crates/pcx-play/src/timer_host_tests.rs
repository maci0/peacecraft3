use super::{EnumHost, Failure, Limits, TimerHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing\nglobals\nconstant playercolor C = ConvertPlayerColor(37)\nendglobals\n";
fn enums(common: &str, map: &str, max_handles: usize) -> EnumHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let p = pcx_script::resolve_with(&map, &[Prelude::new("synthetic:timers", &common)]).unwrap();
    EnumHost::checked_selected(
        Arc::new(p),
        "synthetic:map:timers",
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
fn host(max_handles: usize) -> TimerHost {
    TimerHost::checked(enums(COMMON, "", max_handles)).unwrap()
}
#[test]
fn distinct_timers_and_symbolic_values_share_one_arena() {
    let mut h = host(4);
    let a = h.native("CreateTimer", &[]).unwrap();
    let c = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    let b = h.native("CreateTimer", &[]).unwrap();
    assert_ne!(a, b);
    assert_ne!(a, c);
    assert_ne!(b, c);
    assert_eq!(h.live_timers(), 2);
    assert_eq!(h.inner().handles(), 3);
    let Value::Handle(id) = a else {
        panic!("timer identity")
    };
    assert_eq!(h.inner().kind(id).unwrap(), "timer");
    assert_eq!(
        h.inner().player_color(id).unwrap_err().failure,
        Failure::WrongFamily
    );
}
#[test]
fn destroy_revokes_identity_and_reallocation_never_revives_aliases() {
    let mut h = host(3);
    let a = h.native("CreateTimer", &[]).unwrap();
    assert_eq!(
        h.native("DestroyTimer", std::slice::from_ref(&a)).unwrap(),
        Value::Null
    );
    assert_eq!(h.live_timers(), 0);
    assert!(
        h.native("DestroyTimer", std::slice::from_ref(&a))
            .unwrap_err()
            .contains("unknown handle")
    );
    let Value::Handle(id) = a.clone() else {
        panic!("timer identity")
    };
    assert_eq!(h.inner().kind(id).unwrap_err().failure, Failure::Identity);
    let b = h.native("CreateTimer", &[]).unwrap();
    assert_ne!(a, b);
    assert_eq!(h.live_timers(), 1);
    assert!(h.native("DestroyTimer", &[a]).is_err());
    assert_eq!(h.live_timers(), 1);
}
#[test]
fn exhaustion_leaves_existing_timer_and_symbolic_identity_intact() {
    let mut h = host(2);
    let a = h.native("CreateTimer", &[]).unwrap();
    let c = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    assert!(
        h.native("CreateTimer", &[])
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(h.live_timers(), 1);
    assert_eq!(
        h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap(),
        c
    );
    h.native("DestroyTimer", &[a]).unwrap();
    assert!(h.native("CreateTimer", &[]).is_err());
    assert_eq!(h.live_timers(), 0);
}
#[test]
fn wrong_family_bad_arguments_and_unsupported_scheduling_do_not_mutate_state() {
    let mut h = host(4);
    let c = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    assert!(
        h.native("DestroyTimer", &[c])
            .unwrap_err()
            .contains("another family")
    );
    for args in [
        vec![],
        vec![Value::Null],
        vec![Value::Int(1)],
        vec![Value::Null, Value::Null],
    ] {
        assert!(h.native("DestroyTimer", &args).is_err());
    }
    assert!(h.native("CreateTimer", &[Value::Int(1)]).is_err());
    assert!(
        h.native("TimerStart", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(h.inner().handles(), 1);
    assert_eq!(h.live_timers(), 0);
}
#[test]
fn incompatible_declarations_and_script_overrides_refuse_installation() {
    for common in [
        COMMON.replace(
            "native CreateTimer takes nothing",
            "constant native CreateTimer takes nothing",
        ),
        COMMON.replace("returns timer", "returns playercolor"),
        COMMON.replace(
            "native DestroyTimer takes timer",
            "native DestroyTimer takes playercolor",
        ),
        COMMON.replace("type timer extends agent", "type timer extends handle"),
        COMMON.replace(
            "native CreateTimer takes nothing",
            "native CreateTimer takes integer i",
        ),
        COMMON.replace(
            "native DestroyTimer takes timer t returns nothing",
            "native DestroyTimer takes timer t returns boolean",
        ),
    ] {
        assert!(TimerHost::checked(enums(&common, "", 4)).is_err());
    }
    assert!(
        TimerHost::checked(enums(
            COMMON,
            "function CreateTimer takes nothing returns timer\nreturn null\nendfunction\n",
            4
        ))
        .is_err()
    );
}
#[test]
fn compiled_initializer_retains_live_object_and_sticky_next_native_failure() {
    let common = format!("{COMMON}native Next takes nothing returns integer\n");
    let map =
        "globals\ntimer A = CreateTimer()\ntimer B = A\ninteger failure = Next()\nendglobals\n";
    let mut vm = TimerHost::checked(enums(&common, map, 4))
        .unwrap()
        .into_vm();
    let first = vm.init_globals().unwrap_err();
    assert!(first.to_string().contains("Next"));
    assert_eq!(vm.global("A"), vm.global("B"));
    assert!(matches!(vm.global("A"), Value::Handle(_)));
    assert_eq!(vm.host().live_timers(), 1);
    let count = vm.host().inner().handles();
    assert_eq!(first, vm.init_globals().unwrap_err());
    assert_eq!(vm.host().live_timers(), 1);
    assert_eq!(vm.host().inner().handles(), count);
}
#[test]
fn actual_vm_create_alias_destroy_sequence_runs_without_minting_a_null_timer() {
    let map = "globals\ntimer A = CreateTimer()\ntimer B = A\nendglobals\nfunction dispose takes nothing returns nothing\ncall DestroyTimer(B)\nendfunction\n";
    let mut vm = TimerHost::checked(enums(COMMON, map, 4)).unwrap().into_vm();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().live_timers(), 1);
    vm.call("dispose", &[]).unwrap();
    assert_eq!(vm.host().live_timers(), 0);
    assert!(
        vm.call("dispose", &[])
            .unwrap_err()
            .to_string()
            .contains("unknown handle")
    );
}

#[test]
fn separately_qualified_duplicate_native_sources_refuse_timer_installation() {
    let common = pcx_script::parse_script(COMMON).unwrap();
    let duplicate =
        pcx_script::parse_script("native CreateTimer takes nothing returns timer\n").unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:common", &common),
                Prelude::new("synthetic:other", &duplicate),
            ],
        )
        .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 4,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    assert!(matches!(TimerHost::checked(inner), Err(e) if e.failure == Failure::Source));
}

#[test]
fn reserving_an_object_with_timer_type_does_not_create_timer_lifecycle_state() {
    let mut h = host(4);
    let unowned = h.inner.reserve_identity("timer").unwrap();
    assert!(
        h.native("DestroyTimer", &[Value::Handle(unowned)])
            .unwrap_err()
            .contains("another family")
    );
    assert_eq!(h.live_timers(), 0);
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert_ne!(timer, Value::Handle(unowned));
}
