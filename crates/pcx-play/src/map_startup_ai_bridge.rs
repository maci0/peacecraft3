//! One owning map callback batch; reentrant AI starts remain explicitly unqualified.
use super::*;
use pcx_script::ScriptCall;

#[derive(Clone, Copy)]
pub(super) struct Frame {
    player: i32,
    boundary: crate::playercolor_host::AiCallbackBoundary,
    pub(super) ends_game_scope: bool,
}

impl MapStartupHost {
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn with_ai_map_bridge_fixture(mut self) -> Self {
        self.ai_map_bridge_fixture = true;
        self
    }
    pub(super) fn ai_map_bridge_admitted(&self) -> bool {
        #[cfg(any(test, feature = "test-fixtures"))]
        {
            self.ai_map_bridge_fixture
        }
        #[cfg(not(any(test, feature = "test-fixtures")))]
        {
            false
        }
    }
    pub(crate) fn ai_bridge_fault(&self) -> Option<Arc<String>> {
        self.ai_bridge_fault.clone()
    }
    pub(crate) fn ai_bridge_pending(&self) -> bool {
        self.ai_bridge.is_some()
    }
    pub(super) fn prepare_ai_bridge(&self) -> Result<(), String> {
        if let Some(error) = &self.ai_bridge_fault {
            return Err((**error).clone());
        }
        if self.ai_bridge.is_some() {
            return Err(
                "reentrant StartCampaignAI from an AI map observer is not qualified".into(),
            );
        }
        Ok(())
    }
    pub(super) fn arm_ai_bridge(&mut self, player: i32) -> Result<(), String> {
        self.prepare_ai_bridge()?;
        self.ai_bridge = Some(Frame {
            player,
            boundary: self.objects().capture_ai_callback_boundary(),
            ends_game_scope: false,
        });
        Ok(())
    }
    pub(super) fn check_ai_observer_native(&self, name: &str) -> Result<(), String> {
        if let Some(error) = &self.ai_bridge_fault {
            return Err((**error).clone());
        }
        if self.ai_bridge.is_some()
            && matches!(
                name,
                "TriggerSleepAction" | "TriggerWaitForSound" | "StartCampaignAI"
            )
        {
            return Err(format!(
                "{name}: sleeping or reentrant AI map observers are not qualified"
            ));
        }
        Ok(())
    }
    pub(crate) fn begin_pending_ai_bridge(
        &mut self,
        player: i32,
        ends_game_scope: bool,
    ) -> Result<Option<ScriptCall>, String> {
        let result = (|| {
            let Some(batch) = self.ai.take_map_call(player) else {
                if self.ai_bridge.is_some_and(|frame| frame.ends_game_scope) {
                    return Err("AI source wake lost its pending map callback".into());
                }
                self.ai_bridge = None;
                return Ok(None);
            };
            let frame = self
                .ai_bridge
                .as_mut()
                .ok_or("campaign AI map callback owner was not reserved")?;
            if frame.player != player || frame.boundary.synchronous_depth() != batch.base_depth {
                return Err("campaign AI map callback base context changed".into());
            }
            frame.ends_game_scope = ends_game_scope;
            self.finish_ai_map_call(batch.call).map(Some)
        })();
        if let Err(error) = &result {
            self.abort_ai_bridge(error);
        }
        result
    }
    pub(super) fn finish_ai_map_call(
        &mut self,
        mut call: ScriptCall,
    ) -> Result<ScriptCall, String> {
        loop {
            let Some(frame) = self.ai_bridge else {
                return Ok(call);
            };
            let depth = self.objects().synchronous_frame_depth();
            if depth < frame.boundary.synchronous_depth() {
                return Err("AI map callback parent frame disappeared".into());
            }
            if depth > frame.boundary.synchronous_depth() {
                return Ok(call);
            }
            if !matches!(call, ScriptCall::Return(Value::Null)) {
                return Err("AI map callback returned outside its owning frame".into());
            }
            if let Err(error) =
                self.ai
                    .resume_map_call(&mut self.inner, frame.player, Ok(Value::Null))
            {
                self.abort_ai_bridge(&error);
                return Err(error);
            }
            if let Some(batch) = self.ai.take_map_call(frame.player) {
                if batch.base_depth != frame.boundary.synchronous_depth() {
                    return Err("AI map callback base context changed".into());
                }
                call = batch.call;
                continue;
            }
            self.ai_bridge = None;
            if frame.ends_game_scope
                && let Err(error) =
                    crate::campaign_ai::AiWorld::ai_end_native_domain_callback(self.world_mut())
            {
                self.ai_bridge_fault = Some(Arc::new(error.clone()));
                return Err(error);
            }
            return Ok(ScriptCall::Return(Value::Null));
        }
    }
    pub(crate) fn retain_ai_bridge_failure(&mut self, primary: String) -> Arc<String> {
        self.abort_ai_bridge(&primary);
        let detail = self.ai_bridge_fault.take();
        let error = match detail {
            Some(detail) if detail.starts_with(&primary) => {
                self.ai_bridge_fault = Some(Arc::clone(&detail));
                return detail;
            }
            Some(detail) => format!("{primary}; cleanup: {detail}"),
            None => primary,
        };
        let fault = Arc::new(error);
        self.ai_bridge_fault = Some(Arc::clone(&fault));
        fault
    }
    pub(crate) fn abort_ai_bridge(&mut self, primary: &str) {
        let Some(frame) = self.ai_bridge.take() else {
            return;
        };
        let mut error = primary.to_owned();
        if let Err(cleanup) = self
            .ai
            .abort_map_call(&mut self.inner, frame.player, primary)
        {
            error.push_str(&format!("; AI retirement: {cleanup}"));
        }
        if let Err(cleanup) = self
            .objects_mut()
            .abort_ai_callback_boundary(frame.boundary)
        {
            error.push_str(&format!("; world callback cleanup: {cleanup}"));
        }
        if frame.ends_game_scope
            && let Err(cleanup) =
                crate::campaign_ai::AiWorld::ai_end_native_domain_callback(self.world_mut())
        {
            error.push_str(&format!("; callback cleanup: {cleanup}"));
        }

        self.ai_bridge_fault = Some(Arc::new(error));
    }
}

impl MapStartupHost {
    pub(super) fn campaign_ai_args<'a>(&self, args: &'a [Value]) -> Result<(i32, &'a str), String> {
        let name = "StartCampaignAI";
        let scripts = &self.context().scripts;
        let declaration = scripts
            .program()
            .natives()
            .get(name)
            .ok_or("StartCampaignAI: selected common does not declare the native")?;
        if scripts
            .digests
            .first()
            .is_none_or(|common| declaration.source != common.label)
            || declaration.params.len() != 2
            || declaration.params[0].ty.kind.name() != "player"
            || declaration.params[1].ty.kind.name() != "string"
            || declaration.ret.kind.name() != "nothing"
        {
            return Err("StartCampaignAI: selected common declaration differs".into());
        }
        let [Value::Handle(player), Value::Str(path)] = args else {
            return Err("StartCampaignAI: expected player and script path".into());
        };
        let slots = self
            .objects()
            .player_slots()
            .ok_or("StartCampaignAI: players are not admitted")?;
        let ordinal = (0..slots)
            .filter_map(|slot| i32::try_from(slot).ok())
            .find(|&slot| self.objects().player_identity(slot) == Some(*player))
            .ok_or("StartCampaignAI: player is not a current identity")?;
        let path = path
            .as_utf8()
            .map_err(|why| format!("StartCampaignAI: unsupported path UTF8: {why}"))?;
        Ok((ordinal, path))
    }
}
