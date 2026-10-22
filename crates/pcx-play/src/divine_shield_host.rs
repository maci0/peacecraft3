//! Divine Shield (`AHds`): a self-cast hero ability for the authored per-rank
//! duration.
//!
//! Per learned rank, `levels`, `Dur`/`HeroDur`, `Cool`, `Cost` and `BuffID`
//! come from `Units\AbilityData.slk`. The order/unorder strings come from the
//! ability profile (`Order`/`Unorder`) and are resolved through the selected
//! order registry. The caster must be a live unit that has learned the ability.
//!
//! Native probe (Human02 Huth rank 3): the "on" order is accepted true at cast,
//! spends 25 mana and sets a 65 cooldown; the "off" order
//! (`undivineshield`) returns **false both when fresh and while active** — so
//! this implementation does not claim a successful cancel (expiry on clock
//! advance remains the only removal path).
//!
//! Invulnerability is modelled (cast marks the unit invulnerable and restores
//! the independently authored `SetUnitInvulnerable` value on expiry) but is
//! **unverified**: the raw `BlzIsUnitInvulnerable` getter read inverted in the
//! probe, so the effect must not be treated as qualified until a damage-control
//! / static-wrapper probe confirms it.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::FieldKind;
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;

pub(super) const ABILITY: [u8; 4] = *b"AHds";
const NATIVE: &str = "IssueImmediateOrder";

pub(super) struct Rules {
    order: i32,
    unorder: i32,
    /// Game time each caster may cast again.
    ready_at: BTreeMap<HandleId, f64>,
    /// Active shields by caster, keyed by game time.
    active: BTreeMap<HandleId, ActiveShield>,
}

struct ActiveShield {
    ends_at: f64,
    /// The `SetUnitInvulnerable` value to restore when the shield ends.
    restore_invulnerable: Option<bool>,
}

struct Spell {
    cost: f32,
    cooldown: f64,
    duration: f64,
}

#[derive(Default)]
pub(super) struct State {
    rules: Option<Rules>,
}

impl ObjectHost {
    fn shield_error(&self, detail: String) -> Error {
        self.inner.error(NATIVE, Failure::HeroSkills(detail))
    }

    /// Needs units, hero skills, the game clock and the selected order registry.
    /// Reads the ability profile's `Order`/`Unorder` and resolves them through
    /// the registry. Admitted once.
    pub(crate) fn admit_divine_shield(&mut self) -> Result<(), Error> {
        if self.divine_shield.rules.is_some()
            || self.units.is_none()
            || !self.has_hero_skills()
            || self.timer_game_time().is_none()
            || self.order_registry.is_none()
        {
            return Err(self.shield_error(
                "divine shield needs units, hero skills, clock and order registry".into(),
            ));
        }
        let resolve = |key: &str| -> Result<i32, Error> {
            let name = self
                .hero_ability_profile(ABILITY, key)
                .map_err(|e| self.shield_error(e))?;
            self.order_registry
                .as_ref()
                .and_then(|r| r.id(&name))
                .filter(|id| *id != -1)
                .ok_or_else(|| self.shield_error(format!("order {name} is not registered")))
        };
        let order = resolve("Order")?;
        let unorder = resolve("Unorder")?;
        self.divine_shield = State {
            rules: Some(Rules {
                order,
                unorder,
                ready_at: BTreeMap::new(),
                active: BTreeMap::new(),
            }),
        };
        Ok(())
    }

    fn shield_rules(&self) -> Option<&Rules> {
        self.divine_shield.rules.as_ref()
    }

    /// The learned rank's values, or `None` for a live unit without the ability.
    fn spell(&self, caster: HandleId) -> Result<Option<Spell>, Error> {
        if self.live(caster).is_none() || !self.unit_ability_available(caster, ABILITY)? {
            return Ok(None);
        }
        let Some(abilities) = self.executed_unit(caster)?.abilities.as_ref() else {
            return Ok(None);
        };
        let rank = abilities.level(ABILITY);
        if rank <= 0 {
            return Ok(None);
        }
        let levels = self
            .hero_ability_number(ABILITY, "levels")
            .map_err(|e| self.shield_error(e))?;
        if rank > levels {
            return Err(self.shield_error(format!("rank {rank} outside {levels} levels")));
        }
        let number = |stem: &str, kind| -> Result<f32, Error> {
            let raw = self
                .hero_ability_level_cell(ABILITY, stem, rank, kind)
                .map_err(|e| self.shield_error(e))?;
            let value = raw
                .parse::<f32>()
                .map_err(|detail| self.shield_error(format!("invalid {stem}{rank}: {detail}")))?;
            if !value.is_finite() || value < 0.0 {
                return Err(self.shield_error(format!("{stem}{rank} must be finite and nonnegative")));
            }
            Ok(value)
        };
        // Hero ability duration: prefer `HeroDur` (the hero-ranked column), with
        // `Dur` as the fallback. Both coincide for `AHds` in the selected data.
        let duration = self
            .hero_ability_level_cell(ABILITY, "HeroDur", rank, FieldKind::Real)
            .or_else(|_| self.hero_ability_level_cell(ABILITY, "Dur", rank, FieldKind::Real))
            .map_err(|e| self.shield_error(e))?
            .parse::<f32>()
            .map_err(|detail| self.shield_error(format!("invalid HeroDur{rank}: {detail}")))?;
        if !duration.is_finite() || duration < 0.0 {
            return Err(self.shield_error(format!("HeroDur{rank} must be finite and nonnegative")));
        }
        Ok(Some(Spell {
            cost: number("Cost", FieldKind::Int)?,
            cooldown: f64::from(self.ability_cooldown_duration(caster, ABILITY, rank)?),
            duration: f64::from(duration),
        }))
    }

    /// Whether an active shield is still running, expiring those past `now` and
    /// restoring the independently authored `SetUnitInvulnerable` value.
    fn shield_active(&mut self, caster: HandleId, now: f64) -> bool {
        let Some(rules) = self.divine_shield.rules.as_mut() else {
            return false;
        };
        let Some(shield) = rules.active.get(&caster) else {
            return false;
        };
        if now < shield.ends_at {
            return true;
        }
        let shield = rules.active.remove(&caster).expect("present");
        if let Ok(Entry::Unit { record }) = self.inner.entry_mut(caster) {
            record.invulnerable = shield.restore_invulnerable;
        }
        false
    }

    /// Expire any shields whose time has passed for `now`.
    pub(super) fn advance_divine_shield(&mut self, now: f64) {
        let ids: Vec<HandleId> = self
            .divine_shield
            .rules
            .as_ref()
            .map(|rules| {
                rules
                    .active
                    .iter()
                    .filter(|(_, shield)| now >= shield.ends_at)
                    .map(|(id, _)| *id)
                    .collect()
            })
            .unwrap_or_default();
        for id in ids {
            self.shield_active(id, now);
        }
    }

    /// Whether the caster is cooling down or lacks enough mana for the spell.
    fn blocked(&self, caster: HandleId, spell: &Spell, now: f64) -> bool {
        let Some(rules) = self.shield_rules() else {
            return true;
        };
        let Some(unit) = self.live(caster) else {
            return true;
        };
        if rules.ready_at.get(&caster).is_some_and(|due| *due > now) {
            return true;
        }
        unit.vitals.is_none_or(|vitals| vitals.mana < spell.cost)
    }

    /// Casts Divine Shield on `caster`. Returns `true` when a shield is applied.
    pub(super) fn cast_divine_shield(&mut self, caster: HandleId) -> Result<bool, Error> {
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.shield_error("clock missing".into()))?;
        self.advance_divine_shield(now);
        if self.shield_active(caster, now) {
            return Ok(true);
        }
        let Some(spell) = self.spell(caster)? else {
            return Ok(false);
        };
        if self.blocked(caster, &spell, now) {
            return Ok(false);
        }
        let restore_invulnerable = match self.inner.entry_mut(caster)? {
            Entry::Unit { record } => {
                let restore = record.invulnerable;
                record.invulnerable = Some(true);
                if let Some(vitals) = &mut record.vitals {
                    vitals.mana -= spell.cost;
                }
                restore
            }
            _ => return Err(self.inner.error(NATIVE, Failure::WrongFamily)),
        };
        if let Some(rules) = &mut self.divine_shield.rules {
            rules.ready_at.insert(caster, now + spell.cooldown);
            rules.active.insert(
                caster,
                ActiveShield {
                    ends_at: now + spell.duration,
                    restore_invulnerable,
                },
            );
        }
        Ok(true)
    }

    /// `undivineshield`: the native probe returns false both for a fresh unit and
    /// for one with an active shield, so this reports false and performs no
    /// cancel side-effect. Ex expiry remains the only removal path.
    pub(super) fn uncast_divine_shield(&mut self, caster: HandleId) -> Result<bool, Error> {
        let Some(now) = self.timer_game_time() else {
            return Ok(false);
        };
        self.advance_divine_shield(now);
        let _ = caster;
        Ok(false)
    }

    /// Forget a caster's shield when the unit dies or is removed.
    pub(super) fn forget_divine_shield(&mut self, caster: HandleId) {
        if let Some(rules) = &mut self.divine_shield.rules {
            rules.active.remove(&caster);
            rules.ready_at.remove(&caster);
        }
    }

    /// Dispatch the native order if it is a Divine Shield cast or cancel.
    pub(super) fn divine_shield_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE {
            return None;
        }
        let [unit, Value::Str(order)] = args else {
            return None;
        };
        let (order_id, order, unorder) = {
            let rules = self.shield_rules()?;
            let order_id = self.order_registry.as_ref().and_then(|r| r.id(order))?;
            (order_id, rules.order, rules.unorder)
        };
        let is_cast = order_id == order;
        let is_cancel = order_id == unorder;
        if !is_cast && !is_cancel {
            return None;
        }
        let id = match unit {
            Value::Null => return Some(Ok(Value::Bool(false))),
            Value::Handle(id) => *id,
            _ => return Some(Err(self.inner.error(NATIVE, Failure::WrongFamily))),
        };
        Some(if is_cast {
            self.cast_divine_shield(id).map(Value::Bool)
        } else {
            self.uncast_divine_shield(id).map(Value::Bool)
        })
    }
}
