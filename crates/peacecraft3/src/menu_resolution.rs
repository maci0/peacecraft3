//! Portable client-size requests, not exclusive display-mode transactions.
use super::*;
use bevy::{ecs::system::NonSendMarker, platform::time::Instant};
use std::time::Duration;

#[cfg(all(test, target_os = "linux"))]
#[path = "menu_resolution_native_test.rs"]
mod native_test;

// Operation watchdog, independent of original game settings or frame rate.
const REQUEST_LIMIT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Host {
    entity: Entity,
    id: u64,
    size: (u32, u32),
}

fn host(entity: Entity) -> Option<Host> {
    bevy::winit::WINIT_WINDOWS.with_borrow(|windows| {
        windows.get_window(entity).map(|window| {
            let size = window.inner_size();
            Host {
                entity,
                id: window.id().into(),
                size: (size.width, size.height),
            }
        })
    })
}

#[derive(Clone, PartialEq)]
struct Source {
    path: String,
    mode: crate::launch::GameMode,
    revision: u32,
}
impl Source {
    fn current(launcher: &crate::launch_gui::Launcher) -> Self {
        let (path, mode) = launcher.audio_selection();
        Self {
            path: path.into(),
            mode,
            #[cfg(target_arch = "wasm32")]
            revision: crate::browser::asset_revision(),
            #[cfg(not(target_arch = "wasm32"))]
            revision: 0,
        }
    }
}

enum Purpose {
    Apply {
        candidate: Prepared,
        baseline: Settings,
    },
    Startup,
}
enum Phase {
    Request,
    Restore { reason: String, cancel: bool },
}
pub(super) struct Pending {
    host: Host,
    target: (u32, u32),
    fit_canvas: bool,
    source: Source,
    purpose: Purpose,
    phase: Phase,
    started: Instant,
    cancel: bool,
}

pub(super) fn apply(
    entity: Entity,
    nav: &mut Navigation,
    window: &mut Window,
    backend: &mut Backend,
    launcher: &crate::launch_gui::Launcher,
) -> Result<ApplyOutcome, String> {
    if backend.resize.is_some() {
        return Ok(ApplyOutcome::Pending);
    }
    let candidate = prepare(nav, backend)?;
    let current = host(entity)
        .filter(|h| h.size.0 > 0 && h.size.1 > 0)
        .ok_or("The window's client size is unavailable; settings were not saved")?;
    backend.observed_resolution = Some(current.size);
    backend.pending_resolution = None;
    if candidate
        .settings
        .resolution
        .is_none_or(|size| size == current.size)
    {
        commit(nav, window, backend, save_preferences)?;
        return Ok(ApplyOutcome::Complete);
    }
    begin(
        window,
        backend,
        current,
        candidate.settings.resolution.unwrap(),
        Purpose::Apply {
            candidate,
            baseline: nav.applied.clone(),
        },
        Source::current(launcher),
    );
    nav.notice = "Changing render size…".into();
    Ok(ApplyOutcome::Pending)
}

fn begin(
    window: &mut Window,
    backend: &mut Backend,
    current: Host,
    target: (u32, u32),
    purpose: Purpose,
    source: Source,
) {
    let pending = Pending {
        host: current,
        target,
        fit_canvas: window.fit_canvas_to_parent,
        source,
        purpose,
        phase: Phase::Request,
        started: Instant::now(),
        cancel: false,
    };
    #[cfg(target_arch = "wasm32")]
    {
        window.fit_canvas_to_parent = false;
    }
    window
        .resolution
        .set_physical_resolution(target.0, target.1);
    backend.resize = Some(pending);
}

pub(super) fn startup(
    entity: Entity,
    nav: &mut Navigation,
    window: &mut Window,
    backend: &mut Backend,
    launcher: &crate::launch_gui::Launcher,
    target: (u32, u32),
) -> bool {
    let Some(current) = host(entity).filter(|h| h.size.0 > 0 && h.size.1 > 0) else {
        return false;
    };
    if current.size == target {
        nav.applied.resolution = Some(target);
        nav.draft.resolution = Some(target);
    } else {
        begin(
            window,
            backend,
            current,
            target,
            Purpose::Startup,
            Source::current(launcher),
        );
        nav.notice = "Restoring saved render size…".into();
    }
    true
}

pub(super) fn cancel(backend: &mut Backend) -> bool {
    if let Some(pending) = &mut backend.resize {
        pending.cancel = true;
        true
    } else {
        false
    }
}

fn restore_window(pending: &Pending, window: &mut Window) {
    window.fit_canvas_to_parent = pending.fit_canvas;
    window
        .resolution
        .set_physical_resolution(pending.host.size.0, pending.host.size.1);
}

fn restore(pending: &mut Pending, window: &mut Window, reason: String, cancel: bool) {
    restore_window(pending, window);
    pending.started = Instant::now();
    pending.phase = Phase::Restore { reason, cancel };
}

fn step(
    nav: &mut Navigation,
    window: &mut Window,
    backend: &mut Backend,
    observation: Option<Host>,
    source: &Source,
    expired: bool,
    save: impl FnOnce(&str) -> Result<(), String>,
) {
    let Some(mut pending) = backend.resize.take() else {
        return;
    };
    if let Some(current) =
        observation.filter(|h| h.entity != pending.host.entity || h.id != pending.host.id)
    {
        if current.entity == pending.host.entity {
            restore_window(&pending, window);
        }
        backend.observed_resolution = None;
        nav.notice =
            "The window changed; the render-size request was abandoned without saving settings"
                .into();
        return;
    }
    let current = observation.map(|h| h.size).filter(|s| s.0 > 0 && s.1 > 0);
    match &pending.phase {
        Phase::Request => {
            let invalid = match &pending.purpose {
                Purpose::Apply {
                    candidate,
                    baseline,
                } => {
                    nav.draft != candidate.settings
                        || nav.applied != *baseline
                        || !matches!(nav.screen, crate::menu_screens::Screen::Options(_))
                        || nav.requested.is_some()
                }
                Purpose::Startup => false,
            };
            if pending.cancel
                || source != &pending.source
                || invalid
                || (expired && current != Some(pending.target))
            {
                let reason = if pending.cancel {
                    "Render-size change cancelled; settings were not saved"
                } else if source != &pending.source || invalid {
                    "Settings changed during the request; nothing was saved"
                } else {
                    "Render size was not confirmed; settings were not saved"
                };
                let cancel = pending.cancel;
                restore(&mut pending, window, reason.into(), cancel);
                nav.notice = format!("{reason}. Restoring the previous render size…");
            } else if current == Some(pending.target) {
                backend.observed_resolution = current;
                match &pending.purpose {
                    Purpose::Startup => {
                        nav.applied.resolution = current;
                        nav.draft.resolution = current;
                        nav.notice.clear();
                        return;
                    }
                    Purpose::Apply {
                        candidate,
                        baseline,
                    } => match publish(candidate, nav, backend, save) {
                        Ok(Some(warning)) => {
                            nav.notice = warning;
                            return;
                        }
                        Ok(None) => {
                            nav.open(crate::menu_screens::Screen::Main);
                            return;
                        }
                        Err(error) => {
                            debug_assert_eq!(&nav.applied, baseline);
                            restore(
                                &mut pending,
                                window,
                                format!("Settings were not saved: {error}"),
                                false,
                            );
                            nav.notice = format!(
                                "Settings were not saved: {error}. Restoring the previous render size…"
                            );
                        }
                    },
                }
            }
        }
        Phase::Restore { reason, cancel } => {
            if current == Some(pending.host.size) {
                // Winit's previous Last has submitted the pixel restoration.
                #[cfg(target_arch = "wasm32")]
                let fit_warning = pending
                    .fit_canvas
                    .then(browser_parent_sizing)
                    .and_then(Result::err);
                backend.observed_resolution = current;
                nav.applied.resolution = current;
                let cancel = *cancel || pending.cancel;
                if cancel || matches!(pending.purpose, Purpose::Startup) {
                    nav.draft = nav.applied.clone();
                }
                let reason = reason.clone();
                if cancel {
                    nav.action("CancelButton");
                }
                nav.notice = format!("{reason}. Previous render size restored.");
                #[cfg(target_arch = "wasm32")]
                if let Some(error) = fit_warning {
                    nav.notice.push_str(&format!(
                        " Parent-following sizing could not be restored: {}",
                        browser_failure(error)
                    ));
                }
                return;
            }
            if expired {
                if let Some(size) = current {
                    backend.observed_resolution = Some(size);
                    nav.applied.resolution = Some(size);
                    if *cancel || pending.cancel || matches!(pending.purpose, Purpose::Startup) {
                        nav.draft = nav.applied.clone();
                    }
                    nav.notice = format!(
                        "{reason}. Restoring the previous size was not confirmed; the client reports {} × {}.",
                        size.0, size.1
                    );
                } else {
                    nav.notice = format!(
                        "{reason}. Restoring the previous size was not confirmed; client size is unavailable."
                    );
                }
                return;
            }
        }
    }
    backend.resize = Some(pending);
}

pub(super) fn poll(
    mut nav: ResMut<Navigation>,
    mut backend: ResMut<Backend>,
    launcher: Res<crate::launch_gui::Launcher>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    _main_thread: NonSendMarker,
) {
    let Ok((entity, mut window)) = windows.single_mut() else {
        if backend.resize.take().is_some() {
            nav.notice = "The window is unavailable; settings were not saved and restoration cannot be confirmed".into();
        }
        return;
    };
    // First follows the previous Last, where Winit submits Window changes.
    // Wayland may report client-buffer state before compositor presentation.
    if backend.resize.is_some()
        || backend.observed_resolution != Some((window.physical_width(), window.physical_height()))
    {
        let observation = host(entity);
        if backend.resize.is_some() {
            let expired = backend
                .resize
                .as_ref()
                .is_some_and(|p| p.started.elapsed() >= REQUEST_LIMIT);
            step(
                &mut nav,
                &mut window,
                &mut backend,
                observation,
                &Source::current(&launcher),
                expired,
                save_preferences,
            );
        } else if let Some(host) = observation.filter(|h| h.size.0 > 0 && h.size.1 > 0) {
            observe_resolution(&mut nav, &mut backend, host.size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (Navigation, Window, Backend, Host, Source) {
        let old = (901, 603);
        let target = (741, 511);
        let mut nav = Navigation {
            screen: crate::menu_screens::Screen::Options(crate::menu_screens::Page::Video),
            resolutions: vec![old, target],
            ..default()
        };
        nav.applied.resolution = Some(old);
        nav.draft = nav.applied.clone();
        nav.draft.resolution = Some(target);
        nav.draft.toggles.insert("MusicCheckBox".into(), false);
        let mut backend = Backend {
            observed_resolution: Some(old),
            ..default()
        };
        let mut window = Window::default();
        window.resolution.set_physical_resolution(old.0, old.1);
        let host = Host {
            entity: Entity::PLACEHOLDER,
            id: 17,
            size: old,
        };
        let source = Source::current(&crate::launch_gui::Launcher::default());
        let candidate = prepare(&nav, &backend).unwrap();
        begin(
            &mut window,
            &mut backend,
            host,
            target,
            Purpose::Apply {
                candidate,
                baseline: nav.applied.clone(),
            },
            source.clone(),
        );
        (nav, window, backend, host, source)
    }
    fn requested(backend: &Backend) -> Host {
        let p = backend.resize.as_ref().unwrap();
        Host {
            size: p.target,
            ..p.host
        }
    }
    fn no_save(_: &str) -> Result<(), String> {
        panic!("unconfirmed or cancelled request must not save")
    }
    #[test]
    fn desired_window_is_not_confirmation_and_a_match_saves_once() {
        let (mut nav, mut window, mut backend, old, source) = setup();
        let target = requested(&backend);
        assert_eq!(
            (window.physical_width(), window.physical_height()),
            target.size
        );
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(old),
            &source,
            false,
            no_save,
        );
        assert!(backend.resize.is_some());
        assert_eq!(nav.applied.resolution, Some(old.size));
        assert!(backend.audio.music_enabled);
        let mut saves = 0;
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(target),
            &source,
            false,
            |text| {
                saves += 1;
                let candidate = decode_preferences(text)?;
                assert_eq!(candidate.settings.resolution, Some(target.size));
                assert!(!candidate.audio.music_enabled);
                Ok(())
            },
        );
        assert_eq!(saves, 1);
        assert!(backend.resize.is_none());
        assert_eq!(nav.applied.resolution, Some(target.size));
        assert!(!backend.audio.music_enabled);
        assert_eq!(nav.requested, Some(crate::menu_screens::Screen::Main));
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(target),
            &source,
            false,
            no_save,
        );
    }
    #[test]
    fn cancel_wins_over_a_matching_request_and_waits_for_restore() {
        let (mut nav, mut window, mut backend, old, source) = setup();
        let target = requested(&backend);
        assert!(cancel(&mut backend));
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(target),
            &source,
            false,
            no_save,
        );
        assert!(backend.resize.is_some());
        assert!(nav.requested.is_none());
        assert!(backend.audio.music_enabled);
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(old),
            &source,
            false,
            no_save,
        );
        assert!(backend.resize.is_none());
        assert_eq!(nav.draft, nav.applied);
        assert_eq!(nav.requested, Some(crate::menu_screens::Screen::Main));
        assert!(nav.notice.contains("restored"));
    }
    #[test]
    fn a_matching_report_does_not_fail_only_because_scheduling_was_delayed() {
        let (mut nav, mut window, mut backend, _, source) = setup();
        let target = requested(&backend);
        let mut saves = 0;
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(target),
            &source,
            true,
            |_| {
                saves += 1;
                Ok(())
            },
        );
        assert_eq!(saves, 1);
        assert!(backend.resize.is_none());
        assert_eq!(nav.applied.resolution, Some(target.size));
    }
    #[test]
    fn save_failure_restores_size_but_keeps_the_dirty_draft_for_retry() {
        let (mut nav, mut window, mut backend, old, source) = setup();
        let target = requested(&backend);
        let dirty = nav.draft.clone();
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(target),
            &source,
            false,
            |_| Err("storage denied".into()),
        );
        assert!(matches!(
            backend.resize.as_ref().unwrap().phase,
            Phase::Restore { .. }
        ));
        assert!(backend.audio.music_enabled);
        assert_eq!(nav.applied.resolution, Some(old.size));
        assert_eq!(nav.draft, dirty);
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(old),
            &source,
            false,
            no_save,
        );
        assert_eq!(nav.draft, dirty);
        assert!(nav.requested.is_none());
        assert!(backend.resize.is_none());
        assert!(nav.notice.contains("storage denied"));
    }
    #[test]
    fn cancel_during_storage_failure_restore_keeps_cancel_navigation() {
        let (mut nav, mut window, mut backend, old, source) = setup();
        let target = requested(&backend);
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(target),
            &source,
            false,
            |_| Err("storage denied".into()),
        );
        assert!(cancel(&mut backend));
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(old),
            &source,
            false,
            no_save,
        );
        assert_eq!(nav.draft, nav.applied);
        assert_eq!(nav.requested, Some(crate::menu_screens::Screen::Main));
    }
    #[test]
    fn missing_or_constrained_observation_times_out_without_saving_or_claiming_restore() {
        for observed in [None, Some((713, 487))] {
            let (mut nav, mut window, mut backend, old, source) = setup();
            let actual = observed.map(|size| Host { size, ..old });
            step(
                &mut nav,
                &mut window,
                &mut backend,
                actual,
                &source,
                false,
                no_save,
            );
            assert!(matches!(
                backend.resize.as_ref().unwrap().phase,
                Phase::Request
            ));
            step(
                &mut nav,
                &mut window,
                &mut backend,
                actual,
                &source,
                true,
                no_save,
            );
            assert!(matches!(
                backend.resize.as_ref().unwrap().phase,
                Phase::Restore { .. }
            ));
            step(
                &mut nav,
                &mut window,
                &mut backend,
                actual,
                &source,
                true,
                no_save,
            );
            assert!(backend.resize.is_none());
            assert!(nav.notice.contains("not confirmed"));
            assert!(!nav.notice.contains("Previous render size restored"));
            if let Some(size) = observed {
                assert_eq!(nav.applied.resolution, Some(size));
            }
            assert!(backend.audio.music_enabled);
        }
    }
    #[test]
    fn changed_source_or_draft_cannot_publish_a_stale_candidate() {
        for change_source in [false, true] {
            let (mut nav, mut window, mut backend, old, mut source) = setup();
            let target = requested(&backend);
            if change_source {
                source.revision += 1;
            } else {
                nav.draft.toggles.insert("SoundCheckBox".into(), false);
            }
            step(
                &mut nav,
                &mut window,
                &mut backend,
                Some(target),
                &source,
                false,
                no_save,
            );
            assert!(matches!(
                backend.resize.as_ref().unwrap().phase,
                Phase::Restore { .. }
            ));
            step(
                &mut nav,
                &mut window,
                &mut backend,
                Some(old),
                &source,
                false,
                no_save,
            );
            assert!(backend.audio.music_enabled);
            assert_eq!(nav.applied.resolution, Some(old.size));
        }
    }
    #[test]
    fn replacement_window_is_not_an_acknowledgement() {
        for same_entity in [true, false] {
            let (mut nav, mut window, mut backend, old, source) = setup();
            let target = requested(&backend);
            let dirty = nav.draft.clone();
            let replacement = Host {
                entity: if same_entity {
                    old.entity
                } else {
                    World::new().spawn_empty().id()
                },
                id: 99,
                ..target
            };
            window.fit_canvas_to_parent = !backend.resize.as_ref().unwrap().fit_canvas;
            let replacement_fit = window.fit_canvas_to_parent;
            step(
                &mut nav,
                &mut window,
                &mut backend,
                Some(replacement),
                &source,
                false,
                no_save,
            );
            assert!(backend.resize.is_none());
            assert!(
                backend.observed_resolution.is_none(),
                "the old window observation is stale"
            );
            assert_eq!(
                (window.physical_width(), window.physical_height()),
                if same_entity { old.size } else { target.size }
            );
            assert_eq!(
                window.fit_canvas_to_parent,
                if same_entity {
                    !replacement_fit
                } else {
                    replacement_fit
                }
            );
            assert_eq!(nav.applied.resolution, Some(old.size));
            assert_eq!(nav.draft, dirty);
            assert!(nav.notice.contains("window changed"));
            assert!(backend.audio.music_enabled);
            observe_resolution(&mut nav, &mut backend, replacement.size);
            assert_eq!(nav.applied.resolution, Some(replacement.size));
            assert_eq!(nav.draft, dirty);
        }
    }
    #[test]
    fn startup_uses_the_same_observation_without_writing_preferences() {
        let (mut nav, mut window, mut backend, _, source) = setup();
        backend.resize.as_mut().unwrap().purpose = Purpose::Startup;
        let target = requested(&backend);
        step(
            &mut nav,
            &mut window,
            &mut backend,
            Some(target),
            &source,
            false,
            no_save,
        );
        assert!(backend.resize.is_none());
        assert_eq!(nav.applied.resolution, Some(target.size));
        assert!(nav.requested.is_none());
        assert!(backend.audio.music_enabled);
        assert!(!backend.has_preferences);
    }
}
