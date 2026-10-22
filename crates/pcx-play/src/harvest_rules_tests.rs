use super::*;
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::SlkId;

fn sylk(headers: &[&str], rows: &[Vec<&str>]) -> String {
    let mut s = String::from("ID;PWXL;N;E\r\n");
    for (col, name) in headers.iter().enumerate() {
        s.push_str(&format!("C;X{};Y1;K\"{name}\"\r\n", col + 1));
    }
    for (r, row) in rows.iter().enumerate() {
        for (col, value) in row.iter().enumerate() {
            let value = if value.parse::<f64>().is_ok() {
                (*value).to_owned()
            } else {
                format!("\"{value}\"")
            };
            s.push_str(&format!("C;X{};Y{};K{value}\r\n", col + 1, r + 2));
        }
    }
    s.push_str("E\r\n");
    s
}

fn tables() -> DefaultTables {
    let ability_headers = [
        "alias", "code", "comments", "levels", "reqLevel", "levelSkip",
        "DataA1", "DataB1", "DataC1", "Rng1", "Area1", "Dur1",
    ];
    let ability_rows = [
        vec!["Ahar", "Ahar", "Harvest", "1", "0", "0", "7", "25", "30", "200", "1000", "3.5"],
        vec!["Argl", "Artn", "Return (Gold & Lumber)", "1", "0", "0", "1", "1", "-", "-", "-", "-"],
        vec!["Argd", "Artn", "Return (Gold)", "1", "0", "0", "1", "0", "-", "-", "-", "-"],
        vec!["Arlm", "Artn", "Return (Lumber)", "1", "0", "0", "0", "1", "-", "-", "-", "-"],
    ];
    let unit_headers = ["unitAbilID", "comment(s)", "abilList"];
    let unit_rows = [
        vec!["hpea", "Peasant", "Ahar"],
        vec!["htow", "Town Hall", "Argl"],
        vec!["hlum", "Lumber Mill", "Arlm"],
    ];
    let dest_headers = ["DestructableID", "comment", "targType", "HP"];
    let dest_rows = [vec!["LTlt", "Tree Wall", "tree", "120"]];

    DefaultTables::from_bytes(
        Identity {
            install: "synthetic-install".into(),
            edition_version: "synthetic-edition".into(),
        },
        &[
            (SlkId::AbilityData.path(), sylk(&ability_headers, &ability_rows).as_bytes()),
            (SlkId::UnitAbilities.path(), sylk(&unit_headers, &unit_rows).as_bytes()),
            (SlkId::DestructableData.path(), sylk(&dest_headers, &dest_rows).as_bytes()),
        ],
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 8,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .expect("synthetic harvest tables")
}

#[test]
fn harvest_rules_read_source_cells() {
    let tables = tables();
    let rules = HarvestRules::from_tables(&tables, *b"hpea").unwrap();
    assert!(rules.has_harvest);
    assert_eq!(rules.damage, 7.0);
    assert_eq!(rules.lumber_capacity, 25);
    assert_eq!(rules.gold_capacity, 30);
    assert_eq!(rules.search_radius, 1000.0);
    assert_eq!(rules.range, 200.0);
    assert_eq!(rules.work_duration, 3.5);
    assert!(rules.source.contains("synthetic-edition"));
}

#[test]
fn tree_class_reads_source_classification() {
    let tables = tables();
    let tree = TreeClass::from_tables(&tables, *b"LTlt").unwrap();
    assert!(tree.is_tree);
    assert_eq!(tree.hp, 120.0);
}

#[test]
fn depot_return_uses_artn_gold_lumber_flags() {
    let tables = tables();
    // Town Hall: Argl -> Artn, DataA=1 (gold), DataB=1 (lumber).
    let hall = DepotReturn::from_tables(&tables, *b"htow").unwrap();
    assert!(hall.accepts_gold && hall.accepts_lumber);
    // Lumber Mill: Arlm -> Artn, DataA=0, DataB=1.
    let mill = DepotReturn::from_tables(&tables, *b"hlum").unwrap();
    assert!(!mill.accepts_gold && mill.accepts_lumber);
}

#[test]
fn malformed_ranked_cells_are_refused_with_context() {
    let ability_headers = [
        "alias", "code", "levels", "reqLevel", "levelSkip",
        "DataA1", "DataB1", "DataC1", "Rng1", "Area1", "Dur1",
    ];
    let ability_rows = [
        vec!["Ahar", "Ahar", "1", "0", "0", "banana", "25", "30", "200", "1000", "3.5"],
    ];
    let unit_headers = ["unitAbilID", "comment(s)", "abilList"];
    let unit_rows = [vec!["hpea", "Peasant", "Ahar"]];
    let dest_headers = ["DestructableID", "comment", "targType", "HP"];
    let dest_rows = [vec!["LTlt", "Tree Wall", "tree", "120"]];
    let tables = DefaultTables::from_bytes(
        Identity {
            install: "synthetic-install".into(),
            edition_version: "synthetic-edition".into(),
        },
        &[
            (SlkId::AbilityData.path(), sylk(&ability_headers, &ability_rows).as_bytes()),
            (SlkId::UnitAbilities.path(), sylk(&unit_headers, &unit_rows).as_bytes()),
            (SlkId::DestructableData.path(), sylk(&dest_headers, &dest_rows).as_bytes()),
        ],
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 8,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .unwrap();
    let err = HarvestRules::from_tables(&tables, *b"hpea").unwrap_err();
    match err {
        HarvestError::Malformed { rawcode, column, detail } => {
            assert_eq!(rawcode, "Ahar");
            assert_eq!(column, "DataA1");
            assert!(!detail.is_empty());
        }
        other => panic!("expected malformed, got {other:?}"),
    }
}

#[test]
fn missing_harvest_row_is_refused_with_context() {
    let unit_headers = ["unitAbilID", "comment(s)", "abilList"];
    let unit_rows = [vec!["hpea", "Peasant", "Ahar"]];
    let dest_headers = ["DestructableID", "comment", "targType", "HP"];
    let dest_rows = [vec!["LTlt", "Tree Wall", "tree", "120"]];
    let tables = DefaultTables::from_bytes(
        Identity {
            install: "synthetic-install".into(),
            edition_version: "synthetic-edition".into(),
        },
        &[
            (SlkId::UnitAbilities.path(), sylk(&unit_headers, &unit_rows).as_bytes()),
            (SlkId::DestructableData.path(), sylk(&dest_headers, &dest_rows).as_bytes()),
        ],
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 8,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .unwrap();
    let err = HarvestRules::from_tables(&tables, *b"hpea").unwrap_err();
    assert!(matches!(err, HarvestError::Missing { .. }), "{err:?}");
}
