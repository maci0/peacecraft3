#[path = "../src/custom_scripts.rs"]
mod custom_scripts;

use pcx_play::scripts::{AssetIdentity, ScriptAsset};
use pcx_trig::{TriggerArity, Wct, Wtg};

fn identity() -> AssetIdentity {
    AssetIdentity {
        edition: "synthetic".into(),
        version: "fixture-1".into(),
        source: "in-memory".into(),
    }
}

fn wct_bytes(block: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(block.len() as u32).to_le_bytes());
    out.extend_from_slice(block);
    out
}

fn wtg_one(name: &[u8], custom: u32) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"WTG!");
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(name);
    out.push(0);
    out.extend_from_slice(b"\0");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&custom.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out
}

#[test]
fn compiles_custom_trigger() {
    let common = b"native Nothing takes nothing returns nothing\n";
    let blizzard = b"function BJ takes nothing returns nothing\nendfunction\n";
    let block =
        b"function Trig_Test_Actions takes nothing returns nothing\ncall Nothing()\nendfunction\0";
    let wctb = wct_bytes(block);
    let wct = Wct::parse(&wctb).unwrap();
    let wtgb = wtg_one(b"Trig_Test_Actions", 1);
    let wtg = Wtg::parse(&wtgb, &TriggerArity::new()).unwrap();
    let out = custom_scripts::CustomScripts::compile(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: common,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: blizzard,
        },
        None,
        &wct,
        &wtg,
        None,
        custom_scripts::CustomScriptBudget::default(),
    )
    .unwrap();
    assert_eq!(out.items.len(), 1);
    assert_eq!(out.items[0].trigger, "Trig_Test_Actions");
    assert!(out.items[0].program.function("Trig_Test_Actions").is_some());
}

#[test]
fn association_failure_typed() {
    let common = b"native Nothing takes nothing returns nothing\n";
    let blizzard = b"function BJ takes nothing returns nothing\nendfunction\n";
    let block = b"function Trig_Test_Actions takes nothing returns nothing\nendfunction\0";
    let wctb = wct_bytes(block);
    let wct = Wct::parse(&wctb).unwrap();
    let wtgb = wtg_one(b"Trig_Test_Actions", 0);
    let wtg = Wtg::parse(&wtgb, &TriggerArity::new()).unwrap();
    let err = custom_scripts::CustomScripts::compile(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: common,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: blizzard,
        },
        None,
        &wct,
        &wtg,
        None,
        custom_scripts::CustomScriptBudget::default(),
    )
    .unwrap_err();
    assert!(format!("{err}").contains("association"));
}

#[test]
fn invalid_identity() {
    let common = b"native Nothing takes nothing returns nothing\n";
    let blizzard = b"function BJ takes nothing returns nothing\nendfunction\n";
    let block = b"function X takes nothing returns nothing\nendfunction\0";
    let wctb = wct_bytes(block);
    let wct = Wct::parse(&wctb).unwrap();
    let wtgb = wtg_one(b"Trig_X_Actions", 1);
    let wtg = Wtg::parse(&wtgb, &TriggerArity::new()).unwrap();
    let mut id = identity();
    id.edition.clear();
    let err = custom_scripts::CustomScripts::compile(
        id,
        ScriptAsset {
            label: "common.j",
            bytes: common,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: blizzard,
        },
        None,
        &wct,
        &wtg,
        None,
        custom_scripts::CustomScriptBudget::default(),
    )
    .unwrap_err();
    assert!(format!("{err}").contains("edition"));
}

#[test]
fn parse_failure_typed() {
    let common = b"native Nothing takes nothing returns nothing\n";
    let blizzard = b"function BJ takes nothing returns nothing\nendfunction\n";
    let block =
        b"function Trig_Bad_Actions takes nothing returns nothing\ncall Missing()\nendfunction\0";
    let wctb = wct_bytes(block);
    let wct = Wct::parse(&wctb).unwrap();
    let wtgb = wtg_one(b"Trig_Bad_Actions", 1);
    let wtg = Wtg::parse(&wtgb, &TriggerArity::new()).unwrap();
    let err = custom_scripts::CustomScripts::compile(
        identity(),
        ScriptAsset {
            label: "common.j",
            bytes: common,
        },
        ScriptAsset {
            label: "Blizzard.j",
            bytes: blizzard,
        },
        None,
        &wct,
        &wtg,
        None,
        custom_scripts::CustomScriptBudget::default(),
    )
    .unwrap_err();
    assert!(format!("{err}").contains("compilation") || format!("{err}").contains("parse"));
}
