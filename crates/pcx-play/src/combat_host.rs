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
use super::{Entry, Error, EventSubject, Failure, ObjectHost};
use pcx_objects::editor::misc::{ATTACK_TYPES, CombatRules, DEFENSE_TYPES};
use pcx_objects::editor::{FieldSource, SlkId};
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, FieldValue};
use crate::inventory_powerup::AttributeBonus;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Friend,
    Neutral,
    Enemy,
}

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
    /// Retained accepted attack-move endpoint while acquisition interrupts travel.
    pub attack_destination: Option<[f32; 2]>,
}

struct Missile {
    source: HandleId,
    target: HandleId,
    impact_at: f64,
    damage: f32,
}

pub(super) struct Rules {
    rules: CombatRules,
    attack_order: i32,
    /// From the selected neutral rule, set once the host is assembled.
    neutral_passive: Option<i32>,
    passive: i32,
    player_death: i32,
    unit_death: i32,
    unit_attacked: i32,
    player_attacked: i32,
    missiles: Vec<Missile>,
}

/// `None` is an omitted SLK cell, which is empty.
fn column<'a>(
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
fn number(value: &FieldValue, name: &str) -> Result<Option<f32>, String> {
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

fn text<'a>(value: &'a FieldValue, name: &str) -> Result<&'a str, String> {
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
    pub armor: f32,
    pub speed: f32,
}

impl ObjectHost {
    pub(crate) fn derive_hero_combat(
        definition: &Definition,
        progress: Option<(i32, i32)>,
        bonus: AttributeBonus,
        rules: &CombatRules,
    ) -> Result<HeroBonuses, String> {
        if !definition.rawcode[0].is_ascii_uppercase() {
            return Ok(HeroBonuses {
                damage: 0.0,
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
            armor,
            speed,
        })
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
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
        self.combat = Some(Rules {
            rules,
            attack_order,
            neutral_passive: None,
            passive: self.source_constant_enumerand("ALLIANCE_PASSIVE", "alliancetype")?,
            player_death: self
                .source_constant_enumerand("EVENT_PLAYER_UNIT_DEATH", "playerunitevent")?,
            unit_death: self.source_constant_enumerand("EVENT_UNIT_DEATH", "unitevent")?,
            unit_attacked: self.source_constant_enumerand("EVENT_UNIT_ATTACKED", "unitevent")?,
            player_attacked: self
                .source_constant_enumerand("EVENT_PLAYER_UNIT_ATTACKED", "playerunitevent")?,
            missiles: Vec::new(),
        });
        Ok(())
    }

    pub fn set_neutral_passive(&mut self, number: i32) {
        if let Some(rules) = &mut self.combat {
            rules.neutral_passive = Some(number);
        }
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

    pub(super) fn combat_attack_order(&self, unit: HandleId) -> Option<i32> {
        let rules = self.combat.as_ref()?;
        let record = self.executed_unit(unit).ok()?;
        let combat = record.combat.as_ref()?;
        (combat.target.is_some() || combat.attack_destination.is_some())
            .then_some(rules.attack_order)
    }

    pub(super) fn is_attack_order(&self, order: i32) -> bool {
        self.combat
            .as_ref()
            .is_some_and(|rules| rules.attack_order == order)
    }

    /// Another order replaces the unit's attack target.
    pub(super) fn clear_combat_target(&mut self, unit: HandleId) {
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(combat) = &mut record.combat
        {
            combat.target = None;
            combat.swing = None;
            combat.chase = None;
            combat.attack_destination = None;
        }
    }

    fn live(&self, unit: HandleId) -> Option<&super::ExecutedUnit> {
        match self.inner.entry(unit).ok()? {
            Entry::Unit { record }
                if record.shown && record.combat.as_ref().is_some_and(|c| !c.dead) =>
            {
                Some(record)
            }
            _ => None,
        }
    }

    fn weapon_target(&self, unit: HandleId) -> Option<&super::ExecutedUnit> {
        self.live(unit)
            .filter(|record| record.invulnerable != Some(true))
    }

    fn hostile(&self, attacker: &super::ExecutedUnit, target: &super::ExecutedUnit) -> bool {
        self.hostile_owners(attacker.owner, target.owner)
    }

    fn hostile_owners(&self, attacker: HandleId, target: HandleId) -> bool {
        let rules = self.combat.as_ref().expect("combat");
        let (Ok(a), Ok(b)) = (self.player_settings(attacker), self.player_settings(target)) else {
            return false;
        };
        if Some(b.number) == rules.neutral_passive || a.identity == b.identity {
            return false;
        }
        !a.alliances.iter().any(|(other, setting, value)| {
            *other == b.identity
                && *value
                && self
                    .inner
                    .symbolic_value(*setting, "alliancetype")
                    .is_ok_and(|v| v == rules.passive)
        })
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
            let can_attack = self
                .live(unit)
                .and_then(|u| u.combat.as_ref()?.weapon)
                .is_some();
            if !can_attack || self.weapon_target(target).is_none() || unit == target {
                return Ok(Value::Bool(false));
            }
            self.guard_ordered(unit);
            self.cancel_inventory_order(unit);
            self.clear_combat_target(unit);
            if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                record.combat.as_mut().expect("live combat").target = Some(target);
                if let Some(activity) = &mut record.activity {
                    activity.release_hold();
                }
            }
            Ok(Value::Bool(true))
        })())
    }

    /// Kill a live unit: stop it, free its occupancy, play death, queue events.
    pub(super) fn kill_unit(
        &mut self,
        unit: HandleId,
        killer: Option<HandleId>,
    ) -> Result<(), Error> {
        let name = "unit death";
        let Some(record) = self.live(unit) else {
            return Ok(());
        };
        let owner = record.owner;
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
        self.forget_hidden_unit_ranges(unit);
        self.forget_unit_activity(unit);
        // Candidate explosion sequence: the source name is not verified (see unit_explosion_host).
        let exploded = self.unit_exploded(unit) == Some(true);
        let mut exploded_presented = false;
        if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
            let combat = record.combat.as_mut().expect("live combat");
            combat.dead = true;
            combat.attack_destination = None;
            combat.target = None;
            combat.swing = None;
            if let Some(vitals) = &mut record.vitals {
                vitals.life = 0.0;
            }
            if let Some(animation) = &mut record.animation {
                exploded_presented = exploded && animation.has_sequence("death explode");
                let sequence = if exploded_presented { "death explode" } else { "death" };
                animation
                    .select(sequence, now)
                    .map_err(|e| self.inner.error(name, Failure::UnitAnimation(e)))?;
            }
        }
        if exploded && !exploded_presented {
            self.note_unrendered_explosion(unit);
        }
        let rules = self.combat.as_ref().expect("combat");
        let (player_death, unit_death) = (rules.player_death, rules.unit_death);
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
                    fired.push((r.trigger, None, None, None));
                }
                _ => {}
            }
        }
        for (trigger, filter, event, player) in fired {
            self.queue_event(trigger, filter, EventSubject { player, ..subject }, event)?;
        }
        Ok(())
    }

    fn deal_weapon_damage(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
    ) -> Result<(), Error> {
        let Some(record) = self.weapon_target(target) else {
            return Ok(());
        };
        let life = record.vitals.map_or(0.0, |v| v.life) - damage;
        if let Entry::Unit { record } = self.inner.entry_mut(target)?
            && let Some(vitals) = &mut record.vitals
        {
            vitals.life = life;
        }
        if life <= 0.0 {
            self.kill_unit(target, Some(source))?;
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
        let armor = armor
            + self.hero_combat_bonuses(target)?.armor
            + self.held_armor_bonus(target)? as f32;
        let primary_damage = self.hero_combat_bonuses(source)?.damage;
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
    pub(super) fn advance_combat(&mut self) -> Result<(), Error> {
        let name = "combat";
        if self.combat.is_none() {
            return Ok(());
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.inner.error(name, Failure::UnitState("clock missing")))?;
        let missiles = std::mem::take(&mut self.combat.as_mut().expect("combat").missiles);
        let (due, pending): (Vec<_>, Vec<_>) =
            missiles.into_iter().partition(|m| m.impact_at <= now);
        self.combat.as_mut().expect("combat").missiles = pending;
        for missile in due {
            self.deal_weapon_damage(missile.source, missile.target, missile.damage)?;
        }
        let units: Vec<HandleId> = self
            .executed_units()
            .filter(|(_, u)| {
                u.combat.as_ref().is_some_and(|c| {
                    !c.dead && (c.weapon.is_some() || c.attack_destination.is_some())
                })
            })
            .map(|(id, _)| id)
            .collect();
        for unit in units {
            let has_valid_target = self
                .live(unit)
                .and_then(|record| record.combat.as_ref()?.target)
                .is_some_and(|target| self.weapon_target(target).is_some());
            if self.guard_combat_step(unit, now, has_valid_target)? {
                continue;
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
            let idle = !record.activity.as_ref().is_some_and(|a| a.moving());
            let held = record.activity.as_ref().is_some_and(|a| a.held());
            let destination = combat.attack_destination;
            let previous_target = combat.target;
            let resume = previous_target.is_some()
                || record
                    .activity
                    .as_ref()
                    .and_then(|a| a.destination())
                    .is_some_and(|p| Some(p) != destination);
            let mut target = previous_target.filter(|t| self.weapon_target(*t).is_some());
            if target.is_none() && (idle || destination.is_some()) && weapon.acquire > 0.0 {
                target = self
                    .executed_units()
                    .filter(|(id, other)| {
                        *id != unit
                            && self.weapon_target(*id).is_some()
                            && other
                                .position
                                .is_some_and(|p| distance(p, position) <= weapon.acquire)
                            && self.hostile(record, other)
                    })
                    .min_by(|a, b| {
                        let da = distance(a.1.position.expect("filtered"), position);
                        let db = distance(b.1.position.expect("filtered"), position);
                        da.total_cmp(&db)
                    })
                    .map(|(id, _)| id);
            }
            if target != combat.target {
                self.clear_combat_target(unit);
                if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                    let combat = record.combat.as_mut().expect("combat");
                    combat.target = target;
                    combat.attack_destination = destination;
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
                    self.steer_unit(unit, Some(target_position), name)?;
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
                                .select("attack", now)
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
                    if let Entry::Unit { record } = self.inner.entry_mut(unit)? {
                        record.combat.as_mut().expect("combat").swing = None;
                    }
                    if self.weapon_target(swung).is_none() {
                        continue;
                    }
                    let damage = self.roll_damage(unit, weapon, swung)?;
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
                        None => self.deal_weapon_damage(unit, swung, damage)?,
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
