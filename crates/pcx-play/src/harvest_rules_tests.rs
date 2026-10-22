use super::*;
use pcx_objects::editor::SlkId;
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::{Mod, Value as ObjectValue, ValueType};

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

const ABILITY_HEADERS: &[&str] = &[
    "alias",
    "code",
    "levels",
    "reqLevel",
    "levelSkip",
    "DataA1",
    "DataB1",
    "DataC1",
    "Rng1",
    "Area1",
    "Dur1",
];
const UNIT_HEADERS: &[&str] = &["unitAbilID", "comment(s)", "abilList", "heroAbilList"];
const DEST_HEADERS: &[&str] = &["DestructableID", "comment", "targType", "HP"];
fn weapon_headers() -> Vec<&'static str> {
    vec![
        "unitWeapID",
        "weapsOn",
        "targs1",
        "cool1",
        "dmgpt1",
        "backSw1",
        "rangeN1",
        "RngBuff1",
        "targs2",
        "cool2",
        "dmgpt2",
        "backSw2",
        "rangeN2",
        "RngBuff2",
    ]
}

fn tables_with(
    ability_rows: &[Vec<&str>],
    unit_rows: &[Vec<&str>],
    dest_rows: &[Vec<&str>],
    weapon_rows: &[Vec<&str>],
) -> DefaultTables {
    tables_with_upgrades(ability_rows, unit_rows, dest_rows, weapon_rows, &[])
}

fn tables_with_upgrades(
    ability_rows: &[Vec<&str>],
    unit_rows: &[Vec<&str>],
    dest_rows: &[Vec<&str>],
    weapon_rows: &[Vec<&str>],
    upgrade_rows: &[Vec<&str>],
) -> DefaultTables {
    let mut members: Vec<(String, Vec<u8>)> = Vec::new();
    members.push((
        SlkId::AbilityData.path().to_string(),
        sylk(ABILITY_HEADERS, ability_rows).into_bytes(),
    ));
    if !ability_rows.is_empty() {
        members.push((
            SlkId::AbilityMetaData.path().to_string(),
            sylk(
                &[
                    "ID",
                    "field",
                    "slk",
                    "index",
                    "repeat",
                    "data",
                    "appendIndex",
                    "useSpecific",
                    "notSpecific",
                    "type",
                    "category",
                ],
                &[
                    vec![
                        "Har1",
                        "Data",
                        "AbilityData",
                        "-1",
                        "4",
                        "1",
                        "0",
                        "Ahar",
                        "-",
                        "int",
                        "stats",
                    ],
                    vec![
                        "Har2",
                        "Data",
                        "AbilityData",
                        "-1",
                        "4",
                        "2",
                        "0",
                        "Ahar",
                        "-",
                        "int",
                        "stats",
                    ],
                    vec![
                        "Har3",
                        "Data",
                        "AbilityData",
                        "-1",
                        "4",
                        "3",
                        "0",
                        "Ahar",
                        "-",
                        "int",
                        "stats",
                    ],
                    vec![
                        "adur",
                        "Dur",
                        "AbilityData",
                        "-1",
                        "4",
                        "0",
                        "0",
                        "Ahar",
                        "-",
                        "unreal",
                        "stats",
                    ],
                    vec![
                        "aran",
                        "Rng",
                        "AbilityData",
                        "-1",
                        "4",
                        "0",
                        "0",
                        "Ahar",
                        "-",
                        "unreal",
                        "stats",
                    ],
                    vec![
                        "aare",
                        "Area",
                        "AbilityData",
                        "-1",
                        "4",
                        "0",
                        "0",
                        "Ahar",
                        "-",
                        "unreal",
                        "stats",
                    ],
                    vec![
                        "Arg1",
                        "Data",
                        "AbilityData",
                        "-1",
                        "4",
                        "1",
                        "0",
                        "Argl",
                        "-",
                        "bool",
                        "stats",
                    ],
                    vec![
                        "Arg2",
                        "Data",
                        "AbilityData",
                        "-1",
                        "4",
                        "2",
                        "0",
                        "Argl",
                        "-",
                        "bool",
                        "stats",
                    ],
                ],
            )
            .into_bytes(),
        ));
    }
    if !unit_rows.is_empty() {
        members.push((
            SlkId::UnitAbilities.path().to_string(),
            sylk(UNIT_HEADERS, unit_rows).into_bytes(),
        ));
    }
    if !dest_rows.is_empty() {
        members.push((
            SlkId::DestructableData.path().to_string(),
            sylk(DEST_HEADERS, dest_rows).into_bytes(),
        ));
    }
    if !weapon_rows.is_empty() {
        members.push((
            SlkId::UnitWeapons.path().to_string(),
            sylk(&weapon_headers(), weapon_rows).into_bytes(),
        ));
    }
    if !upgrade_rows.is_empty() {
        members.push((
            SlkId::UpgradeData.path().to_string(),
            sylk(
                &["upgradeid", "maxlevel", "effect1", "base1", "mod1"],
                upgrade_rows,
            )
            .into_bytes(),
        ));
        members.push((
            SlkId::UpgradeMetaData.path().to_string(),
            sylk(
                &["ID", "field", "slk", "index", "type", "category"],
                &[
                    vec!["Umlv", "maxlevel", "UpgradeData", "-1", "int", "tech"],
                    vec!["Uef1", "effect1", "UpgradeData", "-1", "token", "tech"],
                    vec!["Uba1", "base1", "UpgradeData", "-1", "int", "tech"],
                    vec!["Umd1", "mod1", "UpgradeData", "-1", "int", "tech"],
                ],
            )
            .into_bytes(),
        ));
    }
    let owned: Vec<(&str, &[u8])> = members
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    DefaultTables::from_bytes(
        Identity {
            install: "synthetic-install".into(),
            edition_version: "synthetic-edition".into(),
        },
        &owned,
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 8,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .unwrap()
}

fn ahar_ability(
    data_a: &'static str,
    data_b: &'static str,
    data_c: &'static str,
    alias: &'static str,
    code: &'static str,
) -> Vec<&'static str> {
    vec![
        alias, code, "1", "0", "0", data_a, data_b, data_c, "200", "1000", "3.5",
    ]
}

fn stock_tables() -> DefaultTables {
    let ability_rows = vec![
        ahar_ability("7", "25", "30", "Ahar", "Ahar"),
        vec!["Argl", "Artn", "1", "0", "0", "1", "1", "-", "-", "-", "-"],
        vec!["Arlm", "Artn", "1", "0", "0", "0", "1", "-", "-", "-", "-"],
    ];
    let unit_rows = vec![
        vec!["hpea", "Peasant", "Ahar", "-"],
        vec!["htow", "Town Hall", "Argl", "-"],
        vec!["hlum", "Lumber Mill", "Arlm", "-"],
        vec!["hnon", "NoHarvest", "-", "-"],
    ];
    let dest_rows = vec![vec!["LTlt", "Tree Wall", "tree", "120"]];
    let weapon_rows = vec![vec![
        "hpea", "2", "ground", "2", "0.433", "0.567", "90", "250", "tree", "1.7", "0.3", "0.6",
        "70", "140",
    ]];
    tables_with(&ability_rows, &unit_rows, &dest_rows, &weapon_rows)
}

fn upgrade_tables() -> DefaultTables {
    let ability_rows = vec![ahar_ability("7", "25", "30", "Ahar", "Ahar")];
    let unit_rows = vec![vec!["hpea", "Peasant", "Ahar", "-"]];
    let weapon_rows = vec![vec![
        "hpea", "2", "ground", "2", "0.433", "0.567", "90", "250", "tree", "1.7", "0.3", "0.6",
        "70", "140",
    ]];
    tables_with_upgrades(
        &ability_rows,
        &unit_rows,
        &[],
        &weapon_rows,
        &[vec!["Rhlh", "2", "rlum", "10", "10"]],
    )
}

#[test]
fn harvest_rules_read_source_cells() {
    let tables = stock_tables();
    let rules = HarvestRules::from_tables(&tables, *b"hpea")
        .unwrap()
        .expect("Ahar");
    assert_eq!(rules.ahar_alias, "Ahar");
    assert_eq!(rules.damage, 7.0);
    assert_eq!(rules.lumber_capacity, 25);
    assert_eq!(rules.gold_capacity, 30);
    assert_eq!(rules.search_radius, 1000.0);
    assert_eq!(rules.range, 200.0);
    assert_eq!(rules.work_duration, 3.5);
    assert!(rules.source.contains("synthetic-edition"));
}

#[test]
fn unit_without_ahar_is_absent_not_numbers() {
    let tables = stock_tables();
    assert!(
        HarvestRules::from_tables(&tables, *b"hnon")
            .unwrap()
            .is_none()
    );
}

#[test]
fn custom_ahar_alias_reads_from_the_resolved_alias() {
    let ability_rows = vec![ahar_ability("3", "11", "12", "Harv", "Ahar")];
    let unit_rows = vec![vec!["hpea", "Peasant", "Harv", "-"]];
    let tables = tables_with(&ability_rows, &unit_rows, &[], &[]);
    let rules = HarvestRules::from_tables(&tables, *b"hpea")
        .unwrap()
        .expect("Ahar");
    assert_eq!(rules.ahar_alias, "Harv");
    assert_eq!(rules.damage, 3.0);
    assert_eq!(rules.lumber_capacity, 11);
    assert_eq!(rules.gold_capacity, 12);
}

#[test]
fn harvest_damage_rejects_fractional_and_out_of_range_integer_cells() {
    for damage in ["0.5", "2147483648", "-2147483649"] {
        let tables = tables_with(
            &[ahar_ability(damage, "11", "12", "Harv", "Ahar")],
            &[vec!["hpea", "Peasant", "Harv", "-"]],
            &[],
            &[],
        );
        let error = HarvestRules::from_tables(&tables, *b"hpea")
            .expect_err("DataA1 is an integer source field");
        assert!(
            matches!(&error, HarvestError::Malformed { rawcode, column, .. }
                if rawcode == "Harv" && column == "DataA1"),
            "{damage}: {error}"
        );
    }
}

#[test]
fn case_different_code_is_not_ahar() {
    // alias "Ahar" (uppercase AHAR) resolves to code "AHAR" != "Ahar".
    let ability_rows = vec![ahar_ability("1", "2", "3", "AHAR", "AHAR")];
    let unit_rows = vec![vec!["hpea", "Peasant", "AHAR", "-"]];
    let tables = tables_with(&ability_rows, &unit_rows, &[], &[]);
    assert!(
        HarvestRules::from_tables(&tables, *b"hpea")
            .unwrap()
            .is_none()
    );
}

#[test]
fn non_four_byte_alias_is_refused() {
    let ability_rows = vec![ahar_ability("1", "2", "3", "AharX", "Ahar")];
    let unit_rows = vec![vec!["hpea", "Peasant", "AharX", "-"]];
    let tables = tables_with(&ability_rows, &unit_rows, &[], &[]);
    let err = HarvestRules::from_tables(&tables, *b"hpea").unwrap_err();
    assert!(matches!(err, HarvestError::Malformed { .. }), "{err:?}");
}

#[test]
fn negative_capacity_is_refused() {
    let ability_rows = vec![ahar_ability("7", "-1", "30", "Ahar", "Ahar")];
    let unit_rows = vec![vec!["hpea", "Peasant", "Ahar", "-"]];
    let tables = tables_with(&ability_rows, &unit_rows, &[], &[]);
    let err = HarvestRules::from_tables(&tables, *b"hpea").unwrap_err();
    match err {
        HarvestError::Malformed { column, .. } => assert_eq!(column, "DataB1"),
        other => panic!("expected malformed, got {other:?}"),
    }
}

#[test]
fn ambiguous_harvest_aliases_are_refused() {
    let ability_rows = vec![
        ahar_ability("1", "2", "3", "Haab", "Ahar"),
        ahar_ability("4", "5", "6", "Habc", "Ahar"),
    ];
    let unit_rows = vec![vec!["hpea", "Peasant", "Haab,Habc", "-"]];
    let tables = tables_with(&ability_rows, &unit_rows, &[], &[]);
    let err = HarvestRules::from_tables(&tables, *b"hpea").unwrap_err();
    assert!(matches!(err, HarvestError::Ambiguous { .. }), "{err:?}");
}

#[test]
fn tree_class_reads_source_classification_and_csv_list() {
    let tables = stock_tables();
    let tree = TreeClass::from_tables(&tables, *b"LTlt").unwrap();
    assert!(tree.is_tree);
    assert_eq!(tree.hp, 120.0);
    // multitarget list must still find the tree token.
    let dest_rows = vec![vec!["LTlt", "Tree Wall", "tree,debris", "50"]];
    let tables = tables_with(&[], &[], &dest_rows, &[]);
    let tree = TreeClass::from_tables(&tables, *b"LTlt").unwrap();
    assert!(tree.is_tree);
    assert_eq!(tree.hp, 50.0);
}

#[test]
fn depot_return_uses_exact_artn_bools() {
    let tables = stock_tables();
    let hall = DepotReturn::from_tables(&tables, *b"htow").unwrap();
    assert!(hall.accepts_gold && hall.accepts_lumber);
    let mill = DepotReturn::from_tables(&tables, *b"hlum").unwrap();
    assert!(!mill.accepts_gold && mill.accepts_lumber);
}

#[test]
fn depot_return_refuses_bool_2() {
    let ability_rows = vec![vec![
        "Argl", "Artn", "1", "0", "0", "2", "1", "-", "-", "-", "-",
    ]];
    let unit_rows = vec![vec!["htow", "Town Hall", "Argl", "-"]];
    let tables = tables_with(&ability_rows, &unit_rows, &[], &[]);
    let err = DepotReturn::from_tables(&tables, *b"htow").unwrap_err();
    assert!(matches!(err, HarvestError::Malformed { .. }), "{err:?}");
}

#[test]
fn work_weapon_selects_the_tree_slot_and_reads_source() {
    let tables = stock_tables();
    let rules = HarvestRules::from_tables(&tables, *b"hpea")
        .unwrap()
        .expect("Ahar");
    let weapon = rules.work_weapon.expect("tree weapon");
    assert_eq!(weapon.slot, 2);
    assert_eq!(weapon.cool, 1.7);
    assert_eq!(weapon.damage_point, 0.3);
    assert_eq!(weapon.backswing, 0.6);
    assert_eq!(weapon.range, 70.0);
}

#[test]
fn invalid_weapon_masks_are_refused() {
    let ability_rows = vec![ahar_ability("7", "25", "30", "Ahar", "Ahar")];
    let unit_rows = vec![vec!["hpea", "Peasant", "Ahar", "-"]];
    for mask in ["-1", "4", "0x40000000"] {
        let weapon_rows = vec![vec![
            "hpea", mask, "tree", "2", "0.433", "0.567", "90", "250", "tree", "1.7", "0.3", "0.6",
            "70", "140",
        ]];
        let tables = tables_with(&ability_rows, &unit_rows, &[], &weapon_rows);
        let err = HarvestRules::from_tables(&tables, *b"hpea").unwrap_err();
        assert!(
            matches!(err, HarvestError::Malformed { .. }),
            "mask {mask}: {err:?}"
        );
    }
}

#[test]
fn malformed_weapon_cool_is_refused_with_context() {
    let ability_rows = vec![ahar_ability("7", "25", "30", "Ahar", "Ahar")];
    let unit_rows = vec![vec!["hpea", "Peasant", "Ahar", "-"]];
    let weapon_rows = vec![vec![
        "hpea", "2", "ground", "2", "0.433", "0.567", "90", "250", "tree", "x", "0.3", "0.6", "70",
        "140",
    ]];
    let tables = tables_with(&ability_rows, &unit_rows, &[], &weapon_rows);
    let err = HarvestRules::from_tables(&tables, *b"hpea").unwrap_err();
    match err {
        HarvestError::Malformed {
            rawcode, column, ..
        } => {
            assert_eq!(rawcode, "hpea");
            assert_eq!(column, "cool2");
        }
        other => panic!("expected malformed, got {other:?}"),
    }
}

#[test]
fn ambiguous_tree_weapon_slots_are_refused() {
    let ability_rows = vec![ahar_ability("7", "25", "30", "Ahar", "Ahar")];
    let unit_rows = vec![vec!["hpea", "Peasant", "Ahar", "-"]];
    let weapon_rows = vec![vec![
        "hpea", "3", "tree", "2", "0.433", "0.567", "90", "250", "tree", "1.7", "0.3", "0.6", "70",
        "140",
    ]];
    let tables = tables_with(&ability_rows, &unit_rows, &[], &weapon_rows);
    let err = HarvestRules::from_tables(&tables, *b"hpea").unwrap_err();
    assert!(matches!(err, HarvestError::Ambiguous { .. }), "{err:?}");
}

fn selected_field(
    fourcc: [u8; 4],
    slk: SlkId,
    column: &str,
    text: &str,
) -> pcx_sim::world::ObjectField {
    pcx_sim::world::ObjectField {
        metadata: Some(pcx_objects::editor::FieldDef {
            fourcc,
            from: slk,
            field: column.into(),
            slk: format!("{slk:?}"),
            index: -1,
            declared: "token".into(),
            kind: pcx_objects::editor::FieldKind::Token,
            category: String::new(),
            source: pcx_objects::editor::FieldSource::Column {
                slk,
                column: column.into(),
            },
            measured: true,
        }),
        value: Ok(pcx_sim::world::FieldValue::Cell(text.into())),
    }
}

fn selected_unit(rawcode: [u8; 4], fields: &[([u8; 4], SlkId, &str, &str)]) -> Definition {
    let mut map = std::collections::BTreeMap::new();
    for (fourcc, slk, column, text) in fields {
        map.insert(*fourcc, selected_field(*fourcc, *slk, column, text));
    }
    Definition {
        rawcode,
        base_rawcode: String::from_utf8_lossy(&rawcode).into_owned(),
        kind: pcx_objects::editor::ObjectKind::Unit,
        fields: map,
        modifications: Vec::new(),
    }
}

#[test]
fn definition_rules_use_the_selected_unit_fields() {
    let tables = stock_tables();
    let definition = selected_unit(
        *b"hpea",
        &[
            (*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar"),
            (*b"ua1w", SlkId::UnitWeapons, "weapsOn", "2"),
            (*b"ua1t", SlkId::UnitWeapons, "targs1", "ground"),
            (*b"ua1c", SlkId::UnitWeapons, "cool1", "2"),
            (*b"ua1d", SlkId::UnitWeapons, "dmgpt1", "0.433"),
            (*b"ua1b", SlkId::UnitWeapons, "backSw1", "0.567"),
            (*b"ua1r", SlkId::UnitWeapons, "rangeN1", "90"),
            (*b"ua2t", SlkId::UnitWeapons, "targs2", "tree"),
            (*b"ua2c", SlkId::UnitWeapons, "cool2", "1.7"),
            (*b"ua2d", SlkId::UnitWeapons, "dmgpt2", "0.3"),
            (*b"ua2b", SlkId::UnitWeapons, "backSw2", "0.6"),
            (*b"ua2r", SlkId::UnitWeapons, "rangeN2", "70"),
        ],
    );
    let rules = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .unwrap()
        .expect("Ahar");
    assert_eq!(rules.damage, 7.0);
    assert_eq!(rules.lumber_capacity, 25);
    let weapon = rules.work_weapon.expect("tree weapon");
    assert_eq!(weapon.slot, 2);
    assert_eq!(weapon.range, 70.0);
    assert_eq!(weapon.cool, 1.7);
}

#[test]
fn definition_without_ability_list_is_a_contextual_refusal() {
    let tables = stock_tables();
    let definition = selected_unit(*b"hpea", &[]);
    let error = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .expect_err("missing abilList");
    match error {
        HarvestError::Missing { column, .. } => assert_eq!(column, "abilList"),
        other => panic!("expected missing, got {other:?}"),
    }
}

#[test]
fn definition_without_ahar_is_absent() {
    let tables = stock_tables();
    let definition = selected_unit(
        *b"hnon",
        &[(*b"uabi", SlkId::UnitAbilities, "abilList", "-")],
    );
    assert!(
        HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
            .unwrap()
            .is_none()
    );
}

fn edit(
    rawcode: &str,
    field: [u8; 4],
    value_type: pcx_objects::ValueType,
    value: pcx_objects::Value,
) -> Overlay {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        rawcode.into(),
        vec![pcx_objects::Mod {
            field,
            value_type,
            selectors: Some([1, 0]),
            value,
            end_word: 0,
        }],
    );
    overlay
}

fn definition_rules(overlay: &Overlay) -> HarvestRules {
    let tables = stock_tables();
    let definition = selected_unit(
        *b"hpea",
        &[(*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar")],
    );
    HarvestRules::from_definition(&tables, overlay, &definition)
        .unwrap()
        .expect("Ahar")
}

#[test]
fn metadata_data_ordinal_selects_and_accepts_a_typed_override() {
    let rules = definition_rules(&edit(
        "Ahar",
        *b"Har2",
        pcx_objects::ValueType::Int,
        pcx_objects::Value::Int(17),
    ));
    assert_eq!(rules.damage, 7.0);
    assert_eq!(rules.lumber_capacity, 17);
    assert_eq!(rules.gold_capacity, 30);
}

#[test]
fn metadata_data_ordinal_damage_override_and_unrelated_edits_survive() {
    let mut overlay = edit(
        "Ahar",
        *b"Har1",
        pcx_objects::ValueType::Int,
        pcx_objects::Value::Int(11),
    );
    overlay
        .mods
        .entry("Ahar".into())
        .or_default()
        .push(pcx_objects::Mod {
            field: *b"acdn",
            value_type: pcx_objects::ValueType::Real,
            selectors: Some([1, 0]),
            value: pcx_objects::Value::Real(9.5),
            end_word: 0,
        });
    let rules = definition_rules(&overlay);
    assert_eq!(rules.damage, 11.0);
    assert_eq!(rules.lumber_capacity, 25);
}

#[test]
fn metadata_scalar_override_uses_the_named_field_not_a_data_column() {
    let rules = definition_rules(&edit(
        "Ahar",
        *b"adur",
        pcx_objects::ValueType::Unreal,
        pcx_objects::Value::Unreal(4.25),
    ));
    assert_eq!(rules.work_duration, 4.25);
}

#[test]
fn depot_return_metadata_override_changes_both_authored_flags() {
    let tables = stock_tables();
    let definition = selected_unit(
        *b"htow",
        &[(*b"uabi", SlkId::UnitAbilities, "abilList", "Argl")],
    );
    let mut overlay = edit(
        "Argl",
        *b"Arg1",
        pcx_objects::ValueType::Int,
        pcx_objects::Value::Int(0),
    );
    overlay
        .mods
        .entry("Argl".into())
        .or_default()
        .push(pcx_objects::Mod {
            field: *b"Arg2",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([1, 0]),
            value: pcx_objects::Value::Int(0),
            end_word: 0,
        });
    let returned = DepotReturn::from_definition(&tables, &overlay, &definition).unwrap();
    assert!(!returned.accepts_gold);
    assert!(!returned.accepts_lumber);
}
#[test]
fn metadata_override_with_the_wrong_type_is_a_contextual_refusal() {
    let error = HarvestRules::from_definition(
        &stock_tables(),
        &edit(
            "Ahar",
            *b"Har2",
            pcx_objects::ValueType::Real,
            pcx_objects::Value::Real(17.5),
        ),
        &selected_unit(
            *b"hpea",
            &[(*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar")],
        ),
    )
    .unwrap_err();
    match error {
        HarvestError::Malformed { column, detail, .. } => {
            assert_eq!(column, "Data2");
            assert!(detail.contains("type"), "{detail}");
        }
        other => panic!("expected malformed, got {other:?}"),
    }
}

fn definition_weapon_fields() -> [([u8; 4], SlkId, &'static str, &'static str); 11] {
    [
        (*b"ua1w", SlkId::UnitWeapons, "weapsOn", "2"),
        (*b"ua1t", SlkId::UnitWeapons, "targs1", "ground"),
        (*b"ua1c", SlkId::UnitWeapons, "cool1", "2"),
        (*b"ua1d", SlkId::UnitWeapons, "dmgpt1", "0.433"),
        (*b"ua1b", SlkId::UnitWeapons, "backSw1", "0.567"),
        (*b"ua1r", SlkId::UnitWeapons, "rangeN1", "90"),
        (*b"ua2t", SlkId::UnitWeapons, "targs2", "tree"),
        (*b"ua2c", SlkId::UnitWeapons, "cool2", "1.25"),
        (*b"ua2d", SlkId::UnitWeapons, "dmgpt2", "0.3"),
        (*b"ua2b", SlkId::UnitWeapons, "backSw2", "0.6"),
        (*b"ua2r", SlkId::UnitWeapons, "rangeN2", "73"),
    ]
}

#[test]
fn definition_reads_nonstock_ahar_damage_and_capacity() {
    let ability_rows = vec![ahar_ability("13", "41", "7", "Ahar", "Ahar")];
    let tables = tables_with(&ability_rows, &[], &[], &[]);
    let mut fields = definition_weapon_fields().to_vec();
    fields.push((*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar"));
    let definition = selected_unit(*b"hpea", &fields);
    let rules = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .unwrap()
        .expect("nonstock Ahar");
    assert_eq!(rules.ahar_alias, "Ahar");
    assert_eq!(rules.damage, 13.0);
    assert_eq!(rules.lumber_capacity, 41);
    assert_eq!(rules.gold_capacity, 7);
    let weapon = rules.work_weapon.expect("tree weapon");
    assert_eq!(weapon.cool, 1.25);
    assert_eq!(weapon.range, 73.0);
}

#[test]
fn definition_with_several_ahar_upgrade_aliases_is_refused() {
    let ability_rows = vec![
        ahar_ability("7", "25", "30", "Ahar", "Ahar"),
        ahar_ability("13", "41", "7", "Ahrl", "Ahar"),
    ];
    let tables = tables_with(&ability_rows, &[], &[], &[]);
    let definition = selected_unit(
        *b"hpea",
        &[(*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar,Ahrl")],
    );
    let error = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .expect_err("two Ahar levels are ambiguous");
    assert!(matches!(error, HarvestError::Ambiguous { .. }), "{error:?}");
}

#[test]
fn definition_reads_nonstock_depot_return_flags() {
    let ability_rows = vec![vec![
        "Argl", "Artn", "1", "0", "0", "0", "1", "-", "-", "-", "-",
    ]];
    let tables = tables_with(&ability_rows, &[], &[], &[]);
    let definition = selected_unit(
        *b"hdep",
        &[(*b"uabi", SlkId::UnitAbilities, "abilList", "Argl")],
    );
    let returned = DepotReturn::from_definition(&tables, &Overlay::default(), &definition).unwrap();
    assert!(!returned.accepts_gold);
    assert!(returned.accepts_lumber);
}

#[test]
fn definition_with_authored_upgrades_keeps_reading_rules() {
    let tables = stock_tables();
    let definition = selected_unit(
        *b"hpea",
        &[
            (*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar"),
            (*b"upgr", SlkId::UnitBalance, "upgradeList", "Rhlh,Rguv"),
        ],
    );
    let mut rules = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .unwrap()
        .expect("Ahar");
    rules
        .apply_lumber_harvesting_upgrades(&tables, &Overlay::default(), &definition, |_| Ok(0))
        .unwrap();
    assert_eq!(rules.damage, 7.0);
    assert_eq!(rules.lumber_capacity, 25);
}

#[test]
fn active_lumber_harvesting_upgrade_adds_authored_capacity() {
    let tables = upgrade_tables();
    let definition = selected_unit(
        *b"hpea",
        &[
            (*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar"),
            (*b"upgr", SlkId::UnitBalance, "upgradeList", "Rhlh"),
        ],
    );
    let mut rules = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .unwrap()
        .expect("Ahar");
    let requested = std::cell::Cell::new(0);
    rules
        .apply_lumber_harvesting_upgrades(&tables, &Overlay::default(), &definition, |tech| {
            requested.set(tech);
            Ok(2)
        })
        .unwrap();
    assert_eq!(requested.get(), i32::from_be_bytes(*b"Rhlh"));
    assert_eq!(rules.lumber_capacity, 45);
}

#[test]
fn w3q_custom_upgrade_overrides_the_base_capacity_and_maximum() {
    let tables = upgrade_tables();
    let mut overlay = Overlay::default();
    overlay.custom.insert(
        "R999".into(),
        (
            "Rhlh".into(),
            vec![
                Mod {
                    field: *b"Umlv",
                    value_type: ValueType::Int,
                    selectors: Some([0, 0]),
                    value: ObjectValue::Int(3),
                    end_word: 0,
                },
                Mod {
                    field: *b"Umd1",
                    value_type: ValueType::Int,
                    selectors: Some([1, 0]),
                    value: ObjectValue::Int(15),
                    end_word: 0,
                },
            ],
        ),
    );
    let definition = selected_unit(
        *b"hpea",
        &[
            (*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar"),
            (*b"upgr", SlkId::UnitBalance, "upgradeList", "R999"),
        ],
    );
    let mut rules = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .unwrap()
        .expect("Ahar");
    rules
        .apply_lumber_harvesting_upgrades(&tables, &overlay, &definition, |_| Ok(3))
        .unwrap();
    assert_eq!(rules.lumber_capacity, 65);
}

#[test]
fn active_upgrade_level_beyond_the_authored_maximum_is_refused() {
    let tables = upgrade_tables();
    let definition = selected_unit(
        *b"hpea",
        &[
            (*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar"),
            (*b"upgr", SlkId::UnitBalance, "upgradeList", "Rhlh"),
        ],
    );
    let mut rules = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .unwrap()
        .expect("Ahar");
    let error = rules
        .apply_lumber_harvesting_upgrades(&tables, &Overlay::default(), &definition, |_| Ok(3))
        .unwrap_err();
    match error {
        HarvestError::Malformed { column, detail, .. } => {
            assert_eq!(column, "maxlevel");
            assert!(detail.contains("exceeds"), "{detail}");
        }
        other => panic!("expected malformed, got {other:?}"),
    }
}

#[test]
fn definition_with_an_empty_upgrade_list_keeps_reading_rules() {
    let tables = stock_tables();
    let definition = selected_unit(
        *b"hpea",
        &[
            (*b"uabi", SlkId::UnitAbilities, "abilList", "Ahar"),
            (*b"upgr", SlkId::UnitBalance, "upgradeList", "-"),
        ],
    );
    let mut rules = HarvestRules::from_definition(&tables, &Overlay::default(), &definition)
        .unwrap()
        .expect("Ahar");
    rules
        .apply_lumber_harvesting_upgrades(&tables, &Overlay::default(), &definition, |_| {
            panic!("empty upgrade list must not query player levels")
        })
        .unwrap();
    assert_eq!(rules.damage, 7.0);
}
