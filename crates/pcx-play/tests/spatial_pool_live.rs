//! Live spatial chain pool in the Human05 farm layout. Requires the selected local install
//! and map, which `PCX_STARTUP_INSTALL` and `PCX_STARTUP_MAP` name.

use pcx_assets::{Addon, AssetMode, Behaviour, Build, GameMode};
use pcx_play::headless_startup::{Startup, start};
use pcx_play::playercolor_host::ObjectHost;
use pcx_play::spatial_pool::{PoolOwner, live_pool};
use pcx_script::{HandleId, Host, Value};
use std::path::PathBuf;

/// Worker positions and the passive farm site of the recorded farm layout
/// (`docs/re-notes/ai-construction.md`, "Farm validator and exact order-time workers").
const FARM_WORKERS: [[f32; 2]; 4] = [
    [5611.785, 2795.562],
    [5676.686, 2656.12],
    [5779.904, 2595.492],
    [5468.591, 2861.923],
];
const FARM_SITE: [f32; 2] = [6208., 2752.];
const FARM_ORACLE_SITE: [f32; 2] = [5952., 2432.];
const MINE: [f32; 2] = [5696., 3456.];
const NEIGHBORHOOD: usize = 1;
/// Ticks per `step_clock` call; the call refuses more than its own bound.
const STEP_BATCH: u64 = 2048;

fn building_facing(angle: f32) -> f32 {
    assert!(angle.is_finite() && angle > i32::MIN as f32 && angle < i32::MAX as f32);
    let angle = (angle as i32) % 360;
    ((angle / 90 * 90).rem_euclid(360)) as f32
}

fn handle(value: Value) -> HandleId {
    let Value::Handle(id) = value else {
        panic!("expected a handle")
    };
    id
}

fn kind(objects: &ObjectHost, identity: u64) -> &'static str {
    if objects
        .executed_regions()
        .unwrap()
        .any(|(region, _)| region.token() == identity)
    {
        "region"
    } else if objects
        .executed_units()
        .any(|(unit, _)| unit.token() == identity)
    {
        "unit"
    } else {
        "texture or removed unit"
    }
}

fn dump(objects: &ObjectHost, pool: &PoolOwner, label: &str, point: [f32; 2]) {
    let center = pool.cell_at(point).unwrap();
    println!("{label} {point:?} native cell {center:?}");
    for dx in 0..=2 * NEIGHBORHOOD {
        for dy in 0..=2 * NEIGHBORHOOD {
            let (Some(x), Some(y)) = (
                (center[0] + dx).checked_sub(NEIGHBORHOOD),
                (center[1] + dy).checked_sub(NEIGHBORHOOD),
            ) else {
                continue;
            };
            let Ok(links) = pool.chain([x, y]) else {
                continue;
            };
            let links: Vec<_> = links
                .map(|link| {
                    let (flags, state) = pool.live().entry_state(link.identity).unwrap_or_default();
                    format!(
                        "{}#{} class {:#x} flags {flags:#x} state {state:#x}",
                        kind(objects, link.identity),
                        link.identity,
                        link.class
                    )
                })
                .collect();
            println!("  cell [{x}, {y}]: {links:?}");
        }
    }
}

/// The started Human05 session and the selected building facing.
fn open() -> (Startup, f32) {
    let install = PathBuf::from(std::env::var("PCX_STARTUP_INSTALL").unwrap());
    let map = PathBuf::from(std::env::var("PCX_STARTUP_MAP").unwrap());
    let bytes = std::fs::read(&map).unwrap();
    let mode = GameMode {
        build: Build::V1_29b,
        addon: Addon::NoAddon,
        assets: AssetMode::Vanilla,
        behaviour: Behaviour::Vanilla,
    };
    let image_path = install.join("Warcraft III.exe");
    let image = std::fs::read(&image_path).unwrap();
    let identity =
        pcx_play::campaign_inputs::client_identity(&image, &image_path.display().to_string());
    let image_rules = pcx_play::campaign_data_set::ImageRules::read(&image, identity).unwrap();
    let recipe = pcx_play::campaign_data_set::CampaignRecipe::pin(
        &install,
        mode,
        image_rules,
        &image,
        None,
        "VERSION_REIGN_OF_CHAOS",
    )
    .unwrap();
    let budget = pcx_mpq::checked::ReadBudget {
        max_archive_bytes: 64 << 20,
        max_archive_table_bytes: 8 << 20,
        max_raw_packed_bytes: 16 << 20,
        max_decoded_bytes: 16 << 20,
    };
    let label = map.display().to_string();
    let resolver = recipe.open_map(&bytes, &label, budget).unwrap().resolver;
    let misc = resolver
        .resolve_bounded("Units/MiscData.txt", 4 << 20)
        .unwrap();
    let map_misc = pcx_map::Map::parse(&bytes)
        .unwrap()
        .extract_checked(&label, "war3mapMisc.txt", budget)
        .unwrap();
    let mut profiles = vec![("Units/MiscData.txt", misc.bytes.as_slice())];
    if let Some(ref map_misc) = map_misc {
        profiles.push(("war3mapMisc.txt", map_misc.as_slice()));
    }
    let ([Some(angle)], _) =
        pcx_objects::editor::misc::misc_scalars(&profiles, ["BuildingAngle"], 8 << 20).unwrap()
    else {
        panic!("selected Misc/BuildingAngle is missing")
    };
    let facing = building_facing(angle);

    let startup = start(&install, mode, &label, &bytes, 1, None).unwrap();
    (startup, facing)
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL and PCX_STARTUP_MAP (Human05)"]
fn live_pool_dumps_the_farm_layout_chains() {
    let (mut startup, facing) = open();
    let objects = startup.session.host().unwrap().objects_mut();
    let pool = live_pool(objects).expect("Human05 starts an AI, so its session is tracked");
    assert!(pool.refusal().is_none(), "{:?}", pool.refusal());
    let startup_sequence = pool.live().sequence();
    println!("registrations at the end of map initialization: {startup_sequence}");

    let orange = objects.native("Player", &[Value::Int(5)]).unwrap();
    let passive = objects.native("Player", &[Value::Int(15)]).unwrap();
    let create = |objects: &mut ObjectHost, owner: &Value, rawcode: &[u8; 4], at: [f32; 2]| {
        handle(
            objects
                .native(
                    "CreateUnit",
                    &[
                        owner.clone(),
                        Value::Int(i32::from_be_bytes(*rawcode)),
                        Value::Real(at[0]),
                        Value::Real(at[1]),
                        Value::Real(facing),
                    ],
                )
                .unwrap(),
        )
    };
    let workers: Vec<_> = FARM_WORKERS
        .iter()
        .map(|&at| create(objects, &orange, b"uaco", at))
        .collect();
    let before_farm: Vec<_> = {
        let pool = live_pool(objects).unwrap();
        pool.chain(pool.cell_at([6160., 2800.]).unwrap())
            .unwrap()
            .map(|link| link.identity)
            .collect()
    };
    let _farm = create(objects, &passive, b"hhou", FARM_SITE);

    let pool = live_pool(objects).unwrap();
    assert!(pool.refusal().is_none(), "{:?}", pool.refusal());
    assert!(pool.live().sequence() > startup_sequence);
    let at_farm: Vec<_> = pool
        .chain(pool.cell_at(FARM_SITE).unwrap())
        .unwrap()
        .map(|link| link.identity)
        .collect();
    assert!(
        !at_farm.is_empty(),
        "the farm has texture entries at its site"
    );
    let captured_cell = pool.cell_at([6160., 2800.]).unwrap();
    assert!(
        pool.chain(captured_cell).unwrap().any(|link| {
            !before_farm.contains(&link.identity)
                && pool
                    .live()
                    .entry_state(link.identity)
                    .is_some_and(|(flags, _)| flags & 0x08 != 0)
        }),
        "the captured farm cell has a blue-channel entry"
    );
    for (worker, &at) in workers.iter().zip(&FARM_WORKERS) {
        assert!(
            pool.chain(pool.cell_at(at).unwrap())
                .unwrap()
                .any(|link| link.identity == worker.token()),
            "worker {worker:?} is linked at its own cell"
        );
    }
    for (label, point) in [
        ("farm site", FARM_SITE),
        ("recorded farm-layout selection", FARM_ORACLE_SITE),
        ("gold mine", MINE),
        ("first worker", FARM_WORKERS[0]),
    ] {
        dump(objects, pool, label, point);
    }
}

#[test]
#[ignore = "requires PCX_STARTUP_INSTALL and PCX_STARTUP_MAP (Human05)"]
fn live_pool_follows_the_ai_session_clock() {
    let (mut startup, _) = open();
    let ticks = std::env::var("PCX_POOL_TICKS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(2048);
    startup.session.sample_admitted_vision().unwrap();
    for batch in (0..ticks).step_by(STEP_BATCH as usize) {
        startup.step_clock(STEP_BATCH.min(ticks - batch)).unwrap();
    }
    let objects = startup.session.host().unwrap().objects_mut();
    let now = objects.timer_game_time().unwrap();
    let pool = live_pool(objects).unwrap();
    println!(
        "after {ticks} ticks at game time {now}: sequence {}, next cleanup {}, refusal {:?}",
        pool.live().sequence(),
        pool.live().next_cleanup(),
        pool.refusal()
    );
    if pool.refusal().is_none() {
        assert!(
            f64::from(pool.live().next_cleanup()) > now - 1.0,
            "cleanup deadlines were delivered up to the session clock"
        );
    }
}
