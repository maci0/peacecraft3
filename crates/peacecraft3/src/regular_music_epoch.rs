//! One native presentation audio epoch, retained across map sessions.
use crate::{
    campaign_audio::{self, Bank},
    music_wall::Sample,
    regular_music_playback::{Inputs, Playback, Progress, TrackReader},
};
use bevy::prelude::*;
use pcx_play::{
    client_module::ClientSources,
    map_startup_host::MapStartupHost,
    playercolor_host::{
        MusicLoopRules, RegularMusicController, RegularMusicOperation as Operation,
        RegularMusicRules, RegularMusicVolumeRules,
    },
};
use peacecraft3::{
    browser_configuration::SelectedMapSources, map_requests::MapRequests,
    map_script_session::MapScriptSession,
};
use std::sync::{Arc, Weak};

#[derive(Clone)]
pub(crate) struct SourceFrame {
    pub revision: u64,
    pub bundle: Arc<ClientSources>,
    pub edition: i32,
}

struct Policy {
    frame: SourceFrame,
    rules: Arc<RegularMusicRules>,
    volume: Arc<RegularMusicVolumeRules>,
    loops: MusicLoopRules,
}

impl Policy {
    fn read(frame: SourceFrame) -> Result<Self, String> {
        let (origin, main) = frame.bundle.main();
        let module = frame
            .bundle
            .module(frame.bundle.demand(), 4 << 20)?
            .ok_or("music backend module is absent")?;
        let rules = Arc::new(RegularMusicRules::read(main, origin.clone())?);
        let volume = Arc::new(RegularMusicVolumeRules::read(
            main,
            &module.bytes,
            module.origin.clone(),
        )?);
        let loops = MusicLoopRules::read(main, &module.bytes)?;
        Ok(Self {
            frame,
            rules,
            volume,
            loops,
        })
    }
}

#[derive(Resource, Default)]
pub(crate) struct Epoch {
    pub playback: Option<Playback>,
    pub(crate) menu: Option<Playback>,
    policy: Option<Policy>,
    session: Option<Weak<()>>,
    bank: Option<Bank>,
    pub menu_dirty: bool,
    pub menu_attempted: bool,
    pub last_error: Option<String>,
    pub observed: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MusicSourceUpdate;

impl Epoch {
    /// Observe absence/errors before comparing stamps; an unchanged revision is not attestation.
    pub fn observe(
        &mut self,
        world: &mut World,
        frame: Option<Result<SourceFrame, String>>,
        wall: Sample,
    ) -> Result<(), String> {
        self.observed = true;
        let frame = match frame {
            Some(Ok(frame)) => frame,
            None => {
                return self.unavailable(world, "music client source attestation is absent".into());
            }
            Some(Err(error)) => return self.unavailable(world, error),
        };
        if self.policy.as_ref().is_some_and(|old| {
            old.frame.revision == frame.revision
                && old.frame.edition == frame.edition
                && Arc::ptr_eq(&old.frame.bundle, &frame.bundle)
                && old.frame.bundle.generation() == frame.bundle.generation()
        }) {
            return Ok(());
        }
        let policy = match Policy::read(frame) {
            Ok(policy) => policy,
            Err(error) => return self.unavailable(world, error),
        };
        let controller = match self.playback.as_ref() {
            Some(playback) => match playback.controller.next_epoch(Arc::clone(&policy.rules)) {
                Ok(controller) => controller,
                Err(error) => return self.unavailable(world, error),
            },
            None => RegularMusicController::new(Arc::clone(&policy.rules), wall.milliseconds),
        };
        // Publication revision changes retire voices even when content is identical.
        if let Some(playback) = &mut self.playback {
            playback.retire(world);
            playback.source(world, Some(policy.frame.bundle.generation()));
            playback.controller = controller;
        } else {
            self.playback = Some(Playback::new(controller, policy.frame.bundle.generation()));
        }
        self.retire_menu(world);
        self.policy = Some(policy);
        self.bank = None;
        self.session = None;
        self.menu_dirty = true;
        self.menu_attempted = false;
        self.last_error = None;
        debug!("music source epoch uses {} wall input", wall.origin);
        Ok(())
    }

    fn retire_menu(&mut self, world: &mut World) {
        if let Some(mut menu) = self.menu.take() {
            menu.retire(world);
        }
        crate::menu_audio::retire_source_audio(world);
    }

    fn unavailable(&mut self, world: &mut World, error: String) -> Result<(), String> {
        self.observed = true;
        if let Some(playback) = &mut self.playback {
            playback.source(world, None);
        }
        self.retire_menu(world);
        self.policy = None;
        self.bank = None;
        self.session = None;
        self.menu_dirty = true;
        self.last_error = Some(error.clone());
        Err(error)
    }

    pub fn rules_for(&self, bundle: &Arc<ClientSources>) -> Option<Arc<RegularMusicRules>> {
        let policy = self.policy.as_ref()?;
        Arc::ptr_eq(&policy.frame.bundle, bundle).then(|| Arc::clone(&policy.rules))
    }

    pub fn menu_choice(
        &mut self,
        skin: &crate::menu_skin::SkinTable,
        read: &TrackReader<'_>,
        publish: impl FnOnce(
            String,
            AudioSource,
            pcx_play::scripts::AssetIdentity,
            bool,
        ) -> Result<Entity, String>,
    ) -> Result<(), String> {
        if self.menu.is_some() {
            return Err("menu music is already published for this epoch".into());
        }
        let policy = self
            .policy
            .as_ref()
            .ok_or("menu music source is not qualified")?;
        let (name, mode, index, position, fade) = policy.rules.menu_start();
        let key = policy.rules.edition_key(name, policy.frame.edition)?;
        let text = skin
            .unique_file(&key)
            .map_err(|error| error.to_string())?
            .file;
        let tracks = policy.rules.tracks(&text)?;
        if position != 0 || fade != 0 || mode != policy.rules.ordinary_mode() {
            return Err("menu music requires an unsupported offset, fade or numeric mode".into());
        }
        let looping = policy.loops.builtin_looping(
            mode,
            tracks.len(),
            pcx_play::playercolor_host::MusicBackendMode::Builtin,
        )?;
        let playback = self.playback.as_ref().ok_or("menu audio epoch is absent")?;
        let mut controller = playback.controller.next_epoch(Arc::clone(&policy.rules))?;
        controller.apply(
            Operation::Select {
                name: name.into(),
                index,
            },
            None,
            0,
            &mut 1024,
        )?;
        controller.apply(Operation::StartSelected, Some(tracks), 0, &mut 1024)?;
        let voice = controller
            .regular()
            .ok_or("menu controller produced no voice")?;
        if voice.looping != looping {
            return Err("menu loop policies disagree".into());
        }
        let path = voice.path.clone();
        let (bytes, origin) = read(&path, 64 << 20)?;
        let source = crate::regular_music_playback::checked_source(&path, bytes, &origin)?;
        if looping {
            crate::regular_music_playback::loop_seekable(&source, position)?;
        } else {
            crate::regular_music_playback::seekable(&source, position)?;
        }
        let entity = publish(path, source.clone(), origin.clone(), looping)?;
        self.playback.as_mut().unwrap().controller.random = controller.random.clone();
        self.menu = Some(Playback::menu(
            controller,
            policy.frame.bundle.generation(),
            entity,
            source,
            origin,
        ));
        self.menu_dirty = false;
        Ok(())
    }

    pub(crate) fn drive_menu(&mut self, world: &mut World, now: u32) -> Result<(), String> {
        let Some(menu) = &mut self.menu else {
            return Ok(());
        };
        let policy = self
            .policy
            .as_ref()
            .ok_or("menu music source epoch is unavailable")?;
        menu.controller.random = self
            .playback
            .as_ref()
            .ok_or("audio random epoch is unavailable")?
            .controller
            .random
            .clone();
        let user = world
            .resource::<crate::menu_settings::Backend>()
            .audio
            .effective_music();
        menu.refresh_gains(world, 1.0, user)?;
        if !menu.backend_ready(world)? {
            return Err("menu music backend sink is pending or unavailable".into());
        }
        if menu.has_pending() {
            return match menu.finish(world)? {
                Progress::Accepted(_) | Progress::Waiting => Ok(()),
                Progress::AwaitingBackend { .. } => {
                    Err("menu music backend sink is pending or unavailable".into())
                }
            };
        }
        let inactive = crate::menu_audio::menu_paused(world);
        if inactive {
            return Ok(());
        }
        let mut next = menu.controller.clone();
        let mut work = 4096;
        let effects = if let Some((role, serial)) = menu.completed(world) {
            next.finished(role, serial, now, &mut work, None)?
        } else {
            next.pump(now, &mut work)?
        };
        if effects.is_empty() && work == 4096 {
            return Ok(());
        }
        let reader = world
            .resource::<crate::native_campaign::Chapters>()
            .audio_sources(world.resource::<crate::launch_gui::Launcher>())
            .ok_or("menu music source publication is absent")??
            .1;
        let gain = crate::menu_audio::RegularMusicGain {
            request: pcx_play::playercolor_host::RegularMusicVolumeRequest {
                rules: Arc::clone(&policy.volume),
                level: policy.volume.initial_level(),
                revision: 0,
            },
            group: 1.0,
        };
        let staged = menu.stage(
            world,
            next,
            effects,
            None,
            Inputs {
                gain: &gain,
                loops: Some(&policy.loops),
                user,
                read: &|path, limit| {
                    let asset = reader
                        .resolve_bounded(path, limit)
                        .map_err(|error| error.to_string())?;
                    let (_, origin) = pcx_play::campaign_assets::identity(
                        &asset,
                        &format!("version enumerand {}", policy.frame.edition),
                        16 << 10,
                    )?;
                    Ok((asset.bytes, origin))
                },
            },
        );
        if let Err(error) = staged {
            menu.fault = Some(error.clone());
            return Err(error);
        }
        match menu.finish(world)? {
            Progress::Accepted(_) | Progress::Waiting => Ok(()),
            Progress::AwaitingBackend { .. } => {
                Err("menu music backend sink is pending or unavailable".into())
            }
        }
    }

    pub fn random_for(
        &mut self,
        bundle: &Arc<ClientSources>,
        producer_rules: Option<&Arc<RegularMusicRules>>,
    ) -> Result<&mut pcx_play::playercolor_host::AudioRandom, String> {
        let Some(rules) = self.rules_for(bundle) else {
            return Err("acknowledgement audio source generation is not qualified".into());
        };
        if !producer_rules.is_some_and(|producer| Arc::ptr_eq(producer, &rules)) {
            return Err("acknowledgement producer belongs to a retired audio epoch".into());
        }
        Ok(&mut self
            .playback
            .as_mut()
            .ok_or("audio epoch is absent")?
            .controller
            .random)
    }

    fn bank(&mut self, world: &mut World) -> Result<(), String> {
        if self.bank.is_some() {
            return Ok(());
        }
        let bank = world.resource_scope(
            |world, mut session: Mut<MapScriptSession<MapStartupHost>>| {
                let reader = campaign_audio::published_reader(
                    world.resource::<MapRequests>(),
                    world.resource::<SelectedMapSources>(),
                    &session,
                )
                .ok_or("music source reader is not published")?;
                let race = session
                    .host()
                    .ok_or("music host is absent")?
                    .objects()
                    .local_race_preference_constant()
                    .map_err(|error| error.to_string())?;
                let maximum = campaign_audio::volume_maximum(&session);
                let seed = world
                    .resource::<peacecraft3::campaign_startup::CampaignSeed>()
                    .prepared
                    .ok_or("music campaign seed is absent")?;
                campaign_audio::prepare_bank(reader, Ok(race), seed, maximum)
            },
        )?;
        self.bank = Some(bank);
        Ok(())
    }
    pub(crate) fn drive(&mut self, world: &mut World, now: u32) -> Result<(), String> {
        let witness = world
            .resource::<MapScriptSession<MapStartupHost>>()
            .session_witness();
        let same = match (&self.session, &witness) {
            (Some(a), Some(b)) => Weak::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            if let Some(playback) = &mut self.playback {
                playback.retire(world);
            }
            self.session = witness.clone();
            self.bank = None;
        }
        if witness.is_none() {
            return Ok(());
        }
        let sources = world.resource::<SelectedMapSources>();
        let bundle = world
            .resource::<MapRequests>()
            .published_receipts
            .as_ref()
            .and_then(|receipts| sources.client_sources_for_receipts(receipts))
            .ok_or("music source publication is absent")?;
        if self.rules_for(&bundle).is_none() {
            return Err("music host belongs to a retired source publication".into());
        }
        let state =
            world.resource_scope(|_, mut session: Mut<MapScriptSession<MapStartupHost>>| {
                let Some(host) = session.host() else {
                    return Ok(None);
                };
                let objects = host.objects();
                let Some(commands) = objects.regular_music_commands() else {
                    return Ok(None);
                };
                let policy = self
                    .policy
                    .as_ref()
                    .ok_or("music source epoch is unavailable")?;
                if !objects
                    .regular_music_rules()
                    .is_some_and(|rules| Arc::ptr_eq(rules, &policy.rules))
                {
                    return Err("music producer belongs to a retired audio epoch".into());
                }
                let volume = objects
                    .regular_music_volume_request()
                    .ok_or("music volume policy is absent")?
                    .clone();
                let group = objects.volume_group_channel_gain(volume.rules.channel())?;
                Ok::<_, String>(Some((
                    commands.front().cloned(),
                    crate::menu_audio::RegularMusicGain {
                        request: volume,
                        group,
                    },
                )))
            })?;
        let Some((command, gain)) = state else {
            return Ok(());
        };
        let policy = self
            .policy
            .as_ref()
            .ok_or("music source epoch is unavailable")?;
        if gain.request.rules.image_sha256 != policy.volume.image_sha256
            || gain.request.rules.backend_sha256 != policy.volume.backend_sha256
        {
            return Err("music source/backend policies disagree".into());
        }
        let user = world
            .resource::<crate::menu_settings::Backend>()
            .audio
            .effective_music();
        let playback = self
            .playback
            .as_mut()
            .ok_or("music controller is unavailable")?;
        if !same {
            playback.controller.apply(
                Operation::Volume(gain.request.level),
                None,
                now,
                &mut 4096,
            )?;
        }
        playback.refresh_gains(world, gain.group, user)?;
        if !playback.backend_ready(world)? {
            return Err(
                "music backend sink is pending or unavailable; command remains unacknowledged"
                    .into(),
            );
        }
        if playback.has_pending() {
            return match playback.finish(world)? {
                Progress::Accepted(Some(sequence)) => {
                    world
                        .resource_mut::<MapScriptSession<MapStartupHost>>()
                        .host()
                        .ok_or("music host retired before acceptance")?
                        .objects_mut()
                        .acknowledge_regular_music(sequence)
                        .map_err(|error| error.to_string())?;
                    self.last_error = None;
                    Ok(())
                }
                Progress::Accepted(None) => {
                    self.last_error = None;
                    Ok(())
                }
                Progress::AwaitingBackend { .. } => Err(
                    "music backend sink is pending or unavailable; command remains unacknowledged"
                        .into(),
                ),
                Progress::Waiting => Ok(()),
            };
        }
        self.bank(world)?;
        let policy = self.policy.as_ref().unwrap();
        let bank = self.bank.as_ref().unwrap();
        let playback = self.playback.as_mut().unwrap();
        let mut next = playback.controller.clone();
        let mut work = 4096;
        // A retained ONCE entity reports EOF exactly once through the controller's playing state.
        let finished = playback.completed(world);
        let (effects, sequence) = if let Some((role, serial)) = finished {
            let resolved = next
                .selection_name()
                .map(|name| bank.regular_playlist(&policy.rules, policy.frame.edition, name))
                .transpose()?;
            (next.finished(role, serial, now, &mut work, resolved)?, None)
        } else {
            let before = work;
            let callbacks = next.pump(now, &mut work)?;
            if !callbacks.is_empty() || before != work {
                (callbacks, None)
            } else if let Some(command) = command {
                let name = match &command.operation {
                    Operation::Play { name, .. } => Some(name.as_str()),
                    Operation::Resume | Operation::EndTheme | Operation::StartSelected => {
                        next.selection_name()
                    }
                    _ => None,
                };
                let resolved = name
                    .map(|name| bank.regular_playlist(&policy.rules, policy.frame.edition, name))
                    .transpose()?;
                (
                    next.apply(command.operation, resolved, now, &mut work)?,
                    Some(command.sequence),
                )
            } else if next.regular().is_none()
                && next.thematic().is_none()
                && next.selection_name().is_some()
            {
                let resolved = next
                    .selection_name()
                    .map(|name| bank.regular_playlist(&policy.rules, policy.frame.edition, name))
                    .transpose()?;
                (
                    next.apply(Operation::StartSelected, resolved, now, &mut work)?,
                    None,
                )
            } else {
                return Ok(());
            }
        };
        let staged = playback.stage(
            world,
            next,
            effects,
            sequence,
            Inputs {
                gain: &gain,
                loops: Some(&policy.loops),
                user,
                read: &|path, limit| bank.music_source(path, limit, policy.frame.edition),
            },
        );
        if let Err(error) = staged {
            playback.fault = Some(error.clone());
            return Err(error);
        }
        // Logical/pause/gain-only operations need no asynchronous source publication.
        match playback.finish(world)? {
            Progress::Accepted(Some(sequence)) => {
                world
                    .resource_mut::<MapScriptSession<MapStartupHost>>()
                    .host()
                    .ok_or("music host retired before acceptance")?
                    .objects_mut()
                    .acknowledge_regular_music(sequence)
                    .map_err(|error| error.to_string())?;
                self.last_error = None;
            }
            Progress::Accepted(None) => {
                self.last_error = None;
            }
            Progress::AwaitingBackend { .. } => {
                return Err(
                    "music backend sink is pending or unavailable; command remains unacknowledged"
                        .into(),
                );
            }
            Progress::Waiting => {}
        }
        Ok(())
    }
}

pub(crate) fn install(app: &mut App) {
    crate::regular_music_playback::install_sources(app);
    app.init_resource::<Epoch>()
        .add_systems(
            Update,
            observe_sources
                .in_set(MusicSourceUpdate)
                .after(crate::menu_settings::SettingsUpdate)
                .before(crate::menu_audio::SourceAudioUpdate)
                .before(crate::campaign_prepare::CampaignPrepareSet),
        )
        .add_systems(
            Update,
            drive
                .in_set(campaign_audio::CampaignAudioSet)
                .after(peacecraft3::campaign_startup::CampaignStartupSet)
                .after(campaign_audio::drive_music)
                .after(crate::menu_audio::SourceAudioUpdate)
                .after(peacecraft3::campaign_camera::CampaignCameraSet)
                .after(crate::intro::IntroUpdate)
                .after(crate::launch_gui::MenuActions)
                .after(peacecraft3::script_audio::ScriptAudioPlayback)
                .before(crate::menu_audio::pause_for_movie)
                .before(crate::menu_audio::apply_user_volume),
        );
}

fn observe_sources(world: &mut World) {
    let Some(launcher) = world.get_resource::<crate::launch_gui::Launcher>() else {
        return;
    };
    let frame = world
        .get_resource::<crate::native_campaign::Chapters>()
        .and_then(|chapters| chapters.audio_sources(launcher))
        .map(|result| result.map(|(frame, _)| frame));
    let wall = match crate::music_wall::sample() {
        Ok(wall) => wall,
        Err(error) => {
            world.resource_scope(|world, mut epoch: Mut<Epoch>| {
                let _ = epoch.unavailable(world, error);
            });
            return;
        }
    };
    world.resource_scope(|world, mut epoch: Mut<Epoch>| {
        if let Err(error) = epoch.observe(world, frame, wall)
            && let Some(mut audio) = world.get_resource_mut::<campaign_audio::CampaignAudio>()
        {
            audio.report(error);
        }
        let pending = world
            .query_filtered::<Option<&AudioSink>, With<crate::menu_audio::MenuMusic>>()
            .iter(world)
            .any(|sink| sink.is_none());
        let message = "menu music backend sink is pending or unavailable";
        if pending {
            epoch.last_error = Some(message.into());
            if let Some(mut audio) = world.get_resource_mut::<campaign_audio::CampaignAudio>() {
                audio.report(message.into());
            }
        } else if epoch.last_error.as_deref() == Some(message) {
            epoch.last_error = None;
        }
    });
}

fn drive(world: &mut World) {
    let now = match crate::music_wall::sample() {
        Ok(wall) => wall.milliseconds,
        Err(error) => {
            world
                .resource_mut::<campaign_audio::CampaignAudio>()
                .report(error);
            return;
        }
    };
    world.resource_scope(|world, mut epoch: Mut<Epoch>| {
        let menu = epoch.drive_menu(world, now);
        let regular = epoch.drive(world, now);
        if let Some(error) = regular.err().or(menu.err()) {
            epoch.last_error = Some(error.clone());
            world
                .resource_mut::<campaign_audio::CampaignAudio>()
                .report(error);
        } else if epoch.policy.is_some() {
            epoch.last_error = None;
            if let Some(mut audio) = world.get_resource_mut::<campaign_audio::CampaignAudio>()
                && matches!(audio.last_error.as_deref(), Some("music backend sink is pending or unavailable; command remains unacknowledged" | "menu music backend sink is pending or unavailable")) {
                audio.last_error = None;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    #[ignore = "requires selected client/backend, unchanged Human01 and verified private null audio"]
    fn native_initial_required_wall_failure_refuses_acknowledgement_before_random_or_voice() {
        crate::campaign_audio_probe::assert_null_audio_environment();
        let mut app = crate::native_campaign::tests::started_audio_app();
        app.world_mut()
            .resource_scope(|world, mut epoch: Mut<Epoch>| {
                if let Some(playback) = &mut epoch.playback {
                    playback.retire(world);
                }
                epoch.retire_menu(world);
                *epoch = Epoch::default();
                assert!(!epoch.observed && epoch.playback.is_none());
                assert!(
                    epoch
                        .unavailable(world, "fixture required music wall is unavailable".into())
                        .is_err()
                );
                assert!(epoch.observed && epoch.policy.is_none() && epoch.playback.is_none());
            });
        crate::campaign_audio_probe::begin(&mut app);
        crate::campaign_audio_probe::select_hero(&mut app);
        app.world_mut()
            .run_system_once(crate::campaign_ack::unit_voices)
            .unwrap();
        assert!(
            app.world().resource::<Epoch>().playback.is_none(),
            "a failed required clock cannot initialize audio random state"
        );
        assert!(
            app.world()
                .resource::<campaign_audio::CampaignAudio>()
                .last_error
                .as_deref()
                .unwrap()
                .contains("source generation is not qualified")
        );
        let world = app.world_mut();
        assert_eq!(
            world
                .query::<&campaign_audio::GameSound>()
                .iter(world)
                .count(),
            0
        );
    }
}
