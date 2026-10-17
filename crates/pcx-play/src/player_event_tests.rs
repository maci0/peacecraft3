use super::{EXTRAS, budget, common_source, context, slots_rule, trigger_limits};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Failure, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;

const REGISTER: &str = "TriggerRegisterPlayerEvent";
const EMPTY_MAP: &str = "function unused takes nothing returns nothing\nendfunction\n";
const DECLARATION: &str =
    "native TriggerRegisterPlayerEvent takes trigger t, player p, playerevent e returns event\n";
const EXTRA: &str = "\
type playerevent extends eventid
constant native ConvertPlayerEvent takes integer n returns playerevent
";

fn common() -> String {
    common_source(&format!("{EXTRAS}{EXTRA}{DECLARATION}")).replacen(
        "\nendglobals\n",
        "\nconstant playerevent PLAYER_SAMPLE = ConvertPlayerEvent(61)\nconstant playerevent OTHER_SAMPLE = ConvertPlayerEvent(69)\nendglobals\n",
        1,
    )
}

fn enums(common: &str, map: &str, max: usize) -> (Arc<ParsedMapContext>, EnumHost) {
    let ctx = context(300, common, map, "player-event");
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic player event",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
            "ConvertPlayerEvent",
        ],
        Limits {
            max_handles: max,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 2000,
        },
    )
    .unwrap();
    (ctx, inner)
}

fn objects(ctx: &Arc<ParsedMapContext>, inner: EnumHost) -> ObjectHost {
    ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(ctx), slots_rule(301, -3, 9), budget())
        .unwrap()
        .with_trigger_admission(trigger_limits(4, 0))
        .unwrap()
}

fn host(common: &str, map: &str, max: usize) -> (Arc<ParsedMapContext>, ObjectHost) {
    let (ctx, inner) = enums(common, map, max);
    let h = objects(&ctx, inner);
    (ctx, h)
}

fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}

fn inputs(h: &mut ObjectHost) -> [Value; 3] {
    [
        h.native("CreateTrigger", &[]).unwrap(),
        h.native("Player", &[Value::Int(4)]).unwrap(),
        h.native("ConvertPlayerEvent", &[Value::Int(61)]).unwrap(),
    ]
}

#[test]
fn owned_vm_retains_exact_player_events_without_enabling_or_running_actions() {
    let map = "\
globals
trigger T=null
player P=null
event A=null
event B=null
event C=null
integer hits=0
endglobals
function action takes nothing returns nothing
set hits=hits+1
endfunction
function main takes nothing returns nothing
set T=CreateTrigger()
set P=Player(4)
call DisableTrigger(T)
call TriggerAddAction(T,function action)
set A=TriggerRegisterPlayerEvent(T,P,PLAYER_SAMPLE)
set B=TriggerRegisterPlayerEvent(T,P,PLAYER_SAMPLE)
set C=TriggerRegisterPlayerEvent(T,Player(5),OTHER_SAMPLE)
endfunction
";
    let (ctx, h) = host(&common(), map, 64);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h.with_player_events(3).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let (t, p, a, b, c) = (
        id(&vm.global("T")),
        id(&vm.global("P")),
        id(&vm.global("A")),
        id(&vm.global("B")),
        id(&vm.global("C")),
    );
    assert_ne!(a, b);
    assert_ne!(b, c);
    let sample = id(&vm.global("PLAYER_SAMPLE"));
    let other = id(&vm.global("OTHER_SAMPLE"));
    let h = &mut vm.host().inner;
    let first = h.player_event_registration(a).unwrap();
    assert_eq!(
        (
            first.trigger,
            first.player,
            first.playerevent,
            first.enumerand
        ),
        (t, p, sample, 61)
    );
    assert_eq!(h.player_event_registration(b).unwrap(), first);
    let third = h.player_event_registration(c).unwrap();
    assert_eq!(
        (third.trigger, third.playerevent, third.enumerand),
        (t, other, 69)
    );
    assert_eq!(h.player_settings(third.player).unwrap().number, 5);
    assert_eq!(h.trigger_enabled(t).unwrap(), Some(false));
    assert_eq!(h.retained_player_events().unwrap(), 3);
    assert_eq!(h.retained_unit_events().unwrap(), 0);
    assert!(h.retained_player_unit_events().is_err());
    assert_eq!(h.inner().kind(a).unwrap(), "event");
    h.native("EnableTrigger", &[Value::Handle(t)]).unwrap();
    assert_eq!(h.player_event_registration(a).unwrap(), first);
    assert_eq!(vm.global("hits"), Value::Int(0));
}

#[test]
fn invalid_inputs_and_reserved_objects_preserve_prior_associations() {
    let (ctx, mut inner) = enums(&common(), EMPTY_MAP, 64);
    let reserved_trigger = inner.reserve_identity("trigger").unwrap();
    let reserved_player = inner.reserve_identity("player").unwrap();
    let mut h = objects(&ctx, inner).with_player_events(4).unwrap();
    let args = inputs(&mut h);
    let first = h.native(REGISTER, &args).unwrap();
    let before = h.inner().handles();
    let record = h.player_event_registration(id(&first)).unwrap();
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(9999).unwrap()));
    for bad in [
        vec![],
        vec![Value::Null; 3],
        vec![args[1].clone(), args[0].clone(), args[2].clone()],
        vec![
            Value::Handle(reserved_trigger),
            args[1].clone(),
            args[2].clone(),
        ],
        vec![
            args[0].clone(),
            Value::Handle(reserved_player),
            args[2].clone(),
        ],
        vec![args[0].clone(), args[1].clone(), args[0].clone()],
        vec![unknown, args[1].clone(), args[2].clone()],
        vec![
            args[0].clone(),
            args[1].clone(),
            args[2].clone(),
            Value::Null,
        ],
    ] {
        assert!(h.native(REGISTER, &bad).is_err());
        assert_eq!(h.inner().handles(), before);
        assert_eq!(h.retained_player_events().unwrap(), 1);
        assert_eq!(h.player_event_registration(id(&first)).unwrap(), record);
    }
    assert!(h.native("ConvertPlayerEvent", &[Value::Int(62)]).is_err());
    assert_eq!(h.inner().handles(), before);
    assert!(h.player_event_registration(id(&args[0])).is_err());
    assert_eq!(h.trigger_enabled(id(&args[0])).unwrap(), None);
}

#[test]
fn separate_record_and_arena_limits_refuse_before_allocation() {
    for (limit, fill) in [(0, false), (1, false), (2, true)] {
        let (_, h) = host(&common(), EMPTY_MAP, 64);
        let mut h = h.with_player_events(limit).unwrap();
        let args = inputs(&mut h);
        let first = if limit > 0 {
            Some(h.native(REGISTER, &args).unwrap())
        } else {
            None
        };
        if fill {
            while h.inner().handles() < 64 {
                h.native("CreateTimer", &[]).unwrap();
            }
        }
        let before = h.inner().handles();
        assert!(h.native(REGISTER, &args).is_err());
        assert_eq!(h.inner().handles(), before);
        assert_eq!(
            h.retained_player_events().unwrap(),
            usize::from(first.is_some())
        );
        if let Some(first) = first {
            assert_eq!(
                h.player_event_registration(id(&first)).unwrap().player,
                id(&args[1])
            );
        }
    }
}

#[test]
fn default_duplicate_capacity_and_contract_refusals_are_explicit() {
    let (_, mut h) = host(&common(), EMPTY_MAP, 64);
    let args = inputs(&mut h);
    assert!(
        h.native(REGISTER, &args)
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(h.retained_player_events().is_err());
    assert!(
        h.with_player_events(1)
            .unwrap()
            .with_player_events(1)
            .is_err()
    );
    let (_, h) = host(&common(), EMPTY_MAP, 64);
    assert!(matches!(h.with_player_events(usize::MAX),Err(e) if e.failure==Failure::Capacity));
    for src in [
        common().replace(DECLARATION, &format!("constant {DECLARATION}")),
        common().replace(
            "player p, playerevent e returns event",
            "integer p, playerevent e returns event",
        ),
        common().replace(
            "playerevent e returns event",
            "playerevent e returns trigger",
        ),
        common().replace(
            "type playerevent extends eventid",
            "type playerevent extends handle",
        ),
    ] {
        let (_, h) = host(&src, EMPTY_MAP, 64);
        assert!(h.with_player_events(1).is_err());
    }
}

#[test]
fn foreign_declaration_and_missing_selected_domain_refuse_admission() {
    let (_, h) = host(
        &common().replace(DECLARATION, ""),
        &format!("{DECLARATION}{EMPTY_MAP}"),
        64,
    );
    assert!(matches!(h.with_player_events(1),Err(e) if e.failure==Failure::Source));
    let (ctx, _) = host(&common(), EMPTY_MAP, 64);
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "missing selected player-event domain",
        &["ConvertUnitEvent"],
        Limits {
            max_handles: 64,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 2000,
        },
    )
    .unwrap();
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_trigger_admission(trigger_limits(1, 0))
        .unwrap();
    assert!(matches!(h.with_player_events(1),Err(e) if e.failure==Failure::Signature));
}
