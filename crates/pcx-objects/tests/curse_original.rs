//! Explicit local gate for the Slow and Polymorph data read by `pcx-play`: the `Aslo` and `Aply`
//! rows, their metadata ordinals, `AbilityFunc` orders and the buff rows.
//! Run with `PEACECRAFT3_LEGACY_DIR` set to the selected install directory:
//! `cargo test -p pcx-objects --test curse_original -- --ignored`
use pcx_objects::editor::SlkId;
use pcx_objects::editor::defaults::{DefaultTables, Family};
use std::path::PathBuf;

/// Legacy resource version of the selected executable.
const EDITION_VERSION: &str = "1.29.2.9231";

fn tables() -> DefaultTables {
    let install = PathBuf::from(
        std::env::var_os("PEACECRAFT3_LEGACY_DIR")
            .expect("set PEACECRAFT3_LEGACY_DIR to the Legacy install directory"),
    );
    DefaultTables::open(&install, EDITION_VERSION).expect("install tables")
}

fn data(tables: &DefaultTables, alias: &str, column: &str) -> String {
    tables
        .object(Family::Ability, alias)
        .expect("ability row")
        .cell(SlkId::AbilityData, column)
        .unwrap_or_else(|e| panic!("{alias} {column}: {e}"))
        .text
        .trim()
        .to_owned()
}

fn profile(tables: &DefaultTables, family: Family, alias: &str, key: &str) -> Vec<String> {
    tables
        .object(family, alias)
        .expect("row")
        .profile(key)
        .iter()
        .map(|cell| cell.text.trim().to_owned())
        .collect()
}

fn metadata(tables: &DefaultTables, id: &str, column: &str) -> String {
    tables
        .metadata(id.as_bytes().try_into().expect("four bytes"))
        .unwrap_or_else(|| panic!("{id} metadata"))
        .attribute(column)
        .unwrap_or_else(|| panic!("{id} {column}"))
        .text
        .trim()
        .to_owned()
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR (the selected Warcraft III Legacy install directory)"]
fn slow_and_polymorph_rows_orders_buffs_and_metadata_come_from_the_install() {
    let tables = tables();
    for (alias, order, buff, hotkey) in [
        ("Aslo", "slow", "Bslo", "W"),
        ("Aply", "polymorph", "Bply", "O"),
    ] {
        assert_eq!(data(&tables, alias, "code"), alias);
        assert_eq!(data(&tables, alias, "levels"), "1");
        assert_eq!(data(&tables, alias, "BuffID1"), buff);
        assert_eq!(profile(&tables, Family::Ability, alias, "Order"), [order]);
        assert_eq!(profile(&tables, Family::Ability, alias, "Hotkey"), [hotkey]);
        assert!(tables.object(Family::Buff, buff).is_ok(), "{buff} buff row");
        // Cast time is zero: the host refuses any other ranked value.
        assert_eq!(data(&tables, alias, "Cast1"), "0");
    }
    assert_eq!(data(&tables, "Aslo", "targs1"), "air,ground,enemy");
    assert_eq!(
        data(&tables, "Aply", "targs1"),
        "air,ground,nonhero,enemy,organic,neutral"
    );
    assert_eq!(
        profile(&tables, Family::Ability, "Aply", "Requires"),
        ["Rhst"]
    );
    assert_eq!(
        profile(&tables, Family::Ability, "Aply", "Requiresamount"),
        ["2"]
    );
    assert!(profile(&tables, Family::Ability, "Aslo", "Requires").is_empty());
    assert_eq!(
        profile(&tables, Family::Buff, "Bslo", "Targetart"),
        ["Abilities\\Spells\\Human\\slow\\slowtarget.mdl"]
    );
    assert!(profile(&tables, Family::Buff, "Bslo", "Targetattach").is_empty());

    // The effect columns are chosen by the metadata ordinals.
    for (id, ordinal, kind) in [
        ("Slo1", "1", "unreal"),
        ("Slo2", "2", "unreal"),
        ("Ply1", "1", "int"),
        ("Ply2", "2", "unitList"),
        ("Ply3", "3", "unitList"),
        ("Ply4", "4", "unitList"),
        ("Ply5", "5", "unitList"),
    ] {
        assert_eq!(metadata(&tables, id, "data"), ordinal, "{id}");
        assert_eq!(metadata(&tables, id, "type"), kind, "{id}");
    }
    for id in ["Slo1", "Slo2"] {
        let users = metadata(&tables, id, "useSpecific");
        assert!(users.split(',').any(|alias| alias == "Aslo"), "{users}");
    }
    for id in ["Ply1", "Ply2", "Ply3", "Ply4", "Ply5"] {
        let users = metadata(&tables, id, "useSpecific");
        assert!(users.split(',').any(|alias| alias == "Aply"), "{users}");
    }

    // The factors, durations and forms the Human02 interlude casts with.
    assert_eq!(data(&tables, "Aslo", "DataA1"), "0.6");
    assert_eq!(data(&tables, "Aslo", "DataB1"), "0.25");
    assert_eq!(data(&tables, "Aslo", "Dur1"), "60");
    assert_eq!(data(&tables, "Aslo", "HeroDur1"), "10");
    assert_eq!(data(&tables, "Aply", "DataA1"), "5");
    assert_eq!(
        [
            data(&tables, "Aply", "DataB1"),
            data(&tables, "Aply", "DataC1"),
            data(&tables, "Aply", "DataD1"),
            data(&tables, "Aply", "DataE1"),
        ],
        ["nshe", "nshf", "nsha", "nshw"]
    );
}
