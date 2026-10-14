use super::{META, fixture, table};
use crate::editor::SlkId;
use crate::editor::defaults::{Family, Location, Refusal, Value};
use crate::requirements::{CatalogError, RequirementsCatalog};
use std::collections::BTreeMap;

#[test]
fn selected_fields_preserve_outcomes_and_both_sources() {
    let tables = fixture(
        BTreeMap::from([
            (
                SlkId::UnitData,
                table(&["unitID", "Amount"], &[&["X001", "7"], &["X002", "bad"]]),
            ),
            (
                SlkId::UnitMetaData,
                table(
                    META,
                    &[
                        &[
                            "Q101", "Amount", "UnitData", "-1", "int", "fixture", "0", "0", "",
                        ],
                        &[
                            "Q102", "Absent", "UnitData", "-1", "int", "fixture", "0", "0", "",
                        ],
                        &[
                            "Q103", "Note", "Profile", "0", "string", "fixture", "0", "0", "",
                        ],
                        &[
                            "Q104", "Amount", "UnitData", "-1", "int", "fixture", "2", "0", "",
                        ],
                    ],
                ),
            ),
        ]),
        &[
            (r"Units\FixtureUnitStrings.txt", "[X001]\nNote=one\n"),
            (r"Units\OtherUnitStrings.txt", "[X001]\nNote=two\n"),
        ],
    );
    let catalog =
        RequirementsCatalog::new(&tables, &[*b"Q101", *b"Q102", *b"Q103", *b"Q104"]).unwrap();
    assert!(std::ptr::eq(catalog.tables(), &tables));
    assert_eq!(catalog.len(), 8);
    assert_eq!(catalog.for_family(Family::Unit).len(), 8);
    assert!(catalog.for_family(Family::Ability).is_empty());
    let first = catalog.for_rawcode(Family::Unit, "X001");
    let mapped = first[0].outcome.as_ref().unwrap();
    assert_eq!(mapped.value, Value::Integer(7));
    assert_eq!(mapped.cell.text, "7");
    assert!(std::ptr::eq(mapped.definition, first[0].field));
    assert_eq!(mapped.cell.provenance.asset.member, SlkId::UnitData.path());
    assert_eq!(mapped.cell.provenance.asset.archive, "synthetic.mpq");
    assert_eq!(mapped.cell.provenance.identity, tables.identity());
    assert_eq!(
        mapped.cell.provenance.location,
        Location::Slk {
            row: 2,
            column: "Amount".into()
        }
    );
    let metadata_id = mapped.metadata.attribute("ID").unwrap();
    assert_eq!(metadata_id.text, "Q101");
    assert_eq!(
        metadata_id.provenance.asset.member,
        SlkId::UnitMetaData.path()
    );
    assert_eq!(metadata_id.provenance.asset.archive, "synthetic.mpq");
    assert_eq!(metadata_id.provenance.identity, tables.identity());
    assert!(
        matches!(&first[1].outcome, Err(Refusal::MissingCell { column, .. }) if column == "Absent")
    );
    assert!(
        matches!(&first[2].outcome, Err(Refusal::AmbiguousProfile { members, .. }) if members.len() == 2)
    );
    assert!(matches!(&first[3].outcome, Err(Refusal::Mapping { fourcc, .. }) if fourcc == b"Q104"));
    let second = catalog.for_rawcode(Family::Unit, "X002");
    assert!(
        matches!(&second[0].outcome, Err(Refusal::MalformedValue { value, .. }) if &**value == "bad")
    );
    assert!(matches!(
        &second[2].outcome,
        Err(Refusal::MissingCell { .. })
    ));
}

#[test]
fn unknown_selection_is_an_error_even_without_objects() {
    let tables = fixture(BTreeMap::new(), &[]);
    assert!(RequirementsCatalog::new(&tables, &[]).unwrap().is_empty());
    assert!(
        matches!(RequirementsCatalog::new(&tables, &[*b"ZZZZ"]), Err(CatalogError::UnknownField { fourcc }) if fourcc == *b"ZZZZ")
    );
}

#[test]
fn selections_keep_order_duplicates_and_metadata_family() {
    let tables = fixture(
        BTreeMap::from([
            (
                SlkId::UnitData,
                table(&["unitID", "Amount"], &[&["X001", "9"]]),
            ),
            (
                SlkId::AbilityData,
                table(&["alias", "Amount"], &[&["A001", "11"]]),
            ),
            (
                SlkId::UnitMetaData,
                table(
                    META,
                    &[
                        &[
                            "Q101", "Amount", "UnitData", "-1", "int", "other", "0", "0", "",
                        ],
                        &[
                            "Q102", "Absent", "UnitData", "-1", "int", "other", "0", "0", "",
                        ],
                    ],
                ),
            ),
            (
                SlkId::AbilityMetaData,
                table(
                    META,
                    &[&[
                        "R101",
                        "Amount",
                        "AbilityData",
                        "-1",
                        "int",
                        "other",
                        "0",
                        "0",
                        "",
                    ]],
                ),
            ),
        ]),
        &[],
    );
    let catalog =
        RequirementsCatalog::new(&tables, &[*b"Q102", *b"R101", *b"Q101", *b"Q102"]).unwrap();
    let unit = catalog.for_rawcode(Family::Unit, "X001");
    assert_eq!(
        unit.iter()
            .map(|entry| entry.field.fourcc)
            .collect::<Vec<_>>(),
        [*b"Q102", *b"Q101", *b"Q102"]
    );
    let ability = catalog.for_family(Family::Ability);
    assert_eq!(ability.len(), 1);
    assert_eq!(ability[0].field.fourcc, *b"R101");
    assert_eq!(
        ability[0].outcome.as_ref().unwrap().value,
        Value::Integer(11)
    );
    assert_eq!(catalog.entries().len(), 4);
    assert!(
        matches!(RequirementsCatalog::new(&tables, &[*b"Q101", *b"ZZZZ"]), Err(CatalogError::UnknownField { fourcc }) if fourcc == *b"ZZZZ")
    );
}
