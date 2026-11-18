use super::*;

pub(crate) fn fixture(exponent: u32, masks: &[u8]) -> (Vec<u8>, Binding, RuleInputBudget) {
    assert!(!masks.is_empty() && masks.len() <= 16);
    let mut image = vec![0; 0x600];
    for (at, value) in [
        (0x3c, 0x80u32),
        (0xb4, 0x300000),
        (0xd0, 0x2000),
        (0x100, 0x400),
        (0x104, 0x1000),
        (0x108, 0x400),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
    ] {
        image[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    image[..2].copy_from_slice(b"MZ");
    image[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, value) in [(0x84, 0x14cu16), (0x86, 1), (0x94, 0x60), (0x98, 0x10b)] {
        image[at..at + 2].copy_from_slice(&value.to_le_bytes());
    }
    image[0x220] = 0x05;
    image[0x221..0x225].copy_from_slice(&(-(exponent as i32 * (1 << 23))).to_le_bytes());
    let va = 0x301040u32;
    let table_offset = 18 + masks.len() * 4 + 4;
    let mut code = vec![
        0x55,
        0x8b,
        0xec,
        0x8b,
        0x45,
        8,
        0x83,
        0xf8,
        (masks.len() - 1) as u8,
        0x77,
        (18 + masks.len() * 4 - 11) as u8,
        0xff,
        0x24,
        0x85,
    ];
    code.extend((va + table_offset as u32).to_le_bytes());
    for mask in masks {
        code.extend([0xb0, *mask, 0x5d, 0xc3]);
    }
    code.extend([0x32, 0xc0, 0x5d, 0xc3]);
    for n in 0..masks.len() {
        code.extend((va + 18 + n as u32 * 4).to_le_bytes());
    }
    image[0x240..0x240 + code.len()].copy_from_slice(&code);
    let binding = Binding {
        origin: AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture-v1".into(),
            source: "synthetic:pathing-code".into(),
        },
        image_sha256: sha256(&image),
        normalization: SourceWindow {
            va: 0x301020,
            len: 5,
            digest: sha256(&image[0x220..0x225]),
        },
        type_switch: SourceWindow {
            va,
            len: code.len() as u32,
            digest: sha256(&code),
        },
    };
    (
        image,
        binding,
        RuleInputBudget {
            image_bytes: 4096,
            profile_bytes: 256,
            sections: 2,
        },
    )
}

pub(crate) fn input(exponent: u32, masks: &[u8]) -> TerrainPathing {
    let (image, binding, budget) = fixture(exponent, masks);
    TerrainPathing::read(&image, binding, budget).unwrap()
}

#[test]
fn scale_and_masks_follow_selected_operands_and_targets() {
    for exponent in [1, 4, 7, 12] {
        let p = input(exponent, &[0xf1, 0x14, 0x29]);
        assert_eq!(p.cell_size(), (1u32 << exponent) as f32);
        assert_eq!([p.mask(0), p.mask(1), p.mask(2)], [0xf1, 0x14, 0x29]);
        assert_eq!([p.mask(-1), p.mask(3), p.mask(i32::MAX)], [0; 3]);
    }
    let (mut bytes, mut binding, budget) = fixture(4, &[0xff]);
    bytes[0x252] = 0x0c; // Source's OR-all-bits handler is also a constant result.
    binding.image_sha256 = sha256(&bytes);
    binding.type_switch.digest = sha256(&bytes[0x240..0x240 + binding.type_switch.len as usize]);
    assert_eq!(
        TerrainPathing::read(&bytes, binding, budget)
            .unwrap()
            .mask(0),
        0xff
    );
}

#[test]
fn changed_fingerprints_budgets_targets_and_instruction_shapes_refuse() {
    for mutation in 0..12 {
        let (mut bytes, mut b, mut budget) = fixture(4, &[0x12, 0x84]);
        match mutation {
            0 => b.origin.edition.clear(),
            1 => b.image_sha256 = sha256(b"different image"),
            2 => budget.image_bytes = bytes.len() - 1,
            3 => budget.profile_bytes = 4,
            4 => bytes[0x220] = 0x90,
            5 => bytes[0x221..0x225].copy_from_slice(&1i32.to_le_bytes()),
            6 => bytes[0x240] = 0x90,
            7 => bytes[0x24e..0x252].copy_from_slice(&0x1234u32.to_le_bytes()),
            8 => bytes[0x240 + 30..0x240 + 34].copy_from_slice(&0x1234u32.to_le_bytes()),
            9 => bytes[0x252] = 0x90,
            10 => bytes[0x25a] = 0x90,
            11 => bytes[0x248] = 0xff,
            _ => unreachable!(),
        }
        if mutation >= 4 {
            b.image_sha256 = sha256(&bytes);
            b.normalization.digest = sha256(&bytes[0x220..0x225]);
            b.type_switch.digest = sha256(&bytes[0x240..0x240 + b.type_switch.len as usize]);
        }
        assert!(
            TerrainPathing::read(&bytes, b, budget).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn legacy_terrain_binding_rejects_an_unreviewed_image_and_enforces_budget() {
    let (image, binding, mut budget) = fixture(4, &[0x12, 0x84]);
    let error = TerrainPathing::read_legacy(&image, binding.origin.clone(), budget).unwrap_err();
    assert!(matches!(error.problem, Problem::Identity));
    budget.image_bytes = image.len() - 1;
    let error = TerrainPathing::read_legacy(&image, binding.origin, budget).unwrap_err();
    assert!(matches!(error.problem, Problem::Budget));
}

#[test]
#[ignore = "requires PCX_WAR3_EXE pointing to the reviewed Legacy image"]
fn selected_legacy_terrain_wrapper_reads_pinned_image() {
    use std::io::Read;
    let budget = RuleInputBudget {
        image_bytes: 32 << 20,
        profile_bytes: 64 << 10,
        sections: 96,
    };
    let path = std::env::var("PCX_WAR3_EXE").unwrap();
    let mut image = Vec::new();
    std::fs::File::open(&path)
        .unwrap()
        .take(budget.image_bytes as u64 + 1)
        .read_to_end(&mut image)
        .unwrap();
    let origin = crate::campaign_inputs::client_identity(&image, &path);
    let policy = TerrainPathing::read_legacy(&image, origin.clone(), budget).unwrap();
    let binding = policy.binding();
    assert_eq!(binding.origin, origin);
    assert_eq!(binding.image_sha256, crate::menu_lighting::REVIEWED_IMAGE);
    let decoded = TerrainPathing::read(&image, binding.clone(), budget).unwrap();
    assert_eq!(policy.cell_size(), decoded.cell_size());
    let pe = Pe::parse(&image, budget).unwrap();
    let switch = pe.window(&binding.type_switch, true).unwrap();
    assert_eq!(policy.masks.len(), usize::from(switch[8]) + 1);
    assert_eq!(policy.masks, decoded.masks);
    assert_eq!(policy.mask(-1), decoded.mask(-1));
    assert!(matches!(legacy_windows(&pe, 0), Err(Problem::Budget)));

    for address in [0x4a0d73, 0x4998a8, 0x4998ba, 0x7f6a1a, 0x7f690b, 0x7f6941] {
        let bytes = pe.read_from(address, true).unwrap();
        let offset = bytes.as_ptr() as usize - image.as_ptr() as usize;
        let mut changed = image.clone();
        changed[offset] ^= 1;
        let pe = Pe::parse(&changed, budget).unwrap();
        assert!(
            matches!(
                legacy_windows(&pe, budget.profile_bytes),
                Err(Problem::Shape)
            ),
            "{address:#x}"
        );
    }
}
