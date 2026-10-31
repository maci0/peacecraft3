use super::{Error, Failure, ObjectHost};
use crate::order_registry::Registry;
use pcx_script::Value;
use std::sync::Arc;

impl ObjectHost {
    /// Admit only the selected common's lookup contracts. A point `smart` order is
    /// the one exception: it is resolved and reissued as the concrete ground order.
    /// Lookup alone still does not admit any other order.
    pub fn with_order_registry(mut self, registry: Arc<Registry>) -> Result<Self, Error> {
        self.admit_order_registry(registry)?;
        Ok(self)
    }

    pub fn admit_order_registry(&mut self, registry: Arc<Registry>) -> Result<(), Error> {
        if self.order_registry.is_some() {
            return Err(self
                .inner
                .error("OrderId", Failure::UnitState("already admitted")));
        }
        for (name, args, result) in [
            ("OrderId", "string", "integer"),
            ("OrderId2String", "integer", "string"),
        ] {
            Self::contract(&self.inner, name, &[args], result, true, None)?;
        }
        let natives = self.inner.program.natives();
        let source = &natives
            .get("OrderId")
            .expect("checked lookup contract")
            .source;
        if source
            != &natives
                .get("OrderId2String")
                .expect("checked inverse contract")
                .source
            || natives
                .get("CreateUnit")
                .is_some_and(|n| &n.source != source)
        {
            return Err(self.inner.error("OrderId", Failure::Source));
        }
        self.order_registry = Some(registry);
        Ok(())
    }

    pub(super) fn order_registry_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if let Some(answer) = self.order_id_lookup(name, args) {
            return Some(answer);
        }
        self.smart_point_native(name, args)
    }

    fn order_id_lookup(&self, name: &str, args: &[Value]) -> Option<Result<Value, Error>> {
        let registry = self.order_registry.as_ref()?;
        Some(match (name, args) {
            ("OrderId", [Value::Str(text)]) => Ok(Value::Int(
                registry.id(text).filter(|id| *id != -1).unwrap_or(0),
            )),
            ("OrderId", [Value::Null]) => Ok(Value::Int(0)),
            ("OrderId2String", [Value::Int(id)]) => Ok(registry
                .name(*id)
                .map_or(Value::Null, |s| Value::Str(s.into()))),
            ("OrderId" | "OrderId2String", _) => Err(self.inner.error(name, Failure::Signature)),
            _ => return None,
        })
    }

    /// Point `smart` (`0xd0003` in the selected client) is the order a ground click
    /// sends. Resolve it with the same rules as that click, then issue the concrete
    /// order. Target `smart` stays on the inventory pickup path.
    fn smart_point_native(&mut self, name: &str, args: &[Value]) -> Option<Result<Value, Error>> {
        let location = matches!(name, "IssuePointOrderLoc" | "IssuePointOrderByIdLoc");
        if !(location || matches!(name, "IssuePointOrder" | "IssuePointOrderById")) {
            return None;
        }
        let smart = self.order_registry.as_ref()?.id("smart")?;
        let wanted = if location { 3 } else { 4 };
        if args.len() != wanted {
            return None;
        }
        let by_id = name.contains("ById");
        let order = match (&args[1], by_id) {
            (Value::Str(text), false) => self.order_registry.as_ref()?.id(text)?,
            (Value::Int(id), true) => *id,
            _ => return None,
        };
        if order != smart {
            return None;
        }
        Some(self.issue_registered_smart_point(name, args, location))
    }

    fn issue_registered_smart_point(
        &mut self,
        name: &str,
        args: &[Value],
        location: bool,
    ) -> Result<Value, Error> {
        let unit = match args[0] {
            Value::Null => return Ok(Value::Bool(false)),
            Value::Handle(id) => id,
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        if matches!(
            self.inner
                .entry(unit)
                .map_err(|error| self.inner.error(name, error.failure))?,
            super::Entry::RemovedUnit { .. }
        ) {
            return Ok(Value::Bool(false));
        }
        let at = if location {
            self.location_point(&args[2], name)?
        } else {
            let (Value::Real(x), Value::Real(y)) = (&args[2], &args[3]) else {
                return Err(self.inner.error(name, Failure::Signature));
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(self.inner.error(
                    name,
                    Failure::UnitActivity("finite destination required".into()),
                ));
            }
            [*x, *y]
        };
        let form = match self.resolve_smart_order(unit, super::SmartTarget::Ground(at)) {
            Ok(form) => form,
            Err(super::SmartRefusal::NoOrder(_) | super::SmartRefusal::SelfTarget) => {
                return Ok(Value::Bool(false));
            }
            Err(refusal) => {
                return Err(self
                    .inner
                    .error(name, Failure::UnitActivity(refusal.to_string())));
            }
        };
        // Issuing this id again would re-enter this function.
        if self
            .order_registry
            .as_ref()
            .is_some_and(|registry| registry.id("smart") == Some(form.order()))
        {
            return Ok(Value::Bool(false));
        }
        self.issue_order_form(unit, form)
            .map(Value::Bool)
            .map_err(|detail| self.inner.error(name, Failure::UnitActivity(detail)))
    }
}
