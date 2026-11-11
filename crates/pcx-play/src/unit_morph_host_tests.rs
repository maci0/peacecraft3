use super::*;
use crate::{camera_margins::SourceWindow, neutral_player::sha256};
use pcx_sim::world::{ObjectCatalog, ResolutionProblem};

const SCRIPT: &str = r#"
globals
unit U=null
integer Seen=0
integer OldKinds=0
integer OldOrders=0
integer Finishes=0
integer Ends=0
integer Mode=0
endglobals
function main takes nothing returns nothing
set U=CreateUnit(Player(4),'u001',16.,16.,0.)
endfunction
function observe takes nothing returns nothing
set Seen=Seen+1
if GetUnitTypeId(U)=='u001' then
set OldKinds=OldKinds+1
endif
if GetUnitCurrentOrder(U)==OrderId("form") then
set OldOrders=OldOrders+1
endif
if Mode==1 then
call RemoveUnit(U)
endif
endfunction
function finish takes nothing returns nothing
set Finishes=Finishes+1
if Mode==2 then
call IssuePointOrder(U,"gamma",72.,16.)
endif
endfunction
function endcast takes nothing returns nothing
set Ends=Ends+1
endfunction
function listen takes nothing returns nothing
local trigger t=CreateTrigger()
call TriggerRegisterUnitEvent(t,U,EVENT_UNIT_ISSUED_ORDER)
call TriggerAddAction(t,function observe)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,U,EVENT_UNIT_SPELL_CHANNEL)
call TriggerAddAction(t,function observe)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,U,EVENT_UNIT_SPELL_CAST)
call TriggerAddAction(t,function observe)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,U,EVENT_UNIT_SPELL_EFFECT)
call TriggerAddAction(t,function observe)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,U,EVENT_UNIT_SPELL_FINISH)
call TriggerAddAction(t,function finish)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,U,EVENT_UNIT_SPELL_ENDCAST)
call TriggerAddAction(t,function endcast)
endfunction
function on takes nothing returns boolean
return IssueImmediateOrder(U,"form")
endfunction
function off takes nothing returns boolean
return IssueImmediateOrderById(U,OrderId("unform"))
endfunction
function root_order takes nothing returns boolean
return IssueImmediateOrder(U,"root")
endfunction
function unroot_order takes nothing returns boolean
return IssueImmediateOrder(U,"unroot")
endfunction
function unroot_by_id takes nothing returns boolean
return IssueImmediateOrderById(U,OrderId("unroot"))
endfunction
function burrow_order takes nothing returns boolean
return IssueImmediateOrder(U,"burrow")
endfunction
function unburrow_order takes nothing returns boolean
return IssueImmediateOrder(U,"unburrow")
endfunction
function move_order takes nothing returns boolean
return IssuePointOrder(U,"gamma",72.,16.)
endfunction
function move_callback takes nothing returns nothing
set Mode=2
endfunction
function remove_callback takes nothing returns nothing
set Mode=1
endfunction
"#;

fn registry_numeric() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
    crate::script_numeric::NumericPolicy,
) {
    let (mut image, mut numeric, budget) = crate::script_numeric::tests::fixture();
    let va = |at: usize| 0x301000 + at as u32 - 0x200;
    let register = va(0x1300);
    image[0x1300] = 0xc3;
    let mut code = vec![0x56, 0x8b, 0x35, 0, 0, 0x40, 0, 0x8b, 0xce];
    for (i, (name, id)) in [
        ("alpha", 431i32),
        ("beta", 744),
        ("delta", 2122),
        ("form", 2701),
        ("unform", 2702),
        ("root", 2801),
        ("unroot", 2802),
        ("burrow", 2803),
        ("unburrow", 2804),
    ]
    .iter()
    .enumerate()
    {
        let at = 0x1400 + i * 0x40;
        image[at..at + name.len()].copy_from_slice(name.as_bytes());
        code.push(0x68);
        code.extend(va(at).to_le_bytes());
        code.push(0x68);
        code.extend(id.to_le_bytes());
        if i != 0 {
            code.extend([0x8b, 0xce]);
        }
        code.push(0xe8);
        let next = va(0x1200) + code.len() as u32 + 4;
        code.extend(register.wrapping_sub(next).to_le_bytes());
    }
    code.extend([0x5e, 0xc3]);
    image[0x1200..0x1200 + code.len()].copy_from_slice(&code);
    image[0x1640..0x1646].copy_from_slice(b"gamma\0");
    image[0x1680..0x1684].copy_from_slice(&1631i32.to_le_bytes());
    let mut boot = vec![0x68];
    boot.extend(va(0x1640).to_le_bytes());
    boot.extend([0xff, 0x35]);
    boot.extend(va(0x1680).to_le_bytes());
    for target in [register, va(0x1200)] {
        boot.push(0xe8);
        let next = va(0x1900) + boot.len() as u32 + 4;
        boot.extend(target.wrapping_sub(next).to_le_bytes());
    }
    image[0x1900..0x1900 + boot.len()].copy_from_slice(&boot);
    let mut window = |at, prefix: &[u8], id: i32| {
        let mut b = prefix.to_vec();
        b.extend(id.to_le_bytes());
        image[at..at + b.len()].copy_from_slice(&b);
        SourceWindow {
            va: va(at),
            len: b.len() as u32,
            digest: sha256(&b),
        }
    };
    let hold = window(0x1700, &[0x3d], 431);
    let stop = window(0x1710, &[0x81, 0xfb], 744);
    let movement = window(0x1720, &[0x3d], 1631);
    let patrol = window(0x1730, &[0x3d], 2122);
    numeric.image_sha256 = sha256(&image);
    let binding = crate::order_registry::Binding {
        origin: numeric.origin.clone(),
        image_sha256: numeric.image_sha256,
        initializer: SourceWindow {
            va: va(0x1200),
            len: code.len() as u32,
            digest: sha256(&code),
        },
        bootstrap: SourceWindow {
            va: va(0x1900),
            len: boot.len() as u32,
            digest: sha256(&boot),
        },
        direct_register: register,
        register,
    };
    let registry =
        Arc::new(crate::order_registry::Registry::read(&image, binding, budget, 12, 32).unwrap());
    let activity = crate::unit_activity::Rules::read(
        &image,
        crate::unit_activity::Binding {
            origin: numeric.origin.clone(),
            image_sha256: numeric.image_sha256,
            hold,
            stop,
            movement,
            patrol,
        },
        &registry,
        budget,
    )
    .unwrap();
    let numeric = crate::script_numeric::NumericPolicy::read(&image, numeric, budget, 64).unwrap();
    (registry, activity, numeric)
}

/// The catalog, the animation property key and whether the alternate form `u002` flies.
struct Forms(Arc<dyn ObjectCatalog + Send + Sync>, String, bool);
impl ObjectCatalog for Forms {
    fn resolve(&self, code: [u8; 4]) -> Result<pcx_sim::world::Definition, ResolutionProblem> {
        let mut d = self.0.resolve(code)?;
        for f in d.fields.values_mut() {
            let Some(m) = &f.metadata else { continue };
            if let FieldSource::Column {
                slk: SlkId::UnitData,
                column,
            } = &m.source
            {
                if column == "movetp" {
                    f.value = Ok(FieldValue::Cell(
                        if code == *b"u002" && self.2 {
                            "fly"
                        } else {
                            "foot"
                        }
                        .into(),
                    ));
                }
                if column == "moveHeight" {
                    // The alternate form carries its height as an unreal override,
                    // the shape of the Human02 interlude's authored form record.
                    f.value = Ok(if code == *b"u002" && self.2 {
                        FieldValue::Override(pcx_objects::Value::Unreal(73.))
                    } else {
                        FieldValue::Cell("0".into())
                    });
                }
            }
            // A ground alternate form regenerates faster and lists one more ability.
            if code == *b"u002"
                && !self.2
                && let FieldSource::Column { slk, column } = &m.source
            {
                if *slk == SlkId::UnitBalance && column == "regenHP" {
                    f.value = Ok(FieldValue::Cell("7".into()));
                }
                if *slk == SlkId::UnitAbilities
                    && column == "abilList"
                    && let Ok(FieldValue::Cell(list)) = &f.value
                {
                    f.value = Ok(FieldValue::Cell(format!("{list},X001")));
                }
            }
            if let FieldSource::Profile { key, .. } = &m.source
                && key == &self.1
            {
                f.value = Ok(FieldValue::Cell(
                    if code == *b"u002" { "wave" } else { "_" }.into(),
                ));
            }
        }
        Ok(d)
    }
}
fn tables(cast: &str, duration: &str) -> DefaultTables {
    let data = sylk(
        &[
            "alias",
            "code",
            "levels",
            "reqLevel",
            "levelSkip",
            "hero",
            "DataA1",
            "DataB1",
            "DataC1",
            "DataD1",
            "UnitID1",
            "Cost1",
            "Cast1",
            "Dur1",
            "Cool1",
        ],
        &[
            &[
                "X001", "Q987", "1", "0", "0", "0", "u001", "5", "0.25", "0.5", "u002", "7", cast,
                duration, "0",
            ],
            &[
                "X002", "R987", "1", "0", "0", "0", "1", "2", "0", "0", "u002", "0", "0", "2.5",
                "0",
            ],
            &[
                "X003", "Z987", "1", "0", "0", "0", "u001", "1", "0", "0", "u002", "0", "0",
                "1.45", "0",
            ],
            &[
                "X004", "R987", "1", "0", "0", "0", "2", "1", "0", "0", "u002", "0", "0", "2.5",
                "0",
            ],
            // A burrow class whose source form is not the unit's type `u001`.
            &[
                "X005", "Z987", "1", "0", "0", "0", "u003", "1", "0", "0", "u002", "0", "0",
                "1.45", "0",
            ],
            // The same delayed change as `X003`, but the harness gives `u002` air movement.
            &[
                "X006", "Z987", "1", "0", "0", "0", "u001", "1", "0", "0", "u002", "0", "0",
                "1.45", "0",
            ],
        ],
    );
    let metadata = sylk(
        &[
            "ID", "field", "slk", "index", "type", "category", "repeat", "data",
        ],
        &[
            &[
                "mDa1",
                "Data",
                "AbilityData",
                "-1",
                "unitCode",
                "stats",
                "1",
                "1",
            ],
            &[
                "mDa2",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "1",
                "2",
            ],
            &[
                "mDa3",
                "Data",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
                "3",
            ],
            &[
                "mDa4",
                "Data",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
                "4",
            ],
            &[
                "mUid",
                "UnitID",
                "AbilityData",
                "-1",
                "unitCode",
                "stats",
                "1",
                "0",
            ],
            &[
                "mCos",
                "Cost",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "1",
                "0",
            ],
            &[
                "mCas",
                "Cast",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
                "0",
            ],
            &[
                "mDur",
                "Dur",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
                "0",
            ],
            &[
                "mCoo",
                "Cool",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
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
            (SlkId::AbilityData.path(), &data),
            (SlkId::AbilityMetaData.path(), &metadata),
        ],
        &[],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 65536,
            max_aggregate_input_bytes: 131072,
        },
    )
    .unwrap()
}
fn open(cast: &str, duration: &str) -> (Vm, HandleId) {
    open_with("X001", cast, duration)
}
fn open_with(abilities: &str, cast: &str, duration: &str) -> (Vm, HandleId) {
    open_with_attack(abilities, cast, duration, true)
}
/// `first_attack` false gives the unit no attack 1 (`weapTp1` a placeholder, `weapsOn` 0), as on a
/// rootable building.
fn open_with_attack(
    abilities: &str,
    cast: &str,
    duration: &str,
    first_attack: bool,
) -> (Vm, HandleId) {
    let (registry, activity, numeric) = registry_numeric();
    let mut rules = hero_rules().0;
    rules.binding.image_sha256 = registry.binding().image_sha256;
    let morph = crate::unit_morph_rules::Rules {
        origin: registry.binding().origin.clone(),
        image_sha256: registry.binding().image_sha256,
        flight: Ok(crate::unit_morph_rules::FlightRules {
            minimum_target: 0.03125,
        }),
        classes: vec![
            crate::unit_morph_rules::Class {
                code: *b"Q987",
                on: registry.id("form").unwrap(),
                off: registry.id("unform").unwrap(),
                uninterruptible: 1,
                immediate_takeoff: 4,
            },
            crate::unit_morph_rules::Class {
                code: *b"Z987",
                on: registry.id("burrow").unwrap(),
                off: registry.id("unburrow").unwrap(),
                uninterruptible: 1,
                immediate_takeoff: 4,
            },
        ],
    };
    let root = crate::unit_root_rules::Rules {
        origin: registry.binding().origin.clone(),
        image_sha256: registry.binding().image_sha256,
        class: crate::unit_root_rules::Class {
            code: *b"R987",
            root: registry.id("root").unwrap(),
            unroot: registry.id("unroot").unwrap(),
            rooted_state_bit: 1 << 27,
        },
    };
    let rules = rules.with_morph(Ok(morph)).with_root(Ok(root));
    let mut fields = hero_fields("STR");
    fields.push(column(
        b"mUpg",
        SlkId::UnitBalance,
        "upgrades",
        FieldKind::Token,
        "Qeff",
    ));
    fields.retain(|(_,f)| !f.metadata.as_ref().is_some_and(|m| matches!(&m.source,FieldSource::Column{slk:SlkId::UnitAbilities,column}if column=="abilList")));
    fields.push(column(
        b"A008",
        SlkId::UnitAbilities,
        "abilList",
        FieldKind::Token,
        abilities,
    ));
    // Attacks 1 and 2 are both enabled by `weapsOn`; ranges tell them apart.
    fields.retain(|(id, _)| id != b"W001" && (first_attack || id != b"W003"));
    if !first_attack {
        fields.push(column(
            b"W003",
            SlkId::UnitWeapons,
            "weapTp1",
            FieldKind::Token,
            "_",
        ));
    }
    for (id, key, kind, value) in [
        (
            b"W001",
            "weapsOn",
            FieldKind::Int,
            if first_attack { "3" } else { "2" },
        ),
        (b"W021", "weapTp2", FieldKind::Token, "normal"),
        (b"W022", "atkType2", FieldKind::Token, "pierce"),
        (b"W023", "rangeN2", FieldKind::Int, "700"),
        (b"W024", "cool2", FieldKind::Real, "2"),
        (b"W025", "dmgpt2", FieldKind::Real, "0.4"),
        (b"W026", "dmgplus2", FieldKind::Int, "9"),
        (b"W027", "dice2", FieldKind::Int, "1"),
        (b"W028", "sides2", FieldKind::Int, "1"),
        (b"W029", "Farea2", FieldKind::Real, "25"),
        (b"W030", "Harea2", FieldKind::Real, "75"),
        (b"W031", "Qarea2", FieldKind::Real, "125"),
        (b"W032", "Hfact2", FieldKind::Real, "0.5"),
        (b"W033", "Qfact2", FieldKind::Real, "0.25"),
        (b"W034", "splashTargs2", FieldKind::Token, "ground,enemy"),
    ] {
        fields.push(column(id, SlkId::UnitWeapons, key, kind, value));
    }
    fields.push(column(
        b"mHgt",
        SlkId::UnitData,
        "moveHeight",
        FieldKind::Real,
        "0",
    ));
    for (id, key, kind, value) in [
        (b"mFus", "fused", FieldKind::Int, "1"),
        (b"mFma", "fmade", FieldKind::Int, "0"),
        (b"mBld", "isbldg", FieldKind::Bool, "0"),
    ] {
        fields.push(column(id, SlkId::UnitBalance, key, kind, value));
    }
    for (id, key, value) in [(b"H002", "manaN", "100"), (b"H003", "mana0", "100")] {
        fields.retain(|(i, _)| i != id);
        fields.push(column(id, SlkId::UnitBalance, key, FieldKind::Int, value));
    }
    let property_key = crate::unit_animation_rarity::tests::rules().props;
    let mut prop = geometry_field(Ok(FieldValue::Cell("_".into())));
    let meta = prop.metadata.as_mut().unwrap();
    meta.fourcc = *b"mPrp";
    meta.field = property_key.clone();
    meta.kind = FieldKind::Token;
    meta.declared = "stringList".into();
    meta.source = FieldSource::Profile {
        key: property_key.clone(),
        index: -1,
        slots: 1,
    };
    fields.push((*b"mPrp", prop));
    let extra = format!(
        "{}\n{}\n{DECLARATIONS}\nnative RemoveUnit takes unit u returns nothing\nconstant native GetUnitTypeId takes unit u returns integer\nconstant native GetUnitFlyHeight takes unit u returns real\nnative SetUnitFlyHeight takes unit u, real height, real rate returns nothing\nconstant native GetIssuedOrderId takes nothing returns integer\nglobals\nconstant playerunitevent EVENT_PLAYER_UNIT_ISSUED_ORDER=ConvertPlayerUnitEvent(80)\nconstant playerunitevent EVENT_PLAYER_UNIT_SPELL_CHANNEL=ConvertPlayerUnitEvent(81)\nconstant playerunitevent EVENT_PLAYER_UNIT_SPELL_CAST=ConvertPlayerUnitEvent(82)\nconstant playerunitevent EVENT_PLAYER_UNIT_SPELL_EFFECT=ConvertPlayerUnitEvent(83)\nconstant playerunitevent EVENT_PLAYER_UNIT_SPELL_FINISH=ConvertPlayerUnitEvent(84)\nconstant playerunitevent EVENT_PLAYER_UNIT_SPELL_ENDCAST=ConvertPlayerUnitEvent(85)\nconstant unitevent EVENT_UNIT_ISSUED_ORDER=ConvertUnitEvent(70)\nconstant unitevent EVENT_UNIT_SPELL_CHANNEL=ConvertUnitEvent(71)\nconstant unitevent EVENT_UNIT_SPELL_CAST=ConvertUnitEvent(72)\nconstant unitevent EVENT_UNIT_SPELL_EFFECT=ConvertUnitEvent(73)\nconstant unitevent EVENT_UNIT_SPELL_FINISH=ConvertUnitEvent(74)\nconstant unitevent EVENT_UNIT_SPELL_ENDCAST=ConvertUnitEvent(75)\nendglobals\n",
        enumeration::TRIGGERS,
        super::super::activity::ORDERS
    );
    let extra = format!(
        "{extra}\nconstant native SetPlayerTechResearched takes player p, integer id, integer level returns nothing\nconstant native GetPlayerTechResearched takes player p, integer id, boolean specific returns boolean\n"
    );
    let mut h = position_host_admitting(2000, 16, SCRIPT, &extra, &fields, |h| h)
        .with_player_tech_researched(crate::playercolor_host::TechLimitBudget { max_records: 8 })
        .unwrap()
        .with_order_registry(registry)
        .unwrap()
        .with_script_numeric(numeric)
        .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap()
        .with_hero_skills(
            tables(cast, duration),
            crate::scripts::AssetIdentity {
                source: source().install,
                edition: "fixture".into(),
                version: "fixture".into(),
            },
            Default::default(),
            rules,
            crate::playercolor_host::HeroBudget {
                abilities_per_unit: 8,
                overlay_objects: 0,
                modifications_per_ability: 0,
            },
        )
        .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.,
        mana_per_intelligence: 15.,
        life_regen_per_strength: 0.,
        mana_regen_per_intelligence: 0.,
    })
    .unwrap();
    h.admit_combat(super::rules(), "alpha").unwrap();
    let model = Arc::new(crate::unit_animation_rarity::tests::model(&[
        ("stand", 0.),
        ("stand wave", 0.),
        ("walk", 0.),
        ("walk wave", 0.),
        ("morph", 0.),
        ("morph wave", 0.),
    ]));
    h.admit_unit_models(move |_| Ok(Some(model.clone())))
        .unwrap();
    let mut rarity = crate::unit_animation_rarity::tests::rules();
    rarity.image_sha256 = [0; 32];
    h.admit_unit_animation_rarity(Ok(rarity)).unwrap();
    h.construction_test_catalog(Arc::new(Forms(
        h.unit_catalog().unwrap(),
        property_key,
        !abilities.contains("X003") && !abilities.contains("X005"),
    )));
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let Value::Handle(u) = vm.global("U") else {
        panic!("unit")
    };
    (vm, u)
}

fn advance(vm: &mut Vm, until: f64) {
    while vm.host().inner.timer_game_time().unwrap() < until {
        let next = (vm.host().inner.timer_game_time().unwrap() + 1. / 64.).min(until);
        step(vm, next);
        while let Some(event) = vm.host().inner.take_fired_event() {
            if let Some(delivery) = vm.host().inner.begin_fired_event(&event).unwrap() {
                assert!(delivery.conditions.is_empty());
                vm.run_thread(&delivery.actions).unwrap();
                vm.host().inner.finish_fired_event();
            }
        }
    }
}

#[test]
fn morph_reapplies_shared_upgrades_with_full_range_rounding() {
    let (mut vm, unit) = open("0.125", "0.375");
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    let upgrade = i32::from_be_bytes(*b"Qeff");
    let table = crate::upgrade_effects::Table::synthetic(
        &[*b"rmnx"],
        vec![(
            upgrade,
            vec![crate::upgrade_effects::Slot {
                number: 1,
                effect: *b"rmnx",
                base: 100.0,
                modifier: 0.0,
                code: String::new(),
            }],
        )],
    );
    host.admit_upgrade_research(
        &context,
        crate::upgrade_research::fixture_with_effects(&context.sources.catalog, table),
    )
    .unwrap();
    let owner = host.executed_unit(unit).unwrap().owner;
    host.native(
        "SetPlayerTechResearched",
        &[Value::Handle(owner), Value::Int(upgrade), Value::Int(1)],
    )
    .unwrap();
    let mana = vm.global("UNIT_STATE_MANA");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(unit), mana, Value::Real(74.0)],
        )
        .unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    let vitals = vm.host().inner.unit_vitals(unit).unwrap().unwrap();
    assert_eq!((vitals.mana, vitals.max_mana), (68.0, 200.0));
}

#[test]
fn failed_morph_upgrade_preflight_keeps_unit_and_mana() {
    let (mut vm, unit) = open("0.125", "0.375");
    let host = &mut vm.host().inner;
    let context = Arc::clone(host.player_context().unwrap());
    let upgrade = i32::from_be_bytes(*b"Qeff");
    let table = crate::upgrade_effects::Table::synthetic(
        &[*b"rmnx"],
        vec![(
            upgrade,
            vec![crate::upgrade_effects::Slot {
                number: 1,
                effect: *b"rmnx",
                base: 100.0,
                modifier: 0.0,
                code: String::new(),
            }],
        )],
    );
    host.admit_upgrade_research(
        &context,
        crate::upgrade_research::fixture_with_effects(&context.sources.catalog, table),
    )
    .unwrap();
    let owner = host.executed_unit(unit).unwrap().owner;
    host.native(
        "SetPlayerTechResearched",
        &[Value::Handle(owner), Value::Int(upgrade), Value::Int(1)],
    )
    .unwrap();
    let mana = vm.global("UNIT_STATE_MANA");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(unit), mana, Value::Real(20.0)],
        )
        .unwrap();
    vm.host().inner.set_test_maximum(unit, None, Some(50.0));
    let before = vm.host().inner.executed_unit(unit).unwrap().clone();
    assert!(vm.call("on", &[]).is_err());
    assert_eq!(vm.host().inner.executed_unit(unit).unwrap(), &before);
}

#[test]
fn same_handle_public_on_off_events_and_independent_nonstock_clocks() {
    for cast in ["0.125", "0.375", "0.75"] {
        let (mut vm, u) = open(cast, "0.375");
        vm.call("listen", &[]).unwrap();
        let before = vm.host().inner.executed_unit(u).unwrap().clone();
        let count = vm.host().inner.executed_units().count();
        assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
        assert_eq!(vm.global("Seen"), Value::Int(4));
        assert_eq!(vm.global("OldKinds"), Value::Int(4));
        assert_eq!(vm.global("OldOrders"), Value::Int(4));
        let r = vm.host().inner.executed_unit(u).unwrap();
        assert_eq!(r.definition.rawcode, *b"u002");
        assert_eq!(r.owner, before.owner);
        assert_eq!(r.position, before.position);
        assert_eq!(r.vitals.unwrap().life, before.vitals.unwrap().life);
        assert_eq!(r.vitals.unwrap().mana, 93.);
        assert!(matches!(
            vm.host().inner.ai_birth_reach_bound(u, u, [0., 0.], 0.),
            Err(crate::ai_placement::ExclusionError::UnknownMovement { unit, .. }) if unit == u
        ));
        assert_eq!(vm.host().inner.executed_units().count(), count);
        assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(false));
        for t in [0.125, 0.375, 0.75, 1., 1.25] {
            advance(&mut vm, t);
        }
        assert_eq!(vm.global("Finishes"), Value::Int(1));
        assert_eq!(vm.global("Ends"), Value::Int(1));
        assert_eq!(
            vm.host()
                .inner
                .native("GetUnitCurrentOrder", &[Value::Handle(u)])
                .unwrap(),
            Value::Int(0)
        );
        assert!(vm.host().inner.test_morph_active_count() == 0);
        assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
        assert_eq!(
            vm.host().inner.executed_unit(u).unwrap().definition.rawcode,
            *b"u002"
        );
        assert_eq!(
            vm.host()
                .inner
                .executed_unit(u)
                .unwrap()
                .movement()
                .unwrap(),
            crate::unit_placement::Movement::Ground
        );
        assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(false));
        advance(&mut vm, 2.125);
        assert_eq!(
            vm.host().inner.executed_unit(u).unwrap().definition.rawcode,
            *b"u001"
        );
        assert!(vm.host().inner.executed_unit(u).unwrap().morph.is_none());
        assert_eq!(vm.global("Finishes"), Value::Int(2));
        assert_eq!(vm.global("Ends"), Value::Int(2));
        assert!(vm.host().inner.test_morph_active_count() == 0);
    }
}
#[test]
fn callback_removal_retires_pending_form_without_resurrection() {
    let (mut vm, u) = open("0.75", "0.375");
    vm.call("listen", &[]).unwrap();
    vm.call("remove_callback", &[]).unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    assert!(vm.host().inner.executed_unit(u).is_err());
    assert!(vm.host().inner.test_morph_active_count() == 0);
    advance(&mut vm, 2.);
    assert!(vm.host().inner.test_morph_fault().is_none());
}

#[test]
fn finish_callback_move_survives_later_cast_clock() {
    let (mut vm, u) = open("0.75", "0.375");
    vm.call("listen", &[]).unwrap();
    vm.call("move_callback", &[]).unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, 0.375);
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(u)])
            .unwrap(),
        Value::Int(1631)
    );
    advance(&mut vm, 0.75);
    assert_eq!(vm.global("Finishes"), Value::Int(1));
    assert_eq!(vm.global("Ends"), Value::Int(1));
    assert!(vm.host().inner.test_morph_fault().is_none());
}

#[test]
fn frame_preflight_failure_preserves_live_unit_and_repeated_off_is_false() {
    let (mut vm, u) = open("0.75", "0.375");
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    vm.host().inner.test_synchronous_frame_limit();
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(false));
    assert!(vm.call("on", &[]).is_err());
    let after = vm.host().inner.executed_unit(u).unwrap();
    assert_eq!(after.definition.rawcode, before.definition.rawcode);
    assert_eq!(after.vitals.unwrap().mana, before.vitals.unwrap().mana);
    assert!(after.morph.is_none());
    assert_eq!(after.definition_revision, before.definition_revision);
    assert_eq!(vm.host().inner.test_morph_active_count(), 0);
}

#[test]
fn landing_public_target_is_distinct_from_visual_flight_and_pause_refuses_atomically() {
    let (mut vm, u) = open("0.125", "0.375");
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    assert!(
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Handle(u), Value::Bool(true)])
            .is_err()
    );
    assert!(
        vm.host()
            .inner
            .executed_unit(u)
            .unwrap()
            .paused_since
            .is_none()
    );
    advance(&mut vm, 0.75);
    assert_eq!(vm.call("off", &[]).unwrap(), Value::Bool(true));
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitFlyHeight", &[Value::Handle(u)])
            .unwrap(),
        Value::Real(0.)
    );
    assert_eq!(
        vm.host().inner.executed_unit(u).unwrap().flight_height,
        Some(73.)
    );
    advance(&mut vm, 0.875);
    let height = vm
        .host()
        .inner
        .executed_unit(u)
        .unwrap()
        .flight_height
        .unwrap();
    assert!(height > 0. && height < 73.);
    advance(&mut vm, 1.625);
    assert_eq!(
        vm.host().inner.executed_unit(u).unwrap().flight_height,
        Some(0.)
    );
    assert_eq!(vm.host().inner.test_morph_active_count(), 0);
}

#[test]
fn public_height_target_replaces_visual_task_then_source_cast_restores_it() {
    let (mut vm, u) = open("0.75", "0.375");
    vm.call("listen", &[]).unwrap();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, 0.5);
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    vm.host()
        .inner
        .native(
            "SetUnitFlyHeight",
            &[Value::Handle(u), Value::Real(113.), Value::Real(80.)],
        )
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitFlyHeight", &[Value::Handle(u)])
            .unwrap(),
        Value::Real(113.)
    );
    assert_eq!(
        vm.host().inner.executed_unit(u).unwrap().flight_height,
        Some(73.)
    );
    // The synthetic reciprocal table is deliberately nonstock; expected timing
    // follows its admitted arithmetic rather than host-language division.
    let (_, _, numeric) = registry_numeric();
    let divide = |a, b| {
        let Value::Real(v) =
            pcx_script::RealOps::binary(&numeric, pcx_script::ast::BinOp::Div, a, b).unwrap()
        else {
            panic!("real duration")
        };
        v
    };
    let first = 73. + 40. * (0.125 / divide(40., 80.));
    advance(&mut vm, 0.625);
    assert_eq!(
        vm.host().inner.executed_unit(u).unwrap().flight_height,
        Some(first)
    );
    // Replacement starts at the current visual altitude, not the previous target.
    vm.host()
        .inner
        .native(
            "SetUnitFlyHeight",
            &[Value::Handle(u), Value::Real(103.), Value::Real(80.)],
        )
        .unwrap();
    advance(&mut vm, 0.75);
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitFlyHeight", &[Value::Handle(u)])
            .unwrap(),
        Value::Real(73.)
    );
    assert_eq!(
        vm.host().inner.executed_unit(u).unwrap().flight_height,
        Some(first + (103. - first) * (0.125 / divide(103. - first, 80.)))
    );
    advance(&mut vm, 1.);
    assert_eq!(
        vm.host().inner.executed_unit(u).unwrap().flight_height,
        Some(73.)
    );
    let after = vm.host().inner.executed_unit(u).unwrap();
    assert_eq!(after.vitals.unwrap().mana, before.vitals.unwrap().mana);
    assert_eq!(after.position, before.position);
    assert_eq!(vm.global("Finishes"), Value::Int(1));
    assert_eq!(vm.global("Ends"), Value::Int(1));
    assert_eq!(vm.host().inner.test_morph_active_count(), 0);
    vm.host()
        .inner
        .native(
            "SetUnitFlyHeight",
            &[Value::Handle(u), Value::Real(-9.), Value::Real(0.)],
        )
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitFlyHeight", &[Value::Handle(u)])
            .unwrap(),
        Value::Real(0.03125)
    );
    assert_eq!(
        vm.host().inner.executed_unit(u).unwrap().flight_height,
        Some(0.03125)
    );
}

#[test]
fn height_refusals_preserve_target_order_and_visual_state() {
    let (mut vm, u) = open("0.125", "0.375");
    let error = vm
        .host()
        .inner
        .native(
            "SetUnitFlyHeight",
            &[Value::Handle(u), Value::Real(99.), Value::Real(1.)],
        )
        .unwrap_err()
        .to_string();
    for field in [
        "SetUnitFlyHeight",
        "unit=",
        "rawcode=[117, 48, 48, 49]",
        "movement=Ok(Ground)",
        "shown=true",
        "combat_admitted=true",
        "abilities_first16=",
        "target=99.0",
        "rate=1.0",
    ] {
        assert!(error.contains(field), "{field}: {error}");
    }
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, 0.75);
    for (height, rate) in [(f32::NAN, 1.), (99., -1.), (99., f32::INFINITY)] {
        let before = vm.host().inner.executed_unit(u).unwrap().clone();
        let error = vm
            .host()
            .inner
            .native(
                "SetUnitFlyHeight",
                &[Value::Handle(u), Value::Real(height), Value::Real(rate)],
            )
            .unwrap_err();
        assert!(error.to_string().contains("SetUnitFlyHeight"), "{error}");
        let after = vm.host().inner.executed_unit(u).unwrap();
        assert_eq!(after.morph, before.morph);
        assert_eq!(after.flight_height, before.flight_height);
        assert_eq!(after.activity, before.activity);
    }
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(u), Value::Bool(true)])
        .unwrap();
    assert!(
        vm.host()
            .inner
            .native(
                "SetUnitFlyHeight",
                &[Value::Handle(u), Value::Real(99.), Value::Real(1.)]
            )
            .is_err()
    );
    assert_eq!(
        vm.host()
            .inner
            .native(
                "SetUnitFlyHeight",
                &[Value::Null, Value::Real(99.), Value::Real(1.)]
            )
            .unwrap(),
        Value::Null
    );
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(u)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native(
                "SetUnitFlyHeight",
                &[Value::Handle(u), Value::Real(99.), Value::Real(1.)]
            )
            .unwrap(),
        Value::Null
    );
}

#[test]
fn native_class_differs_from_owned_alias_and_interrupts_existing_attack_order() {
    let (mut vm, u) = open("0.75", "0.375");
    let h = &mut vm.host().inner;
    // An inherited ability row is an identity, not its implementation class.
    assert_eq!(h.ability_parent_code(*b"X001").unwrap(), *b"X001");
    assert_eq!(h.ability_data_text(*b"X001", "code").unwrap(), "Q987");
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(u),
                Value::Str("alpha".into()),
                Value::Real(96.),
                Value::Real(16.)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(u)])
            .unwrap(),
        h.native("OrderId", &[Value::Str("alpha".into())]).unwrap()
    );
    let before = h.executed_unit(u).unwrap().clone();
    assert_eq!(vm.call("on", &[]).unwrap(), Value::Bool(true));
    let h = &mut vm.host().inner;
    let record = h.executed_unit(u).unwrap();
    assert_eq!(record.definition.rawcode, *b"u002");
    assert_eq!(record.position, before.position);
    assert!(record.combat.as_ref().unwrap().attack_destination.is_none());
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(u)])
            .unwrap(),
        h.native("OrderId", &[Value::Str("form".into())]).unwrap()
    );
}

fn weapon_of(vm: &mut Vm, u: HandleId) -> Option<crate::playercolor_host::Weapon> {
    vm.host()
        .inner
        .executed_unit(u)
        .unwrap()
        .combat
        .as_ref()
        .expect("combat admitted")
        .weapon
}

#[test]
fn creation_applies_the_rooted_attack_mask_over_weapons_on() {
    // `weapsOn` enables both attacks; DataA of 2 leaves a rooted unit with attack 2 only
    // (the native attach routine runs the mask at 0xb8f470), and `unroot` (DataB 1) swaps it.
    let (mut vm, u) = open_with("X004", "0.125", "0.375");
    let rooted = weapon_of(&mut vm, u).expect("attack 2 is enabled");
    assert_eq!(rooted.range, 700.);
    assert_eq!(rooted.splash.map(|s| s.full), Some(25.));
    assert_eq!(vm.call("unroot_order", &[]).unwrap(), Value::Bool(true));
    assert_eq!(weapon_of(&mut vm, u).map(|w| w.range), Some(64.));
}

#[test]
fn creation_without_a_root_ability_keeps_the_weapons_on_set() {
    let (mut vm, u) = open_with("X001", "0.125", "0.375");
    assert_eq!(weapon_of(&mut vm, u).map(|w| w.range), Some(64.));
}

#[test]
fn unroot_is_accepted_once_and_applies_the_uprooted_attack_mask() {
    let (mut vm, u) = open_with("X002", "0.125", "0.375");
    // DataA is 1: rooted, attack 1 only.
    assert_eq!(weapon_of(&mut vm, u).map(|w| w.range), Some(64.));
    assert!(!vm.host().inner.unit_uprooted(u));
    // The class rejects `root` as an immediate order, rooted or not.
    assert_eq!(vm.call("root_order", &[]).unwrap(), Value::Bool(false));
    assert!(!vm.host().inner.unit_uprooted(u));
    assert_eq!(vm.call("unroot_order", &[]).unwrap(), Value::Bool(true));
    assert!(vm.host().inner.unit_uprooted(u));
    // DataB is 2: attack 2 only, with its own range and splash columns.
    let second = weapon_of(&mut vm, u).expect("attack 2 is enabled");
    assert_eq!(second.range, 700.);
    assert_eq!(second.cooldown, 2.);
    assert_eq!(second.splash.map(|s| s.full), Some(25.));
    assert_eq!(vm.call("unroot_order", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.call("unroot_by_id", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.call("root_order", &[]).unwrap(), Value::Bool(false));
    assert_eq!(
        vm.host().inner.executed_unit(u).unwrap().definition.rawcode,
        *b"u001",
        "unroot changes no form"
    );
}

#[test]
fn movement_of_an_uprooted_unit_is_refused_precisely() {
    let (mut vm, u) = open_with("X002", "0.125", "0.375");
    vm.call("move_order", &[])
        .expect("a rooted unit's order is unchanged");
    assert_eq!(vm.call("unroot_order", &[]).unwrap(), Value::Bool(true));
    let error = vm.call("move_order", &[]).unwrap_err().to_string();
    assert!(error.contains("uprooted unit is not modelled"), "{error}");
    assert!(vm.host().inner.unit_uprooted(u));
}

#[test]
fn units_without_a_handling_ability_refuse_root_and_burrow_orders() {
    for abilities in ["", "X001"] {
        let (mut vm, u) = open_with(abilities, "0.125", "0.375");
        let before = vm.host().inner.executed_unit(u).unwrap().clone();
        for order in ["unroot_order", "unroot_by_id", "root_order", "burrow_order"] {
            assert_eq!(vm.call(order, &[]).unwrap(), Value::Bool(false), "{order}");
        }
        assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
        assert!(!vm.host().inner.unit_uprooted(u));
    }
}

#[test]
fn unroot_with_issued_order_listeners_is_refused_before_any_change() {
    let (mut vm, u) = open_with("X002", "0.125", "0.375");
    vm.call("listen", &[]).unwrap();
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    let error = vm.call("unroot_order", &[]).unwrap_err().to_string();
    assert!(error.contains("issued-order event delivery"), "{error}");
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
    assert!(!vm.host().inner.unit_uprooted(u));
}

fn rawcode(vm: &mut Vm, u: HandleId) -> [u8; 4] {
    vm.host().inner.executed_unit(u).unwrap().definition.rawcode
}

#[test]
fn burrow_changes_the_form_dur_seconds_after_the_order_and_not_before() {
    let (mut vm, u) = open_with("X003", "0.125", "0.375");
    vm.call("listen", &[]).unwrap();
    let ordered = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(true));
    // The order itself changes nothing, and neither the order nor the opposite order is
    // accepted while the change is pending (`0xb4adf0`: the ability's busy counter).
    assert_eq!(rawcode(&mut vm, u), *b"u001");
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.call("unburrow_order", &[]).unwrap(), Value::Bool(false));
    advance(&mut vm, ordered + 1.4);
    assert_eq!(rawcode(&mut vm, u), *b"u001");
    assert_eq!(vm.global("Finishes"), Value::Int(0));
    advance(&mut vm, ordered + 1.46);
    assert_eq!(rawcode(&mut vm, u), *b"u002");
    assert_eq!(vm.global("Finishes"), Value::Int(1));
    assert_eq!(vm.global("Ends"), Value::Int(1));
    assert_eq!(vm.host().inner.test_morph_active_count(), 0);
    // Unburrow runs the same delay and changes the form back at its end.
    let ordered = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("unburrow_order", &[]).unwrap(), Value::Bool(true));
    assert_eq!(rawcode(&mut vm, u), *b"u002");
    advance(&mut vm, ordered + 1.4);
    assert_eq!(rawcode(&mut vm, u), *b"u002");
    advance(&mut vm, ordered + 1.46);
    assert_eq!(rawcode(&mut vm, u), *b"u001");
    assert!(vm.host().inner.executed_unit(u).unwrap().morph.is_none());
    assert_eq!(vm.global("Finishes"), Value::Int(2));
    assert_eq!(vm.call("unburrow_order", &[]).unwrap(), Value::Bool(false));
}

#[test]
fn burrow_swap_keeps_the_unit_and_its_pools() {
    let (mut vm, u) = open_with("X003", "0.125", "0.375");
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, 1.5);
    let after = vm.host().inner.executed_unit(u).unwrap();
    assert_eq!(after.owner, before.owner);
    assert_eq!(after.position, before.position);
    assert_eq!(after.vitals.unwrap().life, before.vitals.unwrap().life);
    assert_eq!(
        after.vitals.unwrap().max_life,
        before.vitals.unwrap().max_life
    );
    assert_eq!(after.vitals.unwrap().mana, before.vitals.unwrap().mana);
    assert_eq!(after.definition_revision, before.definition_revision + 1);
}

#[test]
fn burrow_swap_takes_the_new_forms_ability_list_and_life_regeneration_and_back() {
    let (mut vm, u) = open_with("X003", "0.125", "0.375");
    let levels = |vm: &mut Vm| {
        vm.host()
            .inner
            .executed_unit(u)
            .unwrap()
            .abilities
            .as_ref()
            .unwrap()
            .levels()
            .map(|(alias, _)| alias)
            .collect::<Vec<_>>()
    };
    let regen = |vm: &mut Vm| {
        vm.host()
            .inner
            .executed_unit(u)
            .unwrap()
            .vitals
            .unwrap()
            .regen
            .life_per_second
    };
    let (base_abilities, base_regen) = (levels(&mut vm), regen(&mut vm));
    assert_eq!(base_abilities, [*b"X003"]);
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(true));
    assert_eq!(regen(&mut vm), base_regen);
    advance(&mut vm, 1.5);
    assert_eq!(levels(&mut vm), [*b"X003", *b"X001"]);
    assert_eq!(regen(&mut vm), 7.);
    assert_eq!(vm.call("unburrow_order", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, 3.);
    assert_eq!(levels(&mut vm), base_abilities);
    assert_eq!(regen(&mut vm), base_regen);
}

#[test]
fn burrow_of_a_unit_that_is_not_the_source_form_runs_the_phase_without_a_type_change() {
    // `0xbaf580` changes the type only when it equals the source form of the direction; a
    // custom type such as UndeadX04's `u000` takes the order, the delay and the events.
    let (mut vm, u) = open_with("X005", "0.125", "0.375");
    vm.call("listen", &[]).unwrap();
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(false));
    advance(&mut vm, 1.4);
    assert_eq!(vm.global("Finishes"), Value::Int(0));
    advance(&mut vm, 1.5);
    assert_eq!(vm.global("Finishes"), Value::Int(1));
    let record = vm.host().inner.executed_unit(u).unwrap();
    assert_eq!(record.definition, before.definition);
    assert_eq!(record.definition_revision, before.definition_revision);
    assert_eq!(
        record.morph.as_ref().map(|m| m.phase),
        Some(crate::playercolor_host::MorphPhase::Alternate)
    );
    assert_eq!(vm.call("unburrow_order", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, 3.);
    assert!(vm.host().inner.executed_unit(u).unwrap().morph.is_none());
    assert_eq!(vm.global("Finishes"), Value::Int(2));
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(true));
}

#[test]
fn burrow_class_form_with_an_air_form_is_refused_before_any_change() {
    let (mut vm, u) = open_with("X006", "0.125", "0.375");
    let before = vm.host().inner.executed_unit(u).unwrap().clone();
    let error = vm.call("burrow_order", &[]).unwrap_err().to_string();
    assert!(
        error.contains("air movement and no immediate form change"),
        "{error}"
    );
    assert_eq!(vm.host().inner.executed_unit(u).unwrap(), &before);
}

#[test]
fn burrow_pending_swap_is_dropped_when_the_unit_is_removed() {
    let (mut vm, u) = open_with("X003", "0.125", "0.375");
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(true));
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(u)])
        .unwrap();
    advance(&mut vm, 2.);
    assert_eq!(vm.host().inner.test_morph_active_count(), 0);
}

#[test]
fn a_rooted_slot_without_an_attack_enables_nothing_and_is_not_refused() {
    // A building without attack 1 under `DataA=1` has no weapon, and `unroot` (`DataB=2`) arms
    // the attack 2 it has.
    let (mut vm, u) = open_with_attack("X002", "0.125", "0.375", false);
    assert_eq!(weapon_of(&mut vm, u), None);
    assert_eq!(vm.call("unroot_order", &[]).unwrap(), Value::Bool(true));
    assert_eq!(weapon_of(&mut vm, u).map(|w| w.range), Some(700.));
    // `DataA=2` roots it with attack 2.
    let (mut vm, u) = open_with_attack("X004", "0.125", "0.375", false);
    assert_eq!(weapon_of(&mut vm, u).map(|w| w.range), Some(700.));
}

#[test]
fn weapons_on_of_attack_two_alone_arms_attack_two() {
    let (mut vm, u) = open_with_attack("X001", "0.125", "0.375", false);
    let weapon = weapon_of(&mut vm, u).expect("attack 2 is enabled");
    assert_eq!(weapon.range, 700.);
    assert_eq!(weapon.splash.map(|s| s.quarter), Some(125.));
}

#[test]
fn pausing_a_burrowing_unit_does_not_stop_the_form_timer() {
    // `PauseUnit` suspends ability timers only through the ability pause virtual (`0xbacde0`),
    // which stops the `0xd01db` duration timer; the delayed swap timer `0xd01c4` (`0xbafb00`)
    // keeps running, so the form changes `Dur` after the order whether or not the unit is paused.
    let (mut vm, u) = open_with("X003", "0.125", "0.375");
    vm.call("listen", &[]).unwrap();
    let ordered = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("burrow_order", &[]).unwrap(), Value::Bool(true));
    advance(&mut vm, ordered + 0.5);
    let pause = |vm: &mut Vm, on: bool| {
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Handle(u), Value::Bool(on)])
            .unwrap();
    };
    pause(&mut vm, true);
    assert!(
        vm.host()
            .inner
            .executed_unit(u)
            .unwrap()
            .paused_since
            .is_some()
    );
    advance(&mut vm, ordered + 1.4);
    assert_eq!(rawcode(&mut vm, u), *b"u001");
    advance(&mut vm, ordered + 1.46);
    assert_eq!(rawcode(&mut vm, u), *b"u002");
    assert_eq!(vm.global("Finishes"), Value::Int(1));
    assert!(
        vm.host()
            .inner
            .executed_unit(u)
            .unwrap()
            .paused_since
            .is_some()
    );
    pause(&mut vm, false);
    assert!(
        vm.host()
            .inner
            .executed_unit(u)
            .unwrap()
            .paused_since
            .is_none()
    );
    // Unpausing mid-phase leaves the timer alone too.
    let ordered = vm.host().inner.timer_game_time().unwrap();
    assert_eq!(vm.call("unburrow_order", &[]).unwrap(), Value::Bool(true));
    pause(&mut vm, true);
    advance(&mut vm, ordered + 0.7);
    pause(&mut vm, false);
    advance(&mut vm, ordered + 1.46);
    assert_eq!(rawcode(&mut vm, u), *b"u001");
}
