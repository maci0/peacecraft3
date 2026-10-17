use super::{EXTRAS, common_source, enumerated, trigger_limits};
use crate::playercolor_host::{EnumHost, Failure, Limits, ObjectHost, TimerEventRegistration};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;

const EMPTY_MAP: &str = "function unused takes nothing returns nothing\nendfunction\n";
const REGISTER: &str =
    "native TriggerRegisterTimerExpireEvent takes trigger owner, timer source returns event\n";
fn common() -> String {
    format!("{}{REGISTER}", common_source(EXTRAS))
}
fn host(common: &str, max_handles: usize) -> ObjectHost {
    enumerated(common, EMPTY_MAP, max_handles)
        .1
        .with_trigger_admission(trigger_limits(8, 0))
        .unwrap()
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}
fn register(h: &mut ObjectHost, t: &Value, timer: &Value) -> Result<Value, String> {
    h.native(
        "TriggerRegisterTimerExpireEvent",
        &[t.clone(), timer.clone()],
    )
}
fn pair(h: &mut ObjectHost) -> (Value, Value) {
    (
        h.native("CreateTrigger", &[]).unwrap(),
        h.native("CreateTimer", &[]).unwrap(),
    )
}

#[test]
fn owned_vm_timer_subscription_keeps_existing_identities_without_callback_or_expiry() {
    let common = format!(
        "{}native TimerStart takes timer t, real seconds, boolean periodic, code callback returns nothing\n",
        common()
    );
    let map = "globals\ntrigger G=null\ntimer T=null\nevent A=null\nevent B=null\ntriggeraction Action=null\ninteger hits=0\nendglobals\nfunction on_expiry takes nothing returns nothing\nset hits=hits+1\nendfunction\nfunction main takes nothing returns nothing\nset G=CreateTrigger()\nset T=CreateTimer()\nset Action=TriggerAddAction(G,function on_expiry)\nset A=TriggerRegisterTimerExpireEvent(G,T)\nset B=TriggerRegisterTimerExpireEvent(G,T)\ncall TimerStart(T,0.01,false,function on_expiry)\nendfunction\n";
    let (ctx, h) = enumerated(&common, map, 32);
    let h = h
        .with_trigger_admission(trigger_limits(2, 0))
        .unwrap()
        .with_timer_events(2)
        .unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    let failure = vm.call("main", &[]).unwrap_err().to_string();
    assert!(
        failure.contains("TimerStart") && failure.contains("unsupported"),
        "{failure}"
    );
    let (t, timer, a, b, action) = (
        id(&vm.global("G")),
        id(&vm.global("T")),
        id(&vm.global("A")),
        id(&vm.global("B")),
        id(&vm.global("Action")),
    );
    assert_ne!(a, b);
    assert_ne!(a, t);
    assert_ne!(a, timer);
    assert_eq!(vm.global("hits"), Value::Int(0));
    let h = &vm.host().inner;
    let record = TimerEventRegistration { trigger: t, timer };
    assert_eq!(h.timer_event_registration(a).unwrap(), record);
    assert_eq!(h.timer_event_registration(b).unwrap(), record);
    assert_eq!(h.retained_timer_events().unwrap(), 2);
    assert_eq!(h.retained_unit_events().unwrap(), 0);
    assert_eq!(h.inner().kind(a).unwrap(), "event");
    assert_eq!(h.inner().kind(timer).unwrap(), "timer");
    assert_eq!(h.trigger_action(action).unwrap().trigger, t);
    vm.call("on_expiry", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(1));
    vm.host()
        .native("DestroyTimer", &[Value::Handle(timer)])
        .unwrap();
    let h = &mut vm.host().inner;
    let count = h.inner().handles();
    assert!(h.inner().kind(timer).is_err());
    assert_eq!(
        h.timer_event_registration(a).unwrap(),
        record,
        "retained association is not a live-delivery claim"
    );
    assert!(
        register(h, &Value::Handle(t), &Value::Handle(timer))
            .unwrap_err()
            .contains("unknown handle")
    );
    assert_eq!(h.inner().handles(), count);
    assert_eq!(h.retained_timer_events().unwrap(), 2);
}

#[test]
fn timer_subscription_budget_and_bad_inputs_preserve_prior_arena_records() {
    let mut h = host(&common(), 16);
    let (t, timer) = pair(&mut h);
    let count = h.inner().handles();
    assert!(
        register(&mut h, &t, &timer)
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(h.inner().handles(), count);
    let mut h = h.with_timer_events(1).unwrap();
    let missing = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
    assert!(register(&mut h, &t, &missing).is_err());
    assert_eq!(h.inner().handles(), count);
    let event = register(&mut h, &t, &timer).unwrap();
    let record = h.timer_event_registration(id(&event)).unwrap();
    let count = h.inner().handles();
    for args in [
        vec![],
        vec![Value::Int(0), Value::Int(1)],
        vec![t.clone()],
        vec![Value::Null, timer.clone()],
        vec![timer.clone(), timer.clone()],
        vec![t.clone(), t.clone()],
        vec![t.clone(), event.clone()],
        vec![t.clone(), timer.clone(), Value::Null],
    ] {
        assert!(h.native("TriggerRegisterTimerExpireEvent", &args).is_err());
        assert_eq!(h.inner().handles(), count);
        assert_eq!(h.retained_timer_events().unwrap(), 1);
    }
    assert!(
        register(&mut h, &t, &timer)
            .unwrap_err()
            .contains("timer event registration policy exceeded")
    );
    assert_eq!(h.timer_event_registration(id(&event)).unwrap(), record);
    assert!(h.timer_event_registration(id(&timer)).is_err());
    assert!(h.with_timer_events(2).is_err());
    let mut zero = host(&common(), 16).with_timer_events(0).unwrap();
    let (t, timer) = pair(&mut zero);
    let before = zero.inner().handles();
    assert!(register(&mut zero, &t, &timer).is_err());
    assert_eq!(zero.inner().handles(), before);
    let mut full = host(&common(), 16).with_timer_events(2).unwrap();
    let (t, timer) = pair(&mut full);
    let event = register(&mut full, &t, &timer).unwrap();
    while full.inner().handles() < 16 {
        full.native("CreateTimer", &[]).unwrap();
    }
    assert!(
        register(&mut full, &t, &timer)
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(full.retained_timer_events().unwrap(), 1);
    assert_eq!(
        full.timer_event_registration(id(&event)).unwrap().timer,
        id(&timer)
    );
}

#[test]
fn reserved_family_names_do_not_become_timer_subscriptions_or_live_timers() {
    let c = common();
    let (ctx, _) = enumerated(&c, EMPTY_MAP, 16);
    let mut inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic:reserved",
        &["ConvertUnitEvent"],
        Limits {
            max_handles: 16,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap();
    let reserved_timer = inner.reserve_identity("timer").unwrap();
    let reserved_trigger = inner.reserve_identity("trigger").unwrap();
    let reserved_event = inner.reserve_identity("event").unwrap();
    let mut h = ObjectHost::checked(inner)
        .unwrap()
        .with_trigger_admission(trigger_limits(1, 0))
        .unwrap()
        .with_timer_events(1)
        .unwrap();
    let (t, timer) = pair(&mut h);
    let before = h.inner().handles();
    assert!(register(&mut h, &t, &Value::Handle(reserved_timer)).is_err());
    assert!(register(&mut h, &Value::Handle(reserved_trigger), &timer).is_err());
    assert!(h.timer_event_registration(reserved_event).is_err());
    assert_eq!(h.inner().handles(), before);
    assert_eq!(h.retained_timer_events().unwrap(), 0);
    h.native("DestroyTimer", std::slice::from_ref(&timer))
        .unwrap();
    assert!(register(&mut h, &t, &timer).is_err());
    assert_eq!(h.inner().handles(), before);
}

#[test]
fn timer_subscription_requires_loaded_contract_common_source_and_authored_ancestry() {
    let c = common();
    assert!(
        matches!(host(&c,16).with_timer_events(usize::MAX),Err(e) if e.failure==Failure::Capacity)
    );
    for c in [
        c.replace(REGISTER, ""),
        c.replace(REGISTER, &format!("constant {REGISTER}")),
        c.replace("timer source returns event", "integer source returns event"),
        c.replace("timer source returns event", "timer source returns timer"),
    ] {
        let h = host(&c, 8);
        assert!(matches!(h.with_timer_events(1),Err(e) if e.failure==Failure::Signature));
    }
    let missing = c.replace(REGISTER, "");
    let (_, h) = enumerated(&missing, REGISTER, 8);
    let h = h.with_trigger_admission(trigger_limits(1, 0)).unwrap();
    assert!(matches!(h.with_timer_events(1),Err(e) if e.failure==Failure::Source));
    let map = "function TriggerRegisterTimerExpireEvent takes trigger owner, timer source returns event\nreturn null\nendfunction\n";
    let (_, h) = enumerated(&c, map, 8);
    let h = h.with_trigger_admission(trigger_limits(1, 0)).unwrap();
    assert!(h.with_timer_events(1).is_err());
    let missing = c.replace("type timer extends agent", "");
    let (_, h) = enumerated(&missing, "type timer extends agent", 8);
    let h = h.with_trigger_admission(trigger_limits(1, 0)).unwrap();
    assert!(matches!(h.with_timer_events(1),Err(e) if e.failure==Failure::Source));
    let c = c.replace(
        "type timer extends agent",
        "type clockobject extends agent\ntype timer extends clockobject",
    );
    host(&c, 8).with_timer_events(1).unwrap();
    let (_, h) = enumerated(&common(), EMPTY_MAP, 8);
    assert!(
        h.with_timer_events(1).is_err(),
        "trigger creation must already be admitted"
    );
}
