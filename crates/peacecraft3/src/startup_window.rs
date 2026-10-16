//! First-run native sizing is a project policy: nine tenths of the host monitor.
//! Explicit saved sizes win. Browser canvas sizing remains owned by page layout.

use bevy::prelude::*;

/// Nine tenths of the reported monitor, so the window keeps a margin.
const MONITOR_FIT_NUMERATOR: u64 = 9;
/// Denominator for [`MONITOR_FIT_NUMERATOR`].
const MONITOR_FIT_DENOMINATOR: u64 = 10;

/// Fits a first native window after settings have loaded.
pub fn install(app: &mut App) {
    app.add_systems(
        Update,
        fit_primary_window
            .after(crate::menu_settings::SettingsUpdate)
            .before(crate::launch_gui::LauncherInput),
    );
}

/// Physical client for a monitor, or `None` when the size cannot be shown.
///
/// Truncation is toward zero. A 1 by 1 monitor becomes 0 and is refused.
pub(crate) fn fitted_physical_client(width: u32, height: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 {
        return None;
    }
    let width = u64::from(width) * MONITOR_FIT_NUMERATOR / MONITOR_FIT_DENOMINATOR;
    let height = u64::from(height) * MONITOR_FIT_NUMERATOR / MONITOR_FIT_DENOMINATOR;
    if width == 0 || height == 0 {
        return None;
    }
    let width = u32::try_from(width).ok()?;
    let height = u32::try_from(height).ok()?;
    Some((width, height))
}

// Wait for settings and a real monitor before changing the seed window.
// Saved sizes, pending Apply, and fullscreen retain their existing owner.
fn fit_primary_window(
    mut done: Local<bool>,
    backend: Option<Res<crate::menu_settings::Backend>>,
    mut windows: Query<
        (&mut Window, Option<&bevy::window::OnMonitor>),
        With<bevy::window::PrimaryWindow>,
    >,
    monitors: Query<&bevy::window::Monitor>,
) {
    if *done {
        return;
    }
    let Ok((mut window, on_monitor)) = windows.single_mut() else {
        return;
    };
    let Some(on_monitor) = on_monitor else {
        return;
    };
    let Ok(monitor) = monitors.get(on_monitor.0) else {
        return;
    };
    let fitted = fitted_physical_client(monitor.physical_width, monitor.physical_height);
    let keep_seed = backend
        .is_some_and(|backend| backend.applying_resolution() || backend.has_saved_resolution());
    if keep_seed || !matches!(window.mode, bevy::window::WindowMode::Windowed) {
        *done = true;
        return;
    }
    let Some((width, height)) = fitted else {
        return;
    };
    if (window.physical_width(), window.physical_height()) != (width, height) {
        window.resolution.set_physical_resolution(width, height);
        window.position =
            bevy::window::WindowPosition::Centered(bevy::window::MonitorSelection::Current);
    }
    *done = true;
}

#[cfg(test)]
mod tests {
    use super::fit_primary_window;
    use bevy::prelude::*;
    use bevy::window::{Monitor, OnMonitor, PrimaryWindow, WindowMode};

    fn monitor(width: u32, height: u32) -> Monitor {
        Monitor {
            name: None,
            physical_height: height,
            physical_width: width,
            physical_position: IVec2::ZERO,
            refresh_rate_millihertz: None,
            scale_factor: 1.0,
            video_modes: Vec::new(),
        }
    }

    fn client(world: &World, window: Entity) -> (u32, u32) {
        let window = world.get::<Window>(window).unwrap();
        (window.physical_width(), window.physical_height())
    }

    fn seed_window(world: &mut World) -> Entity {
        world
            .spawn((
                Window {
                    resolution: (880u32, 860u32).into(),
                    ..default()
                },
                PrimaryWindow,
            ))
            .id()
    }

    #[test]
    fn fitted_physical_client() {
        assert_eq!(super::fitted_physical_client(1920, 1080), Some((1728, 972)));
        assert_eq!(super::fitted_physical_client(800, 600), Some((720, 540)));
        assert_eq!(
            super::fitted_physical_client(2560, 1440),
            Some((2304, 1296))
        );
        assert_eq!(super::fitted_physical_client(1024, 768), Some((921, 691)));
        assert_eq!(super::fitted_physical_client(0, 1080), None);
        assert_eq!(super::fitted_physical_client(1920, 0), None);
        assert_eq!(super::fitted_physical_client(1, 1), None);
    }

    #[test]
    fn startup_window_fits_once_from_the_monitor() {
        let mut world = World::new();
        world.insert_resource(crate::menu_settings::Backend::default());
        let window = seed_window(&mut world);
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (880, 860));

        let display = world.spawn(monitor(0, 0)).id();
        world.entity_mut(window).insert(OnMonitor(display));
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (880, 860));

        world.get_mut::<Monitor>(display).unwrap().physical_width = 1920;
        world.get_mut::<Monitor>(display).unwrap().physical_height = 1080;
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (1728, 972));
        assert_eq!(
            world.get::<Window>(window).unwrap().position,
            WindowPosition::Centered(MonitorSelection::Current)
        );

        world.get_mut::<Monitor>(display).unwrap().physical_width = 800;
        world.get_mut::<Monitor>(display).unwrap().physical_height = 600;
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (1728, 972));
    }

    #[test]
    fn startup_window_keeps_an_explicit_saved_resolution() {
        let mut world = World::new();
        let mut backend = crate::menu_settings::Backend::default();
        backend.set_test_saved_resolution(true);
        world.insert_resource(backend);
        let display = world.spawn(monitor(1920, 1080)).id();
        let window = seed_window(&mut world);
        world.entity_mut(window).insert(OnMonitor(display));
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (880, 860));
        assert_eq!(
            world.get::<Window>(window).unwrap().position,
            WindowPosition::Automatic
        );
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (880, 860));
    }

    #[test]
    fn startup_window_fits_when_preferences_only_contain_audio() {
        let mut world = World::new();
        let mut backend = crate::menu_settings::Backend::default();
        backend.set_test_saved_resolution(false);
        world.insert_resource(backend);
        let display = world.spawn(monitor(1920, 1080)).id();
        let window = seed_window(&mut world);
        world.entity_mut(window).insert(OnMonitor(display));
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (1728, 972));
    }

    #[test]
    fn startup_window_keeps_seed_for_fullscreen() {
        let mut world = World::new();
        world.insert_resource(crate::menu_settings::Backend::default());
        let display = world.spawn(monitor(1920, 1080)).id();
        let window = seed_window(&mut world);
        world.entity_mut(window).insert(OnMonitor(display));
        world.get_mut::<Window>(window).unwrap().mode =
            WindowMode::BorderlessFullscreen(MonitorSelection::Current);
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (880, 860));
        world.get_mut::<Window>(window).unwrap().mode = WindowMode::Windowed;
        world.run_system_cached(fit_primary_window).unwrap();
        assert_eq!(client(&world, window), (880, 860));
    }
}

#[cfg(test)]
#[path = "startup_window_native_test.rs"]
mod native_gate;
