//! Bounded group orders reuse the canonical per-unit command consumers.
use super::{Error, Failure, ObjectHost};
use crate::group_order_rules::Rules;
use pcx_script::Value;

const NATIVES: [(&str, &str, &[&str]); 8] = [
    (
        "GroupImmediateOrder",
        "IssueImmediateOrder",
        &["group", "string"],
    ),
    (
        "GroupImmediateOrderById",
        "IssueImmediateOrderById",
        &["group", "integer"],
    ),
    (
        "GroupPointOrder",
        "IssuePointOrder",
        &["group", "string", "real", "real"],
    ),
    (
        "GroupPointOrderById",
        "IssuePointOrderById",
        &["group", "integer", "real", "real"],
    ),
    (
        "GroupPointOrderLoc",
        "IssuePointOrderLoc",
        &["group", "string", "location"],
    ),
    (
        "GroupPointOrderByIdLoc",
        "IssuePointOrderByIdLoc",
        &["group", "integer", "location"],
    ),
    (
        "GroupTargetOrder",
        "IssueTargetOrder",
        &["group", "string", "widget"],
    ),
    (
        "GroupTargetOrderById",
        "IssueTargetOrderById",
        &["group", "integer", "widget"],
    ),
];
impl ObjectHost {
    pub fn with_group_orders(mut self, rules: Rules) -> Result<Self, Error> {
        if self.group_orders.is_some() {
            return Err(self.inner.error(
                "group orders",
                Failure::GroupState("group orders already admitted"),
            ));
        }
        if self.group_limit.is_none() || self.unit_activity.is_none() {
            return Err(self.inner.error(
                "group orders",
                Failure::GroupState("groups and unit activity must be admitted"),
            ));
        }
        let registry = self.order_registry.as_ref().ok_or_else(|| {
            self.inner.error(
                "group orders",
                Failure::GroupState("order registry must be admitted"),
            )
        })?;
        if registry.binding().image_sha256 != rules.image_sha256
            || registry.binding().origin != rules.origin
        {
            return Err(self.inner.error(
                "group orders",
                Failure::GroupState("selected image lineage differs from order registry"),
            ));
        }
        for (name, unit, args) in NATIVES {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, "boolean", false, Some("group"))?;
            let mut unit_args = args.to_vec();
            unit_args[0] = "unit";
            Self::contract(
                &self.inner,
                unit,
                &unit_args,
                "boolean",
                false,
                Some("unit"),
            )?;
            if self
                .inner
                .program
                .natives()
                .get(unit)
                .is_none_or(|n| n.source != native.source)
            {
                return Err(self.inner.error(name, Failure::Signature));
            }
        }
        self.group_orders = Some(rules);
        Ok(self)
    }
    pub(super) fn group_order_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let rules = self.group_orders.as_ref()?;
        let (_, unit_name, signature) = NATIVES.iter().find(|(n, _, _)| *n == name)?;
        self.inner.program.natives().get(name)?;
        let limit = if signature.len() > 2 {
            Some(rules.point_member_limit())
        } else {
            None
        };
        Some((|| {
            if args.len() != signature.len() {
                return Err(self.inner.error(name, Failure::GroupArguments));
            }
            let by_id = name.contains("ById");
            let order = match (&args[1], by_id) {
                (Value::Str(s), false) => self.order_registry.as_ref().and_then(|r| r.id(s)),
                (Value::Int(id), true) => Some(*id),
                _ => return Err(self.inner.error(name, Failure::GroupArguments)),
            };
            let Some(_) = order else {
                return Ok(Value::Bool(false));
            };
            // Native location wrappers resolve the location before the group.
            if name.ends_with("Loc") {
                if args[2] == Value::Null {
                    return Ok(Value::Bool(false));
                }
                self.location_point(&args[2], name)?;
            } else if signature.len() == 4
                && !matches!((&args[2],&args[3]),(Value::Real(x),Value::Real(y)) if x.is_finite()&&y.is_finite())
            {
                return Err(self.inner.error(name, Failure::GroupArguments));
            } else if signature.last().is_some_and(|ty| *ty == "widget")
                && !self.group_order_widget(&args[2], name)?
            {
                // The widget resolver runs before the group. Null and unresolved
                // handles return false; they do not read or reject the group.
                return Ok(Value::Bool(false));
            }
            if args[0] == Value::Null {
                return Ok(Value::Bool(false));
            }
            let group = self.inner.id(self.group_index(&args[0], name)?, name)?;
            // A null-unit probe checks command support without changing canonical state.
            let mut command = [Value::Null, args[1].clone(), Value::Null, Value::Null];
            command[2..args.len()].clone_from_slice(&args[2..]);
            let command = &mut command[..args.len()];
            match self.member_order(unit_name, command) {
                Some(Ok(Value::Bool(false))) => {}
                Some(Err(e)) => return Err(self.inner.error(name, e.failure)),
                _ => {
                    return Err(self
                        .inner
                        .error(name, Failure::GroupState("per-unit order is not supported")));
                }
            }
            let mut any = false;
            let mut visited = 0usize;
            // Per-unit order consumers only defer events; they cannot run group-mutating callbacks.
            // Read retained membership order directly, without minting handles or copying groups.
            for at in 0..self.members.len() {
                let (member_group, unit) = self.members[at];
                if member_group != group {
                    continue;
                }
                if limit.is_some_and(|max| visited >= max) {
                    break;
                }
                visited += 1;
                command[0] = Value::Handle(unit);
                match self.member_order(unit_name, command) {
                    Some(Ok(Value::Bool(accepted))) => any |= accepted,
                    Some(Err(e)) => return Err(self.inner.error(name, e.failure)),
                    _ => {
                        return Err(self.inner.error(
                            name,
                            Failure::GroupState("per-unit order consumer did not return boolean"),
                        ));
                    }
                }
            }
            Ok(Value::Bool(any))
        })())
    }

    /// True when `value` is a widget-family handle. Null and unresolved handles
    /// are false, matching the native widget resolver's null return.
    fn group_order_widget(&self, value: &Value, name: &str) -> Result<bool, Error> {
        match value {
            Value::Null => Ok(false),
            Value::Handle(id) => match self.inner.kind(*id) {
                Ok(kind) if super::below(&self.inner.program, kind, "widget") => Ok(true),
                _ => Ok(false),
            },
            _ => Err(self.inner.error(name, Failure::GroupArguments)),
        }
    }

    /// Point and immediate orders stay on the activity consumer. Target orders
    /// use the same consumers as `ObjectHost::native`, in that order.
    fn member_order(&mut self, unit_name: &str, command: &[Value]) -> Option<Result<Value, Error>> {
        if !matches!(unit_name, "IssueTargetOrder" | "IssueTargetOrderById") {
            return self.activity_native(unit_name, command);
        }
        self.order_queue_observe(unit_name, command);
        if let Some(result) = self.harvest_native(unit_name, command) {
            return Some(result);
        }
        if let Some(result) = self.inventory_native(unit_name, command) {
            return Some(result);
        }
        if let Some(result) = self.follow_target_native(unit_name, command) {
            return Some(result);
        }
        if let Some(result) = self.construction_native(unit_name, command) {
            return Some(result);
        }
        self.combat_native(unit_name, command)
    }
}
