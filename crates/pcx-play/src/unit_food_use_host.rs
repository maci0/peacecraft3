//! `SetUnitUseFood` (`0x4abc70`, `docs/re-notes/unit-use-food.md`): switches whether one unit's
//! food cost counts toward its owner's food used. The original keeps the state as bit `0x200`
//! of the unit's flags and adds or subtracts the unit type's `fused` on the owner's counter
//! only when the bit changes. This host derives food used from the live units (`live_food`), so
//! the state is the set of units whose bit was cleared; every other counted unit has it set.
//! A removed or null unit does nothing. A dead, illusion or under-construction unit refuses:
//! their bit and the death, illusion and construction accounting are outside the live total.
use super::super::{Entry, Error, Failure};
use super::ObjectHost;
use pcx_script::{HandleId, Value};
use std::collections::BTreeSet;

const NATIVE: &str = "SetUnitUseFood";
/// Unit-catalog native admitted before this one; its declaration anchors the source.
const UNIT_ANCHOR: &str = "CreateUnit";

pub(super) struct UnitFoodUse {
    max_units: usize,
    /// Units whose bit `0x200` is clear: their `fused` is not part of the owner's food used.
    disabled: BTreeSet<HandleId>,
}

impl ObjectHost {
    /// Admits `SetUnitUseFood` when the selected source declares it. `max_units` bounds the
    /// number of units whose food use is switched off.
    pub fn with_unit_food_use(mut self, max_units: usize) -> Result<Self, Error> {
        if self.inner.program.natives().get(NATIVE).is_none() {
            return Ok(self);
        }
        if self.unit_food_use.is_some() {
            return Err(self
                .inner
                .error(NATIVE, Failure::UnitState("unit food use already admitted")));
        }
        Self::contract(
            &self.inner,
            NATIVE,
            &["unit", "boolean"],
            "nothing",
            false,
            None,
        )?;
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
        self.unit_food_use = Some(UnitFoodUse {
            max_units,
            disabled: BTreeSet::new(),
        });
        Ok(self)
    }

    pub(super) fn reenroll_structure_upgrade_food(&mut self, unit: HandleId) {
        if let Some(admitted) = &mut self.unit_food_use {
            admitted.disabled.remove(&unit);
        }
    }

    /// True when `SetUnitUseFood(unit, false)` cleared the unit's food bit.
    pub(super) fn unit_food_disabled(&self, unit: HandleId) -> bool {
        self.unit_food_use
            .as_ref()
            .is_some_and(|admitted| admitted.disabled.contains(&unit))
    }

    pub(super) fn unit_food_use_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.unit_food_use.is_none() || name != NATIVE {
            return None;
        }
        Some(self.set_unit_use_food(args))
    }

    fn set_unit_use_food(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [unit, Value::Bool(enable)] = args else {
            return Err(self.inner.error(NATIVE, Failure::Arguments));
        };
        // The handler returns without effect when the handle does not resolve to a unit.
        if matches!(unit, Value::Null) {
            return Ok(Value::Null);
        }
        let id = self.unit(unit, NATIVE)?;
        let record = match self.inner.entry(id)? {
            Entry::RemovedUnit { .. } => return Ok(Value::Null),
            Entry::Unit { record } => record,
            _ => return Err(self.inner.error(NATIVE, Failure::WrongFamily)),
        };
        if record.illusion.is_some()
            || record.construction.is_some()
            || record.combat.as_ref().is_some_and(|c| c.dead)
            || record.vitals.is_some_and(|v| v.life <= 0.0)
        {
            return Err(self.inner.error(
                NATIVE,
                Failure::UnitState(
                    "SetUnitUseFood on a dead, illusion or under-construction unit: its food bit is not decoded",
                ),
            ));
        }
        self.prepare_upgrade_food_toggle(id, *enable)?;
        let admitted = self
            .unit_food_use
            .as_mut()
            .expect("dispatch requires admission");
        if *enable {
            admitted.disabled.remove(&id);
        } else if !admitted.disabled.contains(&id) {
            if admitted.disabled.len() >= admitted.max_units {
                return Err(self
                    .inner
                    .error(NATIVE, Failure::Budget("unit food use flags")));
            }
            admitted.disabled.insert(id);
        }
        Ok(Value::Null)
    }
}
