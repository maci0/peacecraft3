//! Selected source menu music and GlueScreenClick feedback.
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
#[cfg(all(test, not(target_arch = "wasm32")))]
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;

#[cfg(target_arch = "wasm32")]
pub(crate) const MENU_MUSIC_PATH: &str = r"Sound\Music\mp3Music\War3XMainScreen.mp3";

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SourceAudioUpdate;

pub fn install(app: &mut App) {
    #[cfg(not(target_arch = "wasm32"))]
    app.init_resource::<MenuSounds>().add_systems(
        Update,
        load.in_set(SourceAudioUpdate)
            .after(crate::menu_settings::SettingsUpdate)
            .before(crate::launch_gui::LauncherInput),
    );
    app.add_systems(
        Update,
        feedback
            .run_if(crate::button_actions::input_allowed)
            .after(crate::button_actions::discard_blocked)
            .after(crate::menu_settings::SettingsUpdate)
            .after(SourceAudioUpdate)
            .before(crate::launch_gui::LauncherInput)
            .before(crate::intro::IntroUpdate),
    );
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        Update,
        pause_for_movie
            .after(peacecraft3::campaign_camera::CampaignCameraSet)
            .before(apply_user_volume)
            .after(crate::intro::IntroUpdate)
            .after(crate::launch_gui::MenuActions)
            .after(peacecraft3::script_audio::ScriptAudioPlayback),
    );
    app.add_systems(
        Update,
        apply_user_volume
            .after(crate::campaign_audio::CampaignAudioSet)
            .after(crate::menu_settings::SettingsUpdate)
            .after(crate::intro::IntroUpdate)
            .after(crate::launch_gui::MenuActions)
            .after(peacecraft3::script_audio::ScriptAudioPlayback),
    );
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = peacecraft3Audio, js_name = click)]
    fn browser_click();
}

#[cfg(target_arch = "wasm32")]
fn feedback(
    intro: Res<crate::intro::Intro>,
    actions: Res<crate::button_actions::ButtonActions>,
    enabled: Query<(), (With<Button>, Without<InteractionDisabled>)>,
) {
    if !intro.playing()
        && actions
            .entities
            .iter()
            .any(|entity| enabled.contains(*entity))
    {
        browser_click();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Default)]
struct MenuSounds {
    click: Option<Handle<AudioSource>>,
    music_origin: Option<(String, pcx_play::scripts::AssetIdentity)>,
    click_volume: f32,
    no_duplicates: bool,
    attempted: Option<(String, crate::launch::GameMode, bool)>,
}

#[derive(Component)]
pub(crate) struct AuthoredVolume(pub(crate) f32);

#[derive(Component, Clone)]
pub(crate) struct RegularMusicGain {
    pub(crate) request: pcx_play::playercolor_host::RegularMusicVolumeRequest,
    pub(crate) group: f32,
}
impl RegularMusicGain {
    pub(crate) fn with_user_gain(&self, user: f32) -> Result<f32, String> {
        self.request
            .rules
            .gain(self.request.level, self.group, user)
    }
}

type UserVolumeQuery<'w, 's> = Query<
    'w,
    's,
    (
        Option<&'static mut AudioSink>,
        &'static mut PlaybackSettings,
        Option<&'static AuthoredVolume>,
        Option<&'static peacecraft3::script_audio::ScriptAudioGain>,
        Option<&'static peacecraft3::script_audio::ScriptAudioCategory>,
        Option<&'static RegularMusicGain>,
        Has<MenuMusic>,
        Has<CampaignMusic>,
    ),
    Or<(
        With<AuthoredVolume>,
        With<peacecraft3::script_audio::ScriptVoice>,
    )>,
>;

pub(crate) fn apply_user_volume(
    settings: Res<crate::menu_settings::Backend>,
    mut sinks: UserVolumeQuery,
) {
    use bevy::audio::AudioSinkPlayback;
    for (sink, mut playback, base, script_gain, category, regular, menu_music, campaign_music) in
        &mut sinks
    {
        let gain = if menu_music || campaign_music {
            settings.audio.effective_music()
        } else {
            settings.audio.effective_effects()
        };
        let authored = base.map_or(1.0, |base| base.0) * script_gain.map_or(1.0, |gain| gain.0);
        let value = if let Some(regular) = regular {
            match regular.with_user_gain(gain) {
                Ok(value) => value,
                Err(error) => {
                    warn!("regular music gain: {error}");
                    continue;
                }
            }
        } else {
            category.map_or(authored * gain, |category| {
                category.with_user_gain(authored, gain)
            })
        };
        playback.volume = bevy::audio::Volume::Linear(value);
        if let Some(mut sink) = sink {
            sink.set_volume(playback.volume);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn click_descriptor(bytes: &[u8], maximum: f32) -> Result<(String, f32, bool), String> {
    let table = pcx_slq::Table::parse_bounded(
        bytes,
        pcx_slq::CoordinateLimits {
            max_row: 100_000,
            max_column: 1024,
        },
    )
    .map_err(|e| e.to_string())?;
    let rows: Vec<_> = table
        .data_rows()
        .filter(|r| r.by_name("SoundName") == Some("GlueScreenClick"))
        .collect();
    let [row] = rows.as_slice() else {
        return Err("UISounds: expected one GlueScreenClick row".into());
    };
    let directory = row
        .by_name("DirectoryBase")
        .ok_or("UISounds: missing DirectoryBase")?;
    let file = row
        .by_name("FileNames")
        .filter(|s| !s.is_empty() && !s.contains([',', ';']))
        .ok_or("UISounds: missing or multiple click files")?;
    if !maximum.is_finite() || maximum <= 0.0 {
        return Err("UISounds: invalid source sound-volume scale".into());
    }
    let volume = row
        .by_name("Volume")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|v| v.is_finite() && (0.0..=maximum).contains(v))
        .ok_or("UISounds: invalid Volume")?;
    if row.by_name("Pitch") != Some("1") {
        return Err("UISounds: unsupported click Pitch".into());
    }
    let duplicates = match row.by_name("Flags") {
        Some("NODUPLICATES") => true,
        Some("0") => false,
        _ => return Err("UISounds: unsupported click Flags".into()),
    };
    Ok((
        format!(
            "{}{file}",
            if directory.is_empty() {
                String::new()
            } else {
                format!("{}\\", directory.trim_end_matches(['\\', '/']))
            }
        ),
        volume / maximum,
        duplicates,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct ClickSound;

#[derive(Component)]
pub(crate) struct MenuMusic;

/// One campaign music track on the user's music volume. It keeps playing under the
/// campaign camera, unlike [`MenuMusic`].
#[derive(Component)]
pub(crate) struct CampaignMusic {
    pub member: String,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn pause_for_movie(
    intro: Res<crate::intro::Intro>,
    campaign: Option<Res<peacecraft3::campaign_camera::CampaignCamera>>,
    mut music: Query<(&mut PlaybackSettings, Option<&AudioSink>), With<MenuMusic>>,
) {
    sync_menu_music(
        intro.playing(),
        campaign.as_ref().is_some_and(|camera| camera.active()),
        &mut music,
    );
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn menu_paused(world: &World) -> bool {
    world
        .get_resource::<crate::intro::Intro>()
        .is_some_and(|intro| intro.playing())
        || world
            .get_resource::<peacecraft3::campaign_camera::CampaignCamera>()
            .is_some_and(|camera| camera.active())
}

#[cfg(not(target_arch = "wasm32"))]
fn sync_menu_music(
    intro_playing: bool,
    campaign_active: bool,
    music: &mut Query<(&mut PlaybackSettings, Option<&AudioSink>), With<MenuMusic>>,
) {
    use bevy::audio::AudioSinkPlayback;
    for (mut playback, sink) in music.iter_mut() {
        playback.paused = intro_playing || campaign_active;
        if let Some(sink) = sink {
            if playback.paused {
                sink.pause();
            } else {
                sink.play();
            }
        }
    }
}

/// Edition-to-skin-key translation is recreation glue; the member path is source data.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn music_path(
    skin: &crate::menu_skin::SkinTable,
    expansion: bool,
) -> Result<String, String> {
    let key = format!("GlueMusic_V{}", u32::from(expansion));
    let path = skin.unique_file(&key).map_err(|e| e.to_string())?.file;
    let mut entries = path.split(';');
    let first = entries
        .next()
        .filter(|entry| !entry.is_empty() && !entry.contains(','))
        .ok_or_else(|| format!("{key}: empty or unsupported music entry"))?;
    if entries.any(|entry| entry != first) {
        return Err(format!(
            "{key}: distinct-entry music playlists are not supported"
        ));
    }
    // Repeating one authored file is equivalent to the existing menu loop.
    Ok(first.to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
fn member(set: &mut pcx_mpq::MpqSet, path: &str, limit: usize) -> Result<Vec<u8>, String> {
    let (_, block) = set.locate(path).ok_or_else(|| format!("missing {path}"))?;
    if block.compressed_size as u64 > limit as u64 || block.file_size as u64 > limit as u64 {
        return Err(format!("{path}: menu-audio input budget exceeded"));
    }
    set.extract(path).map_err(|e| format!("{path}: {e}"))
}

#[cfg(not(target_arch = "wasm32"))]
type MenuVoice = Or<(With<MenuMusic>, With<ClickSound>)>;

#[cfg(not(target_arch = "wasm32"))]
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct NativeAudio<'w, 's> {
    assets: ResMut<'w, Assets<AudioSource>>,
    loop_assets: Option<ResMut<'w, Assets<crate::regular_music_playback::LoopSource>>>,
    sounds: ResMut<'w, MenuSounds>,
    settings: ResMut<'w, crate::menu_settings::Backend>,
    nav: ResMut<'w, crate::menu_screens::Navigation>,
    intro: Res<'w, crate::intro::Intro>,
    epoch: Option<ResMut<'w, crate::regular_music_epoch::Epoch>>,
    chapters: Option<Res<'w, crate::native_campaign::Chapters>>,
    voices: Query<'w, 's, Entity, MenuVoice>,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn load(
    mut commands: Commands,
    launcher: Res<crate::launch_gui::Launcher>,
    state: NativeAudio,
) {
    let NativeAudio {
        mut assets,
        mut loop_assets,
        mut sounds,
        mut settings,
        mut nav,
        voices,
        intro,
        mut epoch,
        chapters,
    } = state;
    if (settings.source_matches(&launcher) && epoch.as_ref().is_none_or(|epoch| !epoch.menu_dirty))
        || !settings.preferences_loaded()
    {
        return;
    }
    let qualified = if let Some(epoch) = epoch.as_deref() {
        let Some(source) = chapters
            .as_ref()
            .and_then(|chapters| chapters.audio_sources(&launcher))
        else {
            return;
        };
        let (frame, reader) = match source {
            Ok(value) => value,
            Err(error) => {
                nav.notice = error;
                return;
            }
        };
        if epoch.rules_for(&frame.bundle).is_none() {
            return;
        }
        Some((reader, frame.edition))
    } else {
        None
    };
    let (source, mode) = launcher.audio_selection();
    if sounds
        .attempted
        .as_ref()
        .is_some_and(|(path, previous, saved)| {
            path == source
                && *previous == mode
                && *saved == settings.has_preferences()
                && epoch
                    .as_ref()
                    .is_none_or(|epoch| !epoch.menu_dirty || epoch.menu_attempted)
        })
    {
        return;
    }
    sounds.attempted = Some((source.to_owned(), mode, settings.has_preferences()));
    if let Some(epoch) = epoch.as_deref_mut() {
        epoch.menu_attempted = true;
    }
    // Resolve and read the selected installation once per changed request, not per frame.
    let install = crate::launch_gui::selected_install(&launcher);
    let expansion = launcher.expansion();
    // Source preparation and preferences precede every entity/voice write. Errors
    // retain existing media and settings; no neighboring installation is borrowed.
    let seed = match crate::menu_settings::source_audio_candidate(
        &settings,
        install.as_deref(),
        expansion,
    ) {
        Ok(seed) => seed,
        Err(error) => {
            nav.notice = format!("Original menu audio defaults: {error}");
            warn!("{}", nav.notice);
            return;
        }
    };
    let mut set = if qualified.is_some() {
        Ok(None)
    } else {
        install.as_ref().map(pcx_mpq::MpqSet::open_wc3).transpose()
    };
    let mut read = |path: &str, limit: usize| -> Result<Vec<u8>, String> {
        if let Some((reader, _)) = &qualified {
            return reader
                .resolve_bounded(path, limit as u64)
                .map(|asset| asset.bytes)
                .map_err(|error| error.to_string());
        }
        let set = set
            .as_mut()
            .map_err(|e| e.to_string())?
            .as_mut()
            .ok_or("No selected MPQ source")?;
        member(set, path, limit)
    };
    enum MusicCandidate {
        Selected(crate::menu_skin::SkinTable),
        Legacy(AudioSource),
    }
    let music = (|| {
        let skin = read(crate::mainmenu::SKIN_TABLE, 8 * 1024 * 1024)?;
        let skin = crate::menu_skin::SkinTable::parse(
            std::str::from_utf8(&skin).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if qualified.is_some() {
            return Ok::<_, String>(MusicCandidate::Selected(skin));
        }
        let path = music_path(&skin, expansion)?;
        Ok(MusicCandidate::Legacy(AudioSource {
            bytes: read(&path, 64 * 1024 * 1024)?.into(),
        }))
    })();
    let click = (|| {
        let script = read(r"Scripts\Blizzard.j", 2 * 1024 * 1024)?;
        let scale = crate::menu_sprites::VolumeScale::checked(&script)?;
        let table = read(r"UI\SoundInfo\UISounds.slk", 8 * 1024 * 1024)?;
        let (path, volume, no_duplicates) = click_descriptor(&table, scale.maximum)?;
        let bytes = read(&path, 16 * 1024 * 1024)?;
        Ok::<_, String>((bytes, volume, no_duplicates))
    })();
    let music = match music {
        Ok(music) => music,
        Err(error) => {
            nav.notice = format!("Selected menu music unavailable: {error}");
            warn!("{}", nav.notice);
            return;
        }
    };
    let click = match click {
        Ok(click) => Some(click),
        Err(error) if sounds.click.is_none() => {
            // Missing/unsupported feedback must not silence otherwise admitted startup music.
            warn!("Selected original menu click unavailable: {error}");
            None
        }
        Err(error) => {
            nav.notice =
                format!("Selected menu click unavailable; previous source retained: {error}");
            warn!("{}", nav.notice);
            return;
        }
    };
    let publish = |music: AudioSource, looping: bool| -> Result<Entity, String> {
        if looping && qualified.is_some() && loop_assets.is_none() {
            return Err("menu loop backend assets are unavailable".into());
        }
        crate::menu_settings::activate_source_audio(&mut nav, &mut settings, seed, &launcher)?;
        // Prepared music, explicit click capability and settings publish together.
        for voice in &voices {
            commands.entity(voice).despawn();
        }
        let playback = (if qualified.is_some() {
            PlaybackSettings::ONCE
        } else {
            PlaybackSettings::LOOP
        })
        .with_volume(bevy::audio::Volume::Linear(
            settings.audio.effective_music(),
        ));
        let playback = if intro.playing() {
            playback.paused()
        } else {
            playback
        };
        let entity = if looping && qualified.is_some() {
            commands
                .spawn((
                    AudioPlayer(
                        loop_assets
                            .as_deref_mut()
                            .unwrap()
                            .add(crate::regular_music_playback::LoopSource(music)),
                    ),
                    playback,
                    MenuMusic,
                    AuthoredVolume(1.0),
                ))
                .id()
        } else {
            commands
                .spawn((
                    AudioPlayer(assets.add(music)),
                    playback,
                    MenuMusic,
                    AuthoredVolume(1.0),
                ))
                .id()
        };
        sounds.click = click.map(|(bytes, volume, no_duplicates)| {
            sounds.click_volume = volume;
            sounds.no_duplicates = no_duplicates;
            assets.add(AudioSource {
                bytes: bytes.into(),
            })
        });
        Ok(entity)
    };
    let result = match music {
        MusicCandidate::Legacy(source) => publish(source, true).map(|_| ()),
        MusicCandidate::Selected(skin) => {
            let (reader, edition) = qualified.as_ref().unwrap();
            let mut receipt = None;
            let result = epoch.as_deref_mut().unwrap().menu_choice(
                &skin,
                &|path, limit| {
                    let asset = reader
                        .resolve_bounded(path, limit)
                        .map_err(|error| error.to_string())?;
                    let (_, origin) = pcx_play::campaign_assets::identity(
                        &asset,
                        &format!("version enumerand {edition}"),
                        16 << 10,
                    )?;
                    Ok((asset.bytes, origin))
                },
                |path, source, origin, looping| {
                    let entity = publish(source, looping)?;
                    receipt = Some((path, origin));
                    Ok(entity)
                },
            );
            if result.is_ok() {
                sounds.music_origin = receipt;
            }
            result
        }
    };
    if let Err(error) = result {
        nav.notice = format!("Selected menu music unavailable: {error}");
        warn!("{}", nav.notice);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(bevy::ecs::system::SystemParam)]
struct AudioContext<'w> {
    launcher: Res<'w, crate::launch_gui::Launcher>,
    settings: Res<'w, crate::menu_settings::Backend>,
}

#[cfg(not(target_arch = "wasm32"))]
fn feedback(
    intro: Res<crate::intro::Intro>,
    mut commands: Commands,
    sounds: Res<MenuSounds>,
    active: Query<(), With<ClickSound>>,
    actions: Res<crate::button_actions::ButtonActions>,
    enabled: Query<(), (With<Button>, Without<InteractionDisabled>)>,
    context: AudioContext,
) {
    let AudioContext { launcher, settings } = context;
    if !settings.source_matches(&launcher)
        || intro.playing()
        || !actions
            .entities
            .iter()
            .any(|entity| enabled.contains(*entity))
        || (sounds.no_duplicates && !active.is_empty())
    {
        return;
    }
    if let Some(handle) = &sounds.click {
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(
                sounds.click_volume * settings.audio.effective_effects(),
            )),
            ClickSound,
            AuthoredVolume(sounds.click_volume),
        ));
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn retire_source_audio(world: &mut World) {
    let voices: Vec<_> = world
        .query_filtered::<Entity, Or<(
            With<MenuMusic>,
            With<ClickSound>,
            With<crate::campaign_audio::GameSound>,
        )>>()
        .iter(world)
        .collect();
    for entity in voices {
        world.despawn(entity);
    }
    if let Some(mut sounds) = world.get_resource_mut::<MenuSounds>() {
        if let Some((path, origin)) = sounds.music_origin.take() {
            debug!("retiring menu music {path} from {}", origin.source);
        }
        sounds.click = None;
        sounds.attempted = None;
    }
}

pub(crate) fn seekable(source: &AudioSource, milliseconds: u32) -> Result<(), String> {
    use bevy::audio::{Decodable, Source};
    let mut decoder = <AudioSource as Decodable>::Decoder::builder()
        .with_byte_len(source.bytes.len() as u64)
        .with_data(std::io::Cursor::new(source.clone()))
        .build()
        .map_err(|error| format!("audio decoder: {error}"))?;
    decoder
        .try_seek(std::time::Duration::from_millis(u64::from(milliseconds)))
        .map_err(|error| format!("audio decoder seek: {error}"))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn menu_music_uses_the_selected_skin_variant_without_substituting_a_path() {
        let skin = crate::menu_skin::SkinTable::parse(
            "[Default]\nGlueMusic_V0=base.mp3\nGlueMusic_V1=expansion.mp3\n",
        )
        .unwrap();
        assert_eq!(music_path(&skin, false).unwrap(), "base.mp3");
        assert_eq!(music_path(&skin, true).unwrap(), "expansion.mp3");
        let repeated = crate::menu_skin::SkinTable::parse(
            "[Default]\nGlueMusic_V0=repeated.mp3;repeated.mp3\n",
        )
        .unwrap();
        assert_eq!(music_path(&repeated, false).unwrap(), "repeated.mp3");
        for text in [
            "[Default]\nGlueMusic_V0=\n",
            "[Default]\nGlueMusic_V0=a,b\n",
            "[Default]\nGlueMusic_V0=a;b\n",
            "[Default]\nGlueMusic_V0=a\nGlueMusic_V0=b\n",
            "[Default]\nGlueMusic_V1=only-expansion.mp3\n",
        ] {
            assert!(music_path(&crate::menu_skin::SkinTable::parse(text).unwrap(), false).is_err());
        }
    }

    #[test]
    #[ignore = "requires explicitly qualified local original install; component preplayback only"]
    fn selected_original_defaults_reach_first_voice_and_failed_replacement_rolls_back() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_OPTIONS_INSTALL").expect("explicit install"),
        );
        let mut world = World::new();
        world.init_resource::<crate::intro::Intro>();
        world.init_resource::<Assets<AudioSource>>();
        world.init_resource::<MenuSounds>();
        world.init_resource::<crate::menu_screens::Navigation>();
        let mut settings = crate::menu_settings::Backend::default();
        settings.set_test_preferences(false);
        world.insert_resource(settings);
        world.insert_resource(crate::launch_gui::Launcher::test_source(
            install.to_string_lossy().into_owned(),
            true,
        ));
        world
            .resource_mut::<crate::intro::Intro>()
            .request_with_install(true, Some(install.clone()));
        world.run_system_once(load).unwrap();
        let settings = world.resource::<crate::menu_settings::Backend>();
        assert!(
            settings.source_matches(&crate::launch_gui::Launcher::test_source(
                install.to_string_lossy().into_owned(),
                true
            )),
            "{}",
            world.resource::<crate::menu_screens::Navigation>().notice
        );
        assert!(!settings.has_preferences());
        let gain = settings.audio.effective_music();
        let (voice, playback) = world
            .query_filtered::<(Entity, &PlaybackSettings), With<MenuMusic>>()
            .single(&world)
            .unwrap();
        assert_eq!(playback.volume, bevy::audio::Volume::Linear(gain));
        assert!(
            playback.paused,
            "the source music must start paused while the intro owns audio"
        );
        let click = world.resource::<MenuSounds>().click.clone();
        world
            .resource_mut::<crate::menu_screens::Navigation>()
            .draft
            .values
            .insert("MusicVolumeSlider".into(), 17.0);
        let before = world
            .resource::<crate::menu_screens::Navigation>()
            .draft
            .clone();
        world.insert_resource(crate::launch_gui::Launcher::test_source(
            "synthetic-missing-replacement".into(),
            false,
        ));
        world.run_system_once(load).unwrap();
        assert!(world.get_entity(voice).is_ok());
        assert_eq!(
            world.resource::<MenuSounds>().click.as_ref(),
            click.as_ref()
        );
        let settings = world.resource::<crate::menu_settings::Backend>();
        assert_eq!(settings.audio.effective_music().to_bits(), gain.to_bits());
        assert!(
            settings.source_matches(&crate::launch_gui::Launcher::test_source(
                install.to_string_lossy().into_owned(),
                true
            ))
        );
        assert!(!settings.source_matches(world.resource::<crate::launch_gui::Launcher>()));
        assert_eq!(
            world.resource::<crate::menu_screens::Navigation>().draft,
            before
        );
    }

    #[test]
    fn failed_replacement_retains_old_media_audio_and_dirty_draft() {
        let mut world = World::new();
        world.init_resource::<crate::intro::Intro>();
        world.init_resource::<Assets<AudioSource>>();
        let mut settings = crate::menu_settings::Backend::default();
        settings.set_test_source(&crate::launch_gui::Launcher::test_source(
            "synthetic-old-source".into(),
            false,
        ));
        settings.audio.music_gain = 0.25;
        // A valid saved candidate avoids any client/FDF read in this synthetic test.
        settings.set_test_preferences(true);
        world.insert_resource(settings);
        let mut nav = crate::menu_screens::Navigation::default();
        nav.applied.values.insert("MusicVolumeSlider".into(), 30.0);
        nav.applied.values.insert("SoundVolumeSlider".into(), 42.0);
        nav.applied.toggles.insert("MusicCheckBox".into(), true);
        nav.applied.toggles.insert("SoundCheckBox".into(), true);
        nav.draft = nav.applied.clone();
        nav.draft.values.insert("MusicVolumeSlider".into(), 17.0);
        let before_applied = nav.applied.clone();
        let before = nav.draft.clone();
        world.insert_resource(nav);
        world.insert_resource(MenuSounds {
            click: Some(Handle::default()),
            ..default()
        });
        let old_voice = world.spawn(MenuMusic).id();
        // No selected installation: use the test constructor, never ambient originals.
        world.insert_resource(crate::launch_gui::Launcher::test_source(
            "synthetic-missing-install".into(),
            true,
        ));
        world.run_system_once(load).unwrap();
        assert!(world.get_entity(old_voice).is_ok());
        assert!(world.resource::<MenuSounds>().click.is_some());
        assert_eq!(
            world
                .resource::<crate::menu_settings::Backend>()
                .audio
                .music_gain,
            0.25
        );
        assert!(
            world
                .resource::<crate::menu_settings::Backend>()
                .source_matches(&crate::launch_gui::Launcher::test_source(
                    "synthetic-old-source".into(),
                    false
                ))
        );
        assert!(
            !world
                .resource::<crate::menu_settings::Backend>()
                .source_matches(world.resource::<crate::launch_gui::Launcher>())
        );
        assert!(!before_applied.values.is_empty());
        assert_eq!(
            world.resource::<crate::menu_screens::Navigation>().applied,
            before_applied
        );
        assert_eq!(
            world.resource::<crate::menu_screens::Navigation>().draft,
            before
        );
    }

    #[test]
    fn intro_can_run_between_launcher_input_and_menu_actions() {
        fn input() {}
        fn finish() {}
        fn action() {}
        fn intro() {}
        fn initialize(old_group: bool) -> Result<(), bevy::ecs::schedule::ScheduleBuildError> {
            let mut app = App::new();
            install(&mut app);
            app.add_systems(
                Update,
                (input.in_set(crate::launch_gui::LauncherInput), finish).chain(),
            );
            app.add_systems(Update, intro.in_set(crate::intro::IntroUpdate));
            if old_group {
                app.add_systems(
                    Update,
                    action
                        .in_set(crate::launch_gui::LauncherInput)
                        .after(finish),
                );
            } else {
                app.add_systems(
                    Update,
                    action.in_set(crate::launch_gui::MenuActions).after(finish),
                );
            }
            app.configure_sets(
                Update,
                crate::intro::IntroUpdate
                    .after(crate::launch_gui::LauncherInput)
                    .before(finish),
            );
            let mut schedule = app
                .world_mut()
                .resource_mut::<Schedules>()
                .remove(Update)
                .unwrap();
            schedule.initialize(app.world_mut()).map(|_| ())
        }
        assert!(
            initialize(true).is_err(),
            "mixed input/action group must reproduce the startup cycle"
        );
        initialize(false).unwrap();
    }

    #[test]
    fn pending_music_gets_current_gains_and_intro_ownership_before_sink_creation() {
        let mut world = World::new();
        let mut intro = crate::intro::Intro::default();
        intro.request(true);
        world.insert_resource(intro);
        let mut settings = crate::menu_settings::Backend::default();
        settings.audio.music_gain = 0.25;
        world.insert_resource(settings);
        let music = world
            .spawn((MenuMusic, AuthoredVolume(0.6), PlaybackSettings::LOOP))
            .id();
        world.run_system_once(pause_for_movie).unwrap();
        world.run_system_once(apply_user_volume).unwrap();
        let playback = world.get::<PlaybackSettings>(music).unwrap();
        assert!(playback.paused);
        assert_eq!(playback.volume, bevy::audio::Volume::Linear(0.6 * 0.25));
        assert!(world.get::<AudioSink>(music).is_none());
    }

    #[test]
    fn pending_menu_music_obeys_campaign_return_and_intro_ownership() {
        let mut world = World::new();
        let music = world.spawn((MenuMusic, PlaybackSettings::LOOP)).id();
        let other = world.spawn(PlaybackSettings::LOOP).id();
        // Exercise the same playback synchronization used before an AudioSink
        // exists, including returning to the menu while the intro still owns audio.
        for (intro, campaign, expected) in [
            (false, false, false),
            (false, true, true),
            (true, true, true),
            (true, false, true),
            (false, false, false),
            (false, true, true),
            (false, false, false),
        ] {
            world
                .run_system_once(
                    move |mut music: Query<
                        (&mut PlaybackSettings, Option<&AudioSink>),
                        With<MenuMusic>,
                    >| sync_menu_music(intro, campaign, &mut music),
                )
                .unwrap();
            assert_eq!(
                world.get::<PlaybackSettings>(music).unwrap().paused,
                expected
            );
            assert!(!world.get::<PlaybackSettings>(other).unwrap().paused);
            assert!(world.get::<AudioSink>(music).is_none());
        }
    }

    #[test]
    fn disabled_and_held_clicks_do_not_repeat_feedback() {
        let mut world = World::new();
        world.init_resource::<crate::intro::Intro>();
        world.init_resource::<crate::menu_settings::Backend>();
        world.insert_resource(MenuSounds {
            click: Some(Handle::default()),
            click_volume: 1.0,
            no_duplicates: true,
            ..default()
        });
        let launcher =
            crate::launch_gui::Launcher::test_source("synthetic-missing-install".into(), true);
        world
            .resource_mut::<crate::menu_settings::Backend>()
            .set_test_source(&launcher);
        world.insert_resource(launcher);
        let disabled = world.spawn((Button, Pressed, InteractionDisabled)).id();
        world.insert_resource(crate::button_actions::ButtonActions {
            entities: vec![disabled],
        });
        world.run_system_once(feedback).unwrap();
        assert_eq!(
            world
                .query_filtered::<Entity, With<ClickSound>>()
                .iter(&world)
                .count(),
            0
        );
        world.entity_mut(disabled).remove::<InteractionDisabled>();
        world.insert_resource(crate::button_actions::ButtonActions {
            entities: vec![disabled],
        });
        world.run_system_once(feedback).unwrap();
        assert_eq!(
            world
                .query_filtered::<Entity, With<ClickSound>>()
                .iter(&world)
                .count(),
            1
        );
        world.insert_resource(crate::button_actions::ButtonActions {
            entities: vec![disabled],
        });
        world.run_system_once(feedback).unwrap();
        assert_eq!(
            world
                .query_filtered::<Entity, With<ClickSound>>()
                .iter(&world)
                .count(),
            1
        );
    }
}

#[cfg(test)]
mod script_volume_tests {
    use super::*;
    #[test]
    fn script_voices_follow_effects_gain_and_mute_before_sink_creation() {
        use peacecraft3::script_audio::ScriptVoice;
        let mut app = App::new();
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, apply_user_volume);
        #[cfg(target_arch = "wasm32")]
        {
            app.init_resource::<crate::intro::Intro>();
            app.init_resource::<crate::button_actions::ButtonActions>();
            install(&mut app);
        }
        let world = app.world_mut();
        let mut settings = crate::menu_settings::Backend::default();
        settings.audio.effects_enabled = true;
        settings.audio.effects_gain = 0.25;
        world.insert_resource(settings);
        let voice = world
            .spawn((
                PlaybackSettings::LOOP,
                ScriptVoice {
                    session: std::sync::Weak::new(),
                    handle: pcx_script::HandleId::new(std::num::NonZeroU64::new(1).unwrap()),
                    looping: true,
                    fade_in_rate: 0,
                    fade_out_rate: 0,
                    encoded_mp3: false,
                    source: Handle::default(),
                    member: "synthetic.wav".into(),
                    source_label: "synthetic".into(),
                    origin_source: "synthetic".into(),
                    origin_edition: "synthetic".into(),
                    origin_version: "synthetic".into(),
                    sha256: [0; 32],
                },
            ))
            .id();
        let unrelated = world.spawn(PlaybackSettings::ONCE).id();
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.get::<PlaybackSettings>(voice).unwrap().volume,
            bevy::audio::Volume::Linear(0.25)
        );
        world
            .entity_mut(voice)
            .insert(peacecraft3::script_audio::ScriptAudioGain(0.4));
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.get::<PlaybackSettings>(voice).unwrap().volume,
            bevy::audio::Volume::Linear(0.1)
        );
        world
            .resource_mut::<crate::menu_settings::Backend>()
            .audio
            .effects_enabled = false;
        app.update();
        let world = app.world_mut();
        assert_eq!(
            world.get::<PlaybackSettings>(voice).unwrap().volume,
            bevy::audio::Volume::Linear(0.0)
        );
        assert_eq!(
            world.get::<PlaybackSettings>(unrelated).unwrap().volume,
            PlaybackSettings::ONCE.volume
        );
        assert!(world.get::<AudioSink>(voice).is_none());
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen::prelude::wasm_bindgen]
    pub fn browser_script_audio_volume_gate() -> u32 {
        script_voices_follow_effects_gain_and_mute_before_sink_creation();
        1
    }
    #[test]
    #[ignore = "requires explicit PCX_WAR3_EXE and PCX_MUSIC_BACKEND local selected images"]
    #[cfg(not(target_arch = "wasm32"))]
    fn regular_music_pending_gain_applies_user_before_curve_and_leaves_thematic_bus() {
        use bevy::ecs::system::RunSystemOnce;
        use std::sync::Arc;
        let rules = Arc::new(crate::campaign_audio_probe::selected_regular_volume_rules());
        let mut world = World::new();
        let mut settings = crate::menu_settings::Backend::default();
        settings.audio.music_gain = 0.5;
        world.insert_resource(settings);
        let request = pcx_play::playercolor_host::RegularMusicVolumeRequest {
            level: rules.maximum_level(),
            rules: Arc::clone(&rules),
            revision: 1,
        };
        let regular = world
            .spawn((
                CampaignMusic {
                    member: "synthetic-regular.mp3".into(),
                },
                AuthoredVolume(1.0),
                PlaybackSettings::LOOP,
                RegularMusicGain {
                    request,
                    group: 1.0,
                },
            ))
            .id();
        let theme = world
            .spawn((
                CampaignMusic {
                    member: "synthetic-theme.mp3".into(),
                },
                AuthoredVolume(1.0),
                PlaybackSettings::LOOP,
            ))
            .id();
        world.run_system_once(apply_user_volume).unwrap();
        let expected = rules.gain(rules.maximum_level(), 1.0, 0.5).unwrap();
        assert_ne!(expected, 0.5, "regular curve precedes the backend mixer");
        assert_eq!(
            world.get::<PlaybackSettings>(regular).unwrap().volume,
            bevy::audio::Volume::Linear(expected)
        );
        assert_eq!(
            world.get::<PlaybackSettings>(theme).unwrap().volume,
            bevy::audio::Volume::Linear(0.5)
        );
        world
            .get_mut::<RegularMusicGain>(regular)
            .unwrap()
            .request
            .level = 0;
        world.run_system_once(apply_user_volume).unwrap();
        assert_eq!(
            world.get::<PlaybackSettings>(regular).unwrap().volume,
            bevy::audio::Volume::Linear(0.0)
        );
        assert!(!world.get::<PlaybackSettings>(regular).unwrap().paused);
        assert_eq!(
            world.get::<PlaybackSettings>(theme).unwrap().volume,
            bevy::audio::Volume::Linear(0.5)
        );
    }
}
