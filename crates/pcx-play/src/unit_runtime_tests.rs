use super::*;
use pcx_script::HandleId;
type Vm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;

const DECLARATIONS: &str = r#"
type region extends agent
type unittype extends handle
constant native ConvertUnitType takes integer i returns unittype
native CreateRegion takes nothing returns region
native RegionAddRect takes region g, rect r returns nothing
native TriggerRegisterEnterRegion takes trigger t, region g, boolexpr f returns event
native TriggerRegisterLeaveRegion takes trigger t, region g, boolexpr f returns event
native RemoveUnit takes unit u returns nothing
native SetUnitFacingTimed takes unit u, real a, real d returns nothing
native SetUnitPathing takes unit u, boolean b returns nothing
native CreateUnitAtLoc takes player id, integer unitid, location whichLocation, real face returns unit
constant native IsUnitType takes unit u, unittype t returns boolean
native ClearSelection takes nothing returns nothing
native SelectUnit takes unit u, boolean b returns nothing
constant native IsUnitSelected takes unit u, player p returns boolean
type effect extends agent
native AddSpecialEffectTarget takes string model, widget w, string point returns effect
native DestroyEffect takes effect e returns nothing
constant native GetEnteringUnit takes nothing returns unit
constant native GetLeavingUnit takes nothing returns unit
globals
constant unittype UNIT_TYPE_HERO = ConvertUnitType(0)
constant unittype UNIT_TYPE_DEAD = ConvertUnitType(1)
endglobals
"#;

const MAP: &str = r#"
globals
unit A=null
unit B=null
region G=null
trigger In=null
trigger Out=null
endglobals
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'U001',16.0,16.0,0.0)
set B=CreateUnit(Player(4),'u001',80.0,16.0,0.0)
set G=CreateRegion()
call RegionAddRect(G,Rect(48.0,0.0,96.0,32.0))
set In=CreateTrigger()
set Out=CreateTrigger()
call TriggerRegisterEnterRegion(In,G,null)
call TriggerRegisterLeaveRegion(Out,G,null)
endfunction
function walk takes real x returns boolean
return IssuePointOrder(A,"gamma",x,16.0)
endfunction
"#;

fn open() -> (Vm, HandleId, HandleId) {
    let extra = format!(
        "{}\n{}{DECLARATIONS}",
        enumeration::TRIGGERS,
        super::activity::ORDERS
    );
    let h = position_host_admitting(1000, 16, MAP, &extra, &[], |h| h)
        .with_locations()
        .unwrap()
        .with_regions(crate::playercolor_host::RegionBudget {
            max_regions: 2,
            max_rect_additions: 2,
            max_events: 4,
        })
        .unwrap()
        .with_selection()
        .unwrap()
        .with_unit_effects()
        .unwrap();
    let (registry, rules) = super::activity::rules();
    let mut h = h.with_order_registry(registry).unwrap();
    h.admit_unit_activity(rules, 65536).unwrap();
    let context = Arc::clone(h.player_context().unwrap());
    let mut vm = context.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let id = |name| match vm.global(name) {
        Value::Handle(id) => id,
        _ => panic!("{name}"),
    };
    let (a, b) = (id("A"), id("B"));
    (vm, a, b)
}

fn tick(vm: &mut Vm, now: f64) {
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(now, &mut 1000).unwrap().is_none());
    h.advance_unit_activity(now, &mut 65536).unwrap();
}

#[test]
fn region_membership_fires_enter_and_leave_with_the_moving_unit() {
    let (mut vm, a, b) = open();
    let (Value::Handle(enter), Value::Handle(leave)) = (vm.global("In"), vm.global("Out")) else {
        panic!("triggers")
    };
    let h = &mut vm.host().inner;
    // B starts inside: the first poll seeds membership without firing.
    h.poll_region_events(&mut 1000).unwrap();
    assert!(h.take_fired_event().is_none());
    assert_eq!(
        vm.call("walk", &[Value::Real(64.0)]).unwrap(),
        Value::Bool(true)
    );
    let start = vm.host().inner.timer_game_time().unwrap();
    tick(&mut vm, start + 10.0);
    let h = &mut vm.host().inner;
    h.poll_region_events(&mut 1000).unwrap();
    let fired = h.take_fired_event().expect("enter");
    assert_eq!(fired.trigger, enter);
    h.begin_fired_event(&fired).unwrap().unwrap();
    assert_eq!(h.native("GetEnteringUnit", &[]).unwrap(), Value::Handle(a));
    h.finish_fired_event();
    assert!(h.take_fired_event().is_none());
    // Removing B leaves the region; removal of a removed unit is a no-op.
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    assert!(h.executed_units().all(|(id, _)| id != b));
    h.poll_region_events(&mut 1000).unwrap();
    let fired = h.take_fired_event().expect("leave");
    assert_eq!(fired.trigger, leave);
    h.begin_fired_event(&fired).unwrap().unwrap();
    assert_eq!(h.native("GetLeavingUnit", &[]).unwrap(), Value::Handle(b));
    h.finish_fired_event();
}

#[test]
fn facing_turns_over_time_and_unit_types_and_selection_read_live_state() {
    let (mut vm, a, b) = open();
    let hero = vm.global("UNIT_TYPE_HERO");
    let dead = vm.global("UNIT_TYPE_DEAD");
    let h = &mut vm.host().inner;
    let start = h.timer_game_time().unwrap();
    h.native(
        "SetUnitFacingTimed",
        &[Value::Handle(a), Value::Real(-90.0), Value::Real(2.0)],
    )
    .unwrap();
    tick(&mut vm, start + 1.0);
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().face, 315.0);
    tick(&mut vm, start + 3.0);
    let h = &mut vm.host().inner;
    assert_eq!(h.executed_unit(a).unwrap().face, 270.0);
    let is =
        |h: &mut ObjectHost, u, t: &Value| h.native("IsUnitType", &[Value::Handle(u), t.clone()]);
    assert_eq!(is(h, a, &hero).unwrap(), Value::Bool(true));
    assert_eq!(is(h, b, &hero).unwrap(), Value::Bool(false));
    assert_eq!(is(h, b, &dead).unwrap(), Value::Bool(false));
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    h.native("SelectUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    h.native("SelectUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    assert_eq!(h.selection(), &[a]);
    h.native("ClearSelection", &[]).unwrap();
    assert!(h.selection().is_empty());
    assert_eq!(
        h.native("IsUnitSelected", &[Value::Handle(a), player])
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn blocked_destinations_route_near_and_disabled_pathing_ignores_collision() {
    let (mut vm, a, b) = open();
    // B blocks the cell at x=80; A stops short instead of refusing the order.
    assert_eq!(
        vm.call("walk", &[Value::Real(80.0)]).unwrap(),
        Value::Bool(true)
    );
    let start = vm.host().inner.timer_game_time().unwrap();
    tick(&mut vm, start + 20.0);
    let x = vm.host().inner.executed_unit(a).unwrap().position.unwrap()[0];
    assert!(x > 16.0 && x < 80.0, "{x}");
    let h = &mut vm.host().inner;
    h.native("SetUnitPathing", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    assert_eq!(
        vm.call("walk", &[Value::Real(80.0)]).unwrap(),
        Value::Bool(true)
    );
    tick(&mut vm, start + 40.0);
    assert_eq!(
        vm.host().inner.executed_unit(a).unwrap().position,
        Some([80.0, 16.0])
    );
    let h = &mut vm.host().inner;
    let location = h
        .native("Location", &[Value::Real(16.0), Value::Real(16.0)])
        .unwrap();
    let player = h.native("Player", &[Value::Int(4)]).unwrap();
    let Value::Handle(c) = h
        .native(
            "CreateUnitAtLoc",
            &[
                player,
                Value::Int(i32::from_be_bytes(*b"u001")),
                location,
                Value::Real(0.0),
            ],
        )
        .unwrap()
    else {
        panic!("unit")
    };
    assert_eq!(h.executed_unit(c).unwrap().x, 16.0);
    let _ = b;
}

#[test]
fn unit_effects_keep_authored_strings_until_destroyed_or_their_unit_is_removed() {
    let (mut vm, a, b) = open();
    let h = &mut vm.host().inner;
    let add = |h: &mut ObjectHost, unit| {
        h.native(
            "AddSpecialEffectTarget",
            &[
                Value::Str("Models\\Talk.mdl".into()),
                Value::Handle(unit),
                Value::Str("overhead".into()),
            ],
        )
    };
    let first = add(h, a).unwrap();
    let second = add(h, b).unwrap();
    let live: Vec<_> = h
        .unit_effects()
        .map(|(_, e)| (e.unit, e.model.to_owned(), e.attach_point.to_owned()))
        .collect();
    assert_eq!(
        live,
        [
            (a, "Models\\Talk.mdl".to_owned(), "overhead".to_owned()),
            (b, "Models\\Talk.mdl".to_owned(), "overhead".to_owned())
        ]
    );
    h.native("DestroyEffect", std::slice::from_ref(&first))
        .unwrap();
    // Destroying again or destroying null is accepted (Human01 does both).
    h.native("DestroyEffect", &[first]).unwrap();
    h.native("DestroyEffect", &[Value::Null]).unwrap();
    assert_eq!(h.unit_effects().count(), 1);
    h.native("RemoveUnit", &[Value::Handle(b)]).unwrap();
    assert_eq!(h.unit_effects().count(), 0);
    h.native("DestroyEffect", &[second]).unwrap();
    assert!(add(h, b).is_err());
    assert!(
        h.native(
            "AddSpecialEffectTarget",
            &[Value::Str("m".into()), Value::Null, Value::Str("o".into())]
        )
        .is_err()
    );
}
