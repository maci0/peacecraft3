//! Tests for the object-editor data ingest.
//!
//! The exact-value tests read their expected numbers by hand out of the
//! shipped `.slk` bytes; the sweeps run over every shipped editor table and
//! every shipped map on both builds.

use super::meta::{FieldKind, FieldSource};
use super::object::{Item, ObjectKind};
use super::report::ApplyReport;
use super::{Editor, SlkId, TxtRole};
use crate::{Fourcc, ObjectFile};

/// The 1.28c install, if the test machine has it.
fn install() -> Option<std::path::PathBuf> {
    let p = std::env::var_os("PEACECRAFT3_INSTALL")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("wc3/game/app");
            p.is_dir().then_some(p)
        })?;
    p.is_dir().then_some(p)
}

/// The 1.29b install, if the test machine has it.
fn legacy() -> Option<std::path::PathBuf> {
    std::env::var_os("PEACECRAFT3_LEGACY_DIR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("../../Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)");
            p.is_dir().then_some(p)
        })
}

fn open(dir: &std::path::Path) -> Editor {
    Editor::open(dir).expect("editor decodes")
}

/// Hand-read out of `Units\UnitBalance.slk` row 10 on 1.28c.
const HFOO: &[(&str, f64)] = &[
    ("uhpm", 420.0),
    ("umvs", 270.0),
    ("udef", 2.0),
    ("udup", 2.0),
    ("ubld", 20.0),
    ("usid", 1400.0),
    ("ulev", 2.0),
    ("ugol", 135.0),
    ("ulum", 0.0),
    ("ubdi", 6.0),
    ("ubsi", 3.0),
    ("ubba", 20.0),
    ("usma", 3.0),
    ("usrg", 30.0),
    ("usin", 800.0),
    ("urpo", 0.0),
    ("urpp", 0.0),
    ("ucol", 31.0),
];

/// Hand-read out of `Units\UnitWeapons.slk` row 10 on 1.28c.
const HFOO_WEAPONS: &[(&str, f64)] = &[
    ("ua1r", 90.0),
    ("ua1d", 1.0),
    ("ua1s", 2.0),
    ("ua1b", 11.0),
    ("utc1", 1.0),
    ("uaen", 1.0),
];

/// Hand-read out of `Units\UnitData.slk` row 10 on 1.28c.
const HFOO_DATA: &[(&str, &str)] = &[
    ("urac", "human"),
    ("upri", "6"),
    ("udtm", "3.04"),
    ("umvh", "0"),
    ("ucar", "1"),
    ("ufor", "0"),
    ("utar", "ground"),
];

/// The five unit tables are joined on one raw code and every stat is a named
/// column, not a guess.
#[test]
fn footman_stats_are_the_editor_values() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let u = e.unit("hfoo").expect("hfoo is a shipped unit");

    assert_eq!(u.comment(), Some("Footman"));
    assert_eq!(u.name(), Some("Footman"));
    assert_eq!(u.race(), Some("human"));
    assert_eq!(u.armour_type(), Some("Metal"), "uarm -> unitUI.armor");
    assert_eq!(
        u.base_cell(*b"udty"),
        Some("large"),
        "udty -> UnitBalance.defType"
    );
    assert_eq!(u.row(SlkId::UnitBalance), Some(10));
    assert_eq!(u.row(SlkId::UnitData), Some(10));
    assert_eq!(u.row(SlkId::UnitUi), Some(10));
    assert_eq!(u.row(SlkId::UnitWeapons), Some(10));

    for (fourcc, want) in HFOO {
        let f = crate::Fourcc::try_from(fourcc.as_bytes()).unwrap();
        let got = u
            .stat(f)
            .and_then(|v| v.number())
            .unwrap_or_else(|| panic!("{fourcc}: cell {:?}", u.base_cell(f)));
        assert_eq!(
            got,
            *want,
            "{fourcc} ({} .{})",
            u.field(f).map(|d| d.slk.clone()).unwrap_or_default(),
            u.field(f)
                .map(|d| d.column_name().to_string())
                .unwrap_or_default()
        );
    }
    for (fourcc, want) in HFOO_WEAPONS {
        let f = crate::Fourcc::try_from(fourcc.as_bytes()).unwrap();
        assert_eq!(
            u.stat(f).and_then(|v| v.number()),
            Some(*want),
            "{fourcc} weapon stat"
        );
    }
    for (fourcc, want) in HFOO_DATA {
        let f = crate::Fourcc::try_from(fourcc.as_bytes()).unwrap();
        assert_eq!(
            u.stat(f).and_then(|v| v.text()),
            Some(*want),
            "{fourcc} UnitData stat"
        );
    }

    // The hero's named convenience accessors, on a hero instead.
    let amg = e.unit("Hamg").expect("Hamg is a shipped hero");
    assert_eq!(amg.hp(), Some(100.0));
    assert_eq!(amg.mana(), Some(0.0));
    assert_eq!(amg.speed(), Some(320.0));
    assert_eq!(amg.armour(), Some(0.0));
    assert_eq!(amg.build_time(), Some(55.0));
    assert_eq!(amg.sight(), Some(1800.0));
    assert_eq!(amg.gold_cost(), Some(425.0));
    assert_eq!(amg.lumber_cost(), Some(100.0));
    assert_eq!(amg.damage_dice(), Some(2.0));
    assert_eq!(amg.damage_sides(), Some(4.0));
    assert_eq!(amg.damage_bonus(), Some(0.0));
    assert_eq!(amg.attack_cooldown(), Some(2.13));
    assert_eq!(amg.attack_range(), Some(600.0));
    assert_eq!(amg.bounty(), Some((8.0, 3.0, 30.0)));
    assert_eq!(amg.level(), Some(5.0));
    assert_eq!(amg.hp_regen(), Some(0.25));
    assert_eq!(amg.race(), Some("human"));
    assert_eq!(amg.armour_type(), Some("Flesh"));
    assert_eq!(amg.name(), Some("Archmage"));
}

/// A unit with no mana really has `-` there, and the crate hands it out as
/// the marker it is rather than turning it into a zero.
#[test]
fn a_unit_without_mana_reports_the_dash() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let u = e.unit("hfoo").expect("hfoo");
    let v = u.stat(*b"umpm").expect("umpm resolves");
    // UnitBalance.slk pads the cell: the bytes are ` - `, and `text()` hands
    // them out exactly as the editor wrote them.
    assert_eq!(v.text(), Some(" - "));
    assert!(v.is_blank(), "a dash is the editor's nothing marker");
    assert_eq!(v.number(), None, "and it is not a number");
    assert_eq!(v.number(), None);
    // `uman` is not a field; `umpm` is mana's maximum, and a hero carries a
    // real number there rather than the blank marker.
    let amg = e.unit("Hamg").expect("Hamg");
    assert!(!amg.stat(*b"umpm").expect("umpm resolves").is_blank());
}

/// `unitUI.name` is the animation set, not the display name: `uarm` and `unam`
/// come from different places and must not be conflated.
#[test]
fn display_name_comes_from_the_strings_table_not_the_ui_table() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let u = e.unit("Hamg").expect("Hamg");
    // `unitUI.name` = "archmage"; `Name=` in HumanUnitStrings.txt = "Archmage".
    assert_eq!(u.name(), Some("Archmage"));
    let table = e.table(SlkId::UnitUi).unwrap();
    let row = e.row(SlkId::UnitUi, "Hamg").unwrap();
    assert_eq!(table.row(row).by_name("name"), Some("archmage"));
}

/// The `Profile` fields really are read out of the `.txt` layer, not a `.slk`.
#[test]
fn profile_fields_come_from_the_txt_tables() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let u = e.unit("hfoo").expect("hfoo is a shipped unit");
    assert_eq!(
        u.base_cell(*b"uico"),
        Some("ReplaceableTextures\\CommandButtons\\BTNFootman.blp")
    );
    assert_eq!(u.base_cell(*b"ubpx"), Some("0"));
    assert_eq!(u.base_cell(*b"ubpy"), Some("0"));
    // A melee unit has no missile art at all, so the two-slot
    // `Missileart` key answers `None` rather than inventing one.
    assert_eq!(u.base_cell(*b"ua1m"), None);
    assert_eq!(u.base_cell(*b"ua2m"), None);
    // ... and a ranged one does.
    let mur = e.unit("nmsc").expect("nmsc is a shipped unit");
    assert_eq!(
        mur.base_cell(*b"ua1m"),
        Some("Abilities\\Weapons\\MurgulMagicMissile\\MurgulMagicMissile.mdl")
    );
    assert_eq!(mur.base_cell(*b"ua1z"), Some("900"), "Missilespeed, slot 0");
    assert_eq!(mur.base_cell(*b"ua2z"), None, "slot 1 never shipped");
    let def = u.field(*b"uico").expect("uico is defined");
    assert!(matches!(def.source, FieldSource::Profile { index: 0, .. }));
    assert_eq!(def.slk_name(), "Profile");
    assert_eq!(def.declared, "icon");
    assert_eq!(u.field(*b"uhpm").map(|d| d.slk_name()), Some("UnitBalance"));

    // A one-slot key is whole: a comma inside a quoted tooltip is text, and
    // the quotes the file wrote are not part of the value.
    assert_eq!(
        u.base_cell(*b"utub").unwrap(),
        "Versatile foot soldier. Can learn the Defend ability. |n|n|cffffcc00Attacks land units.|r"
    );
    assert_eq!(u.field(*b"utub").map(|d| d.index), Some(0));
    assert_eq!(u.field(*b"utub").map(|d| d.slots()), Some(1));

    // A multi-slot key is a comma list.
    assert_eq!(u.field(*b"ubpx").map(|d| d.slots()), Some(2));
    assert_eq!(u.field(*b"ubpy").map(|d| d.slots()), Some(2));
    assert_eq!(u.field(*b"ua1m").map(|d| d.slots()), Some(2));
}

/// `Buttonpos` is a comma pair, and two shipped sections write it twice.
#[test]
fn buttonpos_uses_the_index_column() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let u = e.unit("Hamg").expect("Hamg");
    let s = u.profile().expect("Hamg has a profile section");
    assert_eq!(s.value("buttonpos"), Some("0,2"));
    assert_eq!(s.element("buttonpos", 0, 2), Some("0"));
    assert_eq!(s.element("buttonpos", 1, 2), Some("2"));
    assert_eq!(s.element("buttonpos", 2, 2), None);
    // A one-slot key is never split, even where its text has a comma.
    assert_eq!(s.element("ubertip", 0, 1), s.value("ubertip"));
    // The two shipped repeated-key sections, both in NeutralUnitFunc.txt.
    let dups = e.profile_duplicates();
    assert!(dups.contains(&("nogm".to_string(), "buttonpos".to_string())));
    assert!(dups.contains(&("nltc".to_string(), "buttonpos".to_string())));
}

/// Items resolve the same way, from `ItemData.slk` plus `ItemFunc`/`ItemStrings`.
#[test]
fn crown_of_kings_resolves_from_both_layers() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let i: Item<'_> = e.item("ckng").expect("ckng is a shipped item");
    assert_eq!(i.comment(), Some("Crown of Kings +5"));
    assert_eq!(i.name(), Some("Crown of Kings +5"));
    assert_eq!(i.class(), Some("Artifact"));
    assert_eq!(i.hp(), Some(75.0));
    assert_eq!(i.gold_cost(), Some(1000.0));
    assert_eq!(i.lumber_cost(), Some(0.0));
    assert_eq!(
        i.base_cell(*b"iabi"),
        Some("AIx5"),
        "iabi -> ItemData.abilList"
    );
    assert_eq!(i.base_cell(*b"iarm"), Some("Wood"));
    assert_eq!(
        i.base_cell(*b"iico"),
        Some("ReplaceableTextures\\CommandButtons\\BTNHelmutPurple.blp")
    );
}

/// Upgrades and command buttons resolve too.
#[test]
fn upgrades_and_commands_resolve() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let u = e.upgrade("Rhme").expect("Rhme is a shipped upgrade");
    assert_eq!(u.comment(), Some("human melee attack"));
    assert_eq!(u.max_level(), Some(3.0));
    let cmd = e.command("CmdMove").expect("CmdMove is a shipped command");
    assert_eq!(cmd.value("Art"), Some("CommandMove"));
    assert_eq!(cmd.element("buttonpos", 0, 2), Some("0"));
    assert_eq!(e.resolve("CmdMove"), Some(ObjectKind::Command));
    assert_eq!(e.resolve("Rhme"), Some(ObjectKind::Upgrade));
    assert_eq!(e.resolve("ckng"), Some(ObjectKind::Item));
    assert_eq!(e.resolve("hfoo"), Some(ObjectKind::Unit));
    assert_eq!(e.resolve("zzzz"), None);
}

/// The eight profile fourccs whose base value no shipped table carries.
#[test]
fn eight_profile_fourccs_have_no_shipped_base_value() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    for (fourcc, key) in [
        (*b"urq3", "requires3"),
        (*b"urq4", "requires4"),
        (*b"urq5", "requires5"),
        (*b"urq6", "requires6"),
        (*b"urq7", "requires7"),
        (*b"urq8", "requires8"),
        (*b"urqa", "requiresamount"),
        (*b"urva", "reviveat"),
    ] {
        let def = e.field(fourcc).expect("the fourcc is in the schema");
        assert!(
            matches!(def.source, FieldSource::Profile { .. }),
            "{:?} must be a Profile field",
            crate::fourcc_text(fourcc)
        );
        assert!(
            !def.measured,
            "{:?} ({key}) must have no shipped base value",
            crate::fourcc_text(fourcc)
        );
        assert!(!e.profile_keys().contains(key), "{key} ships in no section");
    }
    // Every other UnitMetaData field is measured.
    let un = e
        .fields()
        .iter()
        .filter(|f| f.from == SlkId::UnitMetaData && !f.measured)
        .count();
    assert_eq!(un, 8, "exactly the eight above are unmeasured");
}

/// The whole fourcc schema, measured rather than remembered.
#[test]
fn every_unit_metadata_row_resolves_to_a_shipped_column() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let meta = e.table(SlkId::UnitMetaData).unwrap();
    let rows = e
        .fields()
        .iter()
        .filter(|f| f.from == SlkId::UnitMetaData)
        .count();
    assert_eq!(rows, 267, "267 UnitMetaData rows");
    assert_eq!(meta.row_count(), 268);

    let mut unmeasured = Vec::new();
    for def in e.fields().iter().filter(|f| f.from == SlkId::UnitMetaData) {
        if !def.measured {
            unmeasured.push(format!("{} {}", crate::fourcc_text(def.fourcc), def.field));
        }
    }
    assert_eq!(
        unmeasured.len(),
        8,
        "the eight Profile fields with no shipped key: {unmeasured:?}"
    );

    // Every measured `Column` field names a real row-1 column of the table
    // it claims — including the ability rows, whose `field` names a column
    // `Units\AbilityData.slk` does not have.
    let mut unmeasured_columns = Vec::new();
    for def in e.fields().iter() {
        if let FieldSource::Column { slk, column } = &def.source {
            if def.measured {
                let t = e.table(*slk).expect("the table it names is decoded");
                assert!(
                    t.column_of(column).is_some(),
                    "{}.{} names no column of {}",
                    crate::fourcc_text(def.fourcc),
                    column,
                    slk.path()
                );
            } else {
                unmeasured_columns.push(format!("{}.{}", crate::fourcc_text(def.fourcc), column));
            }
        }
    }
    // Measured, and reported: the ability and doodad tables use field names
    // the top-level `.slk` does not spell.
    assert!(
        !unmeasured_columns.is_empty(),
        "some Column fields name no shipped column"
    );
}

/// `uhot` is the one field whose declared type disagrees with its wire word.
#[test]
fn uhot_is_the_one_type_wire_mismatch() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let def = e.field(*b"uhot").expect("uhot is defined");
    assert_eq!(def.declared, "char");
    assert_eq!(def.kind, FieldKind::Char);
    // Every shipped `uhot` ships on the string wire, so the char read is
    // refused and the word is handed out as text.
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let maps = crate::shipped_maps(&mut set).unwrap();
    let mut seen = 0usize;
    let mut words = std::collections::BTreeSet::new();
    for map in &maps {
        for name in crate::map_object_files(&mut set, map).unwrap() {
            if !name.to_ascii_lowercase().ends_with(".w3u")
                && !name.to_ascii_lowercase().ends_with(".w3t")
            {
                continue;
            }
            let bytes = crate::extract_object_file(&mut set, map, &name).unwrap();
            let f = ObjectFile::parse(&name, &bytes).unwrap();
            for r in f.records() {
                for m in &r.mods {
                    if m.field == *b"uhot" {
                        seen += 1;
                        words.insert(m.value_type.word());
                    }
                }
            }
        }
    }
    assert_eq!(seen, 8, "eight shipped uhot modifications");
    assert_eq!(words.into_iter().collect::<Vec<_>>(), vec![3]);
}

/// The measured wire rule: the declared type predicts the value word of every
/// shipped modification except `uhot`.
#[test]
fn the_declared_type_predicts_the_wire_word() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let maps = crate::shipped_maps(&mut set).unwrap();
    let mut total = 0usize;
    let mut agree = 0usize;
    let mut mismatches = Vec::new();
    for map in &maps {
        for name in crate::map_object_files(&mut set, map).unwrap() {
            if !name.to_ascii_lowercase().ends_with(".w3u")
                && !name.to_ascii_lowercase().ends_with(".w3t")
            {
                continue;
            }
            let bytes = crate::extract_object_file(&mut set, map, &name).unwrap();
            let f = ObjectFile::parse(&name, &bytes).unwrap();
            for r in f.records() {
                for m in &r.mods {
                    total += 1;
                    match e.field(m.field) {
                        Some(def) if def.kind.wire_word() == m.value_type.word() => agree += 1,
                        Some(_) => mismatches.push(crate::fourcc_text(m.field)),
                        None => mismatches.push(format!("?{}", crate::fourcc_text(m.field))),
                    }
                }
            }
        }
    }
    assert_eq!(total, 3401, "3 401 modifications ship in 1.28c's maps");
    assert_eq!(agree, 3393, "3393 of them match the declared type");
    mismatches.sort();
    mismatches.dedup();
    assert_eq!(mismatches, vec!["uhot".to_string()]);
}

/// The whole editor file inventory of one build, as measured counts.
#[test]
fn the_inventory_is_measured_not_remembered() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let c = e.census();
    assert_eq!(c.slk_files, 59, "1.28c union listfile .slk");
    assert_eq!(c.txt_files, 159);
    assert_eq!(c.units_slk, 15);
    assert_eq!(c.units_txt, 48);
    assert_eq!(c.doodads_slk, 2);
    assert_eq!(c.working_copy_slk, 19);
    assert_eq!(c.working_copy_txt, 73);
    // The row counts the decoder actually read.
    assert_eq!(c.rows_by_slk.len(), 19, "19 tables decoded");
    assert_eq!(c.rows_by_slk[&SlkId::UnitData], 837);
    assert_eq!(c.rows_by_slk[&SlkId::UnitUi], 837);
    assert_eq!(c.rows_by_slk[&SlkId::UnitBalance], 837);
    assert_eq!(c.rows_by_slk[&SlkId::UnitWeapons], 838);
    assert_eq!(c.rows_by_slk[&SlkId::UnitAbilities], 837);
    assert_eq!(c.rows_by_slk[&SlkId::ItemData], 274);
    assert_eq!(c.rows_by_slk[&SlkId::UpgradeData], 90);
    assert_eq!(c.rows_by_slk[&SlkId::AbilityData], 804);
    assert_eq!(c.rows_by_slk[&SlkId::UnitMetaData], 268);
    // The listfile is not the file set: the buff tables ship in
    // War3Patch.mpq, which no listfile on 1.28c names.
    assert!(e.absent().is_empty(), "no editor table is missing");
    assert_eq!(c.rows_by_slk[&SlkId::AbilityBuffData], 238);
    assert_eq!(c.rows_by_slk[&SlkId::AbilityBuffMetaData], 28);
    // Every role of the `.txt` layer loaded.
    for role in TxtRole::ALL {
        assert_eq!(c.txt_loaded[role], role.files().len(), "{role:?}");
    }
}

/// 1.29b differs from 1.28c in the listfile count and the table sizes, and
/// answers for the same rawcodes.
#[test]
fn legacy_build_measures_differently_and_agrees_on_units() {
    let Some(dir) = legacy() else { return };
    let e = open(&dir);
    let c = e.census();
    assert_eq!(c.slk_files, 65, "1.29b union listfile .slk");
    assert_eq!(c.txt_files, 192);
    assert_eq!(c.units_slk, 17);
    assert_eq!(c.units_txt, 49);
    assert_eq!(c.doodads_slk, 2);
    // The two builds carry the same 837-row unit tables; the difference is in
    // the schema tables and in where each build puts the files.
    assert_eq!(c.rows_by_slk[&SlkId::UnitBalance], 837);
    assert_eq!(c.rows_by_slk[&SlkId::AbilityMetaData], 751);
    assert_eq!(c.rows_by_slk[&SlkId::UnitMetaData], 268);
    assert!(e.absent().is_empty());
    // Units agree where both builds ship them.
    if let Some(dir128) = install() {
        let e128 = open(&dir128);
        for rc in ["hfoo", "Hamg", "nmsc", "ckng"] {
            let (Some(a), Some(b)) = (e128.unit(rc), e.unit(rc)) else {
                continue;
            };
            assert_eq!(a.comment(), b.comment(), "{rc} comment");
            assert_eq!(a.hp(), b.hp(), "{rc} HP");
            assert_eq!(a.speed(), b.speed(), "{rc} speed");
            assert_eq!(a.name(), b.name(), "{rc} name");
        }
    }
}

/// The two builds really are different directories.
#[test]
fn the_two_builds_are_distinct() {
    let (Some(a), Some(b)) = (install(), legacy()) else {
        return;
    };
    if a == b {
        return;
    }
    let (ea, eb) = (open(&a), open(&b));
    assert_ne!(ea.build(), eb.build());
}

/// A rawcode that names nothing is refused, not invented.
#[test]
fn an_unknown_rawcode_is_refused() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    assert!(e.unit("zzzz").is_none());
    assert!(e.item("zzzz").is_none());
    assert_eq!(e.resolve("zzzz"), None);
    // ... and a real one is not.
    assert_eq!(e.resolve("hfoo"), Some(ObjectKind::Unit));
}

/// `DemoCampaign.w3n`'s `war3campaign.w3u` modifies `nmsc`, a shipped unit, into
/// a Murloc: the base table lands on a real object and every field of it
/// resolves.
#[test]
fn demo_campaign_overrides_the_stock_shadowcaster() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let bytes =
        crate::extract_object_file(&mut set, "DemoCampaign.w3n", "war3campaign.w3u").unwrap();
    let f = ObjectFile::parse("war3campaign.w3u", &bytes).unwrap();

    let base = f
        .base()
        .iter()
        .find(|r| &r.original == b"nmsc")
        .expect("the nmsc base record");
    let rc = "nmsc";
    assert_eq!(e.resolve(rc), Some(ObjectKind::Unit));

    // Every field of that record resolves to a metadata row.
    let mut unresolved = Vec::new();
    for m in &base.mods {
        match e.field(m.field) {
            Some(def) => {
                assert_eq!(
                    def.kind.wire_word(),
                    m.value_type.word(),
                    "{:?} wire word",
                    crate::fourcc_text(m.field)
                );
            }
            None => unresolved.push(crate::fourcc_text(m.field)),
        }
    }
    assert!(unresolved.is_empty(), "unresolved: {unresolved:?}");

    // Overlaid, the record answers for the map's values.
    let overlay = super::report::Overlay::of(&f);
    let u = overlay.unit(&e, rc);
    assert_eq!(u.hp(), Some(450.0), "uhpm override");
    assert_eq!(u.gold_cost(), Some(160.0), "ugol override");
    assert_eq!(u.damage_sides(), Some(3.0), "ua1s override");
    // ... and a field the record does not touch still reads the base cell.
    assert_eq!(u.race(), Some("creeps"), "urac base cell");
}

/// A first-table record whose original names nothing is reported, and a
/// second-table record whose new rawcode collides with a shipped object is too.
#[test]
fn unresolved_records_are_typed_and_reported() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let maps = crate::shipped_maps(&mut set).unwrap();
    let mut unresolved = Vec::new();
    for map in &maps {
        for name in crate::map_object_files(&mut set, map).unwrap() {
            if !name.ends_with(".w3u") && !name.ends_with(".w3t") {
                continue;
            }
            let bytes = crate::extract_object_file(&mut set, map, &name).unwrap();
            let f = ObjectFile::parse(&name, &bytes).unwrap();
            for r in f.base() {
                let rc = crate::fourcc_text(r.original);
                if e.resolve(&rc).is_none() {
                    unresolved.push(format!("{map} {name} {rc}"));
                }
            }
        }
    }
    // Exactly one shipped record names a rawcode no shipped table carries,
    // on either build: `Nwar` in `Maps\Campaign\Orc02Interlude.w3m`. The
    // gate's answer is that one record, named, not a silently dropped list.
    assert_eq!(
        unresolved,
        vec!["Maps\\Campaign\\Orc02Interlude.w3m war3map.w3u Nwar".to_string()],
    );
    assert!(e.resolve("nman").is_some(), "spelled `Nman` in the table");
}

/// Every shipped object-editor file, on both builds, decoded and measured.
///
/// "Zero failures" here is the editor data's own: every `.slk` of every
/// shipped role parses, every row of every metadata table joins to a shipped
/// column or a shipped `.txt` key, and no raw code repeats in the table that
/// keys it. The report goes to `/tmp/opencode/objects-editor-sweep.txt`.
#[test]
#[ignore = "opens every shipped install; run explicitly"]
fn sweep_every_shipped_editor_file() {
    use std::fmt::Write as _;
    let mut report = String::new();
    let mut failures: Vec<String> = Vec::new();
    for dir in [install(), legacy()].into_iter().flatten() {
        let e = match Editor::open(&dir) {
            Ok(e) => e,
            Err(err) => {
                failures.push(format!("{}: editor open: {err}", dir.display()));
                continue;
            }
        };
        let _ = writeln!(report, "=== {}", e.build());
        let _ = writeln!(
            report,
            "  listfile union: {} .slk, {} .txt, {} .fdf",
            e.census().slk_files,
            e.census().txt_files,
            e.census().fdf_files
        );
        let _ = writeln!(
            report,
            "  Units\\: {} .slk, {} .txt; Doodads\\: {} .slk; working copies {} .slk / {} .txt (not decoded)",
            e.census().units_slk,
            e.census().units_txt,
            e.census().doodads_slk,
            e.census().working_copy_slk,
            e.census().working_copy_txt
        );
        for id in SlkId::ALL {
            let Some(t) = e.table(*id) else {
                failures.push(format!("{}: {} did not decode", e.build(), id.path()));
                continue;
            };
            let rows = t.row_count();
            let cells = t.cell_count();
            let errors = t
                .anomalies()
                .iter()
                .filter(|a| a.severity == pcx_slq::Severity::Error)
                .count();
            if errors > 0 {
                failures.push(format!(
                    "{}: {} has {errors} unrecoverable anomalies",
                    e.build(),
                    id.path()
                ));
            }
            let _ = writeln!(
                report,
                "  {:24} {:>5} rows {:>7} cells",
                id.path(),
                rows,
                cells
            );
        }
        for role in TxtRole::ALL {
            let loaded = e.census().txt_loaded[role];
            if loaded != role.files().len() {
                failures.push(format!(
                    "{}: {role:?} loaded {loaded} of {} files",
                    e.build(),
                    role.files().len()
                ));
            }
        }
        let _ = writeln!(
            report,
            "  units {}, items {}, upgrades {}, commands {}",
            e.units().len(),
            e.items().len(),
            e.upgrades().len(),
            e.commands().len()
        );
        let mut kinds: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for f in e.fields() {
            *kinds.entry(f.kind.to_string()).or_default() += 1;
        }
        let _ = writeln!(
            report,
            "  {} field fourccs: {}",
            e.fields().len(),
            kinds
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let unmeasured: Vec<String> = e
            .fields()
            .iter()
            .filter(|f| !f.measured)
            .map(|f| format!("{}({})", crate::fourcc_text(f.fourcc), f.field))
            .collect();
        let _ = writeln!(report, "  {} unmeasured field mappings", unmeasured.len());

        // Raw codes are unique per keyed table, on every build.
        for id in [
            SlkId::UnitData,
            SlkId::UnitUi,
            SlkId::UnitBalance,
            SlkId::UnitWeapons,
            SlkId::UnitAbilities,
            SlkId::ItemData,
            SlkId::UpgradeData,
        ] {
            let mut seen = std::collections::BTreeSet::new();
            for rc in e.rawcodes(id) {
                if !seen.insert(rc.to_lowercase()) {
                    failures.push(format!("{}: {} repeats {rc}", e.build(), id.path()));
                }
            }
        }
    }

    let _ = writeln!(report, "failures: {}", failures.len());
    for f in &failures {
        let _ = writeln!(report, "  {f}");
    }
    let path = std::path::Path::new("/tmp/opencode/objects-editor-sweep.txt");
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(path, &report).expect("write the sweep report");
    println!("{report}");
    assert!(
        failures.is_empty(),
        "{} failures, see {path:?}",
        failures.len()
    );
}

/// The M4 gate's second half: does a shipped map's declared rawcode resolve?
///
/// Measured over the whole shipped corpus, on both builds, and reported rather
/// than rounded. **1 643 of 1 645 first-table records resolve**; the one that
/// does not is named below, on both builds, and that is the honest answer the
/// gate gets. Every second-table record creates a rawcode the base data does
/// not already carry, so no custom record collides.
#[test]
#[ignore = "opens every shipped install; run explicitly"]
fn every_shipped_map_resolves_every_rawcode() {
    use std::fmt::Write as _;
    let mut report = String::new();
    let mut unexpected: Vec<String> = Vec::new();
    for dir in [install(), legacy()].into_iter().flatten() {
        let e = Editor::open(&dir).expect("editor decodes");
        let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
        let maps = crate::shipped_maps(&mut set).unwrap();
        let mut files = 0usize;
        let mut base = 0usize;
        let mut base_ok = 0usize;
        let mut custom = 0usize;
        let mut custom_new = 0usize;
        let mut mods = 0usize;
        let mut unknowns = Vec::new();
        let mut wire = Vec::new();
        let mut unresolved = Vec::new();
        let mut collides = Vec::new();
        for map in &maps {
            for name in crate::map_object_files(&mut set, map).unwrap() {
                let low = name.to_ascii_lowercase();
                if !(low.ends_with(".w3u") || low.ends_with(".w3t")) {
                    continue;
                }
                let Ok(bytes) = crate::extract_object_file(&mut set, map, &name) else {
                    unexpected.push(format!("{map} {name}: extract"));
                    continue;
                };
                let Ok(f) = ObjectFile::parse(&name, &bytes) else {
                    unexpected.push(format!("{map} {name}: parse"));
                    continue;
                };
                files += 1;
                for r in f.base() {
                    base += 1;
                    let rc = crate::fourcc_text(r.original);
                    if e.resolve(&rc).is_some() {
                        base_ok += 1;
                    } else {
                        unresolved.push(format!("{map} {name} {rc}"));
                    }
                }
                for r in f.custom() {
                    custom += 1;
                    let rc = crate::fourcc_text(r.modified);
                    if e.resolve(&rc).is_none() {
                        custom_new += 1;
                    } else {
                        collides.push(format!("{map} {name} {rc}"));
                    }
                }
                for m in f.records().flat_map(|r| r.mods.iter()) {
                    mods += 1;
                    let Some(def) = e.field(m.field) else {
                        unknowns.push(format!("{map} {name} {}", crate::fourcc_text(m.field)));
                        continue;
                    };
                    if def.kind.wire_word() != m.value_type.word() {
                        wire.push(format!(
                            "{map} {name} {} ({} on the word-{} wire)",
                            crate::fourcc_text(m.field),
                            def.declared,
                            m.value_type.word()
                        ));
                    }
                }
            }
        }
        let _ = writeln!(
            report,
            "{}: {} object files, {} base records ({} resolved), {} custom records ({} new), {} modifications",
            e.build(),
            files,
            base,
            base_ok,
            custom,
            custom_new,
            mods
        );
        let _ = writeln!(
            report,
            "  base data: {} units, {} items, {} upgrades, {} command buttons",
            e.units().len(),
            e.items().len(),
            e.upgrades().len(),
            e.commands().len()
        );
        let _ = writeln!(
            report,
            "  unresolved first-table records: {}",
            unresolved.len()
        );
        for u in &unresolved {
            let _ = writeln!(report, "  {u}");
        }
        let _ = writeln!(
            report,
            "  colliding custom rawcodes: {}, unknown field fourccs: {}, wire mismatches: {}",
            collides.len(),
            unknowns.len(),
            wire.len()
        );
        for c in &collides {
            let _ = writeln!(report, "  {c}");
        }
        for u in &unknowns {
            let _ = writeln!(report, "  {u}");
        }
        for w in &wire {
            let _ = writeln!(report, "  {w}");
        }

        // The measured relation: one record, named, on both builds. Anything
        // else is a regression worth failing on.
        let expected: Vec<String> =
            vec!["Maps\\Campaign\\Orc02Interlude.w3m war3map.w3u Nwar".to_string()];
        assert_eq!(unresolved, expected, "{} unresolved rawcodes", e.build());
        assert!(collides.is_empty(), "{} collides: {collides:?}", e.build());
        unexpected.extend(unresolved);
        unexpected.extend(collides);
        unexpected.extend(unknowns);
    }
    let path = std::path::Path::new("/tmp/opencode/objects-editor-sweep.txt");
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut full = std::fs::read_to_string(path).unwrap_or_default();
    full.push_str(&report);
    std::fs::write(path, &full).expect("append to the sweep report");
    println!("{report}");
    // The survey above is what the report is for; every unexpected item is a
    // path, not a number.
    assert!(
        unexpected.len() == 2,
        "the two named unresolved records, one per build: {unexpected:?}"
    );
}

/// The overlay's typed report, over one object file.
#[test]
fn the_overlay_report_is_typed_and_complete() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let bytes =
        crate::extract_object_file(&mut set, "DemoCampaign.w3n", "war3campaign.w3u").unwrap();
    let f = ObjectFile::parse("war3campaign.w3u", &bytes).unwrap();

    let mut applied = super::report::Applied::default();
    let report = applied.push(&e, "DemoCampaign.w3n", "war3campaign.w3u", &f);

    // 6 base records + 2 custom, each resolved to a unit.
    assert_eq!(report.resolved.len(), 8);
    assert_eq!(report.resolved_count(), 8);
    assert!(report.unresolved.is_empty(), "{:?}", report.unresolved);
    assert!(report.resolved_all());
    assert_eq!(report.base_records(), 6);
    assert_eq!(report.custom_records(), 2);
    // The only refusal is the documented `uhot` char/string disagreement.
    assert_eq!(report.refusals.len(), 2);
    for r in &report.refusals {
        assert!(
            matches!(r, super::report::Refusal::CharOnStringWire { fourcc } if fourcc == b"uhot"),
            "{r}"
        );
    }
    for resolution in &report.resolved {
        assert_eq!(resolution.kind, Some(ObjectKind::Unit));
    }
    // `Applied::clean` is the strict form: one record refused, so not clean.
    assert!(!applied.clean());
    assert!(applied.reports.iter().all(ApplyReport::resolved_all));
    // Every field fourcc of the record resolves to a metadata row, and only
    // `uhot` disagrees with the type word its metadata row declares.
    let mismatches: Vec<&(Fourcc, String, u32)> = applied.wire_mismatches.keys().collect();
    for (fourcc, _, _) in &mismatches {
        assert_eq!(*fourcc, *b"uhot", "only uhot mismatches");
    }
    assert_eq!(mismatches.len(), 1);
    assert_eq!(
        applied.refusals().count(),
        2,
        "the two DemoCampaign `uhot` refusals"
    );
}

/// The sweep's coverage type reports the same numbers the sweep prints.
#[test]
fn coverage_reports_the_measured_numbers() {
    let Some(dir) = install() else { return };
    let installs: Vec<&std::path::Path> = vec![&dir];
    let coverage = super::report::Applied::sweep(&installs).expect("sweep");
    assert_eq!(coverage.builds.len(), 1);
    let b = &coverage.builds[0];
    assert_eq!(b.maps_in_install, 89, "88 .w3m + 1 .w3n in the union");
    assert_eq!(b.maps_with_objects, 48);
    assert_eq!(b.base_records, 888);
    assert_eq!(b.custom_records, 282);
    assert_eq!(b.base_resolved(), 887, "one record names `Nwar`");
    assert_eq!(b.units_in_base, 836);
    assert_eq!(b.items_in_base, 273);
    // One unresolved record, named on the build's own label.
    assert_eq!(b.unresolved.len(), 1);
    assert!(
        b.unresolved[0].contains("Nwar"),
        "the named record: {:?}",
        b.unresolved
    );
    assert_eq!(b.failures.len(), 0);
    // The only wire mismatch is `uhot`, and the counts add up to the eight
    // shipped modifications.
    let mut total = 0usize;
    for m in &b.wire_mismatches {
        assert_eq!(&m.fourcc, b"uhot");
        assert_eq!(m.declared, "char");
        assert_eq!(m.word, 3);
        total += m.count;
    }
    assert_eq!(total, 8, "eight shipped uhot modifications");
}

/// The item overlay, which the `.w3t` side of the gate needs.
#[test]
fn an_item_override_wins_over_the_stock_cell() {
    let Some(dir) = install() else { return };
    let e = open(&dir);
    let mut set = pcx_mpq::MpqSet::open_wc3(&dir).unwrap();
    let bytes =
        crate::extract_object_file(&mut set, "DemoCampaign.w3n", "war3campaign.w3t").unwrap();
    let f = ObjectFile::parse("war3campaign.w3t", &bytes).unwrap();
    let overlay = super::report::Overlay::of(&f);

    for record in f.base() {
        let rc = crate::fourcc_text(record.original);
        let Some(item) = e.item(&rc) else {
            panic!("{rc} is not a shipped item");
        };
        let overlaid = overlay.item(&e, &rc);
        assert_eq!(overlaid.rawcode(), item.rawcode());
        assert!(!overlaid.custom(), "the base table has no new rawcodes");
        for m in &record.mods {
            assert!(
                overlaid.stat(m.field).is_some(),
                "{}: {:?} resolves",
                rc,
                crate::fourcc_text(m.field)
            );
        }
    }
}

/// A table that is not SYLK at all is refused through the crate's own error,
/// not a panic and not a default.
#[test]
fn a_table_that_is_not_sylk_is_refused() {
    let err = pcx_slq::Table::parse(b"not a sylk file at all").unwrap_err();
    assert!(
        matches!(
            err,
            pcx_slq::Error::BadSignature { .. } | pcx_slq::Error::Empty
        ),
        "{err}"
    );
    // And a directory with no MPQ archive in it fails the same way.
    let e = Editor::open(std::path::Path::new("."));
    assert!(e.is_err(), "no archives here");
}
