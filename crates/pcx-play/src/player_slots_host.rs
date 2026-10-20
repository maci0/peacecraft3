//! Explicit current slot-state input, including not-yet-created player rows.
use super::{Error, Failure, ObjectHost, ParsedMapContext};
use pcx_script::HandleId;
use std::sync::Arc;
impl ObjectHost {
    /// Replace the caller-qualified `(player, playerslotstate)` startup input.
    ///
    /// Requires the exact admitted context, real player rows and selected symbolic
    /// states. Omitted and subsequently created rows stay unset. Context identity
    /// does not authenticate provenance. Refresh or invalidate on external lobby
    /// changes; accepted controller writes invalidate conservatively. Failed
    /// admission preserves all prior state. No identities or rows are allocated.
    pub fn admit_startup_player_slots(
        &mut self,
        context: &Arc<ParsedMapContext>,
        states: &[(HandleId, HandleId)],
    ) -> Result<(), Error> {
        let fail = |reason| self.inner.error("GetPlayerSlotState", reason);
        if states.len() > self.player_slots().unwrap_or(0) {
            return Err(fail(Failure::Budget("player slot states")));
        }
        let mut numbered = Vec::new();
        numbered
            .try_reserve_exact(states.len())
            .map_err(|_| fail(Failure::Capacity))?;
        for (player, state) in states {
            let number = self
                .player_settings(*player)
                .map_err(|e| fail(e.failure))?
                .number;
            numbered.push((number, *state));
        }
        self.admit_startup_player_slot_numbers(context, &numbered)
    }
    /// Replace explicit `(source slot number, playerslotstate)` startup input.
    ///
    /// Entries may precede lazy Player creation. Missing numbers stay unset; no
    /// player handles are minted. Context, contracts, domains and freshness use
    /// the same rules as the identity-based admission above.
    pub fn admit_startup_player_slot_numbers(
        &mut self,
        context: &Arc<ParsedMapContext>,
        states: &[(i32, HandleId)],
    ) -> Result<(), Error> {
        let name = "GetPlayerSlotState";
        let fail = |reason| self.inner.error(name, reason);
        if self
            .player_context()
            .is_none_or(|selected| !Arc::ptr_eq(selected, context))
        {
            return Err(fail(Failure::PlayerState(
                "slot-state map context is not admitted",
            )));
        }
        let players = self.players.as_ref().expect("checked player context");
        if states.len() > players.slots {
            return Err(fail(Failure::Budget("player slot states")));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("Player")
            .ok_or_else(|| fail(Failure::Signature))?
            .source;
        if &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?
            .source
            != source
        {
            return Err(fail(Failure::Source));
        }
        Self::contract(
            &self.inner,
            name,
            &["player"],
            "playerslotstate",
            false,
            Some("player"),
        )?;
        Self::contract(
            &self.inner,
            "ConvertPlayerSlotState",
            &["integer"],
            "playerslotstate",
            true,
            None,
        )?;
        for native in [name, "ConvertPlayerSlotState"] {
            if &self
                .inner
                .program
                .natives()
                .get(native)
                .ok_or_else(|| fail(Failure::Signature))?
                .source
                != source
            {
                return Err(self.inner.error(native, Failure::Source));
            }
        }
        let conversion = self
            .inner
            .conversions
            .get("ConvertPlayerSlotState")
            .ok_or_else(|| fail(Failure::Signature))?;
        if &conversion.source != source {
            return Err(fail(Failure::Source));
        }
        if conversion.kind != "playerslotstate" {
            return Err(fail(Failure::Signature));
        }
        let mut kind = "playerslotstate";
        for _ in 0..=self.inner.program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = self
                .inner
                .program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if &ty.source != source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        for (at, (number, state)) in states.iter().enumerate() {
            if usize::try_from(*number)
                .ok()
                .is_none_or(|n| n >= players.slots)
            {
                return Err(fail(Failure::OutsideDomain(*number)));
            }
            if states[..at].iter().any(|(prior, _)| prior == number) {
                return Err(fail(Failure::PlayerState("duplicate slot-state player")));
            }
            let enumerand = self
                .inner
                .symbolic_value(*state, "playerslotstate")
                .map_err(|e| fail(e.failure))?;
            if !conversion.domain.contains(&enumerand) {
                return Err(fail(Failure::OutsideDomain(enumerand)));
            }
        }
        let mut copy = Vec::new();
        copy.try_reserve_exact(states.len())
            .map_err(|_| fail(Failure::Capacity))?;
        copy.extend_from_slice(states);
        let players = self.players.as_mut().expect("checked player context");
        for row in &mut players.rows {
            row.slot_state = states
                .iter()
                .find(|(number, _)| *number == row.number)
                .map(|(_, state)| *state);
        }
        players.slot_states = Some(copy);
        Ok(())
    }
    /// Retire the snapshot on external participation changes; setup assignments stay.
    pub fn invalidate_startup_player_slots(&mut self) {
        if let Some(players) = self.players.as_mut() {
            players.slot_states = None;
            for row in &mut players.rows {
                row.slot_state = None;
            }
        }
    }
}
