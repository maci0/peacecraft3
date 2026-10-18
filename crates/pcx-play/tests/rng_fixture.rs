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
