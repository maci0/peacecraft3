//! Synthetic joins use no original table contents; the local gate requires explicit assets.
use std::{convert::Infallible, path::Path};

use pcx_objects::editor::defaults::{DefaultTables, Family, Identity, Location, Unresolved};
use pcx_objects::editor::{Editor, EditorReadLimits, FieldSource, SlkId};

type Inputs = Vec<(SlkId, Vec<u8>)>;

fn table(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in std::iter::once(headers)
        .chain(rows.iter().copied())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn limits() -> EditorReadLimits {
    EditorReadLimits {
        max_listed_paths: 16,
        max_asset_bytes: 64 << 10,
        max_aggregate_input_bytes: 1 << 20,
    }
}

fn inputs(weapons: Vec<u8>) -> Inputs {
    vec![
        (
            SlkId::UnitData,
            table(
                &["race", "unitID"],
                &[&["synthetic", "Q001"], &["synthetic", "Q002"]],
            ),
        ),
        (
            SlkId::UnitBalance,
            table(
                &["HP", "unitBalanceID"],
                &[&["23", "q002"], &["31", "q001"]],
            ),
        ),
        (SlkId::UnitWeapons, weapons),
        (
            SlkId::UnitMetaData,
            table(
                &["ID", "field", "slk", "index", "type", "category"],
                &[&[
                    "uaen",
                    "weapsOn",
                    "UnitWeapons",
                    "-1",
                    "attackBits",
                    "synthetic",
                ]],
            ),
        ),
    ]
}

fn editor(inputs: &Inputs) -> Result<Editor, pcx_objects::Error> {
    Editor::from_reader::<Infallible>(Path::new("synthetic row join"), &[], limits(), |member| {
        Ok(inputs
            .iter()
            .find(|(id, _)| id.path() == member)
            .map(|(_, bytes)| bytes.clone()))
    })
}

fn defaults(inputs: &Inputs) -> Result<DefaultTables, Unresolved> {
    DefaultTables::from_bytes(
        Identity {
            install: "synthetic row join".into(),
            edition_version: "synthetic revision".into(),
        },
        &inputs
            .iter()
            .map(|(id, bytes)| (id.path(), bytes.as_slice()))
            .collect::<Vec<_>>(),
        &[],
        limits(),
    )
}

#[test]
fn changed_header_and_reordered_columns_rows_join_both_consumers_without_aliases() {
    let input = inputs(table(
        &["weapsOn", "arbitrary authored label", "rangeN1"],
        &[
            &["2", "q002", "70"],
            &["1", "Q001", "91"],
            &["0", "Q003", "0"],
        ],
    ));
    let editor = editor(&input).unwrap();
    assert_eq!(editor.row(SlkId::UnitWeapons, "q001"), Some(3));
    assert_eq!(
        editor
            .unit("Q001")
            .unwrap()
            .stat(*b"uaen")
            .unwrap()
            .number(),
        Some(1.0)
    );
    assert_eq!(
        editor
            .unit("Q002")
            .unwrap()
            .stat(*b"uaen")
            .unwrap()
            .number(),
        Some(2.0)
    );
    // Additional valid weapon rows remain authored rows, without inventing a canonical unit.
    assert!(editor.row(SlkId::UnitWeapons, "Q003").is_some());
    assert!(editor.unit("Q003").is_none());
    let defaults = defaults(&input).unwrap();
    let object = defaults.object(Family::Unit, "Q001").unwrap();
    let cell = object.cell(SlkId::UnitWeapons, "weapsOn").unwrap();
    assert_eq!(cell.text, "1");
    assert_eq!(
        cell.provenance.location,
        Location::Slk {
            row: 3,
            column: "weapsOn".into()
        }
    );
    assert_eq!(cell.provenance.asset.fnv1a64, pcx_mpq::fnv1a64(&input[2].1));
    assert!(defaults.unresolved().iter().all(|issue| !matches!(issue, Unresolved::Row { member, .. } if member == SlkId::UnitWeapons.path())));
}

#[test]
fn one_canonical_table_is_enough_but_an_unproven_partial_join_is_not() {
    for kept in [SlkId::UnitData, SlkId::UnitBalance] {
        let mut input = inputs(table(
            &["different label", "weapsOn"],
            &[&["Q002", "0"], &["Q001", "1"]],
        ));
        input.retain(|(id, _)| !matches!(id, SlkId::UnitData | SlkId::UnitBalance) || *id == kept);
        assert!(editor(&input).is_ok());
        assert!(defaults(&input).is_ok());
    }
    let partial = inputs(table(&["different label", "weapsOn"], &[&["Q001", "1"]]));
    assert!(
        editor(&partial)
            .unwrap_err()
            .to_string()
            .contains("complete unique fourcc join")
    );
}

#[test]
fn ambiguous_columns_missing_keys_and_invalid_or_duplicate_source_keys_refuse() {
    let ambiguous = inputs(table(
        &["first candidate", "other candidate", "weapsOn"],
        &[&["Q001", "Q002", "1"], &["Q002", "Q001", "0"]],
    ));
    for error in [
        editor(&ambiguous).unwrap_err().to_string(),
        defaults(&ambiguous).unwrap_err().to_string(),
    ] {
        assert!(error.contains("ambiguous identity columns"), "{error}");
        assert!(error.contains(SlkId::UnitWeapons.path()), "{error}");
    }
    for rows in [
        vec![vec!["1", "Q001"], vec!["0", "Q002"], vec!["0", "q001"]],
        vec![vec!["1", "Q001"], vec!["0", "Q002"], vec!["0", "bad"]],
        vec![vec!["1", "Q001"], vec!["0", "Q002"], vec!["0", "é01"]],
        vec![vec!["1", "Q001"], vec!["0"]],
        vec![vec!["1", "Q001"], vec!["0", "Q004"]],
    ] {
        let rows: Vec<_> = rows.iter().map(Vec::as_slice).collect();
        let input = inputs(table(&["weapsOn", "unfamiliar label"], &rows));
        assert!(editor(&input).is_err());
        assert!(defaults(&input).is_err());
    }
}

#[test]
fn canonical_key_failures_and_missing_evidence_keep_context() {
    let weapons = table(
        &["not a known alias", "weapsOn"],
        &[&["Q001", "1"], &["Q002", "0"]],
    );
    let missing = vec![(SlkId::UnitWeapons, weapons.clone())];
    assert!(
        editor(&missing)
            .unwrap_err()
            .to_string()
            .contains("without UnitData or UnitBalance")
    );
    for canonical in [
        table(&["unitID"], &[&["Q001"], &["q001"]]),
        table(&["unitID"], &[&["Q001"], &["bad"]]),
        table(&["wrong schema"], &[&["Q001"], &["Q002"]]),
        table(&["unitID"], &[]),
    ] {
        let mut input = inputs(weapons.clone());
        input[0].1 = canonical;
        let error = editor(&input).unwrap_err().to_string();
        assert!(error.contains("synthetic row join"));
        assert!(error.contains(SlkId::UnitData.path()), "{error}");
    }
}

#[test]
fn standard_header_needs_no_anchor_and_duplicate_named_keys_still_refuse() {
    let valid = vec![(
        SlkId::UnitWeapons,
        table(&["weapsOn", "unitWeapID"], &[&["1", "Q001"]]),
    )];
    assert_eq!(
        editor(&valid).unwrap().row(SlkId::UnitWeapons, "Q001"),
        Some(2)
    );
    assert_eq!(
        defaults(&valid)
            .unwrap()
            .object(Family::Unit, "Q001")
            .unwrap()
            .cell(SlkId::UnitWeapons, "weapsOn")
            .unwrap()
            .text,
        "1"
    );
    let duplicated = vec![(
        SlkId::UnitWeapons,
        table(
            &["unitWeapID", "weapsOn"],
            &[&["Q001", "1"], &["q001", "0"]],
        ),
    )];
    assert!(
        editor(&duplicated)
            .unwrap_err()
            .to_string()
            .contains("duplicate identity")
    );
    let duplicate_header = vec![(
        SlkId::UnitWeapons,
        table(&["unitWeapID", "unitWeapID"], &[&["Q001", "Q001"]]),
    )];
    assert!(
        editor(&duplicate_header)
            .unwrap_err()
            .to_string()
            .contains("duplicate identity header")
    );
}

#[test]
#[ignore = "requires PEACECRAFT3_LEGACY_DIR supplying original archives; proves source joins, not gameplay"]
fn local_original_weapon_fields_reach_editor_and_default_tables() {
    let install =
        std::env::var_os("PEACECRAFT3_LEGACY_DIR").expect("set PEACECRAFT3_LEGACY_DIR explicitly");
    let editor = Editor::open(Path::new(&install)).unwrap();
    let defaults = DefaultTables::open(
        Path::new(&install),
        "caller-selected original install; edition/version unverified",
    )
    .unwrap();
    let FieldSource::Column { slk, column } =
        &editor.field(*b"uaen").expect("authored metadata").source
    else {
        panic!("expected authored SLK field");
    };
    assert_eq!(*slk, SlkId::UnitWeapons);
    let table = editor.table(*slk).unwrap();
    for code in ["Hart", "Hpal", "hfoo"] {
        let row = editor
            .row(*slk, code)
            .expect("authored weapon row joins canonical unit");
        let source = table
            .get(row, table.column_of(column).unwrap())
            .expect("authored weapon-enable cell");
        assert_eq!(editor.unit(code).unwrap().base_cell(*b"uaen"), Some(source));
        assert_eq!(
            defaults
                .object(Family::Unit, code)
                .unwrap()
                .cell(*slk, column)
                .unwrap()
                .text,
            source
        );
    }
    for code in editor.rawcodes(SlkId::UnitData) {
        assert!(
            editor.row(SlkId::UnitWeapons, &code).is_some(),
            "weapon join missing {code}"
        );
    }
    println!(
        "source={} weapon headers={:?}",
        Path::new(&install).display(),
        table.headers().collect::<Vec<_>>()
    );
}
