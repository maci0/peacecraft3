use super::super::{EnumHost, Limits, ObjectHost};
use crate::quest_session::QuestBudget;
use pcx_script::{Host, Prelude, Value};
use std::sync::Arc;

fn host() -> ObjectHost {
    host_from(false)
}

fn host_from(rename_formals: bool) -> ObjectHost {
    let mut source = "type agent extends handle\ntype timer extends agent\ntype quest extends handle\ntype questitem extends handle\ntype playercolor extends handle\nconstant native ConvertPlayerColor takes integer i returns playercolor\nnative CreateTimer takes nothing returns timer\nnative DestroyTimer takes timer t returns nothing\nnative CreateQuest takes nothing returns quest\nnative DestroyQuest takes quest whichQuest returns nothing\nnative QuestSetTitle takes quest whichQuest, string title returns nothing\nnative QuestSetDescription takes quest whichQuest, string description returns nothing\nnative QuestSetIconPath takes quest whichQuest, string iconPath returns nothing\nnative QuestSetRequired takes quest whichQuest, boolean required returns nothing\nnative QuestSetCompleted takes quest whichQuest, boolean completed returns nothing\nnative QuestSetDiscovered takes quest whichQuest, boolean discovered returns nothing\nnative QuestSetFailed takes quest whichQuest, boolean failed returns nothing\nnative QuestSetEnabled takes quest whichQuest, boolean enabled returns nothing\nnative IsQuestRequired takes quest whichQuest returns boolean\nnative IsQuestCompleted takes quest whichQuest returns boolean\nnative IsQuestDiscovered takes quest whichQuest returns boolean\nnative IsQuestFailed takes quest whichQuest returns boolean\nnative IsQuestEnabled takes quest whichQuest returns boolean\nnative QuestCreateItem takes quest whichQuest returns questitem\nnative QuestItemSetDescription takes questitem whichQuestItem, string description returns nothing\nnative QuestItemSetCompleted takes questitem whichQuestItem, boolean completed returns nothing\nnative IsQuestItemCompleted takes questitem whichQuestItem returns boolean\nnative FlashQuestDialogButton takes nothing returns nothing\nnative ForceQuestDialogUpdate takes nothing returns nothing\nglobals\nconstant playercolor C = ConvertPlayerColor(53)\nendglobals\n".to_string();
    if rename_formals {
        source = source
            .replace("whichQuestItem", "qi")
            .replace("whichQuest", "q")
            .replace("string title", "string caption")
            .replace("string description", "string body")
            .replace("string iconPath", "string icon")
            .replace("boolean required", "boolean must")
            .replace("boolean completed", "boolean done")
            .replace("boolean discovered", "boolean seen")
            .replace("boolean failed", "boolean lost")
            .replace("boolean enabled", "boolean on");
    }
    let common = pcx_script::parse_script(&source).unwrap();
    let map = pcx_script::parse_script("").unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    let enumerated = EnumHost::checked_selected(
        program,
        "synthetic:selected-map",
        &["ConvertPlayerColor"],
        Limits {
            max_handles: 8,
            admission_steps: 100,
            expression_depth: 16,
            expression_work: 100,
        },
    )
    .unwrap();
    ObjectHost::checked(enumerated).unwrap()
}

fn budget() -> QuestBudget {
    QuestBudget {
        max_quests: 2,
        max_items: 2,
        text_bytes: 32,
        max_events: 8,
    }
}

#[test]
fn object_host_refuses_quests_until_admission_then_shares_timer_identity() {
    let mut host = host();
    let missing = host.native("CreateQuest", &[]).unwrap_err();
    assert!(missing.contains("unsupported"), "{missing}");
    let mut host = host.with_quests(budget()).unwrap();
    let timer = host.native("CreateTimer", &[]).unwrap();
    let quest = host.native("CreateQuest", &[]).unwrap();
    assert_ne!(timer, quest);
    let cross = host
        .native("QuestSetDiscovered", &[timer, Value::Bool(true)])
        .unwrap_err();
    assert!(cross.contains("another family"), "{cross}");
    let cross = host
        .native("DestroyTimer", std::slice::from_ref(&quest))
        .unwrap_err();
    assert!(cross.contains("another family"), "{cross}");
    assert_eq!(
        host.native("IsQuestDiscovered", std::slice::from_ref(&quest))
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        host.native("QuestSetDiscovered", &[quest.clone(), Value::Bool(true)])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        host.native("IsQuestDiscovered", std::slice::from_ref(&quest))
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        host.native("DestroyQuest", std::slice::from_ref(&quest))
            .unwrap(),
        Value::Null
    );
    let stale = host
        .native("IsQuestDiscovered", std::slice::from_ref(&quest))
        .unwrap_err();
    assert!(stale.contains("unknown handle identity"), "{stale}");
    let Err(again) = host.with_quests(budget()) else {
        panic!("second quest admission succeeded");
    };
    assert!(again.to_string().contains("already admitted"), "{again}");
}

#[test]
fn renamed_formals_use_the_same_object_host_lifecycle() {
    let mut host = host_from(true).with_quests(budget()).unwrap();
    let timer = host.native("CreateTimer", &[]).unwrap();
    let quest = host.native("CreateQuest", &[]).unwrap();
    let cross = host
        .native("QuestSetDiscovered", &[timer.clone(), Value::Bool(true)])
        .unwrap_err();
    assert!(cross.contains("another family"), "{cross}");
    assert_eq!(
        host.native("IsQuestEnabled", std::slice::from_ref(&quest))
            .unwrap(),
        Value::Bool(true)
    );
    let item = host
        .native("QuestCreateItem", std::slice::from_ref(&quest))
        .unwrap();
    host.native("DestroyQuest", std::slice::from_ref(&quest))
        .unwrap();
    let stale = host
        .native("IsQuestItemCompleted", std::slice::from_ref(&item))
        .unwrap_err();
    assert!(stale.contains("unknown handle identity"), "{stale}");
    let timer_still = host.native("DestroyTimer", &[timer]).unwrap();
    assert_eq!(timer_still, Value::Null);
}
