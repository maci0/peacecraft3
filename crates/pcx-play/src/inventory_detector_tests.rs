//! `Adet` rows, the registration and the radius field name read from the image. The rule under
//! test is the one decoded in `docs/re-notes/passive-held-items.md`, "`Adet`".
use super::*;
use crate::inventory_powerup::detector::fixture;

/// Columns: alias, code, Rng1, DataA1. `Area1` is deliberately different from `Rng1`: the client
/// reads the radius from the range column.
type Row = [&'static str; 4];

const GEM: Row = ["Adt1", "Adet", "1100", "3"];

fn tables(rows: &[Row]) -> Arc<DefaultTables> {
    let ability_rows: Vec<Vec<&str>> = rows
        .iter()
        .map(|row| vec![row[0], row[1], "1", "250", row[2], row[3]])
        .collect();
    let refs: Vec<&[&str]> = ability_rows.iter().map(Vec::as_slice).collect();
    let abilities = sylk(
        &["alias", "code", "levels", "Area1", "Rng1", "DataA1"],
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
            &["gemt", "Adt1", "0", "0", "0", "1"],
            &["gem2", "Adt1,Adt2", "0", "0", "0", "1"],
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
    Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic detector".into(),
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
    plain(tables(rows), overlay).with_detector(&fixture::image())
}

fn held(item: &str, row: Row) -> Result<Option<Detector>, String> {
    rules_over(&[row, ["Adt2", "Adet", "900", "1"]], Overlay::default())?
        .held_attributes(&catalog_item(item))
        .map(|held| held.unwrap().detector)
}

#[test]
fn the_radius_is_the_range_column_and_the_types_are_data_a() {
    assert_eq!(
        held("gemt", GEM).unwrap(),
        Some(Detector {
            radius: 1100.0,
            types: 3
        })
    );
    let mut single = GEM;
    single[3] = "1";
    assert_eq!(held("gemt", single).unwrap().unwrap().types, 1);
    let mut none = GEM;
    none[3] = "0";
    assert_eq!(held("gemt", none).unwrap().unwrap().types, 0);
}

#[test]
fn the_radius_column_follows_the_name_read_from_the_image() {
    let mut image = fixture::image();
    image.radius_field = "Area".into();
    let rules = plain(tables(&[GEM]), Overlay::default())
        .with_detector(&image)
        .unwrap();
    let detector = rules
        .held_attributes(&catalog_item("gemt"))
        .unwrap()
        .unwrap()
        .detector
        .unwrap();
    assert_eq!(detector.radius, 250.0);
}

#[test]
fn rows_outside_the_decoded_shape_refuse() {
    let edited = |column: usize, value: &'static str| {
        let mut row = GEM;
        row[column] = value;
        held("gemt", row)
    };
    assert!(edited(2, "-1").unwrap_err().contains("negative"));
    assert!(edited(2, "x").unwrap_err().contains("Rng1"));
    assert!(edited(2, "inf").unwrap_err().contains("Rng1"));
    assert!(edited(3, "4").unwrap_err().contains("DataA1"));
    assert!(edited(3, "-1").unwrap_err().contains("DataA1"));
    assert!(edited(3, "x").unwrap_err().contains("DataA1"));
    // Two abilities of the class on one item are not decoded, and a map edit is not either.
    let two = rules_over(&[GEM, ["Adt2", "Adet", "900", "1"]], Overlay::default()).unwrap();
    assert!(
        two.held_attributes(&catalog_item("gem2"))
            .unwrap_err()
            .contains("two detector")
    );
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("Adt1".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    let error = rules_over(&[GEM], overlay)
        .unwrap()
        .held_attributes(&catalog_item("gemt"))
        .unwrap_err();
    assert!(error.contains("map-modified"), "{error}");
}

#[test]
fn registration_reads_the_class_and_the_radius_name_from_the_image() {
    let attach = |image: DetectorImage| {
        plain(tables(&[GEM]), Overlay::default())
            .with_detector(&image)
            .err()
    };
    assert_eq!(attach(fixture::image()), None);
    let mut duplicate = fixture::image();
    duplicate.window = crate::inventory_powerup::spell_orb::fixture::computed_at(
        "AIsm",
        crate::inventory_powerup::detector::DETECTOR_WINDOW,
    );
    assert!(attach(duplicate).unwrap().contains("distinct"));
    for name in ["", "R1g", "Rng\0"] {
        let mut image = fixture::image();
        image.radius_field = name.into();
        assert!(attach(image).unwrap().contains("not letters"), "{name:?}");
    }
    let mut truncated = fixture::image();
    truncated.window.pop();
    assert!(attach(truncated).is_some());
    let twice = plain(tables(&[GEM]), Overlay::default())
        .with_detector(&fixture::image())
        .unwrap()
        .with_detector(&fixture::image());
    assert!(twice.err().unwrap().contains("distinct"));
    // Without the registration the class stays an unsupported passive ability.
    let error = plain(tables(&[GEM]), Overlay::default())
        .held_attributes(&catalog_item("gemt"))
        .unwrap_err();
    assert!(
        error.contains("unsupported passive ability Adt1"),
        "{error}"
    );
}

#[test]
#[ignore = "requires PEACECRAFT3_INSTALL (the install directory with Warcraft III.exe)"]
fn local_original_image_supplies_the_detector_class_and_radius_name() {
    let install = std::env::var("PEACECRAFT3_INSTALL").unwrap();
    let image = std::fs::read(std::path::Path::new(&install).join("Warcraft III.exe")).unwrap();
    assert_eq!(sha256(&image), REVIEWED_IMAGE);
    let pe = Pe::parse(&image, IMAGE_BUDGET).unwrap();
    let detector = DetectorImage::read(&pe).unwrap();
    assert_eq!(
        class_from_call_window_at(
            &detector.window,
            crate::inventory_powerup::detector::DETECTOR_WINDOW,
            &detector.base
        )
        .unwrap(),
        ("Adet".to_string(), "APas".to_string())
    );
    assert_eq!(detector.radius_field, "Rng");
}
