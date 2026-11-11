//! `ANss` rows and the registration read from the image. The rule under test is the one decoded
//! in `docs/re-notes/passive-held-items.md`, "`ANss`".
use super::*;
use crate::inventory_powerup::spell_shield::fixture;

/// Columns: alias, code, Cool1. `Dur1` is deliberately different from `Cool1`: the cooldown
/// the item text prints is `Cool1`.
type Row = [&'static str; 3];

const AMULET: Row = ["ANss", "ANss", "40"];

fn tables(rows: &[Row], levels: &str) -> Arc<DefaultTables> {
    let ability_rows: Vec<Vec<&str>> = rows
        .iter()
        .map(|row| vec![row[0], row[1], levels, "7", row[2]])
        .collect();
    let refs: Vec<&[&str]> = ability_rows.iter().map(Vec::as_slice).collect();
    let abilities = sylk(&["alias", "code", "levels", "Dur1", "Cool1"], &refs);
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
            &["spsh", "ANss", "0", "0", "0", "1"],
            &["spsx", "ANss,ANsx", "0", "0", "0", "1"],
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
                install: "synthetic spell shield".into(),
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

fn held(item: &str, rows: &[Row], levels: &str) -> Result<Option<SpellShield>, String> {
    plain(tables(rows, levels), Overlay::default())
        .with_spell_shield(&fixture::image())?
        .held_attributes(&catalog_item(item))
        .map(|held| held.unwrap().spell_shield)
}

#[test]
fn the_cooldown_is_the_cool1_column() {
    assert_eq!(
        held("spsh", &[AMULET], "1").unwrap(),
        Some(SpellShield { cooldown: 40.0 })
    );
    assert_eq!(
        held("spsh", &[["ANss", "ANss", "12.5"]], "1")
            .unwrap()
            .unwrap()
            .cooldown,
        12.5
    );
    // An absent cooldown reads as zero, as every ability number does.
    assert_eq!(
        held("spsh", &[["ANss", "ANss", "-"]], "1")
            .unwrap()
            .unwrap()
            .cooldown,
        0.0
    );
}

#[test]
fn rows_outside_the_decoded_shape_refuse() {
    assert!(
        held("spsh", &[["ANss", "ANss", "-1"]], "1")
            .unwrap_err()
            .contains("negative")
    );
    assert!(
        held("spsh", &[["ANss", "ANss", "x"]], "1")
            .unwrap_err()
            .contains("Cool1")
    );
    assert!(
        held("spsh", &[AMULET], "2")
            .unwrap_err()
            .contains("multi-level")
    );
    // Two abilities of the class on one item are not decoded, and a map edit is not either.
    let two = held("spsx", &[AMULET, ["ANsx", "ANss", "30"]], "1").unwrap_err();
    assert!(two.contains("two spell shield"), "{two}");
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("ANss".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    let error = plain(tables(&[AMULET], "1"), overlay)
        .with_spell_shield(&fixture::image())
        .unwrap()
        .held_attributes(&catalog_item("spsh"))
        .unwrap_err();
    assert!(error.contains("map-modified"), "{error}");
}

#[test]
fn registration_reads_the_class_from_the_image() {
    let attach = |image: SpellShieldImage| {
        plain(tables(&[AMULET], "1"), Overlay::default())
            .with_spell_shield(&image)
            .err()
    };
    assert_eq!(attach(fixture::image()), None);
    let mut duplicate = fixture::image();
    duplicate.window = crate::inventory_powerup::spell_orb::fixture::computed_at(
        "AIsm",
        crate::inventory_powerup::spell_shield::SPELL_SHIELD_WINDOW,
    );
    assert!(attach(duplicate).unwrap().contains("distinct"));
    let mut truncated = fixture::image();
    truncated.window.pop();
    assert!(attach(truncated).is_some());
    let twice = plain(tables(&[AMULET], "1"), Overlay::default())
        .with_spell_shield(&fixture::image())
        .unwrap()
        .with_spell_shield(&fixture::image());
    assert!(twice.err().unwrap().contains("distinct"));
    // Without the registration the class stays an unsupported passive ability.
    let error = plain(tables(&[AMULET], "1"), Overlay::default())
        .held_attributes(&catalog_item("spsh"))
        .unwrap_err();
    assert!(
        error.contains("unsupported passive ability ANss"),
        "{error}"
    );
}

/// The one real-install check of this change: the Spell Shield registration and the game type
/// launch flags (`game_type_rules`), both read from the reviewed image.
#[test]
#[ignore = "requires PEACECRAFT3_INSTALL (the install directory with Warcraft III.exe)"]
fn local_original_image_supplies_the_spell_shield_class_and_the_launch_flags() {
    let install = std::env::var("PEACECRAFT3_INSTALL").unwrap();
    let path = std::path::Path::new(&install).join("Warcraft III.exe");
    let image = std::fs::read(&path).unwrap();
    assert_eq!(sha256(&image), REVIEWED_IMAGE);
    let pe = Pe::parse(&image, IMAGE_BUDGET).unwrap();
    let shield = SpellShieldImage::read(&pe).unwrap();
    assert_eq!(
        class_from_call_window_at(
            &shield.window,
            crate::inventory_powerup::spell_shield::SPELL_SHIELD_WINDOW,
            &shield.base
        )
        .unwrap(),
        ("ANss".to_string(), "AAsm".to_string())
    );
    let rules = crate::game_type_rules::Rules::reviewed_legacy(
        &image,
        AssetIdentity {
            source: path.display().to_string(),
            edition: "selected Legacy executable".into(),
            version: "VERSIONINFO 1.29.2.9231".into(),
        },
    )
    .unwrap();
    use crate::game_type_rules::LaunchRoute;
    assert_eq!(rules.selected(LaunchRoute::CreditsButton), 1);
    assert_eq!(rules.selected(LaunchRoute::SinglePlayerStart), 0x100);
}
