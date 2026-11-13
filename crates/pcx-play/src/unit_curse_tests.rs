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
    map: &'static str,
    script: &'static str,
    heading: bool,
    mana_minimum: Option<f32>,
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
            map: HOLY_MAP,
            script: "",
            heading: false,
            mana_minimum: Some(0.),
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
    let mut activity = activity
        .with_move_speed_profiles(&[("synthetic game misc", MISC.as_bytes())], 4096)
        .unwrap();
    let heading_numeric = config.heading.then(|| {
        let (image, origin, numeric, budget) =
            crate::unit_activity::heading::tests::selected_numeric_image();
        let orders = crate::campaign_client::order_registry(&image, origin.clone()).unwrap();
        let admitted = crate::campaign_client::unit_activity(&image, origin, &orders)
            .unwrap()
            .with_heading(&image, &numeric, budget)
            .unwrap();
        let mut heading = admitted.heading.unwrap();
        // Synthetic request cadence and readiness window exercise the called route without fitting native phase.
        heading.period = 0.125;
        heading.window = 0.1875;
        activity.heading = Some(heading);
        numeric.with_native_game_clock_fixture(crate::native_game_clock::Rules {
            simple: crate::autocast_rules::clock::Rules {
                origin: 0.,
                cycle: 2.,
                minimum: 0.00390625,
                epsilon: 0.0078125,
            },
            numerator: 1,
            denominator: 32,
            step: 0.03125_f32.to_bits(),
            wrap_tolerance: 0.00001_f32.to_bits(),
            frame: crate::native_game_clock::frame::Rules {
                unsigned_correction: [0f64.to_bits(), 4294967296f64.to_bits()],
                scale: 0.125f32.to_bits(),
                chunk: 1f32.to_bits(),
            },
            sleep: crate::native_game_clock::sleep::Rules {
                script: crate::native_game_clock::Domain::Real,
                ai: crate::native_game_clock::Domain::Game,
            },
            turn: crate::native_game_clock::turn::Rules {
                quantum_ms: 25,
                credit_cap: 4000,
                speed_steps: Arc::from([1, 2, 3]),
                scale: 1f32.to_bits(),
                modifier: 1,
                elapsed_numerator: 1,
                elapsed_denominator: 1,
                initial_carry: 0,
            },
        })
    });
    let (mut hero_rules, combat_rules) = hero_rules();
    hero_rules.autocast = Ok(Arc::new(crate::autocast_rules::Rules {
        origin: hero_rules.binding.origin.clone(),
        image_sha256: hero_rules.binding.image_sha256,
        classes: vec![],
        groups: vec![],
        alarm_code: None,
        mana_minimum: config.mana_minimum,
        clock: None,
        slow: Some(crate::autocast_rules::slow::Rules {
            class: *b"Aslo",
            data_column: "DataC".into(),
            empty_data: 0.,
            attack_victim_alliance: 47,
        }),
    }));
    let mut fields = hero_fields("STR");
    if let Some(profile) = activity.heading.as_ref().and_then(|h| h.profile.as_ref()) {
        fields.push(column(
            b"Shtn",
            SlkId::UnitData,
            &profile.turn_column,
            FieldKind::Real,
            "0.5",
        ));
        fields.push(column(
            b"Shwn",
            SlkId::UnitData,
            &profile.window_column,
            FieldKind::Real,
            "53",
        ));
        fields.push(column(
            b"Shsi",
            SlkId::UnitData,
            &profile.selector_column,
            FieldKind::Int,
            "5",
        ));
    }
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
        b"Smb0",
        SlkId::UnitWeapons,
        "castbsw",
        FieldKind::Unreal,
        "0.375",
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
        b"A011",
        SlkId::UnitAbilities,
        "auto",
        FieldKind::Token,
        "_",
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
        "{}\n{}{DECLARATIONS}{HOLY_DECLARATIONS}\nnative SetUnitTimeScale takes unit u, real scale returns nothing\nconstant native GetUnitTypeId takes unit u returns integer\nnative UnitRemoveBuffs takes unit whichUnit, boolean removePositive, boolean removeNegative returns nothing\nglobals\nconstant unitevent EVENT_UNIT_SPELL_FINISH=ConvertUnitEvent(7901)\nconstant playerunitevent EVENT_PLAYER_UNIT_SPELL_FINISH=ConvertPlayerUnitEvent(-7902)\nendglobals\n",
        enumeration::TRIGGERS,
        super::activity::ORDERS
    );
    let extra = format!("{extra}\n{}", config.script);
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
    let mut h = position_host_admitting(1000, 16, config.map, &extra, &fields, |h| h)
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
    if let Some(numeric) = heading_numeric {
        h = h.with_script_numeric(numeric);
        h.admit_native_game_clock().unwrap();
    }
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

const HEADING_CALLBACKS: &str = r#"
native GetPlayerNeutralPassive takes nothing returns integer
native GetPlayerNeutralAggressive takes nothing returns integer
native GetBJMaxPlayers takes nothing returns integer
native GetBJPlayerNeutralVictim takes nothing returns integer
native GetBJPlayerNeutralExtra takes nothing returns integer
native SetUnitPosition takes unit u, real x, real y returns nothing
constant native GetUnitFacing takes unit u returns real
native PauseTimer takes timer t returns nothing
native ResumeTimer takes timer t returns nothing
native StartCampaignAI takes player p, string path returns nothing
globals
integer HeadingEvents=0
real HeadingChannelFace=-1.0
real HeadingEffectMana=-1.0
integer HeadingEffectBuff=-1
boolean HeadingFail=false
timer HeadingPaused=null
timer HeadingOneShot=null
integer HeadingPausedCalls=0
integer HeadingOneShotCalls=0
integer HeadingTimedCalls=0
endglobals
function HeadingArbitrationSetup takes nothing returns nothing
set HeadingPaused=CreateTimer()
call TimerStart(HeadingPaused,4.0,false,null)
set HeadingOneShot=CreateTimer()
endfunction
function HeadingObserve takes nothing returns nothing
set HeadingChannelFace=TimerGetElapsed(HeadingPaused)
set HeadingFail=GetExpiredTimer()==HeadingOneShot
endfunction
function HeadingStartAi takes nothing returns nothing
call StartCampaignAI(Player(0),"heading.ai")
endfunction
function HeadingPausedFire takes nothing returns nothing
set HeadingPausedCalls=HeadingPausedCalls+1
endfunction
function HeadingOneShotFire takes nothing returns nothing
set HeadingOneShotCalls=HeadingOneShotCalls+1
endfunction
function HeadingTimedFire takes nothing returns nothing
set HeadingTimedCalls=HeadingTimedCalls+1
endfunction
function HeadingTimersStart takes nothing returns nothing
local trigger t=CreateTrigger()
set HeadingPaused=CreateTimer()
call TimerStart(HeadingPaused,0.5,false,function HeadingPausedFire)
call PauseTimer(HeadingPaused)
set HeadingOneShot=CreateTimer()
call TimerStart(HeadingOneShot,0.1875,false,function HeadingOneShotFire)
call TriggerRegisterTimerEvent(t,0.25,false)
call TriggerAddAction(t,function HeadingTimedFire)
endfunction
function HeadingTimersResume takes nothing returns nothing
call ResumeTimer(HeadingPaused)
endfunction
function HeadingKillTarget takes nothing returns nothing
call KillUnit(E)
endfunction
function HeadingChannel takes nothing returns nothing
set HeadingEvents=HeadingEvents*10+1
set HeadingChannelFace=GetUnitFacing(H)
if HeadingFail then
call HeadingUnsupported()
endif
endfunction
function HeadingFaultOn takes nothing returns nothing
set HeadingFail=true
endfunction
function HeadingCast takes nothing returns nothing
set HeadingEvents=HeadingEvents*10+2
endfunction
function HeadingEffect takes nothing returns nothing
set HeadingEvents=HeadingEvents*10+3
set HeadingEffectMana=GetUnitState(H,UNIT_STATE_MANA)
set HeadingEffectBuff=GetUnitAbilityLevel(E,'Bslo')
endfunction
function HeadingFinish takes nothing returns nothing
set HeadingEvents=HeadingEvents*10+4
endfunction
function HeadingEnd takes nothing returns nothing
set HeadingEvents=HeadingEvents*10+5
endfunction
function HeadingWatch takes nothing returns nothing
local trigger t=CreateTrigger()
call SetUnitPosition(E,16.0,112.0)
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_CHANNEL)
call TriggerAddAction(t,function HeadingChannel)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_CAST)
call TriggerAddAction(t,function HeadingCast)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_EFFECT)
call TriggerAddAction(t,function HeadingEffect)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_FINISH)
call TriggerAddAction(t,function HeadingFinish)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_ENDCAST)
call TriggerAddAction(t,function HeadingEnd)
endfunction
function HeadingIssue takes nothing returns boolean
return IssueTargetOrder(H,"alpha",E)
endfunction
function HeadingStop takes nothing returns boolean
return IssueImmediateOrder(H,"beta")
endfunction
"#;

fn heading_session() -> (
    crate::map_script_session::MapScriptSession<crate::map_startup_host::MapStartupHost>,
    HandleId,
    HandleId,
) {
    heading_session_with(|host| host)
}

fn heading_session_with(
    prepare: impl FnOnce(
        crate::map_startup_host::MapStartupHost,
    ) -> crate::map_startup_host::MapStartupHost,
) -> (
    crate::map_script_session::MapScriptSession<crate::map_startup_host::MapStartupHost>,
    HandleId,
    HandleId,
) {
    use crate::{
        map_script_session::MapScriptSession,
        map_startup_host::MapStartupHost,
        neutral_player::{SelectedMapNeutralHost, tests as neutral},
    };
    let mut host = host_with(&Config {
        heading: true,
        script: HEADING_CALLBACKS,
        ..Config::default()
    });
    host.admit_unit_curses().unwrap();
    let context = Arc::clone(host.player_context().unwrap());
    let world =
        SelectedMapNeutralHost::try_new(context.clone(), neutral::slots_rule(301, -3, 9), host)
            .unwrap();
    let world =
        SelectedMapNeutralHost::try_new(context.clone(), neutral::extra_rule(301, -3, 9), world)
            .unwrap();
    let world =
        SelectedMapNeutralHost::try_new(context.clone(), neutral::victim_rule(301, -3, 9), world)
            .unwrap();
    let world = SelectedMapNeutralHost::try_new(
        context.clone(),
        neutral::bj_max_players_rule(301, -3, 9),
        world,
    )
    .unwrap();
    let world =
        SelectedMapNeutralHost::try_new(context.clone(), neutral::aggressive_rules(301), world)
            .unwrap();
    let world = SelectedMapNeutralHost::try_new(context.clone(), neutral::rules(), world).unwrap();
    let mut session = MapScriptSession::default();
    let entrypoints = ["setup", "prime", "HeadingWatch"]
        .map(|name| context.scripts.program().function_id(name).unwrap());
    session
        .replace(
            context,
            |_| {
                Ok::<_, std::convert::Infallible>(prepare(MapStartupHost::timer_session_fixture(
                    world,
                )))
            },
            &entrypoints,
        )
        .unwrap();
    crate::campaign_host::advance_paced_tick(&mut session, std::time::Duration::ZERO, 0., 100)
        .unwrap();
    let Value::Handle(caster) = session.global("H").unwrap() else {
        panic!("caster")
    };
    let Value::Handle(target) = session.global("E").unwrap() else {
        panic!("target")
    };
    (session, caster, target)
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; source arithmetic with synthetic unit properties and queue cadence"]
fn published_native_slow_turn_resumes_order_before_visual_phase_and_settles_canonical_effect() {
    let (mut session, caster, target) = heading_session();
    let issue = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("HeadingIssue")
        .unwrap();
    assert_eq!(
        session.call_function(issue, &[]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(0)));
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(375),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(0)));
    let before = session
        .host()
        .unwrap()
        .objects_mut()
        .native("GetUnitFacing", &[Value::Handle(caster)])
        .unwrap();
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(125),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(12)));
    assert_eq!(session.global("HeadingChannelFace"), Some(before.clone()));
    let Value::Real(after) = session
        .host()
        .unwrap()
        .objects_mut()
        .native("GetUnitFacing", &[Value::Handle(caster)])
        .unwrap()
    else {
        panic!("facing")
    };
    assert!(
        after > before.as_real().unwrap(),
        "visual phase must run after CHANNEL/CAST callbacks"
    );
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(250),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(123)));
    assert_eq!(session.global("HeadingEffectMana"), Some(Value::Real(200.)));
    assert_eq!(session.global("HeadingEffectBuff"), Some(Value::Int(0)));
    let objects = session.host().unwrap().objects_mut();
    assert_eq!(objects.unit_vitals(caster).unwrap().unwrap().mana, 150.);
    assert!(
        objects
            .unit_buffs(target)
            .unwrap()
            .iter()
            .any(|b| b.rawcode() == *b"Bslo")
    );
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(375),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(12345)));
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; source arithmetic with synthetic unit properties and queue cadence"]
fn published_native_slow_stop_during_turn_discards_order_without_spell_events_or_payment() {
    let (mut session, caster, target) = heading_session();
    let script = session.context().unwrap().scripts.program();
    let issue = script.function_id("HeadingIssue").unwrap();
    let stop = script.function_id("HeadingStop").unwrap();
    assert_eq!(
        session.call_function(issue, &[]).unwrap(),
        Value::Bool(true)
    );
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(250),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.call_function(stop, &[]).unwrap(), Value::Bool(true));
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_secs(1),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(0)));
    let objects = session.host().unwrap().objects_mut();
    assert_eq!(objects.unit_vitals(caster).unwrap().unwrap().mana, 200.);
    assert!(objects.unit_buffs(target).unwrap().is_empty());
    assert_eq!(objects.current_unit_order(caster).unwrap(), 0);
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; source arithmetic with synthetic unit properties and queue cadence"]
fn published_native_slow_heading_callback_fault_retains_phase_without_replay_or_payment() {
    let (mut session, caster, target) = heading_session();
    let script = session.context().unwrap().scripts.program();
    let issue = script.function_id("HeadingIssue").unwrap();
    let fault = script.function_id("HeadingFaultOn").unwrap();
    session.call_function(fault, &[]).unwrap();
    assert_eq!(
        session.call_function(issue, &[]).unwrap(),
        Value::Bool(true)
    );
    let error = crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(500),
        0.,
        100,
    )
    .unwrap_err();
    assert!(error.to_string().contains("HeadingUnsupported"), "{error}");
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(1)));
    let facing = session.global("HeadingChannelFace").unwrap();
    let objects = session.host().unwrap().objects_mut();
    assert!(objects.pending_native_physics().is_some());
    assert_eq!(
        objects
            .native("GetUnitFacing", &[Value::Handle(caster)])
            .unwrap(),
        facing
    );
    assert_eq!(objects.unit_vitals(caster).unwrap().unwrap().mana, 200.);
    assert!(objects.unit_buffs(target).unwrap().is_empty());
    let state = format!("{:?}", objects.test_native_clock_state());
    for producer in 0..4 {
        let retry = match producer {
            0 => crate::campaign_host::advance_paced_tick(
                &mut session,
                std::time::Duration::ZERO,
                0.,
                100,
            ),
            1 => crate::campaign_host::advance_paced_tick(
                &mut session,
                std::time::Duration::from_millis(333),
                1.,
                100,
            ),
            2 => session
                .advance_native_counter_frame(Some(std::time::Duration::from_millis(333)), 100),
            _ => session.advance_native_counter_frame(None, 100),
        }
        .unwrap_err();
        assert!(
            retry.to_string().contains("HeadingUnsupported"),
            "producer {producer}: {retry}"
        );
        assert_eq!(session.global("HeadingEvents"), Some(Value::Int(1)));
        let objects = session.host().unwrap().objects_mut();
        assert_eq!(
            format!("{:?}", objects.test_native_clock_state()),
            state,
            "producer {producer} changed source pacing before refusing the retained fault"
        );
        assert!(objects.pending_native_physics().is_some());
        assert_eq!(
            objects
                .native("GetUnitFacing", &[Value::Handle(caster)])
                .unwrap(),
            facing
        );
        assert_eq!(objects.unit_vitals(caster).unwrap().unwrap().mana, 200.);
        assert!(objects.unit_buffs(target).unwrap().is_empty());
    }
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; synthetic two-second queue cycle and physics cadence"]
fn published_native_physics_wrap_rebases_retained_request_without_duplicate_spell_events() {
    use crate::native_game_clock::Point;
    let (mut session, caster, target) = heading_session();
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_secs(4),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(0)));
    let objects = session.host().unwrap().objects_mut();
    let now = Point { cycle: 2, local: 0 };
    assert_eq!(objects.native_game_point(), Some(now));
    assert_eq!(objects.next_native_physics_boundary(now).unwrap(), None);
    let next = Point {
        cycle: 2,
        local: 0.125_f32.to_bits(),
    };
    assert_eq!(
        objects.next_native_physics_boundary(next).unwrap(),
        Some(next)
    );
    assert_eq!(objects.unit_vitals(caster).unwrap().unwrap().mana, 200.);
    assert!(objects.unit_buffs(target).unwrap().is_empty());
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(125),
        0.,
        100,
    )
    .unwrap();
    let objects = session.host().unwrap().objects_mut();
    assert_eq!(objects.native_game_point(), Some(next));
    assert_eq!(objects.next_native_physics_boundary(next).unwrap(), None);
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(0)));
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; source arithmetic with synthetic timers and physics cadence"]
fn published_native_physics_skips_paused_and_consumed_timer_requests_across_resume_rearm() {
    let (mut session, caster, _) = heading_session();
    let script = session.context().unwrap().scripts.program();
    let start = script.function_id("HeadingTimersStart").unwrap();
    let resume = script.function_id("HeadingTimersResume").unwrap();
    let issue = script.function_id("HeadingIssue").unwrap();
    session.call_function(start, &[]).unwrap();
    assert_eq!(
        session.call_function(issue, &[]).unwrap(),
        Value::Bool(true)
    );
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(375),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingOneShotCalls"), Some(Value::Int(1)));
    assert_eq!(session.global("HeadingTimedCalls"), Some(Value::Int(1)));
    assert_eq!(session.global("HeadingPausedCalls"), Some(Value::Int(0)));
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(0)));
    session.call_function(resume, &[]).unwrap();
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(500),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingPausedCalls"), Some(Value::Int(1)));
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(123)));
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(500),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingOneShotCalls"), Some(Value::Int(1)));
    assert_eq!(session.global("HeadingTimedCalls"), Some(Value::Int(1)));
    assert_eq!(session.global("HeadingPausedCalls"), Some(Value::Int(2)));
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(500),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingPausedCalls"), Some(Value::Int(2)));
    assert_eq!(session.global("HeadingOneShotCalls"), Some(Value::Int(1)));
    assert_eq!(session.global("HeadingTimedCalls"), Some(Value::Int(1)));
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(12345)));
    let objects = session.host().unwrap().objects_mut();
    assert_eq!(
        objects.native_game_point().unwrap().local,
        1.875_f32.to_bits()
    );
    assert_eq!(objects.unit_vitals(caster).unwrap().unwrap().mana, 150.);
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; source arithmetic with synthetic unit properties and physics cadence"]
fn published_native_slow_target_death_during_turn_clears_retained_heading_relation() {
    let (mut session, caster, target) = heading_session();
    let script = session.context().unwrap().scripts.program();
    let issue = script.function_id("HeadingIssue").unwrap();
    let kill = script.function_id("HeadingKillTarget").unwrap();
    assert_eq!(
        session.call_function(issue, &[]).unwrap(),
        Value::Bool(true)
    );
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(125),
        0.,
        100,
    )
    .unwrap();
    assert!(
        session
            .host()
            .unwrap()
            .objects()
            .native_heading_waiting(caster)
    );
    session.call_function(kill, &[]).unwrap();
    assert!(
        session
            .host()
            .unwrap()
            .objects()
            .executed_unit(target)
            .unwrap()
            .position
            .is_some()
    );
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_millis(125),
        0.,
        100,
    )
    .unwrap();
    assert!(
        !session
            .host()
            .unwrap()
            .objects()
            .native_heading_waiting(caster)
    );
    crate::campaign_host::advance_paced_tick(
        &mut session,
        std::time::Duration::from_secs(1),
        0.,
        100,
    )
    .unwrap();
    assert_eq!(session.global("HeadingEvents"), Some(Value::Int(0)));
    let objects = session.host().unwrap().objects_mut();
    assert!(!objects.native_heading_waiting(caster));
    assert_eq!(objects.current_unit_order(caster).unwrap(), 0);
    assert_eq!(objects.unit_vitals(caster).unwrap().unwrap().mana, 200.);
    assert!(objects.unit_buffs(target).unwrap().is_empty());
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

/// `GetUnitMoveSpeed` as a script reads it.
fn script_speed(vm: &mut Vm, unit: HandleId) -> f32 {
    match vm
        .host()
        .inner
        .native("GetUnitMoveSpeed", &[Value::Handle(unit)])
        .unwrap()
    {
        Value::Real(speed) => speed,
        other => panic!("GetUnitMoveSpeed returned {other:?}"),
    }
}

#[test]
fn the_script_speed_natives_see_the_slow_and_set_the_base_under_it() {
    let (mut vm, [caster, _, enemy, _]) = open();
    vm.call("prime", &[]).unwrap();
    let base = speed(&mut vm, enemy);
    assert!(order(&mut vm, caster, SLOW_ORDER, enemy));
    step(&mut vm, 0.4);
    assert_eq!(script_speed(&mut vm, enemy), clamped(base, MOVEMENT_FACTOR));
    // SetUnitMoveSpeed writes the base; the slow multiplier keeps acting on it.
    vm.host()
        .inner
        .native(
            "SetUnitMoveSpeed",
            &[Value::Handle(enemy), Value::Real(300.0)],
        )
        .unwrap();
    assert_eq!(
        script_speed(&mut vm, enemy),
        clamped(300.0, MOVEMENT_FACTOR)
    );
    // At expiry the slow is taken out of the multiplier, not the old base put back.
    step(&mut vm, CAST_POINT + SLOW_SECONDS + 0.1);
    assert_eq!(script_speed(&mut vm, enemy), 300.0);
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

const SLOW_CALLBACKS: &str = r#"
native UnitRemoveAbility takes unit u, integer a returns boolean
native UnitAddAbility takes unit u, integer a returns boolean
globals
integer SlowMode
integer SlowEvents=0
real SlowEffectMana
integer SlowEffectBuff
real SlowActionMana
real SlowEndMana
unit SlowEndTarget
unit SlowFinishTarget
unit SlowRetarget
trigger SlowEffectTrigger
boolean SlowAction
endglobals
function SlowChannel takes nothing returns nothing
set SlowEvents=SlowEvents*10+1
endfunction
function SlowCast takes nothing returns nothing
set SlowEvents=SlowEvents*10+2
endfunction
function SlowEffect takes nothing returns nothing
set SlowEvents=SlowEvents*10+3
set SlowEffectMana=GetUnitState(H,UNIT_STATE_MANA)
set SlowEffectBuff=GetUnitAbilityLevel(E,'Bslo')
if SlowMode==1 then
set SlowAction=IssueImmediateOrder(H,"beta")
elseif SlowMode==2 then
set SlowAction=IssueTargetOrder(H,"alpha",SlowRetarget)
elseif SlowMode==3 then
call RemoveUnit(E)
elseif SlowMode==4 then
call KillUnit(H)
elseif SlowMode==5 then
set SlowAction=UnitRemoveAbility(H,'Aslo')
elseif SlowMode==6 then
call RemoveUnit(H)
elseif SlowMode==7 then
call SetUnitTimeScale(H,-1.0)
elseif SlowMode==8 then
call SetUnitState(H,UNIT_STATE_MANA,40.0)
elseif SlowMode==9 then
set SlowAction=UnitRemoveAbility(H,'Aslo')
set SlowAction=UnitAddAbility(H,'Aslo')
elseif SlowMode>=10 then
loop
exitwhen SlowMode==10
call CreateTimer()
set SlowMode=SlowMode-1
endloop
endif
set SlowActionMana=GetUnitState(H,UNIT_STATE_MANA)
endfunction
function SlowFinish takes nothing returns nothing
set SlowEvents=SlowEvents*10+4
set SlowFinishTarget=GetSpellTargetUnit()
endfunction
function SlowEnd takes nothing returns nothing
set SlowEvents=SlowEvents*10+5
set SlowEndMana=GetUnitState(H,UNIT_STATE_MANA)
if SlowMode==5 then
set SlowMode=GetUnitAbilityLevel(H,'Aslo')
endif
set SlowEndTarget=GetSpellTargetUnit()
endfunction
function SlowWatch takes integer mode, unit retarget returns nothing
local trigger t=CreateTrigger()
set SlowMode=mode
set SlowRetarget=retarget
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_CHANNEL)
call TriggerAddAction(t,function SlowChannel)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_CAST)
call TriggerAddAction(t,function SlowCast)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_EFFECT)
call TriggerAddAction(t,function SlowEffect)
set SlowEffectTrigger=t
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_FINISH)
call TriggerAddAction(t,function SlowFinish)
set t=CreateTrigger()
call TriggerRegisterUnitEvent(t,H,EVENT_UNIT_SPELL_ENDCAST)
call TriggerAddAction(t,function SlowEnd)
endfunction
function SlowIssue takes nothing returns boolean
return IssueTargetOrder(H,"alpha",E)
endfunction
function SlowFillOnEffect takes integer count returns nothing
set SlowMode=count+10
set SlowEvents=0
endfunction
"#;

fn slow_callbacks(mode: i32) -> (Vm, HandleId, HandleId, HandleId) {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        script: SLOW_CALLBACKS,
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    let other = spawn(&mut vm, 5, b"u001", 64.);
    vm.call("SlowWatch", &[Value::Int(mode), Value::Handle(other)])
        .unwrap();
    assert_eq!(vm.call("SlowIssue", &[]).unwrap(), Value::Bool(true));
    assert_eq!(
        vm.global("SlowEvents"),
        Value::Int(12),
        "CHANNEL/CAST precede the public order return"
    );
    (vm, caster, enemy, other)
}

fn slow_advance_and_deliver(vm: &mut Vm, until: f64) -> Vec<f64> {
    let mut times = Vec::new();
    while vm.host().inner.timer_game_time().unwrap() < until {
        let next = vm
            .host()
            .inner
            .next_timer_activity_boundary(until)
            .unwrap_or(until);
        assert!(
            vm.host()
                .inner
                .next_timer_expiry(next, &mut 100)
                .unwrap()
                .is_none()
        );
        vm.host()
            .inner
            .advance_unit_activity(next, &mut 65536)
            .unwrap();
        while let Some(event) = vm.host().inner.take_fired_event() {
            times.push(vm.host().inner.timer_game_time().unwrap());
            if let Some(delivery) = vm.host().inner.begin_fired_event(&event).unwrap() {
                assert!(delivery.conditions.is_empty());
                let result = vm.run_thread(&delivery.actions);
                vm.host().inner.finish_fired_event();
                result.unwrap();
            }
        }
    }
    times
}

fn slow_cooldown(vm: &mut Vm, unit: HandleId) -> f32 {
    let Value::Real(value) = vm
        .host()
        .inner
        .native(
            "BlzGetUnitAbilityCooldownRemaining",
            &[
                Value::Handle(unit),
                Value::Int(i32::from_be_bytes(*b"Aslo")),
            ],
        )
        .unwrap()
    else {
        panic!("cooldown")
    };
    value
}

#[test]
fn slow_public_order_effect_callback_precedes_payment_and_source_timed_finish() {
    let (mut vm, caster, enemy, _) = slow_callbacks(0);
    let base = speed(&mut vm, enemy);
    let times = slow_advance_and_deliver(&mut vm, 1.);
    assert_eq!(times, [CAST_POINT, CAST_POINT + 0.375, CAST_POINT + 0.375]);
    assert_eq!(vm.global("SlowEffectMana"), Value::Real(200.));
    assert_eq!(vm.global("SlowEffectBuff"), Value::Int(0));
    assert_eq!(vm.global("SlowEvents"), Value::Int(12345));
    assert_eq!(vm.global("SlowFinishTarget"), Value::Null);
    assert_eq!(vm.global("SlowEndTarget"), Value::Null);
    assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
    assert_eq!(speed(&mut vm, enemy), clamped(base, MOVEMENT_FACTOR));
    assert_eq!(current_order(&mut vm, caster), 0);
}

#[test]
fn slow_event_capacity_refuses_public_order_before_cast_publication() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        script: SLOW_CALLBACKS,
        forms: true,
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    vm.call("SlowWatch", &[Value::Int(0), Value::Handle(enemy)])
        .unwrap();
    let Value::Handle(trigger) = vm.global("SlowEffectTrigger") else {
        panic!("trigger")
    };
    let before = vm
        .host()
        .inner
        .executed_unit(caster)
        .unwrap()
        .animation
        .clone();
    let base = speed(&mut vm, enemy);
    // The shared synthetic position host admits 256 handles/pending game events.
    for _ in 0..256 {
        vm.host().inner.push_reserved_event(
            trigger,
            None,
            crate::playercolor_host::EventSubject::default(),
            None,
        );
    }
    let error = vm.call("SlowIssue", &[]).unwrap_err().to_string();
    assert!(error.contains("pending game events"), "{error}");
    assert_eq!(vm.global("SlowEvents"), Value::Int(0));
    assert_eq!(
        vm.host().inner.executed_unit(caster).unwrap().animation,
        before
    );
    assert_eq!(current_order(&mut vm, caster), 0);
    let mut count = 0;
    while let Some(event) = vm.host().inner.take_fired_event() {
        assert_eq!(event.subject.spell_ability, None);
        count += 1;
    }
    assert_eq!(count, 256);
    step(&mut vm, CAST_POINT + 1.);
    assert_eq!(mana(&mut vm, caster), 200.);
    assert_eq!(speed(&mut vm, enemy), base);
    assert!(buffs(&mut vm, enemy).is_empty());
    assert_eq!(slow_cooldown(&mut vm, caster), 0.);
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn slow_effect_handle_exhaustion_preserves_payment_and_previous_target_state() {
    for refreshing in [false, true] {
        let (mut vm, caster, enemy, _) = slow_callbacks(0);
        if refreshing {
            slow_advance_and_deliver(&mut vm, 3.);
        }
        let before_speed = speed(&mut vm, enemy);
        let before_mana = mana(&mut vm, caster);
        let before_buffs = buffs(&mut vm, enemy);
        let before_effects = effect_count(&mut vm);
        let remaining = 256 - vm.host().inner.inner().handles();
        vm.call("SlowFillOnEffect", &[Value::Int(remaining as i32)])
            .unwrap();
        let start = vm.host().inner.timer_game_time().unwrap();
        if refreshing {
            assert_eq!(vm.call("SlowIssue", &[]).unwrap(), Value::Bool(true));
        }
        step(&mut vm, start + CAST_POINT);
        let event = vm.host().inner.take_fired_event().unwrap();
        let delivery = vm.host().inner.begin_fired_event(&event).unwrap().unwrap();
        vm.run_thread(&delivery.actions).unwrap();
        vm.host().inner.finish_fired_event();
        assert_eq!(vm.global("SlowMode"), Value::Int(10));
        assert_eq!(vm.host().inner.inner().handles(), 256);
        assert_eq!(mana(&mut vm, caster), before_mana);
        assert_eq!(speed(&mut vm, enemy), before_speed);
        assert_eq!(buffs(&mut vm, enemy), before_buffs);
        assert_eq!(effect_count(&mut vm), before_effects);
        assert_eq!(slow_cooldown(&mut vm, caster), 0.);
        assert!(vm.host().inner.take_fired_event().is_none());
        for until in [start + 1., start + 2.] {
            assert!(
                vm.host()
                    .inner
                    .next_timer_expiry(until, &mut 100)
                    .unwrap()
                    .is_none()
            );
            let error = vm
                .host()
                .inner
                .advance_unit_activity(until, &mut 65536)
                .unwrap_err()
                .to_string();
            assert!(error.contains("capacity"), "{error}");
            assert_eq!(mana(&mut vm, caster), before_mana);
            assert_eq!(speed(&mut vm, enemy), before_speed);
            assert_eq!(buffs(&mut vm, enemy), before_buffs);
            assert_eq!(effect_count(&mut vm), before_effects);
        }
    }
}

#[test]
fn slow_full_buff_capacity_refuses_before_art_identity_or_target_mutation() {
    use crate::playercolor_host::{ActiveBuff, BuffTraits, MAX_BUFFS_PER_UNIT};
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        script: SLOW_CALLBACKS,
        forms: true,
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    vm.call("SlowWatch", &[Value::Int(0), Value::Handle(enemy)])
        .unwrap();
    assert_eq!(vm.call("SlowIssue", &[]).unwrap(), Value::Bool(true));
    step(&mut vm, CAST_POINT);
    for _ in 0..MAX_BUFFS_PER_UNIT {
        vm.host()
            .inner
            .attach_unit_buff(enemy, ActiveBuff::new(*b"Xcap", BuffTraits::default()))
            .unwrap();
    }
    let record = vm.host().inner.executed_unit(enemy).unwrap();
    let before = (record.activity.clone(), record.animation.clone());
    let handles = vm.host().inner.inner().handles();
    let before_buffs = buffs(&mut vm, enemy);
    let before_speed = speed(&mut vm, enemy);
    assert_eq!(effect_count(&mut vm), 0, "no preexisting effect history");
    let event = vm.host().inner.take_fired_event().unwrap();
    let delivery = vm.host().inner.begin_fired_event(&event).unwrap().unwrap();
    vm.run_thread(&delivery.actions).unwrap();
    vm.host().inner.finish_fired_event();
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(
        effect_count(&mut vm),
        0,
        "failed art must not leave even a destroyed identity"
    );
    assert_eq!(vm.host().inner.unit_effects().count(), 0);
    let record = vm.host().inner.executed_unit(enemy).unwrap();
    assert_eq!((record.activity.clone(), record.animation.clone()), before);
    assert_eq!(buffs(&mut vm, enemy), before_buffs);
    assert_eq!(speed(&mut vm, enemy), before_speed);
    assert_eq!(vm.host().inner.curse_attack_penalty(enemy), 0.);
    assert_eq!(mana(&mut vm, caster), 200.);
    assert_eq!(slow_cooldown(&mut vm, caster), 0.);
    assert!(vm.host().inner.take_fired_event().is_none());
    assert!(
        vm.host()
            .inner
            .next_timer_expiry(1., &mut 100)
            .unwrap()
            .is_none()
    );
    let error = vm
        .host()
        .inner
        .advance_unit_activity(1., &mut 65536)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unit buff capacity"), "{error}");
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(mana(&mut vm, caster), 200.);
}

#[test]
fn slow_effect_stop_or_caster_death_keeps_payment_without_buff_cooldown_or_finish() {
    for mode in [1, 4] {
        let (mut vm, caster, enemy, _) = slow_callbacks(mode);
        slow_advance_and_deliver(&mut vm, 1.);
        assert_eq!(vm.global("SlowEvents"), Value::Int(1235));
        assert_eq!(vm.global("SlowEndTarget"), Value::Handle(enemy));
        assert_eq!(vm.global("SlowEndMana"), Value::Real(200.));
        assert_eq!(vm.global("SlowActionMana"), Value::Real(200.));
        assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
        assert!(buffs(&mut vm, enemy).is_empty());
        assert_eq!(slow_cooldown(&mut vm, caster), 0.);
        slow_advance_and_deliver(&mut vm, 2.);
        assert_eq!(vm.global("SlowEvents"), Value::Int(1235));
        assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
    }
}

#[test]
fn slow_effect_retarget_keeps_original_cast_and_removed_target_still_pays_and_cools() {
    for mode in [2, 3] {
        let (mut vm, caster, enemy, other) = slow_callbacks(mode);
        slow_advance_and_deliver(&mut vm, 1.);
        assert_eq!(vm.global("SlowEvents"), Value::Int(12345));
        assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
        assert!(slow_cooldown(&mut vm, caster) > 0.);
        assert!(buffs(&mut vm, other).is_empty());
        if mode == 2 {
            assert_eq!(vm.global("SlowAction"), Value::Bool(true));
            assert_eq!(buffs(&mut vm, enemy), [*b"Bslo"]);
        }
    }
}

#[test]
fn slow_effect_ability_withdrawal_or_caster_removal_does_not_leave_or_replay_jobs() {
    for mode in [5, 6] {
        let (mut vm, caster, enemy, _) = slow_callbacks(mode);
        slow_advance_and_deliver(&mut vm, 1.);
        assert_eq!(vm.global("SlowEvents"), Value::Int(1235));
        assert!(buffs(&mut vm, enemy).is_empty());
        if mode == 5 {
            assert_eq!(vm.global("SlowAction"), Value::Bool(true));
            assert_eq!(vm.global("SlowMode"), Value::Int(0));
            assert_eq!(vm.global("SlowEndMana"), Value::Real(200.));
            assert_eq!(vm.global("SlowEndTarget"), Value::Handle(enemy));
            assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
            assert_eq!(
                vm.host()
                    .inner
                    .native(
                        "GetUnitAbilityLevel",
                        &[
                            Value::Handle(caster),
                            Value::Int(i32::from_be_bytes(*b"Aslo"))
                        ]
                    )
                    .unwrap(),
                Value::Int(0)
            );
        }
        slow_advance_and_deliver(&mut vm, 2.);
        assert_eq!(vm.global("SlowEvents"), Value::Int(1235));
    }
}

#[test]
fn slow_disabled_or_destroyed_effect_trigger_still_settles_payment_once() {
    for native in ["DisableTrigger", "DestroyTrigger"] {
        let (mut vm, caster, enemy, _) = slow_callbacks(0);
        let trigger = vm.global("SlowEffectTrigger");
        vm.host().inner.native(native, &[trigger]).unwrap();
        slow_advance_and_deliver(&mut vm, 1.);
        assert_eq!(vm.global("SlowEvents"), Value::Int(1245));
        assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
        assert_eq!(buffs(&mut vm, enemy), [*b"Bslo"]);
        slow_advance_and_deliver(&mut vm, 2.);
        assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
        assert_eq!(vm.global("SlowEvents"), Value::Int(1245));
    }
}

#[test]
fn slow_effect_callback_error_restores_context_and_does_not_replay_payment() {
    let (mut vm, caster, enemy, _) = slow_callbacks(7);
    step(&mut vm, CAST_POINT);
    let event = vm.host().inner.take_fired_event().unwrap();
    let delivery = vm.host().inner.begin_fired_event(&event).unwrap().unwrap();
    let error = vm.run_thread(&delivery.actions).unwrap_err().to_string();
    vm.host().inner.finish_fired_event();
    assert!(error.contains("SetUnitTimeScale"), "{error}");
    assert_eq!(
        vm.host().inner.native("GetSpellTargetUnit", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
    assert_eq!(buffs(&mut vm, enemy), [*b"Bslo"]);
    slow_advance_and_deliver(&mut vm, 1.);
    assert_eq!(vm.global("SlowEvents"), Value::Int(12345));
    assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
}

#[test]
fn slow_pause_shifts_effect_and_backswing_boundaries_without_duplicate_events() {
    let (mut vm, caster, _, _) = slow_callbacks(0);
    slow_advance_and_deliver(&mut vm, CAST_POINT / 2.);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(true)])
        .unwrap();
    assert!(slow_advance_and_deliver(&mut vm, 1.).is_empty());
    assert_eq!(mana(&mut vm, caster), 200.);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(false)])
        .unwrap();
    let effect = 1. + CAST_POINT / 2.;
    assert_eq!(slow_advance_and_deliver(&mut vm, effect), [effect]);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(true)])
        .unwrap();
    assert!(slow_advance_and_deliver(&mut vm, 2.).is_empty());
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(false)])
        .unwrap();
    assert_eq!(slow_advance_and_deliver(&mut vm, 3.), [2.375, 2.375]);
    assert_eq!(vm.global("SlowEvents"), Value::Int(12345));
    assert_eq!(mana(&mut vm, caster), 200. - SLOW_COST);
}

#[test]
fn slow_effect_payment_reads_callback_mana_and_the_bound_nonstock_minimum() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        script: SLOW_CALLBACKS,
        mana_minimum: Some(7.),
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    vm.call("SlowWatch", &[Value::Int(8), Value::Handle(enemy)])
        .unwrap();
    assert_eq!(vm.call("SlowIssue", &[]).unwrap(), Value::Bool(true));
    slow_advance_and_deliver(&mut vm, 1.);
    assert_eq!(vm.global("SlowEffectMana"), Value::Real(200.));
    assert_eq!(vm.global("SlowActionMana"), Value::Real(40.));
    assert_eq!(mana(&mut vm, caster), 7.);
    assert_eq!(buffs(&mut vm, enemy), [*b"Bslo"]);
    assert!(slow_cooldown(&mut vm, caster) > 0.);
    assert_eq!(vm.global("SlowEvents"), Value::Int(12345));
}

#[test]
fn slow_missing_mana_policy_refuses_before_order_or_phase_delivery() {
    let (mut vm, [caster, _, enemy, _]) = open_with(Config {
        script: SLOW_CALLBACKS,
        mana_minimum: None,
        ..Config::default()
    });
    vm.call("prime", &[]).unwrap();
    vm.call("SlowWatch", &[Value::Int(0), Value::Handle(enemy)])
        .unwrap();
    let error = vm.call("SlowIssue", &[]).unwrap_err().to_string();
    assert!(error.contains("mana minimum is not admitted"), "{error}");
    assert_eq!(vm.global("SlowEvents"), Value::Int(0));
    assert_eq!(current_order(&mut vm, caster), 0);
    assert_eq!(mana(&mut vm, caster), 200.);
    assert!(buffs(&mut vm, enemy).is_empty());
}

#[test]
fn slow_marker_policy_preserves_strict_readers_and_refuses_malformed_numeric_data() {
    for marker in ["-", "_"] {
        let (mut vm, [caster, _, _, _]) = open_with(Config {
            slow: vec![("DataC1", marker)],
            ..Config::default()
        });
        let host = &mut vm.host().inner;
        assert!(
            host.hero_ability_level_cell(*b"Aslo", "DataC", 1, FieldKind::Bool)
                .unwrap_err()
                .contains("missing DataC1")
        );
        assert_eq!(
            host.hero_ability_level_cell_including_marker(*b"Aslo", "DataC", 1, FieldKind::Bool)
                .unwrap(),
            marker
        );
        let now = host.timer_game_time().unwrap();
        let mut work = host.executed_units().count();
        assert_eq!(
            host.advance_slow_autocast(caster, *b"Aslo", now, &mut work)
                .unwrap(),
            Some(crate::autocast_rules::Outcome::Retry)
        );
        assert_eq!(work, 0);
    }
    let (mut vm, [caster, _, _, _]) = open_with(Config {
        slow: vec![("DataC1", "garbage")],
        ..Config::default()
    });
    let now = vm.host().inner.timer_game_time().unwrap();
    assert!(
        vm.host()
            .inner
            .advance_slow_autocast(caster, *b"Aslo", now, &mut 100)
            .is_err()
    );
}

#[test]
fn slow_no_target_evaluation_charges_every_unit_and_refuses_a_potential_attacker() {
    let (mut vm, [caster, _, enemy, _]) = open();
    let host = &mut vm.host().inner;
    let now = host.timer_game_time().unwrap();
    let error = host
        .advance_slow_autocast(caster, *b"Aslo", now, &mut 0)
        .unwrap_err()
        .to_string();
    assert!(error.contains("work budget"), "{error}");
    assert_eq!(
        host.native(
            "IssueTargetOrder",
            &[
                Value::Handle(enemy),
                Value::Str("beta".into()),
                Value::Handle(caster)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let error = host
        .advance_slow_autocast(caster, *b"Aslo", now, &mut 100)
        .unwrap_err()
        .to_string();
    assert!(error.contains("potential attacking candidate"), "{error}");
    assert!(host.take_fired_event().is_none());
}

#[test]
fn slow_negative_filter_uses_directed_help_response_and_preserves_unknown_state() {
    let (mut vm, [caster, friend, enemy, _]) = open_with(Config {
        script: "globals\nconstant alliancetype SLOW_FIXTURE_RESPONSE=ConvertAllianceType(47)\nendglobals\n",
        ..Config::default()
    });
    let victim = spawn(&mut vm, 3, b"u001", 80.);
    let host = &mut vm.host().inner;
    let now = host.timer_game_time().unwrap();
    let attack = |host: &mut ObjectHost, target| {
        assert_eq!(
            host.native(
                "IssueTargetOrder",
                &[
                    Value::Handle(enemy),
                    Value::Str("beta".into()),
                    Value::Handle(target)
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
    };
    let owner = host.executed_unit(caster).unwrap().owner;
    let other = host.executed_unit(victim).unwrap().owner;
    let response = host
        .native("ConvertAllianceType", &[Value::Int(47)])
        .unwrap();
    let passive = host
        .native("ConvertAllianceType", &[Value::Int(0)])
        .unwrap();
    let alliance = |host: &mut ObjectHost, a, b, setting: Value, value| {
        host.native(
            "SetPlayerAlliance",
            &[
                Value::Handle(a),
                Value::Handle(b),
                setting,
                Value::Bool(value),
            ],
        )
        .unwrap();
    };
    attack(host, victim);
    let error = host
        .advance_slow_autocast(caster, *b"Aslo", now, &mut 100)
        .unwrap_err()
        .to_string();
    assert!(error.contains("Help Response initial state"), "{error}");
    alliance(host, owner, other, passive, true);
    alliance(host, other, owner, response.clone(), true);
    alliance(host, owner, other, response.clone(), false);
    let before: Vec<_> = host
        .executed_units()
        .map(|(id, unit)| (id, unit.clone()))
        .collect();
    let mut work = before.len() + 2;
    assert_eq!(
        host.advance_slow_autocast(caster, *b"Aslo", now, &mut work)
            .unwrap(),
        Some(crate::autocast_rules::Outcome::Retry)
    );
    assert_eq!(work, 0);
    assert_eq!(
        host.executed_units()
            .map(|(id, unit)| (id, unit.clone()))
            .collect::<Vec<_>>(),
        before
    );
    let error = host
        .advance_slow_autocast(caster, *b"Aslo", now, &mut (before.len() + 1))
        .unwrap_err()
        .to_string();
    assert!(error.contains("work budget"), "{error}");
    alliance(host, owner, other, response.clone(), true);
    let error = host
        .advance_slow_autocast(caster, *b"Aslo", now, &mut 100)
        .unwrap_err()
        .to_string();
    assert!(error.contains("potential attacking candidate"), "{error}");
    attack(host, friend);
    let error = host
        .advance_slow_autocast(caster, *b"Aslo", now, &mut 100)
        .unwrap_err()
        .to_string();
    assert!(error.contains("potential attacking candidate"), "{error}");
    alliance(host, owner, owner, response, false);
    assert_eq!(
        host.advance_slow_autocast(caster, *b"Aslo", now, &mut 100)
            .unwrap(),
        Some(crate::autocast_rules::Outcome::Retry)
    );
    assert!(host.take_fired_event().is_none());
}

#[test]
fn slow_effect_ability_replacement_refuses_the_unqualified_cost_read_point() {
    let (mut vm, caster, enemy, _) = slow_callbacks(9);
    step(&mut vm, CAST_POINT);
    let event = vm.host().inner.take_fired_event().unwrap();
    let delivery = vm.host().inner.begin_fired_event(&event).unwrap().unwrap();
    vm.run_thread(&delivery.actions).unwrap();
    vm.host().inner.finish_fired_event();
    assert!(
        vm.host()
            .inner
            .next_timer_expiry(1., &mut 100)
            .unwrap()
            .is_none()
    );
    let error = vm
        .host()
        .inner
        .advance_unit_activity(1., &mut 1000)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("native cost read point is unqualified"),
        "{error}"
    );
    assert_eq!(mana(&mut vm, caster), 200.);
    assert!(buffs(&mut vm, enemy).is_empty());
    assert_eq!(vm.global("SlowEvents"), Value::Int(1235));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; source arithmetic with nonstock physics and paired queue cadence"]
fn paired_native_physics_arbitrates_timer_ai_and_equal_deadlines_before_entering_a_scope() {
    use crate::{
        native_game_clock::{Domain, FrameInput, Point},
        scripts::{AssetIdentity, MapScripts, ScriptAsset},
    };
    for (timer_at, ai_at, physics_first) in [
        (0.0625, 0.1875, false),
        (0.1875, 0.0625, false),
        (0.21875, 0.1875, true),
        (0.125, 0.125, true),
    ] {
        let (mut session, _, _) = heading_session_with(|mut host| {
            let context = Arc::clone(host.context());
            host.objects_mut()
                .admit_campaign_mode(
                    &context,
                    crate::campaign_mode_policy::CampaignModePolicy::fixture(),
                )
                .unwrap();
            let common = &context.scripts.digests[0].label;
            let entry = format!(
                "globals\nunit H=null\nunit E=null\nendglobals\nfunction main takes nothing returns nothing\nlocal real retained=0.25\ncall Sleep({ai_at})\ncall Sleep(retained)\nendfunction\n"
            );
            let program = Arc::new(
                MapScripts::compile(
                    AssetIdentity {
                        source: "heading.ai".into(),
                        edition: "synthetic".into(),
                        version: "paired-physics".into(),
                    },
                    ScriptAsset {
                        label: common,
                        bytes: context.scripts.source(common).unwrap().as_bytes(),
                    },
                    ScriptAsset {
                        label: "synthetic:heading-ai-common",
                        bytes: b"native GetAiPlayer takes nothing returns integer\nnative Sleep takes real seconds returns nothing\nnative StartThread takes code callback returns nothing\nnative GetUpgradeLevel takes integer id returns integer\nnative GetUnitGoldCost takes integer unitid returns integer\nnative GetUnitWoodCost takes integer unitid returns integer\n",
                    },
                    ScriptAsset {
                        label: "synthetic:heading-ai",
                        bytes: entry.as_bytes(),
                    },
                    None,
                )
                .unwrap(),
            );
            host.with_ai_pause_rules(Ok(crate::campaign_ai_pause_policy::Rules {
                initial: 0,
                retry_seconds: 1.,
                retry_word: 1f32.to_bits(),
            }))
            .with_ai_loader(move |_| Ok(Arc::clone(&program)))
        });
        let setup = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("HeadingArbitrationSetup")
            .unwrap();
        let start = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("HeadingStartAi")
            .unwrap();
        let callback = session
            .context()
            .unwrap()
            .scripts
            .program()
            .function_id("HeadingObserve")
            .unwrap();
        session.call_function(setup, &[]).unwrap();
        let timer = session.global("HeadingOneShot").unwrap();
        session
            .host()
            .unwrap()
            .native(
                "TimerStart",
                &[
                    timer,
                    Value::Real(timer_at),
                    Value::Bool(false),
                    Value::Code(callback),
                ],
            )
            .unwrap();
        session.call_function(start, &[]).unwrap();
        let until = Point {
            cycle: 0,
            local: 0.5f32.to_bits(),
        };
        let error = session
            .advance_native_frame(
                Some(FrameInput {
                    game_elapsed: std::time::Duration::from_millis(250),
                    real_elapsed_ms: 0,
                }),
                1,
            )
            .unwrap_err();
        assert!(
            matches!(
                error,
                crate::map_script_session::TimerSessionError::WakeBudget
                    | crate::map_script_session::TimerSessionError::QueueBudget(_)
            ),
            "{error}"
        );
        assert!(
            session
                .host()
                .unwrap()
                .objects()
                .pending_native_physics()
                .is_none()
        );
        let next_physics = session
            .host()
            .unwrap()
            .objects()
            .next_native_physics_boundary(until)
            .unwrap()
            .unwrap();
        assert_eq!(
            f32::from_bits(next_physics.local),
            if physics_first { 0.25 } else { 0.125 }
        );
        let next_ai = session
            .host()
            .unwrap()
            .next_native_ai_wake(until, &mut 1024)
            .unwrap()
            .unwrap();
        let ai_resumed = ai_at < 0.125;
        assert_eq!(
            f32::from_bits(next_ai.point.local),
            if ai_resumed { ai_at + 0.25 } else { ai_at }
        );
        assert_eq!(
            session.global("HeadingChannelFace"),
            Some(Value::Real(if timer_at < 0.125 { timer_at } else { -1. }))
        );
        session.advance_native_frame(None, 100).unwrap();
        assert_eq!(
            session.global("HeadingChannelFace"),
            Some(Value::Real(timer_at))
        );
        assert_eq!(session.global("HeadingFail"), Some(Value::Bool(true)));
        let next_ai = session
            .host()
            .unwrap()
            .next_native_ai_wake(until, &mut 1024)
            .unwrap()
            .unwrap();
        assert_eq!(f32::from_bits(next_ai.point.local), ai_at + 0.25);
        let objects = session.host().unwrap().objects();
        assert_eq!(
            objects.native_game_point(),
            Some(Point {
                cycle: 0,
                local: 0.25f32.to_bits()
            })
        );
        assert_eq!(
            objects.native_queue_point(Domain::Real),
            Some(Point { cycle: 0, local: 0 })
        );
        assert_eq!(
            objects
                .next_native_physics_boundary(until)
                .unwrap()
                .unwrap()
                .local,
            0.375f32.to_bits()
        );
        session.advance_native_frame(None, 100).unwrap();
        assert_eq!(
            session.global("HeadingChannelFace"),
            Some(Value::Real(timer_at))
        );
    }
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; source arithmetic and nonstock physics admission"]
fn native_physics_counter_admission_retains_fresh_state_on_zero_work_and_arms_once_before_globals()
{
    use crate::native_game_clock::{Domain, FrameInput, Input, Point};
    let mut host = host_with(&Config {
        heading: true,
        ..Config::default()
    });
    let before = format!("{:?}", host.test_native_clock_state());
    assert!(
        host.enqueue_native_input(
            Input::Independent(FrameInput {
                game_elapsed: std::time::Duration::ZERO,
                real_elapsed_ms: 0
            }),
            &mut 0
        )
        .is_err()
    );
    assert_eq!(format!("{:?}", host.test_native_clock_state()), before);
    let until = Point {
        cycle: 0,
        local: 0.5f32.to_bits(),
    };
    assert_eq!(host.next_native_physics_boundary(until).unwrap(), None);
    assert!(host.select_native_counter_platform().unwrap());
    let next = host.next_native_physics_boundary(until).unwrap().unwrap();
    assert_eq!(next.local, 0.125f32.to_bits());
    assert!(host.native_counter_platform());
    assert!(host.select_native_counter_platform().unwrap());
    assert_eq!(
        host.next_native_physics_boundary(until).unwrap(),
        Some(next)
    );
    assert_eq!(
        host.native_queue_point(Domain::Game),
        Some(Point { cycle: 0, local: 0 })
    );
    assert_eq!(
        host.native_queue_point(Domain::Real),
        Some(Point { cycle: 0, local: 0 })
    );
}

#[test]
#[ignore = "requires PCX_WAR3_EXE; source arithmetic and synthetic pre-entry cast properties"]
fn counter_preentry_selection_keeps_global_config_and_main_slow_orders_in_native_heading() {
    const MAP: &str = r#"
globals
unit H=null
unit E=null
real GlobalFace=-1.0
real ConfigFace=-1.0
real MainFace=-1.0
boolean GlobalAccepted=false
boolean ConfigAccepted=false
boolean MainAccepted=false
boolean Ready=SpawnGlobal()
endglobals
function SpawnGlobal takes nothing returns boolean
set H=CreateUnit(Player(4),'H001',16.0,16.0,0.0)
set E=CreateUnit(Player(5),'u001',16.0,112.0,180.0)
call SetUnitState(H,UNIT_STATE_MANA,200.0)
call HeadingWatch()
call SetUnitFacing(H,45.0)
set GlobalAccepted=IssueTargetOrder(H,"alpha",E)
set GlobalFace=GetUnitFacing(H)
return true
endfunction
function config takes nothing returns nothing
call SetUnitFacing(H,180.0)
set ConfigAccepted=IssueTargetOrder(H,"alpha",E)
set ConfigFace=GetUnitFacing(H)
endfunction
function main takes nothing returns nothing
call SetUnitFacing(H,270.0)
set MainAccepted=IssueTargetOrder(H,"alpha",E)
set MainFace=GetUnitFacing(H)
endfunction
"#;
    let mut host = host_with(&Config {
        map: MAP,
        script: HEADING_CALLBACKS,
        heading: true,
        ..Config::default()
    });
    host.admit_unit_curses().unwrap();
    assert!(host.select_native_counter_platform().unwrap());
    let context = Arc::clone(host.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("config", &[]).unwrap();
    vm.call("main", &[]).unwrap();
    for name in ["Ready", "GlobalAccepted", "ConfigAccepted", "MainAccepted"] {
        assert_eq!(vm.global(name), Value::Bool(true), "{name}");
    }
    for name in ["GlobalFace", "ConfigFace", "MainFace"] {
        assert_eq!(
            vm.global(name),
            Value::Real(0.),
            "{name} must precede visual publication"
        );
    }
    let Value::Handle(caster) = vm.global("H") else {
        panic!("caster")
    };
    let Value::Handle(target) = vm.global("E") else {
        panic!("target")
    };
    let objects = &mut vm.host().inner;
    assert!(objects.native_counter_platform());
    assert!(objects.native_heading_waiting(caster));
    assert_eq!(objects.unit_vitals(caster).unwrap().unwrap().mana, 200.);
    assert!(objects.unit_buffs(target).unwrap().is_empty());
    assert_eq!(vm.global("HeadingEvents"), Value::Int(0));
}
