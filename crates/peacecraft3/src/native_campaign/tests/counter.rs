use super::*;
use peacecraft3::campaign_runtime::CampaignRuntime;
use std::time::Duration;

#[test]
#[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL, MAP=Maps\\Campaign\\Human01.w3m, ADDON=roc; private display/audio/profile isolation"]
fn selected_human01_counter_frontier() {
    chapter_counter_horizon("Human01.w3m");
}

#[test]
#[ignore = "requires PCX_NATIVE_CHAPTER_INSTALL, MAP=Maps\\Campaign\\Undead02.w3m, ADDON=roc; private display/audio/profile isolation"]
fn selected_undead02_counter_frontier() {
    chapter_counter_horizon("Undead02.w3m");
}

fn chapter_counter_horizon(expected: &str) {
    let (install, map, expansion) = selected_chapter_env();
    assert!(!expansion, "explicitly select the RoC source namespace");
    assert!(
        map.rsplit(['\\', '/'])
            .next()
            .unwrap()
            .eq_ignore_ascii_case(expected)
    );
    let mut app = chapter_app_with_startup(
        install,
        expansion,
        peacecraft3::campaign_startup::install_native_counter,
    );
    let recipe = enqueue_selected_chapter(&mut app, &map);
    for _ in 0..6 {
        app.update();
    }
    assert_published_chapter(&mut app, &map, recipe, true);
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let context = Arc::clone(session.context().unwrap());
    assert!(session.host().unwrap().objects().native_counter_platform());
    println!("Counter chapter={map} context={:?}", context.sources.map);
    // Diagnostic input cadence and horizon; source Counter rules retain all game timing.
    let input = Duration::from_millis(25);
    let mut runtime = CampaignRuntime::new(input, 8, 65536);
    let mut cinematic = false;
    let initial_progress = session
        .host()
        .unwrap()
        .objects()
        .native_counter_progress_ns()
        .unwrap();
    let mut control_point = None;
    const FRAMES: u32 = 12000;
    for frame in 0..FRAMES {
        runtime.advance(&mut session, input);
        if runtime.last_error.is_some() {
            print_counter_failure_candidates(&mut session);
        }
        let objects = session.host().unwrap().objects();
        assert!(
            runtime.last_error.is_none(),
            "Counter frontier: chapter={map} frame={frame} input={input:?} clock={:?} context={:?} error={:?}",
            objects.native_game_point(),
            context.sources.map,
            runtime.last_error
        );
        let state = objects.presentation().unwrap();
        cinematic |= state.control_enabled == Some(false);
        if cinematic && state.control_enabled == Some(true) && state.interface_visible == Some(true)
        {
            control_point.get_or_insert(objects.native_game_point().unwrap());
        }
        if frame % 1000 == 0 {
            println!(
                "Counter progress: chapter={map} frame={frame} game={:?} committed_ns={:?} pending_ns={:?}",
                objects.native_game_point(),
                objects.native_counter_progress_ns(),
                objects.native_counter_pending_ns()
            );
        }
        if state.control_enabled == Some(false) && frame % 10 == 0 {
            session
                .host()
                .unwrap()
                .objects_mut()
                .fire_local_player_event("EVENT_PLAYER_END_CINEMATIC")
                .unwrap();
        }
        assert!(
            requested_exit(&mut session).is_none(),
            "unexpected early Counter exit: chapter={map} frame={frame} clock={:?} context={:?}",
            session.host().unwrap().objects().native_game_point(),
            context.sources.map,
        );
    }
    let control_point = control_point
        .expect("Counter chapter must restore player control within the diagnostic horizon");
    // Runtime's first session bind deliberately discards that call's input.
    let submitted_ns = u128::from(FRAMES - 1) * input.as_nanos();
    let objects = session.host().unwrap().objects();
    let committed = objects.native_counter_progress_ns().unwrap() - initial_progress;
    let pending = objects.native_counter_pending_ns().unwrap();
    println!(
        "Counter input accounting: chapter={map} submitted_ns={submitted_ns} committed_ns={committed} pending_ns={pending} control={control_point:?} game={:?}",
        objects.native_game_point()
    );
    assert_eq!(
        committed.checked_add(pending).unwrap(),
        submitted_ns,
        "Counter input accounting: {map}"
    );
    // Diagnostic retry bound; these calls supply no new elapsed input or additional per-call budget.
    for drain in 0..64 {
        if session
            .host()
            .unwrap()
            .objects()
            .native_counter_pending_ns()
            == Some(0)
        {
            break;
        }
        if let Err(error) = session.advance_native_counter_frame(None, 65536) {
            assert!(
                session.native_pacing_can_resume(&error),
                "Counter drain frontier: chapter={map} drain={drain} clock={:?} context={:?} error={error}",
                session.host().unwrap().objects().native_game_point(),
                context.sources.map
            );
        }
        assert!(
            requested_exit(&mut session).is_none(),
            "unexpected Counter exit while draining {map}"
        );
    }
    let objects = session.host().unwrap().objects();
    let final_point = objects.native_game_point().unwrap();
    let committed = objects.native_counter_progress_ns().unwrap() - initial_progress;
    let pending = objects.native_counter_pending_ns().unwrap();
    println!(
        "Counter final: chapter={map} submitted_ns={submitted_ns} committed_ns={committed} pending_ns={pending} control={control_point:?} game={final_point:?}"
    );
    assert_eq!(pending, 0, "persistent recoverable Counter refusal: {map}");
    assert_eq!(
        committed, submitted_ns,
        "Counter input must drain completely: {map}"
    );
    // The cycle distinguishes native local-clock wraps; local stores a binary32 time word.
    assert!(
        final_point.cycle > control_point.cycle
            || (final_point.cycle == control_point.cycle
                && f32::from_bits(final_point.local) > f32::from_bits(control_point.local)),
        "Game point did not advance after control: chapter={map} control={control_point:?} game={final_point:?}"
    );
}

fn print_counter_failure_candidates(session: &mut MapScriptSession<MapStartupHost>) {
    for name in ["udg_IntroCinematicSkip", "bj_cineModeAlreadyIn"] {
        eprintln!(
            "Counter failure global: name={name} value={:?}",
            session.global(name)
        );
    }
    // These source-script globals are candidates, not a trace of the refused call's arguments.
    for name in [
        "udg_Arthas",
        "gg_unit_Hpb1_0018",
        "gg_unit_uaco_0048",
        "gg_unit_Utic_0006",
        "gg_unit_nsqt_0054",
        "gg_unit_nsqe_0072",
        "udg_KelThuzad",
    ] {
        let value = session.global(name);
        eprintln!("Counter failure candidate: global={name} value={value:?}");
        let Some(pcx_script::Value::Handle(unit)) = value else {
            continue;
        };
        let Some(host) = session.host() else {
            eprintln!("Counter failure candidate: global={name} session host absent");
            continue;
        };
        let objects = host.objects();
        match objects.executed_unit(unit) {
            Ok(record) => {
                eprintln!(
                    "Counter failure candidate: global={name} unit={unit:?} type={:?} owner={:?} creation_owner={} position={:?} face={} shown={} paused_since={:?} construction={} morph={:?} admitted_order={:?} acquire_request={:?}",
                    record.definition.rawcode,
                    record.owner,
                    record.executed_owner,
                    record.position,
                    record.face,
                    record.shown,
                    record.paused_since,
                    record.construction.is_some(),
                    record.morph.as_ref().map(|m| (
                        m.ability,
                        m.normal,
                        m.alternate,
                        m.revision,
                        m.phase
                    )),
                    record.admitted_order,
                    record.acquire_range,
                );
                if let Some(activity) = &record.activity {
                    eprintln!(
                        "Counter failure candidate: global={name} movable={} held={} moving={} speed={:?} destination={:?} followed_unit={:?} follow_range={:?} waypoint_count={} next_waypoint={} internal_heading=not_exposed",
                        activity.movable(),
                        activity.held(),
                        activity.moving(),
                        activity.speed(),
                        activity.destination(),
                        activity.followed_unit(),
                        activity.follow_range(),
                        activity.waypoints().0.len(),
                        activity.waypoints().1,
                    );
                } else {
                    eprintln!("Counter failure candidate: global={name} activity=absent");
                }
                if let Some(combat) = &record.combat {
                    eprintln!(
                        "Counter failure candidate: global={name} dead={} weapon_range_acquire_cooldown={:?} target={:?} attack_destination={:?} ground_target={:?}",
                        combat.dead,
                        combat
                            .weapon
                            .as_ref()
                            .map(|w| (w.range, w.acquire, w.cooldown)),
                        combat.target,
                        combat.attack_destination,
                        combat.ground_target,
                    );
                } else {
                    eprintln!("Counter failure candidate: global={name} combat=absent");
                }
                eprintln!(
                    "Counter failure candidate: global={name} current_order={:?} queued_count={} training_count={:?} owner_captains={:?} guard={:?}",
                    objects.current_unit_order(unit),
                    objects.queued_orders(unit).len(),
                    objects.unit_training_queue(unit).map(|queue| queue.len()),
                    objects.campaign_captain_state(record.owner),
                    objects.guard_state().and_then(|state| state.get(unit)),
                );
            }
            Err(error) => eprintln!(
                "Counter failure candidate: global={name} unit={unit:?} read_error={error}"
            ),
        }
    }
}
