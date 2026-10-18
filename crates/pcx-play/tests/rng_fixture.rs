//! Executes the identical generated JASS fixture in two fresh owned VMs.
//! All selected image data, declarations, fixture and output paths stay local.
#[path = "support/random_local.rs"]
mod random_local;

use pcx_play::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value, Vm};
use std::{fs::OpenOptions, io::Write, sync::Arc};

struct Capture {
    inner: ObjectHost,
    lines: Vec<String>,
}

impl Host for Capture {
    fn real_ops(&self) -> Option<&dyn pcx_script::RealOps> {
        self.inner.real_ops()
    }

    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        match (name, args) {
            // Fixture output encoding only; this does not admit production I2S.
            ("I2S", [Value::Int(value)]) => Ok(Value::Str(value.to_string())),
            ("Preload", [Value::Str(line)]) => {
                self.lines.push(line.clone());
                Ok(Value::Null)
            }
            ("PreloadGenClear" | "PreloadGenStart", []) | ("PreloadGenEnd", [Value::Str(_)]) => {
                Ok(Value::Null)
            }
            _ => self.inner.native(name, args),
        }
    }
}

fn numeric(image: &[u8]) -> pcx_play::script_numeric::NumericPolicy {
    use pcx_play::script_numeric::{Binding, NumericPolicy};
    let selection = random_local::local_bytes("PEACECRAFT3_NUMERIC_BINDING", 16 << 10);
    let (image_sha256, mut windows) = random_local::binding_windows(&selection);
    let mut take = |name| windows.remove(name).expect("numeric role");
    let binding = Binding {
        origin: pcx_play::scripts::AssetIdentity {
            edition: "explicit local selected executable".into(),
            version: "caller-reviewed numeric roles".into(),
            source: "PEACECRAFT3_RANDOM_REAL_IMAGE".into(),
        },
        image_sha256,
        literal: take("literal"),
        division: take("division"),
        reciprocal: take("reciprocal"),
        reciprocal_table: take("reciprocal_table"),
        equality: take("equality"),
        equality_constant: take("equality_constant"),
        inequality: take("inequality"),
        math_sub: take("math_sub"),
        math_add: take("math_add"),
        math_mul: take("math_mul"),
        math_lzcnt: take("math_lzcnt"),
        math_i32_to_real: take("math_i32_to_real"),
    };
    assert!(windows.is_empty());
    NumericPolicy::read(
        image,
        binding,
        pcx_play::neutral_player::RuleInputBudget {
            image_bytes: 64 << 20,
            profile_bytes: 16 << 10,
            sections: 96,
        },
        4096,
    )
    .unwrap()
}

#[test]
#[ignore = "requires selected local image, RNG/numeric bindings, common.j, generated JASS and fresh output directory"]
fn repeat_selected_rng_fixture_in_fresh_owned_vms() {
    let common = random_local::local_bytes("PEACECRAFT3_RANDOM_COMMON", 4 << 20);
    let mut common = pcx_script::parse_script(std::str::from_utf8(&common).unwrap()).unwrap();
    // This isolated fixture uses the supplied declarations, not common's game globals.
    common
        .items
        .retain(|item| !matches!(item, pcx_script::ast::Item::Globals(..)));
    let probe = random_local::local_bytes("PEACECRAFT3_RNG_PROBE", 256 << 10);
    let map = pcx_script::parse_script(std::str::from_utf8(&probe).unwrap()).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[Prelude::new("selected common declarations", &common)],
        )
        .unwrap(),
    );
    let policy = Arc::new(random_local::local_policy());
    let image = random_local::local_bytes("PEACECRAFT3_RANDOM_REAL_IMAGE", 64 << 20);
    let output = std::path::PathBuf::from(std::env::var_os("PEACECRAFT3_RNG_OUTPUT").unwrap());
    let mut observed = Vec::new();
    for index in 0..2 {
        let enums = EnumHost::checked_selected(
            Arc::clone(&program),
            "explicit serial RNG fixture",
            &[],
            Limits {
                max_handles: 0,
                admission_steps: 100_000,
                expression_depth: 64,
                expression_work: 100_000,
            },
        )
        .unwrap();
        let inner = ObjectHost::checked(enums)
            .unwrap()
            .with_script_random(Arc::clone(&policy), 256)
            .unwrap()
            .with_script_numeric(numeric(&image));
        let mut vm = Vm::new_owned(
            Arc::clone(&program),
            Capture {
                inner,
                lines: Vec::new(),
            },
        );
        vm.init_globals().unwrap();
        assert_eq!(vm.call("PCXRngFixture", &[]).unwrap(), Value::Null);
        // Check the extractor independently of the generator's RNG results.
        for value in [0.0f32, -0.0, 1.0, -1.0, f32::MIN_POSITIVE, 0.5, -0.125] {
            assert_eq!(
                vm.call("PCXRngBits", &[Value::Real(value)]).unwrap(),
                Value::Int(value.to_bits() as i32)
            );
        }
        assert!(vm.host().inner.script_random_state().is_some());
        let bytes = (vm.host().lines.join("\n") + "\n").into_bytes();
        assert!(bytes.len() <= 2 << 20);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(format!("run-{index}.txt")))
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        observed.push(bytes);
    }
    assert_eq!(observed[0], observed[1]);
    println!("two fresh selected-policy VM observations are byte-identical");
}

#[test]
#[ignore = "requires selected local image, RNG/numeric bindings and common.j"]
fn selected_rng_timer_trace_is_independent_of_polling_and_cpu_work() {
    use pcx_play::playercolor_host::Failure;
    use pcx_play::script_random::state::State;
    use sha2::{Digest, Sha256};

    // Synthetic conditions, not game defaults or measured original timer ordering.
    let script = r#"
globals
    integer count = 0
    integer kind = 0
    integer result = 0
    real sample = 0.0
endglobals
function Record takes integer which returns nothing
    set count = count + 1
    set kind = which
    set result = GetRandomInt(-9, 13)
    set sample = GetRandomReal(I2R(-1), I2R(1))
endfunction
function A takes nothing returns nothing
    call Record(1)
endfunction
function B takes nothing returns nothing
    call Record(2)
endfunction
function Setup takes integer seed returns nothing
    call SetRandomSeed(seed)
    call TimerStart(CreateTimer(), 0.25, true, function A)
    call TimerStart(CreateTimer(), 0.375, true, function B)
endfunction
"#;
    let common = random_local::local_bytes("PEACECRAFT3_RANDOM_COMMON", 4 << 20);
    let mut common = pcx_script::parse_script(std::str::from_utf8(&common).unwrap()).unwrap();
    common
        .items
        .retain(|item| !matches!(item, pcx_script::ast::Item::Globals(..)));
    let map = pcx_script::parse_script(script).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[Prelude::new("selected common declarations", &common)],
        )
        .unwrap(),
    );
    let policy = Arc::new(random_local::local_policy());
    let image = random_local::local_bytes("PEACECRAFT3_RANDOM_REAL_IMAGE", 64 << 20);
    let run = |polls: &[u32], quantum: usize, work: u64, seed: i32| {
        let enums = EnumHost::checked_selected(
            Arc::clone(&program),
            "synthetic:deterministic-timers",
            &[],
            Limits {
                max_handles: 4,
                admission_steps: 100_000,
                expression_depth: 64,
                expression_work: 100_000,
            },
        )
        .unwrap();
        let host = ObjectHost::checked(enums)
            .unwrap()
            .with_script_random(Arc::clone(&policy), 256)
            .unwrap()
            .with_script_numeric(numeric(&image))
            .with_timer_runtime(0.0)
            .unwrap();
        let mut vm = Vm::new_owned(Arc::clone(&program), host);
        vm.call("Setup", &[Value::Int(seed)]).unwrap();
        let mut trace: Vec<(u64, i32, i32, u32, State)> = Vec::new();
        let mut refusals = 0;
        let mut work_done = 0u64;
        for &tick in polls {
            let until = f64::from(tick) / 8.0;
            let mut budget = quantum;
            loop {
                // Deterministic work varies pressure without advancing game time.
                let mut noise = work_done;
                for n in 0..work {
                    noise = std::hint::black_box(noise.wrapping_add(n).rotate_left(3));
                }
                std::hint::black_box(noise);
                work_done += work;
                let before = vm.host().script_random_state().unwrap().clone();
                let expiry = match vm.host().next_timer_expiry(until, &mut budget) {
                    Ok(Some(expiry)) => expiry,
                    Ok(None) => break,
                    Err(error) => {
                        assert_eq!(error.failure, Failure::Budget("timer delivery"));
                        assert_eq!(vm.host().script_random_state(), Some(&before));
                        refusals += 1;
                        budget = quantum;
                        continue;
                    }
                };
                assert!(expiry.triggers().is_empty());
                let callback = expiry.callback.unwrap();
                let name = &program.functions()[callback.0 as usize].name;
                let result = vm.call(name, &[]);
                vm.host().finish_timer_expiry(expiry).unwrap();
                assert_eq!(result.unwrap(), Value::Null);
                let Value::Int(kind) = vm.global("kind") else {
                    panic!("kind")
                };
                let Value::Int(integer) = vm.global("result") else {
                    panic!("integer")
                };
                let Value::Real(real) = vm.global("sample") else {
                    panic!("real")
                };
                trace.push((
                    vm.host().timer_game_time().unwrap().to_bits(),
                    kind,
                    integer,
                    real.to_bits(),
                    vm.host().script_random_state().unwrap().clone(),
                ));
                assert_eq!(vm.global("count"), Value::Int(trace.len() as i32));
            }
            assert_eq!(vm.host().timer_game_time(), Some(until));
        }
        assert_eq!(vm.host().timer_game_time(), Some(3.0));
        assert_eq!(trace.len(), 20);
        (trace, refusals, work_done)
    };
    let fine: Vec<_> = (0..=24).collect();
    let (baseline, _, baseline_work) = run(&fine, 64, 0, 37);
    let (loaded, refusals, loaded_work) = run(&[0, 6, 6, 24], 1, 32_768, 37);
    assert!(refusals > 0);
    assert!(loaded_work > baseline_work);
    assert_eq!(baseline, loaded);
    assert_eq!(baseline, run(&[24], 3, 1024, 37).0);
    assert_ne!(baseline, run(&fine, 64, 0, 38).0);
    let digest: String = Sha256::digest(format!("{baseline:?}").as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    println!(
        "DST: 20 identical exact callback/RNG states across 3 polling/work schedules; {refusals} budget boundaries; {loaded_work} injected work iterations; trace {digest}"
    );
}
