use super::{Error, Failure, ObjectHost, below};
use pcx_script::{HandleId, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StartPriority {
    pub other_start: i32,
    pub priority: HandleId,
}
/// A declared slot count with explicit unset entries until assigned by the script.
#[derive(Debug)]
pub struct StartPriorities {
    pub start: i32,
    pub slots: Vec<Option<StartPriority>>,
}
const CONTRACTS: &[(&str, &[&str], &str)] = &[
    ("SetStartLocPrioCount", &["integer", "integer"], "nothing"),
    (
        "SetStartLocPrio",
        &["integer", "integer", "integer", "startlocprio"],
        "nothing",
    ),
    ("GetStartLocPrioSlot", &["integer", "integer"], "integer"),
    ("GetStartLocPrio", &["integer", "integer"], "startlocprio"),
];
impl ObjectHost {
    /// Caller limit for total declared priority slots across all defined start locations.
    pub fn with_start_priorities(mut self, max_entries: usize) -> Result<Self, Error> {
        let fail = |p| self.inner.error("start priorities", p);
        let config = self
            .config
            .as_mut()
            .ok_or_else(|| fail(Failure::ConfigState("map config is not admitted")))?;
        if config.priority_limit.is_some() {
            return Err(fail(Failure::ConfigState("priorities already admitted")));
        }
        std::alloc::Layout::array::<Option<StartPriority>>(max_entries)
            .map_err(|_| fail(Failure::Capacity))?;
        for &(name, parameters, result) in CONTRACTS {
            Self::contract(&self.inner, name, parameters, result, false, None)?;
        }
        if !below(&self.inner.program, "startlocprio", "handle") {
            return Err(fail(Failure::Signature));
        }
        config.priority_limit = Some(max_entries);
        Ok(self)
    }
    pub(super) fn priority_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.config.as_ref()?.priority_limit?;
        if !CONTRACTS.iter().any(|c| c.0 == name) {
            return None;
        }
        Some(self.apply_priority(name, args))
    }
    fn apply_priority(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |p| self.inner.error(name, p);
        let config = self
            .config
            .as_mut()
            .ok_or_else(|| fail(Failure::ConfigState("map config is not admitted")))?;
        let limit = config
            .priority_limit
            .ok_or_else(|| fail(Failure::ConfigState("priorities are not admitted")))?;
        let Some(Value::Int(start)) = args.first() else {
            return Err(fail(Failure::ConfigArguments));
        };
        if !config.start_locations.iter().any(|p| p.index == *start) {
            return Err(fail(Failure::ConfigState("start location is undefined")));
        }
        let existing = config
            .start_priorities
            .iter()
            .position(|p| p.start == *start);
        if let ("SetStartLocPrioCount", [Value::Int(_), Value::Int(count)]) = (name, args) {
            let count = usize::try_from(*count)
                .map_err(|_| fail(Failure::ConfigState("negative priority count")))?;
            config
                .start_priorities
                .iter()
                .filter(|row| row.start != *start)
                .try_fold(count, |sum, row| sum.checked_add(row.slots.len()))
                .filter(|&total| total <= limit)
                .ok_or_else(|| fail(Failure::Budget("start priority entries")))?;
            if existing.is_none() {
                config
                    .start_priorities
                    .try_reserve(1)
                    .map_err(|_| fail(Failure::Capacity))?;
            }
            let mut slots = Vec::new();
            slots
                .try_reserve_exact(count)
                .map_err(|_| fail(Failure::Capacity))?;
            // shortcut: resize preserves assigned prefix slots; verify native resize behavior before parity claims.
            if let Some(at) = existing {
                slots.extend(
                    config.start_priorities[at]
                        .slots
                        .iter()
                        .take(count)
                        .copied(),
                );
            }
            slots.resize(count, None);
            if let Some(at) = existing {
                config.start_priorities[at].slots = slots;
            } else {
                config.start_priorities.push(StartPriorities {
                    start: *start,
                    slots,
                });
            }
            return Ok(Value::Null);
        }
        let at = existing.ok_or_else(|| fail(Failure::ConfigState("priority count is unset")))?;
        match (name, args) {
            (
                "SetStartLocPrio",
                [
                    Value::Int(_),
                    Value::Int(slot),
                    Value::Int(other),
                    Value::Handle(priority),
                ],
            ) => {
                if !config.start_locations.iter().any(|p| p.index == *other) {
                    return Err(fail(Failure::ConfigState(
                        "target start location is undefined",
                    )));
                }
                self.inner
                    .symbolic_value(*priority, "startlocprio")
                    .map_err(|e| fail(e.failure))?;
                let slot = usize::try_from(*slot)
                    .ok()
                    .and_then(|slot| config.start_priorities[at].slots.get_mut(slot))
                    .ok_or_else(|| {
                        fail(Failure::ConfigState(
                            "priority slot is outside declared count",
                        ))
                    })?;
                *slot = Some(StartPriority {
                    other_start: *other,
                    priority: *priority,
                });
                Ok(Value::Null)
            }
            ("GetStartLocPrioSlot" | "GetStartLocPrio", [Value::Int(_), Value::Int(slot)]) => {
                let slot = usize::try_from(*slot)
                    .ok()
                    .and_then(|slot| config.start_priorities[at].slots.get(slot))
                    .and_then(|p| *p)
                    .ok_or_else(|| {
                        fail(Failure::ConfigState(
                            "priority slot is unset or outside declared count",
                        ))
                    })?;
                Ok(if name == "GetStartLocPrioSlot" {
                    Value::Int(slot.other_start)
                } else {
                    Value::Handle(slot.priority)
                })
            }
            _ => Err(fail(Failure::ConfigArguments)),
        }
    }
}

#[cfg(test)]
#[path = "start_priorities_tests.rs"]
mod tests;
