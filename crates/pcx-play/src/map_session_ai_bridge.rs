//! The selected AI wake retains its phase while the owning map VM runs listeners.
use super::*;
use pcx_script::ScriptCall;

impl MapScriptSession<crate::map_startup_host::MapStartupHost> {
    pub(super) fn deliver_ai_map_bridge(
        &mut self,
        call: ScriptCall,
    ) -> Result<(), TimerSessionError> {
        let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
        if active.ai_delivery.is_some() {
            return Err(ScriptSessionError::PendingAiDelivery.into());
        }
        active.ai_delivery = Some(AiDelivery {
            phase: AiDeliveryPhase::Pending(call),
            fault: None,
        });
        let result = self.drain_ai_map_bridge();
        if let Err(error) = result {
            let primary = error.to_string();
            let active = self.active.as_mut().expect("AI delivery owns its session");
            let fault = active.vm.host().inner.retain_ai_bridge_failure(primary);
            active
                .ai_delivery
                .as_mut()
                .expect("retained AI delivery")
                .fault = Some(Arc::clone(&fault));
            return Err(TimerSessionError::AiBridge(fault));
        }
        self.active
            .as_mut()
            .expect("AI delivery owns its session")
            .ai_delivery = None;
        Ok(())
    }

    fn drain_ai_map_bridge(&mut self) -> Result<(), TimerSessionError> {
        loop {
            let active = self.active.as_mut().ok_or(ScriptSessionError::NoSession)?;
            let phase = std::mem::replace(
                &mut active.ai_delivery.as_mut().expect("AI delivery").phase,
                AiDeliveryPhase::NativeReturn,
            );
            let call = match phase {
                AiDeliveryPhase::Pending(ScriptCall::Return(Value::Null)) => return Ok(()),
                AiDeliveryPhase::Pending(ScriptCall::Return(_)) => {
                    return Err(TimerSessionError::Ai(
                        "AI observer native returned a non-void value".into(),
                    ));
                }
                AiDeliveryPhase::Pending(ScriptCall::Call(id)) => {
                    active.ai_delivery.as_mut().expect("AI delivery").phase =
                        AiDeliveryPhase::Condition(id);
                    let name = function_name(&active.context, id)?;
                    let value = active.vm.call(&name, &[]).map_err(|error| {
                        execution_failure(&active.context, &mut active.vm, Some(id), error)
                    })?;
                    active
                        .vm
                        .host()
                        .continue_script_call(value)
                        .map_err(TimerSessionError::Ai)?
                }
                AiDeliveryPhase::Pending(ScriptCall::Spawn(actions)) => {
                    for &id in &actions {
                        function_name(&active.context, id)?;
                    }
                    active.ai_delivery.as_mut().expect("AI delivery").phase =
                        AiDeliveryPhase::Actions(actions);
                    let AiDeliveryPhase::Actions(actions) =
                        &active.ai_delivery.as_ref().expect("AI delivery").phase
                    else {
                        unreachable!()
                    };
                    let first = actions.first().copied();
                    active.vm.run_thread(actions).map_err(|error| {
                        execution_failure(&active.context, &mut active.vm, first, error)
                    })?;
                    active
                        .vm
                        .host()
                        .leave_thread()
                        .map_err(TimerSessionError::Ai)?;
                    active
                        .vm
                        .host()
                        .after_script_spawn()
                        .map_err(TimerSessionError::Ai)?
                        .ok_or_else(|| {
                            TimerSessionError::Ai(
                                "AI observer lost its synchronous Spawn owner".into(),
                            )
                        })?
                }
                AiDeliveryPhase::Condition(id) => {
                    function_name(&active.context, id)?;
                    active.ai_delivery.as_mut().expect("AI delivery").phase =
                        AiDeliveryPhase::Condition(id);
                    return Err(ScriptSessionError::PendingAiDelivery.into());
                }
                retained => {
                    active.ai_delivery.as_mut().expect("AI delivery").phase = retained;
                    return Err(ScriptSessionError::PendingAiDelivery.into());
                }
            };
            active.ai_delivery.as_mut().expect("AI delivery").phase =
                AiDeliveryPhase::Pending(call);
        }
    }
}
