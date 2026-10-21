//! Fresh single-player roster policy on the existing player authority.
use super::*;

impl MapStartupHost {
    pub(super) fn fresh_observations(
        &mut self,
        context: &Arc<ParsedMapContext>,
        input: &ResolvedObservations,
        roles: &ResolvedNewGame,
    ) -> Result<ResolvedObservations, MapStartupError> {
        let fail = |reason: &str| MapStartupError::Observation(format!("new game: {reason}"));
        if !Arc::ptr_eq(self.context(), context) {
            return Err(fail("map context differs"));
        }
        if input.controllers.is_some() || input.slot_states.is_some() {
            return Err(fail(
                "fresh roster conflicts with explicit current snapshots",
            ));
        }
        let Value::Int(normal) = self
            .native(crate::neutral_player::BJ_MAX_PLAYERS_NATIVE_NAME, &[])
            .map_err(MapStartupError::Observation)?
        else {
            return Err(fail("normal player count"));
        };
        let total = self
            .objects()
            .player_slots()
            .ok_or_else(|| fail("player slots unavailable"))?;
        let normal = usize::try_from(normal)
            .ok()
            .filter(|n| *n <= total)
            .ok_or_else(|| fail("normal player domain"))?;
        let participants = input
            .participants
            .as_ref()
            .ok_or_else(|| fail("participants missing"))?;
        if participants.len() > normal
            || input
                .local_player
                .is_none_or(|p| !participants.contains(&p))
        {
            return Err(fail("local player must occupy a normal participant slot"));
        }
        for (at, number) in participants.iter().enumerate() {
            if usize::try_from(*number).ok().is_none_or(|n| n >= normal)
                || participants[..at].contains(number)
            {
                return Err(fail("participant domain or duplicate"));
            }
        }
        let mut states = Vec::new();
        let mut controllers = Vec::new();
        states
            .try_reserve_exact(total)
            .map_err(|_| fail("slot storage"))?;
        controllers
            .try_reserve_exact(total)
            .map_err(|_| fail("controller storage"))?;
        for number in 0..total {
            let slot = i32::try_from(number).map_err(|_| fail("slot number"))?;
            let virtual_player = number >= normal;
            let control = if virtual_player {
                roles.creep
            } else {
                self.objects()
                    .player_identity(slot)
                    .and_then(|p| self.objects().player_settings(p).ok())
                    .and_then(|p| p.controller)
                    .unwrap_or(roles.none)
            };
            controllers.push((slot, control));
            states.push((
                slot,
                if virtual_player || participants.contains(&slot) {
                    roles.playing
                } else {
                    roles.empty
                },
            ));
        }
        // Occupied game slots are real players; unoccupied slots remain lazy.
        for number in participants {
            self.native("Player", &[Value::Int(*number)])
                .map_err(MapStartupError::Observation)?;
        }
        if let Some(rules) = &roles.initial_alliances {
            self.objects_mut()
                .admit_startup_alliances(context, rules.clone(), &controllers)?;
        }
        Ok(ResolvedObservations {
            new_game: None,
            local_player: input.local_player,
            participants: Some(participants.clone()),
            settings: input.settings,
            version: input.version,
            difficulty: input.difficulty,
            default_difficulty: input.default_difficulty,
            slot_states: Some(states),
            controllers: Some(controllers),
        })
    }
}
