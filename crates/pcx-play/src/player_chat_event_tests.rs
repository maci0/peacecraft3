use super::{EXTRAS, budget, common_source, context, slots_rule, trigger_limits};
use crate::map_admission::ParsedMapContext;
use crate::playercolor_host::{EnumHost, Limits, ObjectHost};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;

const DECLARATION: &str = "native TriggerRegisterPlayerChatEvent takes trigger t, player p, string s, boolean b returns event\n";
const EMPTY_MAP: &str = "function unused takes nothing returns nothing\nendfunction\n";

fn common() -> String {
    common_source(&format!("{EXTRAS}{DECLARATION}"))
}

fn host(common: &str, map: &str) -> (Arc<ParsedMapContext>, ObjectHost) {
    let ctx = context(302, common, map, "player-chat-event");
    let inner = EnumHost::checked_selected(
        ctx.scripts.shared_program(),
        "synthetic player chat event",
        &[
            "ConvertPlacement",
            "ConvertPlayerColor",
            "ConvertRacePref",
            "ConvertMapControl",
            "ConvertUnitEvent",
        ],
        Limits {
            max_handles: 32,
            admission_steps: 2000,
            expression_depth: 16,
            expression_work: 2000,
        },
    )
    .unwrap();
    let h = ObjectHost::checked(inner)
        .unwrap()
        .with_players(Arc::clone(&ctx), slots_rule(302, -3, 9), budget())
        .unwrap()
        .with_trigger_admission(trigger_limits(4, 0))
        .unwrap();
    (ctx, h)
}

fn id(v: &Value) -> HandleId {
    let Value::Handle(id) = v else {
        panic!("identity")
    };
    *id
}

#[test]
fn chat_registration_is_unsupported_until_admitted() {
    let (_, mut h) = host(&common(), EMPTY_MAP);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let error = h
        .native(
            "TriggerRegisterPlayerChatEvent",
            &[trigger, player, Value::Str("A".into()), Value::Bool(true)],
        )
        .unwrap_err();
    assert!(error.contains("unsupported by this host"), "{error}");
}

#[test]
fn admitted_chat_registrations_are_retained_up_to_the_budget() {
    let (_, h) = host(&common(), EMPTY_MAP);
    let mut h = h.with_player_chat_events(2).unwrap();
    assert_eq!(h.retained_player_chat_events().unwrap(), 0);
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let first = h
        .native(
            "TriggerRegisterPlayerChatEvent",
            &[
                trigger.clone(),
                player.clone(),
                Value::Str("A".into()),
                Value::Bool(true),
            ],
        )
        .unwrap();
    assert!(matches!(first, Value::Handle(_)));
    let stored = h.player_chat_registration(id(&first)).unwrap();
    assert_eq!(stored.message, "A");
    assert!(stored.exact_match_only);
    assert_eq!(stored.trigger, id(&trigger));
    assert_eq!(stored.player, id(&player));
    let second = h
        .native(
            "TriggerRegisterPlayerChatEvent",
            &[
                trigger.clone(),
                player.clone(),
                Value::Str("-cheat".into()),
                Value::Bool(false),
            ],
        )
        .unwrap();
    assert_ne!(id(&first), id(&second));
    assert_eq!(h.retained_player_chat_events().unwrap(), 2);
    let handles = h.inner().handles();
    let error = h
        .native(
            "TriggerRegisterPlayerChatEvent",
            &[trigger, player, Value::Str("B".into()), Value::Bool(true)],
        )
        .unwrap_err();
    assert!(error.contains("player chat event registration"), "{error}");
    assert_eq!(h.retained_player_chat_events().unwrap(), 2);
    assert_eq!(h.inner().handles(), handles);
}

#[test]
fn malformed_chat_registrations_refuse_without_retaining_anything() {
    let (_, h) = host(&common(), EMPTY_MAP);
    let mut h = h.with_player_chat_events(4).unwrap();
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    let text = Value::Str("A".into());
    let handles = h.inner().handles();
    for args in [
        vec![],
        vec![trigger.clone(), player.clone(), text.clone()],
        vec![Value::Null, player.clone(), text.clone(), Value::Bool(true)],
        vec![
            trigger.clone(),
            Value::Int(0),
            text.clone(),
            Value::Bool(true),
        ],
        vec![
            trigger.clone(),
            player.clone(),
            Value::Null,
            Value::Bool(true),
        ],
        vec![
            trigger.clone(),
            player.clone(),
            Value::Bool(true),
            Value::Bool(true),
        ],
        vec![trigger.clone(), player.clone(), text.clone(), Value::Int(1)],
        vec![
            player.clone(),
            player.clone(),
            text.clone(),
            Value::Bool(true),
        ],
        vec![
            trigger.clone(),
            trigger.clone(),
            text.clone(),
            Value::Bool(true),
        ],
    ] {
        assert!(
            h.native("TriggerRegisterPlayerChatEvent", &args).is_err(),
            "{args:?}"
        );
    }
    assert_eq!(h.retained_player_chat_events().unwrap(), 0);
    assert_eq!(h.inner().handles(), handles);
}

#[test]
fn zero_budget_admits_no_registration() {
    let (_, h) = host(&common(), EMPTY_MAP);
    let mut h = h.with_player_chat_events(0).unwrap();
    let trigger = h.native("CreateTrigger", &[]).unwrap();
    let player = h.native("Player", &[Value::Int(0)]).unwrap();
    assert!(
        h.native(
            "TriggerRegisterPlayerChatEvent",
            &[trigger, player, Value::Str("A".into()), Value::Bool(true)],
        )
        .is_err()
    );
    assert_eq!(h.retained_player_chat_events().unwrap(), 0);
}

#[test]
fn script_registration_retains_the_chat_request_through_the_vm() {
    let map = "\
function register takes nothing returns nothing
local trigger t = CreateTrigger()
call TriggerRegisterPlayerChatEvent(t, Player(0), \"A\", true)
endfunction
";
    let (ctx, h) = host(&common(), map);
    let h = h.with_player_chat_events(1).unwrap();
    let mut vm = Arc::clone(&ctx.scripts).owned_vm(h);
    drop(ctx);
    vm.init_globals().unwrap();
    vm.call("register", &[]).unwrap();
    assert_eq!(vm.host().inner.retained_player_chat_events().unwrap(), 1);
    assert!(vm.call("register", &[]).is_err());
}

#[test]
fn chat_admission_requires_the_declaration_and_admits_once() {
    let (_, undeclared) = host(&common_source(EXTRAS), EMPTY_MAP);
    assert!(undeclared.with_player_chat_events(1).is_err());
    let (_, h) = host(&common(), EMPTY_MAP);
    let h = h.with_player_chat_events(1).unwrap();
    assert!(h.with_player_chat_events(1).is_err());
}
