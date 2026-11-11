//! `UnitId` on synthetic `UnitUI` rows: the names and ids below are fixtures, not the install's.
use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_objects::editor::defaults::Identity;
use pcx_script::{Host, Prelude};

const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nconstant native UnitId takes string unitIdString returns integer\nglobals\nconstant playercolor COLOR = ConvertPlayerColor(37)\nendglobals\n";
const DECLARATION: &str = "constant native UnitId takes string unitIdString returns integer\n";

fn host(common: &str) -> ObjectHost {
    let c = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program =
        Arc::new(pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &c)]).unwrap());
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:unit-id",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 8,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(inner).unwrap()
}

fn sylk(headers: &[&str], rows: &[[&str; 2]]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (x, name) in headers.iter().enumerate() {
        text.push_str(&format!("C;X{};Y1;K\"{name}\"\r\n", x + 1));
    }
    for (y, row) in rows.iter().enumerate() {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{value}\"\r\n", x + 1, y + 2));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

/// `UnitUI` rows under `headers`, and `balance_only` units that exist in `UnitBalance` alone.
fn tables_with(
    headers: &[&str],
    rows: &[[&str; 2]],
    balance_only: &[[&str; 2]],
) -> Arc<DefaultTables> {
    let ui = sylk(headers, rows);
    let balance = sylk(&["unitBalanceID", "level"], balance_only);
    Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic".into(),
                edition_version: "fixture".into(),
            },
            &[
                (SlkId::UnitUi.path(), ui.as_slice()),
                (SlkId::UnitBalance.path(), balance.as_slice()),
            ],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 2,
                max_asset_bytes: 64 << 10,
                max_aggregate_input_bytes: 128 << 10,
            },
        )
        .unwrap(),
    )
}

fn tables(rows: &[[&str; 2]]) -> Arc<DefaultTables> {
    tables_with(&["unitUIID", "name"], rows, &[])
}

fn id(h: &mut ObjectHost, value: Value) -> Result<Value, String> {
    h.native("UnitId", &[value])
}

#[test]
fn a_name_answers_its_rawcode_case_insensitively_and_a_miss_or_null_answers_zero() {
    let mut h = host(COMMON)
        .with_unit_id_lookup(tables(&[["zq01", "Quillbeast"], ["zq02", "gloomhound"]]))
        .unwrap();
    for text in ["quillbeast", "QUILLBEAST", "QuillBeast"] {
        assert_eq!(
            id(&mut h, Value::Str(text.into())).unwrap(),
            Value::Int(i32::from_be_bytes(*b"zq01")),
            "{text}"
        );
    }
    assert_eq!(
        id(&mut h, Value::Str("gloomhound".into())).unwrap(),
        Value::Int(i32::from_be_bytes(*b"zq02"))
    );
    assert_eq!(
        id(&mut h, Value::Str("missing".into())).unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        id(&mut h, Value::Str(String::new())).unwrap(),
        Value::Int(0)
    );
    assert_eq!(id(&mut h, Value::Null).unwrap(), Value::Int(0));
}

#[test]
fn a_name_shared_by_two_rows_refuses_and_other_names_still_answer() {
    let mut h = host(COMMON)
        .with_unit_id_lookup(tables(&[
            ["zq01", "twin"],
            ["zq02", "Twin"],
            ["zq03", "single"],
        ]))
        .unwrap();
    let error = id(&mut h, Value::Str("TWIN".into())).unwrap_err();
    assert!(error.contains("shared by several unit rows"), "{error}");
    assert_eq!(
        id(&mut h, Value::Str("single".into())).unwrap(),
        Value::Int(i32::from_be_bytes(*b"zq03"))
    );
}

#[test]
fn wrong_arguments_refuse_and_the_native_needs_admission_and_a_matching_declaration() {
    let mut h = host(COMMON)
        .with_unit_id_lookup(tables(&[["zq01", "quillbeast"]]))
        .unwrap();
    assert!(h.native("UnitId", &[]).is_err());
    assert!(id(&mut h, Value::Int(1)).is_err());
    assert!(
        host(COMMON)
            .native("UnitId", &[Value::Str("quillbeast".into())])
            .is_err()
    );
    // A program without the declaration keeps the native unsupported and admits nothing.
    let mut undeclared = host(&COMMON.replace(DECLARATION, ""))
        .with_unit_id_lookup(tables(&[["zq01", "quillbeast"]]))
        .unwrap();
    assert!(undeclared.unit_ids.is_none());
    assert!(
        undeclared
            .native("UnitId", &[Value::Str("quillbeast".into())])
            .is_err()
    );
    // Declared in a different shape: a signature refusal at admission.
    let wrong = COMMON.replace(
        DECLARATION,
        "constant native UnitId takes integer unitIdString returns integer\n",
    );
    assert!(
        host(&wrong)
            .with_unit_id_lookup(tables(&[["zq01", "quillbeast"]]))
            .is_err()
    );
}

#[test]
fn a_unit_with_no_unitui_row_registers_no_name_and_does_not_block_the_others() {
    let mut h = host(COMMON)
        .with_unit_id_lookup(tables_with(
            &["unitUIID", "name"],
            &[["zq01", "quillbeast"]],
            &[["zq09", "1"]],
        ))
        .unwrap();
    assert_eq!(
        id(&mut h, Value::Str("quillbeast".into())).unwrap(),
        Value::Int(i32::from_be_bytes(*b"zq01"))
    );
}

#[test]
fn tables_without_any_unitui_name_refuse_at_the_first_lookup() {
    let mut h = host(COMMON)
        .with_unit_id_lookup(tables_with(
            &["unitUIID", "file"],
            &[["zq01", "model"]],
            &[],
        ))
        .unwrap();
    let error = id(&mut h, Value::Str("quillbeast".into())).unwrap_err();
    assert!(
        error.contains("no unit row carries a UnitUI name"),
        "{error}"
    );
}
