use super::*;
use crate::hero_skills;
use crate::playercolor_host::HeroBudget;
use pcx_objects::editor::Overlay;

const NATIVES: &str = "native SelectHeroSkill takes unit u, integer ability returns nothing\nconstant native GetHeroLevel takes unit u returns integer\nconstant native GetHeroSkillPoints takes unit u returns integer\nconstant native GetUnitAbilityLevel takes unit u, integer ability returns integer\n";
const MAP: &str = "globals\nunit H=null\nunit N=null\nendglobals\nfunction setup takes nothing returns nothing\nset H=CreateUnit(Player(4),'H001',1.0,2.0,3.0)\nset N=CreateUnit(Player(4),'h001',4.0,5.0,6.0)\nendfunction\nfunction learn takes integer ability returns nothing\ncall SelectHeroSkill(H,ability)\nendfunction\n";
struct Catalog {
    list: String,
}
impl ObjectCatalog for Catalog {
    fn resolve(&self, code: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        let mut fields = BTreeMap::new();
        fields.insert(
            *b"uabi",
            ability_field("abilList", if code == *b"hypc" { "Yher" } else { "L000" }),
        );
        let mut heroes = ability_field("heroAbilList", &self.list);
        heroes.metadata.as_mut().unwrap().fourcc = *b"uhab";
        if code == *b"Hbad" {
            heroes.metadata.as_mut().unwrap().kind = FieldKind::Int;
        }
        if code == *b"Hidx" {
            heroes.metadata.as_mut().unwrap().index = 0;
        }
        fields.insert(*b"uhab", heroes);
        let mut level = ability_field("level", "7");
        let metadata = level.metadata.as_mut().unwrap();
        metadata.fourcc = *b"ulev";
        metadata.slk = "UnitBalance".into();
        metadata.kind = FieldKind::Int;
        metadata.source = FieldSource::Column {
            slk: SlkId::UnitBalance,
            column: "level".into(),
        };
        fields.insert(*b"ulev", level);
        Ok(definition(
            code,
            pcx_objects::fourcc_text(code),
            ObjectKind::Unit,
            fields,
        ))
    }
}
fn tables(profile: &str) -> DefaultTables {
    tables_with_data_ordinal(profile, "1")
}
fn tables_with_data_ordinal(profile: &str, data_ordinal: &str) -> DefaultTables {
    tables_with(profile, "1", data_ordinal)
}
fn tables_with_tooltip_repeat(profile: &str, repeat: &str) -> DefaultTables {
    tables_with(profile, repeat, "1")
}
fn tables_with(profile: &str, repeat: &str, data_ordinal: &str) -> DefaultTables {
    let data = sylk(
        &[
            "alias",
            "code",
            "levels",
            "reqLevel",
            "levelSkip",
            "DataA1",
            "DataA2",
            "UnitID1",
            "UnitID2",
            "DataB1",
            "DataB2",
        ],
        &[
            &["L000", "L000", "1", "0", "0"],
            &[
                "L001", "L001", "3", "1", "0", "2", "4", "u001", "u002", "7", "8",
            ],
            &["L002", "L002", "1", "4", "3"],
            &["Yher", "Yher", "1", "0", "0"],
        ],
    );
    let meta = sylk(
        &[
            "ID", "field", "slk", "index", "type", "category", "repeat", "data",
        ],
        &[
            &[
                "adat",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "1",
                data_ordinal,
            ],
            &[
                "auni",
                "UnitID",
                "AbilityData",
                "-1",
                "unitCode",
                "stats",
                "1",
                "0",
            ],
            &[
                "alev",
                "levels",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "0",
                "0",
            ],
            &[
                "arlv",
                "reqLevel",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "0",
                "0",
            ],
            &[
                "alsk",
                "levelSkip",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "0",
                "0",
            ],
            &["ahky", "Hotkey", "Profile", "0", "char", "text", "0", "0"],
            &["atip", "Tip", "Profile", "0", "string", "text", repeat, "0"],
            &[
                "aubt", "UberTip", "Profile", "0", "string", "text", repeat, "0",
            ],
            &["aart", "Art", "Profile", "0", "icon", "art", "0", "0"],
            &["abpx", "Buttonpos", "Profile", "0", "int", "art", "0", "0"],
            &["abpy", "Buttonpos", "Profile", "1", "int", "art", "0", "0"],
        ],
    );
    DefaultTables::from_bytes(
        Identity {
            install: source().install,
            edition_version: "fixture".into(),
        },
        &[
            (SlkId::AbilityData.path(), data.as_slice()),
            (SlkId::AbilityMetaData.path(), meta.as_slice()),
        ],
        &[("Units\\SyntheticAbilityFunc.txt", profile.as_bytes())],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 1 << 16,
            max_aggregate_input_bytes: 1 << 18,
        },
    )
    .unwrap()
}
fn catalog_origin() -> crate::scripts::AssetIdentity {
    crate::scripts::AssetIdentity {
        source: source().install,
        edition: "fixture".into(),
        version: "fixture".into(),
    }
}

#[test]
fn learned_tooltips_select_the_authored_rank_and_expand_its_numeric_cells() {
    let mut vm = host(
        NATIVES,
        "L001",
        Overlay::default(),
        "[L001]\nTip=First,Second,Third\nUberTip=\"Heals <L001,DataA1>, now.\",\"Heals <L001,DataA2>.\",Third\n",
        8,
    );
    vm.call("setup", &[]).unwrap();
    let Value::Handle(unit) = vm.global("H") else {
        panic!("hero")
    };
    assert!(
        vm.host()
            .inner
            .ability_tooltip(unit, *b"L001")
            .unwrap()
            .is_none()
    );
    learn(&mut vm, *b"L001");
    let tip = vm
        .host()
        .inner
        .ability_tooltip(unit, *b"L001")
        .unwrap()
        .unwrap();
    assert_eq!(
        (tip.title.as_str(), tip.description.as_str()),
        ("First", "Heals 2, now.")
    );
    raise(&mut vm, 3);
    learn(&mut vm, *b"L001");
    let tip = vm
        .host()
        .inner
        .ability_tooltip(unit, *b"L001")
        .unwrap()
        .unwrap();
    assert_eq!(
        (tip.title.as_str(), tip.description.as_str()),
        ("Second", "Heals 4.")
    );
}

#[test]
fn tooltip_data_expansion_requires_the_metadata_data_pointer() {
    let tooltip = "[L001]\nTip=First,Second,Third\nUberTip=\"Heals <L001,DataA1>, now.\",\"Heals <L001,DataA2>.\",Third\n";
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![pcx_objects::Mod {
            field: *b"adat",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([2, 1]),
            value: pcx_objects::Value::Int(9),
            end_word: 0,
        }],
    );
    let mut vm = host(NATIVES, "L001", overlay, tooltip, 8);
    vm.call("setup", &[]).unwrap();
    let Value::Handle(unit) = vm.global("H") else {
        panic!("hero")
    };
    learn(&mut vm, *b"L001");
    raise(&mut vm, 3);
    learn(&mut vm, *b"L001");
    let tip = vm
        .host()
        .inner
        .ability_tooltip(unit, *b"L001")
        .unwrap()
        .unwrap();
    assert_eq!(tip.description, "Heals 9.");

    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![pcx_objects::Mod {
            field: *b"adat",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([2, 0]),
            value: pcx_objects::Value::Int(9),
            end_word: 0,
        }],
    );
    let mut vm = host(NATIVES, "L001", overlay, tooltip, 8);
    vm.call("setup", &[]).unwrap();
    let Value::Handle(unit) = vm.global("H") else {
        panic!("hero")
    };
    learn(&mut vm, *b"L001");
    raise(&mut vm, 3);
    learn(&mut vm, *b"L001");
    let error = vm
        .host()
        .inner
        .ability_tooltip(unit, *b"L001")
        .unwrap_err()
        .to_string();
    assert!(error.contains("data pointer 0 does not match metadata data column 1"));
}

#[test]
fn ranked_tooltip_map_edits_select_only_their_level_and_do_not_fall_back() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![pcx_objects::Mod {
            field: *b"atip",
            value_type: pcx_objects::ValueType::Str,
            selectors: Some([2, 0]),
            value: pcx_objects::Value::Str(b"Map rank two".to_vec()),
            end_word: 0,
        }],
    );
    let source = tables("[L001]\nTip=First,Second,Third\n");
    assert_eq!(
        crate::ability_profile::ranked(&source, &overlay, *b"L001", "Tip", 1, 3).unwrap(),
        "First"
    );
    assert_eq!(
        crate::ability_profile::ranked(&source, &overlay, *b"L001", "Tip", 2, 3).unwrap(),
        "Map rank two"
    );
    assert!(crate::ability_profile::ranked(&source, &overlay, *b"L001", "Tip", 4, 3).is_err());
    assert!(crate::ability_profile::ranked(&source, &overlay, *b"L001", "UberTip", 1, 3).is_err());
    let malformed = tables_with_tooltip_repeat("[L001]\nTip=First,Second,Third\n", "bad");
    assert!(
        crate::ability_profile::ranked(&malformed, &overlay, *b"L001", "Tip", 2, 3)
            .unwrap_err()
            .contains("repeat metadata")
    );
    overlay.mods.get_mut("L001").unwrap()[0].value = pcx_objects::Value::Str(Vec::new());
    assert_eq!(
        crate::ability_profile::ranked(&source, &overlay, *b"L001", "Tip", 2, 3).unwrap(),
        ""
    );
}

#[test]
fn button_positions_resolve_each_metadata_slot_and_map_override_independently() {
    let source_tables = tables("[L001]\nButtonpos=2,1\n");
    let mut overlay = Overlay::default();
    assert_eq!(
        crate::ability_profile::button_position(&source_tables, &overlay, *b"L001").unwrap(),
        [2, 1]
    );
    overlay.mods.insert(
        "L001".into(),
        vec![pcx_objects::Mod {
            field: *b"abpy",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([0, 0]),
            value: pcx_objects::Value::Int(3),
            end_word: 0,
        }],
    );
    assert_eq!(
        crate::ability_profile::button_position(&source_tables, &overlay, *b"L001").unwrap(),
        [2, 3]
    );
    let empty = tables("");
    overlay
        .mods
        .get_mut("L001")
        .unwrap()
        .push(pcx_objects::Mod {
            field: *b"abpx",
            value_type: pcx_objects::ValueType::Int,
            selectors: None,
            value: pcx_objects::Value::Int(4),
            end_word: 0,
        });
    assert_eq!(
        crate::ability_profile::button_position(&empty, &overlay, *b"L001").unwrap(),
        [4, 3]
    );
    for invalid in [u32::MAX, i32::MIN as u32] {
        overlay.mods.get_mut("L001").unwrap()[0].value = pcx_objects::Value::Int(invalid);
        assert!(
            crate::ability_profile::button_position(&source_tables, &overlay, *b"L001").is_err()
        );
    }
}

#[test]
fn button_positions_refuse_malformed_stock_or_map_coordinates() {
    for profile in [
        "[L001]\nButtonpos=2\n",
        "[L001]\nButtonpos=-1,0\n",
        "[L001]\nButtonpos=x,1\n",
    ] {
        assert!(
            crate::ability_profile::button_position(
                &tables(profile),
                &Overlay::default(),
                *b"L001"
            )
            .is_err()
        );
    }
    for (selectors, value_type, value) in [
        (
            Some([1, 0]),
            pcx_objects::ValueType::Int,
            pcx_objects::Value::Int(3),
        ),
        (
            None,
            pcx_objects::ValueType::Str,
            pcx_objects::Value::Str(b"3".to_vec()),
        ),
        (
            None,
            pcx_objects::ValueType::Int,
            pcx_objects::Value::Str(b"3".to_vec()),
        ),
    ] {
        let mut overlay = Overlay::default();
        overlay.mods.insert(
            "L001".into(),
            vec![pcx_objects::Mod {
                field: *b"abpx",
                selectors,
                value_type,
                value,
                end_word: 0,
            }],
        );
        assert!(
            crate::ability_profile::button_position(
                &tables("[L001]\nButtonpos=2,1\n"),
                &overlay,
                *b"L001"
            )
            .is_err()
        );
    }
}

#[test]
fn ability_buttons_follow_learned_rank_and_authored_art() {
    let mut vm = host(
        NATIVES,
        "L001",
        Overlay::default(),
        "[L001]\nArt=Textures\\Authored.blp\nButtonpos=2,1\n",
        8,
    );
    vm.call("setup", &[]).unwrap();
    let Value::Handle(unit) = vm.global("H") else {
        panic!("created hero")
    };
    assert!(
        vm.host()
            .inner
            .ability_button(unit, *b"L001")
            .unwrap()
            .is_none()
    );
    vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"L001"))])
        .unwrap();
    let button = vm
        .host()
        .inner
        .ability_button(unit, *b"L001")
        .unwrap()
        .unwrap();
    assert_eq!(button.rank, 1);
    assert_eq!(button.art, "Textures\\Authored.blp");
    assert_eq!(button.position, [2, 1]);
}

#[test]
fn scalar_ability_profiles_use_map_overrides_and_keep_unrelated_stock_values() {
    let modification = |field, text: &[u8]| pcx_objects::Mod {
        field,
        value_type: pcx_objects::ValueType::Str,
        selectors: Some([0, 0]),
        value: pcx_objects::Value::Str(text.to_vec()),
        end_word: 0,
    };
    let profile = "[L001]\nHotkey=Q\nArt=Textures\\Stock.blp\nOrder=synthetic\n";
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![
            modification(*b"ahky", b"T"),
            modification(*b"ahky", b" W "),
            modification(*b"aart", b"Textures\\Map.blp"),
        ],
    );
    let mut vm = host(NATIVES, "L001", overlay, profile, 8);
    let h = &vm.host().inner;
    assert_eq!(h.hero_ability_profile(*b"L001", "Hotkey").unwrap(), "W");
    assert_eq!(
        h.hero_ability_profile(*b"L001", "Art").unwrap(),
        "Textures\\Map.blp"
    );
    assert_eq!(
        h.hero_ability_profile(*b"L001", "Order").unwrap(),
        "synthetic"
    );
}

#[test]
fn malformed_or_indexed_profile_overrides_never_fall_back_to_stock() {
    let profile = "[L001]\nHotkey=Q\nButtonpos=1,2\n";
    for (field, selectors, value_type, value, key) in [
        (
            *b"ahky",
            Some([1, 0]),
            pcx_objects::ValueType::Str,
            pcx_objects::Value::Str(b"T".to_vec()),
            "Hotkey",
        ),
        (
            *b"ahky",
            None,
            pcx_objects::ValueType::Int,
            pcx_objects::Value::Int(5),
            "Hotkey",
        ),
        (
            *b"ahky",
            None,
            pcx_objects::ValueType::Str,
            pcx_objects::Value::Str(vec![255]),
            "Hotkey",
        ),
        (
            *b"abpx",
            None,
            pcx_objects::ValueType::Int,
            pcx_objects::Value::Int(3),
            "Buttonpos",
        ),
        (
            *b"xxxx",
            None,
            pcx_objects::ValueType::Str,
            pcx_objects::Value::Str(b"T".to_vec()),
            "Hotkey",
        ),
    ] {
        let mut overlay = Overlay::default();
        overlay.mods.insert(
            "L001".into(),
            vec![pcx_objects::Mod {
                field,
                selectors,
                value_type,
                value,
                end_word: 0,
            }],
        );
        let mut vm = host(NATIVES, "L001", overlay, profile, 8);
        assert!(vm.host().inner.hero_ability_profile(*b"L001", key).is_err());
    }
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![pcx_objects::Mod {
            field: *b"ahky",
            selectors: None,
            value_type: pcx_objects::ValueType::Str,
            value: pcx_objects::Value::Str(Vec::new()),
            end_word: 0,
        }],
    );
    let mut vm = host(NATIVES, "L001", overlay, profile, 8);
    assert!(
        vm.host()
            .inner
            .hero_ability_profiles(*b"L001", "Hotkey")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn ranked_ability_cells_use_metadata_overrides_and_keep_other_ranks() {
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![
            pcx_objects::Mod {
                field: *b"adat",
                value_type: pcx_objects::ValueType::Int,
                selectors: Some([2, 1]),
                value: pcx_objects::Value::Int(9),
                end_word: 0,
            },
            pcx_objects::Mod {
                field: *b"auni",
                value_type: pcx_objects::ValueType::Str,
                selectors: Some([2, 0]),
                value: pcx_objects::Value::Str(b"u003".to_vec()),
                end_word: 0,
            },
        ],
    );
    let mut vm = host(NATIVES, "L001", overlay, "", 8);
    let h = &vm.host().inner;
    assert_eq!(
        h.hero_ability_level_cell(*b"L001", "DataA", 1, FieldKind::Int)
            .unwrap(),
        "2"
    );
    assert_eq!(
        h.hero_ability_level_cell(*b"L001", "DataA", 2, FieldKind::Int)
            .unwrap(),
        "9"
    );
    assert_eq!(
        h.hero_ability_level_cell(*b"L001", "UnitID", 1, FieldKind::Token)
            .unwrap(),
        "u001"
    );
    assert_eq!(
        h.hero_ability_level_cell(*b"L001", "UnitID", 2, FieldKind::Token)
            .unwrap(),
        "u003"
    );
    assert!(
        h.hero_ability_level_cell(*b"L001", "UnitID", 2, FieldKind::Int)
            .is_err()
    );
    assert!(
        h.hero_ability_level_cell(*b"L001", "UnitID", 0, FieldKind::Token)
            .is_err()
    );
}

#[test]
fn ranked_ability_cells_follow_nonstock_metadata_data_and_refuse_mismatches() {
    let data_mod = |pointer, value_type, value| pcx_objects::Mod {
        field: *b"adat",
        value_type,
        selectors: Some([2, pointer]),
        value,
        end_word: 0,
    };
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![data_mod(
            2,
            pcx_objects::ValueType::Int,
            pcx_objects::Value::Int(9),
        )],
    );
    let mut vm = host_with_tables(
        NATIVES,
        "L001",
        overlay,
        "",
        8,
        tables_with_data_ordinal("", "2"),
    );
    let h = &vm.host().inner;
    assert_eq!(
        h.hero_ability_level_cell(*b"L001", "DataB", 2, FieldKind::Int)
            .unwrap(),
        "9",
        "a non-stock ordinal maps Data to DataB"
    );
    assert_eq!(
        h.hero_ability_level_cell(*b"L001", "DataB", 1, FieldKind::Int)
            .unwrap(),
        "7"
    );
    assert_eq!(
        h.hero_ability_level_cell(*b"L001", "DataA", 2, FieldKind::Int)
            .unwrap(),
        "4",
        "the decoy DataA column is not the metadata Data column"
    );

    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![data_mod(
            1,
            pcx_objects::ValueType::Int,
            pcx_objects::Value::Int(9),
        )],
    );
    let mut vm = host_with_tables(
        NATIVES,
        "L001",
        overlay,
        "",
        8,
        tables_with_data_ordinal("", "2"),
    );
    let error = vm
        .host()
        .inner
        .hero_ability_level_cell(*b"L001", "DataB", 2, FieldKind::Int)
        .unwrap_err();
    assert!(error.contains("data pointer 1 does not match metadata data column 2"));

    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![data_mod(
            2,
            pcx_objects::ValueType::Str,
            pcx_objects::Value::Str(b"9".to_vec()),
        )],
    );
    let mut vm = host_with_tables(
        NATIVES,
        "L001",
        overlay,
        "",
        8,
        tables_with_data_ordinal("", "2"),
    );
    assert!(
        vm.host()
            .inner
            .hero_ability_level_cell(*b"L001", "DataB", 2, FieldKind::Int)
            .unwrap_err()
            .contains("wrong type")
    );

    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![
            data_mod(2, pcx_objects::ValueType::Int, pcx_objects::Value::Int(5)),
            pcx_objects::Mod {
                field: *b"atip",
                value_type: pcx_objects::ValueType::Str,
                selectors: Some([2, 0]),
                value: pcx_objects::Value::Str(b"profile source".to_vec()),
                end_word: 0,
            },
            data_mod(2, pcx_objects::ValueType::Int, pcx_objects::Value::Int(9)),
        ],
    );
    let mut vm = host_with_tables(
        NATIVES,
        "L001",
        overlay,
        "",
        8,
        tables_with_data_ordinal("", "2"),
    );
    let h = &vm.host().inner;
    assert_eq!(
        h.hero_ability_level_cell(*b"L001", "DataB", 2, FieldKind::Int)
            .unwrap(),
        "9",
        "the latest matching Data override wins"
    );
}

#[test]
fn ranked_ability_cells_refuse_unknown_fields_and_unsupported_data_selectors() {
    for (field, selectors) in [(*b"adat", Some([2, 0])), (*b"ZZZZ", Some([2, 0]))] {
        let mut overlay = Overlay::default();
        overlay.mods.insert(
            "L001".into(),
            vec![pcx_objects::Mod {
                field,
                selectors,
                value_type: pcx_objects::ValueType::Int,
                value: pcx_objects::Value::Int(9),
                end_word: 0,
            }],
        );
        let mut vm = host(NATIVES, "L001", overlay, "", 8);
        assert!(
            vm.host()
                .inner
                .hero_ability_level_cell(*b"L001", "DataA", 2, FieldKind::Int)
                .is_err()
        );
    }
}
fn host(
    extra: &str,
    list: &str,
    overlay: Overlay,
    profile: &str,
    abilities: usize,
) -> pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>> {
    host_with_tables(extra, list, overlay, profile, abilities, tables(profile))
}
fn host_with_tables(
    extra: &str,
    list: &str,
    overlay: Overlay,
    _profile: &str,
    abilities: usize,
    tables: DefaultTables,
) -> pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>> {
    let common = script_common_with("widget", true, extra);
    let map = if extra.contains("SelectHeroSkill") {
        MAP
    } else {
        MAP.split("function learn").next().unwrap()
    };
    let ctx = context(300, &common, map, "unit-catalog");
    let mut rules = hero_skills::tests::rules();
    if extra.contains("GetHeroXP") {
        rules.experience = Some(Arc::new(crate::hero_experience::tests::rules()));
    }
    let h = object(&ctx, 64)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::new(Catalog { list: list.into() }), source(), limits(4))
        .unwrap()
        .with_hero_skills(
            tables,
            catalog_origin(),
            overlay,
            rules,
            HeroBudget {
                abilities_per_unit: abilities,
                overlay_objects: 8,
                modifications_per_ability: 8,
            },
        )
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    vm
}
#[test]
fn hero_xp_thresholds_positive_set_and_suspension_share_real_level_state() {
    let natives = format!(
        "{NATIVES}\nnative GetHeroXP takes unit u returns integer\nnative SetHeroXP takes unit u, integer xp, boolean eye returns nothing\nnative AddHeroXP takes unit u, integer xp, boolean eye returns nothing\nnative SuspendHeroXP takes unit u, boolean flag returns nothing\nnative IsSuspendedXP takes unit u returns boolean\n"
    );
    let mut vm = host(&natives, "L001", Overlay::default(), "", 8);
    vm.call("setup", &[]).unwrap();
    let hero = vm.global("H");
    let ordinary = vm.global("N");
    assert_eq!(query(&mut vm, "GetHeroXP", None), 0);
    assert_eq!(
        vm.host().inner.native("GetHeroXP", &[ordinary]).unwrap(),
        Value::Int(0)
    );
    let call = |vm: &mut pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>,
                name: &str,
                value: i32| {
        vm.host()
            .inner
            .native(name, &[hero.clone(), Value::Int(value), Value::Bool(false)])
            .unwrap();
    };
    call(&mut vm, "SetHeroXP", 17);
    assert_eq!(query(&mut vm, "GetHeroLevel", None), 2);
    call(&mut vm, "SetHeroXP", 16);
    assert_eq!(query(&mut vm, "GetHeroXP", None), 17);
    call(&mut vm, "AddHeroXP", 21);
    assert_eq!(query(&mut vm, "GetHeroXP", None), 38);
    assert_eq!(query(&mut vm, "GetHeroLevel", None), 3);
    vm.host()
        .inner
        .native("SuspendHeroXP", &[hero.clone(), Value::Bool(true)])
        .unwrap();
    call(&mut vm, "AddHeroXP", 7);
    assert_eq!(query(&mut vm, "GetHeroXP", None), 38);
    vm.host()
        .inner
        .native("SuspendHeroXP", &[hero.clone(), Value::Bool(false)])
        .unwrap();
    call(&mut vm, "AddHeroXP", 1);
    assert_eq!(query(&mut vm, "GetHeroXP", None), 39);
    for args in [
        vec![],
        vec![hero.clone(), Value::Bool(true), Value::Bool(false)],
        vec![hero.clone(), Value::Int(i32::MAX), Value::Bool(false)],
    ] {
        assert!(vm.host().inner.native("AddHeroXP", &args).is_err());
        assert_eq!(query(&mut vm, "GetHeroXP", None), 39);
    }
    raise(&mut vm, 4);
    assert_eq!(query(&mut vm, "GetHeroXP", None), 66);
}

fn query(
    vm: &mut pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>,
    name: &str,
    ability: Option<[u8; 4]>,
) -> i32 {
    let mut args = vec![vm.global("H")];
    if let Some(a) = ability {
        args.push(Value::Int(i32::from_be_bytes(a)))
    }
    let Value::Int(n) = vm.host().inner.native(name, &args).unwrap() else {
        panic!("integer query")
    };
    n
}

#[test]
fn implicit_aliases_do_not_fall_through_to_an_unrelated_ability_or_unit_class_level() {
    let mut vm = host(NATIVES, "L001,L002", Overlay::default(), "", 16);
    vm.call("setup", &[]).unwrap();
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"Yher")), 1);
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"Yxyz")), 0);
    let ordinary = vm.global("N");
    assert_eq!(
        vm.host()
            .inner
            .native(
                "GetUnitAbilityLevel",
                &[ordinary.clone(), Value::Int(i32::from_be_bytes(*b"Yher"))]
            )
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetHeroLevel", std::slice::from_ref(&ordinary))
            .unwrap(),
        Value::Int(7)
    );
    let hero = vm.global("H");
    for unit in [&hero, &ordinary] {
        assert!(
            vm.host()
                .inner
                .native(
                    "GetUnitAbilityLevel",
                    &[unit.clone(), Value::Int(i32::from_be_bytes(*b"Yimp"))]
                )
                .unwrap_err()
                .to_string()
                .contains("implicit ability component")
        );
    }
    let owner = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let explicit_component = vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*b"hypc")),
                Value::Real(1.0),
                Value::Real(2.0),
                Value::Real(3.0),
            ],
        )
        .unwrap();
    assert!(
        vm.host()
            .inner
            .native("GetHeroLevel", &[explicit_component])
            .unwrap_err()
            .to_string()
            .contains("explicit hero component")
    );
}
fn learn(
    vm: &mut pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>,
    code: [u8; 4],
) {
    vm.call("learn", &[Value::Int(i32::from_be_bytes(code))])
        .unwrap();
}
fn raise(vm: &mut pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>, level: i32) {
    let hero = vm.global("H");
    vm.host()
        .inner
        .native(
            "SetHeroLevel",
            &[hero, Value::Int(level), Value::Bool(false)],
        )
        .unwrap();
}

#[test]
fn hero_learning_updates_the_same_handle_and_respects_levels_points_and_caps() {
    let mut vm = host(NATIVES, "L001,L002", Overlay::default(), "", 8);
    vm.call("setup", &[]).unwrap();
    let hero = vm.global("H");
    let handles = vm.host().inner.inner().handles();
    assert_eq!(query(&mut vm, "GetHeroLevel", None), 1); // not the class rating seven
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), 3); // synthetic source counter
    let Value::Handle(hero_id) = hero.clone() else {
        panic!("hero handle: {hero:?}");
    };
    let listed = vm.host().inner.learnable_skills(hero_id).unwrap();
    assert!(
        listed
            .iter()
            .any(|skill| { skill.alias == *b"L001" && skill.block.is_none() && skill.rank == 0 })
    );
    assert!(listed.iter().any(|skill| {
        skill.alias == *b"L002"
            && matches!(
                skill.block,
                Some(crate::playercolor_host::LearnBlock::HeroLevel { .. })
            )
    }));
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L000")), 1);
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L001")), 0);
    learn(&mut vm, *b"L001");
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L001")), 1);
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), 2);
    learn(&mut vm, *b"L001");
    learn(&mut vm, *b"L002");
    learn(&mut vm, *b"none");
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), 2);
    raise(&mut vm, 3);
    assert_eq!(query(&mut vm, "GetHeroLevel", None), 3);
    learn(&mut vm, *b"L001");
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L001")), 2);
    raise(&mut vm, 99);
    assert_eq!(query(&mut vm, "GetHeroLevel", None), 11);
    learn(&mut vm, *b"L001");
    learn(&mut vm, *b"L002");
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), 0);
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L001")), 3);
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L002")), 1);
    let listed = vm.host().inner.learnable_skills(hero_id).unwrap();
    assert!(listed.iter().any(|skill| {
        skill.alias == *b"L001" && skill.block == Some(crate::playercolor_host::LearnBlock::MaxRank)
    }));
    assert!(listed.iter().any(|skill| {
        skill.alias == *b"L002" && skill.block == Some(crate::playercolor_host::LearnBlock::MaxRank)
    }));
    assert!(
        vm.host()
            .inner
            .skills_ready_to_learn(hero_id)
            .unwrap()
            .is_empty()
    );
    learn(&mut vm, *b"L001");
    raise(&mut vm, 1);
    raise(&mut vm, -1);
    assert_eq!(query(&mut vm, "GetHeroLevel", None), 11);
    assert_eq!(vm.global("H"), hero);
    assert_eq!(vm.host().inner.inner().handles(), handles);
    let normal = vm.global("N");
    assert_eq!(
        vm.host()
            .inner
            .native("GetHeroLevel", std::slice::from_ref(&normal))
            .unwrap(),
        Value::Int(7)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetHeroSkillPoints", std::slice::from_ref(&normal))
            .unwrap(),
        Value::Int(0)
    );
    let Value::Handle(normal_id) = normal else {
        panic!("normal unit handle: {normal:?}");
    };
    assert!(
        vm.host()
            .inner
            .skills_ready_to_learn(normal_id)
            .unwrap()
            .is_empty()
    );
    vm.host()
        .inner
        .native(
            "SelectHeroSkill",
            &[normal, Value::Int(i32::from_be_bytes(*b"L001"))],
        )
        .unwrap();
}

#[test]
fn custom_aliases_and_scalar_map_overrides_are_not_replaced_by_stock_values() {
    let mut overlay = Overlay::default();
    overlay.custom.insert(
        "L003".into(),
        (
            "L001".into(),
            vec![pcx_objects::Mod {
                field: *b"arlv",
                value_type: pcx_objects::ValueType::Int,
                selectors: Some([0, 0]),
                value: pcx_objects::Value::Int(5),
                end_word: 0,
            }],
        ),
    );
    let mut vm = host(NATIVES, "L003", overlay, "", 8);
    vm.call("setup", &[]).unwrap();
    learn(&mut vm, *b"L003");
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L003")), 0);
    raise(&mut vm, 5);
    learn(&mut vm, *b"L003");
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L003")), 1);
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L001")), 0);
}

#[test]
fn unknown_dependencies_and_indexed_rules_refuse_before_state_changes() {
    let mut vm = host(
        NATIVES,
        "L001",
        Overlay::default(),
        "[L001]\nRequires=R001\n",
        8,
    );
    vm.call("setup", &[]).unwrap();
    let before = query(&mut vm, "GetHeroSkillPoints", None);
    assert!(
        vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"L001"))])
            .is_err()
    );
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), before);
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L001")), 0);
    let mut overlay = Overlay::default();
    overlay.mods.insert(
        "L001".into(),
        vec![pcx_objects::Mod {
            field: *b"arlv",
            value_type: pcx_objects::ValueType::Int,
            selectors: Some([2, 0]),
            value: pcx_objects::Value::Int(5),
            end_word: 0,
        }],
    );
    let mut vm = host(NATIVES, "L001", overlay, "", 8);
    vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let before = vm.host().inner.inner().handles();
    assert!(vm.call("setup", &[]).is_err());
    assert_eq!(vm.host().inner.inner().handles(), before);
}

#[test]
fn null_and_invalid_inputs_do_not_spend_points_or_create_abilities() {
    let mut vm = host(NATIVES, "L001", Overlay::default(), "", 8);
    vm.call("setup", &[]).unwrap();
    for name in ["GetHeroLevel", "GetHeroSkillPoints"] {
        assert_eq!(
            vm.host().inner.native(name, &[Value::Null]).unwrap(),
            Value::Int(0)
        );
    }
    assert_eq!(
        vm.host()
            .inner
            .native("SelectHeroSkill", &[Value::Null, Value::Int(0)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitAbilityLevel", &[Value::Null, Value::Int(0)])
            .unwrap(),
        Value::Int(0)
    );
    let before = query(&mut vm, "GetHeroSkillPoints", None);
    let player = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    assert!(
        vm.host()
            .inner
            .native("SelectHeroSkill", &[player, Value::Int(0)])
            .is_err()
    );
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), before);
}

#[test]
fn omitted_learning_declarations_stay_unsupported_and_empty_lists_have_no_points() {
    let mut vm = host(
        &NATIVES.replace(
            "native SelectHeroSkill takes unit u, integer ability returns nothing\n",
            "",
        ),
        "_",
        Overlay::default(),
        "",
        8,
    );
    vm.call("setup", &[]).unwrap();
    let hero = vm.global("H");
    assert!(
        vm.host()
            .inner
            .native("SelectHeroSkill", &[hero, Value::Int(0)])
            .is_err()
    );
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), 0);
    let mut vm = host(NATIVES, "L001,L002", Overlay::default(), "", 1);
    vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let before = vm.host().inner.inner().handles();
    assert!(vm.call("setup", &[]).is_err());
    assert_eq!(vm.host().inner.inner().handles(), before);
}

#[test]
fn already_installed_learnable_abilities_keep_the_source_initial_point_pool() {
    let mut vm = host(NATIVES, "L000", Overlay::default(), "", 8);
    vm.call("setup", &[]).unwrap();
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), 1);
    learn(&mut vm, *b"L000");
    assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), 1);
    assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L000")), 1);
}

#[test]
fn live_skill_event_listeners_are_not_silently_skipped() {
    for player_event in [false, true] {
        for declared in [None, Some(13), Some(21)] {
            let extra = format!(
                "{NATIVES}type trigger extends agent\ntype event extends agent\ntype eventid extends handle\ntype unitevent extends eventid\ntype playerunitevent extends eventid\ntype boolexpr extends agent\nnative CreateTrigger takes nothing returns trigger\nnative TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event\nnative TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event\nnative DisableTrigger takes trigger t returns nothing\nconstant native ConvertUnitEvent takes integer n returns unitevent\nconstant native ConvertPlayerUnitEvent takes integer n returns playerunitevent\n"
            );
            let mut common=script_common_with("widget",true,&extra).replace("\nendglobals","\nconstant unitevent SAMPLE=ConvertUnitEvent(13)\nconstant playerunitevent PLAYER_SAMPLE=ConvertPlayerUnitEvent(13)\nendglobals");
            if let Some(n) = declared {
                common=common.replace("\nendglobals",&format!("\nconstant unitevent EVENT_UNIT_HERO_SKILL=ConvertUnitEvent({n})\nconstant playerunitevent EVENT_PLAYER_HERO_SKILL=ConvertPlayerUnitEvent({n})\nendglobals"));
            }
            let ctx = context(300, &common, MAP, "unit-catalog");
            let inner = EnumHost::checked_selected(
                ctx.scripts.shared_program(),
                "hero event guard",
                &[
                    "ConvertPlacement",
                    "ConvertPlayerColor",
                    "ConvertRacePref",
                    "ConvertMapControl",
                    "ConvertUnitEvent",
                    "ConvertPlayerUnitEvent",
                ],
                Limits {
                    max_handles: 64,
                    admission_steps: 4096,
                    expression_depth: 32,
                    expression_work: 4096,
                },
            )
            .unwrap();
            let host = ObjectHost::checked(inner)
                .unwrap()
                .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
                .unwrap()
                .with_unit_catalog(
                    Arc::new(Catalog {
                        list: "L001".into(),
                    }),
                    source(),
                    limits(4),
                )
                .unwrap()
                .with_hero_skills(
                    tables(""),
                    catalog_origin(),
                    Overlay::default(),
                    hero_skills::tests::rules(),
                    HeroBudget {
                        abilities_per_unit: 8,
                        overlay_objects: 8,
                        modifications_per_ability: 8,
                    },
                )
                .unwrap()
                .with_trigger_admission(crate::playercolor_host::TriggerBudget {
                    max_triggers: 1,
                    max_unit_events: 1,
                    max_actions: 0,
                })
                .unwrap()
                .with_player_unit_events(1)
                .unwrap();
            let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
            vm.init_globals().unwrap();
            vm.call("setup", &[]).unwrap();
            let unit = vm.global("H");
            let trigger = vm.host().inner.native("CreateTrigger", &[]).unwrap();
            if player_event {
                let player = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
                let event = vm.global("PLAYER_SAMPLE");
                vm.host()
                    .inner
                    .native(
                        "TriggerRegisterPlayerUnitEvent",
                        &[trigger.clone(), player, event, Value::Null],
                    )
                    .unwrap();
            } else {
                let event = vm.global("SAMPLE");
                vm.host()
                    .inner
                    .native("TriggerRegisterUnitEvent", &[trigger.clone(), unit, event])
                    .unwrap();
            }
            let before = query(&mut vm, "GetHeroSkillPoints", None);
            let result = vm.call("learn", &[Value::Int(i32::from_be_bytes(*b"L001"))]);
            if declared == Some(21) {
                assert!(result.is_ok());
                assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L001")), 1);
            } else {
                assert!(result.is_err());
                assert_eq!(query(&mut vm, "GetHeroSkillPoints", None), before);
                vm.host()
                    .inner
                    .native("DisableTrigger", &[trigger])
                    .unwrap();
                learn(&mut vm, *b"L001");
                assert_eq!(query(&mut vm, "GetUnitAbilityLevel", Some(*b"L001")), 1);
            }
        }
    }
}

#[test]
fn hero_event_response_declarations_must_match_the_source_contract() {
    for declaration in [
        "constant native GetLearnedSkill takes nothing returns unit\n",
        "native GetLearnedSkill takes nothing returns integer\n",
    ] {
        let common = script_common_with("widget", true, &format!("{NATIVES}{declaration}"));
        let ctx = context(300, &common, MAP, "unit-catalog");
        let h = object(&ctx, 64)
            .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
            .unwrap()
            .with_unit_catalog(
                Arc::new(Catalog {
                    list: "L001".into(),
                }),
                source(),
                limits(4),
            )
            .unwrap();
        let result = h.with_hero_skills(
            tables(""),
            catalog_origin(),
            Overlay::default(),
            hero_skills::tests::rules(),
            HeroBudget {
                abilities_per_unit: 8,
                overlay_objects: 8,
                modifications_per_ability: 8,
            },
        );
        assert!(matches!(
            result,
            Err(crate::playercolor_host::Error {
                failure: Failure::Signature,
                ..
            })
        ));
    }
}

#[test]
fn invalid_ability_list_metadata_refuses_before_unit_allocation() {
    let mut vm = host(NATIVES, "L001", Overlay::default(), "", 8);
    let player = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let before = vm.host().inner.inner().handles();
    for rawcode in [*b"Hbad", *b"Hidx"] {
        assert!(
            vm.host()
                .inner
                .native(
                    "CreateUnit",
                    &[
                        player.clone(),
                        Value::Int(i32::from_be_bytes(rawcode)),
                        Value::Real(0.0),
                        Value::Real(0.0),
                        Value::Real(0.0)
                    ]
                )
                .is_err()
        );
        assert_eq!(vm.host().inner.inner().handles(), before);
    }
}

#[test]
fn catalog_identity_is_distinct_from_the_map_record_source_and_remains_checked() {
    let common = script_common_with("widget", true, NATIVES);
    let ctx = context(300, &common, MAP, "unit-catalog");
    let make = || {
        let mut map_source = source();
        map_source.install = "selected map container".into();
        object(&ctx, 64)
            .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
            .unwrap()
            .with_unit_catalog(
                Arc::new(Catalog {
                    list: "L001".into(),
                }),
                map_source,
                limits(4),
            )
            .unwrap()
    };
    let limits = HeroBudget {
        abilities_per_unit: 8,
        overlay_objects: 8,
        modifications_per_ability: 8,
    };
    assert!(
        make()
            .with_hero_skills(
                tables(""),
                catalog_origin(),
                Overlay::default(),
                hero_skills::tests::rules(),
                limits
            )
            .is_ok()
    );
    let mut foreign = catalog_origin();
    foreign.source = "other catalog".into();
    assert!(
        make()
            .with_hero_skills(
                tables(""),
                foreign,
                Overlay::default(),
                hero_skills::tests::rules(),
                limits
            )
            .is_err()
    );
}
