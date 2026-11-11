//! Purge rows, the lightning orb and the registrations read from the image. The rule under
//! test is the one decoded in `docs/re-notes/passive-held-items.md`, "Purge".
use super::*;
use crate::inventory_powerup::purge::fixture;
use crate::inventory_powerup::spell_orb::fixture as spell_fixture;

/// Columns: alias, code, DataA1, DataB1, DataC1, DataD1, DataE1, DataF1, Rng1, Cool1, Cost1,
/// targs1, Dur1, HeroDur1, BuffID1, UnitID1.
type Row = [&'static str; 16];

const ORB_TARGS: &str = "ground,air,ward";

const LIGHTNING_ORB: Row = [
    "AIlb", "AIlb", "6", "-", "-", "-", "2", "-", "-", "-", "-", ORB_TARGS, "-", "-", "-", "-",
];
const LIGHTNING_PURGE: Row = [
    "AIlp", "AIlp", "2", "-", "35", "-", "-", "-", "-", "-", "-", ORB_TARGS, "3", "0.3", "Bprg",
    "-",
];
const SPELL_ORB: Row = [
    "AIll", "AIsb", "5", "35", "10", "35", "2", "-", "-", "-", "-", ORB_TARGS, "-", "-", "-",
    "AIpg",
];
const WRAPPED: Row = [
    "AIpg",
    "Aprg",
    "5",
    "-",
    "150",
    "3",
    "1",
    "-",
    "700",
    "-",
    "-",
    "air,ground,ward",
    "15",
    "5",
    "Bprg",
    "-",
];

fn rows() -> [Row; 4] {
    [LIGHTNING_ORB, LIGHTNING_PURGE, SPELL_ORB, WRAPPED]
}

fn tables(rows: &[Row], items: &[&[&str]]) -> Arc<DefaultTables> {
    let ability_rows: Vec<Vec<&str>> = rows
        .iter()
        .map(|row| {
            let mut cells = vec![row[0], row[1], "1"];
            cells.extend(&row[2..]);
            cells
        })
        .collect();
    let refs: Vec<&[&str]> = ability_rows.iter().map(Vec::as_slice).collect();
    let abilities = sylk(
        &[
            "alias", "code", "levels", "DataA1", "DataB1", "DataC1", "DataD1", "DataE1", "DataF1",
            "Rng1", "Cool1", "Cost1", "targs1", "Dur1", "HeroDur1", "BuffID1", "UnitID1",
        ],
        &refs,
    );
    let items = sylk(
        &[
            "itemID",
            "abilList",
            "powerup",
            "usable",
            "perishable",
            "uses",
        ],
        items,
    );
    let meta = sylk(
        &["ID", "field", "slk", "index", "type", "data", "useSpecific"],
        &[
            &["Iagi", "Data", "AbilityData", "-1", "int", "1", "AIsm,AIxm"],
            &["Iint", "Data", "AbilityData", "-1", "int", "2", "AIsm,AIxm"],
            &["Istr", "Data", "AbilityData", "-1", "int", "3", "AIsm,AIxm"],
            &["Idef", "Data", "AbilityData", "-1", "int", "1", "AId1"],
            &["Iatt", "Data", "AbilityData", "-1", "int", "1", "AIat,AItf"],
        ],
    );
    Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic purge".into(),
                edition_version: "fixture".into(),
            },
            &[
                (SlkId::ItemData.path(), &items),
                (SlkId::AbilityData.path(), &abilities),
                (SlkId::AbilityMetaData.path(), &meta),
            ],
            &[],
            EditorReadLimits {
                max_listed_paths: 4,
                max_asset_bytes: 65536,
                max_aggregate_input_bytes: 131072,
            },
        )
        .unwrap(),
    )
}

const ITEMS: &[&[&str]] = &[
    &["olig", "AIlb,AIlp", "0", "0", "0", "-"],
    &["olib", "AIlb", "0", "0", "0", "-"],
    &["olip", "AIlp", "0", "0", "0", "-"],
    &["oli2", "AIll", "0", "0", "0", "-"],
];

fn orb_base(tables: Arc<DefaultTables>, overlay: Overlay) -> Result<Rules, String> {
    let mut orb_base = vec![PUSH_IMM32, 1, 2, 3, 4, CALL_REL32];
    orb_base.extend((0x100_i32 - 10).to_le_bytes());
    orb_base.push(PUSH_EAX);
    orb_base.push(PUSH_IMM32);
    orb_base.extend(b"AIDB".iter().rev());
    orb_base.push(CALL_REL32);
    orb_base.extend(
        ((i64::from(REGISTER_FN) - (i64::from(ORB_BASE_WINDOW) + 21)) as i32).to_le_bytes(),
    );
    orb_base.extend(CLEANUP_RET);
    let mut at = vec![MOV_EAX_IMM32];
    at.extend(b"AIat".iter().rev());
    at.push(RET);
    Rules::decode(
        tables,
        overlay,
        AssetIdentity {
            source: "synthetic".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        },
        [0; 32],
        [
            &window("AIsm", "AIxm"),
            &window("AIab", "APbo"),
            &window("AIde", "APbo"),
            &window("AIat", "APbo"),
        ],
    )?
    .with_orbs(
        &orb_base,
        &at,
        &window("AIfb", "AIDB"),
        &window("AIcb", "AIDB"),
    )
}

fn rules_over(rows: &[Row], overlay: Overlay) -> Result<Rules, String> {
    orb_base(tables(rows, ITEMS), overlay)?
        .with_spell_orb(&spell_fixture::image(1))?
        .with_purge(&fixture::image())
}

fn rules() -> Rules {
    rules_over(&rows(), Overlay::default()).unwrap()
}

fn held(item: &str) -> Result<Orb, String> {
    rules()
        .held_attributes(&catalog_item(item))
        .map(|held| held.unwrap().orb.unwrap())
}

fn lightning(item: &str, rows: [Row; 4]) -> Result<Orb, String> {
    rules_over(&rows, Overlay::default())?
        .held_attributes(&catalog_item(item))
        .map(|held| held.unwrap().orb.unwrap())
}

fn near(value: f32, expected: f32) {
    assert!(
        (value - expected).abs() <= expected.abs() * 1e-4 + 1e-9,
        "{value} is not {expected}"
    );
}

#[test]
fn the_lightning_orb_reads_its_sibling_purge_row() {
    let orb = held("olig").unwrap();
    assert_eq!(orb.damage, 6, "the orb's own DataA1 is the flat bonus");
    let OrbKind::Lightning(purge) = orb.kind else {
        panic!("a lightning orb")
    };
    assert_eq!(purge.buff, *b"Bprg");
    // Dur1 3 and HeroDur1 0.3 gain the image's slack; DataA1 2 splits each into two steps.
    near(purge.duration, 3.0 + fixture::SLACK);
    near(purge.hero_duration, 0.3 + fixture::SLACK);
    let slow = purge.slow.expect("DataA1 is set");
    near(slow.step, 0.5);
    near(slow.interval, 1.5);
    near(slow.hero_interval, 0.15);
    assert_eq!(purge.summoned_damage, 35.0);
    assert_eq!((purge.unit_pause, purge.hero_pause), (0.0, 0.0));
    assert_eq!(
        purge.targets,
        PurgeTargets {
            air: true,
            ground: true,
            structure: false
        }
    );
    assert_eq!(
        orb.targets,
        OrbTargets {
            ground: true,
            air: true,
            structure: false,
            organic: false
        }
    );
}

#[test]
fn the_spell_orb_wraps_purge_with_its_pauses() {
    let orb = held("oli2").unwrap();
    let OrbKind::Spell(spell) = orb.kind else {
        panic!("a spell orb")
    };
    let WrappedAbility::Purge(purge) = spell.wrapped else {
        panic!("purge")
    };
    near(purge.slow.unwrap().step, 0.2);
    near(purge.slow.unwrap().interval, 3.0);
    near(purge.slow.unwrap().hero_interval, 1.0);
    assert_eq!(
        (purge.summoned_damage, purge.unit_pause, purge.hero_pause),
        (150.0, 3.0, 1.0)
    );
    assert_eq!(purge.range, 700.0);
}

#[test]
fn a_frequency_of_zero_leaves_the_buff_without_a_slow() {
    let mut rows = rows();
    rows[1][2] = "0";
    let OrbKind::Lightning(purge) = lightning("olig", rows).unwrap().kind else {
        panic!("a lightning orb")
    };
    assert_eq!(purge.slow, None);
}

#[test]
fn the_lightning_orb_and_its_purge_travel_together() {
    let plain = || rules();
    assert!(
        plain()
            .held_attributes(&catalog_item("olib"))
            .unwrap_err()
            .contains("exactly one AIlp")
    );
    assert!(
        plain()
            .held_attributes(&catalog_item("olip"))
            .unwrap_err()
            .contains("exactly one AIlb")
    );
}

#[test]
fn rows_outside_the_decoded_shape_refuse() {
    let edited = |row: usize, column: usize, value: &'static str| {
        let mut rows = rows();
        rows[row][column] = value;
        rows
    };
    // DataB is the attack speed slow and DataF the mana loss: neither is hosted.
    assert!(
        lightning("olig", edited(1, 3, "1"))
            .unwrap_err()
            .contains("DataB1")
    );
    assert!(
        lightning("olig", edited(1, 7, "10"))
            .unwrap_err()
            .contains("DataF1")
    );
    // A negative duration, a duration of nothing, and a different class list refuse.
    assert!(
        lightning("olig", edited(1, 12, "-1"))
            .unwrap_err()
            .contains("negative")
    );
    assert!(
        lightning("olig", edited(1, 12, "0"))
            .unwrap_err()
            .contains("not positive")
    );
    assert!(
        lightning("olig", edited(1, 11, "ground,ward"))
            .unwrap_err()
            .contains("different target classes")
    );
    assert!(
        lightning("olig", edited(1, 11, "ground,air,enemy"))
            .unwrap_err()
            .contains("not decoded")
    );
    assert!(
        lightning("olig", edited(1, 14, "Bp"))
            .unwrap_err()
            .contains("BuffID1")
    );
    // The generic cast of a spell orb spends a cost and sets a cooldown, neither decoded; the
    // lightning orb never reaches that cast.
    assert!(
        lightning("oli2", edited(3, 10, "10"))
            .unwrap_err()
            .contains("Cost1 or Cool1")
    );
    assert!(lightning("olig", edited(1, 10, "25")).is_ok());
    // A map that modifies the purge row is not decoded.
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIlp".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    let rules = rules_over(&rows(), overlay).unwrap();
    assert!(
        rules
            .held_attributes(&catalog_item("olig"))
            .unwrap_err()
            .contains("map-modified")
    );
}

#[test]
fn registration_reads_the_classes_from_the_image() {
    let tables = || Arc::clone(&rules().tables);
    let attach = |image: PurgeImage| {
        orb_base(tables(), Overlay::default())
            .unwrap()
            .with_purge(&image)
            .err()
    };
    assert_eq!(attach(fixture::image()), None);
    let mut wrong_base = fixture::image();
    wrong_base.orb_window = window("AIlb", "APbo");
    assert!(attach(wrong_base).unwrap().contains("name the orb base"));
    let mut wrong_sibling = fixture::image();
    wrong_sibling.sibling_base = purge_base("AAsm");
    assert!(
        attach(wrong_sibling)
            .unwrap()
            .contains("name the Purge class")
    );
    let mut duplicate = fixture::image();
    duplicate.orb_window = window("AIfb", "AIDB");
    assert!(attach(duplicate).unwrap().contains("distinct"));
    // The slack is a nonnegative real, and the reciprocal table has its full length.
    let mut nan = fixture::image();
    nan.slack = f32::NAN.to_le_bytes();
    assert!(attach(nan).unwrap().contains("slack"));
    let mut short = fixture::image();
    short.reciprocal.truncate(16);
    assert!(attach(short).unwrap().contains("reciprocal"));
    // Attaching twice, or without the orb base, is refused.
    let twice = orb_base(tables(), Overlay::default())
        .unwrap()
        .with_purge(&fixture::image())
        .unwrap()
        .with_purge(&fixture::image());
    assert!(twice.err().unwrap().contains("distinct"));
    let bare = Rules::decode(
        tables(),
        Overlay::default(),
        AssetIdentity {
            source: "synthetic".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        },
        [0; 32],
        [
            &window("AIsm", "AIxm"),
            &window("AIab", "APbo"),
            &window("AIde", "APbo"),
            &window("AIat", "APbo"),
        ],
    )
    .unwrap();
    assert!(
        bare.with_purge(&fixture::image())
            .err()
            .unwrap()
            .contains("orb base")
    );
}

fn purge_base(name: &str) -> Vec<u8> {
    let mut code = vec![MOV_EAX_IMM32];
    code.extend(name.bytes().rev());
    code.push(RET);
    code
}

#[test]
fn without_the_registration_the_lightning_orb_stays_an_unsupported_class() {
    let rules = orb_base(tables(&rows(), ITEMS), Overlay::default()).unwrap();
    let error = rules.held_attributes(&catalog_item("olig")).unwrap_err();
    assert!(
        error.contains("unsupported passive ability AIlb"),
        "{error}"
    );
}

#[test]
#[ignore = "requires PEACECRAFT3_INSTALL (the install directory with Warcraft III.exe)"]
fn local_original_image_supplies_the_purge_classes_the_slack_and_the_divide() {
    let install = std::env::var("PEACECRAFT3_INSTALL").unwrap();
    let image = std::fs::read(std::path::Path::new(&install).join("Warcraft III.exe")).unwrap();
    assert_eq!(sha256(&image), REVIEWED_IMAGE);
    let pe = Pe::parse(&image, IMAGE_BUDGET).unwrap();
    let purge = PurgeImage::read(&pe).unwrap();
    assert_eq!(
        class_from_call_window_at(
            &purge.purge_window,
            crate::inventory_powerup::purge::PURGE_WINDOW,
            &purge.purge_base
        )
        .unwrap(),
        ("Aprg".to_string(), "AAsm".to_string())
    );
    assert_eq!(
        class_from_call_window_at(
            &purge.sibling_window,
            crate::inventory_powerup::purge::LIGHTNING_PURGE_WINDOW,
            &purge.sibling_base
        )
        .unwrap(),
        ("AIlp".to_string(), "Aprg".to_string())
    );
    assert_eq!(
        class_from_window(&purge.orb_window).unwrap(),
        ("AIlb".to_string(), "AIDB".to_string())
    );
    assert_eq!(f32::from_le_bytes(purge.slack), 0.01);
    // The step of the shipped Purge rows (DataA1 5 and 2) and the interval of AIlp (3 over 2).
    let table: Vec<u32> = purge
        .reciprocal
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect();
    for (numerator, denominator) in [(1, 5), (1, 2), (15, 5), (3, 2)] {
        let bits = crate::script_random::real::divide(
            crate::script_random::real::from_i32(numerator).unwrap(),
            crate::script_random::real::from_i32(denominator).unwrap(),
            &table,
        )
        .unwrap();
        let exact = numerator as f32 / denominator as f32;
        let got = f32::from_bits(bits);
        assert!(
            (got - exact).abs() <= exact * 2e-6,
            "{numerator}/{denominator} = {got}, exact {exact}"
        );
    }
}
