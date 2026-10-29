//! Slow and Polymorph on synthetic non-stock tables: the numbers below are fixtures, not the
//! install's values.
use super::*;
use pcx_objects::editor::defaults::{DefaultTables, Identity};
use pcx_objects::editor::{Overlay, SlkId};
use pcx_script::{HandleId, Value};
use pcx_sim::world::{ObjectCatalog, ResolutionProblem};

const MISC: &str = "[Misc]\nMinUnitSpeed=10\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300\n";
const SLOW_ORDER: &str = "alpha";
const POLYMORPH_ORDER: &str = "delta";
const CONVERSION_ORDER: &str = "darkconversion";
const CONVERSION_SECONDS: f64 = 6.0;
const MOVEMENT_FACTOR: f32 = 0.5;
const ATTACK_FACTOR: f32 = 0.25;
const SLOW_SECONDS: f64 = 20.0;
const SLOW_HERO_SECONDS: f64 = 5.0;
const SLOW_COOLDOWN: f64 = 2.0;
const SLOW_COST: f32 = 50.0;
const POLYMORPH_SECONDS: f64 = 12.0;
const POLYMORPH_COST: f32 = 70.0;
const CAST_POINT: f64 = 0.25;

fn sylk(headers: &[&str], rows: &[Vec<String>]) -> Vec<u8> {
    let mut text = String::from("ID;PWXL;N;E\r\n");
    for (y, row) in std::iter::once(headers.iter().map(|cell| (*cell).to_owned()).collect())
        .chain(rows.iter().cloned())
        .enumerate()
    {
        for (x, value) in row.iter().enumerate() {
            text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
        }
    }
    text.push_str("E\r\n");
    text.into_bytes()
}

fn cells(headers: &[&str], values: &[(&str, &str)]) -> Vec<String> {
    headers
        .iter()
        .map(|header| {
            values
                .iter()
                .find(|(name, _)| name == header)
                .map(|(_, value)| (*value).to_owned())
                .unwrap_or_else(|| "0".to_owned())
        })
        .collect()
}

#[derive(Clone)]
struct Config {
    /// `abilList` of every fixture unit.
    abilities: &'static str,
    /// Replacement `AbilityData` cells of the Slow row.
    slow: Vec<(&'static str, &'static str)>,
    polymorph: Vec<(&'static str, &'static str)>,
    /// Replacement `AbilityData` cells of the Dark Conversion row.
    conversion: Vec<(&'static str, &'static str)>,
    /// `data` ordinals of `Slo1` and `Slo2`.
    ordinals: [&'static str; 2],
    overlay: Overlay,
    /// Install the sheep catalog, models and animation rules that Polymorph needs.
    forms: bool,
    /// `movetp` of every unit except the sheep.
    movement: &'static str,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            abilities: "Aslo",
            slow: Vec::new(),
            polymorph: Vec::new(),
            conversion: Vec::new(),
            ordinals: ["1", "2"],
            overlay: Overlay::default(),
            forms: false,
            movement: "foot",
        }
    }
}

fn tables(config: &Config) -> DefaultTables {
    let headers = [
        "alias",
        "code",
        "levels",
        "reqLevel",
        "levelSkip",
        "hero",
        "targs1",
        "Cast1",
        "Dur1",
        "HeroDur1",
        "Cool1",
        "Cost1",
        "Rng1",
        "DataA1",
        "DataB1",
        "DataC1",
        "DataD1",
        "DataE1",
        "BuffID1",
        "UnitID1",
    ];
    let with = |base: &[(&'static str, &'static str)], edits: &[(&'static str, &'static str)]| {
        let mut values = base.to_vec();
        for (name, value) in edits {
            values.retain(|(existing, _)| existing != name);
            values.push((name, value));
        }
        values
    };
    let seconds = |value: f64| -> &'static str { Box::leak(value.to_string().into_boxed_str()) };
    let slow = with(
        &[
            ("alias", "Aslo"),
            ("code", "Aslo"),
            ("levels", "1"),
            ("targs1", "air,ground,enemy"),
            ("Dur1", seconds(SLOW_SECONDS)),
            ("HeroDur1", seconds(SLOW_HERO_SECONDS)),
            ("Cool1", seconds(SLOW_COOLDOWN)),
            ("Cost1", seconds(f64::from(SLOW_COST))),
            ("Rng1", "100"),
            ("DataA1", seconds(f64::from(MOVEMENT_FACTOR))),
            ("DataB1", seconds(f64::from(ATTACK_FACTOR))),
            ("BuffID1", "Bslo"),
        ],
        &config.slow,
    );
    let polymorph = with(
        &[
            ("alias", "Aply"),
            ("code", "Aply"),
            ("levels", "1"),
            ("targs1", "air,ground,nonhero,enemy,organic,neutral"),
            ("Dur1", seconds(POLYMORPH_SECONDS)),
            ("HeroDur1", seconds(POLYMORPH_SECONDS)),
            ("Cool1", "3"),
            ("Cost1", seconds(f64::from(POLYMORPH_COST))),
            ("Rng1", "100"),
            ("DataA1", "5"),
            ("DataB1", "u002"),
            ("DataC1", "u003"),
            ("DataD1", "u004"),
            ("DataE1", "u005"),
            ("BuffID1", "Bply"),
        ],
        &config.polymorph,
    );
    let conversion = with(
        &[
            ("alias", "ANdc"),
            ("code", "ANdc"),
            ("levels", "1"),
            ("targs1", "air,ground,organic,nonhero"),
            ("Dur1", seconds(CONVERSION_SECONDS)),
            ("HeroDur1", seconds(CONVERSION_SECONDS)),
            ("Cool1", "0.5"),
            ("Cost1", "0"),
            ("Rng1", "100"),
            ("DataA1", "commoner"),
            ("BuffID1", "Bndc"),
            ("UnitID1", "u001"),
        ],
        &config.conversion,
    );
    let wander = [("alias", "Awan"), ("code", "Awan"), ("levels", "1")];
    let rows = [
        cells(&headers, &slow),
        cells(&headers, &polymorph),
        cells(&headers, &conversion),
        cells(&headers, &wander),
        cells(
            &headers,
            &[("alias", "Zoth"), ("code", "Zoth"), ("levels", "1")],
        ),
        cells(
            &headers,
            &[("alias", "Yher"), ("code", "Yher"), ("levels", "1")],
        ),
    ];
    let meta_headers = [
        "ID",
        "field",
        "slk",
        "index",
        "type",
        "category",
        "minVal",
        "maxVal",
        "data",
        "useSpecific",
    ];
    let meta = |id, kind, data, specific| {
        cells(
            &meta_headers,
            &[
                ("ID", id),
                ("field", "Data"),
                ("slk", "AbilityData"),
                ("index", "-1"),
                ("type", kind),
                ("category", "data"),
                ("minVal", "-10"),
                ("maxVal", "10"),
                ("data", data),
                ("useSpecific", specific),
            ],
        )
    };
    let slow_users = "Aslo,Zoth";
    let polymorph_users = "Aply,Zoth";
    let conversion_users = "ANdc,Zoth";
    let metadata = sylk(
        &meta_headers,
        &[
            meta("Slo1", "unreal", config.ordinals[0], slow_users),
            meta("Slo2", "unreal", config.ordinals[1], slow_users),
            meta("Slo3", "bool", "3", slow_users),
            meta("Ply1", "int", "1", polymorph_users),
            meta("Ply2", "unitList", "2", polymorph_users),
            meta("Ply3", "unitList", "3", polymorph_users),
            meta("Ply4", "unitList", "4", polymorph_users),
            meta("Ply5", "unitList", "5", polymorph_users),
            meta("Ndc1", "unitRace", "1", conversion_users),
            cells(
                &meta_headers,
                &[
                    ("ID", "Ndc2"),
                    ("field", "UnitID"),
                    ("slk", "AbilityData"),
                    ("index", "-1"),
                    ("type", "unitCode"),
                    ("category", "data"),
                    ("data", "0"),
                    ("useSpecific", conversion_users),
                ],
            ),
        ],
    );
    let buff_headers = ["alias", "code"];
    let buffs = sylk(
        &buff_headers,
        &[
            vec!["Bslo".into(), "Bslo".into()],
            vec!["Bply".into(), "Bply".into()],
            vec!["Bndc".into(), "Bndc".into()],
        ],
    );
    let profile = format!(
        "[Aslo]\r\nOrder={SLOW_ORDER}\r\nHotkey=W\r\n[Aply]\r\nOrder={POLYMORPH_ORDER}\r\nHotkey=O\r\n[Bslo]\r\nTargetart=slow.mdl\r\n[Bply]\r\nEffectart=done.mdl\r\n[Bndc]\r\nTargetart=convert.mdl\r\n[ANdc]\r\nOrder={CONVERSION_ORDER}\r\nHotkey=C\r\n"
    );
    DefaultTables::from_bytes(
        Identity {
            install: super::super::source().install,
            edition_version: "fixture".into(),
        },
        &[
            (SlkId::AbilityData.path(), &sylk(&headers, &rows)),
            (SlkId::AbilityMetaData.path(), &metadata),
            (SlkId::AbilityBuffData.path(), &buffs),
        ],
        &[("Units\\HumanAbilityFunc.txt", profile.as_bytes())],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 6,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 256 << 10,
        },
    )
    .expect("synthetic curse tables")
}

/// The sheep `u002` has its own abilities, no weapon and a model apart from every other unit.
struct Sheep(Arc<dyn ObjectCatalog + Send + Sync>, &'static str);

impl ObjectCatalog for Sheep {
    fn resolve(&self, code: [u8; 4]) -> Result<pcx_sim::world::Definition, ResolutionProblem> {
        let mut definition = self.0.resolve(code)?;
        for field in definition.fields.values_mut() {
            if let Some(metadata) = &field.metadata
                && matches!(&metadata.source, FieldSource::Column { slk: SlkId::UnitData, column } if column == "movetp")
            {
                field.value = Ok(FieldValue::Cell(
                    if code == *b"u002" { "foot" } else { self.1 }.into(),
                ));
            }
        }
        if code == *b"u002" {
            for field in definition.fields.values_mut() {
                let Some(metadata) = &field.metadata else {
                    continue;
                };
                if let FieldSource::Column { slk, column } = &metadata.source {
                    match (slk, column.as_str()) {
                        (SlkId::UnitAbilities, "abilList") => {
                            field.value = Ok(FieldValue::Cell("Awan".into()));
                        }
                        (SlkId::UnitWeapons, "weapsOn") => {
                            field.value = Ok(FieldValue::Cell("0".into()));
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(definition)
    }
}

fn host_with(config: &Config) -> ObjectHost {
    let (registry, activity) = super::activity::rules_with_darkconversion();
    let activity = activity
        .with_move_speed_profiles(&[("synthetic game misc", MISC.as_bytes())], 4096)
        .unwrap();
    let (hero_rules, combat_rules) = hero_rules();
    let mut fields = hero_fields("STR");
    fields.push(column(
        b"ubdg",
        SlkId::UnitBalance,
        "isbldg",
        FieldKind::Bool,
        "0",
    ));
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "200",
    ));
    fields.push(column(
        b"Smp0",
        SlkId::UnitWeapons,
        "castpt",
        FieldKind::Unreal,
        "0.25",
    ));
    fields.push(column(
        b"H001",
        SlkId::UnitBalance,
        "HP",
        FieldKind::Int,
        "1000",
    ));
    fields.push(column(
        b"H003",
        SlkId::UnitBalance,
        "mana0",
        FieldKind::Int,
        "300",
    ));
    fields.push(column(
        b"A010",
        SlkId::UnitBalance,
        "type",
        FieldKind::Token,
        "",
    ));
    fields.push(column(
        b"A008",
        SlkId::UnitAbilities,
        "abilList",
        FieldKind::Token,
        config.abilities,
    ));
    fields.push(column(
        b"urac",
        SlkId::UnitData,
        "race",
        FieldKind::Token,
        "commoner",
    ));
    fields.push(column(
        b"ufus",
        SlkId::UnitBalance,
        "fused",
        FieldKind::Int,
        "3",
    ));
    fields.push(column(
        b"ufma",
        SlkId::UnitBalance,
        "fmade",
        FieldKind::Int,
        "0",
    ));
    for (id, field) in &mut fields {
        if id == b"ufus" || id == b"ufma" {
            field.metadata.as_mut().unwrap().declared = "int".into();
        }
    }
    let property_key = crate::unit_animation_rarity::tests::rules().props;
    let mut prop = geometry_field(Ok(FieldValue::Cell("_".into())));
    let meta = prop.metadata.as_mut().unwrap();
    meta.fourcc = *b"mPrp";
    meta.field = property_key.clone();
    meta.kind = FieldKind::Token;
    meta.declared = "stringList".into();
    meta.source = FieldSource::Profile {
        key: property_key,
        index: -1,
        slots: 1,
    };
    fields.push((*b"mPrp", prop));
    let extra = format!(
        "{}\n{}{DECLARATIONS}{HOLY_DECLARATIONS}\nnative SetUnitTimeScale takes unit u, real scale returns nothing\nconstant native GetUnitTypeId takes unit u returns integer\nnative UnitRemoveBuffs takes unit whichUnit, boolean removePositive, boolean removeNegative returns nothing\n",
        enumeration::TRIGGERS,
        super::activity::ORDERS
    );
    let overlay = config.overlay.clone();
    let hero_budget = crate::playercolor_host::HeroBudget {
        abilities_per_unit: 8,
        overlay_objects: overlay.mods.len() + overlay.custom.len(),
        modifications_per_ability: 0,
    };
    let origin = crate::scripts::AssetIdentity {
        source: source().install,
        edition: "fixture".into(),
        version: "fixture".into(),
    };
    let mut h = position_host_admitting(1000, 16, HOLY_MAP, &extra, &fields, |h| h)
        .with_player_abilities(16)
        .unwrap()
        .with_locations()
        .unwrap()
        .with_player_unit_events(8)
        .unwrap()
        .with_unit_states(crate::playercolor_host::UnitStateBudget { max_records: 8 })
        .unwrap()
        .with_player_alliances(crate::playercolor_host::PlayerAllianceBudget { max_records: 8 })
        .unwrap()
        .with_unit_explosions(8)
        .unwrap()
        .with_order_registry(registry)
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap()
        .with_unit_effects()
        .unwrap()
        .with_hero_skills(tables(config), origin, overlay, hero_rules, hero_budget)
        .unwrap();
    h.admit_unit_activity(activity, 65536).unwrap();
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.0,
        mana_per_intelligence: 100.0,
        life_regen_per_strength: 0.0,
        mana_regen_per_intelligence: 0.0,
    })
    .unwrap();
    h.admit_combat(combat_rules, "beta").unwrap();
    h.set_neutral_passive(15);
    if config.forms {
        let base = Arc::new(crate::unit_animation_rarity::tests::model(&[
            ("stand", 0.),
            ("walk", 0.),
            ("spell", 0.5),
        ]));
        let sheep = Arc::new(crate::unit_animation_rarity::tests::model(&[
            ("stand", 0.),
            ("walk", 0.),
        ]));
        h.admit_unit_models(move |definition| {
            Ok(Some(if definition.rawcode == *b"u002" {
                sheep.clone()
            } else {
                base.clone()
            }))
        })
        .unwrap();
        let mut rarity = crate::unit_animation_rarity::tests::rules();
        rarity.image_sha256 = [0; 32];
        h.admit_unit_animation_rarity(Ok(rarity)).unwrap();
        h.construction_test_catalog(Arc::new(Sheep(h.unit_catalog().unwrap(), config.movement)));
    }
    h
}

fn open_with(config: Config) -> (Vm, [HandleId; 4]) {
    let mut h = host_with(&config);
    h.admit_unit_curses().unwrap();
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = |vm: &Vm, name| match vm.global(name) {
        Value::Handle(id) => id,
        _ => panic!("{name}"),
    };
    let ids = [id(&vm, "H"), id(&vm, "F"), id(&vm, "E"), id(&vm, "G")];
    (vm, ids)
}

fn open() -> (Vm, [HandleId; 4]) {
    open_with(Config::default())
}

fn order(vm: &mut Vm, caster: HandleId, name: &str, target: HandleId) -> bool {
    match vm
        .host()
        .inner
        .native(
            "IssueTargetOrder",
            &[
                Value::Handle(caster),
                Value::Str(name.into()),
                Value::Handle(target),
            ],
        )
        .unwrap()
    {
        Value::Bool(accepted) => accepted,
        other => panic!("order returned {other:?}"),
    }
}

fn mana(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().mana
}

fn life(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().life
}

fn set_state(vm: &mut Vm, unit: HandleId, state: &str, value: f32) {
    let state = vm.global(state);
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(unit), state, Value::Real(value)],
        )
        .unwrap();
}

fn speed(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host()
        .inner
        .executed_unit(unit)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .speed()
        .expect("movable fixture unit")
}

fn clamped(base: f32, factor: f32) -> f32 {
    (base * (1.0 - factor)).clamp(10.0, 400.0)
}

fn buffs(vm: &mut Vm, unit: HandleId) -> Vec<[u8; 4]> {
    vm.host()
        .inner
        .unit_buffs(unit)
        .unwrap()
        .iter()
        .map(|buff| buff.rawcode())
        .collect()
}

fn spawn(vm: &mut Vm, player: i32, kind: &[u8; 4], x: f32) -> HandleId {
    let owner = vm
        .host()
        .inner
        .native("Player", &[Value::Int(player)])
        .unwrap();
    match vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*kind)),
                Value::Real(x),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    {
        Value::Handle(id) => id,
        other => panic!("spawn returned {other:?}"),
    }
}

fn type_id(vm: &mut Vm, unit: HandleId) -> i32 {
    match vm
        .host()
        .inner
        .native("GetUnitTypeId", &[Value::Handle(unit)])
        .unwrap()
    {
        Value::Int(id) => id,
        other => panic!("type returned {other:?}"),
    }
}

fn current_order(vm: &mut Vm, unit: HandleId) -> i32 {
    vm.host().inner.current_unit_order(unit).unwrap()
}

fn effect_count(vm: &mut Vm) -> usize {
    vm.host().inner.test_chain_effect_count()
}

#[test]
fn slow_applies_the_data_factors_at_the_cast_point_and_expires() {
    let (mut vm, [caster, _, enemy, _]) = open();
    vm.call("prime", &[]).unwrap();
    let base = speed(&mut vm, enemy);
    assert!(
        clamped(base, MOVEMENT_FACTOR) > 10.0,
        "fixture speed {base}"
    );
    let effects = effect_count(&mut vm);
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    let slow = match vm
        .host()
        .inner
        .native("OrderId", &[Value::Str(SLOW_ORDER.into())])
        .unwrap()
    {
        Value::Int(id) => id,
        other => panic!("{other:?}"),
    };
    assert_eq!(current_order(&mut vm, caster), slow, "order while casting");
    assert_eq!(
        mana(&mut vm, caster),
        200.0,
        "mana is paid at the cast point"
    );
    assert_eq!(speed(&mut vm, enemy), base);
    step(&mut vm, CAST_POINT - 0.05);
    assert_eq!(speed(&mut vm, enemy), base);
    step(&mut vm, CAST_POINT + 0.05);
    assert_eq!(mana(&mut vm, caster), 200.0 - SLOW_COST);
    assert_eq!(speed(&mut vm, enemy), clamped(base, MOVEMENT_FACTOR));
    assert_eq!(vm.host().inner.curse_attack_penalty(enemy), ATTACK_FACTOR);
    assert_eq!(buffs(&mut vm, enemy), [*b"Bslo"]);
    assert_eq!(effect_count(&mut vm), effects + 1, "Targetart of the buff");
    step(&mut vm, SLOW_SECONDS);
    assert_eq!(
        buffs(&mut vm, enemy),
        [*b"Bslo"],
        "still slowed before the duration"
    );
    step(&mut vm, CAST_POINT + SLOW_SECONDS + 0.1);
    assert_eq!(speed(&mut vm, enemy), base);
    assert_eq!(vm.host().inner.curse_attack_penalty(enemy), 0.0);
    assert!(buffs(&mut vm, enemy).is_empty());
}

#[test]
fn slow_spends_and_cools_down_and_refuses_ineligible_targets() {
    let (mut vm, [caster, friend, enemy, _]) = open();
    vm.call("prime", &[]).unwrap();
    assert!(!order(&mut vm, caster, SLOW_ORDER, friend), "enemy only");
    assert!(!order(&mut vm, caster, SLOW_ORDER, caster));
    set_state(&mut vm, caster, "UNIT_STATE_MANA", SLOW_COST - 1.0);
    assert!(!order(&mut vm, caster, SLOW_ORDER, enemy), "short of mana");
    set_state(&mut vm, caster, "UNIT_STATE_MANA", 200.0);
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, 0.4);
    assert_eq!(mana(&mut vm, caster), 200.0 - SLOW_COST);
    assert!(!order(&mut vm, caster, SLOW_ORDER, enemy), "cooling down");
    step(&mut vm, 0.25 + SLOW_COOLDOWN + 0.1);
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy), "cooldown over");
}

#[test]
fn slow_recast_refreshes_instead_of_compounding_and_heroes_use_the_hero_duration() {
    let (mut vm, [caster, _, enemy, _]) = open();
    vm.call("prime", &[]).unwrap();
    let base = speed(&mut vm, enemy);
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, 0.4);
    let second = 0.4 + SLOW_COOLDOWN + 0.1;
    step(&mut vm, second);
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, second + 0.4);
    assert_eq!(speed(&mut vm, enemy), clamped(base, MOVEMENT_FACTOR));
    assert_eq!(buffs(&mut vm, enemy), [*b"Bslo"]);
    // The first cast would have ended at 20.25 s; the refresh lasts to 20 s after the second.
    step(&mut vm, 0.25 + SLOW_SECONDS + 0.5);
    assert_eq!(speed(&mut vm, enemy), clamped(base, MOVEMENT_FACTOR));
    step(&mut vm, second + 0.25 + SLOW_SECONDS + 0.1);
    assert_eq!(speed(&mut vm, enemy), base);

    let (mut vm, [caster, _, _, _]) = open();
    vm.call("prime", &[]).unwrap();
    let hero = spawn(&mut vm, 5, b"H001", 70.0);
    let hero_base = speed(&mut vm, hero);
    assert!(order(&mut vm, caster, SLOW_ORDER, hero));
    step(&mut vm, 0.4);
    assert_eq!(speed(&mut vm, hero), clamped(hero_base, MOVEMENT_FACTOR));
    step(&mut vm, CAST_POINT + SLOW_HERO_SECONDS + 0.1);
    assert_eq!(speed(&mut vm, hero), hero_base, "HeroDur, not Dur");
}

#[test]
fn slow_walks_into_range_before_casting() {
    let (mut vm, [caster, _, _, _]) = open();
    vm.call("prime", &[]).unwrap();
    let far = spawn(&mut vm, 5, b"u001", 125.0);
    let base = speed(&mut vm, far);
    let start = placed_x(&mut vm, caster);
    assert!(order(&mut vm, caster, SLOW_ORDER, far));
    step(&mut vm, 0.05);
    assert!(
        placed_x(&mut vm, caster) > start,
        "the caster walks toward the target"
    );
    assert_eq!(speed(&mut vm, far), base);
    assert_eq!(mana(&mut vm, caster), 200.0);
    step(&mut vm, 0.5);
    assert_eq!(mana(&mut vm, caster), 200.0 - SLOW_COST);
    assert_eq!(speed(&mut vm, far), clamped(base, MOVEMENT_FACTOR));
    let stopped = placed_x(&mut vm, caster);
    let gap = placed_x(&mut vm, far) - stopped;
    assert!(
        gap <= 100.0 && gap > 90.0,
        "cast from the edge of Rng, not from on top of the target: {gap}"
    );
}

fn placed_x(vm: &mut Vm, unit: HandleId) -> f32 {
    match vm
        .host()
        .inner
        .native("GetUnitX", &[Value::Handle(unit)])
        .unwrap()
    {
        Value::Real(x) => x,
        other => panic!("GetUnitX returned {other:?}"),
    }
}

#[test]
fn a_new_order_cancels_a_pending_cast_without_payment() {
    let (mut vm, [caster, _, enemy, _]) = open();
    vm.call("prime", &[]).unwrap();
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    let moved = vm
        .host()
        .inner
        .native(
            "IssuePointOrder",
            &[
                Value::Handle(caster),
                Value::Str("gamma".into()),
                Value::Real(30.0),
                Value::Real(16.0),
            ],
        )
        .unwrap();
    assert!(matches!(moved, Value::Bool(_)));
    step(&mut vm, 0.6);
    assert_eq!(mana(&mut vm, caster), 200.0);
    assert!(buffs(&mut vm, enemy).is_empty());
}

#[test]
fn removing_negative_buffs_ends_the_slow() {
    let (mut vm, [caster, _, enemy, _]) = open();
    vm.call("prime", &[]).unwrap();
    let base = speed(&mut vm, enemy);
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, 0.4);
    assert_ne!(speed(&mut vm, enemy), base);
    assert!(!vm.host().inner.has_unqualified_motion_buff(enemy));
    vm.host()
        .inner
        .native(
            "UnitRemoveBuffs",
            &[Value::Handle(enemy), Value::Bool(false), Value::Bool(true)],
        )
        .unwrap();
    assert_eq!(speed(&mut vm, enemy), base);
    assert_eq!(vm.host().inner.curse_attack_penalty(enemy), 0.0);
}

#[test]
fn the_script_speed_natives_refuse_while_a_slow_owns_the_speed() {
    let (mut vm, [caster, _, enemy, _]) = open();
    vm.call("prime", &[]).unwrap();
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, 0.4);
    let error = vm
        .host()
        .inner
        .native("GetUnitMoveSpeed", &[Value::Handle(enemy)])
        .unwrap_err();
    assert!(error.contains("speed modifiers"), "{error}");
}

#[test]
fn metadata_ordinals_and_custom_aliases_choose_the_columns() {
    let mut overlay = Overlay::default();
    overlay
        .custom
        .insert("Zslo".into(), ("Aslo".into(), Vec::new()));
    // Swap the roles: the movement factor is DataB and the attack factor DataA.
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        abilities: "Zslo",
        ordinals: ["2", "1"],
        overlay,
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    let base = speed(&mut vm, enemy);
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, 0.4);
    assert_eq!(speed(&mut vm, enemy), clamped(base, ATTACK_FACTOR));
    assert_eq!(vm.host().inner.curse_attack_penalty(enemy), MOVEMENT_FACTOR);
}

#[test]
fn unlearned_and_unbound_orders_are_not_accepted() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        abilities: "_",
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    let levels: Vec<_> = vm
        .host()
        .inner
        .executed_unit(caster)
        .unwrap()
        .abilities
        .as_ref()
        .unwrap()
        .levels()
        .collect();
    assert!(
        !order(&mut vm, caster, SLOW_ORDER, enemy),
        "not learned {levels:?}"
    );
}

#[test]
fn a_nonzero_casting_time_and_a_wrong_metadata_class_refuse_loudly() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        slow: vec![("Cast1", "1.5")],
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    let error = vm
        .host()
        .inner
        .native(
            "IssueTargetOrder",
            &[
                Value::Handle(caster),
                Value::Str(SLOW_ORDER.into()),
                Value::Handle(enemy),
            ],
        )
        .unwrap_err();
    assert!(error.contains("Cast1"), "{error}");
    assert_eq!(mana(&mut vm, caster), 200.0);
}

#[test]
fn the_spell_effect_event_names_the_ability_and_target() {
    let (mut vm, [caster, _, enemy, _]) = open();
    vm.call("prime", &[]).unwrap();
    vm.call("listen", &[]).unwrap();
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, 0.4);
    let mut events = Vec::new();
    while let Some(event) = vm.host().inner.take_fired_event() {
        events.push(event);
    }
    let spells: Vec<_> = events
        .iter()
        .filter(|event| event.subject.spell_target.is_some())
        .collect();
    assert_eq!(spells.len(), 1);
    assert_eq!(spells[0].subject.spell_target, Some(enemy));
    assert_eq!(spells[0].subject.unit, Some(caster));
    assert_eq!(
        spells[0].subject.spell_ability,
        Some(i32::from_be_bytes(*b"Aslo"))
    );
}

#[test]
fn polymorph_replaces_the_unit_keeps_life_and_armor_and_restores_it() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        abilities: "Aslo,Aply",
        forms: true,
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    set_state(&mut vm, enemy, "UNIT_STATE_LIFE", 321.0);
    let before = vm.host().inner.executed_unit(enemy).unwrap().clone();
    let original = type_id(&mut vm, enemy);
    let armor = before.combat.as_ref().unwrap().armor;
    assert!(before.combat.as_ref().unwrap().weapon.is_some());
    assert!(order(&mut vm, caster, POLYMORPH_ORDER, enemy));
    step(&mut vm, 0.4);
    assert_eq!(mana(&mut vm, caster), 200.0 - POLYMORPH_COST);
    let sheep = vm.host().inner.executed_unit(enemy).unwrap().clone();
    assert_eq!(type_id(&mut vm, enemy), i32::from_be_bytes(*b"u002"));
    assert_eq!(sheep.definition.rawcode, *b"u002");
    assert_eq!(sheep.definition_revision, before.definition_revision + 1);
    assert_eq!(life(&mut vm, enemy), 321.0, "life is retained");
    assert_eq!(sheep.combat.as_ref().unwrap().armor, armor);
    assert!(
        sheep.combat.as_ref().unwrap().weapon.is_none(),
        "cannot attack"
    );
    assert!(before.animation.as_ref().unwrap().has_sequence("spell"));
    assert!(
        !sheep.animation.as_ref().unwrap().has_sequence("spell"),
        "the sheep model"
    );
    assert_eq!(buffs(&mut vm, enemy), [*b"Bply"]);
    assert_eq!(
        sheep
            .abilities
            .as_ref()
            .unwrap()
            .levels()
            .collect::<Vec<_>>(),
        [(*b"Awan", 1)],
        "the sheep owns the sheep's abilities"
    );
    step(&mut vm, 0.25 + POLYMORPH_SECONDS + 0.1);
    let restored = vm.host().inner.executed_unit(enemy).unwrap().clone();
    assert_eq!(type_id(&mut vm, enemy), original);
    assert_eq!(restored.definition_revision, before.definition_revision + 2);
    assert_eq!(restored.definition, before.definition);
    assert_eq!(restored.abilities, before.abilities);
    assert_eq!(
        restored.combat.as_ref().unwrap().weapon,
        before.combat.as_ref().unwrap().weapon
    );
    assert_eq!(
        restored.activity.as_ref().unwrap().speed(),
        before.activity.as_ref().unwrap().speed()
    );
    assert_eq!(life(&mut vm, enemy), 321.0);
    assert!(buffs(&mut vm, enemy).is_empty());
}

#[test]
fn polymorph_refuses_heroes_friends_and_unready_casts_and_refuses_loudly_where_unmodelled() {
    let (mut vm, [caster, friend, enemy, _]) = open_with(Config {
        abilities: "Aply",
        forms: true,
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    let hero = spawn(&mut vm, 5, b"H001", 70.0);
    assert!(!order(&mut vm, caster, POLYMORPH_ORDER, hero), "nonhero");
    assert!(!order(&mut vm, caster, POLYMORPH_ORDER, friend));
    assert_eq!(mana(&mut vm, caster), 200.0);
    assert!(order(&mut vm, caster, POLYMORPH_ORDER, enemy));
    step(&mut vm, 0.4);
    assert!(!order(&mut vm, caster, POLYMORPH_ORDER, enemy), "cooldown");
    // A second caster refreshes the buff duration of the sheep instead of morphing again.
    let other = spawn(&mut vm, 4, b"H001", 30.0);
    set_state(&mut vm, other, "UNIT_STATE_MANA", 200.0);
    let revision = vm
        .host()
        .inner
        .executed_unit(enemy)
        .unwrap()
        .definition_revision;
    assert!(order(&mut vm, other, POLYMORPH_ORDER, enemy));
    step(&mut vm, 0.9);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(enemy)
            .unwrap()
            .definition_revision,
        revision
    );
}

#[test]
fn polymorph_of_a_slowed_unit_and_slow_of_a_polymorphed_unit_refuse_loudly() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        abilities: "Aslo,Aply",
        forms: true,
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, 0.4);
    let other = spawn(&mut vm, 4, b"H001", 30.0);
    set_state(&mut vm, other, "UNIT_STATE_MANA", 200.0);
    assert!(order(&mut vm, other, POLYMORPH_ORDER, enemy));
    let failure = advance_to_failure(&mut vm, 0.4);
    assert!(failure.contains("slowed unit is not modelled"), "{failure}");
}

#[test]
fn admission_refuses_metadata_that_does_not_describe_the_class() {
    let mut host = host_with(&Config {
        ordinals: ["0", "2"],
        ..Config::default()
    });
    let error = host.admit_unit_curses().unwrap_err().to_string();
    assert!(error.contains("data ordinal"), "{error}");
}

#[test]
fn rows_that_share_the_metadata_but_not_the_class_are_not_bound() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        abilities: "Zoth",
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    assert!(vm.host().inner.curse_alias_is_bound(*b"Aslo"));
    assert!(vm.host().inner.curse_alias_is_bound(*b"Aply"));
    assert!(!vm.host().inner.curse_alias_is_bound(*b"Zoth"));
    assert!(!order(&mut vm, caster, SLOW_ORDER, enemy));
    assert_eq!(mana(&mut vm, caster), 200.0);
}

#[test]
fn hover_targets_take_the_ground_form_and_air_targets_refuse_loudly() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        abilities: "Aply",
        forms: true,
        movement: "hover",
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    assert!(order(&mut vm, caster, POLYMORPH_ORDER, enemy));
    step(&mut vm, 0.4);
    assert_eq!(type_id(&mut vm, enemy), i32::from_be_bytes(*b"u002"));

    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        abilities: "Aply",
        forms: true,
        movement: "fly",
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    assert!(order(&mut vm, caster, POLYMORPH_ORDER, enemy));
    let failure = advance_to_failure(&mut vm, 0.4);
    assert!(failure.contains("only the ground form is"), "{failure}");
    assert_eq!(type_id(&mut vm, enemy), i32::from_be_bytes(*b"u001"));
}

/// Steps from the current time for one second and returns the first host failure.
fn advance_to_failure(vm: &mut Vm, from: f64) -> String {
    let h = &mut vm.host().inner;
    for tick in 1..=64 {
        let until = from + f64::from(tick) / 64.0;
        if h.timer_game_time().unwrap() < until {
            let next = h.timer_game_time().unwrap() + 1.0 / 64.0;
            h.next_timer_expiry(next, &mut 100).unwrap();
            if let Err(error) = h.advance_unit_activity(next, &mut 65536) {
                return error.to_string();
            }
        }
    }
    panic!("no failure");
}

fn hide(vm: &mut Vm, unit: HandleId) {
    vm.host()
        .inner
        .native("ShowUnit", &[Value::Handle(unit), Value::Bool(false)])
        .unwrap();
}

fn is_removed(vm: &mut Vm, unit: HandleId) -> bool {
    vm.host().inner.executed_unit(unit).is_err()
}

#[test]
fn dark_conversion_replaces_the_target_after_the_duration_for_the_caster_owner() {
    let (mut vm, [caster, _, target, _]) = open_with(Config {
        abilities: "ANdc",
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    hide(&mut vm, caster);
    let event = vm.global("EVENT_UNIT_SUMMON");
    vm.call("hero_listen_unit", &[Value::Handle(caster), event])
        .unwrap();
    let before = vm.host().inner.executed_units().count();
    let effects = effect_count(&mut vm);
    assert!(order(&mut vm, caster, CONVERSION_ORDER, target));
    step(&mut vm, CAST_POINT - 0.05);
    assert!(buffs(&mut vm, target).is_empty(), "before the cast point");
    step(&mut vm, CAST_POINT + 0.05);
    assert_eq!(buffs(&mut vm, target), [*b"Bndc"]);
    assert_eq!(effect_count(&mut vm), effects + 1, "Targetart of the buff");
    assert!(!is_removed(&mut vm, target), "alive until Dur");
    assert_eq!(vm.host().inner.executed_units().count(), before);
    step(&mut vm, CAST_POINT + CONVERSION_SECONDS - 0.5);
    assert!(!is_removed(&mut vm, target));
    step(&mut vm, CAST_POINT + CONVERSION_SECONDS + 0.5);
    assert!(is_removed(&mut vm, target));
    let (created, _) = vm.host().inner.executed_units().last().unwrap();
    assert_ne!(created, target);
    let record = vm.host().inner.executed_unit(created).unwrap().clone();
    let caster_owner = vm.host().inner.executed_unit(caster).unwrap().owner;
    assert_eq!(record.owner, caster_owner, "the caster's owner");
    assert_eq!(
        record.position,
        Some([56.0, 16.0]),
        "where the target stood"
    );
    let event = vm.host().inner.take_fired_event().expect("summon event");
    assert!(vm.host().inner.begin_fired_event(&event).unwrap().is_some());
    assert_eq!(
        vm.host().inner.native("GetSummoningUnit", &[]).unwrap(),
        Value::Handle(caster)
    );
    assert_eq!(
        vm.host().inner.native("GetSummonedUnit", &[]).unwrap(),
        Value::Handle(created)
    );
    vm.host().inner.finish_fired_event();
}

#[test]
fn dark_conversion_refuses_a_target_of_another_race_and_a_hero() {
    let (mut vm, [caster, _, target, _]) = open_with(Config {
        abilities: "ANdc",
        conversion: vec![("DataA1", "undead")],
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    assert!(!order(&mut vm, caster, CONVERSION_ORDER, target), "race");
    let (mut vm, [caster, _, _, _]) = open_with(Config {
        abilities: "ANdc",
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    let hero = spawn(&mut vm, 5, b"H001", 70.0);
    assert!(!order(&mut vm, caster, CONVERSION_ORDER, hero), "nonhero");
}

#[test]
fn dark_conversion_is_cancelled_when_the_buff_is_removed() {
    let (mut vm, [caster, _, target, _]) = open_with(Config {
        abilities: "ANdc",
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    assert!(order(&mut vm, caster, CONVERSION_ORDER, target));
    step(&mut vm, CAST_POINT + 0.05);
    vm.host()
        .inner
        .native(
            "UnitRemoveBuffs",
            &[Value::Handle(target), Value::Bool(true), Value::Bool(true)],
        )
        .unwrap();
    step(&mut vm, CONVERSION_SECONDS + 1.0);
    assert!(!is_removed(&mut vm, target));
    assert!(buffs(&mut vm, target).is_empty());
}
