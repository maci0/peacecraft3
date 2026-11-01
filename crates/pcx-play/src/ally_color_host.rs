use super::{Error, ExecutedUnit, Failure, ObjectHost};
use crate::ally_color::{Input, PersistedAllyColor, Role, SpecialMinimapLayer};
use pcx_script::Value;
pub(super) struct State {
    input: Input,
    current: i32,
    minimap_filtered: bool,
}
impl ObjectHost {
    pub fn admit_ally_colors(&mut self, input: Input) -> Result<(), Error> {
        let name = "SetAllyColorFilterState";
        if self.ally_colors.is_some()
            || self.player_client_sha256() != Some(input.rules.image_sha256)
        {
            return Err(self.inner.error(
                name,
                Failure::SettingsState("ally-color source differs or already admitted"),
            ));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("Player")
            .ok_or_else(|| self.inner.error(name, Failure::Source))?
            .source;
        for (name, args, result) in [
            (name, &["integer"][..], "nothing"),
            ("GetAllyColorFilterState", &[][..], "integer"),
        ] {
            Self::contract(&self.inner, name, args, result, false, None)?;
            if &self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked")
                .source
                != source
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        let current = input.rules.initial;
        let minimap_filtered = input.rules.minimap_filtered_at_start();
        self.ally_colors = Some(State {
            input,
            current,
            minimap_filtered,
        });
        Ok(())
    }
    pub fn ally_color_filter_state(&self) -> Option<i32> {
        self.ally_colors.as_ref().map(|s| s.current)
    }
    /// Export the raw UI preference for persistence across sessions. No
    /// interpretation happens here; the next admitted map re-derives roles.
    pub fn export_ally_color_preference(&self) -> Option<PersistedAllyColor> {
        self.ally_colors
            .as_ref()
            .map(|s| PersistedAllyColor { raw_state: s.current })
    }
    /// Apply a persisted preference after admission. Arbitrary integers
    /// round-trip; modes are re-derived from the newly admitted rules, so a
    /// state saved under one client never manufactures another client's modes.
    pub fn apply_persisted_ally_color(&mut self, saved: PersistedAllyColor) -> Result<(), Error> {
        let Some(state) = self.ally_colors.as_mut() else {
            return Err(self.inner.error(
                "SetAllyColorFilterState",
                Failure::SettingsState("ally-color preference applied before admission"),
            ));
        };
        state.current = saved.raw_state;
        state.minimap_filtered = state.input.rules.filters_minimap(saved.raw_state);
        Ok(())
    }
    pub(super) fn ally_color_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, "SetAllyColorFilterState" | "GetAllyColorFilterState") {
            return None;
        }
        self.ally_colors.as_ref()?;
        Some(match (name, args) {
            ("GetAllyColorFilterState", []) => Ok(Value::Int(
                self.ally_colors.as_ref().expect("admitted").current,
            )),
            ("SetAllyColorFilterState", [Value::Int(value)]) => {
                let state = self.ally_colors.as_mut().expect("admitted");
                state.current = *value;
                state.minimap_filtered = state.input.rules.filters_minimap(*value);
                Ok(Value::Null)
            }
            _ => Err(self.inner.error(
                name,
                Failure::SettingsState("ally-color arguments do not match declaration"),
            )),
        })
    }
    fn display_role(&self, unit: &ExecutedUnit, minimap: bool) -> Result<Role, String> {
        let Some(state) = &self.ally_colors else {
            return Ok(Role::Original);
        };
        let filtered = if minimap {
            state.minimap_filtered
        } else {
            state.input.rules.filters_units(state.current)
        };
        if !minimap && (!filtered || unit.executed_owner == state.input.rules.unfiltered_owner) {
            return Ok(Role::Original);
        }
        let local = self
            .local_player_settings()
            .ok_or("ally-color viewer is absent")?;
        if unit.owner == local.identity {
            return Ok(Role::Player);
        }
        if !filtered {
            return Ok(Role::Original);
        }
        if unit.executed_owner >= state.input.rules.creep_first {
            return Ok(Role::Creep);
        }
        if self.combat.is_none() {
            // Observer/team projection: without passive-alliance rules (observer
            // views, pre-combat admission, or full-mission replays that outlive
            // one combat context), fall back to authored team numbers, which are
            // map data rather than invented alliances. Same-team owners are
            // allies; other teams are enemies. Absent team data refuses instead
            // of manufacturing a relation.
            return self.team_projected_role(unit, local.identity);
        }
        Ok(if self.hostile_owners(local.identity, unit.owner) {
            Role::Enemy
        } else {
            Role::Ally
        })
    }
    /// Team-number projection for observer views and combat-absent contexts.
    /// Authored `team` values come from the admitted map/player rows. Neutral
    /// owners keep their original model color via the creep/unfiltered checks
    /// above, which run before this fallback.
    fn team_projected_role(
        &self,
        unit: &ExecutedUnit,
        local: pcx_script::HandleId,
    ) -> Result<Role, String> {
        let viewer = self
            .player_settings(local)
            .map_err(|e| e.to_string())?;
        let other = self
            .player_settings(unit.owner)
            .map_err(|e| e.to_string())?;
        match (viewer.team, other.team) {
            (Some(a), Some(b)) if a == b => Ok(Role::Ally),
            (Some(_), Some(_)) => Ok(Role::Enemy),
            _ => Err("ally-color team projection needs authored team numbers".into()),
        }
    }
    /// Special minimap layer classification for hero/resource/camp markers.
    /// These read the unit's own catalog definition through the shared
    /// `column_field` lookup rather than invented lists; a missing or
    /// ambiguous column is not a layer. Callers keep the role color and only
    /// vary marker geometry/retention.
    pub fn minimap_special_layer(&self, unit: &ExecutedUnit) -> SpecialMinimapLayer {
        use pcx_sim::world::FieldValue;
        let cell_is_one = |column: &str| -> bool {
            let Ok((_, field)) = unit.definition.column_field(column) else {
                return false;
            };
            matches!(&field.value, Ok(FieldValue::Cell(text)) if text.trim() == "1")
        };
        // `hero` is not a numeric catalog column on every definition; a live
        // hero-level request is the executed hero marker instead.
        if unit.requested_hero_level.is_some() || cell_is_one("hero") {
            return SpecialMinimapLayer::Hero;
        }
        if unit.resource_amount.is_some() {
            return SpecialMinimapLayer::Resource;
        }
        let creep_first = self
            .ally_colors
            .as_ref()
            .map(|s| s.input.rules.creep_first)
            .unwrap_or(i32::MAX);
        if unit.executed_owner >= creep_first {
            return SpecialMinimapLayer::Camp;
        }
        SpecialMinimapLayer::Ordinary
    }
    pub fn display_team_color(&self, unit: &ExecutedUnit, original: i32) -> Result<i32, String> {
        let role = self.display_role(unit, false)?;
        Ok(self
            .ally_colors
            .as_ref()
            .and_then(|s| s.input.palette.model(role))
            .unwrap_or(original))
    }
    pub fn minimap_role_color(&self, unit: &ExecutedUnit) -> Result<Option<[u8; 4]>, String> {
        let role = self.display_role(unit, true)?;
        Ok(self
            .ally_colors
            .as_ref()
            .and_then(|s| s.input.palette.minimap(role)))
    }
}
