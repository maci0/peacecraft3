use super::{EnumHost, Failure, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

const COMMON: &str = "\
type agent extends handle
type timer extends agent
type playercolor extends handle
constant native ConvertPlayerColor takes integer i returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
globals
constant playercolor C = ConvertPlayerColor(17)
endglobals
";
fn checked(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program =
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:timer-common", &common)]).unwrap();
    ObjectHost::checked(
        EnumHost::checked_selected(
            Arc::new(program),
            "synthetic:timer-map",
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 16,
                admission_steps: 100,
                expression_depth: 16,
                expression_work: 100,
            },
        )
        .unwrap(),
    )
    .unwrap()
}
fn host() -> ObjectHost {
    checked(COMMON, "").with_timer_runtime(20.0).unwrap()
}
fn start(h: &mut ObjectHost, timer: &Value, timeout: f32, periodic: bool) {
    h.native(
        "TimerStart",
        &[
            timer.clone(),
            Value::Real(timeout),
            Value::Bool(periodic),
            Value::Null,
        ],
    )
    .unwrap();
}
fn query(h: &mut ObjectHost, name: &str, timer: &Value) -> Value {
    h.native(name, std::slice::from_ref(timer)).unwrap()
}

#[test]
fn bounded_delivery_preserves_next_expiry_and_restarted_order_without_replay() {
    let mut h = host();
    let a = h.native("CreateTimer", &[]).unwrap();
    let b = h.native("CreateTimer", &[]).unwrap();
    start(&mut h, &a, 2.0, false);
    start(&mut h, &b, 2.0, false);
    start(&mut h, &a, 2.0, false);
    assert!(h.next_timer_expiry(24.0, &mut 0).is_err());
    assert_eq!(h.timer_game_time(), Some(20.0));
    let mut budget = 1;
    let expiry = h.next_timer_expiry(24.0, &mut budget).unwrap().unwrap();
    assert_eq!(h.native("GetExpiredTimer", &[]).unwrap(), b);
    assert_eq!(query(&mut h, "TimerGetElapsed", &b), Value::Real(2.0));
    assert!(h.next_timer_expiry(24.0, &mut 1).is_err());
    h.finish_timer_expiry(expiry).unwrap();
    assert_eq!(
        h.next_timer_expiry(24.0, &mut budget).unwrap_err().failure,
        Failure::Budget("timer delivery")
    );
    assert_eq!(h.timer_game_time(), Some(22.0));
    let expiry = h.next_timer_expiry(24.0, &mut 1).unwrap().unwrap();
    assert_eq!(h.native("GetExpiredTimer", &[]).unwrap(), a);
    h.finish_timer_expiry(expiry).unwrap();
    assert!(h.next_timer_expiry(24.0, &mut 0).unwrap().is_none());
    assert_eq!(h.timer_game_time(), Some(24.0));
    assert_eq!(query(&mut h, "TimerGetElapsed", &b), Value::Real(2.0));
    assert_eq!(query(&mut h, "TimerGetRemaining", &b), Value::Real(0.0));
    assert!(h.native("GetExpiredTimer", &[]).is_err());
}

#[test]
fn periodic_callback_queries_and_restart_destroy_use_the_same_timer_entry() {
    let mut h = host();
    let timer = h.native("CreateTimer", &[]).unwrap();
    start(&mut h, &timer, 0.5, true);
    assert!(h.next_timer_expiry(20.25, &mut 0).unwrap().is_none());
    assert_eq!(query(&mut h, "TimerGetElapsed", &timer), Value::Real(0.25));
    let expiry = h.next_timer_expiry(21.0, &mut 1).unwrap().unwrap();
    assert_eq!(query(&mut h, "TimerGetElapsed", &timer), Value::Real(0.5));
    assert_eq!(query(&mut h, "TimerGetRemaining", &timer), Value::Real(0.0));
    start(&mut h, &timer, 0.125, false);
    assert_eq!(query(&mut h, "TimerGetElapsed", &timer), Value::Real(0.0));
    assert_eq!(
        query(&mut h, "TimerGetRemaining", &timer),
        Value::Real(0.125)
    );
    h.finish_timer_expiry(expiry).unwrap();
    let expiry = h.next_timer_expiry(21.0, &mut 1).unwrap().unwrap();
    assert_eq!(h.timer_game_time(), Some(20.625));
    h.native("DestroyTimer", std::slice::from_ref(&timer))
        .unwrap();
    assert!(h.native("GetExpiredTimer", &[]).is_err());
    h.finish_timer_expiry(expiry).unwrap();
    assert!(h.next_timer_expiry(30.0, &mut 0).unwrap().is_none());
    assert_eq!(h.inner().handles(), 1);
    assert!(h.native("TimerGetTimeout", &[timer]).is_err());
}

#[test]
fn invalid_start_time_identity_or_callback_keeps_the_previous_run() {
    let mut h = host();
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(
        h.native("TimerGetElapsed", std::slice::from_ref(&timer))
            .is_err()
    );
    start(&mut h, &timer, 1.0, false);
    for timeout in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            h.native(
                "TimerStart",
                &[
                    timer.clone(),
                    Value::Real(timeout),
                    Value::Bool(false),
                    Value::Null
                ]
            )
            .is_err()
        );
    }
    for callback in [
        Value::Code(pcx_script::value::FunctionId(u32::MAX)),
        Value::Int(3),
    ] {
        assert!(
            h.native(
                "TimerStart",
                &[timer.clone(), Value::Int(2), Value::Bool(false), callback]
            )
            .is_err()
        );
    }
    assert!(
        h.native(
            "TimerStart",
            &[Value::Null, Value::Int(2), Value::Bool(false), Value::Null]
        )
        .is_err()
    );
    let symbolic = h.native("ConvertPlayerColor", &[Value::Int(17)]).unwrap();
    assert!(h.native("TimerGetElapsed", &[symbolic]).is_err());
    for until in [19.0, f64::NAN, f64::INFINITY] {
        assert!(h.next_timer_expiry(until, &mut 1).is_err());
    }
    assert_eq!(h.timer_game_time(), Some(20.0));
    let expiry = h.next_timer_expiry(21.0, &mut 1).unwrap().unwrap();
    h.finish_timer_expiry(expiry).unwrap();
    assert_eq!(query(&mut h, "TimerGetTimeout", &timer), Value::Real(1.0));
}

#[test]
fn runtime_admission_checks_contracts_sources_and_deadline_precision() {
    for declaration in [
        COMMON.replace("native TimerGetTimeout", "constant native TimerGetTimeout"),
        COMMON.replace("code callback", "integer callback"),
        COMMON.replace(
            "native GetExpiredTimer takes nothing",
            "native GetExpiredTimer takes timer t",
        ),
    ] {
        assert!(checked(&declaration, "").with_timer_runtime(0.0).is_err());
    }
    let map = "function TimerGetElapsed takes timer t returns real\nreturn 1.0\nendfunction\n";
    assert!(checked(COMMON, map).with_timer_runtime(0.0).is_err());
    assert!(checked(COMMON, "").with_timer_runtime(f64::NAN).is_err());
    let mut h = checked(COMMON, "").with_timer_runtime(f64::MAX).unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(
        h.native(
            "TimerStart",
            &[timer, Value::Real(1.0), Value::Bool(false), Value::Null]
        )
        .is_err()
    );
    assert!(host().with_timer_runtime(20.0).is_err());
}

#[test]
fn loaded_callback_shape_is_checked_before_replacing_the_handler() {
    let map = "function good takes nothing returns nothing\nendfunction\nfunction bad takes integer n returns nothing\nendfunction\n";
    let mut h = checked(COMMON, map).with_timer_runtime(0.0).unwrap();
    let good = h.inner.program.function_id("good").unwrap();
    let bad = h.inner.program.function_id("bad").unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    h.native(
        "TimerStart",
        &[
            timer.clone(),
            Value::Int(1),
            Value::Bool(false),
            Value::Code(good),
        ],
    )
    .unwrap();
    assert!(
        h.native(
            "TimerStart",
            &[timer, Value::Int(2), Value::Bool(false), Value::Code(bad)]
        )
        .is_err()
    );
    let expiry = h.next_timer_expiry(2.0, &mut 1).unwrap().unwrap();
    assert_eq!(expiry.callback, Some(good));
    h.finish_timer_expiry(expiry).unwrap();
}

#[test]
fn replacement_null_and_repeated_restarts_keep_one_run_per_identity() {
    let map = "function old takes nothing returns nothing\nendfunction\nfunction new takes nothing returns nothing\nendfunction\n";
    let mut h = checked(COMMON, map).with_timer_runtime(0.0).unwrap();
    let old = h.inner.program.function_id("old").unwrap();
    let new = h.inner.program.function_id("new").unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    let destroyed = h.native("CreateTimer", &[]).unwrap();
    for callback in [old, new] {
        h.native(
            "TimerStart",
            &[
                timer.clone(),
                Value::Int(1),
                Value::Bool(false),
                Value::Code(callback),
            ],
        )
        .unwrap();
    }
    let expiry = h.next_timer_expiry(1.0, &mut 1).unwrap().unwrap();
    assert_eq!(expiry.callback, Some(new));
    h.finish_timer_expiry(expiry).unwrap();
    h.native(
        "TimerStart",
        &[
            timer.clone(),
            Value::Int(1),
            Value::Bool(false),
            Value::Code(old),
        ],
    )
    .unwrap();
    for _ in 0..1000 {
        start(&mut h, &timer, 1.0, false);
    }
    start(&mut h, &destroyed, 1.0, false);
    h.native("DestroyTimer", &[destroyed]).unwrap();
    assert_eq!(h.inner().handles(), 2);
    let expiry = h.next_timer_expiry(3.0, &mut 1).unwrap().unwrap();
    assert_eq!(expiry.callback, None);
    h.finish_timer_expiry(expiry).unwrap();
    assert!(h.next_timer_expiry(3.0, &mut 0).unwrap().is_none());
}

#[test]
#[ignore = "requires a caller-selected local common.j; checks declarations, not runtime parity"]
fn selected_common_timer_contract_is_admitted() {
    let path = std::env::var("PEACECRAFT3_COMMON_J").expect("explicit common.j path");
    let common = std::fs::read_to_string(&path).unwrap();
    let common = pcx_script::parse_script(&common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = pcx_script::resolve_with(&map, &[Prelude::new(&path, &common)]).unwrap();
    let host = ObjectHost::checked(
        EnumHost::checked_selected(
            Arc::new(program),
            &path,
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 32,
                admission_steps: 100_000,
                expression_depth: 64,
                expression_work: 100_000,
            },
        )
        .unwrap(),
    )
    .unwrap()
    .with_timer_runtime(17.0)
    .unwrap();
    assert_eq!(host.timer_game_time(), Some(17.0));
}

#[test]
fn ambiguous_timer_start_sources_refuse_runtime_admission() {
    let common = pcx_script::parse_script(COMMON).unwrap();
    let other = pcx_script::parse_script("native TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing\n").unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = pcx_script::resolve_with(
        &map,
        &[
            Prelude::new("synthetic:common", &common),
            Prelude::new("synthetic:other", &other),
        ],
    )
    .unwrap();
    let host = ObjectHost::checked(
        EnumHost::checked_selected(
            Arc::new(program),
            "synthetic:map",
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 16,
                admission_steps: 100,
                expression_depth: 16,
                expression_work: 100,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(host.with_timer_runtime(0.0), Err(error) if error.failure == Failure::Source));
}
