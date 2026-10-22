//! Executed `CreateUnit` joined to the caller's already selected catalog.
//!
//! The catalog value is the same `Arc` the caller holds. This module does not
//! load SLK, overlays, or map files. Record budget and arena capacity are
//! checked before `resolve` and before the per-unit `Source` clone. Trigger
//! creation does not read preplaced owners. The integer rawcode is
//! `i32::to_be_bytes`. `base_rawcode` is stored as the catalog returned it.
//!
//! `SetUnitState` is a separate opt-in on that same unit. It retains the
//! executed `ConvertUnitState` handle and finite value. Current life and mana
//! assignments are clamped to the live maximum; maximum-life and maximum-mana
//! requests are retained but leave the pools unchanged. It does not spend a
//! resource or apply combat.
//!
//! `IssueImmediateOrder` has two separate opt-ins. The Defend opt-in takes one
//! caller-named ability alias; the Militia opt-in selects the authored `Amil`
//! row. Admission reads the selected tables the caller already holds and keeps
//! a snapshot. It does not open archives or scan every order pair.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use super::players::TechRead;
use pcx_objects::editor::defaults::{self, DefaultTables, Family};
use pcx_objects::editor::{FieldKind, FieldSource, ObjectKind, Overlay, SlkId};
use pcx_script::span::Span;
use pcx_script::{HandleId, Value, ast::Expr};
use pcx_sim::world::{
    Definition, FieldValue, ObjectCatalog, ObjectField, ResolutionProblem, Source,
};
use std::sync::Arc;

#[path = "unit_hero_host.rs"]
mod hero;
pub use hero::{HeroBudget, HeroProgress, UnitAbilities};

const NATIVE: &str = "CreateUnit";
const GET_TYPE: &str = "GetUnitTypeId";
const GET_OWNER: &str = "GetOwningPlayer";
const SET_OWNER: &str = "SetUnitOwner";
const GET_COLLISION: &str = "BlzGetUnitCollisionSize";
const SET_RANGE: &str = "SetUnitAcquireRange";
const SET_HERO_LEVEL: &str = "SetHeroLevel";
const SET_INVULNERABLE: &str = "SetUnitInvulnerable";
const GET_INVULNERABLE: &str = "BlzIsUnitInvulnerable";
const SET_STATE: &str = "SetUnitState";
const CONVERT_STATE: &str = "ConvertUnitState";
const SET_RESOURCE: &str = "SetResourceAmount";
const ADD_RESOURCE: &str = "AddResourceAmount";
const GET_RESOURCE: &str = "GetResourceAmount";
const ORDER: &str = "IssueImmediateOrder";
const STATE_KIND: &str = "unitstate";
const KIND: &str = "unit";
/// Binary class binding: `Agld` maps to the amount-owning `Agmb` class.
const RESOURCE_ALIAS: &str = "Agld";
/// Authored ability class for the source-backed Call to Arms off slice.
const MILITIA_ALIAS: &str = "Amil";
/// Caller bounds. Not a world, population, or rendering limit.
#[derive(Clone, Copy, Debug)]
pub struct UnitBudget {
    pub max_records: usize,
    /// Maximum `Definition.fields` retained from one successful resolve.
    pub max_retained_fields: usize,
    /// Maximum authored modifications retained from one successful resolve.
    pub max_modifications: usize,
}

/// Caller bound on retained `(unit, unitstate handle)` pairs.
///
/// Not a life, mana, population, or combat limit.
#[derive(Clone, Copy, Debug)]
pub struct UnitStateBudget {
    pub max_records: usize,
}

/// One executed `SetUnitState` pair on an admitted unit.
///
/// `unitstate` is the handle from the selected `ConvertUnitState` domain.
/// The domain ordinal is not stored. `value` is the exact finite input:
/// `0.0` is an assignment, and a handle absent from the unit's list has not
/// been assigned. A maximum-life or maximum-mana request keeps the exact
/// input but does not overwrite the live maximum. This is not a resource
/// balance or a derived maximum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExecutedUnitState {
    pub unitstate: HandleId,
    pub value: f32,
}

/// Checked catalog result plus the executed call. `executed_owner` is the
/// player number from this call, not a preplaced control word.
/// `owner` is the current existing player handle; ownership transfers preserve
/// the creation-time `executed_owner` number.
///
/// `requested_hero_level` and `requested_show_eye_candy` record one executed
/// `SetHeroLevel` call. They are not derived experience, hero stats, or
/// eye-candy effects.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutedUnit {
    pub definition: Definition,
    /// Derived once from the retained definition; invalid geometry refuses only when queried.
    /// Runtime collision setters must update this value if they are admitted later.
    pub collision_radius: Result<f32, &'static str>,
    pub executed_owner: i32,
    /// Current player handle, initially the executed `CreateUnit` argument.
    pub owner: HandleId,
    /// Known team-color identity assigned by a color-changing transfer. Initial
    /// unit color stays unknown until its class/creation color rules are supplied.
    pub team_color: Option<HandleId>,
    /// Assigned creation-player colour, captured before later ownership transfers.
    pub creation_color: Option<HandleId>,
    pub source: Source,
    /// Executed `CreateUnit` x. Live acceptance is still unresolved.
    pub x: f32,
    /// Executed `CreateUnit` y. Live acceptance is still unresolved.
    pub y: f32,
    /// Accepted world position on this same handle, when creation placement is admitted.
    /// `x` and `y` remain the executed request.
    pub position: Option<[f32; 2]>,
    pub face: f32,
    /// Executed `SetUnitAcquireRange` real. `None` means the native has not
    /// run for this handle. `Some(0.0)` is an executed zero, not that absence.
    pub acquire_range: Option<f32>,
    /// Executed `SetHeroLevel` integer. `None` means that native has not run.
    /// `Some(0)` and negative values are the executed input, not a clamp or
    /// a derived level.
    pub requested_hero_level: Option<i32>,
    /// Executed `showEyeCandy` argument. `None` means `SetHeroLevel` has not
    /// run. `Some(false)` is an executed false, not that absence, and it does
    /// not play an effect.
    pub requested_show_eye_candy: Option<bool>,
    /// Executed `SetUnitState` pairs. Empty means none have run. A stored
    /// `0.0` is an executed value, not an omitted state.
    pub unit_states: Vec<ExecutedUnitState>,
    /// Executed resource amount. `None` means the unit is not in the
    /// `UnitData` `buffType=resource` family, or resource amounts were not
    /// admitted. A resource unit is seeded at creation from the `Agld`
    /// `DataA1` row (or zero when that alias is absent), matching the original
    /// ability constructor; the `war3mapUnits.doo` placement amount is not
    /// derived. `Some` is the exact raw integer: negative values are stored,
    /// and `AddResourceAmount` wraps instead of clamping.
    pub resource_amount: Option<i32>,
    /// Explicit protection set on this unit. None means its initial ability effects
    /// have not been derived; queries refuse rather than inventing that state.
    pub invulnerable: Option<bool>,
    /// Admitted immediate-order class on this unit.
    ///
    /// `None` means the unit ability list does not name the admitted alias,
    /// or no order was admitted. `Some((code, false))` is the ability-data
    /// `code` cell, inactive. `Some((code, true))` is the executed on order.
    pub admitted_order: Option<([u8; 4], bool)>,
    /// Source-backed ability levels and hero counters, when that runtime is admitted.
    pub abilities: Option<UnitAbilities>,
    pub animation: Option<crate::unit_model::Animation>,
    pub look_at: Option<super::UnitLookAt>,
    pub summoned: bool,
    pub activity: Option<super::unit_activity::UnitActivity>,
    /// Runtime ShowUnit state, independent of fog, invisibility and explicit pause.
    pub shown: bool,
    /// Start of an explicit PauseUnit interval on host game time.
    pub paused_since: Option<f64>,
    /// Natural sleep state when admitted from the selected unit definition.
    pub sleep: Option<super::unit_sleep::UnitSleep>,
    /// Live life/mana when vitals are admitted.
    pub vitals: Option<super::Vitals>,
    /// Weapon, armor and attack state when combat is admitted.
    pub combat: Option<super::Combat>,
    /// Permanent strength granted by automatic powerups, added to derived STR. Zero before any grant.
    pub permanent_strength: i32,
}

type ModelLoader = Box<
    dyn FnMut(&Definition) -> Result<Option<Arc<crate::unit_model::Model>>, String> + Send + Sync,
>;

struct DefendAbility {
    alias: String,
    code: [u8; 4],
    on: String,
    off: String,
    tech: i32,
    required_level: i32,
}

struct MilitiaAbility {
    alias: String,
    off: String,
    base: [u8; 4],
}

struct UnitStateLimits {
    max_records: usize,
    records: usize,
}

pub(super) struct Admission {
    catalog: Arc<dyn ObjectCatalog + Send + Sync>,
    source: Source,
    declaration_source: String,
    span: Span,
    max_records: usize,
    max_retained_fields: usize,
    max_modifications: usize,
    records: usize,
    /// Present when this program declares `GetUnitTypeId` on the same source.
    type_span: Option<Span>,
    /// Present when this program declares `GetOwningPlayer` on the same source.
    owner_span: Option<Span>,
    owner_setter_span: Option<Span>,
    collision_span: Option<Span>,
    /// Present when this program declares `SetUnitAcquireRange` on the same source.
    acquire_span: Option<Span>,
    /// Present when this program declares `SetHeroLevel` on the same source.
    hero_level_span: Option<Span>,
    invulnerable_span: Option<Span>,
    invulnerable_query_span: Option<Span>,
    /// Present after [`ObjectHost::with_unit_states`] admits the setter.
    unit_state: Option<UnitStateLimits>,
    /// Present after [`ObjectHost::with_resource_amounts`] admits the family.
    resource_amounts: bool,
    /// Authoring tables used to seed the constructor's resource slot.
    resource_tables: Option<Arc<DefaultTables>>,
    /// Present after [`ObjectHost::with_defend_ability`].
    order: Option<DefendAbility>,
    /// Present after [`ObjectHost::with_militia_ability`].
    militia: Option<MilitiaAbility>,
    heroes: Option<hero::Admission>,
    models: Option<ModelLoader>,
}

impl Admission {
    pub(super) fn source(&self) -> &Source {
        &self.source
    }
}

impl ObjectHost {
    pub(super) fn check_unit_creation_capacity(&self, count: usize) -> Result<(), Error> {
        let admission = self.admitted_units()?;
        if count > admission.max_records.saturating_sub(admission.records)
            || count
                > self
                    .inner
                    .max_handles
                    .saturating_sub(self.inner.arena.len())
        {
            return Err(self.inner.error(NATIVE, Failure::Budget("unit creation")));
        }
        Ok(())
    }

    /// Admit `CreateUnit` against an already selected catalog and source.
    ///
    /// `catalog` must be the caller's existing arc. `source` must be an
    /// identity the caller already selected, such as `ImportedWorld.source`.
    /// Empty install, edition, or map is refused. `unit` must extend `widget`.
    pub fn with_unit_catalog(
        mut self,
        catalog: Arc<dyn ObjectCatalog + Send + Sync>,
        source: Source,
        budget: UnitBudget,
    ) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(NATIVE, reason);
        if self.units.is_some() {
            return Err(fail(Failure::UnitState("already admitted")));
        }
        if source.install.is_empty() || source.edition.is_empty() || source.map.is_empty() {
            return Err(fail(Failure::UnitState("source identity is incomplete")));
        }
        std::alloc::Layout::array::<ExecutedUnit>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if !below(&self.inner.program, KIND, "widget") {
            return Err(fail(Failure::Signature));
        }
        Self::contract(
            &self.inner,
            NATIVE,
            &["player", "integer", "real", "real", "real"],
            KIND,
            false,
            Some(KIND),
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(NATIVE)
            .ok_or_else(|| fail(Failure::Signature))?;
        let mut declaration_source = String::new();
        declaration_source
            .try_reserve_exact(native.source.len())
            .map_err(|_| fail(Failure::Capacity))?;
        declaration_source.push_str(&native.source);
        let type_span = self.optional_unit_getter(&declaration_source, GET_TYPE, "integer")?;
        let owner_span = self.optional_unit_getter(&declaration_source, GET_OWNER, "player")?;
        let owner_setter_span = self.owner_declaration(&declaration_source)?;
        let collision_span =
            self.optional_unit_getter(&declaration_source, GET_COLLISION, "real")?;
        let acquire_span = self.acquire_declaration(&declaration_source)?;
        let hero_level_span = self.hero_level_declaration(&declaration_source)?;
        let invulnerable_span = self.invulnerability_declaration(&declaration_source)?;
        let invulnerable_query_span =
            self.optional_unit_getter(&declaration_source, GET_INVULNERABLE, "boolean")?;
        self.unit_visibility_declarations(&declaration_source)?;
        self.unit_pause_declarations(&declaration_source)?;
        self.unit_look_at_declarations(&declaration_source)?;
        self.unit_sleep_declarations(&declaration_source)?;
        self.unit_buff_declarations(&declaration_source)?;
        self.unit_removal_declaration(&declaration_source)?;
        self.unit_cooldown_reset_declaration(&declaration_source)?;
        if self.sound_properties {
            self.sound_attachment_declaration(&declaration_source)?;
        }
        self.units = Some(Admission {
            catalog,
            source,
            declaration_source,
            span: native.span,
            max_records: budget.max_records,
            max_retained_fields: budget.max_retained_fields,
            max_modifications: budget.max_modifications,
            records: 0,
            type_span,
            owner_span,
            owner_setter_span,
            collision_span,
            acquire_span,
            hero_level_span,
            invulnerable_span,
            invulnerable_query_span,
            unit_state: None,
            resource_amounts: false,
            resource_tables: None,
            order: None,
            militia: None,
            heroes: None,
            models: None,
        });
        Ok(self)
    }

    /// Admit executed `SetUnitState` on units from [`Self::with_unit_catalog`].
    ///
    /// Callers that skip this keep unit creation. `SetUnitState` stays
    /// unsupported when the program omits it, and also when it is declared
    /// but this admission was not requested. Opting in requires the loaded
    /// non-constant `unit, unitstate, real -> nothing` contract, the same
    /// source as `CreateUnit`, and a selected `ConvertUnitState` domain of
    /// kind `unitstate` from that source.
    ///
    /// `budget.max_records` counts distinct pairs. Replacing a retained pair
    /// updates the finite value in place. A new pair at the limit is refused
    /// before `try_reserve`. `GetUnitState` is not admitted: a missing pair is
    /// not a returned real.
    pub fn with_unit_states(mut self, budget: UnitStateBudget) -> Result<Self, Error> {
        let admitted = self.units.as_ref().ok_or_else(|| {
            self.inner
                .error(SET_STATE, Failure::UnitState("not admitted"))
        })?;
        if admitted.unit_state.is_some() {
            return Err(self
                .inner
                .error(SET_STATE, Failure::UnitState("already admitted")));
        }
        let create_source = &admitted.declaration_source;
        Self::contract(
            &self.inner,
            SET_STATE,
            &["unit", STATE_KIND, "real"],
            "nothing",
            false,
            Some(KIND),
        )?;
        let setter = self
            .inner
            .program
            .natives()
            .get(SET_STATE)
            .ok_or_else(|| self.inner.error(SET_STATE, Failure::Signature))?;
        if setter.source != *create_source {
            return Err(self.inner.error(SET_STATE, Failure::Source));
        }
        Self::contract(
            &self.inner,
            CONVERT_STATE,
            &["integer"],
            STATE_KIND,
            true,
            None,
        )?;
        let convert = self
            .inner
            .program
            .natives()
            .get(CONVERT_STATE)
            .ok_or_else(|| self.inner.error(CONVERT_STATE, Failure::Signature))?;
        if convert.source != *create_source {
            return Err(self.inner.error(CONVERT_STATE, Failure::Source));
        }
        let conversion = self
            .inner
            .conversions
            .get(CONVERT_STATE)
            .ok_or_else(|| self.inner.error(CONVERT_STATE, Failure::Signature))?;
        if conversion.source != *create_source {
            return Err(self.inner.error(CONVERT_STATE, Failure::Source));
        }
        if conversion.kind != STATE_KIND {
            return Err(self.inner.error(CONVERT_STATE, Failure::Signature));
        }
        if conversion.domain.is_empty() {
            return Err(self.inner.error(CONVERT_STATE, Failure::EmptyDomain));
        }
        if !below(&self.inner.program, STATE_KIND, "handle") {
            return Err(self.inner.error(SET_STATE, Failure::Signature));
        }
        std::alloc::Layout::array::<ExecutedUnitState>(budget.max_records)
            .map_err(|_| self.inner.error(SET_STATE, Failure::Capacity))?;
        let admitted = self.units.as_mut().ok_or_else(|| {
            self.inner
                .error(SET_STATE, Failure::UnitState("not admitted"))
        })?;
        admitted.unit_state = Some(UnitStateLimits {
            max_records: budget.max_records,
            records: 0,
        });
        Ok(self)
    }

    /// Admit the original resource amount natives on units from [`Self::with_unit_catalog`].
    ///
    /// `SetResourceAmount` is the anchor: a program that omits it keeps these
    /// natives unsupported. `GetResourceAmount` and `AddResourceAmount` are
    /// admitted when the program declares them. Every present declaration must
    /// share `CreateUnit`'s source, and `Add` and `Set` must be non-constant.
    /// `unit` must extend `widget`.
    ///
    /// `tables` is retained to read the constructor seed at `CreateUnit`:
    /// the `Agld` row's authored `DataA1`. The retained amount is one `i32` on
    /// an already admitted unit, so [`UnitBudget::max_records`] bounds it;
    /// this opt-in takes no separate budget.
    pub fn with_resource_amounts(mut self, tables: Arc<DefaultTables>) -> Result<Self, Error> {
        let admitted = self.units.as_ref().ok_or_else(|| {
            self.inner
                .error(SET_RESOURCE, Failure::UnitState("not admitted"))
        })?;
        if admitted.resource_amounts {
            return Err(self
                .inner
                .error(SET_RESOURCE, Failure::UnitState("already admitted")));
        }
        if self.inner.program.natives().get(SET_RESOURCE).is_none() {
            return Ok(self);
        }
        let create_source = &admitted.declaration_source;
        Self::contract(
            &self.inner,
            SET_RESOURCE,
            &["unit", "integer"],
            "nothing",
            false,
            Some(KIND),
        )?;
        let setter = self
            .inner
            .program
            .natives()
            .get(SET_RESOURCE)
            .ok_or_else(|| self.inner.error(SET_RESOURCE, Failure::Signature))?;
        if setter.source != *create_source {
            return Err(self.inner.error(SET_RESOURCE, Failure::Source));
        }
        if self.inner.program.natives().get(ADD_RESOURCE).is_some() {
            Self::contract(
                &self.inner,
                ADD_RESOURCE,
                &["unit", "integer"],
                "nothing",
                false,
                Some(KIND),
            )?;
            let adder = self
                .inner
                .program
                .natives()
                .get(ADD_RESOURCE)
                .ok_or_else(|| self.inner.error(ADD_RESOURCE, Failure::Signature))?;
            if adder.source != *create_source {
                return Err(self.inner.error(ADD_RESOURCE, Failure::Source));
            }
        }
        self.optional_unit_getter(create_source, GET_RESOURCE, "integer")?;
        let admitted = self.units.as_mut().ok_or_else(|| {
            self.inner
                .error(SET_RESOURCE, Failure::UnitState("not admitted"))
        })?;
        admitted.resource_amounts = true;
        admitted.resource_tables = Some(tables);
        Ok(self)
    }

    /// Admit the Defend on/off behavior for one caller-selected ability on units from [`Self::with_unit_catalog`].
    ///
    /// `alias` selects one ability row. The retained class id is that row's
    /// `code` cell. `tables` is borrowed for this call and is not stored.
    /// A missing, blank, marker, or non-zero `Cost1` or `Cool1` is refused.
    /// `Requiresamount` may be absent or one blank cell; its required upgrade
    /// must author exactly one level. Other requirement shapes are refused.
    /// The declaration is non-constant `unit, string -> boolean` and shares
    /// `CreateUnit`'s source. Admit this behavior before creating units.
    /// Other order strings stay unsupported until the order registry is bound.
    pub fn with_defend_ability(
        mut self,
        tables: &DefaultTables,
        alias: &str,
    ) -> Result<Self, Error> {
        let create_source = {
            let admitted = self
                .units
                .as_ref()
                .ok_or_else(|| self.inner.error(ORDER, Failure::UnitState("not admitted")))?;
            if admitted.order.is_some() {
                return Err(self
                    .inner
                    .error(ORDER, Failure::UnitState("already admitted")));
            }
            if admitted.records != 0 {
                return Err(self.inner.error(
                    ORDER,
                    Failure::UnitState("admit Defend before creating units"),
                ));
            }
            admitted.declaration_source.clone()
        };
        Self::contract(
            &self.inner,
            ORDER,
            &["unit", "string"],
            "boolean",
            false,
            Some(KIND),
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(ORDER)
            .ok_or_else(|| self.inner.error(ORDER, Failure::Signature))?;
        if native.source != create_source {
            return Err(self.inner.error(ORDER, Failure::Source));
        }
        let snapshot = read_defend_ability(tables, alias).map_err(|reason| {
            let mut error = self.inner.error(ORDER, Failure::UnitState(reason));
            error.source_context.push_str(&format!(
                "; Defend ability {alias:?}, object data {:?}",
                tables.identity()
            ));
            error
        })?;
        let admitted = self
            .units
            .as_mut()
            .ok_or_else(|| self.inner.error(ORDER, Failure::UnitState("not admitted")))?;
        admitted.order = Some(snapshot);
        Ok(self)
    }

    /// Admit the authored Call to Arms off order for the source-backed `Amil`
    /// class on units from [`Self::with_unit_catalog`].
    ///
    /// The class id is a named constant. The profile's `Order`/`Unorder` text
    /// and the rank-1 `DataA1`/`DataB1` base and active forms come from the
    /// selected tables. An absent class leaves this admission off; a present,
    /// malformed class refuses with the table identity in the error context.
    /// `abilities` is the caller's selected ability overlay, the same one hero
    /// admission receives: a map modification or custom row for `Amil` is
    /// refused because this slice consumes only the default row. This retains
    /// only the off order and the base form; it does not read `Dur1`, morph, or
    /// keep per-unit state. The declaration is non-constant
    /// `unit, string -> boolean` and shares `CreateUnit`'s source. Admit this
    /// behavior before creating units.
    pub fn with_militia_ability(
        mut self,
        tables: &DefaultTables,
        abilities: &Overlay,
    ) -> Result<Self, Error> {
        let create_source = {
            let admitted = self
                .units
                .as_ref()
                .ok_or_else(|| self.inner.error(ORDER, Failure::UnitState("not admitted")))?;
            if !tables
                .rawcodes(Family::Ability)
                .any(|rawcode| rawcode.eq_ignore_ascii_case(MILITIA_ALIAS))
            {
                return Ok(self);
            }
            if admitted.militia.is_some() {
                return Err(self
                    .inner
                    .error(ORDER, Failure::UnitState("militia already admitted")));
            }
            if admitted.records != 0 {
                return Err(self.inner.error(
                    ORDER,
                    Failure::UnitState("admit militia before creating units"),
                ));
            }
            admitted.declaration_source.clone()
        };
        if abilities.new_object(MILITIA_ALIAS).is_some() || !abilities.on(MILITIA_ALIAS).is_empty()
        {
            return Err(self.inner.error(
                ORDER,
                Failure::UnitState(
                    "map ability overlay modifies Amil; narrow slice does not consume it",
                ),
            ));
        }
        Self::contract(
            &self.inner,
            ORDER,
            &["unit", "string"],
            "boolean",
            false,
            Some(KIND),
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(ORDER)
            .ok_or_else(|| self.inner.error(ORDER, Failure::Signature))?;
        if native.source != create_source {
            return Err(self.inner.error(ORDER, Failure::Source));
        }
        let snapshot = read_militia_ability(tables).map_err(|reason| {
            let mut error = self.inner.error(ORDER, Failure::UnitState(reason));
            error.source_context.push_str(&format!(
                "; militia ability {MILITIA_ALIAS:?}, object data {:?}",
                tables.identity()
            ));
            error
        })?;
        let admitted = self
            .units
            .as_mut()
            .ok_or_else(|| self.inner.error(ORDER, Failure::UnitState("not admitted")))?;
        admitted.militia = Some(snapshot);
        Ok(self)
    }

    /// Same catalog allocation passed to [`Self::with_unit_catalog`].
    pub fn unit_catalog(&self) -> Result<Arc<dyn ObjectCatalog + Send + Sync>, Error> {
        Ok(Arc::clone(&self.admitted_units()?.catalog))
    }

    /// Declaration retained at admission. The span is the native's own span.
    pub fn unit_declaration(&self) -> Result<(&str, Span), Error> {
        let admitted = self.admitted_units()?;
        Ok((&admitted.declaration_source, admitted.span))
    }

    /// Checked result for this unit handle. Other families are refused.
    pub fn executed_unit(&self, id: HandleId) -> Result<&ExecutedUnit, Error> {
        match self.inner.entry(id)? {
            Entry::Unit { record } => Ok(record),
            _ => Err(self.inner.error(NATIVE, Failure::WrongFamily)),
        }
    }

    pub fn executed_units(&self) -> impl Iterator<Item = (HandleId, &ExecutedUnit)> {
        self.inner
            .arena
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| match entry {
                Entry::Unit { record } => Some((
                    self.inner.id(i, NATIVE).expect("allocated identity"),
                    &**record,
                )),
                _ => None,
            })
    }

    /// Retain selected model assets on the existing executed units.
    pub fn admit_unit_models(
        &mut self,
        loader: impl FnMut(&Definition) -> Result<Option<Arc<crate::unit_model::Model>>, String>
        + Send
        + Sync
        + 'static,
    ) -> Result<(), Error> {
        let units = self.admitted_units()?;
        if units.records != 0 || units.models.is_some() || self.timer_game_time().is_none() {
            return Err(self.inner.error(
                "SetUnitAnimation",
                Failure::UnitAnimation(
                    "model loading requires an initial game clock and no created units".into(),
                ),
            ));
        }
        for name in ["SetUnitAnimation", "QueueUnitAnimation"] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(
                    &self.inner,
                    name,
                    &["unit", "string"],
                    "nothing",
                    false,
                    Some(KIND),
                )?;
                if native.source != units.declaration_source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        self.units.as_mut().expect("admitted").models = Some(Box::new(loader));
        Ok(())
    }

    fn set_unit_animation(&mut self, args: &[Value]) -> Result<Value, Error> {
        self.animation_native("SetUnitAnimation", args, false)
    }

    fn queue_unit_animation(&mut self, args: &[Value]) -> Result<Value, Error> {
        self.animation_native("QueueUnitAnimation", args, true)
    }

    fn animation_native(
        &mut self,
        name: &str,
        args: &[Value],
        queue: bool,
    ) -> Result<Value, Error> {
        if self.inner.program.natives().get(name).is_none() {
            return Err(self.inner.error(name, Failure::Signature));
        }
        let [unit, Value::Str(sequence)] = args else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        if matches!(unit, Value::Null) {
            return Ok(Value::Null);
        }
        let Value::Handle(unit) = unit else {
            return Err(self.inner.error(name, Failure::WrongFamily));
        };
        let now = self.timer_game_time().ok_or_else(|| {
            self.inner
                .error(name, Failure::UnitAnimation("missing game clock".into()))
        })?;
        let mut animation = self.executed_unit(*unit)?.animation.clone();
        let mut queued = false;
        if let Some(state) = &mut animation {
            if queue {
                self.reserve_animation_active(*unit)
                    .map_err(|failure| self.inner.error(name, failure))?;
                queued = state
                    .queue(sequence, now)
                    .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
            } else {
                state
                    .select(sequence, now, None)
                    .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
            }
        }
        let Entry::Unit { record } = self.inner.entry_mut(*unit)? else {
            unreachable!()
        };
        record.animation = animation;
        if queued {
            self.commit_animation_active(*unit);
        }
        Ok(Value::Null)
    }

    pub(super) fn unit_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.units.as_ref()?;
        if name == "UnitResetCooldown" && self.inner.program.natives().get(name).is_some() {
            return Some(self.unit_reset_cooldown(args));
        }
        if let Some(result) = self.unit_visibility_native(name, args) {
            return Some(result);
        }
        if let Some(result) = self.unit_look_at_native(name, args) {
            return Some(result);
        }
        if let Some(result) = self.unit_pause_native(name, args) {
            return Some(result);
        }
        if let Some(result) = self.unit_sleep_native(name, args) {
            return Some(result);
        }
        if let Some(result) = self.unit_buff_native(name, args) {
            return Some(result);
        }
        if let Some(result) = self.divine_shield_native(name, args) {
            return Some(result);
        }
        if name == "SetUnitAnimation" && self.units.as_ref()?.models.is_some() {
            return Some(self.set_unit_animation(args));
        }
        if name == "QueueUnitAnimation" && self.units.as_ref()?.models.is_some() {
            return Some(self.queue_unit_animation(args));
        }
        if let Some(result) = self.hero_native(name, args) {
            return Some(result);
        }
        if name == NATIVE {
            return Some(self.create_unit(args));
        }
        if name == GET_TYPE && self.admitted_units().ok()?.type_span.is_some() {
            return Some(self.unit_type_id(args));
        }
        if name == GET_OWNER && self.admitted_units().ok()?.owner_span.is_some() {
            return Some(self.owning_player(args));
        }
        if name == SET_OWNER && self.admitted_units().ok()?.owner_setter_span.is_some() {
            return Some(self.set_unit_owner(args));
        }
        if name == GET_COLLISION && self.admitted_units().ok()?.collision_span.is_some() {
            return Some(self.unit_collision(args));
        }
        if name == SET_RANGE && self.admitted_units().ok()?.acquire_span.is_some() {
            return Some(self.set_acquire_range(args));
        }
        if name == SET_HERO_LEVEL && self.admitted_units().ok()?.hero_level_span.is_some() {
            return Some(self.set_hero_level(args));
        }
        if name == SET_INVULNERABLE && self.admitted_units().ok()?.invulnerable_span.is_some() {
            return Some(self.set_unit_invulnerable(args));
        }
        if name == GET_INVULNERABLE
            && self
                .admitted_units()
                .ok()?
                .invulnerable_query_span
                .is_some()
        {
            return Some(self.unit_invulnerability(args));
        }
        if name == SET_STATE && self.admitted_units().ok()?.unit_state.is_some() {
            return Some(self.set_unit_state(args));
        }
        if self.admitted_units().ok()?.resource_amounts {
            match name {
                SET_RESOURCE => return Some(self.set_resource_amount(args, false)),
                ADD_RESOURCE => return Some(self.set_resource_amount(args, true)),
                GET_RESOURCE => return Some(self.get_resource_amount(args)),
                _ => {}
            }
        }
        if name == ORDER {
            if let Some(result) = self.militia_immediate_order(args) {
                return Some(result);
            }
            if self.admitted_units().ok()?.order.is_some() {
                return self.issue_immediate_order(args);
            }
        }
        None
    }

    fn admitted_units(&self) -> Result<&Admission, Error> {
        self.units
            .as_ref()
            .ok_or_else(|| self.inner.error(NATIVE, Failure::UnitState("not admitted")))
    }

    /// `Ok(None)` when the program does not declare `name`. A present
    /// declaration must take one `unit` and return `result`, and share
    /// `CreateUnit`'s source. Omitting the native leaves that getter unsupported.
    fn optional_unit_getter(
        &self,
        create_source: &str,
        name: &str,
        result: &str,
    ) -> Result<Option<Span>, Error> {
        let Some(native) = self.inner.program.natives().get(name) else {
            return Ok(None);
        };
        // Both declaration forms describe the same read-only operation.
        Self::contract(
            &self.inner,
            name,
            &[KIND],
            result,
            native.is_constant,
            Some(KIND),
        )?;
        if native.source != create_source {
            return Err(self.inner.error(name, Failure::Source));
        }
        Ok(Some(native.span))
    }

    fn created_unit(&self, id: HandleId, name: &str) -> Result<&ExecutedUnit, Error> {
        self.executed_unit(id)
            .map_err(|error| self.inner.error(name, error.failure))
    }

    /// Rawcode integer from the admitted unit's resolved definition.
    ///
    /// This is `i32::from_be_bytes` of that definition's rawcode, the inverse of
    /// the `CreateUnit` argument conversion. It is not a life, level, or state.
    fn unit_type_id(&self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(GET_TYPE, reason);
        let [Value::Handle(unit)] = args else {
            return Err(fail(Failure::UnitState(
                "arguments do not match the unit native declaration",
            )));
        };
        let rawcode = self.created_unit(*unit, GET_TYPE)?.definition.rawcode;
        Ok(Value::Int(i32::from_be_bytes(rawcode)))
    }

    /// The current player identity on the same admitted unit.
    fn owning_player(&self, args: &[Value]) -> Result<Value, Error> {
        if matches!(args, [Value::Null]) {
            return Ok(Value::Null);
        }
        let fail = |reason| self.inner.error(GET_OWNER, reason);
        let [Value::Handle(unit)] = args else {
            return Err(fail(Failure::UnitState(
                "arguments do not match the unit native declaration",
            )));
        };
        Ok(Value::Handle(self.current_unit_owner(*unit, GET_OWNER)?.0))
    }

    fn current_unit_owner(&self, unit: HandleId, name: &str) -> Result<(HandleId, i32), Error> {
        let fail = |reason| self.inner.error(name, reason);
        let owner = self.created_unit(unit, name)?.owner;
        match self
            .inner
            .entry(owner)
            .map_err(|error| fail(error.failure))?
        {
            Entry::Player { number } => Ok((owner, *number)),
            _ => Err(fail(Failure::Identity)),
        }
    }

    fn owner_declaration(&self, create_source: &str) -> Result<Option<Span>, Error> {
        let Some(native) = self.inner.program.natives().get(SET_OWNER) else {
            return Ok(None);
        };
        Self::contract(
            &self.inner,
            SET_OWNER,
            &[KIND, "player", "boolean"],
            "nothing",
            false,
            Some(KIND),
        )?;
        if native.source != create_source {
            return Err(self.inner.error(SET_OWNER, Failure::Source));
        }
        Ok(Some(native.span))
    }

    fn set_unit_owner(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(SET_OWNER, reason);
        let [unit, player, Value::Bool(change_color)] = args else {
            return Err(fail(Failure::UnitState(
                "unit, player and boolean required",
            )));
        };
        if !matches!(unit, Value::Null | Value::Handle(_))
            || !matches!(player, Value::Null | Value::Handle(_))
        {
            return Err(fail(Failure::UnitState(
                "unit and player identities required",
            )));
        }
        if matches!(unit, Value::Null) || matches!(player, Value::Null) {
            return Ok(Value::Null);
        }
        let (Value::Handle(unit), Value::Handle(player)) = (unit, player) else {
            unreachable!()
        };
        let (previous, previous_number) = self.current_unit_owner(*unit, SET_OWNER)?;
        let number = match self.inner.entry(*player).map_err(|e| fail(e.failure))? {
            Entry::Player { number } => *number,
            _ => return Err(fail(Failure::WrongFamily)),
        };
        if number == previous_number {
            return Ok(Value::Null);
        }
        let unit_event = self.unit_event_enumerand(
            SET_OWNER,
            "EVENT_UNIT_CHANGE_OWNER",
            "unitevent",
            "ConvertUnitEvent",
        )?;
        let player_event = self.unit_event_enumerand(
            SET_OWNER,
            "EVENT_PLAYER_UNIT_CHANGE_OWNER",
            "playerunitevent",
            "ConvertPlayerUnitEvent",
        )?;
        // shortcut: ownership-event delivery is not wired; refuse matching or unqualified live listeners before changing state.
        if self.inner.arena.iter().any(|entry| {
            let trigger = match entry {
                Entry::RegisteredUnitEvent { trigger, unit: watched, enumerand, .. } if watched == unit && unit_event.is_none_or(|event| event == *enumerand) => Some(trigger),
                Entry::RegisteredPlayerUnitEvent(registration) if (registration.player == previous || registration.player == *player) && player_event.is_none_or(|event| event == registration.enumerand) => Some(&registration.trigger),
                _ => None,
            };
            trigger.is_some_and(|id| matches!(self.inner.entry(*id), Ok(Entry::Trigger { enabled }) if *enabled != Some(false)))
        }) {
            return Err(fail(Failure::UnitState("ownership change with live unit listeners requires event delivery")));
        }
        let color = if *change_color {
            let color = self
                .player_settings(*player)
                .map_err(|e| fail(e.failure))?
                .color
                .ok_or_else(|| fail(Failure::UnitState("new owner's team color is unset")))?;
            self.inner
                .player_color(color)
                .map_err(|e| fail(e.failure))?;
            Some(color)
        } else {
            None
        };
        let guard = if self.guard_managed_owner(*player)? {
            let record = self.executed_unit(*unit)?;
            let eligible =
                super::unit_guards::candidate(&record.definition, record.combat.as_ref())
                    .map_err(|error| self.inner.error(SET_OWNER, Failure::UnitActivity(error)))?;
            if eligible {
                let position = record.position.ok_or_else(|| {
                    self.inner.error(
                        SET_OWNER,
                        Failure::UnitActivity("guard transfer requires accepted placement".into()),
                    )
                })?;
                self.prepare_unit_guard(Some(*unit), *player, position, true)?
            } else {
                None
            }
        } else {
            None
        };
        if self
            .guard_state()
            .and_then(|state| state.get(*unit))
            .is_some_and(|guard| guard.returning)
        {
            self.steer_unit(*unit, None, SET_OWNER)?;
            self.clear_combat_target(*unit);
        }
        match self.inner.entry_mut(*unit)? {
            Entry::Unit { record } => {
                record.owner = *player;
                if *change_color {
                    record.team_color = color;
                }
            }
            _ => unreachable!("validated unit identity"),
        }
        self.publish_unit_guard(*unit, guard);
        Ok(Value::Null)
    }

    fn unit_event_enumerand(
        &self,
        caller: &str,
        name: &str,
        kind: &str,
        convert: &str,
    ) -> Result<Option<i32>, Error> {
        let source = &self.admitted_units()?.declaration_source;
        let mut globals = self
            .inner
            .program
            .globals()
            .iter()
            .filter(|g| g.name == name && &g.source == source);
        let Some(global) = globals.next() else {
            return Ok(None);
        };
        let fail = || self.inner.error(caller, Failure::Signature);
        if globals.next().is_some()
            || !global.is_constant
            || global.array
            || global.ty.array
            || global.ty.kind.name() != kind
        {
            return Err(fail());
        }
        let Some(Expr::Call { callee, args, .. }) = &global.init else {
            return Err(fail());
        };
        if !matches!(callee.as_ref(), Expr::Ident(native, _) if native.as_str() == convert) {
            return Err(fail());
        }
        let [argument] = args.as_slice() else {
            return Err(fail());
        };
        Self::contract(&self.inner, convert, &["integer"], kind, true, None)?;
        if self
            .inner
            .program
            .natives()
            .get(convert)
            .is_none_or(|native| &native.source != source)
        {
            return Err(self.inner.error(caller, Failure::Source));
        }
        // Local expression limits, independent of the source enumerand.
        let mut work = 1024;
        super::super::integer(argument, 64, &mut work)
            .map(Some)
            .map_err(|failure| self.inner.error(caller, failure))
    }

    fn unit_collision(&self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(GET_COLLISION, reason);
        let [Value::Handle(unit)] = args else {
            return Err(fail(Failure::UnitState(
                "collision query requires one unit",
            )));
        };
        self.unit_collision_radius(*unit, GET_COLLISION)
            .map(Value::Real)
    }

    pub(super) fn unit_collision_radius(&self, unit: HandleId, caller: &str) -> Result<f32, Error> {
        self.created_unit(unit, caller)?
            .collision_radius
            .map_err(|reason| self.inner.error(caller, Failure::UnitState(reason)))
    }

    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "units.collision_field")
    )]
    fn catalog_collision_radius(definition: &Definition) -> Result<f32, &'static str> {
        let mut fields = definition.fields.iter().filter(|(_, field)| {
            field.metadata.as_ref().is_some_and(|meta| {
                matches!(&meta.source, FieldSource::Column { slk: SlkId::UnitBalance, column } if column == "collision")
            })
        });
        let Some((id, field)) = fields.next() else {
            return Err("missing UnitBalance collision field");
        };
        if fields.next().is_some() {
            return Err("ambiguous UnitBalance collision field");
        }
        let metadata = field
            .metadata
            .as_ref()
            .expect("field matched by metadata source");
        if metadata.fourcc != *id
            || metadata.from != SlkId::UnitMetaData
            || metadata.index != -1
            || !matches!(metadata.kind, FieldKind::Real | FieldKind::Unreal)
        {
            return Err("unsupported collision metadata");
        }
        let value = match (&field.value, metadata.kind) {
            (Ok(FieldValue::Cell(text)), _) => text.trim().parse::<f32>().ok(),
            (Ok(FieldValue::Override(pcx_objects::Value::Real(value))), FieldKind::Real)
            | (Ok(FieldValue::Override(pcx_objects::Value::Unreal(value))), FieldKind::Unreal) => {
                Some(*value)
            }
            _ => None,
        }
        .filter(|value| value.is_finite() && *value >= 0.0)
        .ok_or("collision must be a finite nonnegative catalog real")?;
        // shortcut: catalog geometry only; qualify runtime collision mutations before admitting their setters.
        Ok(value)
    }

    /// `Ok(None)` when the program does not declare the setter. A present
    /// declaration must return `nothing`, take `unit` then `real`, and share
    /// `CreateUnit`'s source. Parameter kinds are the loaded native's contract.
    fn acquire_declaration(&self, create_source: &str) -> Result<Option<Span>, Error> {
        let Some(native) = self.inner.program.natives().get(SET_RANGE) else {
            return Ok(None);
        };
        Self::contract(
            &self.inner,
            SET_RANGE,
            &["unit", "real"],
            "nothing",
            false,
            Some(KIND),
        )?;
        if native.source != create_source {
            return Err(self.inner.error(SET_RANGE, Failure::Source));
        }
        Ok(Some(native.span))
    }

    /// `Ok(None)` when the program does not declare `SetHeroLevel`. A present
    /// declaration must return `nothing`, take `unit`, `integer`, then
    /// `boolean`, and share `CreateUnit`'s source.
    fn hero_level_declaration(&self, create_source: &str) -> Result<Option<Span>, Error> {
        let Some(native) = self.inner.program.natives().get(SET_HERO_LEVEL) else {
            return Ok(None);
        };
        Self::contract(
            &self.inner,
            SET_HERO_LEVEL,
            &["unit", "integer", "boolean"],
            "nothing",
            false,
            Some(KIND),
        )?;
        if native.source != create_source {
            return Err(self.inner.error(SET_HERO_LEVEL, Failure::Source));
        }
        Ok(Some(native.span))
    }

    fn set_hero_level(&mut self, args: &[Value]) -> Result<Value, Error> {
        if self.admitted_units()?.heroes.is_some() {
            return self.change_hero_level(args);
        }
        let (id, level, show_eye_candy) = {
            let fail = |reason| self.inner.error(SET_HERO_LEVEL, reason);
            let [
                Value::Handle(unit),
                Value::Int(level),
                Value::Bool(show_eye_candy),
            ] = args
            else {
                return Err(fail(Failure::UnitState(
                    "arguments do not match the unit native declaration",
                )));
            };
            match self
                .inner
                .entry(*unit)
                .map_err(|error| self.inner.error(SET_HERO_LEVEL, error.failure))?
            {
                Entry::Unit { .. } => {}
                _ => return Err(fail(Failure::WrongFamily)),
            }
            (*unit, *level, *show_eye_candy)
        };
        match self.inner.entry_mut(id)? {
            Entry::Unit { record } => {
                record.requested_hero_level = Some(level);
                record.requested_show_eye_candy = Some(show_eye_candy);
                Ok(Value::Null)
            }
            _ => Err(self.inner.error(SET_HERO_LEVEL, Failure::WrongFamily)),
        }
    }

    fn invulnerability_declaration(&self, create_source: &str) -> Result<Option<Span>, Error> {
        let Some(native) = self.inner.program.natives().get(SET_INVULNERABLE) else {
            return Ok(None);
        };
        Self::contract(
            &self.inner,
            SET_INVULNERABLE,
            &[KIND, "boolean"],
            "nothing",
            false,
            Some(KIND),
        )?;
        if native.source != create_source {
            return Err(self.inner.error(SET_INVULNERABLE, Failure::Source));
        }
        Ok(Some(native.span))
    }

    fn set_unit_invulnerable(&mut self, args: &[Value]) -> Result<Value, Error> {
        if matches!(args, [Value::Null, Value::Bool(_)]) {
            return Ok(Value::Null);
        }
        let [Value::Handle(unit), Value::Bool(flag)] = args else {
            return Err(self.inner.error(
                SET_INVULNERABLE,
                Failure::UnitState("unit and boolean required"),
            ));
        };
        self.created_unit(*unit, SET_INVULNERABLE)?;
        match self.inner.entry_mut(*unit)? {
            Entry::Unit { record } => {
                // shortcut: explicit setter state only; derive initial and other ability effects with the ability runtime.
                record.invulnerable = Some(*flag);
                Ok(Value::Null)
            }
            _ => Err(self.inner.error(SET_INVULNERABLE, Failure::WrongFamily)),
        }
    }

    fn unit_invulnerability(&self, args: &[Value]) -> Result<Value, Error> {
        if matches!(args, [Value::Null]) {
            return Ok(Value::Bool(false));
        }
        let [Value::Handle(unit)] = args else {
            return Err(self
                .inner
                .error(GET_INVULNERABLE, Failure::UnitState("one unit required")));
        };
        let record = self.created_unit(*unit, GET_INVULNERABLE)?;
        let shielded = self
            .timer_game_time()
            .is_some_and(|now| self.divine_shield_protects(*unit, now));
        // The reviewed 1.29 getter returns the inverse of effective protection
        // (static `499fa0` evaluates `!(unit+0x20 bit 3)`). Keep the internal
        // explicit flag semantics true=protected and invert only this return.
        match record.invulnerable {
            None if !shielded => Err(self.inner.error(
                GET_INVULNERABLE,
                Failure::UnitState("initial invulnerability is not derived"),
            )),
            explicit => {
                let protected = explicit.unwrap_or(false) || shielded;
                Ok(Value::Bool(!protected))
            }
        }
    }

    fn set_unit_state(&mut self, args: &[Value]) -> Result<Value, Error> {
        let (unit, state, value, enumerand) = {
            let fail = |reason| self.inner.error(SET_STATE, reason);
            let [Value::Handle(unit), Value::Handle(state), value] = args else {
                return Err(fail(Failure::UnitState(
                    "arguments do not match the unit native declaration",
                )));
            };
            match self
                .inner
                .entry(*unit)
                .map_err(|error| self.inner.error(SET_STATE, error.failure))?
            {
                Entry::Unit { .. } => {}
                _ => return Err(fail(Failure::WrongFamily)),
            }
            let enumerand = match self
                .inner
                .entry(*state)
                .map_err(|error| self.inner.error(SET_STATE, error.failure))?
            {
                Entry::Symbolic { kind, enumerand } if kind == STATE_KIND => *enumerand,
                _ => return Err(fail(Failure::WrongFamily)),
            };
            let in_domain = self
                .inner
                .conversions
                .get(CONVERT_STATE)
                .is_some_and(|conversion| {
                    conversion.kind == STATE_KIND && conversion.domain.contains(&enumerand)
                });
            if !in_domain {
                return Err(fail(Failure::OutsideDomain(enumerand)));
            }
            let value = finite_state_value(value).map_err(fail)?;
            let (records, max_records) = {
                let limits = self
                    .admitted_units()?
                    .unit_state
                    .as_ref()
                    .ok_or_else(|| fail(Failure::UnitState("not admitted")))?;
                (limits.records, limits.max_records)
            };
            let replacing = match self
                .inner
                .entry(*unit)
                .map_err(|error| self.inner.error(SET_STATE, error.failure))?
            {
                Entry::Unit { record } => record
                    .unit_states
                    .iter()
                    .any(|assignment| assignment.unitstate == *state),
                _ => return Err(fail(Failure::WrongFamily)),
            };
            if !replacing && records >= max_records {
                return Err(fail(Failure::Budget("unit state assignments")));
            }
            (*unit, *state, value, enumerand)
        };
        let inserted = match self.inner.entry_mut(unit)? {
            Entry::Unit { record } => {
                if let Some(found) = record
                    .unit_states
                    .iter_mut()
                    .find(|assignment| assignment.unitstate == state)
                {
                    found.value = value;
                    false
                } else if record.unit_states.try_reserve(1).is_err() {
                    return Err(self.inner.error(SET_STATE, Failure::Capacity));
                } else {
                    record.unit_states.push(ExecutedUnitState {
                        unitstate: state,
                        value,
                    });
                    true
                }
            }
            _ => return Err(self.inner.error(SET_STATE, Failure::WrongFamily)),
        };
        if inserted {
            let limits = self
                .units
                .as_mut()
                .and_then(|admitted| admitted.unit_state.as_mut())
                .ok_or_else(|| {
                    self.inner
                        .error(SET_STATE, Failure::UnitState("not admitted"))
                })?;
            limits.records += 1;
        }
        if self.assign_unit_vital(unit, enumerand, value) {
            self.kill_unit(unit, None)?;
        }
        Ok(Value::Null)
    }

    fn set_resource_amount(&mut self, args: &[Value], add: bool) -> Result<Value, Error> {
        let name = if add { ADD_RESOURCE } else { SET_RESOURCE };
        if matches!(args, [Value::Null, Value::Int(_)]) {
            return Ok(Value::Null);
        }
        let (unit, amount, current, removed) = {
            let [Value::Handle(unit), Value::Int(amount)] = args else {
                return Err(self.inner.error(
                    name,
                    Failure::UnitState("arguments do not match the unit native declaration"),
                ));
            };
            match self
                .inner
                .entry(*unit)
                .map_err(|error| self.inner.error(name, error.failure))?
            {
                Entry::Unit { record } => {
                    if !self.resource_amount_family(name, &record.definition)? {
                        return Ok(Value::Null);
                    }
                    (*unit, *amount, record.resource_amount, false)
                }
                Entry::RemovedUnit {
                    removed_at,
                    resource_amount,
                    ..
                } => {
                    if !self.resource_retained(*removed_at) {
                        return Ok(Value::Null);
                    }
                    (*unit, *amount, *resource_amount, true)
                }
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }
        };
        let current = match current {
            Some(current) => current,
            None if removed => return Ok(Value::Null),
            None => {
                return Err(self
                    .inner
                    .error(name, Failure::UnitState("resource slot was not seeded")));
            }
        };
        if removed {
            match self.inner.entry_mut(unit)? {
                Entry::RemovedUnit {
                    resource_amount, ..
                } => {
                    *resource_amount = Some(if add {
                        current.wrapping_add(amount)
                    } else {
                        amount
                    });
                    Ok(Value::Null)
                }
                _ => Err(self.inner.error(name, Failure::WrongFamily)),
            }
        } else {
            match self.inner.entry_mut(unit)? {
                Entry::Unit { record } => {
                    record.resource_amount = Some(if add {
                        current.wrapping_add(amount)
                    } else {
                        amount
                    });
                    Ok(Value::Null)
                }
                _ => Err(self.inner.error(name, Failure::WrongFamily)),
            }
        }
    }

    fn get_resource_amount(&self, args: &[Value]) -> Result<Value, Error> {
        if matches!(args, [Value::Null]) {
            return Ok(Value::Int(0));
        }
        let [Value::Handle(unit)] = args else {
            return Err(self.inner.error(
                GET_RESOURCE,
                Failure::UnitState("arguments do not match the unit native declaration"),
            ));
        };
        match self
            .inner
            .entry(*unit)
            .map_err(|error| self.inner.error(GET_RESOURCE, error.failure))?
        {
            Entry::Unit { record } => {
                if self.resource_amount_family(GET_RESOURCE, &record.definition)? {
                    Ok(Value::Int(record.resource_amount.ok_or_else(|| {
                        self.inner.error(
                            GET_RESOURCE,
                            Failure::UnitState("resource slot was not seeded"),
                        )
                    })?))
                } else {
                    Ok(Value::Int(0))
                }
            }
            Entry::RemovedUnit {
                removed_at,
                resource_amount,
                ..
            } => Ok(Value::Int(if self.resource_retained(*removed_at) {
                resource_amount.unwrap_or(0)
            } else {
                0
            })),
            _ => Err(self.inner.error(GET_RESOURCE, Failure::WrongFamily)),
        }
    }

    /// Same-tick tombstone reads/writes retain the captured resource amount.
    /// Later ticks answer zero and ignore writes, matching the followup probe
    /// at 5.063 and 6.063 after removal at 5.000. The exact native
    /// reclamation phase remains unqualified.
    fn resource_retained(&self, removed_at: Option<f64>) -> bool {
        removed_at.is_some() && self.timer_game_time() == removed_at
    }

    /// Read the constructor seed from the original ability row.
    ///
    /// The client's class binding maps the asset alias `Agld` to the `Agmb`
    /// ability class that owns the `+0x70` amount slot. That class constructor
    /// initializes the slot from the row's `DataA1`. The other gold-mine
    /// abilities (`Aegm`, `Abgm`) are separate classes and were observed to
    /// start at zero, so only the bound alias supplies a seed. A resource
    /// family without that alias starts at the constructor's zero value.
    fn initial_resource_amount(&self, definition: &Definition) -> Result<Option<i32>, Error> {
        if !self.resource_amount_family(NATIVE, definition)? {
            return Ok(None);
        }
        let admitted = self.admitted_units()?;
        let tables = admitted.resource_tables.as_deref().ok_or_else(|| {
            let mut error = self.inner.error(
                NATIVE,
                Failure::UnitState("resource tables are not admitted"),
            );
            error.source_context.push_str(&format!(
                "; unit {:?} needs the selected ability tables",
                definition.base_rawcode
            ));
            error
        })?;
        let Some(alias) = resource_amount_alias(definition) else {
            return Ok(Some(0));
        };
        let fail = |detail: String| {
            let mut error = self.inner.error(
                NATIVE,
                Failure::UnitState("resource ability seed is not readable"),
            );
            error.source_context.push_str(&format!(
                "; unit {:?} ability {alias:?}: {detail}",
                definition.base_rawcode
            ));
            error
        };
        let ability = tables
            .object(Family::Ability, alias)
            .map_err(|error| fail(error.to_string()))?;
        let cell = ability
            .cell(SlkId::AbilityData, "DataA1")
            .map_err(|error| fail(error.to_string()))?;
        let value = cell
            .typed(FieldKind::Int)
            .map_err(|error| fail(error.to_string()))?;
        let pcx_objects::editor::defaults::Value::Integer(value) = value else {
            return Err(fail(format!("DataA1 is not an integer: {value:?}")));
        };
        let amount = i32::try_from(value)
            .map_err(|_| fail(format!("DataA1 is outside the integer range: {value}")))?;
        Ok(Some(amount))
    }

    /// The original ability read/write is reached only through the unit's
    /// resource ability. `UnitData` `buffType` is the asset-authored family
    /// marker: `resource` names the three gold mines; every other token does
    /// not. A definition that cannot supply the cell is reported rather than
    /// treated as non-resource.
    fn resource_amount_family(&self, name: &str, definition: &Definition) -> Result<bool, Error> {
        let token = super::unit_positions::text(definition, SlkId::UnitData, "buffType").map_err(
            |reason| {
                let mut error = self.inner.error(name, Failure::UnitState(reason));
                error.source_context.push_str(&format!(
                    "; unit {:?} has no usable UnitData buffType",
                    definition.base_rawcode
                ));
                error
            },
        )?;
        Ok(token.trim() == "resource")
    }

    fn set_acquire_range(&mut self, args: &[Value]) -> Result<Value, Error> {
        let (id, range) = {
            let fail = |reason| self.inner.error(SET_RANGE, reason);
            let [Value::Handle(unit), range] = args else {
                return Err(fail(Failure::UnitState(
                    "arguments do not match the unit native declaration",
                )));
            };
            let range = finite_real(range).map_err(fail)?;
            match self
                .inner
                .entry(*unit)
                .map_err(|error| self.inner.error(SET_RANGE, error.failure))?
            {
                Entry::Unit { .. } => {}
                _ => return Err(fail(Failure::WrongFamily)),
            }
            (*unit, range)
        };
        match self.inner.entry_mut(id)? {
            Entry::Unit { record } => {
                record.acquire_range = Some(range);
                Ok(Value::Null)
            }
            _ => Err(self.inner.error(SET_RANGE, Failure::WrongFamily)),
        }
    }

    fn create_unit(&mut self, args: &[Value]) -> Result<Value, Error> {
        self.create_unit_with_cache(args, None, false)
    }

    pub(super) fn create_summoned_unit(&mut self, args: &[Value]) -> Result<Value, Error> {
        self.create_unit_with_cache(args, None, true)
    }

    pub(super) fn create_cached_unit(
        &mut self,
        args: &[Value],
        saved: &crate::game_cache::CachedUnit,
    ) -> Result<Value, Error> {
        saved
            .validate()
            .map_err(|detail| self.inner.error("RestoreUnit", Failure::GameCache(detail)))?;
        let source = &self.admitted_units()?.source;
        if self
            .order_registry
            .as_ref()
            .is_none_or(|registry| registry.binding().image_sha256 != saved.client_sha256)
        {
            return Err(self.inner.error(
                "RestoreUnit",
                Failure::GameCache("cached unit belongs to a different client image".into()),
            ));
        }
        if source.install != saved.install || source.edition != saved.edition {
            return Err(self.inner.error(
                "RestoreUnit",
                Failure::GameCache("cached unit belongs to a different install/edition".into()),
            ));
        }
        self.create_unit_with_cache(args, Some(saved), false)
    }

    fn create_unit_with_cache(
        &mut self,
        args: &[Value],
        saved: Option<&crate::game_cache::CachedUnit>,
        summoned: bool,
    ) -> Result<Value, Error> {
        let record = {
            let fail = |reason| self.inner.error(NATIVE, reason);
            let [Value::Handle(player), Value::Int(unitid), x, y, face] = args else {
                return Err(fail(Failure::UnitState(
                    "arguments do not match the unit native declaration",
                )));
            };
            let x = finite_real(x).map_err(fail)?;
            let y = finite_real(y).map_err(fail)?;
            let face = finite_real(face).map_err(fail)?;
            let rawcode = unitid.to_be_bytes();
            let owner_number = match self
                .inner
                .entry(*player)
                .map_err(|error| self.inner.error(NATIVE, error.failure))?
            {
                Entry::Player { number } => *number,
                _ => return Err(fail(Failure::WrongFamily)),
            };
            let owner = *player;
            let (records, max_records) = {
                let admitted = self.admitted_units()?;
                (admitted.records, admitted.max_records)
            };
            if records >= max_records {
                return Err(fail(Failure::Budget("unit creation")));
            }
            if self.inner.handles() >= self.inner.max_handles {
                return Err(fail(Failure::Capacity));
            }
            let catalog = Arc::clone(&self.admitted_units()?.catalog);
            let (max_fields, max_modifications) = {
                let admitted = self.admitted_units()?;
                (admitted.max_retained_fields, admitted.max_modifications)
            };
            let definition = match catalog.resolve(rawcode) {
                Ok(definition) => definition,
                Err(problem) => {
                    return Err(fail(Failure::UnitCatalog {
                        requested: rawcode,
                        problem: Box::new(problem),
                    }));
                }
            };
            if definition.rawcode != rawcode {
                return Err(fail(Failure::UnitCatalog {
                    requested: rawcode,
                    problem: Box::new(ResolutionProblem::RawcodeMismatch {
                        requested: rawcode,
                        resolved: definition.rawcode,
                    }),
                }));
            }
            if definition.kind != ObjectKind::Unit {
                return Err(fail(Failure::UnitCatalog {
                    requested: rawcode,
                    problem: Box::new(ResolutionProblem::WrongFamily {
                        expected: ObjectKind::Unit,
                        actual: definition.kind,
                    }),
                }));
            }
            if definition.fields.len() > max_fields {
                return Err(fail(Failure::UnitState(
                    "resolved definition exceeds the retained-field budget",
                )));
            }
            if definition.modifications.len() > max_modifications {
                return Err(fail(Failure::UnitState(
                    "resolved definition exceeds the modification budget",
                )));
            }
            let source = self.admitted_units()?.source.clone();
            let admitted_order = self
                .units
                .as_ref()
                .and_then(|admitted| admitted.order.as_ref())
                .and_then(|order| {
                    lists_admitted_alias(&definition, &order.alias).then_some((order.code, false))
                });
            let resource_amount = if self.admitted_units()?.resource_amounts {
                self.initial_resource_amount(&definition)?
            } else {
                None
            };
            ExecutedUnit {
                collision_radius: Self::catalog_collision_radius(&definition),
                definition,
                executed_owner: owner_number,
                owner,
                team_color: None,
                creation_color: None,
                source,
                x,
                y,
                position: None,
                face,
                acquire_range: None,
                requested_hero_level: None,
                requested_show_eye_candy: None,
                unit_states: Vec::new(),
                resource_amount,
                invulnerable: None,
                admitted_order,
                abilities: None,
                animation: None,
                look_at: None,
                summoned,
                activity: None,
                shown: true,
                paused_since: None,
                sleep: None,
                vitals: None,
                permanent_strength: 0,
                combat: None,
            }
        };
        let mut record = record;
        record.sleep = self.prepare_unit_sleep(&record.definition)?;
        record.abilities = self.prepare_unit_abilities(&record.definition)?;
        if let Some(saved) = saved {
            let abilities = record.abilities.as_mut().ok_or_else(|| {
                self.inner.error(
                    "RestoreUnit",
                    Failure::GameCache("cached unit ability admission missing".into()),
                )
            })?;
            self.restore_cached_abilities(abilities, saved)?;
            record.permanent_strength = saved.permanent_strength;
        }
        record.activity = self.prepare_unit_activity(&record.definition)?;
        record.vitals = self.prepare_unit_vitals(&record.definition)?;
        record.combat = self.prepare_unit_combat(&record.definition)?;
        if let Some(loader) = self.units.as_mut().and_then(|u| u.models.as_mut()) {
            let model = loader(&record.definition)
                .map_err(|e| self.inner.error(NATIVE, Failure::UnitAnimation(e)))?;
            if let Some(model) = model {
                record.animation = Some(
                    crate::unit_model::Animation::new(
                        model,
                        self.timer_game_time().expect("admitted clock"),
                    )
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitAnimation(e)))?,
                );
            }
        }
        let placement = self.prepare_unit_position(&record)?;
        if record.animation.is_some() {
            record.creation_color = self.player_settings(record.owner)?.color;
        }
        if let Some(prepared) = &placement {
            record.position = Some(prepared.position);
        }
        let guard = if !record.summoned
            && self.guard_managed_owner(record.owner)?
            && super::unit_guards::candidate(&record.definition, record.combat.as_ref())
                .map_err(|error| self.inner.error(NATIVE, Failure::UnitActivity(error)))?
        {
            let position = record.position.ok_or_else(|| {
                self.inner.error(
                    NATIVE,
                    Failure::UnitActivity("guard creation requires accepted placement".into()),
                )
            })?;
            self.prepare_unit_guard(None, record.owner, position, true)?
        } else {
            None
        };
        let inventory = saved
            .map(|saved| self.prepare_cached_inventory(&record, &saved.items))
            .transpose()?;
        if let Some(prepared) = &inventory {
            record.vitals =
                Some(self.prepare_cached_restored_vitals(&record, prepared.definitions())?);
        }
        let id = self.inner.allocate(
            Entry::Unit {
                record: Box::new(record),
            },
            NATIVE,
        )?;
        self.publish_unit_guard(id, guard);
        if let Some(prepared) = placement {
            self.unit_placement
                .as_mut()
                .expect("admitted placement")
                .commit(id, prepared);
        }
        let admitted = self
            .units
            .as_mut()
            .ok_or_else(|| self.inner.error(NATIVE, Failure::UnitState("not admitted")))?;
        admitted.records += 1;
        if let Some(inventory) = inventory {
            self.commit_cached_inventory(id, inventory);
        }
        Ok(Value::Handle(id))
    }

    /// The authored `militiaoff` request on the rank-1 base form.
    ///
    /// Only a successful base-form match returns a boolean. The active form,
    /// an unrelated unit type, the morph-on text and every other order text
    /// stay on the unsupported path. Null and removed units return false;
    /// a non-unit handle refuses as the wrong family.
    fn militia_immediate_order(&mut self, args: &[Value]) -> Option<Result<Value, Error>> {
        let (alias, off, base) = {
            let militia = self.units.as_ref()?.militia.as_ref()?;
            (militia.alias.as_str(), militia.off.as_str(), militia.base)
        };
        let [unit, Value::Str(text)] = args else {
            return None;
        };
        if text != off {
            return None;
        }
        let unit = match unit {
            Value::Null => return Some(Ok(Value::Bool(false))),
            Value::Handle(unit) => *unit,
            _ => return Some(Err(self.inner.error(ORDER, Failure::WrongFamily))),
        };
        let record = match self
            .inner
            .entry(unit)
            .map_err(|error| self.inner.error(ORDER, error.failure))
        {
            Ok(Entry::Unit { record }) => record,
            Ok(Entry::RemovedUnit { .. }) => return Some(Ok(Value::Bool(false))),
            Ok(_) => return Some(Err(self.inner.error(ORDER, Failure::WrongFamily))),
            Err(error) => return Some(Err(error)),
        };
        if record.definition.rawcode != base {
            return None;
        }
        if !lists_admitted_alias(&record.definition, alias) {
            return None;
        }
        let ability = one_fourcc(MILITIA_ALIAS).expect("named ability rawcode");
        Some(
            self.unit_ability_available(unit, ability)
                .map(|_| Value::Bool(false)),
        )
    }

    fn issue_immediate_order(&mut self, args: &[Value]) -> Option<Result<Value, Error>> {
        let (on, off, tech, required_level, ability) = {
            let order = self.units.as_ref()?.order.as_ref()?;
            (
                order.on.as_str(),
                order.off.as_str(),
                order.tech,
                order.required_level,
                one_fourcc(&order.alias).expect("admitted ability rawcode"),
            )
        };
        let [Value::Handle(unit), Value::Str(text)] = args else {
            return Some(Err(self.inner.error(
                ORDER,
                Failure::UnitState("arguments do not match the unit native declaration"),
            )));
        };
        let unit = *unit;
        let active = match self
            .inner
            .entry(unit)
            .map_err(|error| self.inner.error(ORDER, error.failure))
        {
            Ok(Entry::Unit { record }) => match record.admitted_order {
                Some((_, active)) => active,
                None => return None,
            },
            Ok(_) => return Some(Err(self.inner.error(ORDER, Failure::WrongFamily))),
            Err(error) => return Some(Err(error)),
        };
        let owner = match self.current_unit_owner(unit, ORDER) {
            Ok((_, number)) => number,
            Err(error) => return Some(Err(error)),
        };
        let turning_off = text == off;
        let turning_on = text == on;
        if !turning_off && !turning_on {
            return None;
        }
        if self
            .executed_unit(unit)
            .is_ok_and(|record| record.paused_since.is_some())
        {
            return Some(Ok(Value::Bool(false)));
        }
        match self.unit_ability_available(unit, ability) {
            Ok(true) => {}
            Ok(false) => return Some(Ok(Value::Bool(false))),
            Err(error) => return Some(Err(error)),
        }
        if turning_off && !active {
            return Some(Ok(Value::Bool(false)));
        }
        let ready = matches!(
            self.stored_tech_level(owner, tech),
            TechRead::Level(level) if level == required_level
        );
        if !ready {
            return None;
        }
        let next = if turning_on && !active {
            Some(true)
        } else if turning_off && active {
            Some(false)
        } else {
            None
        };
        let Some(next) = next else {
            return Some(Ok(Value::Bool(false)));
        };
        match self.inner.entry_mut(unit) {
            Ok(Entry::Unit { record }) => {
                if let Some(bit) = record.admitted_order.as_mut() {
                    bit.1 = next;
                }
                self.wake_natural_sleep(unit);
                Some(Ok(Value::Bool(true)))
            }
            Ok(_) => Some(Err(self.inner.error(ORDER, Failure::WrongFamily))),
            Err(error) => Some(Err(self.inner.error(ORDER, error.failure))),
        }
    }
}

fn read_militia_ability(tables: &DefaultTables) -> Result<MilitiaAbility, &'static str> {
    let alias = MILITIA_ALIAS;
    let object = tables
        .object(Family::Ability, alias)
        .map_err(|_| "militia ability lookup failed")?;
    let alias_cell = present_cell(object.cell(SlkId::AbilityData, "alias"), "alias")?;
    if alias_cell.text != alias {
        return Err("militia alias spelling differs from the selected row");
    }
    let code_cell = present_cell(object.cell(SlkId::AbilityData, "code"), "code")?;
    if code_cell.text != alias {
        return Err("militia code differs from the authored class");
    }
    require_int(object.cell(SlkId::AbilityData, "levels"), "levels", 1)?;
    require_int(object.level_cell("Cost", 1), "Cost1", 0)?;
    require_real_zero(object.level_cell("Cool", 1), "Cool1")?;
    require_int(object.cell(SlkId::AbilityData, "reqLevel"), "reqLevel", 0)?;
    require_int(object.cell(SlkId::AbilityData, "levelSkip"), "levelSkip", 0)?;
    require_bool(object.cell(SlkId::AbilityData, "hero"), "hero", false)?;
    require_bool(object.cell(SlkId::AbilityData, "item"), "item", false)?;
    require_bool(
        object.cell(SlkId::AbilityData, "checkDep"),
        "checkDep",
        true,
    )?;
    let on = one_assignment(object, "Order")?;
    let off = one_assignment(object, "Unorder")?;
    if on == off {
        return Err("militia Order and Unorder are the same text");
    }
    let base_cell = present_cell(object.level_cell("DataA", 1), "DataA1")?;
    let base = one_fourcc(base_cell.text).ok_or("militia DataA1 is not one fourcc")?;
    let active_cell = present_cell(object.level_cell("DataB", 1), "DataB1")?;
    let active = one_fourcc(active_cell.text).ok_or("militia DataB1 is not one fourcc")?;
    if base == active {
        return Err("militia DataA1 and DataB1 are the same form");
    }
    let requires = object.profile("Requires");
    if !(requires.is_empty()
        || requires.len() == 1
            && requires[0].parse_error.is_none()
            && requires[0].text.trim().is_empty())
    {
        return Err("militia Requires is not an absent or blank level");
    }
    let amount = object.profile("Requiresamount");
    if !(amount.is_empty()
        || amount.len() == 1 && amount[0].parse_error.is_none() && amount[0].text.trim().is_empty())
    {
        return Err("militia Requiresamount is not an absent or blank level");
    }
    Ok(MilitiaAbility {
        alias: alias.to_string(),
        off,
        base,
    })
}

fn read_defend_ability(tables: &DefaultTables, alias: &str) -> Result<DefendAbility, &'static str> {
    if one_fourcc(alias).is_none() {
        return Err("ability alias is not one fourcc");
    }
    let object = tables
        .object(Family::Ability, alias)
        .map_err(|_| "ability alias is absent")?;
    let alias_cell = present_cell(object.cell(SlkId::AbilityData, "alias"), "alias")?;
    if alias_cell.text != alias {
        return Err("ability alias spelling differs from the selected row");
    }
    let code_cell = present_cell(object.cell(SlkId::AbilityData, "code"), "code")?;
    let code = one_fourcc(code_cell.text).ok_or("code is not one fourcc")?;
    require_int(object.cell(SlkId::AbilityData, "levels"), "levels", 1)?;
    require_int(object.level_cell("Cost", 1), "Cost1", 0)?;
    require_real_zero(object.level_cell("Cool", 1), "Cool1")?;
    require_int(object.cell(SlkId::AbilityData, "reqLevel"), "reqLevel", 0)?;
    require_int(object.cell(SlkId::AbilityData, "levelSkip"), "levelSkip", 0)?;
    require_bool(object.cell(SlkId::AbilityData, "hero"), "hero", false)?;
    require_bool(object.cell(SlkId::AbilityData, "item"), "item", false)?;
    require_bool(
        object.cell(SlkId::AbilityData, "checkDep"),
        "checkDep",
        true,
    )?;
    let on = one_assignment(object, "Order")?;
    let off = one_assignment(object, "Unorder")?;
    if on == off {
        return Err("Order and Unorder are the same text");
    }
    let requires = one_assignment(object, "Requires")?;
    let tech = one_fourcc(&requires).ok_or("Requires is not one fourcc")?;
    let amount = object.profile("Requiresamount");
    match amount.as_slice() {
        [] => {}
        [cell] if cell.parse_error.is_none() && cell.text.trim().is_empty() => {}
        _ => return Err("Requiresamount is not an absent or blank level"),
    }
    let upgrade = tables
        .object(Family::Upgrade, requires.trim())
        .map_err(|_| "required upgrade is absent")?;
    let upgrade_key = SlkId::UpgradeData
        .id_column()
        .ok_or("upgrade identity column is absent")?;
    let upgrade_id = present_cell(upgrade.cell(SlkId::UpgradeData, upgrade_key), "upgradeid")?;
    if upgrade_id.text != requires.trim() {
        return Err("required upgrade spelling differs from the selected row");
    }
    let maximum = present_cell(upgrade.cell(SlkId::UpgradeData, "maxlevel"), "maxlevel")?;
    let required_level = match maximum.typed(FieldKind::Int) {
        Ok(defaults::Value::Integer(level)) if level == 1 => {
            i32::try_from(level).map_err(|_| "upgrade level width")?
        }
        _ => return Err("Defend requires one authored upgrade level"),
    };
    Ok(DefendAbility {
        alias: alias.to_string(),
        code,
        on,
        off,
        tech: i32::from_be_bytes(tech),
        required_level,
    })
}

fn present_cell<'a>(
    cell: Result<defaults::Cell<'a>, defaults::Refusal>,
    column: &'static str,
) -> Result<defaults::Cell<'a>, &'static str> {
    let cell = cell.map_err(|_| column_problem(column, "missing"))?;
    if cell.parse_error.is_some() || cell.text.trim().is_empty() {
        return Err(column_problem(column, "blank"));
    }
    Ok(cell)
}

fn require_int(
    cell: Result<defaults::Cell<'_>, defaults::Refusal>,
    column: &'static str,
    expected: i64,
) -> Result<(), &'static str> {
    let cell = present_cell(cell, column)?;
    match cell.typed(FieldKind::Int) {
        Ok(defaults::Value::Marker(_)) => Err(column_problem(column, "marker")),
        Ok(defaults::Value::Integer(value)) if value == expected => Ok(()),
        _ if expected == 1 => Err(column_problem(column, "not-level")),
        _ => Err(column_problem(column, "not-zero")),
    }
}

fn require_real_zero(
    cell: Result<defaults::Cell<'_>, defaults::Refusal>,
    column: &'static str,
) -> Result<(), &'static str> {
    let cell = present_cell(cell, column)?;
    match cell.typed(FieldKind::Unreal) {
        Ok(defaults::Value::Marker(_)) => Err(column_problem(column, "marker")),
        Ok(defaults::Value::Real(0.0)) => Ok(()),
        _ => Err(column_problem(column, "not-zero")),
    }
}

fn require_bool(
    cell: Result<defaults::Cell<'_>, defaults::Refusal>,
    column: &'static str,
    expected: bool,
) -> Result<(), &'static str> {
    let cell = present_cell(cell, column)?;
    match cell.typed(FieldKind::Bool) {
        Ok(defaults::Value::Marker(_)) => Err(column_problem(column, "marker")),
        Ok(defaults::Value::Boolean(value)) if value == expected => Ok(()),
        _ if expected => Err(column_problem(column, "not-true")),
        _ => Err(column_problem(column, "not-zero")),
    }
}

fn one_assignment(object: defaults::Object<'_>, key: &'static str) -> Result<String, &'static str> {
    let cells = object.profile(key);
    let cell = match cells.as_slice() {
        [cell] => cell,
        [] => return Err(column_problem(key, "missing")),
        _ => return Err(column_problem(key, "ambiguous")),
    };
    if cell.parse_error.is_some() || cell.text.trim().is_empty() {
        return Err(column_problem(key, "blank"));
    }
    if cell.text.contains(',') || cell.text.contains('"') {
        return Err(column_problem(key, "not-one"));
    }
    Ok(cell.text.to_string())
}

fn one_fourcc(text: &str) -> Option<[u8; 4]> {
    let text = text.trim();
    if text.len() == 4 && text.bytes().all(|byte| byte.is_ascii_graphic()) {
        let mut code = [0; 4];
        code.copy_from_slice(text.as_bytes());
        Some(code)
    } else {
        None
    }
}

fn column_problem(column: &'static str, problem: &'static str) -> &'static str {
    match (column, problem) {
        ("code", "missing") => "missing code",
        ("levels", "missing") => "missing levels",
        ("levels", "blank") => "blank levels",
        ("levels", "marker") => "marker levels",
        ("levels", "not-level") => "levels is not 1",
        ("Cost1", "missing") => "missing Cost1",
        ("Cost1", "blank") => "blank Cost1",
        ("Cost1", "marker") => "marker Cost1",
        ("Cost1", "not-zero") => "Cost1 is not authored zero",
        ("Cool1", "missing") => "missing Cool1",
        ("Cool1", "blank") => "blank Cool1",
        ("Cool1", "marker") => "marker Cool1",
        ("Cool1", "not-zero") => "Cool1 is not authored zero",
        ("reqLevel", "missing") => "missing reqLevel",
        ("reqLevel", "blank") => "blank reqLevel",
        ("reqLevel", "marker") => "marker reqLevel",
        ("reqLevel", "not-zero") => "reqLevel is not authored zero",
        ("levelSkip", "missing") => "missing levelSkip",
        ("levelSkip", "blank") => "blank levelSkip",
        ("levelSkip", "marker") => "marker levelSkip",
        ("levelSkip", "not-zero") => "levelSkip is not authored zero",
        ("hero", "missing") => "missing hero",
        ("hero", "blank") => "blank hero",
        ("hero", "marker") => "marker hero",
        ("hero", "not-zero") => "hero is not authored zero",
        ("item", "missing") => "missing item",
        ("item", "blank") => "blank item",
        ("item", "marker") => "marker item",
        ("item", "not-zero") => "item is not authored zero",
        ("checkDep", "missing") => "missing checkDep",
        ("checkDep", "blank") => "blank checkDep",
        ("checkDep", "marker") => "marker checkDep",
        ("checkDep", "not-true") => "checkDep is not true",
        ("Order", "missing") => "missing Order",
        ("Order", "blank") => "blank Order",
        ("Order", "not-one") => "Order is not one text",
        ("Order", "ambiguous") => "Order has more than one assignment",
        ("Unorder", "missing") => "missing Unorder",
        ("Unorder", "blank") => "blank Unorder",
        ("Unorder", "not-one") => "Unorder is not one text",
        ("Unorder", "ambiguous") => "Unorder has more than one assignment",
        ("DataA1", "missing") => "missing DataA1",
        ("DataA1", "blank") => "blank DataA1",
        ("DataB1", "missing") => "missing DataB1",
        ("DataB1", "blank") => "blank DataB1",
        ("Requires", "missing") => "missing Requires",
        ("Requires", "blank") => "blank Requires",
        ("Requires", "not-one") => "Requires is not one fourcc",
        ("Requires", "ambiguous") => "Requires has more than one assignment",
        _ => "immediate order row is not the admitted class",
    }
}

fn lists_admitted_alias(definition: &Definition, alias: &str) -> bool {
    definition.fields.values().any(|field| {
        ability_list_text(field).is_some_and(|text| {
            text.split(',').any(|token| {
                let token = token.trim();
                token.len() == 4 && token == alias
            })
        })
    })
}

fn resource_amount_alias(definition: &Definition) -> Option<&str> {
    definition
        .fields
        .values()
        .filter_map(ability_list_text)
        .flat_map(|text| text.split(','))
        .map(str::trim)
        .find(|token| *token == RESOURCE_ALIAS)
}

fn ability_list_text(field: &ObjectField) -> Option<&str> {
    let metadata = field.metadata.as_ref()?;
    let FieldSource::Column { slk, column } = &metadata.source else {
        return None;
    };
    if *slk != SlkId::UnitAbilities || column != "abilList" {
        return None;
    }
    match field.value.as_ref().ok()? {
        FieldValue::Cell(text) => Some(text.as_str()),
        FieldValue::Override(pcx_objects::Value::Str(bytes)) => std::str::from_utf8(bytes).ok(),
        _ => None,
    }
}

fn finite_real(value: &Value) -> Result<f32, Failure> {
    match value {
        Value::Real(real) if real.is_finite() => Ok(*real),
        Value::Real(_) => Err(Failure::UnitState(
            "recreation policy: finite real required",
        )),
        _ => Err(Failure::UnitState(
            "arguments do not match the unit native declaration",
        )),
    }
}

/// Finite `real`, or an integer literal in that slot via [`Value::as_real`].
///
/// The integer conversion is `i as f32`. It is not a clamp or a unit-state scale.
fn finite_state_value(value: &Value) -> Result<f32, Failure> {
    let real = value.as_real().ok_or(Failure::UnitState(
        "arguments do not match the unit native declaration",
    ))?;
    if real.is_finite() {
        Ok(real)
    } else {
        Err(Failure::UnitState(
            "recreation policy: finite real required",
        ))
    }
}
