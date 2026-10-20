use super::{Error, Failure, ObjectHost};
use pcx_script::Value;

/// Exact executed selection arguments, not an expanded playlist or playback state.
#[derive(Debug, PartialEq, Eq)]
pub struct MapMusicSelection {
    pub name: String,
    pub random: bool,
    pub index: i32,
}
pub(super) struct State {
    max_name_bytes: usize,
    selection: Option<MapMusicSelection>,
    thematic: Option<String>,
}

const OPTIONAL: [(&str, &[&str]); 3] = [
    ("ClearMapMusic", &[]),
    ("PlayThematicMusic", &["string"]),
    ("EndThematicMusic", &[]),
];
impl ObjectHost {
    /// Admits selection storage only. Track resolution and music playback are
    /// separate consumers; the native's name/index conventions are not invented.
    pub fn with_map_music_selection(mut self, max_name_bytes: usize) -> Result<Self, Error> {
        let name = "SetMapMusic";
        if self.map_music.is_some() {
            return Err(self
                .inner
                .error(name, Failure::MusicState("already admitted")));
        }
        std::alloc::Layout::array::<u8>(max_name_bytes)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        Self::contract(
            &self.inner,
            name,
            &["string", "boolean", "integer"],
            "nothing",
            false,
            None,
        )?;
        let source = &self
            .inner
            .program
            .natives()
            .get(name)
            .expect("checked contract")
            .source;
        for (optional, args) in OPTIONAL {
            let Some(native) = self.inner.program.natives().get(optional) else {
                continue;
            };
            Self::contract(&self.inner, optional, args, "nothing", false, None)?;
            if &native.source != source {
                return Err(self.inner.error(optional, Failure::Source));
            }
        }
        self.map_music = Some(State {
            max_name_bytes,
            selection: None,
            thematic: None,
        });
        Ok(self)
    }
    pub fn map_music_selection(&self) -> Option<&MapMusicSelection> {
        self.map_music.as_ref()?.selection.as_ref()
    }
    /// Executed `PlayThematicMusic` path until `EndThematicMusic`.
    pub fn thematic_music(&self) -> Option<&str> {
        self.map_music.as_ref()?.thematic.as_deref()
    }
    pub(super) fn map_music_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.map_music.as_mut()?;
        if OPTIONAL.iter().any(|&(n, _)| n == name) {
            return Some(match (name, args) {
                ("ClearMapMusic", []) => {
                    state.selection = None;
                    Ok(Value::Null)
                }
                ("PlayThematicMusic", [Value::Str(path)]) if path.len() <= state.max_name_bytes => {
                    state.thematic = Some(path.clone());
                    Ok(Value::Null)
                }
                ("EndThematicMusic", []) => {
                    state.thematic = None;
                    Ok(Value::Null)
                }
                _ => Err(self
                    .inner
                    .error(name, Failure::MusicState("invalid music arguments"))),
            });
        }
        if name != "SetMapMusic" {
            return None;
        }
        let fail = |reason| self.inner.error(name, reason);
        Some((|| {
            let [Value::Str(text), Value::Bool(random), Value::Int(index)] = args else {
                return Err(fail(Failure::MusicState(
                    "expected name, random flag and integer index",
                )));
            };
            if text.len() > state.max_name_bytes {
                return Err(fail(Failure::Budget("map music selection bytes")));
            }
            if text.contains('\0') {
                return Err(fail(Failure::MusicState("embedded NUL is unsupported")));
            }
            let mut name = String::new();
            name.try_reserve_exact(text.len())
                .map_err(|_| fail(Failure::Capacity))?;
            name.push_str(text);
            state.selection = Some(MapMusicSelection {
                name,
                random: *random,
                index: *index,
            });
            Ok(Value::Null)
        })())
    }
}

#[cfg(test)]
#[path = "map_music_host_tests.rs"]
mod tests;
