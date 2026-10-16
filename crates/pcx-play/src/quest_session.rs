//! Quest and quest-item state for the declared JASS quest natives.
//!
//! The selected local `common.j` files agree on these native signatures. One
//! parents `quest` and `questitem` on `agent`; the other parents both directly
//! on `handle`. Admission accepts either chain when it reaches `handle`.
//!
//! Initial flags follow the pinned Legacy runtime probe documented in
//! `docs/re-notes/script-quests.md`; text stays absent until assigned.
//! There is no declared getter for the string fields and no declared
//! `DestroyQuestItem`; destroying a quest
//! makes its items stale. `SaveQuestHandle` / `LoadQuestHandle` and defeat
//! conditions are outside this session.
//!
//! Live-object counts, retained text bytes and the observation log are caller
//! budgets, not values read from the scripts. Handle tokens come from the
//! supplied arena so one host can share them with other object families.

use pcx_script::{HandleId, Program, Value};

const NATIVES: &[(&str, &[&str], &str)] = &[
    ("CreateQuest", &[], "quest"),
    ("DestroyQuest", &["quest"], "nothing"),
    ("QuestSetTitle", &["quest", "string"], "nothing"),
    ("QuestSetDescription", &["quest", "string"], "nothing"),
    ("QuestSetIconPath", &["quest", "string"], "nothing"),
    ("QuestSetRequired", &["quest", "boolean"], "nothing"),
    ("QuestSetCompleted", &["quest", "boolean"], "nothing"),
    ("QuestSetDiscovered", &["quest", "boolean"], "nothing"),
    ("QuestSetFailed", &["quest", "boolean"], "nothing"),
    ("QuestSetEnabled", &["quest", "boolean"], "nothing"),
    ("IsQuestRequired", &["quest"], "boolean"),
    ("IsQuestCompleted", &["quest"], "boolean"),
    ("IsQuestDiscovered", &["quest"], "boolean"),
    ("IsQuestFailed", &["quest"], "boolean"),
    ("IsQuestEnabled", &["quest"], "boolean"),
    ("QuestCreateItem", &["quest"], "questitem"),
    (
        "QuestItemSetDescription",
        &["questitem", "string"],
        "nothing",
    ),
    (
        "QuestItemSetCompleted",
        &["questitem", "boolean"],
        "nothing",
    ),
    ("IsQuestItemCompleted", &["questitem"], "boolean"),
    ("FlashQuestDialogButton", &[], "nothing"),
    ("ForceQuestDialogUpdate", &[], "nothing"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Quest,
    Item,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectClass {
    Quest,
    Item,
    DeadQuest,
    DeadItem,
    Other,
}

pub trait QuestArena {
    fn alloc(&mut self, kind: ObjectKind) -> Result<HandleId, QuestFailure>;
    fn classify(&self, id: HandleId) -> Result<ObjectClass, QuestFailure>;
    fn kill(&mut self, id: HandleId) -> Result<(), QuestFailure>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestBudget {
    pub max_quests: usize,
    pub max_items: usize,
    pub text_bytes: usize,
    pub max_events: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuestEvent {
    Created(HandleId),
    Destroyed(HandleId),
    TitleSet(HandleId),
    DescriptionSet(HandleId),
    IconPathSet(HandleId),
    Required(HandleId, bool),
    Completed(HandleId, bool),
    Discovered(HandleId, bool),
    Failed(HandleId, bool),
    Enabled(HandleId, bool),
    ItemCreated { quest: HandleId, item: HandleId },
    ItemDescription(HandleId),
    ItemCompleted(HandleId, bool),
    DialogButtonFlash,
    DialogUpdate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuestFlag {
    Required,
    Completed,
    Discovered,
    Failed,
    Enabled,
}

#[derive(Debug, PartialEq, Eq)]
pub enum QuestFailure {
    Arguments,
    Identity,
    WrongFamily,
    Budget(&'static str),
    Capacity,
    Signature,
    Source,
    State(&'static str),
}

impl std::fmt::Display for QuestFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Arguments => write!(f, "arguments do not match the quest native declaration"),
            Self::Identity => write!(f, "unknown handle identity"),
            Self::WrongFamily => write!(f, "handle identifies another family"),
            Self::Budget(what) => write!(f, "{what} policy exceeded"),
            Self::Capacity => write!(f, "host identity capacity or allocation exceeded"),
            Self::Signature => write!(f, "missing or incompatible native/type declaration"),
            Self::Source => write!(f, "multiple declaration sources require qualification"),
            Self::State(detail) => write!(f, "quest state: {detail}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct QuestAdmitError {
    pub native: &'static str,
    pub failure: QuestFailure,
}

#[derive(Debug)]
struct QuestPayload {
    title: Option<String>,
    description: Option<String>,
    icon_path: Option<String>,
    required: bool,
    completed: bool,
    discovered: bool,
    failed: bool,
    enabled: bool,
    items: Vec<HandleId>,
}

#[derive(Debug)]
struct ItemPayload {
    quest: HandleId,
    description: Option<String>,
    completed: bool,
}

#[derive(Debug)]
pub struct QuestSession {
    budget: QuestBudget,
    text_used: usize,
    quests: Vec<(HandleId, QuestPayload)>,
    items: Vec<(HandleId, ItemPayload)>,
    events: Vec<QuestEvent>,
    quest_parent: String,
    item_parent: String,
}

impl QuestPayload {
    fn empty() -> Self {
        Self {
            title: None,
            description: None,
            icon_path: None,
            required: true,
            completed: false,
            discovered: true,
            failed: false,
            enabled: true,
            items: Vec::new(),
        }
    }
}

pub fn accepts(name: &str) -> bool {
    NATIVES.iter().any(|native| native.0 == name)
}

impl QuestSession {
    pub fn admit(
        program: &Program,
        budget: QuestBudget,
        conversion_kinds: &[String],
    ) -> Result<Self, QuestAdmitError> {
        representable(&budget)?;
        for (name, parameters, result) in NATIVES {
            let native = program.natives().get(name).ok_or(QuestAdmitError {
                native: name,
                failure: QuestFailure::Signature,
            })?;
            if native.is_constant
                || program.function(name).is_some()
                || native.ret.array
                || native.ret.kind.name() != *result
                || native.params.len() != parameters.len()
                || native
                    .params
                    .iter()
                    .zip(*parameters)
                    .any(|(param, expected)| param.ty.array || param.ty.kind.name() != *expected)
                || native
                    .also_declared_in
                    .iter()
                    .any(|(source, _)| source != &native.source)
            {
                let failure = if native
                    .also_declared_in
                    .iter()
                    .any(|(source, _)| source != &native.source)
                {
                    QuestFailure::Source
                } else {
                    QuestFailure::Signature
                };
                return Err(QuestAdmitError {
                    native: name,
                    failure,
                });
            }
        }
        let quest_parent = handle_parent(program, "quest", conversion_kinds)?;
        let item_parent = handle_parent(program, "questitem", conversion_kinds)?;
        Ok(Self {
            budget,
            text_used: 0,
            quests: Vec::new(),
            items: Vec::new(),
            events: Vec::new(),
            quest_parent,
            item_parent,
        })
    }

    pub fn quest_parent(&self) -> &str {
        &self.quest_parent
    }

    pub fn item_parent(&self) -> &str {
        &self.item_parent
    }

    pub fn events(&self) -> &[QuestEvent] {
        &self.events
    }

    pub fn text_used(&self) -> usize {
        self.text_used
    }

    pub fn live_quests(&self) -> usize {
        self.quests.len()
    }

    pub fn live_items(&self) -> usize {
        self.items.len()
    }

    pub fn quest_title(&self, id: HandleId) -> Result<Option<&str>, QuestFailure> {
        Ok(self.quest_ref(id)?.title.as_deref())
    }

    pub fn quest_flag(&self, id: HandleId, flag: QuestFlag) -> Result<bool, QuestFailure> {
        Ok(flag_value(self.quest_ref(id)?, flag))
    }

    pub fn call<A: QuestArena>(
        &mut self,
        arena: &mut A,
        name: &str,
        args: &[Value],
    ) -> Result<Value, QuestFailure> {
        match name {
            "CreateQuest" => self.create_quest(arena, args),
            "DestroyQuest" => self.destroy_quest(arena, args),
            "QuestSetTitle" => self.set_quest_text(arena, args, TextField::Title),
            "QuestSetDescription" => self.set_quest_text(arena, args, TextField::Description),
            "QuestSetIconPath" => self.set_quest_text(arena, args, TextField::Icon),
            "QuestSetRequired" => self.set_flag(arena, args, QuestFlag::Required),
            "QuestSetCompleted" => self.set_flag(arena, args, QuestFlag::Completed),
            "QuestSetDiscovered" => self.set_flag(arena, args, QuestFlag::Discovered),
            "QuestSetFailed" => self.set_flag(arena, args, QuestFlag::Failed),
            "QuestSetEnabled" => self.set_flag(arena, args, QuestFlag::Enabled),
            "IsQuestRequired" => self.query_flag(arena, args, QuestFlag::Required),
            "IsQuestCompleted" => self.query_flag(arena, args, QuestFlag::Completed),
            "IsQuestDiscovered" => self.query_flag(arena, args, QuestFlag::Discovered),
            "IsQuestFailed" => self.query_flag(arena, args, QuestFlag::Failed),
            "IsQuestEnabled" => self.query_flag(arena, args, QuestFlag::Enabled),
            "QuestCreateItem" => self.create_item(arena, args),
            "QuestItemSetDescription" => self.set_item_text(arena, args),
            "QuestItemSetCompleted" => self.set_item_completed(arena, args),
            "IsQuestItemCompleted" => self.query_item_completed(arena, args),
            "FlashQuestDialogButton" => self.dialog(args, QuestEvent::DialogButtonFlash),
            "ForceQuestDialogUpdate" => self.dialog(args, QuestEvent::DialogUpdate),
            _ => Err(QuestFailure::Signature),
        }
    }

    fn create_quest<A: QuestArena>(
        &mut self,
        arena: &mut A,
        args: &[Value],
    ) -> Result<Value, QuestFailure> {
        if !args.is_empty() {
            return Err(QuestFailure::Arguments);
        }
        if self.quests.len() >= self.budget.max_quests {
            return Err(QuestFailure::Budget("quest objects"));
        }
        self.reserve_event()?;
        self.quests
            .try_reserve(1)
            .map_err(|_| QuestFailure::Capacity)?;
        let id = arena.alloc(ObjectKind::Quest)?;
        self.quests.push((id, QuestPayload::empty()));
        self.events.push(QuestEvent::Created(id));
        Ok(Value::Handle(id))
    }

    fn destroy_quest<A: QuestArena>(
        &mut self,
        arena: &mut A,
        args: &[Value],
    ) -> Result<Value, QuestFailure> {
        let id = one_handle(args)?;
        match arena.classify(id)? {
            ObjectClass::Quest => {}
            ObjectClass::DeadQuest => return Err(QuestFailure::Identity),
            _ => return Err(QuestFailure::WrongFamily),
        }
        self.reserve_event()?;
        let item_ids = &self.quest_ref(id)?.items;
        for item in item_ids {
            if self.item_ref(*item)?.quest != id {
                return Err(QuestFailure::State("quest item parent mismatch"));
            }
        }
        for item in item_ids {
            arena.kill(*item)?;
        }
        arena.kill(id)?;
        let quest = self.take_quest(id)?;
        for item in quest.items {
            let payload = self.take_item(item)?;
            release_text(&mut self.text_used, payload.description.as_ref());
        }
        release_text(&mut self.text_used, quest.title.as_ref());
        release_text(&mut self.text_used, quest.description.as_ref());
        release_text(&mut self.text_used, quest.icon_path.as_ref());
        self.events.push(QuestEvent::Destroyed(id));
        Ok(Value::Null)
    }

    fn set_quest_text<A: QuestArena>(
        &mut self,
        arena: &mut A,
        args: &[Value],
        field: TextField,
    ) -> Result<Value, QuestFailure> {
        let (id, text) = handle_and_string(args)?;
        self.require_live_quest(arena, id)?;
        self.reserve_event()?;
        let budget = self.budget.text_bytes;
        let mut used = self.text_used;
        {
            let quest = self.quest_mut(id)?;
            let slot = match field {
                TextField::Title => &mut quest.title,
                TextField::Description => &mut quest.description,
                TextField::Icon => &mut quest.icon_path,
            };
            store_text(&mut used, budget, slot, text)?;
        }
        self.text_used = used;
        self.events.push(match field {
            TextField::Title => QuestEvent::TitleSet(id),
            TextField::Description => QuestEvent::DescriptionSet(id),
            TextField::Icon => QuestEvent::IconPathSet(id),
        });
        Ok(Value::Null)
    }

    fn set_flag<A: QuestArena>(
        &mut self,
        arena: &mut A,
        args: &[Value],
        flag: QuestFlag,
    ) -> Result<Value, QuestFailure> {
        let (id, value) = handle_and_bool(args)?;
        self.require_live_quest(arena, id)?;
        self.reserve_event()?;
        *flag_slot(self.quest_mut(id)?, flag) = value;
        self.events.push(match flag {
            QuestFlag::Required => QuestEvent::Required(id, value),
            QuestFlag::Completed => QuestEvent::Completed(id, value),
            QuestFlag::Discovered => QuestEvent::Discovered(id, value),
            QuestFlag::Failed => QuestEvent::Failed(id, value),
            QuestFlag::Enabled => QuestEvent::Enabled(id, value),
        });
        Ok(Value::Null)
    }

    fn query_flag<A: QuestArena>(
        &self,
        arena: &A,
        args: &[Value],
        flag: QuestFlag,
    ) -> Result<Value, QuestFailure> {
        let id = one_handle(args)?;
        self.require_live_quest(arena, id)?;
        Ok(Value::Bool(flag_value(self.quest_ref(id)?, flag)))
    }

    fn create_item<A: QuestArena>(
        &mut self,
        arena: &mut A,
        args: &[Value],
    ) -> Result<Value, QuestFailure> {
        let quest_id = one_handle(args)?;
        self.require_live_quest(arena, quest_id)?;
        if self.items.len() >= self.budget.max_items {
            return Err(QuestFailure::Budget("quest items"));
        }
        self.reserve_event()?;
        self.items
            .try_reserve(1)
            .map_err(|_| QuestFailure::Capacity)?;
        self.quest_mut(quest_id)?
            .items
            .try_reserve(1)
            .map_err(|_| QuestFailure::Capacity)?;
        let item = arena.alloc(ObjectKind::Item)?;
        self.items.push((
            item,
            ItemPayload {
                quest: quest_id,
                description: None,
                completed: false,
            },
        ));
        self.quest_mut(quest_id)?.items.push(item);
        self.events.push(QuestEvent::ItemCreated {
            quest: quest_id,
            item,
        });
        Ok(Value::Handle(item))
    }

    fn set_item_text<A: QuestArena>(
        &mut self,
        arena: &mut A,
        args: &[Value],
    ) -> Result<Value, QuestFailure> {
        let (id, text) = handle_and_string(args)?;
        self.require_live_item(arena, id)?;
        self.reserve_event()?;
        let budget = self.budget.text_bytes;
        let mut used = self.text_used;
        {
            let item = self.item_mut(id)?;
            store_text(&mut used, budget, &mut item.description, text)?;
        }
        self.text_used = used;
        self.events.push(QuestEvent::ItemDescription(id));
        Ok(Value::Null)
    }

    fn set_item_completed<A: QuestArena>(
        &mut self,
        arena: &mut A,
        args: &[Value],
    ) -> Result<Value, QuestFailure> {
        let (id, value) = handle_and_bool(args)?;
        self.require_live_item(arena, id)?;
        self.reserve_event()?;
        self.item_mut(id)?.completed = value;
        self.events.push(QuestEvent::ItemCompleted(id, value));
        Ok(Value::Null)
    }

    fn query_item_completed<A: QuestArena>(
        &self,
        arena: &A,
        args: &[Value],
    ) -> Result<Value, QuestFailure> {
        let id = one_handle(args)?;
        self.require_live_item(arena, id)?;
        Ok(Value::Bool(self.item_ref(id)?.completed))
    }

    fn dialog(&mut self, args: &[Value], event: QuestEvent) -> Result<Value, QuestFailure> {
        if !args.is_empty() {
            return Err(QuestFailure::Arguments);
        }
        self.reserve_event()?;
        self.events.push(event);
        Ok(Value::Null)
    }

    fn require_live_quest<A: QuestArena>(
        &self,
        arena: &A,
        id: HandleId,
    ) -> Result<(), QuestFailure> {
        match arena.classify(id)? {
            ObjectClass::Quest => Ok(()),
            ObjectClass::DeadQuest => Err(QuestFailure::Identity),
            _ => Err(QuestFailure::WrongFamily),
        }
    }

    fn require_live_item<A: QuestArena>(
        &self,
        arena: &A,
        id: HandleId,
    ) -> Result<(), QuestFailure> {
        match arena.classify(id)? {
            ObjectClass::Item => Ok(()),
            ObjectClass::DeadItem => Err(QuestFailure::Identity),
            _ => Err(QuestFailure::WrongFamily),
        }
    }

    fn quest_ref(&self, id: HandleId) -> Result<&QuestPayload, QuestFailure> {
        self.quests
            .iter()
            .find(|(quest, _)| *quest == id)
            .map(|(_, payload)| payload)
            .ok_or(QuestFailure::Identity)
    }

    fn quest_mut(&mut self, id: HandleId) -> Result<&mut QuestPayload, QuestFailure> {
        self.quests
            .iter_mut()
            .find(|(quest, _)| *quest == id)
            .map(|(_, payload)| payload)
            .ok_or(QuestFailure::Identity)
    }

    fn take_quest(&mut self, id: HandleId) -> Result<QuestPayload, QuestFailure> {
        let index = self
            .quests
            .iter()
            .position(|(quest, _)| *quest == id)
            .ok_or(QuestFailure::Identity)?;
        Ok(self.quests.remove(index).1)
    }

    fn item_ref(&self, id: HandleId) -> Result<&ItemPayload, QuestFailure> {
        self.items
            .iter()
            .find(|(item, _)| *item == id)
            .map(|(_, payload)| payload)
            .ok_or(QuestFailure::Identity)
    }

    fn item_mut(&mut self, id: HandleId) -> Result<&mut ItemPayload, QuestFailure> {
        self.items
            .iter_mut()
            .find(|(item, _)| *item == id)
            .map(|(_, payload)| payload)
            .ok_or(QuestFailure::Identity)
    }

    fn take_item(&mut self, id: HandleId) -> Result<ItemPayload, QuestFailure> {
        let index = self
            .items
            .iter()
            .position(|(item, _)| *item == id)
            .ok_or(QuestFailure::Identity)?;
        Ok(self.items.remove(index).1)
    }

    fn reserve_event(&mut self) -> Result<(), QuestFailure> {
        if self.events.len() >= self.budget.max_events {
            return Err(QuestFailure::Budget("quest events"));
        }
        self.events
            .try_reserve(1)
            .map_err(|_| QuestFailure::Capacity)?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum TextField {
    Title,
    Description,
    Icon,
}

fn flag_value(quest: &QuestPayload, flag: QuestFlag) -> bool {
    match flag {
        QuestFlag::Required => quest.required,
        QuestFlag::Completed => quest.completed,
        QuestFlag::Discovered => quest.discovered,
        QuestFlag::Failed => quest.failed,
        QuestFlag::Enabled => quest.enabled,
    }
}

fn flag_slot(quest: &mut QuestPayload, flag: QuestFlag) -> &mut bool {
    match flag {
        QuestFlag::Required => &mut quest.required,
        QuestFlag::Completed => &mut quest.completed,
        QuestFlag::Discovered => &mut quest.discovered,
        QuestFlag::Failed => &mut quest.failed,
        QuestFlag::Enabled => &mut quest.enabled,
    }
}

fn representable(budget: &QuestBudget) -> Result<(), QuestAdmitError> {
    let layouts = [
        std::alloc::Layout::array::<(HandleId, QuestPayload)>(budget.max_quests),
        std::alloc::Layout::array::<(HandleId, ItemPayload)>(budget.max_items),
        std::alloc::Layout::array::<u8>(budget.text_bytes),
        std::alloc::Layout::array::<QuestEvent>(budget.max_events),
    ];
    if layouts.into_iter().any(|layout| layout.is_err()) {
        return Err(QuestAdmitError {
            native: "CreateQuest",
            failure: QuestFailure::Capacity,
        });
    }
    Ok(())
}

fn handle_parent(
    program: &Program,
    name: &'static str,
    conversion_kinds: &[String],
) -> Result<String, QuestAdmitError> {
    let fail = |failure| QuestAdmitError {
        native: name,
        failure,
    };
    let declared = program
        .types()
        .find(|ty| ty.name == name)
        .ok_or_else(|| fail(QuestFailure::Signature))?;
    if !reaches_handle(program, name)
        || conversion_kinds
            .iter()
            .any(|kind| below(program, name, kind))
    {
        return Err(fail(QuestFailure::Signature));
    }
    Ok(declared.parent.clone())
}

fn reaches_handle<'a>(program: &'a Program, name: &'a str) -> bool {
    let mut current: &'a str = name;
    for _ in 0..=program.types().count() {
        if current == "handle" {
            return true;
        }
        let Some(ty) = program.types().find(|ty| ty.name == current) else {
            return false;
        };
        current = &ty.parent;
    }
    false
}

fn below<'a>(program: &'a Program, mut name: &'a str, ancestor: &str) -> bool {
    for _ in 0..=program.types().count() {
        if name == ancestor {
            return true;
        }
        let Some(ty) = program.types().find(|ty| ty.name == name) else {
            return false;
        };
        name = &ty.parent;
    }
    false
}

fn one_handle(args: &[Value]) -> Result<HandleId, QuestFailure> {
    match args {
        [Value::Handle(id)] => Ok(*id),
        _ => Err(QuestFailure::Arguments),
    }
}

fn handle_and_string(args: &[Value]) -> Result<(HandleId, &str), QuestFailure> {
    match args {
        [Value::Handle(id), Value::Str(text)] => Ok((*id, text)),
        _ => Err(QuestFailure::Arguments),
    }
}

fn handle_and_bool(args: &[Value]) -> Result<(HandleId, bool), QuestFailure> {
    match args {
        [Value::Handle(id), Value::Bool(value)] => Ok((*id, *value)),
        _ => Err(QuestFailure::Arguments),
    }
}

fn store_text(
    used: &mut usize,
    budget: usize,
    slot: &mut Option<String>,
    next: &str,
) -> Result<(), QuestFailure> {
    let old = slot.as_ref().map_or(0, String::len);
    let next_used = used
        .checked_sub(old)
        .and_then(|value| value.checked_add(next.len()))
        .ok_or(QuestFailure::Capacity)?;
    if next_used > budget {
        return Err(QuestFailure::Budget("quest text bytes"));
    }
    let mut owned = String::new();
    owned
        .try_reserve(next.len())
        .map_err(|_| QuestFailure::Capacity)?;
    owned.push_str(next);
    *slot = Some(owned);
    *used = next_used;
    Ok(())
}

fn release_text(used: &mut usize, text: Option<&String>) {
    if let Some(text) = text {
        *used = used.saturating_sub(text.len());
    }
}

#[cfg(test)]
#[path = "quest_session_tests.rs"]
mod tests;
