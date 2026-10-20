//! The lowered editor trigger graph and a hand-written script with the same
//! behavior run in the VM under one recording host. Both must make the same native
//! calls, in the same order, and leave the same globals.

use pcx_play::editor_triggers::compile_editor_triggers;
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{HandleId, Host, Value};
use pcx_trig::catalog::Catalog;
use std::num::NonZeroU64;

const TRIGGER_DATA: &str = "\
[TriggerTypes]
integer=0,1,1,WESTRING_integer
[TriggerTypeDefaults]
integer=0
[TriggerParams]
RealHalf=0,real,0.5,WESTRING_HALF
Num1=0,integer,1,WESTRING_ONE
PlayerOne=0,player,Player(0),WESTRING_P1
OperatorAdd=0,ArithmeticOperator,\"+\",WESTRING_ADD
[TriggerEvents]
TriggerRegisterTimerEventSingle=0,real
[TriggerActions]
DisplayTextToPlayer=0,player,string
SetVariable=0,AnyGlobal,Null
[TriggerCalls]
OperatorInt=0,1,integer,integer,ArithmeticOperator,integer
";

const BLIZZARD: &[u8] = b"function BlizzardStub takes nothing returns nothing\nendfunction\n";

const PRELUDE: &str = "type trigger extends handle
type triggeraction extends handle
type triggercondition extends handle
type event extends handle
type player extends handle
native CreateTrigger takes nothing returns trigger
native DisableTrigger takes trigger t returns nothing
native TriggerAddAction takes trigger t, code c returns triggeraction
native TriggerRegisterTimerEventSingle takes trigger t, real r returns event
native DisplayTextToPlayer takes player p, string s returns nothing
native Player takes integer n returns player
";

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

fn u(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}

fn s(v: &mut Vec<u8>, text: &str) {
    v.extend_from_slice(text.as_bytes());
    v.push(0);
}

fn param(kind: u32, value: &str, sub: Option<Vec<u8>>) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, kind);
    s(&mut v, value);
    u(&mut v, sub.is_some() as u32);
    if let Some(sub) = sub {
        v.extend_from_slice(&sub);
    }
    u(&mut v, 0); // not an array index
    v
}

fn nested(name: &str, params: Vec<Vec<u8>>) -> Vec<u8> {
    let mut sub = Vec::new();
    u(&mut sub, 3);
    s(&mut sub, name);
    u(&mut sub, 1);
    for p in params {
        sub.extend_from_slice(&p);
    }
    param(2, name, Some(sub))
}

fn eca(kind: u32, name: &str, params: Vec<Vec<u8>>) -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, kind);
    s(&mut v, name);
    u(&mut v, 1);
    for p in params {
        v.extend_from_slice(&p);
    }
    v
}

fn trigger(name: &str, enabled: bool, ecas: &[Vec<u8>]) -> Vec<u8> {
    let mut v = Vec::new();
    s(&mut v, name);
    s(&mut v, "");
    u(&mut v, enabled as u32);
    u(&mut v, 0); // custom
    u(&mut v, 0); // initially off
    u(&mut v, 0); // run on init
    u(&mut v, 1); // category
    u(&mut v, ecas.len() as u32);
    for e in ecas {
        v.extend_from_slice(e);
    }
    v
}

/// Count = 5 at init. "Start" registers a timer and, when it fires, adds Num1 to
/// Count and says hi. "Dormant" is disabled and must not be emitted.
fn wtg() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"WTG!");
    u(&mut v, 4);
    u(&mut v, 1);
    u(&mut v, 1);
    s(&mut v, "Cat");
    u(&mut v, 1);
    u(&mut v, 1); // one variable
    s(&mut v, "Count");
    s(&mut v, "integer");
    u(&mut v, 1);
    u(&mut v, 0);
    u(&mut v, 1);
    s(&mut v, "5");
    u(&mut v, 2); // two triggers
    let start_ecas = [
        eca(
            0,
            "TriggerRegisterTimerEventSingle",
            vec![param(0, "RealHalf", None)],
        ),
        eca(
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
        eca(
            2,
            "DisplayTextToPlayer",
            vec![param(0, "PlayerOne", None), param(3, "hi", None)],
        ),
    ];
    v.extend(trigger("Start", true, &start_ecas));
    let dormant = [eca(
        2,
        "DisplayTextToPlayer",
        vec![param(0, "PlayerOne", None), param(3, "never", None)],
    )];
    v.extend(trigger("Dormant", false, &dormant));
    v
}

fn wct() -> Vec<u8> {
    let mut v = Vec::new();
    u(&mut v, 0);
    u(&mut v, 2);
    u(&mut v, 0);
    u(&mut v, 0);
    v
}

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

fn run(scripts: &MapScripts) -> (Vec<String>, Value) {
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
    vm.call("Trig_Start_Actions", &[]).unwrap();
    let count = vm.global("udg_Count");
    (vm.host().inner.trace.clone(), count)
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
    let (lowered_trace, lowered_count) = run(&lowered);
    let (hand_trace, hand_count) = run(&hand);
    assert_eq!(lowered_trace, hand_trace);
    assert_eq!(lowered_count, hand_count);
    assert_eq!(lowered_count, Value::Int(6));
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
