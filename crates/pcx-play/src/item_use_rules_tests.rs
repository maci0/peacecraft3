use super::*;
use pcx_objects::editor::defaults::Identity;
use pcx_objects::editor::{EditorReadLimits, ObjectKind, SlkId};
use pcx_objects::{Mod, ValueType};
use pcx_sim::world::{FieldValue, ObjectCatalog, ObjectField};
use std::collections::BTreeMap;

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

/// `AIh1` (alias) with `AIhe` (code) and level-1 `Cool1` 20, and `AIh2` with `Cool1` 0.
fn tables() -> Arc<DefaultTables> {
    let ability = sylk(
        &["alias", "code", "levels", "Cool1"],
        &[
            &["AIh1", "AIhe", "1", "20"],
            &["AIh2", "AIhe", "1", "0"],
            &["AIzz", "AIzz", "1", "5"],
        ],
    );
    Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic item use".into(),
                edition_version: "fixture".into(),
            },
            &[(SlkId::AbilityData.path(), &ability)],
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

fn rules(overlay: Overlay) -> Rules {
    Rules::new(tables(), overlay)
}

/// An item definition with the given authored cells. `Missing` is written as `None`.
fn item(cells: &[(&[u8; 4], Option<&str>)]) -> Definition {
    let fields = cells
        .iter()
        .map(|(fourcc, text)| {
            let value = match text {
                Some(text) => Ok(FieldValue::Cell((*text).into())),
                None => Err(FieldProblem::Missing),
            };
            (
                **fourcc,
                ObjectField {
                    metadata: None,
                    value,
                },
            )
        })
        .collect();
    Definition {
        rawcode: *b"ITM1",
        base_rawcode: "synthetic base".into(),
        kind: ObjectKind::Item,
        fields,
        modifications: Vec::new(),
    }
}

/// The potion row shape: one healing ability, group `AIhe`, charged once, perishable.
fn potion(uses: &str) -> Definition {
    item(&[
        (b"iuse", Some(uses)),
        (b"iper", Some("1")),
        (b"iicd", Some("0")),
        (b"icid", Some("AIhe")),
        (b"iabi", Some("AIh1")),
    ])
}

#[test]
fn uses_cell_is_the_initial_charge_model() {
    let r = rules(Overlay::default());
    assert_eq!(r.charges(&potion("-")), Ok(Charges::Uncharged));
    assert_eq!(r.charges(&potion("3")), Ok(Charges::Charged(3)));
    assert!(
        r.charges(&potion("many"))
            .unwrap_err()
            .contains("not a charge count")
    );
    assert!(
        r.charges(&potion("-1"))
            .unwrap_err()
            .contains("not a charge count")
    );
    let missing = item(&[(b"iuse", None)]);
    assert!(r.charges(&missing).unwrap_err().contains("not derived"));
    assert!(r.charges(&item(&[])).unwrap_err().contains("absent"));
}

#[test]
fn perishable_reads_its_boolean_flag() {
    let r = rules(Overlay::default());
    assert_eq!(r.perishable(&potion("1")), Ok(true));
    assert_eq!(r.perishable(&item(&[(b"iper", Some("0"))])), Ok(false));
    assert!(r.perishable(&item(&[(b"iper", Some("yes"))])).is_err());
}

#[test]
fn cooldown_group_and_duration_come_from_the_items_level_one_ability() {
    let r = rules(Overlay::default());
    assert_eq!(
        r.cooldown(&potion("1")),
        Ok(Some(Cooldown {
            group: "AIhe".into(),
            seconds: 20.0,
            ignores_wait: false,
        }))
    );
}

#[test]
fn an_absent_or_empty_cooldown_id_names_no_group() {
    let r = rules(Overlay::default());
    assert_eq!(
        r.cooldown(&item(&[(b"icid", None), (b"iabi", None)])),
        Ok(None)
    );
    assert_eq!(
        r.cooldown(&item(&[(b"icid", Some("")), (b"iabi", Some("AIh1"))])),
        Ok(None)
    );
}

#[test]
fn ignore_cooldown_keeps_the_group_but_lifts_the_wait() {
    let r = rules(Overlay::default());
    let mut def = potion("1");
    def.fields.insert(
        *b"iicd",
        ObjectField {
            metadata: None,
            value: Ok(FieldValue::Cell("1".into())),
        },
    );
    let cooldown = r.cooldown(&def).unwrap().unwrap();
    assert_eq!(
        (cooldown.group.as_str(), cooldown.ignores_wait),
        ("AIhe", true)
    );
}

#[test]
fn a_group_needs_one_listed_ability_as_its_duration_source() {
    let r = rules(Overlay::default());
    let mixed = item(&[(b"icid", Some("AIhe")), (b"iabi", Some("AIh1,AIh2")), (b"iicd", Some("0"))]);
    assert!(r.cooldown(&mixed).unwrap_err().contains("duration source unresolved, 2 item abilities"));
    let none = item(&[(b"icid", Some("AIhe")), (b"iabi", Some("_")), (b"iicd", Some("0"))]);
    assert!(r.cooldown(&none).unwrap_err().contains("duration source unresolved, 0 item abilities"));
    let absent = item(&[(b"icid", Some("AIhe")), (b"iabi", Some("Xnope")), (b"iicd", Some("0"))]);
    assert!(r.cooldown(&absent).is_err(), "an absent ability row is not skipped");
}

#[test]
fn the_group_identity_is_separate_from_the_duration_alias() {
    let r = rules(Overlay::default());
    // The group names no ability of the item, and it still shares the group with its one ability.
    let other = item(&[(b"icid", Some("ZZZZ")), (b"iabi", Some("AIh1")), (b"iicd", Some("0"))]);
    assert_eq!(
        r.cooldown(&other).unwrap().unwrap(),
        Cooldown { group: "ZZZZ".into(), seconds: 20.0, ignores_wait: false }
    );
    // A group equal to one of several listed aliases or codes never selects a duration source.
    let direct = item(&[(b"icid", Some("AIzz")), (b"iabi", Some("AIh1,AIzz")), (b"iicd", Some("0"))]);
    assert!(r.cooldown(&direct).unwrap_err().contains("2 item abilities"));
    let code = item(&[(b"icid", Some("AIhe")), (b"iabi", Some("AIzz,AIh1")), (b"iicd", Some("0"))]);
    assert!(r.cooldown(&code).unwrap_err().contains("2 item abilities"));
    let single_alias = item(&[(b"icid", Some("AIzz")), (b"iabi", Some("AIzz")), (b"iicd", Some("0"))]);
    assert_eq!(r.cooldown(&single_alias).unwrap().unwrap().seconds, 5.0);
}

#[test]
fn a_zero_cooldown_is_a_group_with_no_wait() {
    let r = rules(Overlay::default());
    let mut def = potion("1");
    def.fields.insert(
        *b"iabi",
        ObjectField {
            metadata: None,
            value: Ok(FieldValue::Cell("AIh2".into())),
        },
    );
    assert_eq!(r.cooldown(&def).unwrap().unwrap().seconds, 0.0);
}

#[test]
fn w3a_cooldown_override_replaces_the_base_level_one_value() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "AIh1".into(),
        vec![Mod {
            field: *b"acdn",
            value_type: ValueType::Real,
            selectors: Some([1, 0]),
            value: pcx_objects::Value::Real(12.5),
            end_word: 0,
        }],
    );
    assert_eq!(
        rules(overlay)
            .cooldown(&potion("1"))
            .unwrap()
            .unwrap()
            .seconds,
        12.5
    );
}

#[test]
fn w3a_override_of_another_level_leaves_the_base_value() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "AIh1".into(),
        vec![Mod {
            field: *b"acdn",
            value_type: ValueType::Real,
            selectors: Some([2, 0]),
            value: pcx_objects::Value::Real(99.0),
            end_word: 0,
        }],
    );
    assert_eq!(
        rules(overlay)
            .cooldown(&potion("1"))
            .unwrap()
            .unwrap()
            .seconds,
        20.0
    );
}

#[test]
fn a_negative_or_indexed_cooldown_override_refuses() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "AIh1".into(),
        vec![Mod {
            field: *b"acdn",
            value_type: ValueType::Real,
            selectors: Some([1, 0]),
            value: pcx_objects::Value::Real(-1.0),
            end_word: 0,
        }],
    );
    assert!(
        rules(overlay)
            .cooldown(&potion("1"))
            .unwrap_err()
            .contains("nonnegative")
    );
    let mut indexed = Overlay::default();
    indexed.mods.insert(
        "AIh1".into(),
        vec![Mod {
            field: *b"acdn",
            value_type: ValueType::Real,
            selectors: Some([1, 1]),
            value: pcx_objects::Value::Real(3.0),
            end_word: 0,
        }],
    );
    assert!(
        rules(indexed)
            .cooldown(&potion("1"))
            .unwrap_err()
            .contains("indexed")
    );
}

/// Original-input gate: every selected `ItemData` row resolves through the metadata catalog, and
/// the charge, perishable and cooldown reads all derive without a refusal.
/// `PCX_ITEM_USE_CATALOG` is the selected `units` directory with `itemdata.slk`, `abilitydata.slk`
/// and `abilitymetadata.slk`, and `unitmetadata.slk` (the field schema for `ItemData` columns).
#[test]
#[ignore = "requires PCX_ITEM_USE_CATALOG selected local assets"]
fn local_original_item_use_census_derives_every_row() {
    let catalog = std::path::PathBuf::from(std::env::var("PCX_ITEM_USE_CATALOG").unwrap());
    let read = |name: &str| std::fs::read(catalog.join(name)).unwrap();
    let (item_rows, ability, meta, unit_meta) = (
        read("itemdata.slk"),
        read("abilitydata.slk"),
        read("abilitymetadata.slk"),
        read("unitmetadata.slk"),
    );
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: catalog.display().to_string(),
                edition_version: "selected install".into(),
            },
            &[
                (SlkId::ItemData.path(), &item_rows),
                (SlkId::AbilityData.path(), &ability),
                (SlkId::AbilityMetaData.path(), &meta),
                (SlkId::UnitMetaData.path(), &unit_meta),
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
    let overlay = Overlay::default();
    let rules = Rules::new(Arc::clone(&tables), overlay.clone());
    let units = Overlay::default();
    let catalog = pcx_sim::world::MetadataCatalog {
        editor: tables.editor(),
        units: &units,
        items: &overlay,
    };
    let (mut charged, mut uncharged, mut grouped) = (0usize, 0usize, 0usize);
    let mut refused_usable = Vec::new();
    let mut refused_passive: BTreeMap<String, usize> = BTreeMap::new();
    let raws: Vec<String> = tables.rawcodes(Family::Item).map(str::to_owned).collect();
    for raw in &raws {
        let rawcode: [u8; 4] = raw.as_bytes().try_into().unwrap();
        let definition = catalog.resolve(rawcode).unwrap();
        let usable = matches!(
            definition.fields.get(b"iusa").map(|f| &f.value),
            Some(Ok(FieldValue::Cell(text))) if text.trim() == "1"
        );
        let charges = rules.charges(&definition);
        match charges {
            Ok(Charges::Charged(_)) => charged += 1,
            Ok(Charges::Uncharged) => uncharged += 1,
            Err(_) => {}
        }
        let refusal = charges
            .err()
            .or_else(|| rules.perishable(&definition).err())
            .or_else(|| rules.cooldown(&definition).err());
        match (usable, refusal) {
            (true, Some(e)) => refused_usable.push(format!("{raw}: {e}")),
            (false, Some(e)) => *refused_passive.entry(e).or_default() += 1,
            (_, None) => {}
        }
        if rules.cooldown(&definition).is_ok_and(|c| c.is_some()) {
            grouped += 1;
        }
    }
    println!("census rows={} charged={charged} uncharged={uncharged} grouped={grouped}", raws.len());
    println!("census refused usable rows={}: {refused_usable:?}", refused_usable.len());
    for (reason, count) in &refused_passive {
        println!("census refused passive rows={count}: {reason}");
    }
    assert_eq!(charged + uncharged, raws.len());
}
