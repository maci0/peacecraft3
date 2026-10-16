use super::super::{EnumHost, Limits, ObjectHost};
use crate::quest_session::{QuestBudget, QuestEvent, QuestFailure};
use crate::quest_view::{project, project_quest};
use crate::scripts::{AssetIdentity, MapScripts, ScriptAsset};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;

const QUESTS: &str = "
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

fn handle(value: &Value) -> HandleId {
    match value {
        Value::Handle(id) => *id,
        other => panic!("expected handle, got {other:?}"),
    }
}

#[test]
fn owned_vm_projects_interleaved_quests_through_the_shared_arena() {
    let common = format!(
        "type agent extends handle\ntype timer extends agent\ntype quest extends handle\ntype questitem extends handle\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\n{QUESTS}\nglobals\nconstant playercolor C = ConvertPlayerColor(53)\nendglobals\n"
    );
    let map = r#"
globals
    timer clock
    quest first
    questitem early
    quest second
    questitem late
    questitem other
    timer later
endglobals
function prepare takes nothing returns nothing
    set clock = CreateTimer()
    set first = CreateQuest()
    set early = QuestCreateItem(first)
    set second = CreateQuest()
    set late = QuestCreateItem(first)
    set other = QuestCreateItem(second)
    set later = CreateTimer()
    call QuestSetTitle(first, "Alpha")
    call QuestSetDescription(second, "")
    call QuestSetIconPath(first, "buttons/alpha.blp")
    call QuestSetRequired(first, false)
    call QuestSetDiscovered(first, false)
    call QuestSetCompleted(first, true)
    call QuestSetFailed(second, true)
    call QuestSetEnabled(second, false)
    call QuestItemSetDescription(early, "hold")
    call QuestItemSetCompleted(late, true)
    call ForceQuestDialogUpdate()
endfunction
function finish takes nothing returns nothing
    call DestroyQuest(first)
endfunction
"#;
    let scripts = Arc::new(
        MapScripts::compile(
            AssetIdentity {
                edition: "synthetic".into(),
                version: "quest-view".into(),
                source: "synthetic:quest-owned-vm".into(),
            },
            ScriptAsset {
                label: "synthetic:common",
                bytes: common.as_bytes(),
            },
            ScriptAsset {
                label: "synthetic:blizzard",
                bytes: b"// blizzard\n",
            },
            ScriptAsset {
                label: "synthetic:map",
                bytes: map.as_bytes(),
            },
            None,
        )
        .unwrap(),
    );
    let enumerated = EnumHost::checked_selected(
        scripts.shared_program(),
        "synthetic:map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 16,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    let host = ObjectHost::checked(enumerated)
        .unwrap()
        .with_quests(QuestBudget {
            max_quests: 4,
            max_items: 8,
            text_bytes: 256,
            max_events: 64,
        })
        .unwrap();
    let mut vm = Arc::clone(&scripts).owned_vm(host);
    vm.call("prepare", &[]).unwrap();
    let color = handle(&vm.global("C"));
    let clock = handle(&vm.global("clock"));
    let first = handle(&vm.global("first"));
    let early = handle(&vm.global("early"));
    let second = handle(&vm.global("second"));
    let late = handle(&vm.global("late"));
    let other = handle(&vm.global("other"));
    let later = handle(&vm.global("later"));
    assert!(color.token() < clock.token());
    assert_eq!(clock.token() + 1, first.token());
    assert_eq!(first.token() + 1, early.token());
    assert_eq!(early.token() + 1, second.token());
    assert_eq!(second.token() + 1, late.token());
    assert_eq!(late.token() + 1, other.token());
    assert_eq!(other.token() + 1, later.token());
    {
        let session = vm.host().inner.quest_session().expect("admitted session");
        assert!(session.events().contains(&QuestEvent::DialogUpdate));
        let board = project(session).unwrap();
        assert_eq!(board.quests.len(), 2);
        assert_eq!(board.quests[0].id, first);
        assert_eq!(board.quests[0].title, Some("Alpha"));
        assert_eq!(board.quests[0].description, None);
        assert_eq!(board.quests[0].icon_path, Some("buttons/alpha.blp"));
        assert!(!board.quests[0].required);
        assert!(!board.quests[0].discovered);
        assert!(board.quests[0].completed);
        assert!(!board.quests[0].failed);
        assert!(board.quests[0].enabled);
        assert_eq!(board.quests[0].items.len(), 2);
        assert_eq!(board.quests[0].items[0].id, early);
        assert_eq!(board.quests[0].items[0].description, Some("hold"));
        assert!(!board.quests[0].items[0].completed);
        assert_eq!(board.quests[0].items[1].id, late);
        assert_eq!(board.quests[0].items[1].description, None);
        assert!(board.quests[0].items[1].completed);
        assert_eq!(board.quests[1].id, second);
        assert_eq!(board.quests[1].title, None);
        assert_eq!(board.quests[1].description, Some(""));
        assert_eq!(board.quests[1].icon_path, None);
        assert!(board.quests[1].required);
        assert!(board.quests[1].discovered);
        assert!(!board.quests[1].completed);
        assert!(board.quests[1].failed);
        assert!(!board.quests[1].enabled);
        assert_eq!(board.quests[1].items.len(), 1);
        assert_eq!(board.quests[1].items[0].id, other);
        assert_eq!(session.item_owner(other).unwrap(), second);
    }
    vm.call("finish", &[]).unwrap();
    {
        let session = vm.host().inner.quest_session().expect("admitted session");
        let board = project(session).unwrap();
        assert_eq!(board.quests.len(), 1);
        assert_eq!(board.quests[0].id, second);
        assert_eq!(board.quests[0].items.len(), 1);
        assert_eq!(board.quests[0].items[0].id, other);
        assert_eq!(
            project_quest(session, first).unwrap_err(),
            QuestFailure::Identity
        );
        assert_eq!(
            session.item_description(early).unwrap_err(),
            QuestFailure::Identity
        );
        assert_eq!(
            session.item_owner(late).unwrap_err(),
            QuestFailure::Identity
        );
    }
    assert_eq!(
        vm.host()
            .inner
            .native("DestroyTimer", &[Value::Handle(clock)])
            .unwrap(),
        Value::Null
    );
    let crossed = vm
        .host()
        .inner
        .native("DestroyTimer", &[Value::Handle(first)])
        .unwrap_err();
    assert!(crossed.contains("another family"), "{crossed}");
    assert_eq!(
        vm.host()
            .inner
            .native("DestroyTimer", &[Value::Handle(later)])
            .unwrap(),
        Value::Null
    );
}
