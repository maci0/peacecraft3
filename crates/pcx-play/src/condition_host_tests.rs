use super::{ConditionBudget, EnumHost, Failure, FilterBudget, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Prelude, Value};
use std::sync::Arc;

const CONDITION: &str = "native Condition takes code func returns conditionfunc";
const COMMON: &str = "type agent extends handle\ntype boolexpr extends agent\ntype conditionfunc extends boolexpr\ntype filterfunc extends boolexpr\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative Condition takes code func returns conditionfunc\nnative DestroyCondition takes conditionfunc c returns nothing\nnative Filter takes code predicate returns filterfunc\nglobals\nconstant playercolor C = ConvertPlayerColor(37)\nendglobals\n";
const MAP: &str = "globals\ninteger hits = 0\nboolexpr A = null\nconditionfunc B = null\nfilterfunc F = null\ntimer T = null\nendglobals\nfunction FalsePred takes nothing returns boolean\nset hits = hits + 1\nreturn false\nendfunction\nfunction TruePred takes nothing returns boolean\nset hits = hits + 10\nreturn true\nendfunction\nfunction BadReturn takes nothing returns nothing\nendfunction\nfunction BadArgs takes integer n returns boolean\nreturn true\nendfunction\nfunction setup takes nothing returns nothing\nset A = Condition(function TruePred)\nset B = Condition(function FalsePred)\nset F = Filter(function TruePred)\nset T = CreateTimer()\nendfunction\n";
fn enums(common: &str, map: &str, max_handles: usize) -> EnumHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program =
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:conditions", &common)]).unwrap();
    EnumHost::checked_selected(
        Arc::new(program),
        "synthetic:map:conditions",
        &["ConvertPlayerColor"],
        Limits {
            max_handles,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap()
}
fn host(handles: usize, records: usize) -> ObjectHost {
    ObjectHost::checked(enums(COMMON, MAP, handles))
        .unwrap()
        .with_conditions(ConditionBudget {
            max_records: records,
        })
        .unwrap()
        .with_filters(FilterBudget { max_records: 1 })
        .unwrap()
}
fn id(value: &Value) -> HandleId {
    let Value::Handle(id) = value else {
        panic!("condition identity")
    };
    *id
}
fn code(host: &ObjectHost, name: &str) -> Value {
    Value::Code(host.inner.program.function_id(name).unwrap())
}
#[test]
fn owned_vm_retains_distinct_condition_wrappers_without_evaluating_them() {
    let host = host(8, 2);
    let mut vm = host.into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let a = vm.global("A");
    let b = vm.global("B");
    let filter = vm.global("F");
    let timer = vm.global("T");
    assert_ne!(a, b);
    assert_ne!(a, filter);
    assert_ne!(b, filter);
    assert_ne!(a, timer);
    assert_eq!(vm.host().retained_conditions().unwrap(), 2);
    assert_eq!(vm.host().inner().handles(), 5);
    for (value, name, result, hits) in [(&a, "TruePred", true, 10), (&b, "FalsePred", false, 11)] {
        let function = vm.program().function_id(name).unwrap();
        let declared = vm.program().functions()[function.index()].clone();
        let callback = vm.host().condition_callback(id(value)).unwrap();
        assert_eq!(callback.function, function);
        assert_eq!(callback.name, declared.name);
        assert_eq!(callback.source, declared.source);
        assert_eq!(callback.span, declared.span);
        assert_eq!(vm.host().inner().kind(id(value)).unwrap(), "conditionfunc");
        assert_eq!(vm.call(name, &[]).unwrap(), Value::Bool(result));
        assert_eq!(vm.global("hits"), Value::Int(hits));
    }
    assert!(
        vm.host()
            .condition_callback(id(&filter))
            .unwrap_err()
            .failure
            == Failure::WrongFamily
    );
    assert!(
        vm.host()
            .native("DestroyCondition", std::slice::from_ref(&a))
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(vm.host().condition_callback(id(&a)).is_ok());
    assert_eq!(vm.global("hits"), Value::Int(11));
}
#[test]
fn invalid_inputs_and_reserved_identities_leave_the_arena_unchanged() {
    let mut host = host(8, 2);
    let reserved = host.inner.reserve_identity("conditionfunc").unwrap();
    let timer = host.native("CreateTimer", &[]).unwrap();
    let filter = host.native("Filter", &[code(&host, "TruePred")]).unwrap();
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    assert_eq!(host.inner().kind(reserved).unwrap(), "conditionfunc");
    assert_eq!(
        host.condition_callback(reserved).unwrap_err().failure,
        Failure::WrongFamily
    );
    for handle in [reserved, id(&timer), id(&filter), unknown] {
        assert!(host.condition_callback(handle).is_err());
    }
    let original = code(&host, "FalsePred");
    let condition = host
        .native("Condition", std::slice::from_ref(&original))
        .unwrap();
    let callback = host.condition_callback(id(&condition)).unwrap().function;
    let handles = host.inner().handles();
    let retained = host.retained_conditions().unwrap();
    for args in [
        vec![],
        vec![Value::Null],
        vec![timer.clone()],
        vec![filter.clone()],
        vec![Value::Code(pcx_script::value::FunctionId(u32::MAX))],
        vec![code(&host, "BadReturn")],
        vec![code(&host, "BadArgs")],
        vec![original.clone(), original.clone()],
    ] {
        assert!(host.native("Condition", &args).is_err());
        assert_eq!(host.inner().handles(), handles);
        assert_eq!(host.retained_conditions().unwrap(), retained);
        assert_eq!(
            host.condition_callback(id(&condition)).unwrap().function,
            callback
        );
    }
    assert!(host.native("Condition", &[code(&host, "TruePred")]).is_ok());
    assert_eq!(host.inner().handles(), handles + 1);
    assert_eq!(host.retained_conditions().unwrap(), retained + 1);
}
#[test]
fn caller_and_shared_budgets_refuse_without_partial_records() {
    for records in [0, 1] {
        let mut host = host(6, records);
        if records == 1 {
            host.native("Condition", &[code(&host, "FalsePred")])
                .unwrap();
        }
        let handles = host.inner().handles();
        let retained = host.retained_conditions().unwrap();
        assert!(
            host.native("Condition", &[code(&host, "TruePred")])
                .unwrap_err()
                .contains("condition wrappers")
        );
        assert_eq!(host.inner().handles(), handles);
        assert_eq!(host.retained_conditions().unwrap(), retained);
        host.native("CreateTimer", &[]).unwrap();
        assert_eq!(host.live_timers(), 1);
    }
    let mut host = host(1, 2);
    let timer = host.native("CreateTimer", &[]).unwrap();
    let next = code(&host, "TruePred");
    assert!(
        host.native("Condition", std::slice::from_ref(&next))
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(host.inner().handles(), 1);
    assert_eq!(host.retained_conditions().unwrap(), 0);
    assert_eq!(host.live_timers(), 1);
    host.native("DestroyTimer", &[timer]).unwrap();
    assert!(host.native("Condition", &[next]).is_err());
    assert_eq!(host.inner().handles(), 1);
    assert_eq!(host.retained_conditions().unwrap(), 0);
}
#[test]
fn admission_checks_loaded_contract_source_and_ancestry() {
    let mut unadmitted = ObjectHost::checked(enums(COMMON, MAP, 4)).unwrap();
    assert!(unadmitted.retained_conditions().is_err());
    assert!(
        unadmitted
            .native("Condition", &[code(&unadmitted, "TruePred")])
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(unadmitted.inner().handles(), 0);
    for common in [
        COMMON.replace(CONDITION, ""),
        COMMON.replace(CONDITION, &format!("constant {CONDITION}")),
        COMMON.replace("Condition takes code", "Condition takes integer"),
        COMMON.replace("returns conditionfunc", "returns boolexpr"),
        COMMON.replace(
            "conditionfunc extends boolexpr",
            "conditionfunc extends agent",
        ),
        COMMON.replace("boolexpr extends agent", "boolexpr extends handle"),
    ] {
        assert!(
            ObjectHost::checked(enums(&common, "", 4))
                .unwrap()
                .with_conditions(ConditionBudget { max_records: 1 })
                .is_err()
        );
    }
    for map in [
        format!("{CONDITION}\n{MAP}"),
        "type boolexpr extends agent\n".to_string() + MAP,
        "type conditionfunc extends boolexpr\n".to_string() + MAP,
        format!(
            "function Condition takes code func returns conditionfunc\nreturn null\nendfunction\n{MAP}"
        ),
    ] {
        assert!(
            ObjectHost::checked(enums(COMMON, &map, 4))
                .unwrap()
                .with_conditions(ConditionBudget { max_records: 1 })
                .is_err()
        );
    }
    let source = COMMON.replace(CONDITION, "");
    assert!(
        ObjectHost::checked(enums(&source, &format!("{CONDITION}\n{MAP}"), 4))
            .unwrap()
            .with_conditions(ConditionBudget { max_records: 1 })
            .is_err()
    );
    let common = pcx_script::parse_script(COMMON).unwrap();
    let other = pcx_script::parse_script(&format!("{CONDITION}\n")).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = pcx_script::resolve_with(
        &map,
        &[
            Prelude::new("synthetic:conditions", &common),
            Prelude::new("synthetic:other", &other),
        ],
    )
    .unwrap();
    let foreign = ObjectHost::checked(
        EnumHost::checked_selected(
            Arc::new(program),
            "synthetic:map:conditions",
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 4,
                admission_steps: 2000,
                expression_depth: 16,
                expression_work: 100,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(
        foreign.with_conditions(ConditionBudget { max_records: 1 }),
        Err(error) if error.failure == Failure::Source
    ));
    let extended = COMMON.replace(
        "type conditionfunc extends boolexpr",
        "type extra extends boolexpr\ntype conditionfunc extends extra",
    );
    assert!(
        ObjectHost::checked(enums(&extended, MAP, 4))
            .unwrap()
            .with_conditions(ConditionBudget { max_records: 1 })
            .is_ok()
    );
    assert!(
        host(4, 1)
            .with_conditions(ConditionBudget { max_records: 1 })
            .is_err()
    );
    assert!(
        ObjectHost::checked(enums(COMMON, MAP, 4))
            .unwrap()
            .with_conditions(ConditionBudget {
                max_records: usize::MAX
            })
            .is_err()
    );
}
