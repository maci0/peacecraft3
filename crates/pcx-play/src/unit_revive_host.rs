//! Canonical hero resurrection on the same unit handle.
//!
//! `ReviveHero` and `ReviveHeroLoc` share the canonical unit identity: no
//! replacement handle is allocated and creation capacity is untouched. The
//! factory state (definition, owner, abilities, experience, items) is retained;
//! only liveness is restored. This preserves the corpse comment in
//! `unit_host.rs` that resurrection must keep the canonical identity.
//!
//! Source derivation: signatures and same-source admission follow the `CreateUnit`
//! declaration from the caller's selected program. Hero classification reuses the
//! creation rule (admitted hero progress when present, else the authored
//! uppercase-rawcode hero convention). Maximum life/mana are the retained
//! `Vitals` maxima, not recomputed catalog values. Placement reuses the admitted
//! `ShowUnit` occupancy path (`prepare_show_class` + `commit`); a blocked
//! footprint returns `false` without mutation rather than inventing a position.
//! `doEyecandy` is validated but has no admitted effect presentation yet.
//! Decay reset (Alive, no deadline, unsuspended) reuses the source-derived
//! `unit_decay::Phase`; animation reselects the authored `stand` clip when
//! present and otherwise leaves the retained model state unchanged. Pause,
//! global-speed and native revive-effect timing remain recreation choices, not
//! original-runtime parity claims.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

const REVIVE: &str = "ReviveHero";
const REVIVE_LOC: &str = "ReviveHeroLoc";

impl ObjectHost {
    pub(super) fn unit_revive_declarations(&self, source: &str) -> Result<(), Error> {
        for (name, args) in [
            (REVIVE, &["unit", "real", "real", "boolean"][..]),
            (REVIVE_LOC, &["unit", "location", "boolean"][..]),
        ] {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, "boolean", false, Some("unit"))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        Ok(())
    }

    pub(super) fn unit_revive_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, "ReviveHero" | "ReviveHeroLoc")
            || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        Some(self.revive_hero(name, args))
    }

    fn revive_hero(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(name, reason);
        let (unit, point, _eyecandy) = match (name, args) {
            ("ReviveHero", [unit, Value::Real(x), Value::Real(y), Value::Bool(eyecandy)]) => {
                if !x.is_finite() || !y.is_finite() {
                    return Err(fail(Failure::UnitState(
                        "revive requires finite coordinates",
                    )));
                }
                (unit, [*x, *y], *eyecandy)
            }
            ("ReviveHeroLoc", [unit, loc, Value::Bool(eyecandy)]) => {
                let point = match loc {
                    Value::Null => return Ok(Value::Bool(false)),
                    Value::Handle(_) => self.location_point(loc, name)?,
                    _ => return Err(fail(Failure::Signature)),
                };
                (unit, point, *eyecandy)
            }
            _ => return Err(fail(Failure::Signature)),
        };
        let unit = match unit {
            Value::Null => return Ok(Value::Bool(false)),
            Value::Handle(unit) => *unit,
            _ => return Err(fail(Failure::WrongFamily)),
        };
        // Snapshot the validation view before any mutation.
        let (is_hero, dead, has_vitals, face, definition_for_geometry) =
            match self.inner.entry(unit).map_err(|e| fail(e.failure))? {
                Entry::Unit { record } => {
                    let hero = record.abilities.as_ref().is_some_and(|a| a.hero.is_some())
                        || record.definition.rawcode[0].is_ascii_uppercase();
                    let dead = record.combat.as_ref().is_some_and(|combat| combat.dead);
                    (
                        hero,
                        dead,
                        record.vitals.is_some(),
                        record.face,
                        record.definition.clone(),
                    )
                }
                Entry::RemovedUnit { .. } => return Ok(Value::Bool(false)),
                _ => return Err(fail(Failure::WrongFamily)),
            };
        if !is_hero || !dead {
            return Ok(Value::Bool(false));
        }
        if !has_vitals {
            return Err(fail(Failure::UnitState("revive requires vitals")));
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| fail(Failure::UnitState("revive requires the game clock")))?;
        self.check_activity_boundary(now, name)?;
        // Validate placement before mutating: a blocked footprint is a `false`
        // return, while an unreadable definition is an asset error.
        let prepared = if let Some(placement) = self.unit_placement.as_mut() {
            let geometry = super::unit_positions::geometry(&definition_for_geometry)
                .map_err(|reason| fail(Failure::UnitState(reason)))?;
            match placement.prepare_show_class(
                unit,
                point,
                geometry.radius,
                geometry.path,
                face,
                geometry.movement,
            ) {
                Ok(prepared) => Some(prepared),
                Err(error) if error.work_exhausted() => {
                    return Err(self
                        .inner
                        .error(name, Failure::UnitPlacement(error.to_string())));
                }
                // Blocked or missing hidden occupancy: the boolean reports
                // failure without mutating the canonical unit or its footprint.
                Err(_) => return Ok(Value::Bool(false)),
            }
        } else {
            None
        };
        // shortcut: doEyecandy has no admitted effect presentation; validated above.
        let Entry::Unit { record } = self.inner.entry_mut(unit)? else {
            unreachable!("validated unit identity")
        };
        let record = &mut **record;
        if let Some(combat) = record.combat.as_mut() {
            combat.dead = false;
            combat.target = None;
            combat.swing = None;
            combat.chase = None;
            combat.attack_destination = None;
            combat.ground_target = None;
        }
        if let Some(vitals) = record.vitals.as_mut() {
            vitals.life = vitals.max_life;
            vitals.mana = vitals.max_mana;
            vitals.regen.through = now;
        }
        if let Some(decay) = record.decay.as_mut() {
            decay.phase = crate::unit_decay::Phase::Alive;
            decay.deadline = None;
            decay.remaining = 0.0;
            decay.suspended = false;
        }
        record.position = Some(prepared.as_ref().map_or(point, |p| p.position));
        record.shown = true;
        if let Some(animation) = record.animation.as_mut() {
            // `select` leaves state unchanged when `stand` is absent; a missing
            // stand is not invented here.
            animation
                .select("stand", now, None)
                .map_err(|detail| self.inner.error(name, Failure::UnitAnimation(detail)))?;
        }
        if let Some(prepared) = prepared {
            self.unit_placement
                .as_mut()
                .expect("placement")
                .commit(unit, prepared);
        }
        Ok(Value::Bool(true))
    }

    #[allow(dead_code)]
    pub(super) fn revive_unit_for_tests(&mut self, unit: HandleId) -> Result<bool, Error> {
        match self.revive_hero(
            REVIVE,
            &[
                Value::Handle(unit),
                Value::Real(0.0),
                Value::Real(0.0),
                Value::Bool(false),
            ],
        )? {
            Value::Bool(ok) => Ok(ok),
            _ => Ok(false),
        }
    }
}
