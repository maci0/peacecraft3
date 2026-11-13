use super::{Error, Failure, ObjectHost};
use pcx_script::Value;
use std::time::Duration;

#[path = "thematic_music_rules.rs"]
pub(super) mod thematic_rules;

/// One executed thematic start, with an independently ordered seek position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThematicMusicRequest {
    pub path: String,
    pub revision: u64,
    pub position: Option<Duration>,
    pub position_revision: u64,
}

/// Exact executed selection arguments, not an expanded playlist or playback state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapMusicSelection {
    pub name: String,
    pub random: bool,
    pub index: i32,
}
#[derive(Clone)]
pub(super) struct State {
    max_name_bytes: usize,
    selection: Option<MapMusicSelection>,
    thematic: Option<ThematicMusicRequest>,
    thematic_rules: Option<thematic_rules::Rules>,
    revision: u64,
}

const OPTIONAL: [(&str, &[&str]); 3] = [
    ("ClearMapMusic", &[]),
    ("PlayThematicMusic", &["string"]),
    ("EndThematicMusic", &[]),
];
impl State {
    pub(super) fn has_thematic_rules(&self) -> bool {
        self.thematic_rules.is_some()
    }
}
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
            thematic_rules: None,
            revision: 0,
        });
        Ok(self)
    }
    pub fn map_music_selection(&self) -> Option<&MapMusicSelection> {
        self.map_music.as_ref()?.selection.as_ref()
    }
    /// Executed `PlayThematicMusic` path until `EndThematicMusic`.
    pub fn thematic_music(&self) -> Option<&str> {
        Some(&self.thematic_music_request()?.path)
    }
    pub fn thematic_music_request(&self) -> Option<&ThematicMusicRequest> {
        self.map_music.as_ref()?.thematic.as_ref()
    }
    pub fn with_thematic_music_rules(
        mut self,
        rules: thematic_rules::Rules,
    ) -> Result<Self, Error> {
        let fail = |reason| self.inner.error("PlayThematicMusicEx", reason);
        if let Some(source) = self
            .inner
            .program
            .natives()
            .get("SetMapMusic")
            .map(|native| &native.source)
        {
            for name in ["PlayThematicMusicEx", "SetThematicMusicPlayPosition"] {
                if self
                    .inner
                    .program
                    .natives()
                    .get(name)
                    .is_some_and(|native| &native.source != source)
                {
                    return Err(fail(Failure::Source));
                }
            }
        }
        let state = self
            .map_music
            .as_mut()
            .ok_or_else(|| fail(Failure::MusicState("map music is not admitted")))?;
        if state.thematic_rules.is_some() || state.revision != 0 {
            return Err(fail(Failure::MusicState(
                "thematic rules must precede playback",
            )));
        }
        state.thematic_rules = Some(rules);
        Ok(self)
    }
    pub(super) fn start_thematic(
        &mut self,
        path: impl AsRef<[u8]>,
        from: Option<i32>,
    ) -> Result<(), Error> {
        let name = if from.is_some() {
            "PlayThematicMusicEx"
        } else {
            "PlayThematicMusic"
        };
        let fail = |reason| self.inner.error(name, reason);
        let mut state = self
            .map_music
            .as_ref()
            .ok_or_else(|| fail(Failure::MusicState("map music is not admitted")))?.clone();
        let path = path.as_ref();
        if path.len() > state.max_name_bytes || path.contains(&0) {
            return Err(fail(Failure::MusicState("invalid thematic music path")));
        }
        let position = match (&state.thematic_rules, from) {
            (Some(rules), Some(from)) => rules.position(from),
            (Some(rules), None) => rules.initial_position(),
            (None, Some(_)) => {
                return Err(fail(Failure::MusicState(
                    "thematic music rules are missing",
                )));
            }
            (None, None) => Ok(None),
        }
        .map_err(|_| fail(Failure::MusicState("thematic music offset is unsupported")))?;
        let revision = state
            .revision
            .checked_add(1)
            .ok_or_else(|| fail(Failure::Budget("thematic music revisions")))?;
        let path = std::str::from_utf8(path)
            .map_err(|why| fail(Failure::ScriptString(format!("thematic path UTF8: {why}"))))?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(path.len())
            .map_err(|_| fail(Failure::Capacity))?;
        owned.push_str(path);
        state.thematic = Some(ThematicMusicRequest {
            path: owned,
            revision,
            position,
            position_revision: revision,
        });
        state.revision = revision;
        if let Some(position) = position {
            self.append_music(name, super::play_music::controller::Operation::Theme {
                path: state.thematic.as_ref().unwrap().path.clone(),
                position_ms: u32::try_from(position.as_millis()).map_err(|_| fail(Failure::MusicState("thematic offset exceeds source range")))?,
            })?;
        } else if self.regular_music_ready() {
            return Err(fail(Failure::MusicState("thematic start with preserved offset is unsupported")));
        }
        self.map_music = Some(state);
        Ok(())
    }
    pub(super) fn seek_thematic(&mut self, millis: i32) -> Result<(), Error> {
        let fail = |reason| self.inner.error("SetThematicMusicPlayPosition", reason);
        let mut state = self
            .map_music
            .as_ref()
            .ok_or_else(|| fail(Failure::MusicState("map music is not admitted")))?.clone();
        let rules = state
            .thematic_rules
            .as_ref()
            .ok_or_else(|| fail(Failure::MusicState("thematic music rules are missing")))?;
        let Some(request) = state.thematic.as_mut() else {
            return Ok(());
        };
        let Some(position) = rules
            .position(millis)
            .map_err(|_| fail(Failure::MusicState("thematic music offset is unsupported")))?
        else {
            return Ok(());
        };
        let revision = state
            .revision
            .checked_add(1)
            .ok_or_else(|| fail(Failure::Budget("thematic music revisions")))?;
        request.position = Some(position);
        request.position_revision = revision;
        state.revision = revision;
        self.append_music("SetThematicMusicPlayPosition", super::play_music::controller::Operation::ThemeSeek(
            u32::try_from(position.as_millis()).map_err(|_| fail(Failure::MusicState("thematic offset exceeds source range")))?
        ))?;
        self.map_music = Some(state);
        Ok(())
    }
    pub(super) fn map_music_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.map_music.as_ref()?;
        if let ("PlayThematicMusic", [Value::Str(path)]) = (name, args) {
            return Some(self.start_thematic(path, None).map(|()| Value::Null));
        }
        use super::play_music::controller::Operation;
        if OPTIONAL.iter().any(|&(n, _)| n == name) {
            return Some((|| {
                let operation = match (name, args) {
                    ("ClearMapMusic", []) => Operation::Clear,
                    ("EndThematicMusic", []) => Operation::EndTheme,
                    _ => return Err(self.inner.error(name, Failure::MusicState("invalid music arguments"))),
                };
                self.append_music(name, operation)?;
                let state = self.map_music.as_mut().unwrap();
                if name == "ClearMapMusic" { state.selection = None; } else { state.thematic = None; }
                Ok(Value::Null)
            })());
        }
        let state = self.map_music.as_ref().expect("checked music admission");
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
            if text.as_bytes().contains(&0) {
                return Err(fail(Failure::MusicState("embedded NUL is unsupported")));
            }
            let text = text.as_utf8().map_err(|why| {
                fail(Failure::ScriptString(format!("map music name UTF8: {why}")))
            })?;
            let mut name = String::new();
            name.try_reserve_exact(text.len())
                .map_err(|_| fail(Failure::Capacity))?;
            name.push_str(text);
            let selected_index = if *random {
                self.regular_music_rules().and_then(Result::ok).map_or(*index, |r| r.random_index())
            } else { *index };
            self.append_music("SetMapMusic", Operation::Select { name: name.clone(), index: selected_index })?;
            self.map_music.as_mut().unwrap().selection = Some(MapMusicSelection { name, random: *random, index: *index });
            Ok(Value::Null)
        })())
    }
}

#[cfg(test)]
#[path = "map_music_host_tests.rs"]
mod tests;
