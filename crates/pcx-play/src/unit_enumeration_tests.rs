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
fn player_enumeration_uses_current_owner_including_hidden_but_not_removed_units() {
    let mut v = vm(
        r#"
function run takes nothing returns nothing
call GroupAddUnit(TARGET, C)
call SetUnitOwner(C, Player(5), false)
call ShowUnit(A, false)
call RemoveUnit(B)
call GroupEnumUnitsOfPlayer(TARGET, Player(4), null)
call GroupEnumUnitsOfPlayer(INNER, Player(5), null)
endfunction
"#,
        8,
        "native SetUnitOwner takes unit u, player p, boolean changeColor returns nothing\nnative ShowUnit takes unit u, boolean show returns nothing\nnative RemoveUnit takes unit u returns nothing\n",
    );
    v.call("run", &[]).unwrap();
    for (group, member) in [("TARGET", "A"), ("INNER", "C")] {
        let group = v.global(group);
        let member = v.global(member);
        assert_eq!(
            v.host()
                .inner
                .native("FirstOfGroup", std::slice::from_ref(&group))
                .unwrap(),
            member
        );
        v.host()
            .inner
            .native("GroupRemoveUnit", &[group.clone(), member])
            .unwrap();
        assert_eq!(
            v.host().inner.native("FirstOfGroup", &[group]).unwrap(),
            Value::Null
        );
    }
}

#[test]
fn nested_player_filters_restore_candidates_and_feed_the_real_group_callback() {
    let mut v = vm(
        r#"
function InnerOwner takes nothing returns boolean
set INNER_HITS = INNER_HITS + 1
if GetFilterUnit() != B then
set OK = false
endif
return true
endfunction
function OuterOwner takes nothing returns boolean
local unit before = GetFilterUnit()
set HITS = HITS + 1
if before != A and before != C then
set OK = false
endif
call GroupEnumUnitsOfPlayer(INNER, Player(5), Condition(function InnerOwner))
if GetFilterUnit() != before then
set OK = false
endif
return before == C
endfunction
function Visit takes nothing returns nothing
if GetEnumUnit() != C or GetFilterUnit() != null then
set OK = false
endif
set HITS = HITS + 10
endfunction
function run takes nothing returns nothing
call GroupAddUnit(TARGET, B)
call GroupEnumUnitsOfPlayer(TARGET, Player(4), Filter(function OuterOwner))
call ForGroup(TARGET, function Visit)
endfunction
"#,
        8,
        "",
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(12));
    assert_eq!(v.global("INNER_HITS"), Value::Int(2));
    assert_eq!(v.global("OK"), Value::Bool(true));
    for name in ["GetFilterUnit", "GetEnumUnit"] {
        assert_eq!(v.host().inner.native(name, &[]).unwrap(), Value::Null);
    }
}

#[test]
fn player_query_preflight_preserves_destination_on_budget_and_identity_failure() {
    let mut v = vm("", 1, "");
    let group = v.global("TARGET");
    let b = v.global("B");
    let owner = v.host().inner.native("Player", &[Value::Int(4)]).unwrap();
    v.host()
        .inner
        .native("GroupAddUnit", &[group.clone(), b.clone()])
        .unwrap();
    for subject in [owner, b.clone(), Value::Null] {
        assert!(
            v.host()
                .inner
                .native(
                    "GroupEnumUnitsOfPlayer",
                    &[group.clone(), subject, Value::Null]
                )
                .is_err()
        );
        assert_eq!(
            v.host()
                .inner
                .native("FirstOfGroup", std::slice::from_ref(&group))
                .unwrap(),
            b
        );
        assert_eq!(v.host().inner.memberships(), 1);
    }
}

#[test]
fn player_predicate_failure_names_its_native_and_unwinds_before_reuse() {
    let mut v = vm(
        r#"
function BadOwner takes nothing returns boolean
return 7
endfunction
function bad takes nothing returns nothing
call GroupEnumUnitsOfPlayer(TARGET, Player(4), Filter(function BadOwner))
endfunction
function run takes nothing returns nothing
call GroupEnumUnitsOfPlayer(TARGET, Player(5), null)
endfunction
"#,
        8,
        "",
    );
    let error = v.call("bad", &[]).unwrap_err().to_string();
    assert!(error.contains("GroupEnumUnitsOfPlayer"), "{error}");
    assert!(
        error.contains("predicate did not return boolean"),
        "{error}"
    );
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    v.call("run", &[]).unwrap();
    let group = v.global("TARGET");
    let b = v.global("B");
    assert_eq!(v.host().inner.native("FirstOfGroup", &[group]).unwrap(), b);
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

const RANGE_DECLARATIONS: &str = r#"
type location extends agent
native Location takes real x, real y returns location
native MoveLocation takes location l, real x, real y returns nothing
native RemoveLocation takes location l returns nothing
native GroupEnumUnitsInRange takes group g, real x, real y, real radius, boolexpr filter returns nothing
native GroupEnumUnitsInRangeOfLoc takes group g, location l, real radius, boolexpr filter returns nothing
native ShowUnit takes unit u, boolean show returns nothing
native RemoveUnit takes unit u returns nothing
"#;

fn range_vm(script: &str, members: usize) -> Vm {
    let host = position_host_program(
        100,
        members,
        &format!("{SCENE}\n{script}"),
        RANGE_DECLARATIONS,
    )
    .with_locations()
    .unwrap();
    let ctx = Arc::clone(host.player_context().unwrap());
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(host);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    vm
}

#[test]
fn range_enumeration_uses_centres_inclusive_edges_and_shown_extant_units() {
    let mut v = range_vm("", 16);
    let group = v.global("TARGET");
    let [a, b, c] = ["A", "B", "C"].map(|name| v.global(name));
    for (radius, expected) in [
        (0., [true, false, false]),
        (31.75, [true, false, false]),
        (32., [true, true, false]),
        (64., [true, true, true]),
    ] {
        v.host()
            .inner
            .native(
                "GroupEnumUnitsInRange",
                &[
                    group.clone(),
                    Value::Real(16.),
                    Value::Real(16.),
                    Value::Real(radius),
                    Value::Null,
                ],
            )
            .unwrap();
        for (unit, present) in [a.clone(), b.clone(), c.clone()].into_iter().zip(expected) {
            assert_eq!(
                v.host()
                    .inner
                    .native("IsUnitInGroup", &[unit, group.clone()])
                    .unwrap(),
                Value::Bool(present)
            );
        }
    }
    v.host()
        .inner
        .native("ShowUnit", &[b.clone(), Value::Bool(false)])
        .unwrap();
    v.host()
        .inner
        .native("RemoveUnit", std::slice::from_ref(&c))
        .unwrap();
    v.host()
        .inner
        .native(
            "GroupEnumUnitsInRange",
            &[
                group.clone(),
                Value::Real(16.),
                Value::Real(16.),
                Value::Real(90.),
                Value::Null,
            ],
        )
        .unwrap();
    assert_eq!(
        v.host()
            .inner
            .native("FirstOfGroup", std::slice::from_ref(&group))
            .unwrap(),
        a
    );
    assert_eq!(v.host().inner.memberships(), 1);
    v.host()
        .inner
        .native("ShowUnit", &[b.clone(), Value::Bool(true)])
        .unwrap();
    v.host()
        .inner
        .native(
            "GroupEnumUnitsInRange",
            &[
                group.clone(),
                Value::Real(48.),
                Value::Real(16.),
                Value::Real(0.),
                Value::Null,
            ],
        )
        .unwrap();
    assert_eq!(v.host().inner.native("FirstOfGroup", &[group]).unwrap(), b);
}

#[test]
fn range_location_snapshots_nested_predicates_restore_filter_context() {
    let mut v = range_vm(
        r#"
globals
location L = null
endglobals
function InnerRange takes nothing returns boolean
set INNER_HITS = INNER_HITS + 1
return GetFilterUnit() == B
endfunction
function OuterRange takes nothing returns boolean
local unit before = GetFilterUnit()
set HITS = HITS + 1
call MoveLocation(L, 700.0, 700.0)
call GroupEnumUnitsInRange(INNER, 48.0, 16.0, 0.0, Condition(function InnerRange))
call GroupEnumUnitsInRangeOfLoc(INNER, null, 30.0, Condition(function FalsePredicate))
if GetFilterUnit() != before or FirstOfGroup(INNER) != B then
set OK = false
endif
return before == B
endfunction
function run takes nothing returns nothing
set L = Location(16.0, 16.0)
call GroupEnumUnitsInRangeOfLoc(TARGET, L, 32.0, Filter(function OuterRange))
endfunction
"#,
        16,
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(2));
    assert_eq!(v.global("INNER_HITS"), Value::Int(2));
    assert_eq!(v.global("OK"), Value::Bool(true));
    let group = v.global("TARGET");
    assert_eq!(
        v.host().inner.native("FirstOfGroup", &[group]).unwrap(),
        v.global("B")
    );
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
}

#[test]
fn range_preflight_failures_preserve_members_and_do_not_install_callbacks() {
    let mut v = range_vm("", 1);
    let group = v.global("TARGET");
    let unit = v.global("B");
    v.host()
        .inner
        .native("GroupAddUnit", &[group.clone(), unit.clone()])
        .unwrap();
    for radius in [90., -1., f32::NAN, f32::INFINITY] {
        assert!(
            v.host()
                .inner
                .native(
                    "GroupEnumUnitsInRange",
                    &[
                        group.clone(),
                        Value::Real(16.),
                        Value::Real(16.),
                        Value::Real(radius),
                        Value::Null
                    ]
                )
                .is_err()
        );
        assert_eq!(
            v.host()
                .inner
                .native("FirstOfGroup", std::slice::from_ref(&group))
                .unwrap(),
            unit
        );
        assert_eq!(
            v.host().inner.native("GetFilterUnit", &[]).unwrap(),
            Value::Null
        );
    }
    for coordinate in [f32::NAN, f32::INFINITY] {
        assert!(
            v.host()
                .inner
                .native(
                    "GroupEnumUnitsInRange",
                    &[
                        group.clone(),
                        Value::Real(coordinate),
                        Value::Real(16.),
                        Value::Real(32.),
                        Value::Null
                    ]
                )
                .is_err()
        );
        assert_eq!(
            v.host()
                .inner
                .native("FirstOfGroup", std::slice::from_ref(&group))
                .unwrap(),
            unit
        );
    }
    assert!(
        v.host()
            .inner
            .native(
                "GroupEnumUnitsInRangeOfLoc",
                &[group.clone(), unit.clone(), Value::Real(4.), Value::Null]
            )
            .is_err()
    );
    v.host()
        .inner
        .native(
            "GroupEnumUnitsInRangeOfLoc",
            &[group.clone(), Value::Null, Value::Real(4.), Value::Null],
        )
        .unwrap();
    assert_eq!(
        v.host().inner.native("FirstOfGroup", &[group]).unwrap(),
        unit
    );
}

#[test]
fn range_bad_predicate_unwinds_before_a_successful_location_query() {
    let mut v = range_vm(
        r#"
function BadRange takes nothing returns boolean
return 7
endfunction
function bad takes nothing returns nothing
call GroupEnumUnitsInRange(TARGET,16.0,16.0,32.0,Filter(function BadRange))
endfunction
function run takes nothing returns nothing
local location l=Location(80.0,16.0)
call GroupEnumUnitsInRangeOfLoc(TARGET,l,0.0,Condition(function TruePredicate))
endfunction
"#,
        8,
    );
    let error = v.call("bad", &[]).unwrap_err().to_string();
    assert!(error.contains("GroupEnumUnitsInRange"), "{error}");
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    v.call("run", &[]).unwrap();
    let group = v.global("TARGET");
    assert_eq!(
        v.host().inner.native("FirstOfGroup", &[group]).unwrap(),
        v.global("C")
    );
    assert_eq!(v.global("HITS"), Value::Int(1));
}

#[test]
fn range_accepted_positions_are_canonical_and_missing_positions_refuse_atomically() {
    let mut v = range_vm("", 8);
    let group = v.global("TARGET");
    let a = v.global("A");
    let Value::Handle(id) = a else {
        panic!("unit");
    };
    v.host().inner.range_test_position(id, Some([160., 160.]));
    for (point, expected) in [([16., 16.], Value::Null), ([160., 160.], a.clone())] {
        v.host()
            .inner
            .native(
                "GroupEnumUnitsInRange",
                &[
                    group.clone(),
                    Value::Real(point[0]),
                    Value::Real(point[1]),
                    Value::Real(0.),
                    Value::Null,
                ],
            )
            .unwrap();
        assert_eq!(
            v.host()
                .inner
                .native("FirstOfGroup", std::slice::from_ref(&group))
                .unwrap(),
            expected
        );
    }
    v.host().inner.range_test_position(id, None);
    assert!(
        v.host()
            .inner
            .native(
                "GroupEnumUnitsInRange",
                &[
                    group.clone(),
                    Value::Real(16.),
                    Value::Real(16.),
                    Value::Real(0.),
                    Value::Null
                ]
            )
            .is_err()
    );
    assert_eq!(v.host().inner.native("FirstOfGroup", &[group]).unwrap(), a);
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
}

const TYPE_COUNT_DECLARATIONS: &str = r#"
native GroupEnumUnitsOfType takes group g, string name, boolexpr filter returns nothing
native GroupEnumUnitsOfTypeCounted takes group g, string name, boolexpr filter, integer limit returns nothing
native GroupEnumUnitsInRectCounted takes group g, rect r, boolexpr filter, integer limit returns nothing
native GroupEnumUnitsInRangeCounted takes group g, real x, real y, real radius, boolexpr filter, integer limit returns nothing
native GroupEnumUnitsInRangeOfLocCounted takes group g, location l, real radius, boolexpr filter, integer limit returns nothing
"#;

fn type_count_vm(script: &str, members: usize) -> Vm {
    use pcx_objects::editor::defaults::{DefaultTables, Identity};
    let ui = b"ID;PWXL;N;E\r\nC;X1;Y1;K\"unitUIID\"\r\nC;X2;K\"name\"\r\nC;X1;Y2;K\"U001\"\r\nC;X2;K\"Quill\"\r\nC;X1;Y3;K\"U002\"\r\nC;X2;K\"TRIGSTR_24\"\r\nC;X1;Y4;K\"U003\"\r\nC;X2;K\"Twin\"\r\nC;X1;Y5;K\"U004\"\r\nC;X2;K\"twin\"\r\nE\r\n";
    let tables = Arc::new(
        DefaultTables::from_bytes(
            Identity {
                install: "synthetic".into(),
                edition_version: "fixture".into(),
            },
            &[(SlkId::UnitUi.path(), ui.as_slice())],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 1,
                max_asset_bytes: 4096,
                max_aggregate_input_bytes: 4096,
            },
        )
        .unwrap(),
    );
    let host = position_host_program(
        100,
        members,
        &format!("{SCENE}\n{script}"),
        &format!("{RANGE_DECLARATIONS}\n{TYPE_COUNT_DECLARATIONS}"),
    )
    .with_locations()
    .unwrap()
    .with_unit_id_lookup(tables)
    .unwrap();
    let ctx = Arc::clone(host.player_context().unwrap());
    let mut v = Arc::clone(&ctx.scripts).owned_vm(host);
    v.init_globals().unwrap();
    v.call("setup", &[]).unwrap();
    v
}

fn membership(v: &mut Vm, group: &str) -> Vec<Value> {
    let group = v.global(group);
    ["A", "B", "C"]
        .into_iter()
        .filter_map(|name| {
            let unit = v.global(name);
            (v.host()
                .inner
                .native("IsUnitInGroup", &[unit.clone(), group.clone()])
                .unwrap()
                == Value::Bool(true))
            .then_some(unit)
        })
        .collect()
}

#[test]
fn type_enumeration_uses_raw_names_and_current_type_including_hidden_not_removed() {
    let mut v = type_count_vm(
        r#"
function replace_b takes nothing returns nothing
call RemoveUnit(B)
set B = CreateUnit(Player(5), 'U002', 48.0, 16.0, 0.0)
endfunction
function raw_argument takes string name returns nothing
call GroupEnumUnitsOfType(TARGET, name, null)
endfunction
function computed_name takes nothing returns nothing
call GroupEnumUnitsOfType(TARGET, "TRIG" + "STR_24", null)
endfunction
"#,
        12,
    );
    let a = v.global("A");
    v.host()
        .inner
        .native("ShowUnit", &[a, Value::Bool(false)])
        .unwrap();
    v.call("replace_b", &[]).unwrap();
    v.call("raw_argument", &[Value::Str("qUiLl".into())])
        .unwrap();
    assert_eq!(
        membership(&mut v, "TARGET"),
        vec![v.global("A"), v.global("C")]
    );
    v.call("computed_name", &[]).unwrap();
    assert_eq!(membership(&mut v, "TARGET"), vec![v.global("B")]);
    v.call("raw_argument", &[Value::Str("TRIGSTR_24".into())])
        .unwrap();
    assert_eq!(membership(&mut v, "TARGET"), vec![v.global("B")]);
    let b = v.global("B");
    v.host().inner.native("RemoveUnit", &[b]).unwrap();
    v.call("raw_argument", &[Value::Str("TRIGSTR_24".into())])
        .unwrap();
    assert!(membership(&mut v, "TARGET").is_empty());
    assert!(
        v.host()
            .inner
            .native("UnitId", &[Value::Str("Quill".into())])
            .is_err(),
        "OfType admission must not admit undeclared UnitId"
    );
}

#[test]
fn unknown_type_and_null_subject_short_circuit_before_group_filter_and_count() {
    let mut v = type_count_vm("", 12);
    let group = v.global("TARGET");
    let a = v.global("A");
    v.host()
        .inner
        .native("GroupAddUnit", &[group.clone(), a.clone()])
        .unwrap();
    for text in [
        Value::Null,
        Value::Str("unknown".into()),
        Value::Str(" Quill".into()),
        Value::Str("U001".into()),
    ] {
        for destination in [group.clone(), Value::Int(77)] {
            v.host()
                .inner
                .native(
                    "GroupEnumUnitsOfTypeCounted",
                    &[destination, text.clone(), Value::Int(9), Value::Null],
                )
                .unwrap();
        }
        assert_eq!(membership(&mut v, "TARGET"), vec![a.clone()]);
    }
    v.host()
        .inner
        .native(
            "GroupEnumUnitsInRangeOfLocCounted",
            &[
                Value::Int(77),
                Value::Null,
                Value::Null,
                Value::Int(9),
                Value::Null,
            ],
        )
        .unwrap();
    v.host()
        .inner
        .native(
            "GroupEnumUnitsInRectCounted",
            &[group.clone(), Value::Null, Value::Int(9), Value::Null],
        )
        .unwrap();
    let error = v
        .host()
        .inner
        .native(
            "GroupEnumUnitsOfTypeCounted",
            &[group, Value::Str("Quill".into()), Value::Null, Value::Null],
        )
        .unwrap_err();
    assert!(error.contains("GroupEnumUnitsOfTypeCounted"), "{error}");
    assert_eq!(membership(&mut v, "TARGET"), vec![a]);
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
}

#[test]
fn all_counted_queries_treat_zero_as_unlimited_and_negative_as_unsigned() {
    let mut v = type_count_vm("", 16);
    let group = v.global("TARGET");
    let bounds = v.global("BOUNDS");
    let loc = v
        .host()
        .inner
        .native("Location", &[Value::Real(48.), Value::Real(16.)])
        .unwrap();
    let queries = [
        (
            "GroupEnumUnitsOfTypeCounted",
            vec![group.clone(), Value::Str("Quill".into()), Value::Null],
        ),
        (
            "GroupEnumUnitsInRectCounted",
            vec![group.clone(), bounds, Value::Null],
        ),
        (
            "GroupEnumUnitsInRangeCounted",
            vec![
                group.clone(),
                Value::Real(48.),
                Value::Real(16.),
                Value::Real(64.),
                Value::Null,
            ],
        ),
        (
            "GroupEnumUnitsInRangeOfLocCounted",
            vec![group, loc, Value::Real(64.), Value::Null],
        ),
    ];
    for (name, args) in queries {
        for (count, expected) in [(1, 1), (2, 2), (0, 3), (-1, 3), (i32::MIN, 3)] {
            let mut args = args.clone();
            args.push(Value::Int(count));
            v.host().inner.native(name, &args).unwrap();
            assert_eq!(
                membership(&mut v, "TARGET").len(),
                expected,
                "{name}: {count}"
            );
        }
    }
}

#[test]
fn counted_predicate_counts_only_new_insertions_and_restores_nested_filter_context() {
    let mut v = type_count_vm(
        r#"
function nested takes nothing returns boolean
set INNER_HITS = INNER_HITS + 1
return true
endfunction
function outer takes nothing returns boolean
local unit before = GetFilterUnit()
set HITS = HITS + 1
call GroupEnumUnitsInRectCounted(INNER, BOUNDS, Condition(function nested), 1)
if GetFilterUnit() != before then
set OK = false
endif
if before == A then
return false
endif
if before == B then
call GroupAddUnit(TARGET, B)
endif
return true
endfunction
function run takes nothing returns nothing
call GroupEnumUnitsOfTypeCounted(TARGET, "Quill", Filter(function outer), 1)
endfunction
"#,
        16,
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(3));
    assert_eq!(v.global("INNER_HITS"), Value::Int(3));
    assert_eq!(v.global("OK"), Value::Bool(true));
    assert_eq!(
        membership(&mut v, "TARGET"),
        vec![v.global("B"), v.global("C")]
    );
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
}

#[test]
fn counted_early_exit_stops_predicate_effects_and_preflight_uses_capped_membership() {
    let mut v = type_count_vm(
        r#"
function run takes nothing returns nothing
call GroupEnumUnitsInRangeCounted(TARGET, 48.0, 16.0, 64.0, Condition(function TruePredicate), 1)
endfunction
"#,
        1,
    );
    v.call("run", &[]).unwrap();
    assert_eq!(v.global("HITS"), Value::Int(1));
    let group = v.global("TARGET");
    v.host()
        .inner
        .native(
            "GroupEnumUnitsOfTypeCounted",
            &[
                group.clone(),
                Value::Str("Quill".into()),
                Value::Null,
                Value::Int(1),
            ],
        )
        .unwrap();
    assert_eq!(membership(&mut v, "TARGET"), vec![v.global("A")]);
    let error = v
        .host()
        .inner
        .native(
            "GroupEnumUnitsOfTypeCounted",
            &[
                group,
                Value::Str("Quill".into()),
                Value::Null,
                Value::Int(0),
            ],
        )
        .unwrap_err();
    assert!(error.contains("group membership"), "{error}");
    assert_eq!(membership(&mut v, "TARGET"), vec![v.global("A")]);
}

#[test]
fn counted_callback_failures_unwind_and_ambiguous_names_preserve_destination() {
    let mut v = type_count_vm(
        r#"
function bad takes nothing returns boolean
return 7
endfunction
function invalid_return takes nothing returns nothing
call GroupEnumUnitsOfTypeCounted(TARGET, "Quill", Filter(function bad), 1)
endfunction
function destroy takes nothing returns boolean
call DestroyGroup(TARGET)
return true
endfunction
function destroyed takes nothing returns nothing
call GroupEnumUnitsInRectCounted(TARGET, BOUNDS, Filter(function destroy), 1)
endfunction
"#,
        8,
    );
    let group = v.global("TARGET");
    let a = v.global("A");
    v.host()
        .inner
        .native("GroupAddUnit", &[group.clone(), a.clone()])
        .unwrap();
    let error = v
        .host()
        .inner
        .native(
            "GroupEnumUnitsOfType",
            &[group, Value::Str("Twin".into()), Value::Null],
        )
        .unwrap_err();
    assert!(
        error.contains("GroupEnumUnitsOfType") && error.contains("shared by several"),
        "{error}"
    );
    assert_eq!(membership(&mut v, "TARGET"), vec![a]);
    assert!(
        v.call("invalid_return", &[])
            .unwrap_err()
            .to_string()
            .contains("predicate did not return boolean")
    );
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    v.call("clear_filtered", &[]).unwrap();
    assert!(v.call("destroyed", &[]).is_err());
    assert_eq!(
        v.host().inner.native("GetFilterUnit", &[]).unwrap(),
        Value::Null
    );
    assert_eq!(v.host().inner.memberships(), 0);
}
