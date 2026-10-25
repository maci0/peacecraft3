//! Rectangle callbacks over canonical ground items and destructables.
use super::{Entry, Error, Failure, NativeCallback, ObjectHost};
use pcx_script::{HandleId, ScriptCall, Value, value::FunctionId};

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Item,
    Destructable,
}
struct Spec {
    name: &'static str,
    enumerated: &'static str,
    filtered: &'static str,
    create: &'static str,
    family: &'static str,
    kind: Kind,
}
const SPECS: [Spec; 2] = [
    Spec {
        name: "EnumItemsInRect",
        enumerated: "GetEnumItem",
        filtered: "GetFilterItem",
        create: "CreateItem",
        family: "item",
        kind: Kind::Item,
    },
    Spec {
        name: "EnumDestructablesInRect",
        enumerated: "GetEnumDestructable",
        filtered: "GetFilterDestructable",
        create: "CreateDestructable",
        family: "destructable",
        kind: Kind::Destructable,
    },
];
pub(super) struct Enumeration {
    spec: &'static Spec,
    candidates: Vec<HandleId>,
    at: usize,
    predicate: Option<FunctionId>,
    action: FunctionId,
    filtering: bool,
}
impl ObjectHost {
    pub(super) fn check_widget_enumeration(&self, create: &str) -> Result<(), Error> {
        let spec = SPECS
            .iter()
            .find(|s| s.create == create)
            .expect("widget creation family");
        let source = &self
            .inner
            .program
            .natives()
            .get(create)
            .expect("creation admitted")
            .source;
        for (name, args, result, constant) in [
            (
                spec.name,
                &["rect", "boolexpr", "code"][..],
                "nothing",
                false,
            ),
            (spec.enumerated, &[][..], spec.family, true),
            (spec.filtered, &[][..], spec.family, true),
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, args, result, constant, Some(spec.family))?;
                if native.source != *source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        Ok(())
    }
    fn widget_admitted(&self, kind: Kind) -> bool {
        match kind {
            Kind::Item => self.items.is_some(),
            Kind::Destructable => self.destructables.is_some(),
        }
    }
    fn widget_position(&self, id: HandleId, kind: Kind) -> Option<[f32; 2]> {
        match (self.inner.entry(id).ok()?, kind) {
            (Entry::Item { record }, Kind::Item)
                if !record.consumed && record.held_by.is_none() =>
            {
                Some([record.x, record.y])
            }
            (Entry::Destructable { creation, .. }, Kind::Destructable) => {
                Some([creation.x, creation.y])
            }
            _ => None,
        }
    }
    pub(super) fn begin_widget_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        let Some(spec) = SPECS.iter().find(|s| s.name == name) else {
            return Ok(None);
        };
        if !self.widget_admitted(spec.kind) || self.inner.program.natives().get(name).is_none() {
            return Ok(None);
        }
        let fail = |reason| self.inner.error(name, Failure::ItemState(reason));
        let [Value::Handle(rect), filter, Value::Code(action)] = args else {
            return Err(fail("rectangle, optional boolexpr and action required"));
        };
        let bounds = self.rectangle_bounds(*rect)?;
        let function = self
            .inner
            .program
            .functions()
            .get(action.index())
            .ok_or_else(|| fail("action is not a loaded function"))?;
        if !function.params.is_empty()
            || function.ret.array
            || function.ret.kind.name() != "nothing"
        {
            return Err(fail("action must take and return nothing"));
        }
        let predicate = match filter {
            Value::Null => None,
            Value::Handle(id) => Some(match self.inner.entry(*id)? {
                Entry::Filter { .. } => self.filter_callback(*id)?.function,
                Entry::Condition { .. } => self.condition_callback(*id)?.function,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }),
            _ => return Err(fail("filter must be a boolexpr or null")),
        };
        if self.callbacks.len() >= pcx_script::eval::MAX_CALL_DEPTH as usize {
            return Err(self
                .inner
                .error(name, Failure::Budget("nested native callbacks")));
        }
        let mut candidates = Vec::new();
        // shortcut: inclusive centre containment and a creation-order snapshot; qualify native boundary/mutation ordering separately.
        for index in 0..self.inner.arena.len() {
            let id = self.inner.id(index, name)?;
            if self.widget_position(id, spec.kind).is_some_and(|[x, y]| {
                bounds[0] <= x && x <= bounds[2] && bounds[1] <= y && y <= bounds[3]
            }) {
                candidates
                    .try_reserve(1)
                    .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                candidates.push(id);
            }
        }
        if candidates.is_empty() {
            return Ok(Some(ScriptCall::Return(Value::Null)));
        }
        self.callbacks
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        self.callbacks.push(NativeCallback::Widget(Enumeration {
            spec,
            candidates,
            at: 0,
            predicate,
            action: *action,
            filtering: false,
        }));
        self.next_widget_call().map(Some)
    }
    fn next_widget_call(&mut self) -> Result<ScriptCall, Error> {
        loop {
            let Some(NativeCallback::Widget(frame)) = self.callbacks.last() else {
                return Err(self.inner.error("widget enumeration", Failure::Identity));
            };
            let Some(&id) = frame.candidates.get(frame.at) else {
                self.callbacks.pop();
                return Ok(ScriptCall::Return(Value::Null));
            };
            let live = self.widget_position(id, frame.spec.kind).is_some();
            let Some(NativeCallback::Widget(frame)) = self.callbacks.last_mut() else {
                unreachable!()
            };
            if !live {
                frame.at += 1;
                continue;
            }
            frame.filtering = frame.predicate.is_some();
            return Ok(ScriptCall::Call(frame.predicate.unwrap_or(frame.action)));
        }
    }
    pub(super) fn continue_widget_call(&mut self, value: Value) -> Result<ScriptCall, Error> {
        let Some(NativeCallback::Widget(frame)) = self.callbacks.last() else {
            return Err(self.inner.error("widget enumeration", Failure::Identity));
        };
        if frame.filtering {
            let Value::Bool(accepted) = value else {
                return Err(self.inner.error(
                    frame.spec.name,
                    Failure::ItemState("predicate did not return boolean"),
                ));
            };
            if accepted
                && self
                    .widget_position(frame.candidates[frame.at], frame.spec.kind)
                    .is_some()
            {
                let Some(NativeCallback::Widget(frame)) = self.callbacks.last_mut() else {
                    unreachable!()
                };
                frame.filtering = false;
                return Ok(ScriptCall::Call(frame.action));
            }
        }
        let Some(NativeCallback::Widget(frame)) = self.callbacks.last_mut() else {
            unreachable!()
        };
        frame.at += 1;
        self.next_widget_call()
    }
    pub(super) fn widget_enumeration_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let spec = SPECS
            .iter()
            .find(|s| s.enumerated == name || s.filtered == name)?;
        if !self.widget_admitted(spec.kind) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(if !args.is_empty() {
            Err(self
                .inner
                .error(name, Failure::ItemState("response takes no arguments")))
        } else {
            Ok(self
                .callbacks
                .iter()
                .rev()
                .find_map(|frame| match frame {
                    NativeCallback::Widget(frame)
                        if frame.spec.kind == spec.kind
                            && frame.filtering == (spec.filtered == name) =>
                    {
                        frame.candidates.get(frame.at).copied().map(Value::Handle)
                    }
                    _ => None,
                })
                .unwrap_or(Value::Null))
        })
    }
}
