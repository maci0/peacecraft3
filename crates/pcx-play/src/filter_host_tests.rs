use super::{EnumHost, Failure, FilterBudget, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Prelude, Value};
use std::sync::Arc;

const FILTER: &str = "native Filter takes code predicate returns filterfunc";
const COMMON: &str = "type agent extends handle\ntype boolexpr extends agent\ntype filterfunc extends boolexpr\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative Filter takes code predicate returns filterfunc\nnative DestroyFilter takes filterfunc f returns nothing\nglobals\nconstant playercolor C = ConvertPlayerColor(37)\nendglobals\n";
const MAP: &str = "globals\ninteger hits = 0\nboolexpr A = null\nfilterfunc B = null\ntimer T = null\nendglobals\nfunction FalsePred takes nothing returns boolean\nset hits = hits + 1\nreturn false\nendfunction\nfunction TruePred takes nothing returns boolean\nset hits = hits + 10\nreturn true\nendfunction\nfunction BadReturn takes nothing returns nothing\nendfunction\nfunction BadArgs takes integer n returns boolean\nreturn true\nendfunction\nfunction setup takes nothing returns nothing\nset A = Filter(function TruePred)\nset B = Filter(function FalsePred)\nset T = CreateTimer()\nendfunction\n";
fn enums(common: &str, map: &str, max_handles: usize) -> EnumHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program =
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:filters", &common)]).unwrap();
    EnumHost::checked_selected(
        Arc::new(program),
        "synthetic:map:filters",
        &["ConvertPlayerColor"],
        Limits {
            max_handles,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap()
}
fn host(handles: usize, records: usize) -> ObjectHost {
    ObjectHost::checked(enums(COMMON, MAP, handles))
        .unwrap()
        .with_filters(FilterBudget {
            max_records: records,
        })
        .unwrap()
}
fn id(value: &Value) -> HandleId {
    let Value::Handle(id) = value else {
        panic!("filter identity")
    };
    *id
}
fn code(host: &ObjectHost, name: &str) -> Value {
    Value::Code(host.inner.program.function_id(name).unwrap())
}
#[test]
fn owned_vm_wraps_authored_callbacks_without_running_them_and_keeps_provenance() {
    let common = COMMON.to_string();
    let map = MAP.to_string();
    let host = ObjectHost::checked(enums(&common, &map, 8))
        .unwrap()
        .with_filters(FilterBudget { max_records: 2 })
        .unwrap();
    drop(common);
    drop(map);
    let mut vm = host.into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    assert_eq!(vm.global("hits"), Value::Int(0));
    let a = vm.global("A");
    let b = vm.global("B");
    let timer = vm.global("T");
    assert_ne!(a, b);
    assert_ne!(a, timer);
    assert_ne!(b, timer);
    assert_eq!(vm.host().inner().handles(), 4);
    for (value, name, result, hits) in [(&a, "TruePred", true, 10), (&b, "FalsePred", false, 11)] {
        let function = vm.program().function_id(name).unwrap();
        let declared = vm.program().functions()[function.index()].clone();
        let callback = vm.host().filter_callback(id(value)).unwrap();
        assert_eq!(callback.function, function);
        assert_eq!(callback.name, declared.name);
        assert_eq!(callback.source, declared.source);
        assert_eq!(callback.span, declared.span);
        let name = callback.name.to_string();
        assert_eq!(vm.host().inner().kind(id(value)).unwrap(), "filterfunc");
        assert_eq!(vm.call(&name, &[]).unwrap(), Value::Bool(result));
        assert_eq!(vm.global("hits"), Value::Int(hits));
    }
    assert!(
        vm.host()
            .native("DestroyFilter", std::slice::from_ref(&a))
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(vm.host().filter_callback(id(&a)).is_ok());
}
#[test]
fn invalid_callbacks_and_reserved_type_names_never_adopt_or_allocate_records() {
    let mut host = host(8, 2);
    let reserved = host.inner.reserve_identity("filterfunc").unwrap();
    let timer = host.native("CreateTimer", &[]).unwrap();
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    assert_eq!(host.inner().kind(reserved).unwrap(), "filterfunc");
    assert_eq!(
        host.filter_callback(reserved).unwrap_err().failure,
        Failure::WrongFamily
    );
    for handle in [reserved, id(&timer), unknown] {
        assert!(host.filter_callback(handle).is_err());
    }
    let original = code(&host, "FalsePred");
    let filter = host
        .native("Filter", std::slice::from_ref(&original))
        .unwrap();
    let callback = host.filter_callback(id(&filter)).unwrap().function;
    let handles = host.inner().handles();
    for args in [
        vec![],
        vec![Value::Null],
        vec![timer],
        vec![Value::Code(pcx_script::value::FunctionId(u32::MAX))],
        vec![code(&host, "BadReturn")],
        vec![code(&host, "BadArgs")],
        vec![original.clone(), original],
    ] {
        assert!(host.native("Filter", &args).is_err());
        assert_eq!(host.inner().handles(), handles);
        assert_eq!(
            host.filter_callback(id(&filter)).unwrap().function,
            callback
        );
    }
    let other = code(&host, "TruePred");
    assert!(host.native("Filter", &[other]).is_ok());
    assert_eq!(host.inner().handles(), handles + 1);
}
#[test]
fn caller_and_shared_arena_limits_refuse_before_partial_allocation() {
    for records in [0, 1] {
        let mut host = host(4, records);
        if records == 1 {
            let first = code(&host, "FalsePred");
            host.native("Filter", &[first]).unwrap();
        }
        let handles = host.inner().handles();
        let next = code(&host, "TruePred");
        assert!(
            host.native("Filter", &[next])
                .unwrap_err()
                .contains("filter wrappers")
        );
        assert_eq!(host.inner().handles(), handles);
        host.native("CreateTimer", &[]).unwrap();
        assert_eq!(host.live_timers(), 1);
    }
    let mut host = host(1, 2);
    let timer = host.native("CreateTimer", &[]).unwrap();
    let next = code(&host, "TruePred");
    assert!(
        host.native("Filter", std::slice::from_ref(&next))
            .unwrap_err()
            .contains("capacity")
    );
    assert_eq!(host.inner().handles(), 1);
    assert_eq!(host.live_timers(), 1);
    host.native("DestroyTimer", &[timer]).unwrap();
    assert!(host.native("Filter", &[next]).is_err());
    assert_eq!(host.inner().handles(), 1);
}
#[test]
fn admission_checks_loaded_interface_and_ancestry_sources_without_copying_edges() {
    let mut unadmitted = ObjectHost::checked(enums(COMMON, MAP, 4)).unwrap();
    let callback = code(&unadmitted, "TruePred");
    assert!(
        unadmitted
            .native("Filter", &[callback])
            .unwrap_err()
            .contains("unsupported")
    );
    assert_eq!(unadmitted.inner().handles(), 0);
    for common in [
        COMMON.replace(FILTER, ""),
        COMMON.replace(FILTER, &format!("constant {FILTER}")),
        COMMON.replace("Filter takes code", "Filter takes integer"),
        COMMON.replace("returns filterfunc", "returns boolexpr"),
        COMMON.replace("filterfunc extends boolexpr", "filterfunc extends agent"),
        COMMON.replace("boolexpr extends agent", "boolexpr extends handle"),
    ] {
        assert!(
            ObjectHost::checked(enums(&common, "", 4))
                .unwrap()
                .with_filters(FilterBudget { max_records: 1 })
                .is_err()
        );
    }
    for map in [
        format!("{FILTER}\n{MAP}"),
        format!("type boolexpr extends agent\n{MAP}"),
        format!("type filterfunc extends boolexpr\n{MAP}"),
        format!(
            "function Filter takes code predicate returns filterfunc\nreturn null\nendfunction\n{MAP}"
        ),
    ] {
        assert!(
            ObjectHost::checked(enums(COMMON, &map, 4))
                .unwrap()
                .with_filters(FilterBudget { max_records: 1 })
                .is_err()
        );
    }
    let source = COMMON.replace(FILTER, "");
    let map = format!("{FILTER}\n{MAP}");
    assert!(
        ObjectHost::checked(enums(&source, &map, 4))
            .unwrap()
            .with_filters(FilterBudget { max_records: 1 })
            .is_err()
    );
    let extended = COMMON.replace(
        "type filterfunc extends boolexpr",
        "type extra extends boolexpr\ntype filterfunc extends extra",
    );
    assert!(
        ObjectHost::checked(enums(&extended, MAP, 4))
            .unwrap()
            .with_filters(FilterBudget { max_records: 1 })
            .is_ok()
    );
    assert!(
        host(4, 1)
            .with_filters(FilterBudget { max_records: 1 })
            .is_err()
    );
    assert!(
        ObjectHost::checked(enums(COMMON, MAP, 4))
            .unwrap()
            .with_filters(FilterBudget {
                max_records: usize::MAX
            })
            .is_err()
    );
}

#[test]
fn null_destruction_is_a_no_op_and_destroying_a_wrapper_frees_it_and_requires_its_source_declaration()
 {
    let common = COMMON.replace(
        "native DestroyFilter",
        "native DestroyBoolExpr takes boolexpr b returns nothing\nnative DestroyFilter",
    );
    let mut h = ObjectHost::checked(enums(&common, MAP, 8))
        .unwrap()
        .with_filters(FilterBudget { max_records: 2 })
        .unwrap();
    let callback = h.inner.program.function_id("TruePred").unwrap();
    let wrapper = h.native("Filter", &[Value::Code(callback)]).unwrap();
    let before = h.inner().handles();
    for _ in 0..2 {
        assert_eq!(
            h.native("DestroyBoolExpr", &[Value::Null]).unwrap(),
            Value::Null
        );
    }
    assert_eq!(h.inner().handles(), before);
    // A live wrapper is freed by the declared native; later lookups refuse (boolexpr_destroy_tests).
    assert_eq!(
        h.native("DestroyBoolExpr", std::slice::from_ref(&wrapper))
            .unwrap(),
        Value::Null
    );
    assert!(h.filter_callback(id(&wrapper)).is_err());
    assert!(
        host(8, 2)
            .native("DestroyBoolExpr", &[Value::Null])
            .is_err()
    );
    let wrong = common.replace(
        "DestroyBoolExpr takes boolexpr b",
        "DestroyBoolExpr takes integer b",
    );
    assert!(
        ObjectHost::checked(enums(&wrong, MAP, 8))
            .unwrap()
            .with_filters(FilterBudget { max_records: 2 })
            .is_err()
    );
}
