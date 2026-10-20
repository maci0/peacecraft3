use super::*;
use crate::playercolor_host::{EnumHost, Limits};
use pcx_script::{Host, Prelude};
use std::sync::Arc;

const DECLARATION: &str =
    "native SetUnitExploded takes unit whichUnit, boolean exploded returns nothing";
const BASE: &str = "\
type agent extends handle
type timer extends agent
type widget extends agent
type unit extends widget
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native CreateUnit takes player p, integer unitid, real x, real y, real face returns unit
";

fn enums(declaration: &str) -> EnumHost {
    let common = pcx_script::parse_script(&format!("{BASE}{declaration}")).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    EnumHost::checked_selected(
        program,
        "synthetic:map",
        &[],
        Limits {
            max_handles: 8,
            admission_steps: 1024,
            expression_depth: 16,
            expression_work: 1024,
        },
    )
    .unwrap()
}

fn host(max_units: usize) -> ObjectHost {
    ObjectHost::checked(enums(DECLARATION))
        .unwrap()
        .with_unit_explosions(max_units)
        .unwrap()
}

#[test]
fn admission_requires_the_declared_unit_and_boolean_signature_once() {
    let wrong =
        enums("native SetUnitExploded takes unit whichUnit, integer exploded returns nothing");
    assert!(
        ObjectHost::checked(wrong)
            .unwrap()
            .with_unit_explosions(1)
            .is_err()
    );
    let missing = enums("");
    assert!(
        ObjectHost::checked(missing)
            .unwrap()
            .with_unit_explosions(1)
            .is_err()
    );
    assert!(host(1).with_unit_explosions(1).is_err());
}

#[test]
fn a_timer_handle_refuses_and_retains_nothing() {
    let mut h = host(4);
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(
        h.native("SetUnitExploded", &[timer, Value::Bool(true)])
            .is_err()
    );
    assert_eq!(h.unit_explosions.as_ref().unwrap().flags.len(), 0);
}

#[test]
fn a_non_boolean_flag_refuses() {
    let mut h = host(4);
    let timer = h.native("CreateTimer", &[]).unwrap();
    assert!(
        h.native("SetUnitExploded", &[timer, Value::Int(1)])
            .is_err()
    );
}

#[test]
fn an_unadmitted_host_does_not_answer_the_native() {
    let mut h = ObjectHost::checked(enums(DECLARATION)).unwrap();
    assert!(
        h.native("SetUnitExploded", &[Value::Null, Value::Bool(true)])
            .is_err()
    );
}

/// The same signature from another selected source does not match the admitted unit catalog.
#[test]
fn a_foreign_source_declaration_refuses_admission() {
    let common = pcx_script::parse_script(BASE).unwrap();
    let foreign = pcx_script::parse_script(DECLARATION).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(
            &map,
            &[
                Prelude::new("synthetic:common", &common),
                Prelude::new("synthetic:foreign", &foreign),
            ],
        )
        .unwrap(),
    );
    let enums = EnumHost::checked_selected(
        program,
        "synthetic:map",
        &[],
        Limits {
            max_handles: 8,
            admission_steps: 1024,
            expression_depth: 16,
            expression_work: 1024,
        },
    )
    .unwrap();
    let refused = ObjectHost::checked(enums).unwrap().with_unit_explosions(1);
    assert_eq!(refused.err().unwrap().failure, Failure::Source);
}
