use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

const NATIVE: &str = "SetAltMinimapIcon";
impl ObjectHost {
    pub(super) fn alt_minimap_icon_declaration(&self, source: &str) -> Result<(), Error> {
        let Some(native) = self.inner.program.natives().get(NATIVE) else {
            return Ok(());
        };
        Self::contract(&self.inner, NATIVE, &["string"], "nothing", false, None)?;
        if native.source != source {
            return Err(self.inner.error(NATIVE, Failure::Source));
        }
        Ok(())
    }

    /// Path last accepted by `SetAltMinimapIcon`, `None` before any accepted call.
    pub fn alt_minimap_icon(&self) -> Option<&str> {
        self.units.as_ref()?.alt_minimap_icon.as_deref()
    }

    /// `SetAltMinimapIcon` (`0x4a6ea0`) replaces the alternate icon path of the
    /// minimap object (`0x64c9b0`, string at `+0x1cc`) and reloads its texture.
    /// A null or empty path is ignored, so no call clears the icon. A
    /// `TRIGSTR_` text is resolved through the map string table first; this
    /// host holds no string table, so it refuses. The texture load and the
    /// minimap drawing of units that `UnitSetUsesAltIcon` flags are not modeled.
    pub(super) fn alt_minimap_icon_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.alt_minimap_icon_native_with_strings(name, args, None)
    }
    pub(super) fn alt_minimap_icon_native_with_strings(
        &mut self,
        name: &str,
        args: &[Value],
        strings: Option<&pcx_script::text::NativeStringLookup<'_>>,
    ) -> Option<Result<Value, Error>> {
        self.units.as_ref()?;
        if name != NATIVE || self.inner.program.natives().get(name).is_none() {
            return None;
        }
        let path = match args {
            [Value::Null] => return Some(Ok(Value::Null)),
            [Value::Str(path)] => path,
            _ => return Some(Err(self.inner.error(name, Failure::Signature))),
        };
        if path.is_empty() {
            return Some(Ok(Value::Null));
        }
        let path = match pcx_script::text::native_map_bytes(path.as_bytes(), strings) {
            Ok(path) => path,
            Err(why) => return Some(Err(self.inner.error(name, Failure::ScriptString(why)))),
        };
        if path.is_empty() {
            return Some(Ok(Value::Null));
        }
        let path = match std::str::from_utf8(&path) {
            Ok(path) => path,
            Err(why) => {
                return Some(Err(self.inner.error(
                    name,
                    Failure::ScriptString(format!("minimap icon path UTF8: {why}")),
                )));
            }
        };
        self.units.as_mut()?.alt_minimap_icon = Some(path.into());
        Some(Ok(Value::Null))
    }
}
