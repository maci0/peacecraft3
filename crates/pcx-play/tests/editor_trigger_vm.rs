//! The lowered editor trigger graph and a hand-written script with the same
//! behavior run in the VM under one recording host. Both must make the same native
//! calls, in the same order, and leave the same globals.

use pcx_play::editor_triggers::compile_editor_triggers;
use pcx_play::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{HandleId, Host, Value};
use pcx_trig::catalog::Catalog;
use std::num::NonZeroU64;

#[path = "support/editor_fixture.rs"]
mod editor_fixture;
use editor_fixture::{BLIZZARD, PRELUDE, TRIGGER_DATA, wct, wtg};

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
