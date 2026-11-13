//! AI joins the candidate's canonical world; no world loan is serialized.
use super::{MapStartupHost, ObjectHost};
use crate::{campaign_ai::CheckpointSource, scripts::MapScripts};
use pcx_script::{
    checkpoint::{Budget, Reader, Writer},
    eval::CheckpointReference,
};
use std::sync::Arc;

impl MapStartupHost {
    pub fn write_ai_checkpoint(
        &mut self,
        out: &mut Writer,
        vm_budget: Budget,
    ) -> Result<(), String> {
        let world = self
            .inner
            .as_ref()
            .ok_or("checkpoint AI world is still lent")?;
        self.ai.write_checkpoint(world, out, vm_budget)
    }

    /// Rebind saved contexts independently of the loader used for future starts.
    /// Restore only into the unpublished candidate after its world arena has been decoded.
    pub fn restore_ai_checkpoint<F, S>(
        &mut self,
        input: &mut Reader<'_>,
        vm_budget: Budget,
        source: S,
        mut reference: F,
    ) -> Result<(), String>
    where
        F: for<'a> FnMut(&ObjectHost, CheckpointReference<'a>) -> Result<(), String>,
        S: for<'a> FnMut(CheckpointSource<'a>) -> Result<Arc<MapScripts>, String>,
    {
        let world = self
            .inner
            .as_ref()
            .ok_or("checkpoint AI world is still lent")?;
        let objects = world.inner().inner().inner().inner().inner().inner();
        self.ai.restore_checkpoint(
            world,
            input,
            vm_budget,
            source,
            (
                |player| {
                    if usize::try_from(player)
                        .ok()
                        .is_none_or(|slot| objects.player_slots().is_none_or(|slots| slot >= slots))
                    {
                        return Err(
                            "checkpoint AI player slot is not admitted in the candidate world"
                                .into(),
                        );
                    }
                    Ok(())
                },
                |player| {
                    if objects.player_identity(player).is_none() {
                        return Err(
                            "checkpoint AI player is not admitted in the candidate world".into(),
                        );
                    }
                    Ok(())
                },
                |item| reference(objects, item),
            ),
        )
    }
}
