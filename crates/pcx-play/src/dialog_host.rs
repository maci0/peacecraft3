//! Executed script dialogs on the shared handle arena. Bounds are caller policy.
use super::{Entry, Error, EventSubject, Failure, ObjectHost};
use pcx_script::{HandleId, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub struct DialogBudget {
    pub max_dialogs: usize,
    pub max_buttons: usize,
    pub text_bytes: usize,
    pub max_events: usize,
    pub max_quit_requests: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogButton {
    pub id: HandleId,
    pub text: String,
    pub hotkey: i32,
    /// Executed DialogAddQuitButton's doScoreScreen argument, absent for normal buttons.
    pub quit_score_screen: Option<bool>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptDialog {
    pub id: HandleId,
    /// Unassigned stays absent; an explicitly assigned empty caption stays Some("").
    pub message: Option<String>,
    pub buttons: Vec<DialogButton>,
    pub shown_revision: u64,
    visible_to: BTreeSet<i32>,
}
impl ScriptDialog {
    pub fn visible_to(&self, player_number: i32) -> bool {
        self.visible_to.contains(&player_number)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogQuitRequest {
    pub player: HandleId,
    pub dialog: HandleId,
    pub button: HandleId,
    pub score_screen: bool,
}
pub struct DialogState {
    budget: DialogBudget,
    dialogs: BTreeMap<HandleId, ScriptDialog>,
    text_bytes: usize,
    buttons: usize,
    events: usize,
    revision: u64,
    quit_requests: Vec<DialogQuitRequest>,
    dialog_event: Option<HandleId>,
    button_event: Option<HandleId>,
    layout: Option<crate::dialog_layout::Rules>,
}
impl DialogState {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn dialogs(&self) -> impl Iterator<Item = &ScriptDialog> {
        self.dialogs.values()
    }
    pub fn get(&self, id: HandleId) -> Option<&ScriptDialog> {
        self.dialogs.get(&id)
    }
    pub fn quit_requests(&self) -> &[DialogQuitRequest] {
        &self.quit_requests
    }
}
const CREATE: &str = "DialogCreate";
const NATIVES: &[(&str, &[&str], &str, bool)] = &[
    (CREATE, &[], "dialog", false),
    ("DialogDestroy", &["dialog"], "nothing", false),
    ("DialogClear", &["dialog"], "nothing", false),
    ("DialogSetMessage", &["dialog", "string"], "nothing", false),
    (
        "DialogAddButton",
        &["dialog", "string", "integer"],
        "button",
        false,
    ),
    (
        "DialogAddQuitButton",
        &["dialog", "boolean", "string", "integer"],
        "button",
        false,
    ),
    (
        "DialogDisplay",
        &["player", "dialog", "boolean"],
        "nothing",
        false,
    ),
    (
        "TriggerRegisterDialogEvent",
        &["trigger", "dialog"],
        "event",
        false,
    ),
    (
        "TriggerRegisterDialogButtonEvent",
        &["trigger", "button"],
        "event",
        false,
    ),
    ("GetClickedDialog", &[], "dialog", true),
    ("GetClickedButton", &[], "button", true),
];

impl ObjectHost {
    /// Missing APIs stay unsupported. Present declarations must share the selected timer source.
    pub fn with_dialogs(mut self, budget: DialogBudget) -> Result<Self, Error> {
        let fail = |why| self.inner.error(CREATE, why);
        if self.dialogs.is_some() {
            return Err(fail(Failure::DialogState("already admitted")));
        }
        std::alloc::Layout::array::<ScriptDialog>(budget.max_dialogs)
            .and_then(|_| std::alloc::Layout::array::<DialogButton>(budget.max_buttons))
            .and_then(|_| std::alloc::Layout::array::<DialogQuitRequest>(budget.max_quit_requests))
            .and_then(|_| {
                std::alloc::Layout::array::<(HandleId, HandleId, Option<HandleId>)>(
                    budget.max_events,
                )
            })
            .map_err(|_| fail(Failure::Capacity))?;
        let source = self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source
            .clone();
        let mut found = false;
        for (name, parameters, result, constant) in NATIVES {
            if self.inner.program.natives().get(name).is_none()
                && self.inner.program.natives().ambiguous(name).is_none()
            {
                continue;
            }
            Self::contract_in(
                &self.inner,
                name,
                parameters,
                result,
                *constant,
                None,
                false,
            )?;
            if self
                .inner
                .program
                .natives()
                .get(name)
                .is_none_or(|n| n.source != source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
            for family in parameters
                .iter()
                .copied()
                .chain(std::iter::once(*result))
                .filter(|f| matches!(*f, "dialog" | "button" | "player" | "trigger" | "event"))
            {
                if !super::below(&self.inner.program, family, "handle")
                    || self
                        .inner
                        .conversions
                        .values()
                        .any(|c| super::below(&self.inner.program, family, &c.kind))
                {
                    return Err(self.inner.error(name, Failure::Signature));
                }
                let mut ancestor = family;
                for _ in 0..=self.inner.program.types().count() {
                    if ancestor == "handle" {
                        break;
                    }
                    let ty = self
                        .inner
                        .program
                        .types()
                        .find(|t| t.name == ancestor)
                        .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
                    if ty.source != source {
                        return Err(self.inner.error(name, Failure::Source));
                    }
                    ancestor = &ty.parent;
                }
                if ancestor != "handle" {
                    return Err(self.inner.error(name, Failure::Signature));
                }
            }
            found = true;
        }
        if !found {
            return Ok(self);
        }
        self.check_trigger_player_response(&source)?;
        let mut event_ids = [None; 2];
        for (i, native, global) in [
            (0, "TriggerRegisterDialogEvent", "EVENT_DIALOG_CLICK"),
            (
                1,
                "TriggerRegisterDialogButtonEvent",
                "EVENT_DIALOG_BUTTON_CLICK",
            ),
        ] {
            if self.inner.program.natives().get(native).is_none() {
                continue;
            }
            if self.triggers.is_none() {
                return Err(self
                    .inner
                    .error(native, Failure::DialogState("requires trigger admission")));
            }
            let enumerand = self.source_enumerand(native, global, "dialogevent")?;
            let Value::Handle(id) = self
                .inner
                .convert_for("ConvertDialogEvent", &[Value::Int(enumerand)])?
            else {
                unreachable!("checked conversion")
            };
            event_ids[i] = Some(id);
        }
        self.dialogs = Some(DialogState {
            budget,
            dialogs: BTreeMap::new(),
            text_bytes: 0,
            buttons: 0,
            events: 0,
            revision: 0,
            quit_requests: Vec::new(),
            dialog_event: event_ids[0],
            button_event: event_ids[1],
            layout: None,
        });
        Ok(self)
    }
    pub fn with_dialog_layout(mut self, rules: crate::dialog_layout::Rules) -> Result<Self, Error> {
        let state = self.dialogs.as_mut().ok_or_else(|| {
            self.inner.error(
                CREATE,
                Failure::DialogState("layout requires admitted dialogs"),
            )
        })?;
        if state.layout.is_some() {
            return Err(self
                .inner
                .error(CREATE, Failure::DialogState("layout already admitted")));
        }
        state.layout = Some(rules);
        Ok(self)
    }
    pub fn dialog_layout(&self) -> Option<&crate::dialog_layout::Rules> {
        self.dialogs.as_ref()?.layout.as_ref()
    }
    pub fn dialog_state(&self) -> Option<&DialogState> {
        self.dialogs.as_ref()
    }
    pub fn take_dialog_quit_requests(&mut self) -> Vec<DialogQuitRequest> {
        self.dialogs
            .as_mut()
            .map_or_else(Vec::new, |s| std::mem::take(&mut s.quit_requests))
    }
    fn dialog_id(&self, value: &Value, name: &str) -> Result<HandleId, Error> {
        let Value::Handle(id) = value else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        match self
            .inner
            .entry(*id)
            .map_err(|e| self.inner.error(name, e.failure))?
        {
            Entry::Dialog { live: true } => Ok(*id),
            Entry::Dialog { live: false } => Err(self.inner.error(name, Failure::Identity)),
            _ => Err(self.inner.error(name, Failure::WrongFamily)),
        }
    }
    fn dialog_button(&self, id: HandleId, name: &str) -> Result<HandleId, Error> {
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(name, e.failure))?
        {
            Entry::DialogButton { dialog, live: true } => Ok(*dialog),
            Entry::DialogButton { live: false, .. } => {
                Err(self.inner.error(name, Failure::Identity))
            }
            _ => Err(self.inner.error(name, Failure::WrongFamily)),
        }
    }
    pub(super) fn dialog_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.dialogs.as_ref()?;
        if !NATIVES.iter().any(|(n, _, _, _)| *n == name)
            || matches!(name, "GetClickedDialog" | "GetClickedButton")
        {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(self.dialog_call(name, args))
    }
    fn dialog_call(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let state = self.dialogs.as_ref().expect("admitted dialogs");
        let revision = state.revision.checked_add(1).ok_or_else(|| {
            self.inner
                .error(name, Failure::DialogState("revision exhausted"))
        })?;
        let result = match (name, args) {
            (CREATE, []) => {
                if state.dialogs.len() >= state.budget.max_dialogs {
                    return Err(self.inner.error(name, Failure::Budget("live dialogs")));
                }
                let id = self.inner.allocate(Entry::Dialog { live: true }, name)?;
                self.dialogs.as_mut().expect("dialogs").dialogs.insert(
                    id,
                    ScriptDialog {
                        id,
                        message: None,
                        buttons: Vec::new(),
                        shown_revision: 0,
                        visible_to: BTreeSet::new(),
                    },
                );
                Value::Handle(id)
            }
            ("DialogDestroy" | "DialogClear", [dialog]) => {
                let id = self.dialog_id(dialog, name)?;
                let record = state.dialogs.get(&id).expect("live dialog payload");
                let bytes = record.message.as_ref().map_or(0, String::len)
                    + record.buttons.iter().map(|b| b.text.len()).sum::<usize>();
                let buttons: Vec<_> = record.buttons.iter().map(|b| b.id).collect();
                for button in &buttons {
                    if let Entry::DialogButton { live, .. } = self.inner.entry_mut(*button)? {
                        *live = false;
                    }
                }
                let state = self.dialogs.as_mut().expect("dialogs");
                state.text_bytes -= bytes;
                state.buttons -= buttons.len();
                if name == "DialogDestroy" {
                    state.dialogs.remove(&id);
                    if let Entry::Dialog { live } = self.inner.entry_mut(id)? {
                        *live = false;
                    }
                } else {
                    let record = state.dialogs.get_mut(&id).expect("live dialog");
                    record.message = None;
                    record.buttons.clear();
                }
                Value::Null
            }
            ("DialogSetMessage", [dialog, Value::Str(text)]) => {
                let id = self.dialog_id(dialog, name)?;
                let previous = state.dialogs[&id].message.as_ref().map_or(0, String::len);
                let bytes = state
                    .text_bytes
                    .checked_sub(previous)
                    .and_then(|n| n.checked_add(text.len()))
                    .filter(|n| *n <= state.budget.text_bytes)
                    .ok_or_else(|| self.inner.error(name, Failure::Budget("dialog text")))?;
                let state = self.dialogs.as_mut().expect("dialogs");
                state.dialogs.get_mut(&id).expect("dialog").message = Some(text.clone());
                state.text_bytes = bytes;
                Value::Null
            }
            ("DialogAddButton", [dialog, Value::Str(text), Value::Int(hotkey)]) => {
                self.add_dialog_button(dialog, text, *hotkey, None, name)?
            }
            (
                "DialogAddQuitButton",
                [
                    dialog,
                    Value::Bool(score),
                    Value::Str(text),
                    Value::Int(hotkey),
                ],
            ) => self.add_dialog_button(dialog, text, *hotkey, Some(*score), name)?,
            ("DialogDisplay", [Value::Handle(player), dialog, Value::Bool(visible)]) => {
                let id = self.dialog_id(dialog, name)?;
                let number = match self
                    .inner
                    .entry(*player)
                    .map_err(|e| self.inner.error(name, e.failure))?
                {
                    Entry::Player { number } => *number,
                    _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                };
                let local = self.local_player_number().ok_or_else(|| {
                    self.inner
                        .error(name, Failure::DialogState("local player is not admitted"))
                })?;
                if number != local {
                    return Ok(Value::Null);
                }
                let record = self
                    .dialogs
                    .as_mut()
                    .expect("dialogs")
                    .dialogs
                    .get_mut(&id)
                    .expect("dialog");
                if *visible {
                    record.shown_revision = revision;
                    record.visible_to.insert(number);
                } else {
                    record.visible_to.remove(&number);
                }
                Value::Null
            }
            (
                "TriggerRegisterDialogEvent" | "TriggerRegisterDialogButtonEvent",
                [Value::Handle(trigger), subject],
            ) => {
                if !matches!(
                    self.inner
                        .entry(*trigger)
                        .map_err(|e| self.inner.error(name, e.failure))?,
                    Entry::Trigger { .. }
                ) {
                    return Err(self.inner.error(name, Failure::WrongFamily));
                }
                let (dialog, button) = if name == "TriggerRegisterDialogEvent" {
                    (self.dialog_id(subject, name)?, None)
                } else {
                    let Value::Handle(button) = subject else {
                        return Err(self.inner.error(name, Failure::Signature));
                    };
                    (self.dialog_button(*button, name)?, Some(*button))
                };
                if state.events >= state.budget.max_events {
                    return Err(self.inner.error(name, Failure::Budget("dialog events")));
                }
                let id = self.inner.allocate(
                    Entry::RegisteredDialogEvent {
                        trigger: *trigger,
                        dialog,
                        button,
                    },
                    name,
                )?;
                self.dialogs.as_mut().expect("dialogs").events += 1;
                Value::Handle(id)
            }
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        self.dialogs.as_mut().expect("dialogs").revision = revision;
        Ok(result)
    }
    fn add_dialog_button(
        &mut self,
        dialog: &Value,
        text: &str,
        hotkey: i32,
        quit_score_screen: Option<bool>,
        name: &str,
    ) -> Result<Value, Error> {
        let id = self.dialog_id(dialog, name)?;
        let state = self.dialogs.as_ref().expect("dialogs");
        if state.buttons >= state.budget.max_buttons {
            return Err(self.inner.error(name, Failure::Budget("dialog buttons")));
        }
        let bytes = state
            .text_bytes
            .checked_add(text.len())
            .filter(|n| *n <= state.budget.text_bytes)
            .ok_or_else(|| self.inner.error(name, Failure::Budget("dialog text")))?;
        self.dialogs
            .as_mut()
            .expect("dialogs")
            .dialogs
            .get_mut(&id)
            .expect("dialog")
            .buttons
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        let button = self.inner.allocate(
            Entry::DialogButton {
                dialog: id,
                live: true,
            },
            name,
        )?;
        let state = self.dialogs.as_mut().expect("dialogs");
        state
            .dialogs
            .get_mut(&id)
            .expect("dialog")
            .buttons
            .push(DialogButton {
                id: button,
                text: text.to_owned(),
                hotkey,
                quit_score_screen,
            });
        state.buttons += 1;
        state.text_bytes = bytes;
        Ok(Value::Handle(button))
    }

    /// Explicit player input; the UI passes the selected local player's existing identity.
    /// A click for an undisplayed or stale dialog refuses before changing visible state.
    pub fn click_dialog_button(
        &mut self,
        player: HandleId,
        button: HandleId,
    ) -> Result<usize, Error> {
        let name = "dialog button input";
        let dialog = self.dialog_button(button, name)?;
        let number = match self
            .inner
            .entry(player)
            .map_err(|e| self.inner.error(name, e.failure))?
        {
            Entry::Player { number } => *number,
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        };
        if self.local_player_number() != Some(number) {
            return Err(self.inner.error(
                name,
                Failure::DialogState("input player is not the local player"),
            ));
        }
        let state = self
            .dialogs
            .as_ref()
            .ok_or_else(|| self.inner.error(name, Failure::DialogState("not admitted")))?;
        let record = state
            .dialogs
            .get(&dialog)
            .ok_or_else(|| self.inner.error(name, Failure::Identity))?;
        if !record.visible_to(number) {
            return Err(self.inner.error(
                name,
                Failure::DialogState("dialog is not displayed for this player"),
            ));
        }
        let quit = record
            .buttons
            .iter()
            .find(|b| b.id == button)
            .ok_or_else(|| self.inner.error(name, Failure::Identity))?
            .quit_score_screen;
        let revision = state.revision.checked_add(1).ok_or_else(|| {
            self.inner
                .error(name, Failure::DialogState("revision exhausted"))
        })?;
        if quit.is_some() && state.quit_requests.len() >= state.budget.max_quit_requests {
            return Err(self
                .inner
                .error(name, Failure::Budget("dialog quit requests")));
        }
        let mut fired = Vec::new();
        for entry in &self.inner.arena {
            if let Entry::RegisteredDialogEvent {
                trigger,
                dialog: registered,
                button: registered_button,
            } = entry
                && *registered == dialog
                && registered_button.is_none_or(|b| b == button)
            {
                let event = if registered_button.is_some() {
                    state.button_event
                } else {
                    state.dialog_event
                };
                fired.push((*trigger, event));
            }
        }
        self.reserve_dialog_event_delivery(fired.len(), state.budget.max_events)?;
        if quit.is_some() {
            self.dialogs
                .as_mut()
                .expect("dialogs")
                .quit_requests
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        }
        let subject = EventSubject {
            dialog: Some(dialog),
            button: Some(button),
            player: Some(player),
            ..EventSubject::default()
        };
        let count = fired.len();
        for (trigger, event) in fired {
            self.queue_event(trigger, None, subject, event)?;
        }
        let state = self.dialogs.as_mut().expect("dialogs");
        state
            .dialogs
            .get_mut(&dialog)
            .expect("dialog")
            .visible_to
            .remove(&number);
        if let Some(score_screen) = quit {
            state.quit_requests.push(DialogQuitRequest {
                player,
                dialog,
                button,
                score_screen,
            });
        }
        state.revision = revision;
        Ok(count)
    }
}
