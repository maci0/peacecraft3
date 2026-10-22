//! `SetUnitExploded`: retains the executed boolean for a unit. The selected script API
//! declares no getter, and `ExplodeUnitBJ` calls it immediately before `KillUnit`
//! (Blizzard.j:3700-3702). `kill_unit` checks the model for a `death explode` sequence. The
//! selected `nC25` model was checked and has none, so the death shows its ordinary sequence.
//! What remains unimplemented is the native explosion effect and its corpse consumer. A
//! removed unit refuses. A dead unit is accepted: an unverified policy, pinned by a test.
use super::super::{Entry, Error, Failure};
use super::ObjectHost;
use pcx_script::{HandleId, Value};
use std::collections::{BTreeMap, BTreeSet};

const NATIVE: &str = "SetUnitExploded";
/// Unit-catalog native admitted before this one; its declaration anchors the source.
const UNIT_ANCHOR: &str = "CreateUnit";

pub(super) struct UnitExplosions {
    max_units: usize,
    flags: BTreeMap<HandleId, bool>,
    /// Units whose death ran with the flag set but no explosion sequence in their model.
    unrendered: BTreeSet<HandleId>,
}

impl ObjectHost {
    /// Validates `SetUnitExploded` against the selected source. `max_units` bounds the
    /// number of units that carry a flag.
    pub fn with_unit_explosions(mut self, max_units: usize) -> Result<Self, Error> {
        if self.unit_explosions.is_some() {
            return Err(self.inner.error(
                NATIVE,
                Failure::UnitState("unit explosions already admitted"),
            ));
        }
        Self::contract(
            &self.inner,
            NATIVE,
            &["unit", "boolean"],
            "nothing",
            false,
            None,
        )?;
        // The declaration must come from the same selected source as the admitted unit catalog.
        let source = &self
            .inner
            .program
            .natives()
            .get(NATIVE)
            .expect("checked native")
            .source;
        if self
            .inner
            .program
            .natives()
            .get(UNIT_ANCHOR)
            .is_none_or(|n| &n.source != source)
        {
            return Err(self.inner.error(NATIVE, Failure::Source));
        }
        self.unit_explosions = Some(UnitExplosions {
            max_units,
            flags: BTreeMap::new(),
            unrendered: BTreeSet::new(),
        });
        Ok(self)
    }

    /// The last value `SetUnitExploded` stored for `unit`; `None` when never set.
    pub fn unit_exploded(&self, unit: HandleId) -> Option<bool> {
        self.unit_explosions.as_ref()?.flags.get(&unit).copied()
    }

    /// True when the unit died with the flag set and its model had no explosion sequence, so
    /// the death shown is the ordinary sequence and the explosion is not rendered.
    pub fn exploded_death_unrendered(&self, unit: HandleId) -> bool {
        self.unit_explosions
            .as_ref()
            .is_some_and(|admitted| admitted.unrendered.contains(&unit))
    }

    pub(super) fn note_unrendered_explosion(&mut self, unit: HandleId) {
        if let Some(admitted) = self.unit_explosions.as_mut() {
            admitted.unrendered.insert(unit);
        }
    }

    pub(super) fn unit_explosion_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.unit_explosions.is_none() || name != NATIVE {
            return None;
        }
        Some(self.set_unit_exploded(args))
    }

    fn set_unit_exploded(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [unit, Value::Bool(flag)] = args else {
            return Err(self.inner.error(NATIVE, Failure::Arguments));
        };
        let id = self.unit(unit, NATIVE)?;
        if matches!(self.inner.entry(id)?, Entry::RemovedUnit { .. }) {
            return Err(self.inner.error(NATIVE, Failure::Identity));
        }
        let admitted = self
            .unit_explosions
            .as_mut()
            .expect("dispatch requires admission");
        if !admitted.flags.contains_key(&id) && admitted.flags.len() >= admitted.max_units {
            return Err(self
                .inner
                .error(NATIVE, Failure::Budget("unit explosion flags")));
        }
        admitted.flags.insert(id, *flag);
        Ok(Value::Null)
    }
}

#[cfg(test)]
#[path = "unit_explosion_tests.rs"]
mod tests;
