use super::*;
use std::sync::{Arc, Mutex};

#[derive(Resource)]
struct Gate {
    started: Instant,
    phase: u8,
    preference: std::path::PathBuf,
    previous: Vec<u8>,
    events: Vec<serde_json::Value>,
    saw_restore: bool,
    result: Arc<Mutex<Option<serde_json::Value>>>,
}

fn drive(
    mut gate: ResMut<Gate>,
    mut nav: ResMut<Navigation>,
    mut backend: ResMut<Backend>,
    launcher: Res<crate::launch_gui::Launcher>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
    _main_thread: NonSendMarker,
) {
    assert!(
        gate.started.elapsed() < Duration::from_secs(30),
        "real-window gate watchdog: {}",
        nav.notice
    );
    let Ok((entity, mut window)) = windows.single_mut() else {
        return;
    };
    let Some(observed) = host(entity).filter(|h| h.size.0 > 0 && h.size.1 > 0) else {
        return;
    };
    match gate.phase {
        0 => {
            assert!(
                !gate.preference.exists(),
                "gate requires an empty private preference directory"
            );
            nav.applied.resolution = Some(observed.size);
            nav.draft = nav.applied.clone();
            let target = (613, 419);
            assert_ne!(observed.size, target);
            nav.resolutions
                .push(crate::menu_screens::OfferedResolution::render_size(
                    target.0, target.1,
                ));
            nav.draft.resolution = Some(target);
            nav.draft.toggles.insert("MusicCheckBox".into(), false);
            assert!(matches!(
                apply(entity, &mut nav, &mut window, &mut backend, &launcher),
                Ok(ApplyOutcome::Pending)
            ));
            assert!(!gate.preference.exists());
            assert!(backend.audio.music_enabled);
            assert_eq!(nav.applied.resolution, Some(observed.size));
            gate.events.push(serde_json::json!({"phase":"requested", "before":observed.size,"requested":target,"saved":false}));
            gate.phase = 1;
        }
        1 if backend.resize.is_none() => {
            assert_eq!(observed.size, (613, 419), "{}", nav.notice);
            assert_eq!(nav.applied.resolution, Some(observed.size));
            assert!(!backend.audio.music_enabled);
            gate.previous = std::fs::read(&gate.preference).unwrap();
            let prefs = decode_preferences(std::str::from_utf8(&gate.previous).unwrap()).unwrap();
            assert_eq!(prefs.settings.resolution, Some(observed.size));
            assert!(!prefs.audio.music_enabled);
            gate.events
                .push(serde_json::json!({"phase":"reported_and_saved","reported":observed.size}));

            // A directory at the private final path makes the atomic rename fail.
            std::fs::rename(&gate.preference, gate.preference.with_extension("backup")).unwrap();
            std::fs::create_dir(&gate.preference).unwrap();
            nav.requested = None;
            nav.resolutions
                .push(crate::menu_screens::OfferedResolution::render_size(
                    577, 367,
                ));
            nav.draft.resolution = Some((577, 367));
            nav.draft.toggles.insert("MusicCheckBox".into(), true);
            assert!(matches!(
                apply(entity, &mut nav, &mut window, &mut backend, &launcher),
                Ok(ApplyOutcome::Pending)
            ));
            assert!(!backend.audio.music_enabled);
            gate.phase = 2;
        }
        2 => {
            if backend
                .resize
                .as_ref()
                .is_some_and(|p| matches!(p.phase, Phase::Restore { .. }))
            {
                if !gate.saw_restore {
                    gate.events.push(serde_json::json!({"phase":"save_failed_restore_requested","reported":observed.size}));
                }
                gate.saw_restore = true;
            }
            if backend.resize.is_some() {
                return;
            }
            assert!(gate.saw_restore, "storage refusal was not exercised");
            assert_eq!(observed.size, (613, 419), "{}", nav.notice);
            assert!(!backend.audio.music_enabled);
            assert_eq!(nav.applied.resolution, Some(observed.size));
            assert_eq!(
                nav.draft.resolution,
                Some((577, 367)),
                "dirty candidate must remain retryable"
            );
            assert!(nav.draft.toggles["MusicCheckBox"]);
            assert_eq!(
                std::fs::read(gate.preference.with_extension("backup")).unwrap(),
                gate.previous
            );
            std::fs::remove_dir(&gate.preference).unwrap();
            std::fs::rename(gate.preference.with_extension("backup"), &gate.preference).unwrap();
            gate.events.push(serde_json::json!({"phase":"save_failure_restore_reported","reported":observed.size,"preferences_unchanged":true,"audio_unchanged":true}));

            nav.resolutions
                .push(crate::menu_screens::OfferedResolution::render_size(
                    589, 373,
                ));
            nav.draft.resolution = Some((589, 373));
            assert!(matches!(
                apply(entity, &mut nav, &mut window, &mut backend, &launcher),
                Ok(ApplyOutcome::Pending)
            ));
            assert!(cancel(&mut backend));
            gate.phase = 3;
        }
        3 if backend.resize.is_none() => {
            assert_eq!(observed.size, (613, 419), "{}", nav.notice);
            assert_eq!(std::fs::read(&gate.preference).unwrap(), gate.previous);
            assert!(!backend.audio.music_enabled);
            assert_eq!(nav.draft, nav.applied);
            assert_eq!(nav.requested, Some(crate::menu_screens::Screen::Main));
            gate.events.push(serde_json::json!({"phase":"cancel_restore_reported","reported":observed.size,"preferences_unchanged":true}));

            nav.requested = None;
            nav.resolutions.clear();
            backend.observed_resolution = None;
            nav.draft.toggles.insert("MusicCheckBox".into(), true);
            assert!(matches!(
                apply(entity, &mut nav, &mut window, &mut backend, &launcher),
                Ok(ApplyOutcome::Complete)
            ));
            assert!(backend.resize.is_none());
            assert_eq!(backend.observed_resolution, Some(observed.size));
            assert_eq!(host(entity).unwrap().size, observed.size);
            assert!(nav.resolutions.is_empty());
            let text = std::fs::read_to_string(&gate.preference).unwrap();
            let prefs = decode_preferences(&text).unwrap();
            assert_eq!(prefs.settings.resolution, Some(observed.size));
            assert!(prefs.audio.music_enabled);
            assert!(backend.audio.music_enabled);
            gate.events.push(serde_json::json!({"phase":"confirmed_current_audio_saved_without_offers","reported":observed.size,"resize_requested":false}));
            let result = serde_json::json!({"ok":true,"events":gate.events,
                "session_type":std::env::var("XDG_SESSION_TYPE").ok(),
                "scope":"actual Winit client-size reports and real private storage; no compositor presentation or physical display-mode claim"});
            println!("PCX_PORTABLE_RESIZE {}", result);
            *gate.result.lock().unwrap() = Some(result);
            gate.phase = 4;
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

#[test]
#[ignore = "requires a live desktop and explicit empty private XDG_CONFIG_HOME/PCX_RESIZE_GATE_CONFIG"]
fn actual_client_resize_save_failure_restore_and_cancel() {
    let private =
        std::env::var_os("PCX_RESIZE_GATE_CONFIG").expect("explicit private directory required");
    assert_eq!(std::env::var_os("XDG_CONFIG_HOME"), Some(private.clone()));
    let private = std::path::PathBuf::from(private);
    assert!(private.is_absolute());
    let preference = preference_path().unwrap();
    assert!(preference.starts_with(&private));
    assert!(!preference.exists());
    let result = Arc::new(Mutex::new(None));
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::image::ImagePlugin::default(),
        bevy::a11y::AccessibilityPlugin,
        bevy::input_focus::InputFocusPlugin,
        WindowPlugin {
            primary_window: Some(Window {
                title: "PeaceCraft render-size check".into(),
                resolution: (421u32, 283u32).into(),
                ..default()
            }),
            ..default()
        },
        bevy::input::InputPlugin,
        bevy::winit::WinitPlugin {
            run_on_any_thread: true,
        },
    ));
    app.insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(crate::launch_gui::Launcher::default())
        .insert_resource(Navigation {
            screen: crate::menu_screens::Screen::Options(crate::menu_screens::Page::Video),
            ..default()
        })
        .insert_resource(Backend {
            loaded: true,
            ..default()
        })
        .insert_resource(Gate {
            started: Instant::now(),
            phase: 0,
            preference,
            previous: Vec::new(),
            events: Vec::new(),
            saw_restore: false,
            result: result.clone(),
        });
    install(&mut app);
    app.add_systems(Update, drive.after(SettingsUpdate));
    assert!(matches!(app.run(), AppExit::Success));
    assert_eq!(
        result
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|r| r["ok"].as_bool()),
        Some(true)
    );
}
