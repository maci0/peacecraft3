//! Local-original Prologue01 objective steps through the production campaign clock.
//!
//! The map completes when a player-one unit enters its victory rects and its own
//! next-level trigger runs. This module does not call that trigger, rewrite the
//! map, or issue victory. The first step restores control with the local
//! end-cinematic event, then moves `udg_Thrall` to the center of `gg_rct_Spot01`.
//! Reaching that rect is only the physical checkpoint. The lesson is complete
//! when the map disables its own move-done trigger and the next lesson starts
//! the following banner's fog modifier. Handles, bounds and fog centers are
//! read from the running session.
//!
//! Chain lightning (`AOcl`) is a later tutorial check. Published learned
//! target/immediate commands are Holy Light and Water Elemental only, so this
//! slice does not cast. Inventory-slot creation is a separate declared fix.
//!
//! The hero lesson is not proved by learning early. `H1_HeroLevel_DoneC` and
//! `H2_HeroPoints_DoneB` are alternate bypasses when `AOcl` is learned before
//! those presentations finish. `A2_MoreGnolls_Abort` enables
//! `H3_SpendPoint_Done` after `SetHeroLevel`, so that trigger being enabled
//! does not show that H1 or H2 ran. Before the public learn, H1 done A/B/C and
//! H2 done A/B must already be disabled by their own timer, skill-menu, or
//! bypass actions, and the H3 prompt must have started its narrator
//! transmission. The learn itself is the card `SelectHeroSkill` path while
//! `H3_SpendPoint_Done` is enabled and `GetLearnedSkillBJ` is `AOcl`. No
//! player-one unit enters the victory rects before the combat and inventory
//! lessons retire. `Victory_Found_Medivh` accepts any player-one unit and then
//! queues the quest completion and end cinematic.

use bevy::prelude::*;
use pcx_play::{
    map_startup_host::MapStartupHost,
    player_command_log::{PlayerCommand, StartIdentity},
    standard_command::Kind,
};
use pcx_script::{HandleId, Host, Value};
use peacecraft3::{
    campaign_runtime::CampaignRuntime, campaign_startup::CampaignSeed,
    map_script_session::MapScriptSession,
};
use std::time::Duration;

const END_CINEMATIC: &str = "EVENT_PLAYER_END_CINEMATIC";
const THRALL: &str = "udg_Thrall";
const FIRST_BANNER: &str = "gg_rct_Spot01";
const NEXT_BANNER: &str = "gg_rct_Spot02";
const THIRD_BANNER: &str = "gg_rct_Spot03";
const MOVE_DONE: &str = "gg_trg_S2_MoveThrall_Done";
const MOVE_PROMPT: &str = "gg_trg_S2_MoveThrall_Q";
const SCROLL_DONE: &str = "gg_trg_S3_Scroll_Done";
const SCROLL_PROMPT: &str = "gg_trg_S3_Scroll_Q";
const MASK_RECT: &str = "gg_rct_FlagExplore03A";
const MASK_DONE: &str = "gg_trg_S4_BlackMask_Done";
const MASK_PROMPT: &str = "gg_trg_S4_BlackMask_Q";
const UNMASK_RECT: &str = "gg_rct_Spot04";
const UNMASK_DONE: &str = "gg_trg_S5_Unmasking_Done";
const UNMASK_PROMPT: &str = "gg_trg_S5_Unmasking_Q";
const MOVE_MORE_DONE: &str = "gg_trg_S6_MoveMore_Done";
const MOVE_MORE_PROMPT: &str = "gg_trg_S6_MoveMore_Q";
const RESCUE_DONE: &str = "gg_trg_R1_Rescue_Done";
const RESCUE_PROMPT: &str = "gg_trg_R1_Rescue_Q";
const YOUR_UNITS: &str = "udg_YourUnits";
const GROUP_DONE: &str = "gg_trg_R2_GroupSelect_Done";
const GROUP_PROMPT: &str = "gg_trg_R2_GroupSelect_Q";
const GROUP_MOVE_RECT: &str = "gg_rct_Spot05";
const GROUP_MOVE_DONE: &str = "gg_trg_R3_GroupMove_Done";
const GROUP_MOVE_PROMPT: &str = "gg_trg_R3_GroupMove_Q";
const GNOLL: &str = "gg_unit_ngno_0060";
const GNOLL_APPROACH: &str = "gg_rct_Spot06";
const GNOLL_DONE: &str = "gg_trg_A1_SingleGnoll_DoneA";
const GNOLL_PROMPT: &str = "gg_trg_A1_SingleGnoll_Q";
const HERO_LEVEL_A: &str = "gg_trg_H1_HeroLevel_DoneA";
const HERO_LEVEL_B: &str = "gg_trg_H1_HeroLevel_DoneB";
const HERO_LEVEL_C: &str = "gg_trg_H1_HeroLevel_DoneC";
const HERO_LEVEL_PROMPT: &str = "gg_trg_H1_HeroLevel_Q";
const MORE_GNOLLS_DONE: &str = "gg_trg_A2_MoreGnolls_Done";
const MORE_GNOLLS_PROMPT: &str = "gg_trg_A2_MoreGnolls_Q";
const GNOLL_GROUP: &str = "udg_GnollGroup";
const HERO_POINTS_A: &str = "gg_trg_H2_HeroPoints_DoneA";
const HERO_POINTS_B: &str = "gg_trg_H2_HeroPoints_DoneB";
const HERO_POINTS_PROMPT: &str = "gg_trg_H2_HeroPoints_Q";
const SPEND_DONE: &str = "gg_trg_H3_SpendPoint_Done";
const SPEND_PROMPT: &str = "gg_trg_H3_SpendPoint_Q";
const CHAIN_SKILL: [u8; 4] = *b"AOcl";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires unchanged RoC Prologue01 via PCX_NATIVE_CHAPTER_* and a fresh mode-0700 XDG_DATA_HOME under .scratch"]
    fn selected_prologue_reaches_the_first_authored_banner() {
        isolate_campaign_profile();
        let map = std::env::var("PCX_NATIVE_CHAPTER_MAP")
            .expect("set PCX_NATIVE_CHAPTER_MAP to the selected chapter row");
        let file = map.rsplit(['\\', '/']).next().unwrap_or(map.as_str());
        assert!(
            file.eq_ignore_ascii_case("Prologue01.w3m"),
            "this gate plays the selected Prologue01 row, not {map}"
        );
        assert_eq!(
            std::env::var("PCX_NATIVE_CHAPTER_ADDON").as_deref(),
            Ok("roc"),
            "Prologue01 is a Reign of Chaos chapter"
        );
        let mut app = crate::native_campaign::tests::started_app();
        begin_journal(&mut app);
        let mut runtime = launcher_runtime();
        bind_runtime(&mut app, &mut runtime);
        restore_control(&mut app, &mut runtime);
        wait_for_discovered_quest(&mut app, &mut runtime);
        assert!(
            trigger_enabled(&mut app, MOVE_DONE).unwrap_or(false),
            "the map's move-done trigger was not armed before the order: {}",
            observe(&mut app, &runtime)
        );
        let bounds = order_thrall_to_banner(&mut app);
        let arrived = walk_into(&mut app, &mut runtime, bounds);
        assert!(
            arrived,
            "Thrall did not physically reach the authored first banner {}",
            observe(&mut app, &runtime)
        );
        {
            let journal = app
                .world()
                .resource::<crate::campaign_input::CampaignInput>();
            assert!(
                journal.commands.entries().iter().any(|entry| {
                    matches!(&entry.command, PlayerCommand::Point { order, x, y, .. }
                        if order.eq_ignore_ascii_case("move")
                            && *x == center(bounds)[0]
                            && *y == center(bounds)[1])
                }),
                "the accepted journal must contain the public move"
            );
        }
        // Position is not the lesson. Drain until the map disables its own
        // move-done trigger and the following lesson starts the next banner's fog.
        let next_center = rect_center(&mut app, NEXT_BANNER);
        let mut lesson = false;
        for _ in 0..30 {
            lesson = lesson_advanced(&mut app, MOVE_DONE, MOVE_PROMPT, next_center);
            if lesson {
                break;
            }
            drive(&mut app, &mut runtime, 1);
        }
        assert!(
            lesson,
            "Thrall is inside the banner, but the authored move lesson did not advance: {}",
            observe(&mut app, &runtime)
        );
        let (discovered, completed) = quest_progress(&mut app);
        assert!(
            discovered && !completed,
            "the move lesson must leave the quest discovered and unfinished: {}",
            observe(&mut app, &runtime)
        );
        // The scroll lesson completes when Thrall enters the next authored rect.
        // The narration mentions the camera; the map's done trigger does not.
        assert!(
            trigger_enabled(&mut app, SCROLL_DONE).unwrap_or(false),
            "the scroll-done trigger was not armed after the move lesson: {}",
            observe(&mut app, &runtime)
        );
        let scroll_bounds = rect_bounds(&mut app, NEXT_BANNER);
        order_thrall_to(&mut app, scroll_bounds);
        assert!(
            walk_into(&mut app, &mut runtime, scroll_bounds),
            "Thrall did not physically reach the authored second banner {}",
            observe(&mut app, &runtime)
        );
        let third_center = rect_center(&mut app, THIRD_BANNER);
        let mut scroll = false;
        for _ in 0..30 {
            scroll = lesson_advanced(&mut app, SCROLL_DONE, SCROLL_PROMPT, third_center);
            if scroll {
                break;
            }
            drive(&mut app, &mut runtime, 1);
        }
        assert!(
            scroll,
            "Thrall is inside the second banner, but the authored scroll lesson did not advance: {}",
            observe(&mut app, &runtime)
        );
        let (discovered, completed) = quest_progress(&mut app);
        assert!(
            discovered && !completed,
            "the scroll lesson must leave the quest discovered and unfinished: {}",
            observe(&mut app, &runtime)
        );
        play_region_lesson(
            &mut app,
            &mut runtime,
            MASK_RECT,
            MASK_DONE,
            MASK_PROMPT,
            "Thrall did not physically reach the authored black-mask rect",
            "Thrall is inside the black-mask rect, but that lesson did not advance",
        );
        play_region_lesson(
            &mut app,
            &mut runtime,
            UNMASK_RECT,
            UNMASK_DONE,
            UNMASK_PROMPT,
            "Thrall did not physically reach the authored unmasking rect",
            "Thrall is inside the unmasking rect, but that lesson did not advance",
        );
        // Spot04 is also S6's done rect, so that lesson retires on the same entry.
        // R1 then gives the grunts on its own timer. Neither is a victory walk.
        retire_lesson(
            &mut app,
            &mut runtime,
            MOVE_MORE_DONE,
            MOVE_MORE_PROMPT,
            "the move-more lesson did not retire after the unmasking rect",
        );
        retire_lesson(
            &mut app,
            &mut runtime,
            RESCUE_DONE,
            RESCUE_PROMPT,
            "the rescue lesson did not retire on its authored timer",
        );
        assert!(
            trigger_enabled(&mut app, GROUP_DONE).unwrap_or(false),
            "rescue did not arm the group-select lesson: {}",
            observe(&mut app, &runtime)
        );
        select_group(&mut app, YOUR_UNITS);
        retire_lesson(
            &mut app,
            &mut runtime,
            GROUP_DONE,
            GROUP_PROMPT,
            "selecting the authored group did not retire the group-select lesson",
        );
        let move_bounds = rect_bounds(&mut app, GROUP_MOVE_RECT);
        order_selected(&mut app, Kind::Move, center(move_bounds), None);
        assert!(
            walk_into(&mut app, &mut runtime, move_bounds),
            "the group move did not bring Thrall into the authored rect {}",
            observe(&mut app, &runtime)
        );
        retire_lesson(
            &mut app,
            &mut runtime,
            GROUP_MOVE_DONE,
            GROUP_MOVE_PROMPT,
            "entering the group-move rect did not retire that lesson",
        );
        attack_the_gnoll(&mut app, &mut runtime);
        clear_gnoll_group(&mut app, &mut runtime);
        assert!(
            trigger_enabled(&mut app, SPEND_DONE).unwrap_or(false),
            "the spend-point lesson was not armed after the gnoll group died: {}",
            observe(&mut app, &runtime)
        );
        assert_eq!(
            ability_level(&mut app, THRALL, CHAIN_SKILL),
            0,
            "chain lightning was learned before the hero lessons"
        );
        show_skill_menu(&mut app);
        assert!(
            triggers_down(
                &mut app,
                &[HERO_LEVEL_A, HERO_LEVEL_B, HERO_LEVEL_C, HERO_LEVEL_PROMPT],
                45,
                &mut runtime
            ),
            "the show-skill event did not retire the hero-level lesson: {}",
            observe(&mut app, &runtime)
        );
        if trigger_enabled(&mut app, HERO_POINTS_A).unwrap_or(false) {
            show_skill_menu(&mut app);
        }
        assert!(
            triggers_down(
                &mut app,
                &[HERO_POINTS_A, HERO_POINTS_B, HERO_POINTS_PROMPT],
                45,
                &mut runtime
            ),
            "the show-skill event did not retire the hero-points lesson: {}",
            observe(&mut app, &runtime)
        );
        assert!(
            narration_armed(&mut app, &mut runtime),
            "the spend-point narration was not armed before learning: {}",
            observe(&mut app, &runtime)
        );
        assert!(
            trigger_enabled(&mut app, SPEND_DONE).unwrap_or(false),
            "the spend-point trigger was not still armed before learning"
        );
        assert_eq!(ability_level(&mut app, THRALL, CHAIN_SKILL), 0);
        learn_ready_skill(&mut app, CHAIN_SKILL);
        assert!(
            triggers_down(&mut app, &[SPEND_DONE, SPEND_PROMPT], 45, &mut runtime),
            "learning the armed skill did not retire the spend-point lesson: {}",
            observe(&mut app, &runtime)
        );
        assert_quest_open(&mut app, &runtime, SPEND_DONE);
        let next = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            session
                .host()
                .and_then(|host| host.objects().next_level().map(str::to_string))
        };
        assert!(
            next.is_none(),
            "the first banner is not the authored victory: next level {next:?}"
        );
    }

    fn isolate_campaign_profile() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../.scratch")
            .canonicalize()
            .expect("local test artifacts directory");
        let profile = std::env::var_os("XDG_DATA_HOME").unwrap_or_else(|| {
            panic!(
                "set XDG_DATA_HOME to a fresh mode-0700 directory under .scratch before this process; the test does not create or select a profile"
            )
        });
        let profile = std::path::PathBuf::from(profile)
            .canonicalize()
            .expect("isolated profile directory");
        assert!(
            profile.starts_with(&scratch),
            "test profile must stay under .scratch"
        );
        let mode = std::fs::metadata(&profile)
            .expect("profile metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700, "isolated profile must be mode 0700");
        assert!(
            !profile.join("peacecraft3/campaign-cache.bin").exists(),
            "use a fresh campaign profile"
        );
    }

    fn launcher_runtime() -> CampaignRuntime {
        // Same application pacing launch_gui installs. Not a game-data constant.
        CampaignRuntime::new(Duration::from_secs_f64(1.0 / 64.0), 64, 4096)
    }

    fn bind_runtime(app: &mut App, runtime: &mut CampaignRuntime) {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        runtime.advance(&mut session, Duration::ZERO);
    }

    fn begin_journal(app: &mut App) {
        app.world_mut().resource_scope(
            |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
                let session = world.resource::<MapScriptSession<MapStartupHost>>();
                let seed = world
                    .resource::<CampaignSeed>()
                    .prepared
                    .expect("prepared seed");
                let context = session.context().expect("admitted context");
                input
                    .commands
                    .begin(StartIdentity::from_context(seed, context));
            },
        );
    }

    fn restore_control(app: &mut App, runtime: &mut CampaignRuntime) {
        let mut cinematic = false;
        let mut skipped = false;
        for _ in 0..90 {
            drive(app, runtime, 1);
            let state = presentation(app);
            cinematic |= state.control_enabled == Some(false);
            if cinematic && !skipped {
                fire_end_cinematic(app);
                skipped = true;
            }
            if cinematic
                && state.control_enabled == Some(true)
                && state.interface_visible == Some(true)
            {
                return;
            }
        }
        panic!(
            "opening did not return control through the production clock: {}",
            observe(app, runtime)
        );
    }

    fn fire_end_cinematic(app: &mut App) {
        app.world_mut().resource_scope(
            |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
                let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                let host = session.host().expect("published host");
                let clock = host.objects().timer_game_time();
                match host.objects_mut().fire_local_player_event(END_CINEMATIC) {
                    Ok(_) => {
                        let command = PlayerCommand::LocalEvent {
                            name: END_CINEMATIC.into(),
                        };
                        if let Err(error) = input.commands.record(clock, command) {
                            input.last_error = Some(error.to_string());
                        }
                    }
                    Err(error) => input.last_error = Some(error.to_string()),
                }
            },
        );
    }

    fn wait_for_discovered_quest(app: &mut App, runtime: &mut CampaignRuntime) {
        for _ in 0..30 {
            if quest_discovered(app) {
                return;
            }
            drive(app, runtime, 1);
        }
        panic!(
            "gameplay did not discover the authored quest: {}",
            observe(app, runtime)
        );
    }

    fn order_thrall_to_banner(app: &mut App) -> [f32; 4] {
        let bounds = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let rect = handle(&session, FIRST_BANNER);
            session
                .host()
                .expect("published host")
                .objects()
                .rectangle_bounds(rect)
                .unwrap_or_else(|error| panic!("{FIRST_BANNER} bounds: {error}"))
        };
        order_thrall_to(app, bounds);
        bounds
    }

    fn order_thrall_to(app: &mut App, bounds: [f32; 4]) {
        let thrall = {
            let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
            handle(session, THRALL)
        };
        let point = center(bounds);
        app.world_mut().resource_scope(
            |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
                let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                let objects = session.host().expect("published host").objects_mut();
                objects.select_units(&[thrall], true);
                let clock = objects.timer_game_time();
                let command = PlayerCommand::Select {
                    units: vec![thrall],
                    replace: true,
                };
                if let Err(error) = input.commands.record(clock, command) {
                    input.last_error = Some(error.to_string());
                }
                crate::campaign_input::standard_target(
                    objects,
                    &mut input,
                    Kind::Move,
                    point,
                    None,
                )
                .unwrap_or_else(|error| panic!("public move refused: {error}"));
            },
        );
        let input = app
            .world()
            .resource::<crate::campaign_input::CampaignInput>();
        let accepted = input.commands.entries().iter().any(|entry| {
            matches!(&entry.command, PlayerCommand::Point { unit, x, y, .. }
                if *unit == thrall && *x == point[0] && *y == point[1])
        });
        let refusal = input.last_error.clone();
        assert!(
            accepted,
            "IssuePointOrderById did not accept the move to {point:?}: {refusal:?}"
        );
    }

    fn walk_into(app: &mut App, runtime: &mut CampaignRuntime, bounds: [f32; 4]) -> bool {
        for _ in 0..180 {
            if inside(thrall_position(app), bounds) {
                return true;
            }
            drive(app, runtime, 1);
        }
        inside(thrall_position(app), bounds)
    }

    fn drive(app: &mut App, runtime: &mut CampaignRuntime, seconds: u32) {
        for _ in 0..seconds {
            {
                let mut session = app
                    .world_mut()
                    .resource_mut::<MapScriptSession<MapStartupHost>>();
                runtime.advance(&mut session, Duration::from_secs(1));
            }
            if let Some(error) = runtime.last_error.as_ref().map(ToString::to_string) {
                panic!(
                    "campaign runtime stopped: {error}; {}",
                    observe(app, runtime)
                );
            }
        }
    }

    struct Shown {
        control_enabled: Option<bool>,
        interface_visible: Option<bool>,
    }

    fn presentation(app: &mut App) -> Shown {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let state = session
            .host()
            .and_then(|host| host.objects().presentation());
        Shown {
            control_enabled: state.and_then(|state| state.control_enabled),
            interface_visible: state.and_then(|state| state.interface_visible),
        }
    }

    fn handle(session: &MapScriptSession<MapStartupHost>, name: &str) -> HandleId {
        match session.global(name) {
            Some(Value::Handle(id)) => id,
            other => panic!("{name} is not a live handle: {other:?}"),
        }
    }

    fn center(bounds: [f32; 4]) -> [f32; 2] {
        [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5]
    }

    fn inside(position: Option<[f32; 2]>, bounds: [f32; 4]) -> bool {
        let Some([x, y]) = position else {
            return false;
        };
        (bounds[0]..=bounds[2]).contains(&x) && (bounds[1]..=bounds[3]).contains(&y)
    }

    fn thrall_position(app: &mut App) -> Option<[f32; 2]> {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let id = handle(&session, THRALL);
        session
            .host()
            .expect("published host")
            .objects()
            .executed_unit(id)
            .ok()
            .and_then(|unit| unit.position)
    }

    fn quest_discovered(app: &mut App) -> bool {
        quest_progress(app).0
    }

    fn quest_progress(app: &mut App) -> (bool, bool) {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let Some(host) = session.host() else {
            return (false, false);
        };
        let Some(quests) = host.objects().quest_session() else {
            return (false, false);
        };
        let Ok(board) = pcx_play::quest_view::project(quests) else {
            return (false, false);
        };
        (
            board.quests.iter().any(|quest| quest.discovered),
            board.quests.iter().any(|quest| quest.completed),
        )
    }

    fn trigger_enabled(app: &mut App, name: &str) -> Result<bool, String> {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let id = handle(&session, name);
        let objects = session.host().expect("published host").objects_mut();
        match objects.native("IsTriggerEnabled", &[Value::Handle(id)]) {
            Ok(Value::Bool(enabled)) => Ok(enabled),
            Ok(other) => Err(format!("{name} enabled query returned {other:?}")),
            Err(error) => Err(format!("{name} enabled query: {error}")),
        }
    }

    fn rect_bounds(app: &mut App, name: &str) -> [f32; 4] {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let id = handle(&session, name);
        session
            .host()
            .expect("published host")
            .objects()
            .rectangle_bounds(id)
            .unwrap_or_else(|error| panic!("{name} bounds: {error}"))
    }

    fn rect_center(app: &mut App, name: &str) -> [f32; 2] {
        center(rect_bounds(app, name))
    }

    /// A lesson action disables its done trigger and its prompt, then the next
    /// lesson starts the fog modifier centered on its authored rect.
    fn lesson_advanced(app: &mut App, done: &str, prompt: &str, next_fog: [f32; 2]) -> bool {
        triggers_retired(app, done, prompt) && fog_started_at(app, next_fog) == Some(true)
    }

    fn triggers_retired(app: &mut App, done: &str, prompt: &str) -> bool {
        trigger_enabled(app, done).ok() == Some(false)
            && trigger_enabled(app, prompt).ok() == Some(false)
    }

    fn play_region_lesson(
        app: &mut App,
        runtime: &mut CampaignRuntime,
        rect: &str,
        done: &str,
        prompt: &str,
        physical: &str,
        lesson: &str,
    ) {
        assert!(
            trigger_enabled(app, done).unwrap_or(false),
            "{done} was not armed: {}",
            observe(app, runtime)
        );
        let bounds = rect_bounds(app, rect);
        order_thrall_to(app, bounds);
        assert!(
            walk_into(app, runtime, bounds),
            "{physical} {}",
            observe(app, runtime)
        );
        let mut advanced = false;
        for _ in 0..30 {
            advanced = triggers_retired(app, done, prompt);
            if advanced {
                break;
            }
            drive(app, runtime, 1);
        }
        assert!(advanced, "{lesson}: {}", observe(app, runtime));
        assert_quest_open(app, runtime, done);
    }

    fn retire_lesson(
        app: &mut App,
        runtime: &mut CampaignRuntime,
        done: &str,
        prompt: &str,
        lesson: &str,
    ) {
        retire_within(app, runtime, done, prompt, 45, lesson);
    }

    fn retire_within(
        app: &mut App,
        runtime: &mut CampaignRuntime,
        done: &str,
        prompt: &str,
        seconds: u32,
        lesson: &str,
    ) {
        let mut advanced = false;
        for _ in 0..seconds {
            advanced = triggers_retired(app, done, prompt);
            if advanced {
                break;
            }
            drive(app, runtime, 1);
        }
        assert!(advanced, "{lesson}: {}", observe(app, runtime));
        assert_quest_open(app, runtime, done);
    }

    fn select_group(app: &mut App, name: &str) {
        let members = units_in_group(app, name);
        assert!(!members.is_empty(), "{name} has no live members");
        app.world_mut().resource_scope(
            |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
                let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                let objects = session.host().expect("published host").objects_mut();
                objects.select_units(&members, true);
                let clock = objects.timer_game_time();
                let command = PlayerCommand::Select {
                    units: members,
                    replace: true,
                };
                if let Err(error) = input.commands.record(clock, command) {
                    input.last_error = Some(error.to_string());
                }
            },
        );
    }

    fn units_in_group(app: &mut App, name: &str) -> Vec<HandleId> {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let group = handle(&session, name);
        let objects = session.host().expect("published host").objects_mut();
        let ids: Vec<HandleId> = objects.executed_units().map(|(id, _)| id).collect();
        ids.into_iter()
            .filter(|id| {
                matches!(
                    objects.native("IsUnitInGroup", &[Value::Handle(*id), Value::Handle(group)]),
                    Ok(Value::Bool(true))
                )
            })
            .collect()
    }

    fn clear_gnoll_group(app: &mut App, runtime: &mut CampaignRuntime) {
        for _ in 0..180 {
            if triggers_retired(app, MORE_GNOLLS_DONE, MORE_GNOLLS_PROMPT) {
                return;
            }
            let living: Vec<HandleId> = units_in_group(app, GNOLL_GROUP)
                .into_iter()
                .filter(|unit| !unit_dead(app, *unit))
                .collect();
            if let Some(target) = living.first().copied() {
                order_selected(app, Kind::Attack, [0.0, 0.0], Some(target));
            }
            drive(app, runtime, 1);
        }
        panic!(
            "the gnoll group did not die through public attacks: {}",
            observe(app, runtime)
        );
    }

    fn triggers_down(
        app: &mut App,
        names: &[&str],
        seconds: u32,
        runtime: &mut CampaignRuntime,
    ) -> bool {
        for _ in 0..seconds {
            if names
                .iter()
                .all(|name| trigger_enabled(app, name).ok() == Some(false))
            {
                return true;
            }
            drive(app, runtime, 1);
        }
        names
            .iter()
            .all(|name| trigger_enabled(app, name).ok() == Some(false))
    }

    fn ability_level(app: &mut App, unit_name: &str, ability: [u8; 4]) -> i32 {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let unit = handle(&session, unit_name);
        let objects = session.host().expect("published host").objects_mut();
        match objects.native(
            "GetUnitAbilityLevel",
            &[Value::Handle(unit), Value::Int(i32::from_be_bytes(ability))],
        ) {
            Ok(Value::Int(level)) => level,
            other => panic!("{unit_name} ability level: {other:?}"),
        }
    }

    fn h3_transmission(app: &mut App) -> (String, String) {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let scripts = &session.host().expect("published host").context().scripts;
        let label = scripts
            .digests
            .iter()
            .find(|digest| digest.label.ends_with(":war3map.j"))
            .map(|digest| digest.label.as_str())
            .expect("admitted map script label");
        let source = scripts.source(label).expect("admitted map script");
        let line = source
            .lines()
            .find(|line| {
                line.contains("TransmissionFromUnitTypeWithNameBJ")
                    && line.contains("gg_snd_T01Narrator37")
            })
            .expect("H3 transmission call");
        let mut ids = Vec::new();
        let mut rest = line;
        while let Some(start) = rest.find("TRIGSTR_") {
            rest = &rest[start + "TRIGSTR_".len()..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let id = digits.parse::<u32>().expect("TRIGSTR id");
            ids.push(id);
            rest = &rest[digits.len()..];
        }
        let [speaker_id, text_id] = ids.as_slice() else {
            panic!(
                "H3 transmission needs speaker then text ids, found {}",
                ids.len()
            );
        };
        let speaker = scripts
            .table_string(*speaker_id)
            .expect("speaker string")
            .expect("speaker entry");
        let text = scripts
            .table_string(*text_id)
            .expect("text string")
            .expect("text entry");
        (speaker, text)
    }

    fn narration_armed(app: &mut App, runtime: &mut CampaignRuntime) -> bool {
        let (speaker, text) = h3_transmission(app);
        let began = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            session
                .host()
                .expect("published host")
                .objects()
                .timer_game_time()
                .unwrap_or(0.0)
        };
        for _ in 0..30 {
            let scene = {
                let mut session = app
                    .world_mut()
                    .resource_mut::<MapScriptSession<MapStartupHost>>();
                session
                    .host()
                    .expect("published host")
                    .objects()
                    .cinematic_scene()
                    .filter(|scene| scene.started >= began)
                    .map(|scene| (scene.speaker.clone(), scene.text.clone()))
            };
            if let Some((found_speaker, found_text)) = scene {
                assert!(
                    found_speaker == speaker && found_text == text,
                    "fresh scene did not match the H3 speaker and text string ids"
                );
                return trigger_enabled(app, SPEND_PROMPT).unwrap_or(false);
            }
            drive(app, runtime, 1);
        }
        false
    }

    fn learn_ready_skill(app: &mut App, ability: [u8; 4]) {
        let unit = {
            let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
            handle(session, THRALL)
        };
        app.world_mut().resource_scope(
            |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
                let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                let objects = session.host().expect("published host").objects_mut();
                let ready = objects
                    .skills_ready_to_learn(unit)
                    .expect("learnable query");
                assert!(
                    ready.contains(&ability),
                    "the armed skill is not ready to learn"
                );
                let clock = objects.timer_game_time();
                match objects.native(
                    "SelectHeroSkill",
                    &[Value::Handle(unit), Value::Int(i32::from_be_bytes(ability))],
                ) {
                    Ok(Value::Null) => {
                        let command = PlayerCommand::Learn {
                            unit,
                            ability: i32::from_be_bytes(ability),
                        };
                        if let Err(error) = input.commands.record(clock, command) {
                            input.last_error = Some(error.to_string());
                        }
                    }
                    other => panic!("SelectHeroSkill: {other:?}"),
                }
            },
        );
    }

    fn show_skill_menu(app: &mut App) {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        session
            .host()
            .expect("published host")
            .objects_mut()
            .fire_game_event("EVENT_GAME_SHOW_SKILL")
            .expect("show-skill event");
    }

    fn attack_the_gnoll(app: &mut App, runtime: &mut CampaignRuntime) {
        let gnoll = {
            let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
            handle(session, GNOLL)
        };
        assert!(
            trigger_enabled(app, GNOLL_DONE).unwrap_or(false),
            "the gnoll death trigger was not armed: {}",
            observe(app, runtime)
        );
        assert!(
            !unit_dead(app, gnoll),
            "the gnoll was already dead before the attack: {}",
            observe(app, runtime)
        );
        {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            session
                .host()
                .expect("published host")
                .objects_mut()
                .enable_combat_trace(64);
        }
        let attackers = selected_units(app);
        let approach = rect_bounds(app, GNOLL_APPROACH);
        order_selected(app, Kind::Move, center(approach), None);
        for _ in 0..120 {
            if triggers_retired(app, GNOLL_DONE, GNOLL_PROMPT) && !unit_dead(app, gnoll) {
                panic!(
                    "the gnoll lesson retired while the gnoll was alive, which is the bypass rect: {}",
                    observe(app, runtime)
                );
            }
            if inside(thrall_position(app), approach) || unit_dead(app, gnoll) {
                break;
            }
            drive(app, runtime, 1);
        }
        assert!(
            inside(thrall_position(app), approach) || unit_dead(app, gnoll),
            "Thrall did not reach the authored gnoll rect {}",
            observe(app, runtime)
        );
        if !unit_dead(app, gnoll) {
            assert!(
                trigger_enabled(app, GNOLL_DONE).unwrap_or(false),
                "the death trigger dropped before the gnoll died: {}",
                observe(app, runtime)
            );
            order_selected(app, Kind::Attack, center(approach), Some(gnoll));
            for _ in 0..180 {
                if triggers_retired(app, GNOLL_DONE, GNOLL_PROMPT) && !unit_dead(app, gnoll) {
                    panic!(
                        "the gnoll lesson retired while the gnoll was alive, which is the bypass rect: {}",
                        observe(app, runtime)
                    );
                }
                if unit_dead(app, gnoll) {
                    break;
                }
                drive(app, runtime, 1);
            }
        }
        assert!(
            unit_dead(app, gnoll),
            "the public attack left the gnoll alive: {}",
            observe(app, runtime)
        );
        assert!(
            attacker_damaged_gnoll(app, gnoll, &attackers),
            "no admitted combat trace shows a selected attacker damaging the gnoll: {}",
            observe(app, runtime)
        );
        retire_within(
            app,
            runtime,
            GNOLL_DONE,
            GNOLL_PROMPT,
            30,
            "the gnoll died but the map did not retire the death lesson",
        );
    }

    fn unit_dead(app: &mut App, unit: HandleId) -> bool {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let Ok(record) = session
            .host()
            .expect("published host")
            .objects()
            .executed_unit(unit)
        else {
            return false;
        };
        record.combat.as_ref().is_some_and(|combat| combat.dead)
            || record
                .vitals
                .as_ref()
                .is_some_and(|vitals| vitals.life <= 0.0)
    }

    fn selected_units(app: &mut App) -> Vec<HandleId> {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        session
            .host()
            .expect("published host")
            .objects()
            .selection()
            .to_vec()
    }

    fn attacker_damaged_gnoll(app: &mut App, gnoll: HandleId, attackers: &[HandleId]) -> bool {
        use pcx_play::playercolor_host::CombatTraceEvent;
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let Some(trace) = session
            .host()
            .expect("published host")
            .objects()
            .combat_trace()
        else {
            return false;
        };
        trace.events().iter().any(|event| {
            matches!(
                event,
                CombatTraceEvent::Damage {
                    source,
                    target,
                    damage,
                    ..
                } if target.unit == gnoll
                    && *damage > 0.0
                    && attackers.contains(&source.unit)
            )
        })
    }

    fn order_selected(app: &mut App, kind: Kind, point: [f32; 2], target: Option<HandleId>) {
        app.world_mut().resource_scope(
            |world, mut input: Mut<crate::campaign_input::CampaignInput>| {
                let mut session = world.resource_mut::<MapScriptSession<MapStartupHost>>();
                let objects = session.host().expect("published host").objects_mut();
                crate::campaign_input::standard_target(objects, &mut input, kind, point, target)
                    .unwrap_or_else(|error| panic!("public {kind:?} order refused: {error}"));
            },
        );
    }

    fn assert_quest_open(app: &mut App, runtime: &CampaignRuntime, done: &str) {
        let (discovered, completed) = quest_progress(app);
        assert!(
            discovered && !completed,
            "{done} must leave the quest discovered and unfinished: {}",
            observe(app, runtime)
        );
    }

    fn fog_started_at(app: &mut App, point: [f32; 2]) -> Option<bool> {
        use pcx_play::playercolor_host::VisionShape;
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let host = session.host()?;
        let mut started = None;
        for modifier in host.objects().fog_modifiers() {
            if let VisionShape::Circle { center, .. } = modifier.shape
                && center == point
            {
                started = Some(started.unwrap_or(false) || modifier.started);
            }
        }
        started
    }

    fn observe(app: &mut App, runtime: &CampaignRuntime) -> String {
        let position = thrall_position(app);
        let shown = presentation(app);
        let (clock, paused, quests) = {
            let mut session = app
                .world_mut()
                .resource_mut::<MapScriptSession<MapStartupHost>>();
            let Some(host) = session.host() else {
                return format!("tick={} no host", runtime.tick());
            };
            let objects = host.objects();
            let quests = objects
                .quest_session()
                .map(|quests| match pcx_play::quest_view::project(quests) {
                    Ok(board) => board
                        .quests
                        .iter()
                        .map(|quest| {
                            format!(
                                "title={:?} discovered={} completed={}",
                                quest.title, quest.discovered, quest.completed
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" | "),
                    Err(error) => format!("quest projection {error:?}"),
                })
                .unwrap_or_else(|| "no quest session".into());
            (objects.timer_game_time(), objects.game_paused(), quests)
        };
        let move_done = trigger_enabled(app, MOVE_DONE);
        let move_prompt = trigger_enabled(app, MOVE_PROMPT);
        let scroll_done = trigger_enabled(app, SCROLL_DONE);
        let scroll_prompt = trigger_enabled(app, SCROLL_PROMPT);
        let next_fog = rect_center(app, NEXT_BANNER);
        let third_fog = rect_center(app, THIRD_BANNER);
        let next_lesson = fog_started_at(app, next_fog);
        let third_lesson = fog_started_at(app, third_fog);
        let input = app
            .world()
            .resource::<crate::campaign_input::CampaignInput>();
        format!(
            "tick={} clock={clock:?} paused={paused:?} control={:?} interface={:?} thrall={position:?} quests={quests} move_done={move_done:?} move_prompt={move_prompt:?} next_banner_fog={next_lesson:?} scroll_done={scroll_done:?} scroll_prompt={scroll_prompt:?} third_banner_fog={third_lesson:?} orders={} last_error={:?}",
            runtime.tick(),
            shown.control_enabled,
            shown.interface_visible,
            input.commands.entries().len(),
            input.last_error
        )
    }
}
