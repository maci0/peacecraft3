#[cfg(test)]
use super::Limits;
use super::{Entry, EnumHost, Error, Failure, below};
use pcx_script::{HandleId, Host, Value};
use std::sync::Arc;
/// Opt-in session objects and authored map/player setup in one bounded arena.
/// Timer delivery uses explicit game time; world enumeration remains unsupported.
pub struct ObjectHost {
    inner: EnumHost,
    group_limit: Option<usize>,
    members: Vec<(HandleId, HandleId)>,
    config: Option<MapConfig>,
    players: Option<players::Players>,
    camera: Option<camera::CameraState>,
    day_night: Option<environment::DayNightState>,
    sounds: Option<crate::label_sounds::SoundBank>,
    label_sound_creation: Option<Vec<crate::label_sounds::SoundBank>>,
    file_sounds: Option<files::State>,
    sound_parameters: Option<crate::sound_parameters::Banks>,
    sound_start_limit: Option<usize>,
    sound_duration: bool,
    sound_properties: bool,
    sound_starts: Vec<HandleId>,
    stacked_sound_limit: Option<usize>,
    stacked_sound_requests: Vec<StackedSoundRequest>,
    startup_clock: Option<clock::StartupClockState>,
    map_music: Option<music::State>,
    quests: Option<crate::quest_session::QuestSession>,
    rectangles: bool,
    regions: Option<regions::Admission>,
    camera_setups: Option<camera_setup::Admission>,
    destructables: Option<destructables::Admission>,
    units: Option<unit_creation::Admission>,
    items: Option<item_creation::Admission>,
    triggers: Option<trigger_admission::Admission>,
    filters: Option<filters::Admission>,
    conditions: Option<conditions::Admission>,
    forces: Option<forces::Admission>,
    startup_settings: Option<StartupSettings>,
    startup_version: Option<HandleId>,
    stock: Option<stock::State>,
    timers: Option<timers::State>,
}
pub type TimerHost = ObjectHost;
impl ObjectHost {
    pub fn checked(inner: EnumHost) -> Result<Self, Error> {
        for (name, parameters, result) in [
            ("CreateTimer", &[][..], "timer"),
            ("DestroyTimer", &["timer"][..], "nothing"),
        ] {
            Self::contract(&inner, name, parameters, result, false, Some("timer"))?;
        }
        Ok(Self {
            inner,
            group_limit: None,
            members: Vec::new(),
            config: None,
            players: None,
            camera: None,
            day_night: None,
            sounds: None,
            label_sound_creation: None,
            file_sounds: None,
            sound_parameters: None,
            sound_start_limit: None,
            sound_duration: false,
            sound_properties: false,
            sound_starts: Vec::new(),
            stacked_sound_limit: None,
            stacked_sound_requests: Vec::new(),
            startup_clock: None,
            map_music: None,
            quests: None,
            rectangles: false,
            regions: None,
            camera_setups: None,
            destructables: None,
            units: None,
            items: None,
            triggers: None,
            filters: None,
            conditions: None,
            forces: None,
            startup_settings: None,
            startup_version: None,
            stock: None,
            timers: None,
        })
    }
    fn checked_boolean_wrapper(&self, id: HandleId, native: &str) -> Result<(), Error> {
        let fail = |failure| self.inner.error(native, failure);
        match self.inner.entry(id).map_err(|e| fail(e.failure))? {
            Entry::Filter { .. } => self
                .filter_callback(id)
                .map(|_| ())
                .map_err(|e| fail(e.failure)),
            Entry::Condition { .. } => self
                .condition_callback(id)
                .map(|_| ())
                .map_err(|e| fail(e.failure)),
            _ => Err(fail(Failure::WrongFamily)),
        }
    }

    fn contract(
        inner: &EnumHost,
        name: &str,
        parameters: &[&str],
        result: &str,
        constant: bool,
        family: Option<&str>,
    ) -> Result<(), Error> {
        Self::contract_in(inner, name, parameters, result, constant, family, true)
    }
    /// Families declared directly under `handle` stay eligible when `require_agent` is false.
    fn contract_in(
        inner: &EnumHost,
        name: &str,
        parameters: &[&str],
        result: &str,
        constant: bool,
        family: Option<&str>,
        require_agent: bool,
    ) -> Result<(), Error> {
        let n = inner
            .program
            .natives()
            .get(name)
            .ok_or_else(|| inner.error(name, Failure::Signature))?;
        if n.is_constant != constant
            || inner.program.function(name).is_some()
            || n.params.len() != parameters.len()
            || n.ret.array
            || n.ret.kind.name() != result
            || n.params
                .iter()
                .zip(parameters)
                .any(|(p, expected)| p.ty.array || p.ty.kind.name() != *expected)
        {
            return Err(inner.error(name, Failure::Signature));
        }
        if let Some(family) = family
            && ((require_agent && !below(&inner.program, family, "agent"))
                || !below(&inner.program, family, "handle")
                || family == "handle"
                || inner
                    .conversions
                    .values()
                    .any(|c| below(&inner.program, family, &c.kind)))
        {
            return Err(inner.error(name, Failure::Signature));
        }
        if n.also_declared_in
            .iter()
            .any(|(source, _)| source != &n.source)
        {
            return Err(inner.error(name, Failure::Source));
        }
        Ok(())
    }
    /// Explicit total live membership bound; group identities use the existing arena limit.
    pub fn with_groups(mut self, max_members: usize) -> Result<Self, Error> {
        std::alloc::Layout::array::<(HandleId, HandleId)>(max_members)
            .map_err(|_| self.inner.error("groups", Failure::Capacity))?;
        if !below(&self.inner.program, "unit", "widget")
            || !below(&self.inner.program, "unit", "handle")
        {
            return Err(self.inner.error("groups", Failure::Signature));
        }
        for (name, parameters, result) in [
            ("CreateGroup", &[][..], "group"),
            ("DestroyGroup", &["group"][..], "nothing"),
            ("GroupClear", &["group"][..], "nothing"),
            ("FirstOfGroup", &["group"][..], "unit"),
            ("GroupAddUnit", &["group", "unit"][..], "nothing"),
            ("GroupRemoveUnit", &["group", "unit"][..], "nothing"),
        ] {
            Self::contract(&self.inner, name, parameters, result, false, Some("group"))?;
        }
        Self::contract(
            &self.inner,
            "IsUnitInGroup",
            &["unit", "group"],
            "boolean",
            true,
            Some("group"),
        )?;
        if self.members.len() > max_members {
            return Err(self
                .inner
                .error("groups", Failure::Budget("group membership")));
        }
        self.group_limit = Some(max_members);
        Ok(self)
    }
    pub fn inner(&self) -> &EnumHost {
        &self.inner
    }
    pub fn into_vm(self) -> pcx_script::OwnedVm<Self> {
        pcx_script::OwnedVm::new_owned(Arc::clone(&self.inner.program), self)
    }
    pub fn live_timers(&self) -> usize {
        self.inner
            .arena
            .iter()
            .filter(|e| matches!(e, Entry::Timer { live: true, .. }))
            .count()
    }
    pub fn live_groups(&self) -> usize {
        self.inner
            .arena
            .iter()
            .filter(|e| matches!(e, Entry::Group { live: true }))
            .count()
    }
    pub fn memberships(&self) -> usize {
        self.members.len()
    }
    fn group_index(&self, value: &Value, name: &str) -> Result<usize, Error> {
        let Value::Handle(id) = value else {
            return Err(self.inner.error(name, Failure::GroupArguments));
        };
        let at = usize::try_from(id.token() - 1)
            .map_err(|_| self.inner.error(name, Failure::Identity))?;
        match self.inner.arena.get(at) {
            Some(Entry::Group { live: true }) => Ok(at),
            Some(Entry::Group { live: false }) | None => {
                Err(self.inner.error(name, Failure::Identity))
            }
            _ => Err(self.inner.error(name, Failure::WrongFamily)),
        }
    }
    fn unit(&self, value: &Value, name: &str) -> Result<HandleId, Error> {
        let Value::Handle(id) = value else {
            return Err(self.inner.error(name, Failure::GroupArguments));
        };
        let kind = self
            .inner
            .kind(*id)
            .map_err(|e| self.inner.error(name, e.failure))?;
        if !below(&self.inner.program, kind, "unit") {
            return Err(self.inner.error(name, Failure::WrongFamily));
        }
        Ok(*id)
    }
    fn group(&mut self, name: &str, args: &[Value], max_members: usize) -> Result<Value, Error> {
        if name == "CreateGroup" {
            if !args.is_empty() {
                return Err(self.inner.error(name, Failure::GroupArguments));
            }
            return self
                .inner
                .allocate(Entry::Group { live: true }, name)
                .map(Value::Handle);
        }
        let (group, member) = match (name, args) {
            ("IsUnitInGroup", [unit, group]) => (group, Some(self.unit(unit, name)?)),
            ("GroupAddUnit" | "GroupRemoveUnit", [group, unit]) => {
                (group, Some(self.unit(unit, name)?))
            }
            ("DestroyGroup" | "GroupClear" | "FirstOfGroup", [group]) => (group, None),
            _ => return Err(self.inner.error(name, Failure::GroupArguments)),
        };
        let at = self.group_index(group, name)?;
        let id = self.inner.id(at, name)?;
        match name {
            "DestroyGroup" | "GroupClear" => {
                self.members.retain(|(group, _)| *group != id);
                if name == "DestroyGroup" {
                    self.inner.arena[at] = Entry::Group { live: false };
                }
                Ok(Value::Null)
            }
            // shortcut: insertion order is deterministic; change it if original traces require another order.
            "FirstOfGroup" => Ok(self
                .members
                .iter()
                .find(|(group, _)| *group == id)
                .map_or(Value::Null, |(_, unit)| Value::Handle(*unit))),
            "GroupAddUnit" => {
                let pair = (
                    id,
                    member.ok_or_else(|| self.inner.error(name, Failure::GroupArguments))?,
                );
                if !self.members.contains(&pair) {
                    if self.members.len() >= max_members {
                        return Err(self.inner.error(name, Failure::Budget("group membership")));
                    }
                    self.members
                        .try_reserve(1)
                        .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                    self.members.push(pair);
                }
                Ok(Value::Null)
            }
            "GroupRemoveUnit" => {
                self.members
                    .retain(|pair| Some(*pair) != member.map(|unit| (id, unit)));
                Ok(Value::Null)
            }
            "IsUnitInGroup" => Ok(Value::Bool(
                member.is_some_and(|unit| self.members.contains(&(id, unit))),
            )),
            _ => Err(self.inner.error(name, Failure::Signature)),
        }
    }
    fn create(&mut self, args: &[Value]) -> Result<Value, Error> {
        if !args.is_empty() {
            return Err(self.inner.error("CreateTimer", Failure::TimerArguments));
        }
        self.inner
            .allocate(
                Entry::Timer {
                    live: true,
                    run: None,
                },
                "CreateTimer",
            )
            .map(Value::Handle)
    }
    fn destroy(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [Value::Handle(id)] = args else {
            return Err(self.inner.error("DestroyTimer", Failure::TimerArguments));
        };
        let at = usize::try_from(id.token() - 1)
            .map_err(|_| self.inner.error("DestroyTimer", Failure::Identity))?;
        let failure = match self.inner.arena.get_mut(at) {
            Some(Entry::Timer { live, run }) if *live => {
                *run = None;
                *live = false;
                return Ok(Value::Null);
            }
            Some(Entry::Timer { live: false, .. }) | None => Failure::Identity,
            _ => Failure::WrongFamily,
        };
        Err(self.inner.error("DestroyTimer", failure))
    }
}
impl Host for ObjectHost {
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if let Some(result) = self.timer_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.stacked_sound_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.sound_properties_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.sound_duration_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.sound_parameter_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.file_sound_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.map_music_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.startup_clock_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.sound_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.day_night_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.camera_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.player_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.config_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.quest_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.camera_setup_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.rect_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.region_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.destructable_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.unit_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.item_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.stock_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.startup_settings_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.force_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.condition_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.filter_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.trigger_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(limit) = self.group_limit
            && matches!(
                name,
                "CreateGroup"
                    | "DestroyGroup"
                    | "GroupClear"
                    | "FirstOfGroup"
                    | "GroupAddUnit"
                    | "GroupRemoveUnit"
                    | "IsUnitInGroup"
            )
        {
            return self.group(name, args, limit).map_err(|e| e.to_string());
        }
        match name {
            "CreateTimer" => self.create(args).map_err(|e| e.to_string()),
            "DestroyTimer" => self.destroy(args).map_err(|e| e.to_string()),
            _ => self.inner.native(name, args),
        }
    }
}

#[cfg(test)]
#[path = "timer_host_tests.rs"]
mod timer_tests;

#[cfg(test)]
#[path = "group_host_tests.rs"]
mod group_tests;

#[path = "map_config_host.rs"]
mod map_config;
pub use map_config::{MapConfig, MapConfigBudget, StartLocation, StartPriorities, StartPriority};

#[path = "player_host.rs"]
mod players;
pub use players::{
    PlayerAdmissionError, PlayerAllianceBudget, PlayerBudget, PlayerSettings,
    PlayerStateRequestBudget, ResearchObservation, TechLimitBudget,
};

#[path = "camera_host.rs"]
mod camera;
pub use camera::CameraState;

#[path = "day_night_host.rs"]
mod environment;
pub use environment::DayNightState;

#[path = "sound_host.rs"]
mod sounds;
pub use sounds::{LabelSoundFlags, SoundState};

#[path = "startup_clock_host.rs"]
mod clock;
pub use clock::StartupClockState;

#[path = "map_music_host.rs"]
mod music;
pub use music::MapMusicSelection;

#[path = "file_sound_host.rs"]
mod files;
pub use files::FileSoundState;

#[path = "sound_parameter_host.rs"]
mod parameters;

#[path = "quest_host.rs"]
mod quests;
pub use quests::{QuestBudget, QuestEvent, QuestFlag};

#[path = "rect_host.rs"]
mod rectangles;

#[path = "sound_properties_host.rs"]
mod sound_properties;
pub use sound_properties::SoundProperties;

#[path = "stacked_sound_host.rs"]
mod stacked_sounds;
pub use stacked_sounds::{StackedSoundAction, StackedSoundRequest};

#[path = "camera_setup_host.rs"]
mod camera_setup;
pub use camera_setup::{CameraDestination, CameraFieldRecord};

#[path = "destructable_host.rs"]
mod destructables;
pub use destructables::{DestructableBudget, DestructableCreation};

#[path = "unit_host.rs"]
mod unit_creation;
pub use unit_creation::{ExecutedUnit, ExecutedUnitState, UnitBudget, UnitStateBudget};

#[path = "item_host.rs"]
mod item_creation;
pub use item_creation::{ExecutedItem, ItemBudget};

#[path = "trigger_host.rs"]
mod trigger_admission;
pub use trigger_admission::{
    DeathEventRegistration, GameStateEventRegistration, PlayerEventRegistration,
    PlayerUnitEventRegistration, TimerEventRegistration, TriggerActionRecord, TriggerBudget,
    TriggerConditionRegistration, UnitEventRegistration,
};

#[cfg(test)]
#[path = "filter_host_tests.rs"]
mod filter_tests;

#[path = "filter_host.rs"]
mod filters;
pub use filters::{FilterBudget, FilterCallback};

#[path = "region_host.rs"]
mod regions;
pub use regions::{RegionBudget, RegionEventKind, RegionEventRegistration, RegionRectRequest};

#[cfg(test)]
#[path = "condition_host_tests.rs"]
mod condition_tests;

#[path = "condition_host.rs"]
mod conditions;
pub use conditions::{ConditionBudget, ConditionCallback};

#[path = "force_host.rs"]
mod forces;
pub use forces::ForceBudget;

#[path = "startup_settings_host.rs"]
mod settings;
pub use settings::StartupSettings;

#[path = "stock_host.rs"]
mod stock;
pub use stock::StockBudget;

#[path = "timer_runtime_host.rs"]
pub(super) mod timers;
pub use timers::TimerExpiry;

#[cfg(test)]
#[path = "timer_runtime_tests.rs"]
mod timer_runtime_tests;
