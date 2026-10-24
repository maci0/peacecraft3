use super::*;
#[path = "ability_cooldown_tests.rs"]
mod ability_cooldowns;
#[path = "campaign_group_timed_life_host_tests.rs"]
mod campaign_timed_life;
#[path = "hero_experience_kill_tests.rs"]
mod experience_kills;
#[path = "standard_command_host_tests.rs"]
mod standard_commands;
#[path = "unit_summon_tests.rs"]
mod summoning;
use crate::playercolor_host::{CombatActor, CombatTraceEvent};
use pcx_objects::editor::misc::{CombatRules, DayCycle, HeroVitals};
use pcx_script::HandleId;
#[path = "blight_regeneration_tests.rs"]
mod blight;
#[path = "unit_buff_host_tests.rs"]
mod buffs;
#[path = "divine_shield_tests.rs"]
mod divine_shield;
#[path = "unit_sleep_tests.rs"]
mod natural_sleep;
#[path = "patrol_combat_tests.rs"]
mod patrol;
type Vm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;

#[test]
fn campaign_unit_counts_follow_the_shared_unit_lifecycle_without_mutating_it() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    let h = &mut vm.host().inner;
    let ctx = Arc::clone(h.player_context().unwrap());
    h.admit_campaign_mode(
        &ctx,
        crate::campaign_mode_policy::CampaignModePolicy::fixture(),
    )
    .unwrap();
    let owner = h.executed_unit(a).unwrap().owner;
    let other = h.executed_unit(b).unwrap().owner;
    let rawcode = i32::from_be_bytes(*b"u001");
    assert!(h.ai_unit_count(4, rawcode, false).is_err());
    h.activate_campaign_ai_player(4).unwrap();
    let mut made = Vec::new();
    for x in [80., 112.] {
        let Value::Handle(unit) = h
            .native(
                "CreateUnit",
                &[
                    Value::Handle(owner),
                    Value::Int(rawcode),
                    Value::Real(x),
                    Value::Real(16.),
                    Value::Real(0.),
                ],
            )
            .unwrap()
        else {
            panic!("unit")
        };
        made.push(unit);
    }
    let snapshot: Vec<_> = h
        .executed_units()
        .map(|(id, record)| (id, record.clone()))
        .collect();
    let handles = h.inner().handles();
    for _ in 0..3 {
        assert_eq!(h.ai_unit_count(4, rawcode, false).unwrap(), 3);
        assert_eq!(h.ai_unit_count(4, rawcode, true).unwrap(), 3);
        assert_eq!(
            h.ai_player_unit_type_count(Some(other), rawcode).unwrap(),
            1
        );
        assert_eq!(h.ai_unit_count(4, 0, false).unwrap(), 0);
        assert_eq!(h.ai_player_unit_type_count(None, rawcode).unwrap(), 0);
    }
    assert_eq!(
        h.executed_units()
            .map(|(id, record)| (id, record.clone()))
            .collect::<Vec<_>>(),
        snapshot
    );
    assert_eq!(h.inner().handles(), handles);
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert_eq!(h.ai_unit_count(4, rawcode, false).unwrap(), 3);
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    assert_eq!(h.ai_unit_count(4, rawcode, false).unwrap(), 2);
    assert_eq!(h.ai_unit_count(4, rawcode, true).unwrap(), 2);
    h.native("RemoveUnit", &[Value::Handle(made[0])]).unwrap();
    assert_eq!(h.ai_unit_count(4, rawcode, true).unwrap(), 1);
    h.native(
        "SetUnitOwner",
        &[
            Value::Handle(made[1]),
            Value::Handle(other),
            Value::Bool(false),
        ],
    )
    .unwrap();
    assert_eq!(h.ai_unit_count(4, rawcode, false).unwrap(), 0);
    assert_eq!(
        h.ai_player_unit_type_count(Some(other), rawcode).unwrap(),
        2
    );
    assert!(h.ai_unit_count(3, rawcode, false).is_err());
    assert!(h.ai_player_unit_type_count(Some(a), rawcode).is_err());
}

pub(super) const DECLARATIONS: &str = r#"
constant native GetLocalPlayer takes nothing returns player
native SetPlayerAlliance takes player a, player b, alliancetype s, boolean v returns nothing
type unitstate extends handle
type playerunitevent extends eventid
type alliancetype extends handle
type playerscore extends handle
constant native ConvertPlayerScore takes integer i returns playerscore
constant native GetPlayerScore takes player p, playerscore s returns integer
constant native ConvertUnitState takes integer i returns unitstate
constant native ConvertPlayerUnitEvent takes integer i returns playerunitevent
constant native ConvertAllianceType takes integer i returns alliancetype
constant native GetUnitState takes unit u, unitstate s returns real
native SetUnitState takes unit u, unitstate s, real v returns nothing
native IssueTargetOrder takes unit u, string o, widget t returns boolean
native KillUnit takes unit u returns nothing
native UnitApplyTimedLife takes unit u, integer buffid, real duration returns nothing
native SetUnitExploded takes unit whichUnit, boolean exploded returns nothing
native SetUnitInvulnerable takes unit u, boolean flag returns nothing
native GetRandomInt takes integer a, integer b returns integer
native GetRandomReal takes real a, real b returns real
native SetRandomSeed takes integer s returns nothing
native TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event
native TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event
constant native GetDyingUnit takes nothing returns unit
constant native GetKillingUnit takes nothing returns unit
constant native GetAttacker takes nothing returns unit
constant native GetTriggerPlayer takes nothing returns player
globals
constant unitstate UNIT_STATE_LIFE = ConvertUnitState(0)
constant unitstate UNIT_STATE_MAX_LIFE = ConvertUnitState(1)
constant unitstate UNIT_STATE_MANA = ConvertUnitState(2)
constant unitstate UNIT_STATE_MAX_MANA = ConvertUnitState(3)
constant playerunitevent EVENT_PLAYER_UNIT_ATTACKED = ConvertPlayerUnitEvent(117)
constant playerunitevent EVENT_PLAYER_UNIT_DEATH = ConvertPlayerUnitEvent(20)
constant unitevent EVENT_UNIT_DEATH = ConvertUnitEvent(53)
constant unitevent EVENT_UNIT_ATTACKED = ConvertUnitEvent(54)
constant alliancetype ALLIANCE_PASSIVE = ConvertAllianceType(0)
constant alliancetype ALLIANCE_SHARED_XP = ConvertAllianceType(6)
constant playerscore PLAYER_SCORE_UNITS_TRAINED = ConvertPlayerScore(0)
constant playerscore PLAYER_SCORE_UNITS_KILLED = ConvertPlayerScore(1)
constant integer PLAYER_NEUTRAL_PASSIVE = 15
endglobals
"#;

const MAP: &str = r#"
globals
unit A=null
unit B=null
unit C=null
trigger Died=null
trigger Hit=null
endglobals
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'u001',16.0,16.0,0.0)
set B=CreateUnit(Player(5),'u001',48.0,16.0,180.0)
endfunction
function spawn takes nothing returns nothing
set C=CreateUnit(Player(4),'u001',16.0,16.0,0.0)
endfunction
function protect takes nothing returns nothing
call SetUnitInvulnerable(B,true)
endfunction
function unprotect takes nothing returns nothing
call SetUnitInvulnerable(B,false)
endfunction
function attack_filter takes nothing returns boolean
return GetFilterUnit()==B and GetTriggerUnit()==B and GetAttacker()==A
endfunction
function filter_value takes nothing returns boolexpr
return Filter(function attack_filter)
endfunction
function listen takes nothing returns nothing
set Died=CreateTrigger()
set Hit=CreateTrigger()
call TriggerRegisterUnitEvent(Died,B,EVENT_UNIT_DEATH)
call TriggerRegisterUnitEvent(Hit,B,EVENT_UNIT_ATTACKED)
endfunction
"#;

fn column(
    id: &[u8; 4],
    slk: SlkId,
    column: &str,
    kind: FieldKind,
    value: &str,
) -> ([u8; 4], ObjectField) {
    let mut field = geometry_field(Ok(FieldValue::Cell(value.into())));
    let meta = field.metadata.as_mut().unwrap();
    meta.fourcc = *id;
    meta.field = column.into();
    meta.kind = kind;
    meta.source = FieldSource::Column {
        slk,
        column: column.into(),
    };
    (*id, field)
}

pub(super) fn armed(range: &str, weapon: &str) -> Vec<([u8; 4], ObjectField)> {
    use FieldKind::{Int, Real, Token};
    use SlkId::{UnitBalance as B, UnitWeapons as W};
    vec![
        column(b"H001", B, "HP", Int, "20"),
        column(b"H002", B, "manaN", Int, "-"),
        column(b"H003", B, "mana0", Int, "-"),
        column(b"H004", B, "def", Int, "2"),
        column(b"H005", B, "defType", Token, "medium"),
        column(b"H006", B, "regenHP", Real, "0"),
        column(b"H007", B, "regenMana", Real, "-"),
        column(b"H008", B, "regenType", Token, "none"),
        column(b"W001", W, "weapsOn", Int, "1"),
        column(b"W002", W, "atkType1", Token, "pierce"),
        column(b"W003", W, "weapTp1", Token, weapon),
        column(b"W004", W, "rangeN1", Int, range),
        column(b"W005", W, "acquire", Real, "0"),
        column(b"W006", W, "cool1", Real, "1.5"),
        column(b"W007", W, "dmgpt1", Real, "0.25"),
        column(b"W008", W, "dmgplus1", Int, "4"),
        column(b"W009", W, "dice1", Int, "1"),
        column(b"W010", W, "sides1", Int, "1"),
    ]
}

pub(super) fn rules() -> CombatRules {
    let mut damage_bonus = [[1.0; 8]; 7];
    damage_bonus[1][1] = 0.75;
    CombatRules {
        sources: vec!["synthetic combat rules".into()],
        armor_factor: 0.06,
        primary_damage_per_attribute: 1.0,
        armor_per_agility: 0.3,
        hero_armor_base: -2.0,
        attack_speed_per_agility: 0.02,
        damage_bonus,
        call_for_help: 0.0,
        creep_call_for_help: 0.0,
        guard_distance: 0.0,
        max_guard_distance: 0.0,
        guard_return_time: 0.0,
    }
}

fn open(fields: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    open_selected(fields, None, rules())
}

fn open_selected(
    fields: &[([u8; 4], ObjectField)],
    heroes: Option<crate::hero_skills::Rules>,
    combat: CombatRules,
) -> (Vm, HandleId, HandleId) {
    open_fixture(fields, heroes, combat, false, None, None)
}

/// The same fixture with the source fgamestate clock and time-of-day admitted at hour 9.
fn open_clocked(
    fields: &[([u8; 4], ObjectField)],
    combat: CombatRules,
) -> (Vm, HandleId, HandleId) {
    open_fixture(fields, None, combat, true, None, None)
}

/// Clock declarations: the stock `CLOCK` handle and the stock dawn/dusk window.
const CLOCK_DECLARATIONS: &str = r#"
type fgamestate extends handle
constant native ConvertFGameState takes integer i returns fgamestate
constant native GetFloatGameState takes fgamestate s returns real
native SetFloatGameState takes fgamestate s, real v returns nothing
native SuspendTimeOfDay takes boolean b returns nothing
native SetTimeOfDayScale takes real r returns nothing
native GetTimeOfDayScale takes nothing returns real
globals
constant fgamestate CLOCK = ConvertFGameState(2)
constant real bj_TOD_DAWN = 6.00
constant real bj_TOD_DUSK = 18.00
endglobals
"#;

const SLEEP_DECLARATIONS: &str = r#"
native UnitAddSleep takes unit u, boolean add returns nothing
native UnitCanSleep takes unit u returns boolean
native UnitIsSleeping takes unit u returns boolean
native UnitWakeUp takes unit u returns nothing
"#;

const BUFF_DECLARATIONS: &str = r#"
native UnitRemoveBuffs takes unit u, boolean p, boolean n returns nothing
native UnitRemoveBuffsEx takes unit u, boolean p, boolean n, boolean m, boolean ph, boolean tl, boolean a, boolean ad returns nothing
native RemoveUnit takes unit u returns nothing
"#;

fn open_fixture(
    fields: &[([u8; 4], ObjectField)],
    heroes: Option<crate::hero_skills::Rules>,
    combat: CombatRules,
    clock: bool,
    blight: Option<crate::terrain_blight::InitialBlight>,
    resource: Option<Arc<DefaultTables>>,
) -> (Vm, HandleId, HandleId) {
    open_fixture_with(
        fields, heroes, combat, clock, blight, resource, None, None, None, None,
    )
}

#[allow(clippy::too_many_arguments)]
fn open_fixture_with(
    fields: &[([u8; 4], ObjectField)],
    heroes: Option<crate::hero_skills::Rules>,
    combat: CombatRules,
    clock: bool,
    blight: Option<crate::terrain_blight::InitialBlight>,
    resource: Option<Arc<DefaultTables>>,
    numeric: Option<crate::script_numeric::NumericPolicy>,
    flee: Option<(crate::campaign_flee_policy::FleePolicy, i32)>,
    defend: Option<Arc<DefaultTables>>,
    flee_hero_range: Option<(u8, u8)>,
) -> (Vm, HandleId, HandleId) {
    let mut extra = format!(
        "{}\n{}{DECLARATIONS}{}{}{}",
        enumeration::TRIGGERS,
        super::activity::ORDERS,
        if clock { CLOCK_DECLARATIONS } else { "" },
        if fields.iter().any(|(_, f)| f.metadata.as_ref().is_some_and(
            |m| matches!(&m.source, FieldSource::Column {column, ..} if column == "canSleep")
        )) {
            SLEEP_DECLARATIONS
        } else {
            ""
        },
        BUFF_DECLARATIONS,
    );
    let experience = heroes
        .as_ref()
        .is_some_and(|h| h.experience.as_ref().is_some_and(|r| r.awards.is_some()));
    if experience {
        extra.push_str("\nconstant native SetPlayerHandicapXP takes player p, real factor returns nothing\nconstant native GetPlayerHandicapXP takes player p returns real\n");
        extra.push_str("native SuspendHeroXP takes unit u, boolean flag returns nothing\n");
    }
    if blight.is_some() {
        extra.push_str("\nnative SetUnitPosition takes unit u, real x, real y returns nothing\n");
    }
    if fields.iter().any(|(_, f)| {
        f.metadata
            .as_ref()
            .is_some_and(|m| m.declared == "unitClass")
    }) {
        extra.push_str("type unittype extends handle\nconstant native ConvertUnitType takes integer n returns unittype\nglobals\nconstant unittype UNIT_TYPE_PEON=ConvertUnitType(16)\nendglobals\n");
    }
    if resource.is_some() {
        extra.push('\n');
        extra.push_str(super::resource::RESOURCE_FAMILY_NATIVES);
    }
    let upgrade_list = fields.iter().any(|(_, f)| {
        f.metadata.as_ref().is_some_and(
            |m| matches!(&m.source, FieldSource::Column { column, .. } if column == "upgradeList"),
        )
    });
    if upgrade_list || defend.is_some() {
        extra.push_str(
            "\nconstant native SetPlayerTechResearched takes player whichPlayer, integer techid, integer setToLevel returns nothing\n",
        );
    }
    if defend.is_some() {
        extra.push_str(
            "native IssueImmediateOrder takes unit whichUnit, string order returns boolean\n\
native SetPlayerAbilityAvailable takes player p, integer ability, boolean available returns nothing\n\
constant native GetPlayerTechResearched takes player whichPlayer, integer techid, boolean specificOnly returns boolean\n",
        );
    }
    let map = if heroes.is_some() {
        MAP.replacen("'u001'", "'H001'", 1)
    } else {
        MAP.into()
    };
    let h = position_host_admitting(1000, 16, &map, &extra, fields, |h| {
        if clock {
            h.with_startup_clock(crate::startup_clock::tests::input(9), "CLOCK", 64, 16, 1000)
                .unwrap()
        } else {
            h
        }
    });
    let h = match resource {
        Some(tables) => h.with_resource_amounts(tables).unwrap(),
        None => h,
    };
    let (registry, activity) = super::activity::rules();
    let mut h = h
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
        .with_player_scores()
        .unwrap()
        .with_order_registry(registry)
        .unwrap()
        .with_script_random(Arc::new(crate::script_random::profile::fixture()), 64)
        .unwrap();
    if upgrade_list || defend.is_some() {
        h = h
            .with_player_tech_researched(crate::playercolor_host::TechLimitBudget {
                max_records: 8,
            })
            .unwrap();
    }
    if let Some(defend) = defend {
        h = h
            .with_defend_ability(&defend, "Qord")
            .unwrap()
            .with_player_abilities(16)
            .unwrap();
    }
    if let Some(numeric) = numeric {
        h = h.with_script_numeric(numeric);
    }
    if experience {
        let (rules, numeric) = crate::player_handicap::tests::rules();
        h = h
            .with_script_numeric(numeric)
            .with_player_experience_handicap(rules)
            .unwrap();
    }
    if let Some(rules) = heroes {
        let origin = crate::scripts::AssetIdentity {
            source: source().install,
            edition: "fixture".into(),
            version: "fixture".into(),
        };
        let tables = DefaultTables::from_bytes(
            Identity {
                install: origin.source.clone(),
                edition_version: origin.version.clone(),
            },
            &[],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 0,
                max_asset_bytes: 1 << 10,
                max_aggregate_input_bytes: 1 << 10,
            },
        )
        .unwrap();
        h = h
            .with_hero_skills(
                tables,
                origin,
                Default::default(),
                rules,
                crate::playercolor_host::HeroBudget {
                    abilities_per_unit: 8,
                    overlay_objects: 0,
                    modifications_per_ability: 0,
                },
            )
            .unwrap();
    }
    h.admit_unit_activity(activity, 65536).unwrap();
    if let Some(blight) = blight {
        h.admit_initial_blight(blight).unwrap();
    }
    if clock {
        h.admit_time_of_day(&DayCycle {
            day_length: 480.0,
            day_hours: 24.0,
            sources: vec!["synthetic".into()],
        })
        .unwrap();
    }
    h.native("SetRandomSeed", &[Value::Int(7)]).unwrap();
    h.admit_unit_vitals(HeroVitals {
        life_per_strength: 25.0,
        mana_per_intelligence: 15.0,
        life_regen_per_strength: 0.0,
        mana_regen_per_intelligence: 0.0,
    })
    .unwrap();
    h.admit_combat(combat, "alpha").unwrap();
    h.set_neutral_passive(15);
    if let Some((policy, _)) = flee.as_ref() {
        let context = Arc::clone(h.player_context().unwrap());
        let mut targeting = crate::campaign_targeting_policy::TargetingPolicy::fixture();
        targeting.image_sha256 = policy.image_sha256;
        if let Some((first, last)) = flee_hero_range {
            targeting.hero_first = first;
            targeting.hero_last = last;
        }
        h.admit_campaign_targeting(&context, targeting).unwrap();
        h.admit_campaign_flee(&context, policy.clone()).unwrap();
    }
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    if let Some((_, player)) = flee {
        vm.host().inner.activate_campaign_ai_player(player).unwrap();
    }
    let id = |vm: &Vm, name| match vm.global(name) {
        Value::Handle(id) => id,
        _ => panic!("{name}"),
    };
    let (a, b) = (id(&vm, "A"), id(&vm, "B"));
    (vm, a, b)
}
fn step(vm: &mut Vm, until: f64) {
    let h = &mut vm.host().inner;
    while h.timer_game_time().unwrap() < until {
        let next = h.timer_game_time().unwrap() + 1.0 / 64.0;
        assert!(h.next_timer_expiry(next, &mut 100).unwrap().is_none());
        h.advance_unit_activity(next, &mut 65536).unwrap();
    }
}

fn step_refused(vm: &mut Vm, until: f64) -> String {
    let h = &mut vm.host().inner;
    while h.timer_game_time().unwrap() < until {
        let next = h.timer_game_time().unwrap() + 1.0 / 64.0;
        assert!(h.next_timer_expiry(next, &mut 100).unwrap().is_none());
        if let Err(error) = h.advance_unit_activity(next, &mut 65536) {
            return error.to_string();
        }
    }
    panic!("expected campaign targeting refusal");
}

fn random_snapshot(vm: &mut Vm) -> (u32, Vec<u32>) {
    let state = vm.host().inner.script_random_state().unwrap();
    (state.primary_word(), state.secondary_words().collect())
}

fn advance_until_swing(vm: &mut Vm, unit: HandleId) {
    let h = &mut vm.host().inner;
    for _ in 0..256 {
        if h.test_swing(unit).is_some() {
            return;
        }
        let next = h.timer_game_time().unwrap() + 1.0 / 64.0;
        assert!(h.next_timer_expiry(next, &mut 100).unwrap().is_none());
        h.advance_unit_activity(next, &mut 65536).unwrap();
    }
    panic!("expected a pending swing");
}

fn advance_until_melee_refusal(vm: &mut Vm) -> (u32, Vec<u32>, String) {
    let h = &mut vm.host().inner;
    for _ in 0..256 {
        let before = (
            h.script_random_state().unwrap().primary_word(),
            h.script_random_state().unwrap().secondary_words().collect(),
        );
        let next = h.timer_game_time().unwrap() + 1.0 / 64.0;
        assert!(h.next_timer_expiry(next, &mut 100).unwrap().is_none());
        if let Err(error) = h.advance_unit_activity(next, &mut 65536) {
            return (before.0, before.1, error.to_string());
        }
    }
    panic!("expected a melee refusal");
}

fn retry_melee_refusal(vm: &mut Vm) -> (u32, Vec<u32>, String) {
    let h = &mut vm.host().inner;
    let before = (
        h.script_random_state().unwrap().primary_word(),
        h.script_random_state().unwrap().secondary_words().collect(),
    );
    let next = h.timer_game_time().unwrap() + 1.0 / 64.0;
    assert!(h.next_timer_expiry(next, &mut 100).unwrap().is_none());
    let error = h
        .advance_unit_activity(next, &mut 65536)
        .expect_err("repeat refusal");
    (before.0, before.1, error.to_string())
}

#[test]
fn killed_resource_unit_retains_amount_across_later_activity() {
    let mut fields = armed("1", "normal");
    fields.extend(super::resource::resource_fields("resource", "Agld,Avul"));
    let (mut vm, a, _) = open_fixture(
        &fields,
        None,
        rules(),
        false,
        None,
        Some(super::resource::resource_tables()),
    );
    let amount = |vm: &mut Vm| vm.host().inner.executed_unit(a).unwrap().resource_amount;
    assert_eq!(amount(&mut vm), Some(4242));
    vm.host()
        .inner
        .native("SetResourceAmount", &[Value::Handle(a), Value::Int(777)])
        .unwrap();
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(a)])
        .unwrap();
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
    assert_eq!(amount(&mut vm), Some(777));
    step(&mut vm, 1.0);
    assert_eq!(amount(&mut vm), Some(777));
    vm.host()
        .inner
        .native("SetResourceAmount", &[Value::Handle(a), Value::Int(5)])
        .unwrap();
    vm.host()
        .inner
        .native("AddResourceAmount", &[Value::Handle(a), Value::Int(2)])
        .unwrap();
    assert_eq!(amount(&mut vm), Some(7));
}

#[test]
fn melee_attacks_follow_the_source_cycle_and_kill_with_death_events() {
    let mut fields = armed("64", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "8",
    ));
    let (mut vm, a, b) = open(&fields);
    let life = |vm: &mut Vm, u| vm.host().inner.unit_vitals(u).unwrap().unwrap().life;
    assert_eq!(life(&mut vm, b), 20.0);
    let h = &mut vm.host().inner;
    let order = |h: &mut ObjectHost, target| {
        h.native(
            "IssueTargetOrder",
            &[Value::Handle(a), Value::Str("alpha".into()), target],
        )
    };
    assert_eq!(order(h, Value::Null).unwrap(), Value::Bool(false));
    assert!(
        h.native(
            "IssueTargetOrder",
            &[
                Value::Handle(a),
                Value::Str("beta".into()),
                Value::Handle(b)
            ]
        )
        .is_err()
    );
    assert_eq!(order(h, Value::Handle(b)).unwrap(), Value::Bool(true));
    // Pierce against medium (0.75) and 2 armor: (4 + 1) * 0.75 * (1 - 0.12 / 1.12).
    let per_hit = 5.0 * 0.75 * (1.0 - 0.12 / 1.12);
    let start = vm.host().inner.timer_game_time().unwrap();
    step(&mut vm, start + 0.3);
    assert!((life(&mut vm, b) - (20.0 - per_hit)).abs() < 1e-4);
    // The next swing waits for the cooldown, not the damage point.
    step(&mut vm, start + 1.4);
    assert!((life(&mut vm, b) - (20.0 - per_hit)).abs() < 1e-4);
    step(&mut vm, start + 1.8);
    assert!((life(&mut vm, b) - (20.0 - 2.0 * per_hit)).abs() < 1e-4);
    // Damage below zero kills once; a dead unit is no longer a valid target.
    step(&mut vm, start + 12.0);
    let state = vm_global(&vm, "UNIT_STATE_LIFE");
    let h = &mut vm.host().inner;
    assert!(h.executed_unit(b).unwrap().combat.as_ref().unwrap().dead);
    assert_eq!(h.unit_vitals(b).unwrap().unwrap().life, 0.0);
    assert_eq!(order(h, Value::Handle(b)).unwrap(), Value::Bool(false));
    assert_eq!(
        h.native("GetUnitState", &[Value::Handle(b), state])
            .unwrap(),
        Value::Real(0.0)
    );
}

#[test]
fn pause_freezes_a_pending_attack_but_the_paused_target_still_takes_damage() {
    let fields = armed("64", "normal");
    let (mut vm, a, b) = open(&fields);
    let (mut control, c, d) = open(&fields);
    for (vm, attacker, target) in [(&mut vm, a, b), (&mut control, c, d)] {
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Handle(target), Value::Bool(true)])
            .unwrap();
        assert_eq!(
            vm.host()
                .inner
                .native(
                    "IssueTargetOrder",
                    &[
                        Value::Handle(attacker),
                        Value::Str("alpha".into()),
                        Value::Handle(target)
                    ]
                )
                .unwrap(),
            Value::Bool(true)
        );
        step(vm, 0.125);
    }
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("alpha".into()),
                    Value::Handle(b)
                ]
            )
            .unwrap(),
        Value::Bool(false)
    );
    step(&mut vm, 3.125);
    assert_eq!(pools(&mut vm, b).0, 20.);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    step(&mut vm, 3.3);
    step(&mut control, 0.3);
    assert_eq!(pools(&mut vm, b), pools(&mut control, d));
    assert!(pools(&mut vm, b).0 < 20.);
    step(&mut vm, 4.8);
    step(&mut control, 1.8);
    assert_eq!(pools(&mut vm, b), pools(&mut control, d));
}

fn vm_global(vm: &Vm, name: &str) -> Value {
    vm.global(name)
}

#[test]
fn missiles_land_after_travel_and_lethal_state_and_kill_unit_reuse_death() {
    let mut fields = armed("640", "missile");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "8",
    ));
    let mut speed = column(
        b"M002",
        SlkId::UnitData,
        "Missilespeed",
        FieldKind::Int,
        "64",
    );
    speed.1.metadata.as_mut().unwrap().source = FieldSource::Profile {
        key: "missilespeed".into(),
        index: 0,
        slots: 1,
    };
    fields.push(speed);
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    h.native(
        "IssueTargetOrder",
        &[
            Value::Handle(a),
            Value::Str("alpha".into()),
            Value::Handle(b),
        ],
    )
    .unwrap();
    let start = h.timer_game_time().unwrap();
    // Swing at 0.25 s, then 32 units at 64/s: no damage until ~0.75 s.
    step(&mut vm, start + 0.6);
    let life = |vm: &mut Vm| vm.host().inner.unit_vitals(b).unwrap().unwrap().life;
    assert_eq!(life(&mut vm), 20.0);
    for unit in [a, b] {
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Handle(unit), Value::Bool(true)])
            .unwrap();
    }
    step(&mut vm, start + 0.8);
    assert!(life(&mut vm) < 20.0);
    let state = vm_global(&vm, "UNIT_STATE_LIFE");
    let h = &mut vm.host().inner;
    h.native(
        "SetUnitState",
        &[Value::Handle(b), state, Value::Real(-1.0)],
    )
    .unwrap();
    assert!(h.executed_unit(b).unwrap().combat.as_ref().unwrap().dead);
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    h.native("KillUnit", &[Value::Null]).unwrap();
    assert!(h.executed_unit(a).unwrap().combat.as_ref().unwrap().dead);
}

#[test]
fn attacks_and_deaths_queue_their_registered_triggers_with_event_responses() {
    let mut fields = armed("64", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "8",
    ));
    let (mut vm, a, b) = open(&fields);
    vm.call("listen", &[]).unwrap();
    let (Value::Handle(died), Value::Handle(hit)) = (vm.global("Died"), vm.global("Hit")) else {
        panic!("triggers")
    };
    let h = &mut vm.host().inner;
    h.native(
        "IssueTargetOrder",
        &[
            Value::Handle(a),
            Value::Str("alpha".into()),
            Value::Handle(b),
        ],
    )
    .unwrap();
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    let h = &mut vm.host().inner;
    let fired = h.take_fired_event().expect("attacked event");
    assert_eq!(fired.trigger, hit);
    assert!(h.take_fired_event().is_none());
    // Delivery installs the attacked unit and attacker as event responses.
    let delivery = h
        .begin_fired_event(&fired)
        .unwrap()
        .expect("enabled trigger");
    assert!(delivery.actions.is_empty());
    assert_eq!(h.native("GetTriggerUnit", &[]).unwrap(), Value::Handle(b));
    assert_eq!(h.native("GetAttacker", &[]).unwrap(), Value::Handle(a));
    assert!(
        h.native("GetTriggerPlayer", &[]).is_err(),
        "ordinary unit event has no admitted player response"
    );
    h.finish_fired_event();
    assert_eq!(h.native("GetAttacker", &[]).unwrap(), Value::Null);
    h.native("KillUnit", &[Value::Handle(b)]).unwrap();
    let mut died_events = Vec::new();
    while let Some(event) = h.take_fired_event() {
        died_events.push(event);
    }
    assert_eq!(died_events.len(), 1);
    assert_eq!(died_events[0].trigger, died);
    h.begin_fired_event(&died_events[0]).unwrap().unwrap();
    assert_eq!(h.native("GetDyingUnit", &[]).unwrap(), Value::Handle(b));
    assert_eq!(h.native("GetKillingUnit", &[]).unwrap(), Value::Null);
    h.finish_fired_event();
}

#[test]
fn selection_relation_tracks_local_owner_passive_owner_and_directional_alliances() {
    use crate::playercolor_host::Relation;
    let (mut vm, a, b) = open(&armed("100", "normal"));
    let alliance = vm.global("ALLIANCE_PASSIVE");
    let h = &mut vm.host().inner;
    assert_eq!(h.local_relation(a), None);
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_local_player(&context, 4).unwrap();
    assert_eq!(h.local_relation(a), Some(Relation::Friend));
    assert_eq!(h.local_relation(b), Some(Relation::Enemy));
    let local = h.native("Player", &[Value::Int(4)]).unwrap();
    let other = h.native("Player", &[Value::Int(5)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            other.clone(),
            local.clone(),
            alliance.clone(),
            Value::Bool(true),
        ],
    )
    .unwrap();
    assert_eq!(h.local_relation(b), Some(Relation::Enemy));
    h.native(
        "SetPlayerAlliance",
        &[local, other, alliance, Value::Bool(true)],
    )
    .unwrap();
    assert_eq!(h.local_relation(b), Some(Relation::Friend));
    h.set_neutral_passive(5);
    assert_eq!(h.local_relation(b), Some(Relation::Neutral));
}

#[test]
fn player_attacked_events_match_the_current_owner_and_keep_filters_and_event_context() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    let filter = vm.call("filter_value", &[]).unwrap();
    let attacked = vm.global("EVENT_PLAYER_UNIT_ATTACKED");
    let death = vm.global("EVENT_PLAYER_UNIT_DEATH");
    let h = &mut vm.host().inner;
    let victim = Value::Handle(h.executed_unit(b).unwrap().owner);
    let attacker = Value::Handle(h.executed_unit(a).unwrap().owner);
    let mut matching = Vec::new();
    for (owner, kind, predicate) in [
        (victim.clone(), attacked.clone(), Value::Null),
        (attacker, attacked.clone(), Value::Null),
        (victim.clone(), death, Value::Null),
        (victim.clone(), attacked.clone(), filter),
        (victim.clone(), attacked.clone(), Value::Null),
        (victim, attacked.clone(), Value::Null),
    ] {
        let trigger = h.native("CreateTrigger", &[]).unwrap();
        h.native(
            "TriggerRegisterPlayerUnitEvent",
            &[trigger.clone(), owner, kind, predicate],
        )
        .unwrap();
        matching.push(trigger);
    }
    h.native(
        "IssueTargetOrder",
        &[
            Value::Handle(a),
            Value::Str("alpha".into()),
            Value::Handle(b),
        ],
    )
    .unwrap();
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    let h = &mut vm.host().inner;
    // Pending events still respect changes to their trigger before delivery.
    h.native("DisableTrigger", &[matching[4].clone()]).unwrap();
    h.native("DestroyTrigger", &[matching[5].clone()]).unwrap();
    let first = h.take_fired_event().unwrap();
    assert_eq!(Value::Handle(first.trigger), matching[0]);
    assert!(first.filter.is_none());
    h.begin_fired_event(&first).unwrap().unwrap();
    assert_eq!(h.native("GetTriggerEventId", &[]).unwrap(), attacked);
    assert_eq!(h.native("GetTriggerUnit", &[]).unwrap(), Value::Handle(b));
    assert_eq!(h.native("GetAttacker", &[]).unwrap(), Value::Handle(a));
    let registered_player = Value::Handle(h.executed_unit(b).unwrap().owner);
    assert_eq!(
        h.native("GetTriggerPlayer", &[]).unwrap(),
        registered_player
    );
    h.finish_fired_event();
    let filtered = h.take_fired_event().unwrap();
    assert_eq!(Value::Handle(filtered.trigger), matching[3]);
    assert!(filtered.filter.is_some());
    h.begin_fired_event(&filtered).unwrap().unwrap();
    assert_eq!(vm.call("attack_filter", &[]).unwrap(), Value::Bool(true));
    let h = &mut vm.host().inner;
    h.finish_fired_event();
    for expected in &matching[4..] {
        let fired = h.take_fired_event().unwrap();
        assert_eq!(Value::Handle(fired.trigger), *expected);
        assert!(h.begin_fired_event(&fired).unwrap().is_none());
    }
    assert!(h.take_fired_event().is_none());
    assert_eq!(h.native("GetAttacker", &[]).unwrap(), Value::Null);
    assert_eq!(h.native("GetFilterUnit", &[]).unwrap(), Value::Null);
    h.native("KillUnit", &[Value::Handle(b)]).unwrap();
    let death = h.take_fired_event().unwrap();
    assert_eq!(Value::Handle(death.trigger), matching[2]);
    h.begin_fired_event(&death).unwrap().unwrap();
    assert_eq!(
        h.native("GetTriggerPlayer", &[]).unwrap(),
        registered_player
    );
    h.finish_fired_event();
    assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), Value::Null);
}

#[test]
fn point_attack_acquires_while_moving_and_resumes_then_finishes_its_destination() {
    let mut fields = armed("8", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("40".into()));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    // Keep the target from acquiring/chasing so the attack-move must find it.
    let alliance = h.native("ConvertAllianceType", &[Value::Int(0)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            Value::Handle(h.executed_unit(b).unwrap().owner),
            Value::Handle(h.executed_unit(a).unwrap().owner),
            alliance,
            Value::Bool(true),
        ],
    )
    .unwrap();
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Real(96.0),
                Value::Real(16.0)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(b)
    );
    step(&mut vm, start + 14.0);
    let h = &mut vm.host().inner;
    assert!(h.executed_unit(b).unwrap().combat.as_ref().unwrap().dead);
    assert_eq!(h.executed_unit(a).unwrap().position, Some([96.0, 16.0]));
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(0)
    );
    assert!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination
            .is_none()
    );
}

#[test]
fn replacement_orders_cancel_the_retained_attack_move_destination() {
    let mut fields = armed("8", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    for (name, tail) in [
        ("IssueImmediateOrder", vec![Value::Str("beta".into())]),
        ("IssueImmediateOrder", vec![Value::Str("alpha".into())]),
        (
            "IssuePointOrder",
            vec![
                Value::Str("gamma".into()),
                Value::Real(80.0),
                Value::Real(32.0),
            ],
        ),
        (
            "IssueTargetOrder",
            vec![Value::Str("alpha".into()), Value::Handle(b)],
        ),
    ] {
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Real(96.0),
                Value::Real(16.0),
            ],
        )
        .unwrap();
        assert!(
            h.executed_unit(a)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .attack_destination
                .is_some()
        );
        let args = [vec![Value::Handle(a)], tail].concat();
        assert_eq!(h.native(name, &args).unwrap(), Value::Bool(true));
        assert!(
            h.executed_unit(a)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .attack_destination
                .is_none()
        );
    }
}

#[test]
fn attack_move_point_and_location_variants_copy_the_admitted_destination() {
    for name in [
        "IssuePointOrderById",
        "IssuePointOrderLoc",
        "IssuePointOrderByIdLoc",
    ] {
        let mut fields = armed("8", "normal");
        fields.push(column(
            b"S001",
            SlkId::UnitBalance,
            "spd",
            FieldKind::Real,
            "32",
        ));
        let (mut vm, a, _) = open(&fields);
        let h = &mut vm.host().inner;
        let location = h
            .native("Location", &[Value::Real(96.0), Value::Real(48.0)])
            .unwrap();
        let order = if name.contains("ById") {
            h.native("OrderId", &[Value::Str("alpha".into())]).unwrap()
        } else {
            Value::Str("alpha".into())
        };
        let mut args = vec![Value::Handle(a), order];
        if name.ends_with("Loc") {
            args.push(location.clone());
        } else {
            args.extend([Value::Real(96.0), Value::Real(48.0)]);
        }
        assert_eq!(h.native(name, &args).unwrap(), Value::Bool(true));
        h.native(
            "MoveLocation",
            &[location.clone(), Value::Real(112.0), Value::Real(80.0)],
        )
        .unwrap();
        h.native("RemoveLocation", &[location]).unwrap();
        let start = h.timer_game_time().unwrap();
        step(&mut vm, start + 5.0);
        let h = &mut vm.host().inner;
        assert_eq!(h.executed_unit(a).unwrap().position, Some([96.0, 48.0]));
        assert_eq!(
            h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
                .unwrap(),
            Value::Int(0)
        );
    }
}

#[test]
fn invalid_attack_move_arguments_preserve_the_previous_activity_and_combat() {
    let mut fields = armed("8", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    let location = h
        .native("Location", &[Value::Real(96.0), Value::Real(48.0)])
        .unwrap();
    h.native(
        "IssueTargetOrder",
        &[
            Value::Handle(a),
            Value::Str("alpha".into()),
            Value::Handle(b),
        ],
    )
    .unwrap();
    let before = h.executed_unit(a).unwrap().clone();
    for (unit, destination, expected) in [
        (location.clone(), location, "unit argument"),
        (Value::Handle(a), Value::Handle(b), "location argument"),
    ] {
        let error = h
            .native(
                "IssuePointOrderLoc",
                &[unit, Value::Str("alpha".into()), destination],
            )
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
        assert_eq!(h.executed_unit(a).unwrap().activity, before.activity);
        assert_eq!(h.executed_unit(a).unwrap().combat, before.combat);
    }
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Real(96.0),
                Value::Real(48.0)
            ]
        )
        .unwrap(),
        Value::Bool(false)
    );
    assert!(
        h.executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .attack_destination
            .is_none()
    );
}

#[test]
fn unarmed_attack_move_finishes_without_acquiring_a_target() {
    let mut fields = armed("8", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W001")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("0".into()));
    let (mut vm, a, _) = open(&fields);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Real(96.0),
                Value::Real(48.0)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 5.0);
    let h = &mut vm.host().inner;
    assert_eq!(h.executed_unit(a).unwrap().position, Some([96.0, 48.0]));
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(0)
    );
}

fn hero_fields(primary: &str) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed("64", "normal");
    for (id, name, kind, value) in [
        (b"A001", "STR", FieldKind::Int, "10"),
        (b"A002", "AGI", FieldKind::Int, "5"),
        (b"A003", "INT", FieldKind::Int, "7"),
        (b"A004", "STRplus", FieldKind::Unreal, "2.5"),
        (b"A005", "AGIplus", FieldKind::Unreal, "1.5"),
        (b"A006", "INTplus", FieldKind::Unreal, "0.75"),
        (b"A007", "Primary", FieldKind::Token, primary),
        (b"H001", "HP", FieldKind::Int, "500"),
    ] {
        fields.push(column(id, SlkId::UnitBalance, name, kind, value));
    }
    for (id, name) in [(b"A008", "abilList"), (b"A009", "heroAbilList")] {
        fields.push(column(
            id,
            SlkId::UnitAbilities,
            name,
            FieldKind::Token,
            "_",
        ));
    }
    fields
}

fn hero_rules() -> (crate::hero_skills::Rules, CombatRules) {
    let mut hero = crate::hero_skills::tests::rules();
    hero.initial_level = 3;
    let mut combat = rules();
    combat.primary_damage_per_attribute = 1.75;
    combat.armor_per_agility = 0.4;
    combat.hero_armor_base = -1.0;
    combat.attack_speed_per_agility = 0.2;
    (hero, combat)
}

#[test]
fn hero_primary_types_fractional_growth_and_source_baseline_follow_live_levels() {
    for (primary, initial, fourth, fifth) in [
        ("STR", 10.0, 12.0, 15.0),
        ("AGI", 5.0, 6.0, 8.0),
        ("INT", 7.0, 7.0, 8.0),
    ] {
        let (hero, combat) = hero_rules();
        let (mut vm, a, b) = open_selected(&hero_fields(primary), Some(hero), combat);
        let initial_bonuses = vm.host().inner.hero_combat_bonuses(a).unwrap();
        assert_eq!(initial_bonuses.damage, initial * 1.75);
        assert_eq!((initial_bonuses.armor, initial_bonuses.speed), (1.0, 2.0));
        let ordinary = vm.host().inner.hero_combat_bonuses(b).unwrap();
        assert_eq!(
            (ordinary.damage, ordinary.armor, ordinary.speed),
            (0.0, 0.0, 1.0)
        );
        for (level, damage, agility) in [(4, fourth, 6.0), (5, fifth, 8.0), (5, fifth, 8.0)] {
            vm.host()
                .inner
                .native(
                    "SetHeroLevel",
                    &[Value::Handle(a), Value::Int(level), Value::Bool(false)],
                )
                .unwrap();
            let bonuses = vm.host().inner.hero_combat_bonuses(a).unwrap();
            assert_eq!(bonuses.damage, damage * 1.75);
            assert!((bonuses.armor - (-1.0 + agility * 0.4)).abs() < 0.00001);
            assert!((bonuses.speed - (1.0 + agility * 0.2)).abs() < 0.00001);
        }
    }
}

#[test]
fn hero_attack_damage_point_cooldown_and_target_armor_use_the_derived_bonuses() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, b) = open_selected(&hero_fields("STR"), Some(hero), combat);
    for (source, target) in [(a, b), (b, a)] {
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(source),
                    Value::Str("alpha".into()),
                    Value::Handle(target),
                ],
            )
            .unwrap();
    }
    let life = |vm: &mut Vm, unit| vm.host().inner.unit_vitals(unit).unwrap().unwrap().life;
    let initial_a = life(&mut vm, a);
    let initial_b = life(&mut vm, b);
    step(&mut vm, 0.125);
    assert_eq!(life(&mut vm, b), initial_b);
    step(&mut vm, 0.15625);
    let damage_b = initial_b - life(&mut vm, b);
    let expected_b = (5.0 + 10.0 * 1.75) * 0.75 / (1.0 + 2.0 * 0.06);
    assert!((damage_b - expected_b).abs() < 0.0001);
    assert_eq!(life(&mut vm, a), initial_a);
    step(&mut vm, 0.28125);
    let expected_a = 5.0 * 0.75 / (1.0 + (2.0 - 1.0 + 5.0 * 0.4) * 0.06);
    assert!((initial_a - life(&mut vm, a) - expected_a).abs() < 0.0001);
    step(&mut vm, 0.875);
    assert!((initial_b - life(&mut vm, b) - expected_b).abs() < 0.0001);
    step(&mut vm, 0.90625);
    assert!((initial_b - life(&mut vm, b) - 2.0 * expected_b).abs() < 0.0001);
}

#[test]
fn missing_hero_state_or_attributes_and_invalid_derived_speed_refuse() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, b) = open_selected(&hero_fields("AGI"), Some(hero.clone()), combat.clone());
    let definition = vm.host().inner.executed_unit(a).unwrap().definition.clone();
    let mut invalid = combat;
    invalid.attack_speed_per_agility = -1.0;
    assert!(
        ObjectHost::derive_hero_combat(&definition, Some((3, 3)), Default::default(), &invalid)
            .unwrap_err()
            .contains("non-positive attack speed")
    );
    for (field, value) in [
        (b"A005", None),
        (b"A007", Some("UNKNOWN")),
        (b"A002", Some("NaN")),
    ] {
        let mut bad = definition.clone();
        if let Some(value) = value {
            bad.fields.get_mut(field).unwrap().value = Ok(FieldValue::Cell(value.into()));
        } else {
            bad.fields.remove(field);
        }
        assert!(
            ObjectHost::derive_hero_combat(&bad, Some((3, 4)), Default::default(), &rules())
                .is_err()
        );
    }
    assert!(
        ObjectHost::derive_hero_combat(&definition, Some((3, 2)), Default::default(), &rules())
            .is_err()
    );
    let ordinary = vm.host().inner.executed_unit(b).unwrap().definition.clone();
    assert_eq!(
        ObjectHost::derive_hero_combat(&ordinary, None, Default::default(), &invalid)
            .unwrap()
            .speed,
        1.0
    );
    assert!(
        ObjectHost::derive_hero_combat(&definition, None, Default::default(), &rules())
            .unwrap_err()
            .contains("admitted initial and current levels")
    );
}

#[test]
fn explicit_protection_rejects_new_attacks_and_cancels_swings_without_erasing_queued_events() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    assert_eq!(vm.host().inner.executed_unit(b).unwrap().invulnerable, None);
    vm.call("listen", &[]).unwrap();
    vm.call("protect", &[]).unwrap();
    let attack = |vm: &mut Vm| {
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("alpha".into()),
                    Value::Handle(b),
                ],
            )
            .unwrap()
    };
    assert_eq!(attack(&mut vm), Value::Bool(false));
    step(&mut vm, 0.5);
    assert_eq!(vm.host().inner.unit_vitals(b).unwrap().unwrap().life, 20.0);
    assert!(vm.host().inner.take_fired_event().is_none());
    vm.call("unprotect", &[]).unwrap();
    assert_eq!(attack(&mut vm), Value::Bool(true));
    step(&mut vm, 0.6);
    vm.call("protect", &[]).unwrap();
    step(&mut vm, 0.9);
    let Value::Handle(hit) = vm.global("Hit") else {
        panic!("trigger");
    };
    let fired = vm
        .host()
        .inner
        .take_fired_event()
        .expect("earlier attacked event retained");
    assert_eq!(fired.trigger, hit);
    assert!(vm.host().inner.take_fired_event().is_none());
    assert_eq!(vm.host().inner.unit_vitals(b).unwrap().unwrap().life, 20.0);
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target
            .is_none()
    );
    vm.call("unprotect", &[]).unwrap();
    assert_eq!(attack(&mut vm), Value::Bool(true));
    step(&mut vm, 2.5);
    assert!(vm.host().inner.unit_vitals(b).unwrap().unwrap().life < 20.0);
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().invulnerable, None);
}

#[test]
fn acquisition_keeps_first_handle_on_equal_distance_with_nonunit_gaps() {
    for held in [false, true] {
        let mut fields = armed("128", "normal");
        fields
            .iter_mut()
            .find(|(id, _)| id == b"W005")
            .unwrap()
            .1
            .value = Ok(FieldValue::Cell("256".into()));
        let (mut vm, a, b) = open(&fields);
        let h = &mut vm.host().inner;
        for _ in 0..16 {
            h.native("CreateTimer", &[]).unwrap();
        }
        let owner = h.executed_unit(b).unwrap().owner;
        let Value::Handle(c) = h
            .native(
                "CreateUnit",
                &[
                    Value::Handle(owner),
                    Value::Int(i32::from_be_bytes(*b"u001")),
                    Value::Real(16.0),
                    Value::Real(48.0),
                    Value::Real(0.0),
                ],
            )
            .unwrap()
        else {
            panic!("unit")
        };
        if held {
            assert_eq!(
                h.native(
                    "IssueImmediateOrder",
                    &[Value::Handle(a), Value::Str("alpha".into())]
                )
                .unwrap(),
                Value::Bool(true)
            );
        }
        step(&mut vm, 1.0 / 64.0);
        assert_eq!(
            vm.host()
                .inner
                .executed_unit(a)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .target,
            Some(b)
        );
        vm.host()
            .inner
            .native("KillUnit", &[Value::Handle(b)])
            .unwrap();
        step(&mut vm, 2.0 / 64.0);
        assert_eq!(
            vm.host()
                .inner
                .executed_unit(a)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .target,
            Some(c)
        );
    }
}

#[test]
fn acquisition_rechecks_a_target_killed_earlier_in_the_same_combat_step() {
    let mut fields = armed("128", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("256".into()));
    let (mut vm, a, b) = open(&fields);
    let life = vm.global("UNIT_STATE_LIFE");
    let h = &mut vm.host().inner;
    let (ally, enemy) = (
        h.executed_unit(a).unwrap().owner,
        h.executed_unit(b).unwrap().owner,
    );
    let passive = h.native("ConvertAllianceType", &[Value::Int(0)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            Value::Handle(enemy),
            Value::Handle(ally),
            passive,
            Value::Bool(true),
        ],
    )
    .unwrap();
    h.native("SetUnitState", &[Value::Handle(b), life, Value::Real(1.0)])
        .unwrap();
    let mut create = |owner, x, y| {
        let Value::Handle(id) = h
            .native(
                "CreateUnit",
                &[
                    Value::Handle(owner),
                    Value::Int(i32::from_be_bytes(*b"u001")),
                    Value::Real(x),
                    Value::Real(y),
                    Value::Real(0.0),
                ],
            )
            .unwrap()
        else {
            panic!("unit")
        };
        id
    };
    let c = create(ally, 16.0, 48.0);
    let d = create(enemy, 80.0, 48.0);
    h.enable_combat_trace(64);
    step(&mut vm, 0.3);
    let h = &vm.host().inner;
    assert!(h.executed_unit(b).unwrap().combat.as_ref().unwrap().dead);
    assert_eq!(
        h.executed_unit(c).unwrap().combat.as_ref().unwrap().target,
        Some(d)
    );
    let events = h.combat_trace().unwrap().events();
    let (hit, clock) = events
        .iter()
        .enumerate()
        .find_map(|(i, e)| match e {
            CombatTraceEvent::Damage {
                source,
                target,
                clock,
                ..
            } if source.unit == a && target.unit == b => Some((i, *clock)),
            _ => None,
        })
        .expect("A kills B before C's turn");
    assert!(events[hit + 1..].iter().any(|e| matches!(e,
        CombatTraceEvent::Acquired { source, target, clock: at } if source.unit == c && target.unit == d && *at == clock
    )));
}

#[test]
fn auto_acquisition_skips_explicit_protection_and_cleared_protection_becomes_eligible() {
    let mut fields = armed("128", "normal");
    fields.push(column(
        b"W005",
        SlkId::UnitWeapons,
        "acquire",
        FieldKind::Real,
        "256",
    ));
    let (mut vm, a, b) = open(&fields);
    vm.call("protect", &[]).unwrap();
    let player = vm.host().inner.executed_unit(b).unwrap().owner;
    let Value::Handle(other) = vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                Value::Handle(player),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(80.0),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit");
    };
    step(&mut vm, 0.1);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(other)
    );
    assert!(
        vm.host().inner.is_enemy(a, b),
        "protection does not change hostility"
    );
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(other)])
        .unwrap();
    vm.call("unprotect", &[]).unwrap();
    step(&mut vm, 0.2);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(b)
    );
}

#[test]
fn rejected_protected_order_preserves_the_accepted_order_and_protection_stops_a_chase() {
    let (mut vm, a, b) = open(&armed("4", "normal"));
    let player = vm.host().inner.executed_unit(b).unwrap().owner;
    let Value::Handle(other) = vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                Value::Handle(player),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(96.0),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit");
    };
    let attack = |vm: &mut Vm, target| {
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("alpha".into()),
                    Value::Handle(target),
                ],
            )
            .unwrap()
    };
    assert_eq!(attack(&mut vm, other), Value::Bool(true));
    vm.call("protect", &[]).unwrap();
    assert_eq!(attack(&mut vm, b), Value::Bool(false));
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(other)
    );
    vm.call("unprotect", &[]).unwrap();
    assert_eq!(attack(&mut vm, b), Value::Bool(true));
    step(&mut vm, 0.1);
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    vm.call("protect", &[]).unwrap();
    step(&mut vm, 0.2);
    let stopped = vm.host().inner.executed_unit(a).unwrap().position;
    step(&mut vm, 0.5);
    let attacker = vm.host().inner.executed_unit(a).unwrap();
    assert_eq!(attacker.position, stopped);
    assert!(!attacker.activity.as_ref().unwrap().moving());
}

#[test]
fn in_flight_missile_uses_current_explicit_protection_at_impact() {
    let mut fields = armed("640", "missile");
    let mut speed = column(
        b"M002",
        SlkId::UnitData,
        "Missilespeed",
        FieldKind::Int,
        "64",
    );
    speed.1.metadata.as_mut().unwrap().source = FieldSource::Profile {
        key: "missilespeed".into(),
        index: 0,
        slots: 1,
    };
    fields.push(speed);
    for clear_before_impact in [false, true] {
        let (mut vm, a, b) = open(&fields);
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("alpha".into()),
                    Value::Handle(b),
                ],
            )
            .unwrap();
        step(&mut vm, 0.4);
        assert_eq!(vm.host().inner.unit_vitals(b).unwrap().unwrap().life, 20.0);
        vm.call("protect", &[]).unwrap();
        step(&mut vm, 0.6);
        if clear_before_impact {
            vm.call("unprotect", &[]).unwrap();
        }
        step(&mut vm, 0.8);
        let life = vm.host().inner.unit_vitals(b).unwrap().unwrap().life;
        if clear_before_impact {
            assert!(life < 20.0);
        } else {
            assert_eq!(life, 20.0);
        }
        assert_eq!(
            vm.host().inner.executed_unit(b).unwrap().invulnerable,
            Some(!clear_before_impact)
        );
    }
}

#[test]
fn explicit_protection_does_not_override_script_kill_or_direct_life_assignment() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    vm.call("protect", &[]).unwrap();
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(b)])
        .unwrap();
    assert!(
        vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(a), Value::Bool(true)],
        )
        .unwrap();
    let state = vm.global("UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(a), state, Value::Real(-1.0)],
        )
        .unwrap();
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[path = "unit_hero_vital_tests.rs"]
mod hero_vital_updates;

#[path = "unit_guard_tests.rs"]
mod guard_positions;

#[cfg(test)]
#[path = "inventory_powerup_host_tests.rs"]
pub(super) mod powerup_host;

#[test]
fn set_unit_exploded_then_kill_unit_retains_the_flag_through_the_death_path() {
    // Melee: the missile path needs a positive missile speed, which this test does not model.
    let (mut vm, a, _b) = open(&armed("100", "normal"));
    let h = &mut vm.host().inner;
    assert_eq!(h.unit_exploded(a), None);
    h.native("SetUnitExploded", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    assert_eq!(h.unit_exploded(a), Some(true));
    h.native("KillUnit", &[Value::Handle(a)]).unwrap();
    assert!(h.executed_unit(a).unwrap().combat.as_ref().unwrap().dead);
    assert_eq!(
        h.unit_exploded(a),
        Some(true),
        "the flag outlives the death"
    );
    // The fixture unit has no model, so no explosion sequence exists: the death is recorded unrendered.
    assert!(h.exploded_death_unrendered(a));
    // Unverified policy: a dead unit is accepted. This pins the current host behavior.
    h.native("SetUnitExploded", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert_eq!(h.unit_exploded(a), Some(false));
}

#[test]
fn hold_position_attacks_only_within_reach_and_never_chases_a_departing_target() {
    // Reach is range 40 plus both collision radii. Acquire 400 would select B for an idle unit.
    let mut fields = armed("40", "normal");
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        "32",
    ));
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("400".into()));
    let (mut vm, a, b) = open(&fields);
    let target = |h: &ObjectHost, unit: HandleId| {
        h.executed_unit(unit)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target
    };
    let h = &mut vm.host().inner;
    // B must not acquire A itself; only A's acquisition of B is under test.
    let alliance = h.native("ConvertAllianceType", &[Value::Int(0)]).unwrap();
    h.native(
        "SetPlayerAlliance",
        &[
            Value::Handle(h.executed_unit(b).unwrap().owner),
            Value::Handle(h.executed_unit(a).unwrap().owner),
            alliance,
            Value::Bool(true),
        ],
    )
    .unwrap();
    // In this fixture "alpha" is both Hold and the attack order; this test issues no attack.
    assert_eq!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Handle(a), Value::Str("alpha".into())]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    assert_eq!(
        target(&vm.host().inner, a),
        Some(b),
        "B at 32 is within reach"
    );
    // B walks away at 32 per second. A holds its position and drops B once B is out of reach.
    let h = &mut vm.host().inner;
    let walk = [
        Value::Handle(b),
        Value::Str("gamma".into()),
        Value::Real(96.0),
        Value::Real(16.0),
    ];
    assert_eq!(
        h.native("IssuePointOrder", &walk).unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 3.0);
    let h = &mut vm.host().inner;
    assert_eq!(h.executed_unit(b).unwrap().position, Some([96.0, 16.0]));
    assert_eq!(target(h, a), None);
    assert_eq!(h.executed_unit(a).unwrap().position, Some([16.0, 16.0]));
    let held = h
        .executed_unit(a)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .held();
    assert!(held);
    // Stop releases the hold: the same walking enemy is acquired by acquire range.
    assert_eq!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Handle(a), Value::Str("beta".into())]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 0.1);
    assert_eq!(target(&vm.host().inner, a), Some(b));
}

fn traced_actor(h: &ObjectHost, unit: HandleId) -> CombatActor {
    h.combat_actor(unit).unwrap()
}

#[test]
fn combat_trace_records_acquisition_then_its_damage_with_source_identity() {
    let mut fields = armed("64", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("400".into()));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    assert!(h.combat_trace().is_none());
    h.enable_combat_trace(16);
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 1.0);
    let (source, target) = (
        traced_actor(&vm.host().inner, a),
        traced_actor(&vm.host().inner, b),
    );
    let trace = vm.host().inner.combat_trace().unwrap();
    assert_eq!(trace.dropped(), 0);
    assert_eq!(source.player, Some(4));
    assert_eq!(target.player, Some(5));
    assert_eq!(source.rawcode, *b"u001");
    let events = trace.events();
    let CombatTraceEvent::Acquired {
        source: s,
        target: t,
        clock,
    } = events[0]
    else {
        panic!("first event {:?}", events[0]);
    };
    assert_eq!((s, t), (source, target));
    assert!(clock.is_some_and(|c| c >= start && c <= start + 1.0));
    let damage = events.iter().find_map(|e| match e {
        CombatTraceEvent::Damage {
            source: s,
            target: t,
            damage,
            life_before,
            ..
        } => Some((*s, *t, *damage, *life_before)),
        _ => None,
    });
    let (s, t, dealt, life_before) = damage.expect("the acquired target is hit");
    assert_eq!((s, t), (source, target));
    assert!(dealt > 0.0);
    assert_eq!(life_before, Some(20.0));
}

#[test]
fn combat_trace_records_an_explicit_target_order_before_its_damage() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    vm.host().inner.enable_combat_trace(16);
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Handle(b)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 1.0);
    let trace = vm.host().inner.combat_trace().unwrap();
    let events = trace.events();
    let CombatTraceEvent::Ordered { target, .. } = events[0] else {
        panic!("first event {:?}", events[0]);
    };
    assert_eq!(target.unit, b);
    assert!(matches!(
        events.get(1),
        Some(CombatTraceEvent::Damage { .. })
    ));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, CombatTraceEvent::Acquired { .. }))
    );
}

#[test]
fn explicit_target_order_attacks_a_passive_ally_while_acquisition_skips_it() {
    let mut fields = armed("64", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("400".into()));
    let (mut vm, a, b) = open(&fields);
    let h = &mut vm.host().inner;
    // Passive alliance is directed: set it both ways so neither side acquires the other.
    let alliance = h.native("ConvertAllianceType", &[Value::Int(0)]).unwrap();
    let (owner_a, owner_b) = (
        h.executed_unit(a).unwrap().owner,
        h.executed_unit(b).unwrap().owner,
    );
    for (source, other) in [(owner_a, owner_b), (owner_b, owner_a)] {
        h.native(
            "SetPlayerAlliance",
            &[
                Value::Handle(source),
                Value::Handle(other),
                alliance.clone(),
                Value::Bool(true),
            ],
        )
        .unwrap();
    }
    h.enable_combat_trace(16);
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 1.0);
    // Automatic acquisition never selects the passive ally.
    assert_eq!(
        vm.host().inner.combat_trace().unwrap().events(),
        &[] as &[CombatTraceEvent]
    );
    let h = &mut vm.host().inner;
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Handle(b)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    let start = h.timer_game_time().unwrap();
    step(&mut vm, start + 1.0);
    let h = &mut vm.host().inner;
    assert!(h.unit_vitals(b).unwrap().unwrap().life < 20.0);
    assert!(matches!(
        h.combat_trace().unwrap().events()[0],
        CombatTraceEvent::Ordered { .. }
    ));
}

/// `armed` with regeneration rates and a 50-point mana pool. Keys `H002`/`H003`
/// override the base mana columns, and `H006`..`H008` the regeneration columns.
fn regen_fields(
    life_per_second: &str,
    life_type: &str,
    mana_per_second: &str,
) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed("64", "normal");
    for (id, column_name, kind, value) in [
        (b"H002", "manaN", FieldKind::Int, "50"),
        (b"H003", "mana0", FieldKind::Int, "0"),
        (b"H006", "regenHP", FieldKind::Real, life_per_second),
        (b"H007", "regenMana", FieldKind::Real, mana_per_second),
        (b"H008", "regenType", FieldKind::Token, life_type),
    ] {
        fields.push(column(id, SlkId::UnitBalance, column_name, kind, value));
    }
    fields
}

fn set_pools(vm: &mut Vm, unit: HandleId, life: f32, mana: f32) {
    let state = |vm: &mut Vm, name: &str| vm.global(name);
    for (name, value) in [("UNIT_STATE_LIFE", life), ("UNIT_STATE_MANA", mana)] {
        let property = state(vm, name);
        vm.host()
            .inner
            .native(
                "SetUnitState",
                &[Value::Handle(unit), property, Value::Real(value)],
            )
            .unwrap();
    }
}

fn pools(vm: &mut Vm, unit: HandleId) -> (f32, f32) {
    let v = vm.host().inner.unit_vitals(unit).unwrap().unwrap();
    (v.life, v.mana)
}

#[test]
fn regeneration_integrates_stock_rates_and_caps_at_maxima_for_any_tick_chunking() {
    let fields = regen_fields("0.25", "always", "0.5");
    let (mut whole, a, b) = open(&fields);
    let (mut stepped, c, d) = open(&fields);
    set_pools(&mut whole, a, 5.0, 0.0);
    set_pools(&mut whole, b, 19.9, 49.0);
    set_pools(&mut stepped, c, 5.0, 0.0);
    set_pools(&mut stepped, d, 19.9, 49.0);
    whole
        .host()
        .inner
        .next_timer_expiry(10.0, &mut 100)
        .unwrap();
    whole
        .host()
        .inner
        .advance_unit_activity(10.0, &mut 65536)
        .unwrap();
    step(&mut stepped, 10.0);
    // 5 + 0.25 * 10 and 0 + 0.5 * 10 are exact; the second unit hits both maxima.
    assert_eq!(pools(&mut whole, a), (7.5, 5.0));
    assert_eq!(pools(&mut stepped, c), (7.5, 5.0));
    assert_eq!(pools(&mut whole, b), (20.0, 50.0));
    assert_eq!(pools(&mut stepped, d), (20.0, 50.0));
}

#[test]
fn pause_freezes_regeneration_without_deferring_it_until_resume() {
    let fields = regen_fields("0.25", "always", "0.5");
    let (mut vm, a, b) = open(&fields);
    set_pools(&mut vm, a, 5., 0.);
    set_pools(&mut vm, b, 5., 0.);
    step(&mut vm, 0.5);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    step(&mut vm, 10.5);
    assert_eq!(pools(&mut vm, a), (5.125, 0.25));
    assert_eq!(pools(&mut vm, b), (7.625, 5.25));
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    step(&mut vm, 12.5);
    assert_eq!(pools(&mut vm, a), (5.625, 1.25));
}

#[test]
fn regeneration_never_applies_to_units_without_rate_or_with_none() {
    let fields = regen_fields("0", "none", "0");
    let (mut vm, a, _) = open(&fields);
    set_pools(&mut vm, a, 5.0, 0.0);
    step(&mut vm, 10.0);
    assert_eq!(pools(&mut vm, a), (5.0, 0.0));
}

#[test]
fn night_and_blight_regeneration_refuse_without_their_source_state() {
    for (life_type, reason) in [
        (
            "night",
            "day/night regeneration needs the admitted time-of-day window",
        ),
        ("blight", "blight regeneration needs terrain blight state"),
    ] {
        let fields = regen_fields("0.25", life_type, "0");
        let (mut vm, a, _) = open(&fields);
        set_pools(&mut vm, a, 5.0, 0.0);
        vm.host().inner.next_timer_expiry(1.0, &mut 100).unwrap();
        let error = vm
            .host()
            .inner
            .advance_unit_activity(1.0, &mut 65536)
            .unwrap_err()
            .to_string();
        assert!(error.contains(reason), "{life_type}: {error}");
        // The refused span is not integrated: the pool and projection clock are unchanged.
        assert_eq!(pools(&mut vm, a), (5.0, 0.0));
    }
}

#[test]
fn dead_units_do_not_regenerate_and_their_clock_still_advances() {
    let fields = regen_fields("0.25", "always", "0.5");
    let (mut vm, a, _) = open(&fields);
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(a)])
        .unwrap();
    // Life above zero on a dead unit isolates the dead flag from the lifeless check.
    set_pools(&mut vm, a, 5.0, 0.0);
    step(&mut vm, 10.0);
    let vitals = vm.host().inner.unit_vitals(a).unwrap().unwrap();
    assert_eq!((vitals.life, vitals.mana), (5.0, 0.0));
    assert_eq!(vitals.regen.through, 10.0);
}

fn clock_hour(vm: &mut Vm, hour: f64) -> Result<Value, String> {
    let clock = vm.global("CLOCK");
    vm.host()
        .inner
        .native("SetFloatGameState", &[clock, Value::Real(hour as f32)])
}

fn clock_scale(vm: &mut Vm, scale: f32) -> Result<Value, String> {
    vm.host()
        .inner
        .native("SetTimeOfDayScale", &[Value::Real(scale)])
}

fn assert_pools(vm: &mut Vm, unit: HandleId, life: f32, mana: f32) {
    let (l, m) = pools(vm, unit);
    assert!(
        (l - life).abs() < 1e-3 && (m - mana).abs() < 1e-3,
        "pools ({l}, {m}) != ({life}, {mana})"
    );
}

#[test]
fn clock_rebases_integrate_each_span_once_for_day_and_night_regeneration() {
    // Hour 17.5 at scale 1 (0.05 h/s). The clock moves to t=6 without projecting the
    // unit, then scale 2 (0.1 h/s) rebases there: 0 to 6 s is at 0.05 h/s (all day),
    // 6 to 8 s at 0.1 h/s reaches dusk, so day spans 8 s of 20 s.
    for (life_type, expected) in [("day", 13.0), ("night", 17.0)] {
        let fields = regen_fields("1", life_type, "0");
        let (mut vm, a, _) = open_clocked(&fields, rules());
        set_pools(&mut vm, a, 5.0, 0.0);
        clock_hour(&mut vm, 17.5).unwrap();
        clock_scale(&mut vm, 1.0).unwrap();
        step(&mut vm, 4.0);
        vm.host().inner.next_timer_expiry(6.0, &mut 8).unwrap();
        clock_scale(&mut vm, 2.0).unwrap();
        step(&mut vm, 20.0);
        assert_pools(&mut vm, a, expected, 0.0);
    }
}

#[test]
fn rejected_clock_change_leaves_pools_and_clock_unchanged() {
    let fields = regen_fields("1", "day", "0");
    let (mut vm, a, _) = open_clocked(&fields, rules());
    set_pools(&mut vm, a, 5.0, 0.0);
    clock_hour(&mut vm, 17.5).unwrap();
    clock_scale(&mut vm, 1.0).unwrap();
    step(&mut vm, 4.0);
    let before = pools(&mut vm, a);
    assert!(clock_scale(&mut vm, f32::NAN).is_err());
    assert!(clock_hour(&mut vm, f64::NAN).is_err());
    assert_eq!(pools(&mut vm, a), before);
    assert_eq!(
        vm.host().inner.native("GetTimeOfDayScale", &[]).unwrap(),
        Value::Real(1.0)
    );
    // Scale 1 reaches dusk at t=10, so day covers 10 s of the 20 s span.
    step(&mut vm, 20.0);
    assert_pools(&mut vm, a, 15.0, 0.0);
}

#[test]
fn mid_run_units_and_manually_lifed_dead_units_gain_no_retroactive_regeneration() {
    let fields = regen_fields("1", "always", "0");
    let (mut vm, a, _) = open(&fields);
    set_pools(&mut vm, a, 5.0, 0.0);
    step(&mut vm, 5.0);
    vm.call("spawn", &[]).unwrap();
    let c = match vm.global("C") {
        Value::Handle(id) => id,
        other => panic!("spawned unit: {other:?}"),
    };
    set_pools(&mut vm, c, 5.0, 0.0);
    step(&mut vm, 10.0);
    assert_pools(&mut vm, a, 15.0, 0.0);
    assert_pools(&mut vm, c, 10.0, 0.0);

    // Manual fixture policy, not a revive: the dead flag stays set and SetUnitState writes life.
    // ReviveHero/ReviveHeroLoc are declared by common.j but not admitted by this host.
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(a)])
        .unwrap();
    step(&mut vm, 13.0);
    set_pools(&mut vm, a, 5.0, 0.0);
    step(&mut vm, 20.0);
    assert_pools(&mut vm, a, 5.0, 0.0);
    assert_eq!(
        vm.host()
            .inner
            .unit_vitals(a)
            .unwrap()
            .unwrap()
            .regen
            .through,
        20.0
    );
}

#[test]
fn moving_unit_regenerates_at_its_rate_while_it_travels() {
    let fields = regen_fields("1", "always", "0");
    let (mut vm, a, _) = open(&fields);
    set_pools(&mut vm, a, 5.0, 0.0);
    let ordered = vm
        .host()
        .inner
        .native(
            "IssuePointOrder",
            &[
                Value::Handle(a),
                Value::Str("gamma".into()),
                Value::Real(72.0),
                Value::Real(16.0),
            ],
        )
        .unwrap();
    assert_eq!(ordered, Value::Bool(true));
    step(&mut vm, 10.0);
    assert_pools(&mut vm, a, 15.0, 0.0);
    let position = vm.host().inner.executed_unit(a).unwrap().position.unwrap();
    assert!((position[0] - 72.0).abs() < 1e-3, "{position:?}");
}

/// Holy Light rows: `AHhb` (three ranks, `Order=alpha` here) and the hero component `Yher`.
fn holy_tables() -> DefaultTables {
    holy_tables_with(&[])
}

fn holy_tables_with(edits: &[(&str, &str)]) -> DefaultTables {
    let headers = [
        "alias",
        "code",
        "levels",
        "reqLevel",
        "levelSkip",
        "hero",
        "DataA1",
        "DataA2",
        "DataA3",
        "Cost1",
        "Cost2",
        "Cost3",
        "Cool1",
        "Cool2",
        "Cool3",
        "Rng1",
        "Rng2",
        "Rng3",
        "targs1",
        "targs2",
        "targs3",
        "Dur1",
        "Dur2",
        "Dur3",
        "UnitID1",
        "UnitID2",
        "UnitID3",
        "Area1",
        "Area2",
        "Area3",
    ];
    let targs = "air,ground,organic,notself,invu,vuln,nonancient";
    let mut rows: [Vec<&str>; 3] = [
        vec![
            "AHhb", "AHhb", "3", "1", "0", "1", "200", "400", "600", "65", "65", "65", "5", "5",
            "5", "60", "60", "60", targs, targs, targs,
        ],
        vec![
            "Yher",
            "Yher",
            "1",
            "0",
            "0",
            "1",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "0",
            "air,ground,invu,vuln",
            "air,ground,invu,vuln",
            "air,ground,invu,vuln",
        ],
        vec![
            "AHwe", "AHwe", "3", "1", "0", "1", "1", "2", "1", "7", "13", "19", "2.5", "4.5",
            "6.5", "_", "_", "_", "_", "_", "_", "3.5", "6.5", "9.5", "u001", "u002", "u003", "5",
            "11", "17",
        ],
    ];
    for &(column, value) in edits {
        let index = headers
            .iter()
            .position(|header| *header == column)
            .expect("fixture column");
        rows[0][index] = value;
    }
    let metadata = sylk(
        &[
            "ID", "field", "slk", "index", "type", "category", "repeat", "data",
        ],
        &[
            &[
                "fCoo",
                "Cool",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
                "0",
            ],
            &[
                "fRng",
                "Rng",
                "AbilityData",
                "-1",
                "unreal",
                "stats",
                "1",
                "0",
            ],
            &[
                "fAmt",
                "Data",
                "AbilityData",
                "-1",
                "real",
                "stats",
                "1",
                "1",
            ],
            &[
                "fCnt",
                "Data",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "1",
                "1",
            ],
            &[
                "fCos",
                "Cost",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "1",
                "0",
            ],
            &[
                "fTar",
                "targs",
                "AbilityData",
                "-1",
                "targetList",
                "stats",
                "1",
                "0",
            ],
            &[
                "fLev",
                "levels",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "0",
                "0",
            ],
            &[
                "fReq",
                "reqLevel",
                "AbilityData",
                "-1",
                "int",
                "stats",
                "0",
                "0",
            ],
            &[
                "fSkp",
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
    let mut slk = String::from("ID;PWXL;N;E\r\n");
    for (column, name) in headers.iter().enumerate() {
        slk.push_str(&format!("C;X{};Y1;K\"{name}\"\r\n", column + 1));
    }
    for (index, row) in rows.iter().enumerate() {
        for (column, value) in row.iter().enumerate() {
            let value = if value.parse::<f64>().is_ok() {
                (*value).to_owned()
            } else {
                format!("\"{value}\"")
            };
            slk.push_str(&format!("C;X{};Y{};K{value}\r\n", column + 1, index + 2));
        }
    }
    slk.push_str("E\r\n");
    let profile = "[AHhb]\r\nOrder=alpha\r\nHotkey=T\r\nTargetart=Abilities\\Spells\\Human\\HolyBolt\\HolyBoltSpecialArt.mdl\r\nUberTip=\"deal half damage to an enemy Undead unit.\"\r\n";
    DefaultTables::from_bytes(
        Identity {
            install: source().install,
            edition_version: "fixture".into(),
        },
        &[
            (SlkId::AbilityData.path(), slk.as_bytes()),
            (SlkId::AbilityMetaData.path(), &metadata),
        ],
        &[(
            "Units\\HumanAbilityFunc.txt",
            format!("{profile}\r\n[AHwe]\r\nOrder=gamma\r\nHotkey=W\r\n").as_bytes(),
        )],
        pcx_objects::editor::EditorReadLimits {
            max_listed_paths: 4,
            max_asset_bytes: 64 << 10,
            max_aggregate_input_bytes: 128 << 10,
        },
    )
    .expect("synthetic holy light tables")
}

const HOLY_MAP: &str = r#"
globals
unit H=null
unit F=null
unit E=null
unit G=null
trigger T=null
integer HeroSeen=0
integer HeroRank=0
integer HeroAbility=0
integer HeroPoints=0
integer HeroLevel=0
unit HeroUnit=null
unit HeroFiltered=null
boolean HeroAllow=true
endglobals
function setup takes nothing returns nothing
set H=CreateUnit(Player(4),'H001',16.0,16.0,0.0)
set F=CreateUnit(Player(4),'u001',48.0,16.0,0.0)
set E=CreateUnit(Player(5),'u001',56.0,16.0,180.0)
set G=CreateUnit(Player(4),'u001',120.0,16.0,0.0)
endfunction
function prime takes nothing returns nothing
call SetUnitState(H,UNIT_STATE_MANA,200.0)
call SetUnitState(F,UNIT_STATE_LIFE,100.0)
endfunction
function listen takes nothing returns nothing
set T=CreateTrigger()
call TriggerRegisterUnitEvent(T,H,EVENT_UNIT_SPELL_EFFECT)
endfunction
function learn takes integer ability returns nothing
call SelectHeroSkill(H,ability)
endfunction
function cast takes unit c, unit t returns boolean
return IssueTargetOrder(c,"alpha",t)
endfunction
function remaining_cd takes unit c, integer ability returns real
return BlzGetUnitAbilityCooldownRemaining(c,ability)
endfunction
function duration_cd takes unit c, integer ability, integer rank returns real
return BlzGetUnitAbilityCooldown(c,ability,rank)
endfunction
function set_duration_cd takes unit c, integer ability, integer rank, real duration returns nothing
call BlzSetUnitAbilityCooldown(c,ability,rank,duration)
endfunction
function end_cd takes unit c, integer ability returns nothing
call BlzEndUnitAbilityCooldown(c,ability)
endfunction
function reset_cd takes unit c returns nothing
call UnitResetCooldown(c)
endfunction
function source_cd takes integer ability, integer rank returns real
return BlzGetAbilityCooldown(ability,rank)
endfunction
function attack_enemy takes unit c, unit t returns nothing
call IssueTargetOrder(c,"beta",t)
endfunction
function hero_record takes nothing returns nothing
set HeroSeen=HeroSeen+1
set HeroUnit=GetLevelingUnit()
if HeroUnit==null then
set HeroUnit=GetLearningUnit()
endif
set HeroAbility=GetLearnedSkill()
set HeroRank=GetLearnedSkillLevel()
set HeroPoints=GetHeroSkillPoints(GetTriggerUnit())
set HeroLevel=GetHeroLevel(GetTriggerUnit())
endfunction
function hero_filter takes nothing returns boolean
set HeroFiltered=GetFilterUnit()
return HeroAllow
endfunction
function hero_filter_handle takes nothing returns boolexpr
return Filter(function hero_filter)
endfunction
function hero_listen_unit takes unit u, unitevent e returns trigger
set T=CreateTrigger()
call TriggerRegisterUnitEvent(T,u,e)
call TriggerAddAction(T,function hero_record)
return T
endfunction
function hero_listen_player takes player p, playerunitevent e, boolexpr f returns trigger
set T=CreateTrigger()
call TriggerRegisterPlayerUnitEvent(T,p,e,f)
call TriggerAddAction(T,function hero_record)
return T
endfunction
function hero_refuse_filter takes nothing returns nothing
set HeroAllow=false
set HeroFiltered=null
endfunction
"#;

const HOLY_DECLARATIONS: &str = r#"
native IssueTargetOrderById takes unit u, integer order, widget target returns boolean
native SetUnitFacing takes unit u, real angle returns nothing
native SetPlayerAbilityAvailable takes player p, integer ability, boolean available returns nothing
native SetUnitOwner takes unit u, player p, boolean changeColor returns nothing
globals
constant unitevent EVENT_UNIT_SPELL_EFFECT = ConvertUnitEvent(294)
constant unitevent EVENT_UNIT_SUMMON = ConvertUnitEvent(-128)
constant playerunitevent EVENT_PLAYER_UNIT_SUMMON = ConvertPlayerUnitEvent(905)
constant playerunitevent EVENT_PLAYER_UNIT_SPELL_EFFECT = ConvertPlayerUnitEvent(277)
constant unitevent EVENT_UNIT_HERO_LEVEL = ConvertUnitEvent(-141)
constant unitevent EVENT_UNIT_HERO_SKILL = ConvertUnitEvent(892)
constant playerunitevent EVENT_PLAYER_HERO_LEVEL = ConvertPlayerUnitEvent(1011)
constant playerunitevent EVENT_PLAYER_HERO_SKILL = ConvertPlayerUnitEvent(-333)
constant unittype UNIT_TYPE_UNDEAD = ConvertUnitType(2)
constant unittype UNIT_TYPE_ANCIENT = ConvertUnitType(3)
endglobals
type effect extends agent
native AddSpecialEffectTarget takes string model, widget target, string attachPoint returns effect
native DestroyEffect takes effect e returns nothing
native RemoveUnit takes unit u returns nothing
native SelectHeroSkill takes unit u, integer ability returns nothing
type unittype extends handle
constant native ConvertUnitType takes integer i returns unittype
native IsUnitType takes unit u, unittype t returns boolean
native BlzGetUnitAbilityCooldownRemaining takes unit u, integer ability returns real
native BlzEndUnitAbilityCooldown takes unit u, integer ability returns nothing
native UnitResetCooldown takes unit u returns nothing
native BlzGetAbilityCooldown takes integer ability, integer rank returns real
native BlzGetUnitAbilityCooldown takes unit u, integer ability, integer rank returns real
native BlzSetUnitAbilityCooldown takes unit u, integer ability, integer rank, real duration returns nothing
constant native GetSpellTargetUnit takes nothing returns unit
constant native GetSummoningUnit takes nothing returns unit
constant native GetSummonedUnit takes nothing returns unit
constant native GetLevelingUnit takes nothing returns unit
constant native GetLearningUnit takes nothing returns unit
constant native GetLearnedSkill takes nothing returns integer
constant native GetLearnedSkillLevel takes nothing returns integer
constant native GetHeroLevel takes unit u returns integer
constant native GetHeroSkillPoints takes unit u returns integer
"#;

#[path = "hero_event_tests.rs"]
mod hero_events;

/// Holy Light caster `H` (hero), friendly `F`, enemy `E`, far friend `G` (120 units away).
/// Every unit shares one fixture definition, so `types` is the `type` list of all of them.
/// `abilities` and `heroes` fill the `abilList` and `heroAbilList` of the caster.
fn holy_open(types: &str, abilities: &str, heroes: &str) -> (Vm, [HandleId; 4]) {
    holy_open_with_art(types, abilities, heroes, HOLY_ART_END_MS)
}

/// The art model of the stock row ends its only sequence at this many milliseconds.
const HOLY_ART_END_MS: u32 = 2800;

fn holy_open_with_art(
    types: &str,
    abilities: &str,
    heroes: &str,
    art_end_ms: u32,
) -> (Vm, [HandleId; 4]) {
    holy_open_with_model(types, abilities, heroes, art_end_ms, None)
}

fn holy_open_with_model(
    types: &str,
    abilities: &str,
    heroes: &str,
    art_end_ms: u32,
    model: Option<Arc<crate::unit_model::Model>>,
) -> (Vm, [HandleId; 4]) {
    holy_open_with_inputs(
        types,
        abilities,
        heroes,
        art_end_ms,
        model,
        holy_tables(),
        Default::default(),
    )
}

fn holy_open_with_inputs(
    types: &str,
    abilities: &str,
    heroes: &str,
    art_end_ms: u32,
    model: Option<Arc<crate::unit_model::Model>>,
    tables: DefaultTables,
    overlay: pcx_objects::editor::Overlay,
) -> (Vm, [HandleId; 4]) {
    let mut fields = hero_fields("STR");
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
        types,
    ));
    fields.push(column(
        b"A008",
        SlkId::UnitAbilities,
        "abilList",
        FieldKind::Token,
        abilities,
    ));
    fields.push(column(
        b"A009",
        SlkId::UnitAbilities,
        "heroAbilList",
        FieldKind::Token,
        heroes,
    ));
    let extra = format!(
        "{}\n{}{DECLARATIONS}{HOLY_DECLARATIONS}",
        enumeration::TRIGGERS,
        super::activity::ORDERS
    );
    let (registry, activity) = super::activity::rules();
    let h = position_host_admitting(1000, 16, HOLY_MAP, &extra, &fields, |h| h)
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
        .unwrap();
    let mut h = h.with_unit_effects().unwrap();
    let origin = crate::scripts::AssetIdentity {
        source: source().install,
        edition: "fixture".into(),
        version: "fixture".into(),
    };
    let hero_budget = crate::playercolor_host::HeroBudget {
        abilities_per_unit: 8,
        overlay_objects: overlay.mods.len() + overlay.custom.len(),
        modifications_per_ability: overlay
            .mods
            .values()
            .chain(overlay.custom.values().map(|(_, mods)| mods))
            .map(Vec::len)
            .max()
            .unwrap_or(0),
    };
    h = h
        .with_hero_skills(tables, origin, overlay, hero_rules().0, hero_budget)
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
    h.admit_combat(rules(), "beta").unwrap();
    h.admit_holy_light(&holy_art(art_end_ms, false)).unwrap();
    h.set_neutral_passive(15);
    if let Some(model) = model {
        h.admit_unit_models(move |_| Ok(Some(Arc::clone(&model))))
            .unwrap();
    }
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

fn holy_cast(vm: &mut Vm, caster: HandleId, target: HandleId) -> bool {
    match vm
        .call("cast", &[Value::Handle(caster), Value::Handle(target)])
        .unwrap()
    {
        Value::Bool(cast) => cast,
        other => panic!("cast returned {other:?}"),
    }
}

fn holy_life(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().life
}

fn holy_mana(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().mana
}

fn holy_learn(vm: &mut Vm, ability: i32) {
    vm.call("learn", &[Value::Int(ability)]).unwrap();
}

const LEARNED: &str = "AHhb";
const UNLEARNED: &str = "_";
const LEARNABLE: &str = "AHhb";
const NOTHING: &str = "_";

/// `AHhb` as `Value::Int` rawcode, as scripts pass it.
fn holy_code() -> i32 {
    i32::from_be_bytes(*b"AHhb")
}

#[test]
fn holy_light_heals_a_friendly_living_unit_and_waits_out_its_cooldown() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let mana = holy_mana(&mut vm, h);
    let ready = vm.host().inner.holy_light_readiness(h).unwrap().unwrap();
    assert_eq!(
        (ready.mana_cost, ready.mana, ready.cooldown_remaining),
        (65., Some(mana), 0.)
    );
    assert!(ready.ready());
    assert_eq!(holy_life(&mut vm, f), 100.0);
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 300.0);
    assert_eq!(holy_mana(&mut vm, h), mana - 65.0);
    let cooling = vm.host().inner.holy_light_readiness(h).unwrap().unwrap();
    assert_eq!(cooling.cooldown_remaining, 5.);
    assert_eq!(cooling.mana, Some(mana - 65.));
    assert!(!cooling.ready());
    assert!(!holy_cast(&mut vm, h, f), "cooldown refuses the recast");
    step(&mut vm, 5.0);
    assert!(
        vm.host()
            .inner
            .holy_light_readiness(h)
            .unwrap()
            .unwrap()
            .ready()
    );
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 500.0);
}

#[test]
fn holy_fractional_source_cells_reach_range_healing_and_cooldown_without_truncation() {
    let tables = holy_tables_with(&[("Cool1", "1.25"), ("Rng1", "31.5"), ("DataA1", "20.25")]);
    let (mut vm, [caster, target, _, _]) = holy_open_with_inputs(
        "",
        LEARNED,
        NOTHING,
        HOLY_ART_END_MS,
        None,
        tables,
        Default::default(),
    );
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(
        holy_life(&mut vm, target),
        100.,
        "32 units is outside the fractional range"
    );
    assert_eq!(holy_mana(&mut vm, caster), 200.);
    let owner = vm.host().inner.executed_unit(caster).unwrap().owner;
    let target = match vm
        .host()
        .inner
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(47.5),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap()
    {
        Value::Handle(unit) => unit,
        other => panic!("fractional target: {other:?}"),
    };
    let life_state = vm.global("UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(target), life_state, Value::Real(100.)],
        )
        .unwrap();
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(holy_life(&mut vm, target), 120.25);
    assert_eq!(holy_mana(&mut vm, caster), 135.);
    assert_eq!(
        vm.host()
            .inner
            .holy_light_readiness(caster)
            .unwrap()
            .unwrap()
            .cooldown_remaining,
        1.25
    );
    step(&mut vm, 1.0);
    assert!(!holy_cast(&mut vm, caster, target));
    step(&mut vm, 1.25);
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(holy_life(&mut vm, target), 140.5);
}

#[test]
fn holy_nonfinite_and_negative_source_cells_refuse_before_effects_or_payment() {
    for (column, value) in [
        ("Cool1", "NaN"),
        ("Rng1", "inf"),
        ("DataA1", "-1"),
        ("Cost1", "-1"),
        ("Cost1", "2147483648"),
    ] {
        let tables = holy_tables_with(&[(column, value)]);
        let (mut vm, [caster, target, _, _]) = holy_open_with_inputs(
            "",
            LEARNED,
            NOTHING,
            HOLY_ART_END_MS,
            None,
            tables,
            Default::default(),
        );
        vm.call("prime", &[]).unwrap();
        assert!(
            vm.host().inner.holy_light_readiness(caster).is_err(),
            "{column}"
        );
        assert!(
            vm.host()
                .inner
                .native(
                    "IssueTargetOrder",
                    &[
                        Value::Handle(caster),
                        Value::Str("alpha".into()),
                        Value::Handle(target)
                    ]
                )
                .is_err(),
            "{column}"
        );
        assert_eq!(holy_life(&mut vm, target), 100.);
        assert_eq!(holy_mana(&mut vm, caster), 200.);
    }
}

#[test]
fn holy_ranked_map_edits_use_metadata_types_and_leave_other_ranks_authored() {
    let mut overlay = pcx_objects::editor::Overlay::default();
    let real = |field, pointer, value_type, value| pcx_objects::Mod {
        field,
        value_type,
        selectors: Some([1, pointer]),
        value: pcx_objects::Value::Real(value),
        end_word: 0,
    };
    overlay.mods.insert(
        "AHhb".into(),
        vec![
            real(*b"fCoo", 0, pcx_objects::ValueType::Unreal, 1.5),
            real(*b"fRng", 0, pcx_objects::ValueType::Unreal, 32.5),
            real(*b"fAmt", 1, pcx_objects::ValueType::Real, 20.25),
            pcx_objects::Mod {
                field: *b"fCos",
                value_type: pcx_objects::ValueType::Int,
                selectors: Some([1, 0]),
                value: pcx_objects::Value::Int(7),
                end_word: 0,
            },
        ],
    );
    let (mut vm, [caster, target, _, _]) = holy_open_with_inputs(
        "",
        UNLEARNED,
        LEARNABLE,
        HOLY_ART_END_MS,
        None,
        holy_tables(),
        overlay,
    );
    holy_learn(&mut vm, holy_code());
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(holy_life(&mut vm, target), 120.25);
    assert_eq!(holy_mana(&mut vm, caster), 193.);
    assert_eq!(
        vm.host()
            .inner
            .holy_light_readiness(caster)
            .unwrap()
            .unwrap()
            .cooldown_remaining,
        1.5
    );
    vm.host()
        .inner
        .native(
            "SetHeroLevel",
            &[Value::Handle(caster), Value::Int(3), Value::Bool(false)],
        )
        .unwrap();
    vm.call("learn", &[Value::Int(holy_code())]).unwrap();
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(caster)
            .unwrap()
            .abilities
            .as_ref()
            .unwrap()
            .level(*b"AHhb"),
        2
    );
    step(&mut vm, 1.5);
    assert_eq!(
        vm.host()
            .inner
            .holy_light_readiness(caster)
            .unwrap()
            .unwrap()
            .mana_cost,
        65.
    );
    assert!(holy_cast(&mut vm, caster, target));
    assert_eq!(holy_life(&mut vm, target), 520.25);
    assert_eq!(
        vm.host()
            .inner
            .holy_light_readiness(caster)
            .unwrap()
            .unwrap()
            .cooldown_remaining,
        5.
    );
}

#[test]
fn holy_readiness_tracks_pause_and_mana_without_spending_or_accepting_a_cast() {
    let (mut vm, [caster, target, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let mana = holy_mana(&mut vm, caster);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(true)])
        .unwrap();
    let paused = vm
        .host()
        .inner
        .holy_light_readiness(caster)
        .unwrap()
        .unwrap();
    assert!(paused.paused);
    assert!(!paused.ready());
    assert!(!holy_cast(&mut vm, caster, target));
    assert_eq!(holy_mana(&mut vm, caster), mana);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(caster), Value::Bool(false)])
        .unwrap();
    assert!(
        vm.host()
            .inner
            .holy_light_readiness(caster)
            .unwrap()
            .unwrap()
            .ready()
    );
    let state = vm.global("UNIT_STATE_MANA");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(caster), state, Value::Real(64.)],
        )
        .unwrap();
    let short = vm
        .host()
        .inner
        .holy_light_readiness(caster)
        .unwrap()
        .unwrap();
    assert_eq!(
        (short.mana_cost, short.mana, short.cooldown_remaining),
        (65., Some(64.), 0.)
    );
    assert!(!short.ready());
    assert!(!holy_cast(&mut vm, caster, target));
    assert_eq!(holy_mana(&mut vm, caster), 64.);
    assert_eq!(holy_life(&mut vm, target), 100.);
}

#[test]
fn unavailable_holy_light_has_no_command_or_cast_and_tracks_current_owner() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let owner = vm.host().inner.executed_unit(h).unwrap().owner;
    vm.host()
        .native(
            "SetPlayerAbilityAvailable",
            &[
                Value::Handle(owner),
                Value::Int(holy_code()),
                Value::Bool(false),
            ],
        )
        .unwrap();
    let mana = holy_mana(&mut vm, h);
    assert!(vm.host().inner.holy_light_command(h).unwrap().is_none());
    assert!(vm.host().inner.holy_light_readiness(h).unwrap().is_none());
    assert!(!holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 100.0);
    assert_eq!(holy_mana(&mut vm, h), mana);
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(h)
            .unwrap()
            .abilities
            .as_ref()
            .unwrap()
            .level(*b"AHhb"),
        1
    );

    let other = vm.host().native("Player", &[Value::Int(5)]).unwrap();
    vm.host()
        .native(
            "SetUnitOwner",
            &[Value::Handle(h), other, Value::Bool(false)],
        )
        .unwrap();
    assert!(vm.host().inner.holy_light_command(h).unwrap().is_some());
    vm.host()
        .native(
            "SetUnitOwner",
            &[Value::Handle(h), Value::Handle(owner), Value::Bool(false)],
        )
        .unwrap();
    assert!(vm.host().inner.holy_light_command(h).unwrap().is_none());
    vm.host()
        .native(
            "SetPlayerAbilityAvailable",
            &[
                Value::Handle(owner),
                Value::Int(holy_code()),
                Value::Bool(true),
            ],
        )
        .unwrap();
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 300.0);
}

#[test]
fn disabling_holy_light_during_a_walk_cancels_without_spending_mana() {
    let (mut vm, [h, _, _, far]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let life_state = vm_global(&vm, "UNIT_STATE_LIFE");
    vm.host()
        .native(
            "SetUnitState",
            &[Value::Handle(far), life_state, Value::Real(100.0)],
        )
        .unwrap();
    let mana = holy_mana(&mut vm, h);
    assert!(holy_cast(&mut vm, h, far));
    assert!(
        vm.host()
            .inner
            .executed_unit(h)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    let owner = vm.host().inner.executed_unit(h).unwrap().owner;
    vm.host()
        .native(
            "SetPlayerAbilityAvailable",
            &[
                Value::Handle(owner),
                Value::Int(holy_code()),
                Value::Bool(false),
            ],
        )
        .unwrap();
    step(&mut vm, 0.25);
    assert!(
        !vm.host()
            .inner
            .executed_unit(h)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    assert_eq!(holy_mana(&mut vm, h), mana);
    assert_eq!(holy_life(&mut vm, far), 100.0);
}

#[test]
fn holy_light_damages_an_enemy_undead_unit_for_half_the_heal_amount() {
    let (mut vm, [h, f, e, _]) = holy_open("undead", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let enemy = holy_life(&mut vm, e);
    assert!(!holy_cast(&mut vm, h, f), "friendly undead is not organic");
    assert!(holy_cast(&mut vm, h, e));
    assert_eq!(enemy - holy_life(&mut vm, e), 100.0);
}

#[test]
fn unit_type_undead_reads_the_type_token_column_through_the_placement_reader() {
    let (mut vm, [_, _, e, _]) = holy_open("undead", LEARNED, NOTHING);
    let undead = vm.global("UNIT_TYPE_UNDEAD");
    let check = |vm: &mut Vm, unit| {
        vm.host()
            .inner
            .native("IsUnitType", &[Value::Handle(unit), undead.clone()])
            .unwrap()
    };
    assert_eq!(check(&mut vm, e), Value::Bool(true));
    let (mut vm, [_, _, e, _]) = holy_open("", LEARNED, NOTHING);
    let undead = vm.global("UNIT_TYPE_UNDEAD");
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitType", &[Value::Handle(e), undead])
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn ancient_undead_and_enemy_organic_and_mana_refusals_leave_state_unchanged() {
    let (mut vm, [h, f, e, _]) = holy_open("undead,ancient", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let (enemy, mana) = (holy_life(&mut vm, e), holy_mana(&mut vm, h));
    assert!(
        !holy_cast(&mut vm, h, e),
        "ancient is excluded by nonancient"
    );
    assert_eq!(
        (holy_life(&mut vm, e), holy_mana(&mut vm, h)),
        (enemy, mana)
    );
    assert!(!holy_cast(&mut vm, f, h), "caster without the ability");
    let (mut vm, [h, f, e, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let enemy = holy_life(&mut vm, e);
    assert!(!holy_cast(&mut vm, h, e), "enemy organic is not a target");
    assert_eq!(holy_life(&mut vm, e), enemy);
    let mana_state = vm_global(&vm, "UNIT_STATE_MANA");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(h), mana_state, Value::Real(10.0)],
        )
        .unwrap();
    assert!(!holy_cast(&mut vm, h, f), "insufficient mana");
    assert_eq!(holy_life(&mut vm, f), 100.0);
}

#[test]
fn out_of_range_orders_walk_the_caster_into_range_then_cast_once() {
    let (mut vm, [h, _, _, g]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let life_state = vm_global(&vm, "UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(g), life_state, Value::Real(100.0)],
        )
        .unwrap();
    let (life, mana) = (holy_life(&mut vm, g), holy_mana(&mut vm, h));
    assert!(holy_cast(&mut vm, h, g), "accepted and walking");
    assert_eq!((holy_life(&mut vm, g), holy_mana(&mut vm, h)), (life, mana));
    let mut until = 0.0;
    while holy_life(&mut vm, g) == life && until < 60.0 {
        until += 0.5;
        step(&mut vm, until);
    }
    assert_eq!(holy_life(&mut vm, g), life + 200.0);
    assert_eq!(holy_mana(&mut vm, h), mana - 65.0);
    let at = vm.host().inner.executed_unit(h).unwrap().position.unwrap();
    let goal = vm.host().inner.executed_unit(g).unwrap().position.unwrap();
    let gap = ((at[0] - goal[0]).powi(2) + (at[1] - goal[1]).powi(2)).sqrt();
    assert!(gap <= 60.0, "{at:?} {goal:?}");
    step(&mut vm, until + 10.0);
    assert_eq!(holy_life(&mut vm, g), life + 200.0, "no second cast");
}

#[test]
fn a_new_order_cancels_a_walking_cast_without_spending_mana() {
    let (mut vm, [h, _, e, g]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let (life, mana) = (holy_life(&mut vm, g), holy_mana(&mut vm, h));
    assert!(holy_cast(&mut vm, h, g));
    vm.call("attack_enemy", &[Value::Handle(h), Value::Handle(e)])
        .unwrap();
    step(&mut vm, 30.0);
    assert_eq!(holy_life(&mut vm, g), life);
    assert_eq!(holy_mana(&mut vm, h), mana);
}

#[test]
fn holy_light_data_override_requires_the_metadata_data_pointer() {
    let mut overlay = pcx_objects::editor::Overlay::default();
    overlay.mods.insert(
        "AHhb".into(),
        vec![pcx_objects::Mod {
            field: *b"fAmt",
            value_type: pcx_objects::ValueType::Real,
            selectors: Some([1, 0]),
            value: pcx_objects::Value::Real(20.25),
            end_word: 0,
        }],
    );
    let (mut vm, [caster, target, _, _]) = holy_open_with_inputs(
        "",
        LEARNED,
        NOTHING,
        HOLY_ART_END_MS,
        None,
        holy_tables(),
        overlay,
    );
    vm.call("prime", &[]).unwrap();
    let mana = holy_mana(&mut vm, caster);
    assert!(
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(caster),
                    Value::Str("alpha".into()),
                    Value::Handle(target)
                ]
            )
            .unwrap_err()
            .to_string()
            .contains("data pointer 0 does not match metadata data column 1")
    );
    assert_eq!(holy_life(&mut vm, target), 100.);
    assert_eq!(holy_mana(&mut vm, caster), mana);
}

#[test]
fn ranks_are_learned_with_skill_points_and_each_rank_uses_its_own_row() {
    let (mut vm, [h, f, _, _]) = holy_open("", UNLEARNED, LEARNABLE);
    vm.call("prime", &[]).unwrap();
    assert!(!holy_cast(&mut vm, h, f), "not learned yet");
    holy_learn(&mut vm, holy_code());
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 300.0, "rank one heals DataA1");
    step(&mut vm, 5.0);
    holy_learn(&mut vm, holy_code());
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(holy_life(&mut vm, f), 700.0, "rank two heals DataA2");
    step(&mut vm, 10.0);
    holy_learn(&mut vm, holy_code());
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(
        holy_life(&mut vm, f),
        1000.0,
        "rank three heals DataA3, capped at max life"
    );
}

#[test]
fn spell_effect_event_names_the_target_and_fires_once_per_cast() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    vm.call("listen", &[]).unwrap();
    assert!(holy_cast(&mut vm, h, f));
    let fired = vm
        .host()
        .inner
        .take_fired_event()
        .expect("spell effect event");
    assert_eq!(fired.subject.unit, Some(h));
    assert_eq!(fired.subject.spell_target, Some(f));
    assert!(vm.host().inner.take_fired_event().is_none());
}

#[test]
fn invulnerable_undead_is_targetable_but_takes_no_damage() {
    let (mut vm, [h, _, e, _]) = holy_open("undead", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    vm.host()
        .inner
        .native(
            "SetUnitInvulnerable",
            &[Value::Handle(e), Value::Bool(true)],
        )
        .unwrap();
    let enemy = holy_life(&mut vm, e);
    let mana = holy_mana(&mut vm, h);
    assert!(holy_cast(&mut vm, h, e));
    assert_eq!(holy_life(&mut vm, e), enemy);
    assert_eq!(holy_mana(&mut vm, h), mana - 65.0);
}

#[test]
fn learned_holy_light_names_its_hotkey_and_order_to_the_player_command() {
    let (mut vm, [h, _, _, _]) = holy_open("", LEARNED, NOTHING);
    let command = vm
        .host()
        .inner
        .holy_light_command(h)
        .unwrap()
        .expect("learned Holy Light");
    // The activity fixture registers `alpha` as order 431.
    assert_eq!((command.hotkey, command.order), ('T', 431));
    let (mut vm, [h, _, _, _]) = holy_open("", UNLEARNED, NOTHING);
    assert!(vm.host().inner.holy_light_command(h).unwrap().is_none());
}

#[test]
fn a_passive_alliance_makes_an_enemy_living_unit_a_heal_target() {
    let (mut vm, [h, _, e, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let life_state = vm_global(&vm, "UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(e), life_state, Value::Real(100.0)],
        )
        .unwrap();
    assert!(
        !holy_cast(&mut vm, h, e),
        "enemy organic before the alliance"
    );
    let alliance = vm_global(&vm, "ALLIANCE_PASSIVE");
    let h_owner = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let e_owner = vm.host().inner.native("Player", &[Value::Int(5)]).unwrap();
    vm.host()
        .inner
        .native(
            "SetPlayerAlliance",
            &[h_owner, e_owner, alliance, Value::Bool(true)],
        )
        .unwrap();
    assert!(holy_cast(&mut vm, h, e));
    assert_eq!(holy_life(&mut vm, e), 300.0);
}

#[test]
fn the_targetart_shows_on_the_target_and_a_recast_replaces_it() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, h, f));
    let art: Vec<_> = vm.host().inner.unit_effects().collect();
    assert_eq!(art.len(), 1, "{art:?}");
    let (_, effect) = art[0];
    assert_eq!(effect.unit, f);
    assert_eq!(
        effect.model,
        "Abilities\\Spells\\Human\\HolyBolt\\HolyBoltSpecialArt.mdl"
    );
    assert_eq!(effect.attach_point, "origin", "no Targetattach is authored");
    step(&mut vm, 5.0);
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(vm.host().inner.unit_effects().count(), 1);
}

#[test]
fn killing_the_caster_cancels_a_walking_cast() {
    let (mut vm, [h, _, _, g]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    let life_state = vm_global(&vm, "UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(g), life_state, Value::Real(100.0)],
        )
        .unwrap();
    assert!(holy_cast(&mut vm, h, g));
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(h)])
        .unwrap();
    step(&mut vm, 30.0);
    assert_eq!(holy_life(&mut vm, g), 100.0);
}

/// A synthetic `MDLX` with one `SEQS` record: the Holy Light art shape, a single sequence
/// spanning `[0, end)` ms, looping or not.
fn holy_art(end: u32, looping: bool) -> Vec<u8> {
    let mut sequence = vec![0; pcx_mdl::SEQUENCE_SIZE];
    sequence[..5].copy_from_slice(b"Birth");
    sequence[84..88].copy_from_slice(&end.to_le_bytes());
    sequence[92..96].copy_from_slice(&u32::from(!looping).to_le_bytes());
    let mut bytes = b"MDLX".to_vec();
    bytes.extend(b"VERS");
    bytes.extend(4_u32.to_le_bytes());
    bytes.extend(800_u32.to_le_bytes());
    bytes.extend(b"SEQS");
    bytes.extend((sequence.len() as u32).to_le_bytes());
    bytes.extend(sequence);
    bytes
}

#[test]
fn the_art_retires_when_its_only_sequence_ends() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(vm.host().inner.unit_effects().count(), 1);
    step(&mut vm, 2.0);
    assert_eq!(
        vm.host().inner.unit_effects().count(),
        1,
        "still in its sequence"
    );
    step(&mut vm, 3.0);
    assert_eq!(vm.host().inner.unit_effects().count(), 0, "sequence ended");
}

#[test]
fn overlapping_art_from_two_casts_keeps_each_lifetime_on_its_own_clock() {
    let (mut vm, [h, f, _, _]) = holy_open_with_art("", LEARNED, NOTHING, 8000);
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, h, f));
    step(&mut vm, 5.0);
    assert!(holy_cast(&mut vm, h, f));
    assert_eq!(
        vm.host().inner.unit_effects().count(),
        2,
        "both live at t=5"
    );
    step(&mut vm, 9.0);
    assert_eq!(
        vm.host().inner.unit_effects().count(),
        1,
        "first ended at t=8"
    );
    step(&mut vm, 14.0);
    assert_eq!(
        vm.host().inner.unit_effects().count(),
        0,
        "second ended at t=13"
    );
}

#[test]
fn art_retires_on_the_clock_after_its_caster_dies() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, h, f));
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(h)])
        .unwrap();
    step(&mut vm, 3.0);
    assert_eq!(vm.host().inner.unit_effects().count(), 0);
}

#[test]
fn removing_the_target_ends_its_art_and_the_clock_cannot_revive_it() {
    let (mut vm, [h, f, _, _]) = holy_open("", LEARNED, NOTHING);
    vm.call("prime", &[]).unwrap();
    assert!(holy_cast(&mut vm, h, f));
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(f)])
        .unwrap();
    // RemoveUnit ends every effect attached to the removed unit, as unit_removal_host does.
    assert_eq!(vm.host().inner.unit_effects().count(), 0);
    step(&mut vm, 3.0);
    assert_eq!(vm.host().inner.unit_effects().count(), 0);
}
#[path = "player_score_tests.rs"]
mod player_score;

#[test]
fn melee_chase_routes_to_attack_reach_instead_of_the_occupied_target_centre() {
    let (mut vm, a, b) = open(&armed("64", "normal"));
    let h = &mut vm.host().inner;
    h.native("ShowUnit", &[Value::Handle(b), Value::Bool(false)])
        .unwrap();
    let owner = h.native("Player", &[Value::Int(5)]).unwrap();
    let Value::Handle(target) = h
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*b"u001")),
                Value::Real(112.0),
                Value::Real(16.0),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("target")
    };
    let life = h.executed_unit(target).unwrap().vitals.unwrap().life;
    assert_eq!(
        h.native(
            "IssueTargetOrder",
            &[
                Value::Handle(a),
                Value::Str("alpha".into()),
                Value::Handle(target)
            ]
        )
        .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 1.0 / 64.0);
    let h = &mut vm.host().inner;
    let goal = h
        .executed_unit(a)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .waypoints()
        .0
        .last()
        .copied()
        .unwrap();
    // Fixture range 64 plus two radius-2 footprints: approach x=44 from the west.
    assert!(goal[0] >= 44.0 && goal[0] < 45.0, "{goal:?}");
    assert_eq!(goal[1], 16.0);
    assert_ne!(goal, h.executed_unit(target).unwrap().position.unwrap());
    step(&mut vm, 5.0);
    let h = &mut vm.host().inner;
    assert!(h.executed_unit(target).unwrap().vitals.unwrap().life < life);
    assert!(
        !h.executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
}

fn targeting_policy() -> crate::campaign_targeting_policy::TargetingPolicy {
    let mut policy = crate::campaign_targeting_policy::TargetingPolicy::fixture();
    policy.hero_min_game_difficulty = 1;
    policy.optional_damage_gate = 4;
    policy.optional_level_gate = 4;
    policy
}

fn admit_targeting(vm: &mut Vm, player: i32, enabled: Option<bool>) {
    admit_targeting_with(vm, player, enabled, targeting_policy(), 1);
}

fn admit_targeting_with(
    vm: &mut Vm,
    player: i32,
    enabled: Option<bool>,
    policy: crate::campaign_targeting_policy::TargetingPolicy,
    difficulty: i32,
) {
    let h = &mut vm.host().inner;
    h.test_set_game_difficulty(difficulty);
    h.set_neutral_aggressive(20);
    let context = Arc::clone(h.player_context().unwrap());
    h.admit_campaign_targeting(&context, policy).unwrap();
    h.activate_campaign_ai_player(player).unwrap();
    if let Some(enabled) = enabled {
        h.set_ai_target_heroes(player, enabled).unwrap();
    }
}

fn create_for(vm: &mut Vm, owner_of: HandleId, rawcode: [u8; 4], x: f32, y: f32) -> HandleId {
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(owner_of).unwrap().owner;
    let Value::Handle(id) = h
        .native(
            "CreateUnit",
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(rawcode)),
                Value::Real(x),
                Value::Real(y),
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit")
    };
    id
}

fn acquired(vm: &mut Vm, unit: HandleId) -> Option<HandleId> {
    vm.host()
        .inner
        .executed_unit(unit)
        .unwrap()
        .combat
        .as_ref()
        .unwrap()
        .target
}

fn wide_acquire() -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed("128", "normal");
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("256".into()));
    fields
}

fn wide_hero_fields(primary: &str) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = hero_fields(primary);
    fields
        .iter_mut()
        .find(|(id, _)| id == b"W005")
        .unwrap()
        .1
        .value = Ok(FieldValue::Cell("256".into()));
    fields
}

#[test]
fn campaign_targeting_hero_selection_uses_the_admitted_flag_and_newest_tie_break() {
    for (enabled, newest_hero) in [
        (Some(true), false),
        (Some(true), true),
        (None, false),
        (None, true),
        (Some(false), false),
        (Some(false), true),
    ] {
        let (hero, combat) = hero_rules();
        let (mut vm, a, b) = open_selected(&wide_hero_fields("STR"), Some(hero), combat);
        admit_targeting(&mut vm, 4, enabled);
        let owner = vm.host().inner.executed_unit(a).unwrap().owner;
        assert_eq!(
            vm.host().inner.campaign_targeting_enabled(owner).unwrap(),
            Some(enabled.unwrap_or(false))
        );
        let (hero, ordinary) = if newest_hero {
            let ordinary = create_for(&mut vm, b, *b"u003", 16.0, 48.0);
            (create_for(&mut vm, b, *b"J001", 16.0, 48.0), ordinary)
        } else {
            let hero = create_for(&mut vm, b, *b"J001", 16.0, 48.0);
            (hero, create_for(&mut vm, b, *b"u003", 16.0, 48.0))
        };
        step(&mut vm, 1.0 / 64.0);
        let target = acquired(&mut vm, a);
        let expected_hero = enabled.unwrap_or(false) || newest_hero;
        assert_eq!(target, Some(if expected_hero { hero } else { ordinary }));
    }
}

#[test]
fn campaign_targeting_false_keeps_a_nearer_hero_eligible() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, b) = open_selected(&wide_hero_fields("STR"), Some(hero), combat);
    admit_targeting(&mut vm, 4, Some(false));
    let hero = create_for(&mut vm, b, *b"J001", 16.0, 40.0);
    create_for(&mut vm, b, *b"u003", 16.0, 48.0);
    step(&mut vm, 1.0 / 64.0);
    assert_eq!(acquired(&mut vm, a), Some(hero));
}

#[test]
fn campaign_targeting_flag_and_difficulty_are_scoped_to_the_attacking_player() {
    let (hero, combat) = hero_rules();
    let (mut vm, a, b) = open_selected(&wide_hero_fields("STR"), Some(hero), combat);
    let mut policy = crate::campaign_targeting_policy::TargetingPolicy::fixture();
    policy.hero_min_game_difficulty = 2;
    policy.optional_level_gate = 4;
    admit_targeting_with(&mut vm, 4, Some(false), policy, 1);
    let h = &mut vm.host().inner;
    h.native("Player", &[Value::Int(1)]).unwrap();
    h.activate_campaign_ai_player(1).unwrap();
    h.set_ai_target_heroes(1, true).unwrap();
    let owner = h.executed_unit(a).unwrap().owner;
    let player = h.player_settings(owner).unwrap().number;
    assert_eq!(h.campaign_targeting_enabled(owner).unwrap(), Some(false));
    let hero = create_for(&mut vm, b, *b"J001", 16.0, 48.0);
    let ordinary = create_for(&mut vm, b, *b"u003", 16.0, 48.0);
    step(&mut vm, 1.0 / 64.0);
    assert_eq!(acquired(&mut vm, a), Some(ordinary));

    let h = &mut vm.host().inner;
    h.set_ai_target_heroes(player, true).unwrap();
    h.test_clear_combat_target(a);
    step(&mut vm, 2.0 / 64.0);
    assert_eq!(acquired(&mut vm, a), Some(ordinary));

    let h = &mut vm.host().inner;
    h.test_set_game_difficulty(2);
    h.test_clear_combat_target(a);
    step(&mut vm, 3.0 / 64.0);
    assert_eq!(acquired(&mut vm, a), Some(hero));
}

#[test]
fn campaign_targeting_refuses_decisive_optional_bands_and_nonunit_specials() {
    for difficulty in [2, 4] {
        let (mut vm, a, _) = open(&wide_acquire());
        admit_targeting_with(
            &mut vm,
            4,
            Some(true),
            crate::campaign_targeting_policy::TargetingPolicy::fixture(),
            difficulty,
        );
        vm.host().inner.enable_combat_trace(8);
        let error = step_refused(&mut vm, 1.0 / 64.0);
        assert!(error.contains("campaign targeting"), "{error}");
        assert!(error.contains("optional scorer bands"), "{error}");
        assert_eq!(acquired(&mut vm, a), None);
        assert!(vm.host().inner.combat_trace().unwrap().events().is_empty());
    }

    let (mut vm, a, b) = open(&wide_acquire());
    let special = targeting_policy().unclassified_rawcodes[0];
    admit_targeting(&mut vm, 4, Some(true));
    create_for(&mut vm, b, special, 48.0, 48.0);
    vm.host().inner.enable_combat_trace(8);
    let error = step_refused(&mut vm, 1.0 / 64.0);
    assert!(error.contains("campaign targeting"), "{error}");
    assert!(error.contains("unclassified special object"), "{error}");
    assert_eq!(acquired(&mut vm, a), None);
    assert!(vm.host().inner.combat_trace().unwrap().events().is_empty());
}

#[test]
fn campaign_targeting_empty_candidate_sets_do_not_require_scorer_bands() {
    let (mut vm, a, b) = open(&wide_acquire());
    admit_targeting_with(
        &mut vm,
        4,
        Some(true),
        crate::campaign_targeting_policy::TargetingPolicy::fixture(),
        2,
    );
    let h = &mut vm.host().inner;
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    h.enable_combat_trace(8);
    step(&mut vm, 1.0 / 64.0);
    assert_eq!(acquired(&mut vm, a), None);
    assert!(vm.host().inner.combat_trace().unwrap().events().is_empty());
}

#[test]
fn campaign_targeting_refuses_destroyed_and_expired_effect_history() {
    for (live, expires_at) in [(false, None), (true, Some(0.0))] {
        let (mut vm, a, b) = open(&wide_acquire());
        admit_targeting(&mut vm, 4, Some(true));
        vm.host()
            .inner
            .test_record_effect_history(b, live, expires_at);
        vm.host().inner.enable_combat_trace(8);
        let error = step_refused(&mut vm, 1.0 / 64.0);
        assert!(error.contains("campaign targeting"), "{error}");
        assert!(error.contains("effect history"), "{error}");
        assert_eq!(acquired(&mut vm, a), None);
        assert!(vm.host().inner.combat_trace().unwrap().events().is_empty());
    }
}

#[test]
fn campaign_targeting_refuses_neutral_aggressive_candidates_and_keeps_explicit_targets() {
    let (mut vm, a, _) = open(&wide_acquire());
    admit_targeting(&mut vm, 4, Some(true));
    vm.host().inner.set_neutral_aggressive(5);
    vm.host().inner.enable_combat_trace(8);
    let error = step_refused(&mut vm, 1.0 / 64.0);
    assert!(error.contains("campaign targeting"), "{error}");
    assert!(error.contains("neutral aggressive"), "{error}");
    assert_eq!(acquired(&mut vm, a), None);
    assert!(vm.host().inner.combat_trace().unwrap().events().is_empty());

    let (mut vm, a, b) = open(&wide_acquire());
    admit_targeting(&mut vm, 4, Some(true));
    vm.host().inner.test_set_game_difficulty(2);
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(a),
                    Value::Str("alpha".into()),
                    Value::Handle(b)
                ]
            )
            .unwrap(),
        Value::Bool(true)
    );
    step(&mut vm, 1.0 / 64.0);
    assert_eq!(acquired(&mut vm, a), Some(b));
}

fn flee_fields(can_flee: &str, speed: &str) -> Vec<([u8; 4], ObjectField)> {
    flee_fields_with("4", "normal", can_flee, speed)
}

fn flee_fields_with(
    range: &str,
    weapon: &str,
    can_flee: &str,
    speed: &str,
) -> Vec<([u8; 4], ObjectField)> {
    let mut fields = armed(range, weapon);
    fields.push(column(
        b"H001",
        SlkId::UnitBalance,
        "HP",
        FieldKind::Int,
        "1000",
    ));
    fields.push(column(
        b"C001",
        SlkId::UnitData,
        "canFlee",
        FieldKind::Bool,
        can_flee,
    ));
    fields.push(column(
        b"S001",
        SlkId::UnitBalance,
        "spd",
        FieldKind::Real,
        speed,
    ));
    fields
}

fn set_weapon_mask(fields: &mut [([u8; 4], ObjectField)], mask: &str) {
    let field = fields
        .iter_mut()
        .find(|(id, _)| id == b"W001")
        .expect("synthetic weapsOn field");
    field.1.value = Ok(FieldValue::Cell(mask.into()));
}

fn add_missile_speed(fields: &mut Vec<([u8; 4], ObjectField)>) {
    let mut speed = column(
        b"M002",
        SlkId::UnitData,
        "Missilespeed",
        FieldKind::Int,
        "64",
    );
    speed.1.metadata.as_mut().unwrap().source = FieldSource::Profile {
        key: "missilespeed".into(),
        index: 0,
        slots: 1,
    };
    fields.push(speed);
}

fn add_upgrade_list(fields: &mut Vec<([u8; 4], ObjectField)>) {
    fields.push(column(
        b"U001",
        SlkId::UnitBalance,
        "upgradeList",
        FieldKind::Token,
        "Rhme",
    ));
}

fn open_flee(
    fields: &[([u8; 4], ObjectField)],
    policy: crate::campaign_flee_policy::FleePolicy,
    numeric: crate::script_numeric::NumericPolicy,
    player: i32,
) -> (Vm, HandleId, HandleId) {
    open_fixture_with(
        fields,
        None,
        rules(),
        false,
        None,
        None,
        Some(numeric),
        Some((policy, player)),
        None,
        None,
    )
}

fn flee_fixture(fields: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    let (policy, numeric) = crate::campaign_flee_policy::tests::route_fixture();
    open_flee(fields, policy, numeric, 5)
}

fn set_flee_fixture_rawcode(vm: &mut Vm, unit: HandleId, rawcode: [u8; 4]) {
    // This changes the classifier input without altering the admitted combat state.
    vm.host().inner.test_set_flee_rawcode(unit, rawcode);
}

fn hero_flee_fixture(fields: &[([u8; 4], ObjectField)]) -> (Vm, HandleId, HandleId) {
    let (policy, numeric) = crate::campaign_flee_policy::tests::route_fixture();
    open_fixture_with(
        fields,
        None,
        rules(),
        false,
        None,
        None,
        Some(numeric),
        Some((policy, 5)),
        None,
        Some((b'u', b'u')),
    )
}

fn inactive_hero_flee_fixture(
    missile: bool,
    mismatched_classifier: bool,
) -> (Vm, HandleId, HandleId) {
    let (hero, combat) = hero_rules();
    let mut fields = hero_fields("STR");
    if missile {
        fields
            .iter_mut()
            .find(|(id, _)| id == b"W003")
            .unwrap()
            .1
            .value = Ok(FieldValue::Cell("missile".into()));
        add_missile_speed(&mut fields);
    }
    let (mut vm, hero, attacker) = open_fixture_with(
        &fields,
        Some(hero),
        combat,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(hero)
            .unwrap()
            .definition
            .rawcode,
        *b"H001"
    );
    let context = Arc::clone(vm.host().inner.player_context().unwrap());
    if mismatched_classifier {
        let mut targeting = crate::campaign_targeting_policy::TargetingPolicy::fixture();
        targeting.image_sha256 = [0x99; 32];
        vm.host()
            .inner
            .admit_campaign_targeting(&context, targeting)
            .unwrap();
    }
    vm.host()
        .inner
        .admit_campaign_flee(&context, crate::campaign_flee_policy::FleePolicy::fixture())
        .unwrap();
    (vm, attacker, hero)
}

fn defend_flee_fixture() -> (Vm, HandleId, HandleId) {
    let (policy, numeric) = crate::campaign_flee_policy::tests::route_fixture();
    let mut fields = flee_fields("1", "8");
    fields.push((*b"uabi", super::ability_field("abilList", "Qord")));
    let tables = Arc::new(super::order_tables(
        super::ORDER_HEADERS,
        &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "0", "0"],
        "",
        "1",
    ));
    open_fixture_with(
        &fields,
        None,
        rules(),
        false,
        None,
        None,
        Some(numeric),
        Some((policy, 5)),
        Some(tables),
        None,
    )
}

fn flee_fixture_with_margin(
    fields: &[([u8; 4], ObjectField)],
    range_margin: f32,
) -> (Vm, HandleId, HandleId) {
    let (mut policy, numeric) = crate::campaign_flee_policy::tests::route_fixture();
    policy.range_margin = range_margin;
    open_flee(fields, policy, numeric, 5)
}

fn set_life(vm: &mut Vm, unit: HandleId, life: f32) {
    let state = vm.global("UNIT_STATE_LIFE");
    vm.host()
        .inner
        .native(
            "SetUnitState",
            &[Value::Handle(unit), state, Value::Real(life)],
        )
        .unwrap();
}

fn life(vm: &mut Vm, unit: HandleId) -> f32 {
    vm.host().inner.unit_vitals(unit).unwrap().unwrap().life
}

fn position(vm: &mut Vm, unit: HandleId) -> [f32; 2] {
    vm.host()
        .inner
        .executed_unit(unit)
        .unwrap()
        .position
        .unwrap()
}

fn low_life_damage(vm: &mut Vm, source: HandleId, target: HandleId, damage: f32) -> bool {
    vm.host()
        .inner
        .combat_damage_unit(source, target, damage)
        .unwrap();
    vm.host().inner.campaign_flee_motion_owns(target)
}

fn flee_decision(
    vm: &mut Vm,
    source: HandleId,
    target: HandleId,
    resulting_life: f32,
) -> Result<Option<[f32; 2]>, crate::playercolor_host::Error> {
    vm.host()
        .inner
        .campaign_flee_damage_decision(source, target, Some(resulting_life))
}

fn issue_attack(vm: &mut Vm, source: HandleId, target: HandleId) {
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueTargetOrder",
                &[
                    Value::Handle(source),
                    Value::Str("alpha".into()),
                    Value::Handle(target)
                ]
            )
            .unwrap(),
        Value::Bool(true)
    );
}

fn damage_events(vm: &mut Vm) -> usize {
    vm.host()
        .inner
        .combat_trace()
        .unwrap()
        .events()
        .iter()
        .filter(|event| matches!(event, CombatTraceEvent::Damage { .. }))
        .count()
}

#[test]
fn low_life_alone_does_not_start_a_retreat() {
    let (mut vm, _, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    let before = position(&mut vm, b);
    step(&mut vm, 1.0);
    assert_eq!(position(&mut vm, b), before);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
}

#[test]
fn ordinary_incoming_hit_starts_a_route_away_from_the_attacker() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    let before = position(&mut vm, b);
    assert!(low_life_damage(&mut vm, a, b, 1.0));
    let destination = vm
        .host()
        .inner
        .executed_unit(b)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .destination()
        .expect("retreat destination");
    assert!(destination[0] > before[0]);
    assert_eq!(destination[1], before[1]);
    step(&mut vm, 0.25);
    assert!(position(&mut vm, b)[0] > before[0]);
}

#[test]
fn group_flee_flag_does_not_replace_the_ordinary_hit_flag() {
    for group_enabled in [false, true] {
        let (mut vm, source, target) =
            flee_fixture_with_margin(&flee_fields_with("40", "normal", "1", "8"), 1.0);
        vm.host()
            .inner
            .set_ai_groups_flee(5, group_enabled)
            .unwrap();
        set_life(&mut vm, target, 20.0);
        issue_attack(&mut vm, source, target);
        step(&mut vm, 0.3);
        assert!(life(&mut vm, target) < 20.0);
        assert!(vm.host().inner.campaign_flee_motion_owns(target));
    }

    let (mut vm, source, target) =
        flee_fixture_with_margin(&flee_fields_with("40", "normal", "1", "8"), 1.0);
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    vm.host().inner.set_ai_groups_flee(5, true).unwrap();
    set_life(&mut vm, target, 20.0);
    issue_attack(&mut vm, source, target);
    step(&mut vm, 0.3);
    assert!(life(&mut vm, target) < 20.0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(target));
}

#[test]
fn melee_hit_starts_the_route_and_traces_damage_once() {
    let (mut vm, a, b) = flee_fixture_with_margin(&flee_fields_with("40", "normal", "1", "8"), 1.0);
    set_life(&mut vm, b, 20.0);
    vm.host().inner.enable_combat_trace(16);
    issue_attack(&mut vm, a, b);
    step(&mut vm, 0.3);
    assert!(life(&mut vm, b) < 20.0);
    assert!(vm.host().inner.campaign_flee_motion_owns(b));
    assert_eq!(damage_events(&mut vm), 1);
}

#[test]
fn missile_impact_starts_the_route_and_traces_damage_once() {
    let mut fields = flee_fields_with("40", "missile", "1", "8");
    add_missile_speed(&mut fields);
    let (mut vm, a, b) = flee_fixture_with_margin(&fields, 1.0);
    set_life(&mut vm, b, 20.0);
    vm.host().inner.enable_combat_trace(16);
    issue_attack(&mut vm, a, b);
    step(&mut vm, 0.6);
    assert_eq!(life(&mut vm, b), 20.0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    step(&mut vm, 0.8);
    assert!(life(&mut vm, b) < 20.0);
    assert!(vm.host().inner.campaign_flee_motion_owns(b));
    assert_eq!(damage_events(&mut vm), 1);
}

#[test]
fn disabled_flee_flag_keeps_the_low_life_unit_in_place() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    set_life(&mut vm, b, 20.0);
    assert!(!low_life_damage(&mut vm, a, b, 1.0));
}

#[test]
fn can_flee_and_minimum_speed_gate_the_retreat() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("0", "8"));
    set_life(&mut vm, b, 20.0);
    assert!(!low_life_damage(&mut vm, a, b, 1.0));

    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "1"));
    set_life(&mut vm, b, 20.0);
    assert!(!low_life_damage(&mut vm, a, b, 1.0));
}

#[test]
fn life_band_maxima_are_inclusive_while_the_ordinary_cutoff_stays_strict() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    assert!(
        flee_decision(&mut vm, a, b, 293.0).unwrap().is_some(),
        "unconditional maximum"
    );
    let error = flee_decision(&mut vm, a, b, 293.5).expect_err("above band");
    assert!(
        error.to_string().contains("earlier range/ability"),
        "{error}"
    );
    for life in [393.0, 493.0, 593.0, 693.0] {
        let error = flee_decision(&mut vm, a, b, life).expect_err("conditional maximum");
        assert!(
            error.to_string().contains("score-dependent"),
            "{life}: {error}"
        );
        let error = flee_decision(&mut vm, a, b, life + 0.5).expect_err("above band");
        assert!(
            error.to_string().contains("earlier range/ability"),
            "{life}: {error}"
        );
    }
    assert!(flee_decision(&mut vm, a, b, 172.5).unwrap().is_some());
    let error = flee_decision(&mut vm, a, b, 173.0).expect_err("strict cutoff");
    assert!(
        error.to_string().contains("earlier range/ability"),
        "{error}"
    );
}

#[test]
fn high_life_is_a_contextual_refusal_that_aborts_before_damage() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 390.0);
    let error = flee_decision(&mut vm, a, b, 390.0).expect_err("conditional band");
    assert!(error.to_string().contains("score-dependent"), "{error}");
    let error = vm
        .host()
        .inner
        .combat_damage_unit(a, b, 1.0)
        .expect_err("fatal contextual refusal");
    assert!(error.to_string().contains("score-dependent"), "{error}");
    assert_eq!(life(&mut vm, b), 390.0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));

    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 200.0);
    let error = flee_decision(&mut vm, a, b, 200.0).expect_err("earlier branch");
    assert!(
        error.to_string().contains("earlier range/ability"),
        "{error}"
    );
    vm.host().inner.enable_combat_trace(16);
    let error = vm
        .host()
        .inner
        .combat_damage_unit(a, b, 1.0)
        .expect_err("earlier branch refusal");
    assert!(
        error.to_string().contains("earlier range/ability"),
        "{error}"
    );
    assert_eq!(life(&mut vm, b), 200.0);
    assert_eq!(damage_events(&mut vm), 0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
}

#[test]
fn summoned_victim_refuses_before_damage_but_a_normal_victim_takes_damage() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    vm.host().inner.test_set_summoned(b, true);
    set_life(&mut vm, b, 20.0);
    let start = position(&mut vm, b);
    vm.host().inner.enable_combat_trace(16);
    let error = vm
        .host()
        .inner
        .combat_damage_unit(a, b, 1.0)
        .expect_err("summoned victim");
    assert!(error.to_string().contains("summoned"), "{error}");
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
    assert_eq!(position(&mut vm, b), start);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert!(
        !vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );

    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    vm.host().inner.combat_damage_unit(a, b, 1.0).unwrap();
    assert_eq!(life(&mut vm, b), 19.0);
    assert_eq!(damage_events(&mut vm), 1);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    set_life(&mut vm, b, 20.0);
    vm.host().inner.set_ai_units_flee(5, true).unwrap();
    vm.host().inner.test_set_summoned(b, false);
    assert!(low_life_damage(&mut vm, a, b, 1.0));
    assert_eq!(life(&mut vm, b), 19.0);
}

#[test]
fn active_admitted_ability_state_refuses_the_victim_but_the_off_state_takes_damage() {
    let (mut vm, a, b) = defend_flee_fixture();
    let player = vm.host().inner.native("Player", &[Value::Int(5)]).unwrap();
    vm.host()
        .inner
        .native(
            "SetPlayerTechResearched",
            &[
                player,
                Value::Int(i32::from_be_bytes(*b"Qtec")),
                Value::Int(1),
            ],
        )
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrder",
                &[Value::Handle(b), Value::Str("engage".into())],
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(
        vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .admitted_order
            .is_some_and(|(_, active)| active)
    );
    set_life(&mut vm, b, 20.0);
    let start = position(&mut vm, b);
    vm.host().inner.enable_combat_trace(16);
    let error = vm
        .host()
        .inner
        .combat_damage_unit(a, b, 1.0)
        .expect_err("active admitted ability state");
    assert!(error.to_string().contains("On/Off"), "{error}");
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
    assert_eq!(position(&mut vm, b), start);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert!(
        !vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );

    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    vm.host().inner.combat_damage_unit(a, b, 1.0).unwrap();
    assert_eq!(life(&mut vm, b), 19.0);
    assert_eq!(damage_events(&mut vm), 1);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    set_life(&mut vm, b, 20.0);
    vm.host().inner.set_ai_units_flee(5, true).unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrder",
                &[Value::Handle(b), Value::Str("release".into())],
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(
        !vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .admitted_order
            .is_some_and(|(_, active)| active)
    );
    assert!(low_life_damage(&mut vm, a, b, 1.0));
    assert_eq!(life(&mut vm, b), 19.0);
}

#[test]
fn unavailable_applied_upgrade_state_is_a_contextual_refusal() {
    let (mut vm, _, _) = flee_fixture(&flee_fields("1", "8"));
    let error = vm
        .host()
        .inner
        .campaign_flee_upgrade_is_active(5, 0x5268_6d65)
        .expect_err("unavailable applied research");
    assert!(
        error
            .to_string()
            .contains("applied research state unavailable"),
        "{error}"
    );
}

#[test]
fn a_refused_retreat_leaves_normal_acquisition_and_attack_running() {
    let (mut vm, a, b) = flee_fixture_with_margin(&flee_fields_with("40", "normal", "1", "8"), 1.0);
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    let start = position(&mut vm, b);
    let start_life = life(&mut vm, b);
    issue_attack(&mut vm, a, b);
    step(&mut vm, 0.3);
    let once = life(&mut vm, b);
    assert!(once < start_life);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert_eq!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .target,
        Some(b)
    );
    step(&mut vm, 2.2);
    assert!(life(&mut vm, b) < once);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert_eq!(position(&mut vm, b), start);
}

#[test]
fn the_post_hit_life_decides_whether_the_cutoff_is_crossed() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 180.0);
    vm.host().inner.enable_combat_trace(16);
    let error = vm
        .host()
        .inner
        .combat_damage_unit(a, b, 1.0)
        .expect_err("above the qualified low leaf");
    assert!(
        error.to_string().contains("earlier range/ability"),
        "{error}"
    );
    assert_eq!(life(&mut vm, b), 180.0);
    assert_eq!(damage_events(&mut vm), 0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));

    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 180.0);
    assert!(low_life_damage(&mut vm, a, b, 10.0));
    assert_eq!(life(&mut vm, b), 170.0);
}

#[test]
fn a_lethal_hit_does_not_start_a_route_before_the_kill() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 10.0);
    vm.host()
        .inner
        .combat_damage_unit(a, b, 20.0)
        .expect("damage applies and kills");
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert!(
        vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .combat
            .as_ref()
            .unwrap()
            .dead
    );
}

#[test]
fn nonpositive_damage_does_not_start_a_retreat() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    assert!(!low_life_damage(&mut vm, a, b, 0.0));
    assert!(!low_life_damage(&mut vm, a, b, -1.0));
}

#[test]
fn clearing_the_flag_midroute_keeps_the_route_and_blocks_later_starts() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    assert!(low_life_damage(&mut vm, a, b, 1.0));
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    step(&mut vm, 0.1);
    assert!(vm.host().inner.campaign_flee_motion_owns(b));
    step(&mut vm, 2.0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert!(!low_life_damage(&mut vm, a, b, 1.0));
}

#[test]
fn an_arrived_unarmed_retreat_is_still_pruned() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    assert!(low_life_damage(&mut vm, a, b, 1.0));
    let start = position(&mut vm, b);
    vm.host().inner.test_disarm_unit(b);
    step(&mut vm, 3.0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert_ne!(position(&mut vm, b), start);
}

#[test]
fn explicit_order_and_death_forget_retreat_ownership() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    assert!(low_life_damage(&mut vm, a, b, 1.0));
    vm.host()
        .inner
        .native(
            "IssuePointOrder",
            &[
                Value::Handle(b),
                Value::Str("gamma".into()),
                Value::Real(52.0),
                Value::Real(16.0),
            ],
        )
        .unwrap();
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));

    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    assert!(low_life_damage(&mut vm, a, b, 1.0));
    vm.host()
        .inner
        .native("KillUnit", &[Value::Handle(b)])
        .unwrap();
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
}

#[test]
fn a_pre_damage_route_error_preserves_current_and_sibling_due_missiles() {
    let (mut vm, a, b) = flee_fixture(&flee_fields_with("5000", "normal", "1", "8"));
    set_life(&mut vm, b, 20.0);
    vm.host().inner.test_queue_missile(a, b, 5.0, 0.0);
    vm.host().inner.test_queue_missile(a, b, 5.0, 0.0);
    let h = &mut vm.host().inner;
    assert!(h.test_advance_combat().is_err());
    assert_eq!(h.test_missile_count(), 2);
    assert_eq!(h.unit_vitals(b).unwrap().unwrap().life, 20.0);
    assert!(h.test_advance_combat().is_err());
    assert_eq!(h.test_missile_count(), 2);
    assert_eq!(h.unit_vitals(b).unwrap().unwrap().life, 20.0);
}

#[test]
fn hero_and_ordinary_flee_flags_select_independent_damage_paths() {
    let (mut vm, attacker, victim) = hero_flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, victim, 20.0);
    let owner = vm.host().inner.executed_unit(victim).unwrap().owner;
    assert_eq!(
        vm.host().inner.campaign_flee_enabled(owner).unwrap(),
        Some(true)
    );
    assert_eq!(
        vm.host().inner.campaign_heroes_flee_enabled(owner).unwrap(),
        None
    );
    assert!(flee_decision(&mut vm, attacker, victim, 19.0).is_err());

    vm.host().inner.set_ai_heroes_flee(5, false).unwrap();
    assert_eq!(
        flee_decision(&mut vm, attacker, victim, 19.0).unwrap(),
        None
    );
    vm.host()
        .inner
        .combat_damage_unit(attacker, victim, 1.0)
        .unwrap();
    assert!(life(&mut vm, victim) < 20.0);
    vm.host().inner.set_ai_heroes_flee(5, true).unwrap();
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    let error = flee_decision(&mut vm, attacker, victim, 19.0).unwrap_err();
    assert!(error.to_string().contains("hero retreat"), "{error}");
    vm.host().inner.deactivate_campaign_ai_player(5).unwrap();
    assert_eq!(
        flee_decision(&mut vm, attacker, victim, 19.0).unwrap(),
        None
    );
    let before = life(&mut vm, victim);
    vm.host()
        .inner
        .combat_damage_unit(attacker, victim, 1.0)
        .unwrap();
    assert!(life(&mut vm, victim) < before);

    let (mut ordinary, source, target) = flee_fixture(&flee_fields("1", "8"));
    ordinary.host().inner.set_ai_units_flee(5, false).unwrap();
    ordinary.host().inner.set_ai_heroes_flee(5, true).unwrap();
    assert_eq!(
        flee_decision(&mut ordinary, source, target, 19.0).unwrap(),
        None
    );
}

#[test]
fn inactive_hero_takes_melee_and_missile_hits_without_a_matching_classifier() {
    for (missile, mismatch) in [(false, false), (true, true)] {
        let (mut vm, attacker, hero) = inactive_hero_flee_fixture(missile, mismatch);
        let owner = vm.host().inner.executed_unit(hero).unwrap().owner;
        assert_eq!(
            vm.host().inner.campaign_heroes_flee_enabled(owner).unwrap(),
            None
        );
        set_life(&mut vm, hero, 100.0);
        vm.host().inner.enable_combat_trace(16);
        issue_attack(&mut vm, attacker, hero);
        step(&mut vm, if missile { 0.8 } else { 0.3 });
        assert!(life(&mut vm, hero) < 100.0);
        assert_eq!(damage_events(&mut vm), 1);
    }
}

#[test]
fn active_both_false_flee_flags_bypass_an_unadmitted_classifier() {
    let (policy, numeric) = crate::campaign_flee_policy::tests::route_fixture();
    let (mut vm, attacker, victim) = open_fixture_with(
        &flee_fields("1", "8"),
        None,
        rules(),
        false,
        None,
        None,
        Some(numeric),
        None,
        None,
        None,
    );
    let context = Arc::clone(vm.host().inner.player_context().unwrap());
    vm.host()
        .inner
        .admit_campaign_flee(&context, policy)
        .unwrap();
    vm.host().inner.activate_campaign_ai_player(5).unwrap();
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    vm.host().inner.set_ai_heroes_flee(5, false).unwrap();
    set_life(&mut vm, victim, 20.0);
    vm.host().inner.enable_combat_trace(16);
    vm.host()
        .inner
        .combat_damage_unit(attacker, victim, 1.0)
        .unwrap();
    assert!(life(&mut vm, victim) < 20.0);
    assert_eq!(damage_events(&mut vm), 1);
}

#[test]
fn source_hero_range_does_not_treat_every_uppercase_rawcode_as_a_hero() {
    let (mut vm, attacker, victim) = flee_fixture(&flee_fields("1", "8"));
    set_flee_fixture_rawcode(&mut vm, attacker, *b"H001");
    set_flee_fixture_rawcode(&mut vm, victim, *b"H002");
    vm.host().inner.set_ai_heroes_flee(5, true).unwrap();
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    assert_eq!(
        flee_decision(&mut vm, attacker, victim, 19.0).unwrap(),
        None
    );

    vm.host().inner.set_ai_units_flee(5, true).unwrap();
    assert!(
        flee_decision(&mut vm, attacker, victim, 19.0)
            .unwrap()
            .is_some()
    );

    set_flee_fixture_rawcode(&mut vm, attacker, *b"K001");
    let error = flee_decision(&mut vm, attacker, victim, 19.0).unwrap_err();
    assert!(
        error.to_string().contains("incoming attacker is a hero"),
        "{error}"
    );
}

#[test]
fn hero_flee_refuses_missing_or_mismatched_classifier_before_damage() {
    for classifier in [None, Some([0x99; 32])] {
        let (policy, numeric) = crate::campaign_flee_policy::tests::route_fixture();
        let (mut vm, attacker, victim) = open_fixture_with(
            &flee_fields("1", "8"),
            None,
            rules(),
            false,
            None,
            None,
            Some(numeric),
            None,
            None,
            None,
        );
        let context = Arc::clone(vm.host().inner.player_context().unwrap());
        if let Some(identity) = classifier {
            let mut targeting = crate::campaign_targeting_policy::TargetingPolicy::fixture();
            targeting.image_sha256 = identity;
            vm.host()
                .inner
                .admit_campaign_targeting(&context, targeting)
                .unwrap();
        }
        vm.host()
            .inner
            .admit_campaign_flee(&context, policy)
            .unwrap();
        vm.host().inner.activate_campaign_ai_player(5).unwrap();
        vm.host().inner.set_ai_heroes_flee(5, true).unwrap();
        set_flee_fixture_rawcode(&mut vm, victim, *b"K001");
        set_life(&mut vm, victim, 20.0);
        let before = random_snapshot(&mut vm);
        let error = vm
            .host()
            .inner
            .combat_damage_unit(attacker, victim, 1.0)
            .unwrap_err();
        assert!(error.to_string().contains("hero classifier"), "{error}");
        assert_eq!(random_snapshot(&mut vm), before);
        assert_eq!(life(&mut vm, victim), 20.0);
    }
}

#[test]
fn hero_flee_melee_refusal_preserves_swing_rng_life_and_trace() {
    let (mut vm, attacker, victim) =
        hero_flee_fixture(&flee_fields_with("640", "normal", "1", "8"));
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    vm.host().inner.set_ai_heroes_flee(5, true).unwrap();
    set_life(&mut vm, victim, 20.0);
    vm.host().inner.enable_combat_trace(16);
    issue_attack(&mut vm, attacker, victim);
    advance_until_swing(&mut vm, attacker);
    let (primary, secondary, error) = advance_until_melee_refusal(&mut vm);
    assert!(error.contains("hero retreat"), "{error}");
    assert_eq!(random_snapshot(&mut vm), (primary, secondary));
    let swing = vm.host().inner.test_swing(attacker).expect("pending swing");
    assert_eq!(life(&mut vm, victim), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
    let (primary, secondary, retry) = retry_melee_refusal(&mut vm);
    assert_eq!(retry, error);
    assert_eq!(random_snapshot(&mut vm), (primary, secondary));
    assert_eq!(vm.host().inner.test_swing(attacker), Some(swing));
    assert_eq!(life(&mut vm, victim), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
}

#[test]
fn queued_hero_flee_missile_refusal_retains_due_impacts_and_damage_state() {
    let (mut vm, attacker, victim) = hero_flee_fixture(&flee_fields("1", "8"));
    vm.host().inner.set_ai_heroes_flee(5, true).unwrap();
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    set_life(&mut vm, victim, 20.0);
    vm.host().inner.enable_combat_trace(16);
    vm.host()
        .inner
        .test_queue_missile(attacker, victim, 5.0, 0.0);
    vm.host()
        .inner
        .test_queue_missile(attacker, victim, 5.0, 0.0);
    let before = random_snapshot(&mut vm);
    for _ in 0..2 {
        let error = vm.host().inner.test_advance_combat().unwrap_err();
        assert!(error.to_string().contains("hero retreat"), "{error}");
        assert_eq!(vm.host().inner.test_missile_count(), 2);
        assert_eq!(random_snapshot(&mut vm), before);
        assert_eq!(life(&mut vm, victim), 20.0);
        assert_eq!(damage_events(&mut vm), 0);
    }
}

#[test]
fn launched_hero_flee_missiles_refuse_at_impact_without_consuming_siblings() {
    let mut fields = flee_fields_with("640", "missile", "1", "8");
    add_missile_speed(&mut fields);
    let (mut vm, attacker, victim) = hero_flee_fixture(&fields);
    vm.call("spawn", &[]).unwrap();
    let Value::Handle(sibling) = vm.global("C") else {
        panic!("sibling attacker")
    };
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    vm.host().inner.set_ai_heroes_flee(5, true).unwrap();
    set_life(&mut vm, victim, 20.0);
    vm.host().inner.enable_combat_trace(16);
    issue_attack(&mut vm, attacker, victim);
    issue_attack(&mut vm, sibling, victim);
    step(&mut vm, 0.6);
    assert_eq!(vm.host().inner.test_missile_count(), 2);
    let before = random_snapshot(&mut vm);
    let error = step_refused(&mut vm, 1.0);
    assert!(error.contains("hero retreat"), "{error}");
    assert_eq!(vm.host().inner.test_missile_count(), 2);
    assert_eq!(random_snapshot(&mut vm), before);
    assert_eq!(life(&mut vm, victim), 20.0);
    assert_eq!(damage_events(&mut vm), 0);

    let (mut vm, attacker, victim) = hero_flee_fixture(&fields);
    vm.host().inner.set_ai_units_flee(5, false).unwrap();
    vm.host().inner.set_ai_heroes_flee(5, false).unwrap();
    set_life(&mut vm, victim, 20.0);
    vm.host().inner.enable_combat_trace(16);
    issue_attack(&mut vm, attacker, victim);
    step(&mut vm, 0.8);
    assert!(life(&mut vm, victim) < 20.0);
    assert_eq!(damage_events(&mut vm), 1);
}

#[test]
fn a_refused_source_aborts_before_damage_and_trace() {
    let mut fields = flee_fields("1", "8");
    set_weapon_mask(&mut fields, "0");
    let (mut vm, a, b) = flee_fixture(&fields);
    vm.host().inner.enable_combat_trace(16);
    set_life(&mut vm, b, 20.0);
    let error = vm
        .host()
        .inner
        .combat_damage_unit(a, b, 1.0)
        .expect_err("refused source");
    assert!(
        error.to_string().contains("not one initial weapon"),
        "{error}"
    );
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
}

#[test]
fn initial_weapon_mask_missing_alternate_and_multiple_sources_are_refused() {
    let mut missing = flee_fields("1", "8");
    let field = missing
        .iter_mut()
        .find(|(id, _)| id == b"W001")
        .expect("synthetic weapsOn field");
    field.1.value = Err(pcx_sim::world::FieldProblem::Missing);
    let (mut vm, a, b) = flee_fixture(&missing);
    let error = flee_decision(&mut vm, a, b, 20.0).expect_err("missing weapsOn");
    assert!(error.to_string().contains("weapsOn"), "{error}");

    for mask in ["2", "3"] {
        let mut fields = flee_fields("1", "8");
        set_weapon_mask(&mut fields, mask);
        let (mut vm, a, b) = flee_fixture(&fields);
        let error = flee_decision(&mut vm, a, b, 20.0).expect_err("noninitial mask");
        assert!(
            error.to_string().contains("not one initial weapon"),
            "{error}"
        );
    }
}

#[test]
fn effect_history_refuses_both_the_source_and_the_victim() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    vm.host().inner.test_record_effect_history(a, false, None);
    let error = flee_decision(&mut vm, a, b, 20.0).expect_err("source history");
    assert!(error.to_string().contains("effect history"), "{error}");

    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    vm.host().inner.test_record_effect_history(b, false, None);
    let error = flee_decision(&mut vm, a, b, 20.0).expect_err("victim history");
    assert!(error.to_string().contains("effect history"), "{error}");
}

#[test]
fn a_refused_melee_source_aborts_before_swing_rng_trace_and_vitals() {
    let (mut vm, a, b) = flee_fixture(&flee_fields_with("640", "normal", "1", "8"));
    vm.host().inner.test_record_effect_history(a, false, None);
    set_life(&mut vm, b, 20.0);
    vm.host().inner.enable_combat_trace(16);
    issue_attack(&mut vm, a, b);
    let (before_primary, before_secondary, error) = loop {
        let h = &mut vm.host().inner;
        if h.timer_game_time().unwrap() >= 1.0 {
            panic!("expected a refused melee source");
        }
        let before_primary = h.script_random_state().unwrap().primary_word();
        let before_secondary: Vec<u32> =
            h.script_random_state().unwrap().secondary_words().collect();
        let next = h.timer_game_time().unwrap() + 1.0 / 64.0;
        assert!(h.next_timer_expiry(next, &mut 100).unwrap().is_none());
        if let Err(error) = h.advance_unit_activity(next, &mut 65536) {
            break (before_primary, before_secondary, error);
        }
    };
    assert!(error.to_string().contains("effect history"), "{error}");
    let after_primary = vm
        .host()
        .inner
        .script_random_state()
        .unwrap()
        .primary_word();
    let after_secondary: Vec<u32> = vm
        .host()
        .inner
        .script_random_state()
        .unwrap()
        .secondary_words()
        .collect();
    assert_eq!(after_primary, before_primary);
    assert_eq!(after_secondary, before_secondary);
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
}

#[test]
fn a_conditional_band_melee_refusal_restores_primary_and_pending_swing() {
    let (mut vm, a, b) = flee_fixture(&flee_fields_with("640", "normal", "1", "8"));
    set_life(&mut vm, b, 390.0);
    vm.host().inner.enable_combat_trace(16);
    issue_attack(&mut vm, a, b);
    advance_until_swing(&mut vm, a);
    let (primary, secondary, error) = advance_until_melee_refusal(&mut vm);
    assert!(error.to_string().contains("score-dependent"), "{error}");
    assert_eq!(random_snapshot(&mut vm), (primary, secondary));
    let swing = vm.host().inner.test_swing(a).expect("pending swing");
    assert_eq!(life(&mut vm, b), 390.0);
    assert_eq!(damage_events(&mut vm), 0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));

    let (primary, secondary, retry) = retry_melee_refusal(&mut vm);
    assert_eq!(retry, error);
    assert_eq!(random_snapshot(&mut vm), (primary, secondary));
    assert_eq!(vm.host().inner.test_swing(a), Some(swing));
    assert_eq!(life(&mut vm, b), 390.0);
    assert_eq!(damage_events(&mut vm), 0);
}

#[test]
fn a_route_work_melee_refusal_restores_primary_and_pending_swing() {
    let (mut vm, a, b) =
        flee_fixture_with_margin(&flee_fields_with("640", "normal", "1", "8"), 100.0);
    set_life(&mut vm, b, 20.0);
    vm.host().inner.enable_combat_trace(16);
    issue_attack(&mut vm, a, b);
    advance_until_swing(&mut vm, a);
    vm.host().inner.test_set_route_work(1);
    let (primary, secondary, error) = advance_until_melee_refusal(&mut vm);
    assert!(error.to_string().contains("work exhausted"), "{error}");
    assert_eq!(random_snapshot(&mut vm), (primary, secondary));
    let swing = vm.host().inner.test_swing(a).expect("pending swing");
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));

    let (primary, secondary, retry) = retry_melee_refusal(&mut vm);
    assert_eq!(retry, error);
    assert_eq!(random_snapshot(&mut vm), (primary, secondary));
    assert_eq!(vm.host().inner.test_swing(a), Some(swing));
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
}

#[test]
fn a_refused_missile_source_preserves_current_and_sibling_due_missiles() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    vm.host().inner.test_record_effect_history(a, false, None);
    set_life(&mut vm, b, 20.0);
    vm.host().inner.enable_combat_trace(16);
    vm.host().inner.test_queue_missile(a, b, 5.0, 0.0);
    vm.host().inner.test_queue_missile(a, b, 5.0, 0.0);
    {
        let h = &mut vm.host().inner;
        let error = h.test_advance_combat().expect_err("refused missile source");
        assert!(error.to_string().contains("effect history"), "{error}");
        assert_eq!(h.test_missile_count(), 2);
        assert!(!h.campaign_flee_motion_owns(b));
    }
    assert_eq!(life(&mut vm, b), 20.0);
    assert_eq!(damage_events(&mut vm), 0);
}

#[test]
fn an_active_weapon_upgrade_refuses_the_source() {
    let mut fields = flee_fields("1", "8");
    add_upgrade_list(&mut fields);
    let (mut vm, a, b) = flee_fixture(&fields);
    let player = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let tech = i32::from_be_bytes(*b"Rhme");
    vm.host()
        .inner
        .native(
            "SetPlayerTechResearched",
            &[player, Value::Int(tech), Value::Int(1)],
        )
        .unwrap();
    let error = flee_decision(&mut vm, a, b, 20.0).expect_err("active upgrade");
    assert!(error.to_string().contains("Rhme"), "{error}");
}

#[test]
fn an_inactive_weapon_upgrade_leaves_the_ordinary_route_admitted() {
    let mut fields = flee_fields("1", "8");
    add_upgrade_list(&mut fields);
    let (mut vm, a, b) = flee_fixture(&fields);
    let player = vm.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    let tech = i32::from_be_bytes(*b"Rhme");
    vm.host()
        .inner
        .native(
            "SetPlayerTechResearched",
            &[player, Value::Int(tech), Value::Int(0)],
        )
        .unwrap();
    assert!(flee_decision(&mut vm, a, b, 20.0).unwrap().is_some());
}

#[test]
fn an_unreachable_route_leaves_no_ownership() {
    let (mut vm, _, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    let error = vm
        .host()
        .inner
        .initiate_campaign_flee(b, [10_000.0, 10_000.0])
        .expect_err("out-of-map route is refused");
    assert!(
        error.to_string().contains("route") || error.to_string().contains("placement"),
        "{error}"
    );
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert!(
        !vm.host()
            .inner
            .executed_unit(b)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    assert_eq!(life(&mut vm, b), 20.0);
}

#[test]
fn generic_damage_does_not_start_a_retreat() {
    let (mut vm, a, b) = flee_fixture(&flee_fields("1", "8"));
    set_life(&mut vm, b, 20.0);
    vm.host().inner.damage_unit(a, b, 1.0).unwrap();
    assert!(!vm.host().inner.campaign_flee_motion_owns(b));
    assert_eq!(life(&mut vm, b), 19.0);
}
