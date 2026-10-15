//! Original TFT menu music and GlueScreenClick feedback.
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
#[cfg(all(test, not(target_arch = "wasm32")))]
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;

pub(crate) const MENU_MUSIC_PATH: &str = r"Sound\Music\mp3Music\War3XMainScreen.mp3";

pub fn install(app: &mut App) {
    #[cfg(not(target_arch = "wasm32"))]
    app.init_resource::<MenuSounds>().add_systems(Startup, load);
    app.add_systems(
        Update,
        feedback
            .before(crate::launch_gui::LauncherInput)
            .before(crate::intro::IntroUpdate),
    );
    #[cfg(not(target_arch = "wasm32"))]
    app.add_systems(
        Update,
        (
            pause_for_movie.after(crate::intro::IntroUpdate),
            apply_user_volume,
        ),
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
    click_volume: f32,
    no_duplicates: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
pub(crate) struct AuthoredVolume(pub(crate) f32);

#[cfg(not(target_arch = "wasm32"))]
fn apply_user_volume(
    settings: Res<crate::menu_settings::Backend>,
    mut sinks: Query<(&mut AudioSink, &AuthoredVolume, Has<MenuMusic>)>,
) {
    use bevy::audio::AudioSinkPlayback;
    for (mut sink, base, music) in &mut sinks {
        let gain = if music {
            settings.audio.effective_music()
        } else {
            settings.audio.effective_effects()
        };
        sink.set_volume(bevy::audio::Volume::Linear(base.0 * gain));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn click_descriptor(bytes: &[u8]) -> Result<(String, f32, bool), String> {
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
    let volume = row
        .by_name("Volume")
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|v| v.is_finite() && (0.0..=127.0).contains(v))
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
        volume / 127.0,
        duplicates,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct ClickSound;

#[cfg(not(target_arch = "wasm32"))]
#[derive(Component)]
struct MenuMusic;

#[cfg(not(target_arch = "wasm32"))]
fn pause_for_movie(intro: Res<crate::intro::Intro>, music: Query<&AudioSink, With<MenuMusic>>) {
    use bevy::audio::AudioSinkPlayback;
    for sink in &music {
        if intro.playing() {
            sink.pause();
        } else {
            sink.play();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn local_audio(name: &str) -> Option<Vec<u8>> {
    let next_to_exe = std::env::current_exe()
        .ok()?
        .parent()?
        .join("assets")
        .join(name);
    std::fs::read(next_to_exe)
        .or_else(|_| std::fs::read(format!("web/assets/{name}")))
        .ok()
}

#[cfg(not(target_arch = "wasm32"))]
fn load(
    mut commands: Commands,
    mut assets: ResMut<Assets<AudioSource>>,
    mut sounds: ResMut<MenuSounds>,
) {
    let music = local_audio("menu-music.mp3").or_else(|| {
        let install = pcx_gfx::viewer::resolve_install(None);
        pcx_mpq::MpqSet::open_wc3(&install)
            .ok()?
            .extract(MENU_MUSIC_PATH)
            .ok()
    });
    if let Some(bytes) = music {
        commands.spawn((
            AudioPlayer(assets.add(AudioSource {
                bytes: bytes.into(),
            })),
            PlaybackSettings::LOOP,
            MenuMusic,
            AuthoredVolume(1.0),
        ));
    } else {
        warn!(
            "Original menu music unavailable; supply a TFT installation or assets/menu-music.mp3 beside the executable"
        );
    }
    let install = pcx_gfx::viewer::resolve_install(None);
    let click = crate::mainmenu::art_bytes(Some(&install), r"UI\SoundInfo\UISounds.slk")
        .ok_or_else(|| "UISounds.slk is unavailable".to_string())
        .and_then(|bytes| click_descriptor(&bytes));
    let (path, volume, no_duplicates) = match click {
        Ok(descriptor) => descriptor,
        Err(error) => {
            warn!("Original menu click settings unavailable: {error}");
            return;
        }
    };
    sounds.click_volume = volume;
    sounds.no_duplicates = no_duplicates;
    sounds.click = crate::mainmenu::art_bytes(Some(&install), &path)
        .or_else(|| local_audio("menu-click.wav"))
        .map(|bytes| {
            assets.add(AudioSource {
                bytes: bytes.into(),
            })
        });
    if sounds.click.is_none() {
        warn!(
            "Original menu click unavailable; extract BigButtonClick.wav into assets/menu-click.wav beside the executable"
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn feedback(
    intro: Res<crate::intro::Intro>,
    mut commands: Commands,
    sounds: Res<MenuSounds>,
    active: Query<(), With<ClickSound>>,
    actions: Res<crate::button_actions::ButtonActions>,
    enabled: Query<(), (With<Button>, Without<InteractionDisabled>)>,
    settings: Res<crate::menu_settings::Backend>,
) {
    if intro.playing()
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

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    #[test]
    fn disabled_and_held_clicks_do_not_repeat_feedback() {
        let mut world = World::new();
        world.init_resource::<crate::intro::Intro>();
        world.init_resource::<crate::menu_settings::Backend>();
        world.insert_resource(MenuSounds {
            click: Some(Handle::default()),
            click_volume: 1.0,
            no_duplicates: true,
        });
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
