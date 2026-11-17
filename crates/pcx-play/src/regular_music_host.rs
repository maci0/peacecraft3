//! Diagnostic ordered regular intents; production admission awaits backend qualification.
use super::{Error, Failure, ObjectHost, controller::Operation, rules::Rules};
use pcx_script::Value;
use std::{collections::VecDeque, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub sequence: u64,
    pub operation: Operation,
}

pub struct Journal {
    rules: Arc<Rules>,
    commands: VecDeque<Command>,
    sequence: u64,
    acknowledged: u64,
    capacity: usize,
}

impl Journal {
    pub(crate) fn check_image(&self, image: [u8; 32]) -> Result<(), Failure> {
        if image != self.rules.image_sha256 {
            return Err(Failure::MusicState(
                "music policies use different client images",
            ));
        }
        Ok(())
    }

    pub(crate) fn selection_index(&self, random: bool, index: i32) -> i32 {
        if random {
            self.rules.map_random_index
        } else {
            index
        }
    }

    pub(crate) fn prepare(&mut self, operation: Operation) -> Result<Command, Failure> {
        if let Operation::Play { name, .. }
        | Operation::Select { name, .. }
        | Operation::Theme { path: name, .. } = &operation
            && (name.len() >= self.rules.name_bytes || name.contains('\0'))
        {
            return Err(Failure::MusicState("music name exceeds source bound"));
        }
        if matches!(&operation, Operation::Theme { path, .. } if path.is_empty()) {
            return Err(Failure::MusicState("empty thematic path is unsupported"));
        }
        if self.commands.len() >= self.capacity {
            return Err(Failure::Budget("ordered music commands"));
        }
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or(Failure::Budget("ordered music sequence"))?;
        self.commands
            .try_reserve(1)
            .map_err(|_| Failure::Capacity)?;
        Ok(Command {
            sequence,
            operation,
        })
    }
    pub(crate) fn commit(&mut self, command: Command) {
        self.sequence = command.sequence;
        self.commands.push_back(command);
    }
}

impl ObjectHost {
    /// Opt in to ordered playback; the caller owns consumption and accepted-prefix acknowledgement.
    pub fn with_regular_music_journal(
        mut self,
        rules: Arc<Rules>,
        capacity: usize,
    ) -> Result<Self, Error> {
        let fail = |why| self.inner.error("PlayMusicEx", Failure::MusicState(why));
        let state = self
            .play_music
            .as_mut()
            .ok_or_else(|| fail("play music is not admitted"))?;
        if state.journal.is_some() {
            return Err(fail("regular music journal already configured"));
        }
        if capacity == 0 {
            return Err(fail("regular music journal capacity is zero"));
        }
        std::alloc::Layout::array::<Command>(capacity)
            .map_err(|_| fail("regular music journal capacity is unsupported"))?;
        state.journal = Some(Journal {
            rules,
            commands: VecDeque::new(),
            sequence: 0,
            acknowledged: 0,
            capacity,
        });
        Ok(self)
    }

    pub fn regular_music_commands(&self) -> Option<&VecDeque<Command>> {
        Some(&self.play_music.as_ref()?.journal.as_ref()?.commands)
    }

    pub fn regular_music_rules(&self) -> Option<&Arc<Rules>> {
        Some(&self.play_music.as_ref()?.journal.as_ref()?.rules)
    }

    /// Retire only a consumed prefix; repeated acknowledgements are harmless.
    pub fn acknowledge_regular_music(&mut self, sequence: u64) -> Result<(), Error> {
        let fail = |why| self.inner.error("PlayMusicEx", Failure::MusicState(why));
        let journal = self
            .play_music
            .as_mut()
            .and_then(|s| s.journal.as_mut())
            .ok_or_else(|| fail("regular music journal is absent"))?;
        if sequence < journal.acknowledged || sequence > journal.sequence {
            return Err(fail(
                "regular music acknowledgement is outside the pending range",
            ));
        }
        while journal
            .commands
            .front()
            .is_some_and(|c| c.sequence <= sequence)
        {
            journal.commands.pop_front();
        }
        journal.acknowledged = sequence;
        Ok(())
    }

    pub(super) fn regular_music_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        let state = self.play_music.as_mut()?;
        let max_name_bytes = state.max_name_bytes;
        let journal = state.journal.as_mut()?;
        if !matches!(
            name,
            "PlayMusic" | "PlayMusicEx" | "StopMusic" | "ResumeMusic" | "SetMusicPlayPosition"
        ) {
            return None;
        }
        Some((|| {
            let fail = |why| self.inner.error(name, why);
            if self.inner.program.natives().get(name).is_none() {
                return Err(fail(Failure::MusicState(
                    "regular music native is undeclared",
                )));
            }
            let nonnegative = |value: i32| {
                u32::try_from(value).map_err(|_| {
                    fail(Failure::MusicState(
                        "negative regular music offset or fade is unsupported",
                    ))
                })
            };
            let operation = match (name, args) {
                ("PlayMusic", [Value::Str(text)]) => Operation::Play {
                    name: super::checked_name(
                        text,
                        max_name_bytes.min(journal.rules.name_bytes.saturating_sub(1)),
                        &fail,
                    )?,
                    position_ms: journal.rules.plain_play_ms,
                    fade_ms: journal.rules.plain_play_ms,
                },
                ("PlayMusicEx", [Value::Str(text), Value::Int(position), Value::Int(fade)]) => {
                    Operation::Play {
                        name: super::checked_name(
                            text,
                            max_name_bytes.min(journal.rules.name_bytes.saturating_sub(1)),
                            &fail,
                        )?,
                        position_ms: nonnegative(*position)?,
                        fade_ms: nonnegative(*fade)?,
                    }
                }
                ("StopMusic", [Value::Bool(fade)]) => Operation::Stop { fade: *fade },
                ("ResumeMusic", []) => Operation::Resume,
                ("SetMusicPlayPosition", [Value::Int(position)]) => {
                    Operation::Seek(nonnegative(*position)?)
                }
                _ => {
                    return Err(fail(Failure::MusicState(
                        "expected the declared music arguments",
                    )));
                }
            };
            let request = if let Operation::Play { name: track, .. } = &operation {
                Some(super::PlayMusicRequest {
                    name: super::checked_name(track, max_name_bytes, &fail)?,
                    from_msecs: match args {
                        [_, Value::Int(from), _] => Some(*from),
                        _ => None,
                    },
                    fadein_msecs: match args {
                        [_, _, Value::Int(fade)] => Some(*fade),
                        _ => None,
                    },
                })
            } else {
                None
            };
            let resumes = if matches!(operation, Operation::Resume) {
                state
                    .resumes
                    .checked_add(1)
                    .ok_or_else(|| fail(Failure::Budget("regular music resume count")))?
            } else {
                state.resumes
            };
            let command = journal.prepare(operation).map_err(fail)?;
            journal.commit(command);
            if let Some(request) = request {
                state.request = Some(request);
            }
            match args {
                [Value::Bool(fade)] if name == "StopMusic" => state.stop_fade_out = Some(*fade),
                [Value::Int(position)] if name == "SetMusicPlayPosition" => {
                    state.play_position = Some(*position)
                }
                _ => {}
            }
            state.resumes = resumes;
            Ok(Value::Null)
        })())
    }
}
