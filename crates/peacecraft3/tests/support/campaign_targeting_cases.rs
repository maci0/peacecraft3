//! Campaign policy dispatch and acquisition share the published map world.
use super::*;
use pcx_objects::editor::{FieldDef, FieldKind, FieldSource};
use pcx_sim::world::{Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem};

struct Fighters;
impl ObjectCatalog for Fighters {
    fn resolve(&self, rawcode: [u8; 4]) -> Result<Definition, ResolutionProblem> {
        if !matches!(&rawcode, b"a001" | b"t001" | b"b001") {
            return Err(ResolutionProblem::MissingRawcode(
                String::from_utf8_lossy(&rawcode).into_owned(),
            ));
        }
        let mut definition = unit_range_cases::Geometry.resolve(*b"UNIT")?;
        definition.rawcode = rawcode;
        definition.base_rawcode = String::from_utf8(rawcode.to_vec()).unwrap();
        for (index, (slk, column, kind, value)) in [
            (SlkId::UnitBalance, "HP", FieldKind::Int, "20"),
            (
                SlkId::UnitBalance,
                "isbldg",
                FieldKind::Bool,
                if rawcode == *b"b001" { "1" } else { "0" },
            ),
            (SlkId::UnitBalance, "manaN", FieldKind::Int, "0"),
            (SlkId::UnitBalance, "mana0", FieldKind::Int, "0"),
            (SlkId::UnitBalance, "def", FieldKind::Int, "0"),
            (SlkId::UnitBalance, "defType", FieldKind::Token, "medium"),
            (SlkId::UnitBalance, "regenHP", FieldKind::Real, "0"),
            (SlkId::UnitBalance, "regenMana", FieldKind::Real, "0"),
            (SlkId::UnitBalance, "regenType", FieldKind::Token, "none"),
            (SlkId::UnitBalance, "level", FieldKind::Int, "1"),
            (SlkId::UnitBalance, "type", FieldKind::Token, "_"),
            (SlkId::UnitAbilities, "abilList", FieldKind::Token, "_"),
            (
                SlkId::UnitWeapons,
                "weapsOn",
                FieldKind::Int,
                if rawcode == *b"a001" { "1" } else { "0" },
            ),
            (SlkId::UnitWeapons, "atkType1", FieldKind::Token, "normal"),
            (SlkId::UnitWeapons, "weapTp1", FieldKind::Token, "normal"),
            (SlkId::UnitWeapons, "rangeN1", FieldKind::Real, "16"),
            (SlkId::UnitWeapons, "acquire", FieldKind::Real, "12"),
            (SlkId::UnitWeapons, "cool1", FieldKind::Real, "1.5"),
            (SlkId::UnitWeapons, "dmgpt1", FieldKind::Real, "0.25"),
            (SlkId::UnitWeapons, "dmgplus1", FieldKind::Int, "4"),
            (SlkId::UnitWeapons, "dice1", FieldKind::Int, "1"),
            (SlkId::UnitWeapons, "sides1", FieldKind::Int, "1"),
        ]
        .into_iter()
        .enumerate()
        {
            let id = [b'T', b'0', b'A' + index as u8, b'0'];
            definition.fields.insert(
                id,
                ObjectField {
                    metadata: Some(FieldDef {
                        fourcc: id,
                        from: SlkId::UnitMetaData,
                        field: column.into(),
                        slk: slk.name().into(),
                        index: -1,
                        declared: match (column, kind) {
                            ("type", _) => "unitClass",
                            (_, FieldKind::Int) => "int",
                            (_, FieldKind::Real) => "real",
                            (_, FieldKind::Bool) => "bool",
                            _ => "string",
                        }
                        .into(),
                        kind,
                        category: "synthetic targeting".into(),
                        source: FieldSource::Column {
                            slk,
                            column: column.into(),
                        },
                        measured: true,
                    }),
                    value: Ok(FieldValue::Cell(value.into())),
                },
            );
        }
        Ok(definition)
    }
}

const EXTRA: &str = r#"
type playerunitevent extends eventid
type gamedifficulty extends handle
type unittype extends handle
constant native ConvertPlayerUnitEvent takes integer n returns playerunitevent
constant native ConvertMapControl takes integer n returns mapcontrol
constant native ConvertGameDifficulty takes integer n returns gamedifficulty
constant native ConvertUnitType takes integer n returns unittype
constant native GetGameDifficulty takes nothing returns gamedifficulty
native SetGameDifficulty takes gamedifficulty d returns nothing
native StartCampaignAI takes player p, string path returns nothing
constant native GetUnitState takes unit u, unitstate s returns real
native SetRandomSeed takes integer n returns nothing
native GetRandomInt takes integer a, integer b returns integer
native GetRandomReal takes real a, real b returns real
constant native OrderId takes string s returns integer
constant native OrderId2String takes integer i returns string
native IssuePointOrder takes unit u, string s, real x, real y returns boolean
native GetUnitCurrentOrder takes unit u returns integer
constant native GetUnitMoveSpeed takes unit u returns real
constant native GetUnitX takes unit u returns real
constant native GetUnitY takes unit u returns real
native GroupEnumUnitsInRect takes group g, rect r, boolexpr f returns nothing
globals
constant unitstate UNIT_STATE_LIFE=ConvertUnitState(0)
constant unitstate UNIT_STATE_MAX_LIFE=ConvertUnitState(1)
constant unitstate UNIT_STATE_MANA=ConvertUnitState(2)
constant unitstate UNIT_STATE_MAX_MANA=ConvertUnitState(3)
constant alliancetype ALLIANCE_PASSIVE=ConvertAllianceType(0)
constant unitevent EVENT_UNIT_DEATH=ConvertUnitEvent(53)
constant unitevent EVENT_UNIT_ATTACKED=ConvertUnitEvent(54)
constant playerunitevent EVENT_PLAYER_UNIT_DEATH=ConvertPlayerUnitEvent(20)
constant playerunitevent EVENT_PLAYER_UNIT_ATTACKED=ConvertPlayerUnitEvent(117)
constant mapcontrol MAP_CONTROL_COMPUTER=ConvertMapControl(1)
constant gamedifficulty MAP_DIFFICULTY_NORMAL=ConvertGameDifficulty(1)
constant unittype UNIT_TYPE_PEON=ConvertUnitType(16)
endglobals
"#;

#[test]
fn campaign_ai_targeting_reaches_automatic_acquisition_in_the_published_session() {
    use pcx_play::map_startup_host::MapStartupError;
    use pcx_play::playercolor_host::CombatTraceEvent;
    let script = r#"
globals
unit A=null
unit B=null
unit AOld=null
unit ANew=null
unit BOld=null
unit BNew=null
unit ABase=null
unit BBase=null
endglobals
function config takes nothing returns nothing
call SetPlayers(3)
call SetPlayerController(Player(0), MAP_CONTROL_COMPUTER)
call SetPlayerController(Player(1), MAP_CONTROL_COMPUTER)
endfunction
function main takes nothing returns nothing
call SetRandomSeed(9)
call SetPlayerAlliance(Player(0),Player(2),ALLIANCE_PASSIVE,false)
call SetPlayerAlliance(Player(1),Player(2),ALLIANCE_PASSIVE,false)
set A=CreateUnit(Player(0),'a001',16.0,16.0,0.0)
set AOld=CreateUnit(Player(2),'t001',24.0,16.0,0.0)
set ANew=CreateUnit(Player(2),'t001',16.0,24.0,0.0)
set B=CreateUnit(Player(1),'a001',80.0,16.0,0.0)
set BOld=CreateUnit(Player(2),'t001',88.0,16.0,0.0)
set BNew=CreateUnit(Player(2),'t001',80.0,24.0,0.0)
set ABase=CreateUnit(Player(0),'b001',40.0,16.0,0.0)
set BBase=CreateUnit(Player(1),'b001',104.0,16.0,0.0)
call StartCampaignAI(Player(0),"false.ai")
call StartCampaignAI(Player(1),"true.ai")
endfunction
"#;
    let mut map = fixture(script);
    map.common
        .extend_from_slice(script_timer_cases::TIMER_COMMON.as_bytes());
    map.common.extend_from_slice(EXTRA.as_bytes());
    map.rebuild();
    let reader = map.resolver();
    let bound = bind_map(&reader, map.input(1), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::new(Fighters));
    inputs.timer_time = Some(0.0);
    inputs.random = Some(pcx_play::map_startup_host::RandomInput {
        policy: Arc::new(pcx_play::script_random::profile::fixture()),
        max_secondary: 8,
    });
    inputs.targeting_policy = Some(pcx_play::campaign_targeting_policy::TargetingPolicy::fixture());
    let flee_policy = pcx_play::campaign_flee_policy::FleePolicy::fixture();
    inputs.flee_policy = Some(flee_policy);
    inputs.gathering_policy = Some(pcx_play::campaign_gathering_policy::GatheringPolicy::fixture());
    let captain_policy = pcx_play::campaign_captain_policy::CaptainPolicy::fixture();
    let captain_selector = i32::from(captain_policy.first_selector_mask);
    inputs.captain_policy = Some(captain_policy);
    inputs.new_heroes_policy =
        Some(pcx_play::campaign_new_heroes_policy::NewHeroesPolicy::fixture());
    inputs.post_config.difficulty_global = Some("MAP_DIFFICULTY_NORMAL".into());
    inputs.enumeration.names.extend(
        [
            "ConvertGameDifficulty",
            "ConvertPlayerUnitEvent",
            "ConvertMapControl",
            "ConvertUnitType",
        ]
        .map(str::to_owned),
    );
    inputs.enumeration.limits.max_handles = 256;
    inputs.budgets.units.max_records = 8;
    inputs.budgets.units.max_retained_fields = 256;
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic campaign targeting".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id,
        binding_revision: 1,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    let context = app
        .world()
        .resource::<MapRequests>()
        .admission
        .shared_context()
        .unwrap();
    let ai_common = format!(
        "{AI_COMMON}native SetUnitsFlee takes boolean enabled returns nothing\nnative StopGathering takes nothing returns nothing\nnative CreateCaptains takes nothing returns nothing\nnative SetCaptainHome takes integer which, real x, real y returns nothing\nnative SetNewHeroes takes boolean enabled returns nothing\n"
    );
    let programs: std::collections::BTreeMap<_, _> = [("false.ai", false), ("true.ai", true)].into_iter().map(|(name, flag)| {
        let home_x = if flag { 112.0 } else { 48.0 };
        let body = format!("function main takes nothing returns nothing\ncall SetTargetHeroes({flag})\ncall SetUnitsFlee({})\ncall SetNewHeroes({flag})\ncall CreateCaptains()\ncall StopGathering()\ncall Sleep(0.25)\ncall SetTargetHeroes({})\ncall SetUnitsFlee({flag})\ncall SetNewHeroes({})\ncall StopGathering()\ncall SetCaptainHome({captain_selector},{home_x:.1},24.0)\nendfunction\n", !flag, !flag, !flag);
        let program = pcx_play::scripts::MapScripts::compile(
            AssetIdentity { edition: "synthetic".into(), version: "targeting".into(), source: name.into() },
            ScriptAsset { label: "fixture:common.j", bytes: &map.common },
            ScriptAsset { label: "fixture:common.ai", bytes: ai_common.as_bytes() },
            ScriptAsset { label: name, bytes: body.as_bytes() }, None,
        ).unwrap();
        (name, Arc::new(program))
    }).collect();
    let entries =
        ["config", "main"].map(|name| context.scripts.program().function_id(name).unwrap());
    let post_config = inputs.post_config.clone();
    let admitted = Arc::clone(&context);
    let mut session = MapScriptSession::<MapStartupHost>::default();
    session
        .replace_sequenced(
            Arc::clone(&context),
            move |ctx| {
                let source = ctx.world.source.clone();
                let mut host = pcx_play::map_startup_host::assemble(ctx, inputs)?;
                let (registry, activity) = unit_range_cases::activity_rules();
                host.objects_mut().admit_order_registry(registry)?;
                host.objects_mut().admit_unit_activity(activity, 65536)?;
                host.objects_mut()
                    .admit_unit_placement(unit_range_cases::placement(source))?;
                host.objects_mut()
                    .admit_unit_vitals(pcx_objects::editor::misc::HeroVitals {
                        life_per_strength: 25.,
                        mana_per_intelligence: 15.,
                        life_regen_per_strength: 0.,
                        mana_regen_per_intelligence: 0.,
                    })?;
                host.objects_mut().admit_combat(
                    pcx_objects::editor::misc::CombatRules {
                        sources: vec!["synthetic targeting rules".into()],
                        armor_factor: 0.06,
                        primary_damage_per_attribute: 1.,
                        armor_per_agility: 0.3,
                        hero_armor_base: -2.,
                        attack_speed_per_agility: 0.02,
                        damage_bonus: [[1.; 8]; 7],
                        call_for_help: 0.,
                        creep_call_for_help: 0.,
                        guard_distance: 0.,
                        max_guard_distance: 0.,
                        guard_return_time: 0.,
                    },
                    "alpha",
                )?;
                host.objects_mut().enable_combat_trace(16);
                Ok::<_, MapStartupError>(host.with_ai_loader(move |name| {
                    programs
                        .get(name)
                        .cloned()
                        .ok_or_else(|| format!("missing {name}"))
                }))
            },
            &entries,
            move |vm, completed| {
                pcx_play::campaign_host::apply_after_config(vm, &admitted, completed, &post_config)
            },
        )
        .unwrap();
    let handle = |session: &mut MapScriptSession<MapStartupHost>, name| {
        let Some(Value::Handle(id)) = session.global(name) else {
            panic!("unit {name}")
        };
        id
    };
    let a = handle(&mut session, "A");
    let b = handle(&mut session, "B");
    let a_new = handle(&mut session, "ANew");
    let b_new = handle(&mut session, "BNew");
    let a_base = handle(&mut session, "ABase");
    let b_base = handle(&mut session, "BBase");
    let flag = |session: &mut MapScriptSession<MapStartupHost>, unit| {
        let objects = session.host().unwrap().objects();
        let owner = objects.executed_unit(unit).unwrap().owner;
        objects.campaign_targeting_enabled(owner).unwrap()
    };
    assert_eq!(flag(&mut session, a), Some(false));
    assert_eq!(flag(&mut session, b), Some(true));
    let flee_flag = |session: &mut MapScriptSession<MapStartupHost>, unit| {
        let objects = session.host().unwrap().objects();
        let owner = objects.executed_unit(unit).unwrap().owner;
        objects.campaign_flee_enabled(owner).unwrap()
    };
    assert_eq!(flee_flag(&mut session, a), Some(true));
    assert_eq!(flee_flag(&mut session, b), Some(false));
    let new_heroes_flag = |session: &mut MapScriptSession<MapStartupHost>, unit| {
        let objects = session.host().unwrap().objects();
        let owner = objects.executed_unit(unit).unwrap().owner;
        objects.campaign_new_heroes_enabled(owner).unwrap()
    };
    assert_eq!(new_heroes_flag(&mut session, a), Some(false));
    assert_eq!(new_heroes_flag(&mut session, b), Some(true));
    let check_gathering = |session: &mut MapScriptSession<MapStartupHost>| {
        let objects = session.host().unwrap().objects();
        for (unit, base) in [(a, a_base), (b, b_base)] {
            let owner = objects.executed_unit(unit).unwrap().owner;
            let state = objects.campaign_gathering_state(owner).unwrap().unwrap();
            assert_eq!(state.enabled, Some(false));
            let position = objects.executed_unit(base).unwrap().position.unwrap();
            assert_eq!(state.association, Some((base, position)));
        }
    };
    check_gathering(&mut session);
    let check_captains = |session: &mut MapScriptSession<MapStartupHost>, updated: bool| {
        let objects = session.host().unwrap().objects();
        for (unit, base) in [(a, a_base), (b, b_base)] {
            let owner = objects.executed_unit(unit).unwrap().owner;
            let state = objects.campaign_captain_state(owner).unwrap().unwrap();
            assert!(state.created);
            let position = objects.executed_unit(base).unwrap().position.unwrap();
            let first_home = if updated {
                [position[0] + 8.0, position[1] + 8.0]
            } else {
                position
            };
            assert_eq!(state.homes, [Some(first_home), Some(position)]);
        }
    };
    check_captains(&mut session, false);
    session.advance_timers(0.125, 1000).unwrap();
    let acquired: Vec<_> = session
        .host()
        .unwrap()
        .objects()
        .combat_trace()
        .unwrap()
        .events()
        .iter()
        .filter_map(|event| {
            if let CombatTraceEvent::Acquired { source, target, .. } = event {
                Some((source.unit, target.unit))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(acquired, [(a, a_new), (b, b_new)]);
    session.advance_timers(0.25, 1000).unwrap();
    assert_eq!(flag(&mut session, a), Some(true));
    assert_eq!(flag(&mut session, b), Some(false));
    assert_eq!(flee_flag(&mut session, a), Some(false));
    assert_eq!(flee_flag(&mut session, b), Some(true));
    assert_eq!(new_heroes_flag(&mut session, a), Some(true));
    assert_eq!(new_heroes_flag(&mut session, b), Some(false));
    check_gathering(&mut session);
    check_captains(&mut session, true);
    for (unit, target) in [(a, a_new), (b, b_new)] {
        assert_eq!(
            session
                .host()
                .unwrap()
                .objects()
                .executed_unit(unit)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .target,
            Some(target)
        );
    }
}
