use super::{Error, Failure, ObjectHost};
use crate::order_registry::Registry;
use pcx_script::Value;
use std::sync::Arc;

impl ObjectHost {
    /// Admit only the selected common's lookup contracts. This does not admit
    /// issuing an order or infer a unit's ability to execute it.
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
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
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
}
