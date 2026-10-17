use super::{EXTRAS, common_source, context, trigger_limits};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Error, Failure, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
const REGISTER: &str = "native TriggerRegisterGameStateEvent takes trigger owner, gamestate state, limitop op, real limit returns event\n";
const TYPES: &str = "type gamestate extends handle
type igamestate extends gamestate
type fgamestate extends gamestate
type limitop extends eventid
constant native ConvertIGameState takes integer i returns igamestate
constant native ConvertFGameState takes integer i returns fgamestate
constant native ConvertLimitOp takes integer i returns limitop
constant native ConvertOtherFloat takes integer i returns fgamestate
constant native ConvertOtherOp takes integer i returns limitop
native TriggerRegisterTimerExpireEvent takes trigger owner, timer source returns event
globals
constant igamestate IntState=ConvertIGameState(43)
constant fgamestate FloatState=ConvertFGameState(-37)
constant limitop Op=ConvertLimitOp(-7)
constant fgamestate OtherFloat=ConvertOtherFloat(103)
constant limitop OtherOp=ConvertOtherOp(107)
endglobals
";
const EMPTY: &str = "// synthetic map\n";
fn common() -> String {
    format!("{}{TYPES}{REGISTER}", common_source(EXTRAS))
}
fn raw(
    c: &str,
    map: &str,
    omit: Option<&str>,
) -> Result<(ObjectHost, Arc<ParsedMapContext>), Error> {
    let ctx = context(300, c, map, "synthetic:state-events");
    let names = [
        "ConvertPlacement",
        "ConvertPlayerColor",
        "ConvertRacePref",
        "ConvertMapControl",
        "ConvertUnitEvent",
        "ConvertIGameState",
        "ConvertFGameState",
        "ConvertLimitOp",
        "ConvertOtherFloat",
        "ConvertOtherOp",
    ]
    .into_iter()
    .filter(|n| Some(*n) != omit)
    .collect::<Vec<_>>();
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:state-events",
        &names,
        Limits {
            max_handles: 64,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 2000,
        },
    )?;
    let h = ObjectHost::checked(inner)?.with_trigger_admission(trigger_limits(8, 0))?;
    Ok((h, ctx))
}
fn host() -> ObjectHost {
    raw(&common(), EMPTY, None).unwrap().0
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}
fn convert(h: &mut ObjectHost, name: &str, n: i32) -> Value {
    h.native(name, &[Value::Int(n)]).unwrap()
}
fn register(
    h: &mut ObjectHost,
    t: &Value,
    state: &Value,
    op: &Value,
    limit: Value,
) -> Result<Value, String> {
    h.native(
        "TriggerRegisterGameStateEvent",
        &[t.clone(), state.clone(), op.clone(), limit],
    )
}

#[test]
fn owned_vm_game_state_subscriptions_preserve_typed_handles_and_threshold_bits_without_firing() {
    let c = format!("{}native Next takes nothing returns nothing\n", common());
    let map = "globals\ntrigger T=null\nevent F=null\nevent I=null\nevent Z=null\ntriggeraction A=null\ninteger hits=0\nendglobals\nfunction changed takes nothing returns nothing\nset hits=hits+1\nendfunction\nfunction main takes nothing returns nothing\nset T=CreateTrigger()\nset F=TriggerRegisterGameStateEvent(T,FloatState,Op,-3.25)\nset I=TriggerRegisterGameStateEvent(T,IntState,Op,123)\nset Z=TriggerRegisterGameStateEvent(T,FloatState,Op,-0.0)\nset A=TriggerAddAction(T,function changed)\ncall Next()\nendfunction\n";
    let (h, ctx) = raw(&c, map, None).unwrap();
    let h = h
        .with_game_state_events(3)
        .unwrap()
        .with_timer_events(1)
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    let before = vm.host().inner.inner().handles();
    let fail = vm.call("main", &[]).unwrap_err().to_string();
    assert!(
        fail.contains("Next") && fail.contains("unsupported"),
        "{fail}"
    );
    assert_eq!(vm.global("hits"), Value::Int(0));
    let (t, f, i, z, a, fs, is, op) = (
        id(&vm.global("T")),
        id(&vm.global("F")),
        id(&vm.global("I")),
        id(&vm.global("Z")),
        id(&vm.global("A")),
        id(&vm.global("FloatState")),
        id(&vm.global("IntState")),
        id(&vm.global("Op")),
    );
    assert_ne!(f, i);
    assert_ne!(f, z);
    let h = &vm.host().inner;
    for (event, state, kind, value, limit) in [
        (f, fs, "fgamestate", -37, -3.25f32),
        (i, is, "igamestate", 43, 123.0f32),
        (z, fs, "fgamestate", -37, -0.0f32),
    ] {
        let r = h.game_state_event_registration(event).unwrap();
        assert_eq!(
            (
                r.trigger,
                r.state,
                r.state_kind,
                r.state_value,
                r.limitop,
                r.limitop_value
            ),
            (t, state, kind, value, op, -7)
        );
        assert_eq!(r.limit.to_bits(), limit.to_bits());
        assert_eq!(h.inner().kind(event).unwrap(), "event");
    }
    assert_eq!(h.retained_game_state_events().unwrap(), 3);
    assert_eq!(h.retained_timer_events().unwrap(), 0);
    assert_eq!(h.retained_unit_events().unwrap(), 0);
    assert_eq!(h.inner().handles(), before + 5);
    assert_eq!(h.trigger_action(a).unwrap().trigger, t);
    vm.call("changed", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(1));
}

#[test]
fn game_state_registration_refusals_and_capacity_preserve_prior_records() {
    let mut h = host();
    let t = h.native("CreateTrigger", &[]).unwrap();
    let fs = convert(&mut h, "ConvertFGameState", -37);
    let is = convert(&mut h, "ConvertIGameState", 43);
    let op = convert(&mut h, "ConvertLimitOp", -7);
    assert!(
        register(&mut h, &t, &fs, &op, Value::Real(0.0))
            .unwrap_err()
            .contains("unsupported")
    );
    let mut h = h.with_game_state_events(4).unwrap();
    let f = register(&mut h, &t, &fs, &op, Value::Real(-0.0)).unwrap();
    let repeated = register(&mut h, &t, &fs, &op, Value::Real(-0.0)).unwrap();
    assert_ne!(f, repeated);
    let i = register(&mut h, &t, &is, &op, Value::Int(i32::MAX)).unwrap();
    assert_eq!(
        h.game_state_event_registration(id(&i))
            .unwrap()
            .limit
            .to_bits(),
        (i32::MAX as f32).to_bits()
    );
    let badstate = convert(&mut h, "ConvertOtherFloat", 103);
    let badop = convert(&mut h, "ConvertOtherOp", 107);
    let timer = h.native("CreateTimer", &[]).unwrap();
    let missing = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
    let count = h.inner().handles();
    for args in [
        vec![],
        vec![Value::Null, fs.clone(), op.clone(), Value::Real(0.0)],
        vec![timer, fs.clone(), op.clone(), Value::Real(0.0)],
        vec![t.clone(), badstate, op.clone(), Value::Real(0.0)],
        vec![t.clone(), fs.clone(), badop, Value::Real(0.0)],
        vec![t.clone(), missing, op.clone(), Value::Real(0.0)],
        vec![t.clone(), fs.clone(), op.clone(), Value::Real(f32::NAN)],
        vec![
            t.clone(),
            fs.clone(),
            op.clone(),
            Value::Real(f32::INFINITY),
        ],
        vec![t.clone(), fs.clone(), op.clone(), Value::Bool(false)],
    ] {
        assert!(h.native("TriggerRegisterGameStateEvent", &args).is_err());
        assert_eq!(h.inner().handles(), count);
        assert_eq!(h.retained_game_state_events().unwrap(), 3);
        assert_eq!(
            h.game_state_event_registration(id(&f))
                .unwrap()
                .limit
                .to_bits(),
            (-0.0f32).to_bits()
        );
    }
    register(&mut h, &t, &fs, &op, Value::Real(0.0)).unwrap();
    assert_eq!(h.retained_game_state_events().unwrap(), 4);
    assert!(
        register(&mut h, &t, &fs, &op, Value::Real(0.0))
            .unwrap_err()
            .contains("game state event registration policy exceeded")
    );
    assert!(h.game_state_event_registration(id(&fs)).is_err());
    assert!(h.with_game_state_events(4).is_err());
    let mut zero = host().with_game_state_events(0).unwrap();
    let t = zero.native("CreateTrigger", &[]).unwrap();
    let fs = convert(&mut zero, "ConvertFGameState", -37);
    let op = convert(&mut zero, "ConvertLimitOp", -7);
    let count = zero.inner().handles();
    assert!(register(&mut zero, &t, &fs, &op, Value::Real(0.0)).is_err());
    assert_eq!(zero.inner().handles(), count);
    let mut full = host().with_game_state_events(2).unwrap();
    let t = full.native("CreateTrigger", &[]).unwrap();
    let fs = convert(&mut full, "ConvertFGameState", -37);
    let op = convert(&mut full, "ConvertLimitOp", -7);
    let event = register(&mut full, &t, &fs, &op, Value::Real(-8.5)).unwrap();
    while full.inner().handles() < 64 {
        full.native("CreateTimer", &[]).unwrap();
    }
    assert!(
        register(&mut full, &t, &fs, &op, Value::Real(0.0))
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(full.retained_game_state_events().unwrap(), 1);
    assert_eq!(
        full.game_state_event_registration(id(&event))
            .unwrap()
            .limit,
        -8.5
    );
}

#[test]
fn game_state_registration_requires_selected_domains_contracts_and_source_ancestry() {
    assert!(
        matches!(host().with_game_state_events(usize::MAX),Err(e) if e.failure==Failure::Capacity)
    );
    let c = common();
    for omit in ["ConvertIGameState", "ConvertFGameState", "ConvertLimitOp"] {
        let (h, _) = raw(&c, EMPTY, Some(omit)).unwrap();
        assert!(h.with_game_state_events(1).is_err());
    }
    for c in [
        c.replace(REGISTER, ""),
        c.replace(REGISTER, &format!("constant {REGISTER}")),
        c.replace("real limit returns event", "integer limit returns event"),
        c.replace("real limit returns event", "real limit returns timer"),
        c.replace("igamestate extends gamestate", "igamestate extends handle"),
    ] {
        if let Ok((h, _)) = raw(&c, EMPTY, None) {
            assert!(h.with_game_state_events(1).is_err());
        }
    }
    for (c,map) in [(c.replace(REGISTER,""),REGISTER.to_string()),(c.clone(),REGISTER.to_string()),(c.clone(),"function TriggerRegisterGameStateEvent takes trigger owner, gamestate state, limitop op, real limit returns event\nreturn null\nendfunction\n".into()),(c.replace("type gamestate extends handle",""),"type gamestate extends handle\n".into())] {
        if let Ok((h,_))=raw(&c,&map,None) {assert!(h.with_game_state_events(1).is_err());}
    }
    let c = c.replace(
        "type gamestate extends handle",
        "type basegame extends handle\ntype gamestate extends basegame",
    );
    raw(&c, EMPTY, None)
        .unwrap()
        .0
        .with_game_state_events(1)
        .unwrap();
}
