//! Orb rows and registrations: what `held_attributes` reads and refuses.
use super::*;

/// A computed-base registration window at `window_va`, with its base function 0x100 bytes on.
fn computed_window(window_va: u32, source: &str) -> Vec<u8> {
    let rel_base: i32 = 0x100 - 10;
    let rel_register = (i64::from(REGISTER_FN) - (i64::from(window_va) + 21)) as i32;
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

type Row = [&'static str; 10];

/// Source-shaped orb rows. Columns: alias, code, DataA1, DataB1, DataE1, Area1, Dur1, HeroDur1,
/// BuffID1, targs1. `AIgd` is not admitted by any metadata row, as in the shipped tables.
const ORB_ROWS: [Row; 3] = [
    [
        "AIfb",
        "AIfb",
        "5",
        "-",
        "2",
        "140",
        "0",
        "0",
        "",
        "ground,air,ward",
    ],
    [
        "AIgd",
        "AIfb",
        "6",
        "-",
        "2",
        "140",
        "0",
        "0",
        "",
        "ground,air,ward",
    ],
    [
        "AIcb",
        "AIcb",
        "5",
        "5",
        "2",
        "-",
        "5",
        "7",
        "BIcb",
        "ground,air,ward",
    ],
];

fn orb_rules_with(rows: &[Row], overlay: Overlay) -> Rules {
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
            &["ofir", "AIfb", "0", "0", "0", "-"],
            &["ocor", "AIcb", "0", "0", "0", "-"],
            &["ogld", "AIgd", "0", "0", "0", "-"],
            &["oboo", "AIfb,AIcb", "0", "0", "0", "-"],
        ],
    );
    let ability_rows: Vec<Vec<&str>> = rows
        .iter()
        .map(|row| {
            let [alias, code, a, b, e, area, dur, hero, buff, targs] = *row;
            vec![alias, code, "1", a, b, e, area, dur, hero, buff, targs]
        })
        .collect();
    let refs: Vec<&[&str]> = ability_rows.iter().map(Vec::as_slice).collect();
    let abilities = sylk(
        &[
            "alias", "code", "levels", "DataA1", "DataB1", "DataE1", "Area1", "Dur1", "HeroDur1",
            "BuffID1", "targs1",
        ],
        &refs,
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
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic orbs".into(),
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
    plain_rules(tables, overlay)
        .with_orbs(
            &computed_window(ORB_BASE_WINDOW, "AIDB"),
            &base_code("AIat"),
            &window("AIfb", "AIDB"),
            &window("AIcb", "AIDB"),
        )
        .unwrap()
}

/// The registered classes without the orbs, over the given tables.
fn plain_rules(tables: Arc<DefaultTables>, overlay: Overlay) -> Rules {
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

fn orb_rules() -> Rules {
    orb_rules_with(&ORB_ROWS, Overlay::default())
}

#[test]
fn fire_orb_reads_damage_area_and_target_classes_from_its_row() {
    let held = orb_rules()
        .held_attributes(&catalog_item("ofir"))
        .unwrap()
        .unwrap();
    assert_eq!(held.damage, 5);
    assert_eq!(
        held.orb,
        Some(Orb {
            kind: OrbKind::Fire { area: 140.0 },
            damage: 5,
            targets: OrbTargets {
                ground: true,
                air: true,
                structure: false
            },
        })
    );
}

#[test]
fn kiljaeden_orb_takes_its_amount_from_its_own_row_without_metadata_admission() {
    let held = orb_rules()
        .held_attributes(&catalog_item("ogld"))
        .unwrap()
        .unwrap();
    assert_eq!(held.damage, 6);
    assert_eq!(
        held.orb.map(|orb| orb.kind),
        Some(OrbKind::Fire { area: 140.0 })
    );
}

#[test]
fn corruption_orb_reads_penalty_durations_and_buff_from_its_row() {
    let held = orb_rules()
        .held_attributes(&catalog_item("ocor"))
        .unwrap()
        .unwrap();
    assert_eq!(held.damage, 5);
    assert_eq!(
        held.orb.map(|orb| orb.kind),
        Some(OrbKind::Corruption {
            armor_penalty: 5,
            duration: 5.0,
            hero_duration: 7.0,
            buff: *b"BIcb",
        })
    );
}

fn refused(rows: [Row; 3], overlay: Overlay, item: &str) -> String {
    orb_rules_with(&rows, overlay)
        .held_attributes(&catalog_item(item))
        .unwrap_err()
}

fn edited(row: usize, column: usize, value: &'static str) -> [Row; 3] {
    let mut rows = ORB_ROWS;
    rows[row][column] = value;
    rows
}

#[test]
fn orb_rows_outside_the_decoded_shape_refuse() {
    let none = Overlay::default;
    // DataE1 other than the second attack, a negative amount and penalty.
    assert!(refused(edited(0, 4, "1"), none(), "ofir").contains("enabled attack index"));
    assert!(refused(edited(0, 2, "-1"), none(), "ofir").contains("damage is negative"));
    assert!(refused(edited(2, 3, "-1"), none(), "ocor").contains("armor penalty"));
    // A fractional damage is not an integer amount.
    assert!(refused(edited(0, 2, "5.5"), none(), "ofir").contains("not an integer"));
    // The splash needs a radius; a timer-less buff is not decoded.
    assert!(refused(edited(0, 5, "0"), none(), "ofir").contains("Area1"));
    assert!(refused(edited(2, 6, "0"), none(), "ocor").contains("duration"));
    assert!(refused(edited(2, 7, "0"), none(), "ocor").contains("duration"));
    assert!(refused(edited(2, 8, "BI"), none(), "ocor").contains("BuffID1"));
    // Relationship and state tokens are resolved by the client's target query.
    assert!(refused(edited(0, 9, "ground,enemies"), none(), "ofir").contains("not decoded"));
    assert!(refused(edited(0, 9, "_"), none(), "ofir").contains("not decoded"));
    // A map that modifies an orb row is not decoded.
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIfb".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    assert!(refused(ORB_ROWS, overlay, "ofir").contains("map-modified"));
}

#[test]
fn an_item_with_two_orb_abilities_refuses() {
    let err = orb_rules()
        .held_attributes(&catalog_item("oboo"))
        .unwrap_err();
    assert!(err.contains("two orb abilities"), "{err}");
}

#[test]
fn structure_class_in_targs_covers_buildings() {
    let rows = edited(0, 9, "ground,structure");
    let held = orb_rules_with(&rows, Overlay::default())
        .held_attributes(&catalog_item("ofir"))
        .unwrap()
        .unwrap();
    let targets = held.orb.unwrap().targets;
    assert!(targets.ground && targets.structure && !targets.air);
}

#[test]
fn orb_registration_must_chain_through_the_attack_damage_base_and_stay_distinct() {
    let attach = |fire: &[u8], corruption: &[u8], base_fn: &[u8]| {
        let rules = orb_rules_with(&ORB_ROWS, Overlay::default());
        let plain = plain_rules(Arc::clone(&rules.tables), Overlay::default());
        plain.with_orbs(
            &computed_window(ORB_BASE_WINDOW, "AIDB"),
            base_fn,
            fire,
            corruption,
        )
    };
    let (fire, corruption) = (window("AIfb", "AIDB"), window("AIcb", "AIDB"));
    assert!(attach(&fire, &corruption, &base_code("AIat")).is_ok());
    // The base must derive from the registered flat attack-damage class.
    let err = attach(&fire, &corruption, &base_code("APbo"))
        .err()
        .unwrap();
    assert!(err.contains("attack-damage"), "{err}");
    // Both orb classes must name the orb base.
    let err = attach(&window("AIfb", "APbo"), &corruption, &base_code("AIat"))
        .err()
        .unwrap();
    assert!(err.contains("orb base"), "{err}");
    // An orb class equal to another registered class, or to its sibling, is refused.
    let err = attach(&window("AIat", "AIDB"), &corruption, &base_code("AIat"))
        .err()
        .unwrap();
    assert!(err.contains("distinct"), "{err}");
    let err = attach(&fire, &window("AIfb", "AIDB"), &base_code("AIat"))
        .err()
        .unwrap();
    assert!(err.contains("distinct"), "{err}");
    // A truncated base window is refused.
    let rules = orb_rules_with(&ORB_ROWS, Overlay::default());
    let plain = plain_rules(Arc::clone(&rules.tables), Overlay::default());
    assert!(
        plain
            .with_orbs(
                &computed_window(ORB_BASE_WINDOW, "AIDB")[..24],
                &base_code("AIat"),
                &fire,
                &corruption
            )
            .is_err()
    );
}

#[test]
fn without_the_orb_registration_an_orb_row_stays_refused() {
    let items = sylk(
        &[
            "itemID",
            "abilList",
            "powerup",
            "usable",
            "perishable",
            "uses",
        ],
        &[&["ofir", "AIfb", "0", "0", "0", "-"]],
    );
    let abilities = sylk(
        &["alias", "code", "levels", "DataA1"],
        &[&["AIfb", "AIfb", "1", "5"]],
    );
    let meta = sylk(
        &["ID", "field", "slk", "index", "type", "data", "useSpecific"],
        &[
            &["Iagi", "Data", "AbilityData", "-1", "int", "1", "AIsm,AIxm"],
            &["Iint", "Data", "AbilityData", "-1", "int", "2", "AIsm,AIxm"],
            &["Istr", "Data", "AbilityData", "-1", "int", "3", "AIsm,AIxm"],
            &["Idef", "Data", "AbilityData", "-1", "int", "1", "AId1"],
            &["Iatt", "Data", "AbilityData", "-1", "int", "1", "AIat"],
        ],
    );
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic orbs".into(),
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
    let err = plain_rules(tables, Overlay::default())
        .held_attributes(&catalog_item("ofir"))
        .unwrap_err();
    assert!(err.contains("unsupported passive ability AIfb"), "{err}");
}
