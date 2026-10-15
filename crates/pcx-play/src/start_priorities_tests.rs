use super::StartPriority;
use crate::playercolor_host::{EnumHost, Failure, Limits, MapConfigBudget, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

fn common() -> String {
    format!(
        "{}type startlocprio extends handle\nconstant native ConvertStartLocPrio takes integer i returns startlocprio\nnative SetStartLocPrioCount takes integer start, integer count returns nothing\nnative SetStartLocPrio takes integer start, integer slot, integer other, startlocprio priority returns nothing\nnative GetStartLocPrioSlot takes integer start, integer slot returns integer\nnative GetStartLocPrio takes integer start, integer slot returns startlocprio\nglobals\nconstant startlocprio LOW = ConvertStartLocPrio(23)\nconstant startlocprio HIGH = ConvertStartLocPrio(-9)\nendglobals\n",
        super::super::tests::COMMON
    )
}
fn object(common: &str, map: &str) -> ObjectHost {
    let common = pcx_script::parse_script(common).unwrap();
    let map = pcx_script::parse_script(map).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:priority-common", &common)])
            .unwrap(),
    );
    let e = EnumHost::checked_selected(
        program,
        "synthetic:priority-map",
        &["ConvertPlacement", "ConvertStartLocPrio"],
        Limits {
            max_handles: 12,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(e).unwrap()
}
fn config(h: ObjectHost) -> ObjectHost {
    h.with_map_config(MapConfigBudget {
        text_bytes: 0,
        start_locations: 4,
    })
    .unwrap()
}
fn host(entries: usize) -> ObjectHost {
    config(object(&common(), ""))
        .with_start_priorities(entries)
        .unwrap()
}
fn define(h: &mut ObjectHost, index: i32) {
    h.native(
        "DefineStartLocation",
        &[
            Value::Int(index),
            Value::Real(index as f32),
            Value::Real(-0.),
        ],
    )
    .unwrap();
}
fn count(h: &mut ObjectHost, start: i32, slots: i32) -> Result<Value, String> {
    h.native(
        "SetStartLocPrioCount",
        &[Value::Int(start), Value::Int(slots)],
    )
}
fn priority(h: &mut ObjectHost, value: i32) -> Value {
    h.native("ConvertStartLocPrio", &[Value::Int(value)])
        .unwrap()
}
fn assign(
    h: &mut ObjectHost,
    start: i32,
    slot: i32,
    other: i32,
    priority: Value,
) -> Result<Value, String> {
    h.native(
        "SetStartLocPrio",
        &[
            Value::Int(start),
            Value::Int(slot),
            Value::Int(other),
            priority,
        ],
    )
}
#[test]
fn priority_interfaces_require_explicit_admission_and_defined_sources() {
    assert!(object(&common(), "").with_start_priorities(4).is_err());
    let mut disabled = config(object(&common(), ""));
    define(&mut disabled, 7);
    assert!(
        count(&mut disabled, 7, 1)
            .unwrap_err()
            .contains("unsupported")
    );
    let mut h = disabled.with_start_priorities(4).unwrap();
    assert!(count(&mut h, 11, 1).unwrap_err().contains("undefined"));
    assert!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(0)])
            .unwrap_err()
            .contains("unset")
    );
    count(&mut h, 7, 0).unwrap();
    assert_eq!(h.map_config().unwrap().start_priorities.len(), 1);
    assert!(h.map_config().unwrap().start_priorities[0].slots.is_empty());
    assert!(
        h.native("GetStartLocPrioSlot", &[Value::Int(7), Value::Int(0)])
            .is_err()
    );
    assert!(h.with_start_priorities(4).is_err());
}
#[test]
fn entries_preserve_sparse_target_relationships_and_symbolic_identity() {
    let mut h = host(3);
    for index in [7, 11, 14] {
        define(&mut h, index);
    }
    count(&mut h, 7, 2).unwrap();
    let low = priority(&mut h, 23);
    let high = priority(&mut h, -9);
    assign(&mut h, 7, 1, 14, high.clone()).unwrap();
    assign(&mut h, 7, 0, 11, low.clone()).unwrap();
    assert_eq!(
        h.native("GetStartLocPrioSlot", &[Value::Int(7), Value::Int(0)])
            .unwrap(),
        Value::Int(11)
    );
    assert_eq!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(1)])
            .unwrap(),
        high
    );
    let Value::Handle(low_id) = low else {
        panic!("priority identity")
    };
    assert_eq!(
        h.map_config().unwrap().start_priorities[0].slots[0],
        Some(StartPriority {
            other_start: 11,
            priority: low_id
        })
    );
    h.native(
        "DefineStartLocation",
        &[Value::Int(11), Value::Real(2.25), Value::Real(4.5)],
    )
    .unwrap();
    assert_eq!(
        h.native("GetStartLocPrioSlot", &[Value::Int(7), Value::Int(0)])
            .unwrap(),
        Value::Int(11)
    );
    assert_eq!(
        h.native("GetStartLocationX", &[Value::Int(11)]).unwrap(),
        Value::Real(2.25)
    );
}
#[test]
fn unset_wrong_family_negative_and_outside_slots_refuse_without_assigning_defaults() {
    let mut h = host(2);
    define(&mut h, 7);
    define(&mut h, 11);
    count(&mut h, 7, 2).unwrap();
    let p = priority(&mut h, 23);
    assert!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(0)])
            .is_err()
    );
    for (slot, other) in [(-1, 11), (2, 11), (0, 99)] {
        assert!(assign(&mut h, 7, slot, other, p.clone()).is_err());
    }
    let wrong = h.native("ConvertPlacement", &[Value::Int(31)]).unwrap();
    assert!(assign(&mut h, 7, 0, 11, wrong).is_err());
    assert!(assign(&mut h, 7, 0, 11, Value::Int(23)).is_err());
    assert!(
        h.map_config().unwrap().start_priorities[0]
            .slots
            .iter()
            .all(Option::is_none)
    );
    assign(&mut h, 7, 0, 11, p.clone()).unwrap();
    assert!(assign(&mut h, 7, 0, 99, p.clone()).is_err());
    assert_eq!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(0)])
            .unwrap(),
        p
    );
}
#[test]
fn aggregate_budget_rejection_is_atomic_and_shrink_releases_logical_slots() {
    let mut h = host(3);
    define(&mut h, 7);
    define(&mut h, 11);
    count(&mut h, 7, 2).unwrap();
    count(&mut h, 11, 1).unwrap();
    let p = priority(&mut h, -9);
    assign(&mut h, 7, 0, 11, p.clone()).unwrap();
    assert!(
        count(&mut h, 11, 2)
            .unwrap_err()
            .contains("start priority entries")
    );
    assert!(count(&mut h, 7, i32::MAX).is_err());
    assert!(count(&mut h, 7, -1).is_err());
    assert_eq!(h.map_config().unwrap().start_priorities[0].slots.len(), 2);
    assert_eq!(h.map_config().unwrap().start_priorities[1].slots.len(), 1);
    count(&mut h, 7, 1).unwrap();
    count(&mut h, 11, 2).unwrap();
    assert_eq!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(0)])
            .unwrap(),
        p
    );
    assert_eq!(
        h.map_config()
            .unwrap()
            .start_priorities
            .iter()
            .map(|p| p.slots.len())
            .sum::<usize>(),
        3
    );
}
#[test]
fn resize_preserves_prefix_and_never_restores_truncated_or_invented_entries() {
    let mut h = host(3);
    define(&mut h, 7);
    define(&mut h, 11);
    count(&mut h, 7, 2).unwrap();
    let p = priority(&mut h, 23);
    assign(&mut h, 7, 0, 11, p.clone()).unwrap();
    assign(&mut h, 7, 1, 7, p.clone()).unwrap();
    count(&mut h, 7, 1).unwrap();
    count(&mut h, 7, 3).unwrap();
    assert_eq!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(0)])
            .unwrap(),
        p
    );
    assert!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(1)])
            .is_err()
    );
    assert!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(2)])
            .is_err()
    );
    count(&mut h, 7, 0).unwrap();
    assert!(h.map_config().unwrap().start_priorities[0].slots.is_empty());
}
#[test]
fn zero_budget_and_platform_layout_refuse_before_nonzero_slot_allocation() {
    let mut h = host(0);
    define(&mut h, 7);
    define(&mut h, 11);
    count(&mut h, 7, 0).unwrap();
    count(&mut h, 11, 0).unwrap();
    assert!(count(&mut h, 7, 1).is_err());
    assert!(
        config(object(&common(), ""))
            .with_start_priorities(usize::MAX)
            .is_err()
    );
    let count = (u32::MAX as usize / 2) / std::mem::size_of::<Option<StartPriority>>() + 1;
    assert_eq!(
        config(object(&common(), ""))
            .with_start_priorities(count)
            .is_err(),
        usize::BITS == 32
    );
}
#[test]
fn incompatible_signatures_constant_flags_and_script_overrides_are_refused() {
    for common in [
        common().replace(
            "native GetStartLocPrio ",
            "constant native GetStartLocPrio ",
        ),
        common().replace(
            "GetStartLocPrioSlot takes integer start, integer slot returns integer",
            "GetStartLocPrioSlot takes integer start, integer slot returns real",
        ),
        common().replace(
            "integer other, startlocprio priority",
            "integer other, placement priority",
        ),
    ] {
        assert!(
            config(object(&common, ""))
                .with_start_priorities(4)
                .is_err()
        );
    }
    let map = "function SetStartLocPrioCount takes integer start, integer count returns nothing\nendfunction\n";
    assert!(
        matches!(config(object(&common(), map)).with_start_priorities(4), Err(e) if e.failure == Failure::Signature)
    );
}
#[test]
fn compiled_config_initializes_queries_and_retains_the_same_program_across_calls() {
    let map = "function config takes nothing returns nothing\ncall DefineStartLocation(7, -0.0, 4.5)\ncall DefineStartLocation(11, 8.0, 9.0)\ncall SetStartLocPrioCount(7, 1)\ncall SetStartLocPrio(7, 0, 11, HIGH)\nendfunction\nfunction target takes nothing returns integer\nreturn GetStartLocPrioSlot(7, 0)\nendfunction\nfunction rank takes nothing returns startlocprio\nreturn GetStartLocPrio(7, 0)\nendfunction\n";
    let h = config(object(&common(), map))
        .with_start_priorities(4)
        .unwrap();
    let mut vm = h.into_vm();
    vm.init_globals().unwrap();
    vm.call("config", &[]).unwrap();
    assert_eq!(vm.call("target", &[]).unwrap(), Value::Int(11));
    assert_eq!(vm.call("rank", &[]).unwrap(), vm.global("HIGH"));
    let handles = vm.host().inner().handles();
    vm.init_globals().unwrap();
    assert_eq!(vm.host().inner().handles(), handles);
}
#[test]
fn malformed_call_shapes_leave_existing_counts_and_entries_intact() {
    let mut h = host(2);
    define(&mut h, 7);
    define(&mut h, 11);
    count(&mut h, 7, 1).unwrap();
    let p = priority(&mut h, 23);
    assign(&mut h, 7, 0, 11, p.clone()).unwrap();
    for (name, args) in [
        ("SetStartLocPrioCount", vec![Value::Int(7), Value::Real(1.)]),
        (
            "SetStartLocPrioCount",
            vec![Value::Int(7), Value::Int(1), Value::Int(2)],
        ),
        (
            "SetStartLocPrio",
            vec![Value::Int(7), Value::Int(0), Value::Int(11), Value::Null],
        ),
        ("GetStartLocPrio", vec![Value::Int(7)]),
        ("GetStartLocPrioSlot", vec![Value::Null, Value::Int(0)]),
    ] {
        assert!(h.native(name, &args).is_err());
    }
    assert_eq!(
        h.native("GetStartLocPrio", &[Value::Int(7), Value::Int(0)])
            .unwrap(),
        p
    );
}
