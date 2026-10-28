//! The single input-state resource: held modifier keys, the "game input open" gate
//! and the scroll options. Click handling, hotkeys and the camera read this instead
//! of `ButtonInput` modifier keys and their own copies of the gate.
//!
//! The gate is the set of reasons input is closed; input is open when it is empty.
//! Each reason is set by the system that knows it, every frame. Evidence for the
//! modifier and option semantics is in `docs/re-notes/input-behaviour-audit.md` and
//! `docs/re-notes/input-control-groups.md`.
use std::collections::BTreeSet;

use bevy::{input::InputSystems, prelude::*};
use pcx_play::{options_defaults::Defaults, scripts::AssetIdentity};

/// Option rows of the client's preference table that gate input (audit section E).
const MOUSE_SCROLL: &str = "mousescroll";
const KEY_SCROLL: &str = "keyscroll";
const MOUSE_SCROLL_DISABLE: &str = "mousescrolldisable";

/// A modifier key pair. The original reads either key of a pair as the modifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifier {
    pub left: bool,
    pub right: bool,
}

impl Modifier {
    pub fn held(self) -> bool {
        self.left || self.right
    }
}

/// Why game input is closed. Ordered only for stable reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Closer {
    /// No campaign view is shown.
    View,
    /// No script session is running.
    NoSession,
    /// The escape menu is open.
    Menu,
    /// The quest log is open.
    QuestLog,
    /// A script dialog blocks input.
    ScriptDialog,
    /// Scripts disabled user control.
    ControlDisabled,
    /// The local player has left the game.
    PlayerLeft,
}

/// Option values that gate input, resolved from the client's fallback rows and
/// the player's saved values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollOptions {
    /// Edge scroll speed slider.
    pub mouse_scroll: u32,
    /// Arrow key scroll speed slider.
    pub key_scroll: u32,
    pub mouse_scroll_disable: bool,
    /// Client the fallback rows were read from.
    pub fallback_origin: AssetIdentity,
}

/// Saved values; an absent field keeps the client fallback. No settings store
/// writes these yet (see the control group note), so nothing produces one today.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SavedScrollOptions {
    pub mouse_scroll: Option<u32>,
    pub key_scroll: Option<u32>,
    pub mouse_scroll_disable: Option<bool>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ScrollOptionError {
    #[error("{name}: the client has no fallback row")]
    Missing { name: &'static str },
    #[error("{name}: fallback {text:?} is not {expected}")]
    Malformed {
        name: &'static str,
        text: String,
        expected: &'static str,
    },
}

impl ScrollOptions {
    pub fn from_defaults(defaults: &Defaults<'_>) -> Result<Self, ScrollOptionError> {
        Self::from_fallbacks(defaults.binding().origin.clone(), |name| {
            defaults.get(name).map(|entry| entry.fallback)
        })
    }

    fn from_fallbacks<'a>(
        fallback_origin: AssetIdentity,
        row: impl Fn(&str) -> Option<&'a [u8]>,
    ) -> Result<Self, ScrollOptionError> {
        let text = |name: &'static str| -> Result<&'a str, ScrollOptionError> {
            let bytes = row(name).ok_or(ScrollOptionError::Missing { name })?;
            Ok(std::str::from_utf8(bytes).unwrap_or(""))
        };
        let slider = |name: &'static str| -> Result<u32, ScrollOptionError> {
            let value = text(name)?;
            value.parse().map_err(|_| ScrollOptionError::Malformed {
                name,
                text: value.into(),
                expected: "an unsigned integer",
            })
        };
        let mouse_scroll_disable = match text(MOUSE_SCROLL_DISABLE)? {
            "0" => false,
            "1" => true,
            other => {
                return Err(ScrollOptionError::Malformed {
                    name: MOUSE_SCROLL_DISABLE,
                    text: other.into(),
                    expected: "0 or 1",
                });
            }
        };
        Ok(Self {
            mouse_scroll: slider(MOUSE_SCROLL)?,
            key_scroll: slider(KEY_SCROLL)?,
            mouse_scroll_disable,
            fallback_origin,
        })
    }

    /// Saved values win over the fallbacks.
    pub fn with_saved(self, saved: SavedScrollOptions) -> Self {
        Self {
            mouse_scroll: saved.mouse_scroll.unwrap_or(self.mouse_scroll),
            key_scroll: saved.key_scroll.unwrap_or(self.key_scroll),
            mouse_scroll_disable: saved
                .mouse_scroll_disable
                .unwrap_or(self.mouse_scroll_disable),
            ..self
        }
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct GameInput {
    pub shift: Modifier,
    pub ctrl: Modifier,
    pub alt: Modifier,
    closed_by: BTreeSet<Closer>,
    scroll: Option<ScrollOptions>,
}

impl GameInput {
    /// Whether clicks, hotkeys and camera keys reach the game.
    pub fn open(&self) -> bool {
        self.closed_by.is_empty()
    }

    pub fn closed_by(&self) -> impl Iterator<Item = Closer> + '_ {
        self.closed_by.iter().copied()
    }

    pub fn set_closer(&mut self, closer: Closer, closed: bool) {
        if closed {
            self.closed_by.insert(closer);
        } else {
            self.closed_by.remove(&closer);
        }
    }

    pub fn scroll_options(&self) -> Option<&ScrollOptions> {
        self.scroll.as_ref()
    }

    pub fn set_scroll_options(&mut self, options: ScrollOptions) {
        self.scroll = Some(options);
    }
}

fn refresh_modifiers(keys: Option<Res<ButtonInput<KeyCode>>>, mut input: ResMut<GameInput>) {
    let held = |left, right| Modifier {
        left: keys.as_ref().is_some_and(|keys| keys.pressed(left)),
        right: keys.as_ref().is_some_and(|keys| keys.pressed(right)),
    };
    input.shift = held(KeyCode::ShiftLeft, KeyCode::ShiftRight);
    input.ctrl = held(KeyCode::ControlLeft, KeyCode::ControlRight);
    input.alt = held(KeyCode::AltLeft, KeyCode::AltRight);
}

/// Installs the resource and the modifier refresh; whichever owner installs first
/// wins and later calls do nothing.
pub fn install(app: &mut App) {
    if app.world().contains_resource::<GameInput>() {
        return;
    }
    app.init_resource::<GameInput>()
        .add_systems(PreUpdate, refresh_modifiers.after(InputSystems));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> AssetIdentity {
        AssetIdentity {
            source: "synthetic client".into(),
            edition: "fixture".into(),
            version: "fixture".into(),
        }
    }

    fn rows<'a>(
        mouse: &'a [u8],
        key: &'a [u8],
        disable: &'a [u8],
    ) -> impl Fn(&str) -> Option<&'a [u8]> {
        move |name| match name {
            MOUSE_SCROLL => Some(mouse),
            KEY_SCROLL => Some(key),
            MOUSE_SCROLL_DISABLE => Some(disable),
            _ => None,
        }
    }

    #[test]
    fn fallback_rows_become_option_values() {
        let options = ScrollOptions::from_fallbacks(origin(), rows(b"37", b"81", b"1")).unwrap();
        assert_eq!(
            (
                options.mouse_scroll,
                options.key_scroll,
                options.mouse_scroll_disable
            ),
            (37, 81, true)
        );
        assert_eq!(options.fallback_origin, origin());
    }

    #[test]
    fn missing_and_malformed_rows_refuse_by_name() {
        let none = |_: &str| None;
        assert_eq!(
            ScrollOptions::from_fallbacks(origin(), none),
            Err(ScrollOptionError::Missing {
                name: MOUSE_SCROLL_DISABLE
            })
        );
        for (mouse, key, disable, name) in [
            (&b"fast"[..], &b"50"[..], &b"0"[..], MOUSE_SCROLL),
            (b"50", b"-1", b"0", KEY_SCROLL),
            (b"50", b"50", b"2", MOUSE_SCROLL_DISABLE),
            (b"50", b"50", &[0xff][..], MOUSE_SCROLL_DISABLE),
        ] {
            let error =
                ScrollOptions::from_fallbacks(origin(), rows(mouse, key, disable)).unwrap_err();
            assert!(
                matches!(error, ScrollOptionError::Malformed { name: n, .. } if n == name),
                "{error}"
            );
        }
    }

    #[test]
    fn saved_values_override_only_what_they_set() {
        let fallback = ScrollOptions::from_fallbacks(origin(), rows(b"50", b"50", b"0")).unwrap();
        let saved = fallback.clone().with_saved(SavedScrollOptions {
            key_scroll: Some(10),
            mouse_scroll_disable: Some(true),
            ..default()
        });
        assert_eq!(
            (
                saved.mouse_scroll,
                saved.key_scroll,
                saved.mouse_scroll_disable
            ),
            (50, 10, true)
        );
        assert_eq!(fallback.clone().with_saved(default()), fallback);
    }

    #[test]
    fn the_gate_is_open_only_when_no_reason_remains() {
        let mut input = GameInput::default();
        assert!(input.open());
        input.set_closer(Closer::Menu, true);
        input.set_closer(Closer::QuestLog, true);
        assert!(!input.open());
        input.set_closer(Closer::Menu, false);
        assert_eq!(input.closed_by().collect::<Vec<_>>(), [Closer::QuestLog]);
        input.set_closer(Closer::QuestLog, false);
        assert!(input.open());
    }

    #[test]
    fn modifiers_follow_either_key_of_a_pair() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>();
        install(&mut app);
        install(&mut app);
        let press = |app: &mut App, key| {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key)
        };
        let release = |app: &mut App, key| {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(key)
        };
        let state = |app: &App| {
            let input = app.world().resource::<GameInput>();
            (input.shift, input.ctrl, input.alt)
        };
        app.update();
        assert_eq!(state(&app), Default::default());
        press(&mut app, KeyCode::ShiftRight);
        press(&mut app, KeyCode::ControlLeft);
        app.update();
        let (shift, ctrl, alt) = state(&app);
        assert_eq!((shift.left, shift.right), (false, true));
        assert_eq!((ctrl.left, ctrl.right), (true, false));
        assert!(shift.held() && ctrl.held() && !alt.held());
        release(&mut app, KeyCode::ShiftRight);
        press(&mut app, KeyCode::AltRight);
        app.update();
        let (shift, _, alt) = state(&app);
        assert!(!shift.held());
        assert!(alt.right && !alt.left);
    }
}
