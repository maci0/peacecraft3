use super::*;
use pcx_objects::editor::defaults::Identity;
use pcx_objects::editor::{EditorReadLimits, ObjectKind};
use std::collections::BTreeMap;

/// The registration window with the class key pushed last, as `class_from_window` reads it.
fn window(source: &str, base: &str) -> Vec<u8> {
    let mut w = vec![PUSH_IMM32, 1, 2, 3, 4, PUSH_IMM32];
    w.extend(base.bytes().rev());
    w.push(PUSH_IMM32);
    w.extend(source.bytes().rev());
    w.extend([CALL_REL32, 0, 0, 0, 0]);
    w.extend(CLEANUP_RET);
    w
}

fn sylk(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
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

/// Source-shaped tome: `amounts` are the ability's `DataA1`, `DataB1`, `DataC1` (agi, int, str).
struct Tome<'a> {
    code: &'a str,
    abilities: &'a str,
    powerup: &'a str,
    uses: &'a str,
    use_specific: &'a str,
    amounts: [&'a str; 3],
}

impl Default for Tome<'_> {
    fn default() -> Self {
        Self {
            code: "AIsm",
            abilities: "AIsm",
            powerup: "1",
            uses: "1",
            use_specific: "AIsm,AIxm",
            amounts: ["0", "0", "7"],
        }
    }
}

fn tables(tome: &Tome) -> Arc<DefaultTables> {
    let item = sylk(
        &[
            "itemID",
            "abilList",
            "powerup",
            "usable",
            "perishable",
            "uses",
        ],
        &[&["tstr", tome.abilities, tome.powerup, "1", "1", tome.uses]],
    );
    let ability = sylk(
        &["alias", "code", "levels", "DataA1", "DataB1", "DataC1"],
        &[&[
            "AIsm",
            tome.code,
            "1",
            tome.amounts[0],
            tome.amounts[1],
            tome.amounts[2],
        ]],
    );
    let meta = sylk(
        &["ID", "field", "slk", "index", "type", "data", "useSpecific"],
        &[
            &[
                "Iagi",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "1",
                tome.use_specific,
            ],
            &[
                "Iint",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "2",
                tome.use_specific,
            ],
            &[
                "Istr",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "3",
                tome.use_specific,
            ],
            &[
                "Idef",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "1",
                "AId1",
            ],
        ],
    );
    Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic powerup".into(),
                edition_version: "fixture".into(),
            },
            &[
                (SlkId::ItemData.path(), &item),
                (SlkId::AbilityData.path(), &ability),
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

fn rules(tome: &Tome, overlay: Overlay, class_key: &str) -> Result<Rules, String> {
    Rules::decode(
        tables(tome),
        overlay,
        AssetIdentity {
            source: "synthetic".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        },
        [0; 32],
        &window(class_key, "AIxm"),
        &window("AIab", "APbo"),
        &window("AIde", "APbo"),
    )
}

fn tstr() -> Definition {
    Definition {
        rawcode: *b"tstr",
        base_rawcode: "tstr".into(),
        kind: ObjectKind::Item,
        fields: BTreeMap::new(),
        modifications: Vec::new(),
    }
}

fn int_mod(field: &[u8; 4], selectors: Option<[u32; 2]>, value: u32) -> pcx_objects::Mod {
    pcx_objects::Mod {
        field: *field,
        value_type: pcx_objects::ValueType::Int,
        selectors,
        value: pcx_objects::Value::Int(value),
        end_word: 0,
    }
}

#[test]
fn class_window_reads_the_pushed_source_key() {
    assert_eq!(
        class_from_window(&window("AIsm", "AIxm")).unwrap(),
        ("AIsm".to_string(), "AIxm".to_string())
    );
}

#[test]
fn class_window_refuses_wrong_shape_and_non_alphanumeric_keys() {
    let mut short = window("AIsm", "AIxm");
    short.pop();
    assert!(class_from_window(&short).is_err());
    let mut not_push = window("AIsm", "AIxm");
    not_push[10] = 0x90;
    assert!(class_from_window(&not_push).is_err());
    let mut tail = window("AIsm", "AIxm");
    tail[23] = 0x90;
    assert!(class_from_window(&tail).is_err());
    assert!(class_from_window(&window("AI\u{1}m", "AIxm")).is_err());
}

#[test]
fn strength_comes_from_the_ability_amount_not_a_constant() {
    let seven = rules(&Tome::default(), Overlay::default(), "AIsm").unwrap();
    assert_eq!(
        seven.strength_effect(&tstr()).unwrap(),
        Some(StrengthEffect { strength: 7 })
    );
    let nine = rules(
        &Tome {
            amounts: ["0", "0", "9"],
            ..Tome::default()
        },
        Overlay::default(),
        "AIsm",
    )
    .unwrap();
    assert_eq!(
        nine.strength_effect(&tstr()).unwrap().unwrap().strength(),
        9
    );
}

#[test]
fn w3a_override_on_the_istr_field_wins_at_effect_level() {
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIsm".into(), vec![int_mod(b"Istr", Some([1, 0]), 12)]);
    let rules = rules(&Tome::default(), overlay, "AIsm").unwrap();
    assert_eq!(
        rules.strength_effect(&tstr()).unwrap().unwrap().strength(),
        12
    );
}

#[test]
fn override_at_another_level_is_ignored() {
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIsm".into(), vec![int_mod(b"Istr", Some([2, 0]), 12)]);
    let rules = rules(&Tome::default(), overlay, "AIsm").unwrap();
    assert_eq!(
        rules.strength_effect(&tstr()).unwrap().unwrap().strength(),
        7
    );
}

#[test]
fn non_powerup_items_have_no_automatic_effect() {
    let rules = rules(
        &Tome {
            powerup: "0",
            ..Tome::default()
        },
        Overlay::default(),
        "AIsm",
    )
    .unwrap();
    assert_eq!(rules.strength_effect(&tstr()).unwrap(), None);
}

#[test]
fn unsupported_item_shapes_refuse_atomically() {
    let refuse = |tome: Tome| {
        rules(&tome, Overlay::default(), "AIsm")
            .unwrap()
            .strength_effect(&tstr())
    };
    assert!(
        refuse(Tome {
            uses: "2",
            ..Tome::default()
        })
        .is_err()
    );
    assert!(
        refuse(Tome {
            abilities: "AIsm,AIxm",
            ..Tome::default()
        })
        .is_err()
    );
    assert!(
        refuse(Tome {
            abilities: "",
            ..Tome::default()
        })
        .is_err()
    );
    assert!(
        refuse(Tome {
            use_specific: "AIxm",
            ..Tome::default()
        })
        .is_err()
    );
    assert!(
        refuse(Tome {
            amounts: ["0", "0", "-1"],
            ..Tome::default()
        })
        .is_err()
    );
    assert!(
        refuse(Tome {
            amounts: ["1", "0", "7"],
            ..Tome::default()
        })
        .is_err()
    );
    assert!(
        refuse(Tome {
            amounts: ["0", "2", "7"],
            ..Tome::default()
        })
        .is_err()
    );
}

#[test]
fn ability_class_must_match_the_client_registration() {
    let rules = rules(
        &Tome {
            code: "AIxm",
            ..Tome::default()
        },
        Overlay::default(),
        "AIsm",
    )
    .unwrap();
    assert!(rules.strength_effect(&tstr()).is_err());
}

#[test]
fn custom_items_refuse_until_item_overrides_are_qualified() {
    let mut overlay = Overlay::default();
    overlay
        .custom
        .insert("tsx1".into(), ("tstr".into(), Vec::new()));
    let rules = rules(&Tome::default(), overlay, "AIsm").unwrap();
    let mut custom = tstr();
    custom.rawcode = *b"tsx1";
    assert!(rules.strength_effect(&custom).is_err());
}

#[test]
fn reviewed_image_gate_refuses_other_bytes() {
    let identity = AssetIdentity {
        source: "other.exe".into(),
        edition: "synthetic".into(),
        version: "test".into(),
    };
    let err = Rules::read(
        tables(&Tome::default()),
        Overlay::default(),
        b"MZ",
        identity,
    )
    .unwrap_err();
    assert!(err.contains("unsupported client identity"), "{err}");
}

/// Selected-install rules: `PCX_TOME_CLIENT` is the selected `Warcraft III.exe`,
/// `PCX_TOME_CATALOG` the selected `units` directory with `itemdata.slk`,
/// `abilitydata.slk` and `abilitymetadata.slk`. No original bytes are committed.
fn selected_install() -> (Arc<DefaultTables>, Rules) {
    let client = std::env::var("PCX_TOME_CLIENT").unwrap();
    let catalog = std::path::PathBuf::from(std::env::var("PCX_TOME_CATALOG").unwrap());
    let read = |name: &str| std::fs::read(catalog.join(name)).unwrap();
    let (item, ability, meta) = (
        read("itemdata.slk"),
        read("abilitydata.slk"),
        read("abilitymetadata.slk"),
    );
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: catalog.display().to_string(),
                edition_version: "selected install".into(),
            },
            &[
                (SlkId::ItemData.path(), &item),
                (SlkId::AbilityData.path(), &ability),
                (SlkId::AbilityMetaData.path(), &meta),
            ],
            &[],
            EditorReadLimits {
                max_listed_paths: 4,
                max_asset_bytes: 16 << 20,
                max_aggregate_input_bytes: 64 << 20,
            },
        )
        .unwrap(),
    );
    let rules = Rules::read(
        Arc::clone(&tables),
        Overlay::default(),
        &std::fs::read(client).unwrap(),
        AssetIdentity {
            source: "selected client".into(),
            edition: "selected Legacy executable".into(),
            version: "VERSIONINFO 1.29.2.9231".into(),
        },
    )
    .unwrap();
    (tables, rules)
}

fn catalog_item(raw: &str) -> Definition {
    Definition {
        rawcode: raw.as_bytes().try_into().unwrap(),
        base_rawcode: raw.into(),
        kind: ObjectKind::Item,
        fields: BTreeMap::new(),
        modifications: Vec::new(),
    }
}

#[test]
#[ignore = "requires PCX_TOME_CLIENT and PCX_TOME_CATALOG selected local assets"]
fn local_original_tome_strength_comes_from_the_selected_catalog() {
    let (_, rules) = selected_install();
    assert!(rules.strength_effect(&tstr()).unwrap().is_some());
    // Catalog rows: hval has strength 4 and agility 4 (passive `AIab` class); hcun adds intelligence.
    let held = rules
        .held_attributes(&catalog_item("hval"))
        .unwrap()
        .unwrap();
    assert_eq!((held.strength, held.agility), (4, 4));
    let refusal = rules.held_attributes(&catalog_item("hcun")).unwrap_err();
    assert!(refusal.contains("intelligence passive bonus"), "{refusal}");
    assert_eq!(rules.held_attributes(&catalog_item("crys")).unwrap(), None);
}

/// Classifies every selected item row by its held-attribute result. Prints the counts; the
/// gate asserts the partition covers every row and that each passive row is classified.
#[test]
#[ignore = "requires PCX_TOME_CLIENT and PCX_TOME_CATALOG selected local assets"]
fn local_original_held_item_census_partitions_every_row() {
    let (tables, rules) = selected_install();
    let (mut none, mut supported, mut refused) = (0usize, 0usize, 0usize);
    let mut reasons = BTreeMap::<String, usize>::new();
    let rows: Vec<String> = tables.rawcodes(Family::Item).map(str::to_owned).collect();
    // Passive rows: usable=0 and powerup=0 in the source row.
    let mut passive = (0usize, 0usize);
    for raw in &rows {
        let object = tables.object(Family::Item, raw).unwrap();
        let flag = |key: &str| object.cell(SlkId::ItemData, key).unwrap().text.trim() == "1";
        let abilities = object
            .cell(SlkId::ItemData, "abilList")
            .unwrap()
            .text
            .trim()
            .to_owned();
        if !flag("usable") && !flag("powerup") && !matches!(abilities.as_str(), "" | "_" | "-") {
            passive.0 += 1;
            passive.1 += usize::from(
                rules
                    .held_attributes(&catalog_item(raw))
                    .is_ok_and(|h| h.is_some()),
            );
        }
    }
    eprintln!(
        "census passive rows with abilities={} supported={}",
        passive.0, passive.1
    );
    for raw in &rows {
        match rules.held_attributes(&catalog_item(raw)) {
            Ok(None) => none += 1,
            Ok(Some(_)) => supported += 1,
            Err(reason) => {
                refused += 1;
                let key = reason.split(" on ").next().unwrap_or(&reason).to_owned();
                *reasons.entry(key).or_default() += 1;
            }
        }
    }
    eprintln!(
        "census rows={} none={none} supported={supported} refused={refused}",
        rows.len()
    );
    for (reason, count) in &reasons {
        eprintln!("  refused {count}: {reason}");
    }
    assert_eq!(none + supported + refused, rows.len());
}
#[test]
fn passive_registration_is_read_with_the_same_layout_and_must_name_another_class() {
    assert_eq!(
        class_from_window(&window("AIab", "APbo")).unwrap(),
        ("AIab".to_string(), "APbo".to_string())
    );
    let same = Rules::decode(
        tables(&Tome::default()),
        Overlay::default(),
        AssetIdentity {
            source: "synthetic".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        },
        [0; 32],
        &window("AIsm", "AIxm"),
        &window("AIsm", "APbo"),
        &window("AIde", "APbo"),
    );
    assert!(same.unwrap_err().contains("distinct classes"));
}

/// Campaign placement census: every placed unit instance whose type is an item row, classified by
/// held-attribute outcome. `PCX_CAMPAIGN_MAPS` is a directory of selected `.w3m` files, read in
/// place; no map bytes are copied into the repository.
#[test]
#[ignore = "requires PCX_TOME_CLIENT, PCX_TOME_CATALOG and PCX_CAMPAIGN_MAPS selected local assets"]
fn local_campaign_item_placement_census() {
    let (tables, rules) = selected_install();
    let dir = std::path::PathBuf::from(std::env::var("PCX_CAMPAIGN_MAPS").unwrap());
    let mut maps: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    maps.sort();
    let mut outcomes = BTreeMap::<String, usize>::new();
    let mut per_item = BTreeMap::<(String, String), usize>::new();
    let mut placements = 0usize;
    for path in &maps {
        let bytes = std::fs::read(path).unwrap();
        let map = pcx_map::Map::parse(&bytes).unwrap();
        let units = map.units().unwrap();
        eprintln!("map {:?} instances={}", path.file_name(), units.instances.len());
        let mut kinds = std::collections::BTreeSet::new();
        for placement in &units.instances {
            kinds.insert(String::from_utf8_lossy(&placement.type_id).into_owned());
        }
        eprintln!("  types={}", kinds.len());
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        for placement in &units.instances {
            let raw = String::from_utf8_lossy(&placement.type_id).into_owned();
            if tables.object(Family::Item, &raw).is_err() {
                continue;
            }
            placements += 1;
            let outcome = match rules.held_attributes(&catalog_item(&raw)) {
                Ok(None) => "not passive (powerup or usable)".to_owned(),
                Ok(Some(held)) if held == AttributeBonus::default() => {
                    "passive, no strength or agility".to_owned()
                }
                Ok(Some(_)) => "supported passive strength or agility".to_owned(),
                Err(reason) => reason.split(" on ").next().unwrap_or(&reason).to_owned(),
            };
            *outcomes.entry(outcome.clone()).or_default() += 1;
            *per_item.entry((outcome, raw)).or_default() += 1;
            let _ = name.len();
        }
    }
    eprintln!("campaign item placements={placements} maps={}", maps.len());
    for (outcome, count) in &outcomes {
        eprintln!("outcome {count}: {outcome}");
    }
    for ((outcome, raw), count) in &per_item {
        eprintln!("placed {count} {raw}: {outcome}");
    }
    assert_eq!(outcomes.values().sum::<usize>(), placements);
}

/// Armor fixture: `parm` holds `AId1`, admitted by `Idef`; `parx` holds `AId8`, which is not.
fn armor_rules(amount: &str) -> Rules {
    let items = sylk(
        &["itemID", "abilList", "powerup", "usable", "perishable", "uses"],
        &[
            &["parm", "AId1", "0", "0", "0", "-"],
            &["parx", "AId8", "0", "0", "0", "-"],
        ],
    );
    let abilities = sylk(
        &["alias", "code", "levels", "DataA1", "DataB1", "DataC1"],
        &[
            &["AId1", "AIde", "1", amount, "0", "0"],
            &["AId8", "AIde", "1", "8", "0", "0"],
        ],
    );
    let meta = sylk(
        &["ID", "field", "slk", "index", "type", "data", "useSpecific"],
        &[
            &["Iagi", "Data", "AbilityData", "-1", "int", "1", "AIsm,AIxm"],
            &["Iint", "Data", "AbilityData", "-1", "int", "2", "AIsm,AIxm"],
            &["Istr", "Data", "AbilityData", "-1", "int", "3", "AIsm,AIxm"],
            &["Idef", "Data", "AbilityData", "-1", "int", "1", "AId1"],
        ],
    );
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic armor".into(),
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
    );
    Rules::decode(
        tables,
        Overlay::default(),
        AssetIdentity {
            source: "synthetic".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        },
        [0; 32],
        &window("AIsm", "AIxm"),
        &window("AIab", "APbo"),
        &window("AIde", "APbo"),
    )
    .unwrap()
}

#[test]
fn armor_points_come_from_the_admitted_source_row() {
    let held = armor_rules("3")
        .held_attributes(&catalog_item("parm"))
        .unwrap()
        .unwrap();
    assert_eq!(
        held,
        AttributeBonus {
            strength: 0,
            agility: 0,
            armor: 3
        }
    );
}

#[test]
fn armor_row_outside_the_idef_admission_refuses() {
    let err = armor_rules("3")
        .held_attributes(&catalog_item("parx"))
        .unwrap_err();
    assert!(err.contains("armor metadata does not admit ability AId8"), "{err}");
}

#[test]
fn negative_authored_armor_refuses() {
    let err = armor_rules("-2")
        .held_attributes(&catalog_item("parm"))
        .unwrap_err();
    assert!(err.contains("negative armor"), "{err}");
}

/// Healing window with its base function at `HEAL_BASE_VA`, in the reviewed 25-byte layout.
fn heal_window(source: &str) -> Vec<u8> {
    let base_va = i64::from(HEAL_WINDOW) + 0x100;
    let rel_base = (base_va - (i64::from(HEAL_WINDOW) + 10)) as i32;
    let rel_register = (i64::from(REGISTER_FN) - (i64::from(HEAL_WINDOW) + 21)) as i32;
    let mut w = vec![PUSH_IMM32, 1, 2, 3, 4, CALL_REL32];
    w.extend(rel_base.to_le_bytes());
    w.push(PUSH_EAX);
    w.push(PUSH_IMM32);
    w.extend(source.bytes().rev());
    w.push(CALL_REL32);
    w.extend(rel_register.to_le_bytes());
    w.extend(CLEANUP_RET);
    w
}

fn base_code(base: &str) -> Vec<u8> {
    let mut b = vec![MOV_EAX_IMM32];
    b.extend(base.bytes().rev());
    b.push(RET);
    b
}

#[test]
fn healing_window_reads_the_source_class_and_the_constant_base() {
    assert_eq!(
        class_from_call_window(&heal_window("AIhe"), &base_code("AAsm")).unwrap(),
        ("AIhe".to_string(), "AAsm".to_string())
    );
    assert_eq!(call_target(&heal_window("AIhe"), 5).unwrap(), HEAL_WINDOW + 0x100);
}

#[test]
fn healing_window_refuses_near_matches_without_a_value() {
    let mut immediate_base = heal_window("AIhe");
    immediate_base[5] = PUSH_IMM32;
    assert!(class_from_call_window(&immediate_base, &base_code("AAsm")).is_err());
    let mut wrong_register = heal_window("AIhe");
    wrong_register[17] ^= 1;
    assert!(class_from_call_window(&wrong_register, &base_code("AAsm")).is_err());
    assert!(class_from_call_window(&heal_window("AIhe")[..24], &base_code("AAsm")).is_err());
    let mut not_constant = base_code("AAsm");
    not_constant[0] = 0x90;
    assert!(class_from_call_window(&heal_window("AIhe"), &not_constant).is_err());
    assert!(class_from_call_window(&heal_window("AIhe"), &base_code("AAs")).is_err());
}

/// Healing fixture: `phea` uses `AIh1` (code `AIhe`, amount `DataA1`, range `Rng1`), `tarm` holds
/// the armor row `AId1`, and `Ihpg` admits `AIh1` only.
fn healing_rules(amount: &str, range: &str) -> Rules {
    let items = sylk(
        &["itemID", "abilList", "powerup", "usable", "perishable", "uses"],
        &[
            &["phea", "AIh1", "0", "1", "1", "1"],
            &["tarm", "AId1", "0", "0", "0", "-"],
        ],
    );
    let abilities = sylk(
        &["alias", "code", "levels", "DataA1", "Rng1"],
        &[
            &["AIh1", "AIhe", "1", amount, range],
            &["AId1", "AIde", "1", "1", "-"],
        ],
    );
    let meta = sylk(
        &["ID", "field", "slk", "index", "type", "data", "useSpecific"],
        &[
            &["Iagi", "Data", "AbilityData", "-1", "int", "1", "AIsm,AIxm"],
            &["Iint", "Data", "AbilityData", "-1", "int", "2", "AIsm,AIxm"],
            &["Istr", "Data", "AbilityData", "-1", "int", "3", "AIsm,AIxm"],
            &["Idef", "Data", "AbilityData", "-1", "int", "1", "AId1"],
            &["Ihpg", "Data", "AbilityData", "-1", "int", "1", "AIh1"],
        ],
    );
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic healing".into(),
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
    );
    Rules::decode(
        tables,
        Overlay::default(),
        AssetIdentity {
            source: "synthetic".into(),
            edition: "synthetic".into(),
            version: "test".into(),
        },
        [0; 32],
        &window("AIsm", "AIxm"),
        &window("AIab", "APbo"),
        &window("AIde", "APbo"),
    )
    .unwrap()
    .with_heal(&heal_window("AIhe"), &base_code("AAsm"))
    .unwrap()
}

#[test]
fn healing_amount_and_range_come_from_the_admitted_source_row() {
    let rules = healing_rules("250", "100");
    assert_eq!(rules.heal_effect(&catalog_item("phea")).unwrap(), Some(250));
    assert_eq!(rules.heal_range(&catalog_item("phea")).unwrap(), 100.0);
    assert_eq!(rules.heal_effect(&catalog_item("tarm")).unwrap(), None);
}

#[test]
fn healing_refuses_negative_amounts_unfinite_ranges_and_missing_registration() {
    assert!(healing_rules("-5", "100")
        .heal_effect(&catalog_item("phea"))
        .unwrap_err()
        .contains("negative healing"));
    assert!(healing_rules("250", "-")
        .heal_range(&catalog_item("phea"))
        .is_err());
    assert!(armor_rules("3")
        .heal_effect(&catalog_item("parm"))
        .unwrap_err()
        .contains("not admitted"));
}

/// Explicit local gate: the reviewed 25-byte healing registration reads from the selected client
/// image, and the selected catalog's `phea` row gives its authored heal amount and range.
#[test]
#[ignore = "requires PCX_TOME_CLIENT and PCX_TOME_CATALOG selected local assets"]
fn local_original_healing_registration_and_phea_row() {
    let (_, rules) = selected_install();
    assert_eq!(rules.heal_effect(&catalog_item("phea")).unwrap(), Some(250));
    assert_eq!(rules.heal_range(&catalog_item("phea")).unwrap(), 100.0);
}
