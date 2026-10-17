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
