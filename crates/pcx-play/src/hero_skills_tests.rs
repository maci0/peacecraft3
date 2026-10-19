use super::*;

fn fixture() -> (Vec<u8>, Binding, Progression, RuleInputBudget) {
    let (mut image, _, mut budget) = crate::script_numeric::tests::fixture();
    let mut put = |offset: usize, bytes: &[u8]| {
        image[0x200 + offset..0x200 + offset + bytes.len()].copy_from_slice(bytes);
        SourceWindow {
            va: 0x301000 + offset as u32,
            len: bytes.len() as u32,
            digest: sha256(bytes),
        }
    };
    let level_initializer = put(0x1200, &[0xc7, 0x46, 0x74, 0, 0, 0, 0]);
    let level_query = put(
        0x1240,
        &[
            0x8b, 0x81, 0xfc, 3, 0, 0, 0x85, 0xc0, 0x74, 0x0a, 0x8d, 0x48, 0x70, 0xe8, 0, 0, 0, 0,
            0x40, 0xc3,
        ],
    );
    let points_initializer = put(
        0x1280,
        &[
            0xc7, 0x86, 0x94, 0, 0, 0, 3, 0, 0, 0, 0x83, 0xfb, 3, 0x7d, 6, 0x89, 0x9e, 0x94, 0, 0,
            0,
        ],
    );
    let slot_limit = put(0x12c0, &[0x83, 0xf9, 3, 0x7d, 0x10]);
    let first_rank = put(
        0x1300,
        &[
            0x6a, 1, 0x8b, 0xb5, 0xdc, 0xfd, 0xff, 0xff, 0x8b, 0xce, 0x57, 0xe8, 0, 0, 0, 0,
        ],
    );
    let hero_comparison = put(0x1400, &[0x81, 0xfe, b'r', b'e', b'h', b'Y', 0x74, 0x38]);
    let hero_handler = put(
        0x1440,
        &[0x8b, 0x81, 0xfc, 3, 0, 0, 0x5e, 0x5d, 0xc2, 0x14, 0],
    );
    let other_implicit = vec![put(0x1480, &[0x81, 0xfe, b'p', b'm', b'i', b'Y'])];
    budget.profile_bytes = 8192;
    let binding = Binding {
        origin: AssetIdentity {
            source: "synthetic PE".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        },
        image_sha256: sha256(&image),
        level_initializer,
        level_query,
        points_initializer,
        slot_limit,
        first_rank,
        hero_comparison,
        hero_handler,
        other_implicit,
    };
    let progression = Progression::read(
        &[(
            "fixture Misc",
            b"[Misc]\nMaxHeroLevel=11\nHeroAbilityLevelSkip=2\n",
        )],
        256,
    )
    .unwrap();
    (image, binding, progression, budget)
}

pub(crate) fn rules() -> Rules {
    let (image, binding, progression, budget) = fixture();
    Rules::read(&image, binding, progression, budget).unwrap()
}
#[test]
fn counters_are_decoded_from_source_windows() {
    let rules = rules();
    assert_eq!(
        (
            rules.initial_level,
            rules.initial_points,
            rules.slots,
            rules.first_rank
        ),
        (1, 3, 3, 1)
    );
    assert_eq!(rules.progression.max_level, 11);
    assert_eq!(rules.hero_alias, *b"Yher");
    assert_eq!(rules.other_implicit, [*b"Yimp"]);
}

#[test]
fn hero_dispatch_must_reach_the_same_component_as_the_level_query() {
    let (mut image, mut binding, progression, budget) = fixture();
    image[0x200 + 0x1400 + 7] += 1;
    binding.image_sha256 = sha256(&image);
    binding.hero_comparison.digest = sha256(&image[0x200 + 0x1400..0x200 + 0x1408]);
    assert!(
        Rules::read(&image, binding, progression, budget)
            .unwrap_err()
            .contains("dispatch relationship")
    );
}
#[test]
fn counter_admission_refuses_changed_shapes_hashes_and_budgets() {
    let (image, binding, progression, budget) = fixture();
    let mut bad = binding.clone();
    bad.image_sha256 = [0; 32];
    assert!(Rules::read(&image, bad, progression.clone(), budget).is_err());
    let mut budget_bad = budget;
    budget_bad.profile_bytes = 1;
    assert!(Rules::read(&image, binding.clone(), progression.clone(), budget_bad).is_err());
    for at in [
        0x1200, 0x1240, 0x1280, 0x12c0, 0x1300, 0x1400, 0x1440, 0x1480,
    ] {
        let mut bytes = image.clone();
        bytes[0x200 + at] ^= 1;
        let mut bad = binding.clone();
        bad.image_sha256 = sha256(&bytes);
        for w in [
            &mut bad.level_initializer,
            &mut bad.level_query,
            &mut bad.points_initializer,
            &mut bad.slot_limit,
            &mut bad.first_rank,
            &mut bad.hero_comparison,
            &mut bad.hero_handler,
        ]
        .into_iter()
        .chain(bad.other_implicit.iter_mut())
        {
            let off = 0x200 + (w.va - 0x301000) as usize;
            w.digest = sha256(&bytes[off..off + w.len as usize]);
        }
        assert!(Rules::read(&bytes, bad, progression.clone(), budget).is_err());
    }
}
