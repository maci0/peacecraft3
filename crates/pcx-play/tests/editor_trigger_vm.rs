//! The lowered editor trigger graph and a hand-written script with the same
//! behavior run in the VM under one recording host. Both must make the same native
//! calls, in the same order, and leave the same globals.
//!
//! The second test covers the forms the editor expands into control flow —
//! the conditional action and the two numeric loops. For those, the desugar
//! must reproduce the editor's compiled script exactly, not merely behave
//! like it.

use pcx_play::editor_triggers::{LOWERED_LABEL, compile_editor_triggers};
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{HandleId, Host, Value};
use pcx_trig::catalog::Catalog;
use std::num::NonZeroU64;

#[path = "support/editor_fixture.rs"]
mod editor_fixture;
use editor_fixture::{
    BLIZZARD, PRELUDE, TRIGGER_DATA, eca, empty_wct, nested, param, s, trigger, u, variable, wct,
    wtg, wtg_with_variables,
};

/// The compiled-script equivalent of the enabled trigger, written as the editor writes it.
const HAND_WRITTEN: &str = "globals
integer udg_Count
trigger gg_trg_Start
endglobals

function InitGlobals takes nothing returns nothing
    set udg_Count = 5
endfunction

function Trig_Start_Actions takes nothing returns nothing
    set udg_Count = (udg_Count + 1)
    call DisplayTextToPlayer(Player(0), \"hi\")
    call DisplayTextToPlayer(Player(0), \"custom\")
endfunction

function InitTrig_Start takes nothing returns nothing
    set gg_trg_Start = CreateTrigger()
    call TriggerRegisterTimerEventSingle(gg_trg_Start, 0.5)
    call TriggerAddAction(gg_trg_Start, function Trig_Start_Actions)
endfunction

function InitCustomTriggers takes nothing returns nothing
    call InitTrig_Start()
endfunction

function RunInitializationTriggers takes nothing returns nothing
endfunction
";

/// Records every native call as `name(args)`. Code arguments are shown by the
/// name of the function they refer to, so the two programs compare by meaning.
struct Recorder {
    names: Vec<String>,
    trace: Vec<String>,
    next_handle: u64,
}

impl Host for Recorder {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        let rendered: Vec<String> = args
            .iter()
            .map(|a| match a {
                Value::Null => "null".to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Int(i) => i.to_string(),
                Value::Real(r) => r.to_string(),
                Value::Str(s) => format!("{s:?}"),
                Value::Code(id) => format!("code {}", self.names[id.index()]),
                Value::Handle(_) => "handle".to_string(),
            })
            .collect();
        self.trace.push(format!("{name}({})", rendered.join(", ")));
        Ok(match name {
            "CreateTrigger" => {
                self.next_handle += 1;
                Value::Handle(HandleId::new(NonZeroU64::new(self.next_handle).unwrap()))
            }
            _ => Value::Null,
        })
    }
}

fn run(
    scripts: &MapScripts,
    actions: &str,
    globals: &[&str],
) -> (Vec<String>, Vec<(String, Value)>) {
    let names: Vec<String> = scripts
        .program()
        .functions()
        .iter()
        .map(|f| f.name.clone())
        .collect();
    let host = Recorder {
        names,
        trace: Vec::new(),
        next_handle: 0,
    };
    let mut vm = scripts.vm(host);
    vm.init_globals().unwrap();
    // The order the editor's `main` uses.
    vm.call("InitGlobals", &[]).unwrap();
    vm.call("InitCustomTriggers", &[]).unwrap();
    vm.call("RunInitializationTriggers", &[]).unwrap();
    vm.call(actions, &[]).unwrap();
    let globals = globals
        .iter()
        .map(|name| ((*name).to_string(), vm.global(name)))
        .collect();
    (vm.host().inner.trace.clone(), globals)
}

fn identity() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "fixture".into(),
        source: "memory".into(),
    }
}

#[test]
fn lowered_triggers_and_the_hand_written_script_make_the_same_calls_and_state() {
    let catalog = Catalog::parse(
        "synthetic-TriggerData.txt",
        "synthetic",
        "fixture",
        TRIGGER_DATA,
    )
    .unwrap();
    let common_text = PRELUDE.as_bytes();
    let lowered = compile_editor_triggers(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: common_text,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        &wtg(),
        &wct(),
        &catalog,
    );
    let lowered = lowered.unwrap();
    let hand = MapScripts::compile(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: common_text,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BLIZZARD,
        },
        ScriptAsset {
            label: "war3map.j",
            bytes: HAND_WRITTEN.as_bytes(),
        },
        None,
    )
    .unwrap();
    let (lowered_trace, lowered_globals) = run(&lowered, "Trig_Start_Actions", &["udg_Count"]);
    let (hand_trace, hand_globals) = run(&hand, "Trig_Start_Actions", &["udg_Count"]);
    assert_eq!(lowered_trace, hand_trace);
    assert_eq!(lowered_globals, hand_globals);
    assert_eq!(lowered_globals[0].1, Value::Int(6));
    assert!(
        lowered_trace.iter().all(|c| !c.contains("never")),
        "{lowered_trace:?}"
    );
    assert!(
        lowered_trace
            .iter()
            .any(|c| c.contains("TriggerAddAction") && c.contains("Trig_Start_Actions")),
        "{lowered_trace:?}"
    );
}

// ----- the branch and loop forms ------------------------------------------
/// A TriggerData with the forms the editor expands into control flow: the
/// conditional action, the two numeric loops and the comparison condition
/// they call.
const BRANCH_TRIGGER_DATA: &str = "\
[TriggerTypes]
integer=0,1,1,WESTRING_integer
string=0,1,1,WESTRING_string
[TriggerTypeDefaults]
integer=0
[TriggerParams]
Num1=0,integer,1,WESTRING_ONE
PlayerOne=0,player,Player(0),WESTRING_P1
RealHalf=0,real,0.5,WESTRING_HALF
OperatorAdd=0,ArithmeticOperator,\"+\",WESTRING_ADD
OperatorEqual=0,ComparisonOperator,\"==\",WESTRING_EQ
[TriggerEvents]
TriggerRegisterTimerEventSingle=0,real
[TriggerConditions]
OperatorCompareInteger=0,integer,ComparisonOperator,integer
[TriggerActions]
DisplayTextToPlayer=0,player,string
SetVariable=0,AnyGlobal,Null
IfThenElse=0,boolexpr,code,code
ForLoopA=0,integer,integer,code
ForLoopVar=1,integervar,integer,integer,code
[TriggerCalls]
OperatorInt=0,1,integer,integer,ArithmeticOperator,integer
";

/// The loop index globals the numeric loop form uses. In the game they are
/// declared by `Blizzard.j`, so they are declared here the same way.
const BRANCH_BLIZZARD: &[u8] = b"globals
integer bj_forLoopAIndex
integer bj_forLoopAIndexEnd
endglobals
";

/// A function-typed parameter: a wrapped condition (kind 1) or a wrapped
/// action (kind 2).
fn wrapped(kind: u32, name: &str, params: Vec<Vec<u8>>) -> Vec<u8> {
    let mut sub = Vec::new();
    u(&mut sub, kind);
    s(&mut sub, name);
    u(&mut sub, 1);
    for p in params {
        sub.extend_from_slice(&p);
    }
    param(2, "", Some(sub))
}

/// A trigger whose actions are the editor's branch and loop forms: a
/// conditional action, a numeric loop that adds to the global, and a loop
/// over the declared integer.
fn branch_wtg() -> (Vec<u8>, Vec<u8>) {
    let ecas = vec![
        eca(
            0,
            "TriggerRegisterTimerEventSingle",
            vec![param(0, "RealHalf", None)],
        ),
        eca(
            2,
            "IfThenElse",
            vec![
                wrapped(
                    1,
                    "OperatorCompareInteger",
                    vec![
                        param(1, "Count", None),
                        param(0, "OperatorEqual", None),
                        param(0, "Num1", None),
                    ],
                ),
                wrapped(
                    2,
                    "DisplayTextToPlayer",
                    vec![param(0, "PlayerOne", None), param(3, "yes", None)],
                ),
                wrapped(
                    2,
                    "DisplayTextToPlayer",
                    vec![param(0, "PlayerOne", None), param(3, "no", None)],
                ),
            ],
        ),
        eca(
            2,
            "ForLoopA",
            vec![
                param(3, "1", None),
                param(3, "3", None),
                wrapped(
                    2,
                    "SetVariable",
                    vec![
                        param(1, "Count", None),
                        nested(
                            "OperatorInt",
                            vec![
                                param(1, "Count", None),
                                param(0, "OperatorAdd", None),
                                param(0, "Num1", None),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        eca(
            2,
            "ForLoopVar",
            vec![
                param(1, "Count", None),
                param(3, "1", None),
                param(3, "2", None),
                wrapped(
                    2,
                    "DisplayTextToPlayer",
                    vec![param(0, "PlayerOne", None), param(3, "loop", None)],
                ),
            ],
        ),
    ];
    let wtg = wtg_with_variables(
        &[variable("Count", "integer", Some("5"))],
        &[trigger("Work", true, &ecas)],
    );
    (wtg, empty_wct(1))
}

/// The compiled-script equivalent of that trigger, in the shape the editor
/// writes: the conditional action as an `if`, the numeric loop as a `loop`
/// over the `Blizzard.j` index globals, and the loop over the declared
/// integer as a `loop` over the variable itself.
const BRANCH_HAND_WRITTEN: &str = "globals
    integer udg_Count
    trigger gg_trg_Work
endglobals

function InitGlobals takes nothing returns nothing
    set udg_Count = 5
endfunction

function Trig_Work_Func001C takes nothing returns boolean
    return (udg_Count == 1)
endfunction

function Trig_Work_Actions takes nothing returns nothing
    if (Trig_Work_Func001C()) then
        call DisplayTextToPlayer(Player(0), \"yes\")
    else
        call DisplayTextToPlayer(Player(0), \"no\")
    endif
    set bj_forLoopAIndex = 1
    set bj_forLoopAIndexEnd = 3
    loop
        exitwhen bj_forLoopAIndex > bj_forLoopAIndexEnd
        set udg_Count = (udg_Count + 1)
        set bj_forLoopAIndex = bj_forLoopAIndex + 1
    endloop
    set udg_Count = 1
    loop
        exitwhen udg_Count > 2
        call DisplayTextToPlayer(Player(0), \"loop\")
        set udg_Count = udg_Count + 1
    endloop
endfunction

function InitTrig_Work takes nothing returns nothing
    set gg_trg_Work = CreateTrigger()
    call TriggerRegisterTimerEventSingle(gg_trg_Work, 0.5)
    call TriggerAddAction(gg_trg_Work, function Trig_Work_Actions)
endfunction

function InitCustomTriggers takes nothing returns nothing
    call InitTrig_Work()
endfunction

function RunInitializationTriggers takes nothing returns nothing
endfunction
";

/// The forms the editor expands into control flow must desugar to the script
/// the editor compiles for the same graph, and both must run the same way in
/// the VM: the same native calls in the same order, and the same globals
/// afterwards.
#[test]
fn the_branch_and_loop_forms_desugar_to_the_editor_script_and_run_the_same() {
    let catalog = Catalog::parse(
        "synthetic-TriggerData.txt",
        "synthetic",
        "fixture",
        BRANCH_TRIGGER_DATA,
    )
    .unwrap();
    let common_text = PRELUDE.as_bytes();
    let (wtg_bytes, wct_bytes) = branch_wtg();
    let lowered = compile_editor_triggers(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: common_text,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BRANCH_BLIZZARD,
        },
        &wtg_bytes,
        &wct_bytes,
        &catalog,
    )
    .unwrap();
    let hand = MapScripts::compile(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: common_text,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: BRANCH_BLIZZARD,
        },
        ScriptAsset {
            label: "war3map.j",
            bytes: BRANCH_HAND_WRITTEN.as_bytes(),
        },
        None,
    )
    .unwrap();
    // The desugar writes the editor's compiled script, not a paraphrase of it.
    assert_eq!(
        lowered.source(LOWERED_LABEL),
        Some(BRANCH_HAND_WRITTEN),
        "the desugared trigger graph must compile to the editor's script"
    );
    let globals = ["udg_Count", "bj_forLoopAIndex", "bj_forLoopAIndexEnd"];
    let (lowered_trace, lowered_globals) = run(&lowered, "Trig_Work_Actions", &globals);
    let (hand_trace, hand_globals) = run(&hand, "Trig_Work_Actions", &globals);
    assert_eq!(lowered_trace, hand_trace);
    assert_eq!(lowered_globals, hand_globals);
    // The branch takes the `else` arm (`udg_Count` is 5, not 1), the numeric
    // loop runs three times and the loop over the declared integer twice.
    assert_eq!(
        lowered_globals,
        vec![
            ("udg_Count".to_string(), Value::Int(3)),
            ("bj_forLoopAIndex".to_string(), Value::Int(4)),
            ("bj_forLoopAIndexEnd".to_string(), Value::Int(3)),
        ]
    );
    assert_eq!(
        lowered_trace,
        vec![
            "CreateTrigger()",
            "TriggerRegisterTimerEventSingle(handle, 0.5)",
            "TriggerAddAction(handle, code Trig_Work_Actions)",
            "Player(0)",
            "DisplayTextToPlayer(null, \"no\")",
            "Player(0)",
            "DisplayTextToPlayer(null, \"loop\")",
            "Player(0)",
            "DisplayTextToPlayer(null, \"loop\")",
        ]
    );
}
