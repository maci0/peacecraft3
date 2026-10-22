use super::{
    ObjectClass, ObjectKind, QuestAdmitError, QuestArena, QuestBudget, QuestEvent, QuestFailure,
    QuestFlag, QuestSession, accepts,
};
use pcx_script::{HandleId, Host, NullHost, Prelude, Program, Value, Vm};
use std::num::NonZeroU64;

const SHIPPED: &[(&str, &str)] = &[
    ("CreateQuest", "CreateQuest takes nothing returns quest"),
    (
        "DestroyQuest",
        "DestroyQuest takes quest whichQuest returns nothing",
    ),
    (
        "QuestSetTitle",
        "QuestSetTitle takes quest whichQuest, string title returns nothing",
    ),
    (
        "QuestSetDescription",
        "QuestSetDescription takes quest whichQuest, string description returns nothing",
    ),
    (
        "QuestSetIconPath",
        "QuestSetIconPath takes quest whichQuest, string iconPath returns nothing",
    ),
    (
        "QuestSetRequired",
        "QuestSetRequired takes quest whichQuest, boolean required returns nothing",
    ),
    (
        "QuestSetCompleted",
        "QuestSetCompleted takes quest whichQuest, boolean completed returns nothing",
    ),
    (
        "QuestSetDiscovered",
        "QuestSetDiscovered takes quest whichQuest, boolean discovered returns nothing",
    ),
    (
        "QuestSetFailed",
        "QuestSetFailed takes quest whichQuest, boolean failed returns nothing",
    ),
    (
        "QuestSetEnabled",
        "QuestSetEnabled takes quest whichQuest, boolean enabled returns nothing",
    ),
    (
        "IsQuestRequired",
        "IsQuestRequired takes quest whichQuest returns boolean",
    ),
    (
        "IsQuestCompleted",
        "IsQuestCompleted takes quest whichQuest returns boolean",
    ),
    (
        "IsQuestDiscovered",
        "IsQuestDiscovered takes quest whichQuest returns boolean",
    ),
    (
        "IsQuestFailed",
        "IsQuestFailed takes quest whichQuest returns boolean",
    ),
    (
        "IsQuestEnabled",
        "IsQuestEnabled takes quest whichQuest returns boolean",
    ),
    (
        "QuestCreateItem",
        "QuestCreateItem takes quest whichQuest returns questitem",
    ),
    (
        "QuestItemSetDescription",
        "QuestItemSetDescription takes questitem whichQuestItem, string description returns nothing",
    ),
    (
        "QuestItemSetCompleted",
        "QuestItemSetCompleted takes questitem whichQuestItem, boolean completed returns nothing",
    ),
    (
        "IsQuestItemCompleted",
        "IsQuestItemCompleted takes questitem whichQuestItem returns boolean",
    ),
    (
        "FlashQuestDialogButton",
        "FlashQuestDialogButton takes nothing returns nothing",
    ),
    (
        "ForceQuestDialogUpdate",
        "ForceQuestDialogUpdate takes nothing returns nothing",
    ),
];

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
    Foreign,
}
struct FakeArena {
    slots: Vec<Slot>,
    max_handles: usize,
}
impl FakeArena {
    fn new(max_handles: usize) -> Self {
        Self {
            slots: Vec::new(),
            max_handles,
        }
    }
    fn issued(&self) -> usize {
        self.slots.len()
    }
    fn foreign(&mut self) -> Result<HandleId, QuestFailure> {
        self.push(SlotKind::Foreign)
    }
    fn push(&mut self, kind: SlotKind) -> Result<HandleId, QuestFailure> {
        if self.slots.len() >= self.max_handles {
            return Err(QuestFailure::Capacity);
        }
        self.slots
            .try_reserve(1)
            .map_err(|_| QuestFailure::Capacity)?;
        self.slots.push(Slot { kind, live: true });
        let token = u64::try_from(self.slots.len()).map_err(|_| QuestFailure::Capacity)?;
        Ok(HandleId::new(
            NonZeroU64::new(token).expect("len is nonzero"),
        ))
    }
}
impl QuestArena for FakeArena {
    fn alloc(&mut self, kind: ObjectKind) -> Result<HandleId, QuestFailure> {
        self.push(match kind {
            ObjectKind::Quest => SlotKind::Quest,
            ObjectKind::Item => SlotKind::Item,
        })
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
            (SlotKind::Foreign, _) => ObjectClass::Other,
        })
    }
    fn kill(&mut self, id: HandleId) -> Result<(), QuestFailure> {
        let slot = usize::try_from(id.token() - 1)
            .ok()
            .and_then(|index| self.slots.get_mut(index))
            .ok_or(QuestFailure::Identity)?;
        match (&slot.kind, slot.live) {
            (SlotKind::Quest | SlotKind::Item, true) => {
                slot.live = false;
                Ok(())
            }
            (SlotKind::Quest | SlotKind::Item, false) => Err(QuestFailure::Identity),
            (SlotKind::Foreign, _) => Err(QuestFailure::WrongFamily),
        }
    }
}

fn budget() -> QuestBudget {
    QuestBudget {
        max_quests: 4,
        max_items: 4,
        text_bytes: 64,
        max_events: 32,
    }
}
fn common(parent: &str) -> String {
    format!(
        "type agent extends handle\ntype quest extends {parent}\ntype questitem extends {parent}\n{NATIVES}"
    )
}
fn program(parent: &str, map: &str) -> Program {
    let common = pcx_script::parse(&common(parent)).unwrap();
    let map = pcx_script::parse(map).unwrap();
    pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap()
}
fn empty_program(parent: &str) -> Program {
    program(
        parent,
        "function main takes nothing returns nothing\nendfunction\n",
    )
}
fn open_session(parent: &str) -> (QuestSession, FakeArena) {
    let program = empty_program(parent);
    let session = QuestSession::admit(&program, budget(), &[]).unwrap();
    (session, FakeArena::new(8))
}
struct VmHost {
    session: QuestSession,
    arena: FakeArena,
}
impl Host for VmHost {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if !accepts(name) {
            return Err(format!(
                "synthetic:selected-map: native `{name}` is unsupported by this host"
            ));
        }
        self.session
            .call(&mut self.arena, name, args)
            .map_err(|error| format!("synthetic:selected-map: {name}: {error}"))
    }
}
fn host(parent: &str, map: &str) -> (Program, VmHost) {
    let program = program(parent, map);
    let session = QuestSession::admit(&program, budget(), &[]).unwrap();
    (
        program,
        VmHost {
            session,
            arena: FakeArena::new(8),
        },
    )
}

#[test]
fn shipped_signature_literals_match_the_parsed_synthetic_declarations() {
    let program = empty_program("agent");
    for (name, signature) in SHIPPED {
        let native = program.natives().get(name).unwrap();
        assert!(!native.is_constant);
        assert_eq!(
            pcx_script::natives::signature_of(&native.name, &native.params, &native.ret),
            *signature
        );
        assert!(accepts(name));
    }
    assert!(!accepts("DestroyQuestItem"));
    assert!(!accepts("SaveQuestHandle"));
    assert!(!accepts("QuestGetTitle"));
}

#[test]
fn null_host_refuses_create_quest_and_admitted_session_returns_the_handle() {
    let mut baseline = NullHost;
    let refused = baseline.native("CreateQuest", &[]).unwrap_err();
    assert!(refused.contains("no host implements the native `CreateQuest`"));
    let (mut session, mut arena) = open_session("handle");
    let created = session.call(&mut arena, "CreateQuest", &[]).unwrap();
    assert!(matches!(created, Value::Handle(_)));
    assert_eq!(session.live_quests(), 1);
    assert_eq!(
        session.events(),
        &[QuestEvent::Created(match created {
            Value::Handle(id) => id,
            _ => unreachable!(),
        })]
    );
}

#[test]
fn both_shipped_parents_admit_and_are_recorded() {
    let (agent, _) = open_session("agent");
    assert_eq!(agent.quest_parent(), "agent");
    assert_eq!(agent.item_parent(), "agent");
    let (handle, _) = open_session("handle");
    assert_eq!(handle.quest_parent(), "handle");
    assert_eq!(handle.item_parent(), "handle");
}

#[test]
fn admission_refuses_wrong_signatures_duplicate_sources_and_conversion_ancestors() {
    let mut broken = common("agent").replace(
        "native CreateQuest takes nothing returns quest",
        "constant native CreateQuest takes nothing returns integer",
    );
    let common_script = pcx_script::parse(&broken).unwrap();
    let map =
        pcx_script::parse("function main takes nothing returns nothing\nendfunction\n").unwrap();
    let program =
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common_script)])
            .unwrap();
    let error = QuestSession::admit(&program, budget(), &[]).unwrap_err();
    assert_eq!(
        error,
        QuestAdmitError {
            native: "CreateQuest",
            failure: QuestFailure::Signature,
        }
    );
    broken = common("agent");
    let first = pcx_script::parse(&broken).unwrap();
    let second = pcx_script::parse("native CreateQuest takes nothing returns quest\n").unwrap();
    let program = pcx_script::resolve_with(
        &map,
        &[
            Prelude::new("install-a:common.j", &first),
            Prelude::new("install-b:common.j", &second),
        ],
    )
    .unwrap();
    let error = QuestSession::admit(&program, budget(), &[]).unwrap_err();
    assert_eq!(error.failure, QuestFailure::Source);
    assert_eq!(error.native, "CreateQuest");
    let program = empty_program("agent");
    let error = QuestSession::admit(&program, budget(), &["agent".into()]).unwrap_err();
    assert_eq!(error.native, "quest");
    assert_eq!(error.failure, QuestFailure::Signature);
    let error = QuestSession::admit(
        &program,
        QuestBudget {
            max_quests: usize::MAX,
            max_items: 1,
            text_bytes: 1,
            max_events: 1,
        },
        &[],
    )
    .unwrap_err();
    assert_eq!(error.failure, QuestFailure::Capacity);
}

#[test]
fn budget_layouts_cover_the_stored_payloads_and_events() {
    let program = empty_program("agent");
    for field in 0..3 {
        let mut limit = budget();
        let size = match field {
            0 => size_of::<(HandleId, super::QuestPayload)>(),
            1 => size_of::<(HandleId, super::ItemPayload)>(),
            _ => size_of::<QuestEvent>(),
        };
        let count = isize::MAX as usize / size + 1;
        match field {
            0 => limit.max_quests = count,
            1 => limit.max_items = count,
            _ => limit.max_events = count,
        }
        assert_eq!(
            QuestSession::admit(&program, limit, &[])
                .unwrap_err()
                .failure,
            QuestFailure::Capacity
        );
    }
}

#[test]
fn initial_flags_match_the_original_runtime_observation() {
    for parent in ["agent", "handle"] {
        let (mut session, mut arena) = open_session(parent);
        for _ in 0..2 {
            let quest = session.call(&mut arena, "CreateQuest", &[]).unwrap();
            for (query, expected) in [
                ("IsQuestRequired", true),
                ("IsQuestCompleted", false),
                ("IsQuestDiscovered", true),
                ("IsQuestFailed", false),
                ("IsQuestEnabled", true),
            ] {
                assert_eq!(
                    session.call(&mut arena, query, std::slice::from_ref(&quest)),
                    Ok(Value::Bool(expected)),
                    "{parent}: {query}"
                );
            }
            let item = session
                .call(&mut arena, "QuestCreateItem", std::slice::from_ref(&quest))
                .unwrap();
            assert_eq!(
                session.call(&mut arena, "IsQuestItemCompleted", &[item]),
                Ok(Value::Bool(false))
            );
        }
    }
}

#[test]
fn quest_and_item_setters_preserve_both_boolean_values() {
    let (mut session, mut arena) = open_session("agent");
    let quest = session.call(&mut arena, "CreateQuest", &[]).unwrap();
    for (setter, query) in [
        ("QuestSetRequired", "IsQuestRequired"),
        ("QuestSetCompleted", "IsQuestCompleted"),
        ("QuestSetDiscovered", "IsQuestDiscovered"),
        ("QuestSetFailed", "IsQuestFailed"),
        ("QuestSetEnabled", "IsQuestEnabled"),
    ] {
        for value in [true, false] {
            session
                .call(&mut arena, setter, &[quest.clone(), Value::Bool(value)])
                .unwrap();
            assert_eq!(
                session.call(&mut arena, query, std::slice::from_ref(&quest)),
                Ok(Value::Bool(value))
            );
        }
    }
    let item = session
        .call(&mut arena, "QuestCreateItem", &[quest])
        .unwrap();
    for value in [true, false] {
        session
            .call(
                &mut arena,
                "QuestItemSetCompleted",
                &[item.clone(), Value::Bool(value)],
            )
            .unwrap();
        assert_eq!(
            session.call(
                &mut arena,
                "IsQuestItemCompleted",
                std::slice::from_ref(&item)
            ),
            Ok(Value::Bool(value))
        );
    }
}

#[test]
fn text_replacement_keeps_the_previous_value_when_the_budget_refuses() {
    let (mut session, mut arena) = open_session("agent");
    let quest = session.call(&mut arena, "CreateQuest", &[]).unwrap();
    session
        .call(
            &mut arena,
            "QuestSetTitle",
            &[quest.clone(), Value::Str("ab".into())],
        )
        .unwrap();
    let error = session
        .call(
            &mut arena,
            "QuestSetTitle",
            &[quest.clone(), Value::Str("x".repeat(65))],
        )
        .unwrap_err();
    assert_eq!(error, QuestFailure::Budget("quest text bytes"));
    assert_eq!(session.quest_title(handle(&quest)).unwrap(), Some("ab"));
    assert_eq!(session.text_used(), 2);
    session
        .call(
            &mut arena,
            "QuestSetTitle",
            &[quest.clone(), Value::Str(String::new())],
        )
        .unwrap();
    assert_eq!(session.quest_title(handle(&quest)).unwrap(), Some(""));
    assert_eq!(session.text_used(), 0);
}

#[test]
fn destruction_stales_items_and_cross_kind_handles_are_refused() {
    let (mut session, mut arena) = open_session("handle");
    let timer = arena.foreign().unwrap();
    let quest = session.call(&mut arena, "CreateQuest", &[]).unwrap();
    let item = session
        .call(&mut arena, "QuestCreateItem", std::slice::from_ref(&quest))
        .unwrap();
    let other = session.call(&mut arena, "CreateQuest", &[]).unwrap();
    let survivor = session
        .call(&mut arena, "QuestCreateItem", std::slice::from_ref(&other))
        .unwrap();
    assert_eq!(
        session
            .call(
                &mut arena,
                "QuestSetCompleted",
                &[Value::Handle(timer), Value::Bool(true)]
            )
            .unwrap_err(),
        QuestFailure::WrongFamily
    );
    assert_eq!(
        session
            .call(
                &mut arena,
                "IsQuestItemCompleted",
                std::slice::from_ref(&quest)
            )
            .unwrap_err(),
        QuestFailure::WrongFamily
    );
    assert_eq!(
        session
            .call(&mut arena, "IsQuestCompleted", std::slice::from_ref(&item))
            .unwrap_err(),
        QuestFailure::WrongFamily
    );
    assert_eq!(
        session
            .call(&mut arena, "DestroyQuest", &[Value::Null])
            .unwrap_err(),
        QuestFailure::Arguments
    );
    assert_eq!(
        session
            .call(&mut arena, "DestroyQuest", std::slice::from_ref(&quest))
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        session
            .call(&mut arena, "DestroyQuest", std::slice::from_ref(&quest))
            .unwrap_err(),
        QuestFailure::Identity
    );
    assert_eq!(
        session
            .call(
                &mut arena,
                "IsQuestItemCompleted",
                std::slice::from_ref(&item)
            )
            .unwrap_err(),
        QuestFailure::Identity
    );
    assert_eq!(
        session
            .call(&mut arena, "QuestCreateItem", std::slice::from_ref(&quest))
            .unwrap_err(),
        QuestFailure::Identity
    );
    assert_eq!(
        session
            .call(
                &mut arena,
                "IsQuestItemCompleted",
                std::slice::from_ref(&survivor)
            )
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(session.live_quests(), 1);
    assert_eq!(session.live_items(), 1);
    assert!(
        session
            .events()
            .contains(&QuestEvent::Destroyed(handle(&quest)))
    );
}

#[test]
fn destruction_releases_item_text_and_invalidates_shared_identities() {
    let (mut session, mut arena) = open_session("agent");
    let quest = handle(&session.call(&mut arena, "CreateQuest", &[]).unwrap());
    let item = session
        .call(&mut arena, "QuestCreateItem", &[Value::Handle(quest)])
        .unwrap();
    session
        .call(
            &mut arena,
            "QuestItemSetCompleted",
            &[item.clone(), Value::Bool(true)],
        )
        .unwrap();
    session
        .call(
            &mut arena,
            "QuestItemSetDescription",
            &[item.clone(), Value::Str("objective".into())],
        )
        .unwrap();
    assert_eq!(session.text_used(), "objective".len());
    session
        .call(&mut arena, "DestroyQuest", &[Value::Handle(quest)])
        .unwrap();
    assert_eq!(
        session
            .call(
                &mut arena,
                "IsQuestItemCompleted",
                std::slice::from_ref(&item)
            )
            .unwrap_err(),
        QuestFailure::Identity
    );
    assert_eq!(arena.classify(quest).unwrap(), ObjectClass::DeadQuest);
    assert_eq!(
        arena.classify(handle(&item)).unwrap(),
        ObjectClass::DeadItem
    );
    assert_eq!(session.live_quests(), 0);
    assert_eq!(session.live_items(), 0);
    assert_eq!(session.text_used(), 0);
}

#[test]
fn event_budget_refuses_before_changing_state_or_issuing_a_handle() {
    let program = empty_program("agent");
    let mut session = QuestSession::admit(
        &program,
        QuestBudget {
            max_quests: 2,
            max_items: 2,
            text_bytes: 32,
            max_events: 0,
        },
        &[],
    )
    .unwrap();
    let mut arena = FakeArena::new(4);
    assert_eq!(
        session.call(&mut arena, "CreateQuest", &[]).unwrap_err(),
        QuestFailure::Budget("quest events")
    );
    assert_eq!(arena.issued(), 0);
    assert!(session.events().is_empty());
    let mut session = QuestSession::admit(
        &program,
        QuestBudget {
            max_quests: 1,
            max_items: 1,
            text_bytes: 8,
            max_events: 1,
        },
        &[],
    )
    .unwrap();
    let quest = session.call(&mut arena, "CreateQuest", &[]).unwrap();
    assert_eq!(
        session
            .call(
                &mut arena,
                "QuestSetRequired",
                &[quest.clone(), Value::Bool(false)]
            )
            .unwrap_err(),
        QuestFailure::Budget("quest events")
    );
    assert_eq!(
        session.quest_flag(handle(&quest), QuestFlag::Required),
        Ok(true)
    );
    let before = session.events().len();
    session
        .call(&mut arena, "IsQuestRequired", std::slice::from_ref(&quest))
        .unwrap();
    assert_eq!(session.events().len(), before);
}

#[test]
fn shared_handle_capacity_and_dialog_natives_preserve_quest_flags() {
    let (mut session, mut arena) = open_session("agent");
    let mut tight = FakeArena::new(1);
    tight.foreign().unwrap();
    assert_eq!(
        session.call(&mut tight, "CreateQuest", &[]).unwrap_err(),
        QuestFailure::Capacity
    );
    assert_eq!(session.live_quests(), 0);
    let quest = session.call(&mut arena, "CreateQuest", &[]).unwrap();
    assert_eq!(
        session
            .call(&mut arena, "FlashQuestDialogButton", &[Value::Int(1)])
            .unwrap_err(),
        QuestFailure::Arguments
    );
    assert_eq!(
        session
            .call(&mut arena, "ForceQuestDialogUpdate", &[])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        session
            .call(&mut arena, "FlashQuestDialogButton", &[])
            .unwrap(),
        Value::Null
    );
    assert_eq!(
        session.quest_flag(handle(&quest), QuestFlag::Discovered),
        Ok(true)
    );
    assert!(session.events().contains(&QuestEvent::DialogUpdate));
    assert!(session.events().contains(&QuestEvent::DialogButtonFlash));
}

#[test]
fn synthetic_chapter_observes_quest_completion_for_both_declared_parents() {
    let map = "
globals
quest chapterQuest = null
questitem objective = null
endglobals
function config takes nothing returns nothing
endfunction
function main takes nothing returns nothing
set chapterQuest = CreateQuest()
call QuestSetTitle(chapterQuest, \"Hold\")
call QuestSetDescription(chapterQuest, \"Hold the ridge\")
call QuestSetIconPath(chapterQuest, \"synthetic-icon\")
call QuestSetDiscovered(chapterQuest, true)
call QuestSetRequired(chapterQuest, true)
call QuestSetEnabled(chapterQuest, true)
call QuestSetFailed(chapterQuest, false)
call QuestSetCompleted(chapterQuest, false)
set objective = QuestCreateItem(chapterQuest)
call QuestItemSetDescription(objective, \"Survive\")
call QuestItemSetCompleted(objective, false)
call ForceQuestDialogUpdate()
endfunction
function Dialogue takes nothing returns boolean
return chapterQuest != null
endfunction
function Quest takes nothing returns boolean
return IsQuestDiscovered(chapterQuest) and IsQuestRequired(chapterQuest) and not IsQuestCompleted(chapterQuest) and not IsQuestItemCompleted(objective)
endfunction
function Advance takes nothing returns nothing
call QuestSetCompleted(chapterQuest, true)
call QuestItemSetCompleted(objective, true)
call FlashQuestDialogButton()
endfunction
function Victory takes nothing returns boolean
return IsQuestCompleted(chapterQuest) and IsQuestItemCompleted(objective)
endfunction
function EndGame takes nothing returns boolean
return IsQuestEnabled(chapterQuest) and not IsQuestFailed(chapterQuest)
endfunction
";
    for parent in ["agent", "handle"] {
        let (program, host) = host(parent, map);
        let mut vm = Vm::new(&program, host);
        vm.call("config", &[]).unwrap();
        vm.call("main", &[]).unwrap();
        assert_eq!(vm.call("Dialogue", &[]).unwrap(), Value::Bool(true));
        assert_eq!(vm.call("Quest", &[]).unwrap(), Value::Bool(true));
        vm.call("Advance", &[]).unwrap();
        assert_eq!(vm.call("Victory", &[]).unwrap(), Value::Bool(true));
        assert_eq!(vm.call("EndGame", &[]).unwrap(), Value::Bool(true));
        let events = vm.host().session.events();
        assert!(matches!(events[0], QuestEvent::Created(_)));
        assert!(events.contains(&QuestEvent::DialogUpdate));
        assert!(events.contains(&QuestEvent::DialogButtonFlash));
        assert!(
            events
                .iter()
                .any(|event| matches!(event, QuestEvent::Completed(_, true)))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, QuestEvent::ItemCompleted(_, true)))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, QuestEvent::Discovered(_, true)))
        );
    }
}

#[test]
fn initial_failed_query_runs_through_the_vm() {
    let map = "
globals
quest chapterQuest = null
endglobals
function main takes nothing returns nothing
set chapterQuest = CreateQuest()
endfunction
function EndGame takes nothing returns boolean
return not IsQuestFailed(chapterQuest)
endfunction
";
    let (program, host) = host("agent", map);
    let mut vm = Vm::new(&program, host);
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.call("EndGame", &[]).unwrap(), Value::Bool(true));
}

#[test]
fn renamed_formal_parameters_still_create_query_and_destroy() {
    let renamed = NATIVES
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
    assert!(!renamed.contains("whichQuest"));
    let common = pcx_script::parse(&common_renamed(&renamed)).unwrap();
    let map =
        pcx_script::parse("function main takes nothing returns nothing\nendfunction\n").unwrap();
    let program =
        pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap();
    let mut session = QuestSession::admit(&program, budget(), &[]).unwrap();
    let mut arena = FakeArena::new(4);
    let quest = session.call(&mut arena, "CreateQuest", &[]).unwrap();
    assert_eq!(
        session
            .call(
                &mut arena,
                "IsQuestDiscovered",
                std::slice::from_ref(&quest)
            )
            .unwrap(),
        Value::Bool(true)
    );
    let other = arena.foreign().unwrap();
    assert_eq!(
        session
            .call(
                &mut arena,
                "QuestSetDiscovered",
                &[Value::Handle(other), Value::Bool(true)]
            )
            .unwrap_err(),
        QuestFailure::WrongFamily
    );
    session
        .call(
            &mut arena,
            "QuestSetTitle",
            &[quest.clone(), Value::Str("Hold".into())],
        )
        .unwrap();
    assert_eq!(session.quest_title(handle(&quest)).unwrap(), Some("Hold"));
    let item = session
        .call(&mut arena, "QuestCreateItem", std::slice::from_ref(&quest))
        .unwrap();
    session
        .call(&mut arena, "DestroyQuest", std::slice::from_ref(&quest))
        .unwrap();
    assert_eq!(
        session
            .call(
                &mut arena,
                "IsQuestItemCompleted",
                std::slice::from_ref(&item)
            )
            .unwrap_err(),
        QuestFailure::Identity
    );
}

fn common_renamed(natives: &str) -> String {
    format!(
        "type agent extends handle\ntype quest extends handle\ntype questitem extends handle\n{natives}"
    )
}

fn handle(value: &Value) -> HandleId {
    match value {
        Value::Handle(id) => *id,
        _ => panic!("quest native did not return a handle"),
    }
}

#[test]
fn null_boolean_setters_preserve_state_and_do_not_consume_event_or_handle_budget() {
    let (mut session, mut arena) = open_session("agent");
    let quest = handle(&session.call(&mut arena, "CreateQuest", &[]).unwrap());
    let events = session.events().to_vec();
    let issued = arena.issued();
    session.budget.max_events = 0;
    for native in [
        "QuestSetRequired",
        "QuestSetCompleted",
        "QuestSetDiscovered",
        "QuestSetFailed",
        "QuestSetEnabled",
        "QuestItemSetCompleted",
    ] {
        for value in [false, true] {
            assert_eq!(
                session.call(&mut arena, native, &[Value::Null, Value::Bool(value)]),
                Ok(Value::Null)
            );
        }
        for args in [
            vec![],
            vec![Value::Null],
            vec![Value::Null, Value::Int(1)],
            vec![Value::Null, Value::Bool(true), Value::Bool(false)],
        ] {
            assert_eq!(
                session.call(&mut arena, native, &args),
                Err(QuestFailure::Arguments)
            );
        }
    }
    assert_eq!(arena.issued(), issued);
    assert_eq!(session.events(), events);
    assert_eq!(session.live_quests(), 1);
    assert_eq!(session.live_items(), 0);
    assert_eq!(session.quest_flag(quest, QuestFlag::Completed), Ok(false));
    assert_eq!(session.text_used(), 0);
}

#[test]
fn the_vm_continues_after_completing_an_unassigned_quest() {
    let (program, host) = host(
        "agent",
        r#"
globals
quest unassigned = null
integer continued = 0
endglobals
function main takes nothing returns nothing
call QuestSetCompleted(unassigned, true)
set continued = 9
endfunction
"#,
    );
    let mut vm = Vm::new(&program, host);
    vm.init_globals().unwrap();
    vm.call("main", &[]).unwrap();
    assert_eq!(vm.global("continued"), Value::Int(9));
    assert!(vm.host().session.events().is_empty());
    assert_eq!(vm.host().arena.issued(), 0);
}
