use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

const NATIVE: &str = "SetReservedLocalHeroButtons";

impl ObjectHost {
    pub(super) fn hero_button_reserve_declaration(&self, source: &str) -> Result<(), Error> {
        let Some(native) = self.inner.program.natives().get(NATIVE) else {
            return Ok(());
        };
        Self::contract(&self.inner, NATIVE, &["integer"], "nothing", false, None)?;
        if native.source != source {
            return Err(self.inner.error(NATIVE, Failure::Source));
        }
        Ok(())
    }

    /// Integer last passed to `SetReservedLocalHeroButtons`, `None` before any call.
    pub fn reserved_local_hero_buttons(&self) -> Option<i32> {
        self.units.as_ref()?.reserved_local_hero_buttons
    }

    /// `SetReservedLocalHeroButtons` (`0x4a9aa0`) stores its integer unclamped
    /// in the hero button bar's field at `+0x184` and relays the bar out
    /// (`0x650e20`, `0x6471f0`). The state is kept; the bar is not rendered.
    pub(super) fn hero_button_reserve_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE || self.inner.program.natives().get(name).is_none() {
            return None;
        }
        let [Value::Int(reserved)] = args else {
            return Some(Err(self.inner.error(name, Failure::Signature)));
        };
        self.units.as_mut()?.reserved_local_hero_buttons = Some(*reserved);
        Some(Ok(Value::Null))
    }
}
