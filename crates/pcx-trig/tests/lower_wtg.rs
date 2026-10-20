//! Synthetic `war3map.wtg` version 4 files lowered against a synthetic
//! TriggerData. Exact JASS text is asserted; no original asset is read here.

use pcx_trig::catalog::Catalog;
use pcx_trig::lower::{LowerError, lower};
use pcx_trig::{TriggerArity, Wct, Wtg};

const TRIGGER_DATA: &str = "\
[TriggerTypes]
boolean=0,1,1,WESTRING_boolean
real=0,1,1,WESTRING_real
string=0,1,1,WESTRING_string
integer=0,1,1,WESTRING_integer
unit=0,1,1,WESTRING_unit
abilcode=0,1,1,WESTRING_abilcode,integer
[TriggerTypeDefaults]
boolean=false,WESTRING_FALSE
integer=0
[TriggerParams]
RealHalf=0,real,0.5,WESTRING_HALF
Num1=0,integer,1,WESTRING_ONE
PlayerOne=0,player,Player(0),WESTRING_P1
OperatorAdd=0,ArithmeticOperator,\"+\",WESTRING_ADD
OperatorEqual=0,ComparisonOperator,\"==\",WESTRING_EQ
OperatorEqualENE=0,EqualNotEqualOperator,\"==\",WESTRING_EQ
[TriggerEvents]
TriggerRegisterTimerEventSingle=0,real
MapInitializationEvent=0,nothing
[TriggerConditions]
OperatorCompareInteger=0,integer,ComparisonOperator,integer
[TriggerActions]
DoNothing=0,nothing
DisplayTextToPlayer=0,player,string
RunLater=0,integer,code
Gate=0,boolexpr
ActionAlias=0,integer
_ActionAlias_ScriptName=RealActionFn
IfThenElse=0,boolexpr,code,code
ForLoopA=0,integer,integer,code
ForLoopVar=1,integervar,integer,integer,code
CommentString=0,scriptcode
SetVariable=0,AnyGlobal,Null
ReturnAction=0,nothing
ConditionalTriggerExecute=0,integer
SetUnitX=0,unit,real
CreateUnitAt=0,player,abilcode,real
[TriggerCalls]
OperatorInt=0,1,integer,integer,ArithmeticOperator,integer
";

fn catalog() -> Catalog {
    Catalog::parse(
        "synthetic-TriggerData.txt",
        "synthetic",
        "test",
        TRIGGER_DATA,
    )
    .unwrap()
}

fn u(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}

fn s(v: &mut Vec<u8>, text: &str) {
    v.extend_from_slice(text.as_bytes());
    v.push(0);
}

fn variable(name: &str, ty: &str, initial: Option<&str>) -> Vec<u8> {
    let mut v = Vec::new();
    s(&mut v, name);
    s(&mut v, ty);
    u(&mut v, 1); // unknown word
    u(&mut v, 0); // is array
    u(&mut v, initial.is_some() as u32);
    s(&mut v, initial.unwrap_or(""));
    v
}

fn preset(key: &str) -> Vec<u8> {
    param(0, key, None, None)
}

fn variable_ref(name: &str) -> Vec<u8> {
    param(1, name, None, None)
}

fn string(text: &str) -> Vec<u8> {
    param(3, text, None, None)
}

/// A function-typed parameter. Nested calls name themselves as the value;
/// wrapped conditions and actions have an empty value.
fn call(sub_kind: u32, name: &str, params: Vec<Vec<u8>>) -> Vec<u8> {
    let value = if sub_kind == 3 { name } else { "" };
    param(2, value, Some(sub(sub_kind, name, params)), None)
}

fn param(kind: u32, value: &str, sub: Option<Vec<u8>>, index: Option<Vec<u8>>) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, kind);
    s(&mut v, value);
    u(&mut v, sub.is_some() as u32);
    if let Some(sub) = sub {
        v.extend_from_slice(&sub);
    }
    match index {
        Some(index) => {
            u(&mut v, 1);
            v.extend_from_slice(&index);
        }
        None => u(&mut v, 0),
    }
    v
}

fn sub(kind: u32, name: &str, params: Vec<Vec<u8>>) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, kind);
    s(&mut v, name);
    u(&mut v, 1); // flag
    for p in params {
        v.extend_from_slice(&p);
    }
    v
}

fn eca(kind: u32, name: &str, enabled: bool, params: Vec<Vec<u8>>) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, kind);
    s(&mut v, name);
    u(&mut v, enabled as u32);
    for p in params {
        v.extend_from_slice(&p);
    }
    v
}

struct Trig {
    name: &'static str,
    enabled: bool,
    custom: bool,
    initially_off: bool,
    run_on_init: bool,
    ecas: Vec<Vec<u8>>,
}

impl Trig {
    fn new(name: &'static str, ecas: Vec<Vec<u8>>) -> Self {
        Self {
            name,
            enabled: true,
            custom: false,
            initially_off: false,
            run_on_init: false,
            ecas,
        }
    }
}

fn trigger(t: &Trig) -> Vec<u8> {
    let mut v = Vec::new();
    s(&mut v, t.name);
    s(&mut v, "");
    u(&mut v, t.enabled as u32);
    u(&mut v, t.custom as u32);
    u(&mut v, t.initially_off as u32);
    u(&mut v, t.run_on_init as u32);
    u(&mut v, 1); // category
    u(&mut v, t.ecas.len() as u32);
    for e in &t.ecas {
        v.extend_from_slice(e);
    }
    v
}

fn wtg(variables: &[Vec<u8>], triggers: &[Trig]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"WTG!");
    u(&mut v, 4);
    u(&mut v, 1); // categories
    u(&mut v, 1);
    s(&mut v, "Cat");
    u(&mut v, 1); // unknown root
    u(&mut v, variables.len() as u32);
    for var in variables {
        v.extend_from_slice(var);
    }
    u(&mut v, triggers.len() as u32);
    for t in triggers {
        v.extend_from_slice(&trigger(t));
    }
    v
}

fn lower_with(bytes: &[u8], blocks: &[&[u8]]) -> Result<String, LowerError> {
    let catalog = catalog();
    let arity: TriggerArity = catalog.arity().unwrap();
    let parsed = Wtg::parse(bytes, &arity).unwrap();
    let wct_bytes = wct(blocks);
    let wct = Wct::parse(&wct_bytes).unwrap();
    lower(&parsed, &wct, &catalog)
}

/// A WCT with the given text blocks: a zero word, the count, then length-prefixed blocks.
fn wct(blocks: &[&[u8]]) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, 0);
    u(&mut v, blocks.len() as u32);
    for block in blocks {
        u(&mut v, block.len() as u32);
        v.extend_from_slice(block);
    }
    v
}

/// Lowers with one empty WCT block per trigger, as for a map with no custom text.
fn lower_bytes(bytes: &[u8]) -> Result<String, LowerError> {
    let catalog = catalog();
    let arity: TriggerArity = catalog.arity().unwrap();
    let count = Wtg::parse(bytes, &arity).unwrap().triggers().len();
    let empty: Vec<&[u8]> = vec![&[]; count];
    lower_with(bytes, &empty)
}

fn sum_eca() -> Vec<u8> {
    eca(
        2,
        "SetVariable",
        true,
        vec![
            variable_ref("Count"),
            call(
                3,
                "OperatorInt",
                vec![variable_ref("Count"), preset("OperatorAdd"), preset("Num1")],
            ),
        ],
    )
}

fn lone(name: &'static str, ecas: Vec<Vec<u8>>) -> Vec<u8> {
    wtg(
        &[variable("Count", "integer", None)],
        &[Trig::new(name, ecas)],
    )
}

/// The `RunInitializationTriggers` body, which is empty for maps without map-init triggers.
const EMPTY_RUN_INITIALIZATION: &str =
    "function RunInitializationTriggers takes nothing returns nothing\nendfunction\n";

#[test]
fn lowers_events_conditions_actions_and_wrappers_to_exact_jass() {
    let variables = [
        variable("Count", "integer", Some("5")),
        variable("Flag", "boolean", None),
        variable("Who", "unit", None),
    ];
    let trig = Trig::new(
        "Hello World",
        vec![
            eca(
                0,
                "TriggerRegisterTimerEventSingle",
                true,
                vec![preset("RealHalf")],
            ),
            eca(
                1,
                "OperatorCompareInteger",
                true,
                vec![
                    variable_ref("Count"),
                    preset("OperatorEqual"),
                    preset("Num1"),
                ],
            ),
            sum_eca(),
            eca(
                2,
                "DisplayTextToPlayer",
                true,
                vec![preset("PlayerOne"), string("hi there")],
            ),
            eca(
                2,
                "RunLater",
                true,
                vec![preset("Num1"), call(2, "DoNothing", vec![])],
            ),
            eca(
                2,
                "Gate",
                true,
                vec![call(
                    1,
                    "OperatorCompareInteger",
                    vec![
                        variable_ref("Count"),
                        preset("OperatorEqual"),
                        preset("Num1"),
                    ],
                )],
            ),
            eca(2, "CommentString", true, vec![string("note")]),
        ],
    );
    let jass = lower_bytes(&wtg(&variables, &[trig])).unwrap();
    let expected = "\
globals
    integer udg_Count
    boolean udg_Flag
    unit udg_Who
    trigger gg_trg_Hello_World
endglobals

function InitGlobals takes nothing returns nothing
    set udg_Count = 5
    set udg_Flag = false
endfunction

function Trig_Hello_World_Func001A takes nothing returns nothing
    call DoNothing()
endfunction

function Trig_Hello_World_Func002C takes nothing returns boolean
    return (udg_Count == 1)
endfunction

function Trig_Hello_World_Actions takes nothing returns nothing
    set udg_Count = (udg_Count + 1)
    call DisplayTextToPlayer(Player(0), \"hi there\")
    call RunLater(1, function Trig_Hello_World_Func001A)
    call Gate(Condition(function Trig_Hello_World_Func002C))
    // note
endfunction

function Trig_Hello_World_Conditions takes nothing returns boolean
    if (not (udg_Count == 1)) then
        return false
    endif
    return true
endfunction

function InitTrig_Hello_World takes nothing returns nothing
    set gg_trg_Hello_World = CreateTrigger()
    call TriggerRegisterTimerEventSingle(gg_trg_Hello_World, 0.5)
    call TriggerAddCondition(gg_trg_Hello_World, Condition(function Trig_Hello_World_Conditions))
    call TriggerAddAction(gg_trg_Hello_World, function Trig_Hello_World_Actions)
endfunction

function InitCustomTriggers takes nothing returns nothing
    call InitTrig_Hello_World()
endfunction

function RunInitializationTriggers takes nothing returns nothing
endfunction
";
    assert_eq!(jass, expected);
}

#[test]
fn map_initialization_runs_from_run_initialization_triggers_without_registering() {
    let jass = lower_bytes(&lone(
        "Lobby",
        vec![
            eca(0, "MapInitializationEvent", true, vec![]),
            eca(
                2,
                "ConditionalTriggerExecute",
                true,
                vec![variable_ref("gg_trg_Other")],
            ),
        ],
    ))
    .unwrap();
    assert!(
        jass.contains("call ConditionalTriggerExecute(gg_trg_Other)\n"),
        "{jass}"
    );
    assert!(
        jass.ends_with(
            "function RunInitializationTriggers takes nothing returns nothing\n    call ConditionalTriggerExecute(gg_trg_Lobby)\nendfunction\n"
        ),
        "{jass}"
    );
    assert!(!jass.contains("TriggerRegister"), "{jass}");
}

#[test]
fn events_are_registered_before_actions_whatever_the_file_order() {
    // The file groups ECAs by kind but does not place events first.
    let jass = lower_bytes(&lone(
        "Late",
        vec![
            eca(2, "DoNothing", true, vec![]),
            eca(
                0,
                "TriggerRegisterTimerEventSingle",
                true,
                vec![preset("RealHalf")],
            ),
        ],
    ))
    .unwrap();
    assert!(
        jass.contains(
            "    set gg_trg_Late = CreateTrigger()\n    call TriggerRegisterTimerEventSingle(gg_trg_Late, 0.5)\n    call TriggerAddAction(gg_trg_Late, function Trig_Late_Actions)\n"
        ),
        "{jass}"
    );
}

#[test]
fn constructs_the_editor_expands_are_refused_with_their_trigger_name() {
    let mut custom = Trig::new("Scripted", vec![]);
    custom.custom = true;
    let mut on_init = Trig::new("Startup", vec![]);
    on_init.run_on_init = true;
    let for_loop = eca(
        2,
        "ForLoopVar",
        true,
        vec![
            variable_ref("Count"),
            preset("Num1"),
            preset("Num1"),
            call(2, "DoNothing", vec![]),
        ],
    );
    let cases = [
        (
            Trig::new("Loop", vec![for_loop]),
            "editor-expanded ForLoopVar",
        ),
        (on_init, "run-on-initialization trigger"),
        (
            Trig::new(
                "Both",
                vec![
                    eca(0, "MapInitializationEvent", true, vec![]),
                    eca(
                        0,
                        "TriggerRegisterTimerEventSingle",
                        true,
                        vec![preset("RealHalf")],
                    ),
                ],
            ),
            "map initialization combined with other events",
        ),
    ];
    for (trig, construct) in cases {
        let name = trig.name;
        let error = lower_bytes(&wtg(&[variable("Count", "integer", None)], &[trig])).unwrap_err();
        assert_eq!(
            error,
            LowerError::Unsupported {
                context: name.into(),
                construct: construct.into(),
            }
        );
    }
}

#[test]
fn unknown_references_and_disabled_actions_are_refused_not_guessed() {
    let unknown_preset = lone(
        "Typo",
        vec![eca(
            2,
            "DisplayTextToPlayer",
            true,
            vec![preset("NoSuchPreset"), string("x")],
        )],
    );
    assert_eq!(
        lower_bytes(&unknown_preset).unwrap_err(),
        LowerError::UnknownPreset {
            context: "Typo".into(),
            key: "NoSuchPreset".into(),
        }
    );
    let undeclared = lone(
        "Ghost",
        vec![eca(
            2,
            "SetVariable",
            true,
            vec![variable_ref("Missing"), preset("Num1")],
        )],
    );
    assert!(matches!(
        lower_bytes(&undeclared).unwrap_err(),
        LowerError::Malformed { context, reason } if context == "Ghost" && reason.contains("Missing")
    ));
    let quoted = lone(
        "Quote",
        vec![eca(
            2,
            "DisplayTextToPlayer",
            true,
            vec![preset("PlayerOne"), string("a\"b")],
        )],
    );
    let jass = lower_bytes(&quoted).unwrap();
    assert!(
        jass.contains(r#"call DisplayTextToPlayer(Player(0), "a\"b")"#),
        "{jass}"
    );
    let literal = jass
        .lines()
        .find(|l| l.contains("DisplayTextToPlayer"))
        .unwrap();
    let literal = &literal[literal.find('"').unwrap()..literal.len() - 1];
    let lexed = pcx_script::lex(literal).unwrap();
    let decoded = match &lexed.tokens[0].kind {
        pcx_script::Tok::Str(s) => s.decoded(),
        other => panic!("not a string literal: {other:?}"),
    };
    assert_eq!(decoded, "a\"b");
    let mystery_type = wtg(&[variable("Odd", "mystery", None)], &[]);
    assert!(matches!(
        lower_bytes(&mystery_type).unwrap_err(),
        LowerError::Malformed { context, reason } if context == "variable Odd" && reason.contains("mystery")
    ));
}

#[test]
fn trigger_handles_and_object_globals_pass_through_unprefixed() {
    let jass = lower_bytes(&lone(
        "Pass",
        vec![eca(
            2,
            "DisplayTextToPlayer",
            true,
            vec![preset("PlayerOne"), variable_ref("gg_unit_h001_0001")],
        )],
    ))
    .unwrap();
    assert!(
        jass.contains("call DisplayTextToPlayer(Player(0), gg_unit_h001_0001)\n"),
        "{jass}"
    );
}

#[test]
fn disabled_triggers_keep_their_handle_declaration_and_emit_no_functions() {
    let mut disabled = Trig::new(
        "Off",
        vec![eca(
            2,
            "DisplayTextToPlayer",
            true,
            vec![preset("Nope"), string("x")],
        )],
    );
    disabled.enabled = false;
    let jass = lower_bytes(&wtg(&[], &[disabled])).unwrap();
    assert_eq!(
        jass,
        format!(
            "globals\n    trigger gg_trg_Off\nendglobals\n\nfunction InitGlobals takes nothing returns nothing\nendfunction\n\nfunction InitCustomTriggers takes nothing returns nothing\nendfunction\n\n{EMPTY_RUN_INITIALIZATION}"
        )
    );
}

#[test]
fn initially_off_trigger_is_disabled_right_after_creation() {
    let mut parked = Trig::new(
        "Sleepy Dog",
        vec![eca(2, "ActionAlias", true, vec![preset("Num1")])],
    );
    parked.initially_off = true;
    let jass = lower_bytes(&wtg(&[], &[parked])).unwrap();
    assert!(jass.contains("call RealActionFn(1)\n"), "{jass}");
    assert!(
        jass.contains(
            "function InitTrig_Sleepy_Dog takes nothing returns nothing\n    set gg_trg_Sleepy_Dog = CreateTrigger()\n    call DisableTrigger(gg_trg_Sleepy_Dog)\n    call TriggerAddAction(gg_trg_Sleepy_Dog, function Trig_Sleepy_Dog_Actions)\nendfunction\n"
        ),
        "{jass}"
    );
}

#[test]
fn names_that_collide_after_sanitizing_are_refused() {
    let first = Trig::new("A B", vec![]);
    let second = Trig::new("A_B", vec![]);
    assert_eq!(
        lower_bytes(&wtg(&[], &[first, second])).unwrap_err(),
        LowerError::Unsupported {
            context: "A_B".into(),
            construct: "trigger names that collide after sanitizing".into(),
        }
    );
}

#[test]
fn if_then_else_is_written_inline_and_disabled_actions_are_omitted() {
    let branch = eca(
        2,
        "IfThenElse",
        true,
        vec![
            call(
                1,
                "OperatorCompareInteger",
                vec![
                    variable_ref("Count"),
                    preset("OperatorEqual"),
                    preset("Num1"),
                ],
            ),
            call(
                2,
                "DisplayTextToPlayer",
                vec![preset("PlayerOne"), string("yes")],
            ),
            call(2, "DoNothing", vec![]),
        ],
    );
    let jass = lower_bytes(&lone(
        "Branch",
        vec![branch, eca(2, "DoNothing", false, vec![])],
    ))
    .unwrap();
    assert!(
        jass.contains(
            "function Trig_Branch_Func001C takes nothing returns boolean\n    return (udg_Count == 1)\nendfunction\n\nfunction Trig_Branch_Actions takes nothing returns nothing\n    if (Trig_Branch_Func001C()) then\n        call DisplayTextToPlayer(Player(0), \"yes\")\n    else\n        call DoNothing()\n    endif\nendfunction\n"
        ),
        "{jass}"
    );
    assert_eq!(jass.matches("DoNothing()").count(), 1, "{jass}");
}

#[test]
fn for_loop_numeric_literals_and_return_follow_the_editor_shape() {
    let loop_body = call(
        2,
        "SetUnitX",
        vec![variable_ref("Count"), param(3, "2.5", None, None)],
    );
    let jass = lower_bytes(&lone(
        "Loop",
        vec![
            eca(
                2,
                "ForLoopA",
                true,
                vec![
                    param(3, "1", None, None),
                    param(3, "7", None, None),
                    loop_body,
                ],
            ),
            eca(2, "ReturnAction", true, vec![]),
        ],
    ))
    .unwrap();
    assert!(
        jass.contains(
            "function Trig_Loop_Actions takes nothing returns nothing\n    set bj_forLoopAIndex = 1\n    set bj_forLoopAIndexEnd = 7\n    loop\n        exitwhen bj_forLoopAIndex > bj_forLoopAIndexEnd\n        call SetUnitX(udg_Count, 2.5)\n        set bj_forLoopAIndex = bj_forLoopAIndex + 1\n    endloop\n    return\nendfunction\n"
        ),
        "{jass}"
    );
}

#[test]
fn object_codes_are_quoted_and_malformed_numbers_refused() {
    let jass = lower_bytes(&lone(
        "Codes",
        vec![eca(
            2,
            "CreateUnitAt",
            true,
            vec![
                preset("PlayerOne"),
                param(3, "hfoo", None, None),
                param(3, "10.5", None, None),
            ],
        )],
    ))
    .unwrap();
    assert!(
        jass.contains("call CreateUnitAt(Player(0), 'hfoo', 10.5)\n"),
        "{jass}"
    );
    let bad = lone(
        "Bad",
        vec![eca(
            2,
            "CreateUnitAt",
            true,
            vec![
                preset("PlayerOne"),
                param(3, "twelve!", None, None),
                param(3, "1", None, None),
            ],
        )],
    );
    assert_eq!(
        lower_bytes(&bad).unwrap_err(),
        LowerError::Malformed {
            context: "Bad".into(),
            reason: "numeric literal \"twelve!\"".into(),
        }
    );
}

#[test]
fn boolean_literals_are_raw_and_string_backslashes_are_escaped() {
    let bytes = wtg(
        &[variable("Flag", "boolean", None)],
        &[Trig::new(
            "Literals",
            vec![
                eca(
                    2,
                    "SetVariable",
                    true,
                    vec![variable_ref("Flag"), string("true")],
                ),
                eca(
                    2,
                    "DisplayTextToPlayer",
                    true,
                    vec![preset("PlayerOne"), string("scripts\\Human.pld")],
                ),
            ],
        )],
    );
    let jass = lower_bytes(&bytes).unwrap();
    assert!(jass.contains("    set udg_Flag = true\n"), "{jass}");
    assert!(
        jass.contains(r#"    call DisplayTextToPlayer(Player(0), "scripts\\Human.pld")"#),
        "{jass}"
    );
}

#[test]
fn initially_off_map_initialization_trigger_is_not_run_from_the_list() {
    let mut parked = Trig::new(
        "Dragons",
        vec![eca(0, "MapInitializationEvent", true, vec![])],
    );
    parked.initially_off = true;
    let jass = lower_bytes(&wtg(&[], &[parked])).unwrap();
    assert!(
        jass.ends_with(
            "function RunInitializationTriggers takes nothing returns nothing\nendfunction\n"
        ),
        "{jass}"
    );
    assert!(
        jass.contains("    call DisableTrigger(gg_trg_Dragons)\n"),
        "{jass}"
    );
}

#[test]
fn custom_text_triggers_emit_their_wct_block_and_are_initialized() {
    let block: &[u8] = b"function Trig_Custom_Actions takes nothing returns nothing\n    call DoNothing()\nendfunction\n\nfunction InitTrig_Custom takes nothing returns nothing\n    set gg_trg_Custom = CreateTrigger()\n    call TriggerAddAction(gg_trg_Custom, function Trig_Custom_Actions)\nendfunction\n\0";
    let mut custom = Trig::new("Custom", vec![]);
    custom.custom = true;
    let jass = lower_with(&wtg(&[], &[custom]), &[block]).unwrap();
    assert!(
        jass.contains(std::str::from_utf8(&block[..block.len() - 1]).unwrap()),
        "{jass}"
    );
    assert!(jass.contains("function InitCustomTriggers takes nothing returns nothing\n    call InitTrig_Custom()\nendfunction\n"), "{jass}");
    assert!(!jass.contains("\0"), "{jass}");
}

#[test]
fn custom_text_trigger_without_text_or_with_a_disabled_flag_is_refused() {
    let mut custom = Trig::new("Empty", vec![]);
    custom.custom = true;
    assert!(matches!(
        lower_with(&wtg(&[], &[custom]), &[&[]]).unwrap_err(),
        LowerError::Malformed { context, .. } if context == "war3map.wct"
    ));
    let mut parked = Trig::new("Parked", vec![]);
    parked.custom = true;
    parked.initially_off = true;
    assert_eq!(
        lower_with(&wtg(&[], &[parked]), &[b"x\0"]).unwrap_err(),
        LowerError::Unsupported {
            context: "Parked".into(),
            construct: "initially-off custom text trigger".into(),
        }
    );
}

#[test]
fn expressions_nested_past_the_limit_are_refused_not_recursed_into() {
    let mut expression = variable_ref("Count");
    for _ in 0..70 {
        expression = call(
            3,
            "OperatorInt",
            vec![expression, preset("OperatorAdd"), preset("Num1")],
        );
    }
    let bytes = lone(
        "Deep",
        vec![eca(
            2,
            "SetVariable",
            true,
            vec![variable_ref("Count"), expression],
        )],
    );
    assert_eq!(
        lower_bytes(&bytes).unwrap_err(),
        LowerError::Malformed {
            context: "Deep".into(),
            reason: "nesting deeper than 64".into(),
        }
    );
}
