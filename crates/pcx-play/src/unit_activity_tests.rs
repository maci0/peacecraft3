use super::*;
use pcx_script::HandleId;
#[path = "campaign_captain_host_tests.rs"]
mod captains;
#[path = "unit_follow_tests.rs"]
mod follow;
#[path = "campaign_gathering_host_tests.rs"]
pub(super) mod gathering;
#[path = "unit_patrol_tests.rs"]
mod patrol;
#[path = "vision_host_tests.rs"]
mod vision;
type Vm = pcx_script::OwnedVm<crate::scripts::OwnedLocalizedHost<ObjectHost>>;
pub(super) const ORDERS: &str = r#"
native PauseUnit takes unit u, boolean paused returns nothing
native IsUnitPaused takes unit u returns boolean
native ShowUnit takes unit u, boolean shown returns nothing
native SetUnitOwner takes unit u, player p, boolean changeColor returns nothing
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
native IssueBuildOrderById takes unit u, integer n, real x, real y returns boolean
native GetUnitCurrentOrder takes unit u returns integer
constant native GetUnitMoveSpeed takes unit u returns real
constant native GetUnitDefaultMoveSpeed takes unit u returns real
native SetUnitMoveSpeed takes unit u, real speed returns nothing
constant native GetTriggerUnit takes nothing returns unit
native TriggerRegisterUnitInRange takes trigger t, unit u, real r, boolexpr f returns event
type playerstate extends handle
native SetPlayerState takes player p, playerstate setting, integer value returns nothing
constant native SetPlayerTechResearched takes player whichPlayer, integer techid, integer setToLevel returns nothing
constant native GetPlayerState takes player p, playerstate setting returns integer
constant native ConvertPlayerState takes integer i returns playerstate
globals
constant unitevent TEST_RANGE=ConvertUnitEvent(-47)
constant playerstate PLAYER_STATE_RESOURCE_GOLD=ConvertPlayerState(1)
constant playerstate PLAYER_STATE_RESOURCE_LUMBER=ConvertPlayerState(2)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_CAP=ConvertPlayerState(4)
constant playerstate PLAYER_STATE_RESOURCE_FOOD_USED=ConvertPlayerState(5)
constant playerstate PLAYER_STATE_FOOD_CAP_CEILING=ConvertPlayerState(6)
constant playerstate PLAYER_STATE_GOLD_GATHERED=ConvertPlayerState(15)
constant playerstate PLAYER_STATE_LUMBER_GATHERED=ConvertPlayerState(16)
endglobals
"#;
#[path = "harvest_runtime_tests.rs"]
mod harvest;
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
    rules_with_patrol(2122)
}

const STOCK_MOVE_SPEED_MISC: &str =
    "[Misc]\nMinUnitSpeed=10\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300\n";

fn move_speed_rules() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    move_speed_rules_with(STOCK_MOVE_SPEED_MISC)
}

fn move_speed_rules_with(
    profile: &'static str,
) -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    let (registry, rules) = rules();
    let rules = rules
        .with_move_speed_profiles(&[("synthetic game misc", profile.as_bytes())], 4096)
        .unwrap();
    (registry, rules)
}

fn rules_with_patrol(
    patrol_id: i32,
) -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    use crate::{camera_margins::SourceWindow, neutral_player::sha256};
    let (mut image, mut binding, budget) = crate::order_registry::tests::fixture([431, 744]);
    // Replace the fixture's duplicate registration with a fourth synthetic order.
    let third = 0x220 + 9 + 15 + 17;
    image[third + 1..third + 5].copy_from_slice(&0x301480u32.to_le_bytes());
    image[third + 6..third + 10].copy_from_slice(&patrol_id.to_le_bytes());
    image[0x680..0x686].copy_from_slice(b"delta\0");
    binding.initializer.digest = sha256(&image[0x220..0x220 + binding.initializer.len as usize]);
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
    let patrol = put(0x350, &[0x3d], patrol_id);
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
            patrol,
        },
        &registry,
        budget,
    )
    .unwrap()
    .with_follow_profiles(
        &[(
            "synthetic follow misc",
            b"[Misc]\nFollowRange=12\nStructureFollowRange=4\n",
        )],
        4096,
    )
    .unwrap();
    (registry, rules)
}

pub(super) fn rules_with_setrally() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    rules_with_extra_order("setrally")
}

/// The fixture registry with a fifth order, `darkconversion`.
pub(super) fn rules_with_darkconversion() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    rules_with_extra_order("darkconversion")
}

pub(super) fn rules_with_repair() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    rules_with_extra_order("repair")
}

/// The fixture registry with a fifth order, `attackground`, and the stock follow profile.
pub(super) fn rules_with_attackground() -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    let (registry, rules) = rules_with_extra_order("attackground");
    let rules = rules
        .with_follow_profiles(
            &[(
                "synthetic follow misc",
                b"[Misc]\nFollowRange=12\nStructureFollowRange=4\n",
            )],
            4096,
        )
        .unwrap();
    (registry, rules)
}

fn rules_with_extra_order(
    order: &str,
) -> (
    Arc<crate::order_registry::Registry>,
    crate::unit_activity::Rules,
) {
    use crate::neutral_player::sha256;
    let (mut image, mut binding, budget) = crate::order_registry::tests::fixture([431, 744]);
    let third = 0x220 + 9 + 15 + 17;
    image[third + 1..third + 5].copy_from_slice(&0x301480u32.to_le_bytes());
    image[third + 6..third + 10].copy_from_slice(&2122i32.to_le_bytes());
    image[0x680..0x686].copy_from_slice(b"delta\0");
    let start = 0x220usize;
    let old_len = binding.initializer.len as usize;
    let insert_at = start + old_len - 2;
    let name_va = 0x3014c0u32;
    let rally_id = 9001i32;
    let mut reg = vec![0x68];
    reg.extend_from_slice(&name_va.to_le_bytes());
    reg.push(0x68);
    reg.extend_from_slice(&rally_id.to_le_bytes());
    reg.extend_from_slice(&[0x8b, 0xce, 0xe8]);
    let va_next = 0x301020 + (insert_at + reg.len() + 4 - start) as u32;
    reg.extend_from_slice(&(0x301100i32 - va_next as i32).to_le_bytes());
    image.copy_within(insert_at..insert_at + 2, insert_at + reg.len());
    image[insert_at..insert_at + reg.len()].copy_from_slice(&reg);
    let name = [order.as_bytes(), &[0]].concat();
    image[0x6c0..0x6c0 + name.len()].copy_from_slice(&name);
    binding.initializer.len = (old_len + reg.len()) as u32;
    let mut put = |at: usize, prefix: &[u8], id: i32| {
        let mut bytes = prefix.to_vec();
        bytes.extend_from_slice(&id.to_le_bytes());
        image[at..at + bytes.len()].copy_from_slice(&bytes);
        crate::camera_margins::SourceWindow {
            va: 0x301000 + at as u32 - 0x200,
            len: bytes.len() as u32,
            digest: sha256(&bytes),
        }
    };
    let hold = put(0x320, &[0x3d], 431);
    let stop = put(0x330, &[0x81, 0xfb], 744);
    let movement = put(0x340, &[0x3d], 1631);
    let patrol = put(0x350, &[0x3d], 2122);
    binding.initializer.digest = sha256(&image[start..start + binding.initializer.len as usize]);
    binding.image_sha256 = sha256(&image);
    let registry = Arc::new(
        crate::order_registry::Registry::read(&image, binding.clone(), budget, 5, 32).unwrap(),
    );
    let rules = crate::unit_activity::Rules::read(
        &image,
        crate::unit_activity::Binding {
            origin: binding.origin,
            image_sha256: binding.image_sha256,
            hold,
            stop,
            movement,
            patrol,
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
    host_fields_with(route_work, fields, rules())
}
fn host_fields_with(
    route_work: usize,
    fields: &[([u8; 4], ObjectField)],
    rules: (
        Arc<crate::order_registry::Registry>,
        crate::unit_activity::Rules,
    ),
) -> ObjectHost {
    host_fields_with_placed(route_work, fields, &[], rules)
}
fn host_fields_with_placed(
    route_work: usize,
    fields: &[([u8; 4], ObjectField)],
    placed: &[crate::playercolor_host::PlacedDestructable],
    (registry, rules): (
        Arc<crate::order_registry::Registry>,
        crate::unit_activity::Rules,
    ),
) -> ObjectHost {
    host_fields_with_placed_source(route_work, fields, placed, (registry, rules), MAP, "")
}
fn host_fields_with_placed_source(
    route_work: usize,
    fields: &[([u8; 4], ObjectField)],
    placed: &[crate::playercolor_host::PlacedDestructable],
    (registry, rules): (
        Arc<crate::order_registry::Registry>,
        crate::unit_activity::Rules,
    ),
    map: &str,
    declarations: &str,
) -> ObjectHost {
    use crate::file_sounds::tests::{PROPERTY_DECLARATIONS, mixed_assets};
    let extra = format!(
        "{}\n{ORDERS}{declarations}native RemoveUnit takes unit u returns nothing
native SetUnitPosition takes unit u, real x, real y returns nothing
native SetUnitAnimation takes unit whichUnit, string whichAnimation returns nothing
native QueueUnitAnimation takes unit whichUnit, string whichAnimation returns nothing
native IssueTargetOrder takes unit u, string s, widget target returns boolean
native IssueTargetOrderById takes unit u, integer n, widget target returns boolean
type unittype extends handle
constant native ConvertUnitType takes integer n returns unittype
constant native IsUnitType takes unit u, unittype kind returns boolean
globals
constant unittype UNIT_TYPE_STRUCTURE=ConvertUnitType(-91)
constant unittype UNIT_TYPE_PEON=ConvertUnitType(16)
endglobals
native SetUnitPathing takes unit u, boolean enabled returns nothing
type sound extends agent\n{SOUNDS}{PROPERTY_DECLARATIONS}",
        enumeration::TRIGGERS
    );
    let sounds = |h: ObjectHost| {
        h.with_file_sounds(mixed_assets(), 8)
            .unwrap()
            .with_sound_properties()
            .unwrap()
    };
    let h = position_host_admitting_placed(1000, 16, map, &extra, fields, placed, sounds)
        .with_locations()
        .unwrap();
    let mut h = h
        .with_order_registry(registry)
        .unwrap()
        .with_unit_in_range(8)
        .unwrap();
    h.admit_unit_activity(rules, route_work).unwrap();
    h.admit_unit_range_runtime("TEST_RANGE", 16).unwrap();
    h
}
pub(super) fn open() -> Vm {
    open_with_fields(&[])
}
fn open_with_fields(fields: &[([u8; 4], ObjectField)]) -> Vm {
    open_admitting(fields, rules())
}
fn open_move_speed() -> Vm {
    open_move_speed_with(STOCK_MOVE_SPEED_MISC, "10")
}
fn open_move_speed_with(profile: &'static str, authored_speed: &str) -> Vm {
    open_admitting(
        &profile_fields(
            FieldValue::Cell(authored_speed.into()),
            Some("foot"),
            Some(false),
        ),
        move_speed_rules_with(profile),
    )
}
fn open_admitting(
    fields: &[([u8; 4], ObjectField)],
    (registry, rules): (
        Arc<crate::order_registry::Registry>,
        crate::unit_activity::Rules,
    ),
) -> Vm {
    let mut h = host_fields_with(65536, fields, (registry, rules));
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
fn queued_animation_advances_on_the_simulation_clock_with_one_work_unit() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let now = vm.host().inner.timer_game_time().unwrap();
    let host = &mut vm.host().inner;
    host.native(
        "SetUnitAnimation",
        &[Value::Handle(a), Value::Str("Death".into())],
    )
    .unwrap();
    host.native(
        "QueueUnitAnimation",
        &[Value::Handle(a), Value::Str("Death".into())],
    )
    .unwrap();
    host.set_timer_game_time(now + 0.5).unwrap();
    host.advance_unit_activity(now + 0.5, &mut 1).unwrap();
    let animation = host.executed_unit(a).unwrap().animation.as_ref().unwrap();
    assert_eq!(animation.sequence, Some(2));
    assert_eq!(animation.started, now + 0.4);
    assert!(animation.needs_advance());
    host.set_timer_game_time(1.).unwrap();
    host.advance_unit_activity(1., &mut 1).unwrap();
    let animation = host.executed_unit(a).unwrap().animation.as_ref().unwrap();
    assert_eq!(animation.sequence, Some(0));
    assert_eq!(animation.started, now + 0.8);
    assert!(!animation.has_pending());
    assert_eq!(animation.sample(1.).unwrap().unwrap().frame, 300);
}

#[test]
fn pause_request_history_tracks_only_successful_requests() {
    use crate::ai_placement::PlacementEventKind;
    let mut vm = open();
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    let host = &mut vm.host().inner;
    assert_eq!(host.placement_events(a).unwrap().count(), 0);
    host.native("IsUnitPaused", &[Value::Handle(a)]).unwrap();
    assert_eq!(host.placement_events(a).unwrap().count(), 0);
    assert!(
        host.native("PauseUnit", &[Value::Handle(a), Value::Int(1)])
            .is_err()
    );
    assert_eq!(host.placement_events(a).unwrap().count(), 0);
    host.native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    let a_events: Vec<_> = host.placement_events(a).unwrap().collect();
    assert_eq!(a_events.len(), 1);
    assert_eq!(a_events[0].kind, PlacementEventKind::PauseUnit(false));
    assert_eq!(a_events[0].game_time, host.timer_game_time().unwrap());
    assert_eq!(host.placement_events(b).unwrap().count(), 0);
    host.native("PauseUnit", &[Value::Handle(b), Value::Bool(true)])
        .unwrap();
    host.native("PauseUnit", &[Value::Handle(b), Value::Bool(false)])
        .unwrap();
    assert_eq!(
        host.placement_events(b)
            .unwrap()
            .map(|event| event.kind)
            .collect::<Vec<_>>(),
        [
            PlacementEventKind::PauseUnit(true),
            PlacementEventKind::PauseUnit(false)
        ]
    );
    assert!(host.executed_unit(b).unwrap().paused_since.is_none());
}

#[test]
fn exhausted_placement_history_refuses_without_blocking_pause() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let host = &mut vm.host().inner;
    for _ in 0..12 {
        host.native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
            .unwrap();
    }
    assert_eq!(host.placement_events(a).unwrap().count(), 12);
    for _ in 12..1024 {
        host.native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
            .unwrap();
    }
    assert!(host.placement_events(a).is_err());
    assert_eq!(
        host.native("IsUnitPaused", &[Value::Handle(a)]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn pause_preserves_the_route_and_repeated_pause_does_not_stack_or_catch_up() {
    let mut vm = open();
    let a = unit(&vm, "A");
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 1.);
    let before = position(&mut vm, a);
    let route = vm.host().inner.executed_unit(a).unwrap().activity.clone();
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    let mut spans = Vec::new();
    vm.host().inner.next_timer_expiry(4., &mut 1000).unwrap();
    vm.host()
        .inner
        .advance_unit_activity_spans(4., &mut 65536, &mut spans)
        .unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(
        (spans[0].from, spans[0].to, spans[0].start, spans[0].end),
        (before, before, 1., 4.)
    );
    assert!(!spans[0].moving);
    assert_eq!(position(&mut vm, a), before);
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().activity, route);
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitPaused", &[Value::Handle(a)])
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        Value::Int(1631)
    );
    assert_eq!(vm.call("stop", &[]).unwrap(), Value::Bool(false));
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(false));
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    tick(&mut vm, 7.);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    tick(&mut vm, 8.);
    let mut control = open();
    control.call("move", &[]).unwrap();
    tick(&mut control, 2.);
    let control_a = unit(&control, "A");
    assert_eq!(position(&mut vm, a), position(&mut control, control_a));
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitPaused", &[Value::Handle(a)])
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn paused_intervals_do_not_consume_queued_animation() {
    let mut vm = open();
    let a = unit(&vm, "A");
    let start = vm.host().inner.timer_game_time().unwrap();
    let host = &mut vm.host().inner;
    host.native(
        "SetUnitAnimation",
        &[Value::Handle(a), Value::Str("Death".into())],
    )
    .unwrap();
    host.native(
        "QueueUnitAnimation",
        &[Value::Handle(a), Value::Str("Walk".into())],
    )
    .unwrap();
    tick(&mut vm, start + 0.2);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    tick(&mut vm, start + 5.);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(false)])
        .unwrap();
    let animation = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(
        (animation.sequence, animation.started),
        (Some(2), start + 4.8)
    );
    assert_eq!(animation.sample(start + 5.).unwrap().unwrap().frame, 2200);
    tick(&mut vm, start + 5.2);
    let animation = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(
        (animation.sequence, animation.started),
        (Some(1), start + 5.2)
    );
    tick(&mut vm, start + 6.);
    let animation = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(
        (animation.sequence, animation.started),
        (Some(0), start + 5.8)
    );
}

#[test]
fn pause_validates_arguments_and_projection_before_changing_state() {
    let mut vm = open();
    let a = unit(&vm, "A");
    assert!(
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Handle(a), Value::Int(1)])
            .is_err()
    );
    assert_eq!(
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Null, Value::Bool(true)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitPaused", &[Value::Null])
            .unwrap(),
        Value::Bool(false)
    );
    assert!(
        vm.host()
            .inner
            .next_timer_expiry(2., &mut 1000)
            .unwrap()
            .is_none()
    );
    assert!(
        vm.host()
            .inner
            .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
            .is_err()
    );
    assert_eq!(vm.host().inner.executed_unit(a).unwrap().paused_since, None);
    tick(&mut vm, 2.);
    vm.host()
        .inner
        .native("PauseUnit", &[Value::Handle(a), Value::Bool(true)])
        .unwrap();
    vm.host()
        .inner
        .native("RemoveUnit", &[Value::Handle(a)])
        .unwrap();
    assert_eq!(
        vm.host()
            .inner
            .native("IsUnitPaused", &[Value::Handle(a)])
            .unwrap(),
        Value::Bool(false)
    );
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
    host_fields(65536, &profile_fields(value, movement, None))
}
fn profile_with_structure(value: FieldValue, movement: Option<&str>, building: bool) -> ObjectHost {
    host_fields_with(
        65536,
        &profile_fields(value, movement, Some(building)),
        move_speed_rules(),
    )
}
pub(super) fn profile_fields(
    value: FieldValue,
    movement: Option<&str>,
    building: Option<bool>,
) -> Vec<([u8; 4], ObjectField)> {
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
    if let Some(building) = building {
        let mut field = geometry_field(Ok(FieldValue::Cell(
            if building { "1" } else { "0" }.into(),
        )));
        let meta = field.metadata.as_mut().unwrap();
        meta.fourcc = *b"B001";
        meta.field = "isbldg".into();
        meta.kind = FieldKind::Bool;
        meta.declared = "boolean".into();
        meta.source = FieldSource::Column {
            slk: SlkId::UnitBalance,
            column: "isbldg".into(),
        };
        fields.push((*b"B001", field));
    }
    fields
}
#[test]
fn move_speed_natives_clamp_by_structure_and_reanchor_the_route() {
    let mut h = profile_with_structure(FieldValue::Cell("100".into()), Some("foot"), false);
    let a = position_create(&mut h, 16., 16.).unwrap();
    assert_eq!(
        h.native("GetUnitDefaultMoveSpeed", std::slice::from_ref(&a))
            .unwrap(),
        Value::Real(100.)
    );
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
    h.next_timer_expiry(0.5, &mut 1000).unwrap();
    h.advance_unit_activity(0.5, &mut 1000).unwrap();
    assert_eq!(
        h.native("GetUnitX", std::slice::from_ref(&a)).unwrap(),
        Value::Real(66.)
    );
    h.native("SetUnitMoveSpeed", &[a.clone(), Value::Real(400.)])
        .unwrap();
    h.next_timer_expiry(0.515, &mut 1000).unwrap();
    h.advance_unit_activity(0.515, &mut 1000).unwrap();
    assert_eq!(
        h.native("GetUnitX", std::slice::from_ref(&a)).unwrap(),
        Value::Real(72.)
    );
    for (speed, expected) in [(0., 10.), (-50., 10.), (1000., 400.)] {
        h.native("SetUnitMoveSpeed", &[a.clone(), Value::Real(speed)])
            .unwrap();
        assert_eq!(
            h.native("GetUnitMoveSpeed", std::slice::from_ref(&a))
                .unwrap(),
            Value::Real(expected)
        );
    }

    let mut h = profile_with_structure(FieldValue::Cell("100".into()), Some("foot"), true);
    let b = position_create(&mut h, 16., 16.).unwrap();
    assert_eq!(
        h.native("GetUnitDefaultMoveSpeed", std::slice::from_ref(&b))
            .unwrap(),
        Value::Real(100.)
    );
    h.native("SetUnitMoveSpeed", &[b.clone(), Value::Real(1000.)])
        .unwrap();
    assert_eq!(
        h.native("GetUnitMoveSpeed", std::slice::from_ref(&b))
            .unwrap(),
        Value::Real(300.)
    );
    h.native("SetUnitMoveSpeed", &[b.clone(), Value::Real(0.)])
        .unwrap();
    assert_eq!(
        h.native("GetUnitMoveSpeed", &[b]).unwrap(),
        Value::Real(25.)
    );

    let mut h = profile_with_structure(FieldValue::Cell("500".into()), Some("foot"), false);
    let c = position_create(&mut h, 16., 16.).unwrap();
    assert_eq!(
        h.native("GetUnitMoveSpeed", std::slice::from_ref(&c))
            .unwrap(),
        Value::Real(400.)
    );
    assert_eq!(
        h.native("GetUnitDefaultMoveSpeed", &[c]).unwrap(),
        Value::Real(500.)
    );
    assert_eq!(
        h.native("SetUnitMoveSpeed", &[Value::Null, Value::Real(1.)])
            .unwrap(),
        Value::Null
    );
}

#[test]
fn non_stock_move_speed_limits_clamp_finite_negative_inputs_without_a_copied_floor() {
    let profile =
        "[Misc]\nMinUnitSpeed=37.5\nMaxUnitSpeed=412.5\nMinBldgSpeed=41.25\nMaxBldgSpeed=311.5\n";
    let fields = profile_fields(FieldValue::Cell("100".into()), Some("foot"), Some(false));
    let mut h = host_fields_with(65536, &fields, move_speed_rules_with(profile));
    let a = position_create(&mut h, 16., 16.).unwrap();
    assert_eq!(
        h.native("GetUnitDefaultMoveSpeed", std::slice::from_ref(&a))
            .unwrap(),
        Value::Real(100.)
    );
    for (input, expected) in [(-50., 37.5), (0., 37.5), (1000., 412.5)] {
        h.native("SetUnitMoveSpeed", &[a.clone(), Value::Real(input)])
            .unwrap();
        assert_eq!(
            h.native("GetUnitMoveSpeed", std::slice::from_ref(&a))
                .unwrap(),
            Value::Real(expected)
        );
        assert_eq!(
            h.native("GetUnitDefaultMoveSpeed", std::slice::from_ref(&a))
                .unwrap(),
            Value::Real(100.)
        );
    }

    let fields = profile_fields(FieldValue::Cell("100".into()), Some("foot"), Some(true));
    let mut h = host_fields_with(65536, &fields, move_speed_rules_with(profile));
    let b = position_create(&mut h, 16., 16.).unwrap();
    for (input, expected) in [(-50., 41.25), (1000., 311.5)] {
        h.native("SetUnitMoveSpeed", &[b.clone(), Value::Real(input)])
            .unwrap();
        assert_eq!(
            h.native("GetUnitMoveSpeed", std::slice::from_ref(&b))
                .unwrap(),
            Value::Real(expected)
        );
    }
}

#[test]
fn zero_minimum_speed_retains_the_route_with_finite_spans_and_resumes() {
    const ZERO_MIN: &str =
        "[Misc]\nMinUnitSpeed=0\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300\n";
    let mut vm = open_move_speed_with(ZERO_MIN, "10");
    let a = unit(&vm, "A");
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 1.);
    let stopped = position(&mut vm, a);
    let order = vm
        .host()
        .inner
        .native("GetUnitCurrentOrder", &[Value::Handle(a)])
        .unwrap();
    let destination = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .destination();
    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(a), Value::Real(0.)])
        .unwrap();
    assert_eq!(move_speed(&mut vm, a), 0.);

    let mut spans = Vec::new();
    tick_spans(&mut vm, 2., &mut spans);
    assert_eq!(position(&mut vm, a), stopped);
    assert!(
        spans
            .iter()
            .all(|s| s.start.is_finite() && s.end.is_finite() && s.from == s.to),
        "{spans:?}"
    );
    let state = vm.host().inner.executed_unit(a).unwrap().activity.clone();
    assert!(
        state
            .as_ref()
            .is_some_and(|s| s.moving() && s.destination() == destination)
    );
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        order
    );

    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(a), Value::Real(10.)])
        .unwrap();
    assert_eq!(move_speed(&mut vm, a), 10.);
    let appended = spans.len();
    tick_spans(&mut vm, 3., &mut spans);
    assert_ne!(position(&mut vm, a), stopped);
    assert!(
        spans[appended..]
            .iter()
            .all(|s| s.start.is_finite() && s.end.is_finite()),
        "{:?}",
        &spans[appended..]
    );
    assert!(spans[appended..].iter().any(|s| s.moving));
}

#[test]
fn move_speed_native_validates_arguments_before_null_and_removed_noops() {
    let mut h = profile_with_structure(FieldValue::Cell("100".into()), Some("foot"), false);
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
    h.next_timer_expiry(0.5, &mut 1000).unwrap();
    h.advance_unit_activity(0.5, &mut 1000).unwrap();
    let before = h.native("GetUnitX", std::slice::from_ref(&a)).unwrap();
    let order = h
        .native("GetUnitCurrentOrder", std::slice::from_ref(&a))
        .unwrap();
    let owner = h.native("Player", &[Value::Int(4)]).unwrap();
    let handles = h.inner().handles();

    for args in [
        vec![a.clone()],
        vec![a.clone(), Value::Str("fast".into())],
        vec![a.clone(), Value::Real(f32::NAN)],
        vec![a.clone(), Value::Real(f32::INFINITY)],
        vec![a.clone(), Value::Real(50.), Value::Real(1.)],
        vec![Value::Null],
        vec![Value::Null, Value::Str("fast".into())],
        vec![Value::Null, Value::Real(f32::NAN)],
        vec![Value::Null, Value::Real(1.), Value::Real(2.)],
    ] {
        assert!(
            h.native("SetUnitMoveSpeed", &args).is_err(),
            "malformed {args:?}"
        );
    }
    for args in [
        vec![],
        vec![a.clone(), Value::Real(1.)],
        vec![Value::Str("unit".into())],
    ] {
        assert!(
            h.native("GetUnitMoveSpeed", &args).is_err(),
            "malformed {args:?}"
        );
        assert!(
            h.native("GetUnitDefaultMoveSpeed", &args).is_err(),
            "malformed {args:?}"
        );
    }
    assert!(
        h.native("GetUnitMoveSpeed", std::slice::from_ref(&owner))
            .is_err()
    );
    assert!(
        h.native("GetUnitDefaultMoveSpeed", std::slice::from_ref(&owner))
            .is_err()
    );
    assert!(
        h.native("SetUnitMoveSpeed", &[owner, Value::Real(1.)])
            .is_err()
    );
    assert_eq!(
        h.native("GetUnitMoveSpeed", std::slice::from_ref(&a))
            .unwrap(),
        Value::Real(100.)
    );
    assert_eq!(
        h.native("GetUnitX", std::slice::from_ref(&a)).unwrap(),
        before
    );
    assert_eq!(
        h.native("GetUnitCurrentOrder", std::slice::from_ref(&a))
            .unwrap(),
        order
    );
    assert_eq!(
        h.native("SetUnitMoveSpeed", &[Value::Null, Value::Real(300.)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(h.inner().handles(), handles);

    h.native("RemoveUnit", std::slice::from_ref(&a)).unwrap();
    assert_eq!(
        h.native("GetUnitMoveSpeed", std::slice::from_ref(&a))
            .unwrap(),
        Value::Real(0.)
    );
    assert_eq!(
        h.native("GetUnitDefaultMoveSpeed", std::slice::from_ref(&a))
            .unwrap(),
        Value::Real(0.)
    );
    assert_eq!(
        h.native("SetUnitMoveSpeed", &[a.clone(), Value::Real(400.)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(h.inner().handles(), handles);
    assert!(
        h.native("GetUnitMoveSpeed", &[a.clone(), Value::Real(1.)])
            .is_err()
    );
    assert!(
        h.native("SetUnitMoveSpeed", &[a.clone(), Value::Real(f32::NAN)])
            .is_err()
    );
    assert!(
        h.native("SetUnitMoveSpeed", &[a.clone(), Value::Str("fast".into())])
            .is_err()
    );
    assert_eq!(h.inner().handles(), handles);
}

#[test]
fn move_speed_setter_reanchors_a_moving_walk_rate_without_disturbing_an_idle_animation() {
    let mut vm = open_move_speed();
    let a = unit(&vm, "A");
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 1.);
    let set_at = 1.;
    let before_position = position(&mut vm, a);
    let before_order = vm
        .host()
        .inner
        .native("GetUnitCurrentOrder", &[Value::Handle(a)])
        .unwrap();
    let before_sample = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .animation
        .as_ref()
        .unwrap()
        .sample(1.1)
        .unwrap()
        .unwrap();
    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(a), Value::Real(400.)])
        .unwrap();
    let record = vm.host().inner.executed_unit(a).unwrap();
    let animation = record.animation.as_ref().unwrap();
    let index = animation.sequence.unwrap();
    let sequence = &animation.model.sequences[index];
    let after_sample = animation.sample(1.1).unwrap().unwrap();
    let expected = if sequence.move_speed > 0. {
        let local = ((1.1 - set_at) * f64::from(400. / sequence.move_speed) * 1000.).floor() as u64;
        let duration = u64::from(sequence.duration());
        if duration == 0 {
            0
        } else if sequence.non_looping {
            local.min(duration)
        } else {
            local % duration
        }
    } else {
        100
    };
    assert_eq!(animation.started, set_at);
    assert_eq!(
        u64::from(after_sample.frame - after_sample.interval[0]),
        expected
    );
    assert_ne!(after_sample, before_sample);
    assert_eq!(record.position.unwrap(), before_position);
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        before_order
    );
    tick(&mut vm, 1.05);
    assert_eq!(position(&mut vm, a), [46., 16.]);

    let b = unit(&vm, "B");
    let idle = vm.host().inner.executed_unit(b).unwrap().animation.clone();
    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(b), Value::Real(350.)])
        .unwrap();
    assert_eq!(vm.host().inner.executed_unit(b).unwrap().animation, idle);
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitMoveSpeed", &[Value::Handle(b)])
            .unwrap(),
        Value::Real(350.)
    );
}

#[test]
fn move_speed_reselect_interrupts_queued_animation_and_keeps_the_unit_scheduled() {
    let mut vm = open_move_speed();
    let a = unit(&vm, "A");
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    tick(&mut vm, 1.);
    vm.host()
        .inner
        .native(
            "QueueUnitAnimation",
            &[Value::Handle(a), Value::Str("Death".into())],
        )
        .unwrap();
    assert!(
        vm.host()
            .inner
            .executed_unit(a)
            .unwrap()
            .animation
            .as_ref()
            .unwrap()
            .has_pending()
    );
    let before = position(&mut vm, a);
    let order = vm
        .host()
        .inner
        .native("GetUnitCurrentOrder", &[Value::Handle(a)])
        .unwrap();

    vm.host()
        .inner
        .native("SetUnitMoveSpeed", &[Value::Handle(a), Value::Real(400.)])
        .unwrap();
    let record = vm.host().inner.executed_unit(a).unwrap();
    let animation = record.animation.as_ref().unwrap();
    assert_eq!(animation.started, 1.);
    assert_eq!(
        animation.model.sequences[animation.sequence.unwrap()]
            .name
            .to_ascii_lowercase(),
        "walk"
    );
    assert!(!animation.has_pending());
    assert_eq!(record.position.unwrap(), before);
    assert_eq!(
        vm.host()
            .inner
            .native("GetUnitCurrentOrder", &[Value::Handle(a)])
            .unwrap(),
        order
    );

    tick(&mut vm, 1.25);
    assert_ne!(position(&mut vm, a), before);
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
fn hover_units_use_ground_collision_routes_and_authored_speed() {
    let mut vm = open_with_fields(&profile_fields(
        FieldValue::Cell("11".into()),
        Some("hover"),
        None,
    ));
    let a = unit(&vm, "A");
    let blocker = position_create(&mut vm.host().inner, 40., 16.).unwrap();
    let Value::Handle(blocker) = blocker else {
        panic!("unit");
    };
    let blocked = position(&mut vm, blocker);
    assert_eq!(vm.call("move", &[]).unwrap(), Value::Bool(true));
    let mut detour = false;
    for tick_number in 1..=12 {
        tick(&mut vm, f64::from(tick_number));
        let at = position(&mut vm, a);
        detour |= at[1] != 16.;
        assert!(!footprints_overlap(cell_of(at), cell_of(blocked)));
    }
    assert!(
        detour,
        "hover must route around ground occupancy rather than fly through it"
    );
    assert_eq!(position(&mut vm, a), [72., 16.]);
    assert_eq!(move_speed(&mut vm, a), 11.);
}

#[test]
fn fly_movement_class_admits_scripted_motion() {
    let mut h = profile(FieldValue::Cell("12".into()), Some("fly"));
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
    assert_eq!(h.native("GetUnitX", &[a]).unwrap(), Value::Real(28.));
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

#[test]
fn setrally_point_forms_store_the_point_and_leave_the_unit_still() {
    let mut vm = open_admitting(&[], rules_with_setrally());
    let unit = unit(&vm, "A");
    let h = &mut vm.host().inner;
    let start = h.executed_unit(unit).unwrap().position.unwrap();
    let location = h
        .native("Location", &[Value::Real(72.0), Value::Real(32.0)])
        .unwrap();
    let id = h
        .native("OrderId", &[Value::Str("setrally".into())])
        .unwrap();
    for (name, tail) in [
        (
            "IssuePointOrderLoc",
            vec![Value::Str("setrally".into()), location.clone()],
        ),
        ("IssuePointOrderByIdLoc", vec![id.clone(), location.clone()]),
        (
            "IssuePointOrder",
            vec![
                Value::Str("setrally".into()),
                Value::Real(72.0),
                Value::Real(32.0),
            ],
        ),
        (
            "IssuePointOrderById",
            vec![id, Value::Real(80.0), Value::Real(24.0)],
        ),
    ] {
        let args = [vec![Value::Handle(unit)], tail].concat();
        assert_eq!(h.native(name, &args).unwrap(), Value::Bool(true), "{name}");
    }
    assert_eq!(
        h.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .rally,
        Some([80.0, 24.0])
    );
    assert_eq!(h.executed_unit(unit).unwrap().position.unwrap(), start);
    assert!(
        !h.executed_unit(unit)
            .unwrap()
            .activity
            .as_ref()
            .unwrap()
            .moving()
    );
    assert_eq!(
        h.native("GetUnitCurrentOrder", &[Value::Handle(unit)])
            .unwrap(),
        Value::Int(0)
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

#[test]
fn successful_replan_defers_its_zero_distance_projection_until_the_next_tick() {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    tick(&mut vm, 2.0);
    position_create(&mut vm.host().inner, 48.0, 16.0).unwrap();
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(10.0, &mut 1000).unwrap().is_none());
    // The blocked segment and successful route use 136 work. The old loop then
    // spent two more on a zero-time loop/check; those cannot advance the unit.
    let mut work = 136;
    h.advance_unit_activity(10.0, &mut work).unwrap();
    assert_eq!(work, 0);
    assert_eq!(position(&mut vm, a), [42.0, 16.0]);
    let state = vm
        .host()
        .inner
        .executed_unit(a)
        .unwrap()
        .activity
        .as_ref()
        .unwrap()
        .clone();
    assert!(state.moving());
    assert_eq!(state.destination(), Some([72.0, 16.0]));
    tick(&mut vm, 12.0);
    assert_ne!(position(&mut vm, a), [42.0, 16.0]);
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
    // The mandatory segment probe needs 20 samples after its unit and waypoint charges.
    // A budget of 21 cannot pay that work, independently of any optional replan.
    let h = &mut vm.host().inner;
    assert!(h.next_timer_expiry(10., &mut 1000).unwrap().is_none());
    assert!(
        h.advance_unit_activity_spans(10., &mut 21, &mut spans)
            .is_err(),
        "mandatory movement refuses an undersized budget"
    );
    assert_eq!(spans, before, "a failed projection leaves no spans behind");
}

#[test]
fn float_and_amphibious_orders_use_the_shared_live_activity_clock() {
    for movement in ["float", "amph"] {
        let mut h = profile(FieldValue::Cell("12".into()), Some(movement));
        let a = position_create(&mut h, 16., 16.).unwrap();
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
            Value::Bool(true)
        );
        assert!(h.next_timer_expiry(1., &mut 1000).unwrap().is_none());
        h.advance_unit_activity(1., &mut 65536).unwrap();
        assert_eq!(
            h.native("GetUnitX", std::slice::from_ref(&a)).unwrap(),
            Value::Real(28.)
        );
        assert!(h.next_timer_expiry(6., &mut 1000).unwrap().is_none());
        h.advance_unit_activity(6., &mut 65536).unwrap();
        assert_eq!(h.native("GetUnitX", &[a]).unwrap(), Value::Real(72.));
    }
}

#[test]
fn deferred_replan_wait_is_not_credited_when_the_obstacle_clears() {
    let mut vm = open();
    let a = unit(&vm, "A");
    vm.call("move", &[]).unwrap();
    tick(&mut vm, 2.0);
    let Value::Handle(blocker) = position_create(&mut vm.host().inner, 48.0, 16.0).unwrap() else {
        panic!("unit");
    };
    let host = &mut vm.host().inner;
    host.next_timer_expiry(10.0, &mut 1000).unwrap();
    host.advance_unit_activity(10.0, &mut 22).unwrap();
    assert_eq!(position(&mut vm, a), [42.0, 16.0]);
    vm.host()
        .inner
        .native("ShowUnit", &[Value::Handle(blocker), Value::Bool(false)])
        .unwrap();
    let speed = move_speed(&mut vm, a);
    tick(&mut vm, 12.0);
    let after = position(&mut vm, a);
    assert!(after[0] > 42.0);
    assert!(
        after[0] <= 42.0 + speed * 2.0,
        "blocked waiting time is not catch-up distance"
    );
}

#[test]
fn removed_image_cleanup_chain_crosses_real_sleep_without_replacing_unit_or_moving_survivor() {
    let declarations = r#"
native SetUnitInvulnerable takes unit u, boolean flag returns nothing
native SetUnitPositionLoc takes unit u, location at returns nothing
native SetUnitFacingTimed takes unit u, real angle, real duration returns nothing
constant native Atan2 takes real y, real x returns real
native TriggerSleepAction takes real timeout returns nothing
"#;
    // Self-authored equivalent call chain; no original script bytes are embedded.
    let map = r#"
globals
unit A=null
unit B=null
location DEST=null
trigger T=null
boolean DONE=false
boolean ORDERED=true
endglobals
function face_retained takes unit image, unit target returns nothing
local location a=GetUnitLoc(image)
local location b=GetUnitLoc(target)
call SetUnitFacingTimed(image,Atan2(GetLocationY(b)-GetLocationY(a),GetLocationX(b)-GetLocationX(a)),0.0)
call RemoveLocation(b)
call RemoveLocation(a)
endfunction
function cleanup takes nothing returns nothing
call SetUnitInvulnerable(A,true)
call SetUnitPositionLoc(A,DEST)
call face_retained(A,B)
call TriggerSleepAction(0.5)
set ORDERED=IssuePointOrderLoc(A,"gamma",DEST)
set DONE=true
endfunction
function setup takes nothing returns nothing
set A=CreateUnit(Player(4),'U001',16.0,16.0,0.0)
set B=CreateUnit(Player(4),'U001',80.0,16.0,0.0)
set DEST=Location(48.0,16.0)
call SetUnitInvulnerable(B,true)
call PauseUnit(B,true)
call RemoveUnit(A)
set T=CreateTrigger()
call TriggerAddAction(T,function cleanup)
call TriggerExecute(T)
endfunction
"#;
    let mut h = host_fields_with_placed_source(65536, &[], &[], rules(), map, declarations);
    let numeric = crate::campaign_flee_policy::tests::route_fixture().1;
    let wait = f64::from(pcx_script::RealOps::literal(&numeric, "0.5").unwrap());
    h = h.with_script_numeric(numeric);
    h.admit_thread_sleep(2).unwrap();
    let ctx = Arc::clone(h.player_context().unwrap());
    let mut vm = ctx.scripts.clone().owned_vm(h);
    vm.init_globals().unwrap();
    vm.call("setup", &[]).unwrap();
    let a = unit(&vm, "A");
    let b = unit(&vm, "B");
    let before = {
        let u = vm.host().inner.executed_unit(b).unwrap();
        (u.position, u.face, u.owner, u.invulnerable, u.paused_since)
    };
    let handles = vm.host().inner.inner().handles();
    assert_eq!(vm.global("DONE"), Value::Bool(false));
    assert_eq!(vm.host().inner.parked_thread_count(), 1);
    assert!(vm.host().inner.executed_unit(a).is_err());
    assert!(
        vm.host()
            .inner
            .next_thread_wake(wait / 2.)
            .unwrap()
            .is_none()
    );
    let ticket = vm.host().inner.next_thread_wake(wait).unwrap().unwrap();
    vm.resume_thread(ticket).unwrap();
    assert_eq!(vm.global("DONE"), Value::Bool(true));
    assert_eq!(vm.global("ORDERED"), Value::Bool(false));
    assert_eq!(vm.host().inner.parked_thread_count(), 0);
    assert_eq!(vm.host().inner.inner().handles(), handles);
    let u = vm.host().inner.executed_unit(b).unwrap();
    assert_eq!(
        (u.position, u.face, u.owner, u.invulnerable, u.paused_since),
        before
    );
    assert_eq!(
        vm.host()
            .inner
            .executed_units()
            .map(|(id, _)| id)
            .collect::<Vec<_>>(),
        [b]
    );
    // The removed alias neither reserves its requested destination nor resurrects
    // its old footprint. A real later factory request can use that freed point.
    let Value::Handle(c) = position_create(&mut vm.host().inner, 16., 16.).unwrap() else {
        panic!("unit")
    };
    assert_eq!(position(&mut vm, c), [16., 16.]);
    let location = vm.global("DEST");
    let timer = vm.host().inner.native("CreateTimer", &[]).unwrap();
    let unknown = HandleId::new(std::num::NonZeroU64::new(u64::MAX).unwrap());
    for bad in [timer.clone(), Value::Handle(unknown)] {
        for (name, rest) in [
            ("SetUnitPositionLoc", vec![location.clone()]),
            ("SetUnitFacingTimed", vec![Value::Real(0.), Value::Real(0.)]),
            (
                "IssuePointOrderLoc",
                vec![Value::Str("gamma".into()), location.clone()],
            ),
        ] {
            let mut args = vec![bad.clone()];
            args.extend(rest);
            assert!(vm.host().inner.native(name, &args).is_err());
        }
    }
    assert!(
        vm.host()
            .inner
            .native("SetUnitPositionLoc", &[Value::Handle(a), timer])
            .is_err()
    );
    assert!(
        vm.host()
            .inner
            .native(
                "SetUnitPosition",
                &[Value::Handle(a), Value::Real(f32::NAN), Value::Real(0.)]
            )
            .is_err()
    );
    assert_eq!(
        vm.host()
            .inner
            .native(
                "SetUnitPosition",
                &[Value::Handle(a), Value::Real(48.), Value::Real(16.)]
            )
            .unwrap(),
        Value::Null
    );
}

#[test]
#[ignore = "requires PCX_REMOVED_SETTER_CLIENT and PCX_REMOVED_SETTER_BLIZZARD local original inputs"]
fn selected_removed_image_cleanup_wrapper_and_native_branches() {
    let bytes = std::fs::read(std::env::var("PCX_REMOVED_SETTER_CLIENT").unwrap()).unwrap();
    assert_eq!(
        crate::neutral_player::sha256(&bytes),
        crate::menu_lighting::REVIEWED_IMAGE
    );
    let pe = crate::pe_image::PeSections::parse(&bytes).unwrap();
    let word = |b: &[u8], i| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
    for (registration, name, call_at, expected_prefix, branch_at, expected_branch) in [
        (
            0x49d446,
            "SetUnitPositionLoc",
            0x21,
            &[0x8b, 0xd8, 0x83, 0xc4, 4, 0x85, 0xdb][..],
            0x2d,
            &[0x74, 0x31][..],
        ),
        (
            0x49d432,
            "SetUnitPosition",
            0x0a,
            &[0x8b, 0xf0, 0x83, 0xc4, 4, 0x85, 0xf6][..],
            0x16,
            &[0x74, 0x3b][..],
        ),
        (
            0x49d471,
            "SetUnitFacingTimed",
            0x1a,
            &[0x83, 0xc4, 4, 0x85, 0xc0][..],
            0x24,
            &[0x0f, 0x84, 0x26, 1, 0, 0][..],
        ),
        (
            0x49e05d,
            "IssuePointOrderLoc",
            0x7f,
            &[0x8b, 0xf0, 0x83, 0xc4, 4, 0x85, 0xf6][..],
            0x8b,
            &[0x74, 0xda][..],
        ),
    ] {
        let reg = pe.va_bytes(registration, 20).unwrap();
        assert_eq!(reg[10], 0x68);
        assert_eq!(
            pe.va_bytes(word(reg, 6), name.len() + 1).unwrap(),
            format!("{name}\0").as_bytes()
        );
        let handler = word(reg, 11);
        let body = pe
            .va_bytes(handler, branch_at + expected_branch.len())
            .unwrap();
        assert_eq!(body[call_at], 0xe8);
        assert_eq!(
            handler
                .wrapping_add(call_at as u32 + 5)
                .wrapping_add(word(body, call_at + 1)),
            0x483940
        );
        assert_eq!(
            &body[call_at + 5..call_at + 5 + expected_prefix.len()],
            expected_prefix
        );
        assert_eq!(
            &body[branch_at..branch_at + expected_branch.len()],
            expected_branch
        );
    }
    let bj =
        std::fs::read_to_string(std::env::var("PCX_REMOVED_SETTER_BLIZZARD").unwrap()).unwrap();
    let body = |name: &str| {
        bj.split(&format!("function {name} takes"))
            .nth(1)
            .unwrap()
            .split("endfunction")
            .next()
            .unwrap()
    };
    for (name, calls) in [
        (
            "SetUnitFacingToFaceUnitTimed",
            &[
                "GetUnitLoc(target)",
                "SetUnitFacingToFaceLocTimed(whichUnit, unitLoc, duration)",
                "RemoveLocation(unitLoc)",
            ][..],
        ),
        (
            "SetUnitFacingToFaceLocTimed",
            &[
                "GetUnitLoc(whichUnit)",
                "SetUnitFacingTimed(whichUnit, AngleBetweenPoints(unitLoc, target), duration)",
                "RemoveLocation(unitLoc)",
            ][..],
        ),
        ("IssuePointOrderLocBJ", &["return IssuePointOrderLoc("][..]),
    ] {
        for call in calls {
            assert!(body(name).contains(call), "{name} missing {call}");
        }
    }
    // The failed unit-resolution branch returns false explicitly, never acceptance.
    assert_eq!(
        pe.va_bytes(0x49af37, 9).unwrap(),
        &[0x5f, 0x5e, 0x33, 0xc0, 0x5b, 0x8b, 0xe5, 0x5d, 0xc3]
    );
}
