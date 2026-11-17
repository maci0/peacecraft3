//! Retained music sinks; a staged decoder must become ready before accepted voices change.
use crate::menu_audio::{AuthoredVolume, CampaignMusic, RegularMusicGain};
use bevy::{
    audio::{AudioSinkPlayback, Volume},
    prelude::*,
};
use pcx_play::{
    playercolor_host::{
        MusicBackendMode, MusicLoopRules, MusicRole, RegularMusicController, RegularMusicEffect,
    },
    scripts::{AssetIdentity, ScriptAsset},
};
use std::{collections::BTreeMap, time::Duration};

const TRACK_BYTES: usize = 64 << 20;
pub(crate) type TrackReader<'a> =
    dyn Fn(&str, u64) -> Result<(Vec<u8>, AssetIdentity), String> + 'a;

#[derive(Asset, TypePath, Clone)]
pub(crate) struct LoopSource(pub AudioSource);

impl bevy::audio::Decodable for LoopSource {
    type Decoder = Box<dyn bevy::audio::Source + Send>;

    fn decoder(&self) -> Self::Decoder {
        use bevy::audio::Decodable;
        Box::new(
            <AudioSource as Decodable>::Decoder::builder()
                .with_byte_len(self.0.bytes.len() as u64)
                .with_data(std::io::Cursor::new(self.0.clone()))
                .build_looped()
                .expect("loop decoder passed synchronous preflight"),
        )
    }
}

pub(crate) fn install_sources(app: &mut App) {
    use bevy::audio::AddAudioSource;
    if app.is_plugin_added::<bevy::audio::AudioPlugin>() {
        app.add_audio_source::<LoopSource>();
    } else {
        app.init_resource::<Assets<LoopSource>>();
    }
}

pub(crate) fn loop_seekable(source: &AudioSource, position_ms: u32) -> Result<(), String> {
    use bevy::audio::{Decodable, Source};
    let mut decoder = <AudioSource as Decodable>::Decoder::builder()
        .with_byte_len(source.bytes.len() as u64)
        .with_data(std::io::Cursor::new(source.clone()))
        .build_looped()
        .map_err(|error| format!("loop music decoder: {error}"))?;
    decoder
        .try_seek(Duration::from_millis(u64::from(position_ms)))
        .map_err(|error| format!("loop music seek: {error}"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bus {
    Campaign,
    Menu,
}

pub(crate) struct Inputs<'a> {
    pub gain: &'a RegularMusicGain,
    pub loops: Option<&'a MusicLoopRules>,
    pub user: f32,
    pub read: &'a TrackReader<'a>,
}

struct Voice {
    entity: Entity,
    source: AudioSource,
    origin: AssetIdentity,
    ready: bool,
}

struct Pending {
    next: RegularMusicController,
    effects: Vec<RegularMusicEffect>,
    staged: BTreeMap<u64, Voice>,
    sequence: Option<u64>,
    gains: BTreeMap<u64, RegularMusicGain>,
    levels: BTreeMap<u64, f32>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Progress {
    Waiting,
    AwaitingBackend { sequence: Option<u64> },
    Accepted(Option<u64>),
}

pub(crate) struct Playback {
    pub controller: RegularMusicController,
    generation: Option<[u8; 32]>,
    voices: BTreeMap<u64, Voice>,
    pending: Option<Pending>,
    pub fault: Option<String>,
    pub waiting_for_backend: bool,
    bus: Bus,
}

pub(crate) use crate::menu_audio::seekable;

pub(crate) fn checked_source(
    path: &str,
    bytes: Vec<u8>,
    origin: &AssetIdentity,
) -> Result<AudioSource, String> {
    // Validate every encoded packet with the existing bounded file-sound reader first.
    pcx_play::file_sounds::Assets::read_encoded(
        &[pcx_play::file_sounds::SoundInput {
            member: path,
            asset: ScriptAsset {
                label: path,
                bytes: &bytes,
            },
            origin: origin.clone(),
        }],
        pcx_play::file_sounds::Budget {
            resources: 1,
            resource_bytes: TRACK_BYTES,
            total_bytes: TRACK_BYTES,
            metadata_bytes: 1 << 20,
        },
        pcx_play::file_sounds::Mp3Budget {
            max_input_allocation_bytes: TRACK_BYTES + pcx_play::mp3_frame::DECODER_RING_BYTES,
            max_decoder_steps: 1 << 22,
            max_packets: 1 << 18,
            max_frames: 128 << 20,
            max_samples: 256 << 20,
        },
    )
    .map_err(|error| format!("music {path}: {error}"))?;
    Ok(AudioSource {
        bytes: bytes.into(),
    })
}

impl Playback {
    pub fn new(controller: RegularMusicController, generation: [u8; 32]) -> Self {
        Self {
            controller,
            generation: Some(generation),
            voices: BTreeMap::new(),
            pending: None,
            fault: None,
            waiting_for_backend: false,
            bus: Bus::Campaign,
        }
    }

    pub fn menu(
        controller: RegularMusicController,
        generation: [u8; 32],
        entity: Entity,
        source: AudioSource,
        origin: AssetIdentity,
    ) -> Self {
        let serial = controller
            .regular()
            .expect("menu publisher prepared a regular voice")
            .serial;
        let mut playback = Self::new(controller, generation);
        playback.bus = Bus::Menu;
        playback.voices.insert(
            serial,
            Voice {
                entity,
                source,
                origin,
                ready: false,
            },
        );
        playback
    }

    fn asset_present(world: &World, entity: Entity) -> bool {
        if let Some(player) = world.get::<AudioPlayer<LoopSource>>(entity) {
            return world
                .get_resource::<Assets<LoopSource>>()
                .is_some_and(|assets| assets.contains(player.0.id()));
        }
        world.get::<AudioPlayer>(entity).is_some_and(|player| {
            world
                .resource::<Assets<AudioSource>>()
                .contains(player.0.id())
        })
    }

    pub fn backend_ready(&mut self, world: &mut World) -> Result<bool, String> {
        if let Some(error) = &self.fault {
            return Err(error.clone());
        }
        let mut waiting = false;
        let mut failure = None;
        for voice in self.voices.values_mut() {
            if !Self::asset_present(world, voice.entity) {
                failure = Some(format!(
                    "retained music source disappeared: {}",
                    voice.origin.source
                ));
                break;
            }
            if let Some(sink) = world.get::<AudioSink>(voice.entity) {
                if world.get::<AudioPlayer<LoopSource>>(voice.entity).is_some() && sink.empty() {
                    failure = Some("infinite music decoder ended unexpectedly".into());
                    break;
                }
                voice.ready = true;
            } else if voice.ready {
                failure = Some("retained music sink disappeared".into());
                break;
            } else {
                waiting = true;
            }
        }
        if let Some(error) = failure {
            return self.fail(world, error).map(|_| false);
        }
        self.waiting_for_backend = waiting;
        Ok(!waiting)
    }

    /// An explicit source boundary retires pending and live voices, preserving audio draws.
    pub fn source(&mut self, world: &mut World, generation: Option<[u8; 32]>) {
        if self.generation == generation {
            return;
        }
        self.retire(world);
        self.generation = generation;
    }

    pub fn retire(&mut self, world: &mut World) {
        if let Some(pending) = self.pending.take() {
            for voice in pending.staged.values() {
                world.despawn(voice.entity);
            }
        }
        for voice in self.voices.values() {
            world.despawn(voice.entity);
        }
        self.voices.clear();
        self.controller.retire_session();
        self.fault = None;
        self.waiting_for_backend = false;
    }

    /// Synchronous preflight precedes staging and the reservation of accepted audio draws.
    pub fn stage(
        &mut self,
        world: &mut World,
        next: RegularMusicController,
        effects: Vec<RegularMusicEffect>,
        sequence: Option<u64>,
        inputs: Inputs<'_>,
    ) -> Result<(), String> {
        let Inputs {
            gain,
            loops,
            user,
            read,
        } = inputs;
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        if self.generation.is_none() {
            return Err("music source generation is absent".into());
        }
        if self.pending.is_some() {
            return Err("music sink publication is still pending".into());
        }
        if !user.is_finite() || user < 0.0 {
            return Err("unsupported thematic user gain".into());
        }
        let mut sources = Vec::new();
        let mut gains = BTreeMap::new();
        for effect in &effects {
            match effect {
                RegularMusicEffect::Start(voice) => {
                    if voice.looping {
                        let loops = loops.ok_or("looping music backend is not qualified")?;
                        if loops.image_sha256 != gain.request.rules.image_sha256
                            || loops.backend_sha256 != gain.request.rules.backend_sha256
                        {
                            return Err("looping music policies use different sources".into());
                        }
                        let (mode, count) = next.regular_loop_parameters();
                        if voice.role != MusicRole::Regular
                            || !loops.builtin_looping(mode, count, MusicBackendMode::Builtin)?
                        {
                            return Err(
                                "music loop predicate does not match the staged voice".into()
                            );
                        }
                    }
                    let (bytes, origin) = read(&voice.path, TRACK_BYTES as u64)?;
                    let source = checked_source(&voice.path, bytes, &origin)?;
                    if voice.looping {
                        loop_seekable(&source, voice.start_ms)?;
                    } else {
                        seekable(&source, voice.start_ms)?;
                    }
                    sources.push((voice.clone(), source, origin));
                }
                RegularMusicEffect::Seek {
                    serial,
                    position_ms,
                } => {
                    let voice = self
                        .voices
                        .get(serial)
                        .ok_or("music seek voice is absent")?;
                    if world.get::<AudioPlayer<LoopSource>>(voice.entity).is_some() {
                        loop_seekable(&voice.source, *position_ms)?;
                    } else {
                        seekable(&voice.source, *position_ms)?;
                    }
                }
                RegularMusicEffect::Level { serial, level } => {
                    if self.bus == Bus::Menu {
                        continue;
                    }
                    let mut applied = gain.clone();
                    applied.request.level = *level;
                    applied.with_user_gain(user)?;
                    gains.insert(*serial, applied);
                }
                _ => {}
            }
        }
        let mut levels = gains
            .iter()
            .map(|(serial, gain)| gain.with_user_gain(user).map(|level| (*serial, level)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        if self.bus == Bus::Menu {
            for effect in &effects {
                if let RegularMusicEffect::Level { serial, .. } = effect {
                    levels.insert(*serial, user);
                }
            }
        }
        let mut staged = BTreeMap::new();
        for (voice, source, origin) in sources {
            let volume = levels.get(&voice.serial).copied().unwrap_or(user);
            let settings = PlaybackSettings::ONCE
                .paused()
                .with_volume(Volume::Linear(volume));
            let entity = if voice.looping {
                let handle = world
                    .resource_mut::<Assets<LoopSource>>()
                    .add(LoopSource(source.clone()));
                world.spawn((AudioPlayer(handle), settings)).id()
            } else {
                let handle = world
                    .resource_mut::<Assets<AudioSource>>()
                    .add(source.clone());
                world.spawn((AudioPlayer(handle), settings)).id()
            };
            staged.insert(
                voice.serial,
                Voice {
                    entity,
                    source,
                    origin,
                    ready: false,
                },
            );
        }
        // Reserve accepted music draws now; pending logical state must never restore an old stream.
        self.controller.random = next.random.clone();
        self.pending = Some(Pending {
            next,
            effects,
            staged,
            sequence,
            gains,
            levels,
        });
        Ok(())
    }

    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn refresh_gains(
        &mut self,
        world: &mut World,
        group: f32,
        user: f32,
    ) -> Result<(), String> {
        if !user.is_finite() || user < 0.0 {
            return Err("unsupported music user gain".into());
        }
        let updated = |gain: &RegularMusicGain| {
            let mut gain = gain.clone();
            gain.group = group;
            let level = gain.with_user_gain(user)?;
            Ok::<_, String>((gain, level))
        };
        let live = (self.bus == Bus::Campaign)
            .then_some(&self.controller)
            .and_then(|controller| controller.regular())
            .and_then(|voice| self.voices.get(&voice.serial))
            .map(|voice| {
                let gain = world
                    .get::<RegularMusicGain>(voice.entity)
                    .ok_or("retained regular music gain metadata disappeared")?;
                updated(gain).map(|(gain, _)| (voice.entity, gain))
            })
            .transpose()?;
        let pending_gains = self
            .pending
            .as_ref()
            .map(|pending| {
                pending
                    .gains
                    .iter()
                    .map(|(serial, gain)| updated(gain).map(|value| (*serial, value)))
                    .collect::<Result<BTreeMap<_, _>, _>>()
            })
            .transpose()?;
        if let Some(pending) = &self.pending {
            for effect in &pending.effects {
                if let RegularMusicEffect::Start(voice) = effect
                    && self.bus == Bus::Campaign
                    && voice.role == MusicRole::Regular
                    && !pending_gains
                        .as_ref()
                        .is_some_and(|gains| gains.contains_key(&voice.serial))
                {
                    return Err("staged regular music gain is absent".into());
                }
            }
        }
        if let Some((entity, gain)) = live {
            world.entity_mut(entity).insert(gain);
        }
        if let Some(pending) = &mut self.pending {
            if self.bus == Bus::Menu {
                pending.levels.values_mut().for_each(|level| *level = user);
            }
            for (serial, (gain, level)) in pending_gains.unwrap() {
                pending.gains.insert(serial, gain);
                pending.levels.insert(serial, level);
            }
            for effect in &pending.effects {
                if let RegularMusicEffect::Start(voice) = effect {
                    let level = if voice.role == MusicRole::Regular && self.bus == Bus::Campaign {
                        pending.levels[&voice.serial]
                    } else {
                        user
                    };
                    if let Some(staged) = pending.staged.get(&voice.serial) {
                        if let Some(mut settings) = world.get_mut::<PlaybackSettings>(staged.entity)
                        {
                            settings.volume = Volume::Linear(level);
                        }
                        if let Some(mut sink) = world.get_mut::<AudioSink>(staged.entity) {
                            sink.set_volume(Volume::Linear(level));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn completed(&self, world: &World) -> Option<(MusicRole, u64)> {
        [self.controller.thematic(), self.controller.regular()]
            .into_iter()
            .flatten()
            .find(|voice| {
                voice.playing
                    && !voice.looping
                    && self
                        .voices
                        .get(&voice.serial)
                        .and_then(|live| world.get::<AudioSink>(live.entity))
                        .is_some_and(AudioSinkPlayback::empty)
            })
            .map(|voice| (voice.role, voice.serial))
    }

    pub fn finish(&mut self, world: &mut World) -> Result<Progress, String> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        if !self.backend_ready(world)? {
            return Ok(Progress::AwaitingBackend {
                sequence: self.pending.as_ref().and_then(|pending| pending.sequence),
            });
        }
        let Some(pending) = &self.pending else {
            return Ok(Progress::Waiting);
        };
        for voice in pending.staged.values() {
            if !Self::asset_present(world, voice.entity) {
                return self.fail(
                    world,
                    format!("staged music source disappeared: {}", voice.origin.source),
                );
            }
            if world.get::<AudioSink>(voice.entity).is_none() {
                self.waiting_for_backend = true;
                return Ok(Progress::AwaitingBackend {
                    sequence: pending.sequence,
                });
            }
        }
        for effect in &pending.effects {
            let serial = match effect {
                RegularMusicEffect::Start(_) => continue,
                RegularMusicEffect::Retire(serial)
                | RegularMusicEffect::Pause(serial)
                | RegularMusicEffect::Resume(serial)
                | RegularMusicEffect::Seek { serial, .. }
                | RegularMusicEffect::Level { serial, .. } => serial,
            };
            let voice = pending
                .staged
                .get(serial)
                .or_else(|| self.voices.get(serial));
            if voice.is_none_or(|voice| {
                world.get::<AudioSink>(voice.entity).is_none()
                    || world.get::<PlaybackSettings>(voice.entity).is_none()
            }) {
                return self.fail(world, "retained music sink disappeared".into());
            }
        }
        // Newly staged sinks remain paused while all fallible backend seeks run.
        for effect in &pending.effects {
            let seek = match effect {
                RegularMusicEffect::Start(voice) => Some((voice.serial, voice.start_ms)),
                RegularMusicEffect::Seek {
                    serial,
                    position_ms,
                } => Some((*serial, *position_ms)),
                _ => None,
            };
            if let Some((serial, position_ms)) = seek {
                let voice = pending
                    .staged
                    .get(&serial)
                    .or_else(|| self.voices.get(&serial));
                let result = voice
                    .and_then(|voice| world.get::<AudioSink>(voice.entity))
                    .ok_or_else(|| "music seek sink disappeared".to_owned())
                    .and_then(|sink| {
                        sink.try_seek(Duration::from_millis(u64::from(position_ms)))
                            .map_err(|e| format!("music backend seek: {e}"))
                    });
                if let Err(error) = result {
                    return self.fail(world, error);
                }
            }
        }
        self.waiting_for_backend = false;
        let mut pending = self.pending.take().unwrap();
        for effect in &pending.effects {
            match effect {
                RegularMusicEffect::Start(voice) => {
                    let mut staged = pending.staged.remove(&voice.serial).unwrap();
                    staged.ready = true;
                    let mut entity = world.entity_mut(staged.entity);
                    entity.insert(AuthoredVolume(1.0));
                    if self.bus == Bus::Menu {
                        entity.insert(crate::menu_audio::MenuMusic);
                    } else {
                        entity.insert(CampaignMusic {
                            member: voice.path.clone(),
                        });
                    }
                    if voice.role == MusicRole::Regular
                        && self.bus == Bus::Campaign
                        && let Some(gain) = pending.gains.get(&voice.serial)
                    {
                        entity.insert(gain.clone());
                    }
                    let paused = self.bus == Bus::Menu && crate::menu_audio::menu_paused(world);
                    world
                        .get_mut::<PlaybackSettings>(staged.entity)
                        .unwrap()
                        .paused = paused;
                    let sink = world.get::<AudioSink>(staged.entity).unwrap();
                    if paused {
                        sink.pause();
                    } else {
                        sink.play();
                    }
                    self.voices.insert(voice.serial, staged);
                }
                RegularMusicEffect::Retire(serial) => {
                    if let Some(voice) = self.voices.remove(serial) {
                        world.despawn(voice.entity);
                    }
                }
                RegularMusicEffect::Pause(serial) | RegularMusicEffect::Resume(serial) => {
                    if let Some(voice) = self.voices.get(serial) {
                        let paused = matches!(effect, RegularMusicEffect::Pause(_));
                        world
                            .get_mut::<PlaybackSettings>(voice.entity)
                            .unwrap()
                            .paused = paused;
                        let sink = world.get::<AudioSink>(voice.entity).unwrap();
                        if paused {
                            sink.pause();
                        } else {
                            sink.play();
                        }
                    }
                }
                RegularMusicEffect::Level { serial, .. } => {
                    if let Some(voice) = self.voices.get(serial) {
                        if let Some(gain) = pending.gains.get(serial) {
                            world.entity_mut(voice.entity).insert(gain.clone());
                        }
                        let level = Volume::Linear(pending.levels[serial]);
                        world
                            .get_mut::<PlaybackSettings>(voice.entity)
                            .unwrap()
                            .volume = level;
                        world
                            .get_mut::<AudioSink>(voice.entity)
                            .unwrap()
                            .set_volume(level);
                    }
                }
                RegularMusicEffect::Seek { .. } => {}
            }
        }
        pending.next.random = self.controller.random.clone();
        self.controller = pending.next;
        Ok(Progress::Accepted(pending.sequence))
    }

    fn fail(&mut self, world: &mut World, error: String) -> Result<Progress, String> {
        if let Some(pending) = self.pending.take() {
            for voice in pending.staged.values() {
                world.despawn(voice.entity);
            }
        }
        // Explicit source/session retirement is the recovery boundary; do not replay partial work.
        self.waiting_for_backend = false;
        self.fault = Some(error.clone());
        Err(error)
    }
}

#[cfg(test)]
#[path = "regular_music_playback_tests.rs"]
mod tests;
