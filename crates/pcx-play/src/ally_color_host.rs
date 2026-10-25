use super::{Error, ExecutedUnit, Failure, ObjectHost};
use crate::ally_color::{Input, Role};
use pcx_script::Value;
pub(super) struct State {
    input: Input,
    current: i32,
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
        self.ally_colors = Some(State { input, current });
        Ok(())
    }
    pub fn ally_color_filter_state(&self) -> Option<i32> {
        self.ally_colors.as_ref().map(|s| s.current)
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
                self.ally_colors.as_mut().expect("admitted").current = *value;
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
            state.input.rules.filters_minimap(state.current)
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
            return Err("ally-color passive alliance rules are absent".into());
        }
        // shortcut: active local-player relation; add observer team projection with observer runtime support.
        Ok(if self.hostile_owners(local.identity, unit.owner) {
            Role::Enemy
        } else {
            Role::Ally
        })
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
