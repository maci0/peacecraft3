//! Orb of Venom rows and registrations: what `held_attributes` reads and refuses. The rule set is
//! decoded in `docs/re-notes/passive-held-items.md` ("`AIpb`").
use super::*;

/// Columns: alias, code, DataA1, DataB1, DataC1, DataD1, DataE1, Dur1, HeroDur1, BuffID1, targs1.
type VenomRow = [&'static str; 11];

/// Source-shaped rows: the orb row (`DataA1` is the flat damage) and its poison row.
const ORB_ROW: VenomRow = [
    "AIpb",
    "AIpb",
    "5",
    "-",
    "-",
    "-",
    "2",
    "0",
    "0",
    "",
    "ground,air,organic",
];
const POISON_ROW: VenomRow = [
    "Apo2",
    "Apo2",
    "9",
    "-",
    "-",
    "1",
    "-",
    "6",
    "8",
    "BIpb,BIpd",
    "ground,air,organic",
];

fn venom_rules_with(orb: VenomRow, poison: VenomRow, overlay: Overlay) -> Rules {
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
            &["oven", "AIpb,Apo2", "0", "0", "0", "-"],
            &["ovrv", "Apo2,AIpb", "0", "0", "0", "-"],
            &["ovno", "AIpb", "0", "0", "0", "-"],
            &["ovpo", "Apo2", "0", "0", "0", "-"],
            &["ovtw", "AIpb,Apo2,Apo2", "0", "0", "0", "-"],
        ],
    );
    let rows: Vec<Vec<&str>> = [orb, poison]
        .iter()
        .map(|row| {
            let [alias, code, a, b, c, d, e, dur, hero, buff, targs] = *row;
            vec![alias, code, "1", a, b, c, d, e, dur, hero, buff, targs]
        })
        .collect();
    let refs: Vec<&[&str]> = rows.iter().map(Vec::as_slice).collect();
    let abilities = sylk(
        &[
            "alias", "code", "levels", "DataA1", "DataB1", "DataC1", "DataD1", "DataE1", "Dur1",
            "HeroDur1", "BuffID1", "targs1",
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
            &["Iatt", "Data", "AbilityData", "-1", "int", "1", "AIat"],
        ],
    );
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic venom".into(),
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
    attach_venom(plain_rules(tables, overlay)).unwrap()
}

/// The orb base and the venom registrations over the plain classes.
fn attach_venom(rules: Rules) -> Result<Rules, String> {
    rules
        .with_orbs(
            &computed_window(ORB_BASE_WINDOW, "AIDB"),
            &base_code("AIat"),
            &window("AIfb", "AIDB"),
            &window("AIcb", "AIDB"),
        )?
        .with_venom_orb(
            &window("AIpb", "AIDB"),
            &computed_window(POISON_WINDOW, "Apo2"),
            &base_code("Apoi"),
            0.01f32.to_le_bytes(),
        )
}

fn venom_rules() -> Rules {
    venom_rules_with(ORB_ROW, POISON_ROW, Overlay::default())
}

fn poison_edited(column: usize, value: &'static str) -> VenomRow {
    let mut row = POISON_ROW;
    row[column + 1] = value;
    row
}

fn refused_poison(row: VenomRow) -> String {
    venom_rules_with(ORB_ROW, row, Overlay::default())
        .held_attributes(&catalog_item("oven"))
        .unwrap_err()
}

#[test]
fn venom_orb_reads_damage_and_the_poison_from_its_siblings_row() {
    let held = venom_rules()
        .held_attributes(&catalog_item("oven"))
        .unwrap()
        .unwrap();
    assert_eq!(held.damage, 5, "the orb row's DataA1 is the flat damage");
    assert_eq!(
        held.orb,
        Some(Orb {
            kind: OrbKind::Poison {
                damage_per_second: 9.0,
                duration: 6.0,
                hero_duration: 8.0,
                first_tick_delay: 0.01,
                buff: *b"BIpb",
            },
            damage: 5,
            air_switch: true,
            targets: OrbTargets {
                ground: true,
                air: true,
                structure: false,
                organic: true,
            },
        })
    );
    // The poison ability may precede the orb in the list.
    assert_eq!(
        venom_rules()
            .held_attributes(&catalog_item("ovrv"))
            .unwrap()
            .unwrap()
            .orb,
        held.orb
    );
}

#[test]
fn the_second_buff_bit_of_datad_selects_the_second_buffid_entry() {
    // `DataD` & 8 indexes `BuffID1` (`0xca2d70` passes `(DataD & 8) != 0` to `0xb3abe0`).
    let held = venom_rules_with(ORB_ROW, poison_edited(4, "9"), Overlay::default())
        .held_attributes(&catalog_item("oven"))
        .unwrap()
        .unwrap();
    let Some(OrbKind::Poison { buff, .. }) = held.orb.map(|orb| orb.kind) else {
        panic!("poison orb");
    };
    assert_eq!(buff, *b"BIpd");
    let err = refused_poison(row_with(poison_edited(4, "9"), 9, "BIpb"));
    assert!(err.contains("BuffID1 entry 1"), "{err}");
}

fn row_with(mut row: VenomRow, column: usize, value: &'static str) -> VenomRow {
    row[column] = value;
    row
}

#[test]
fn poison_rows_outside_the_decoded_shape_refuse() {
    // Columns after the alias and code: DataA1 1, DataB1 2, DataC1 3, DataD1 4, DataE1 5, Dur1 6,
    // HeroDur1 7, BuffID1 8, targs1 9 (`poison_edited` adds the two leading columns itself).
    assert!(refused_poison(poison_edited(1, "0")).contains("not positive"));
    assert!(refused_poison(poison_edited(1, "-4")).contains("heal over time"));
    assert!(refused_poison(poison_edited(2, "0.25")).contains("slows move or attack"));
    assert!(refused_poison(poison_edited(3, "0.25")).contains("slows move or attack"));
    // DataD & 7 == 0 is the single-buff mode, and bits above 8 are undecoded.
    assert!(refused_poison(poison_edited(4, "0")).contains("stacking mode"));
    assert!(refused_poison(poison_edited(4, "8")).contains("stacking mode"));
    assert!(refused_poison(poison_edited(4, "16")).contains("stacking mode"));
    assert!(refused_poison(poison_edited(4, "x")).contains("not an integer"));
    assert!(refused_poison(poison_edited(6, "0")).contains("duration"));
    assert!(refused_poison(poison_edited(7, "0")).contains("duration"));
    assert!(refused_poison(poison_edited(8, "BI")).contains("BuffID1"));
    assert!(refused_poison(poison_edited(9, "ground,air")).contains("different target classes"));
    assert!(refused_poison(poison_edited(9, "ground,air,enemies")).contains("not decoded"));
}

#[test]
fn the_orb_and_its_poison_must_share_one_item() {
    let rules = venom_rules();
    let err = rules.held_attributes(&catalog_item("ovno")).unwrap_err();
    assert!(err.contains("exactly one Apo2"), "{err}");
    let err = rules.held_attributes(&catalog_item("ovpo")).unwrap_err();
    assert!(err.contains("exactly one AIpb"), "{err}");
    let err = rules.held_attributes(&catalog_item("ovtw")).unwrap_err();
    assert!(err.contains("exactly one Apo2"), "{err}");
}

#[test]
fn a_map_modified_poison_row_refuses() {
    let mut overlay = Overlay::default();
    overlay
        .mods
        .insert("Apo2".into(), vec![int_mod(b"Idam", Some([1, 1]), 9)]);
    let err = venom_rules_with(ORB_ROW, POISON_ROW, overlay)
        .held_attributes(&catalog_item("oven"))
        .unwrap_err();
    assert!(err.contains("map-modified poison"), "{err}");
}

#[test]
fn venom_registrations_name_the_orb_base_and_apoi_and_stay_distinct() {
    let plain = || {
        let rules = venom_rules();
        let plain = plain_rules(Arc::clone(&rules.tables), Overlay::default());
        plain
            .with_orbs(
                &computed_window(ORB_BASE_WINDOW, "AIDB"),
                &base_code("AIat"),
                &window("AIfb", "AIDB"),
                &window("AIcb", "AIDB"),
            )
            .unwrap()
    };
    let attach = |orb: &[u8], poison: &[u8], base_fn: &[u8], tick: f32| {
        plain().with_venom_orb(orb, poison, base_fn, tick.to_le_bytes())
    };
    let (orb, poison) = (
        window("AIpb", "AIDB"),
        computed_window(POISON_WINDOW, "Apo2"),
    );
    assert!(attach(&orb, &poison, &base_code("Apoi"), 0.01).is_ok());
    let err = attach(&window("AIpb", "APbo"), &poison, &base_code("Apoi"), 0.01)
        .err()
        .unwrap();
    assert!(err.contains("orb base"), "{err}");
    let err = attach(&orb, &poison, &base_code("AIat"), 0.01)
        .err()
        .unwrap();
    assert!(err.contains("base Apoi"), "{err}");
    // A class equal to a registered one, or to its sibling, is refused.
    let err = attach(&window("AIfb", "AIDB"), &poison, &base_code("Apoi"), 0.01)
        .err()
        .unwrap();
    assert!(err.contains("distinct"), "{err}");
    let err = attach(&window("Apo2", "AIDB"), &poison, &base_code("Apoi"), 0.01)
        .err()
        .unwrap();
    assert!(err.contains("distinct"), "{err}");
    // The first tick delay read from the image must be a positive real.
    for bad in [0.0, -0.5, f32::NAN, f32::INFINITY] {
        let err = attach(&orb, &poison, &base_code("Apoi"), bad)
            .err()
            .unwrap();
        assert!(err.contains("first tick delay"), "{err}");
    }
    // The orb base must be attached first, and a truncated window is refused.
    let bare = plain_rules(Arc::clone(&venom_rules().tables), Overlay::default());
    assert!(
        bare.with_venom_orb(&orb, &poison, &base_code("Apoi"), 0.01f32.to_le_bytes())
            .is_err()
    );
    assert!(attach(&orb[..20], &poison, &base_code("Apoi"), 0.01).is_err());
    assert!(attach(&orb, &poison[..24], &base_code("Apoi"), 0.01).is_err());
}

#[test]
fn without_the_venom_registration_the_orb_stays_refused() {
    let rules = venom_rules_with(ORB_ROW, POISON_ROW, Overlay::default());
    let plain = plain_rules(Arc::clone(&rules.tables), Overlay::default());
    let err = plain.held_attributes(&catalog_item("oven")).unwrap_err();
    assert!(err.contains("unsupported passive ability AIpb"), "{err}");
}

#[test]
fn organic_targets_exclude_mechanical_units() {
    let targets = orb_targets("AIpb", "ground,air,organic").unwrap();
    assert!(targets.ground && targets.air && targets.organic && !targets.structure);
    assert!(!orb_targets("AIfb", "ground,air,ward").unwrap().organic);
}

#[test]
#[ignore = "requires PEACECRAFT3_INSTALL, the selected local Warcraft III install"]
fn the_selected_install_registers_the_venom_classes_and_reads_the_first_tick_delay() {
    let install = std::env::var_os("PEACECRAFT3_INSTALL").expect("PEACECRAFT3_INSTALL");
    let exe = std::path::Path::new(&install).join("Warcraft III.exe");
    let image = std::fs::read(&exe).expect("read the reviewed client");
    let pe = Pe::parse(&image, IMAGE_BUDGET).unwrap();
    let orb = pe.read_from(VENOM_ORB_WINDOW, true).unwrap();
    assert_eq!(
        class_from_window(&orb[..CLASS_WINDOW_LEN]).unwrap(),
        ("AIpb".to_string(), "AIDB".to_string())
    );
    let poison = pe.read_from(POISON_WINDOW, true).unwrap()[..HEAL_WINDOW_LEN].to_vec();
    let base_va = call_target(&poison, POISON_WINDOW, 5).unwrap();
    let base_fn = pe.read_from(base_va, true).unwrap()[..BASE_FN_LEN].to_vec();
    assert_eq!(
        class_from_call_window_at(&poison, POISON_WINDOW, &base_fn).unwrap(),
        ("Apo2".to_string(), "Apoi".to_string())
    );
    let delay = pe.read_from(POISON_FIRST_TICK_VA, false).unwrap();
    assert_eq!(f32::from_le_bytes(delay[..4].try_into().unwrap()), 0.01);
    println!("venom classes registered; first tick delay 0.01");
}
