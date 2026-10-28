use super::*;
use crate::pe_image::{RDATA, TEXT, tests::synthetic_image};

const COMMON: &[u8] = b"constant unitstate UNIT_STATE_MAX_LIFE = ConvertUnitState(1)\n";
const PROFILE: &[u8] = b"[Misc]\nConstructionRefundRate=.37\nConstructionDamageRefundPenalty=0\nConstructionLifeDrainRate=4.25\n";

fn origin() -> AssetIdentity {
    AssetIdentity {
        source: "synthetic client".into(),
        edition: "fixture".into(),
        version: "nonstock".into(),
    }
}
fn budget() -> DiscoveryBudget {
    DiscoveryBudget {
        image: crate::neutral_player::RuleInputBudget {
            image_bytes: 1 << 20,
            profile_bytes: 1 << 16,
            sections: 8,
        },
        registration_candidates: 128,
        work_bytes: 1 << 20,
    }
}
fn put(bytes: &mut [u8], at: usize, value: &[u8]) {
    bytes[at..at + value.len()].copy_from_slice(value);
}
fn word(bytes: &mut [u8], at: usize, value: u32) {
    put(bytes, at, &value.to_le_bytes());
}
fn call(bytes: &mut [u8], at: usize, base: u32, target: u32) {
    bytes[at] = 0xe8;
    word(bytes, at + 1, target.wrapping_sub(base + at as u32 + 5));
}

fn fixture(shift: u32, fraction: f32) -> Vec<u8> {
    let base = 0x400000 + shift;
    let text_va = base + 0x1000;
    let data_va = base + 0x8000;
    let rdata_va = base + 0x5000;
    let mut text = vec![0xcc; 0x2000];
    let mut rdata = vec![0; 0x2000];
    let names = [
        b"GetUnitState\0".as_slice(),
        b"SetWidgetLife\0",
        b"UnitSetConstructionProgress\0",
    ];
    for (i, name) in names.iter().enumerate() {
        put(&mut rdata, i * 0x40, name);
        let at = i * 16;
        text[at] = 0x68;
        word(&mut text, at + 1, rdata_va + (i * 0x40) as u32);
        text[at + 5] = 0x68;
        word(&mut text, at + 6, text_va + 0x100 + (i * 0x100) as u32);
        call(&mut text, at + 10, text_va, text_va + 0x900);
    }
    put(
        &mut text,
        0x100,
        &[0x55, 0x8b, 0xec, 0x56, 0x8b, 0x75, 0x0c, 0x83, 0xfe, 4],
    );
    call(&mut text, 0x12f, text_va, text_va + 0x500);
    put(
        &mut text,
        0x200,
        &[0x55, 0x8b, 0xec, 0xff, 0x75, 0x08, 0xe8],
    );
    put(
        &mut text,
        0x212,
        &[0x8b, 0x10, 0x8b, 0xc8, 0xff, 0x75, 0x0c, 0xff, 0x92],
    );
    word(&mut text, 0x21b, 0x124);
    put(
        &mut text,
        0x300,
        &[0x55, 0x8b, 0xec, 0x56, 0xff, 0x75, 0x08, 0xe8],
    );
    call(&mut text, 0x32c, text_va, text_va + 0x400);
    put(&mut text, 0x420, &[0x3d, b'P', b'n', b'B', b'A']);
    put(&mut text, 0xb00, &[0xb8, b'P', b'n', b'B', b'A', 0xc3]);
    put(
        &mut text,
        0x500,
        &[0x55, 0x8b, 0xec, 0x8b, 0x45, 0x0c, 0x56, 0x83],
    );
    put(&mut text, 0x50c, &[0xff, 0x24, 0x85]);
    word(&mut text, 0x50f, text_va + 0x580);
    word(&mut text, 0x584, text_va + 0x600);
    put(
        &mut text,
        0x600,
        &[0x8b, 0x11, 0x8b, 0x75, 0x08, 0x56, 0xff, 0x92],
    );
    word(&mut text, 0x608, 0x130);
    // Static type-code helper has the same body but no vtable entry.
    put(&mut text, 0xb20, &[0xb8, b'P', b'n', b'B', b'A', 0xc3]);
    let vtable = rdata_va + 0x400;
    word(&mut rdata, 0x41c, text_va + 0xb00);
    word(&mut rdata, 0x458, text_va + 0x700);
    word(&mut rdata, 0x758, text_va + 0x900);
    put(&mut rdata, 0xc0, b"CAbilityBuildInProgress\0");
    text[0x700] = 0xb8;
    word(&mut text, 0x701, rdata_va + 0xc0);
    text[0x705] = 0xc3;
    put(&mut text, 0x800, &[0xc7, 0x06]);
    word(&mut text, 0x802, vtable);
    text[0x940] = 0x68;
    word(&mut text, 0x941, data_va);
    put(&mut text, 0x945, &[0x51, 0x8b, 0xc8, 0xff, 0x92]);
    word(&mut text, 0x94a, 0x130);
    put(&mut text, 0x94e, &[0x50, 0x8d, 0x45, 0x08, 0x50]);
    call(&mut text, 0x953, text_va, text_va + 0xa00);
    put(
        &mut text,
        0x958,
        &[0x83, 0xc4, 0x0c, 0x8b, 0xcb, 0x50, 0xff, 0x96],
    );
    word(&mut text, 0x960, 0x124);
    put(&mut rdata, 0x100, b"CAbilityHumanBuild\0");
    put(&mut text, 0xc00, &[0xb8]);
    word(&mut text, 0xc01, rdata_va + 0x100);
    text[0xc05] = 0xc3;
    word(&mut rdata, 0xd58, text_va + 0xc00);
    word(&mut rdata, 0xd1c, text_va + 0xc20);
    put(&mut text, 0xc20, &[0xb8, b'9', b'X', b'Y', b'Z', 0xc3]);
    put(&mut text, 0xc40, &[0xc7, 0x06]);
    word(&mut text, 0xc42, rdata_va + 0xd00);
    put(&mut rdata, 0x140, b"CAbilityUndeadBuild\0");
    put(&mut text, 0xc60, &[0xb8]);
    word(&mut text, 0xc61, rdata_va + 0x140);
    text[0xc65] = 0xc3;
    word(&mut rdata, 0xf58, text_va + 0xc60);
    word(&mut rdata, 0xf1c, text_va + 0xc80);
    put(&mut text, 0xc80, &[0xb8, b'8', b'X', b'Y', b'Z', 0xc3]);
    put(&mut text, 0xca0, &[0xc7, 0x06]);
    word(&mut text, 0xca2, rdata_va + 0xf00);
    let data = fraction.to_le_bytes();
    let mut image = synthetic_image(&[
        (TEXT, 0x1000, &text),
        (RDATA, 0x5000, &rdata),
        (b".data\0\0\0", 0x8000, &data),
    ]);
    word(&mut image, 0x80 + 4 + 20 + 28, base);
    put(&mut image, 0x84, &0x14cu16.to_le_bytes());
    image
}
fn multiplication(image: &[u8]) -> SourceWindow {
    let pe = PeSections::parse(image).unwrap();
    let va = pe.image_base + 0x1a00;
    SourceWindow {
        va,
        len: 4,
        digest: sha256(pe.va_bytes(va, 4).unwrap()),
    }
}
fn read(
    image: &[u8],
    common: &[u8],
    profiles: &[(&str, &[u8])],
    budget: DiscoveryBudget,
) -> Result<Rules, String> {
    Rules::read(
        image,
        origin(),
        common,
        origin(),
        profiles,
        multiplication(image),
        budget,
    )
}

#[test]
fn relocated_nonstock_initial_life_and_ordered_profiles() {
    for shift in [0, 0x200000] {
        let image = fixture(shift, 0.23);
        let rules = read(
            &image,
            COMMON,
            &[
                ("stock", PROFILE),
                ("map", b"[Misc]\nConstructionRefundRate=.61\n"),
            ],
            budget(),
        )
        .unwrap();
        assert_eq!(rules.initial_life_fraction, 0.23);
        assert_eq!(rules.human_build_ability, *b"ZYX9");
        assert_eq!(rules.undead_build_ability, *b"ZYX8");
        assert_eq!(rules.refund_rate, 0.61);
        assert_eq!(rules.life_drain_rate, 4.25);
        assert!(!rules.damage_refund_penalty);
        assert_eq!(rules.image_sha256, sha256(&image));
        assert_eq!(rules.common_sha256, sha256(COMMON));
    }
}

#[test]
fn malformed_relationships_and_fraction_ranges_refuse() {
    for fraction in [0.0, 1.0, -0.2, f32::NAN, f32::INFINITY] {
        assert!(
            read(
                &fixture(0, fraction),
                COMMON,
                &[("stock", PROFILE)],
                budget()
            )
            .is_err()
        );
    }
    let image = fixture(0, 0.23);
    for text_offset in [
        0x12f, 0x212, 0x32c, 0x420, 0x500, 0x50c, 0x600, 0x700, 0x800, 0x940, 0x945, 0x953, 0x95e,
    ] {
        let mut bad = image.clone();
        bad[0x400 + text_offset] ^= 1;
        assert!(
            read(&bad, COMMON, &[("stock", PROFILE)], budget()).is_err(),
            "offset {text_offset:#x}"
        );
    }
    let mut bad = image.clone();
    bad[0x400 + 0xc60] ^= 1;
    assert!(
        read(&bad, COMMON, &[("stock", PROFILE)], budget())
            .unwrap_err()
            .contains("Undead build class")
    );
    assert!(
        read(
            &image,
            b"constant unitstate UNIT_STATE_MAX_LIFE = ConvertUnitState(9)",
            &[("stock", PROFILE)],
            budget()
        )
        .is_err()
    );
    assert!(
        read(
            &image,
            COMMON,
            &[("stock", b"[Misc]\nConstructionRefundRate=.3\n")],
            budget()
        )
        .is_err()
    );
}

#[test]
fn work_candidate_image_and_profile_limits_refuse() {
    let image = fixture(0, 0.23);
    for limits in [
        DiscoveryBudget {
            work_bytes: 1,
            ..budget()
        },
        DiscoveryBudget {
            registration_candidates: 0,
            ..budget()
        },
        DiscoveryBudget {
            image: crate::neutral_player::RuleInputBudget {
                image_bytes: 1,
                ..budget().image
            },
            ..budget()
        },
        DiscoveryBudget {
            image: crate::neutral_player::RuleInputBudget {
                profile_bytes: COMMON.len(),
                ..budget().image
            },
            ..budget()
        },
        DiscoveryBudget {
            image: crate::neutral_player::RuleInputBudget {
                sections: 1,
                ..budget().image
            },
            ..budget()
        },
    ] {
        assert!(read(&image, COMMON, &[("stock", PROFILE)], limits).is_err());
    }
    let mut other = origin();
    other.edition.clear();
    assert!(
        Rules::read(
            &image,
            origin(),
            COMMON,
            other,
            &[("stock", PROFILE)],
            multiplication(&image),
            budget()
        )
        .is_err()
    );
}

#[test]
#[ignore = "requires explicitly supplied selected Legacy install, executable, common.j and MiscGame.txt"]
fn selected_original_construction_rules() {
    let path = |key| {
        std::env::var(key).unwrap_or_else(|_| panic!("set {key} to the selected local source"))
    };
    let image_path = path("PCX_WAR3_EXE");
    let common_path = path("PEACECRAFT3_COMMON_J");
    let misc_path = path("PCX_CONSTRUCTION_MISC");
    let image = std::fs::read(&image_path).unwrap();
    let common = std::fs::read(&common_path).unwrap();
    let misc = std::fs::read(&misc_path).unwrap();
    let common_origin = AssetIdentity {
        source: common_path,
        edition: "explicit supplied common source".into(),
        version: format!("sha256 {:x?}", sha256(&common)),
    };
    let rules = Rules::read(
        &image,
        crate::campaign_inputs::client_identity(&image, &image_path),
        &common,
        common_origin,
        &[(&misc_path, &misc)],
        crate::campaign_client::numeric_multiplication(&image).unwrap(),
        DiscoveryBudget {
            image: crate::neutral_player::RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 8 << 20,
                sections: 96,
            },
            registration_candidates: 100000,
            work_bytes: 256 << 20,
        },
    )
    .unwrap();
    assert_eq!(rules.image_sha256, sha256(&image));
    assert_eq!(rules.common_sha256, sha256(&common));
    assert_eq!(rules.profile_sources, vec![misc_path]);
    // Selected shipped profile inputs, not a rendered or full campaign admission proof.
    let install = path("PCX_NATIVE_CHAPTER_INSTALL");
    let tables = pcx_objects::editor::defaults::DefaultTables::open(
        std::path::Path::new(&install),
        rules.origin.version.clone(),
    )
    .unwrap();
    let alias = rules.human_build_ability;
    let row = std::str::from_utf8(&alias).unwrap();
    let ability = tables
        .object(pcx_objects::editor::defaults::Family::Ability, row)
        .unwrap();
    let race = ability
        .cell(pcx_objects::editor::SlkId::AbilityData, "race")
        .unwrap();
    assert!(!matches!(race.text.trim(), "" | "_" | "-"));
    let undead = std::str::from_utf8(&rules.undead_build_ability).unwrap();
    let race = tables
        .object(pcx_objects::editor::defaults::Family::Ability, undead)
        .unwrap()
        .cell(pcx_objects::editor::SlkId::AbilityData, "race")
        .unwrap();
    assert!(!matches!(race.text.trim(), "" | "_" | "-"));
    crate::ability_profile::button_position(&tables, &Default::default(), alias).unwrap();
    let art = crate::ability_profile::profiles(&tables, &Default::default(), alias, "Art").unwrap();
    assert!(matches!(art.as_slice(), [one] if !one.is_empty()));
}
