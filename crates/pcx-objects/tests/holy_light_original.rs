//! Explicit local gate for the Holy Light data read by `pcx-play`: the `AHhb` row, its
//! `AbilityFunc` order, the `UnitBalance` `type` column kind, and an undead unit's type list.
//! Run with `PEACECRAFT3_LEGACY_DIR` set to the selected install directory:
//! `cargo test -p pcx-objects --test holy_light_original -- --ignored`
use pcx_objects::editor::defaults::{DefaultTables, Family, Value};
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use std::path::PathBuf;

/// Legacy resource version of the selected executable.
const EDITION_VERSION: &str = "1.29.2.9231";

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR (the selected Warcraft III Legacy install directory)"]
fn holy_light_row_order_and_undead_type_come_from_the_install() {
    let install = PathBuf::from(
        std::env::var_os("PEACECRAFT3_LEGACY_DIR")
            .expect("set PEACECRAFT3_LEGACY_DIR to the Legacy install directory"),
    );
    let tables = DefaultTables::open(&install, EDITION_VERSION).expect("install tables");

    let ability = tables
        .object(Family::Ability, "AHhb")
        .expect("AHhb ability row");
    let number = |column: &str| match ability
        .cell(SlkId::AbilityData, column)
        .expect("AbilityData cell")
        .typed(FieldKind::Int)
        .expect("integer cell")
    {
        Value::Integer(n) => n,
        other => panic!("{column}: {other:?}"),
    };
    assert_eq!(number("levels"), 3);
    assert_eq!(
        [number("DataA1"), number("DataA2"), number("DataA3")],
        [200, 400, 600]
    );
    assert_eq!(number("Cost1"), 65);
    assert_eq!(number("Cool1"), 5);
    assert_eq!(number("Rng1"), 800);

    let orders: Vec<&str> = ability
        .profile("Order")
        .iter()
        .map(|cell| cell.text.trim())
        .collect();
    assert_eq!(orders, ["holybolt"]);

    let hotkeys: Vec<&str> = ability
        .profile("Hotkey")
        .iter()
        .map(|cell| cell.text.trim())
        .collect();
    assert_eq!(hotkeys, ["T"]);

    let type_kinds: Vec<FieldKind> = tables
        .editor()
        .fields()
        .iter()
        .filter(|field| {
            field.from == SlkId::UnitMetaData
                && matches!(&field.source, FieldSource::Column { slk: SlkId::UnitBalance, column } if column == "type")
        })
        .map(|field| field.kind)
        .collect();
    assert_eq!(type_kinds, [FieldKind::Token]);

    let ghoul = tables.object(Family::Unit, "ugho").expect("ghoul unit row");
    let classes = ghoul
        .cell(SlkId::UnitBalance, "type")
        .expect("UnitBalance type cell")
        .text;
    assert!(
        classes.split(',').any(|class| class.trim() == "undead"),
        "{classes}"
    );
}
