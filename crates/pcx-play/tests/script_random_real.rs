//! Explicit local selected-code comparison; reference values are never distributed.
use pcx_play::script_random::real::{self, MathError};
use sha2::{Digest, Sha256};
#[path = "support/random_local.rs"]
mod random_local;
use random_local::{local_bytes, local_policy};

#[test]
#[ignore = "requires explicit qualified local image and selected-code reference TSV"]
fn selected_code_normal_real_reference() {
    let input = local_bytes("PEACECRAFT3_RANDOM_REAL_REFERENCE", 2 << 20);
    let text = std::str::from_utf8(&input).expect("reference UTF-8");
    let mut lines = text.lines();
    let header: Vec<_> = lines
        .next()
        .expect("reference header")
        .split('\t')
        .collect();
    assert_eq!(header.len(), 2);
    assert_eq!(header[0], "image_sha256");
    let image = local_bytes("PEACECRAFT3_RANDOM_REAL_IMAGE", 64 << 20);
    let digest: String = Sha256::digest(&image)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        digest, header[1],
        "reference must bind the current complete image"
    );
    let mut counts = [0usize; 4];
    for (index, line) in lines.enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4, "case {index}");
        let word = |s| u32::from_str_radix(s, 16).expect("reference word");
        let left = word(fields[1]);
        let right = word(fields[2]);
        let (kind, actual) = match fields[0] {
            "add" => (0, real::add(left, right)),
            "sub" => (1, real::sub(left, right)),
            "mul" => (2, real::mul(left, right)),
            "convert" => (3, real::from_i32(left as i32)),
            _ => panic!("case {index}: unsupported reference operation"),
        };
        let expected = if fields[3] == "unsupported" {
            Err(MathError::Unsupported)
        } else {
            Ok(word(fields[3]))
        };
        assert_eq!(actual, expected, "case {index}: {line}");
        counts[kind] += 1;
    }
    assert!(counts.iter().all(|count| *count != 0));
    println!("selected-code arithmetic cases: {counts:?}");
}

#[test]
#[ignore = "requires explicit qualified local image and reviewed twelve-role binding"]
fn selected_code_profile() {
    let policy = local_policy();
    let state = pcx_play::script_random::state::seeded(&policy, -1, 256).unwrap();
    assert!(state.secondary_words().len() > 0);
    assert!(pcx_play::script_random::state::seeded(&policy, -1, 0).is_err());
}

#[test]
#[ignore = "requires explicit local image, role binding and selected-code state reference"]
fn selected_code_state_through_object_host() {
    use pcx_script::{Host, Value};
    use std::sync::Arc;
    let policy = Arc::new(local_policy());
    let mut host = local_host(Arc::clone(&policy));
    assert!(host.script_random_state().is_none());
    assert!(
        host.native("GetRandomInt", &[Value::Int(0), Value::Int(1)])
            .is_err()
    );
    let input = local_bytes("PEACECRAFT3_RANDOM_STATE_REFERENCE", 2 << 20);
    let text = std::str::from_utf8(&input).unwrap();
    let mut lines = text.lines();
    let hex = |text| u32::from_str_radix(text, 16).unwrap();
    let digest: String = policy
        .image_sha256()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(lines.next().unwrap(), format!("image_sha256\t{digest}"));
    let mut counts = [0; 4];
    for (index, line) in lines.enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 5);
        let (a, b, expected, word) = (
            hex(fields[1]),
            hex(fields[2]),
            hex(fields[3]),
            hex(fields[4]),
        );
        let actual = match fields[0] {
            "seed" => {
                counts[0] += 1;
                assert_eq!(
                    host.native("SetRandomSeed", &[Value::Int(a as i32)])
                        .unwrap(),
                    Value::Null
                );
                0
            }
            "secondary" => {
                counts[1] += 1;
                host.script_random_state()
                    .unwrap()
                    .secondary_words()
                    .nth(a as usize)
                    .unwrap()
            }
            "integer" => {
                counts[2] += 1;
                let Value::Int(value) = host
                    .native(
                        "GetRandomInt",
                        &[Value::Int(a as i32), Value::Int(b as i32)],
                    )
                    .unwrap()
                else {
                    panic!("integer result")
                };
                value as u32
            }
            "real" => {
                counts[3] += 1;
                let Value::Real(value) = host
                    .native(
                        "GetRandomReal",
                        &[
                            Value::Real(f32::from_bits(a)),
                            Value::Real(f32::from_bits(b)),
                        ],
                    )
                    .unwrap()
                else {
                    panic!("real result")
                };
                value.to_bits()
            }
            _ => panic!("unknown reference operation"),
        };
        assert_eq!(actual, expected, "case {index}: {line}");
        assert_eq!(
            host.script_random_state().unwrap().primary_word(),
            word,
            "state after case {index}"
        );
        assert_eq!(
            host.inner().handles(),
            0,
            "RNG must not allocate identities"
        );
    }
    assert!(counts.iter().all(|n| *n > 0));
    println!("selected-code Host checkpoints: {counts:?}");
}

fn local_host(
    policy: std::sync::Arc<pcx_play::script_random::profile::Policy>,
) -> pcx_play::playercolor_host::ObjectHost {
    use pcx_play::playercolor_host::{EnumHost, Limits, ObjectHost};
    use pcx_script::Prelude;
    use std::sync::Arc;
    let common = local_bytes("PEACECRAFT3_RANDOM_COMMON", 4 << 20);
    let common = pcx_script::parse_script(std::str::from_utf8(&common).unwrap()).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("explicit local common.j", &common)])
            .unwrap(),
    );
    let enums = EnumHost::checked_selected(
        program,
        "local RNG comparison",
        &[],
        Limits {
            max_handles: 0,
            admission_steps: 100_000,
            expression_depth: 32,
            expression_work: 100_000,
        },
    )
    .unwrap();
    ObjectHost::checked(enums)
        .unwrap()
        .with_script_random(policy, 256)
        .unwrap()
}

#[test]
#[ignore = "requires retained original runtime checkpoint TSV, local image, binding and common.j"]
fn retained_original_runtime_checkpoints() {
    use pcx_script::{Host, Value};
    let mut host = local_host(std::sync::Arc::new(local_policy()));
    let bytes = local_bytes("PEACECRAFT3_RANDOM_OBSERVATIONS", 2 << 20);
    let text = std::str::from_utf8(&bytes).unwrap();
    let mut lines = text.lines();
    let image = local_bytes("PEACECRAFT3_RANDOM_REAL_IMAGE", 64 << 20);
    let digest: String = Sha256::digest(&image)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(lines.next().unwrap(), format!("image_sha256\t{digest}"));
    let hex = |s| u32::from_str_radix(s, 16).unwrap();
    let mut counts = [0; 3];
    for (index, line) in lines.enumerate() {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 4);
        let (a, b) = (hex(fields[1]), hex(fields[2]));
        match fields[0] {
            "seed" => {
                counts[0] += 1;
                host.native("SetRandomSeed", &[Value::Int(a as i32)])
                    .unwrap();
            }
            "integer" => {
                counts[1] += 1;
                assert_eq!(
                    host.native(
                        "GetRandomInt",
                        &[Value::Int(a as i32), Value::Int(b as i32)]
                    )
                    .unwrap(),
                    Value::Int(hex(fields[3]) as i32),
                    "observation {index}"
                );
            }
            "real_formatted" => {
                counts[2] += 1;
                let Value::Real(value) = host
                    .native(
                        "GetRandomReal",
                        &[
                            Value::Real(f32::from_bits(a)),
                            Value::Real(f32::from_bits(b)),
                        ],
                    )
                    .unwrap()
                else {
                    panic!("real")
                };
                assert_eq!(
                    format!("{value:.3}"),
                    fields[3],
                    "formatted observation {index}"
                );
            }
            _ => panic!("unknown retained observation"),
        }
    }
    assert!(counts.iter().all(|n| *n > 0));
    println!(
        "retained original checkpoints: {counts:?}; real observations are formatted, not original bits"
    );
}
