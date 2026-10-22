//! Script special effects attached to executed units. The renderer resolves and
//! draws the model; this host owns identity, target and lifetime.
use super::{Entry, Error, Failure, ObjectHost, below};
use pcx_script::{HandleId, Value};

const ADD: &str = "AddSpecialEffectTarget";
const DESTROY: &str = "DestroyEffect";

/// One live effect as executed: authored model string and attach point, verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitEffect<'a> {
    pub model: &'a str,
    pub unit: HandleId,
    pub attach_point: &'a str,
}

impl ObjectHost {
    /// Requires the unit catalog; both natives must share its declaration source.
    pub fn with_unit_effects(mut self) -> Result<Self, Error> {
        let source = self
            .units
            .as_ref()
            .and(self.inner.program.natives().get("CreateUnit"))
            .map(|n| n.source.clone())
            .ok_or_else(|| {
                self.inner
                    .error(ADD, Failure::UnitState("unit catalog required"))
            })?;
        if self.effects || !below(&self.inner.program, "effect", "agent") {
            return Err(self.inner.error(ADD, Failure::Signature));
        }
        for (name, args, result) in [
            (ADD, &["string", "widget", "string"][..], "effect"),
            (DESTROY, &["effect"][..], "nothing"),
        ] {
            Self::contract(&self.inner, name, args, result, false, Some("effect"))?;
            if self.inner.program.natives().get(name).map(|n| &n.source) != Some(&source) {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.effects = true;
        Ok(self)
    }

    /// Live effects at the current game time. An effect with `expires_at` is over once the
    /// clock reaches it, so its state is derived from the clock rather than stored.
    pub fn unit_effects(&self) -> impl Iterator<Item = (HandleId, UnitEffect<'_>)> {
        let now = self.timer_game_time();
        self.inner
            .arena
            .iter()
            .enumerate()
            .filter_map(move |(i, entry)| match entry {
                Entry::Effect {
                    model,
                    unit,
                    attach_point,
                    live: true,
                    expires_at,
                } if !expires_at.zip(now).is_some_and(|(end, now)| now >= end) => Some((
                    self.inner.id(i, ADD).expect("allocated identity"),
                    UnitEffect {
                        model,
                        unit: *unit,
                        attach_point,
                    },
                )),
                _ => None,
            })
    }

    /// Any effect identity ever attached to `unit`, including destroyed and expired
    /// entries, which remain in the arena.
    pub(super) fn unit_has_effect_history(&self, unit: HandleId) -> bool {
        self.inner.arena.iter().any(|entry| {
            matches!(
                entry,
                Entry::Effect {
                    unit: attached, ..
                } if *attached == unit
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn test_record_effect_history(
        &mut self,
        unit: HandleId,
        live: bool,
        expires_at: Option<f64>,
    ) {
        self.inner
            .allocate(
                Entry::Effect {
                    model: "synthetic-history".into(),
                    unit,
                    attach_point: "origin".into(),
                    live,
                    expires_at,
                },
                ADD,
            )
            .unwrap();
    }

    /// Attaches a live effect to an executed unit. Scripts and spell art share this path.
    pub(super) fn add_unit_effect(
        &mut self,
        model: &str,
        unit: HandleId,
        attach_point: &str,
        expires_at: Option<f64>,
    ) -> Result<HandleId, Error> {
        self.executed_unit(unit)
            .map_err(|e| self.inner.error(ADD, e.failure))?;
        let entry = Entry::Effect {
            model: model.into(),
            unit,
            attach_point: attach_point.into(),
            live: true,
            expires_at,
        };
        self.inner.allocate(entry, ADD)
    }

    pub(super) fn effect_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !self.effects || !matches!(name, ADD | DESTROY) {
            return None;
        }
        Some((|| match (name, args) {
            (
                ADD,
                [
                    Value::Str(model),
                    Value::Handle(unit),
                    Value::Str(attach_point),
                ],
            ) => self
                .add_unit_effect(model, *unit, attach_point, None)
                .map(Value::Handle),
            (ADD, [Value::Str(_), Value::Null, Value::Str(_)]) => Err(self.inner.error(
                name,
                Failure::UnitState("null target behavior is not established"),
            )),
            // Human01 destroys the same effect variable twice; null and destroyed
            // effects are accepted without change.
            (DESTROY, [Value::Null]) => Ok(Value::Null),
            (DESTROY, [Value::Handle(id)]) => match self.inner.entry_mut(*id)? {
                Entry::Effect { live, .. } => {
                    *live = false;
                    Ok(Value::Null)
                }
                _ => Err(self.inner.error(name, Failure::WrongFamily)),
            },
            _ => Err(self.inner.error(name, Failure::Signature)),
        })())
    }
}
