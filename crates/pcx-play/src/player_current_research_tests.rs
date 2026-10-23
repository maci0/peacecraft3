use super::{COMMON, budget, context, id, object, slots_rule};
use crate::campaign_ai::{AiWorld, Runtime};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{ObjectHost, ResearchObservation, TechLimitBudget};
use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{HandleId, Host, Value};
use std::sync::{Arc, Mutex};
fn common() -> String {
    format!(
        "{COMMON}\nconstant native GetPlayerTechResearched takes player p, integer tech, boolean specific returns boolean\nconstant native SetPlayerTechResearched takes player p, integer tech, integer level returns nothing\nconstant native GetPlayerTechCount takes player p, integer tech, boolean specific returns integer\n"
    )
}
fn setup(source: &str, map: &str, capacity: usize) -> (ObjectHost, Arc<ParsedMapContext>) {
    let ctx = context(301, source, map, "synthetic:current-research");
    let h = object(&ctx, 32)
        .with_players(ctx.clone(), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_player_tech_researched(TechLimitBudget {
            max_records: capacity,
        })
        .unwrap();
    (h, ctx)
}
fn row(
    player_number: i32,
    techid: i32,
    specific_only: bool,
    researched: bool,
) -> ResearchObservation {
    ResearchObservation {
        player_number,
        techid,
        specific_only,
        researched,
    }
}
fn admit(h: &mut ObjectHost, ctx: &Arc<ParsedMapContext>, rows: Vec<ResearchObservation>) {
    h.admit_current_research(ctx, rows, TechLimitBudget { max_records: 8 })
        .unwrap();
}
fn query(h: &mut ObjectHost, player: &Value, tech: i32, specific: bool) -> Result<Value, String> {
    h.native(
        "GetPlayerTechResearched",
        &[player.clone(), Value::Int(tech), Value::Bool(specific)],
    )
}
fn query_count(
    h: &mut ObjectHost,
    player: &Value,
    tech: i32,
    specific: bool,
) -> Result<Value, String> {
    h.native(
        "GetPlayerTechCount",
        &[player.clone(), Value::Int(tech), Value::Bool(specific)],
    )
}

struct AiResearchWorld {
    object: ObjectHost,
    clock: Arc<Mutex<f64>>,
}

impl Host for AiResearchWorld {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        self.object.native(name, args)
    }
}

impl AiWorld for AiResearchWorld {
    fn ai_debug_text(&mut self) -> Result<(), String> {
        self.object.ai_debug_text().map_err(|e| e.to_string())
    }
    fn ai_clock(&self) -> Option<f64> {
        Some(*self.clock.lock().unwrap())
    }

    fn has_map_player_state_callbacks(&self) -> bool {
        false
    }

    fn ai_upgrade_level(&mut self, player: i32, upgrade: i32) -> Result<i32, String> {
        let identity = self
            .object
            .player_identity(player)
            .ok_or("GetUpgradeLevel: AI player identity is not admitted")?;
        match self.object.native(
            "GetPlayerTechCount",
            &[
                Value::Handle(identity),
                Value::Int(upgrade),
                Value::Bool(true),
            ],
        )? {
            Value::Int(level) => Ok(level),
            _ => Err("GetUpgradeLevel: player research returned a non-integer".into()),
        }
    }

    fn ai_unit_amount(&mut self, _player: i32, _unit: i32, _column: &str) -> Result<i32, String> {
        Err("unit costs are outside this research fixture".into())
    }

    fn ai_unit_count(&mut self, player: i32, unit: i32, done: bool) -> Result<i32, String> {
        self.object
            .ai_unit_count(player, unit, done)
            .map_err(|e| e.to_string())
    }

    fn ai_ignored_units(&mut self, player: i32, unit: i32) -> Result<i32, String> {
        self.object
            .ai_ignored_units(player, unit)
            .map_err(|e| e.to_string())
    }

    fn ai_player_unit_type_count(
        &mut self,
        player: Option<HandleId>,
        unit: i32,
    ) -> Result<i32, String> {
        self.object
            .ai_player_unit_type_count(player, unit)
            .map_err(|e| e.to_string())
    }

    fn ai_activate_player(&mut self, player: i32) -> Result<bool, String> {
        self.object
            .activate_campaign_ai_player(player)
            .map_err(|e| e.to_string())
    }

    fn ai_deactivate_player(&mut self, player: i32) -> Result<(), String> {
        self.object
            .deactivate_campaign_ai_player(player)
            .map_err(|e| e.to_string())
    }

    fn ai_set_target_heroes(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.object
            .set_ai_target_heroes(player, enabled)
            .map_err(|e| e.to_string())
    }
    fn ai_set_units_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.object
            .set_ai_units_flee(player, enabled)
            .map_err(|e| e.to_string())
    }
    fn ai_set_heroes_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.object
            .set_ai_heroes_flee(player, enabled)
            .map_err(|e| e.to_string())
    }
    fn ai_set_groups_flee(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.object
            .set_ai_groups_flee(player, enabled)
            .map_err(|e| e.to_string())
    }
    fn ai_set_new_heroes(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.object
            .set_ai_new_heroes(player, enabled)
            .map_err(|e| e.to_string())
    }

    fn ai_set_campaign_mode(&mut self, player: i32) -> Result<(), String> {
        self.object
            .set_ai_campaign_mode(player)
            .map_err(|e| e.to_string())
    }

    fn ai_set_replacement_count(&mut self, player: i32, count: i32) -> Result<(), String> {
        self.object
            .set_ai_replacement_count(player, count)
            .map_err(|e| e.to_string())
    }

    fn ai_set_slow_chopping(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.object
            .set_ai_slow_chopping(player, enabled)
            .map_err(|e| e.to_string())
    }
    fn ai_stop_gathering(&mut self, player: i32) -> Result<(), String> {
        self.object
            .stop_ai_gathering(player)
            .map_err(|e| e.to_string())
    }
    fn ai_create_captains(&mut self, player: i32) -> Result<(), String> {
        self.object
            .create_ai_captains(player)
            .map_err(|e| e.to_string())
    }

    fn ai_set_captain_home(
        &mut self,
        player: i32,
        selector: i32,
        home: [f32; 2],
    ) -> Result<(), String> {
        self.object
            .set_ai_captain_home(player, selector, home)
            .map_err(|e| e.to_string())
    }
    fn ai_set_group_timed_life(&mut self, player: i32, enabled: bool) -> Result<(), String> {
        self.object
            .set_ai_group_timed_life(player, enabled)
            .map_err(|e| e.to_string())
    }
    fn ai_captain_group_size(&mut self, player: i32) -> Result<i32, String> {
        let owner = self
            .object
            .player_identity(player)
            .ok_or("player not admitted")?;
        self.object
            .campaign_captain_group_size(owner)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "captain policy absent".into())
    }
    fn ai_captain_is_empty(&mut self, player: i32) -> Result<bool, String> {
        let owner = self
            .object
            .player_identity(player)
            .ok_or("player not admitted")?;
        self.object
            .campaign_captain_is_empty(owner)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "captain policy absent".into())
    }
}

#[test]
fn campaign_ai_upgrade_query_uses_live_admitted_player_research_without_side_effects() {
    let common = common();
    let (mut object, ctx) = setup(&common, "// map", 2);
    object
        .admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
        .unwrap();
    let first = object.native("Player", &[Value::Int(0)]).unwrap();
    let second = object.native("Player", &[Value::Int(1)]).unwrap();
    for (player, level) in [(&first, 1), (&second, 2)] {
        object
            .native(
                "SetPlayerTechResearched",
                &[player.clone(), Value::Int(5), Value::Int(level)],
            )
            .unwrap();
    }
    let source = "globals\ninteger first=0\ninteger unknown=0\ninteger later=0\ninteger repeated=0\nendglobals\nfunction main takes nothing returns nothing\nset first=GetUpgradeLevel(5)\nset unknown=GetUpgradeLevel(6)\ncall Sleep(1.0)\nset later=GetUpgradeLevel(5)\nset repeated=GetUpgradeLevel(5)\nendfunction\n";
    let ai_common = "native GetAiPlayer takes nothing returns integer\nnative Sleep takes real seconds returns nothing\nnative StartThread takes code callback returns nothing\nnative GetUpgradeLevel takes integer id returns integer\nnative GetUnitGoldCost takes integer unitid returns integer\nnative GetUnitWoodCost takes integer unitid returns integer\n";
    let script = |name: &'static str| {
        Arc::new(
            MapScripts::compile(
                AssetIdentity {
                    edition: "synthetic".into(),
                    version: "upgrade-query".into(),
                    source: name.into(),
                },
                ScriptAsset {
                    label: "synthetic:common",
                    bytes: common.as_bytes(),
                },
                ScriptAsset {
                    label: "synthetic:common-ai",
                    bytes: ai_common.as_bytes(),
                },
                ScriptAsset {
                    label: name,
                    bytes: source.as_bytes(),
                },
                None,
            )
            .unwrap(),
        )
    };
    let red = script("red.ai");
    let purple = script("purple.ai");
    let mut runtime = Runtime::<AiResearchWorld>::default();
    runtime.set_loader(move |name| match name {
        "red.ai" => Ok(Arc::clone(&red)),
        "purple.ai" => Ok(Arc::clone(&purple)),
        _ => Err("unknown AI source".into()),
    });
    let clock = Arc::new(Mutex::new(0.0));
    let mut world = Some(AiResearchWorld {
        object,
        clock: Arc::clone(&clock),
    });
    let snapshot = |world: &Option<AiResearchWorld>| {
        let host = &world.as_ref().unwrap().object;
        (
            host.inner().handles(),
            host.retained_player_state_events().ok(),
            host.script_random_state().cloned(),
            *clock.lock().unwrap(),
            host.player_settings(id(&first))
                .unwrap()
                .research_levels
                .clone(),
            host.player_settings(id(&second))
                .unwrap()
                .research_levels
                .clone(),
        )
    };
    runtime.start(&mut world, 0, "red.ai").unwrap();
    runtime.start(&mut world, 1, "purple.ai").unwrap();
    assert_eq!(runtime.global(0, "first"), Some(Value::Int(1)));
    assert_eq!(runtime.global(1, "first"), Some(Value::Int(2)));
    assert_eq!(runtime.global(0, "unknown"), Some(Value::Int(0)));
    assert_eq!(runtime.global(1, "unknown"), Some(Value::Int(0)));
    world
        .as_mut()
        .unwrap()
        .object
        .native(
            "SetPlayerTechResearched",
            &[first.clone(), Value::Int(5), Value::Int(3)],
        )
        .unwrap();
    *clock.lock().unwrap() = 1.0;
    let before_wake = snapshot(&world);
    for _ in 0..2 {
        let (_, player, ticket) = runtime.next_wake(1.0).unwrap();
        runtime.resume(&mut world, player, ticket).unwrap();
    }
    assert_eq!(runtime.global(0, "later"), Some(Value::Int(3)));
    assert_eq!(runtime.global(1, "later"), Some(Value::Int(2)));
    assert_eq!(runtime.global(0, "repeated"), Some(Value::Int(3)));
    assert_eq!(runtime.global(1, "repeated"), Some(Value::Int(2)));
    assert_eq!(snapshot(&world), before_wake);
}

#[test]
fn fresh_upgrade_queries_use_applied_levels_and_preserve_raw_setter_requests() {
    let map = "globals\nplayer P=Player(0)\ninteger Initial=GetPlayerTechCount(P,5,true)\nendglobals\nfunction change takes nothing returns nothing\ncall SetPlayerTechResearched(P,5,9)\nendfunction\n";
    let (mut h, ctx) = setup(&common(), map, 1);
    h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
        .unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    let p = vm.global("P");
    assert_eq!(vm.global("Initial"), Value::Int(0));
    assert_eq!(query(vm.host(), &p, 5, true).unwrap(), Value::Bool(false));
    let handles = vm.host().inner().handles();
    vm.call("change", &[]).unwrap();
    assert_eq!(query(vm.host(), &p, 5, false).unwrap(), Value::Bool(true));
    assert_eq!(
        vm.host()
            .native(
                "GetPlayerTechCount",
                &[p.clone(), Value::Int(5), Value::Bool(false)]
            )
            .unwrap(),
        Value::Int(4)
    );
    assert_eq!(
        vm.host().player_settings(id(&p)).unwrap().tech_researched,
        [(5, 9)]
    );
    assert_eq!(
        vm.host().player_settings(id(&p)).unwrap().research_levels,
        [(5, 4)]
    );
    for tech in [-37, 6] {
        assert!(
            vm.host()
                .native(
                    "SetPlayerTechResearched",
                    &[p.clone(), Value::Int(tech), Value::Int(1)]
                )
                .is_err()
        );
        assert_eq!(
            vm.host().player_settings(id(&p)).unwrap().research_levels,
            [(5, 4)]
        );
    }
    vm.host()
        .native(
            "SetPlayerTechResearched",
            &[p.clone(), Value::Int(5), Value::Int(-3)],
        )
        .unwrap();
    assert_eq!(query(vm.host(), &p, 5, true).unwrap(), Value::Bool(false));
    assert_eq!(
        vm.host().player_settings(id(&p)).unwrap().tech_researched,
        [(5, -3)]
    );
    assert_eq!(
        vm.host().player_settings(id(&p)).unwrap().research_levels,
        [(5, 0)]
    );
    assert_eq!(vm.host().inner().handles(), handles);
    let late = vm.host().native("Player", &[Value::Int(8)]).unwrap();
    assert_eq!(
        query(vm.host(), &late, 5, true).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        query(vm.host(), &late, 6, true).unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        query_count(vm.host(), &late, 6, true).unwrap(),
        Value::Int(0)
    );
    assert!(
        vm.host()
            .admit_current_research(
                &ctx,
                vec![row(0, 5, true, false)],
                TechLimitBudget { max_records: 8 }
            )
            .is_err()
    );
    vm.host().invalidate_current_research();
    assert!(
        query(vm.host(), &p, 5, true)
            .unwrap_err()
            .contains("invalidated")
    );
    assert!(
        vm.host()
            .native(
                "SetPlayerTechResearched",
                &[p.clone(), Value::Int(5), Value::Int(1)]
            )
            .is_err()
    );
    assert_eq!(
        vm.host().player_settings(id(&p)).unwrap().research_levels,
        [(5, 0)]
    );
}

#[test]
fn absent_research_ids_answer_false_and_zero_for_both_queries_and_flags() {
    let (mut h, ctx) = setup(&common(), "globals\nplayer P = Player(0)\nendglobals\n", 1);
    h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
        .unwrap();
    let p = h.native("Player", &[Value::Int(0)]).unwrap();
    let other = h.native("Player", &[Value::Int(1)]).unwrap();
    for player in [&p, &other] {
        for tech in [-37, 5, 6] {
            for specific in [true, false] {
                assert_eq!(
                    query(&mut h, player, tech, specific).unwrap(),
                    Value::Bool(false)
                );
                assert_eq!(
                    query_count(&mut h, player, tech, specific).unwrap(),
                    Value::Int(0)
                );
            }
        }
    }
    assert!(
        h.native(
            "SetPlayerTechResearched",
            &[p.clone(), Value::Int(6), Value::Int(1)]
        )
        .is_err()
    );
    let color = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    let bad = [
        vec![],
        vec![p.clone(), Value::Int(5)],
        vec![p.clone(), Value::Int(5), Value::Bool(true), Value::Int(0)],
        vec![p.clone(), Value::Real(5.0), Value::Bool(true)],
        vec![p.clone(), Value::Int(5), Value::Int(1)],
        vec![color, Value::Int(5), Value::Bool(true)],
        vec![Value::Null, Value::Int(5), Value::Bool(true)],
    ];
    for args in bad {
        assert!(h.native("GetPlayerTechResearched", &args).is_err());
        assert!(h.native("GetPlayerTechCount", &args).is_err());
    }
}

#[test]
fn upgrade_admission_rejects_nonfresh_or_wrong_contexts_without_switching_query_modes() {
    let c = common();
    let (mut h, ctx) = setup(&c, "// map", 2);
    let wrong = context(301, &c, "// map", "synthetic:current-research");
    assert!(
        h.admit_upgrade_research(
            &wrong,
            crate::upgrade_research::fixture(&ctx.sources.catalog)
        )
        .is_err()
    );
    let p = h.native("Player", &[Value::Int(0)]).unwrap();
    assert!(
        query(&mut h, &p, 5, true)
            .unwrap_err()
            .contains("unsupported")
    );
    admit(&mut h, &ctx, vec![row(0, 5, true, true)]);
    assert!(
        h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
            .is_err()
    );
    assert_eq!(query(&mut h, &p, 5, true).unwrap(), Value::Bool(true));
    let (mut h, ctx) = setup(&c, "// map", 2);
    let p = h.native("Player", &[Value::Int(0)]).unwrap();
    h.native(
        "SetPlayerTechResearched",
        &[p.clone(), Value::Int(5), Value::Int(2)],
    )
    .unwrap();
    assert!(
        h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
            .is_err()
    );
    assert_eq!(h.player_settings(id(&p)).unwrap().tech_researched, [(5, 2)]);
    let c = c.replace(
        "constant native GetPlayerTechCount",
        "native GetPlayerTechCount",
    );
    let (mut h, ctx) = setup(&c, "// map", 2);
    assert!(
        h.admit_upgrade_research(&ctx, crate::upgrade_research::fixture(&ctx.sources.catalog))
            .is_err()
    );
    let p = h.native("Player", &[Value::Int(0)]).unwrap();
    assert!(
        query(&mut h, &p, 5, true)
            .unwrap_err()
            .contains("unsupported")
    );
}
#[test]
fn owned_vm_uses_explicit_query_keys_without_replacing_setter_levels_or_minting_players() {
    let map = "globals\nplayer P = Player(0)\nplayer Alias = Player(0)\nplayer Later = null\nendglobals\nfunction config takes nothing returns nothing\ncall SetPlayerTechResearched(P,-37,0)\nendfunction\nfunction late takes nothing returns boolean\nset Later = Player(8)\nreturn GetPlayerTechResearched(Later,-37,true)\nendfunction\n";
    let (h, ctx) = setup(&common(), map, 2);
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("config", &[]).unwrap();
    let p = vm.global("P");
    let alias = vm.global("Alias");
    assert!(query(vm.host(), &p, -37, true).is_err());
    let before = vm.host().inner().handles();
    admit(
        vm.host(),
        &ctx,
        vec![
            row(8, -37, true, false),
            row(0, -37, true, true),
            row(0, -37, false, false),
        ],
    );
    assert_eq!(vm.host().inner().handles(), before);
    assert_eq!(
        query(vm.host(), &alias, -37, true).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        query(vm.host(), &p, -37, false).unwrap(),
        Value::Bool(false)
    );
    assert!(query(vm.host(), &p, 0, true).unwrap_err().contains("unset"));
    assert_eq!(
        vm.host().player_settings(id(&p)).unwrap().tech_researched,
        vec![(-37, 0)]
    );
    assert_eq!(vm.call("late", &[]).unwrap(), Value::Bool(false));
    let later = vm.global("Later");
    assert_eq!(vm.host().inner().handles(), before + 1);
    assert!(
        vm.host()
            .player_settings(id(&later))
            .unwrap()
            .tech_researched
            .is_empty()
    );
    admit(vm.host(), &ctx, vec![]);
    assert!(
        query(vm.host(), &p, -37, true)
            .unwrap_err()
            .contains("unset")
    );
    admit(vm.host(), &ctx, vec![row(0, -37, true, true)]);
    vm.host().invalidate_current_research();
    assert!(
        query(vm.host(), &p, -37, true)
            .unwrap_err()
            .contains("unset")
    );
    assert!(
        vm.host()
            .native(
                "GetPlayerTechCount",
                &[p.clone(), Value::Int(-37), Value::Bool(true)]
            )
            .is_err()
    );
    assert_eq!(
        vm.host().player_settings(id(&p)).unwrap().tech_researched,
        vec![(-37, 0)]
    );
}
#[test]
fn invalid_input_and_failed_setters_preserve_answers_while_successful_writes_retire_player_queries()
{
    let source = common();
    let (mut h, ctx) = setup(&source, "// map", 1);
    let a = h.native("Player", &[Value::Int(0)]).unwrap();
    let b = h.native("Player", &[Value::Int(1)]).unwrap();
    h.native(
        "SetPlayerTechResearched",
        &[a.clone(), Value::Int(5), Value::Int(1)],
    )
    .unwrap();
    let input = vec![
        row(0, 5, true, false),
        row(0, 5, false, true),
        row(0, -7, true, true),
        row(1, 5, true, true),
    ];
    admit(&mut h, &ctx, input.clone());
    for bad in [
        vec![row(-1, 5, true, true)],
        vec![row(9, 5, true, true)],
        vec![row(0, 5, true, true), row(0, 5, true, false)],
    ] {
        assert!(
            h.admit_current_research(&ctx, bad, TechLimitBudget { max_records: 8 })
                .is_err()
        );
        assert_eq!(query(&mut h, &a, 5, true).unwrap(), Value::Bool(false));
    }
    assert!(
        h.admit_current_research(&ctx, input.clone(), TechLimitBudget { max_records: 3 })
            .is_err()
    );
    assert!(
        h.admit_current_research(
            &ctx,
            input.clone(),
            TechLimitBudget {
                max_records: usize::MAX
            }
        )
        .is_err()
    );
    let foreign = context(301, &source, "// map", "other equivalent context");
    assert!(
        h.admit_current_research(&foreign, input.clone(), TechLimitBudget { max_records: 8 })
            .is_err()
    );
    let color = h.native("ConvertPlayerColor", &[Value::Int(37)]).unwrap();
    let before = h.inner().handles();
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
    for bad in [
        vec![],
        vec![a.clone(), Value::Int(5)],
        vec![a.clone(), Value::Int(5), Value::Int(1)],
        vec![a.clone(), Value::Real(5.0), Value::Bool(true)],
        vec![color, Value::Int(5), Value::Bool(true)],
        vec![unknown, Value::Int(5), Value::Bool(true)],
        vec![Value::Null, Value::Int(5), Value::Bool(true)],
    ] {
        assert!(h.native("GetPlayerTechResearched", &bad).is_err());
    }
    for bad in [
        vec![a.clone(), Value::Int(6), Value::Int(2)],
        vec![a.clone(), Value::Int(5), Value::Real(2.0)],
        vec![Value::Null, Value::Int(5), Value::Int(2)],
    ] {
        assert!(h.native("SetPlayerTechResearched", &bad).is_err());
        assert_eq!(query(&mut h, &a, 5, true).unwrap(), Value::Bool(false));
    }
    assert_eq!(h.inner().handles(), before);
    h.native(
        "SetPlayerTechResearched",
        &[a.clone(), Value::Int(5), Value::Int(0)],
    )
    .unwrap();
    for (tech, specific) in [(5, true), (5, false), (-7, true)] {
        assert!(query(&mut h, &a, tech, specific).is_err());
    }
    assert_eq!(query(&mut h, &b, 5, true).unwrap(), Value::Bool(true));
    assert_eq!(
        h.player_settings(id(&a)).unwrap().tech_researched,
        vec![(5, 0)]
    );
    admit(&mut h, &ctx, input);
    assert_eq!(query(&mut h, &a, 5, true).unwrap(), Value::Bool(false));
    h.invalidate_current_research();
    assert!(query(&mut h, &b, 5, true).is_err());
}
#[test]
fn current_research_requires_exact_boolean_contract_and_admitted_context() {
    let source = common();
    let ctx = context(301, &source, "// map", "no players");
    assert!(
        object(&ctx, 32)
            .admit_current_research(&ctx, vec![], TechLimitBudget { max_records: 0 })
            .is_err()
    );
    for source in [
        source.replace(
            "constant native GetPlayerTechResearched",
            "native GetPlayerTechResearched",
        ),
        source.replace("specific returns boolean", "specific returns integer"),
        source.replace(
            "integer tech, boolean specific returns boolean",
            "integer tech, integer specific returns boolean",
        ),
    ] {
        let (mut h, ctx) = setup(&source, "// map", 1);
        assert!(
            h.admit_current_research(&ctx, vec![], TechLimitBudget { max_records: 0 })
                .is_err()
        );
    }
    let (mut h, ctx) = setup(
        &source,
        "function GetPlayerTechResearched takes player p, integer tech, boolean specific returns boolean\nreturn true\nendfunction\n",
        1,
    );
    assert!(
        h.admit_current_research(&ctx, vec![], TechLimitBudget { max_records: 0 })
            .is_err()
    );
    for declaration in [
        "constant native GetPlayerTechResearched takes player p, integer tech, boolean specific returns boolean\n",
        "constant native SetPlayerTechResearched takes player p, integer tech, integer level returns nothing\n",
    ] {
        let foreign = source.replace(declaration, "");
        let (mut h, ctx) = setup(&foreign, declaration, 1);
        assert!(
            h.admit_current_research(&ctx, vec![], TechLimitBudget { max_records: 0 })
                .is_err()
        );
    }
    let (mut h, ctx) = setup(&source, "// empty", 1);
    h.invalidate_current_research();
    h.admit_current_research(&ctx, vec![], TechLimitBudget { max_records: 0 })
        .unwrap();
}
