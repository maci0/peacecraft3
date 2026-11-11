//! Same-handle form identity; phase execution shares canonical units and game time.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MorphPhase {
    TakingOff,
    Alternate,
    Landing,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UnitMorph {
    pub ability: [u8; 4],
    pub normal: [u8; 4],
    pub alternate: [u8; 4],
    pub revision: u64,
    pub phase: MorphPhase,
    pending: Option<Box<Pending>>,
    height_target: f32,
    flight: Option<Flight>,
}

use super::{Entry, Error, EventSubject, Failure, ObjectHost, UnitAbilities};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::{HandleId, RealOps, ScriptCall, Value, ast::BinOp};
use pcx_sim::world::Definition;
use pcx_sim::world::FieldValue;

/// `UnitBalance` columns an immediate form change keeps equal across the forms.
const POOL_COLUMNS: [&str; 5] = ["HP", "manaN", "fused", "fmade", "isbldg"];
/// A delayed change also keeps the unit's armor, defense type and mana regeneration.
const SWAP_COLUMNS: [&str; 8] = [
    "HP",
    "manaN",
    "fused",
    "fmade",
    "isbldg",
    "def",
    "defType",
    "regenMana",
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Flight {
    from: f32,
    to: f32,
    start: f64,
    finish: f64,
}
impl Flight {
    fn sample(self, now: f64) -> f32 {
        let t = if self.finish > self.start {
            ((now - self.start) / (self.finish - self.start)).clamp(0., 1.) as f32
        } else {
            1.
        };
        self.from + (self.to - self.from) * t
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Form {
    pub definition: Definition,
    pub animation: Option<crate::unit_model::Animation>,
    pub activity: Option<super::UnitActivity>,
    pub combat: Option<super::Combat>,
}
#[derive(Clone, Debug, PartialEq)]
struct Pending {
    stage: EventPhase,
    order: i32,
    overridden: bool,
    finish: f64,
    cast: Option<f64>,
    spec: crate::unit_morph_rules::Spec,
    change: Change,
}
/// When and how the unit becomes the other form.
#[derive(Clone, Debug, PartialEq)]
enum Change {
    /// Immediate-form-change bit set: the unit changes at the order and the altitude phases follow.
    Immediate(Box<Immediate>),
    /// Bit clear: `0xbafb00` posts event `0xd01c4` after `Dur` and `0xbaf580` changes the unit
    /// then. `None` when the unit's type is not the source form of the direction; `0xbaf580`
    /// skips the type change for such a unit (`docs/re-notes/unit-root-burrow.md`).
    Delayed(Option<Box<Swap>>),
}
#[derive(Clone, Debug, PartialEq)]
struct Immediate {
    normal: Form,
    alternate: Form,
    normal_height: f32,
    alternate_height: f32,
    normal_movement: crate::unit_placement::Movement,
}
/// The prepared target form of a delayed change and the ability list it replaces.
#[derive(Clone, Debug, PartialEq)]
struct Swap {
    form: Form,
    /// The target type's ability list, from its `abilList`.
    abilities: Option<UnitAbilities>,
    /// The unit's list at the order; a different list at the swap refuses the change.
    source_abilities: Option<UnitAbilities>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventPhase {
    Begin,
    Running,
    Finish,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Context {
    pub unit: HandleId,
    pub revision: u64,
    pub phase: EventPhase,
}

impl UnitMorph {
    pub(super) fn movement(&self) -> Option<crate::unit_placement::Movement> {
        let pending = self.pending.as_ref()?;
        let Change::Immediate(change) = &pending.change else {
            return None;
        };
        (self.phase == MorphPhase::Landing && pending.stage != EventPhase::Begin)
            .then_some(change.normal_movement)
    }
    /// A delayed change in progress and nothing else: the form swap is the `0xd01c4` timer on the
    /// ability (`0xbafb00`), which the unit pause does not stop (`docs/re-notes/unit-root-burrow.md`).
    fn delayed_timer_only(&self) -> bool {
        self.flight.is_none()
            && self
                .pending
                .as_ref()
                .is_some_and(|p| matches!(p.change, Change::Delayed(_)))
    }
    fn order(&self) -> Option<i32> {
        self.pending
            .as_ref()
            .filter(|p| {
                !p.overridden
                    && (p.stage != EventPhase::Running || self.phase != MorphPhase::Alternate)
            })
            .map(|p| p.order)
    }
}
impl ObjectHost {
    fn morph_error(&self, why: impl Into<String>) -> Error {
        self.inner.error("morph", Failure::UnitActivity(why.into()))
    }
    fn morph_order(&self, name: &str, args: &[Value]) -> Option<(i32, bool)> {
        let [_, order] = args else { return None };
        let id = match (name, order) {
            ("IssueImmediateOrder", Value::Str(s)) => self.order_registry.as_ref()?.id(s)?,
            ("IssueImmediateOrderById", Value::Int(i)) => *i,
            _ => return None,
        };
        let rules = self.admitted_morph_rules()?.ok()?;
        rules.classes.iter().find_map(|c| {
            if id == c.on {
                Some((id, true))
            } else if id == c.off {
                Some((id, false))
            } else {
                None
            }
        })
    }
    pub(super) fn morph_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name == "SetUnitFlyHeight" && self.inner.program.natives().get(name).is_some() {
            return Some(
                self.set_morph_flight_height(name, args)
                    .map_err(|e| self.inner.error(name, e.failure)),
            );
        }
        if name == "GetUnitFlyHeight" && self.inner.program.natives().get(name).is_some() {
            return Some((|| {
                Self::contract(&self.inner, name, &["unit"], "real", true, Some("unit"))?;
                let unit = match args {
                    [Value::Null] => return Ok(Value::Real(0.)),
                    [Value::Handle(u)] => *u,
                    _ => return Err(self.inner.error(name, Failure::Signature)),
                };
                let record = match self.inner.entry(unit)? {
                    Entry::RemovedUnit { .. } => return Ok(Value::Real(0.)),
                    Entry::Unit { record } => record,
                    _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                };
                Ok(Value::Real(match &record.morph {
                    Some(m) => m.height_target,
                    None => self.morph_height(&record.definition)?,
                }))
            })());
        }
        let (order, on) = self.morph_order(name, args)?;
        Some(
            self.accept_morph(name, args, order, on)
                .map(Value::Bool)
                .map_err(|e| self.inner.error(name, e.failure)),
        )
    }
    fn set_morph_flight_height(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        Self::contract(
            &self.inner,
            name,
            &["unit", "real", "real"],
            "nothing",
            false,
            Some("unit"),
        )?;
        let (unit, target, rate) = match args {
            [Value::Null, Value::Real(_), Value::Real(_)] => return Ok(Value::Null),
            [Value::Handle(u), Value::Real(target), Value::Real(rate)] => (*u, *target, *rate),
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        let record = match self.inner.entry(unit)? {
            Entry::RemovedUnit { .. } => return Ok(Value::Null),
            Entry::Unit { record } => record,
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        };
        if !target.is_finite() || !rate.is_finite() || rate < 0. {
            return Err(self.inner.error(
                name,
                Failure::UnitActivity("finite height and nonnegative flight rate required".into()),
            ));
        }
        // This first capability covers the observed ordinary form, without inventing
        // a generic flight-granting ability or accepting unqualified ground behavior.
        let morph = record.morph.as_ref().ok_or_else(|| {
            let abilities = record.abilities.as_ref().map(|a| a.levels().take(16)
                .map(|(alias, rank)| (alias, rank, self.unit_ability_available(unit, alias).map_err(|e| e.to_string())))
                .collect::<Vec<_>>());
            self.morph_error(format!(
                "height setter without an admitted form is not qualified: unit={unit:?} rawcode={:?} owner={:?} revision={} movement={:?} shown={} paused={:?} vitals={:?} combat_admitted={} dead={:?} current_order={:?} abilities_first16={abilities:?} target={target:?} rate={rate:?} clock={:?}",
                record.definition.rawcode, record.owner, record.definition_revision, record.movement(),
                record.shown, record.paused_since, record.vitals, record.combat.is_some(),
                record.combat.as_ref().map(|c| c.dead), self.current_unit_order(unit), self.timer_game_time(),
            ))
        })?;
        if record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|c| c.dead)
            || record.movement().map_err(|e| self.morph_error(e))?
                != crate::unit_placement::Movement::Air
        {
            return Err(self.morph_error("paused, dead or grounded height setter is not qualified"));
        }
        let rules = self
            .admitted_morph_rules()
            .ok_or_else(|| self.morph_error("morph admission missing"))?
            .map_err(|e| self.morph_error(e))?;
        let flight_rules = rules.flight.as_ref().map_err(|e| self.morph_error(e))?;
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.morph_error("numeric policy missing"))?;
        if numeric.image_sha256() != &rules.image_sha256 {
            return Err(self.morph_error("flight numeric/client identity differs"));
        }
        let target = if target > 0. {
            target
        } else {
            flight_rules.minimum_target
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.morph_error("flight clock missing"))?;
        let from = morph.flight.map_or(
            record
                .flight_height
                .unwrap_or(self.morph_height(&record.definition)?),
            |f| f.sample(now),
        );
        let real = |op, a, b| match numeric
            .binary(op, a, b)
            .map_err(|e| self.morph_error(e.to_string()))?
        {
            Value::Real(v) if v.is_finite() => Ok(v),
            _ => Err(self.morph_error("flight arithmetic is not finite")),
        };
        let duration = if rate == 0. {
            0.
        } else {
            real(BinOp::Div, real(BinOp::Sub, target, from)?.abs(), rate)?
        };
        let finish = now + f64::from(duration);
        if !finish.is_finite() {
            return Err(self.morph_error("flight clock overflow"));
        }
        if !self.morph_active.contains(&unit) {
            self.morph_active
                .try_reserve(1)
                .map_err(|_| self.morph_error("flight index allocation failed"))?;
        }
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let morph = record.morph.as_mut().expect("preflight form");
            morph.height_target = target;
            morph.flight = (duration > 0.).then_some(Flight {
                from,
                to: target,
                start: now,
                finish,
            });
            record.flight_height = Some(if duration == 0. { target } else { from });
        }
        if !self.morph_active.contains(&unit) {
            self.morph_active.push(unit);
            self.morph_active.sort_unstable();
        }
        self.retire_morph_index(unit);
        Ok(Value::Null)
    }
    pub(super) fn begin_synchronous_morph_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        let Some((order, on)) = self.morph_order(name, args) else {
            return Ok(None);
        };
        if self.morph_inactive(args, order, on)? {
            return Ok(Some(ScriptCall::Return(Value::Bool(false))));
        }
        self.prepare_synchronous_frame(name)?;
        self.begin_synchronous_delivery();
        let accepted = self.accept_morph(name, args, order, on);
        let events = self
            .take_synchronous_delivery()
            .ok_or_else(|| self.morph_error("synchronous batch missing"))?;
        let accepted = accepted.map_err(|e| self.inner.error(name, e.failure))?;
        self.begin_synchronous_events_with_result(events, Value::Bool(accepted))
            .map(Some)
    }
    fn morph_inactive(&self, args: &[Value], order: i32, on: bool) -> Result<bool, Error> {
        let unit = match args.first() {
            Some(Value::Null) => return Ok(true),
            Some(Value::Handle(u)) => *u,
            _ => return Err(self.morph_error("unit argument required")),
        };
        let record = match self.inner.entry(unit)? {
            Entry::RemovedUnit { .. } => return Ok(true),
            Entry::Unit { record } => record,
            _ => {
                return Err(self
                    .inner
                    .error("IssueImmediateOrder", Failure::WrongFamily));
            }
        };
        if record.morph.as_ref().is_some_and(|m| {
            (on && m.phase != MorphPhase::Landing) || (!on && m.phase == MorphPhase::Landing)
        }) || (!on && record.morph.is_none())
        {
            return Ok(true);
        }
        let Some(abilities) = &record.abilities else {
            return Ok(false);
        };
        let rules = self
            .admitted_morph_rules()
            .ok_or_else(|| self.morph_error("ability admission missing"))?
            .map_err(|e| self.morph_error(e))?;
        for (alias, _) in abilities.levels().filter(|(_, rank)| *rank > 0) {
            let code = self
                .ability_data_text(alias, "code")
                .map_err(|e| self.morph_error(e))?;
            if rules
                .classes
                .iter()
                .any(|c| c.code.as_slice() == code.as_bytes() && (c.on == order || c.off == order))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn accept_morph(
        &mut self,
        name: &str,
        args: &[Value],
        order: i32,
        on: bool,
    ) -> Result<bool, Error> {
        if let Some(fault) = &self.morph_fault {
            return Err(self.morph_error(fault.clone()));
        }
        let unit = match args.first() {
            Some(Value::Null) => return Ok(false),
            Some(Value::Handle(u)) => *u,
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        match self.inner.entry(unit)? {
            Entry::RemovedUnit { .. } => return Ok(false),
            Entry::Unit { .. } => {}
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        }
        let rules = self
            .admitted_morph_rules()
            .ok_or_else(|| self.morph_error("ability admission missing"))?
            .map_err(|e| self.morph_error(e))?;
        if self
            .order_registry
            .as_ref()
            .is_none_or(|r| r.binding().image_sha256 != rules.image_sha256)
        {
            return Err(self.morph_error("order/client identity differs"));
        }
        let source = self.executed_unit(unit)?.clone();
        let Some(abilities) = &source.abilities else {
            return Err(self.morph_error("owned ability state missing"));
        };
        let mut aliases = Vec::new();
        for (alias, rank) in abilities.levels().filter(|(_, rank)| *rank > 0) {
            let code = self
                .ability_data_text(alias, "code")
                .map_err(|e| self.morph_error(e))?;
            if let Some(class) = rules
                .classes
                .iter()
                .find(|c| c.code.as_slice() == code.as_bytes() && (c.on == order || c.off == order))
            {
                aliases.push((alias, rank, class.clone()));
            }
        }
        let [(ability, rank, class)] = aliases.as_slice() else {
            if aliases.is_empty() {
                return Ok(false);
            }
            return Err(self.morph_error("ambiguous owned form class"));
        };
        if !self.unit_ability_available(unit, *ability)? {
            return Ok(false);
        }
        if source.morph.as_ref().is_some_and(|m| {
            (on && m.phase != MorphPhase::Landing) || (!on && m.phase == MorphPhase::Landing)
        }) {
            return Ok(false);
        }
        if !on && source.morph.is_none() {
            return Ok(false);
        }
        // A delayed change holds the ability's busy counter (`0x46e130`, released by the `0xd01c4`
        // handler), and the order check answers `0xb9` while it is above zero (`0xb4adf0`).
        if source.morph.as_ref().is_some_and(|m| {
            m.pending
                .as_ref()
                .is_some_and(|p| matches!(p.change, Change::Delayed(_)))
        }) {
            return Ok(false);
        }
        if source.morph.as_ref().is_some_and(|m| m.pending.is_some()) {
            return Err(self
                .morph_error("opposite form request during an unfinished phase is not qualified"));
        }
        let spec = crate::unit_morph_rules::Spec::read(class, |key, kind| {
            self.hero_ability_level_cell(*ability, key, *rank, kind)
        })
        .map_err(|e| self.morph_error(e))?;
        if spec.immediate
            && source.definition.rawcode != if on { spec.normal } else { spec.alternate }
        {
            return Err(self.morph_error("current definition does not match source form"));
        }
        // A delayed change takes any other type through the same path without the type change,
        // but a unit created as the alternate form starts in the alternate state (`0xbacd75`).
        if !spec.immediate && on && source.definition.rawcode == spec.alternate {
            return Err(self.morph_error(
                "a unit created as the alternate form starts in the alternate state; not modelled",
            ));
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.morph_error("clock missing"))?;
        if source.paused_since.is_some()
            || !source.shown
            || source.combat.as_ref().is_none_or(|c| c.dead)
            || source
                .vitals
                .is_none_or(|v| v.life <= 0. || v.mana < spec.mana_cost)
        {
            return Ok(false);
        }
        if source.cached
            || source.illusion.is_some()
            || source.summoned
            || source.construction.is_some()
            || source.construction_request.is_some()
            || abilities.hero.is_some()
            || self.mirror_busy(unit)
        {
            return Err(
                self.morph_error("form snapshot requires an ordinary uncached nonhero unit")
            );
        }
        if self.ability_cooldown_duration(unit, *ability, *rank)? != 0. {
            return Err(self.morph_error("nonzero form cooldown is not qualified"));
        }
        if self
            .executed_items()
            .any(|(_, item)| item.held_by.is_some_and(|(owner, _)| owner == unit))
        {
            return Err(self.morph_error("form inventory snapshot is not qualified"));
        }
        let numeric = self
            .numeric
            .as_ref()
            .ok_or_else(|| self.morph_error("numeric policy missing"))?;
        if numeric.image_sha256() != &rules.image_sha256 {
            return Err(self.morph_error("numeric/client identity differs"));
        }
        let mana = source.vitals.expect("checked pools").mana;
        let debit = match numeric
            .binary(BinOp::Sub, mana, spec.mana_cost)
            .map_err(|e| self.morph_error(e.to_string()))?
        {
            Value::Real(v) if v.is_finite() && v >= 0. => v,
            _ => return Err(self.morph_error("invalid mana debit")),
        };
        let (change, height_target) = if spec.immediate {
            let normal = self.prepare_morph_form(unit, spec.normal)?;
            let alternate = self.prepare_morph_form(unit, spec.alternate)?;
            self.validate_morph_forms(&normal.definition, &alternate.definition, &POOL_COLUMNS)?;
            self.project_form_upgrades(
                unit,
                if on {
                    &alternate.definition
                } else {
                    &normal.definition
                },
            )?;
            let normal_height = self.morph_height(&normal.definition)?;
            let alternate_height = self.morph_height(&alternate.definition)?;
            let normal_movement = super::unit_positions::geometry(&normal.definition)
                .map_err(|e| self.morph_error(e))?
                .movement;
            (
                Change::Immediate(Box::new(Immediate {
                    normal,
                    alternate,
                    normal_height,
                    alternate_height,
                    normal_movement,
                })),
                if on { normal_height } else { alternate_height },
            )
        } else {
            let swap = if source.definition.rawcode == if on { spec.normal } else { spec.alternate }
            {
                Some(Box::new(
                    self.prepare_delayed_swap(unit, &source, &spec, on)?,
                ))
            } else {
                None
            };
            (
                Change::Delayed(swap),
                self.morph_height(&source.definition)?,
            )
        };
        let (finish, cast) = if spec.immediate {
            (
                now + f64::from(spec.duration)
                    + if on {
                        0.
                    } else {
                        f64::from(spec.landing_delay)
                    },
                on.then_some(now + f64::from(spec.cast)),
            )
        } else {
            (now + f64::from(spec.duration), None)
        };
        if !finish.is_finite() || cast.is_some_and(|t| !t.is_finite()) {
            return Err(self.morph_error("phase clock overflow"));
        }
        let revision = source
            .definition_revision
            .checked_add(1)
            .ok_or_else(|| self.morph_error("revision exhausted"))?;
        let route = self
            .prepare_steer_unit_in_range(unit, None, 0., name)?
            .ok_or_else(|| self.morph_error("cast cannot interrupt current activity"))?;
        // shortcut: authored Morph uses its own clock; qualify native clip retiming and idle completion before claiming animation parity.
        let mut animation = source.animation.clone();
        if let Some(animation) = &mut animation {
            animation
                .select_engine("morph", now, None)
                .map_err(|e| self.morph_error(e))?;
        }
        let mut batches = Vec::new();
        for suffix in [
            "ISSUED_ORDER",
            "SPELL_CHANNEL",
            "SPELL_CAST",
            "SPELL_EFFECT",
        ] {
            batches.push(self.prepare_ability_events(
                unit,
                &format!("EVENT_UNIT_{suffix}"),
                &format!("EVENT_PLAYER_UNIT_{suffix}"),
            )?);
        }
        // Completion registrations are preflighted too; callbacks may change them later.
        for suffix in ["FINISH", "ENDCAST"] {
            self.morph_events(unit, suffix)?;
        }
        let count = batches
            .iter()
            .try_fold(0usize, |n, b| n.checked_add(b.len()))
            .ok_or_else(|| self.morph_error("event count overflow"))?;
        self.reserve_game_event_delivery(count)?;
        self.reserve_steer_commits(1, name)?;
        if self.morph_active.binary_search(&unit).is_err() {
            self.morph_active
                .try_reserve(1)
                .map_err(|_| self.morph_error("active index capacity"))?;
        }
        self.commit_steer_unit(unit, route, name)?;
        self.unit_ordered(unit);
        self.cancel_inventory_order(unit);
        self.clear_combat_target(unit);
        let pending = Pending {
            stage: EventPhase::Begin,
            order,
            overridden: false,
            finish,
            cast,
            spec: spec.clone(),
            change,
        };
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record.vitals.as_mut().expect("checked pools").mana = debit;
            record.animation = animation;
            record.morph = Some(UnitMorph {
                ability: *ability,
                normal: spec.normal,
                alternate: spec.alternate,
                revision,
                phase: if on {
                    MorphPhase::TakingOff
                } else {
                    MorphPhase::Landing
                },
                height_target,
                flight: None,
                pending: Some(Box::new(pending)),
            });
        }
        if let Err(at) = self.morph_active.binary_search(&unit) {
            self.morph_active.insert(at, unit);
        }
        let context = Context {
            unit,
            revision,
            phase: EventPhase::Begin,
        };
        for (i, events) in batches.into_iter().enumerate() {
            let subject = EventSubject {
                unit: Some(unit),
                issued_order: (i == 0).then_some(order),
                spell_ability: (i != 0).then_some(i32::from_be_bytes(*ability)),
                morph: Some(context),
                ..Default::default()
            };
            self.commit_ability_events(events, subject);
        }
        self.settle_morph_event(Some(context));
        Ok(true)
    }
    fn morph_events(
        &mut self,
        unit: HandleId,
        suffix: &str,
    ) -> Result<Vec<super::combat::AbilityEvent>, Error> {
        self.prepare_ability_events(
            unit,
            &format!("EVENT_UNIT_SPELL_{suffix}"),
            &format!("EVENT_PLAYER_UNIT_SPELL_{suffix}"),
        )
    }
    fn morph_height(&self, definition: &Definition) -> Result<f32, Error> {
        let field = super::unit_positions::field(
            definition,
            SlkId::UnitData,
            "moveHeight",
            &[FieldKind::Real, FieldKind::Unreal],
        )
        .map_err(|e| self.morph_error(e))?;
        let value = match field {
            FieldValue::Cell(value) => value
                .parse::<f32>()
                .map_err(|_| self.morph_error("malformed source movement height"))?,
            // Object-data overrides store this metadata-real field as either type.
            FieldValue::Override(
                pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value),
            ) => *value,
            _ => return Err(self.morph_error("unsupported source movement height value")),
        };
        if !value.is_finite() || value < 0. {
            return Err(self.morph_error("invalid source movement height"));
        }
        Ok(value)
    }
    fn validate_morph_forms(
        &self,
        normal: &Definition,
        alternate: &Definition,
        columns: &[&str],
    ) -> Result<(), Error> {
        // Ordinary source forms preserve pools/food/technology. Other morph economies
        // require their own admitted transition instead of factory side effects.
        for column in columns {
            let a = super::combat::column(normal, SlkId::UnitBalance, column)
                .map_err(|e| self.morph_error(e))?;
            let b = super::combat::column(alternate, SlkId::UnitBalance, column)
                .map_err(|e| self.morph_error(e))?;
            if a.is_none() || b.is_none() || a != b {
                return Err(
                    self.morph_error(format!("form changes unsupported UnitBalance/{column}"))
                );
            }
        }
        if super::unit_types::structure(normal).map_err(|e| self.morph_error(e))?
            || super::unit_types::structure(alternate).map_err(|e| self.morph_error(e))?
        {
            return Err(self.morph_error("structure morph is not qualified"));
        }
        Ok(())
    }
    /// Prepares the delayed change to the form the direction leads to: both forms must be ground
    /// (`0xbaec20` sends an air form to the flight stages), the pools, armor and mana regeneration
    /// must agree because the replacement keeps them, and the unit's ability list must be its
    /// type's own, since the swap replaces it with the target type's list.
    fn prepare_delayed_swap(
        &mut self,
        unit: HandleId,
        source: &super::ExecutedUnit,
        spec: &crate::unit_morph_rules::Spec,
        on: bool,
    ) -> Result<Swap, Error> {
        let to = if on { spec.alternate } else { spec.normal };
        let target = self.resolve_form_definition(to)?;
        for definition in [&source.definition, &target] {
            let movement = super::unit_positions::geometry(definition)
                .map_err(|e| self.morph_error(e))?
                .movement;
            if movement == crate::unit_placement::Movement::Air {
                return Err(self.morph_error(
                    "a form with air movement and no immediate form change runs the flight stages, which are not decoded",
                ));
            }
        }
        self.validate_morph_forms(&source.definition, &target, &SWAP_COLUMNS)?;
        if self.morph_height(&source.definition)? != self.morph_height(&target)? {
            return Err(
                self.morph_error("a form change that moves the unit's height is not decoded")
            );
        }
        let source_abilities = self.prepare_unit_abilities(&source.definition)?;
        let levels = |a: &Option<UnitAbilities>| a.as_ref().map(|a| a.levels().collect::<Vec<_>>());
        if levels(&source_abilities) != levels(&source.abilities) {
            return Err(self.morph_error(
                "form change of a unit whose ability list differs from its type's is not modelled",
            ));
        }
        let form =
            self.prepare_form(unit, target, super::unit_creation::FormChange::Replacement)?;
        let abilities = self.prepare_unit_abilities(&form.definition)?;
        Ok(Swap {
            form,
            abilities,
            source_abilities,
        })
    }
    pub(super) fn morph_current_order(&self, unit: HandleId) -> Option<i32> {
        self.executed_unit(unit).ok()?.morph.as_ref()?.order()
    }
    pub(super) fn settle_morph_event(&mut self, context: Option<Context>) {
        let Some(c) = context else { return };
        if self.morph_event_pending(c) {
            return;
        }
        if let Err(e) = self.settle_morph_phase(c) {
            self.morph_fault = Some(e.to_string());
        }
    }
    fn settle_morph_phase(&mut self, c: Context) -> Result<(), Error> {
        let Some(morph) = self
            .executed_unit(c.unit)
            .ok()
            .and_then(|r| r.morph.clone())
            .filter(|m| m.revision == c.revision)
        else {
            return Ok(());
        };
        let Some(pending) = morph.pending.as_ref().filter(|p| p.stage == c.phase) else {
            return Ok(());
        };
        if self
            .executed_unit(c.unit)?
            .combat
            .as_ref()
            .is_some_and(|v| v.dead)
        {
            self.forget_morph_unit(c.unit);
            return Ok(());
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.morph_error("settlement clock missing"))?;
        match c.phase {
            EventPhase::Begin => {
                let Change::Immediate(change) = &pending.change else {
                    // A delayed change waits for the `0xd01c4` timer: nothing moves at the order.
                    if let Entry::Unit { record } = self.inner.entry_mut(c.unit)? {
                        let m = record.morph.as_mut().expect("same revision");
                        m.pending.as_mut().expect("pending").stage = EventPhase::Running;
                    }
                    self.retire_morph_index(c.unit);
                    return Ok(());
                };
                if morph.phase == MorphPhase::TakingOff {
                    self.commit_morph_form(c.unit, &change.alternate, true)?;
                }
                if morph.phase == MorphPhase::Landing {
                    self.commit_morph_placement(c.unit, &change.normal.definition)?;
                }
                if let Entry::Unit { record } = self.inner.entry_mut(c.unit)? {
                    let record = &mut **record;
                    let from = record.flight_height.unwrap_or(change.alternate_height);
                    if morph.phase == MorphPhase::Landing {
                        let m = record.morph.as_mut().expect("same revision");
                        m.flight = Some(Flight {
                            from,
                            to: change.normal_height,
                            start: now,
                            finish: now + f64::from(pending.spec.altitude_duration),
                        });
                    }
                    record.flight_height = Some(if morph.phase == MorphPhase::TakingOff {
                        change.alternate_height
                    } else {
                        change.normal_height
                    });
                    let m = record.morph.as_mut().expect("same revision");
                    m.height_target = if morph.phase == MorphPhase::TakingOff {
                        change.alternate_height
                    } else {
                        change.normal_height
                    };
                    if let Some(flight) = m.flight {
                        record.flight_height = Some(flight.sample(now));
                    }
                    m.pending.as_mut().expect("pending").stage = EventPhase::Running;
                }
            }
            EventPhase::Finish => {
                if let Entry::Unit { record } = self.inner.entry_mut(c.unit)? {
                    if morph.phase == MorphPhase::Landing {
                        record.morph = None;
                    } else {
                        let m = record.morph.as_mut().expect("same revision");
                        m.phase = MorphPhase::Alternate;
                        if let Some(p) = m.pending.as_mut() {
                            p.stage = EventPhase::Running;
                            p.finish = f64::INFINITY;
                        }
                        if m.pending.as_ref().is_some_and(|p| p.cast.is_none()) {
                            m.pending = None;
                        }
                    }
                }
            }
            EventPhase::Running => {}
        }
        self.retire_morph_index(c.unit);
        Ok(())
    }
    pub(crate) fn next_morph_boundary(&self, until: f64) -> Option<f64> {
        let now = self.timer_game_time()?;
        self.morph_active
            .iter()
            .filter_map(|u| {
                let record = self.executed_unit(*u).ok()?;
                let m = record.morph.as_ref()?;
                if record.paused_since.is_some() && !m.delayed_timer_only() {
                    return None;
                }
                let phase = m
                    .pending
                    .as_ref()
                    .filter(|p| p.stage == EventPhase::Running)
                    .map(|p| p.cast.map_or(p.finish, |t| t.min(p.finish)));
                let flight = m.flight.map(|f| f.finish);
                let due = match (phase, flight) {
                    (Some(a), Some(b)) => a.min(b),
                    (Some(a), None) | (None, Some(a)) => a,
                    (None, None) => return None,
                };
                (due <= until).then_some(due.max(now))
            })
            .min_by(f64::total_cmp)
    }
    pub(super) fn advance_morph(&mut self, now: f64) -> Result<(), Error> {
        if let Some(fault) = &self.morph_fault {
            return Err(self.morph_error(fault.clone()));
        }
        let active = self.morph_active.clone();
        for unit in active {
            if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit) {
                let record = &mut **record;
                if let Some(m) = &mut record.morph
                    && let Some(flight) = m.flight
                {
                    record.flight_height = Some(flight.sample(now));
                    if flight.finish <= now {
                        m.flight = None;
                    }
                }
            }
            let Some(m) = self.executed_unit(unit).ok().and_then(|r| r.morph.clone()) else {
                self.retire_morph_index(unit);
                continue;
            };
            let Some(p) = m
                .pending
                .as_ref()
                .filter(|p| p.stage == EventPhase::Running)
            else {
                self.retire_morph_index(unit);
                continue;
            };
            let record = self.executed_unit(unit)?;
            if record.paused_since.is_some() && !m.delayed_timer_only() {
                return Err(self.morph_error("pause during an active form phase is not qualified"));
            }
            if record.combat.as_ref().is_some_and(|c| c.dead) {
                self.forget_morph_unit(unit);
                continue;
            }
            if let (Some(cast), Change::Immediate(change)) = (p.cast, &p.change)
                && cast <= now
                && let Entry::Unit { record } = self.inner.entry_mut(unit)?
            {
                let from = record.flight_height.unwrap_or(change.alternate_height);
                let state = record.morph.as_mut().expect("active");
                state.height_target = change.alternate_height;
                state.flight = Some(Flight {
                    from,
                    to: change.alternate_height,
                    start: now,
                    finish: now + f64::from(p.spec.altitude_duration),
                });
                state.pending.as_mut().expect("pending").cast = None;
                if state.phase == MorphPhase::Alternate {
                    state.pending = None;
                }
            }
            if p.finish <= now {
                let finish = self.morph_events(unit, "FINISH")?;
                let end = self.morph_events(unit, "ENDCAST")?;
                self.reserve_game_event_delivery(
                    finish
                        .len()
                        .checked_add(end.len())
                        .ok_or_else(|| self.morph_error("completion event overflow"))?,
                )?;
                match &p.change {
                    Change::Immediate(change) => {
                        if m.phase == MorphPhase::Landing {
                            self.commit_morph_form(unit, &change.normal, true)?;
                        }
                    }
                    Change::Delayed(Some(swap)) => self.commit_delayed_swap(unit, swap)?,
                    Change::Delayed(None) => {}
                }
                let c = Context {
                    unit,
                    revision: m.revision,
                    phase: EventPhase::Finish,
                };
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    record
                        .morph
                        .as_mut()
                        .expect("active")
                        .pending
                        .as_mut()
                        .expect("pending")
                        .stage = EventPhase::Finish;
                }
                let subject = EventSubject {
                    unit: Some(unit),
                    spell_ability: Some(i32::from_be_bytes(m.ability)),
                    morph: Some(c),
                    ..Default::default()
                };
                self.commit_ability_events(finish, subject);
                self.commit_ability_events(end, subject);
                self.settle_morph_event(Some(c));
            }
            self.retire_morph_index(unit);
        }
        Ok(())
    }
    /// The `0xd01c4` handler's type change (`0xbaf580` to `0xbaed20`): the unit becomes the target
    /// form with its pools, armor and defense type (equal in both forms, checked at the order).
    /// Regeneration runs to the swap on the old form's rates and continues on the new form's
    /// life rate and type; the type change refreshes both regeneration operations
    /// (`0x680b1a`, `0x680b21`), read here as taking the new type row.
    fn commit_delayed_swap(&mut self, unit: HandleId, swap: &Swap) -> Result<(), Error> {
        let record = self.executed_unit(unit)?;
        let levels = |a: &Option<UnitAbilities>| a.as_ref().map(|a| a.levels().collect::<Vec<_>>());
        if levels(&record.abilities) != levels(&swap.source_abilities) {
            return Err(self.morph_error(
                "the unit's ability list changed during the form change; not modelled",
            ));
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.morph_error("form swap clock missing"))?;
        let next = self
            .prepare_unit_vitals(&swap.form.definition, record.health_handicap)?
            .ok_or_else(|| self.morph_error("form swap requires admitted vitals"))?;
        let mut vitals = match self.project_form_upgrades(unit, &swap.form.definition)? {
            Some(projected) => projected,
            None => {
                let mut vitals = record
                    .vitals
                    .ok_or_else(|| self.morph_error("form swap requires unit vitals"))?;
                let bonuses = self
                    .vitals_bonuses()
                    .ok_or_else(|| self.morph_error("form swap requires admitted vitals"))?;
                let mut work = usize::MAX;
                if let Some((life, mana)) =
                    self.project_regen_unit(unit, &bonuses, now, None, &mut work)?
                {
                    vitals.life = life;
                    vitals.mana = mana;
                    vitals.regen.through = now;
                }
                vitals
            }
        };
        vitals.regen.life_per_second = next.regen.life_per_second;
        vitals.regen.life_type = next.regen.life_type;
        self.commit_replacement_form(unit, swap.form.clone(), swap.abilities.clone())?;
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            record.vitals = Some(vitals);
        }
        Ok(())
    }
    fn retire_morph_index(&mut self, unit: HandleId) {
        if self
            .executed_unit(unit)
            .ok()
            .and_then(|r| r.morph.as_ref())
            .is_none_or(|m| m.pending.is_none() && m.flight.is_none())
        {
            self.morph_active.retain(|u| *u != unit);
        }
    }
    pub(super) fn require_morph_interruption(
        &self,
        unit: HandleId,
        operation: &str,
    ) -> Result<(), Error> {
        if self.executed_unit(unit)?.morph.as_ref().is_some_and(|m| {
            if operation == "PauseUnit" {
                (m.pending.is_some() || m.flight.is_some()) && !m.delayed_timer_only()
            } else if operation == "unit death" {
                m.pending.is_some() || m.flight.is_some()
            } else {
                m.order().is_some()
                    && m.pending
                        .as_ref()
                        .is_none_or(|p| p.stage != EventPhase::Finish)
            }
        }) {
            return Err(self.morph_error(format!(
                "{operation} during an unfinished form phase is not qualified"
            )));
        }
        Ok(())
    }
    pub(super) fn morph_ordered(&mut self, unit: HandleId) {
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(p) = record.morph.as_mut().and_then(|m| m.pending.as_mut())
            && p.stage == EventPhase::Finish
        {
            p.overridden = true;
        }
    }
    pub(super) fn morph_context_live(&self, c: Context) -> bool {
        self.executed_unit(c.unit)
            .ok()
            .and_then(|r| r.morph.as_ref())
            .is_some_and(|m| m.revision == c.revision && m.pending.is_some())
    }
    pub(super) fn forget_morph_unit(&mut self, unit: HandleId) {
        self.morph_active.retain(|u| *u != unit);
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(m) = &mut record.morph
        {
            m.pending = None;
            m.flight = None;
            m.phase = MorphPhase::Alternate;
        }
    }
}

#[cfg(test)]
impl ObjectHost {
    pub(crate) fn test_morph_active_count(&self) -> usize {
        self.morph_active.len()
    }
    pub(crate) fn test_morph_fault(&self) -> Option<&str> {
        self.morph_fault.as_deref()
    }
}
