//! Ordinary and thematic voices with source wall callbacks and retained regular state.
use super::{random::AudioRandom, rules::Rules};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Regular,
    Thematic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voice {
    pub serial: u64,
    pub role: Role,
    pub path: String,
    pub start_ms: u32,
    pub looping: bool,
    pub playing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Start(Voice),
    Retire(u64),
    Pause(u64),
    Resume(u64),
    Seek { serial: u64, position_ms: u32 },
    Level { serial: u64, level: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Select {
        name: String,
        index: i32,
    },
    Clear,
    StartSelected,
    Play {
        name: String,
        position_ms: u32,
        fade_ms: u32,
    },
    Stop {
        fade: bool,
    },
    Resume,
    Seek(u32),
    Theme {
        path: String,
        position_ms: u32,
    },
    EndTheme,
    ThemeSeek(u32),
    Volume(u32),
    Enabled(bool),
}

#[derive(Clone, Debug)]
struct Selection {
    name: String,
    index: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallbackKind {
    Fade,
    Next,
}
#[derive(Clone, Copy, Debug)]
struct Callback {
    due: u32,
    kind: CallbackKind,
}
#[derive(Clone, Copy, Debug)]
struct Fade {
    duration: u32,
    out: bool,
    start: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct Controller {
    rules: Arc<Rules>,
    pub random: AudioRandom,
    selection: Option<Selection>,
    tracks: Arc<Vec<String>>,
    index: usize,
    regular: Option<Voice>,
    thematic: Option<Voice>,
    serial: u64,
    enabled: bool,
    level: u32,
    effective_level: u32,
    fade: Option<Fade>,
    pending: Option<String>,
    callbacks: Vec<Callback>,
}

fn charge(work: &mut usize) -> Result<(), String> {
    *work = work
        .checked_sub(1)
        .ok_or("music controller work budget exhausted")?;
    Ok(())
}
fn due(at: u32, now: u32) -> bool {
    at.wrapping_sub(now).cast_signed() <= 0
}

impl Controller {
    pub fn new(rules: Arc<Rules>, initialized_at: u32) -> Self {
        Self {
            random: AudioRandom::new(Arc::clone(&rules), initialized_at),
            level: rules.initial_level,
            effective_level: rules.initial_level,
            enabled: rules.initial_enabled,
            rules,
            selection: None,
            tracks: Arc::new(Vec::new()),
            index: 0,
            regular: None,
            thematic: None,
            serial: 0,
            fade: None,
            pending: None,
            callbacks: Vec::new(),
        }
    }
    pub fn regular(&self) -> Option<&Voice> {
        self.regular.as_ref()
    }
    pub fn thematic(&self) -> Option<&Voice> {
        self.thematic.as_ref()
    }
    pub fn target_level(&self) -> u32 {
        self.level
    }
    pub fn effective_level(&self) -> u32 {
        self.effective_level
    }
    pub fn selection_name(&self) -> Option<&str> {
        self.selection.as_ref().map(|s| s.name.as_str())
    }

    fn cancel(&mut self) {
        self.callbacks.clear();
        self.fade = None;
        self.pending = None;
    }
    fn retire(slot: &mut Option<Voice>, effects: &mut Vec<Effect>) {
        if let Some(voice) = slot.take() {
            effects.push(Effect::Retire(voice.serial));
        }
    }
    fn start_voice(
        &mut self,
        role: Role,
        path: String,
        position_ms: u32,
        looping: bool,
        effects: &mut Vec<Effect>,
    ) -> Result<(), String> {
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or("music voice serial exhausted")?;
        let voice = Voice {
            serial: self.serial,
            role,
            path,
            start_ms: position_ms,
            looping,
            playing: true,
        };
        effects.push(Effect::Start(voice.clone()));
        match role {
            Role::Regular => self.regular = Some(voice),
            Role::Thematic => self.thematic = Some(voice),
        }
        Ok(())
    }
    fn start_track(
        &mut self,
        path: String,
        position_ms: u32,
        fade_ms: u32,
        now: u32,
        effects: &mut Vec<Effect>,
    ) -> Result<(), String> {
        self.cancel();
        if position_ms == 0
            && self
                .regular
                .as_ref()
                .is_some_and(|v| v.playing && self.rules.same_path(&v.path, &path))
        {
            return Ok(());
        }
        if !self.enabled {
            Self::retire(&mut self.regular, effects);
            self.pending = Some(path);
            return Ok(());
        }
        if self.regular.is_some() {
            Self::retire(&mut self.thematic, effects);
            Self::retire(&mut self.regular, effects);
        }
        self.start_voice(
            Role::Regular,
            path,
            position_ms,
            self.tracks.len() == 1,
            effects,
        )?;
        self.effective_level = if fade_ms == 0 { self.level } else { 0 };
        if fade_ms != 0 {
            self.arm_fade(false, fade_ms, now);
        }
        self.emit_level(effects);
        Ok(())
    }
    fn emit_level(&self, effects: &mut Vec<Effect>) {
        if let Some(v) = &self.regular {
            effects.push(Effect::Level {
                serial: v.serial,
                level: self.effective_level,
            });
        }
    }
    fn arm_fade(&mut self, out: bool, duration: u32, now: u32) {
        self.fade = Some(Fade {
            duration,
            out,
            start: None,
        });
        self.callbacks.push(Callback {
            due: now.wrapping_add(self.rules.fade_interval_ms),
            kind: CallbackKind::Fade,
        });
    }
    fn stop(&mut self, fade: bool, now: u32, effects: &mut Vec<Effect>) {
        if self.regular.is_none() {
            return;
        }
        self.cancel();
        if !self.regular.as_ref().is_some_and(|v| v.playing) {
            return;
        }
        if fade {
            self.arm_fade(true, self.rules.default_fade_ms, now);
        } else if let Some(v) = &mut self.regular {
            v.playing = false;
            effects.push(Effect::Pause(v.serial));
        }
    }
    fn resume(
        &mut self,
        fade: bool,
        now: u32,
        effects: &mut Vec<Effect>,
        resolved: Option<Vec<String>>,
        work: &mut usize,
    ) -> Result<(), String> {
        if !self.enabled || self.thematic.as_ref().is_some_and(|v| v.playing) {
            return Ok(());
        }
        if let Some(v) = &mut self.regular {
            if v.playing {
                return Ok(());
            }
            v.playing = true;
            effects.push(Effect::Resume(v.serial));
            self.effective_level = if fade { 0 } else { self.level };
            self.emit_level(effects);
            if fade {
                self.arm_fade(false, self.rules.default_fade_ms, now);
            }
        } else if let Some(path) = self.pending.take() {
            self.start_track(path, 0, 0, now, effects)?;
        } else if self.selection.is_some() {
            self.start_selected(resolved, now, work, effects)?;
        }
        Ok(())
    }

    fn start_selected(
        &mut self,
        resolved: Option<Vec<String>>,
        now: u32,
        work: &mut usize,
        effects: &mut Vec<Effect>,
    ) -> Result<(), String> {
        let Some(selection) = &self.selection else {
            return Ok(());
        };
        let tracks = resolved.ok_or("selected regular music playlist is unresolved")?;
        self.validate_tracks(&tracks)?;
        let index = if selection.index < 0 {
            self.random.choose(tracks.len(), work)?
        } else {
            selection.index as usize % tracks.len()
        };
        let path = tracks[index].clone();
        self.tracks = Arc::new(tracks);
        self.index = index;
        self.start_track(path, 0, 0, now, effects)
    }
    fn validate_tracks(&self, tracks: &[String]) -> Result<(), String> {
        if tracks.is_empty()
            || tracks
                .iter()
                .any(|p| p.is_empty() || p.len() >= self.rules.name_bytes || p.contains('\0'))
        {
            return Err("regular music playlist has unsupported paths".into());
        }
        Ok(())
    }

    /// Resolve and validate all selected files before passing the playlist here.
    /// Failed work/admission leaves accepted voice, callbacks and RNG unchanged.
    pub fn apply(
        &mut self,
        op: Operation,
        resolved: Option<Vec<String>>,
        now: u32,
        work: &mut usize,
    ) -> Result<Vec<Effect>, String> {
        charge(work)?;
        let mut next = self.clone();
        let mut effects = Vec::new();
        effects
            .try_reserve(8)
            .map_err(|_| "music effect allocation")?;
        next.apply_inner(op, resolved, now, work, &mut effects)?;
        *self = next;
        Ok(effects)
    }
    fn apply_inner(
        &mut self,
        op: Operation,
        resolved: Option<Vec<String>>,
        now: u32,
        work: &mut usize,
        effects: &mut Vec<Effect>,
    ) -> Result<(), String> {
        match op {
            Operation::Select { name, index } => {
                self.selection = Some(Selection { name, index });
                self.tracks = Arc::new(Vec::new());
            }
            Operation::Clear => {
                self.selection = None;
                self.tracks = Arc::new(Vec::new());
            }
            Operation::StartSelected => {
                if self.regular.is_none() && self.thematic.is_none() {
                    self.start_selected(resolved, now, work, effects)?;
                }
            }
            Operation::Play {
                name,
                position_ms,
                fade_ms,
            } => {
                if fade_ms > i32::MAX as u32 {
                    return Err("unsupported music fade duration".into());
                }
                let tracks = resolved.ok_or("regular music playlist is unresolved")?;
                self.validate_tracks(&tracks)?;
                let index = self.random.choose(tracks.len(), work)?;
                self.selection = Some(Selection {
                    name,
                    index: self.rules.random_index,
                });
                let path = tracks[index].clone();
                self.tracks = Arc::new(tracks);
                self.index = index;
                self.start_track(path, position_ms, fade_ms, now, effects)?;
            }
            Operation::Stop { fade } => self.stop(fade, now, effects),
            Operation::Resume => self.resume(false, now, effects, resolved, work)?,
            Operation::Seek(position_ms) => {
                if let Some(v) = &mut self.regular {
                    v.start_ms = position_ms;
                    effects.push(Effect::Seek {
                        serial: v.serial,
                        position_ms,
                    });
                }
            }
            Operation::Theme { path, position_ms } => {
                Self::retire(&mut self.thematic, effects);
                self.start_voice(Role::Thematic, path, position_ms, false, effects)?;
                self.stop(true, now, effects);
            }
            Operation::EndTheme => {
                Self::retire(&mut self.thematic, effects);
                self.resume(true, now, effects, resolved, work)?;
            }
            Operation::ThemeSeek(position_ms) => {
                if let Some(v) = &mut self.thematic {
                    v.start_ms = position_ms;
                    effects.push(Effect::Seek {
                        serial: v.serial,
                        position_ms,
                    });
                }
            }
            Operation::Volume(level) => {
                if level > self.rules.maximum_level {
                    return Err("unsupported regular music level".into());
                }
                self.level = level;
                self.effective_level = level;
                self.emit_level(effects);
            }
            Operation::Enabled(enabled) => {
                self.enabled = enabled;
                if enabled {
                    self.resume(false, now, effects, resolved, work)?;
                } else {
                    self.stop(false, now, effects);
                }
            }
        }
        Ok(())
    }

    pub fn pump(&mut self, now: u32, work: &mut usize) -> Result<Vec<Effect>, String> {
        let mut next = self.clone();
        let mut effects = Vec::new();
        effects
            .try_reserve(8)
            .map_err(|_| "music callback effect allocation")?;
        loop {
            let index = next
                .callbacks
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.due.wrapping_sub(b.due).cast_signed().cmp(&0))
                .map(|(i, _)| i);
            let Some(index) = index else {
                break;
            };
            if !due(next.callbacks[index].due, now) {
                break;
            }
            charge(work)?;
            let callback = next.callbacks.swap_remove(index);
            match callback.kind {
                CallbackKind::Next => {
                    if let Some(path) = next.pending.take() {
                        next.start_track(path, 0, 0, now, &mut effects)?;
                    }
                }
                CallbackKind::Fade => {
                    if let Some(mut fade) = next.fade.take()
                        && next.regular.as_ref().is_some_and(|v| v.playing)
                    {
                        if let Some(start) = fade.start {
                            let elapsed = now.wrapping_sub(start);
                            if elapsed > fade.duration {
                                if fade.out {
                                    next.stop(false, now, &mut effects);
                                } else {
                                    next.effective_level = next.level;
                                    next.emit_level(&mut effects);
                                }
                                continue;
                            }
                            let progress = ((elapsed as f32 / fade.duration as f32)
                                * next.level as f32)
                                .trunc() as u32;
                            next.effective_level = if fade.out {
                                next.level - progress
                            } else {
                                progress
                            };
                            next.emit_level(&mut effects);
                        } else {
                            fade.start = Some(now);
                        }
                        next.fade = Some(fade);
                        next.callbacks.push(Callback {
                            due: now.wrapping_add(next.rules.fade_interval_ms),
                            kind: CallbackKind::Fade,
                        });
                    }
                }
            }
        }
        *self = next;
        Ok(effects)
    }

    pub fn finished(
        &mut self,
        role: Role,
        serial: u64,
        now: u32,
        work: &mut usize,
        resolved: Option<Vec<String>>,
    ) -> Result<Vec<Effect>, String> {
        charge(work)?;
        let mut next = self.clone();
        let mut effects = Vec::new();
        match role {
            Role::Thematic if next.thematic.as_ref().is_some_and(|v| v.serial == serial) => {
                Self::retire(&mut next.thematic, &mut effects);
                next.resume(true, now, &mut effects, resolved, work)?;
            }
            Role::Regular
                if next
                    .regular
                    .as_ref()
                    .is_some_and(|v| v.serial == serial && !v.looping && v.playing) =>
            {
                if let Some(v) = &mut next.regular {
                    v.playing = false;
                }
                if next.tracks.len() > 1 {
                    next.index = (next.index + 1) % next.tracks.len();
                    next.pending = Some(next.tracks[next.index].clone());
                    next.callbacks.push(Callback {
                        due: now.wrapping_add(next.rules.next_interval_ms),
                        kind: CallbackKind::Next,
                    });
                }
            }
            _ => {}
        }
        *self = next;
        Ok(effects)
    }

    pub fn retire_session(&mut self) -> Vec<Effect> {
        let mut effects = Vec::new();
        self.cancel();
        Self::retire(&mut self.regular, &mut effects);
        Self::retire(&mut self.thematic, &mut effects);
        self.selection = None;
        self.tracks = Arc::new(Vec::new());
        effects
    }
}

#[cfg(test)]
#[path = "regular_music_controller_tests.rs"]
mod tests;
