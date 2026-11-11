//! `AIrc` rows, the restored amounts and the registrations read from the image. The rule under
//! test is the one decoded in `docs/re-notes/passive-held-items.md`, "`AIrc`".
use super::*;
use crate::inventory_powerup::reincarnation::fixture;

/// Columns: alias, code, DataA1, DataB1, DataC1, EfctID1.
type Row = [&'static str; 6];

const ANKH: Row = ["AIrc", "AIrc", "7", "500", "-1", "XOre"];

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
            "alias", "code", "levels", "DataA1", "DataB1", "DataC1", "EfctID1",
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
                install: "synthetic reincarnation".into(),
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
    &["ankh", "AIrc", "0", "0", "1", "1"],
    &["ank2", "AIrc,AIr2", "0", "0", "1", "1"],
];

fn plain(tables: Arc<DefaultTables>, overlay: Overlay) -> Rules {
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
    )
    .unwrap()
}

fn rules_over(rows: &[Row], overlay: Overlay) -> Result<Rules, String> {
    plain(tables(rows, ITEMS), overlay).with_reincarnation(&fixture::image())
}

fn rules() -> Rules {
    rules_over(
        &[ANKH, ["AIr2", "AIrc", "3", "0", "50", "XOre"]],
        Overlay::default(),
    )
    .unwrap()
}

fn held(item: &str, row: Row) -> Result<Option<Reincarnation>, String> {
    rules_over(
        &[row, ["AIr2", "AIrc", "3", "0", "50", "XOre"]],
        Overlay::default(),
    )?
    .held_attributes(&catalog_item(item))
    .map(|held| held.unwrap().reincarnation)
}

#[test]
fn the_shipped_row_values_are_read_per_item() {
    let spec = held("ankh", ANKH).unwrap().unwrap();
    assert_eq!(
        spec,
        Reincarnation {
            delay: 7.0,
            life: 500.0,
            mana: -1.0
        }
    );
}

#[test]
fn a_negative_data_b_is_raised_to_zero_and_zero_means_the_maximum() {
    let mut row = ANKH;
    row[3] = "-40";
    let spec = held("ankh", row).unwrap().unwrap();
    assert_eq!(spec.life, 0.0);
    assert_eq!(spec.restored_life(900.0), 900.0);
}

#[test]
fn the_restored_life_is_the_row_value_with_a_floor_and_the_maximum_as_its_ceiling() {
    let at = |life: f32| Reincarnation {
        delay: 7.0,
        life,
        mana: -1.0,
    };
    assert_eq!(at(500.0).restored_life(900.0), 500.0);
    assert_eq!(at(500.0).restored_life(300.0), 300.0);
    // Within 0.001 of zero is the unset value: the maximum.
    assert_eq!(at(0.0005).restored_life(900.0), 900.0);
    // Anything else raises to half a point.
    assert_eq!(at(0.2).restored_life(900.0), 0.5);
}

#[test]
fn a_negative_data_c_keeps_the_mana_at_death_and_otherwise_sets_it() {
    let at = |mana: f32| Reincarnation {
        delay: 7.0,
        life: 500.0,
        mana,
    };
    assert_eq!(at(-1.0).restored_mana(120.0, 300.0), 120.0);
    assert_eq!(at(80.0).restored_mana(120.0, 300.0), 80.0);
    assert_eq!(at(0.0).restored_mana(120.0, 300.0), 0.0);
    assert_eq!(at(900.0).restored_mana(120.0, 300.0), 300.0);
    assert_eq!(at(-1.0).restored_mana(500.0, 300.0), 300.0);
}

#[test]
fn rows_outside_the_decoded_shape_refuse() {
    let edited = |column: usize, value: &'static str| {
        let mut row = ANKH;
        row[column] = value;
        held("ankh", row)
    };
    assert!(edited(2, "-1").unwrap_err().contains("negative"));
    assert!(edited(2, "x").unwrap_err().contains("DataA1"));
    assert!(edited(3, "x").unwrap_err().contains("DataB1"));
    assert!(edited(4, "inf").unwrap_err().contains("DataC1"));
    assert!(edited(5, "XO").unwrap_err().contains("EfctID1"));
    // Two abilities of the class on one item are not decoded, and a map edit is not either.
    assert!(
        rules()
            .held_attributes(&catalog_item("ank2"))
            .unwrap_err()
            .contains("two reincarnation")
    );
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIrc".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    let error = rules_over(&[ANKH], overlay)
        .unwrap()
        .held_attributes(&catalog_item("ankh"))
        .unwrap_err();
    assert!(error.contains("map-modified"), "{error}");
}

#[test]
fn registration_reads_the_classes_from_the_image_and_ranks_the_hero_classes() {
    let rules = rules();
    assert!(rules.is_hero_reincarnation("AOre"));
    assert!(rules.is_hero_reincarnation("ACrn"));
    assert!(!rules.is_hero_reincarnation("AIrc"));
    let tables = || Arc::clone(&rules.tables);
    let attach = |image: ReincarnationImage| {
        plain(tables(), Overlay::default())
            .with_reincarnation(&image)
            .err()
    };
    assert_eq!(attach(fixture::image()), None);
    let mut wrong_creep = fixture::image();
    wrong_creep.creep_window = window("ACrn", "APbo");
    assert!(
        attach(wrong_creep)
            .unwrap()
            .contains("must name the hero class")
    );
    let mut duplicate = fixture::image();
    duplicate.ankh_window = crate::inventory_powerup::spell_orb::fixture::computed_at(
        "AIsm",
        crate::inventory_powerup::reincarnation::ANKH_WINDOW,
    );
    assert!(attach(duplicate).unwrap().contains("distinct"));
    let twice = plain(tables(), Overlay::default())
        .with_reincarnation(&fixture::image())
        .unwrap()
        .with_reincarnation(&fixture::image());
    assert!(twice.err().unwrap().contains("distinct"));
    // Without the registration the class stays an unsupported passive ability.
    let bare = plain(tables(), Overlay::default());
    let error = bare.held_attributes(&catalog_item("ankh")).unwrap_err();
    assert!(
        error.contains("unsupported passive ability AIrc"),
        "{error}"
    );
    assert!(!bare.is_hero_reincarnation("AOre"));
}

#[test]
#[ignore = "requires PEACECRAFT3_INSTALL (the install directory with Warcraft III.exe)"]
fn local_original_image_supplies_the_reincarnation_classes() {
    let install = std::env::var("PEACECRAFT3_INSTALL").unwrap();
    let image = std::fs::read(std::path::Path::new(&install).join("Warcraft III.exe")).unwrap();
    assert_eq!(sha256(&image), REVIEWED_IMAGE);
    let pe = Pe::parse(&image, IMAGE_BUDGET).unwrap();
    let classes = ReincarnationImage::read(&pe).unwrap();
    assert_eq!(
        class_from_call_window_at(
            &classes.ankh_window,
            crate::inventory_powerup::reincarnation::ANKH_WINDOW,
            &classes.ankh_base
        )
        .unwrap(),
        ("AIrc".to_string(), "abil".to_string())
    );
    assert_eq!(
        class_from_call_window_at(
            &classes.hero_window,
            crate::inventory_powerup::reincarnation::HERO_WINDOW,
            &classes.hero_base
        )
        .unwrap(),
        ("AOre".to_string(), "AAsp".to_string())
    );
    assert_eq!(
        class_from_window(&classes.creep_window).unwrap(),
        ("ACrn".to_string(), "AOre".to_string())
    );
}
