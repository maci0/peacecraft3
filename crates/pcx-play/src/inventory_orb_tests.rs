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

/// The Frost orb row and the spell-resistance row, shaped like the shipped rows.
const FROST_ROW: Row = [
    "AIob",
    "AIob",
    "6",
    "-",
    "2",
    "-",
    "3",
    "1",
    "Bfro",
    "ground,air,ward",
];
const SPELL_ROW: Row = ["AIsr", "AIsr", "0", "0.33", "-", "-", "0", "0", "", ""];
/// Boots of Speed shaped rows: `DataA1` is the flat movement speed bonus.
const MOVE_ROWS: [Row; 3] = [
    ["AIms", "AIms", "60", "0", "-", "-", "0", "0", "", ""],
    ["AIm2", "AIms", "20", "0", "-", "-", "0", "0", "", ""],
    ["AIm3", "AIms", "-30", "0", "-", "-", "0", "0", "", ""],
];
/// Vampiric attack rows: the shipped shape, one without `enemy`, one with a negative fraction.
const LIFE_STEAL_ROWS: [Row; 3] = [
    [
        "AIva",
        "AIva",
        "0.5",
        "-",
        "-",
        "-",
        "0",
        "0",
        "",
        "air,ground,enemy",
    ],
    [
        "AIvn",
        "AIva",
        "0.5",
        "-",
        "-",
        "-",
        "0",
        "0",
        "",
        "air,ground",
    ],
    [
        "AIvz",
        "AIva",
        "-0.5",
        "-",
        "-",
        "-",
        "0",
        "0",
        "",
        "air,ground,enemy",
    ],
];
/// The `Misc` section the life-steal hook reads, and one that sets the magic-immune veto.
const LIFE_STEAL_PROFILES: &[(&str, &[u8])] =
    &[("synthetic MiscGame", b"[Misc]\nIllusionsCanRestoreLife=1\n")];
const MAGIC_IMMUNE_PROFILES: &[(&str, &[u8])] =
    &[("synthetic MiscGame", b"[Misc]\nMagicImmunesResistLeech=1\n")];
/// The `Misc` section the speed-bonus class reads.
const MOVE_PROFILES: &[(&str, &[u8])] =
    &[("synthetic MiscGame", b"[Misc]\nMoveSpeedBonusesStack=0\n")];
/// The `Misc` section the Frost buff reads.
const FROST_MISC: &[u8] = b"[Misc]\nFrostMoveSpeedDecrease=0.5\nFrostAttackSpeedDecrease=0.25\n";
const FROST_PROFILES: &[(&str, &[u8])] = &[("synthetic MiscGame", FROST_MISC)];

fn orb_rules_with(rows: &[Row], overlay: Overlay) -> Rules {
    orb_rules_full(rows, FROST_ROW, SPELL_ROW, overlay)
}

fn orb_rules_full(rows: &[Row], frost: Row, spell: Row, overlay: Overlay) -> Rules {
    let rows: Vec<Row> = rows
        .iter()
        .copied()
        .chain([frost, spell])
        .chain(MOVE_ROWS)
        .chain(LIFE_STEAL_ROWS)
        .collect();
    let rows = rows.as_slice();
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
            &["ofro", "AIob", "0", "0", "0", "-"],
            &["brac", "AIsr", "0", "0", "0", "-"],
            &["bra2", "AIsr,AIsr", "0", "0", "0", "-"],
            &["obob", "AIob,AIfb", "0", "0", "0", "-"],
            &["bspd", "AIms", "0", "0", "0", "-"],
            &["bsp2", "AIms,AIm2", "0", "0", "0", "-"],
            &["bsp3", "AIm3", "0", "0", "0", "-"],
            &["mskd", "AIva", "0", "0", "0", "-"],
            &["msk2", "AIva,AIva", "0", "0", "0", "-"],
            &["mskn", "AIvn", "0", "0", "0", "-"],
            &["mskz", "AIvz", "0", "0", "0", "-"],
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
        .with_frost_orb(&window("AIob", "AIDB"))
        .unwrap()
        .with_frost_slow(FROST_PROFILES, 4096)
        .unwrap()
        .with_spell_resistance(&window("AIsr", "AIat"))
        .unwrap()
        .with_move_speed_bonus(&window("AIms", "APbo"))
        .unwrap()
        .with_move_speed_stacking(MOVE_PROFILES, 4096)
        .unwrap()
        .with_life_steal(&window("AIva", "APbo"))
        .unwrap()
        .with_life_steal_misc(LIFE_STEAL_PROFILES, 4096)
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
            air_switch: true,
            targets: OrbTargets {
                ground: true,
                air: true,
                structure: false,
                organic: false
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
fn an_orb_row_with_data_e1_zero_attaches_without_the_air_switch() {
    let rules = orb_rules_with(&edited(0, 4, "0"), Overlay::default());
    let orb = rules
        .held_attributes(&catalog_item("ofir"))
        .unwrap()
        .unwrap()
        .orb
        .unwrap();
    assert!(!orb.air_switch);
    assert_eq!(orb.damage, 5, "the flat damage is still held");
    let shipped = orb_rules().held_attributes(&catalog_item("ofir")).unwrap();
    assert!(shipped.unwrap().orb.unwrap().air_switch);
    for other in ["1", "3", "-1"] {
        let rows = edited(0, 4, other);
        let err = refused(rows, Overlay::default(), "ofir");
        assert!(err.contains("is not 0 or 2"), "{other}: {err}");
    }
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

fn row_with(row: Row, column: usize, value: &'static str) -> Row {
    let mut row = row;
    row[column] = value;
    row
}

/// Rules whose frost and spell rows are replaced by the given edited rows.
fn rules_with_rows(frost: Row, spell: Row, overlay: Overlay) -> Rules {
    orb_rules_full(&ORB_ROWS, frost, spell, overlay)
}

#[test]
fn frost_orb_reads_durations_and_buff_from_its_row_and_decreases_from_misc() {
    let held = orb_rules()
        .held_attributes(&catalog_item("ofro"))
        .unwrap()
        .unwrap();
    assert_eq!(held.damage, 6);
    assert_eq!(
        held.orb,
        Some(Orb {
            kind: OrbKind::Frost {
                move_decrease: 0.5,
                attack_decrease: 0.25,
                duration: 3.0,
                hero_duration: 1.0,
                buff: *b"Bfro",
            },
            damage: 6,
            air_switch: true,
            targets: OrbTargets {
                ground: true,
                air: true,
                structure: false,
                organic: false
            },
        })
    );
}

#[test]
fn frost_decreases_follow_the_profiles_in_order_and_default_to_zero() {
    let rules = |profiles: &[(&str, &[u8])]| {
        let base = orb_rules();
        base.with_frost_slow(profiles, 4096)
    };
    let kind = |rules: Rules| {
        rules
            .held_attributes(&catalog_item("ofro"))
            .unwrap()
            .unwrap()
            .orb
            .unwrap()
            .kind
    };
    let map = b"[Misc]\nFrostMoveSpeedDecrease=0.2\n".as_slice();
    let OrbKind::Frost {
        move_decrease,
        attack_decrease,
        ..
    } = kind(rules(&[("stock", FROST_MISC), ("map", map)]).unwrap())
    else {
        panic!("frost orb");
    };
    // The later profile replaces only the key it names.
    assert_eq!((move_decrease, attack_decrease), (0.2, 0.25));
    let OrbKind::Frost {
        move_decrease,
        attack_decrease,
        ..
    } = kind(rules(&[("empty", b"[Misc]\n".as_slice())]).unwrap())
    else {
        panic!("frost orb");
    };
    assert_eq!((move_decrease, attack_decrease), (0.0, 0.0));
    // A decrease that would stall the unit is not modelled.
    for bad in ["1", "1.5", "-0.1"] {
        let text = format!("[Misc]\nFrostMoveSpeedDecrease={bad}\n");
        let err = rules(&[("bad", text.as_bytes())]).err().unwrap();
        assert!(err.contains("FrostMoveSpeedDecrease"), "{err}");
    }
}

#[test]
fn frost_orb_without_the_misc_decreases_refuses() {
    let rules = plain_with_frost_registration();
    let err = rules.held_attributes(&catalog_item("ofro")).unwrap_err();
    assert!(err.contains("FrostMoveSpeedDecrease"), "{err}");
}

/// The orb registrations and the frost class, without the `Misc` read.
fn plain_with_frost_registration() -> Rules {
    let rules = orb_rules();
    let plain = plain_rules(Arc::clone(&rules.tables), Overlay::default());
    plain
        .with_orbs(
            &computed_window(ORB_BASE_WINDOW, "AIDB"),
            &base_code("AIat"),
            &window("AIfb", "AIDB"),
            &window("AIcb", "AIDB"),
        )
        .unwrap()
        .with_frost_orb(&window("AIob", "AIDB"))
        .unwrap()
}

#[test]
fn frost_rows_outside_the_decoded_shape_refuse() {
    let none = Overlay::default;
    let refused = |frost: Row, overlay: Overlay| {
        rules_with_rows(frost, SPELL_ROW, overlay)
            .held_attributes(&catalog_item("ofro"))
            .unwrap_err()
    };
    assert!(refused(row_with(FROST_ROW, 4, "1"), none()).contains("enabled attack index"));
    assert!(refused(row_with(FROST_ROW, 6, "0"), none()).contains("duration"));
    assert!(refused(row_with(FROST_ROW, 7, "0"), none()).contains("duration"));
    assert!(refused(row_with(FROST_ROW, 8, "Bf"), none()).contains("BuffID1"));
    assert!(refused(row_with(FROST_ROW, 9, "ground,enemies"), none()).contains("not decoded"));
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIob".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    assert!(refused(FROST_ROW, overlay).contains("map-modified"));
}

#[test]
fn a_frost_orb_beside_another_orb_refuses() {
    let err = orb_rules()
        .held_attributes(&catalog_item("obob"))
        .unwrap_err();
    assert!(err.contains("two orb abilities"), "{err}");
}

#[test]
fn frost_registration_must_name_the_orb_base_and_a_distinct_class() {
    let plain = || plain_with_frost_registration_base();
    assert!(plain().with_frost_orb(&window("AIob", "AIDB")).is_ok());
    let err = plain()
        .with_frost_orb(&window("AIob", "APbo"))
        .err()
        .unwrap();
    assert!(err.contains("orb base"), "{err}");
    let err = plain()
        .with_frost_orb(&window("AIfb", "AIDB"))
        .err()
        .unwrap();
    assert!(err.contains("distinct"), "{err}");
    // Without the orb base attached there is nothing to derive from.
    let rules = orb_rules();
    let bare = plain_rules(Arc::clone(&rules.tables), Overlay::default());
    assert!(bare.with_frost_orb(&window("AIob", "AIDB")).is_err());
}

fn plain_with_frost_registration_base() -> Rules {
    let rules = orb_rules();
    plain_rules(Arc::clone(&rules.tables), Overlay::default())
        .with_orbs(
            &computed_window(ORB_BASE_WINDOW, "AIDB"),
            &base_code("AIat"),
            &window("AIfb", "AIDB"),
            &window("AIcb", "AIDB"),
        )
        .unwrap()
}

#[test]
fn spell_resistance_reads_its_fraction_and_the_flat_damage_from_its_row() {
    let held = orb_rules()
        .held_attributes(&catalog_item("brac"))
        .unwrap()
        .unwrap();
    assert_eq!(held.damage, 0);
    assert_eq!(held.spell_damage_taken, Some(1.0 - 0.33));
    assert_eq!(held.orb, None);
    // Two abilities multiply: each handler scales the record in turn.
    let held = orb_rules()
        .held_attributes(&catalog_item("bra2"))
        .unwrap()
        .unwrap();
    assert_eq!(held.spell_damage_taken, Some((1.0 - 0.33) * (1.0 - 0.33)));
    // The `AIat` base adds `DataA1` as flat damage.
    let held = rules_with_rows(FROST_ROW, row_with(SPELL_ROW, 2, "4"), Overlay::default())
        .held_attributes(&catalog_item("brac"))
        .unwrap()
        .unwrap();
    assert_eq!(held.damage, 4);
}

#[test]
fn spell_resistance_rows_outside_the_decoded_shape_refuse() {
    let refused = |spell: Row, overlay: Overlay| {
        rules_with_rows(FROST_ROW, spell, overlay)
            .held_attributes(&catalog_item("brac"))
            .unwrap_err()
    };
    assert!(refused(row_with(SPELL_ROW, 3, "1.5"), Overlay::default()).contains("fraction"));
    assert!(refused(row_with(SPELL_ROW, 3, "-0.1"), Overlay::default()).contains("fraction"));
    assert!(refused(row_with(SPELL_ROW, 2, "-1"), Overlay::default()).contains("DataA1"));
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIsr".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    assert!(refused(SPELL_ROW, overlay).contains("map-modified"));
}

#[test]
fn spell_resistance_registration_must_name_the_attack_damage_base() {
    let rules = || plain_with_frost_registration();
    assert!(
        rules()
            .with_spell_resistance(&window("AIsr", "AIat"))
            .is_ok()
    );
    let err = rules()
        .with_spell_resistance(&window("AIsr", "APbo"))
        .err()
        .unwrap();
    assert!(err.contains("attack-damage base"), "{err}");
    let err = rules()
        .with_spell_resistance(&window("AIob", "AIat"))
        .err()
        .unwrap();
    assert!(err.contains("distinct"), "{err}");
}

#[test]
fn speed_bonus_reads_data_a1_and_keeps_the_largest_of_several() {
    let held = orb_rules()
        .held_attributes(&catalog_item("bspd"))
        .unwrap()
        .unwrap();
    assert_eq!(held.move_speed_bonus, Some(60.0));
    assert_eq!(
        (held.damage, held.orb, held.spell_damage_taken),
        (0, None, None)
    );
    // Two abilities on one item: only the larger counts, as the client takes the maximum.
    let held = orb_rules()
        .held_attributes(&catalog_item("bsp2"))
        .unwrap()
        .unwrap();
    assert_eq!(held.move_speed_bonus, Some(60.0));
    // A negative bonus is admitted; it never beats the zero the client starts from.
    let held = orb_rules()
        .held_attributes(&catalog_item("bsp3"))
        .unwrap()
        .unwrap();
    assert_eq!(held.move_speed_bonus, Some(-30.0));
}

#[test]
fn speed_bonus_marks_the_skipped_refresh_and_refuses_unread_misc_and_modified_rows() {
    let held = orb_rules()
        .held_attributes(&catalog_item("bspd"))
        .unwrap()
        .unwrap();
    assert!(!held.move_speed_refresh_skipped);
    let stacking = orb_rules()
        .with_move_speed_stacking(
            &[("map", b"[Misc]\nMoveSpeedBonusesStack=1\n".as_slice())],
            4096,
        )
        .unwrap();
    let held = stacking
        .held_attributes(&catalog_item("bspd"))
        .unwrap()
        .unwrap();
    assert_eq!(held.move_speed_bonus, Some(60.0));
    assert!(held.move_speed_refresh_skipped);
    let unread = plain_rules(Arc::clone(&orb_rules().tables), Overlay::default())
        .with_move_speed_bonus(&window("AIms", "APbo"))
        .unwrap();
    let err = unread.held_attributes(&catalog_item("bspd")).unwrap_err();
    assert!(err.contains("MoveSpeedBonusesStack"), "{err}");
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIms".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    let modified = orb_rules_with(&ORB_ROWS, overlay);
    let err = modified.held_attributes(&catalog_item("bspd")).unwrap_err();
    assert!(err.contains("map-modified"), "{err}");
}

#[test]
fn speed_bonus_registration_must_name_the_passive_base() {
    let plain = || plain_rules(Arc::clone(&orb_rules().tables), Overlay::default());
    assert!(
        plain()
            .with_move_speed_bonus(&window("AIms", "APbo"))
            .is_ok()
    );
    let err = plain()
        .with_move_speed_bonus(&window("AIms", "AIat"))
        .err()
        .unwrap();
    assert!(err.contains("APbo"), "{err}");
    let err = plain()
        .with_move_speed_stacking(MOVE_PROFILES, 4096)
        .err()
        .unwrap();
    assert!(err.contains("registration"), "{err}");
    let err = plain()
        .with_move_speed_bonus(&window("AIab", "APbo"))
        .err()
        .unwrap();
    assert!(err.contains("distinct"), "{err}");
}
#[test]
fn life_steal_reads_its_fraction_and_target_classes_from_its_row() {
    let held = orb_rules()
        .held_attributes(&catalog_item("mskd"))
        .unwrap()
        .unwrap();
    assert_eq!(
        held.life_steal,
        Some(LifeSteal {
            fraction: 0.5,
            targets: OrbTargets {
                ground: true,
                air: true,
                structure: false,
                organic: false
            },
            illusions_restore: true,
        })
    );
    assert_eq!(held.damage, 0);
    assert_eq!(held.orb, None);
}

#[test]
fn life_steal_rows_outside_the_decoded_shape_refuse() {
    let rules = orb_rules();
    let refused = |raw: &str| rules.held_attributes(&catalog_item(raw)).unwrap_err();
    assert!(
        refused("msk2").contains("two life-steal"),
        "{}",
        refused("msk2")
    );
    assert!(refused("mskn").contains("does not list enemy"));
    assert!(refused("mskz").contains("nonnegative"));
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("AIva".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    let err = orb_rules_with(&ORB_ROWS, overlay)
        .held_attributes(&catalog_item("mskd"))
        .unwrap_err();
    assert!(err.contains("map-modified"), "{err}");
}

#[test]
fn life_steal_needs_its_misc_keys_and_refuses_the_magic_immune_veto() {
    let tables = Arc::clone(&orb_rules().tables);
    let plain = || {
        plain_rules(Arc::clone(&tables), Overlay::default())
            .with_life_steal(&window("AIva", "APbo"))
            .unwrap()
    };
    let err = plain().held_attributes(&catalog_item("mskd")).unwrap_err();
    assert!(err.contains("IllusionsCanRestoreLife"), "{err}");
    let veto = plain()
        .with_life_steal_misc(MAGIC_IMMUNE_PROFILES, 4096)
        .unwrap();
    let err = veto.held_attributes(&catalog_item("mskd")).unwrap_err();
    assert!(err.contains("MagicImmunesResistLeech"), "{err}");
    let unset = plain().with_life_steal_misc(&[], 4096).unwrap();
    let held = unset
        .held_attributes(&catalog_item("mskd"))
        .unwrap()
        .unwrap();
    assert!(!held.life_steal.unwrap().illusions_restore);
}

#[test]
fn life_steal_registration_must_name_the_passive_base_and_a_distinct_class() {
    let plain = || plain_rules(Arc::clone(&orb_rules().tables), Overlay::default());
    assert!(plain().with_life_steal(&window("AIva", "APbo")).is_ok());
    let err = plain()
        .with_life_steal(&window("AIva", "AIat"))
        .err()
        .unwrap();
    assert!(err.contains("APbo"), "{err}");
    let err = plain()
        .with_life_steal(&window("AIab", "APbo"))
        .err()
        .unwrap();
    assert!(err.contains("distinct"), "{err}");
    let err = plain()
        .with_life_steal_misc(LIFE_STEAL_PROFILES, 4096)
        .err()
        .unwrap();
    assert!(err.contains("registration"), "{err}");
}

#[path = "inventory_venom_tests.rs"]
mod venom;
