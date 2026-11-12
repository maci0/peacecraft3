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
                max_handles: 32,
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
fn fresh_timer_queries_are_zero_without_starting_or_advancing_the_timer() {
    let mut h = host();
    let timer = h.native("CreateTimer", &[]).unwrap();
    for name in ["TimerGetElapsed", "TimerGetRemaining", "TimerGetTimeout"] {
        assert_eq!(query(&mut h, name, &timer), Value::Real(0.0));
        assert!(h.native(name, &[Value::Null]).is_err());
        assert!(h.native(name, &[]).is_err());
    }
    assert_eq!(h.timer_game_time(), Some(20.0));
    assert!(h.next_timer_expiry(23.0, &mut 0).unwrap().is_none());
    assert_eq!(h.timer_game_time(), Some(23.0));
    assert_eq!(query(&mut h, "TimerGetRemaining", &timer), Value::Real(0.0));
    start(&mut h, &timer, 0.75, false);
    assert_eq!(query(&mut h, "TimerGetTimeout", &timer), Value::Real(0.75));
    h.native("DestroyTimer", std::slice::from_ref(&timer))
        .unwrap();
    for name in ["TimerGetElapsed", "TimerGetRemaining", "TimerGetTimeout"] {
        assert!(h.native(name, std::slice::from_ref(&timer)).is_err());
    }
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
    assert_eq!(query(&mut h, "TimerGetElapsed", &timer), Value::Real(0.0));
    start(&mut h, &timer, 1.0, false);
    for (timeout, periodic) in [
        (0.0, true),
        (-1.0, false),
        (f32::NAN, false),
        (f32::INFINITY, false),
    ] {
        assert!(
            h.native(
                "TimerStart",
                &[
                    timer.clone(),
                    Value::Real(timeout),
                    Value::Bool(periodic),
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
#[test]
fn zero_one_shot_timeout_expires_at_the_current_boundary() {
    let mut h = host();
    let timer = h.native("CreateTimer", &[]).unwrap();
    start(&mut h, &timer, 0.0, false);
    assert_eq!(query(&mut h, "TimerGetRemaining", &timer), Value::Real(0.0));
    let mut budget = 4;
    assert!(h.next_timer_expiry(20.0, &mut budget).unwrap().is_some());
    assert_eq!(h.native("GetExpiredTimer", &[]).unwrap(), timer);
}

/// One delivery to the same final game time, split at different boundaries.
/// The boundaries are dyadic, as are the timeouts, so the deadline sum does not
/// depend on how many times `now` is published between firings.
#[test]
fn equal_final_game_time_is_independent_of_until_chunking() {
    let schedules: [&[f64]; 3] = [
        &[24.0],
        &[21.0, 21.5, 22.25, 24.0],
        &[20.5, 21.25, 23.0, 24.0],
    ];
    let runs = schedules.map(drive_until);
    let expected = runs[0].clone();
    for run in &runs[1..] {
        assert_eq!(run, &expected);
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TimerSample {
    firings: Vec<Firing>,
    now_bits: u64,
    first: (u32, u32, u32),
    second: (u32, u32, u32),
}

#[derive(Clone, Debug, PartialEq)]
struct Firing {
    timer: usize,
    now_bits: u64,
    first_elapsed_bits: u32,
    second_elapsed_bits: u32,
}

fn drive_until(limits: &[f64]) -> TimerSample {
    let mut host = host();
    let first = host.native("CreateTimer", &[]).unwrap();
    let second = host.native("CreateTimer", &[]).unwrap();
    start(&mut host, &first, 1.0, true);
    start(&mut host, &second, 1.5, false);
    let timers = [first, second];
    let mut firings = Vec::new();
    for until in limits {
        loop {
            let mut budget = 1;
            let Some(expiry) = host.next_timer_expiry(*until, &mut budget).unwrap() else {
                break;
            };
            let expired = host.native("GetExpiredTimer", &[]).unwrap();
            let timer = timers.iter().position(|known| known == &expired).unwrap();
            firings.push(Firing {
                timer,
                now_bits: host.timer_game_time().unwrap().to_bits(),
                first_elapsed_bits: real_bits(&query(&mut host, "TimerGetElapsed", &timers[0])),
                second_elapsed_bits: real_bits(&query(&mut host, "TimerGetElapsed", &timers[1])),
            });
            host.finish_timer_expiry(expiry).unwrap();
        }
    }
    TimerSample {
        firings,
        now_bits: host.timer_game_time().unwrap().to_bits(),
        first: timer_bits(&mut host, &timers[0]),
        second: timer_bits(&mut host, &timers[1]),
    }
}

fn timer_bits(host: &mut ObjectHost, timer: &Value) -> (u32, u32, u32) {
    (
        real_bits(&query(host, "TimerGetElapsed", timer)),
        real_bits(&query(host, "TimerGetRemaining", timer)),
        real_bits(&query(host, "TimerGetTimeout", timer)),
    )
}

fn real_bits(value: &Value) -> u32 {
    match value {
        Value::Real(real) => real.to_bits(),
        other => panic!("timer query returned {other}"),
    }
}

#[test]
fn native_query_policy_preserves_pause_cache_restart_and_refusal_atomicity() {
    let common = format!(
        "{COMMON}native PauseTimer takes timer t returns nothing\nnative ResumeTimer takes timer t returns nothing\n"
    );
    let mut h = checked(&common, "")
        .with_script_numeric(crate::script_numeric::timer_query::tests::policy(8, 0.125))
        .with_timer_runtime(1.0)
        .unwrap();
    let timer = h.native("CreateTimer", &[]).unwrap();
    start(&mut h, &timer, 16.0, false);
    assert!(h.next_timer_expiry(1.5, &mut 0).unwrap().is_none());
    h.native("PauseTimer", std::slice::from_ref(&timer))
        .unwrap();
    assert_eq!(
        query(&mut h, "TimerGetRemaining", &timer),
        Value::Real(15.5)
    );
    assert!(h.next_timer_expiry(2.0, &mut 0).unwrap().is_none());
    h.native("ResumeTimer", std::slice::from_ref(&timer))
        .unwrap();
    assert_eq!(query(&mut h, "TimerGetElapsed", &timer), Value::Real(0.0));
    assert_eq!(query(&mut h, "TimerGetTimeout", &timer), Value::Real(16.0));
    start(&mut h, &timer, 0.25, false);
    let before = ["TimerGetElapsed", "TimerGetRemaining", "TimerGetTimeout"]
        .map(|name| query(&mut h, name, &timer));
    let handles = h.inner().handles();
    assert!(
        h.native(
            "TimerStart",
            &[
                timer.clone(),
                Value::Real(8.0 * 65536.0),
                Value::Bool(false),
                Value::Null
            ]
        )
        .is_err()
    );
    assert_eq!(h.inner().handles(), handles);
    assert_eq!(h.timer_game_time(), Some(2.0));
    assert_eq!(
        ["TimerGetElapsed", "TimerGetRemaining", "TimerGetTimeout"]
            .map(|name| query(&mut h, name, &timer)),
        before
    );
    let expiry = h.next_timer_expiry(3.0, &mut 1).unwrap().unwrap();
    assert_eq!(
        query(&mut h, "TimerGetElapsed", &timer),
        Value::Real(-15.25)
    );
    assert_eq!(
        query(&mut h, "TimerGetRemaining", &timer),
        Value::Real(15.5)
    );
    h.finish_timer_expiry(expiry).unwrap();
    assert_eq!(
        query(&mut h, "TimerGetElapsed", &timer),
        Value::Real(-15.25)
    );
}

#[test]
#[ignore = "requires explicit local selected image and replicated native query witness"]
fn selected_native_timer_query_bits_match_public_lifecycle() {
    use crate::{
        camera_margins::SourceWindow,
        neutral_player::{RuleInputBudget, sha256},
        neutral_rule_input::Pe,
        script_numeric::{Binding, NumericPolicy, timer_query},
        scripts::AssetIdentity,
    };
    use pcx_script::RealOps;
    use std::io::Read;
    fn bounded(path: impl AsRef<std::path::Path>, limit: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .unwrap()
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() as u64 <= limit, "local input exceeds budget");
        bytes
    }

    let image = bounded(
        std::env::var("PEACECRAFT3_QUERY_IMAGE").expect("explicit local image"),
        32 << 20,
    );
    let witness: serde_json::Value = serde_json::from_slice(&bounded(
        std::env::var("PEACECRAFT3_QUERY_WITNESS").expect("explicit replicated witness"),
        2 << 20,
    ))
    .unwrap();
    let image_sha256 = sha256(&image);
    assert_eq!(image_sha256, crate::menu_lighting::REVIEWED_IMAGE);
    assert_eq!(
        witness["source_sha256"],
        image_sha256
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    assert_eq!(witness["identical_replicates"], true);
    let budget = RuleInputBudget {
        image_bytes: 32 << 20,
        profile_bytes: 64 << 10,
        sections: 96,
    };
    let pe = Pe::parse(&image, budget).unwrap();
    let window = |va, len, code| {
        let bytes = pe.read_from(va, code).unwrap().get(..len as usize).unwrap();
        SourceWindow {
            va,
            len,
            digest: sha256(bytes),
        }
    };
    let numeric = NumericPolicy::read(
        &image,
        Binding {
            origin: AssetIdentity {
                edition: "Legacy".into(),
                version: "1.29.2.9231".into(),
                source: "explicit selected timer query witness".into(),
            },
            image_sha256,
            literal: window(0x8df660, 238, true),
            division: window(0x9a02d0, 57, true),
            reciprocal: window(0x9a1d00, 103, true),
            reciprocal_table: window(0xed6cf0, 4100, false),
            equality: window(0x8d8d10, 53, true),
            equality_constant: window(0x10db054, 4, false),
            inequality: window(0x8d7e32, 162, true),
            math_sub: window(0x9a00b0, 264, true),
            math_add: window(0x9a01c0, 262, true),
            math_mul: crate::campaign_client::numeric_multiplication(&image).unwrap(),
            math_lzcnt: window(0x9a0330, 29, true),
            math_i32_to_real: window(0x9a1620, 92, true),
        },
        budget,
        4096,
    )
    .unwrap()
    .with_timer_queries(
        &image,
        &timer_query::Binding {
            image_sha256,
            segment_initializer: window(0x40d210, 16, true),
            zero_initializer: window(0x401300, 16, true),
            constructor: window(0x4fe380, 0xd1, true),
            start: window(0x7fb820, 0x118, true),
            schedule: window(0x7fb940, 0x5c, true),
            remainder: window(0x9a1a90, 0x142, true),
            remaining: window(0x7fc0b0, 0x104, true),
            full_timeout: window(0x7fbad0, 0xbe, true),
            active_elapsed: window(0x7fbb90, 0x34, true),
            inactive_elapsed: window(0x4fe470, 0x6f, true),
        },
        budget,
    )
    .unwrap();
    let durations = ["2000.0", "240.47", "121.03", "0.47"].map(|s| numeric.literal(s).unwrap());
    let observations = [
        "0.02", "0.47", "1.03", "1.25", "2.0317", "119.97", "120.0", "120.02", "120.0317",
        "121.03", "121.05", "122.06", "122.1",
    ]
    .map(|s| numeric.literal(s).unwrap());
    let short = numeric.literal("0.0317").unwrap();
    let common = format!(
        "{COMMON}native PauseTimer takes timer t returns nothing\nnative ResumeTimer takes timer t returns nothing\n"
    );
    let mut h = checked(&common, "")
        .with_script_numeric(numeric)
        .with_timer_runtime(0.0)
        .unwrap();
    let kickoff = h.native("CreateTimer", &[]).unwrap();
    start(&mut h, &kickoff, 1.0, false);
    let kickoff_expiry = h.next_timer_expiry(1.0, &mut 1).unwrap().unwrap();
    let timers = durations.map(|duration| {
        let t = h.native("CreateTimer", &[]).unwrap();
        start(&mut h, &t, duration, false);
        t
    });
    let observers = observations.map(|duration| {
        let t = h.native("CreateTimer", &[]).unwrap();
        start(&mut h, &t, duration, false);
        t
    });
    let mut checked_values = 0;
    let mut snapshot = |h: &mut ObjectHost, point: &str| {
        for (i, timer) in timers.iter().enumerate() {
            let row = witness["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["point"] == point && r["timer"] == i)
                .expect("complete witnessed snapshot");
            for (name, key) in [
                ("TimerGetElapsed", "elapsed_bits"),
                ("TimerGetRemaining", "remaining_bits"),
                ("TimerGetTimeout", "timeout_bits"),
            ] {
                let value = real_bits(&query(h, name, timer));
                assert_eq!(
                    u64::from(value),
                    row[key].as_u64().unwrap(),
                    "{point} timer{i} {name}"
                );
                checked_values += 1;
            }
        }
    };
    snapshot(&mut h, "arm");
    h.finish_timer_expiry(kickoff_expiry).unwrap();
    let mut observed = 0;
    while let Some(expiry) = h.next_timer_expiry(124.0, &mut 100).unwrap() {
        let expired = h.native("GetExpiredTimer", &[]).unwrap();
        if let Some(i) = observers.iter().position(|timer| *timer == expired) {
            assert_eq!(i, observed);
            snapshot(&mut h, &format!("before_{i}"));
            match i {
                0 => {
                    for j in [0, 1, 3] {
                        h.native("PauseTimer", std::slice::from_ref(&timers[j]))
                            .unwrap();
                    }
                    snapshot(&mut h, "after_pause");
                }
                1 => {
                    for j in [0, 1, 3] {
                        h.native("ResumeTimer", std::slice::from_ref(&timers[j]))
                            .unwrap();
                    }
                    snapshot(&mut h, "after_resume");
                }
                2 => {
                    start(&mut h, &timers[0], durations[0], false);
                    start(&mut h, &timers[1], durations[2], false);
                    snapshot(&mut h, "after_restart");
                }
                3 => {
                    start(&mut h, &timers[3], short, false);
                    snapshot(&mut h, "after_short_restart");
                }
                _ => {}
            }
            observed += 1;
        }
        h.finish_timer_expiry(expiry).unwrap();
    }
    assert_eq!(observed, 13);
    assert_eq!(checked_values, 216);
    assert_eq!(
        witness["rows"].as_array().unwrap().len() * 3,
        checked_values
    );
    assert_eq!(h.timer_game_time(), Some(124.0));
}

#[test]
fn timer_query_policy_changes_refuse_without_rearming_existing_runs() {
    let common = format!(
        "{COMMON}native PauseTimer takes timer t returns nothing\nnative ResumeTimer takes timer t returns nothing\n"
    );
    let mut h = checked(&common, "")
        .with_script_numeric(crate::script_numeric::timer_query::tests::policy(8, 0.125))
        .with_timer_runtime(1.0)
        .unwrap();
    let t = h.native("CreateTimer", &[]).unwrap();
    start(&mut h, &t, 1.0, false);
    let deadline = h.next_timer_deadline();
    h = h.with_script_numeric(crate::script_numeric::timer_query::tests::policy(16, 0.125));
    for name in [
        "TimerGetElapsed",
        "TimerGetRemaining",
        "TimerGetTimeout",
        "PauseTimer",
    ] {
        assert!(h.native(name, std::slice::from_ref(&t)).is_err());
    }
    assert!(
        h.native(
            "TimerStart",
            &[t.clone(), Value::Real(2.0), Value::Bool(false), Value::Null]
        )
        .is_err()
    );
    assert_eq!(h.next_timer_deadline(), deadline);
    assert_eq!(h.timer_game_time(), Some(1.0));
    assert!(h.next_timer_expiry(2.0, &mut 1).is_err());
    assert_eq!(h.timer_game_time(), Some(1.0));
    h = h.with_script_numeric(crate::script_numeric::timer_query::tests::policy(8, 0.125));
    assert_eq!(query(&mut h, "TimerGetRemaining", &t), Value::Real(1.0));
    assert_eq!(h.next_timer_deadline(), Some(2.0));
    let mut legacy = host();
    let t = legacy.native("CreateTimer", &[]).unwrap();
    start(&mut legacy, &t, 1.0, false);
    legacy =
        legacy.with_script_numeric(crate::script_numeric::timer_query::tests::policy(8, 0.125));
    assert!(
        legacy
            .native("TimerGetElapsed", std::slice::from_ref(&t))
            .is_err()
    );
    assert!(
        legacy
            .native(
                "TimerStart",
                &[t, Value::Real(2.0), Value::Bool(false), Value::Null]
            )
            .is_err()
    );
    assert_eq!(legacy.next_timer_deadline(), Some(21.0));
}
