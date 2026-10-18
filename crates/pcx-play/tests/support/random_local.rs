use std::io::Read;
pub fn local_bytes(variable: &str, maximum: u64) -> Vec<u8> {
    let path = std::env::var_os(variable).expect(variable);
    let file = std::fs::File::open(&path).expect("explicit local input");
    let length = file.metadata().expect("local input metadata").len();
    assert!(
        length <= maximum,
        "{variable}: input exceeds caller ceiling"
    );
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(usize::try_from(length + 1).unwrap())
        .expect("local comparison input allocation");
    file.take(length + 1)
        .read_to_end(&mut bytes)
        .expect("local comparison input read");
    assert_eq!(
        bytes.len() as u64,
        length,
        "{variable}: input changed while reading"
    );
    bytes
}

pub fn binding_windows(
    selection: &[u8],
) -> (
    [u8; 32],
    std::collections::BTreeMap<&str, pcx_play::camera_margins::SourceWindow>,
) {
    use pcx_play::camera_margins::SourceWindow;
    use std::collections::BTreeMap;
    assert!(selection.len() <= 16 << 10);
    let text = std::str::from_utf8(selection).expect("binding UTF-8");
    let mut lines = text.lines();
    let hash = |s: &str| -> [u8; 32] {
        assert_eq!(s.len(), 64);
        std::array::from_fn(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
    };
    let image_sha256 = hash(
        lines
            .next()
            .unwrap()
            .strip_prefix("image_sha256\t")
            .unwrap(),
    );
    let mut windows = BTreeMap::new();
    for line in lines {
        let columns: Vec<_> = line.split('\t').collect();
        assert_eq!(columns.len(), 4);
        assert!(
            windows
                .insert(
                    columns[0],
                    SourceWindow {
                        va: u32::from_str_radix(columns[1], 16).unwrap(),
                        len: columns[2].parse().unwrap(),
                        digest: hash(columns[3]),
                    }
                )
                .is_none()
        );
    }
    assert_eq!(windows.len(), 12);
    (image_sha256, windows)
}

pub fn policy_from_inputs(
    image: &[u8],
    selection: &[u8],
) -> pcx_play::script_random::profile::Policy {
    use pcx_play::neutral_player::RuleInputBudget;
    use pcx_play::script_random::profile::{Binding, Policy, Problem};
    use pcx_play::scripts::AssetIdentity;
    let (image_sha256, mut windows) = binding_windows(selection);
    let mut window = |name| windows.remove(name).expect("missing reviewed role");
    let binding = Binding {
        origin: AssetIdentity {
            edition: "explicit local legacy".into(),
            version: "caller-reviewed selected code".into(),
            source: "PEACECRAFT3_RANDOM_REAL_IMAGE".into(),
        },
        image_sha256,
        seed: window("seed"),
        draw: window("draw"),
        integer: window("integer"),
        real: window("real"),
        set_seed: window("set_seed"),
        secondary: window("secondary"),
        real_constant_init: window("real_constant_init"),
        math_sub: window("math_sub"),
        math_add: window("math_add"),
        math_mul: window("math_mul"),
        math_lzcnt: window("math_lzcnt"),
        math_i32_to_real: window("math_i32_to_real"),
    };
    let budget = RuleInputBudget {
        image_bytes: 64 << 20,
        profile_bytes: 64 << 10,
        sections: 96,
    };
    let mut short = budget;
    short.profile_bytes = 1;
    assert!(matches!(
        Policy::read(image, binding.clone(), short)
            .unwrap_err()
            .problem,
        Problem::Budget
    ));
    let mut bad = binding.clone();
    bad.origin.version.clear();
    assert!(matches!(
        Policy::read(image, bad, budget).unwrap_err().problem,
        Problem::Identity
    ));
    let mut bad = binding.clone();
    bad.image_sha256[0] ^= 1;
    assert!(matches!(
        Policy::read(image, bad, budget).unwrap_err().problem,
        Problem::Identity
    ));
    let mut bad = binding.clone();
    bad.seed.digest[0] ^= 1;
    assert!(matches!(
        Policy::read(image, bad, budget).unwrap_err().problem,
        Problem::Image(_)
    ));
    let mut bad = binding.clone();
    bad.draw = bad.seed;
    assert!(matches!(
        Policy::read(image, bad, budget).unwrap_err().problem,
        Problem::Ambiguous
    ));
    let mut bad = binding.clone();
    std::mem::swap(&mut bad.math_sub, &mut bad.math_add);
    assert!(matches!(
        Policy::read(image, bad, budget).unwrap_err().problem,
        Problem::Real
    ));
    Policy::read(image, binding, budget).expect("selected-code profile")
}

pub fn local_policy() -> pcx_play::script_random::profile::Policy {
    policy_from_inputs(
        &local_bytes("PEACECRAFT3_RANDOM_REAL_IMAGE", 64 << 20),
        &local_bytes("PEACECRAFT3_RANDOM_BINDING", 16 << 10),
    )
}
