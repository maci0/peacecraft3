//! Weapon attacks between executed units on the owned game clock.
//!
//! Weapon, armor and missile values come from each unit's resolved catalog
//! fields; damage bonuses, armor factor and ranges from the selected `[Misc]`
//! rules. Recreation choices, all unqualified against the original: reach is
//! weapon range plus both collision radii; idle units acquire the nearest
//! enemy in acquire range; units are enemies unless the attacker's owner has
//! an explicit passive alliance toward the target's owner, and Neutral Passive
//! units are never acquired automatically; dice draw from the shared script
//! RNG; armor uses the factor in the usual reduction curve. Attack-move reuses
//! that acquisition policy during travel and resumes its accepted endpoint after
//! losing a target; native interrupt/resume and event nesting remain unqualified.
use super::{
    Entry, Error, EventSubject, ExecutedUnit, Failure, ObjectHost, VisionCapability, VisionState,
};
use crate::inventory_powerup::AttributeBonus;
use pcx_objects::editor::misc::{ATTACK_TYPES, CombatRules, DEFENSE_TYPES};
use pcx_objects::editor::{FieldSource, SlkId};
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, FieldValue};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Friend,
    Neutral,
    Enemy,
}

pub(super) type AbilityEvent = (
    HandleId,
    Option<pcx_script::value::FunctionId>,
    Option<HandleId>,
    Option<HandleId>,
);

const ORDER: &str = "IssueTargetOrder";
const ORDER_BY_ID: &str = "IssueTargetOrderById";
const KILL: &str = "KillUnit";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weapon {
    pub range: f32,
    pub acquire: f32,
    pub cooldown: f32,
    pub damage_point: f32,
    pub base: i32,
    pub dice: i32,
    pub sides: i32,
    pub attack: usize,
    pub missile_speed: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Combat {
    pub weapon: Option<Weapon>,
    pub armor: f32,
    pub defense: usize,
    pub target: Option<HandleId>,
    pub dead: bool,
    ready_at: f64,
    swing: Option<(HandleId, f64)>,
    chase: Option<[f32; 2]>,
    /// Retained accepted attack-move or patrol endpoint while acquisition interrupts travel.
    pub attack_destination: Option<[f32; 2]>,
}

struct Missile {
    source: HandleId,
    target: HandleId,
    impact_at: f64,
    damage: f32,
}

/// A damage error plus whether the victim's life was already committed, so a
/// due missile list can distinguish a pre-damage refusal from a post-hit failure.
struct DamageFailure {
    error: Box<Error>,
    applied: bool,
}

impl Combat {
    pub(super) fn replace_form_profile(&mut self, source: &Self) {
        self.weapon = source.weapon;
        self.armor = source.armor;
        self.defense = source.defense;
    }

    pub(super) fn resume_after_pause(&mut self, elapsed: f64) {
        self.ready_at += elapsed;
        if let Some((_, impact)) = &mut self.swing {
            *impact += elapsed;
        }
    }
}

pub(super) struct Rules {
    rules: CombatRules,
    attack_order: i32,
    /// From the selected neutral rule, set once the host is assembled.
    neutral_passive: Option<i32>,
    pub(super) neutral_aggressive: Option<i32>,
    passive: i32,
    player_death: i32,
    unit_death: i32,
    death_event: Option<HandleId>,
    unit_attacked: i32,
    player_attacked: i32,
    missiles: Vec<Missile>,
}

/// `None` is an omitted SLK cell, which is empty.
pub(super) fn column<'a>(
    d: &'a Definition,
    slk: SlkId,
    wanted: &str,
) -> Result<Option<&'a FieldValue>, String> {
    let mut found = d.fields.values().filter(|f| {
        f.metadata.as_ref().is_some_and(|m| {
            m.from == SlkId::UnitMetaData
                && matches!(&m.source, FieldSource::Column { slk: s, column } if *s == slk && column == wanted)
        })
    });
    let field = found
        .next()
        .ok_or_else(|| format!("missing unit field {wanted}"))?;
    if found.next().is_some() {
        return Err(format!("ambiguous unit field {wanted}"));
    }
    match &field.value {
        Ok(value) => Ok(Some(value)),
        Err(pcx_sim::world::FieldProblem::Missing) => Ok(None),
        Err(e) => Err(format!("unit field {wanted}: {e:?}")),
    }
}

fn profile<'a>(d: &'a Definition, wanted: &str) -> Result<Option<&'a FieldValue>, String> {
    let mut found = d.fields.values().filter(|f| {
        f.metadata.as_ref().is_some_and(|m| {
            matches!(&m.source, FieldSource::Profile { key, index, .. } if key == wanted && matches!(index, -1 | 0))
        })
    });
    let Some(field) = found.next() else {
        return Ok(None);
    };
    if found.next().is_some() {
        return Err(format!("ambiguous unit profile {wanted}"));
    }
    field
        .value
        .as_ref()
        .map(Some)
        .map_err(|e| format!("unit profile {wanted}: {e:?}"))
}

/// An SLK `-`/empty cell is an empty numeric.
pub(super) fn number(value: &FieldValue, name: &str) -> Result<Option<f32>, String> {
    let parsed = match value {
        FieldValue::Cell(s) if matches!(s.trim(), "-" | "" | "_") => return Ok(None),
        FieldValue::Cell(s) => s.trim().parse::<f32>().ok(),
        FieldValue::Override(pcx_objects::Value::Int(n)) => Some(*n as f32),
        FieldValue::Override(pcx_objects::Value::Real(n) | pcx_objects::Value::Unreal(n)) => {
            Some(*n)
        }
        _ => None,
    };
    parsed
        .filter(|v| v.is_finite())
        .map(Some)
        .ok_or_else(|| format!("unit field {name} is not a finite number"))
}

pub(super) fn text<'a>(value: &'a FieldValue, name: &str) -> Result<&'a str, String> {
    match value {
        FieldValue::Cell(s) => Ok(s.trim()),
        FieldValue::Override(pcx_objects::Value::Str(bytes)) => {
            std::str::from_utf8(bytes).map_err(|_| format!("unit field {name} is not UTF-8"))
        }
        _ => Err(format!("unit field {name} is not text")),
    }
}

fn read_combat(d: &Definition) -> Result<Combat, String> {
    let num = |slk, name| match column(d, slk, name)? {
        Some(v) => number(v, name),
        None => Ok(None),
    };
    let word = |slk, name| {
        column(d, slk, name)?
            .ok_or_else(|| format!("unit field {name} is empty"))
            .and_then(|v| text(v, name))
    };
    let required = |slk, name| num(slk, name)?.ok_or_else(|| format!("unit field {name} is empty"));
    let defense_name = word(SlkId::UnitBalance, "defType")?;
    let defense = DEFENSE_TYPES
        .iter()
        .position(|t| t.eq_ignore_ascii_case(defense_name))
        .ok_or_else(|| format!("unsupported defense type {defense_name}"))?;
    let armor = num(SlkId::UnitBalance, "def")?.unwrap_or(0.0);
    let enabled = num(SlkId::UnitWeapons, "weapsOn")?.unwrap_or(0.0) as i32;
    let weapon = if enabled & 1 == 0 {
        None
    } else {
        let attack_name = word(SlkId::UnitWeapons, "atkType1")?;
        let attack = ATTACK_TYPES
            .iter()
            .position(|t| t.eq_ignore_ascii_case(attack_name))
            .ok_or_else(|| format!("unsupported attack type {attack_name}"))?;
        let kind = word(SlkId::UnitWeapons, "weapTp1")?;
        let missile_speed = match kind {
            "normal" | "instant" => None,
            "missile" | "msplash" | "mbounce" | "mline" | "artillery" | "aline" => Some(
                profile(d, "missilespeed")?
                    .map(|v| number(v, "Missilespeed"))
                    .transpose()?
                    .flatten()
                    .filter(|v| *v > 0.0)
                    .ok_or("missile weapon without a positive Missilespeed")?,
            ),
            other => return Err(format!("unsupported weapon type {other}")),
        };
        Some(Weapon {
            range: required(SlkId::UnitWeapons, "rangeN1")?,
            acquire: required(SlkId::UnitWeapons, "acquire")?,
            cooldown: required(SlkId::UnitWeapons, "cool1")?,
            damage_point: required(SlkId::UnitWeapons, "dmgpt1")?,
            base: required(SlkId::UnitWeapons, "dmgplus1")? as i32,
            dice: required(SlkId::UnitWeapons, "dice1")? as i32,
            sides: required(SlkId::UnitWeapons, "sides1")? as i32,
            attack,
            missile_speed,
        })
    };
    Ok(Combat {
        weapon,
        armor,
        defense,
        target: None,
        dead: false,
        ready_at: f64::NEG_INFINITY,
        swing: None,
        chase: None,
        attack_destination: None,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HeroBonuses {
    pub damage: f32,
    /// Flat attack damage from held passive items, applied once per attack.
    pub held_damage: f32,
    pub armor: f32,
    pub speed: f32,
}

/// Read-only combat numbers for the selected-unit panel.
///
/// Damage dice and hero bonuses are the values combat already applies. The
/// panel formats them; it does not derive a second attack or armor total.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitStatSnapshot {
    pub damage: Option<DamageStat>,
    pub armor: f32,
    pub armor_bonus: f32,
    pub attack: Option<&'static str>,
    pub defense: &'static str,
    pub attributes: Option<HeroAttributeStat>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageStat {
    /// Weapon `base + dice`, clamped at zero. Natural hero damage is separate.
    pub minimum: i32,
    /// Weapon `base + dice * sides`, clamped at zero.
    pub maximum: i32,
    /// Primary-attribute damage from growth and permanent strength, not held items.
    pub natural: f32,
    /// Held attribute damage plus flat held damage. Natural hero damage is not included.
    pub bonus: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeroAttributeStat {
    pub primary: String,
    pub strength: f32,
    pub agility: f32,
    pub intelligence: f32,
    pub strength_bonus: f32,
    pub agility_bonus: f32,
    pub intelligence_bonus: f32,
}

impl ObjectHost {
    pub(super) fn combat_step_eligible(&self, unit: HandleId, record: &ExecutedUnit) -> bool {
        !self.mirror_busy(unit)
            && self.summon_current_order(unit).is_none()
            && record.construction.is_none()
            && record.construction_request.is_none()
            && self.unit_construction_order(unit).ok().flatten().is_none()
            && record.combat.as_ref().is_some_and(|combat| {
                record.paused_since.is_none()
                    && !record.is_sleeping()
                    && !combat.dead
                    && (combat.weapon.is_some() || combat.attack_destination.is_some())
            })
    }

    pub(crate) fn derive_hero_combat(
        definition: &Definition,
        progress: Option<(i32, i32)>,
        bonus: AttributeBonus,
        rules: &CombatRules,
    ) -> Result<HeroBonuses, String> {
        if !definition.rawcode[0].is_ascii_uppercase() {
            return Ok(HeroBonuses {
                damage: 0.0,
                held_damage: bonus.damage as f32,
                armor: 0.0,
                speed: 1.0,
            });
        }
        let attribute = |name: &str| Self::derive_hero_attribute(definition, progress, bonus, name);
        let primary =
            column(definition, SlkId::UnitBalance, "Primary")?.ok_or("hero Primary is empty")?;
        let primary = text(primary, "Primary")?;
        if !matches!(primary, "STR" | "AGI" | "INT") {
            return Err(format!("unsupported hero Primary {primary:?}"));
        }
        let agility = attribute("AGI")?;
        let damage = attribute(primary)? * rules.primary_damage_per_attribute;
        let armor = rules.hero_armor_base + agility * rules.armor_per_agility;
        let speed = 1.0 + agility * rules.attack_speed_per_agility;
        if !damage.is_finite() || !armor.is_finite() || !speed.is_finite() || speed <= 0.0 {
            return Err("invalid derived hero combat bonuses or non-positive attack speed".into());
        }
        Ok(HeroBonuses {
            damage,
            held_damage: bonus.damage as f32,
            armor,
            speed,
        })
    }
}

/// Identity of a unit in a combat trace, read when the event is recorded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatActor {
    pub unit: HandleId,
    pub owner: HandleId,
    /// Current owner's player number; `None` if the owner is not an admitted player.
    pub player: Option<i32>,
    pub rawcode: [u8; 4],
}

/// Diagnostic combat decisions in logical-clock order. Recording reads state only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CombatTraceEvent {
    /// Target chosen by automatic acquisition (no order issued).
    Acquired {
        source: CombatActor,
        target: CombatActor,
        clock: Option<f64>,
    },
    /// Target set by `IssueTargetOrder`, whatever the owners' relation.
    Ordered {
        source: CombatActor,
        target: CombatActor,
        clock: Option<f64>,
    },
    /// Weapon damage applied to a live target; `life_before` is its life before the hit.
    Damage {
        source: CombatActor,
        target: CombatActor,
        damage: f32,
        life_before: Option<f32>,
        clock: Option<f64>,
    },
}

/// Bounded trace; events past `capacity` are counted in `dropped`, not stored.
#[derive(Debug, Clone, PartialEq)]
pub struct CombatTrace {
    capacity: usize,
    events: Vec<CombatTraceEvent>,
    dropped: usize,
}

impl CombatTrace {
    pub fn events(&self) -> &[CombatTraceEvent] {
        &self.events
    }
    pub fn dropped(&self) -> usize {
        self.dropped
    }
}

impl ObjectHost {
    /// Starts recording combat events. Recording never changes combat outcomes.
    pub fn enable_combat_trace(&mut self, capacity: usize) {
        self.combat_trace = Some(CombatTrace {
            capacity,
            events: Vec::new(),
            dropped: 0,
        });
    }

    pub fn combat_trace(&self) -> Option<&CombatTrace> {
        self.combat_trace.as_ref()
    }

    pub(crate) fn combat_actor(&self, unit: HandleId) -> Option<CombatActor> {
        let record = self.executed_unit(unit).ok()?;
        Some(CombatActor {
            unit,
            owner: record.owner,
            player: self.player_settings(record.owner).ok().map(|s| s.number),
            rawcode: record.definition.rawcode,
        })
    }

    fn record_combat(&mut self, event: impl FnOnce(&Self) -> Option<CombatTraceEvent>) {
        if self.combat_trace.is_none() {
            return;
        }
        let Some(event) = event(self) else {
            return;
        };
        let trace = self.combat_trace.as_mut().expect("trace enabled");
        if trace.events.len() < trace.capacity {
            trace.events.push(event);
        } else {
            trace.dropped += 1;
        }
    }
}

pub(super) fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl ObjectHost {
    /// Before any unit exists; needs vitals, activity, placement rules and the RNG.
    /// `attack_order` names the registered target order (`"attack"` in the client).
    pub fn admit_combat(&mut self, rules: CombatRules, attack_order: &str) -> Result<(), Error> {
        let fail = |reason| self.inner.error(ORDER, Failure::UnitState(reason));
        if self.combat.is_some()
            || self.unit_vitals.is_none()
            || self.unit_activity.is_none()
            || self.random.is_none()
            || self.executed_units().next().is_some()
        {
            return Err(fail(
                "combat needs vitals, activity and RNG before units, once",
            ));
        }
        let attack_order = self
            .order_registry
            .as_ref()
            .and_then(|r| r.id(attack_order))
            .ok_or_else(|| fail("attack order is not registered"))?;
        for (name, args, result) in [
            (ORDER, &["unit", "string", "widget"][..], "boolean"),
            (ORDER_BY_ID, &["unit", "integer", "widget"][..], "boolean"),
            (KILL, &["unit"][..], "nothing"),
        ] {
            if self.inner.program.natives().get(name).is_some() {
                Self::contract(&self.inner, name, args, result, false, Some("unit"))?;
            }
        }
        let death_event_source = self
            .inner
            .program
            .natives()
            .get("TriggerRegisterDeathEvent")
            .or_else(|| self.inner.program.natives().get("ConvertWidgetEvent"))
            .map_or_else(String::new, |n| n.source.clone());
        let death_event = self.optional_widget_death_event(&death_event_source)?;
        self.combat = Some(Rules {
            rules,
            attack_order,
            neutral_passive: None,
            neutral_aggressive: None,
            passive: self.source_constant_enumerand("ALLIANCE_PASSIVE", "alliancetype")?,
            player_death: self
                .source_constant_enumerand("EVENT_PLAYER_UNIT_DEATH", "playerunitevent")?,
            unit_death: self.source_constant_enumerand("EVENT_UNIT_DEATH", "unitevent")?,
            death_event,
            unit_attacked: self.source_constant_enumerand("EVENT_UNIT_ATTACKED", "unitevent")?,
            player_attacked: self
                .source_constant_enumerand("EVENT_PLAYER_UNIT_ATTACKED", "playerunitevent")?,
            missiles: Vec::new(),
        });
        Ok(())
    }

    /// Same player, or an effective passive alliance from the caster's owner.
    pub(super) fn is_ally(&self, caster: HandleId, target: HandleId) -> bool {
        let (Ok(caster), Ok(target)) = (self.executed_unit(caster), self.executed_unit(target))
        else {
            return false;
        };
        let (Ok(a), Ok(b)) = (
            self.player_settings(caster.owner),
            self.player_settings(target.owner),
        ) else {
            return false;
        };
        let Some(rules) = self.combat.as_ref() else {
            return false;
        };
        a.identity == b.identity || self.effective_alliance(a, b, rules.passive)
    }

    /// Restores `amount` life, capped at max life.
    pub(super) fn heal_unit(&mut self, target: HandleId, amount: f32) -> Result<(), Error> {
        if let Entry::Unit { record } = self.inner.entry_mut(target)?
            && let Some(vitals) = &mut record.vitals
        {
            vitals.life = (vitals.life + amount).min(vitals.max_life);
        }
        Ok(())
    }

    /// `EVENT_UNIT_SPELL_EFFECT` and `EVENT_PLAYER_UNIT_SPELL_EFFECT` for a caster whose
    /// spell took effect on `target`; `GetSpellTargetUnit` reads the target.
    pub(super) fn queue_spell_effect(
        &mut self,
        caster: HandleId,
        target: Option<HandleId>,
    ) -> Result<(), Error> {
        let subject = EventSubject {
            unit: Some(caster),
            spell_target: target,
            ..EventSubject::default()
        };
        self.queue_ability_event(
            caster,
            "EVENT_UNIT_SPELL_EFFECT",
            "EVENT_PLAYER_UNIT_SPELL_EFFECT",
            subject,
        )
    }

    pub(super) fn prepare_ability_events(
        &mut self,
        caster: HandleId,
        unit_name: &str,
        player_name: &str,
    ) -> Result<Vec<AbilityEvent>, Error> {
        let unit_event = self.source_constant_enumerand(unit_name, "unitevent")?;
        let player_event = self.source_constant_enumerand(player_name, "playerunitevent")?;
        let owner = self.executed_unit(caster)?.owner;
        let mut fired = Vec::new();
        for entry in &self.inner.arena {
            match entry {
                Entry::RegisteredUnitEvent {
                    trigger,
                    unit,
                    unitevent,
                    enumerand,
                } if *unit == caster && *enumerand == unit_event => {
                    fired.push((*trigger, None, Some(*unitevent), None));
                }
                Entry::RegisteredPlayerUnitEvent(r)
                    if r.player == owner && r.enumerand == player_event =>
                {
                    fired.push((r.trigger, r.filter, Some(r.playerunitevent), Some(r.player)));
                }
                _ => {}
            }
        }
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(fired.len())
            .map_err(|_| self.inner.error("ability events", Failure::Capacity))?;
        for (trigger, filter, event, player) in fired {
            prepared.push((trigger, self.resolve_event_filter(filter)?, event, player));
        }
        Ok(prepared)
    }
    pub(super) fn commit_ability_events(
        &mut self,
        events: Vec<AbilityEvent>,
        subject: EventSubject,
    ) {
        for (trigger, filter, event, player) in events {
            self.push_reserved_event(trigger, filter, EventSubject { player, ..subject }, event);
        }
    }
    pub(super) fn queue_ability_event(
        &mut self,
        caster: HandleId,
        unit_name: &str,
        player_name: &str,
        subject: EventSubject,
    ) -> Result<(), Error> {
        let events = self.prepare_ability_events(caster, unit_name, player_name)?;
        self.reserve_event_delivery(events.len())?;
        self.commit_ability_events(events, subject);
        Ok(())
    }

    pub fn set_neutral_passive(&mut self, number: i32) {
        if let Some(rules) = &mut self.combat {
            rules.neutral_passive = Some(number);
        }
    }
    pub(crate) fn set_neutral_aggressive(&mut self, number: i32) {
        if let Some(rules) = &mut self.combat {
            rules.neutral_aggressive = Some(number);
        }
    }
    pub(super) fn creep_owner(&self, owner: HandleId, native: &str) -> Result<bool, Error> {
        let aggressive = self
            .combat
            .as_ref()
            .and_then(|r| r.neutral_aggressive)
            .ok_or_else(|| {
                self.inner.error(
                    native,
                    Failure::UnitState("neutral aggressive owner is not admitted"),
                )
            })?;
        Ok(self.player_settings(owner)?.number == aggressive)
    }

    pub(super) fn prepare_unit_combat(&self, d: &Definition) -> Result<Option<Combat>, Error> {
        let Some(input) = &self.combat else {
            return Ok(None);
        };
        let result = || {
            // A unit being created holds no powerup strength and no held passive items.
            Self::derive_hero_combat(
                d,
                self.hero_initial_level().map(|level| (level, level)),
                AttributeBonus::default(),
                &input.rules,
            )?;
            read_combat(d)
        };
        result()
            .map(Some)
            .map_err(|e| self.inner.error(ORDER, Failure::UnitActivity(e)))
    }

    pub(crate) fn hero_combat_bonuses(&self, unit: HandleId) -> Result<HeroBonuses, Error> {
        let record = self.executed_unit(unit)?;
        if let Some(image) = &record.illusion {
            return Ok(image.combat_bonuses);
        }
        let progress = self.hero_initial_level().zip(
            record
                .abilities
                .as_ref()
                .and_then(|abilities| abilities.hero.as_ref())
                .map(|hero| hero.level),
        );
        let rules = &self
            .combat
            .as_ref()
            .ok_or_else(|| {
                self.inner
                    .error("combat", Failure::UnitState("combat missing"))
            })?
            .rules;
        let bonus = self.unit_hero_bonus(unit)?;
        Self::derive_hero_combat(&record.definition, progress, bonus, rules)
            .map_err(|error| self.inner.error("combat", Failure::UnitActivity(error)))
    }

    pub(crate) fn held_combat_bonus(
        natural: &HeroBonuses,
        full: &HeroBonuses,
        held_armor: i32,
    ) -> (f32, f32, f32) {
        (
            (full.damage - natural.damage) + (full.held_damage - natural.held_damage),
            natural.armor,
            (full.armor - natural.armor) + held_armor as f32,
        )
    }

    /// Combat totals for one executed unit, using the same hero and held-item
    /// readers as damage resolution.
    pub fn unit_stat_snapshot(&self, unit: HandleId) -> Result<UnitStatSnapshot, Error> {
        let full_bonus = self.unit_hero_bonus(unit)?;
        let held_armor = full_bonus.armor;
        let prepared = {
            let record = self.executed_unit(unit)?;
            let combat = record.combat.as_ref().ok_or_else(|| {
                self.inner
                    .error("combat", Failure::UnitState("combat missing"))
            })?;
            let defense = *DEFENSE_TYPES.get(combat.defense).ok_or_else(|| {
                self.inner
                    .error("combat", Failure::UnitState("defense type missing"))
            })?;
            let weapon = combat
                .weapon
                .map(|weapon| (weapon.base, weapon.dice, weapon.sides, weapon.attack));
            let hero = record
                .abilities
                .as_ref()
                .and_then(|abilities| abilities.hero.as_ref())
                .map(|hero| {
                    (
                        record.definition.clone(),
                        record.permanent_strength,
                        hero.level,
                    )
                });
            (combat.armor, defense, weapon, hero)
        };
        let (base_armor, defense, weapon, hero) = prepared;
        let rules = &self
            .combat
            .as_ref()
            .ok_or_else(|| {
                self.inner
                    .error("combat", Failure::UnitState("combat missing"))
            })?
            .rules;
        let (natural, full) = match &hero {
            Some((definition, permanent, level)) => {
                let initial = self.hero_initial_level().ok_or_else(|| {
                    self.inner
                        .error("combat", Failure::UnitState("hero rules not admitted"))
                })?;
                let progress = Some((initial, *level));
                let natural_bonus = AttributeBonus {
                    strength: *permanent,
                    ..AttributeBonus::default()
                };
                let derive = |bonus| {
                    Self::derive_hero_combat(definition, progress, bonus, rules)
                        .map_err(|error| self.inner.error("combat", Failure::UnitActivity(error)))
                };
                (derive(natural_bonus)?, derive(full_bonus)?)
            }
            None => {
                let empty = HeroBonuses {
                    damage: 0.0,
                    held_damage: 0.0,
                    armor: 0.0,
                    speed: 1.0,
                };
                let full = Self::derive_hero_combat(
                    &self.executed_unit(unit)?.definition,
                    None,
                    full_bonus,
                    rules,
                )
                .map_err(|error| self.inner.error("combat", Failure::UnitActivity(error)))?;
                (empty, full)
            }
        };
        let (damage, attack) = match weapon {
            Some((base, dice, sides, attack)) => {
                let dice = dice.max(0);
                let minimum = base.checked_add(dice).ok_or_else(|| {
                    self.inner.error(
                        "combat",
                        Failure::UnitActivity("damage display overflow".into()),
                    )
                })?;
                let span = dice.checked_mul(sides).ok_or_else(|| {
                    self.inner.error(
                        "combat",
                        Failure::UnitActivity("damage display overflow".into()),
                    )
                })?;
                let maximum = base.checked_add(span).ok_or_else(|| {
                    self.inner.error(
                        "combat",
                        Failure::UnitActivity("damage display overflow".into()),
                    )
                })?;
                let (bonus, _, _) = Self::held_combat_bonus(&natural, &full, 0);
                if !natural.damage.is_finite() || !bonus.is_finite() {
                    return Err(self.inner.error(
                        "combat",
                        Failure::UnitActivity("damage bonus is non-finite".into()),
                    ));
                }
                let attack = ATTACK_TYPES.get(attack).copied().ok_or_else(|| {
                    self.inner
                        .error("combat", Failure::UnitState("attack type missing"))
                })?;
                (
                    Some(DamageStat {
                        minimum: minimum.max(0),
                        maximum: maximum.max(0),
                        natural: natural.damage,
                        bonus,
                    }),
                    Some(attack),
                )
            }
            None => (None, None),
        };
        let (_, hero_armor, armor_bonus) = Self::held_combat_bonus(&natural, &full, held_armor);
        let armor = base_armor + hero_armor;
        if !armor.is_finite() || !armor_bonus.is_finite() {
            return Err(self.inner.error(
                "combat",
                Failure::UnitActivity("armor display is non-finite".into()),
            ));
        }
        Ok(UnitStatSnapshot {
            damage,
            armor,
            armor_bonus,
            attack,
            defense,
            attributes: hero
                .map(|(definition, permanent, level)| {
                    self.hero_attribute_stat(unit, &definition, permanent, level)
                })
                .transpose()?,
        })
    }

    fn hero_attribute_stat(
        &self,
        unit: HandleId,
        definition: &pcx_sim::world::Definition,
        permanent: i32,
        level: i32,
    ) -> Result<HeroAttributeStat, Error> {
        let fail = |reason: String| self.inner.error("combat", Failure::UnitActivity(reason));
        let initial = self.hero_initial_level().ok_or_else(|| {
            self.inner
                .error("combat", Failure::UnitState("hero rules not admitted"))
        })?;
        let progress = Some((initial, level));
        let full = self.unit_hero_bonus(unit)?;
        let base_bonus = AttributeBonus {
            strength: permanent,
            ..AttributeBonus::default()
        };
        let split = |name: &str| -> Result<(f32, f32), Error> {
            let base = Self::derive_hero_attribute(definition, progress, base_bonus, name)
                .map_err(fail)?;
            let total =
                Self::derive_hero_attribute(definition, progress, full, name).map_err(fail)?;
            let bonus = total - base;
            if !bonus.is_finite() {
                return Err(fail(format!("hero {name} bonus is non-finite")));
            }
            Ok((base, bonus))
        };
        let primary = column(definition, SlkId::UnitBalance, "Primary")
            .map_err(fail)?
            .ok_or_else(|| fail("hero Primary is empty".into()))?;
        let primary = text(primary, "Primary").map_err(fail)?;
        if !matches!(primary, "STR" | "AGI" | "INT") {
            return Err(fail(format!("unsupported hero Primary {primary:?}")));
        }
        let (strength, strength_bonus) = split("STR")?;
        let (agility, agility_bonus) = split("AGI")?;
        let (intelligence, intelligence_bonus) = split("INT")?;
        Ok(HeroAttributeStat {
            primary: primary.to_string(),
            strength,
            agility,
            intelligence,
            strength_bonus,
            agility_bonus,
            intelligence_bonus,
        })
    }

    pub(super) fn combat_attack_order(&self, unit: HandleId) -> Option<i32> {
        let rules = self.combat.as_ref()?;
        let record = self.executed_unit(unit).ok()?;
        let combat = record.combat.as_ref()?;
        (combat.target.is_some() || combat.attack_destination.is_some())
            .then_some(rules.attack_order)
    }

    pub(super) fn standard_attack_order(&self, unit: HandleId) -> Option<i32> {
        self.live(unit)?.combat.as_ref()?.weapon?;
        Some(self.combat.as_ref()?.attack_order)
    }

    pub(super) fn is_attack_order(&self, order: i32) -> bool {
        self.combat
            .as_ref()
            .is_some_and(|rules| rules.attack_order == order)
    }

    /// Another order replaces the unit's attack target.
    pub(super) fn clear_combat_target(&mut self, unit: HandleId) {
        self.cancel_mirror_cast(unit);
        self.cancel_summon_cast(unit);
        self.forget_holy_cast(unit);
        self.forget_chain_cast(unit);
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(combat) = &mut record.combat
        {
            combat.target = None;
            combat.swing = None;
            combat.chase = None;
            combat.attack_destination = None;
        }
    }

    pub(super) fn live(&self, unit: HandleId) -> Option<&super::ExecutedUnit> {
        match self.inner.entry(unit).ok()? {
            Entry::Unit { record }
                if record.shown && record.combat.as_ref().is_some_and(|c| !c.dead) =>
            {
                Some(record)
            }
            _ => None,
        }
    }

    pub(super) fn weapon_target(&self, unit: HandleId) -> Option<&super::ExecutedUnit> {
        // Effective protection = explicit `SetUnitInvulnerable` OR an active
        // Divine Shield contribution.
        let shielded = self
            .timer_game_time()
            .is_some_and(|now| self.divine_shield_protects(unit, now));
        self.live(unit).filter(|record| {
            record.invulnerable != Some(true) && !shielded && !self.mirror_protects(unit)
        })
    }

    fn hostile(&self, attacker: &super::ExecutedUnit, target: &super::ExecutedUnit) -> bool {
        self.hostile_owners(attacker.owner, target.owner)
    }

    fn acquisition_candidate(
        &self,
        unit: HandleId,
        owner: HandleId,
        position: [f32; 2],
        acquire: f32,
        id: HandleId,
        vision_gate: Option<(bool, bool)>,
    ) -> Result<Option<(HandleId, f32)>, Error> {
        if id == unit {
            return Ok(None);
        }
        let Some(other) = self.weapon_target(id) else {
            return Ok(None);
        };
        let Some(other_position) = other.position else {
            return Ok(None);
        };
        let gap = distance(other_position, position);
        if gap > acquire || !self.hostile_owners(owner, other.owner) {
            return Ok(None);
        }
        if let Some((fog, mask)) = vision_gate
            && self.vision_presented(owner, other_position, fog, mask)?
                != Some(VisionState::Visible)
        {
            return Ok(None);
        }
        Ok(Some((id, gap)))
    }

    pub(super) fn hostile_owners(&self, attacker: HandleId, target: HandleId) -> bool {
        let rules = self.combat.as_ref().expect("combat");
        let (Ok(a), Ok(b)) = (self.player_settings(attacker), self.player_settings(target)) else {
            return false;
        };
        if Some(b.number) == rules.neutral_passive || a.identity == b.identity {
            return false;
        }
        !self.effective_alliance(a, b, rules.passive)
    }

    /// How the local player regards `unit`'s owner, for selection circle colours:
    /// its own units and passive allies are friends, Neutral Passive is neutral,
    /// everyone else an enemy (the same alliance rule as [`Self::is_enemy`]).
    pub fn local_relation(&self, unit: HandleId) -> Option<Relation> {
        let rules = self.combat.as_ref()?;
        let Entry::Unit { record } = self.inner.entry(unit).ok()? else {
            return None;
        };
        let owner = self.player_settings(record.owner).ok()?;
        let local = self.local_player_settings()?;
        Some(if owner.identity == local.identity {
            Relation::Friend
        } else if Some(owner.number) == rules.neutral_passive {
            Relation::Neutral
        } else if self.hostile_owners(local.identity, owner.identity) {
            Relation::Enemy
        } else {
            Relation::Friend
        })
    }

    /// Hold Position acquisition: the nearest hostile unit within attack reach (weapon range
    /// plus both collision radii). Acquire range does not apply, and the unit never moves.
    fn hold_position_target(
        &self,
        unit: HandleId,
        range: f32,
        name: &str,
        units: &[(HandleId, bool)],
        vision_gate: Option<(bool, bool)>,
    ) -> Result<Option<HandleId>, Error> {
        let Some(attacker) = self.live(unit) else {
            return Ok(None);
        };
        let Some(position) = attacker.position else {
            return Ok(None);
        };
        let own_radius = self.unit_collision_radius(unit, name)?;
        let mut nearest: Option<(HandleId, f32)> = None;
        for &(id, _) in units {
            if id == unit {
                continue;
            }
            let Some(other) = self.weapon_target(id) else {
                continue;
            };
            let Some(other_position) = other.position else {
                continue;
            };
            if !self.hostile(attacker, other) {
                continue;
            }
            if let Some((fog, mask)) = vision_gate
                && self.vision_presented(attacker.owner, other_position, fog, mask)?
                    != Some(VisionState::Visible)
            {
                continue;
            }
            let gap = distance(position, other_position);
            if gap > range + own_radius + self.unit_collision_radius(id, name)? {
                continue;
            }
            if nearest.is_none_or(|(_, best)| gap < best) {
                nearest = Some((id, gap));
            }
        }
        Ok(nearest.map(|(id, _)| id))
    }

    /// Whether `attacker`'s owner treats `target` as hostile under the combat rules.
    pub fn is_enemy(&self, attacker: HandleId, target: HandleId) -> bool {
        match (self.live(attacker), self.live(target)) {
            (Some(a), Some(b)) => self.combat.is_some() && self.hostile(a, b),
            _ => false,
        }
    }

    pub(super) fn combat_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let rules = self.combat.as_ref()?;
        if !matches!(name, ORDER | ORDER_BY_ID | KILL) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        let attack_order = rules.attack_order;
        let holy_light_order = self.holy_light_order();
        Some((|| {
            if name == KILL {
                return match args {
                    [Value::Null] => Ok(Value::Null),
                    [Value::Handle(unit)] => {
                        if matches!(self.inner.entry(*unit)?, Entry::Unit { .. }) {
                            self.kill_unit(*unit, None)?;
                        }
                        Ok(Value::Null)
                    }
                    _ => Err(self.inner.error(name, Failure::Signature)),
                };
            }
            let [unit, order, target] = args else {
                return Err(self.inner.error(name, Failure::Signature));
            };
            let order = match (name, order) {
                (ORDER, Value::Str(s)) => self.order_registry.as_ref().and_then(|r| r.id(s)),
                (ORDER_BY_ID, Value::Int(id)) => Some(*id),
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            if order.is_some() && order == holy_light_order {
                let (Value::Handle(unit), Value::Handle(target)) = (unit, target) else {
                    return Ok(Value::Bool(false));
                };
                return Ok(Value::Bool(self.order_holy_light(*unit, *target)?));
            }
            if let Some(id) = order
                && self.chain_order_claimed(id)
            {
                let (Value::Handle(unit), Value::Handle(target)) = (unit, target) else {
                    return Ok(Value::Bool(false));
                };
                return Ok(Value::Bool(self.order_chain_lightning(*unit, *target, id)?));
            }
            if order != Some(attack_order) {
                return Err(self.inner.error(
                    name,
                    Failure::UnitActivity("only the attack target order is supported".into()),
                ));
            }
            let (Value::Handle(unit), Value::Handle(target)) = (unit, target) else {
                return Ok(Value::Bool(false));
            };
            let (unit, target) = (*unit, *target);
            if self.live(unit).is_some_and(|record| {
                record.paused_since.is_some() || record.construction.is_some()
            }) {
                return Ok(Value::Bool(false));
            }
            let can_attack = self
                .live(unit)
                .and_then(|u| u.combat.as_ref()?.weapon)
                .is_some();
            if !can_attack || self.weapon_target(target).is_none() || unit == target {
                return Ok(Value::Bool(false));
            }
            self.interrupt_unit_construction(unit)?;
            self.unit_ordered(unit);
            self.cancel_inventory_order(unit);
            self.clear_combat_target(unit);
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                record.combat.as_mut().expect("live combat").target = Some(target);
                if let Some(activity) = &mut record.activity {
                    activity.release_hold();
                }
            }
            self.record_combat(|h| {
                Some(CombatTraceEvent::Ordered {
                    source: h.combat_actor(unit)?,
                    target: h.combat_actor(target)?,
                    clock: h.timer_game_time(),
                })
            });
            Ok(Value::Bool(true))
        })())
    }

    /// Kill a live unit: stop it, free its occupancy, play death, queue events.
    pub(crate) fn kill_unit(
        &mut self,
        unit: HandleId,
        killer: Option<HandleId>,
    ) -> Result<(), Error> {
        let name = "unit death";
        let Some(record) = self
            .executed_unit(unit)
            .ok()
            .filter(|record| record.combat.as_ref().is_some_and(|combat| !combat.dead))
        else {
            return Ok(());
        };
        self.require_morph_interruption(unit, "unit death")?;
        let mut decay = record.decay.clone();
        if let Some(state) = &mut decay {
            state
                .begin(self.timer_game_time().unwrap_or(0.))
                .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
        }
        let owner = record.owner;
        let experience = self.plan_hero_kill_experience(unit, killer)?;
        self.forget_morph_unit(unit);
        self.forget_mirror_unit(unit);
        let training = self.prepare_training_death(unit)?;
        let now = self.timer_game_time().unwrap_or(0.0);
        self.forget_inventory_unit(unit, false)?;
        self.forget_unit_guard(unit);
        self.steer_unit(unit, None, name)?;
        if let Some(placement) = &mut self.unit_placement {
            placement.hide(unit).map_err(|e| {
                self.inner
                    .error(name, Failure::UnitPlacement(e.to_string()))
            })?;
        }
        if let Some(training) = training {
            self.commit_training_death(training);
        }
        self.forget_hidden_unit_ranges(unit);
        self.forget_unit_activity(unit);
        self.wake_natural_sleep(unit);
        // Candidate explosion sequence: the source name is not verified (see unit_explosion_host).
        let exploded = self.unit_exploded(unit) == Some(true);
        let mut exploded_presented = false;
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let combat = record.combat.as_mut().expect("live combat");
            combat.dead = true;
            record.decay = decay;
            combat.attack_destination = None;
            combat.target = None;
            combat.swing = None;
            if let Some(vitals) = &mut record.vitals {
                vitals.life = 0.0;
            }
            if let Some(animation) = &mut record.animation {
                exploded_presented = exploded && animation.has_sequence("death explode");
                let sequence = if exploded_presented {
                    "death explode"
                } else {
                    "death"
                };
                let selected = if !exploded_presented && let Some(decay) = &record.decay {
                    animation.select_for_duration(sequence, now, decay.death)
                } else {
                    animation.select_engine(sequence, now, None)
                };
                selected.map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
            }
        }
        self.forget_construction_unit(unit)?;
        if exploded && !exploded_presented {
            self.note_unrendered_explosion(unit);
        }
        for (hero, xp) in experience {
            self.assign_hero_experience(hero, xp, false)?;
        }
        let rules = self.combat.as_ref().expect("combat");
        let (player_death, unit_death, death_event) =
            (rules.player_death, rules.unit_death, rules.death_event);
        let subject = EventSubject {
            unit: Some(unit),
            source: killer,
            ..EventSubject::default()
        };
        let mut fired = Vec::new();
        for entry in &self.inner.arena {
            match entry {
                Entry::RegisteredPlayerUnitEvent(r)
                    if r.player == owner && r.enumerand == player_death =>
                {
                    fired.push((r.trigger, r.filter, Some(r.playerunitevent), Some(r.player)));
                }
                Entry::RegisteredUnitEvent {
                    trigger,
                    unit: u,
                    unitevent,
                    enumerand,
                } if *u == unit && *enumerand == unit_death => {
                    fired.push((*trigger, None, Some(*unitevent), None));
                }
                Entry::RegisteredDeathEvent(r) if r.widget == unit => {
                    fired.push((r.trigger, None, death_event, None));
                }
                _ => {}
            }
        }
        for (trigger, filter, event, player) in fired {
            self.queue_event(trigger, filter, EventSubject { player, ..subject }, event)?;
        }
        Ok(())
    }

    /// Lethal or partial life loss from any damage source. Kills name `source`
    /// as killer but a generic source does not start a combat retreat.
    pub(crate) fn damage_unit(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
    ) -> Result<(), Error> {
        self.apply_damage(source, target, damage, false)
    }

    #[cfg(test)]
    pub(crate) fn test_advance_combat(&mut self) -> Result<(), Error> {
        self.advance_combat()
    }

    #[cfg(test)]
    pub(crate) fn test_missile_count(&self) -> usize {
        self.combat.as_ref().expect("combat").missiles.len()
    }

    #[cfg(test)]
    pub(crate) fn test_swing(&self, unit: HandleId) -> Option<(HandleId, f64)> {
        self.executed_unit(unit).ok()?.combat.as_ref()?.swing
    }

    #[cfg(test)]
    pub(crate) fn test_queue_missile(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
        impact_at: f64,
    ) {
        self.combat
            .as_mut()
            .expect("combat")
            .missiles
            .push(Missile {
                source,
                target,
                impact_at,
                damage,
            });
    }

    /// A landed melee hit or missile impact. This is the incoming combat damage
    /// that the ordinary campaign flee leaf observes.
    #[cfg(test)]
    pub(crate) fn combat_damage_unit(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
    ) -> Result<(), Error> {
        self.apply_damage(source, target, damage, true)
    }

    fn apply_damage(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
        combat_hit: bool,
    ) -> Result<(), Error> {
        self.apply_damage_outcome(source, target, damage, combat_hit)
            .map_err(|failure| *failure.error)
    }

    fn apply_damage_outcome(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
        combat_hit: bool,
    ) -> Result<(), DamageFailure> {
        let mut applied = false;
        self.apply_damage_with_retreat(source, target, damage, combat_hit, &mut applied)
            .map_err(|error| DamageFailure {
                error: Box::new(error),
                applied,
            })
    }

    /// Prepare a retreat before any damage mutation. A refusal propagates with
    /// its source context; only a qualified non-eligible hit declines silently.
    fn campaign_flee_retreat(
        &self,
        source: HandleId,
        target: HandleId,
        resulting_life: f32,
    ) -> Result<Option<super::unit_activity::PreparedSteer>, Error> {
        let Some(destination) =
            self.campaign_flee_damage_decision(source, target, Some(resulting_life))?
        else {
            return Ok(None);
        };
        self.prepare_campaign_flee(target, destination)
    }

    fn apply_damage_with_retreat(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
        combat_hit: bool,
        applied: &mut bool,
    ) -> Result<(), Error> {
        if self.weapon_target(target).is_none() {
            return Ok(());
        }
        let damage = self.mirror_damage(source, target, damage)?;
        let record = self.weapon_target(target).expect("validated target");
        let retreat = if combat_hit && damage > 0.0 {
            let life = record.vitals.map(|v| v.life).unwrap_or(0.0) - damage;
            if life > 0.0 {
                self.campaign_flee_retreat(source, target, life)?
            } else {
                None
            }
        } else {
            None
        };
        self.apply_damage_commit(source, target, damage, retreat, applied)
    }

    fn apply_damage_commit(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
        retreat: Option<super::unit_activity::PreparedSteer>,
        applied: &mut bool,
    ) -> Result<(), Error> {
        let Some(record) = self.weapon_target(target) else {
            return Ok(());
        };
        let life_before = record.vitals.map(|v| v.life);
        let life = life_before.unwrap_or(0.0) - damage;
        if damage > 0.0 {
            self.wake_natural_sleep(target);
        }
        self.record_combat(|h| {
            Some(CombatTraceEvent::Damage {
                source: h.combat_actor(source)?,
                target: h.combat_actor(target)?,
                damage,
                life_before,
                clock: h.timer_game_time(),
            })
        });
        if let Entry::Unit { record } = self.inner.entry_mut(target)?
            && let Some(vitals) = &mut record.vitals
        {
            if life < vitals.life
                && let Some(site) = &mut record.construction
            {
                site.note_damage();
            }
            vitals.life = life;
        }
        if damage > 0.0 {
            *applied = true;
        }
        if life <= 0.0 {
            self.kill_unit(target, Some(source))?;
            return Ok(());
        }
        if let Some(prepared) = retreat {
            self.commit_campaign_flee(target, prepared)?;
        }
        Ok(())
    }

    pub(crate) fn roll_damage(
        &mut self,
        source: HandleId,
        weapon: Weapon,
        target: HandleId,
    ) -> Result<f32, Error> {
        let name = "combat damage";
        let (armor, defense) = {
            let combat = self
                .executed_unit(target)?
                .combat
                .as_ref()
                .ok_or_else(|| self.inner.error(name, Failure::UnitState("combat missing")))?;
            (combat.armor, combat.defense)
        };
        // Hero armor derives from agility; held armor points are a separate item bonus.
        let armor =
            armor + self.hero_combat_bonuses(target)?.armor + self.held_armor_bonus(target)? as f32;
        let bonuses = self.hero_combat_bonuses(source)?;
        let primary_damage = bonuses.damage + bonuses.held_damage;
        let mut roll = weapon.base;
        for _ in 0..weapon.dice.max(0) {
            roll += self.draw_random_integer(1, weapon.sides.max(1), name)?;
        }
        let rules = &self.combat.as_ref().expect("combat").rules;
        let factor = rules.armor_factor;
        let armor_scale = if armor >= 0.0 {
            1.0 - armor * factor / (1.0 + armor * factor)
        } else {
            2.0 - (1.0 - factor).powf(-armor)
        };
        Ok((roll as f32 + primary_damage)
            * rules.damage_bonus[weapon.attack][defense]
            * armor_scale)
    }

    /// One combat step at the current game time, in unit handle order.
    #[cfg_attr(
        feature = "profile-cpu",
        tracing::instrument(skip_all, target = "pcx_cpu", name = "simulation.combat")
    )]
    pub(super) fn advance_combat(&mut self) -> Result<(), Error> {
        let name = "combat";
        if self.combat.is_none() {
            return Ok(());
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.inner.error(name, Failure::UnitState("clock missing")))?;
        self.project_assault_orders()?;
        self.prune_campaign_flee_motion();
        let missiles = std::mem::take(&mut self.combat.as_mut().expect("combat").missiles);
        let (due, pending): (Vec<_>, Vec<_>) =
            missiles.into_iter().partition(|m| m.impact_at <= now);
        self.combat.as_mut().expect("combat").missiles = pending;
        let mut due = due.into_iter();
        while let Some(missile) = due.next() {
            if let Err(failure) =
                self.apply_damage_outcome(missile.source, missile.target, missile.damage, true)
            {
                let combat = self.combat.as_mut().expect("combat");
                let mut restore = Vec::with_capacity(
                    due.len() + combat.missiles.len() + usize::from(!failure.applied),
                );
                if !failure.applied {
                    restore.push(missile);
                }
                restore.extend(due);
                restore.append(&mut combat.missiles);
                combat.missiles = restore;
                return Err(*failure.error);
            }
        }
        self.advance_morph(now)?;
        self.advance_mirror_image(now)?;
        self.advance_holy_light(now)?;
        self.advance_chain_lightning(now)?;
        self.advance_unit_expirations(now)?;
        self.advance_summoning(now)?;
        // Callbacks are queued, not executed here; later deaths still require live target checks.
        let units: Vec<_> = self
            .executed_units()
            .map(|(id, unit)| (id, self.combat_step_eligible(id, unit)))
            .collect();
        // A missing ledger refuses only a new acquisition. Missiles, expirations,
        // casts, and units that keep an existing target still advance this tick.
        let mut acquisition_refusal: Option<&'static str> = None;
        for &(unit, attacks) in &units {
            if !attacks {
                continue;
            }
            if self.captain_route_owned(unit) || self.harvest_owns_unit(unit) {
                continue;
            }
            let has_valid_target = self
                .live(unit)
                .and_then(|record| record.combat.as_ref()?.target)
                .is_some_and(|target| self.weapon_target(target).is_some());
            let guard = self.guard_combat_step(unit, now, has_valid_target)?;
            if guard.skip_acquisition {
                if guard.motion_replaced && self.campaign_flee_motion_owns(unit) {
                    self.forget_campaign_flee(unit);
                }
                continue;
            }
            if self.campaign_flee_motion_owns(unit) {
                let moving = self
                    .live(unit)
                    .and_then(|record| record.activity.as_ref())
                    .is_some_and(|activity| activity.moving());
                if moving {
                    continue;
                }
                self.forget_campaign_flee(unit);
            }
            let Some(record) = self.live(unit) else {
                continue;
            };
            let combat = record.combat.as_ref().expect("live combat");
            let Some(weapon) = combat.weapon else {
                if !record.activity.as_ref().is_some_and(|a| a.moving())
                    && let Entry::Unit { record } = self.inner.entry_mut(unit)?
                {
                    record.combat.as_mut().expect("combat").attack_destination = None;
                }
                continue;
            };
            let Some(position) = record.position else {
                continue;
            };
            let idle = !record
                .activity
                .as_ref()
                .is_some_and(|a| a.moving() || a.following());
            let held = record.activity.as_ref().is_some_and(|a| a.held());
            let destination = combat.attack_destination;
            let previous_target = combat.target;
            let owner = record.owner;
            let resume = previous_target.is_some()
                || record
                    .activity
                    .as_ref()
                    .and_then(|a| a.destination())
                    .is_some_and(|p| Some(p) != destination);
            let mut target = previous_target.filter(|t| self.weapon_target(*t).is_some());
            let searching = target.is_none()
                && (idle || destination.is_some())
                && (held || weapon.acquire > 0.0);
            let vision_gate = if searching {
                match self.vision_capability() {
                    VisionCapability::Unadmitted => None,
                    VisionCapability::Unavailable(reason) => {
                        acquisition_refusal = Some(reason);
                        continue;
                    }
                    VisionCapability::Ready { fog, mask } => Some((fog, mask)),
                }
            } else {
                None
            };
            if target.is_none() && (idle || destination.is_some()) {
                #[cfg(feature = "profile-cpu")]
                let _acquisition =
                    tracing::info_span!(target: "pcx_cpu", "combat.acquisition").entered();
                if held {
                    target =
                        self.hold_position_target(unit, weapon.range, name, &units, vision_gate)?;
                } else if weapon.acquire > 0.0 {
                    let mut candidates = Vec::new();
                    for &(id, _) in &units {
                        if let Some(candidate) = self.acquisition_candidate(
                            unit,
                            owner,
                            position,
                            weapon.acquire,
                            id,
                            vision_gate,
                        )? {
                            candidates.push(candidate);
                        }
                    }
                    if let Some(target_heroes) = self.campaign_target_heroes(unit) {
                        target = self
                            .campaign_acquisition(target_heroes, candidates)
                            .map_err(|reason| {
                                self.inner.error(
                                    name,
                                    Failure::UnitActivity(format!(
                                        "campaign targeting for unit {unit:?}: {reason}"
                                    )),
                                )
                            })?;
                    } else {
                        target = candidates
                            .into_iter()
                            .min_by(|a, b| a.1.total_cmp(&b.1))
                            .map(|(id, _)| id);
                    }
                }
            }
            if target != combat.target {
                self.clear_combat_target(unit);
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    let combat = record.combat.as_mut().expect("combat");
                    combat.target = target;
                    combat.attack_destination = destination;
                }
                if let Some(acquired) = target {
                    self.record_combat(|h| {
                        Some(CombatTraceEvent::Acquired {
                            source: h.combat_actor(unit)?,
                            target: h.combat_actor(acquired)?,
                            clock: h.timer_game_time(),
                        })
                    });
                }
            }
            let Some(target) = target else {
                if let Some(destination) = destination {
                    if resume {
                        self.steer_unit(unit, Some(destination), name)?;
                    } else if idle && let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                        record.combat.as_mut().expect("combat").attack_destination = None;
                    }
                } else if previous_target.is_some() && !idle {
                    self.steer_unit(unit, None, name)?;
                }
                continue;
            };
            let target_position = self
                .live(target)
                .and_then(|t| t.position)
                .expect("live target");
            let reach = weapon.range
                + self.unit_collision_radius(unit, name)?
                + self.unit_collision_radius(target, name)?;
            let gap = distance(position, target_position);
            if gap > reach {
                if held {
                    // Hold Position neither chases nor keeps a target that left its reach.
                    self.clear_combat_target(unit);
                    continue;
                }
                let chase = self
                    .executed_unit(unit)?
                    .combat
                    .as_ref()
                    .expect("combat")
                    .chase;
                let stale = chase.is_none_or(|c| distance(c, target_position) > weapon.range * 0.5);
                if stale || idle {
                    self.steer_unit_in_range(unit, Some(target_position), reach, name)?;
                    if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                        record.combat.as_mut().expect("combat").chase = Some(target_position);
                    }
                }
                continue;
            }
            if !idle {
                self.steer_unit(unit, None, name)?;
            }
            let facing = (target_position[1] - position[1])
                .atan2(target_position[0] - position[0])
                .to_degrees()
                .rem_euclid(360.0);
            let (swing, ready_at) = {
                let combat = self.executed_unit(unit)?.combat.as_ref().expect("combat");
                (combat.swing, combat.ready_at)
            };
            match swing {
                None if now >= ready_at => {
                    let speed = self.hero_combat_bonuses(unit)?.speed;
                    if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                        record.face = facing;
                        let combat = record.combat.as_mut().expect("combat");
                        combat.swing = Some((target, now + f64::from(weapon.damage_point / speed)));
                        combat.ready_at = now + f64::from(weapon.cooldown / speed);
                        combat.chase = None;
                        if let Some(animation) = &mut record.animation {
                            animation
                                .select_engine("attack", now, None)
                                .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
                        }
                    }
                    self.guard_attacked(target, now);
                    let rules = self.combat.as_ref().expect("combat");
                    let (attacked, player_attacked) = (rules.unit_attacked, rules.player_attacked);
                    let owner = self.executed_unit(target)?.owner;
                    let fired: Vec<_> = self
                        .inner
                        .arena
                        .iter()
                        .filter_map(|e| match e {
                            Entry::RegisteredUnitEvent {
                                trigger,
                                unit: u,
                                unitevent,
                                enumerand,
                            } if *u == target && *enumerand == attacked => {
                                Some((*trigger, None, *unitevent, None))
                            }
                            Entry::RegisteredPlayerUnitEvent(r)
                                if r.player == owner && r.enumerand == player_attacked =>
                            {
                                Some((r.trigger, r.filter, r.playerunitevent, Some(r.player)))
                            }
                            _ => None,
                        })
                        .collect();
                    let subject = EventSubject {
                        unit: Some(target),
                        source: Some(unit),
                        ..EventSubject::default()
                    };
                    for (trigger, filter, event, player) in fired {
                        self.queue_event(
                            trigger,
                            filter,
                            EventSubject { player, ..subject },
                            Some(event),
                        )?;
                    }
                }
                Some((swung, hit_at)) if now >= hit_at => {
                    if self.weapon_target(swung).is_none() {
                        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                            record.combat.as_mut().expect("combat").swing = None;
                        }
                        continue;
                    }
                    // A refused melee source consumes no RNG and leaves the swing
                    // pending; the missile impact is the incoming hit instead.
                    let preflight = weapon.missile_speed.is_none()
                        && self.campaign_flee_source_preflight(unit, swung)?;
                    let checkpoint = self.script_random_primary_checkpoint();
                    let prepared = (|| -> Result<_, Error> {
                        let damage = self.roll_damage(unit, weapon, swung)?;
                        let retreat = if preflight {
                            let life = self
                                .weapon_target(swung)
                                .map(|victim| victim.vitals.map(|v| v.life).unwrap_or(0.0) - damage)
                                .unwrap_or(0.0);
                            if life > 0.0 {
                                self.campaign_flee_retreat(unit, swung, life)?
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        Ok((damage, retreat))
                    })();
                    let (damage, retreat) = match prepared {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            self.restore_script_random_primary(checkpoint);
                            return Err(error);
                        }
                    };
                    let cleared = (|| -> Result<(), Error> {
                        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                            record.combat.as_mut().expect("combat").swing = None;
                        }
                        Ok(())
                    })();
                    if let Err(error) = cleared {
                        self.restore_script_random_primary(checkpoint);
                        return Err(error);
                    }
                    match weapon.missile_speed {
                        Some(speed) => {
                            let missiles = &mut self.combat.as_mut().expect("combat").missiles;
                            missiles
                                .try_reserve(1)
                                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                            missiles.push(Missile {
                                source: unit,
                                target: swung,
                                impact_at: now + f64::from(gap / speed),
                                damage,
                            });
                        }
                        None => {
                            let mut applied = false;
                            if let Err(error) =
                                self.apply_damage_commit(unit, swung, damage, retreat, &mut applied)
                            {
                                if !applied {
                                    self.restore_script_random_primary(checkpoint);
                                }
                                return Err(error);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        self.refresh_captain_combat();
        if let Some(reason) = acquisition_refusal {
            return Err(self.inner.error(name, Failure::UnitActivity(reason.into())));
        }
        Ok(())
    }
}
