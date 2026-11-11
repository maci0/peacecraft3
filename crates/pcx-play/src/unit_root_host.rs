//! Immediate `root` and `unroot` orders of the selected client's `CAbilityRoot` class.
//!
//! Decoded rule set: `docs/re-notes/unit-root-burrow.md`. A unit with the class ability starts
//! rooted. `unroot` is accepted once and moves it to the uprooted state; `root` as an immediate
//! order is always refused by the class. Uprooted movement is not modelled and stays refused.
use super::UnitAbilities;
use super::combat::Combat;
use super::{Entry, Error, Failure, ObjectHost};
use pcx_objects::editor::FieldKind;
use pcx_script::{HandleId, Value};
use pcx_sim::world::Definition;

impl ObjectHost {
    fn root_error(&self, name: &str, why: impl Into<String>) -> Error {
        self.inner.error(name, Failure::UnitActivity(why.into()))
    }
    pub(super) fn root_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let order = self.immediate_order_id(name, args)?;
        let rules = self.admitted_root_rules()?.ok()?;
        if order != rules.class.root && order != rules.class.unroot {
            return None;
        }
        Some(self.issue_root_order(name, args, order, &rules))
    }
    /// Whether `unit` has left the rooted state through an accepted `unroot`.
    pub(crate) fn unit_uprooted(&self, unit: HandleId) -> bool {
        self.uprooted.contains(&unit)
    }
    /// The root-class ability among `abilities` and its rank; at most one may be owned.
    fn owned_root_ability(
        &self,
        abilities: &UnitAbilities,
        rules: &crate::unit_root_rules::Rules,
    ) -> Result<Option<([u8; 4], i32)>, String> {
        let mut owned = Vec::new();
        for (alias, rank) in abilities.levels().filter(|(_, rank)| *rank > 0) {
            if self.ability_data_text(alias, "code")?.as_bytes() == rules.class.code {
                owned.push((alias, rank));
            }
        }
        match owned.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            _ => Err("ambiguous owned root ability".into()),
        }
    }
    /// A created unit owning the root class starts rooted: the attach routine applies the
    /// rooted attack mask (`DataA`) through `0xb8f470`, replacing the `weapsOn` set.
    pub(super) fn apply_creation_root_mask(
        &self,
        definition: &Definition,
        abilities: Option<&UnitAbilities>,
        combat: &mut Combat,
    ) -> Result<(), String> {
        let (Some(Ok(rules)), Some(abilities)) = (self.admitted_root_rules(), abilities) else {
            return Ok(());
        };
        let Some((ability, rank)) = self.owned_root_ability(abilities, &rules)? else {
            return Ok(());
        };
        let mask = self
            .hero_ability_level_cell(ability, "DataA", rank, FieldKind::Int)?
            .parse::<i32>()
            .map_err(|_| "malformed root DataA".to_string())?;
        combat.weapon = super::combat::weapon_for_ability_mask(definition, mask)?;
        Ok(())
    }
    fn issue_root_order(
        &mut self,
        name: &str,
        args: &[Value],
        order: i32,
        rules: &crate::unit_root_rules::Rules,
    ) -> Result<Value, Error> {
        let unit = match args.first() {
            Some(Value::Null) => return Ok(Value::Bool(false)),
            Some(Value::Handle(u)) => *u,
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        match self.inner.entry(unit)? {
            Entry::RemovedUnit { .. } => return Ok(Value::Bool(false)),
            Entry::Unit { .. } => {}
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        }
        if self
            .order_registry
            .as_ref()
            .is_none_or(|r| r.binding().image_sha256 != rules.image_sha256)
        {
            return Err(self.root_error(name, "order/client identity differs"));
        }
        // The class rejects `root` as an immediate order: it needs a point target.
        if order == rules.class.root {
            return Ok(Value::Bool(false));
        }
        let record = self.executed_unit(unit)?;
        let Some(abilities) = &record.abilities else {
            return Err(self.root_error(name, "owned ability state missing"));
        };
        let Some((ability, rank)) = self
            .owned_root_ability(abilities, rules)
            .map_err(|e| self.root_error(name, e))?
        else {
            return Ok(Value::Bool(false));
        };
        if !self.unit_ability_available(unit, ability)? || self.unit_uprooted(unit) {
            return Ok(Value::Bool(false));
        }
        if record.paused_since.is_some()
            || record.combat.as_ref().is_some_and(|c| c.dead)
            || record.construction.is_some()
            || record.construction_request.is_some()
        {
            return Err(self.root_error(
                name,
                "unroot of a paused, dead or constructing unit takes the deferred order path, which is not decoded",
            ));
        }
        let cell = |key: &str| {
            self.hero_ability_level_cell(ability, key, rank, FieldKind::Int)
                .map_err(|e| self.root_error(name, e))?
                .parse::<i32>()
                .map_err(|_| self.root_error(name, format!("malformed root {key}")))
        };
        if cell("Cost")? != 0 || self.ability_cooldown_duration(unit, ability, rank)? != 0. {
            return Err(
                self.root_error(name, "unroot with a mana cost or cooldown is not qualified")
            );
        }
        // `DataB` is the attack-enable mask applied while uprooted.
        let weapon = super::combat::weapon_for_ability_mask(&record.definition, cell("DataB")?)
            .map_err(|e| self.root_error(name, e))?;
        if !self
            .prepare_ability_events(
                unit,
                "EVENT_UNIT_ISSUED_ORDER",
                "EVENT_PLAYER_UNIT_ISSUED_ORDER",
            )?
            .is_empty()
        {
            return Err(self.root_error(
                name,
                "issued-order event delivery for the root class is not decoded",
            ));
        }
        self.uprooted.insert(unit);
        if let Entry::Unit { record } = self.inner.entry_mut(unit)?
            && let Some(combat) = &mut record.combat
        {
            combat.weapon = weapon;
        }
        Ok(Value::Bool(true))
    }
}
