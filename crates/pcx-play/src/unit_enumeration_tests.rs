use super::*;

type Vm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;

const SCENE: &str = r#"
globals
unit A = null
unit B = null
unit C = null
group TARGET = null
group INNER = null
group OTHER = null
rect BOUNDS = null
rect EMPTY = null
integer HITS = 0
integer INNER_HITS = 0
boolean OK = true
unit SAVED = null
endglobals
function setup takes nothing returns nothing
set A = CreateUnit(Player(4), 'U001', 16.0, 16.0, 0.0)
set B = CreateUnit(Player(5), 'U001', 48.0, 16.0, 0.0)
set C = CreateUnit(Player(4), 'U001', 80.0, 16.0, 0.0)
set TARGET = CreateGroup()
set INNER = CreateGroup()
set OTHER = CreateGroup()
set BOUNDS = Rect(0.0, 0.0, 100.0, 100.0)
set EMPTY = Rect(0.0, 0.0, 1.0, 1.0)
endfunction
function FalsePredicate takes nothing returns boolean
set HITS = HITS + 1
return false
endfunction
function TruePredicate takes nothing returns boolean
set HITS = HITS + 1
return true
endfunction
function clear_filtered takes nothing returns nothing
call GroupEnumUnitsInRect(TARGET, BOUNDS, Condition(function FalsePredicate))
endfunction
"#;

fn vm(script: &str, members: usize, declarations: &str) -> Vm {
    let host = position_host_program(100, members, &format!("{SCENE}\n{script}"), declarations);
    let ctx = Arc::clone(host.player_context().unwrap());
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    vm
}

#[test]
fn owner_predicate_runs_on_the_same_vm_and_group_is_cleared_before_callbacks() {
    let mut v = vm(
        r#"
function OwnerPredicate takes nothing returns boolean
set HITS = HITS + 1
if HITS == 1 then
if FirstOfGroup(TARGET) != null then
set OK = false
endif
else
if FirstOfGroup(TARGET) != A then
set OK = false
endif
endif
if IsUnitInGroup(GetFilterUnit(), TARGET) then
set OK = false
endif
return GetOwningPlayer(GetFilterUnit()) == Player(4)
endfunction
function run takes nothing returns nothing
call GroupAddUnit(TARGET, B)
call GroupAddUnit(OTHER, B)
call GroupEnumUnitsInRect(TARGET, BOUNDS, Filter(function OwnerPredicate))
endfunction
"#,
        8,
        "",
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(3));
    assert_eq!(v.global("OK"), Value::Bool(true));
    let group = v.global("TARGET");
    let other = v.global("OTHER");
    for (name, wanted) in [("A", true), ("B", false), ("C", true)] {
        let unit = v.global(name);
        assert_eq!(
            v.host()
                .inner
                .native("IsUnitInGroup", &[unit, group.clone()])
                .unwrap(),
            Value::Bool(wanted)
        );
    }
    let b = v.global("B");
    assert_eq!(
        v.host().inner.native("IsUnitInGroup", &[b, other]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
}

#[test]
fn nested_rectangles_restore_the_outer_candidate_including_empty_inner_calls() {
    let mut v = vm(
        r#"
function InnerPredicate takes nothing returns boolean
set INNER_HITS = INNER_HITS + 1
if GetFilterUnit() != A and GetFilterUnit() != B and GetFilterUnit() != C then
set OK = false
endif
return GetFilterUnit() == B
endfunction
function OuterPredicate takes nothing returns boolean
local unit before = GetFilterUnit()
call GroupEnumUnitsInRect(INNER, BOUNDS, Filter(function InnerPredicate))
if GetFilterUnit() != before then
set OK = false
endif
call GroupEnumUnitsInRect(INNER, EMPTY, Filter(function InnerPredicate))
if GetFilterUnit() != before then
set OK = false
endif
return true
endfunction
function run takes nothing returns nothing
call GroupEnumUnitsInRect(TARGET, BOUNDS, Condition(function OuterPredicate))
endfunction
"#,
        8,
        "",
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("INNER_HITS"), Value::Int(9));
    assert_eq!(v.global("OK"), Value::Bool(true));
    let group = v.global("INNER");
    assert_eq!(
        v.host().inner.native("FirstOfGroup", &[group]).unwrap(),
        Value::Null
    );
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
}

#[test]
fn predicate_budget_failure_keeps_prior_effects_and_unwinds_context_before_reuse() {
    let mut v = vm(
        r#"
function run takes nothing returns nothing
call GroupEnumUnitsInRect(TARGET, BOUNDS, Filter(function TruePredicate))
endfunction
"#,
        1,
        "",
    );
    assert!(
        v.call("run", &[])
            .unwrap_err()
            .to_string()
            .contains("group membership")
    );
    assert_eq!(v.global("HITS"), Value::Int(2));
    let group = v.global("TARGET");
    let a = v.global("A");
    assert_eq!(
        v.host()
            .inner
            .native("FirstOfGroup", std::slice::from_ref(&group))
            .unwrap(),
        a
    );
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    v.call("clear_filtered", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(5));
    assert_eq!(
        v.host().inner.native("FirstOfGroup", &[group]).unwrap(),
        Value::Null
    );
}

#[test]
fn invalid_return_and_destroyed_target_do_not_leave_a_candidate_context() {
    let mut v = vm(
        r#"
function BadPredicate takes nothing returns boolean
return 7
endfunction
function DestroyPredicate takes nothing returns boolean
call DestroyGroup(TARGET)
return true
endfunction
function bad takes nothing returns nothing
call GroupEnumUnitsInRect(TARGET, BOUNDS, Filter(function BadPredicate))
endfunction
function destroy takes nothing returns nothing
call GroupEnumUnitsInRect(TARGET, BOUNDS, Filter(function DestroyPredicate))
endfunction
"#,
        8,
        "",
    );
    assert!(
        v.call("bad", &[])
            .unwrap_err()
            .to_string()
            .contains("predicate did not return boolean")
    );
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    assert!(v.call("destroy", &[]).is_err());
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(v.host().inner.memberships(), 0);
}

pub(super) const TRIGGERS: &str = r#"
type trigger extends agent
type event extends agent
type eventid extends handle
type unitevent extends eventid
type gameevent extends eventid
constant native ConvertUnitEvent takes integer i returns unitevent
constant native ConvertGameEvent takes integer i returns gameevent
native CreateTrigger takes nothing returns trigger
native DestroyTrigger takes trigger t returns nothing
native EnableTrigger takes trigger t returns nothing
native DisableTrigger takes trigger t returns nothing
native IsTriggerEnabled takes trigger t returns boolean
native TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event
native TriggerAddAction takes trigger t, code action returns triggeraction
type triggeraction extends handle
type triggercondition extends agent
native TriggerAddCondition takes trigger t, boolexpr c returns triggercondition
native TriggerRegisterTimerExpireEvent takes trigger t, timer which returns event
native TriggerRegisterTimerEvent takes trigger t, real timeout, boolean periodic returns event
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
native TriggerEvaluate takes trigger t returns boolean
native TriggerExecute takes trigger t returns nothing
native TimerStart takes timer t, real timeout, boolean periodic, code handler returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
globals
constant unitevent TEST_UNIT = ConvertUnitEvent(1)
constant gameevent TEST_TIMER = ConvertGameEvent(2)
endglobals
"#;

#[test]
fn rectangle_and_trigger_callbacks_share_one_lifo_stack_and_restore_both_contexts() {
    let mut v = vm(
        r#"
globals
trigger TOP = null
trigger NESTED = null
endglobals
function InnerPredicate takes nothing returns boolean
set INNER_HITS = INNER_HITS + 1
if GetTriggeringTrigger() != NESTED then
set OK = false
endif
return false
endfunction
function NestedCondition takes nothing returns boolean
if GetTriggeringTrigger() != NESTED or GetFilterUnit() != SAVED then
set OK = false
endif
call GroupEnumUnitsInRect(INNER, BOUNDS, Filter(function InnerPredicate))
return GetTriggeringTrigger() == NESTED and GetFilterUnit() == SAVED
endfunction
function NestedAction takes nothing returns nothing
if GetTriggeringTrigger() != NESTED or GetFilterUnit() != SAVED then
set OK = false
endif
call GroupEnumUnitsInRect(INNER, BOUNDS, Filter(function InnerPredicate))
if GetTriggeringTrigger() != NESTED or GetFilterUnit() != SAVED then
set OK = false
endif
endfunction
function OuterPredicate takes nothing returns boolean
set SAVED = GetFilterUnit()
if not TriggerEvaluate(NESTED) then
set OK = false
endif
call TriggerExecute(NESTED)
if GetTriggeringTrigger() != TOP or GetFilterUnit() != SAVED then
set OK = false
endif
return true
endfunction
function TopAction takes nothing returns nothing
call GroupEnumUnitsInRect(TARGET, BOUNDS, Filter(function OuterPredicate))
if GetTriggeringTrigger() != TOP then
set OK = false
endif
endfunction
function run takes nothing returns nothing
set TOP = CreateTrigger()
set NESTED = CreateTrigger()
call TriggerAddAction(TOP, function TopAction)
call TriggerAddAction(NESTED, function NestedAction)
call TriggerAddCondition(NESTED, Condition(function NestedCondition))
call TriggerExecute(TOP)
endfunction
"#,
        8,
        TRIGGERS,
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("OK"), Value::Bool(true));
    assert_eq!(v.global("INNER_HITS"), Value::Int(18));
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(
        v.host().inner.native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Null
    );
}

#[test]
fn mixed_callback_error_unwinds_trigger_and_unit_contexts_together() {
    let mut v = vm(
        r#"
globals
trigger TOP = null
endglobals
function BadPredicate takes nothing returns boolean
set HITS = HITS + 1
return 7
endfunction
function Action takes nothing returns nothing
call GroupEnumUnitsInRect(TARGET, BOUNDS, Filter(function BadPredicate))
endfunction
function run takes nothing returns nothing
set TOP = CreateTrigger()
call TriggerAddAction(TOP, function Action)
call TriggerExecute(TOP)
endfunction
"#,
        8,
        TRIGGERS,
    );
    assert!(
        v.call("run", &[])
            .unwrap_err()
            .to_string()
            .contains("predicate did not return boolean")
    );
    assert_eq!(v.global("HITS"), Value::Int(1));
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(
        v.host().inner.native("GetTriggeringTrigger", &[]).unwrap(),
        Value::Null
    );
    v.call("clear_filtered", &[]).unwrap();
}

#[test]
fn group_rectangle_and_trigger_contexts_are_independent_and_nested() {
    let mut v = vm(
        r#"
globals
trigger T = null
unit ENUM_SAVED = null
endglobals
function TriggerAction takes nothing returns nothing
if GetEnumUnit() != ENUM_SAVED or GetFilterUnit() != SAVED or GetTriggeringTrigger() != T then
set OK = false
endif
endfunction
function InnerEnum takes nothing returns nothing
if GetFilterUnit() != SAVED then
set OK = false
endif
set INNER_HITS = INNER_HITS + 1
endfunction
function Predicate takes nothing returns boolean
set SAVED = GetFilterUnit()
if GetEnumUnit() != ENUM_SAVED then
set OK = false
endif
call TriggerExecute(T)
call ForGroup(OTHER, function InnerEnum)
if GetEnumUnit() != ENUM_SAVED or GetFilterUnit() != SAVED then
set OK = false
endif
return true
endfunction
function OuterEnum takes nothing returns nothing
set ENUM_SAVED = GetEnumUnit()
if GetFilterUnit() != null then
set OK = false
endif
call GroupEnumUnitsInRect(INNER, BOUNDS, Filter(function Predicate))
if GetFilterUnit() != null or GetEnumUnit() != ENUM_SAVED then
set OK = false
endif
set HITS = HITS + 1
endfunction
function run takes nothing returns nothing
set T = CreateTrigger()
call TriggerAddAction(T, function TriggerAction)
call GroupAddUnit(TARGET, A)
call GroupAddUnit(TARGET, B)
call GroupAddUnit(OTHER, C)
call ForGroup(TARGET, function OuterEnum)
endfunction
"#,
        12,
        TRIGGERS,
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("OK"), Value::Bool(true));
    assert_eq!(v.global("HITS"), Value::Int(2));
    assert_eq!(v.global("INNER_HITS"), Value::Int(6));
    for name in ["GetEnumUnit", "GetFilterUnit", "GetTriggeringTrigger"] {
        assert_eq!(v.host().inner.native(name, &[]).unwrap(), Value::Null);
    }
}
