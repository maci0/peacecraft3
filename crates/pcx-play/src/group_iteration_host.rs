//! Group actions run on the owning VM over a retained member snapshot.
use super::{Error, Failure, NativeCallback, ObjectHost};
use pcx_script::{HandleId, ScriptCall, Value, value::FunctionId};

pub(super) struct Iteration {
    members: Vec<HandleId>,
    next: usize,
    function: FunctionId,
}

impl ObjectHost {
    pub(super) fn begin_group_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        if name != "ForGroup"
            || self.group_limit.is_none()
            || self.inner.program.natives().get(name).is_none()
        {
            return Ok(None);
        }
        let [group, Value::Code(function)] = args else {
            return Err(self.inner.error(name, Failure::GroupArguments));
        };
        let group = self.inner.id(self.group_index(group, name)?, name)?;
        let callback = self
            .inner
            .program
            .functions()
            .get(function.index())
            .ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::GroupState("callback is not a loaded function"),
                )
            })?;
        if !callback.params.is_empty()
            || callback.ret.array
            || callback.ret.kind.name() != "nothing"
        {
            return Err(self.inner.error(
                name,
                Failure::GroupState("callback takes nothing returns nothing"),
            ));
        }
        let count = self.members.iter().filter(|(g, _)| *g == group).count();
        if count == 0 {
            return Ok(Some(ScriptCall::Return(Value::Null)));
        }
        if self.callbacks.len() >= pcx_script::eval::MAX_CALL_DEPTH as usize {
            return Err(self
                .inner
                .error(name, Failure::Budget("nested native callbacks")));
        }
        let mut members = Vec::new();
        members
            .try_reserve_exact(count)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        members.extend(
            self.members
                .iter()
                .filter(|(g, _)| *g == group)
                .map(|(_, u)| *u),
        );
        self.callbacks
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        self.callbacks.push(NativeCallback::Group(Iteration {
            members,
            next: 1,
            function: *function,
        }));
        Ok(Some(ScriptCall::Call(*function)))
    }

    pub(super) fn continue_group_call(&mut self) -> Result<ScriptCall, Error> {
        let Some(NativeCallback::Group(frame)) = self.callbacks.last_mut() else {
            return Err(self
                .inner
                .error("ForGroup", Failure::GroupState("no active group callback")));
        };
        if frame.next < frame.members.len() {
            frame.next += 1;
            return Ok(ScriptCall::Call(frame.function));
        }
        self.callbacks.pop();
        Ok(ScriptCall::Return(Value::Null))
    }

    pub(super) fn group_iteration_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != "GetEnumUnit" || self.group_limit.is_none() {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(if args.is_empty() {
            Ok(self
                .callbacks
                .iter()
                .rev()
                .find_map(|frame| match frame {
                    NativeCallback::Group(frame) => {
                        Some(Value::Handle(frame.members[frame.next - 1]))
                    }
                    _ => None,
                })
                .unwrap_or(Value::Null))
        } else {
            Err(self.inner.error(name, Failure::GroupArguments))
        })
    }
}
