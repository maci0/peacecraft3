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
    let data = sylk(
        &["alias", "code", "levels", "reqLevel", "levelSkip"],
        &[
            &["L000", "L000", "1", "0", "0"],
            &["L001", "L001", "3", "1", "0"],
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
fn host(
    extra: &str,
    list: &str,
    overlay: Overlay,
    profile: &str,
    abilities: usize,
) -> pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>> {
    let common = script_common_with("widget", true, extra);
    let map = if extra.contains("SelectHeroSkill") {
        MAP
    } else {
        MAP.split("function learn").next().unwrap()
    };
    let ctx = context(300, &common, map, "unit-catalog");
    let h = object(&ctx, 64)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::new(Catalog { list: list.into() }), source(), limits(4))
        .unwrap()
        .with_hero_skills(
            tables(profile),
            catalog_origin(),
            overlay,
            hero_skills::tests::rules(),
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
