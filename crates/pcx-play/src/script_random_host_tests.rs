use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use crate::script_random::profile;
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

const BASE: &str = "type agent extends handle\ntype timer extends agent\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\n";
const RANDOM: &str = "native GetRandomInt takes integer low, integer high returns integer\nnative GetRandomReal takes real low, real high returns real\nnative SetRandomSeed takes integer seed returns nothing\n";
fn host(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(&format!("{BASE}{common}")).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic common", &common)]).unwrap(),
    );
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "synthetic RNG map",
            &[],
            Limits {
                max_handles: 2,
                admission_steps: 100,
                expression_depth: 16,
                expression_work: 100,
            },
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn owned_vm_executes_seed_and_reseed_without_allocating_handles() {
    let host = host(
        RANDOM,
        "globals\ninteger first = 0\ninteger again = 0\nreal sampled = 0.0\nendglobals\nfunction main takes nothing returns nothing\ncall SetRandomSeed(-7)\nset first = GetRandomInt(-20, 30)\nset sampled = GetRandomReal(-1.0, 2.0)\ncall SetRandomSeed(-7)\nset again = GetRandomInt(-20, 30)\nendfunction\n",
    );
    let mut vm = host
        .with_script_random(Arc::new(profile::fixture()), 3)
        .unwrap()
        .into_vm();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("first"), vm.global("again"));
    assert!(matches!(vm.global("sampled"), Value::Real(_)));
    assert_eq!(vm.host().inner().handles(), 0);
    assert_eq!(
        vm.host()
            .script_random_state()
            .unwrap()
            .secondary_words()
            .len(),
        3
    );
}

#[test]
fn admission_requires_complete_exact_declarations_and_explicit_budget() {
    let policy = Arc::new(profile::fixture());
    assert!(
        host(RANDOM, "")
            .native("SetRandomSeed", &[Value::Int(1)])
            .is_err()
    );
    assert!(
        host("", "")
            .with_script_random(Arc::clone(&policy), 3)
            .is_err()
    );
    assert!(
        host(
            &RANDOM.replace("native GetRandomInt", "constant native GetRandomInt"),
            ""
        )
        .with_script_random(Arc::clone(&policy), 3)
        .is_err()
    );
    assert!(
        host(&RANDOM.replace("integer seed", "real seed"), "")
            .with_script_random(Arc::clone(&policy), 3)
            .is_err()
    );
    assert!(host(RANDOM, "").with_script_random(policy, 2).is_err());
}

#[test]
fn failed_calls_do_not_consume_existing_seed_and_new_host_has_no_seed() {
    let policy = Arc::new(profile::fixture());
    let mut host = host(RANDOM, "")
        .with_script_random(Arc::clone(&policy), 3)
        .unwrap();
    assert_eq!(
        host.native("GetRandomInt", &[Value::Int(8), Value::Int(8)])
            .unwrap(),
        Value::Int(8)
    );
    let error = host
        .native("GetRandomInt", &[Value::Int(0), Value::Int(1)])
        .unwrap_err();
    assert!(error.contains("synthetic common") && error.contains("explicit seed"));
    host.native("SetRandomSeed", &[Value::Int(1)]).unwrap();
    let before = host.script_random_state().cloned();
    assert!(host.native("SetRandomSeed", &[Value::Real(1.0)]).is_err());
    assert!(
        host.native("GetRandomReal", &[Value::Real(f32::NAN), Value::Real(1.0)])
            .is_err()
    );
    assert_eq!(host.script_random_state().cloned(), before);
    assert!(
        self::host(RANDOM, "")
            .with_script_random(policy, 3)
            .unwrap()
            .script_random_state()
            .is_none()
    );
}

#[test]
fn duplicate_or_split_declaration_sources_are_rejected() {
    let base = pcx_script::parse_script(BASE).unwrap();
    let first = pcx_script::parse_script(RANDOM).unwrap();
    let second =
        pcx_script::parse_script("native SetRandomSeed takes integer seed returns nothing\n")
            .unwrap();
    let map = pcx_script::parse_script("").unwrap();
    for extra in [&second, &first] {
        let program = Arc::new(
            pcx_script::resolve_with(
                &map,
                &[
                    Prelude::new("base", &base),
                    Prelude::new("first", &first),
                    Prelude::new("second", extra),
                ],
            )
            .unwrap(),
        );
        let enums = EnumHost::checked_selected(
            program,
            "duplicate RNG",
            &[],
            Limits {
                max_handles: 0,
                admission_steps: 100,
                expression_depth: 8,
                expression_work: 100,
            },
        )
        .unwrap();
        assert!(
            ObjectHost::checked(enums)
                .unwrap()
                .with_script_random(Arc::new(profile::fixture()), 3)
                .is_err()
        );
    }
    let first=pcx_script::parse_script("native GetRandomInt takes integer low, integer high returns integer\nnative GetRandomReal takes real low, real high returns real\n").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("base", &base),
                Prelude::new("first", &first),
                Prelude::new("second", &second),
            ],
        )
        .unwrap(),
    );
    let enums = EnumHost::checked_selected(
        program,
        "split RNG",
        &[],
        Limits {
            max_handles: 0,
            admission_steps: 100,
            expression_depth: 8,
            expression_work: 100,
        },
    )
    .unwrap();
    assert!(
        ObjectHost::checked(enums)
            .unwrap()
            .with_script_random(Arc::new(profile::fixture()), 3)
            .is_err()
    );
}

#[test]
fn identical_seeds_and_call_order_reproduce_outputs_and_rng_state() {
    use crate::script_random::order::CallSite;
    const MAP: &str = "globals\ninteger a = 0\ninteger b = 0\ninteger c = 0\nreal x = 0.0\nreal y = 0.0\nendglobals\nfunction main takes nothing returns nothing\ncall SetRandomSeed(12345)\nset a = GetRandomInt(1, 100)\nset x = GetRandomReal(0.0, 1.0)\nset b = GetRandomInt(1, 100)\ncall SetRandomSeed(12345)\nset c = GetRandomInt(1, 100)\nset y = GetRandomReal(0.0, 1.0)\nendfunction\n";
    fn run_once() -> (Vec<Value>, u32, Vec<u32>, Vec<CallSite>) {
        let vm_host = host(RANDOM, MAP)
            .with_script_random(Arc::new(profile::fixture()), 3)
            .unwrap();
        let mut vm = vm_host.into_vm();
        vm.init_globals().unwrap();
        vm.call("main", &[]).unwrap();
        let globals = ["a", "b", "c", "x", "y"]
            .into_iter()
            .map(|name| vm.global(name))
            .collect::<Vec<_>>();
        let host = vm.host();
        let state = host.script_random_state().expect("seeded stream");
        let words = state.secondary_words().collect::<Vec<_>>();
        let sites = host
            .script_random_calls()
            .expect("caller sequence")
            .iter()
            .map(|record| record.site.clone())
            .collect::<Vec<_>>();
        (globals, state.primary_word(), words, sites)
    }
    let first = run_once();
    let second = run_once();
    // Same ticks (one `main` call), same input (same seed calls in the same
    // order): exactly the same outputs and exactly the same RNG words.
    assert_eq!(first.0, second.0, "script outputs must match");
    assert_eq!(first.1, second.1, "primary words must match");
    assert_eq!(first.2, second.2, "secondary words must match");
    assert_eq!(first.3, second.3, "caller order must match");
    // Reseeding replays the stream: the post-reseed integer repeats the
    // pre-reseed one, and the reseed clears the caller log alongside the
    // generator state, so only the post-reseed pair remains recorded.
    assert_eq!(first.0[0], first.0[2], "reseed must replay the stream");
    assert_eq!(
        first.3,
        [CallSite::ScriptInteger, CallSite::ScriptReal,],
        "caller order is part of the reproducible state"
    );
    assert!(second.3.len() == 2);
}

#[test]
fn interleaved_secondary_real_and_die_retain_distinct_exact_state_records() {
    use crate::script_random::{
        order::{CallSite, DrawRecord},
        state,
    };
    let policy = Arc::new(profile::fixture());
    let mut host = host(RANDOM, "")
        .with_script_random(Arc::clone(&policy), 3)
        .unwrap();
    assert!(host.draw_random_secondary_real(1, "orb").is_err());
    assert_eq!(host.script_random_calls().unwrap(), []);
    host.native("SetRandomSeed", &[Value::Int(-71)]).unwrap();
    let mut expected = Some(state::seeded(&policy, -71, 3).unwrap());
    let mut records = Vec::new();
    for (site, channel) in [
        (CallSite::SecondaryReal { channel: 1 }, Some(1)),
        (CallSite::ScriptInteger, None),
        (CallSite::SecondaryDie { channel: 1 }, Some(1)),
        (CallSite::SecondaryReal { channel: 2 }, Some(2)),
        (CallSite::ScriptReal, None),
        (CallSite::SecondaryReal { channel: 1 }, Some(1)),
    ] {
        match site {
            CallSite::SecondaryReal { channel } => assert_eq!(
                host.draw_random_secondary_real(channel, "orb")
                    .unwrap()
                    .to_bits(),
                state::secondary_real(&policy, &mut expected, channel)
                    .unwrap()
                    .to_bits(),
            ),
            CallSite::SecondaryDie { channel } => assert_eq!(
                host.draw_random_secondary_die(channel, 1, "illusion")
                    .unwrap(),
                state::secondary_die(&policy, &mut expected, channel, 1).unwrap(),
            ),
            CallSite::ScriptInteger => assert_eq!(
                host.native("GetRandomInt", &[Value::Int(-19), Value::Int(41)])
                    .unwrap(),
                Value::Int(state::integer(&policy, &mut expected, -19, 41).unwrap()),
            ),
            CallSite::ScriptReal => assert_eq!(
                host.native("GetRandomReal", &[Value::Real(-2.0), Value::Real(7.0)])
                    .unwrap(),
                Value::Real(state::real(&policy, &mut expected, -2.0, 7.0).unwrap()),
            ),
            _ => unreachable!(),
        }
        let state = expected.as_ref().unwrap();
        records.push(DrawRecord {
            site,
            primary_word: Some(state.primary_word()),
            secondary_word: channel.and_then(|channel| state.secondary_words().nth(channel)),
        });
        assert_eq!(host.script_random_state(), expected.as_ref());
        assert_eq!(host.script_random_calls().unwrap(), records);
        assert!(host.draw_random_secondary_real(3, "orb").is_err());
        assert!(host.draw_random_secondary_die(1, 0, "illusion").is_err());
        assert_eq!(host.script_random_state(), expected.as_ref());
        assert_eq!(host.script_random_calls().unwrap(), records);
    }
    host.native("SetRandomSeed", &[Value::Int(-71)]).unwrap();
    assert_eq!(host.script_random_calls().unwrap(), []);
    assert_eq!(
        host.script_random_state(),
        Some(&state::seeded(&policy, -71, 3).unwrap())
    );
}
