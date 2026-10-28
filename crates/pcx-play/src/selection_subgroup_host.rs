//! Subgroups of the local player's selection and the active one, which the portrait, unit
//! panel and command card follow. See [`crate::selection_subgroups`] for the ordering.
use super::ObjectHost;
use crate::selection_subgroups::{Member, Subgroup, cycle, group};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::HandleId;
use pcx_sim::world::FieldValue;

const PRIORITY_COLUMN: &str = "prio";

fn priority(definition: &pcx_sim::world::Definition) -> Result<i32, String> {
    let field = super::unit_positions::field(
        definition,
        SlkId::UnitData,
        PRIORITY_COLUMN,
        &[FieldKind::Int],
    )
    .map_err(|e| format!("UnitData/{PRIORITY_COLUMN}: {e}"))?;
    match field {
        FieldValue::Cell(value) => value.trim().parse().ok(),
        FieldValue::Override(pcx_objects::Value::Int(value)) => i32::try_from(*value).ok(),
        _ => None,
    }
    .ok_or_else(|| format!("UnitData/{PRIORITY_COLUMN} requires a source integer"))
}

impl ObjectHost {
    /// Subgroups of the selection in order. The type priority is read only when the selection
    /// holds more than one type, since a single subgroup needs no ordering.
    pub fn subgroups(&self) -> Result<Vec<Subgroup>, String> {
        let mut members = Vec::with_capacity(self.selection().len());
        for &id in self.selection() {
            let unit = self.executed_unit(id).map_err(|e| e.to_string())?;
            members.push(Member {
                id,
                kind: unit.definition.rawcode,
                hero: unit.abilities.as_ref().is_some_and(|a| a.hero.is_some()),
                priority: 0,
            });
        }
        let mixed = members.iter().any(|m| m.kind != members[0].kind);
        if mixed {
            for member in &mut members {
                let unit = self.executed_unit(member.id).map_err(|e| e.to_string())?;
                member.priority = priority(&unit.definition)?;
            }
        }
        Ok(group(&members))
    }

    /// Index into [`subgroups`](Self::subgroups) of the active subgroup: the one last chosen
    /// while it still exists, otherwise the first. `None` for an empty selection.
    pub fn active_subgroup(&self) -> Result<Option<usize>, String> {
        let groups = self.subgroups()?;
        Ok(self.active_index(&groups))
    }

    fn active_index(&self, groups: &[Subgroup]) -> Option<usize> {
        if groups.is_empty() {
            return None;
        }
        Some(
            self.active_subgroup
                .and_then(|kind| groups.iter().position(|g| g.kind == kind))
                .unwrap_or(0),
        )
    }

    /// The unit the portrait, unit panel and command card show: the first unit of the
    /// active subgroup.
    pub fn active_unit(&self) -> Result<Option<HandleId>, String> {
        let groups = self.subgroups()?;
        Ok(self
            .active_index(&groups)
            .and_then(|i| groups[i].units.first().copied()))
    }

    /// Tab (`forward`) or Shift+Tab. A selection of one subgroup stays as it is.
    pub fn cycle_subgroup(&mut self, forward: bool) -> Result<(), String> {
        let groups = self.subgroups()?;
        let Some(active) = self.active_index(&groups) else {
            return Ok(());
        };
        if groups.len() > 1 {
            self.active_subgroup = Some(groups[cycle(groups.len(), active, forward)].kind);
        }
        Ok(())
    }

    /// Makes the subgroup holding `unit` active. `false` when `unit` is not selected.
    pub fn activate_subgroup_of(&mut self, unit: HandleId) -> Result<bool, String> {
        let groups = self.subgroups()?;
        match groups.iter().find(|g| g.units.contains(&unit)) {
            Some(found) => {
                self.active_subgroup = Some(found.kind);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// The selected units of `unit`'s subgroup, selection order. Empty when `unit` is not selected.
    pub fn subgroup_units_of(&self, unit: HandleId) -> Result<Vec<HandleId>, String> {
        Ok(self
            .subgroups()?
            .into_iter()
            .find(|g| g.units.contains(&unit))
            .map(|g| g.units)
            .unwrap_or_default())
    }
}
