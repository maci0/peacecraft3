use super::{EXTRAS, common_source, enumerated, trigger_limits};
use crate::playercolor_host::{ConditionBudget, Failure, FilterBudget, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;

const ADD: &str = "TriggerAddCondition";
const EXTRA: &str = "\
type boolexpr extends agent
type conditionfunc extends boolexpr
type filterfunc extends boolexpr
type triggercondition extends agent
native Condition takes code f returns conditionfunc
native Filter takes code f returns filterfunc
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native DestroyBoolExpr takes boolexpr e returns nothing
";
const MAP: &str = "\
globals
trigger T = null
boolexpr C = null
boolexpr F = null
triggercondition A = null
triggercondition B = null
triggercondition D = null
integer hits = 0
endglobals
function predicate takes nothing returns boolean
set hits = hits + 1
return false
endfunction
function main takes nothing returns nothing
set T = CreateTrigger()
set F = Filter(function predicate)
set C = Condition(function predicate)
set A = TriggerAddCondition(T,C)
set B = TriggerAddCondition(T,F)
set D = TriggerAddCondition(T,C)
endfunction
function kill takes nothing returns nothing
call DestroyBoolExpr(C)
endfunction
";
fn common() -> String {
    common_source(&format!("{EXTRAS}{EXTRA}"))
}
fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}
fn host(limit: usize) -> (Arc<crate::map_admission::ParsedMapContext>, ObjectHost) {
    let (ctx, h) = enumerated(&common(), MAP, 64);
    let h = h
        .with_trigger_admission(trigger_limits(4, 0))
        .unwrap()
        .with_filters(FilterBudget { max_records: 4 })
        .unwrap()
        .with_conditions(ConditionBudget { max_records: 4 })
        .unwrap()
        .with_trigger_conditions(limit)
        .unwrap();
    (ctx, h)
}
#[test]
fn owned_vm_associates_both_wrappers_without_evaluating_or_merging_them() {
    let (ctx, h) = host(3);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let t = id(&vm.global("T"));
    let c = id(&vm.global("C"));
    let f = id(&vm.global("F"));
    let a = id(&vm.global("A"));
    let b = id(&vm.global("B"));
    let d = id(&vm.global("D"));
    assert_ne!(a, b);
    assert_ne!(a, d);
    assert_ne!(b, d);
    let h = &mut vm.host().inner;
    for (token, condition) in [(a, c), (b, f), (d, c)] {
        let record = h.trigger_condition_registration(token).unwrap();
        assert_eq!((record.trigger, record.condition), (t, condition));
        assert_eq!(h.inner().kind(token).unwrap(), "triggercondition");
    }
    assert_eq!(h.retained_trigger_conditions().unwrap(), 3);
    assert_eq!(h.retained_conditions().unwrap(), 1);
    assert_eq!(h.retained_unit_events().unwrap(), 0);
    assert!(h.native("IsTriggerEnabled", &[Value::Handle(t)]).is_err());
    assert_eq!(vm.call("predicate", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.global("hits"), Value::Int(1));
}
#[test]
fn bad_calls_preserve_an_existing_association_with_capacity_remaining() {
    let (ctx, mut h) = host(4);
    let t = h.native("CreateTrigger", &[]).unwrap();
    let f = h
        .native(
            "Filter",
            &[Value::Code(
                ctx.scripts.program().function_id("predicate").unwrap(),
            )],
        )
        .unwrap();
    let first = h.native(ADD, &[t.clone(), f.clone()]).unwrap();
    let before = h.inner().handles();
    for args in [
        vec![],
        vec![Value::Null, f.clone()],
        vec![f.clone(), t.clone()],
        vec![t.clone(), t.clone()],
        vec![t.clone(), Value::Null],
        vec![t, Value::Bool(false)],
    ] {
        assert!(h.native(ADD, &args).is_err());
        assert_eq!(h.inner().handles(), before);
        assert_eq!(h.retained_trigger_conditions().unwrap(), 1);
        assert_eq!(
            h.trigger_condition_registration(id(&first))
                .unwrap()
                .condition,
            id(&f)
        );
    }
    assert!(h.trigger_condition_registration(id(&f)).is_err());
}
#[test]
fn record_and_arena_limits_leave_prior_associations_unchanged() {
    for (limit, fill_arena) in [(0, false), (1, false), (2, true)] {
        let (ctx, mut h) = host(limit);
        let t = h.native("CreateTrigger", &[]).unwrap();
        let f = h
            .native(
                "Filter",
                &[Value::Code(
                    ctx.scripts.program().function_id("predicate").unwrap(),
                )],
            )
            .unwrap();
        let first = if limit > 0 {
            Some(h.native(ADD, &[t.clone(), f.clone()]).unwrap())
        } else {
            None
        };
        if fill_arena {
            while h.inner().handles() < 64 {
                h.native("CreateTimer", &[]).unwrap();
            }
        }
        let before = h.inner().handles();
        assert!(h.native(ADD, &[t, f.clone()]).is_err());
        assert_eq!(h.inner().handles(), before);
        assert_eq!(
            h.retained_trigger_conditions().unwrap(),
            usize::from(first.is_some())
        );
        if let Some(first) = first {
            assert_eq!(
                h.trigger_condition_registration(id(&first))
                    .unwrap()
                    .condition,
                id(&f)
            );
        }
    }
}
#[test]
fn admission_is_explicit_source_bound_and_nonrepeatable() {
    let (ctx, h) = enumerated(&common(), MAP, 64);
    let mut h = h
        .with_trigger_admission(trigger_limits(4, 0))
        .unwrap()
        .with_filters(FilterBudget { max_records: 4 })
        .unwrap();
    let t = h.native("CreateTrigger", &[]).unwrap();
    let f = h
        .native(
            "Filter",
            &[Value::Code(
                ctx.scripts.program().function_id("predicate").unwrap(),
            )],
        )
        .unwrap();
    assert!(h.native(ADD, &[t, f]).unwrap_err().contains("unsupported"));
    assert!(h.retained_trigger_conditions().is_err());
    assert!(
        h.with_trigger_conditions(1)
            .unwrap()
            .with_trigger_conditions(1)
            .is_err()
    );
    for bad in [
        common().replace(
            "native TriggerAddCondition",
            "constant native TriggerAddCondition",
        ),
        common().replace(
            "boolexpr c returns triggercondition",
            "boolexpr c returns event",
        ),
        common().replace(
            "type triggercondition extends agent",
            "type triggercondition extends handle",
        ),
    ] {
        let (_, h) = enumerated(&bad, MAP, 64);
        assert!(
            h.with_trigger_admission(trigger_limits(4, 0))
                .unwrap()
                .with_trigger_conditions(1)
                .is_err()
        );
    }
    let (_, h) = enumerated(&common(), MAP, 64);
    assert!(
        matches!(h.with_trigger_admission(trigger_limits(4,0)).unwrap().with_trigger_conditions(usize::MAX),Err(e) if e.failure==Failure::Capacity)
    );
}

#[test]
fn a_wrapper_retained_by_a_trigger_condition_refuses_destruction_and_stays_live() {
    let (ctx, h) = host(3);
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    let c = id(&vm.global("C"));
    assert!(
        vm.call("kill", &[]).is_err(),
        "a retained wrapper refuses destruction"
    );
    assert!(
        vm.host().inner.condition_callback(c).is_ok(),
        "the wrapper stays live"
    );
}
