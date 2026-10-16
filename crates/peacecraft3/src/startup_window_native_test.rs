use super::*;
use bevy::{ecs::system::NonSendMarker, window::PrimaryWindow};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Resource)]
struct Gate {
    seed: (u32, u32),
    saved: bool,
    started: Instant,
    result: Arc<Mutex<Option<serde_json::Value>>>,
}

fn observe(
    gate: Res<Gate>,
    windows: Query<(Entity, &Window, &bevy::window::OnMonitor), With<PrimaryWindow>>,
    monitors: Query<&bevy::window::Monitor>,
    mut exit: MessageWriter<AppExit>,
    _main_thread: NonSendMarker,
) {
    // Execution watchdog only; acceptance compares exact physical dimensions.
    assert!(gate.started.elapsed() < Duration::from_secs(30));
    let Ok((entity, window, on_monitor)) = windows.single() else {
        return;
    };
    let Ok(monitor) = monitors.get(on_monitor.0) else {
        return;
    };
    let expected = if gate.saved {
        gate.seed
    } else {
        fitted_physical_client(monitor.physical_width, monitor.physical_height)
            .expect("nonempty host monitor")
    };
    let actual = bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        windows.get_window(entity).map(|window| {
            let size = window.inner_size();
            (size.width, size.height)
        })
    });
    if actual != Some(expected) || (window.physical_width(), window.physical_height()) != expected {
        return;
    }
    let result = serde_json::json!({
        "ok": true,
        "saved_resolution": gate.saved,
        "seed": gate.seed,
        "monitor": [monitor.physical_width, monitor.physical_height],
        "expected": expected,
        "native_client": actual,
        "scope": "Actual Winit monitor and client dimensions; no preference files or game assets read"
    });
    println!("PCX_STARTUP_WINDOW {result}");
    *gate.result.lock().unwrap() = Some(result);
    exit.write(AppExit::Success);
}

#[test]
#[ignore = "requires a live desktop; run each PCX_STARTUP_WINDOW_GATE mode in a fresh process"]
fn actual_native_startup_size_respects_saved_resolution() {
    let mode =
        std::env::var("PCX_STARTUP_WINDOW_GATE").expect("explicit live-window mode required");
    assert!(matches!(mode.as_str(), "audio_only" | "saved_size"));
    let saved = mode == "saved_size";
    let mut backend = crate::menu_settings::Backend::default();
    backend.set_test_saved_resolution(saved);
    let mut window = crate::launch_gui::window_plugin(None);
    let primary = window.primary_window.as_mut().unwrap();
    primary.title = "PeaceCraft startup window check".into();
    let seed = (primary.physical_width(), primary.physical_height());
    let result = Arc::new(Mutex::new(None));
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        bevy::image::ImagePlugin::default(),
        bevy::a11y::AccessibilityPlugin,
        bevy::input_focus::InputFocusPlugin,
        window,
        bevy::input::InputPlugin,
        bevy::winit::WinitPlugin {
            run_on_any_thread: true,
        },
    ));
    app.insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(backend)
        .insert_resource(Gate {
            seed,
            saved,
            started: Instant::now(),
            result: result.clone(),
        });
    install(&mut app);
    app.add_systems(Update, observe.after(fit_primary_window));
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
