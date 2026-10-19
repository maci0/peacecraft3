use super::*;

const SETTER: &str =
    "native SetUnitOwner takes unit u, player p, boolean changeColor returns nothing\n";
const GETTER: &str = "constant native GetOwningPlayer takes unit u returns player\n";
const MAP: &str = "globals\nunit A=null\nunit ALIAS=null\nunit B=null\nplayer OLD=null\nplayer NEW=null\nendglobals\nfunction setup takes nothing returns nothing\nset OLD=Player(4)\nset NEW=Player(5)\nset A=CreateUnit(OLD,'hfoo',1.5,-2.25,90.0)\nset ALIAS=A\nset B=CreateUnit(OLD,'hfoo',4.0,5.0,6.0)\nendfunction\nfunction transfer takes nothing returns nothing\ncall SetUnitOwner(ALIAS,NEW,false)\nendfunction\n";

fn open(extra: &str) -> VmHost {
    open_map(extra, MAP)
}

fn open_map(extra: &str, map: &str) -> VmHost {
    let common = script_common_with("widget", true, extra);
    let map = if extra.contains("native SetUnitOwner") {
        map
    } else {
        map.split("function transfer").next().unwrap()
    };
    let ctx = context(300, &common, map, "unit-catalog");
    let (catalog, hits, seen) = counting();
    let catalog: Arc<dyn ObjectCatalog + Send + Sync> = catalog;
    let host = object(&ctx, 32)
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_unit_catalog(Arc::clone(&catalog), source(), limits(2))
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    VmHost {
        vm,
        hits,
        seen,
        catalog,
    }
}

fn id(value: &Value) -> pcx_script::HandleId {
    let Value::Handle(id) = value else {
        panic!("handle")
    };
    *id
}

#[test]
fn transfer_changes_alias_queries_without_replacing_creation_or_other_units() {
    let mut s = open(&format!("{SETTER}{GETTER}"));
    let a = s.vm.global("A");
    let old = s.vm.global("OLD");
    let new = s.vm.global("NEW");
    let handles = s.vm.host().inner.inner().handles();
    let before = s.vm.host().inner.executed_unit(id(&a)).unwrap().clone();
    s.vm.call("transfer", &[]).unwrap();
    assert_eq!(s.vm.global("ALIAS"), a);
    assert_eq!(
        s.vm.host()
            .inner
            .native("GetOwningPlayer", std::slice::from_ref(&a))
            .unwrap(),
        new
    );
    let mut expected = before;
    expected.owner = id(&new);
    assert_eq!(s.vm.host().inner.executed_unit(id(&a)).unwrap(), &expected);
    assert_eq!(expected.executed_owner, 4);
    let b = s.vm.global("B");
    assert_eq!(
        s.vm.host().inner.native("GetOwningPlayer", &[b]).unwrap(),
        old
    );
    s.vm.call("transfer", &[]).unwrap();
    assert_eq!(s.vm.host().inner.executed_unit(id(&a)).unwrap(), &expected);
    assert_eq!(s.vm.host().inner.inner().handles(), handles);
    assert_eq!(s.hits.load(Ordering::SeqCst), 2);
}

#[test]
fn color_changes_require_known_player_color_and_false_and_same_owner_preserve_it() {
    let mut s = open(&format!("{SETTER}{GETTER}"));
    let a = s.vm.global("A");
    let old = s.vm.global("OLD");
    let new = s.vm.global("NEW");
    let before = s.vm.host().inner.executed_unit(id(&a)).unwrap().clone();
    assert!(
        s.vm.host()
            .inner
            .native("SetUnitOwner", &[a.clone(), new.clone(), Value::Bool(true)])
            .unwrap_err()
            .contains("color is unset")
    );
    assert_eq!(s.vm.host().inner.executed_unit(id(&a)).unwrap(), &before);
    let color = s.vm.global("C");
    s.vm.host()
        .inner
        .native("SetPlayerColor", &[new.clone(), color.clone()])
        .unwrap();
    s.vm.host()
        .inner
        .native("SetUnitOwner", &[a.clone(), new.clone(), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        s.vm.host().inner.executed_unit(id(&a)).unwrap().team_color,
        Some(id(&color))
    );
    s.vm.host()
        .inner
        .native("SetUnitOwner", &[a.clone(), old, Value::Bool(false)])
        .unwrap();
    assert_eq!(
        s.vm.host().inner.executed_unit(id(&a)).unwrap().team_color,
        Some(id(&color))
    );
    s.vm.host()
        .inner
        .native("SetUnitOwner", &[a.clone(), new, Value::Bool(false)])
        .unwrap();
    assert_eq!(
        s.vm.host().inner.executed_unit(id(&a)).unwrap().team_color,
        Some(id(&color))
    );
    // An unchanged owner returns before color lookup, even if its color is unknown.
    let b = s.vm.global("B");
    let old = s.vm.global("OLD");
    s.vm.host()
        .inner
        .native("SetUnitOwner", &[b.clone(), old, Value::Bool(true)])
        .unwrap();
    assert_eq!(
        s.vm.host().inner.executed_unit(id(&b)).unwrap().team_color,
        None
    );
}

#[test]
fn creation_does_not_guess_unit_color_from_an_owner_assignment() {
    let map = MAP.replace(
        "set A=CreateUnit",
        "call SetPlayerColor(OLD,C)\nset A=CreateUnit",
    );
    let mut s = open_map(&format!("{SETTER}{GETTER}"), &map);
    let a = s.vm.global("A");
    assert_eq!(
        s.vm.host().inner.executed_unit(id(&a)).unwrap().team_color,
        None
    );
    s.vm.call("transfer", &[]).unwrap();
    assert_eq!(
        s.vm.host().inner.executed_unit(id(&a)).unwrap().team_color,
        None
    );
}

#[test]
fn invalid_calls_are_atomic_and_null_handles_have_no_effect() {
    let mut s = open(&format!("{SETTER}{GETTER}"));
    let a = s.vm.global("A");
    let p = s.vm.global("NEW");
    let timer = s.vm.host().inner.native("CreateTimer", &[]).unwrap();
    let before = s.vm.host().inner.executed_unit(id(&a)).unwrap().clone();
    let unknown = Value::Handle(pcx_script::HandleId::new(
        std::num::NonZeroU64::new(9999).unwrap(),
    ));
    for args in [
        vec![],
        vec![a.clone(), p.clone()],
        vec![a.clone(), p.clone(), Value::Int(0)],
        vec![timer.clone(), p.clone(), Value::Bool(false)],
        vec![a.clone(), timer, Value::Bool(false)],
        vec![a.clone(), unknown.clone(), Value::Bool(false)],
        vec![unknown, p.clone(), Value::Bool(false)],
    ] {
        assert!(s.vm.host().inner.native("SetUnitOwner", &args).is_err());
        assert_eq!(s.vm.host().inner.executed_unit(id(&a)).unwrap(), &before);
    }
    for args in [
        vec![Value::Null, p, Value::Bool(true)],
        vec![a.clone(), Value::Null, Value::Bool(true)],
        vec![Value::Null, Value::Null, Value::Bool(false)],
    ] {
        assert_eq!(
            s.vm.host().inner.native("SetUnitOwner", &args).unwrap(),
            Value::Null
        );
    }
    assert_eq!(s.vm.host().inner.executed_unit(id(&a)).unwrap(), &before);
    assert_eq!(
        s.vm.host()
            .inner
            .native("GetOwningPlayer", &[Value::Null])
            .unwrap(),
        Value::Null
    );
}

#[test]
fn setter_admission_is_optional_and_checks_its_source_and_signature() {
    let mut s = open(GETTER);
    let a = s.vm.global("A");
    let p = s.vm.global("NEW");
    assert!(
        s.vm.host()
            .inner
            .native("SetUnitOwner", &[a, p, Value::Bool(false)])
            .unwrap_err()
            .contains("unsupported")
    );
    let mut setter_only = open(SETTER);
    setter_only.vm.call("transfer", &[]).unwrap();
    let unit = setter_only.vm.global("A");
    let player = setter_only.vm.global("NEW");
    assert_eq!(
        setter_only
            .vm
            .host()
            .inner
            .executed_unit(id(&unit))
            .unwrap()
            .owner,
        id(&player)
    );
    assert!(
        setter_only
            .vm
            .host()
            .inner
            .native("GetOwningPlayer", &[unit])
            .unwrap_err()
            .contains("unsupported")
    );
    for declaration in [
        SETTER.replace("boolean changeColor", "integer changeColor"),
        SETTER.replace("player p", "unit p"),
        SETTER.replace("returns nothing", "returns boolean"),
        format!("constant {SETTER}"),
    ] {
        let common = script_common_with("widget", true, &declaration);
        let ctx = context(
            300,
            &common,
            "function unused takes nothing returns nothing\nendfunction\n",
            "unit-catalog",
        );
        let (catalog, hits, _) = counting();
        let error = object(&ctx, 4)
            .with_unit_catalog(catalog, source(), limits(1))
            .err()
            .expect("malformed setter");
        assert_eq!(error.native, "SetUnitOwner");
        assert_eq!(error.failure, Failure::Signature);
        assert_eq!(hits.load(Ordering::SeqCst), 0);
    }
    let common = pcx_script::parse_script(&script_common_with("widget", true, "")).unwrap();
    let foreign = pcx_script::parse_script(SETTER).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                pcx_script::Prelude::new("synthetic/common.j", &common),
                pcx_script::Prelude::new("other.j", &foreign),
            ],
        )
        .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "foreign owner setter",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 4,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    let (catalog, _, _) = counting();
    let error = ObjectHost::checked(inner)
        .unwrap()
        .with_unit_catalog(catalog, source(), limits(1))
        .err()
        .expect("foreign setter");
    assert_eq!(error.native, "SetUnitOwner");
    assert_eq!(error.failure, Failure::Source);
    assert_eq!(error.declaration_source, "other.j");
}

#[test]
fn defend_checks_the_current_owners_research_after_transfer() {
    let tables = order_tables(
        ORDER_HEADERS,
        &["Qord", "Qcls", "0", "0", "1", "1", "0", "0", "0", "0"],
        "",
        "1",
    );
    let (mut vm, _) = order_vm(&tables, true);
    research(&mut vm, *b"Qtec", 1);
    let a = vm.global("A");
    let player = vm.host().inner.native("Player", &[Value::Int(5)]).unwrap();
    vm.host()
        .inner
        .native("SetUnitOwner", &[a, player.clone(), Value::Bool(false)])
        .unwrap();
    assert!(
        issue(&mut vm, "A", "engage")
            .unwrap_err()
            .contains("unsupported")
    );
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
    assert_eq!(issue(&mut vm, "A", "engage").unwrap(), Value::Bool(true));
}

#[test]
fn live_owner_listeners_refuse_matching_or_unqualified_events_but_not_unrelated_or_disabled() {
    for player_listener in [false, true] {
        for ownership_event in [None, Some(13), Some(21)] {
            let extra = format!(
                "{SETTER}{GETTER}type trigger extends agent\ntype event extends agent\ntype eventid extends handle\ntype unitevent extends eventid\ntype playerunitevent extends eventid\ntype boolexpr extends agent\nnative CreateTrigger takes nothing returns trigger\nnative TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event\nnative DisableTrigger takes trigger t returns nothing\nconstant native ConvertUnitEvent takes integer n returns unitevent\nconstant native ConvertPlayerUnitEvent takes integer n returns playerunitevent\nnative TriggerRegisterPlayerUnitEvent takes trigger t, player p, playerunitevent e, boolexpr f returns event\n"
            );
            let mut common = script_common_with("widget", true, &extra).replace(
            "\nendglobals",
            "\nconstant unitevent SAMPLE=ConvertUnitEvent(13)\nconstant playerunitevent PLAYER_SAMPLE=ConvertPlayerUnitEvent(13)\nendglobals",
        );
            if let Some(value) = ownership_event {
                common = common.replace("\nendglobals", &format!("\nconstant unitevent EVENT_UNIT_CHANGE_OWNER=ConvertUnitEvent({value})\nconstant playerunitevent EVENT_PLAYER_UNIT_CHANGE_OWNER=ConvertPlayerUnitEvent({value})\nendglobals"));
            }
            let ctx = context(300, &common, MAP, "unit-catalog");
            let inner = EnumHost::checked_selected(
                ctx.scripts.shared_program(),
                "owner event guard",
                &[
                    "ConvertPlacement",
                    "ConvertPlayerColor",
                    "ConvertRacePref",
                    "ConvertMapControl",
                    "ConvertUnitEvent",
                    "ConvertPlayerUnitEvent",
                ],
                Limits {
                    max_handles: 32,
                    admission_steps: 2000,
                    expression_depth: 16,
                    expression_work: 2000,
                },
            )
            .unwrap();
            let (catalog, _, _) = counting();
            let host = ObjectHost::checked(inner)
                .unwrap()
                .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
                .unwrap()
                .with_unit_catalog(catalog, source(), limits(2))
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
            let unit = vm.global("A");
            let old = vm.global("OLD");
            let player = vm.global("NEW");
            let event = vm.global("SAMPLE");
            let trigger = vm.host().inner.native("CreateTrigger", &[]).unwrap();
            if player_listener {
                let event = vm.global("PLAYER_SAMPLE");
                vm.host()
                    .inner
                    .native(
                        "TriggerRegisterPlayerUnitEvent",
                        &[trigger.clone(), player.clone(), event, Value::Null],
                    )
                    .unwrap();
            } else {
                vm.host()
                    .inner
                    .native(
                        "TriggerRegisterUnitEvent",
                        &[trigger.clone(), unit.clone(), event],
                    )
                    .unwrap();
            }
            if ownership_event == Some(21) {
                vm.host()
                    .inner
                    .native(
                        "SetUnitOwner",
                        &[unit.clone(), player.clone(), Value::Bool(false)],
                    )
                    .unwrap();
                assert_eq!(
                    vm.host().inner.native("GetOwningPlayer", &[unit]).unwrap(),
                    player
                );
                continue;
            }
            assert!(
                vm.host()
                    .inner
                    .native(
                        "SetUnitOwner",
                        &[unit.clone(), player.clone(), Value::Bool(false)]
                    )
                    .unwrap_err()
                    .contains("requires event delivery")
            );
            assert_eq!(
                vm.host()
                    .inner
                    .native("GetOwningPlayer", std::slice::from_ref(&unit))
                    .unwrap(),
                old
            );
            vm.host()
                .inner
                .native("DisableTrigger", &[trigger])
                .unwrap();
            vm.host()
                .inner
                .native(
                    "SetUnitOwner",
                    &[unit.clone(), player.clone(), Value::Bool(false)],
                )
                .unwrap();
            assert_eq!(
                vm.host().inner.native("GetOwningPlayer", &[unit]).unwrap(),
                player
            );
        }
    }
}
