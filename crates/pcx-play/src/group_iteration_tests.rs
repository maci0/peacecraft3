use super::*;

const ITERATION: &str = "native ForGroup takes group g, code f returns nothing\nconstant native GetEnumUnit takes nothing returns unit\n";
const SCENE: &str = r#"
globals
group G = CreateGroup()
group DEST = CreateGroup()
group EMPTY = CreateGroup()
unit A = null
unit B = null
unit C = null
integer HITS = 0
integer INNER = 0
boolean OK = true
endglobals
function setup takes unit a, unit b, unit c returns nothing
set A = a
set B = b
set C = c
call GroupAddUnit(G, A)
call GroupAddUnit(G, B)
endfunction
function Copy takes nothing returns nothing
set HITS = HITS + 1
call GroupAddUnit(DEST, GetEnumUnit())
endfunction
"#;

fn vm(script: &str) -> pcx_script::OwnedVm<ObjectHost> {
    let mut h = timer_host(
        &format!("{COMMON}{ITERATION}"),
        &format!("{SCENE}{script}"),
        16,
    )
    .with_groups(12)
    .unwrap();
    let units = [unit(&mut h), unit(&mut h), unit(&mut h)];
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("setup", &units).unwrap();
    vm
}

#[test]
fn actions_share_vm_globals_and_handles_and_empty_group_does_not_call() {
    let mut v = vm(
        "function run takes nothing returns nothing\ncall ForGroup(EMPTY, function Copy)\ncall ForGroup(G, function Copy)\nendfunction\n",
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(2));
    for name in ["A", "B"] {
        let args = [v.global(name), v.global("DEST")];
        assert_eq!(
            v.host().native("IsUnitInGroup", &args).unwrap(),
            Value::Bool(true)
        );
    }
    let group = v.global("G");
    let a = v.global("A");
    assert_eq!(first(v.host(), &group), a);
    assert_eq!(v.host().native("GetEnumUnit", &[]).unwrap(), Value::Null);
}

#[test]
fn edits_and_destruction_do_not_replace_the_retained_members() {
    let mut v = vm(r#"
function Mutate takes nothing returns nothing
call Copy()
if HITS == 1 then
call GroupRemoveUnit(G, B)
call GroupClear(G)
call GroupAddUnit(G, C)
call DestroyGroup(G)
endif
endfunction
function run takes nothing returns nothing
call ForGroup(G, function Mutate)
endfunction
"#);
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(2));
    for (name, expected) in [("A", true), ("B", true), ("C", false)] {
        let args = [v.global(name), v.global("DEST")];
        assert_eq!(
            v.host().native("IsUnitInGroup", &args).unwrap(),
            Value::Bool(expected)
        );
    }
}

#[test]
fn nested_same_group_restores_member_and_callback() {
    let mut v = vm(r#"
function InnerAction takes nothing returns nothing
set INNER = INNER + 1
endfunction
function OuterAction takes nothing returns nothing
local unit saved = GetEnumUnit()
call ForGroup(G, function InnerAction)
call ForGroup(EMPTY, function InnerAction)
if GetEnumUnit() != saved then
set OK = false
endif
call Copy()
endfunction
function run takes nothing returns nothing
call ForGroup(G, function OuterAction)
endfunction
"#);
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(2));
    assert_eq!(v.global("INNER"), Value::Int(4));
    assert_eq!(v.global("OK"), Value::Bool(true));
    assert_eq!(v.host().native("GetEnumUnit", &[]).unwrap(), Value::Null);
}

#[test]
fn errors_clear_context_and_keep_completed_effects() {
    let mut v = vm(r#"
native Missing takes nothing returns nothing
function Fail takes nothing returns nothing
call Copy()
call Missing()
endfunction
function run takes nothing returns nothing
call ForGroup(G, function Fail)
endfunction
function retry takes nothing returns nothing
call ForGroup(G, function Copy)
endfunction
"#);
    assert!(
        v.call("run", &[])
            .unwrap_err()
            .to_string()
            .contains("Missing")
    );
    assert_eq!(v.global("HITS"), Value::Int(1));
    assert_eq!(v.host().native("GetEnumUnit", &[]).unwrap(), Value::Null);
    v.call("retry", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(3));
}

#[test]
fn invalid_callbacks_refuse_before_empty_iteration() {
    let mut v = vm(r#"
function WrongReturn takes nothing returns boolean
return true
endfunction
function WrongParam takes unit u returns nothing
endfunction
function wrong_return takes nothing returns nothing
call ForGroup(EMPTY, function WrongReturn)
endfunction
function wrong_param takes nothing returns nothing
call ForGroup(G, function WrongParam)
endfunction
function null_code takes nothing returns nothing
call ForGroup(G, null)
endfunction
"#);
    for name in ["wrong_return", "wrong_param", "null_code"] {
        assert!(
            v.call(name, &[])
                .unwrap_err()
                .to_string()
                .contains("ForGroup")
        );
        assert_eq!(v.host().native("GetEnumUnit", &[]).unwrap(), Value::Null);
    }
    assert!(v.host().native("GetEnumUnit", &[Value::Null]).is_err());
}

#[test]
fn optional_declarations_require_matching_contract_and_source() {
    for extra in [
        ITERATION.replace("returns unit", "returns group"),
        ITERATION.replace("group g, code f", "group g, integer f"),
        ITERATION.replace("constant native GetEnumUnit", "native GetEnumUnit"),
    ] {
        assert!(
            timer_host(&format!("{COMMON}{extra}"), "", 8)
                .with_groups(4)
                .is_err()
        );
    }
    let foreign = pcx_script::parse_script(ITERATION).unwrap();
    let common = pcx_script::parse_script(COMMON).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let p = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:common", &common),
                Prelude::new("synthetic:foreign", &foreign),
            ],
        )
        .unwrap(),
    );
    let h = ObjectHost::checked(
        EnumHost::checked_selected(
            p,
            "map",
            &["ConvertPlayerColor"],
            Limits {
                max_handles: 8,
                admission_steps: 100,
                expression_depth: 16,
                expression_work: 100,
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(h.with_groups(4), Err(e) if e.failure == Failure::Source));
    let mut h = host(8, 4);
    assert!(
        h.native("GetEnumUnit", &[])
            .unwrap_err()
            .contains("unsupported")
    );
    assert!(h.begin_script_call("ForGroup", &[]).unwrap().is_none());
}
