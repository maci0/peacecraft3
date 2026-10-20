use super::super::super::{EnumHost, Limits, ObjectHost};
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

const DECLARATIONS: &str = "type agent extends handle
type timer extends agent
type rect extends agent
type playercolor extends handle
constant native ConvertPlayerColor takes integer value returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer which returns nothing
native Rect takes real a, real b, real c, real d returns rect
native RemoveRect takes rect which returns nothing
native SetRect takes rect which, real a, real b, real c, real d returns nothing
native MoveRectTo takes rect which, real x, real y returns nothing
native GetRectCenterX takes rect which returns real
native GetRectCenterY takes rect which returns real
native GetRectMinX takes rect which returns real
native GetRectMinY takes rect which returns real
native GetRectMaxX takes rect which returns real
native GetRectMaxY takes rect which returns real
globals
constant playercolor COLOR = ConvertPlayerColor(19)
endglobals
";

fn host_from(declarations: &str, script: &str, max_handles: usize) -> ObjectHost {
    let common = pcx_script::parse_script(declarations).unwrap();
    let map = pcx_script::parse_script(script).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:rect-contract", &common)])
            .unwrap(),
    );
    let inner = EnumHost::checked_selected(
        program,
        "synthetic:selected-map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(inner).unwrap()
}

fn host() -> ObjectHost {
    host_from(DECLARATIONS, "", 16).with_rectangles().unwrap()
}

fn values(bounds: [f32; 4]) -> [Value; 4] {
    bounds.map(Value::Real)
}

fn create(host: &mut ObjectHost, bounds: [f32; 4]) -> Value {
    host.native("Rect", &values(bounds)).unwrap()
}

fn read(host: &mut ObjectHost, name: &str, rect: &Value) -> Value {
    host.native(name, std::slice::from_ref(rect)).unwrap()
}

#[test]
fn shared_identity_lifecycle_preserves_other_families_and_invalidates_aliases() {
    let mut host = host();
    let timer = host.native("CreateTimer", &[]).unwrap();
    let rect = create(&mut host, [-30.0, 5.0, 10.0, 17.0]);
    assert_ne!(rect, timer);
    assert_eq!(read(&mut host, "GetRectMinX", &rect), Value::Real(-30.0));
    assert_eq!(read(&mut host, "GetRectMaxY", &rect), Value::Real(17.0));
    for (name, args) in [
        ("GetRectMinX", vec![timer.clone()]),
        ("DestroyTimer", vec![rect.clone()]),
    ] {
        assert!(
            host.native(name, &args)
                .unwrap_err()
                .contains("another family")
        );
    }
    let alias = rect.clone();
    host.native("RemoveRect", &[rect]).unwrap();
    for name in ["GetRectCenterX", "RemoveRect"] {
        assert!(
            host.native(name, std::slice::from_ref(&alias))
                .unwrap_err()
                .contains("unknown handle identity")
        );
    }
    host.native("DestroyTimer", &[timer]).unwrap();
    assert_ne!(alias, create(&mut host, [0.0; 4]));
}

#[test]
fn setters_and_moves_use_the_existing_identity_and_source_dimensions() {
    let mut host = host();
    let rect = create(&mut host, [-10.0, 4.0, 6.0, 12.0]);
    let mut args = vec![rect.clone()];
    args.extend(values([-3.0, -8.0, 7.0, 4.0]));
    host.native("SetRect", &args).unwrap();
    host.native(
        "MoveRectTo",
        &[rect.clone(), Value::Int(20), Value::Real(-1.0)],
    )
    .unwrap();
    let Value::Handle(id) = rect else {
        panic!("not a handle");
    };
    assert_eq!(host.rectangle_bounds(id).unwrap(), [15.0, -7.0, 25.0, 5.0]);
    assert_eq!(
        read(&mut host, "GetRectCenterX", &Value::Handle(id)),
        Value::Real(20.0)
    );
    assert_eq!(
        read(&mut host, "GetRectCenterY", &Value::Handle(id)),
        Value::Real(-1.0)
    );
}

#[test]
fn invalid_mutations_are_atomic_and_never_create_infinite_bounds() {
    let mut host = host();
    let point = create(&mut host, [f32::MAX, 0.0, f32::MAX, 0.0]);
    assert_eq!(
        read(&mut host, "GetRectCenterX", &point),
        Value::Real(f32::MAX)
    );
    let rect = create(&mut host, [-f32::MAX, -2.0, f32::MAX, 8.0]);
    assert_eq!(read(&mut host, "GetRectCenterX", &rect), Value::Real(0.0));
    for (name, args) in [
        (
            "MoveRectTo",
            vec![rect.clone(), Value::Real(f32::MAX), Value::Int(0)],
        ),
        (
            "MoveRectTo",
            vec![rect.clone(), Value::Real(f32::NAN), Value::Int(0)],
        ),
        (
            "SetRect",
            vec![
                rect.clone(),
                Value::Int(3),
                Value::Int(0),
                Value::Int(1),
                Value::Int(2),
            ],
        ),
        (
            "SetRect",
            vec![
                rect.clone(),
                Value::Int(0),
                Value::Int(0),
                Value::Real(f32::INFINITY),
                Value::Int(2),
            ],
        ),
        ("GetRectMinX", vec![rect.clone(), Value::Int(0)]),
    ] {
        assert!(host.native(name, &args).is_err(), "{name}");
        let Value::Handle(id) = rect else {
            panic!("not a handle");
        };
        assert_eq!(
            host.rectangle_bounds(id).unwrap(),
            [-f32::MAX, -2.0, f32::MAX, 8.0]
        );
    }
    assert!(host.native("Rect", &values([2.0, 0.0, 1.0, 3.0])).is_err());
    assert!(
        host.native("Rect", &[const { Value::Bool(false) }; 4])
            .is_err()
    );
    assert!(host.native("RemoveRect", &[Value::Null]).is_err());
}

#[test]
fn removed_identities_do_not_bypass_the_shared_capacity() {
    let mut host = host_from(DECLARATIONS, "", 2).with_rectangles().unwrap();
    let first = create(&mut host, [0.0; 4]);
    let timer = host.native("CreateTimer", &[]).unwrap();
    host.native("RemoveRect", &[first]).unwrap();
    host.native("DestroyTimer", &[timer]).unwrap();
    assert!(
        host.native("Rect", &values([0.0; 4]))
            .unwrap_err()
            .contains("capacity")
    );
}

#[test]
fn declarations_are_checked_and_context_dependent_natives_stay_unsupported() {
    let mut unadmitted = host_from(DECLARATIONS, "", 16);
    assert!(
        unadmitted
            .native("Rect", &values([0.0; 4]))
            .unwrap_err()
            .contains("unsupported")
    );
    let mut host = host();
    assert!(host.native("GetWorldBounds", &[]).is_err());
    assert!(
        host.native("RectFromLoc", &[Value::Null, Value::Null])
            .is_err()
    );
    let Err(second) = host.with_rectangles() else {
        panic!("admitted twice");
    };
    assert!(second.to_string().contains("already admitted"));
    for changed in [
        DECLARATIONS.replace("type rect extends agent", "type rect extends handle"),
        DECLARATIONS.replace(
            "native GetRectMinX takes rect which returns real",
            "native GetRectMinX takes rect which returns integer",
        ),
        DECLARATIONS.replace("native RemoveRect", "constant native RemoveRect"),
    ] {
        let Err(error) = host_from(&changed, "", 16).with_rectangles() else {
            panic!("bad contract admitted");
        };
        assert!(error.to_string().contains("synthetic:rect-contract"));
        assert!(error.to_string().contains("incompatible"));
    }
}

#[test]
fn actual_vm_executes_alias_mutation_and_removal() {
    let map = "function exercise takes nothing returns real
local rect original = Rect(-4.0, 2.0, 8.0, 6.0)
local rect other = original
local real result
call MoveRectTo(other, 30.0, 10.0)
set result = GetRectMinX(original) + GetRectCenterY(original)
call RemoveRect(other)
return result
endfunction
";
    let mut vm = host_from(DECLARATIONS, map, 16)
        .with_rectangles()
        .unwrap()
        .into_vm();
    assert_eq!(vm.call("exercise", &[]).unwrap(), Value::Real(34.0));
}

fn location_declarations() -> String {
    DECLARATIONS.replace(
        "native Rect takes",
        "type location extends agent\n\
native Location takes real x, real y returns location\n\
native MoveLocation takes location point, real x, real y returns nothing\n\
native RemoveLocation takes location point returns nothing\n\
native MoveRectToLoc takes rect area, location point returns nothing\n\
native Rect takes",
    )
}

#[test]
fn actual_vm_moves_rectangle_to_copied_location_coordinates() {
    let script = "function exercise takes nothing returns real
local rect area = Rect(-4.0, 2.0, 8.0, 6.0)
local location point = Location(30.0, 10.0)
call MoveRectToLoc(area, point)
call MoveLocation(point, -50.0, -60.0)
call RemoveLocation(point)
return GetRectMinX(area) + GetRectCenterY(area)
endfunction
";
    let mut vm = host_from(&location_declarations(), script, 16)
        .with_rectangles()
        .unwrap()
        .with_locations()
        .unwrap()
        .into_vm();
    assert_eq!(vm.call("exercise", &[]).unwrap(), Value::Real(34.0));
}

#[test]
fn location_movement_checks_admission_lifetime_contracts_and_atomic_bounds() {
    let declarations = location_declarations();
    let mut host = host_from(&declarations, "", 16).with_rectangles().unwrap();
    let area = create(&mut host, [-4., 2., 8., 6.]);
    assert!(
        host.native("MoveRectToLoc", &[area.clone(), area.clone()])
            .unwrap_err()
            .contains("locations not admitted")
    );
    let mut host = host.with_locations().unwrap();
    let point = host
        .native("Location", &[Value::Real(30.), Value::Real(10.)])
        .unwrap();
    host.native("RemoveLocation", std::slice::from_ref(&point))
        .unwrap();
    for invalid in [point, area.clone(), Value::Null] {
        let error = host
            .native("MoveRectToLoc", &[area.clone(), invalid])
            .unwrap_err();
        assert!(error.contains("MoveRectToLoc"));
        assert_eq!(read(&mut host, "GetRectMinX", &area), Value::Real(-4.));
    }
    let wide = create(&mut host, [-f32::MAX, 0., f32::MAX, 0.]);
    let point = host
        .native("Location", &[Value::Real(f32::MAX), Value::Real(0.)])
        .unwrap();
    assert!(
        host.native("MoveRectToLoc", &[wide.clone(), point])
            .unwrap_err()
            .contains("moved bounds exceed real range")
    );
    assert_eq!(
        read(&mut host, "GetRectMinX", &wide),
        Value::Real(-f32::MAX)
    );
    for bad in [
        declarations.replace(
            "location point returns nothing\nnative Rect",
            "rect point returns nothing\nnative Rect",
        ),
        declarations.replace(
            "type location extends agent",
            "type location extends handle",
        ),
    ] {
        assert!(host_from(&bad, "", 16).with_rectangles().is_err());
    }
    assert!(
        host_from(DECLARATIONS, "", 16)
            .with_rectangles()
            .unwrap()
            .native("MoveRectToLoc", &[area.clone(), area])
            .unwrap_err()
            .contains("unsupported")
    );
}
