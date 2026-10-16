use super::{project, project_quest};
use crate::quest_session::{
    ObjectClass, ObjectKind, QuestArena, QuestBudget, QuestFailure, QuestSession, accepts,
};
use pcx_script::{HandleId, Host, OwnedVm, Prelude, Value};
use std::num::NonZeroU64;
use std::sync::Arc;

const NATIVES: &str = "
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

struct Slot {
    kind: SlotKind,
    live: bool,
}
enum SlotKind {
    Quest,
    Item,
}
struct Arena {
    slots: Vec<Slot>,
}
impl Arena {
    fn new() -> Self {
        Self { slots: Vec::new() }
    }
}
impl QuestArena for Arena {
    fn alloc(&mut self, kind: ObjectKind) -> Result<HandleId, QuestFailure> {
        self.slots.push(Slot {
            kind: match kind {
                ObjectKind::Quest => SlotKind::Quest,
                ObjectKind::Item => SlotKind::Item,
            },
            live: true,
        });
        let token = u64::try_from(self.slots.len()).map_err(|_| QuestFailure::Capacity)?;
        Ok(HandleId::new(
            NonZeroU64::new(token).expect("len is nonzero"),
        ))
    }
    fn classify(&self, id: HandleId) -> Result<ObjectClass, QuestFailure> {
        let slot = usize::try_from(id.token() - 1)
            .ok()
            .and_then(|index| self.slots.get(index))
            .ok_or(QuestFailure::Identity)?;
        Ok(match (&slot.kind, slot.live) {
            (SlotKind::Quest, true) => ObjectClass::Quest,
            (SlotKind::Quest, false) => ObjectClass::DeadQuest,
            (SlotKind::Item, true) => ObjectClass::Item,
            (SlotKind::Item, false) => ObjectClass::DeadItem,
        })
    }
    fn kill(&mut self, id: HandleId) -> Result<(), QuestFailure> {
        let slot = usize::try_from(id.token() - 1)
            .ok()
            .and_then(|index| self.slots.get_mut(index))
            .ok_or(QuestFailure::Identity)?;
        if slot.live {
            slot.live = false;
            Ok(())
        } else {
            Err(QuestFailure::Identity)
        }
    }
}

struct ScriptHost {
    session: QuestSession,
    arena: Arena,
}
impl Host for ScriptHost {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if accepts(name) {
            self.session
                .call(&mut self.arena, name, args)
                .map_err(|failure| failure.to_string())
        } else {
            Err(format!("unsupported native {name}"))
        }
    }
}

fn budget() -> QuestBudget {
    QuestBudget {
        max_quests: 4,
        max_items: 8,
        text_bytes: 256,
        max_events: 64,
    }
}

fn execute(map: &str) -> OwnedVm<ScriptHost> {
    let common = format!(
        "type agent extends handle\ntype quest extends agent\ntype questitem extends agent\n{NATIVES}"
    );
    let common = pcx_script::parse(&common).unwrap();
    let map = pcx_script::parse(map).unwrap();
    let program = Arc::new(
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
    );
    let session = QuestSession::admit(program.as_ref(), budget(), &[]).unwrap();
    let mut vm = OwnedVm::new_owned(
        Arc::clone(&program),
        ScriptHost {
            session,
            arena: Arena::new(),
        },
    );
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    vm
}

#[test]
fn jass_mutation_projects_authored_text_flags_and_creation_order() {
    let mut vm = execute(
        r#"
        function main takes nothing returns nothing
            local quest first = CreateQuest()
            local questitem early = QuestCreateItem(first)
            local quest second = CreateQuest()
            local questitem late = QuestCreateItem(first)
            local questitem other = QuestCreateItem(second)
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
        endfunction
        "#,
    );
    let (second_id, second_item) = {
        let board = project(&vm.host().session).unwrap();
        assert_eq!(board.quests.len(), 2);
        let first = &board.quests[0];
        let second = &board.quests[1];
        assert!(first.id.token() < second.id.token());
        assert_eq!(first.title, Some("Alpha"));
        assert_eq!(first.description, None);
        assert_eq!(first.icon_path, Some("buttons/alpha.blp"));
        assert!(!first.required);
        assert!(!first.discovered);
        assert!(first.completed);
        assert!(!first.failed);
        assert!(first.enabled);
        assert_eq!(first.items.len(), 2);
        assert!(first.items[0].id.token() < first.items[1].id.token());
        assert_eq!(first.items[0].description, Some("hold"));
        assert!(!first.items[0].completed);
        assert_eq!(first.items[1].description, None);
        assert!(first.items[1].completed);
        assert_eq!(second.title, None);
        assert_eq!(second.description, Some(""));
        assert_eq!(second.icon_path, None);
        assert!(second.required);
        assert!(second.discovered);
        assert!(!second.completed);
        assert!(second.failed);
        assert!(!second.enabled);
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.items[0].description, None);
        assert!(!second.items[0].completed);
        (second.id, second.items[0].id)
    };
    assert_eq!(
        vm.host().session.item_owner(second_item).unwrap(),
        second_id
    );
    assert_eq!(vm.host().session.live_items(), 3);
}

#[test]
fn destroyed_parent_drops_its_items_and_refuses_the_identity() {
    let mut vm = execute(
        r#"
        function main takes nothing returns nothing
            local quest parent = CreateQuest()
            local questitem child = QuestCreateItem(parent)
            local quest survivor = CreateQuest()
            local questitem kept = QuestCreateItem(survivor)
            call QuestSetTitle(survivor, "Kept")
            call QuestItemSetDescription(child, "gone")
            call QuestItemSetDescription(kept, "stay")
            call DestroyQuest(parent)
        endfunction
        "#,
    );
    {
        let board = project(&vm.host().session).unwrap();
        assert_eq!(board.quests.len(), 1);
        assert_eq!(board.quests[0].title, Some("Kept"));
        assert_eq!(board.quests[0].items.len(), 1);
        assert_eq!(board.quests[0].items[0].description, Some("stay"));
        let dead_quest = HandleId::new(NonZeroU64::new(1).unwrap());
        let dead_item = HandleId::new(NonZeroU64::new(2).unwrap());
        assert!(!board.quests.iter().any(|quest| {
            quest.id == dead_quest || quest.items.iter().any(|item| item.id == dead_item)
        }));
    }
    assert_eq!(vm.host().session.live_quests(), 1);
    assert_eq!(vm.host().session.live_items(), 1);
    let dead_quest = HandleId::new(NonZeroU64::new(1).unwrap());
    let dead_item = HandleId::new(NonZeroU64::new(2).unwrap());
    assert_eq!(
        vm.host().arena.classify(dead_quest).unwrap(),
        ObjectClass::DeadQuest
    );
    assert_eq!(
        vm.host().arena.classify(dead_item).unwrap(),
        ObjectClass::DeadItem
    );
    assert_eq!(
        project_quest(&vm.host().session, dead_quest).unwrap_err(),
        QuestFailure::Identity
    );
    assert_eq!(
        vm.host().session.item_description(dead_item).unwrap_err(),
        QuestFailure::Identity
    );
    assert_eq!(
        vm.host().session.item_owner(dead_item).unwrap_err(),
        QuestFailure::Identity
    );
}
