use super::{COMMON, budget, context, id, object, slots_rule};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{ObjectHost, ResearchObservation, TechLimitBudget};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
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
