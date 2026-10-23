//! Ordinary campaign flee consumer. The recreation trigger is positive incoming
//! combat damage that lands on a live ordinary victim, not the start of a guard
//! swing. The source envelope is a live nonhero ordinary unit with one initial
//! weapon, no active weapon-slot upgrades and no effect history; other sources
//! are refused with their unit context instead of being approximated. The
//! victim must be a shown, mobile, nonhero ordinary unit whose owner enabled
//! `SetUnitsFlee` and whose resulting life is in a source life band. It owns
//! only the decision and the bounded set of units whose accepted route is a
//! retreat; movement, route work and arrival stay in the activity host.
use super::{Error, Failure, ObjectHost};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::HandleId;
use pcx_sim::world::{Definition, FieldValue};
use std::collections::BTreeSet;

const NATIVE: &str = "SetUnitsFlee";

#[derive(Default)]
pub(super) struct State {
    active: BTreeSet<HandleId>,
}

impl ObjectHost {
    pub(crate) fn campaign_flee_motion_owns(&self, unit: HandleId) -> bool {
        self.campaign_flee_motion.active.contains(&unit)
    }

    pub(super) fn forget_campaign_flee(&mut self, unit: HandleId) {
        self.campaign_flee_motion.active.remove(&unit);
    }

    /// Clear retreat ownership as soon as the accepted route is no longer
    /// moving, including for units that combat no longer considers attackers.
    pub(super) fn prune_campaign_flee_motion(&mut self) {
        let units: Vec<_> = self.campaign_flee_motion.active.iter().copied().collect();
        for unit in units {
            let moving = self
                .live(unit)
                .and_then(|record| record.activity.as_ref())
                .is_some_and(|activity| activity.moving());
            if !moving {
                self.campaign_flee_motion.active.remove(&unit);
            }
        }
    }

    /// Evaluate the ordinary flee leaf for one positive incoming hit. Returns
    /// the accepted destination when the unit must retreat. The caller applies
    /// vitals and trace only after this decision, so a refused source leaves
    /// them untouched.
    #[cfg(test)]
    pub(crate) fn test_disarm_unit(&mut self, unit: HandleId) {
        if let Ok(super::Entry::Unit { record }) = self.inner.entry_mut(unit)
            && let Some(combat) = record.combat.as_mut()
        {
            combat.weapon = None;
        }
    }

    pub(crate) fn campaign_flee_damage_decision(
        &self,
        source: HandleId,
        target: HandleId,
        resulting_life: Option<f32>,
    ) -> Result<Option<[f32; 2]>, Error> {
        let Some(policy) = self.campaign_flee.as_ref() else {
            return Ok(None);
        };
        let Some(victim) = self.live(target) else {
            return Ok(None);
        };
        let owner = victim.owner;
        if self.campaign_flee_enabled(owner)? != Some(true) {
            return Ok(None);
        }
        if victim.definition.rawcode[0].is_ascii_uppercase() {
            return Ok(None);
        }
        if self.unit_has_effect_history(target) {
            return Err(
                self.campaign_flee_error(target, "victim has an unqualified active effect history")
            );
        }
        if !self.campaign_flee_victim_ready(target, victim)? {
            return Ok(None);
        }
        let Some(life) = resulting_life else {
            return Ok(None);
        };
        if life <= 0.0 {
            return Ok(None);
        }

        let Some(attacker) = self.live(source) else {
            return Ok(None);
        };
        if !self.hostile_owners(owner, attacker.owner) {
            return Ok(None);
        }
        let Some(victim_position) = victim.position else {
            return Err(self.campaign_flee_error(target, "accepted victim position is missing"));
        };
        let Some(attacker_position) = attacker.position else {
            return Err(self.campaign_flee_error(source, "accepted attacker position is missing"));
        };
        self.campaign_flee_source_is_ordinary(source, attacker)?;
        let Some(weapon) = attacker.combat.as_ref().and_then(|combat| combat.weapon) else {
            return Err(
                self.campaign_flee_error(source, "incoming attacker has no admitted combat weapon")
            );
        };
        if !campaign_flee_low_life(policy, life) {
            return Err(self.campaign_flee_error(
                target,
                "resulting life is above the ordinary cutoff in a score-dependent band",
            ));
        }
        let numeric = self.numeric.as_ref().ok_or_else(|| {
            self.campaign_flee_error(target, "selected numeric policy is not admitted")
        })?;
        policy
            .ordinary_destination(numeric, victim_position, attacker_position, weapon.range)
            .map(Some)
            .map_err(|error| self.campaign_flee_error(target, &error.to_string()))
    }

    fn campaign_flee_victim_ready(
        &self,
        unit: HandleId,
        victim: &super::ExecutedUnit,
    ) -> Result<bool, Error> {
        if victim.is_sleeping() || victim.paused_since.is_some() {
            return Ok(false);
        }
        if victim.summoned || victim.admitted_order.is_some_and(|(_, active)| active) {
            return Ok(false);
        }
        let Some(activity) = victim.activity.as_ref() else {
            return Ok(false);
        };
        if !activity.can_start_motion() || activity.moving() || activity.held() {
            return Ok(false);
        }
        let Some(speed) = activity.speed() else {
            return Ok(false);
        };
        let policy = self.campaign_flee.as_ref().expect("checked campaign flee");
        if speed < policy.minimum_move_speed {
            return Ok(false);
        }
        campaign_flee_can_flee(&victim.definition)
            .map_err(|detail| self.campaign_flee_error(unit, &detail))
    }

    fn campaign_flee_source_is_ordinary(
        &self,
        source: HandleId,
        attacker: &super::ExecutedUnit,
    ) -> Result<(), Error> {
        if attacker.definition.rawcode[0].is_ascii_uppercase() {
            return Err(self.campaign_flee_error(source, "incoming attacker is a hero"));
        }
        if attacker.summoned || attacker.admitted_order.is_some_and(|(_, active)| active) {
            return Err(self
                .campaign_flee_error(source, "incoming attacker has an unqualified active order"));
        }
        if self.unit_has_effect_history(source) {
            return Err(self.campaign_flee_error(
                source,
                "incoming attacker has an unqualified active effect history",
            ));
        }
        let mask = campaign_flee_weapon_mask(&attacker.definition)
            .map_err(|detail| self.campaign_flee_error(source, &detail))?;
        if mask != 1 {
            return Err(self.campaign_flee_error(
                source,
                &format!("incoming attacker weapsOn mask {mask} is not one initial weapon"),
            ));
        }
        self.campaign_flee_source_has_no_active_upgrades(source, attacker)?;
        Ok(())
    }

    fn campaign_flee_source_has_no_active_upgrades(
        &self,
        source: HandleId,
        attacker: &super::ExecutedUnit,
    ) -> Result<(), Error> {
        let codes = crate::harvest_rules::definition_upgrade_codes(&attacker.definition)
            .map_err(|error| self.campaign_flee_error(source, &error.to_string()))?;
        if codes.is_empty() {
            return Ok(());
        }
        let settings = self.player_settings(attacker.owner)?;
        for code in codes {
            let bytes = code.as_bytes();
            let [a, b, c, d] = bytes else {
                return Err(self.campaign_flee_error(
                    source,
                    &format!("upgrade code {code:?} is not a four-character identifier"),
                ));
            };
            let techid = i32::from_be_bytes([*a, *b, *c, *d]);
            if self.campaign_flee_upgrade_is_active(settings.number, techid, settings) {
                return Err(self.campaign_flee_error(
                    source,
                    &format!("incoming attacker has active weapon-slot upgrade {code}"),
                ));
            }
        }
        Ok(())
    }

    fn campaign_flee_error(&self, unit: HandleId, detail: &str) -> Error {
        self.inner.error(
            NATIVE,
            Failure::UnitActivity(format!("campaign flee unit {unit:?}: {detail}")),
        )
    }

    /// Start the accepted retreat. A route that cannot be accepted leaves no
    /// ownership behind; the accepted route itself is the movement owner.
    pub(crate) fn initiate_campaign_flee(
        &mut self,
        unit: HandleId,
        destination: [f32; 2],
    ) -> Result<(), Error> {
        let capacity = self.executed_units().count();
        if self.campaign_flee_motion.active.contains(&unit) {
            return Ok(());
        }
        if self.campaign_flee_motion.active.len() >= capacity {
            return Err(self.campaign_flee_error(unit, "retreat ownership capacity exhausted"));
        }
        if !self.steer_unit_in_range(unit, Some(destination), 0.0, NATIVE)? {
            return Ok(());
        }
        self.campaign_flee_motion.active.insert(unit);
        Ok(())
    }
}

fn campaign_flee_low_life(policy: &crate::campaign_flee_policy::FleePolicy, life: f32) -> bool {
    life < policy.ordinary_life_cutoff
        || policy
            .unconditional_life_bands()
            .iter()
            .any(|band| life >= band.minimum && life < band.maximum)
}

fn campaign_flee_can_flee(definition: &Definition) -> Result<bool, String> {
    let value =
        super::unit_positions::field(definition, SlkId::UnitData, "canFlee", &[FieldKind::Bool])
            .map_err(|detail| format!("UnitData canFlee: {detail}"))?;
    match value {
        FieldValue::Cell(text) if text.trim() == "0" => Ok(false),
        FieldValue::Cell(text) if text.trim() == "1" => Ok(true),
        FieldValue::Override(pcx_objects::Value::Int(0)) => Ok(false),
        FieldValue::Override(pcx_objects::Value::Int(1)) => Ok(true),
        _ => Err("UnitData canFlee is not a source boolean".into()),
    }
}

fn campaign_flee_weapon_mask(definition: &Definition) -> Result<u32, String> {
    let Some(value) = super::combat::column(definition, SlkId::UnitWeapons, "weapsOn")? else {
        return Err("UnitWeapons weapsOn is missing".into());
    };
    match value {
        FieldValue::Cell(text) => {
            let text = text.trim();
            if text.is_empty() || matches!(text, "-" | "_") {
                return Err("UnitWeapons weapsOn is not an admitted weapon-slot mask".into());
            }
            text.parse::<u32>()
                .map_err(|_| format!("UnitWeapons weapsOn {text:?} is not an integer mask"))
        }
        FieldValue::Override(pcx_objects::Value::Int(mask)) => Ok(*mask),
        _ => Err("UnitWeapons weapsOn is not an integer mask".into()),
    }
}
