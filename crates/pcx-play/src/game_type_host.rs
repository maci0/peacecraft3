//! `GetGameTypeSelected` for the launch route the application started the map from.
use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

const NATIVE: &str = "GetGameTypeSelected";
const CONVERSION: &str = "ConvertGameType";

impl ObjectHost {
    /// Admit the selected game type, the value the launch route stores in the game info
    /// (`game_type_rules::Rules::selected`). Without it the native stays unsupported. A
    /// value outside the `ConvertGameType` constants fails at the call, not at admission,
    /// since only a script that asks for it depends on it.
    pub fn with_game_type(mut self, selected: i32) -> Result<Self, Error> {
        if self.game_type.is_some() {
            return Err(self
                .inner
                .error(NATIVE, Failure::SettingsState("game type already admitted")));
        }
        Self::contract(&self.inner, NATIVE, &[], "gametype", false, None)?;
        if self
            .inner
            .conversions
            .get(CONVERSION)
            .is_none_or(|c| c.kind != "gametype")
        {
            return Err(self.inner.error(NATIVE, Failure::Signature));
        }
        self.game_type = Some(selected);
        Ok(self)
    }

    pub(super) fn game_type_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let selected = self.game_type.filter(|_| name == NATIVE)?;
        Some(match args {
            [] => self.inner.convert_for(CONVERSION, &[Value::Int(selected)]),
            _ => Err(self.inner.error(NATIVE, Failure::Arguments)),
        })
    }
}
