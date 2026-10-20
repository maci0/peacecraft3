use super::super::{ConditionBudget, EnumHost, Failure, FilterBudget, Limits, ObjectHost};
use pcx_script::{Prelude, Value};
use std::sync::Arc;

const DECLARATION: &str = "native DestroyBoolExpr takes boolexpr e returns nothing";
const COMMON: &str = "type agent extends handle\ntype boolexpr extends agent\ntype conditionfunc extends boolexpr\ntype filterfunc extends boolexpr\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative Condition takes code func returns conditionfunc\nnative Filter takes code predicate returns filterfunc\nnative DestroyBoolExpr takes boolexpr e returns nothing\nglobals\nconstant playercolor C = ConvertPlayerColor(37)\nendglobals\n";
const MAP: &str = "globals\nfilterfunc F = null\nconditionfunc C = null\ntimer T = null\nendglobals\nfunction TruePred takes nothing returns boolean\nreturn true\nendfunction\nfunction setup takes nothing returns nothing\nset F = Filter(function TruePred)\nset C = Condition(function TruePred)\nset T = CreateTimer()\nendfunction\nfunction kill_filter takes nothing returns nothing\ncall DestroyBoolExpr(F)\nendfunction\nfunction kill_condition takes nothing returns nothing\ncall DestroyBoolExpr(C)\nendfunction\nfunction kill_timer takes nothing returns nothing\ncall DestroyBoolExpr(T)\nendfunction\nfunction kill_null takes nothing returns nothing\ncall DestroyBoolExpr(null)\nendfunction\n";

fn enums(common: &str) -> EnumHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(MAP).unwrap();
    let program =
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:boolexpr", &common)]).unwrap();
    EnumHost::checked_selected(
        Arc::new(program),
        "synthetic:map:boolexpr",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 16,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap()
}

fn host() -> ObjectHost {
    ObjectHost::checked(enums(COMMON))
        .unwrap()
        .with_conditions(ConditionBudget { max_records: 4 })
        .unwrap()
        .with_filters(FilterBudget { max_records: 4 })
        .unwrap()
}

fn handle(vm: &pcx_script::OwnedVm<ObjectHost>, name: &str) -> pcx_script::HandleId {
    let Value::Handle(id) = vm.global(name) else {
        panic!("{name} is a handle")
    };
    id
}

#[test]
fn destroying_a_filter_frees_it_and_every_later_use_refuses() {
    let mut vm = host().into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = handle(&vm, "F");
    assert!(vm.host().filter_callback(id).is_ok());
    vm.call("kill_filter", &[]).unwrap();
    assert_eq!(
        vm.host().filter_callback(id).unwrap_err().failure,
        Failure::Identity
    );
    assert!(vm.call("kill_filter", &[]).is_err(), "a second destroy refuses");
}

#[test]
fn destroying_a_condition_frees_it_and_every_later_use_refuses() {
    let mut vm = host().into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = handle(&vm, "C");
    assert!(vm.host().condition_callback(id).is_ok());
    vm.call("kill_condition", &[]).unwrap();
    assert_eq!(
        vm.host().condition_callback(id).unwrap_err().failure,
        Failure::Identity
    );
    assert!(vm.call("kill_condition", &[]).is_err());
}

#[test]
fn destroying_null_is_the_declared_no_op() {
    let mut vm = host().into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    vm.call("kill_null", &[]).unwrap();
    let id = handle(&vm, "F");
    assert!(vm.host().filter_callback(id).is_ok());
}

#[test]
fn a_non_boolexpr_handle_refuses_and_stays_live() {
    let mut vm = host().into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    assert!(vm.call("kill_timer", &[]).is_err());
}

#[test]
fn admission_requires_the_declared_boolexpr_parameter() {
    let wrong = COMMON.replace(DECLARATION, "native DestroyBoolExpr takes timer e returns nothing");
    let refused = ObjectHost::checked(enums(&wrong))
        .unwrap()
        .with_conditions(ConditionBudget { max_records: 4 })
        .unwrap()
        .with_filters(FilterBudget { max_records: 4 });
    assert!(refused.is_err());
}
