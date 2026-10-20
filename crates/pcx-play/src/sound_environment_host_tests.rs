use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use crate::sound_environment::from_table;
use crate::sound_environment::tests::sheet;
use pcx_script::{Host, Prelude};
use std::sync::Arc;

const COMMON: &str = "type agent extends handle\ntype timer extends agent\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative NewSoundEnvironment takes string environmentName returns nothing\nglobals\nconstant playercolor C=ConvertPlayerColor(37)\nendglobals\n";

const DEFAULT: [&str; 17] = [
    "\"Default\"",
    "\"MOUNTAINS\"",
    "1",
    "2.5",
    "1",
    "10",
    "1",
    "0",
    "0",
    "1",
    "-2250",
    "0.07",
    "-2500",
    "0.03",
    "0",
    "-2",
    "\"WESTRING_SNDENV_DEFAULT\"",
];

fn host(common: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    ObjectHost::checked(
        EnumHost::checked_selected(
            program,
            "synthetic:environment",
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
    .unwrap()
}

fn environments(rows: &[[&str; 17]]) -> Vec<SoundEnvironment> {
    let table = pcx_slq::Table::parse(&sheet(rows)).unwrap();
    from_table(&table).unwrap()
}

fn select(h: &mut ObjectHost, args: &[Value]) -> Result<Value, String> {
    h.native("NewSoundEnvironment", args)
}

#[test]
fn unadmitted_call_is_refused_and_leaves_no_environment() {
    let mut h = host(COMMON);
    assert!(h.sound_environment().is_none());
    assert!(select(&mut h, &[Value::Str("Default".into())]).is_err());
    assert!(h.sound_environment().is_none());
}

#[test]
fn admitted_call_selects_the_exact_record_and_refuses_unknown_names_atomically() {
    let mut dungeon = DEFAULT;
    dungeon[0] = "\"Dungeon\"";
    let mut h = host(COMMON)
        .with_sound_environments(environments(&[DEFAULT, dungeon]))
        .unwrap();
    assert!(h.sound_environment().is_none());

    assert_eq!(
        select(&mut h, &[Value::Str("Default".into())]).unwrap(),
        Value::Null
    );
    let selected = h.sound_environment().unwrap();
    assert_eq!(selected.name, "Default");
    assert_eq!(selected.display_text, "WESTRING_SNDENV_DEFAULT");
    assert_eq!(selected.reverb.reflections, -2250.0);

    assert!(select(&mut h, &[Value::Str("Unknown".into())]).is_err());
    assert!(select(&mut h, &[Value::Str("default".into())]).is_err());
    assert!(select(&mut h, &[Value::Int(1)]).is_err());
    assert!(select(&mut h, &[]).is_err());
    assert_eq!(h.sound_environment().unwrap().name, "Default");

    assert_eq!(
        select(&mut h, &[Value::Str("Dungeon".into())]).unwrap(),
        Value::Null
    );
    assert_eq!(h.sound_environment().unwrap().name, "Dungeon");
}

#[test]
fn admission_refuses_empty_records_and_a_second_admission() {
    assert!(host(COMMON).with_sound_environments(Vec::new()).is_err());
    let admitted = host(COMMON)
        .with_sound_environments(environments(&[DEFAULT]))
        .unwrap();
    assert!(
        admitted
            .with_sound_environments(environments(&[DEFAULT]))
            .is_err()
    );
}

#[test]
fn admission_refuses_a_native_with_a_different_signature() {
    let wrong = COMMON.replace(
        "takes string environmentName returns nothing",
        "takes integer environmentName returns nothing",
    );
    assert!(
        host(&wrong)
            .with_sound_environments(environments(&[DEFAULT]))
            .is_err()
    );
}
