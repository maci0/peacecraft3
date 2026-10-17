//! Opt-in quests in the production map consumer's existing VM and object arena.
use super::*;
use pcx_play::playercolor_host::QuestBudget;
use pcx_play::quest_view;
use pcx_script::{HandleId, Host};

const EXTRA: &str = "\
type quest extends handle
type questitem extends handle
native CreateQuest takes nothing returns quest
native DestroyQuest takes quest q returns nothing
native QuestSetTitle takes quest q, string title returns nothing
native QuestSetDescription takes quest q, string description returns nothing
native QuestSetIconPath takes quest q, string icon returns nothing
native QuestSetRequired takes quest q, boolean value returns nothing
native QuestSetCompleted takes quest q, boolean value returns nothing
native QuestSetDiscovered takes quest q, boolean value returns nothing
native QuestSetFailed takes quest q, boolean value returns nothing
native QuestSetEnabled takes quest q, boolean value returns nothing
native IsQuestRequired takes quest q returns boolean
native IsQuestCompleted takes quest q returns boolean
native IsQuestDiscovered takes quest q returns boolean
native IsQuestFailed takes quest q returns boolean
native IsQuestEnabled takes quest q returns boolean
native QuestCreateItem takes quest q returns questitem
native QuestItemSetDescription takes questitem item, string description returns nothing
native QuestItemSetCompleted takes questitem item, boolean value returns nothing
native IsQuestItemCompleted takes questitem item returns boolean
native FlashQuestDialogButton takes nothing returns nothing
native ForceQuestDialogUpdate takes nothing returns nothing
";
const MAP: &str = r#"
globals
timer before = null
quest q = null
questitem item = null
quest other = null
timer after = null
boolean completed = false
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set before = CreateTimer()
set q = CreateQuest()
set item = QuestCreateItem(q)
set other = CreateQuest()
set after = CreateTimer()
call QuestSetTitle(q,"Alpha")
call QuestSetDescription(q,"")
call QuestSetIconPath(q,"synthetic/icon.blp")
call QuestSetRequired(q,false)
call QuestSetCompleted(q,true)
call QuestSetDiscovered(q,false)
call QuestSetFailed(q,false)
call QuestSetEnabled(q,true)
call QuestItemSetDescription(item,"Hold")
call QuestItemSetCompleted(item,true)
call FlashQuestDialogButton()
call ForceQuestDialogUpdate()
set completed = IsQuestCompleted(q) and IsQuestItemCompleted(item)
endfunction
function finish takes nothing returns nothing
call DestroyQuest(q)
endfunction
"#;

fn budget() -> QuestBudget {
    QuestBudget {
        max_quests: 2,
        max_items: 1,
        text_bytes: 64,
        max_events: 32,
    }
}

fn deliver_quests(app: &mut App, revision: u64, budget: Option<QuestBudget>, extra: &str) {
    let mut map = fixture(MAP);
    map.common.extend_from_slice(extra.as_bytes());
    map.rebuild();
    let resolver = map.resolver();
    let bound = bind_map(&resolver, map.input(revision), map.limits()).unwrap();
    let mut inputs = host_inputs(Arc::clone(&bound.catalog.0));
    inputs.budgets.quests = budget;
    let bytes = Arc::clone(&bound.catalog_map_bytes);
    app.insert_resource(MapRequests {
        bindings: Some(bound),
        ..Default::default()
    });
    app.insert_resource(CampaignHostSupply {
        revision,
        inputs: Some(inputs),
    });
    app.world_mut().write_message(MapRequest {
        label: "synthetic quest map".into(),
        map_member: pcx_assets::AssetPath::new("selected map.w3m").unwrap(),
        identity: map.map_id.clone(),
        binding_revision: revision,
        payload: MapPayload::Shared(bytes),
    });
    app.update();
    assert!(
        app.world()
            .resource::<MapRequests>()
            .last_outcome
            .as_ref()
            .unwrap()
            .result
            .is_ok()
    );
}

fn app() -> App {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    app
}

fn handle(session: &MapScriptSession<MapStartupHost>, name: &str) -> HandleId {
    let Some(Value::Handle(id)) = session.global(name) else {
        panic!("expected handle {name}")
    };
    id
}

#[test]
fn published_quests_share_the_timer_arena_and_execute_in_the_published_vm() {
    let mut app = app();
    deliver_quests(&mut app, 1, Some(budget()), EXTRA);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert_eq!(session.global("completed"), Some(Value::Bool(true)));
    let ids = ["before", "q", "item", "other", "after"].map(|name| handle(&session, name));
    for pair in ids.windows(2) {
        assert_eq!(pair[1].token(), pair[0].token() + 1);
    }
    let [before, q, item, other, after] = ids;
    let finish = session
        .context()
        .unwrap()
        .scripts
        .program()
        .function_id("finish")
        .unwrap();
    let ticket = session.callback_ticket(finish).unwrap();
    let count = {
        let host = session.host().unwrap();
        let quests = host.objects().quest_session().unwrap();
        let view = quest_view::project_quest(quests, q).unwrap();
        assert_eq!(view.title, Some("Alpha"));
        assert_eq!(view.description, Some(""));
        assert_eq!(view.icon_path, Some("synthetic/icon.blp"));
        assert!(
            !view.required && !view.discovered && view.completed && !view.failed && view.enabled
        );
        assert_eq!(view.items.len(), 1);
        assert_eq!(view.items[0].id, item);
        assert_eq!(view.items[0].description, Some("Hold"));
        assert!(view.items[0].completed);
        assert_eq!(quests.quest_title(other).unwrap(), None);
        let events = quests.events().len();
        assert!(
            host.native(
                "QuestSetTitle",
                &[Value::Handle(before), Value::Str("wrong".into())]
            )
            .is_err()
        );
        assert!(host.native("DestroyTimer", &[Value::Handle(q)]).is_err());
        assert_eq!(host.objects().quest_events().unwrap().len(), events);
        assert_eq!(host.objects().live_timers(), 2);
        host.objects().inner().handles()
    };
    session.call_callback(&ticket, &[]).unwrap();
    let host = session.host().unwrap();
    let quests = host.objects().quest_session().unwrap();
    assert_eq!(quests.live_quests(), 1);
    assert!(quests.quest_title(q).is_err());
    assert!(quests.item_owner(item).is_err());
    assert_eq!(quests.quest_title(other).unwrap(), None);
    assert!(
        host.native("IsQuestCompleted", &[Value::Handle(q)])
            .is_err()
    );
    assert!(
        host.native("IsQuestItemCompleted", &[Value::Handle(item)])
            .is_err()
    );
    assert_eq!(host.objects().inner().handles(), count);
    for timer in [before, after] {
        host.native("DestroyTimer", &[Value::Handle(timer)])
            .unwrap();
    }
    assert_eq!(host.objects().live_timers(), 0);
}

#[test]
fn quest_opt_out_budgets_and_bad_declarations_preserve_the_published_session() {
    let mut app = app();
    deliver_quests(&mut app, 1, Some(budget()), EXTRA);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let (witness, context, q, ticket) = {
        let session = app.world().resource::<MapScriptSession<MapStartupHost>>();
        let context = Arc::clone(session.context().unwrap());
        let finish = context.scripts.program().function_id("finish").unwrap();
        (
            session.session_witness().unwrap(),
            context,
            handle(session, "q"),
            session.callback_ticket(finish).unwrap(),
        )
    };
    let valid = budget();
    for (index, budget) in [
        None,
        Some(QuestBudget {
            max_quests: 0,
            ..valid
        }),
        Some(QuestBudget {
            max_items: 0,
            ..valid
        }),
        Some(QuestBudget {
            text_bytes: 0,
            ..valid
        }),
        Some(QuestBudget {
            max_events: 0,
            ..valid
        }),
    ]
    .into_iter()
    .enumerate()
    {
        deliver_quests(&mut app, index as u64 + 2, budget, EXTRA);
        assert!(matches!(
            app.world().resource::<CampaignStartup>().last_error,
            Some(StartupError::Session(ScriptSessionError::Execution(_)))
        ));
        assert!(witness.upgrade().is_some());
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        assert!(Arc::ptr_eq(session.context().unwrap(), &context));
        let host = session.host().unwrap();
        assert_eq!(
            host.objects()
                .quest_session()
                .unwrap()
                .quest_title(q)
                .unwrap(),
            Some("Alpha")
        );
        assert_eq!(host.objects().live_timers(), 2);
    }
    let bad = EXTRA.replace("native IsQuestFailed takes quest q returns boolean\n", "");
    deliver_quests(&mut app, 7, Some(valid), &bad);
    assert!(matches!(
        app.world().resource::<CampaignStartup>().last_error,
        Some(StartupError::Session(ScriptSessionError::Factory { .. }))
    ));
    assert!(witness.upgrade().is_some());
    app.world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>()
        .call_callback(&ticket, &[])
        .unwrap();
    deliver_quests(&mut app, 8, Some(valid), EXTRA);
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    assert!(witness.upgrade().is_none());
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert!(matches!(
        session.call_callback(&ticket, &[]),
        Err(ScriptSessionError::StaleCallback { .. })
    ));
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .quest_session()
            .unwrap()
            .live_quests(),
        2
    );
    assert_eq!(session.global("completed"), Some(Value::Bool(true)));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn browser_quest_factory_gate() -> u32 {
    published_quests_share_the_timer_arena_and_execute_in_the_published_vm();
    quest_opt_out_budgets_and_bad_declarations_preserve_the_published_session();
    2
}
