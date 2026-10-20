//! Headless production startup on a selected install and local map. Ignored by default: the
//! gate needs selected local files. It proves preparation, initialization, one accepted player
//! order and the clock step for two independent starts from identical inputs.
//!
//! PCX_STARTUP_INSTALL=<install dir> PCX_STARTUP_MAP=<map file> PCX_STARTUP_ADDON=roc|tft \
//!   PCX_STARTUP_SEED=<i32> cargo test -p pcx-play --test headless_startup -- --ignored
use std::path::PathBuf;

use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode};
use pcx_play::headless_startup::{Startup, start};
use pcx_script::{HandleId, Host, Value};

/// Campaign ticks stepped after the order; bounded and explicit.
const ORDER_TICKS: u64 = 256;
/// World units east of the unit; a short order completes before the first step.
const ORDER_DISTANCE: f32 = 64.0;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}; see the test module comment"))
}

fn selected_mode() -> (GameMode, bool) {
    let (addon, expansion) = match env("PCX_STARTUP_ADDON").as_str() {
        "roc" => (Addon::NoAddon, false),
        "tft" => (Addon::Tft, true),
        other => panic!("PCX_STARTUP_ADDON must be roc or tft, not {other}"),
    };
    let mode = GameMode {
        build: Build::V1_29b,
        addon,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    };
    (mode, expansion)
}

/// Executed units as (handle, owner number, current x bits, current y bits), in arena order.
/// The current position is the moved position when one exists, else the creation coordinates.
type UnitState = Vec<(HandleId, i32, u32, u32)>;

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

/// Issues the same move order the application's right click issues, on the first unit owned
/// by `slot`, and returns the unit's handle.
fn order_first_owned_unit(startup: &mut Startup, slot: i32) -> HandleId {
    let (unit, x, y) = {
        let host = startup.session.host().expect("admitted session");
        let (id, unit) = host
            .objects()
            .executed_units()
            .find(|(_, unit)| unit.executed_owner == slot)
            .unwrap_or_else(|| panic!("local slot {slot} owns no executed unit"));
        (id, unit.x, unit.y)
    };
    let target = [x + ORDER_DISTANCE, y];
    startup
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
        )
        .unwrap_or_else(|error| panic!("the local order was refused: {error}"));
    unit
}

fn position(startup: &mut Startup, unit: HandleId) -> [f32; 2] {
    let host = startup.session.host().expect("admitted session");
    {
        let record = host.objects().executed_unit(unit).expect("order target");
        current(record.x, record.y, record.position)
    }
}

/// Where a unit is now: its last moved position, or where it was created.
fn current(x: f32, y: f32, position: Option<[f32; 2]>) -> [f32; 2] {
    position.unwrap_or([x, y])
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL, PCX_STARTUP_MAP, PCX_STARTUP_ADDON and PCX_STARTUP_SEED"]
fn selected_map_repeats_startup_order_and_clock_from_the_same_inputs() {
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_STARTUP_MAP"));
    let seed: i32 = env("PCX_STARTUP_SEED")
        .parse()
        .expect("PCX_STARTUP_SEED is an i32");
    let (mode, expansion) = selected_mode();
    let bytes = std::fs::read(&map).expect("read map");
    let label = map.display().to_string();

    let mut first = start(&install, mode, &label, &bytes, seed, expansion)
        .unwrap_or_else(|error| panic!("first start: {error}"));
    let mut second = start(&install, mode, &label, &bytes, seed, expansion)
        .unwrap_or_else(|error| panic!("second start: {error}"));
    assert_eq!(first.context.sources.map, second.context.sources.map);
    assert_eq!(local_slot(&first), local_slot(&second), "roster");
    let slot = local_slot(&first);
    let startup_units = units(&mut first);
    assert_eq!(startup_units, units(&mut second), "startup diverged");
    assert!(!startup_units.is_empty(), "initialization created no units");

    let unit = order_first_owned_unit(&mut first, slot);
    let unit_second = order_first_owned_unit(&mut second, slot);
    assert_eq!(unit, unit_second);
    let before = position(&mut first, unit);
    first.step_clock(ORDER_TICKS).expect("first clock");
    second.step_clock(ORDER_TICKS).expect("second clock");
    assert_ne!(
        position(&mut first, unit),
        before,
        "the accepted order did not move the unit"
    );
    assert_eq!(
        units(&mut first),
        units(&mut second),
        "orders and clock diverged"
    );
    eprintln!(
        "receipts: map {:?}; common {:?}; blizzard {:?}; catalog {:?}",
        first.context.sources.map,
        first.context.sources.common,
        first.context.sources.blizzard,
        first.context.sources.catalog
    );
    eprintln!(
        "script-created units {}, local slot {slot}, ordered unit {:?} moved from {before:?}",
        startup_units.len(),
        unit
    );
}
