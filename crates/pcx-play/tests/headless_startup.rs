//! Headless production startup on a selected install and local map. Ignored by default: the gate
//! needs selected local files.
//!
//! What it checks, for independent starts from identical inputs: script-driven initialization
//! (`config` and `main` through the shared session), the map's human roster, one accepted player
//! move order, and the campaign clock. A control start takes the same steps without the player
//! order. Authored script orders can move units too, so the gate attributes the result to the
//! player order only through the difference from that control run.
//!
//! The state compared is executed unit identity, owner, and current position. It does not
//! establish VM globals, RNG streams, or script-level parity.
//!
//! PCX_STARTUP_INSTALL=<install dir> PCX_STARTUP_MAP=<map file> PCX_STARTUP_ADDON=roc|tft \
//!   PCX_STARTUP_SEED=<i32> cargo test -p pcx-play --test headless_startup -- --ignored
use std::path::PathBuf;

use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode};
use pcx_play::headless_startup::{Startup, start};
use pcx_script::{HandleId, Host, Value};

/// Campaign ticks stepped after the order. Explicit and bounded per call.
const ORDER_TICKS: u64 = 256;
/// World units east of the unit.
const ORDER_DISTANCE: f32 = 64.0;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}; see the test module comment"))
}

fn selected_mode() -> GameMode {
    let addon = match env("PCX_STARTUP_ADDON").as_str() {
        "roc" => Addon::NoAddon,
        "tft" => Addon::Tft,
        other => panic!("PCX_STARTUP_ADDON must be roc or tft, not {other}"),
    };
    GameMode {
        build: Build::V1_29b,
        addon,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    }
}

/// Executed units as (handle, owner number, current x bits, current y bits), in arena order.
type UnitState = Vec<(HandleId, i32, u32, u32)>;

/// Where a unit is now: its last moved position, or where it was created.
fn current(x: f32, y: f32, position: Option<[f32; 2]>) -> [f32; 2] {
    position.unwrap_or([x, y])
}

fn units(startup: &mut Startup) -> UnitState {
    startup
        .session
        .host()
        .expect("admitted session")
        .objects()
        .executed_units()
        .map(|(id, unit)| {
            let [x, y] = current(unit.x, unit.y, unit.position);
            (id, unit.executed_owner, x.to_bits(), y.to_bits())
        })
        .collect()
}

/// The local human slot, from the map's own player records.
fn local_slot(startup: &Startup) -> i32 {
    let mut humans = startup.context.world.info.human_players();
    let local = humans.next().expect("the map has a human slot");
    assert!(
        humans.next().is_none(),
        "fresh campaign needs one human slot"
    );
    i32::try_from(local.number()).expect("slot number")
}

/// The first executed unit owned by `slot`, with its current position before any command.
fn first_owned_unit(startup: &mut Startup, slot: i32) -> (HandleId, [f32; 2]) {
    let host = startup.session.host().expect("admitted session");
    let (id, unit) = host
        .objects()
        .executed_units()
        .find(|(_, unit)| unit.executed_owner == slot)
        .unwrap_or_else(|| panic!("local slot {slot} owns no executed unit"));
    (id, current(unit.x, unit.y, unit.position))
}

fn position(startup: &mut Startup, unit: HandleId) -> [f32; 2] {
    let host = startup.session.host().expect("admitted session");
    let record = host.objects().executed_unit(unit).expect("order target");
    current(record.x, record.y, record.position)
}

/// The destination of the unit's accepted route, or `None` when it has no route pending.
fn pending_destination(startup: &mut Startup, unit: HandleId) -> Option<[f32; 2]> {
    let host = startup.session.host().expect("admitted session");
    let record = host.objects().executed_unit(unit).expect("order target");
    record
        .activity
        .as_ref()
        .filter(|activity| activity.moving())
        .and_then(|activity| activity.destination())
}

/// Issues the move order the application's right click issues. Acceptance is the native's
/// `true` result, and every other result fails the gate.
fn order_move(startup: &mut Startup, unit: HandleId, target: [f32; 2]) {
    let result = startup
        .session
        .host()
        .expect("admitted session")
        .objects_mut()
        .native(
            "IssuePointOrder",
            &[
                Value::Handle(unit),
                Value::Str("move".into()),
                Value::Real(target[0]),
                Value::Real(target[1]),
            ],
        );
    match result {
        Ok(Value::Bool(true)) => {}
        other => panic!("the player move order was not accepted: {other:?}"),
    }
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP and PCX_STARTUP_ADDON selected local files"]
fn selected_map_starts_orders_and_steps_identically_and_the_order_changes_the_outcome() {
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_STARTUP_MAP"));
    let seed: i32 = env("PCX_STARTUP_SEED")
        .parse()
        .expect("PCX_STARTUP_SEED is an i32");
    let mode = selected_mode();
    let bytes = std::fs::read(&map).expect("read map");
    let label = map.display().to_string();

    let mut first = start(&install, mode, &label, &bytes, seed)
        .unwrap_or_else(|error| panic!("first start: {error}"));
    let mut second = start(&install, mode, &label, &bytes, seed)
        .unwrap_or_else(|error| panic!("second start: {error}"));
    let mut control = start(&install, mode, &label, &bytes, seed)
        .unwrap_or_else(|error| panic!("control start: {error}"));
    assert_eq!(first.context.sources.map, second.context.sources.map);
    let slot = local_slot(&first);
    assert_eq!(slot, local_slot(&second), "roster");
    assert_eq!(slot, local_slot(&control), "roster");
    let startup_units = units(&mut first);
    assert!(!startup_units.is_empty(), "initialization created no units");
    assert_eq!(startup_units, units(&mut second), "repeat startup diverged");
    assert_eq!(
        startup_units,
        units(&mut control),
        "control startup diverged"
    );

    let (unit, before) = first_owned_unit(&mut first, slot);
    let (unit_second, before_second) = first_owned_unit(&mut second, slot);
    let (unit_control, before_control) = first_owned_unit(&mut control, slot);
    assert_eq!((unit, before), (unit_second, before_second));
    assert_eq!((unit, before), (unit_control, before_control));
    let target = [before[0] + ORDER_DISTANCE, before[1]];

    order_move(&mut first, unit, target);
    order_move(&mut second, unit_second, target);
    assert_eq!(
        pending_destination(&mut first, unit),
        Some(target),
        "the accepted order did not record its destination"
    );

    first.step_clock(ORDER_TICKS).expect("first clock");
    second.step_clock(ORDER_TICKS).expect("second clock");
    control.step_clock(ORDER_TICKS).expect("control clock");

    let moved = position(&mut first, unit);
    let control_moved = position(&mut control, unit_control);
    assert_ne!(
        moved, control_moved,
        "the player order changed nothing relative to the run without it"
    );
    assert_eq!(
        units(&mut first),
        units(&mut second),
        "repeat orders and clock diverged"
    );
    eprintln!(
        "script-created units {}, local slot {slot}, unit {unit:?}: start {before:?}; with order {moved:?}; control {control_moved:?} after {ORDER_TICKS} ticks",
        startup_units.len()
    );
}
