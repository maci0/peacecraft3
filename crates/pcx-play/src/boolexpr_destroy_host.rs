//! `DestroyBoolExpr`: frees a filter or condition wrapper. Null is the declared native's
//! no-op. A live wrapper becomes `Entry::DeadBoolExpr`, so every later lookup refuses
//! with `Identity`. Destroying a dead wrapper refuses too. Handles are never reused.
//!
//! A wrapper that a stored registration still names is not freed. The arena keeps the
//! registration's handle, and the source API can remove some of them (`TriggerRemoveCondition`,
//! `TriggerClearConditions`). Their host support is not implemented, so removal cannot
//! release a wrapper here. Engine ownership of a retained wrapper is not verified, so
//! destruction refuses instead of guessing. Queued fired events keep the resolved
//! callback, not the handle, so they do not block destruction.
use super::super::{Entry, Error, Failure};
use super::ObjectHost;
use pcx_script::{HandleId, Value};

const NATIVE: &str = "DestroyBoolExpr";

/// Whether a stored arena record names `id` as a boolean expression.
fn names(entry: &Entry, id: HandleId) -> bool {
    match entry {
        Entry::TriggerCondition(r) => r.condition == id,
        Entry::RegisteredPlayerUnitEvent(r) => r.filter == Some(id),
        Entry::RegisteredUnitInRange(r) => r.filter == Some(id),
        Entry::RegisteredRegionEvent(r) => r.filter == Some(id),
        _ => false,
    }
}

impl ObjectHost {
    pub(super) fn destroy_boolexpr(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [value] = args else {
            return Err(self.inner.error(NATIVE, Failure::Arguments));
        };
        let id = match value {
            Value::Null => return Ok(Value::Null),
            Value::Handle(id) => *id,
            _ => return Err(self.inner.error(NATIVE, Failure::Arguments)),
        };
        match self.inner.entry(id)? {
            Entry::Filter { .. } | Entry::Condition { .. } => {}
            Entry::DeadBoolExpr => return Err(self.inner.error(NATIVE, Failure::Identity)),
            _ => return Err(self.inner.error(NATIVE, Failure::WrongFamily)),
        }
        if self.inner.arena.iter().any(|entry| names(entry, id)) {
            return Err(self.inner.error(
                NATIVE,
                Failure::TriggerState(
                    "wrapper is named by a stored registration; destruction is unverified",
                ),
            ));
        }
        *self.inner.entry_mut(id)? = Entry::DeadBoolExpr;
        Ok(Value::Null)
    }
}

#[cfg(test)]
#[path = "boolexpr_destroy_tests.rs"]
mod tests;
