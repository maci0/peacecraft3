#[cfg(test)]
use super::Limits;
use super::{Entry, EnumHost, Error, Failure, below};
use pcx_script::{HandleId, Host, Value, ast::Expr};
use std::sync::Arc;
/// Opt-in session objects and authored map/player setup in one bounded arena.
/// Timer delivery uses explicit game time; world enumeration remains unsupported.
pub struct ObjectHost {
    cheat_queries: Option<crate::cheat_rules::Rules>,
    fog_modifiers: bool,
    selection: Option<Vec<HandleId>>,
    pings: Vec<indicators::MinimapPing>,
    cinematic_scene: Option<Option<cinematic_scene::CinematicScene>>,
    dialogs: Option<dialogs::DialogState>,
    indicators: Option<(Vec<indicators::Indicator>, usize)>,
    combat: Option<combat::Rules>,
    combat_trace: Option<combat::CombatTrace>,
    holy_light: Option<holy_light::Rules>,
    summoning: Option<unit_summon::State>,
    events: event_delivery::Delivery,
    unit_vitals: Option<unit_vitals::Rules>,
    unit_buffs: unit_buff::State,
    divine_shield: divine_shield::State,
    volume_groups: Option<volume_groups::State>,
    camera_view: Option<camera_view::CameraView>,
    scene: Option<scene::Environment>,
    threads: thread::Threads,
    time_of_day: Option<time_of_day::TimeOfDay>,
    locations: bool,
    effects: bool,
    unit_activity: Option<unit_activity::State>,
    harvest: Option<harvest_runtime::State>,
    order_registry: Option<std::sync::Arc<crate::order_registry::Registry>>,
    inner: EnumHost,
    group_limit: Option<usize>,
    group_orders: Option<crate::group_order_rules::Rules>,
    members: Vec<(HandleId, HandleId)>,
    config: Option<MapConfig>,
    players: Option<players::Players>,
    player_abilities: Option<player_ability::PlayerAbilityState>,
    camera: Option<camera::CameraState>,
    day_night: Option<environment::DayNightState>,
    sounds: Option<Vec<crate::label_sounds::SoundBank>>,
    label_sound_creation: Option<Vec<crate::label_sounds::SoundBank>>,
    label_sound_loader: Option<(sounds::Loader, crate::label_sounds::LoadBudget)>,
    file_sounds: Option<files::State>,
    sound_parameters: Option<crate::sound_parameters::Banks>,
    sound_parameter_loader: Option<(parameters::Loader, crate::sound_parameters::BanksBudget)>,
    sound_start_limit: Option<usize>,
    sound_duration: bool,
    sound_properties: bool,
    sound_starts: Vec<HandleId>,
    /// Game time of each sound's latest start, until it is stopped.
    sound_playing: std::collections::BTreeMap<HandleId, f64>,
    /// Executed `StopSound` requests for the audio consumer, in order.
    sound_stops: Vec<(HandleId, bool, bool)>,
    stacked_sound_limit: Option<usize>,
    stacked_sound_requests: Vec<StackedSoundRequest>,
    startup_clock: Option<clock::StartupClockState>,
    numeric: Option<crate::script_numeric::NumericPolicy>,
    terrain_pathing: Option<crate::terrain_pathing::TerrainPathing>,
    initial_blight: Option<crate::terrain_blight::InitialBlight>,
    unit_placement: Option<crate::unit_placement::Placement>,
    destructable_catalog: Option<destructables::DestructableCatalog>,
    destructable_restore: Option<destructables::RestoreLifePolicy>,
    placed_sources: std::collections::BTreeMap<HandleId, destructables::PlacedSource>,
    map_music: Option<music::State>,
    sound_environments: Option<environments::State>,
    quests: Option<crate::quest_session::QuestSession>,
    game_caches: Option<game_caches::State>,
    presentation: Option<presentation::PresentationState>,
    message_layout: Option<crate::message_layout::Rules>,
    no_occlusion_pass: bool,
    occlusion_enabled: Option<bool>,
    messages: Option<messages::MessageState>,
    rectangles: bool,
    regions: Option<regions::Admission>,
    camera_setups: Option<camera_setup::Admission>,
    destructables: Option<destructables::Admission>,
    units: Option<unit_creation::Admission>,
    items: Option<item_creation::Admission>,
    item_drops: Option<item_drop::ItemDrops>,
    item_queries: Option<crate::item_class_table::ItemClassTable>,
    item_sales: Option<crate::item_sales::ItemSaleTable>,
    item_use: Option<item_use::ItemUse>,
    player_scores: Option<Vec<player_score::PlayerScoreCounter>>,
    unit_explosions: Option<unit_explosion::UnitExplosions>,
    player_removal: Option<player_removal::PlayerRemovalState>,
    localized_queries: Option<crate::script_localization::Bank>,
    game_pause: Option<Option<bool>>,
    game_exit: Option<game_exit::GameExits>,
    guards: Option<unit_guards::GuardState>,
    triggers: Option<trigger_admission::Admission>,
    filters: Option<filters::Admission>,
    conditions: Option<conditions::Admission>,
    forces: Option<forces::Admission>,
    startup_settings: Option<StartupSettings>,
    startup_version: Option<HandleId>,
    game_difficulty: Option<HandleId>,
    campaign_targeting: Option<targeting::State>,
    campaign_ai: campaign_flee::Registry,
    campaign_flee: Option<crate::campaign_flee_policy::FleePolicy>,
    campaign_gathering: Option<crate::campaign_gathering_policy::GatheringPolicy>,
    default_difficulty: Option<HandleId>,
    campaign_menu_race: Option<Option<HandleId>>,
    stock: Option<stock::State>,
    timers: Option<timers::State>,
    random: Option<random::Random>,
    trigger_runtime: Option<trigger_runtime::State>,
    unit_range_runtime: Option<unit_ranges::State>,
    callbacks: Vec<NativeCallback>,
    synchronous_frames: Vec<SynchronousFrame>,
}
pub(super) struct SynchronousFrame {
    events: std::collections::VecDeque<event_delivery::FiredEvent>,
    active: bool,
    spawn_depth: usize,
    callback_depth: usize,
    conditions: Option<SynchronousDelivery>,
}
/// Remaining callbacks for the event whose conditions just ran.
pub(super) struct SynchronousDelivery {
    pub(super) conditions: Vec<pcx_script::value::FunctionId>,
    pub(super) actions: Vec<pcx_script::value::FunctionId>,
    pub(super) next: usize,
}
enum NativeCallback {
    Trigger(trigger_runtime::DirectCall),
    Rectangle(unit_positions::Enumeration),
    Group(group_iteration::Iteration),
}
pub type TimerHost = ObjectHost;
impl ObjectHost {
    #[cfg(test)]
    pub(crate) fn test_attach_placement(&mut self, placement: crate::unit_placement::Placement) {
        self.unit_placement = Some(placement);
    }

    #[cfg(test)]
    pub(crate) fn test_placement_cell(&self, cell: usize) -> Option<u8> {
        self.unit_placement
            .as_ref()
            .map(|placement| placement.test_cell(cell))
    }

    /// Complete authored `.doo` record retained for one placed handle.
    pub fn placed_source(&self, id: HandleId) -> Option<destructables::PlacedSource> {
        self.placed_sources.get(&id).copied()
    }

    pub fn with_script_numeric(mut self, policy: crate::script_numeric::NumericPolicy) -> Self {
        self.numeric = Some(policy);
        self
    }

    pub fn with_message_layout(
        mut self,
        rules: crate::message_layout::Rules,
    ) -> Result<Self, Error> {
        if self.message_layout.is_some() {
            return Err(self.inner.error(
                "message layout",
                Failure::Message("layout already admitted"),
            ));
        }
        self.message_layout = Some(rules);
        Ok(self)
    }

    pub fn message_layout(&self) -> Option<&crate::message_layout::Rules> {
        self.message_layout.as_ref()
    }

    pub fn checked(inner: EnumHost) -> Result<Self, Error> {
        for (name, parameters, result) in [
            ("CreateTimer", &[][..], "timer"),
            ("DestroyTimer", &["timer"][..], "nothing"),
        ] {
            Self::contract(&inner, name, parameters, result, false, Some("timer"))?;
        }
        Ok(Self {
            cheat_queries: None,
            inner,
            group_limit: None,
            group_orders: None,
            members: Vec::new(),
            config: None,
            players: None,
            player_abilities: None,
            camera: None,
            day_night: None,
            sounds: None,
            label_sound_creation: None,
            label_sound_loader: None,
            file_sounds: None,
            sound_parameters: None,
            sound_parameter_loader: None,
            sound_start_limit: None,
            sound_duration: false,
            sound_properties: false,
            sound_starts: Vec::new(),
            sound_playing: std::collections::BTreeMap::new(),
            sound_stops: Vec::new(),
            stacked_sound_limit: None,
            stacked_sound_requests: Vec::new(),
            startup_clock: None,
            numeric: None,
            terrain_pathing: None,
            initial_blight: None,
            unit_placement: None,
            destructable_catalog: None,
            destructable_restore: None,
            placed_sources: std::collections::BTreeMap::new(),
            map_music: None,
            sound_environments: None,
            quests: None,
            game_caches: None,
            presentation: None,
            message_layout: None,
            no_occlusion_pass: false,
            occlusion_enabled: None,
            messages: None,
            rectangles: false,
            regions: None,
            camera_setups: None,
            destructables: None,
            units: None,
            order_registry: None,
            unit_activity: None,
            harvest: None,
            locations: false,
            time_of_day: None,
            threads: thread::Threads::default(),
            scene: None,
            camera_view: None,
            volume_groups: None,
            unit_vitals: None,
            unit_buffs: unit_buff::State::default(),
            divine_shield: divine_shield::State::default(),
            events: event_delivery::Delivery::default(),
            combat: None,
            combat_trace: None,
            holy_light: None,
            summoning: None,
            indicators: None,
            cinematic_scene: None,
            dialogs: None,
            pings: Vec::new(),
            selection: None,
            fog_modifiers: false,
            effects: false,
            items: None,
            unit_explosions: None,
            item_drops: None,
            item_queries: None,
            item_sales: None,
            item_use: None,
            player_scores: None,
            player_removal: None,
            localized_queries: None,
            game_pause: None,
            game_exit: None,
            guards: None,
            triggers: None,
            filters: None,
            conditions: None,
            forces: None,
            startup_settings: None,
            startup_version: None,
            game_difficulty: None,
            campaign_targeting: None,
            campaign_ai: campaign_flee::Registry::default(),
            campaign_flee: None,
            campaign_gathering: None,
            default_difficulty: None,
            campaign_menu_race: None,
            stock: None,
            timers: None,
            random: None,
            trigger_runtime: None,
            unit_range_runtime: None,
            callbacks: Vec::new(),
            synchronous_frames: Vec::new(),
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
            Entry::DeadBoolExpr => Err(fail(Failure::Identity)),
            _ => Err(fail(Failure::WrongFamily)),
        }
    }

    fn source_enumerand(&self, name: &str, global_name: &str, kind: &str) -> Result<i32, Error> {
        let fail = |why| self.inner.error(name, why);
        let source = &self
            .inner
            .program
            .natives()
            .get(name)
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        let mut parent = kind;
        for _ in 0..=self.inner.program.types().count() {
            if parent == "handle" {
                break;
            }
            let ty = self
                .inner
                .program
                .types()
                .find(|ty| ty.name == parent)
                .ok_or_else(|| fail(Failure::Signature))?;
            if ty.source != *source {
                return Err(fail(Failure::Source));
            }
            parent = &ty.parent;
        }
        if parent != "handle" {
            return Err(fail(Failure::Signature));
        }
        let global = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == global_name)
            .ok_or_else(|| fail(Failure::Initializer(global_name.to_owned())))?;
        if global.source != *source {
            return Err(fail(Failure::Source));
        }
        if !global.is_constant || global.array || global.ty.array || global.ty.kind.name() != kind {
            return Err(fail(Failure::Signature));
        }
        let Some(Expr::Call { callee, args, .. }) = &global.init else {
            return Err(fail(Failure::Initializer(global.name.clone())));
        };
        let (Expr::Ident(conversion, _), [arg]) = (callee.as_ref(), args.as_slice()) else {
            return Err(fail(Failure::Initializer(global.name.clone())));
        };
        let admitted = self
            .inner
            .conversions
            .get(conversion.as_str())
            .filter(|c| c.kind == kind && c.source == *source)
            .ok_or_else(|| fail(Failure::Signature))?;
        let mut work = 256;
        let value = super::integer(arg, 16, &mut work).map_err(fail)?;
        if !admitted.domain.contains(&value) {
            return Err(fail(Failure::OutsideDomain(value)));
        }
        Ok(value)
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
        for (name, args, result, constant) in [
            ("ForGroup", &["group", "code"][..], "nothing", false),
            ("GetEnumUnit", &[][..], "unit", true),
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, args, result, constant, Some("group"))?;
                if native.source
                    != self
                        .inner
                        .program
                        .natives()
                        .get("CreateGroup")
                        .expect("group admission")
                        .source
                {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
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
impl ObjectHost {
    /// `CreateUnitAtLoc` is `CreateUnit` at the location's current point.
    fn create_unit_at_location(&mut self, args: &[Value]) -> Result<Value, Error> {
        let name = "CreateUnitAtLoc";
        Self::contract(
            &self.inner,
            name,
            &["player", "integer", "location", "real"],
            "unit",
            false,
            Some("unit"),
        )?;
        let natives = self.inner.program.natives();
        if natives.get(name).map(|n| &n.source) != natives.get("CreateUnit").map(|n| &n.source) {
            return Err(self.inner.error(name, Failure::Source));
        }
        let [player, id, location, face] = args else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        let [x, y] = self.location_point(location, name)?;
        let forwarded = [
            player.clone(),
            id.clone(),
            Value::Real(x),
            Value::Real(y),
            face.clone(),
        ];
        pcx_script::Host::native(self, "CreateUnit", &forwarded)
            .map_err(|why| self.inner.error(name, Failure::UnitActivity(why)))
    }

    /// Enumerand of a selected constant `kind` global initialized by an admitted conversion.
    pub(super) fn source_constant_enumerand(&self, global: &str, kind: &str) -> Result<i32, Error> {
        let fail = |reason| self.inner.error(global, Failure::UnitState(reason));
        let declared = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == global && g.is_constant && !g.array && g.ty.kind.name() == kind)
            .ok_or_else(|| fail("missing selected constant"))?;
        let Some(Expr::Call { callee, args, .. }) = &declared.init else {
            return Err(fail("expected conversion initializer"));
        };
        let (Expr::Ident(conversion, _), [arg]) = (callee.as_ref(), args.as_slice()) else {
            return Err(fail("expected one conversion argument"));
        };
        let admitted = self
            .inner
            .conversions()
            .find(|c| c.name == conversion.as_str() && c.kind == kind)
            .ok_or_else(|| fail("conversion is not admitted"))?;
        let mut work = 4096;
        let value = crate::playercolor_host::integer(arg, 64, &mut work)
            .map_err(|failure| self.inner.error(global, failure))?;
        if !admitted.domain.contains(&value) {
            return Err(self.inner.error(global, Failure::OutsideDomain(value)));
        }
        Ok(value)
    }

    /// Selected software math; absent policy leaves these natives unsupported.
    fn math_native(&self, name: &str, args: &[Value]) -> Option<Result<Value, Error>> {
        let numeric = self.numeric.as_ref()?;
        let math = numeric.math()?;
        let real = |v: &Value| v.as_real();
        let result = match (name, args) {
            ("SquareRoot", [x]) => math.square_root_native(numeric, real(x)?),
            ("Atan2", [y, x]) => math.arctangent2_native(numeric, real(y)?, real(x)?),
            _ => return None,
        };
        Some(
            result
                .map(Value::Real)
                .map_err(|e| self.inner.error(name, Failure::ScriptMath(e))),
        )
    }
}
impl Host for ObjectHost {
    fn begin_script_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<pcx_script::ScriptCall>, String> {
        if let Some(call) = self
            .begin_rectangle_call(name, args)
            .map_err(|e| e.to_string())?
        {
            return Ok(Some(call));
        }
        if let Some(call) = self
            .begin_group_call(name, args)
            .map_err(|e| e.to_string())?
        {
            return Ok(Some(call));
        }
        if let Some(call) = self
            .begin_selection_call(name, args)
            .map_err(|e| e.to_string())?
        {
            return Ok(Some(call));
        }
        if let Some(call) = self
            .begin_synchronous_destructable_death_call(name, args)
            .map_err(|e| e.to_string())?
        {
            return Ok(Some(call));
        }
        if let Some(call) = self
            .begin_synchronous_player_state_call(name, args)
            .map_err(|e| e.to_string())?
        {
            return Ok(Some(call));
        }
        self.begin_trigger_call(name, args)
            .map_err(|e| e.to_string())
    }
    fn continue_script_call(&mut self, value: Value) -> Result<pcx_script::ScriptCall, String> {
        if self.synchronous_frames.last().is_some_and(|frame| {
            frame.conditions.is_some() && self.callbacks.len() == frame.callback_depth
        }) {
            return self
                .continue_synchronous_call(value)
                .map_err(|e| e.to_string());
        }
        match self.callbacks.last() {
            Some(NativeCallback::Rectangle(_)) => self.continue_rectangle_call(value),
            Some(NativeCallback::Trigger(_)) => self.continue_trigger_call(value),
            Some(NativeCallback::Group(_)) => self.continue_group_call(),
            None => return Err("no native script callback is active".into()),
        }
        .map_err(|e| e.to_string())
    }
    fn after_script_spawn(&mut self) -> Result<Option<pcx_script::ScriptCall>, String> {
        self.after_synchronous_spawn().map_err(|e| e.to_string())
    }
    fn abort_script_calls(&mut self) {
        self.abort_threads();
        self.abort_native_callbacks();
        self.abort_synchronous_delivery();
    }
    fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
        self.thread_sleep_requested(name, args)
    }
    fn park_thread(&mut self) -> Result<u64, String> {
        self.park_thread_now().map_err(|e| e.to_string())
    }
    fn resume_thread(&mut self, ticket: u64) -> Result<(), String> {
        self.resume_thread_now(ticket).map_err(|e| e.to_string())
    }
    fn leave_thread(&mut self) -> Result<(), String> {
        self.leave_thread_now().map_err(|e| e.to_string())
    }
    fn real_ops(&self) -> Option<&dyn pcx_script::RealOps> {
        self.numeric.as_ref().map(|p| p as &dyn pcx_script::RealOps)
    }

    #[cfg_attr(feature = "profile-cpu", tracing::instrument(skip_all, target = "pcx_cpu", name = "script.native", fields(native = name)))]
    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if let Some(result) = self.summoning_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if name == "CreateUnitAtLoc" && self.locations && self.units.is_some() {
            return self
                .create_unit_at_location(args)
                .map_err(|e| e.to_string());
        }
        if let Some(result) = self.message_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.presentation_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.game_cache_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.random_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.trigger_runtime_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.event_response_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.unit_range_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.timer_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.stacked_sound_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.sound_attachment_native(name, args) {
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
        if let Some(result) = self.sound_environment_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.time_of_day_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.startup_clock_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.terrain_pathing_native(name, args) {
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
        if let Some(result) = self.player_ability_native(name, args) {
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
        if let Some(result) = self.order_registry_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.harvest_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.unit_vitals_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.cheat_query_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.volume_group_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.unit_removal_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.math_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.camera_view_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.environment_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.thread_sleep_native(name) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.effect_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.location_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.unit_type_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.fog_modifier_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.selection_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.dialog_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.cinematic_scene_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.indicator_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.inventory_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.localized_query_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.game_pause_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.player_score_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.game_exit_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.player_removal_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.guard_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.follow_target_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.combat_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.activity_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.group_order_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.unit_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.group_iteration_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.unit_position_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.unit_explosion_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.item_drop_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.item_query_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.item_sale_native(name, args) {
            return result.map_err(|e| e.to_string());
        }
        if let Some(result) = self.item_use_native(name, args) {
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

#[path = "script_random_host.rs"]
mod random;

#[path = "group_iteration_host.rs"]
mod group_iteration;

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

#[path = "player_ability.rs"]
mod player_ability;

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

#[path = "terrain_pathing_host.rs"]
mod terrain_pathing;

#[path = "map_music_host.rs"]
mod music;
pub use music::MapMusicSelection;

#[path = "sound_environment_host.rs"]
mod environments;

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
pub use camera_setup::{AppliedCameraSetup, CameraDestination, CameraFieldRecord};

#[path = "destructable_host.rs"]
mod destructables;
pub use destructables::{
    DestructableBudget, DestructableCatalog, DestructableCreation, DestructableOrigin,
    DestructableState, DestructableStats, PlacedDestructable, PlacedSource, RestoreLifeBinding,
    RestoreLifePolicy,
};

#[path = "unit_host.rs"]
mod unit_creation;
pub use unit_creation::{
    ExecutedUnit, ExecutedUnitState, HeroBudget, HeroProgress, UnitAbilities, UnitBudget,
    UnitStateBudget,
};

#[path = "unit_position_host.rs"]
mod unit_positions;

#[path = "item_host.rs"]
mod item_creation;
pub use item_creation::{ExecutedItem, ItemBudget};

#[path = "unit_explosion_host.rs"]
mod unit_explosion;

#[path = "boolexpr_destroy_host.rs"]
mod boolexpr_destroy;
#[path = "item_drop_host.rs"]
mod item_drop;

#[path = "item_query_host.rs"]
mod item_query;

#[path = "item_sale_host.rs"]
mod item_sale;
#[path = "item_use_host.rs"]
mod item_use;
pub use item_use::{UseCheck, UseCommit, UseGrant};

#[path = "trigger_host.rs"]
mod trigger_admission;
pub use trigger_admission::{
    DeathEventRegistration, GameStateEventRegistration, PlayerEventRegistration,
    PlayerStateEventRegistration, PlayerUnitEventRegistration, TimerEventRegistration,
    TriggerActionRecord, TriggerBudget, TriggerConditionRegistration, UnitEventRegistration,
    UnitInRangeRegistration,
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

#[path = "game_cache_host.rs"]
mod game_caches;

#[path = "unit_cache_host.rs"]
mod unit_caches;

#[path = "presentation_host.rs"]
mod presentation;
pub use presentation::{CineFilter, FilterSample, PresentationState};

#[path = "stock_host.rs"]
mod stock;
pub use stock::{ShopCapacity, StockBudget};

#[path = "timer_runtime_host.rs"]
pub(super) mod timers;
pub use timers::TimerExpiry;

#[cfg(test)]
#[path = "timer_runtime_tests.rs"]
mod timer_runtime_tests;

#[path = "trigger_runtime_host.rs"]
mod trigger_runtime;
pub use trigger_runtime::{TriggerDelivery, TriggerRuntimeInput};

#[path = "order_registry_host.rs"]
mod order_registry;
#[path = "unit_range_runtime_host.rs"]
mod unit_ranges;
pub use unit_ranges::UnitRangeEntry;
#[cfg(test)]
#[path = "editor_trigger_runtime_tests.rs"]
mod editor_trigger_runtime_tests;
#[cfg(test)]
#[path = "trigger_runtime_tests.rs"]
mod trigger_runtime_tests;

#[path = "message_host.rs"]
mod messages;
pub use messages::{MessageBudget, MessageState, TimedMessage};

#[path = "harvest_runtime.rs"]
pub(crate) mod harvest_runtime;
#[path = "unit_activity_host.rs"]
mod unit_activity;
pub use unit_activity::{AcceptedSpan, UnitActivity};

#[path = "location_host.rs"]
mod locations;

#[path = "unit_visibility_host.rs"]
mod unit_visibility;

#[path = "unit_look_at_host.rs"]
mod unit_look_at;
pub use unit_look_at::UnitLookAt;

#[path = "unit_pause_host.rs"]
mod unit_pause;

#[path = "unit_sleep_host.rs"]
mod unit_sleep;
pub use unit_sleep::UnitSleep;

#[path = "unit_buff_host.rs"]
pub(crate) mod unit_buff;

#[path = "divine_shield_host.rs"]
pub(crate) mod divine_shield;

#[path = "camera_view_host.rs"]
mod camera_view;

#[cfg(test)]
#[path = "state_host_tests.rs"]
mod state_host_tests;

#[path = "environment_host.rs"]
mod scene;
pub use scene::{Environment, TerrainFog};

#[path = "unit_type_host.rs"]
mod unit_types;

#[path = "fog_modifier_host.rs"]
mod fog_modifiers;
pub use fog_modifiers::FogModifier;

#[path = "selection_host.rs"]
mod selection;

#[path = "dialog_host.rs"]
mod dialogs;
pub use dialogs::{DialogBudget, DialogButton, DialogQuitRequest, DialogState, ScriptDialog};

#[path = "cinematic_scene_host.rs"]
mod cinematic_scene;
pub use cinematic_scene::CinematicScene;

#[path = "indicator_host.rs"]
mod indicators;
pub use indicators::{Indicator, MinimapPing};

#[path = "campaign_targeting_host.rs"]
mod targeting;

#[path = "campaign_flee_host.rs"]
mod campaign_flee;

#[path = "campaign_gathering_host.rs"]
mod campaign_gathering;
pub use campaign_gathering::GatheringObservation;

#[path = "combat_host.rs"]
mod combat;
pub use combat::{Combat, CombatActor, CombatTraceEvent, Relation, Weapon};

#[path = "event_delivery_host.rs"]
mod event_delivery;
pub use event_delivery::{EventSubject, FiredEvent, HeroEvent, SaleContext, SummonContext};
pub use item_sale::{PurchaseOutcome, PurchaseRefusal};

#[path = "unit_vitals_host.rs"]
mod unit_vitals;
pub use unit_vitals::Vitals;

#[path = "volume_group_host.rs"]
mod volume_groups;

#[path = "unit_removal_host.rs"]
mod unit_removal;

#[path = "thread_host.rs"]
mod thread;

#[path = "time_of_day_host.rs"]
mod time_of_day;

#[path = "effect_host.rs"]
mod effects;
pub use effects::UnitEffect;

#[path = "cheat_host.rs"]
mod cheat_queries;

#[path = "player_removal_host.rs"]
mod player_removal;
pub use player_removal::{PlayerRemoval, PlayerRemovalBudget, PlayerRemovalState};

#[path = "game_pause_host.rs"]
mod game_pause;
#[path = "player_score_host.rs"]
mod player_score;
pub use player_score::{PlayerScoreCounter, PlayerScoreSnapshot};
#[path = "localized_queries_host.rs"]
mod localized_queries;

#[path = "group_order_host.rs"]
mod group_orders;

#[path = "game_exit_host.rs"]
mod game_exit;
pub use game_exit::GameExitRequest;

#[path = "unit_guard_host.rs"]
mod unit_guards;
pub use unit_guards::{GuardBudget, GuardState, UnitGuard};

#[path = "unit_summon_host.rs"]
mod unit_summon;

#[path = "holy_light_host.rs"]
mod holy_light;
