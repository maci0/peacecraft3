use super::*;
use pcx_script::HandleId;
type Vm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;
pub(super) const ORDERS: &str = r#"
native ShowUnit takes unit u, boolean shown returns nothing
constant native IsUnitHidden takes unit u returns boolean
type location extends agent
native Location takes real x, real y returns location
native MoveLocation takes location l, real x, real y returns nothing
native RemoveLocation takes location l returns nothing
native GetLocationX takes location l returns real
native GetLocationY takes location l returns real
constant native GetUnitLoc takes unit u returns location
native IssuePointOrderLoc takes unit u, string s, location l returns boolean
native IssuePointOrderByIdLoc takes unit u, integer n, location l returns boolean

constant native OrderId takes string s returns integer
constant native OrderId2String takes integer n returns string
native IssueImmediateOrder takes unit u, string s returns boolean
native IssueImmediateOrderById takes unit u, integer n returns boolean
native IssuePointOrder takes unit u, string s, real x, real y returns boolean
native IssuePointOrderById takes unit u, integer n, real x, real y returns boolean
native GetUnitCurrentOrder takes unit u returns integer
constant native GetUnitMoveSpeed takes unit u returns real
constant native GetTriggerUnit takes nothing returns unit
native TriggerRegisterUnitInRange takes trigger t, unit u, real r, boolexpr f returns event
globals
constant unitevent TEST_RANGE=ConvertUnitEvent(-47)
endglobals
"#;
const SOUNDS: &str = "native CreateSound takes string file, boolean repeating, boolean spatial, boolean stop, integer a, integer b, string eax returns sound
native AttachSoundToUnit takes sound s, unit u returns nothing
";
const MAP: &str = r#"
globals
unit A=null
unit B=null
trigger T=null
sound S=null
endglobals
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'U001',16.0,16.0,0.0)
set B=CreateUnit(Player(4),'U001',80.0,16.0,0.0)
set T=CreateTrigger()
call TriggerRegisterUnitInRange(T,B,12.0,null)
set S=CreateSound("Media\\a.wav",false,true,false,10,10,"")
endfunction
function attach takes unit u returns nothing
call AttachSoundToUnit(S,u)
endfunction
function move takes nothing returns boolean
return IssuePointOrder(A,"gamma",72.0,16.0)
endfunction
function hold takes nothing returns boolean
return IssueImmediateOrder(A,"alpha")
endfunction
function stop takes nothing returns boolean
return IssueImmediateOrderById(A,OrderId("beta"))
endfunction
"#;

pub(super) fn rules() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    use crate::{camera_margins::SourceWindow, neutral_player::sha256};
    let (mut image, mut binding, budget) = crate::order_registry::tests::fixture([431, 744]);
    let mut put = |at: usize, prefix: &[u8], id: i32| {
        let mut b = prefix.to_vec();
        b.extend_from_slice(&id.to_le_bytes());
        image[at..at + b.len()].copy_from_slice(&b);
        SourceWindow {
            va: 0x301000 + at as u32 - 0x200,
            len: b.len() as u32,
            digest: sha256(&b),
        }
    };
    let hold = put(0x320, &[0x3d], 431);
    let stop = put(0x330, &[0x81, 0xfb], 744);
    let movement = put(0x340, &[0x3d], 1631);
    binding.image_sha256 = sha256(&image);
    let registry = Arc::new(
        crate::order_registry::Registry::read(&image, binding.clone(), budget, 4, 32).unwrap(),
    );
    let rules = crate::unit_activity::Rules::read(
        &image,
        crate::unit_activity::Binding {
            origin: binding.origin,
            image_sha256: binding.image_sha256,
            hold,
            stop,
            movement,
        },
        &registry,
        budget,
    )
    .unwrap();
    (registry, rules)
}
fn host(route_work: usize) -> ObjectHost {
    host_fields(route_work, &[])
}
fn host_fields(route_work: usize, fields: &[([u8; 4], ObjectField)]) -> ObjectHost {
    use crate::file_sounds::tests::{PROPERTY_DECLARATIONS, mixed_assets};
    let extra = format!(
        "{}\n{ORDERS}native RemoveUnit takes unit u returns nothing
type sound extends agent\n{SOUNDS}{PROPERTY_DECLARATIONS}",
        enumeration::TRIGGERS
    );
    let sounds = |h: ObjectHost| {
        h.with_file_sounds(mixed_assets(), 8)
            .unwrap()
            .with_sound_properties()
            .unwrap()
    };
    let h = position_host_admitting(1000, 16, MAP, &extra, fields, sounds)
        .with_locations()
        .unwrap();
    let (registry, rules) = rules();
    let mut h = h
        .with_order_registry(registry)
        .unwrap()
        .with_unit_in_range(8)
        .unwrap();
    h.admit_unit_activity(rules, route_work).unwrap();
    h.admit_unit_range_runtime("TEST_RANGE", 16).unwrap();
    h
}
fn open() -> Vm {
    let mut h = host(65536);
    let model = crate::unit_model::tests::model();
    let mut bytes = model.bytes.to_vec();
    let at = bytes
        .windows(12)
        .position(|b| b == b"Stand Defend")
        .unwrap();
    bytes[at..at + 80].fill(0);
    bytes[at..at + 4].copy_from_slice(b"Walk");
    // The SEQS move speed field follows the interval; Walk needs one for its rate.
    bytes[at + 88..at + 92].copy_from_slice(&200f32.to_le_bytes());
    let model = Arc::new(
        crate::unit_model::Model::read(
            model.origin.clone(),
            "synthetic-walk.mdx".into(),
            bytes.into(),
            4096,
        )
        .unwrap(),
    );
    h.admit_unit_models(move |_| Ok(Some(Arc::clone(&model))))
        .unwrap();
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    vm
}
fn unit(vm: &Vm, name: &str) -> HandleId {
    let Value::Handle(id) = vm.global(name) else {
        panic!("unit")
    };
    id
}
fn tick(vm: &mut Vm, now: f64) {
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(now, &mut 1000).unwrap().is_none());
    h.advance_unit_activity(now, &mut 65536).unwrap();
}
fn position(vm: &mut Vm, id: HandleId) -> [f32; 2] {
    vm.host().inner.executed_unit(id).unwrap().position.unwrap()
}

#[test]
fn hold_stops_real_motion_and_explicit_move_resumes_on_the_same_range_and_render_records() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    let handles = vm.host().inner.inner().handles();
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 2.);
    assert_eq!(position(&mut vm, a), [32., 16.]);
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(1631)
    );
    assert_eq!(vm.call("hold", &[]).unwrap(), Value::Bool(true));
    assert_eq!(vm.call("hold", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 4.);
    assert_eq!(position(&mut vm, a), [32., 16.]);
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .held()
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(0)
    );
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 9.);
    assert_eq!(position(&mut vm, a), [72., 16.]);
    assert_eq!(position(&mut vm, b), [80., 16.]);
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitX", &[Value::Handle(a)])
            .unwrap(),
        Value::Real(72.)
    );
    assert_eq!(
        vm.host().inner.poll_unit_ranges(&mut 1000).unwrap().len(),
        1
    );
    assert_eq!(vm.host().inner.inner().handles(), handles);
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().x, 16.);
}

#[test]
fn absolute_game_time_projection_and_arrival_animation_are_partition_independent() {
    let run = |times: &[f64]| {
        let mut vm = open();
        let a = unit(&vm, "A");
        vm.call("move", &[]).unwrap();
        for &time in times {
            tick(&mut vm, time);
        }
        let record = vm.host().inner.executed_unit(a).unwrap();
        (
            record.position.unwrap().map(f32::to_bits),
            record.face.to_bits(),
            record.activity.clone(),
            record.animation.as_ref().unwrap().sample(10.).unwrap(),
        )
    };
    assert_eq!(run(&[10.]), run(&[1., 2., 4., 7., 10.]));
    assert_eq!(run(&[10.]), run(&[0., 0.125, 0.5, 3.25, 8., 10.]));
}

#[test]
fn stop_releases_hold_and_work_failure_preserves_unit_and_occupancy_state() {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    let before = position(&mut vm, a);
    assert!(
        vm.host()
            .inner
            .next_timer_expiry(1., &mut 1000)
            .unwrap()
            .is_none()
    );
    assert!(vm.host().inner.advance_unit_activity(1., &mut 0).is_err());
    assert_eq!(position(&mut vm, a), before);
    vm.call("hold", &[]).unwrap();
    assert_eq!(vm.call("stop", &[]).unwrap(), Value::Bool(true));
    let state = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .activity
        .as_ref()
        .unwrap();
    assert!(!state.held() && !state.moving());
    let mut h = host(1);
    let context = Arc::clone(h.player_context().unwrap());
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let a = h
        .native(
            "CreateUnit",
            &[
                owner,
                Value::Int(i32::from_be_bytes(*b"U001")),
                Value::Real(16.),
                Value::Real(16.),
                Value::Real(0.),
            ],
        )
        .unwrap();
    assert!(
        h.native(
            "IssuePointOrder",
            &[
                a.clone(),
                Value::Str("gamma".into()),
                Value::Real(72.),
                Value::Real(16.)
            ]
        )
        .is_err()
    );
    assert_eq!(h.native("GetUnitX", &[a]).unwrap(), Value::Real(16.));
    assert!(Arc::ptr_eq(&context, h.player_context().unwrap()));
}

fn profile(value: FieldValue, movement: Option<&str>) -> ObjectHost {
    let mut field = geometry_field(Ok(value));
    let meta = field.metadata.as_mut().unwrap();
    meta.fourcc = *b"S001";
    meta.field = "spd".into();
    meta.kind = FieldKind::Real;
    meta.declared = "real".into();
    meta.source = FieldSource::Column {
        slk: SlkId::UnitBalance,
        column: "spd".into(),
    };
    let mut fields = vec![(*b"S001", field)];
    if let Some(movement) = movement {
        let mut field = geometry_field(Ok(FieldValue::Cell(movement.into())));
        let meta = field.metadata.as_mut().unwrap();
        meta.fourcc = *b"M001";
        meta.field = "movetp".into();
        meta.kind = FieldKind::Token;
        meta.declared = "moveType".into();
        meta.source = FieldSource::Column {
            slk: SlkId::UnitData,
            column: "movetp".into(),
        };
        fields.push((*b"M001", field));
    }
    host_fields(65536, &fields)
}
#[test]
fn selected_speed_overrides_and_movement_class_control_real_displacement() {
    for (value, expected) in [
        (FieldValue::Cell("12".into()), 12.),
        (FieldValue::Override(pcx_objects::Value::Real(20.)), 20.),
    ] {
        let mut h = profile(value, None);
        let a = position_create(&mut h, 16., 16.).unwrap();
        h.native(
            "IssuePointOrder",
            &[
                a.clone(),
                Value::Str("gamma".into()),
                Value::Real(72.),
                Value::Real(16.),
            ],
        )
        .unwrap();
        assert!(h.next_timer_expiry(1., &mut 1000).unwrap().is_none());
        h.advance_unit_activity(1., &mut 65536).unwrap();
        assert_eq!(
            h.native("GetUnitX", std::slice::from_ref(&a)).unwrap(),
            Value::Real(16. + expected)
        );
        assert_eq!(
            h.native("GetUnitMoveSpeed", &[a]).unwrap(),
            Value::Real(expected)
        );
    }
    for value in ["0", "_"] {
        let mut h = profile(FieldValue::Cell(value.into()), Some("none"));
        let a = position_create(&mut h, 16., 16.).unwrap();
        assert_eq!(
            h.native(
                "IssueImmediateOrder",
                &[a.clone(), Value::Str("alpha".into())]
            )
            .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            h.native(
                "IssuePointOrder",
                &[
                    a.clone(),
                    Value::Str("gamma".into()),
                    Value::Real(72.),
                    Value::Real(16.)
                ]
            )
            .unwrap(),
            Value::Bool(false)
        );
        let query = h.native("GetUnitMoveSpeed", &[a]);
        if value == "0" {
            assert_eq!(query.unwrap(), Value::Real(0.));
        } else {
            assert!(query.is_err(), "a marker is not an invented zero speed");
        }
    }
}

#[test]
fn malformed_orders_and_profiles_refuse_without_position_or_handle_mutation() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let count = vm.host().inner.inner().handles();
    for args in [
        vec![
            Value::Handle(a),
            Value::Str("gamma".into()),
            Value::Real(f32::NAN),
            Value::Real(16.),
        ],
        vec![
            Value::Int(1),
            Value::Str("gamma".into()),
            Value::Real(32.),
            Value::Real(16.),
        ],
    ] {
        assert!(vm.host().inner.native("IssuePointOrder", &args).is_err());
    }
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrder",
                &[Value::Null, Value::Str("alpha".into())]
            )
            .unwrap(),
        Value::Bool(false)
    );
    assert!(
        vm.host()
            .inner
            .native(
                "IssueImmediateOrder",
                &[Value::Handle(a), Value::Str("unqualified".into())]
            )
            .is_err()
    );
    assert_eq!(position(&mut vm, a), [16., 16.]);
    assert_eq!(vm.host().inner.inner().handles(), count);
    for value in ["_", "NaN", "-1"] {
        let mut h = profile(FieldValue::Cell(value.into()), None);
        let owner = h.native("Player", &[Value::Int(4)]).unwrap();
        let before = h.inner().handles();
        assert!(
            h.native(
                "CreateUnit",
                &[
                    owner,
                    Value::Int(i32::from_be_bytes(*b"U001")),
                    Value::Real(16.),
                    Value::Real(16.),
                    Value::Real(0.)
                ]
            )
            .is_err()
        );
        assert_eq!(h.inner().handles(), before);
    }
}

#[test]
fn location_orders_copy_their_destination_and_locations_keep_their_own_lifetime() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let location = vm
        .host()
        .inner
        .native("Location", &[Value::Real(72.), Value::Real(16.)])
        .unwrap();
    let snapshot = vm
        .host()
        .inner
        .native("GetUnitLoc", &[Value::Handle(a)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native(
                "IssuePointOrderByIdLoc",
                &[Value::Handle(a), Value::Int(1631), location.clone()]
            )
            .unwrap(),
        Value::Bool(true)
    );
    vm.host()
        .inner
        .native(
            "MoveLocation",
            &[location.clone(), Value::Real(120.), Value::Real(100.)],
        )
        .unwrap();
    vm.host()
        .inner
        .native("RemoveLocation", std::slice::from_ref(&location))
        .unwrap();
    tick(&mut vm, 10.);
    assert_eq!(position(&mut vm, a), [72., 16.]);
    assert_eq!(
        vm.host().inner.native("GetLocationX", &[snapshot]).unwrap(),
        Value::Real(16.)
    );
    assert!(vm.host().inner.native("GetLocationX", &[location]).is_err());
}

#[test]
fn motion_projection_requires_the_current_monotonic_clock_and_is_idempotent() {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    let before = position(&mut vm, a);
    assert!(
        vm.host()
            .inner
            .advance_unit_activity(1., &mut 65536)
            .is_err()
    );
    assert_eq!(position(&mut vm, a), before);
    tick(&mut vm, 1.);
    let after = position(&mut vm, a);
    assert_eq!(
        vm.host().inner.advance_unit_activity(1., &mut 0).unwrap(),
        0
    );
    assert!(
        vm.host()
            .inner
            .advance_unit_activity(0., &mut 65536)
            .is_err()
    );
    assert_eq!(position(&mut vm, a), after);
}

#[test]
fn hidden_units_keep_moving_but_release_collision_and_leave_spatial_enumerations() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    vm.call("move", &[]).unwrap();
    let h = &mut vm.host().inner;
    let group = h.native("CreateGroup", &[]).unwrap();
    let enumerated = h.native("CreateGroup", &[]).unwrap();
    let rect = h
        .native(
            "Rect",
            &[
                Value::Real(0.),
                Value::Real(0.),
                Value::Real(100.),
                Value::Real(100.),
            ],
        )
        .unwrap();
    h.native("GroupAddUnit", &[group.clone(), Value::Handle(a)])
        .unwrap();
    assert_eq!(
        h.native("IsUnitHidden", &[Value::Handle(a)]).unwrap(),
        Value::Bool(false)
    );
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert_eq!(
        h.native("IsUnitInGroup", &[Value::Handle(a), group])
            .unwrap(),
        Value::Bool(true),
        "existing groups retain the hidden unit for later ShowUnit"
    );
    h.native(
        "GroupEnumUnitsInRect",
        &[enumerated.clone(), rect.clone(), Value::Null],
    )
    .unwrap();
    assert_eq!(
        h.native("IsUnitInGroup", &[Value::Handle(a), enumerated.clone()])
            .unwrap(),
        Value::Bool(false)
    );
    let c = position_create(h, 16., 16.).unwrap();
    let Value::Handle(c) = c else { panic!("unit") };
    assert_eq!(h.executed_unit(c).unwrap().position, Some([16., 16.]));
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(1631)
    );
    assert_eq!(
        vm.call("move", &[]).unwrap(),
        Value::Bool(true),
        "hidden orders remain accepted"
    );
    tick(&mut vm, 7.015625);
    assert_eq!(position(&mut vm, a), [72., 16.]);
    assert!(
        vm.host()
            .inner
            .poll_unit_ranges(&mut 1000)
            .unwrap()
            .is_empty()
    );
    let h = &mut vm.host().inner;
    let handles = h.inner().handles();
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    h.native(
        "GroupEnumUnitsInRect",
        &[enumerated.clone(), rect, Value::Null],
    )
    .unwrap();
    assert_eq!(
        h.native("IsUnitInGroup", &[Value::Handle(a), enumerated])
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(h.poll_unit_ranges(&mut 1000).unwrap().len(), 1);
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        h.poll_unit_ranges(&mut 1000).unwrap().len(),
        1,
        "hide/show clears the previous spatial membership even without an intervening poll"
    );
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    h.native("ShowUnit", &[Value::Handle(b), Value::Bool(false)])
        .unwrap();
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    assert_eq!(
        h.poll_unit_ranges(&mut 1000).unwrap().len(),
        1,
        "a hidden anchor still delivers a visible entrant"
    );
    h.native("ShowUnit", &[Value::Handle(b), Value::Bool(true)])
        .unwrap();
    assert!(
        h.poll_unit_ranges(&mut 1000).unwrap().is_empty(),
        "revealing an anchor does not invent a second entry"
    );
    assert_eq!(h.inner().handles(), handles);
}

#[test]
fn reveal_relocates_blocked_space_without_losing_the_unit_or_its_movement_intent() {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    let h = &mut vm.host().inner;
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    position_create(h, 16., 16.).unwrap();
    let count = h.inner().handles();
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    let revealed = h.executed_unit(a).unwrap().position.unwrap();
    assert_ne!(revealed, [16., 16.]);
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(1631)
    );
    assert_eq!(h.inner().handles(), count);
    tick(&mut vm, 0.125);
    let next = position(&mut vm, a);
    let distance = (next[0] - revealed[0]).hypot(next[1] - revealed[1]);
    assert!(
        (distance - 1.).abs() < 0.0001,
        "movement restarts from the accepted reveal location, not the old path origin"
    );
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().x, 16.);
}

#[test]
fn visibility_rejects_invalid_handles_and_shapes_without_mutation() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    let h = &mut vm.host().inner;
    let owner = h.executed_unit(a).unwrap().owner;
    for args in [
        vec![Value::Handle(a), Value::Int(0)],
        vec![Value::Handle(owner), Value::Bool(false)],
    ] {
        assert!(h.native("ShowUnit", &args).is_err());
    }
    assert_eq!(
        h.native("ShowUnit", &[Value::Null, Value::Bool(false)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        h.native("IsUnitHidden", &[Value::Null]).unwrap(),
        Value::Bool(false)
    );
    assert!(h.native("IsUnitHidden", &[Value::Handle(owner)]).is_err());
    h.native("ShowUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert!(!h.executed_unit(a).unwrap().shown);
    assert!(h.executed_unit(b).unwrap().shown);
}

#[test]
fn attached_sounds_follow_the_accepted_unit_until_an_explicit_position_detaches() {
    let mut vm = open();
    let (a, b) = (unit(&vm, "A"), unit(&vm, "B"));
    let Value::Handle(s) = vm.global("S") else {
        panic!("sound")
    };
    let attached = |vm: &mut Vm| vm.host().inner.sound_attached_position(s).unwrap();
    assert_eq!(attached(&mut vm), None);
    vm.call("attach", &[Value::Handle(a)]).unwrap();
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 2.);
    assert_eq!(attached(&mut vm), Some([32., 16.]));
    tick(&mut vm, 9.);
    assert_eq!(attached(&mut vm), Some([72., 16.]));
    vm.call("attach", &[Value::Handle(b)]).unwrap();
    assert_eq!(attached(&mut vm), Some([80., 16.]));
    // Null sound or unit skips without detaching (native wrapper evidence).
    vm.call("attach", &[Value::Null]).unwrap();
    assert_eq!(attached(&mut vm), Some([80., 16.]));
    let h = &mut vm.host().inner;
    let at = [
        Value::Handle(s),
        Value::Real(1.),
        Value::Real(2.),
        Value::Real(3.),
    ];
    h.native("SetSoundPosition", &at[..3]).unwrap_err();
    assert_eq!(h.sound_attached_position(s).unwrap(), Some([80., 16.]));
    h.native("SetSoundPosition", &at).unwrap();
    assert_eq!(h.sound_attached_position(s).unwrap(), None);
    assert_eq!(h.sound_properties(s).unwrap().position, Some([1., 2., 3.]));
    let handles = h.inner().handles();
    for args in [
        [Value::Handle(a), Value::Handle(s)],
        [Value::Handle(s), Value::Handle(s)],
        [Value::Handle(s), Value::Int(1)],
    ] {
        h.native("AttachSoundToUnit", &args).unwrap_err();
    }
    assert_eq!(h.sound_attached_position(s).unwrap(), None);
    assert_eq!(h.inner().handles(), handles);
}

#[test]
fn removed_units_refuse_supported_orders_without_accepting_other_handle_families() {
    let mut vm = open();
    let (a, b) = (unit(&vm, "A"), unit(&vm, "B"));
    let h = &mut vm.host().inner;
    let location = h
        .native("Location", &[Value::Real(72.0), Value::Real(32.0)])
        .unwrap();
    let hold = h.native("OrderId", &[Value::Str("alpha".into())]).unwrap();
    let movement = h.native("OrderId", &[Value::Str("gamma".into())]).unwrap();
    h.native("RemoveUnit", &[Value::Handle(a)]).unwrap();
    assert_eq!(h.inner().kind(a).unwrap(), "unit");
    let before = h.executed_unit(b).unwrap().clone();
    let handles = h.inner().handles();
    for (name, tail) in [
        ("IssueImmediateOrder", vec![Value::Str("alpha".into())]),
        ("IssueImmediateOrderById", vec![hold]),
        (
            "IssuePointOrder",
            vec![
                Value::Str("gamma".into()),
                Value::Real(72.0),
                Value::Real(32.0),
            ],
        ),
        (
            "IssuePointOrderById",
            vec![movement.clone(), Value::Real(72.0), Value::Real(32.0)],
        ),
        (
            "IssuePointOrderLoc",
            vec![Value::Str("gamma".into()), location.clone()],
        ),
        ("IssuePointOrderByIdLoc", vec![movement, location.clone()]),
    ] {
        let args = [vec![Value::Handle(a)], tail.clone()].concat();
        assert_eq!(h.native(name, &args).unwrap(), Value::Bool(false), "{name}");
        let wrong_family = [vec![location.clone()], tail.clone()].concat();
        assert!(h.native(name, &wrong_family).is_err(), "{name}");
        let unknown = Value::Handle(HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap()));
        let unknown = [vec![unknown], tail].concat();
        assert!(
            h.native(name, &unknown)
                .unwrap_err()
                .contains("unknown handle"),
            "{name}"
        );
        assert!(h.executed_unit(a).is_err());
        assert_eq!(h.executed_unit(b).unwrap(), &before);
        assert_eq!(h.inner().handles(), handles);
    }
    // A later live unit cannot revive the removed identity.
    let new = position_create(h, 16.0, 16.0).unwrap();
    assert_ne!(new, Value::Handle(a));
    assert!(h.executed_unit(a).is_err());
    assert_eq!(
        h.native(
            "IssueImmediateOrder",
            &[Value::Handle(a), Value::Str("alpha".into())]
        )
        .unwrap(),
        Value::Bool(false)
    );
}

/// Live placement cell of a point. The fixture grid has cell size 4.
fn cell_of(point: [f32; 2]) -> [i64; 2] {
    point.map(|v| (v / 4.0).floor() as i64)
}

/// Placement footprints overlap when their cells overlap. The fixture collision 2
/// divided by cell size 4 is 0.5, which selects the first radius band (thresholds
/// 0.5, 1, 1.5) and extents [1, 0]: a unit covers cells `c - 1 ..= c` on each
/// axis. Two footprints overlap when their centre cells differ by at most one on
/// both axes.
fn footprints_overlap(a: [i64; 2], b: [i64; 2]) -> bool {
    (0..2).all(|axis| (a[axis] - b[axis]).abs() <= 1)
}

/// Work for the enclosed tick, counted from the fixture operations: 1 unit tick,
/// 1 loop iteration, and 2 segment samples from 16 (sample 1 is cell 4, clear;
/// sample 2 at x 20 is cell 5, blocked by the east blocker). The unit walks to
/// sample 1 (x 18). The replan from 18 spends 1 on the blocked first sample, 1
/// start charge, 1 expansion of its cell, and 8 for the blocked neighbours; it
/// makes no progress. Total 4 + 11 = 15. Corner checks never run.
const ENCLOSED_TICK_WORK: usize = 15;

fn move_speed(vm: &mut Vm, id: HandleId) -> f32 {
    let Value::Real(speed) = vm
        .host()
        .inner
        .native("GetUnitMoveSpeed", &[Value::Handle(id)])
        .unwrap()
    else {
        panic!("real move speed");
    };
    speed
}

/// Work spent by one timer tick at `now`, from a fixed budget.
fn tick_work(vm: &mut Vm, now: f64) -> usize {
    const BUDGET: usize = 65536;
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(now, &mut 1000).unwrap().is_none());
    let mut work = BUDGET;
    h.advance_unit_activity(now, &mut work).unwrap();
    BUDGET - work
}

#[test]
fn blocked_motion_replans_around_a_unit_that_enters_its_path() {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    tick(&mut vm, 2.);
    assert_eq!(position(&mut vm, a), [32., 16.]);
    let blocker = {
        let h = &mut vm.host().inner;
        position_create(h, 48., 16.).unwrap()
    };
    let Value::Handle(blocker) = blocker else {
        panic!("blocker unit");
    };
    let blocker_at = position(&mut vm, blocker);
    let speed = move_speed(&mut vm, a);
    let mut samples = vec![(2., [32., 16.])];
    for now in [4., 6., 8., 10., 12., 14., 16., 18., 20., 30., 40., 50.] {
        tick(&mut vm, now);
        samples.push((now, position(&mut vm, a)));
    }
    for pair in samples.windows(2) {
        let (before, after) = (pair[0], pair[1]);
        let travelled = (after.1[0] - before.1[0]).hypot(after.1[1] - before.1[1]);
        assert!(
            f64::from(travelled) <= f64::from(speed) * (after.0 - before.0) + 1e-4,
            "{travelled} exceeds speed {speed} over {}..{}",
            before.0,
            after.0
        );
    }
    let samples: Vec<[f32; 2]> = samples.into_iter().map(|(_, p)| p).collect();
    assert!(
        samples.iter().any(|p| p[1] != 16.),
        "the route leaves the straight line around the blocker"
    );
    assert!(
        samples
            .iter()
            .all(|p| !footprints_overlap(cell_of(*p), cell_of(blocker_at))),
        "no sampled position overlaps the blocker's placement footprint"
    );
    assert_eq!(samples.last(), Some(&[72., 16.]));
    assert!(
        !vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
}

#[test]
fn enclosed_motion_stops_without_progress_and_spends_no_work_on_later_ticks() {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    {
        let h = &mut vm.host().inner;
        // Four blockers, two cells from A's cell (4, 4) on each side, forbid all eight neighbours.
        for at in [[26., 16.], [10., 16.], [16., 26.], [16., 10.]] {
            position_create(h, at[0], at[1]).unwrap();
        }
    }
    let first = tick_work(&mut vm, 2.);
    assert_eq!(position(&mut vm, a), [18., 16.]);
    assert!(
        !vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving(),
        "a replan with no progress stops the unit"
    );
    assert_eq!(first, ENCLOSED_TICK_WORK);
    // Stays put at the probe's 0.0625 s cadence with no new order: no route search
    // and no credited movement. The order query after a stop is not asserted; see
    // docs/re-notes/unit-collision-replanning.md.
    for step in 1..=104 {
        let now = 2. + f64::from(step) * 0.0625;
        assert_eq!(tick_work(&mut vm, now), 0, "work after the stop at {now}");
    }
    assert_eq!(position(&mut vm, a), [18., 16.]);
}

#[test]
fn unreachable_destination_keeps_the_requested_goal_and_never_reports_a_shortened_arrival() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let goal = [56., 16.];
    {
        let h = &mut vm.host().inner;
        let blocker = position_create(h, goal[0], goal[1]).unwrap();
        let Value::Handle(blocker) = blocker else {
            panic!("blocker unit");
        };
        assert_eq!(position(&mut vm, blocker), goal);
        let h = &mut vm.host().inner;
        assert_eq!(
            h.native(
                "IssuePointOrder",
                &[
                    Value::Handle(a),
                    Value::Str("gamma".into()),
                    Value::Real(goal[0]),
                    Value::Real(goal[1]),
                ],
            )
            .unwrap(),
            Value::Bool(true)
        );
    }
    tick(&mut vm, 2.);
    let activity = |vm: &mut Vm| {
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .activity
            .clone()
            .unwrap()
    };
    assert_eq!(activity(&mut vm).destination(), Some(goal));
    tick(&mut vm, 40.);
    let end = position(&mut vm, a);
    assert_ne!(
        end, goal,
        "a route that stops short does not arrive at the goal"
    );
    assert!(
        end[0] < goal[0],
        "the unit stops on its own side of the blocker"
    );
    assert!(!activity(&mut vm).moving());
    assert_eq!(activity(&mut vm).destination(), None);
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(0)
    );
}

#[test]
fn obstruction_after_long_travel_grants_no_distance_on_the_replan_tick() {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    tick(&mut vm, 2.);
    let before = position(&mut vm, a);
    let speed = move_speed(&mut vm, a);
    {
        let h = &mut vm.host().inner;
        position_create(h, 48., 16.).unwrap();
    }
    // Ten seconds of motion from the order would reach the goal unobstructed.
    // The blocker's footprint starts at cell 11 (x 44); the last clear sample on
    // the 2-unit grid from x 32 is x 42. Nothing past it is credited on this tick.
    tick(&mut vm, 10.);
    let stopped = position(&mut vm, a);
    assert_eq!(stopped, [42., 16.]);
    assert_eq!(before, [32., 16.]);
    tick(&mut vm, 12.);
    let resumed = position(&mut vm, a);
    let travelled = (resumed[0] - stopped[0]).hypot(resumed[1] - stopped[1]);
    assert!(
        f64::from(travelled) <= f64::from(speed) * 2. + 1e-4,
        "{travelled} exceeds speed {speed} over two seconds"
    );
    assert!(resumed != stopped, "the unit resumes on the new route");
}

/// One projection through the span handoff; returns the work it spent.
fn tick_spans(
    vm: &mut Vm,
    now: f64,
    spans: &mut Vec<crate::playercolor_host::AcceptedSpan>,
) -> usize {
    const BUDGET: usize = 65536;
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(now, &mut 1000).unwrap().is_none());
    let mut work = BUDGET;
    h.advance_unit_activity_spans(now, &mut work, spans)
        .unwrap();
    BUDGET - work
}

#[test]
fn accepted_spans_chain_across_projections_and_cover_a_blocked_stop_on_host_time() {
    use crate::playercolor_host::AcceptedSpan;
    let mut vm = open();
    let a = unit(&vm, "A");
    let speed = move_speed(&mut vm, a);
    vm.call("move", &[]).unwrap();
    let mut spans: Vec<AcceptedSpan> = Vec::new();
    tick_spans(&mut vm, 2., &mut spans);
    assert_eq!(
        spans,
        [AcceptedSpan {
            unit: a,
            from: [16., 16.],
            to: [32., 16.],
            start: 0.,
            end: 2.,
            moving: true,
        }]
    );
    {
        let h = &mut vm.host().inner;
        position_create(h, 48., 16.).unwrap();
    }
    let appended = spans.len();
    let work = tick_spans(&mut vm, 10., &mut spans);
    // Eight units of a 40-unit span stop on the sample before cell 11 (x 44): travelled 10 units in 1.25 s.
    assert_eq!(
        spans[appended..],
        [
            AcceptedSpan {
                unit: a,
                from: [32., 16.],
                to: [42., 16.],
                start: 2.,
                end: 3.25,
                moving: true,
            },
            AcceptedSpan {
                unit: a,
                from: [42., 16.],
                to: [42., 16.],
                start: 3.25,
                end: 10.,
                moving: false,
            },
        ]
    );
    assert!(spans.len() - appended <= work, "spans are paid for by work");
    let appended = spans.len();
    for now in (12..=40).step_by(2).map(f64::from) {
        tick_spans(&mut vm, now, &mut spans);
    }
    let fresh = &spans[appended..];
    assert_eq!(fresh.first().map(|s| s.start), Some(10.));
    assert_eq!(fresh.first().map(|s| s.from), Some([42., 16.]));
    for pair in spans.windows(2) {
        assert_eq!(pair[0].end, pair[1].start, "spans are contiguous in time");
        assert_eq!(pair[0].to, pair[1].from, "spans are contiguous in position");
    }
    for span in spans.iter().filter(|s| s.moving) {
        let travelled = (span.to[0] - span.from[0]).hypot(span.to[1] - span.from[1]);
        assert!(
            f64::from(travelled) <= f64::from(speed) * (span.end - span.start) + 1e-4,
            "{span:?} exceeds speed {speed}"
        );
    }
    assert_eq!(
        spans.last().map(|s| s.to),
        Some(position(&mut vm, a)),
        "the consumer's last point is the accepted position"
    );
}

#[test]
fn failed_span_projection_removes_its_spans_from_the_callers_buffer() {
    use crate::playercolor_host::AcceptedSpan;
    let mut vm = open();
    unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    let mut spans: Vec<AcceptedSpan> = Vec::new();
    tick_spans(&mut vm, 2., &mut spans);
    {
        let h = &mut vm.host().inner;
        position_create(h, 48., 16.).unwrap();
    }
    let before = spans.clone();
    // Budget 22: the unit tick and loop step cost 2, the 20-sample clear check fits,
    // six samples are then spent before the blocked one, leaving 14. The replan route
    // needs a 15-sample segment, so it refuses after the partial stop and tail were emitted.
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(10., &mut 1000).unwrap().is_none());
    assert!(
        h.advance_unit_activity_spans(10., &mut 22, &mut spans)
            .is_err(),
        "the replan runs out of work after spans were emitted"
    );
    assert_eq!(spans, before, "a failed projection leaves no spans behind");
}
