//! Ordinary campaign flee consumer: the pressure valve that starts one
//! accepted retreat after incoming combat damage. It owns only the decision
//! and the bounded set of units whose accepted route is a retreat; movement,
//! route work and arrival stay in the activity host.
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

    /// Evaluate the ordinary flee leaf for one positive incoming hit. Returns
    /// the accepted destination when the unit must retreat. The caller applies
    /// vitals and trace only after this decision, so a refused source leaves
    /// them untouched.
    pub(super) fn campaign_flee_damage_decision(
        &self,
        source: HandleId,
        target: HandleId,
        life_before: Option<f32>,
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
        if !self.campaign_flee_victim_ready(target, victim)? {
            return Ok(None);
        }
        let Some(life) = life_before else {
            return Ok(None);
        };
        if !campaign_flee_low_life(policy, life) {
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
        if attacker.definition.rawcode == *b"Bens" {
            return Err(
                self.campaign_flee_error(source, "incoming attacker is the unsupported Bens unit")
            );
        }
        if attacker.summoned || attacker.admitted_order.is_some_and(|(_, active)| active) {
            return Err(self.campaign_flee_error(
                source,
                "incoming attacker has an unqualified active effect",
            ));
        }
        if campaign_flee_second_weapon(&attacker.definition)
            .map_err(|detail| self.campaign_flee_error(source, &detail))?
        {
            return Err(
                self.campaign_flee_error(source, "incoming attacker enables a second weapon")
            );
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
    pub(super) fn initiate_campaign_flee(
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

fn campaign_flee_second_weapon(definition: &Definition) -> Result<bool, String> {
    let Some(value) = super::combat::column(definition, SlkId::UnitWeapons, "weapsOn")? else {
        return Ok(false);
    };
    let enabled = super::combat::number(value, "weapsOn")?.unwrap_or(0.0) as i32;
    Ok(enabled & 2 != 0)
}
