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

#[test]
#[ignore = "requires selected Human01 through PCX_STARTUP_*; diagnostic XP stimulus, not player acceptance"]
fn selected_campaign_hero_xp_runs_the_authored_level_cap() {
    use pcx_assets::ContentSource;
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_STARTUP_MAP"));
    let bytes = std::fs::read(&map).unwrap();
    let source = pcx_play::install_source::InstallSource::open(&install).unwrap();
    let misc = source
        .read_bounded("Units\\MiscGame.txt", 8 << 20)
        .unwrap()
        .unwrap();
    let profiles = [(misc.locator.as_str(), misc.bytes.as_slice())];
    let progression = pcx_objects::editor::hero::Progression::read(&profiles, 8 << 20).unwrap();
    let mut startup = start(
        &install,
        selected_mode(),
        &map.display().to_string(),
        &bytes,
        env("PCX_STARTUP_SEED").parse().unwrap(),
        None,
    )
    .unwrap();
    let hero = startup.session.global("udg_Arthas").expect("authored hero");
    let objects = startup.session.host().unwrap().objects_mut();
    let Value::Int(initial) = objects
        .native("GetHeroLevel", std::slice::from_ref(&hero))
        .unwrap()
    else {
        panic!("hero level")
    };
    let thresholds = pcx_objects::editor::hero_experience::Experience::read(
        &profiles,
        initial,
        progression.max_level,
        8 << 20,
        65536,
    )
    .unwrap();
    let threshold = thresholds.minimum_for(initial, initial + 1).unwrap();
    assert_eq!(
        objects
            .native("GetHeroXP", std::slice::from_ref(&hero))
            .unwrap(),
        Value::Int(0)
    );
    objects
        .native(
            "SetHeroXP",
            &[hero.clone(), Value::Int(threshold), Value::Bool(false)],
        )
        .unwrap();
    startup.session.advance_queued_events(100_000).unwrap();
    let objects = startup.session.host().unwrap().objects_mut();
    assert_eq!(
        objects
            .native("GetHeroLevel", std::slice::from_ref(&hero))
            .unwrap(),
        Value::Int(initial + 1)
    );
    assert_eq!(
        objects
            .native("IsSuspendedXP", std::slice::from_ref(&hero))
            .unwrap(),
        Value::Bool(true),
        "the map's real hero-level trigger must suspend XP"
    );
    objects
        .native(
            "AddHeroXP",
            &[hero.clone(), Value::Int(threshold), Value::Bool(false)],
        )
        .unwrap();
    assert_eq!(
        objects.native("GetHeroXP", &[hero]).unwrap(),
        Value::Int(threshold)
    );
}

#[test]
#[ignore = "requires selected Human01 through PCX_STARTUP_*; authored cinematic skip gate"]
fn selected_campaign_escape_returns_control_through_the_authored_trigger() {
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_STARTUP_MAP"));
    let bytes = std::fs::read(&map).unwrap();
    let seed = env("PCX_STARTUP_SEED").parse().unwrap();
    let mut startup = start(
        &install,
        selected_mode(),
        &map.display().to_string(),
        &bytes,
        seed,
        None,
    )
    .unwrap();
    startup.step_clock(64).unwrap();
    let before = startup
        .session
        .host()
        .unwrap()
        .objects()
        .presentation()
        .unwrap()
        .control_enabled;
    assert_eq!(before, Some(false));
    let fired = startup
        .session
        .host()
        .unwrap()
        .objects_mut()
        .fire_local_player_event("EVENT_PLAYER_END_CINEMATIC")
        .unwrap();
    assert!(
        fired > 0,
        "the original intro-cancel registration must receive Escape"
    );
    startup.step_clock(2048).unwrap();
    let skipped = startup.session.global("udg_IntroSkipped");
    let arrived = startup.session.global("udg_HasArthasArrived");
    let objects = startup.session.host().unwrap().objects();
    let presentation = objects.presentation().unwrap();
    eprintln!(
        "ESCAPE clock={:?} control={:?} interface={:?} intro_skipped={:?} arrived={:?}",
        objects.timer_game_time(),
        presentation.control_enabled,
        presentation.interface_visible,
        skipped,
        arrived
    );
    assert_eq!(presentation.control_enabled, Some(true));
    assert_eq!(presentation.interface_visible, Some(true));
    assert_eq!(skipped, Some(Value::Bool(true)));
    assert_eq!(arrived, Some(Value::Bool(true)));
    let board = pcx_play::quest_view::project(objects.quest_session().unwrap()).unwrap();
    for quest in &board.quests {
        eprintln!(
            "QUEST {} discovered={} complete={} failed={}",
            quest.title.unwrap_or("<unset>"),
            quest.discovered,
            quest.completed,
            quest.failed
        );
    }

    // Diagnostic stimulus: replay the map's footman destination orders after skip.
    // The real EnterRect actions must finish even though their target Uther is gone.
    let uther = startup.session.global("udg_Uther").unwrap();
    assert_eq!(
        startup
            .session
            .host()
            .unwrap()
            .native("GetUnitLoc", &[uther])
            .unwrap(),
        Value::Null
    );
    let mut callbacks = Vec::new();
    for (unit, rect, trigger) in [
        (
            "udg_StartingFootman01",
            "gg_rct_Footman_01_End",
            "gg_trg___Footman01_End_Movement",
        ),
        (
            "udg_StartingFootman02",
            "gg_rct_Footman_02_End",
            "gg_trg___Footman02_End_Movement",
        ),
    ] {
        let unit = startup.session.global(unit).unwrap();
        let Some(Value::Handle(rect)) = startup.session.global(rect) else {
            panic!("authored destination rectangle");
        };
        let trigger = startup.session.global(trigger).unwrap();
        let host = startup.session.host().unwrap();
        assert_eq!(
            host.native("IsTriggerEnabled", std::slice::from_ref(&trigger))
                .unwrap(),
            Value::Bool(true)
        );
        let bounds = host.objects().rectangle_bounds(rect).unwrap();
        let target = [(bounds[0] + bounds[2]) / 2.0, (bounds[1] + bounds[3]) / 2.0];
        assert_eq!(
            host.native(
                "IssuePointOrder",
                &[
                    unit.clone(),
                    Value::Str("move".into()),
                    Value::Real(target[0]),
                    Value::Real(target[1])
                ]
            )
            .unwrap(),
            Value::Bool(true)
        );
        callbacks.push((trigger, unit));
    }
    startup
        .step_clock(256)
        .expect("post-skip region callbacks must finish");
    let callbacks = callbacks
        .into_iter()
        .map(|(trigger, unit)| {
            let enabled = startup
                .session
                .host()
                .unwrap()
                .native("IsTriggerEnabled", &[trigger])
                .unwrap();
            eprintln!("FOOTMAN callback for {unit:?}: enabled={enabled:?}");
            enabled
        })
        .collect::<Vec<_>>();
    assert!(
        callbacks.contains(&Value::Bool(false)),
        "a real footman EnterRect callback must have run"
    );
}

/// Observes the unchanged opening, without injecting control, pause, alliances or orders.
/// Checks the opening survival regression and repeatability, not complete native parity.
#[test]
#[ignore = "requires selected Human01 through PCX_STARTUP_*; diagnostic trace, not chapter acceptance"]
fn selected_campaign_opening_trace_repeats_without_player_input() {
    use pcx_play::playercolor_host::CombatTraceEvent;
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_STARTUP_MAP"));
    let bytes = std::fs::read(&map).unwrap();
    let seed = env("PCX_STARTUP_SEED").parse().unwrap();
    let run = || {
        let mut startup = start(
            &install,
            selected_mode(),
            &map.display().to_string(),
            &bytes,
            seed,
            None,
        )
        .unwrap();
        let local = local_slot(&startup);
        let objects = startup.session.host().unwrap().objects_mut();
        let (hero, rawcode, initial) = objects
            .executed_units()
            .find(|(_, unit)| {
                unit.executed_owner == local
                    && unit.abilities.as_ref().is_some_and(|a| a.hero.is_some())
            })
            .map(|(id, unit)| (id, unit.definition.rawcode, unit.vitals.unwrap().life))
            .expect("local source hero");
        objects.enable_combat_trace(8192);
        startup
            .step_clock(3840)
            .expect("bounded opening projection");
        let passive = startup.session.global("ALLIANCE_PASSIVE").unwrap();
        let objects = startup.session.host().unwrap().objects();
        let trace = objects.combat_trace().unwrap();
        assert_eq!(trace.dropped(), 0, "opening trace capacity exhausted");
        let events = trace.events().to_vec();
        let life = objects.executed_unit(hero).unwrap().vitals.unwrap().life;
        assert!(
            life > 0.0,
            "the source human hero must survive the opening without player input"
        );
        assert_eq!(
            objects.timer_game_time(),
            Some(60.0),
            "opening must reach the requested clock rather than an unintended Defeat pause"
        );
        eprintln!(
            "OPENING hero={hero:?} type={} initial_life={initial} final_life={life} clock={:?} paused={:?} presentation={:?} exits={:?}",
            String::from_utf8_lossy(&rawcode),
            objects.timer_game_time(),
            objects.game_paused(),
            objects
                .presentation()
                .map(|p| (p.control_enabled, p.interface_visible)),
            objects.game_exit_requests()
        );
        for event in &events {
            let (source, target) = match event {
                CombatTraceEvent::Acquired { source, target, .. }
                | CombatTraceEvent::Damage { source, target, .. } => (source, target),
                _ => continue,
            };
            if target.unit != hero {
                continue;
            }
            let a = objects.player_settings(source.owner).unwrap();
            let b = objects.player_settings(target.owner).unwrap();
            let passive = match passive {
                Value::Handle(setting) => a
                    .alliances
                    .iter()
                    .find(|(other, kind, _)| *other == b.identity && *kind == setting)
                    .map(|(_, _, value)| *value),
                _ => None,
            };
            eprintln!(
                "LOCAL_HERO source_team={:?} target_team={:?} explicit_passive={passive:?} {event:?}",
                a.team, b.team
            );
        }
        (
            events,
            life.to_bits(),
            objects.timer_game_time(),
            objects.game_paused(),
        )
    };
    let first = run();
    let repeat = run();
    assert_eq!(first, repeat, "unchanged no-input openings must reproduce");
}

/// PCX_SUMMON_MAP selects an unchanged map whose authored script casts AHwe.
/// This is a bounded cast gate, not chapter completion or native parity.
#[test]
#[ignore = "requires PCX_SUMMON_MAP and the selected PCX_STARTUP_* install/mode/seed"]
fn selected_summoning_map_creates_a_live_source_loaded_unit_without_player_orders() {
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_SUMMON_MAP"));
    let bytes = std::fs::read(&map).expect("read summoning map");
    let seed = env("PCX_STARTUP_SEED").parse().expect("seed");
    let mut startup = start(
        &install,
        selected_mode(),
        &map.display().to_string(),
        &bytes,
        seed,
        None,
    )
    .expect("summoning startup");
    assert!(
        !startup
            .session
            .host()
            .unwrap()
            .objects()
            .executed_units()
            .any(|(_, u)| u.summoned)
    );
    for tick in 1..=2048 {
        startup
            .step_clock(1)
            .unwrap_or_else(|error| panic!("tick {tick}: {error}"));
        let objects = startup.session.host().unwrap().objects();
        if let Some((id, unit)) = objects.executed_units().find(|(_, u)| u.summoned) {
            assert!(
                unit.position
                    .is_some_and(|p| p.into_iter().all(f32::is_finite))
            );
            assert!(unit.vitals.is_some_and(|v| v.life > 0.));
            assert!(unit.combat.as_ref().is_some_and(|c| !c.dead));
            let model = &unit.animation.as_ref().expect("source unit model").model;
            assert!(!model.origin.source.is_empty());
            assert!(!model.origin.version.is_empty());
            eprintln!(
                "summon tick={tick} handle={id:?} rawcode={} owner={} position={:?} model={} origin={:?}",
                String::from_utf8_lossy(&unit.definition.rawcode),
                unit.executed_owner,
                unit.position,
                model.member,
                model.origin
            );
            return;
        }
    }
    panic!("no scripted summon within the bounded diagnostic window");
}

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
#[ignore = "requires selected local install/map through PCX_STARTUP_*; see module comment"]
fn selected_map_profile_is_explicit_in_identity_and_saves_stay_in_the_session() {
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_STARTUP_MAP"));
    let bytes = std::fs::read(&map).unwrap();
    let label = map.display().to_string();
    let seed = env("PCX_STARTUP_SEED").parse().unwrap();
    let mut first = start(&install, selected_mode(), &label, &bytes, seed, None).unwrap();
    let initial = first.admission_digest();
    let host = first.session.host().unwrap().objects_mut();
    let cache = host
        .native("InitGameCache", &[Value::Str("synthetic-profile".into())])
        .unwrap();
    let keys = [
        cache.clone(),
        Value::Str("mission".into()),
        Value::Str("sentinel".into()),
    ];
    let mut put = keys.to_vec();
    put.push(Value::Int(42));
    host.native("StoreInteger", &put).unwrap();
    assert_eq!(
        host.native("SaveGameCache", &[cache]).unwrap(),
        Value::Bool(true)
    );
    let saved = first.campaign_profile().unwrap().unwrap();
    assert_eq!(
        first.admission_digest(),
        initial,
        "saving must not change initial identity"
    );
    let mut second = start(
        &install,
        selected_mode(),
        &label,
        &bytes,
        seed,
        Some(&saved),
    )
    .unwrap();
    let repeat = start(
        &install,
        selected_mode(),
        &label,
        &bytes,
        seed,
        Some(&saved),
    )
    .unwrap();
    assert_ne!(
        initial,
        second.admission_digest(),
        "profile bytes must affect admission"
    );
    assert_eq!(second.admission_digest(), repeat.admission_digest());
    let host = second.session.host().unwrap().objects_mut();
    let cache = host
        .native("InitGameCache", &[Value::Str("synthetic-profile".into())])
        .unwrap();
    let keys = [cache.clone(), keys[1].clone(), keys[2].clone()];
    assert_eq!(
        host.native("GetStoredInteger", &keys).unwrap(),
        Value::Int(42)
    );
    let mut put = keys.to_vec();
    put.push(Value::Int(99));
    host.native("StoreInteger", &put).unwrap();
    host.native("SaveGameCache", &[cache]).unwrap();
    let changed = second.campaign_profile().unwrap().unwrap();
    assert_ne!(changed, saved);
    let changed_start = start(
        &install,
        selected_mode(),
        &label,
        &bytes,
        seed,
        Some(&changed),
    )
    .unwrap();
    assert_ne!(
        second.admission_digest(),
        changed_start.admission_digest(),
        "different present profiles must affect identity"
    );
    assert_eq!(first.campaign_profile().unwrap().unwrap(), saved);
    assert_eq!(repeat.campaign_profile().unwrap().unwrap(), saved);
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

    let mut first = start(&install, mode, &label, &bytes, seed, None)
        .unwrap_or_else(|error| panic!("first start: {error}"));
    let mut second = start(&install, mode, &label, &bytes, seed, None)
        .unwrap_or_else(|error| panic!("second start: {error}"));
    let mut control = start(&install, mode, &label, &bytes, seed, None)
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

/// PCX_PATROL_MAP must name an unchanged map whose initialization issues patrol orders
/// (the selected Human03 ROC map exercises this path).
#[test]
#[ignore = "requires PCX_PATROL_MAP plus PCX_STARTUP_INSTALL, PCX_STARTUP_ADDON and PCX_STARTUP_SEED"]
fn selected_patrol_map_retains_scripted_patrol_endpoints_after_initialization() {
    let install = PathBuf::from(env("PCX_STARTUP_INSTALL"));
    let map = PathBuf::from(env("PCX_PATROL_MAP"));
    let bytes = std::fs::read(&map).expect("read patrol map");
    let seed = env("PCX_STARTUP_SEED").parse().expect("seed");
    let mut startup = start(
        &install,
        selected_mode(),
        &map.display().to_string(),
        &bytes,
        seed,
        None,
    )
    .expect("patrol startup");
    let objects = startup.session.host().unwrap().objects_mut();
    let patrol = objects
        .native("OrderId", &[Value::Str("patrol".into())])
        .unwrap();
    assert_eq!(
        objects
            .native("OrderId2String", std::slice::from_ref(&patrol))
            .unwrap(),
        Value::Str("patrol".into())
    );
    let units: Vec<_> = objects.executed_units().map(|(id, _)| id).collect();
    let mut count = 0;
    for unit in units {
        if objects
            .native("GetUnitCurrentOrder", &[Value::Handle(unit)])
            .unwrap()
            == patrol
        {
            let activity = objects
                .executed_unit(unit)
                .unwrap()
                .activity
                .as_ref()
                .unwrap();
            assert!(activity.movable());
            assert!(
                activity.destination().is_some(),
                "the script's endpoint must be retained"
            );
            count += 1;
        }
    }
    assert!(
        count > 0,
        "the selected map did not exercise scripted patrol"
    );
    eprintln!("scripted patrol endpoints retained: {count}");
}
