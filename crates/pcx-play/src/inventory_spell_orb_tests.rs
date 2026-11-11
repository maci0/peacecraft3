//! Spell orb rows, registration and the image shapes the proc rule is read from.
use super::*;
use crate::inventory_powerup::spell_orb::{self as image_rules, fixture};

/// Columns: alias, code, DataA1, DataB1, DataC1, DataD1, DataE1, Rng1, Cool1, Cost1, targs1,
/// Dur1, HeroDur1, BuffID1, UnitID1.
type Row = [&'static str; 15];

const ORB_TARGS: &str = "ground,air,ward";
const ARROW_TARGS: &str = "air,ground,enemy,organic,neutral";

const DARKNESS: Row = [
    "AIdf", "AIsb", "6", "100", "100", "100", "2", "-", "-", "-", ORB_TARGS, "-", "-", "-", "ANbs",
];
const LIGHTNING: Row = [
    "AIll", "AIsb", "5", "35", "10", "35", "2", "-", "-", "-", ORB_TARGS, "-", "-", "-", "AIpg",
];
const ARROW: Row = [
    "ANbs",
    "ANbs",
    "-",
    "-",
    "-",
    "-",
    "-",
    "600",
    "-",
    "-",
    ARROW_TARGS,
    "3",
    "-",
    "BNba,BNdm",
    "ndr1",
];
const PURGE: Row = [
    "AIpg",
    "Aprg",
    "5",
    "-",
    "150",
    "3",
    "1",
    "700",
    "-",
    "-",
    "air,ground,ward",
    "15",
    "5",
    "Bprg",
    "-",
];

/// The shipped rows, with `DataB1` and `DataC1` of the arrow row as its summon count and
/// duration.
fn rows() -> [Row; 4] {
    let mut arrow = ARROW;
    arrow[3] = "1";
    arrow[4] = "80";
    [DARKNESS, LIGHTNING, arrow, PURGE]
}

fn rules_with(rows: &[Row], overlay: Overlay, image: &SpellOrbImage) -> Result<Rules, String> {
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
            "alias", "code", "levels", "DataA1", "DataB1", "DataC1", "DataD1", "DataE1", "Rng1",
            "Cool1", "Cost1", "targs1", "Dur1", "HeroDur1", "BuffID1", "UnitID1",
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
        &[
            &["odef", "AIdf", "0", "0", "0", "-"],
            &["oli2", "AIll", "0", "0", "0", "-"],
        ],
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
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic spell orbs".into(),
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
    );
    orb_base(tables, overlay)?.with_spell_orb(image)
}

fn orb_base(tables: Arc<DefaultTables>, overlay: Overlay) -> Result<Rules, String> {
    let mut orb_base = vec![PUSH_IMM32, 1, 2, 3, 4, CALL_REL32];
    let rel_base: i32 = 0x100 - 10;
    orb_base.extend(rel_base.to_le_bytes());
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

fn rules() -> Rules {
    rules_with(&rows(), Overlay::default(), &fixture::image(1)).unwrap()
}

fn held(item: &str) -> Result<Orb, String> {
    rules()
        .held_attributes(&catalog_item(item))
        .map(|held| held.unwrap().orb.unwrap())
}

fn refused(rows: [Row; 4], overlay: Overlay) -> String {
    rules_with(&rows, overlay, &fixture::image(1))
        .unwrap()
        .held_attributes(&catalog_item("odef"))
        .unwrap_err()
}

fn step(numerator: u32) -> f32 {
    numerator as f32 / fixture::DENOMINATOR as f32
}

fn near(bits: u32, expected: f32) {
    let value = f32::from_bits(bits);
    assert!(
        (value - expected).abs() <= expected.abs() * 1e-4 + 1e-9,
        "{value} is not {expected}"
    );
}

#[test]
fn orb_of_darkness_reads_its_chances_and_wrapped_black_arrow_from_the_rows() {
    let orb = held("odef").unwrap();
    assert_eq!(orb.damage, 6);
    let OrbKind::Spell(spell) = orb.kind else {
        panic!("a spell orb")
    };
    assert_eq!(spell.channel, 1);
    // A chance of 100 percent is the last step, which is exactly one.
    assert_eq!(spell.increments, [1.0_f32.to_bits(); 3]);
    let WrappedAbility::BlackArrow(arrow) = spell.wrapped else {
        panic!("black arrow")
    };
    assert_eq!(
        (arrow.buff, arrow.marker, arrow.summon),
        (*b"BNba", *b"BNdm", *b"ndr1")
    );
    assert_eq!(
        (arrow.duration, arrow.hero_duration, arrow.range),
        (3.0, 0.0, 600.0)
    );
    assert_eq!((arrow.count, arrow.summon_duration), (1, 80.0));
    assert_eq!(
        arrow.targets,
        WrappedTargets {
            air: true,
            ground: true,
            structure: false,
            enemy: true,
            neutral: true,
            friend: false,
            organic: true,
        }
    );
}

#[test]
fn orb_of_lightning_picks_a_step_per_target_kind_and_wraps_an_undecoded_class() {
    let orb = held("oli2").unwrap();
    let OrbKind::Spell(spell) = orb.kind else {
        panic!("a spell orb")
    };
    // 35 percent is step 7, 10 percent step 2 (10 * 0.2 + 0.5 truncates to 2).
    near(spell.increments[0], step(fixture::NUMERATORS[7]));
    near(spell.increments[1], step(fixture::NUMERATORS[2]));
    near(spell.increments[2], step(fixture::NUMERATORS[7]));
    assert_eq!(
        spell.wrapped,
        WrappedAbility::Undecoded {
            alias: *b"AIpg",
            code: *b"Aprg"
        }
    );
}

#[test]
fn chance_steps_round_clamp_and_floor_like_the_client() {
    let mut edited = rows();
    let cases = [
        ("0", 0),
        ("2", 0),
        ("3", 1),
        ("7", 1),
        ("8", 2),
        ("99", 20),
        ("100", 20),
        ("250", 20),
        ("-5", 0),
    ];
    for (percent, expected) in cases {
        edited[0][3] = percent;
        let orb = rules_with(&edited, Overlay::default(), &fixture::image(1))
            .unwrap()
            .held_attributes(&catalog_item("odef"))
            .unwrap()
            .unwrap()
            .orb
            .unwrap();
        let OrbKind::Spell(spell) = orb.kind else {
            panic!("a spell orb")
        };
        near(
            spell.increments[0],
            step(fixture::NUMERATORS[expected]) + 0.0,
        );
    }
}

#[test]
fn the_roll_stream_comes_from_the_image() {
    for channel in [0, 1, 36] {
        let rules = rules_with(&rows(), Overlay::default(), &fixture::image(channel)).unwrap();
        let orb = rules
            .held_attributes(&catalog_item("odef"))
            .unwrap()
            .unwrap()
            .orb
            .unwrap();
        let OrbKind::Spell(spell) = orb.kind else {
            panic!("a spell orb")
        };
        assert_eq!(spell.channel, usize::from(channel));
    }
}

#[test]
fn rows_outside_the_decoded_shape_refuse() {
    let none = Overlay::default;
    let edited = |row: usize, column: usize, value: &'static str| {
        let mut rows = rows();
        rows[row][column] = value;
        rows
    };
    // The generic spell spends mana and sets a cooldown, neither decoded.
    assert!(refused(edited(2, 9, "10"), none()).contains("Cost1"));
    assert!(refused(edited(2, 8, "2"), none()).contains("Cool1"));
    // The arrow lists the mark and the minion buff, and a rawcode summon.
    assert!(refused(edited(2, 13, "BNba"), none()).contains("BuffID1"));
    assert!(refused(edited(2, 14, "nd"), none()).contains("UnitID1"));
    assert!(refused(edited(2, 3, "1.5"), none()).contains("whole"));
    assert!(refused(edited(2, 10, "enemy,organic"), none()).contains("movement"));
    assert!(refused(edited(2, 10, "air,ground,enemy,ward"), none()).contains("not decoded"));
    // The wrapped alias must be a row, and the orb keeps the shared orb shape.
    assert!(refused(edited(0, 14, "Axxx"), none()).contains("Axxx"));
    assert!(refused(edited(0, 6, "1"), none()).contains("enabled attack index"));
    // A map that modifies the orb or the wrapped row is not decoded.
    for alias in ["AIdf", "ANbs"] {
        let mut overlay = Overlay::default();
        overlay
            .mods
            .insert(alias.into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
        assert!(refused(rows(), overlay).contains("map-modified"), "{alias}");
    }
}

#[test]
fn registration_needs_the_orb_base_and_distinct_classes() {
    let tables = || {
        let rules = rules();
        Arc::clone(&rules.tables)
    };
    let attach = |image: SpellOrbImage| {
        orb_base(tables(), Overlay::default())
            .unwrap()
            .with_spell_orb(&image)
    };
    assert!(attach(fixture::image(1)).is_ok());
    let mut wrong_base = fixture::image(1);
    wrong_base.window = window("AIsb", "APbo");
    assert!(
        attach(wrong_base)
            .err()
            .unwrap()
            .contains("name the orb base")
    );
    let mut duplicate = fixture::image(1);
    duplicate.window = window("AIfb", "AIDB");
    assert!(attach(duplicate).err().unwrap().contains("distinct"));
    // Attaching twice, or without the orb base, is refused.
    let twice = attach(fixture::image(1))
        .unwrap()
        .with_spell_orb(&fixture::image(1));
    assert!(twice.err().unwrap().contains("distinct"));
    let bare = plain_rules_for_spell(tables());
    assert!(
        bare.with_spell_orb(&fixture::image(1))
            .err()
            .unwrap()
            .contains("orb base")
    );
}

fn plain_rules_for_spell(tables: Arc<DefaultTables>) -> Rules {
    Rules::decode(
        tables,
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
    .unwrap()
}

#[test]
fn image_shapes_outside_the_decoded_windows_refuse() {
    let attach = |image: SpellOrbImage| {
        let tables = Arc::clone(&rules().tables);
        orb_base(tables, Overlay::default())
            .unwrap()
            .with_spell_orb(&image)
            .err()
    };
    // The roll must push a stream and call the secondary real draw.
    let mut image = fixture::image(1);
    image.roll[0x8a + 3] = 0x90;
    assert!(attach(image).unwrap().contains("roll function"));
    let mut image = fixture::image(1);
    image.roll[0x8a + 4] ^= 1;
    assert!(attach(image).unwrap().contains("roll function"));
    let mut image = fixture::image(1);
    image.roll.truncate(8);
    assert!(attach(image).unwrap().contains("roll function"));
    // The chance function needs its lookup, clamp, every store, and rising steps ending at one.
    let mut image = fixture::image(1);
    let last = image.chance.len() - 7;
    image.chance[last] = 0x90;
    assert!(attach(image).unwrap().contains("lookup"));
    let mut image = fixture::image(1);
    image.chance[0x5c] = 0x90;
    assert!(attach(image).unwrap().contains("step clamp"));
    let mut falling = fixture::NUMERATORS;
    falling.swap(3, 4);
    assert!(
        attach(fixture::image_with(1, &falling))
            .unwrap()
            .contains("rise")
    );
    let mut short = fixture::NUMERATORS;
    short[20] = 9_999;
    assert!(
        attach(fixture::image_with(1, &short))
            .unwrap()
            .contains("rise")
    );
    let mut image = fixture::image(1);
    image.chance.truncate(0x100);
    assert!(attach(image).unwrap().contains("chance function"));
    // The reciprocal table has its 1025 entries.
    let mut image = fixture::image(1);
    image.reciprocal.truncate(64);
    assert!(attach(image).unwrap().contains("reciprocal table"));
    // The wrapped class registration must be a computed-base window.
    let mut image = fixture::image(1);
    image.black_arrow_window = window("ANbs", "AAsm");
    assert!(attach(image).unwrap().contains("computed-base"));
}

#[test]
fn proc_counters_raise_the_chance_after_a_miss_and_reset_after_a_proc() {
    let increment = 0.1_f32.to_bits();
    let mut counters = ProcCounters::attached();
    let chance = |counters: &ProcCounters, kind| counters.chance(kind, increment).unwrap();
    assert!((chance(&counters, ProcKind::Unit) - 0.1).abs() < 1e-6);
    counters.rolled(ProcKind::Unit, false);
    counters.rolled(ProcKind::Unit, false);
    assert!((chance(&counters, ProcKind::Unit) - 0.3).abs() < 1e-6);
    // The other kinds keep their own counters.
    assert!((chance(&counters, ProcKind::Hero) - 0.1).abs() < 1e-6);
    assert!((chance(&counters, ProcKind::Summon) - 0.1).abs() < 1e-6);
    counters.rolled(ProcKind::Unit, true);
    assert!((chance(&counters, ProcKind::Unit) - 0.1).abs() < 1e-6);
    // A zero increment never raises the chance above zero.
    assert_eq!(counters.chance(ProcKind::Unit, 0).unwrap(), 0.0);
}

/// The numerators of the increment table the reviewed image builds (`0xcaec30`), as read with
/// `objdump` from the immediates before each store.
const IMAGE_NUMERATORS: [u32; 21] = [
    0, 38, 147, 320, 547, 819, 1126, 1461, 1815, 2184, 2562, 2953, 3333, 3781, 4203, 4607, 5000,
    5799, 6666, 7706, 10000,
];

#[test]
#[ignore = "requires PEACECRAFT3_INSTALL (the install directory with Warcraft III.exe)"]
fn local_original_image_supplies_the_stream_the_table_and_the_classes() {
    let install = std::env::var("PEACECRAFT3_INSTALL").unwrap();
    let image = std::fs::read(std::path::Path::new(&install).join("Warcraft III.exe")).unwrap();
    assert_eq!(sha256(&image), REVIEWED_IMAGE);
    let pe = Pe::parse(&image, IMAGE_BUDGET).unwrap();
    let spell = SpellOrbImage::read(&pe).unwrap();
    assert_eq!(
        class_from_window(&spell.window).unwrap(),
        ("AIsb".to_string(), "AIDB".to_string())
    );
    assert_eq!(
        class_from_call_window_at(
            &spell.black_arrow_window,
            image_rules::BLACK_ARROW_WINDOW,
            &spell.black_arrow_base
        )
        .unwrap(),
        ("ANbs".to_string(), "AAsm".to_string())
    );
    assert_eq!(image_rules::roll_channel(&spell.roll).unwrap(), 0x24);
    let (numerators, denominator) = image_rules::step_numerators(&spell.chance).unwrap();
    assert_eq!(
        (numerators.as_slice(), denominator),
        (&IMAGE_NUMERATORS[..], 10_000)
    );
    assert_eq!(f32::from_le_bytes(spell.scale), 0.2);
    assert_eq!(f32::from_le_bytes(spell.round), 0.5);
    // The reviewed divide agrees with the exact quotient to a few units in the last place.
    let table: Vec<u32> = spell
        .reciprocal
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes(*chunk))
        .collect();
    for numerator in &numerators {
        let bits = image_rules::step_bits(*numerator, denominator, &table).unwrap();
        let exact = *numerator as f32 / denominator as f32;
        let got = f32::from_bits(bits);
        assert!(
            (got - exact).abs() <= exact * 2e-6,
            "{numerator}/{denominator} = {got}, exact {exact}"
        );
    }
}
