use super::{Error, Failure, ObjectHost};
use crate::day_night::{Assets, BoundModel};
use pcx_script::Value;

#[derive(Debug)]
pub struct DayNightState {
    assets: Assets,
    selected: Option<[usize; 2]>,
}
impl DayNightState {
    pub fn assets(&self) -> &Assets {
        &self.assets
    }
    /// Only an executed, fully resolved pair is selected; no initial models are invented.
    pub fn selected(&self) -> Option<[&BoundModel; 2]> {
        self.selected
            .map(|indices| indices.map(|i| &self.assets.models()[i]))
    }
}
impl ObjectHost {
    /// Replace the active pair only after both caller-loaded models validate.
    pub(crate) fn apply_day_night_assets(
        &mut self,
        assets: Assets,
        paths: [&str; 2],
    ) -> Result<Value, Error> {
        let name = "SetDayNightModels";
        let fail = |failure| self.inner.error(name, failure);
        let state = self
            .day_night
            .as_mut()
            .ok_or_else(|| fail(Failure::DayNightState("not admitted")))?;
        let pair = select(&assets, paths).map_err(fail)?;
        *state = DayNightState {
            assets,
            selected: Some(pair),
        };
        Ok(Value::Null)
    }
    pub fn with_day_night(mut self, assets: Assets) -> Result<Self, Error> {
        if self.day_night.is_some() {
            return Err(self.inner.error(
                "SetDayNightModels",
                Failure::DayNightState("already admitted"),
            ));
        }
        Self::contract(
            &self.inner,
            "SetDayNightModels",
            &["string", "string"],
            "nothing",
            false,
            None,
        )?;
        self.day_night = Some(DayNightState {
            assets,
            selected: None,
        });
        Ok(self)
    }
    pub fn day_night_state(&self) -> Option<&DayNightState> {
        self.day_night.as_ref()
    }
    pub(super) fn day_night_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.day_night.as_mut()?;
        if name != "SetDayNightModels" {
            return None;
        }
        let [Value::Str(terrain), Value::Str(unit)] = args else {
            return Some(Err(self
                .inner
                .error(name, Failure::DayNightState("expected two model strings"))));
        };
        let pair = match select(&state.assets, [terrain, unit]) {
            Ok(pair) => pair,
            Err(failure) => return Some(Err(self.inner.error(name, failure))),
        };
        state.selected = Some(pair);
        Some(Ok(Value::Null))
    }
}

fn select(assets: &Assets, paths: [&str; 2]) -> Result<[usize; 2], Failure> {
    let mut pair = [0; 2];
    for (out, path) in pair.iter_mut().zip(paths) {
        if path.len() > assets.path_limit() {
            return Err(Failure::Budget("day/night path"));
        }
        *out = assets
            .models()
            .iter()
            .position(|model| model.script_path() == path)
            .ok_or_else(|| Failure::MissingDayNightModel(path.to_owned()))?;
    }
    Ok(pair)
}
