//! Delivered-map admission of the existing quest arena.
//! None refuses the natives. A supplied budget does not invent text or flags.
use super::*;
use pcx_play::playercolor_host::QuestBudget;
use pcx_play::quest_session::QuestFlag;
use pcx_play::quest_view::project;

const EXTRA: &str = "\
type quest extends handle
type questitem extends handle
native CreateQuest takes nothing returns quest
native DestroyQuest takes quest whichQuest returns nothing
native QuestSetTitle takes quest whichQuest, string title returns nothing
native QuestSetDescription takes quest whichQuest, string description returns nothing
native QuestSetIconPath takes quest whichQuest, string iconPath returns nothing
native QuestSetRequired takes quest whichQuest, boolean required returns nothing
native QuestSetCompleted takes quest whichQuest, boolean completed returns nothing
native QuestSetDiscovered takes quest whichQuest, boolean discovered returns nothing
native QuestSetFailed takes quest whichQuest, boolean failed returns nothing
native QuestSetEnabled takes quest whichQuest, boolean enabled returns nothing
native IsQuestRequired takes quest whichQuest returns boolean
native IsQuestCompleted takes quest whichQuest returns boolean
native IsQuestDiscovered takes quest whichQuest returns boolean
native IsQuestFailed takes quest whichQuest returns boolean
native IsQuestEnabled takes quest whichQuest returns boolean
native QuestCreateItem takes quest whichQuest returns questitem
native QuestItemSetDescription takes questitem whichQuestItem, string description returns nothing
native QuestItemSetCompleted takes questitem whichQuestItem, boolean completed returns nothing
native IsQuestItemCompleted takes questitem whichQuestItem returns boolean
native FlashQuestDialogButton takes nothing returns nothing
native ForceQuestDialogUpdate takes nothing returns nothing
";

const MAP: &str = "\
globals
quest chapter = null
quest untouched = null
questitem objective = null
string title = \"Hold the ridge\"
string body = \"Survive the night\"
string icon = \"ReplaceableTextures\\\\CommandButtons\\\\BTNHold.blp\"
string itemBody = \"Keep the gate\"
boolean required = true
boolean completed = false
boolean discovered = true
boolean failed = false
boolean enabled = true
boolean itemDone = false
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set chapter = CreateQuest()
set untouched = CreateQuest()
call QuestSetTitle(chapter, title)
call QuestSetDescription(chapter, body)
call QuestSetIconPath(chapter, icon)
call QuestSetRequired(chapter, required)
call QuestSetCompleted(chapter, completed)
call QuestSetDiscovered(chapter, discovered)
call QuestSetFailed(chapter, failed)
call QuestSetEnabled(chapter, enabled)
set objective = QuestCreateItem(chapter)
call QuestItemSetDescription(objective, itemBody)
call QuestItemSetCompleted(objective, itemDone)
call ForceQuestDialogUpdate()
endfunction
";

fn budget() -> QuestBudget {
    QuestBudget {
        max_quests: 2,
        max_items: 1,
        text_bytes: 256,
        max_events: 16,
    }
}

fn deliver_quests(app: &mut App, revision: u64, budget: Option<QuestBudget>) {
    let mut map = fixture(MAP);
    map.common.extend_from_slice(EXTRA.as_bytes());
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
        label: "synthetic quests".into(),
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

fn handle(value: Option<Value>) -> pcx_script::HandleId {
    match value {
        Some(Value::Handle(id)) => id,
        other => panic!("expected a quest handle, got {other:?}"),
    }
}

#[test]
fn delivered_map_admits_source_text_and_keeps_unknown_flags() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    deliver_quests(&mut app, 1, Some(budget()));
    assert!(
        app.world()
            .resource::<CampaignStartup>()
            .last_error
            .is_none()
    );
    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    let chapter = handle(session.global("chapter"));
    let untouched = handle(session.global("untouched"));
    let objective = handle(session.global("objective"));
    let host = session.host().unwrap().objects();
    let admitted = host.quest_session().expect("opted-in quest arena");
    let board = project(admitted).unwrap();
    assert_eq!(board.quests.len(), 2);
    assert_eq!(board.quests[0].id, chapter);
    assert_eq!(board.quests[0].title, Some("Hold the ridge"));
    assert_eq!(board.quests[0].description, Some("Survive the night"));
    assert_eq!(
        board.quests[0].icon_path,
        Some("ReplaceableTextures\\CommandButtons\\BTNHold.blp")
    );
    assert!(board.quests[0].required);
    assert!(!board.quests[0].completed);
    assert!(board.quests[0].discovered);
    assert!(!board.quests[0].failed);
    assert!(board.quests[0].enabled);
    assert_eq!(board.quests[0].items.len(), 1);
    assert_eq!(board.quests[0].items[0].id, objective);
    assert_eq!(board.quests[0].items[0].description, Some("Keep the gate"));
    assert!(!board.quests[0].items[0].completed);
    assert_eq!(board.quests[1].id, untouched);
    assert_eq!(board.quests[1].title, None);
    assert_eq!(board.quests[1].description, None);
    assert_eq!(board.quests[1].icon_path, None);
    assert_eq!(board.quests[1].items.len(), 0);
    assert_eq!(
        admitted.quest_flag(untouched, QuestFlag::Required),
        Ok(true)
    );
    assert_eq!(
        admitted.quest_flag(untouched, QuestFlag::Completed),
        Ok(false)
    );
    assert_eq!(
        admitted.quest_flag(untouched, QuestFlag::Discovered),
        Ok(true)
    );
    assert_eq!(admitted.quest_flag(untouched, QuestFlag::Failed), Ok(false));
    assert_eq!(admitted.quest_flag(untouched, QuestFlag::Enabled), Ok(true));
    assert!(
        admitted
            .events()
            .contains(&pcx_play::playercolor_host::QuestEvent::DialogUpdate)
    );
}

#[test]
fn no_opt_in_and_failed_replacement_keep_the_published_quest_state() {
    let mut app = App::new();
    peacecraft3::map_requests::install(&mut app);
    peacecraft3::campaign_startup::install(&mut app);
    deliver_quests(&mut app, 1, Some(budget()));
    let witness = app
        .world()
        .resource::<MapScriptSession<MapStartupHost>>()
        .session_witness()
        .unwrap();
    let (chapter, title, context) = {
        let mut session = app
            .world_mut()
            .resource_mut::<MapScriptSession<MapStartupHost>>();
        let chapter = handle(session.global("chapter"));
        let title = session
            .host()
            .unwrap()
            .objects()
            .quest_session()
            .unwrap()
            .quest_title(chapter)
            .unwrap()
            .map(str::to_owned);
        (chapter, title, Arc::clone(session.context().unwrap()))
    };

    deliver_quests(&mut app, 2, None);
    let refused = app
        .world()
        .resource::<CampaignStartup>()
        .last_error
        .as_ref()
        .map(ToString::to_string);
    match refused.as_deref() {
        Some(diagnostic) => {
            assert!(diagnostic.contains("CreateQuest"), "{diagnostic}");
            assert!(
                diagnostic.contains("unsupported by this host"),
                "{diagnostic}"
            );
        }
        None => panic!("expected no-opt-in refusal"),
    }

    let tight = QuestBudget {
        max_quests: 1,
        ..budget()
    };
    deliver_quests(&mut app, 3, Some(tight));
    let refused = app
        .world()
        .resource::<CampaignStartup>()
        .last_error
        .as_ref()
        .map(ToString::to_string);
    match refused.as_deref() {
        Some(diagnostic) => {
            assert!(diagnostic.contains("CreateQuest"), "{diagnostic}");
            assert!(diagnostic.contains("quest objects"), "{diagnostic}");
        }
        None => panic!("expected quest budget refusal"),
    }

    let mut session = app
        .world_mut()
        .resource_mut::<MapScriptSession<MapStartupHost>>();
    assert!(witness.upgrade().is_some());
    assert!(Arc::ptr_eq(session.context().unwrap(), &context));
    assert_eq!(handle(session.global("chapter")), chapter);
    assert_eq!(
        session
            .host()
            .unwrap()
            .objects()
            .quest_session()
            .unwrap()
            .quest_title(chapter)
            .unwrap(),
        title.as_deref()
    );
}
