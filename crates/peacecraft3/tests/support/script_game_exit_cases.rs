//! Executed `EndGame` through an ordinary dialog button, delivered by the published runtime.
use super::*;
use peacecraft3::campaign_runtime::{
    self, CampaignExitResult, CampaignExitSnapshot, CampaignRuntime,
};
use peacecraft3::campaign_score::{ActionOutcome, ScoreScreen, ScoreScreenState, press};
use peacecraft3::campaign_score_assets::ScoreScreenAssets;
use std::collections::BTreeMap;
use std::time::Duration;

const QUIT_COMMON: &str = "\
type dialog extends agent
type button extends agent
native EndGame takes boolean doScoreScreen returns nothing
native ChangeLevel takes string newLevel, boolean doScoreScreen returns nothing
constant native GetLocalPlayer takes nothing returns player
native DialogCreate takes nothing returns dialog
native DialogAddButton takes dialog d, string text, integer key returns button
native DialogDisplay takes player p, dialog d, boolean visible returns nothing
native TriggerRegisterDialogButtonEvent takes trigger t, button b returns event
type dialogevent extends eventid
constant native ConvertDialogEvent takes integer n returns dialogevent
type gamedifficulty extends handle
constant native ConvertGameDifficulty takes integer i returns gamedifficulty
native SetGameDifficulty takes gamedifficulty whichdifficulty returns nothing
constant native GetGameDifficulty takes nothing returns gamedifficulty
native GetDefaultDifficulty takes nothing returns gamedifficulty
native SetDefaultDifficulty takes gamedifficulty g returns nothing
globals
constant dialogevent EVENT_DIALOG_BUTTON_CLICK = ConvertDialogEvent(90)
constant gamedifficulty MAP_DIFFICULTY_NORMAL = ConvertGameDifficulty(1)
constant gamedifficulty MAP_DIFFICULTY_HARD = ConvertGameDifficulty(2)
endglobals
";

/// Synthetic no-profile fallback, deliberately not the current `MAP_DIFFICULTY_NORMAL`
/// input, so the Quit can only take the default when it is the value the native returns.
const SYNTHETIC_DEFAULT_FALLBACK: i32 = 2;

/// Mirrors `CustomDefeatQuitBJ`. The defeat reset sets the current difficulty to the default,
/// so `EndGame` reports the score only when the default, not the current input, was applied.
pub(crate) const QUIT_SCRIPT: &str = "\
globals
player p = Player(0)
dialog d = DialogCreate()
button b = DialogAddButton(d, \"Quit fixture\", 81)
trigger listener = CreateTrigger()
endglobals
function quitAction takes nothing returns nothing
call PauseGame(false)
call SetGameDifficulty(GetDefaultDifficulty())
if GetGameDifficulty() == MAP_DIFFICULTY_HARD then
call EndGame(true)
else
call EndGame(false)
endif
endfunction
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerRegisterDialogButtonEvent(listener, b)
call TriggerAddAction(listener, function quitAction)
call PauseGame(true)
call DialogDisplay(p, d, true)
endfunction
";

/// Idle replacement: same declarations, no exit request.
const IDLE_SCRIPT: &str = "\
globals
player p = Player(0)
dialog d = DialogCreate()
button b = DialogAddButton(d, \"Idle fixture\", 81)
trigger listener = CreateTrigger()
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
call TriggerRegisterDialogButtonEvent(listener, b)
endfunction
";

fn quit_app(script: &str, configure: impl FnOnce(&mut MapStartupInputs)) -> App {
    let mut app = timer_app();
    app.init_resource::<Time<Real>>();
    campaign_runtime::install(
        &mut app,
        CampaignRuntime::new(Duration::from_millis(125), 8, 8),
    );
    publish(&mut app, script, 1, configure);
    app
}

fn publish(
    app: &mut App,
    script: &str,
    revision: u64,
    configure: impl FnOnce(&mut MapStartupInputs),
) {
    publish_with_assets(app, script, revision, BTreeMap::new(), configure);
}

pub(crate) fn publish_with_assets(
    app: &mut App,
    script: &str,
    revision: u64,
    assets: BTreeMap<String, crate::binding::Reply>,
    configure: impl FnOnce(&mut MapStartupInputs),
) {
    let common = format!(
        "{}\n{QUIT_COMMON}",
        super::super::timed_trigger_cases::COMMON
    );
    deliver_timer_with_assets(app, script, revision, &common, assets, |inputs| {
        inputs.post_config.local_player = Some(0);
        inputs.post_config.difficulty_global = Some("MAP_DIFFICULTY_NORMAL".into());
        inputs.post_config.default_difficulty = Some(
            pcx_play::map_startup_host::DefaultDifficultyObservation::NoProfile {
                native_fallback: SYNTHETIC_DEFAULT_FALLBACK,
            },
        );
        inputs.enumeration.names.extend([
            "ConvertGameEvent".into(),
            "ConvertDialogEvent".into(),
            "ConvertGameDifficulty".into(),
        ]);
        inputs.enumeration.limits.max_handles = 256;
        inputs.trigger_runtime = Some(pcx_play::playercolor_host::TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "TEST_TIMER_EVENT".into(),
            max_timed_events: 8,
            max_parked_threads: 8,
        });
        inputs.budgets.dialogs = Some(pcx_play::playercolor_host::DialogBudget {
            max_dialogs: 2,
            max_buttons: 4,
            text_bytes: 256,
            max_events: 4,
            max_quit_requests: 2,
        });
        configure(inputs);
    });
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "{:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
}

pub(crate) fn session_present(app: &mut App) -> bool {
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .is_some()
}

/// Returns the queued callback count and the paused state observed before delivery.
pub(crate) fn click_quit(app: &mut App) -> (usize, Option<bool>) {
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let Value::Handle(player) = session.global("p").unwrap() else {
        panic!("fixture player")
    };
    let Value::Handle(button) = session.global("b").unwrap() else {
        panic!("fixture button")
    };
    let host = session.host().unwrap();
    let paused = host.objects().game_paused();
    let queued = host
        .objects_mut()
        .click_dialog_button(player, button)
        .unwrap();
    (queued, paused)
}

#[test]
pub(super) fn normal_quit_button_in_paused_state_ends_the_session_in_the_delivering_update() {
    let mut app = quit_app(QUIT_SCRIPT, |_| {});
    app.init_resource::<ObservedExit>().add_systems(
        Update,
        observe_exit.after(campaign_runtime::CampaignExitSet),
    );
    app.update();
    let clock_before_click = simulation_clock(&mut app);
    let (queued, paused) = click_quit(&mut app);
    assert_eq!(queued, 1);
    assert_eq!(paused, Some(true), "the paused delivery path is under test");
    assert!(
        session_present(&mut app),
        "a queued click has not run its callback yet"
    );
    app.update();
    assert!(
        !session_present(&mut app),
        "the consumer must observe the request in the update that delivers the callback"
    );
    assert!(
        app.world()
            .resource::<CampaignRuntime>()
            .last_error
            .is_none()
    );
    // Taken by the observer, which is ordered after the exit set in the delivering update.
    let snapshot = app
        .world_mut()
        .resource_mut::<ObservedExit>()
        .0
        .take()
        .expect("the score consumer sees the exit snapshot in the delivering update");
    assert!(snapshot.score_screen, "QUIT_SCRIPT ends with EndGame(true)");
    assert_eq!(snapshot.game_time, clock_before_click);
    assert_eq!(
        snapshot.local_result, None,
        "no removal record is admitted, so the result is unknown"
    );
    assert!(
        take_exit_snapshot(&mut app).is_none(),
        "the handoff slot is consumed once"
    );
}

/// Stands in for the score consumer: takes the handoff after `CampaignExitSet`.
#[derive(Resource, Default)]
struct ObservedExit(Option<CampaignExitSnapshot>);

fn observe_exit(mut exit: ResMut<CampaignExitResult>, mut observed: ResMut<ObservedExit>) {
    observed.0 = exit.take();
}

fn simulation_clock(app: &mut App) -> Option<f64> {
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .host()
        .and_then(|host| host.objects().timer_game_time())
}

fn take_exit_snapshot(app: &mut App) -> Option<CampaignExitSnapshot> {
    app.world_mut().resource_mut::<CampaignExitResult>().take()
}

fn transition_app(show_scores: bool, next: Option<Vec<u8>>) -> App {
    let mut assets = selected_score_ui();
    if let Some(next) = next {
        assets.insert(
            "next chapter.w3m".into(),
            crate::binding::Reply::Bytes(next),
        );
    }
    let script = QUIT_SCRIPT.replace(
        "call EndGame(true)",
        &format!("call ChangeLevel(\"next chapter.w3m\", {show_scores})"),
    );
    transition_script_app(&script, assets)
}

fn transition_script_app(script: &str, assets: BTreeMap<String, crate::binding::Reply>) -> App {
    let mut app = timer_app();
    app.init_resource::<Time<Real>>();
    campaign_runtime::install(
        &mut app,
        CampaignRuntime::new(Duration::from_millis(125), 8, 8),
    );
    peacecraft3::campaign_score::install(&mut app);
    peacecraft3::campaign_score_assets::install(&mut app);
    publish_with_assets(&mut app, script, 1, assets, |_| {});
    app.update();
    click_quit(&mut app);
    app.update();
    app
}

#[test]
fn conflicting_change_level_requests_stop_without_choosing_a_destination() {
    let script = QUIT_SCRIPT.replace(
        "call EndGame(true)",
        "call ChangeLevel(\"first.w3m\", true)\ncall ChangeLevel(\"second.w3m\", true)",
    );
    let mut app = transition_script_app(&script, selected_score_ui());
    assert!(session_present(&mut app));
    assert!(matches!(
        app.world().resource::<CampaignRuntime>().last_error,
        Some(campaign_runtime::CampaignRuntimeError::ConflictingDestinations)
    ));
    assert!(app.world().resource::<ScoreScreenState>().0.is_none());
    assert!(take_exit_snapshot(&mut app).is_none());
    assert_eq!(
        app.world()
            .resource::<MapRequests>()
            .bindings
            .as_ref()
            .unwrap()
            .revision,
        1
    );
}

fn next_chapter() -> Vec<u8> {
    fixture("globals\ninteger chapter_marker=2\nendglobals\nfunction config takes nothing returns nothing\nendfunction\nfunction main takes nothing returns nothing\nendfunction\n").raw
}

fn continue_score(app: &mut App) -> campaign_runtime::CampaignTransitionRequest {
    let request = {
        let mut score = app.world_mut().resource_mut::<ScoreScreenState>();
        assert_eq!(
            press(&mut score, "ScoreScreenOkButton"),
            Some(ActionOutcome::Continue)
        );
        assert!(
            score.0.is_some(),
            "a press must not dismiss before admission"
        );
        score.continuation().unwrap()
    };
    app.world_mut().write_message(request.clone());
    app.update();
    request
}

fn supply_next_chapter(app: &mut App) {
    let bindings = app
        .world()
        .resource::<MapRequests>()
        .bindings
        .as_ref()
        .unwrap();
    let mut inputs = host_inputs(Arc::clone(&bindings.catalog.0));
    inputs.timer_time = Some(10.0);
    inputs.enumeration.names.extend([
        "ConvertGameEvent".into(),
        "ConvertDialogEvent".into(),
        "ConvertGameDifficulty".into(),
    ]);
    let revision = bindings.revision;
    app.insert_resource(CampaignHostSupply {
        revision,
        inputs: Some(inputs),
    });
}

#[test]
fn campaign_continuation_publishes_the_selected_next_map_and_runs_its_script() {
    let mut app = transition_app(true, Some(next_chapter()));
    assert!(!session_present(&mut app));
    let request = continue_score(&mut app);
    assert!(app.world().resource::<ScoreScreenState>().0.is_some());
    assert!(
        app.world()
            .resource::<campaign_runtime::CampaignTransition>()
            .last_error
            .is_none()
    );
    let requests = app.world().resource::<MapRequests>();
    let context = requests.admission.shared_context().unwrap();
    assert!(!Arc::ptr_eq(&context, &request.context));
    let bindings = requests.bindings.as_ref().unwrap();
    assert_eq!(bindings.receipts.map.path.as_str(), "next chapter.w3m");
    supply_next_chapter(&mut app);
    app.update();
    assert!(app.world().resource::<ScoreScreenState>().0.is_none());
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none(),
        "next startup: {:?}",
        app.world().resource::<CampaignStartup>().last_error
    );
    assert_eq!(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .global("chapter_marker"),
        Some(Value::Int(2))
    );
}

#[test]
fn campaign_continuation_without_scores_does_not_wait_for_a_score_press() {
    let app = transition_app(false, Some(next_chapter()));
    assert!(app.world().resource::<ScoreScreenState>().0.is_none());
    assert!(
        app.world()
            .resource::<campaign_runtime::CampaignTransition>()
            .last_error
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<MapRequests>()
            .bindings
            .as_ref()
            .unwrap()
            .revision,
        2
    );
}

#[test]
fn campaign_continuation_refusal_keeps_the_score_binding_and_selected_reader() {
    for next in [None, Some(b"malformed map".to_vec())] {
        let mut app = transition_app(true, next);
        let old = app
            .world()
            .resource::<MapRequests>()
            .published_receipts
            .clone()
            .unwrap();
        let reader = app
            .world()
            .resource::<SelectedMapSources>()
            .for_receipts(&old)
            .unwrap();
        let request = continue_score(&mut app);
        let requests = app.world().resource::<MapRequests>();
        assert_eq!(requests.bindings.as_ref().unwrap().revision, 1);
        assert!(Arc::ptr_eq(
            &requests.admission.shared_context().unwrap(),
            &request.context
        ));
        assert!(Arc::ptr_eq(
            requests.published_receipts.as_ref().unwrap(),
            &old
        ));
        assert!(Arc::ptr_eq(
            &app.world()
                .resource::<SelectedMapSources>()
                .for_receipts(&old)
                .unwrap(),
            &reader
        ));
        assert!(app.world().resource::<ScoreScreenState>().0.is_some());
        assert!(
            app.world()
                .resource::<campaign_runtime::CampaignTransition>()
                .last_error
                .is_some()
        );
    }
}

#[test]
fn campaign_continuation_rejects_a_second_request_from_the_old_context() {
    let mut app = transition_app(true, Some(next_chapter()));
    let request = continue_score(&mut app);
    supply_next_chapter(&mut app);
    app.update();
    assert!(app.world().resource::<ScoreScreenState>().0.is_none());
    let current = app
        .world()
        .resource::<MapRequests>()
        .published_receipts
        .clone()
        .unwrap();
    app.world_mut().write_message(request);
    app.update();
    assert!(matches!(
        app.world()
            .resource::<campaign_runtime::CampaignTransition>()
            .last_error,
        Some(campaign_runtime::CampaignTransitionError::StaleSession)
    ));
    assert!(Arc::ptr_eq(
        app.world()
            .resource::<MapRequests>()
            .published_receipts
            .as_ref()
            .unwrap(),
        &current
    ));
    assert_eq!(
        app.world()
            .resource::<MapRequests>()
            .bindings
            .as_ref()
            .unwrap()
            .revision,
        2
    );
}

#[test]
fn campaign_continuation_keeps_the_score_until_failed_startup_can_be_retried() {
    let mut app = transition_app(true, Some(next_chapter()));
    let request = continue_score(&mut app);
    supply_next_chapter(&mut app);
    app.world_mut()
        .resource_mut::<CampaignHostSupply>()
        .inputs
        .as_mut()
        .unwrap()
        .enumeration
        .names
        .clear();
    app.update();
    assert!(matches!(
        app.world().resource::<CampaignStartup>().last_error,
        Some(StartupError::Session(_))
    ));
    assert!(app.world().resource::<ScoreScreenState>().0.is_some());
    assert!(!session_present(&mut app));
    let retry = continue_score(&mut app);
    assert!(Arc::ptr_eq(&retry.context, &request.context));
    assert_eq!(
        app.world()
            .resource::<MapRequests>()
            .bindings
            .as_ref()
            .unwrap()
            .revision,
        3
    );
    assert!(app.world().resource::<ScoreScreenState>().0.is_some());
    supply_next_chapter(&mut app);
    app.update();
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert!(app.world().resource::<ScoreScreenState>().0.is_none());
    assert_eq!(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .global("chapter_marker"),
        Some(Value::Int(2))
    );
}

#[test]
fn campaign_continuation_retry_screen_retires_when_another_map_starts() {
    let mut app = transition_app(true, Some(next_chapter()));
    continue_score(&mut app);
    assert!(app.world().resource::<ScoreScreenState>().0.is_some());
    publish_with_assets(&mut app, IDLE_SCRIPT, 3, BTreeMap::new(), |_| {});
    app.update();
    assert!(session_present(&mut app));
    assert!(app.world().resource::<ScoreScreenState>().0.is_none());
}

#[test]
pub(super) fn second_end_game_past_the_queue_bound_fails_the_callback_and_keeps_the_first_request()
{
    let mut app = quit_app(
        QUIT_SCRIPT
            .replace(
                "call EndGame(true)\n",
                "call EndGame(true)\ncall EndGame(false)\n",
            )
            .as_str(),
        |inputs| {
            inputs.budgets.game_exit = 1;
        },
    );
    app.update();
    click_quit(&mut app);
    app.update();
    let runtime = app.world().resource::<CampaignRuntime>();
    assert!(
        runtime.last_error.is_some(),
        "the bounded queue must refuse the second request"
    );
    assert!(
        !session_present(&mut app),
        "the first executed request ends the session even though its callback failed"
    );
}

#[test]
pub(super) fn replacement_session_starts_without_inherited_exit_and_keeps_running() {
    let mut app = quit_app(QUIT_SCRIPT, |_| {});
    app.update();
    click_quit(&mut app);
    app.update();
    assert!(!session_present(&mut app));
    assert!(
        take_exit_snapshot(&mut app).is_some(),
        "the first exit's snapshot is pending until taken"
    );
    publish(&mut app, IDLE_SCRIPT, 2, |_| {});
    assert!(
        take_exit_snapshot(&mut app).is_none(),
        "publishing a replacement must not create an exit snapshot"
    );
    let inherited = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        session
            .host()
            .map(|host| host.objects().game_exit_requests().to_vec())
    };
    assert_eq!(inherited, Some(Vec::new()));
    app.update();
    app.update();
    assert!(
        session_present(&mut app),
        "a replacement without EndGame must not be ended by the earlier request"
    );
    assert!(
        app.world()
            .resource::<CampaignRuntime>()
            .last_error
            .is_none()
    );
}

#[test]
pub(super) fn ended_session_stays_ended_without_runtime_errors_across_later_updates() {
    let mut app = quit_app(QUIT_SCRIPT, |_| {});
    app.update();
    click_quit(&mut app);
    app.update();
    assert!(!session_present(&mut app));
    app.update();
    app.update();
    assert!(
        app.world()
            .resource::<CampaignRuntime>()
            .last_error
            .is_none()
    );
    assert!(!session_present(&mut app));
}

/// Synthetic selected score UI: one authored label, resolved through `GlobalStrings`.
const SCORE_SCREEN_FDF: &str = "\
Frame \"FRAME\" \"ScoreScreenFrame\" {
\tWidth 0.8,
\tHeight 0.6,
}
Frame\"TEXT\" \"ScoreScreenOkText\" {
\tWidth 0.2,
\tHeight 0.03,
\tText \"OK_WHITE\",
}
";
const SCORE_STRINGS_FDF: &str = "OK_WHITE \"|cffffffffO|rK\",\n";

pub(crate) fn score_app(assets: BTreeMap<String, crate::binding::Reply>) -> App {
    let mut app = timer_app();
    app.init_resource::<Time<Real>>();
    campaign_runtime::install(
        &mut app,
        CampaignRuntime::new(Duration::from_millis(125), 8, 8),
    );
    peacecraft3::campaign_score::install(&mut app);
    peacecraft3::campaign_score_assets::install(&mut app);
    publish_with_assets(&mut app, QUIT_SCRIPT, 1, assets, |_| {});
    app
}

pub(crate) fn selected_score_ui() -> BTreeMap<String, crate::binding::Reply> {
    BTreeMap::from([
        (
            r"UI\FrameDef\Glue\ScoreScreen.fdf".to_owned(),
            crate::binding::Reply::Bytes(SCORE_SCREEN_FDF.as_bytes().to_vec()),
        ),
        (
            r"UI\FrameDef\GlobalStrings.fdf".to_owned(),
            crate::binding::Reply::Bytes(SCORE_STRINGS_FDF.as_bytes().to_vec()),
        ),
    ])
}

/// The exit, the request and the asset load all happen in the update that delivers the quit
/// callback, and the request keeps the exiting session's context.
#[test]
pub(super) fn same_update_exit_hands_its_context_to_the_selected_score_ui() {
    let mut app = score_app(selected_score_ui());
    app.update();
    let admitted = Arc::clone(
        app.world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>()
            .context()
            .unwrap(),
    );
    click_quit(&mut app);
    app.update();
    assert!(!session_present(&mut app), "the exit ended the session");
    let state = app.world().resource::<ScoreScreenState>();
    let Some(ScoreScreen::Refused { context, .. }) = &state.0 else {
        panic!("the score request is recorded in the delivering update");
    };
    assert!(
        Arc::ptr_eq(context, &admitted),
        "the request keeps the exiting session's context"
    );
    let assets = app.world().resource::<ScoreScreenAssets>();
    assert!(
        assets.attempted_for(&admitted),
        "the selected UI was attempted for that context"
    );
    assert!(
        assets.source_for(&admitted).is_some(),
        "the selected resolver that read the UI is kept with it"
    );
    let layout = assets
        .layout_for(&admitted)
        .expect("selected UI resolves for the exiting session");
    assert_eq!(layout.label("ScoreScreenOkText").unwrap(), "|cffffffffO|rK");
}

/// A selected score UI missing from the bindings' resolver refuses for this context; nothing
/// from an earlier layout is kept.
#[test]
pub(super) fn missing_selected_score_ui_refuses_without_a_layout_for_the_exiting_session() {
    let mut app = score_app(BTreeMap::new());
    app.update();
    click_quit(&mut app);
    app.update();
    let state = app.world().resource::<ScoreScreenState>();
    assert!(matches!(state.0, Some(ScoreScreen::Refused { .. })));
    let assets = app.world().resource::<ScoreScreenAssets>();
    let Some(ScoreScreen::Refused { context, .. }) = &state.0 else {
        panic!("the refused request is recorded");
    };
    assert!(
        assets.layout_for(context).is_none(),
        "no layout is usable for the refused session"
    );
    assert!(
        assets.attempted_for(context),
        "the refused attempt is cached, not retried"
    );
}

/// Score buttons act on the pending request: Save Replay is refused and keeps it, OK dismisses
/// it, and a later press with no request does nothing.
#[test]
pub(super) fn score_buttons_refuse_replay_and_dismiss_through_the_pending_request() {
    let mut app = score_app(selected_score_ui());
    app.update();
    click_quit(&mut app);
    app.update();
    let mut state = app.world_mut().resource_mut::<ScoreScreenState>();
    assert_eq!(
        press(&mut state, "ScoreScreenSaveReplayButton"),
        Some(ActionOutcome::ReplayUnsupported)
    );
    assert!(state.0.is_some(), "a refused replay keeps the request");
    assert_eq!(
        press(&mut state, "ScoreScreenOkButton"),
        Some(ActionOutcome::Dismiss)
    );
    assert!(state.0.is_none(), "OK dismisses the request");
    assert_eq!(press(&mut state, "ScoreScreenOkButton"), None);
    assert_eq!(press(&mut state, "ScoreScreenTabButton0"), None);
}
