use super::*;
use crate::playercolor_host::{ConditionBudget, EnumHost, FilterBudget, Limits, TriggerBudget};
use pcx_script::{Host, Prelude};
use std::sync::Arc;

const COMMON: &str = "\
type agent extends handle
type timer extends agent
type rect extends agent
type region extends agent
type widget extends agent
type unit extends widget
type trigger extends agent
type triggeraction extends handle
type event extends agent
type eventid extends handle
type unitevent extends eventid
type boolexpr extends agent
type filterfunc extends boolexpr
type conditionfunc extends boolexpr
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native Rect takes real a, real b, real c, real d returns rect
native RemoveRect takes rect r returns nothing
native SetRect takes rect r, real a, real b, real c, real d returns nothing
native MoveRectTo takes rect r, real x, real y returns nothing
native GetRectCenterX takes rect r returns real
native GetRectCenterY takes rect r returns real
native GetRectMinX takes rect r returns real
native GetRectMinY takes rect r returns real
native GetRectMaxX takes rect r returns real
native GetRectMaxY takes rect r returns real
native CreateTrigger takes nothing returns trigger
native DisableTrigger takes trigger t returns nothing
native TriggerAddAction takes trigger t, code f returns triggeraction
native TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event
constant native ConvertUnitEvent takes integer i returns unitevent
native Filter takes code f returns filterfunc
native Condition takes code f returns conditionfunc
native CreateRegion takes nothing returns region
native RegionAddRect takes region g, rect r returns nothing
native TriggerRegisterEnterRegion takes trigger t, region g, boolexpr f returns event
native TriggerRegisterLeaveRegion takes trigger t, region g, boolexpr f returns event
native RemoveRegion takes region g returns nothing
native RegionClearRect takes region g, rect r returns nothing
native IsPointInRegion takes region g, real x, real y returns boolean
globals
constant unitevent SAMPLE=ConvertUnitEvent(9)
endglobals
";
const MAP: &str = "\
globals
integer hits=0
region G=null
rect R=null
trigger T=null
event A=null
event B=null
event C=null
filterfunc F=null
conditionfunc P=null
endglobals
function predicate takes nothing returns boolean
set hits=hits+1
return true
endfunction
function action takes nothing returns nothing
set hits=hits+100
endfunction
function main takes nothing returns nothing
set G=CreateRegion()
set R=Rect(-4,2,8,10)
call RegionAddRect(G,R)
call MoveRectTo(R,20,30)
call RegionAddRect(G,R)
set T=CreateTrigger()
call DisableTrigger(T)
call TriggerAddAction(T,function action)
set F=Filter(function predicate)
set P=Condition(function predicate)
set A=TriggerRegisterEnterRegion(T,G,null)
set B=TriggerRegisterEnterRegion(T,G,F)
set C=TriggerRegisterLeaveRegion(T,G,P)
endfunction
";

fn enums(common: &str, map: &str, capacity: usize) -> EnumHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let p = pcx_script::resolve_with(&map, &[Prelude::new("synthetic:region-contract", &common)])
        .unwrap();
    EnumHost::checked_selected(
        Arc::new(p),
        "synthetic:region-map",
        &["ConvertUnitEvent"],
        Limits {
            max_handles: capacity,
            admission_steps: 1000,
            expression_depth: 16,
            expression_work: 1000,
        },
    )
    .unwrap()
}
fn base(common: &str, map: &str, capacity: usize) -> ObjectHost {
    ObjectHost::checked(enums(common, map, capacity))
        .unwrap()
        .with_rectangles()
        .unwrap()
        .with_trigger_admission(TriggerBudget {
            max_triggers: 4,
            max_unit_events: 0,
            max_actions: 4,
        })
        .unwrap()
        .with_filters(FilterBudget { max_records: 4 })
        .unwrap()
        .with_conditions(ConditionBudget { max_records: 4 })
        .unwrap()
}
fn limits() -> RegionBudget {
    RegionBudget {
        max_regions: 4,
        max_rect_additions: 4,
        max_events: 4,
    }
}
fn host(budget: RegionBudget) -> ObjectHost {
    base(COMMON, MAP, 64).with_regions(budget).unwrap()
}
fn id(value: &Value) -> HandleId {
    let Value::Handle(id) = value else {
        panic!("identity")
    };
    *id
}
fn inputs(h: &mut ObjectHost) -> (Value, Value, Value) {
    (
        h.native(CREATE, &[]).unwrap(),
        h.native(
            "Rect",
            &[Value::Int(-4), Value::Int(2), Value::Int(8), Value::Int(10)],
        )
        .unwrap(),
        h.native("CreateTrigger", &[]).unwrap(),
    )
}

#[test]
fn live_region_enumeration_includes_empty_and_added_regions() {
    assert!(base(COMMON, MAP, 64).executed_regions().is_err());
    let mut h = host(limits());
    let (first, rectangle, _) = inputs(&mut h);
    let second = h.native(CREATE, &[]).unwrap();
    h.native(ADD, &[second.clone(), rectangle]).unwrap();
    let regions: Vec<_> = h.executed_regions().unwrap().collect();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].0, id(&first));
    assert!(regions[0].1.is_empty());
    assert_eq!(regions[1].0, id(&second));
    assert_eq!(regions[1].1[0].bounds, [-4., 2., 8., 10.]);
}

#[test]
fn region_registration_history_preserves_interleaved_call_order_and_bounds() {
    let mut h = host(limits());
    assert!(h.region_registrations().is_err());
    h.enable_region_registration_order().unwrap();
    assert!(h.regions.as_ref().unwrap().registration_order.is_none());
    assert_eq!(h.region_registrations().unwrap().count(), 0);
    assert!(h.enable_region_registration_order().is_err());
    let (first, rectangle, _) = inputs(&mut h);
    let second = h.native(CREATE, &[]).unwrap();
    h.native(ADD, &[second.clone(), rectangle.clone()]).unwrap();
    h.native(ADD, &[first.clone(), rectangle.clone()]).unwrap();
    h.native(
        "MoveRectTo",
        &[rectangle.clone(), Value::Int(20), Value::Int(30)],
    )
    .unwrap();
    h.native(ADD, &[second.clone(), rectangle]).unwrap();
    let registrations = h
        .region_registrations()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        registrations
            .iter()
            .map(|entry| entry.0)
            .collect::<Vec<_>>(),
        [id(&second), id(&first), id(&second)]
    );
    assert_eq!(registrations[0].1.bounds, [-4., 2., 8., 10.]);
    assert_eq!(registrations[2].1.bounds, [14., 26., 26., 34.]);

    let mut late = host(limits());
    let (region, rectangle, _) = inputs(&mut late);
    late.native(ADD, &[region, rectangle]).unwrap();
    assert!(late.enable_region_registration_order().is_err());
    assert!(late.region_registrations().is_err());
}

#[cfg(target_pointer_width = "64")]
#[test]
fn region_registration_history_supports_wide_handle_budgets() {
    let mut h = host(limits());
    h.inner.max_handles = u32::MAX as usize + 1;
    h.enable_region_registration_order().unwrap();
    let (region, rectangle, _) = inputs(&mut h);
    h.native(ADD, &[region.clone(), rectangle]).unwrap();
    assert!(matches!(
        h.regions.as_ref().unwrap().registration_order.as_deref(),
        Some(RegistrationOrder::Wide(_))
    ));
    let entries = h
        .region_registrations()
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries[0].0, id(&region));
}

#[test]
fn owned_vm_retains_call_time_bounds_and_both_subscription_kinds_without_evaluation() {
    let mut vm = host(limits()).into_vm();
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let (g, r, t, a, b, c, f, p) = (
        id(&vm.global("G")),
        id(&vm.global("R")),
        id(&vm.global("T")),
        id(&vm.global("A")),
        id(&vm.global("B")),
        id(&vm.global("C")),
        id(&vm.global("F")),
        id(&vm.global("P")),
    );
    let h = vm.host();
    assert_eq!(h.inner().kind(g).unwrap(), "region");
    assert_ne!(g, r);
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_eq!(
        h.region_rect_requests(g).unwrap(),
        [
            RegionRectRequest {
                rectangle: r,
                bounds: [-4., 2., 8., 10.]
            },
            RegionRectRequest {
                rectangle: r,
                bounds: [14., 26., 26., 34.]
            }
        ]
    );
    for (event, filter, kind) in [
        (a, None, RegionEventKind::Enter),
        (b, Some(f), RegionEventKind::Enter),
        (c, Some(p), RegionEventKind::Leave),
    ] {
        assert_eq!(
            h.region_event_registration(event).unwrap(),
            RegionEventRegistration {
                trigger: t,
                region: g,
                filter,
                kind
            }
        );
        assert_eq!(h.inner().kind(event).unwrap(), "event");
    }
    assert_eq!(h.trigger_enabled(t).unwrap(), Some(false));
    assert_eq!(
        (
            h.retained_regions().unwrap(),
            h.retained_region_rect_requests().unwrap(),
            h.retained_region_events().unwrap()
        ),
        (1, 2, 3)
    );
    let before = h.inner().handles();
    for (name, args) in [
        ("RemoveRegion", vec![Value::Handle(g)]),
        ("RegionClearRect", vec![Value::Handle(g), Value::Handle(r)]),
        (
            "IsPointInRegion",
            vec![Value::Handle(g), Value::Int(0), Value::Int(0)],
        ),
    ] {
        assert!(
            vm.host()
                .native(name, &args)
                .unwrap_err()
                .contains("unsupported")
        );
    }
    vm.host().native("RemoveRect", &[Value::Handle(r)]).unwrap();
    assert_eq!(vm.host().region_rect_requests(g).unwrap().len(), 2);
    assert!(
        vm.host()
            .native(ADD, &[Value::Handle(g), Value::Handle(r)])
            .is_err()
    );
    assert_eq!(vm.host().inner().handles(), before);
    assert_eq!(vm.global("hits"), Value::Int(0));
}

#[test]
fn invalid_identities_and_filters_preserve_prior_requests_and_events() {
    let mut h = host(limits());
    let (g, r, t) = inputs(&mut h);
    h.native(ADD, &[g.clone(), r.clone()]).unwrap();
    let event = h
        .native(ENTER, &[t.clone(), g.clone(), Value::Null])
        .unwrap();
    let fake_region = h.inner.reserve_identity("region").unwrap();
    let fake_filter = h.inner.reserve_identity("boolexpr").unwrap();
    let fake_trigger = h.inner.reserve_identity("trigger").unwrap();
    let fake_rect = h.inner.reserve_identity("rect").unwrap();
    let before = h.inner().handles();
    let request = h.region_rect_requests(id(&g)).unwrap().to_vec();
    let record = h.region_event_registration(id(&event)).unwrap();
    let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(9999).unwrap()));
    for args in [
        vec![],
        vec![Value::Null, r.clone()],
        vec![Value::Handle(fake_region), r.clone()],
        vec![g.clone(), Value::Handle(fake_rect)],
        vec![r.clone(), g.clone()],
        vec![unknown.clone(), r.clone()],
        vec![g.clone(), r.clone(), Value::Null],
    ] {
        assert!(h.native(ADD, &args).is_err());
    }
    for name in [ENTER, LEAVE] {
        for args in [
            vec![],
            vec![Value::Null, g.clone(), Value::Null],
            vec![Value::Handle(fake_trigger), g.clone(), Value::Null],
            vec![t.clone(), Value::Handle(fake_region), Value::Null],
            vec![t.clone(), g.clone(), Value::Handle(fake_filter)],
            vec![t.clone(), g.clone(), r.clone()],
            vec![t.clone(), unknown.clone(), Value::Null],
        ] {
            assert!(h.native(name, &args).is_err());
        }
    }
    assert!(h.native(CREATE, &[Value::Null]).is_err());
    assert!(h.region_event_registration(id(&t)).is_err());
    assert!(h.region_rect_requests(id(&r)).is_err());
    assert_eq!(h.inner().handles(), before);
    assert_eq!(h.region_rect_requests(id(&g)).unwrap(), request);
    assert_eq!(h.region_event_registration(id(&event)).unwrap(), record);
    assert_eq!(
        (
            h.retained_regions().unwrap(),
            h.retained_region_rect_requests().unwrap(),
            h.retained_region_events().unwrap()
        ),
        (1, 1, 1)
    );
}

#[test]
fn independent_limits_and_shared_capacity_refuse_before_partial_changes() {
    let mut h = host(RegionBudget {
        max_regions: 0,
        ..limits()
    });
    let before = h.inner().handles();
    assert!(h.native(CREATE, &[]).is_err());
    assert_eq!(h.inner().handles(), before);
    assert_eq!(h.retained_regions().unwrap(), 0);
    let mut h = host(RegionBudget {
        max_rect_additions: 1,
        max_events: 1,
        ..limits()
    });
    let (g, r, t) = inputs(&mut h);
    let other = h.native(CREATE, &[]).unwrap();
    h.native(ADD, &[g.clone(), r.clone()]).unwrap();
    let event = h
        .native(ENTER, &[t.clone(), g.clone(), Value::Null])
        .unwrap();
    let before = h.inner().handles();
    assert!(h.native(ADD, &[other.clone(), r.clone()]).is_err());
    assert!(h.native(LEAVE, &[t.clone(), other, Value::Null]).is_err());
    assert_eq!(h.inner().handles(), before);
    assert!(h.region_rect_requests(id(&g)).is_ok());
    assert_eq!(
        h.region_event_registration(id(&event)).unwrap().region,
        id(&g)
    );
    let mut h = host(limits());
    let (g, r, t) = inputs(&mut h);
    while h.inner().handles() < 64 {
        h.native("CreateTimer", &[]).unwrap();
    }
    assert!(h.native(CREATE, &[]).is_err());
    assert!(h.native(ENTER, &[t, g.clone(), Value::Null]).is_err());
    assert_eq!(h.retained_regions().unwrap(), 1);
    assert_eq!(h.retained_region_events().unwrap(), 0);
    h.native(ADD, &[g.clone(), r]).unwrap();
    assert_eq!(h.region_rect_requests(id(&g)).unwrap().len(), 1);
    assert_eq!(h.inner().handles(), 64);
}

#[test]
fn admissions_are_opt_in_source_checked_and_platform_bounded() {
    let mut h = base(COMMON, MAP, 64);
    assert!(h.native(CREATE, &[]).unwrap_err().contains("unsupported"));
    assert!(h.retained_regions().is_err());
    assert!(
        h.with_regions(limits())
            .unwrap()
            .with_regions(limits())
            .is_err()
    );
    for budget in [
        RegionBudget {
            max_regions: usize::MAX,
            ..limits()
        },
        RegionBudget {
            max_rect_additions: usize::MAX,
            ..limits()
        },
        RegionBudget {
            max_events: usize::MAX,
            ..limits()
        },
    ] {
        assert!(
            matches!(base(COMMON,MAP,64).with_regions(budget),Err(e) if e.failure==Failure::Capacity)
        );
    }
    for common in [
        COMMON.replace(
            "native CreateRegion takes nothing",
            "constant native CreateRegion takes nothing",
        ),
        COMMON.replace(
            "region g, rect r returns nothing",
            "region g, rect r returns region",
        ),
        COMMON.replace("type region extends agent", "type region extends handle"),
    ] {
        assert!(base(&common, MAP, 64).with_regions(limits()).is_err());
    }
    let declaration = "native CreateRegion takes nothing returns region\n";
    let h = base(
        &COMMON.replace(declaration, ""),
        &format!("{declaration}{MAP}"),
        64,
    );
    assert!(matches!(h.with_regions(limits()),Err(e) if e.failure==Failure::Source));
    let h = ObjectHost::checked(enums(COMMON, MAP, 64)).unwrap();
    assert!(h.with_regions(limits()).is_err());
}

#[test]
fn absence_of_leave_declaration_keeps_creation_and_enter_support() {
    let common = COMMON.replace(
        "native TriggerRegisterLeaveRegion takes trigger t, region g, boolexpr f returns event\n",
        "",
    );
    let mut h = base(&common, "", 64).with_regions(limits()).unwrap();
    let (g, r, t) = inputs(&mut h);
    h.native(ADD, &[g.clone(), r]).unwrap();
    h.native(ENTER, &[t.clone(), g.clone(), Value::Null])
        .unwrap();
    let before = h.inner().handles();
    assert!(
        h.native(LEAVE, &[t, g, Value::Null])
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(h.inner().handles(), before);
}
