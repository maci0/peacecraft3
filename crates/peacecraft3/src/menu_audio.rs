//! Original TFT menu music and GlueScreenClick feedback.
use bevy::prelude::*;
use bevy::ui::{InteractionDisabled, Pressed};
use bevy::ui_widgets::Button;

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
    app.add_systems(Update, pause_for_movie.after(crate::intro::IntroUpdate));
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
            .extract(r"Sound\Music\mp3Music\War3XMainScreen.mp3")
            .ok()
    });
    if let Some(bytes) = music {
        commands.spawn((
            AudioPlayer(assets.add(AudioSource {
                bytes: bytes.into(),
            })),
            PlaybackSettings::LOOP,
            MenuMusic,
        ));
    } else {
        warn!(
            "Original menu music unavailable; supply a TFT installation or assets/menu-music.mp3 beside the executable"
        );
    }
    sounds.click = local_audio("menu-click.wav").map(|bytes| {
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
) {
    // UISounds.slk: GlueScreenClick has volume 100/127 and NODUPLICATES.
    if intro.playing()
        || !actions
            .entities
            .iter()
            .any(|entity| enabled.contains(*entity))
        || !active.is_empty()
    {
        return;
    }
    if let Some(handle) = &sounds.click {
        commands.spawn((
            AudioPlayer(handle.clone()),
            PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(100.0 / 127.0)),
            ClickSound,
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
        world.insert_resource(MenuSounds {
            click: Some(Handle::default()),
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
