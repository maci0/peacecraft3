use super::*;
fn controller() -> Controller {
    Controller::new(Arc::new(Rules::fixture()), 0xabcd_0123)
}
fn play(c: &mut Controller, name: &str, fade: u32) -> Vec<Effect> {
    c.apply(
        Operation::Play {
            name: name.into(),
            position_ms: 0,
            fade_ms: fade,
        },
        Some(vec![name.into()]),
        0,
        &mut 100,
    )
    .unwrap()
}

#[test]
fn regular_controller_stop_resume_retains_regular_voice_and_theme_is_independent() {
    let mut c = controller();
    play(&mut c, "a.ogg", 0);
    let regular = c.regular().unwrap().serial;
    assert_eq!(
        c.apply(Operation::Stop { fade: false }, None, 2, &mut 100)
            .unwrap(),
        [Effect::Pause(regular)]
    );
    assert!(!c.regular().unwrap().playing);
    let resumed = c.apply(Operation::Resume, None, 3, &mut 100).unwrap();
    assert!(resumed.contains(&Effect::Resume(regular)));
    assert_eq!(c.regular().unwrap().serial, regular);
    c.apply(
        Operation::Theme {
            path: "theme.ogg".into(),
            position_ms: 12,
        },
        None,
        5,
        &mut 100,
    )
    .unwrap();
    let theme = c.thematic().unwrap().serial;
    let seek = c.apply(Operation::Seek(5000), None, 6, &mut 100).unwrap();
    assert_eq!(
        seek,
        [Effect::Seek {
            serial: regular,
            position_ms: 5000
        }]
    );
    assert_eq!(c.thematic().unwrap().start_ms, 12);
    c.apply(Operation::Stop { fade: false }, None, 6, &mut 100)
        .unwrap();
    assert!(
        c.apply(Operation::Resume, None, 6, &mut 100)
            .unwrap()
            .is_empty()
    );
    assert_eq!(c.thematic().unwrap().serial, theme);
}

#[test]
fn regular_controller_nonstock_fade_uses_actual_first_callback_and_strict_end() {
    let mut c = controller();
    play(&mut c, "a.ogg", 0);
    c.apply(Operation::Stop { fade: true }, None, 0, &mut 100)
        .unwrap();
    assert!(c.pump(6, &mut 100).unwrap().is_empty());
    // Late first delivery arms at 20; it does not synthesize callbacks at 7 and14.
    assert!(c.pump(20, &mut 100).unwrap().is_empty());
    assert_eq!(c.effective_level(), 8);
    c.pump(51, &mut 100).unwrap();
    assert_eq!(c.effective_level(), 0);
    assert!(c.regular().unwrap().playing);
    c.pump(58, &mut 100).unwrap();
    assert!(!c.regular().unwrap().playing);
}

#[test]
fn regular_controller_same_playing_path_cancels_fade_but_paused_path_replaces() {
    let mut c = controller();
    play(&mut c, "a.ogg", 0);
    let serial = c.regular().unwrap().serial;
    c.apply(Operation::Stop { fade: true }, None, 0, &mut 100)
        .unwrap();
    assert!(play(&mut c, "A.OGG", 0).is_empty());
    assert!(c.pump(100, &mut 100).unwrap().is_empty());
    c.apply(Operation::Stop { fade: false }, None, 100, &mut 100)
        .unwrap();
    let effects = play(&mut c, "a.ogg", 0);
    assert!(effects.contains(&Effect::Retire(serial)));
    assert_ne!(c.regular().unwrap().serial, serial);
}

#[test]
fn regular_controller_selection_is_deferred_modulo_and_completion_is_once() {
    let mut c = controller();
    c.apply(
        Operation::Select {
            name: "set".into(),
            index: 8,
        },
        None,
        0,
        &mut 100,
    )
    .unwrap();
    assert!(c.regular().is_none());
    c.apply(
        Operation::StartSelected,
        Some(vec!["a.ogg".into(), "b.ogg".into(), "c.ogg".into()]),
        0,
        &mut 100,
    )
    .unwrap();
    assert_eq!(c.regular().unwrap().path, "c.ogg");
    let serial = c.regular().unwrap().serial;
    c.finished(Role::Regular, serial, 7, &mut 100, None)
        .unwrap();
    c.finished(Role::Regular, serial, 7, &mut 100, None)
        .unwrap();
    assert!(c.pump(25, &mut 100).unwrap().is_empty());
    c.pump(26, &mut 100).unwrap();
    assert_eq!(c.regular().unwrap().path, "a.ogg");
    assert_ne!(c.regular().unwrap().serial, serial);
}

#[test]
fn regular_controller_budget_refusal_keeps_rng_voice_and_callback_state() {
    let mut c = controller();
    play(&mut c, "a.ogg", 0);
    c.apply(Operation::Stop { fade: true }, None, 0, &mut 100)
        .unwrap();
    let mut expected = c.clone();
    assert!(
        c.apply(
            Operation::Play {
                name: "bad".into(),
                position_ms: 0,
                fade_ms: 0
            },
            Some(vec!["x.ogg".into(), "y.ogg".into()]),
            3,
            &mut 1
        )
        .is_err()
    );
    assert_eq!(c.regular(), expected.regular());
    assert_eq!(
        c.random.choose(31, &mut 100).unwrap(),
        expected.random.choose(31, &mut 100).unwrap()
    );
    assert_eq!(
        c.pump(8, &mut 100).unwrap(),
        expected.pump(8, &mut 100).unwrap()
    );
}

#[test]
fn regular_controller_stale_theme_completion_and_session_retirement_preserve_epoch() {
    let mut c = controller();
    play(&mut c, "a.ogg", 0);
    c.apply(
        Operation::Theme {
            path: "old.ogg".into(),
            position_ms: 0,
        },
        None,
        0,
        &mut 100,
    )
    .unwrap();
    let old = c.thematic().unwrap().serial;
    c.apply(
        Operation::Theme {
            path: "new.ogg".into(),
            position_ms: 0,
        },
        None,
        1,
        &mut 100,
    )
    .unwrap();
    assert!(
        c.finished(Role::Thematic, old, 2, &mut 100, None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(c.thematic().unwrap().path, "new.ogg");
    let seed = c.random.initialized_at();
    let mut expected = c.random.clone();
    assert_eq!(c.retire_session().len(), 2);
    assert!(c.pump(500, &mut 100).unwrap().is_empty());
    assert_eq!(c.random.initialized_at(), seed);
    assert_eq!(
        c.random.choose(31, &mut 100).unwrap(),
        expected.choose(31, &mut 100).unwrap()
    );
}
