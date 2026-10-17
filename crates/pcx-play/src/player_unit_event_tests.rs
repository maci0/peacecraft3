use super::{EXTRAS, budget, common_source, context, slots_rule, trigger_limits};
use crate::playercolor_host::{EnumHost, Failure, FilterBudget, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Prelude, Value};
use std::sync::Arc;

const EMPTY_MAP: &str = "function unused takes nothing returns nothing\nendfunction\n";
const REGISTER: &str = "TriggerRegisterPlayerUnitEvent";
const EXTRA: &str = "\
type playerunitevent extends eventid
type boolexpr extends agent
type filterfunc extends boolexpr
constant native ConvertPlayerUnitEvent takes integer n returns playerunitevent
native TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event
native Filter takes code predicate returns filterfunc
";
fn common() -> String {
    common_source(&format!("{EXTRAS}{EXTRA}")).replacen(
        "\nendglobals\n",
        "\nconstant playerunitevent PLAYER_SAMPLE = ConvertPlayerUnitEvent(27)\nendglobals\n",
        1,
    )
}
fn host(
    common: &str,
    map: &str,
    max: usize,
) -> (Arc<crate::map_admission::ParsedMapContext>, ObjectHost) {
    let ctx = context(300, common, map, "player-unit-event");
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic player-unit event",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
            "ConvertPlayerUnitEvent",
        ],
        Limits {
            max_handles: max,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 2000,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_trigger_admission(trigger_limits(4, 0))
        .unwrap()
        .with_filters(FilterBudget { max_records: 4 })
        .unwrap();
    (ctx, host)
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}
fn inputs(h: &mut ObjectHost) -> [Value; 4] {
    [
        h.native("CreateTrigger", &[]).unwrap(),
        h.native("Player", &[Value::Int(4)]).unwrap(),
        h.native("ConvertPlayerUnitEvent", &[Value::Int(27)])
            .unwrap(),
        Value::Null,
    ]
}

#[test]
fn owned_vm_retains_null_and_filter_associations_without_evaluating_callbacks() {
    let map = "globals\nplayer P=null\ntrigger T=null\nevent A=null\nevent B=null\nfilterfunc F=null\ninteger hits=0\nendglobals\nfunction predicate takes nothing returns boolean\nset hits=hits+1\nreturn true\nendfunction\nfunction main takes nothing returns nothing\nset P=Player(4)\nset T=CreateTrigger()\nset F=Filter(function predicate)\nset A=TriggerRegisterPlayerUnitEvent(T,P,PLAYER_SAMPLE,null)\nset B=TriggerRegisterPlayerUnitEvent(T,P,PLAYER_SAMPLE,F)\nendfunction\n";
    let (ctx, h) = host(&common(), map, 64);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h.with_player_unit_events(2).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let a = id(&vm.global("A"));
    let b = id(&vm.global("B"));
    assert_ne!(a, b);
    assert_eq!(vm.global("hits"), Value::Int(0));
    let p = id(&vm.global("P"));
    let t = id(&vm.global("T"));
    let f = id(&vm.global("F"));
    let h = &mut vm.host().inner;
    let first = h.player_unit_event_registration(a).unwrap();
    let second = h.player_unit_event_registration(b).unwrap();
    assert_eq!(
        (first.trigger, first.player, first.enumerand, first.filter),
        (t, p, 27, None)
    );
    assert_eq!(second.filter, Some(f));
    assert_eq!(
        (
            first.trigger,
            first.player,
            first.playerunitevent,
            first.enumerand
        ),
        (
            second.trigger,
            second.player,
            second.playerunitevent,
            second.enumerand
        )
    );
    assert_eq!(h.inner().kind(a).unwrap(), "event");
    assert_eq!(h.retained_player_unit_events().unwrap(), 2);
    assert_eq!(h.retained_unit_events().unwrap(), 0);
    assert!(h.native("IsTriggerEnabled", &[Value::Handle(t)]).is_err());
}

#[test]
fn invalid_arguments_and_identity_families_preserve_existing_records() {
    let (_, h) = host(&common(), EMPTY_MAP, 64);
    let mut h = h.with_player_unit_events(4).unwrap();
    let args = inputs(&mut h);
    let first = h.native(REGISTER, &args).unwrap();
    let before = h.inner().handles();
    for bad in [
        vec![],
        vec![Value::Null; 4],
        vec![
            args[1].clone(),
            args[0].clone(),
            args[2].clone(),
            Value::Null,
        ],
        vec![
            args[0].clone(),
            args[1].clone(),
            args[0].clone(),
            Value::Null,
        ],
        vec![
            args[0].clone(),
            args[1].clone(),
            args[2].clone(),
            args[1].clone(),
        ],
    ] {
        assert!(h.native(REGISTER, &bad).is_err());
        assert_eq!(h.inner().handles(), before);
    }
    assert_eq!(h.retained_player_unit_events().unwrap(), 1);
    assert_eq!(
        h.player_unit_event_registration(id(&first)).unwrap().player,
        id(&args[1])
    );
    assert!(h.player_unit_event_registration(id(&args[0])).is_err());
}

#[test]
fn record_and_shared_arena_budgets_fail_without_partial_registration() {
    for (limit, max, expected) in [(0, 64, 0), (1, 64, 1), (2, 64, 1)] {
        let (_, h) = host(&common(), EMPTY_MAP, max);
        let mut h = h.with_player_unit_events(limit).unwrap();
        let args = inputs(&mut h);
        if expected == 1 {
            h.native(REGISTER, &args).unwrap();
        }
        if limit == 2 {
            while h.inner().handles() < max {
                h.native("CreateTimer", &[]).unwrap();
            }
        }
        let before = h.inner().handles();
        assert!(h.native(REGISTER, &args).is_err());
        assert_eq!(h.retained_player_unit_events().unwrap(), expected);
        assert_eq!(h.inner().handles(), before);
    }
}

#[test]
fn contract_ancestry_duplicate_admission_and_default_refusal_are_explicit() {
    let (_, mut h) = host(&common(), EMPTY_MAP, 32);
    let args = inputs(&mut h);
    assert!(
        h.native(REGISTER, &args)
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(h.retained_player_unit_events().is_err());
    assert!(
        h.with_player_unit_events(1)
            .unwrap()
            .with_player_unit_events(1)
            .is_err()
    );
    let (_, h) = host(&common(), EMPTY_MAP, 32);
    assert!(matches!(h.with_player_unit_events(usize::MAX),Err(e) if e.failure==Failure::Capacity));
    for bad in [
        common().replace(
            "native TriggerRegisterPlayerUnitEvent",
            "constant native TriggerRegisterPlayerUnitEvent",
        ),
        common().replace("boolexpr f returns event", "boolexpr f returns trigger"),
        common().replace(
            "type playerunitevent extends eventid",
            "type playerunitevent extends handle",
        ),
    ] {
        let (_, h) = host(&bad, EMPTY_MAP, 32);
        assert!(h.with_player_unit_events(1).is_err());
    }
}

#[test]
fn duplicate_register_sources_are_rejected_by_the_shared_contract() {
    let src = common();
    let common = pcx_script::parse_script(&src).unwrap();
    let other=pcx_script::parse_script("native TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event\n").unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = pcx_script::resolve_with(
        &map,
        &[
            Prelude::new("synthetic:common", &common),
            Prelude::new("synthetic:other", &other),
        ],
    )
    .unwrap();
    let inner = EnumHost::checked_selected(
        Arc::new(program),
        "synthetic map",
        &["ConvertPlayerUnitEvent", "ConvertUnitEvent"],
        Limits {
            max_handles: 16,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 2000,
        },
    )
    .unwrap();
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_trigger_admission(trigger_limits(4, 0))
        .unwrap();
    assert!(matches!(h.with_player_unit_events(1),Err(e) if e.failure==Failure::Source));
}

#[test]
fn owned_vm_retains_a_condition_wrapper_as_the_player_unit_filter() {
    let source = common().replace("type filterfunc extends boolexpr", "type filterfunc extends boolexpr\ntype conditionfunc extends boolexpr\nnative Condition takes code f returns conditionfunc");
    let map = "globals\ninteger hits=0\nconditionfunc Pred=null\nevent E=null\nendglobals\nfunction predicate takes nothing returns boolean\nset hits=hits+1\nreturn true\nendfunction\nfunction main takes nothing returns nothing\nset Pred=Condition(function predicate)\nset E=TriggerRegisterPlayerUnitEvent(CreateTrigger(),Player(4),PLAYER_SAMPLE,Pred)\nendfunction\n";
    let (ctx, h) = host(&source, map, 64);
    let h = h
        .with_conditions(crate::playercolor_host::ConditionBudget { max_records: 1 })
        .unwrap()
        .with_player_unit_events(1)
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let e = id(&vm.global("E"));
    let pred = id(&vm.global("Pred"));
    let h = &vm.host().inner;
    assert_eq!(
        h.player_unit_event_registration(e).unwrap().filter,
        Some(pred)
    );
    assert_eq!(h.condition_callback(pred).unwrap().name, "predicate");
}
