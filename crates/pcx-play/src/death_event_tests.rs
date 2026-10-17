use super::{
    Counting, EXTRAS, budget, common_source, enum_input, enumerated, slots_rule, source,
    trigger_limits, unit_limits,
};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{DeathEventRegistration, DestructableBudget, Failure, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::{Arc, atomic::AtomicUsize};

const REGISTER: &str = "TriggerRegisterDeathEvent";
const DECLARATION: &str =
    "native TriggerRegisterDeathEvent takes trigger t, widget w returns event\n";
const EXTRA: &str = "\
type destructable extends widget
native CreateDestructable takes integer id, real x, real y, real f, real s, integer v returns destructable
native CreateDeadDestructableZ takes integer id, real x, real y, real z, real f, real s, integer v returns destructable
";
const EMPTY: &str = "function empty takes nothing returns nothing\nendfunction\n";
const MAP: &str = "\
globals
trigger T=null
unit U=null
destructable D=null
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
call DisableTrigger(T)
call TriggerAddAction(T,function action)
set U=CreateUnit(Player(4),'hfoo',1.0,2.0,3.0)
set D=CreateDeadDestructableZ('Zxyz',4.0,5.0,6.0,7.0,1.0,2)
set A=TriggerRegisterDeathEvent(T,U)
set B=TriggerRegisterDeathEvent(T,D)
set C=TriggerRegisterDeathEvent(T,D)
endfunction
";
fn common() -> String {
    common_source(&format!("{EXTRAS}{EXTRA}{DECLARATION}"))
}
fn objects(h: ObjectHost) -> ObjectHost {
    h.with_unit_catalog(
        Arc::new(Counting {
            hits: Arc::new(AtomicUsize::new(0)),
        }),
        source(),
        unit_limits(),
    )
    .unwrap()
    .with_destructables(DestructableBudget { max_records: 4 })
    .unwrap()
    .with_trigger_admission(trigger_limits(4, 0))
    .unwrap()
}
fn host(common: &str, map: &str) -> (Arc<ParsedMapContext>, ObjectHost) {
    let (ctx, h) = enumerated(common, map, 64);
    (ctx, objects(h))
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}
fn inputs(h: &mut ObjectHost) -> [Value; 2] {
    [
        h.native("CreateTrigger", &[]).unwrap(),
        h.native(
            "CreateDestructable",
            &[
                Value::Int(i32::from_be_bytes(*b"Zxyz")),
                Value::Real(1.0),
                Value::Real(2.0),
                Value::Real(3.0),
                Value::Real(1.0),
                Value::Int(0),
            ],
        )
        .unwrap(),
    ]
}
#[test]
fn owned_vm_retains_units_and_destructables_without_running_actions_or_changing_state() {
    let (ctx, h) = host(&common(), MAP);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h.with_death_events(3).unwrap());
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let (t, u, d, a, b, c) = (
        id(&vm.global("T")),
        id(&vm.global("U")),
        id(&vm.global("D")),
        id(&vm.global("A")),
        id(&vm.global("B")),
        id(&vm.global("C")),
    );
    assert_ne!(a, b);
    assert_ne!(b, c);
    let h = &vm.host().inner;
    for (event, widget) in [(a, u), (b, d), (c, d)] {
        assert_eq!(
            h.death_event_registration(event).unwrap(),
            DeathEventRegistration { trigger: t, widget }
        );
        assert_eq!(h.inner().kind(event).unwrap(), "event");
        assert_ne!(event, t);
        assert_ne!(event, widget);
    }
    assert!(h.destructable_creation(d).unwrap().dead);
    assert_eq!(h.retained_death_events().unwrap(), 3);
    assert_eq!(h.retained_unit_events().unwrap(), 0);
    assert_eq!(h.trigger_enabled(t).unwrap(), Some(false));
}
#[test]
fn bad_arguments_and_reserved_families_preserve_existing_subscriptions() {
    let (ctx, mut inner) = enum_input(&common(), EMPTY, 64);
    let reserved = ["widget", "unit", "destructable", "trigger"]
        .map(|kind| inner.reserve_identity(kind).unwrap());
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(301, -3, 9), budget())
        .unwrap();
    let mut h = objects(h).with_death_events(4).unwrap();
    let args = inputs(&mut h);
    let event = h.native(REGISTER, &args).unwrap();
    let record = h.death_event_registration(id(&event)).unwrap();
    let before = h.inner().handles();
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(9999).unwrap()));
    let mut invalid = vec![
        vec![],
        vec![Value::Null, args[1].clone()],
        vec![args[0].clone(), Value::Null],
        vec![args[1].clone(), args[0].clone()],
        vec![unknown.clone(), args[1].clone()],
        vec![args[0].clone(), unknown],
        vec![args[0].clone(), args[1].clone(), Value::Null],
        vec![args[0].clone(), event.clone()],
    ];
    for fake in reserved {
        invalid.push(vec![args[0].clone(), Value::Handle(fake)]);
        invalid.push(vec![Value::Handle(fake), args[1].clone()]);
    }
    for bad in invalid {
        assert!(h.native(REGISTER, &bad).is_err());
        assert_eq!(h.inner().handles(), before);
        assert_eq!(h.retained_death_events().unwrap(), 1);
        assert_eq!(h.death_event_registration(id(&event)).unwrap(), record);
    }
    assert!(h.death_event_registration(id(&args[0])).is_err());
    assert_eq!(h.trigger_enabled(id(&args[0])).unwrap(), None);
}
#[test]
fn separate_record_limit_and_shared_arena_exhaustion_do_not_partially_register() {
    for (limit, fill) in [(0, false), (1, false), (2, true)] {
        let (_, h) = host(&common(), EMPTY);
        let mut h = h.with_death_events(limit).unwrap();
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
            h.retained_death_events().unwrap(),
            usize::from(first.is_some())
        );
        if let Some(first) = first {
            assert_eq!(
                h.death_event_registration(id(&first)).unwrap().widget,
                id(&args[1])
            );
        }
    }
}
#[test]
fn opt_in_contract_source_ancestry_and_platform_limits_are_checked() {
    let (_, mut h) = host(&common(), EMPTY);
    let args = inputs(&mut h);
    assert!(
        h.native(REGISTER, &args)
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(h.retained_death_events().is_err());
    assert!(
        h.with_death_events(1)
            .unwrap()
            .with_death_events(1)
            .is_err()
    );
    let (_, h) = host(&common(), EMPTY);
    assert!(matches!(h.with_death_events(usize::MAX),Err(e) if e.failure==Failure::Capacity));
    for src in [
        common().replace(DECLARATION, &format!("constant {DECLARATION}")),
        common().replace("widget w returns event", "unit w returns event"),
        common().replace("widget w returns event", "widget w returns trigger"),
        common().replace("type widget extends agent", "type widget extends handle"),
        common().replace(DECLARATION, ""),
    ] {
        let (_, h) = enumerated(&src, EMPTY, 64);
        let h = h.with_trigger_admission(trigger_limits(1, 0)).unwrap();
        assert!(h.with_death_events(1).is_err());
    }
    let (_, h) = host(
        &common().replace(DECLARATION, ""),
        &format!("{DECLARATION}{EMPTY}"),
    );
    assert!(matches!(h.with_death_events(1),Err(e) if e.failure==Failure::Source));
    let declaration = "type destructable extends widget\n";
    let (_, h) = host(
        &common().replace(declaration, ""),
        &format!("{declaration}{EMPTY}"),
    );
    assert!(matches!(h.with_death_events(1),Err(e) if e.failure==Failure::Source));
    let (_, inner) = enum_input(&common(), EMPTY, 64);
    let h = ObjectHost::checked(inner).unwrap();
    assert!(h.with_death_events(1).is_err());
}
