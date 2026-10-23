//! Caller-qualified map host: one object arena and six neutral wrappers.
//! Asset bytes, profiles, budgets and runtime observations stay with the caller.
use crate::camera_margins::CameraMargins;
use crate::day_night;
use crate::file_sounds;
use crate::label_sounds::SoundBank;
use crate::map_admission::ParsedMapContext;
use crate::neutral_player::{NeutralError, SelectedMapNeutralHost, VerifiedNeutralRule};
use crate::playercolor_host::{
    ConditionBudget, DestructableBudget, DialogBudget, EnumHost, Error as HostError, FilterBudget,
    ForceBudget, ItemBudget, Limits, MapConfigBudget, ObjectHost, PlayerAdmissionError,
    PlayerAllianceBudget, PlayerBudget, PlayerStateRequestBudget, QuestBudget, RegionBudget,
    ResearchObservation, StartupSettings, StockBudget, TechLimitBudget, TimerHost, TriggerBudget,
    UnitBudget, UnitStateBudget,
};
use crate::sound_parameters::Banks;
use crate::startup_clock::StartupClock;
use pcx_objects::editor::defaults::DefaultTables;
use pcx_script::{HandleId, Host, Program, Value};
use pcx_sim::world::ObjectCatalog;
use std::sync::Arc;

#[path = "map_new_game.rs"]
mod new_game;

#[derive(Debug, thiserror::Error)]
pub enum MapStartupError {
    #[error(transparent)]
    Host(Box<HostError>),
    #[error(transparent)]
    Players(Box<PlayerAdmissionError>),
    #[error(transparent)]
    Neutral(#[from] NeutralError),
    #[error("startup observation `{0}` is not an admitted handle")]
    Observation(String),
    #[error("startup placement: {0}")]
    Placement(String),
}

impl From<HostError> for MapStartupError {
    fn from(error: HostError) -> Self {
        Self::Host(Box::new(error))
    }
}
impl From<PlayerAdmissionError> for MapStartupError {
    fn from(error: PlayerAdmissionError) -> Self {
        Self::Players(Box::new(error))
    }
}

#[derive(Clone, Debug)]
pub struct MapStartupBudgets {
    pub inventory: Option<crate::inventory::InventoryBudget>,
    pub player_removal: Option<crate::playercolor_host::PlayerRemovalBudget>,
    pub groups: usize,
    pub map_config: MapConfigBudget,
    pub players: PlayerBudget,
    pub filters: FilterBudget,
    /// None keeps `Condition` unsupported.
    pub conditions: Option<ConditionBudget>,
    pub forces: ForceBudget,
    pub state_requests: PlayerStateRequestBudget,
    pub alliances: PlayerAllianceBudget,
    pub start_priorities: usize,
    pub sound_starts: usize,
    pub map_music_bytes: usize,
    pub file_sound_eax_bytes: usize,
    pub stacked_sounds: usize,
    pub camera_setup_fields: usize,
    pub tech_limits: TechLimitBudget,
    pub tech_researched: TechLimitBudget,
    pub ability_availability: usize,
    pub destructables: DestructableBudget,
    pub units: UnitBudget,
    pub unit_states: UnitStateBudget,
    pub triggers: TriggerBudget,
    /// None keeps trigger-condition registration unsupported.
    pub trigger_conditions: Option<usize>,
    /// None keeps player-unit event registration unsupported.
    pub player_unit_events: Option<usize>,
    /// None keeps widget death-event registration unsupported.
    pub death_events: Option<usize>,
    /// None keeps `TriggerRegisterUnitInRange` unsupported. Registration does not place or deliver.
    pub unit_in_range: Option<usize>,
    /// None keeps player-event registration unsupported.
    pub player_events: Option<usize>,
    /// None keeps region creation, rectangle requests and subscriptions unsupported.
    pub regions: Option<RegionBudget>,
    pub timer_events: usize,
    pub game_state_events: usize,
    pub player_state_events: usize,
    /// None keeps stock slot natives unsupported.
    pub stock: Option<StockBudget>,
    /// None keeps quest and quest-item natives unsupported.
    pub quests: Option<QuestBudget>,
    /// None keeps script-created dialogs and dialog events unsupported.
    pub dialogs: Option<DialogBudget>,
    /// None keeps `CreateItem` unsupported.
    pub items: Option<ItemBudget>,
    /// Pending `EndGame` requests awaiting the campaign consumer.
    pub game_exit: usize,
}

pub struct EnumContracts {
    pub context: String,
    pub names: Vec<String>,
    pub limits: Limits,
}

pub struct NeutralRuleSet {
    pub passive: Arc<VerifiedNeutralRule>,
    pub aggressive: Arc<VerifiedNeutralRule>,
    pub max_players: Arc<VerifiedNeutralRule>,
    pub victim: Arc<VerifiedNeutralRule>,
    pub extra: Arc<VerifiedNeutralRule>,
    pub slots: Arc<VerifiedNeutralRule>,
}
impl NeutralRuleSet {
    /// Opt in to the selected-successful-W3I reconstruction's first getter family.
    /// Discovery qualifies encodings, not the original active family or cache lifetime.
    pub fn discover_first_family(
        image: &[u8],
        origin: crate::scripts::AssetIdentity,
        budget: crate::neutral_player::DiscoveryBudget,
    ) -> Result<Self, NeutralError> {
        let [passive, aggressive, max_players, victim, extra, slots] =
            crate::neutral_rule_discovery::discover(image, origin, budget)?;
        Ok(Self {
            passive: Arc::new(passive),
            aggressive: Arc::new(aggressive),
            max_players: Arc::new(max_players),
            victim: Arc::new(victim),
            extra: Arc::new(extra),
            slots: Arc::new(slots),
        })
    }
}

pub struct StartupClockInput {
    pub clock: StartupClock,
    pub global_name: String,
    pub max_globals: usize,
    pub expression_depth: usize,
    pub expression_work: usize,
}

pub struct RandomInput {
    pub policy: Arc<crate::script_random::profile::Policy>,
    pub max_secondary: usize,
}

pub struct DefendSelection {
    pub tables: Arc<DefaultTables>,
    pub alias: String,
}

pub struct UnitGuardsInput {
    pub native: crate::unit_guard_rules::Rules,
    pub source: pcx_objects::editor::misc::CombatRules,
}

/// The Holy Light row, which the implementation is bound to.
pub const HOLY_LIGHT_ROW: [u8; 4] = *b"AHhb";

pub struct HeroSkillsInput {
    pub rules: crate::hero_skills::Rules,
    pub abilities: pcx_objects::editor::Overlay,
    pub budget: crate::playercolor_host::HeroBudget,
    /// Bytes of the Holy Light `Targetart` model. `None` leaves Holy Light unadmitted.
    pub holy_light_art: Option<Vec<u8>>,
}

pub struct CurrentResearchInput {
    pub entries: Vec<ResearchObservation>,
    pub budget: TechLimitBudget,
}

#[derive(Clone, Debug)]
pub struct SettingsObservation {
    pub speed_global: String,
    pub fog_enabled: bool,
    pub fog_mask_enabled: bool,
}

#[derive(Clone, Debug)]
pub struct SlotObservation {
    pub player_number: i32,
    pub state_global: String,
}

#[derive(Clone, Debug)]
pub struct ControllerObservation {
    pub player_number: i32,
    pub control_global: String,
}

/// Explicit recreation new-game input, not the original engine's seed mixer.
#[derive(Clone, Debug)]
pub struct NewGameInput {
    pub seed: i32,
    /// Selected client's initial passive alliances, applied after config and before main.
    pub initial_alliances: Option<crate::player_alliance_rules::Rules>,
    pub orders: Option<Arc<crate::order_registry::Registry>>,
    pub activity: Option<crate::unit_activity::Rules>,
    /// The map's `war3map.w3q` upgrade overrides, resolved by upgrade readers.
    pub upgrades: pcx_objects::editor::Overlay,
    /// Selected client's group-order rules; absent leaves group orders unsupported.
    pub group_orders: Option<crate::group_order_rules::Rules>,
    /// None leaves research queries unsupported rather than installing defaults.
    pub research: Option<crate::upgrade_research::Rules>,
    /// Source rules for a deferred placement cache, after stationary creations.
    pub placement_rules: Option<crate::unit_placement::Rules>,
    /// Selected common's unit-range event; absent leaves spatial delivery unset.
    pub unit_range_event: Option<String>,
    /// Selected `[Misc]` day rate; absent leaves time-of-day setters unsupported.
    pub day_cycle: Option<pcx_objects::editor::misc::DayCycle>,
    /// Selected game camera; absent leaves live camera natives unsupported.
    pub game_camera: Option<pcx_objects::editor::camera::GameCamera>,
    /// Selected hero attribute bonuses; absent leaves unit vitals unsupported.
    pub hero_vitals: Option<pcx_objects::editor::misc::HeroVitals>,
    /// Selected `[Misc]` combat rules; absent leaves attacks unsupported.
    pub combat: Option<pcx_objects::editor::misc::CombatRules>,
}

pub struct ResolvedNewGame {
    initial_alliances: Option<crate::player_alliance_rules::Rules>,
    empty: HandleId,
    playing: HandleId,
    none: HandleId,
    creep: HandleId,
}

/// Source of the `GetDefaultDifficulty` fallback. The app supplies no profile input, so only
/// the native fallback applies; saved `.w3p`/registry profiles are not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultDifficultyObservation {
    /// `native_fallback` is the ordinal the selected image returns without a profile entry.
    NoProfile { native_fallback: i32 },
}

impl DefaultDifficultyObservation {
    fn fallback(&self) -> i32 {
        match self {
            Self::NoProfile { native_fallback } => *native_fallback,
        }
    }
}

/// `None` skips that admission. It does not install a lobby, speed or research default.
#[derive(Clone, Debug, Default)]
pub struct PostConfigObservations {
    /// Initialize the selected RNG before globals and derive a fresh roster after config.
    /// Participants and local player must be explicit. Do not mix with slot/controller snapshots.
    pub new_game: Option<NewGameInput>,
    /// This client's explicit source slot; not inferred from participants or controllers.
    pub local_player: Option<i32>,
    pub participants: Option<Vec<i32>>,
    pub settings: Option<SettingsObservation>,
    pub version_global: Option<String>,
    /// Selected `gamedifficulty` constant, an explicit game input.
    pub difficulty_global: Option<String>,
    /// Default difficulty for `GetDefaultDifficulty`, separate from the current difficulty.
    pub default_difficulty: Option<DefaultDifficultyObservation>,
    /// Explicit source numbers, including players not yet created by the script.
    pub slot_states: Option<Vec<SlotObservation>>,
    pub controllers: Option<Vec<ControllerObservation>>,
}

pub struct ResolvedObservations {
    pub new_game: Option<ResolvedNewGame>,
    pub local_player: Option<i32>,
    pub participants: Option<Vec<i32>>,
    pub settings: Option<StartupSettings>,
    pub version: Option<HandleId>,
    pub difficulty: Option<HandleId>,
    pub default_difficulty: Option<i32>,
    pub slot_states: Option<Vec<(i32, HandleId)>>,
    pub controllers: Option<Vec<(i32, HandleId)>>,
}

#[path = "sound_playback.rs"]
pub mod sound_playback;

pub struct MapStartupInputs {
    pub ai_source: Option<crate::campaign_ai::SourceNames>,
    /// Checked client operands used by campaign automatic target selection.
    pub targeting_policy: Option<crate::campaign_targeting_policy::TargetingPolicy>,
    /// Checked client operands used by ordinary campaign retreat decisions.
    pub flee_policy: Option<crate::campaign_flee_policy::FleePolicy>,
    /// Checked client operands used by campaign gathering associations.
    pub gathering_policy: Option<crate::campaign_gathering_policy::GatheringPolicy>,
    pub enumeration: EnumContracts,
    pub rules: NeutralRuleSet,
    pub budgets: MapStartupBudgets,
    pub camera: CameraMargins,
    pub day_night: day_night::Assets,
    pub label_sounds: SoundBank,
    pub label_sound_creation: Vec<SoundBank>,
    pub file_sounds: file_sounds::Assets,
    pub sound_parameters: Banks,
    /// Every `EnvironmentType` record from the selected sources; empty keeps NewSoundEnvironment unsupported.
    /// Records and executable tables; absent keeps NewSoundEnvironment unsupported.
    pub sound_environments: Option<crate::sound_environment::SoundEnvironmentInput>,
    pub sound_playback: Option<sound_playback::Rules>,
    pub resources: Option<crate::player_resources::Rules>,
    pub inventory: Option<crate::inventory::Rules>,
    /// Requires vitals and inventory admission; absent leaves automatic powerups unsupported.
    pub inventory_powerup: Option<crate::inventory_powerup::Rules>,
    /// Required with item budgets: class ordinals derived from the selected executable.
    pub item_classes: Option<crate::item_class_table::ItemClassTable>,
    /// Required with item budgets: sale rules derived from the loaded item metadata.
    pub item_sales: Option<crate::item_sales::ItemSaleTable>,
    /// Required with item budgets: charge and cooldown rules from the selected ItemData and ability rows.
    pub item_use: Option<crate::item_use_rules::Rules>,
    pub localization: Option<crate::script_localization::Bank>,
    pub unit_guards: Option<UnitGuardsInput>,
    pub player_removal_rules: Option<crate::player_removal_rules::Rules>,
    pub cheats: Option<crate::cheat_rules::Rules>,
    pub dialog_layout: Option<crate::dialog_layout::Rules>,
    pub message_layout: Option<crate::message_layout::Rules>,
    pub clock: StartupClockInput,
    /// Explicit game time for positive-timeout timers; absent keeps TimerStart unsupported.
    pub timer_time: Option<f64>,
    /// Explicit trigger defaults and loaded timer event for callback delivery.
    pub trigger_runtime: Option<crate::playercolor_host::TriggerRuntimeInput>,
    /// Source-qualified parameters without an invented initial engine seed.
    pub random: Option<RandomInput>,
    /// Source-qualified static WPM queries; absent keeps IsTerrainPathable unsupported.
    pub terrain_pathing: Option<crate::terrain_pathing::TerrainPathing>,
    /// Selected real literal and operator policy; absent retains host arithmetic.
    pub numeric: Option<crate::script_numeric::NumericPolicy>,
    pub destructable_restore: Option<crate::playercolor_host::RestoreLifePolicy>,
    /// Source-qualified XP-factor limits; absent leaves the handicap natives unsupported.
    pub experience_handicap: Option<crate::player_handicap::Rules>,
    /// Source-qualified health-factor limits and unit-pool rounding.
    pub health_handicap: Option<crate::player_handicap::HealthRules>,
    pub hero_skills: Option<HeroSkillsInput>,
    pub catalog: Arc<dyn ObjectCatalog + Send + Sync>,
    pub defend: DefendSelection,
    pub current_research: Option<CurrentResearchInput>,
    pub post_config: PostConfigObservations,
}

pub(crate) type NeutralStack = SelectedMapNeutralHost<
    SelectedMapNeutralHost<
        SelectedMapNeutralHost<
            SelectedMapNeutralHost<SelectedMapNeutralHost<SelectedMapNeutralHost<ObjectHost>>>,
        >,
    >,
>;

pub struct MapStartupHost {
    sound_playback: Option<sound_playback::Rules>,
    inner: Option<NeutralStack>,
    ai: crate::campaign_ai::Runtime<NeutralStack>,
    research: Option<CurrentResearchInput>,
    day_night_loader: Option<DayNightLoader>,
    placement_rules: Option<crate::unit_placement::Rules>,
    placement_tables: Option<Arc<pcx_objects::editor::defaults::DefaultTables>>,
    placement_loader: Option<PlacementLoader>,
}

type DayNightLoader = Box<dyn FnMut(&str, &str) -> Result<day_night::Assets, String> + Send + Sync>;
type PlacementLoader = Box<
    dyn FnMut(
            &ObjectHost,
            &crate::unit_placement::Rules,
            &pcx_objects::editor::defaults::DefaultTables,
        ) -> Result<crate::unit_placement::Placement, String>
        + Send
        + Sync,
>;

impl MapStartupHost {
    fn world(&self) -> &NeutralStack {
        self.inner.as_ref().expect("map world is not lent to AI")
    }

    fn world_mut(&mut self) -> &mut NeutralStack {
        self.inner.as_mut().expect("map world is not lent to AI")
    }

    pub fn with_ai_loader(
        mut self,
        loader: impl FnMut(&str) -> Result<Arc<crate::scripts::MapScripts>, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.ai.set_loader(loader);
        self
    }

    pub(crate) fn next_ai_wake(&mut self, until: f64) -> Option<(f64, i32, u64)> {
        self.ai.next_wake(until)
    }

    pub(crate) fn resume_ai(&mut self, player: i32, ticket: u64) -> Result<(), String> {
        self.ai.resume(&mut self.inner, player, ticket)
    }

    pub fn sound_playback(&self) -> Option<&sound_playback::Rules> {
        self.sound_playback.as_ref()
    }
    pub fn with_placement_loader(
        mut self,
        loader: impl FnMut(
            &ObjectHost,
            &crate::unit_placement::Rules,
            &pcx_objects::editor::defaults::DefaultTables,
        ) -> Result<crate::unit_placement::Placement, String>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.placement_loader = Some(Box::new(loader));
        self
    }
    /// Resolve the executed paths on this session's retained source reader.
    pub fn with_day_night_loader(
        mut self,
        loader: impl FnMut(&str, &str) -> Result<day_night::Assets, String> + Send + Sync + 'static,
    ) -> Self {
        self.day_night_loader = Some(Box::new(loader));
        self
    }
    pub fn context(&self) -> &Arc<ParsedMapContext> {
        self.world().context()
    }

    pub fn objects(&self) -> &ObjectHost {
        self.world().inner().inner().inner().inner().inner().inner()
    }

    pub fn objects_mut(&mut self) -> &mut ObjectHost {
        self.world_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
            .inner_mut()
    }

    pub fn apply_post_config(
        &mut self,
        context: &Arc<ParsedMapContext>,
        observations: &ResolvedObservations,
    ) -> Result<(), MapStartupError> {
        let fresh = observations
            .new_game
            .as_ref()
            .map(|roles| self.fresh_observations(context, observations, roles))
            .transpose()?;
        let observations = fresh.as_ref().unwrap_or(observations);
        if let Some(number) = observations.local_player {
            self.objects_mut().admit_local_player(context, number)?;
        }
        if let Some(numbers) = &observations.participants {
            let mut players = Vec::new();
            players
                .try_reserve_exact(numbers.len())
                .map_err(|_| MapStartupError::Observation("startup participants".into()))?;
            for number in numbers {
                let id = self
                    .objects_mut()
                    .player_identity(*number)
                    .ok_or_else(|| MapStartupError::Observation(format!("player {number}")))?;
                players.push(id);
            }
            self.objects_mut()
                .admit_startup_force_players(context, &players)?;
        }
        if let Some(settings) = observations.settings {
            self.objects_mut()
                .admit_startup_settings(context, settings)?;
        }
        if let Some(version) = observations.version {
            self.objects_mut().admit_startup_version(context, version)?;
        }
        if let Some(fallback) = observations.default_difficulty {
            self.objects_mut()
                .admit_default_difficulty(context, fallback)?;
        }
        if let Some(difficulty) = observations.difficulty {
            self.objects_mut()
                .admit_game_difficulty(context, difficulty)?;
        }
        if let Some(states) = &observations.slot_states {
            self.objects_mut()
                .admit_startup_player_slot_numbers(context, states)?;
        }
        if let Some(entries) = &observations.controllers {
            self.objects_mut()
                .admit_startup_controllers(context, entries)?;
        }
        Ok(())
    }
}

pub fn resolve_observations(
    program: &Program,
    observations: &PostConfigObservations,
    mut read_global: impl FnMut(&str) -> Value,
) -> Result<ResolvedObservations, MapStartupError> {
    let mut handle = |name: &str| -> Result<HandleId, MapStartupError> {
        if program.global(name).is_none() {
            return Err(MapStartupError::Observation(name.to_owned()));
        }
        match read_global(name) {
            Value::Handle(id) => Ok(id),
            _ => Err(MapStartupError::Observation(name.to_owned())),
        }
    };
    let new_game = observations
        .new_game
        .as_ref()
        .map(|input| -> Result<_, MapStartupError> {
            Ok(ResolvedNewGame {
                initial_alliances: input.initial_alliances.clone(),
                empty: handle("PLAYER_SLOT_STATE_EMPTY")?,
                playing: handle("PLAYER_SLOT_STATE_PLAYING")?,
                none: handle("MAP_CONTROL_NONE")?,
                creep: handle("MAP_CONTROL_CREEP")?,
            })
        })
        .transpose()?;
    let settings = match &observations.settings {
        Some(settings) => Some(StartupSettings {
            speed: handle(&settings.speed_global)?,
            fog_enabled: settings.fog_enabled,
            fog_mask_enabled: settings.fog_mask_enabled,
        }),
        None => None,
    };
    let version = match &observations.version_global {
        Some(name) => Some(handle(name)?),
        None => None,
    };
    let difficulty = match &observations.difficulty_global {
        Some(name) => Some(handle(name)?),
        None => None,
    };
    let mut slot_states = None;
    if let Some(rows) = &observations.slot_states {
        let mut resolved = Vec::new();
        resolved
            .try_reserve_exact(rows.len())
            .map_err(|_| MapStartupError::Observation("startup slot states".into()))?;
        for row in rows {
            resolved.push((row.player_number, handle(&row.state_global)?));
        }
        slot_states = Some(resolved);
    }
    let mut controllers = None;
    if let Some(rows) = &observations.controllers {
        let mut resolved = Vec::new();
        resolved
            .try_reserve_exact(rows.len())
            .map_err(|_| MapStartupError::Observation("startup controllers".into()))?;
        for row in rows {
            resolved.push((row.player_number, handle(&row.control_global)?));
        }
        controllers = Some(resolved);
    }
    Ok(ResolvedObservations {
        new_game,
        local_player: observations.local_player,
        participants: observations.participants.clone(),
        settings,
        version,
        difficulty,
        default_difficulty: observations
            .default_difficulty
            .as_ref()
            .map(DefaultDifficultyObservation::fallback),
        slot_states,
        controllers,
    })
}

pub fn assemble(
    context: Arc<ParsedMapContext>,
    inputs: MapStartupInputs,
) -> Result<MapStartupHost, MapStartupError> {
    let MapStartupInputs {
        ai_source: _,
        targeting_policy,
        flee_policy,
        gathering_policy,
        enumeration,
        rules,
        budgets,
        camera,
        day_night,
        label_sounds,
        label_sound_creation,
        file_sounds,
        sound_parameters,
        sound_environments,
        sound_playback,
        resources,
        inventory,
        inventory_powerup,
        item_classes,
        item_sales,
        item_use,
        localization,
        unit_guards,
        player_removal_rules,
        cheats,
        dialog_layout,
        message_layout,
        clock,
        timer_time,
        trigger_runtime,
        random,
        terrain_pathing,
        numeric,
        destructable_restore,
        experience_handicap,
        health_handicap,
        hero_skills,
        catalog,
        defend,
        current_research,
        post_config,
    } = inputs;
    let enum_host = {
        let names: Vec<&str> = enumeration.names.iter().map(String::as_str).collect();
        EnumHost::checked_selected(
            context.scripts.shared_program(),
            &enumeration.context,
            &names,
            enumeration.limits,
        )?
    };
    let empty_abilities = pcx_objects::editor::Overlay::default();
    let abilities = hero_skills
        .as_ref()
        .map_or(&empty_abilities, |input| &input.abilities);
    let harvest_abilities = abilities.clone();
    let mut object = TimerHost::checked(enum_host)?
        .with_groups(budgets.groups)?
        .with_map_config(budgets.map_config)?
        .with_players(
            Arc::clone(&context),
            Arc::clone(&rules.slots),
            budgets.players,
        )?
        .with_filters(budgets.filters)?;
    if let Some(budget) = budgets.conditions {
        object = object.with_conditions(budget)?;
    }
    object = object
        .with_forces(budgets.forces)?
        .with_player_state_requests(budgets.state_requests)?
        .with_player_alliances(budgets.alliances)?
        .with_start_priorities(budgets.start_priorities)?
        .with_camera(camera)?
        .with_camera_bound_getters()?
        .with_day_night(day_night)?
        .with_label_sounds(label_sounds)?
        .with_label_sound_creation(label_sound_creation)?
        .with_sound_starts(budgets.sound_starts)?
        .with_map_music_selection(budgets.map_music_bytes)?
        .with_file_sounds(file_sounds, budgets.file_sound_eax_bytes)?
        .with_sound_parameter_banks(sound_parameters)?
        .with_sound_duration()?
        .with_startup_clock(
            clock.clock,
            &clock.global_name,
            clock.max_globals,
            clock.expression_depth,
            clock.expression_work,
        )?
        .with_rectangles()?
        .with_sound_properties()?
        .with_stacked_sounds(budgets.stacked_sounds)?
        .with_camera_setups(budgets.camera_setup_fields)?
        .with_player_tech_limits(budgets.tech_limits)?
        .with_player_tech_researched(budgets.tech_researched)?
        .with_player_abilities(budgets.ability_availability)?
        .with_destructables(budgets.destructables)?
        .with_unit_catalog(
            Arc::clone(&catalog),
            context.world.source.clone(),
            budgets.units,
        )?
        .with_unit_states(budgets.unit_states)?
        .with_resource_amounts(Arc::clone(&defend.tables))?
        .with_defend_ability(&defend.tables, &defend.alias)?
        .with_militia_ability(&defend.tables, abilities)?;
    // Retained SetUnitExploded flags only; explosion presentation is not implemented.
    object = object.with_unit_explosions(budgets.units.max_records)?;
    if let Some(rules) = resources {
        object = object.with_player_resources(rules)?;
    }
    if let Some(rules) = cheats {
        object = object.with_cheat_queries(rules)?;
    }
    if context
        .scripts
        .program()
        .natives()
        .get("Location")
        .is_some()
    {
        object = object.with_locations()?;
    }
    if context
        .scripts
        .program()
        .natives()
        .get("AddSpecialEffectTarget")
        .is_some()
    {
        object = object.with_unit_effects()?;
    }
    object = object.with_environment()?.with_volume_groups()?;
    if let Some(input) = sound_environments {
        object = object.with_sound_environments(input)?;
    }
    if let Some(rules) = &sound_playback {
        object.configure_volume_group_playback(rules.group_policy());
    }
    object = object
        .with_indicators(256)?
        .with_cinematic_scenes()?
        .with_selection()?;
    if context
        .scripts
        .program()
        .natives()
        .get("Location")
        .is_some()
    {
        object = object.with_fog_modifiers()?;
    }
    object = object.with_game_pause()?;
    if let Some(bank) = localization {
        object = object.with_localized_queries(bank)?;
    }
    let tables = defend.tables;
    let holy_light_art = hero_skills
        .as_ref()
        .and_then(|input| input.holy_light_art.clone());
    if let Some(input) = hero_skills {
        object = object.with_hero_skills(
            Arc::clone(&tables),
            context.sources.catalog.clone(),
            input.abilities,
            input.rules,
            input.budget,
        )?;
    }
    object = object
        .with_trigger_admission(budgets.triggers)?
        .with_timer_events(budgets.timer_events)?
        .with_game_state_events(budgets.game_state_events)?
        .with_player_state_events(budgets.player_state_events)?;
    if let Some(limit) = budgets.trigger_conditions {
        object = object.with_trigger_conditions(limit)?;
    }
    if let Some(limit) = budgets.player_unit_events {
        object = object.with_player_unit_events(limit)?;
    }
    if let Some(limit) = budgets.death_events {
        object = object.with_death_events(limit)?;
    }
    if let Some(limit) = budgets.unit_in_range {
        object = object.with_unit_in_range(limit)?;
    }
    if let Some(limit) = budgets.player_events {
        object = object.with_player_events(limit)?;
    }
    match (player_removal_rules, budgets.player_removal) {
        (Some(rules), Some(budget)) => object = object.with_player_removal(rules, budget)?,
        (None, None) => {}
        _ => {
            return Err(MapStartupError::Observation(
                "player removal rules and budget must be supplied together".into(),
            ));
        }
    }
    if let Some(budget) = budgets.regions {
        object = object.with_regions(budget)?;
    }
    object = object.with_game_exit(budgets.game_exit)?;
    if context
        .scripts
        .program()
        .natives()
        .get("GetPlayerScore")
        .is_some()
    {
        object = object.with_player_scores()?;
    }
    if let Some(now) = timer_time {
        object = object.with_timer_runtime(now)?;
    }
    if let Some(input) = trigger_runtime {
        let parked = input.max_parked_threads;
        object = object.with_trigger_runtime(input)?;
        if context
            .scripts
            .program()
            .natives()
            .get("TriggerSleepAction")
            .is_some()
        {
            object.admit_thread_sleep(parked)?;
        }
    }
    if let Some(budget) = budgets.dialogs {
        object = object.with_dialogs(budget)?;
    }
    if let Some(rules) = dialog_layout {
        object = object.with_dialog_layout(rules)?;
    }
    if let Some(rules) = message_layout {
        object = object.with_message_layout(rules)?;
    }
    if let Some(policy) = numeric {
        object = object.with_script_numeric(policy);
    }
    if let Some(policy) = destructable_restore {
        object = object.with_destructable_restore(policy)?;
    }
    if let Some(rules) = experience_handicap {
        object = object.with_player_experience_handicap(rules)?;
    }
    if let Some(rules) = health_handicap {
        object = object.with_player_health_handicap(rules)?;
    }
    if let Some(input) = random {
        object = object.with_script_random(input.policy, input.max_secondary)?;
    }
    let mut placement_rules = None;
    if let Some(input) = post_config.new_game {
        let had_orders = input.orders.is_some();
        if let Some(orders) = input.orders {
            object = object.with_order_registry(orders)?;
        }
        if let Some(activity) = input.activity {
            object.admit_unit_activity(activity, 65536)?;
            object = object.with_harvest(
                Arc::clone(&tables),
                harvest_abilities,
                input.upgrades,
                crate::playercolor_host::harvest_runtime::Budget::new(
                    budgets.units.max_records,
                    65536,
                ),
            )?;
        }
        if let Some(rules) = input.group_orders {
            object = object.with_group_orders(rules)?;
        }
        placement_rules = input.placement_rules;
        if let (Some(rules), Some(_)) = (&placement_rules, &input.hero_vitals) {
            let blight = crate::terrain_blight::InitialBlight::read(
                &context.terrain,
                rules.cell_size() * context.alignment.uniform_scale as f32,
                context.sources.map.clone(),
                context.input_budget.terrain_bytes / pcx_map::CORNER_RECORD,
            )
            .map_err(|e| {
                MapStartupError::Observation(format!("{}: {e}", context.sources.map.source))
            })?;
            object.admit_initial_blight(blight)?;
        }
        if let Some(event) = input.unit_range_event {
            object.admit_unit_range_runtime(&event, 65536)?;
        }
        if let Some(cycle) = &input.day_cycle {
            object.admit_time_of_day(cycle)?;
        }
        if let Some(camera) = &input.game_camera {
            object.admit_camera_view(camera)?;
        }
        if let Some(bonuses) = input.hero_vitals {
            object.admit_unit_vitals(bonuses)?;
        }
        if let Some(rules) = input.combat {
            object.admit_combat(rules, "attack")?;
            if object.has_hero_skills() {
                object.admit_water_elemental()?;
            }
            if let (true, Some(art)) = (object.has_hero_skills(), holy_light_art.as_deref()) {
                object.admit_holy_light(art)?;
            }
            if object.has_hero_skills() && had_orders {
                object.admit_divine_shield()?;
            }
        }
        if let Some(rules) = input.research {
            object.admit_upgrade_research(&context, rules)?;
        }
        object
            .native("SetRandomSeed", &[Value::Int(input.seed)])
            .map_err(MapStartupError::Observation)?;
    }
    if let Some(input) = unit_guards {
        object = object.with_unit_guards(
            input.native,
            input.source,
            crate::playercolor_host::GuardBudget {
                max_units: budgets.units.max_records,
            },
        )?;
    }
    if let Some(policy) = targeting_policy {
        object.admit_campaign_targeting(&context, policy)?;
    }
    if let Some(policy) = flee_policy {
        object.admit_campaign_flee(&context, policy)?;
    }
    if let Some(policy) = gathering_policy {
        object.admit_campaign_gathering(&context, policy)?;
    }
    if let Some(input) = terrain_pathing {
        object = object.with_terrain_pathing(input)?;
    }
    if let Some(stock) = budgets.stock {
        object = object.with_stock(stock)?;
    }
    if let Some(quests) = budgets.quests {
        object = object.with_quests(quests)?;
    }
    if let Some(items) = budgets.items {
        object = object.with_item_catalog(catalog, context.world.source.clone(), items)?;
        object = object.with_item_drops(items.max_records)?;
        let item_use = item_use.ok_or_else(|| {
            MapStartupError::Observation("item use requires charge and cooldown rules".into())
        })?;
        object = object.with_item_use(item_use, items.max_records)?;
        let classes = item_classes.ok_or_else(|| {
            MapStartupError::Observation("item queries require derived item classes".into())
        })?;
        object = object.with_item_queries(classes)?;
        let sales = item_sales.ok_or_else(|| {
            MapStartupError::Observation("item sale rules require loaded item metadata".into())
        })?;
        object = object.with_item_sales(sales)?;
    }
    match (inventory, budgets.inventory) {
        (Some(rules), Some(budget)) => object = object.with_inventory(rules, budget)?,
        (None, None) => {}
        _ => {
            return Err(MapStartupError::Observation(
                "inventory rules and budget must be supplied together".into(),
            ));
        }
    }
    if let Some(rules) = inventory_powerup {
        object = object.with_inventory_powerup(rules)?;
    }
    let slots = SelectedMapNeutralHost::try_new(Arc::clone(&context), rules.slots, object)?;
    let extra = SelectedMapNeutralHost::try_new(Arc::clone(&context), rules.extra, slots)?;
    let victim = SelectedMapNeutralHost::try_new(Arc::clone(&context), rules.victim, extra)?;
    let max_players =
        SelectedMapNeutralHost::try_new(Arc::clone(&context), rules.max_players, victim)?;
    let aggressive =
        SelectedMapNeutralHost::try_new(Arc::clone(&context), rules.aggressive, max_players)?;
    let inner = SelectedMapNeutralHost::try_new(context, rules.passive, aggressive)?;
    let neutral_passive = inner.selected_result();
    let neutral_aggressive = inner.inner().selected_result();
    let mut host = MapStartupHost {
        sound_playback,
        inner: Some(inner),
        ai: crate::campaign_ai::Runtime::default(),
        research: current_research,
        day_night_loader: None,
        placement_tables: placement_rules.as_ref().map(|_| tables),
        placement_rules,
        placement_loader: None,
    };
    host.objects_mut().set_neutral_passive(neutral_passive);
    host.objects_mut()
        .set_neutral_aggressive(neutral_aggressive);
    Ok(host)
}

impl Host for MapStartupHost {
    fn begin_script_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<pcx_script::ScriptCall>, String> {
        self.world_mut().begin_script_call(name, args)
    }
    fn continue_script_call(&mut self, value: Value) -> Result<pcx_script::ScriptCall, String> {
        self.world_mut().continue_script_call(value)
    }
    fn abort_script_calls(&mut self) {
        self.world_mut().abort_script_calls();
    }
    fn real_ops(&self) -> Option<&dyn pcx_script::RealOps> {
        self.world().real_ops()
    }
    fn suspend(&mut self, name: &str, args: &[Value]) -> bool {
        self.world_mut().suspend(name, args)
    }
    fn park_thread(&mut self) -> Result<u64, String> {
        self.world_mut().park_thread()
    }
    fn resume_thread(&mut self, ticket: u64) -> Result<(), String> {
        self.world_mut().resume_thread(ticket)
    }
    fn leave_thread(&mut self) -> Result<(), String> {
        self.world_mut().leave_thread()
    }
    fn after_script_spawn(&mut self) -> Result<Option<pcx_script::ScriptCall>, String> {
        self.world_mut().after_script_spawn()
    }

    fn native(&mut self, name: &str, args: &[Value]) -> Result<Value, String> {
        if name == "StartCampaignAI" {
            let scripts = &self.context().scripts;
            let declaration = scripts
                .program()
                .natives()
                .get(name)
                .ok_or("StartCampaignAI: selected common does not declare the native")?;
            if scripts
                .digests
                .first()
                .is_none_or(|common| declaration.source != common.label)
                || declaration.params.len() != 2
                || declaration.params[0].ty.kind.name() != "player"
                || declaration.params[1].ty.kind.name() != "string"
                || declaration.ret.kind.name() != "nothing"
            {
                return Err("StartCampaignAI: selected common declaration differs".into());
            }
            let [Value::Handle(player), Value::Str(path)] = args else {
                return Err("StartCampaignAI: expected player and script path".into());
            };
            let slots = self
                .objects()
                .player_slots()
                .ok_or("StartCampaignAI: players are not admitted")?;
            let ordinal = (0..slots)
                .filter_map(|slot| i32::try_from(slot).ok())
                .find(|&slot| self.objects().player_identity(slot) == Some(*player))
                .ok_or("StartCampaignAI: player is not a current identity")?;
            self.ai.start(&mut self.inner, ordinal, path)?;
            return Ok(Value::Null);
        }
        if name == "CreateUnit"
            && let Some(rules) = &self.placement_rules
        {
            let mut loader = self
                .placement_loader
                .take()
                .ok_or("CreateUnit: fresh placement source loader is missing")?;
            let result = loader(
                self.objects(),
                rules,
                self.placement_tables
                    .as_deref()
                    .ok_or("CreateUnit: placement catalog is missing")?,
            );
            self.placement_loader = Some(loader);
            self.objects_mut()
                .admit_unit_placement(result?)
                .map_err(|e| e.to_string())?;
            self.placement_rules = None;
            self.placement_tables = None;
            self.placement_loader = None;
        }
        if name == "SetDayNightModels"
            && let [Value::Str(terrain), Value::Str(unit)] = args
        {
            let loaded = self.objects().day_night_state().is_some_and(|state| {
                [terrain, unit].iter().all(|path| {
                    state
                        .assets()
                        .models()
                        .iter()
                        .any(|model| model.script_path() == path.as_str())
                })
            });
            if !loaded && let Some(loader) = &mut self.day_night_loader {
                let assets = loader(terrain, unit)?;
                return self
                    .objects_mut()
                    .apply_day_night_assets(assets, [terrain, unit])
                    .map_err(|error| error.to_string());
            }
        }
        if name == "GetPlayerTechResearched"
            && let Some(pending) = self.research.take()
        {
            let context = Arc::clone(self.world().context());
            self.objects_mut()
                .admit_current_research(&context, pending.entries, pending.budget)
                .map_err(|error| error.to_string())?;
        }
        self.world_mut().native(name, args)
    }
    fn table_string(&self, id: u32) -> Option<String> {
        self.world().table_string(id)
    }
    fn resolve_table_string(&self, id: u32) -> Result<Option<String>, String> {
        self.world().resolve_table_string(id)
    }
    fn code_tag(&self, tag: &str) -> Option<String> {
        self.world().code_tag(tag)
    }
}
