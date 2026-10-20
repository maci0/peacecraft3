use super::{COMMON, budget as player_budget, context, id, player, slots_rule};
use crate::playercolor_host::{
    DialogBudget, EnumHost, Limits, MapConfigBudget, ObjectHost, TriggerBudget, TriggerRuntimeInput,
};
use pcx_script::{Host, Value};

const EMPTY_MAP: &str = "function fixture_unused takes nothing returns nothing\nendfunction\n";
const DECLARATIONS: &str = r#"
constant native GetLocalPlayer takes nothing returns player
type dialog extends agent
type button extends agent
type widget extends agent
type unit extends widget
type trigger extends agent
type event extends agent
type eventid extends handle
type unitevent extends eventid
type dialogevent extends eventid
type gameevent extends eventid
type playerevent extends eventid
constant native ConvertUnitEvent takes integer n returns unitevent
constant native ConvertDialogEvent takes integer n returns dialogevent
constant native ConvertGameEvent takes integer n returns gameevent
constant native ConvertPlayerEvent takes integer n returns playerevent
native TriggerRegisterPlayerEvent takes trigger t, player p, playerevent e returns event
native DialogCreate takes nothing returns dialog
native DialogDestroy takes dialog d returns nothing
native DialogClear takes dialog d returns nothing
native DialogSetMessage takes dialog d, string text returns nothing
native DialogAddButton takes dialog d, string text, integer key returns button
native DialogAddQuitButton takes dialog d, boolean score, string text, integer key returns button
native DialogDisplay takes player p, dialog d, boolean flag returns nothing
native TriggerRegisterDialogEvent takes trigger t, dialog d returns event
native TriggerRegisterDialogButtonEvent takes trigger t, button b returns event
constant native GetClickedDialog takes nothing returns dialog
constant native GetClickedButton takes nothing returns button
constant native GetTriggerPlayer takes nothing returns player
native CreateTrigger takes nothing returns trigger
native DestroyTrigger takes trigger t returns nothing
native TriggerRegisterUnitEvent takes trigger t, unit u, unitevent e returns event
native TriggerRegisterTimerEvent takes trigger t, real timeout, boolean periodic returns event
constant native GetTriggeringTrigger takes nothing returns trigger
constant native GetTriggerEventId takes nothing returns eventid
native TimerStart takes timer t, real timeout, boolean periodic, code callback returns nothing
native TimerGetElapsed takes timer t returns real
native TimerGetRemaining takes timer t returns real
native TimerGetTimeout takes timer t returns real
native GetExpiredTimer takes nothing returns timer
globals
constant unitevent EXAMPLE_UNIT = ConvertUnitEvent(701)
constant dialogevent EVENT_DIALOG_CLICK = ConvertDialogEvent(721)
constant dialogevent EVENT_DIALOG_BUTTON_CLICK = ConvertDialogEvent(722)
constant gameevent EXAMPLE_TIMER = ConvertGameEvent(723)
constant playerevent EXAMPLE_PLAYER_EVENT = ConvertPlayerEvent(724)
endglobals
"#;
fn budget() -> DialogBudget {
    DialogBudget {
        max_dialogs: 3,
        max_buttons: 4,
        text_bytes: 32,
        max_events: 4,
        max_quit_requests: 1,
    }
}
fn try_host_with_map(
    declarations: &str,
    dialog_budget: DialogBudget,
    map: &str,
) -> Result<ObjectHost, crate::playercolor_host::Error> {
    let map = if map.is_empty() { EMPTY_MAP } else { map };
    let ctx = context(
        301,
        &format!("{COMMON}{declarations}"),
        map,
        "synthetic:script-dialog",
    );
    let enums = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        &ctx.map_digest.label,
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
            "ConvertDialogEvent",
            "ConvertGameEvent",
            "ConvertPlayerEvent",
        ],
        Limits {
            max_handles: 80,
            admission_steps: 4096,
            expression_depth: 32,
            expression_work: 4096,
        },
    )
    .unwrap();
    let mut host = ObjectHost::checked(enums)
        .unwrap()
        .with_map_config(MapConfigBudget {
            text_bytes: 128,
            start_locations: 2,
        })
        .unwrap()
        .with_players(ctx.clone(), slots_rule(301, -3, 9), player_budget())
        .unwrap()
        .with_timer_runtime(0.)
        .unwrap()
        .with_trigger_admission(TriggerBudget {
            max_triggers: 4,
            max_unit_events: 0,
            max_actions: 0,
        })
        .unwrap()
        .with_trigger_runtime(TriggerRuntimeInput {
            initial_enabled: true,
            timer_event_global: "EXAMPLE_TIMER".into(),
            max_timed_events: 0,
            max_parked_threads: 0,
        })
        .unwrap();
    host.admit_local_player(&ctx, 2).unwrap();
    host.with_dialogs(dialog_budget)
}
fn host_with(declarations: &str, dialog_budget: DialogBudget) -> ObjectHost {
    try_host_with_map(declarations, dialog_budget, "").unwrap()
}
fn host() -> ObjectHost {
    host_with(DECLARATIONS, budget())
}
fn create(h: &mut ObjectHost) -> Value {
    h.native("DialogCreate", &[]).unwrap()
}
fn button(h: &mut ObjectHost, dialog: &Value, label: &str) -> Value {
    h.native(
        "DialogAddButton",
        &[dialog.clone(), Value::Str(label.into()), Value::Int(77)],
    )
    .unwrap()
}
fn display(h: &mut ObjectHost, dialog: &Value, number: i32, show: bool) {
    let p = player(h, number);
    h.native("DialogDisplay", &[p, dialog.clone(), Value::Bool(show)])
        .unwrap();
}
#[test]
fn executed_dialog_lifecycle_shares_identity_and_releases_text_and_button_budgets() {
    let mut h = host();
    let d = create(&mut h);
    let b = button(&mut h, &d, "choice");
    assert_ne!(d, b);
    h.native(
        "DialogSetMessage",
        &[d.clone(), Value::Str("caption".into())],
    )
    .unwrap();
    let snapshot = h.dialog_state().unwrap().get(id(&d)).unwrap().clone();
    assert_eq!(snapshot.message.as_deref(), Some("caption"));
    assert_eq!(snapshot.buttons[0].text, "choice");
    assert_eq!(snapshot.buttons[0].hotkey, 77);
    assert!(
        h.native("DialogSetMessage", &[b.clone(), Value::Str("wrong".into())])
            .is_err()
    );
    h.native("DialogClear", std::slice::from_ref(&d)).unwrap();
    assert!(
        h.dialog_state()
            .unwrap()
            .get(id(&d))
            .unwrap()
            .buttons
            .is_empty()
    );
    assert_eq!(h.dialog_state().unwrap().get(id(&d)).unwrap().message, None);
    assert!(h.inner().kind(id(&b)).is_err());
    let fresh = button(&mut h, &d, "new");
    assert_ne!(fresh, b);
    h.native("DialogDestroy", std::slice::from_ref(&d)).unwrap();
    assert!(h.inner().kind(id(&d)).is_err());
    assert!(h.inner().kind(id(&fresh)).is_err());
    assert!(h.native("DialogClear", &[d]).is_err());
    assert_eq!(h.dialog_state().unwrap().dialogs().count(), 0);
}
#[test]
fn dialog_display_filters_other_players_and_click_context_routes_both_registration_kinds() {
    let mut h = host();
    let d = create(&mut h);
    let b = button(&mut h, &d, "yes");
    let a = h.native("CreateTrigger", &[]).unwrap();
    let c = h.native("CreateTrigger", &[]).unwrap();
    h.native("TriggerRegisterDialogEvent", &[a.clone(), d.clone()])
        .unwrap();
    h.native("TriggerRegisterDialogButtonEvent", &[c.clone(), b.clone()])
        .unwrap();
    let before = h.dialog_state().unwrap().revision();
    display(&mut h, &d, 1, true);
    assert_eq!(h.dialog_state().unwrap().revision(), before);
    assert!(!h.dialog_state().unwrap().get(id(&d)).unwrap().visible_to(1));
    let local = player(&mut h, 2);
    assert!(h.click_dialog_button(id(&local), id(&b)).is_err());
    display(&mut h, &d, 2, true);
    assert_eq!(h.click_dialog_button(id(&local), id(&b)).unwrap(), 2);
    assert!(!h.dialog_state().unwrap().get(id(&d)).unwrap().visible_to(2));
    for (trigger, enumerand) in [(a, 721), (c, 722)] {
        let fired = h.take_fired_event().unwrap();
        assert_eq!(fired.trigger, id(&trigger));
        assert!(h.begin_fired_event(&fired).unwrap().is_some());
        assert_eq!(h.native("GetClickedDialog", &[]).unwrap(), d);
        assert_eq!(h.native("GetClickedButton", &[]).unwrap(), b);
        assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), local);
        let event = h.native("GetTriggerEventId", &[]).unwrap();
        assert_eq!(
            h.inner().symbolic_value(id(&event), "dialogevent").unwrap(),
            enumerand
        );
        h.finish_fired_event();
    }
    assert_eq!(h.native("GetClickedDialog", &[]).unwrap(), Value::Null);
    assert_eq!(h.native("GetClickedButton", &[]).unwrap(), Value::Null);
    assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), Value::Null);
}
#[test]
fn dialog_budget_refusals_preserve_state_and_quit_requests_retain_executed_flag() {
    let mut h = host_with(
        DECLARATIONS,
        DialogBudget {
            text_bytes: 8,
            max_buttons: 1,
            ..budget()
        },
    );
    let d = create(&mut h);
    h.native("DialogSetMessage", &[d.clone(), Value::Str("éé".into())])
        .unwrap();
    let before = h.dialog_state().unwrap().revision();
    assert!(
        h.native(
            "DialogSetMessage",
            &[d.clone(), Value::Str("too-long-text".into())]
        )
        .is_err()
    );
    assert_eq!(h.dialog_state().unwrap().revision(), before);
    assert_eq!(
        h.dialog_state()
            .unwrap()
            .get(id(&d))
            .unwrap()
            .message
            .as_deref(),
        Some("éé")
    );
    let b = h
        .native(
            "DialogAddQuitButton",
            &[
                d.clone(),
                Value::Bool(false),
                Value::Str("quit".into()),
                Value::Int(0),
            ],
        )
        .unwrap();
    assert!(
        h.native(
            "DialogAddButton",
            &[d.clone(), Value::Str("x".into()), Value::Int(0)]
        )
        .is_err()
    );
    display(&mut h, &d, 2, true);
    let p = player(&mut h, 2);
    h.click_dialog_button(id(&p), id(&b)).unwrap();
    let request = h.take_dialog_quit_requests();
    assert_eq!(request.len(), 1);
    assert!(!request[0].score_screen);
    assert_eq!(request[0].player, id(&p));
    assert!(h.dialog_state().unwrap().quit_requests().is_empty());
}

#[test]
fn actual_vm_dialog_calls_project_executed_content_and_display() {
    let map = r#"
function BuildDialog takes nothing returns nothing
    local dialog d = DialogCreate()
    call DialogSetMessage(d, "choose from script")
    call DialogAddButton(d, "script choice", 82)
    call DialogDisplay(Player(2), d, true)
endfunction
"#;
    let mut vm = try_host_with_map(
        DECLARATIONS,
        DialogBudget {
            text_bytes: 128,
            ..budget()
        },
        map,
    )
    .unwrap()
    .into_vm();
    vm.init_globals().unwrap();
    vm.call("BuildDialog", &[]).unwrap();
    let state = vm.host().dialog_state().unwrap();
    let dialogs: Vec<_> = state.dialogs().collect();
    assert_eq!(dialogs.len(), 1);
    assert_eq!(dialogs[0].message.as_deref(), Some("choose from script"));
    assert_eq!(dialogs[0].buttons[0].text, "script choice");
    assert_eq!(dialogs[0].buttons[0].hotkey, 82);
    assert!(dialogs[0].visible_to(2));
}
#[test]
fn incompatible_dialog_declarations_and_second_admission_refuse() {
    let wrong = DECLARATIONS.replace(
        "native DialogSetMessage takes dialog d, string text",
        "native DialogSetMessage takes dialog d, integer text",
    );
    assert!(try_host_with_map(&wrong, budget(), "").is_err());
    let wrong = DECLARATIONS.replace(
        "constant native GetClickedDialog",
        "native GetClickedDialog",
    );
    assert!(try_host_with_map(&wrong, budget(), "").is_err());
    assert!(host().with_dialogs(budget()).is_err());
}

#[test]
fn player_event_response_uses_registered_player_and_symbolic_event() {
    let mut h = host().with_player_events(2).unwrap();
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let local = player(&mut h, 2);
    let event = h.native("ConvertPlayerEvent", &[Value::Int(724)]).unwrap();
    h.native(
        "TriggerRegisterPlayerEvent",
        &[trigger, local.clone(), event.clone()],
    )
    .unwrap();
    assert_eq!(
        h.fire_local_player_event("EXAMPLE_PLAYER_EVENT").unwrap(),
        1
    );
    let fired = h.take_fired_event().unwrap();
    h.begin_fired_event(&fired).unwrap().unwrap();
    assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), local);
    assert_eq!(h.native("GetTriggerEventId", &[]).unwrap(), event);
    h.finish_fired_event();
    assert_eq!(h.native("GetTriggerPlayer", &[]).unwrap(), Value::Null);
}
