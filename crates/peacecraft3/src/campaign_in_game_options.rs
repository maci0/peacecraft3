// In-game Options route: Open / Back / Cancel / OK.
//
// This module owns the focused route state only; shared page wiring stays in
// campaign_escape_menu / menu_screens / menu_settings / glue_menu.
//
// Route contract:
// - `route_open_escape_options` (escape `OptionsButton`): enters Options,
//   restores draft := applied, binds session witness.
// - `route_back_to_escape`: from Options mark, back to escape root.
// - `route_cancel_options`: restores draft := applied, back to escape root.
// - `route_ok_options`: transitions to escape root only AFTER the caller
//   reports a successful commit (e.g. `menu_settings::apply` returning Ok).
//   This helper never assigns `applied` and never persists; on a failed
//   commit the caller must not call it, so Options stays open with
//   draft/applied intact.
// - Session-bound: every transition requires the bound witness. `Weak<()>` is
//   not `Eq`, so events carry only `cancelled`, not the witness.
// - Caller must keep `CampaignCamera::set_menu_open(true)` while route is
//   `Options`, preserving pause.

use std::sync::Weak;

use bevy::ecs::prelude::*;
use bevy::prelude::Resource;

use crate::menu_screens::{Navigation, Page, Screen};

/// Authored escape-menu entry that opens in-game Options.
#[allow(dead_code)]
pub const ESCAPE_OPTIONS_BUTTON: &str = "OptionsButton";
/// Authored Options-menu cancel button.
#[allow(dead_code)]
pub const OPTIONS_CANCEL_BUTTON: &str = "CancelButton";
/// Authored Options-menu confirm button.
#[allow(dead_code)]
pub const OPTIONS_OK_BUTTON: &str = "OKButton";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InGameRoute {
    #[default]
    Escape,
    #[allow(dead_code)]
    Options,
}

/// Focused route state. Witness binds the modal to one admitted session.
#[derive(Debug, Clone, Default, Resource)]
pub struct InGameOptionsRoute {
    #[allow(dead_code)]
    pub state: InGameRoute,
    #[allow(dead_code)]
    pub session: Option<Weak<()>>,
}

impl InGameOptionsRoute {
    #[allow(dead_code)]
    pub fn new() -> Self { Self::default() }
    #[allow(dead_code)]
    pub fn is_open(&self) -> bool { self.state == InGameRoute::Options }
    #[allow(dead_code)]
    fn bound(&self, witness: &Weak<()>) -> bool {
        self.session.as_ref().is_some_and(|w| Weak::ptr_eq(w, witness))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Event)]
#[allow(dead_code)]
pub struct InGameOptionsOpened;

#[derive(Debug, Clone, PartialEq, Eq, Event)]
#[allow(dead_code)]
pub struct InGameOptionsClosed { pub cancelled: bool }

#[allow(dead_code)]
pub fn route_open_escape_options(
    route: &mut InGameOptionsRoute,
    nav: &mut Navigation,
    witness: &Weak<()>,
) -> bool {
    if route.state == InGameRoute::Options { return false; }
    if !matches!(nav.screen, Screen::Main | Screen::Options(Page::Unselected)) && route.state != InGameRoute::Escape {
        return false;
    }
    nav.draft = nav.applied.clone();
    nav.selected_offer = None;
    route.session = Some(witness.clone());
    route.state = InGameRoute::Options;
    nav.open(Screen::Options(Page::Unselected));
    true
}

#[allow(dead_code)]
pub fn route_back_to_escape(
    route: &mut InGameOptionsRoute,
    nav: &mut Navigation,
    witness: &Weak<()>,
) -> bool {
    if !route.bound(witness) { return false; }
    route.state = InGameRoute::Escape;
    route.session = None;
    nav.open(Screen::Main);
    true
}

#[allow(dead_code)]
pub fn route_cancel_options(
    route: &mut InGameOptionsRoute,
    nav: &mut Navigation,
    witness: &Weak<()>,
) -> bool {
    if !route.bound(witness) { return false; }
    nav.draft = nav.applied.clone();
    nav.selected_offer = None;
    route.state = InGameRoute::Escape;
    route.session = None;
    nav.open(Screen::Main);
    true
}

/// OK transition: call only AFTER the caller's commit returned Ok.
/// Never assigns `applied` or persists; the validated commit owned by the
/// caller (e.g. `menu_settings::apply` / `menu_settings::commit`) has
/// already done that. A failed commit must bypass this helper so the route
/// stays Options with draft/applied intact.
#[allow(dead_code)]
pub fn route_ok_options(
    route: &mut InGameOptionsRoute,
    nav: &mut Navigation,
    witness: &Weak<()>,
    committed: Result<(), String>,
) -> bool {
    if committed.is_err() { return false; }
    if !route.bound(witness) { return false; }
    nav.selected_offer = None;
    route.state = InGameRoute::Escape;
    route.session = None;
    nav.open(Screen::Main);
    true
}

/// Minimal registration. Events are plain types; main can register them with
/// `World::register_event` if a message system is needed.
#[allow(dead_code)]
pub fn install(app: &mut bevy::prelude::App) {
    app.init_resource::<InGameOptionsRoute>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn setup() -> (InGameOptionsRoute, Navigation, Weak<()>) {
        let mut nav = Navigation { screen: Screen::Main, ..Default::default() };
        nav.applied.toggles.insert("TooltipsCheckBox".into(), true);
        nav.draft.toggles.insert("TooltipsCheckBox".into(), false);
        let arc = Arc::new(());
        let weak = Arc::downgrade(&arc);
        std::mem::forget(arc);
        (InGameOptionsRoute::default(), nav, weak)
    }

    #[test]
    fn open_restores_draft_and_binds() {
        let (mut route, mut nav, w) = setup();
        assert!(route_open_escape_options(&mut route, &mut nav, &w));
        assert_eq!(route.state, InGameRoute::Options);
        assert!(nav.draft.toggles["TooltipsCheckBox"]);
        assert!(route.is_open());
    }

    #[test]
    fn cancel_restores_draft_without_touching_applied() {
        let (mut route, mut nav, w) = setup();
        route_open_escape_options(&mut route, &mut nav, &w);
        nav.draft.toggles.insert("TooltipsCheckBox".into(), false);
        assert!(route_cancel_options(&mut route, &mut nav, &w));
        assert!(nav.draft.toggles["TooltipsCheckBox"]);
        assert!(nav.applied.toggles["TooltipsCheckBox"]);
        assert_eq!(route.state, InGameRoute::Escape);
    }

    #[test]
    fn foreign_session_cannot_cancel_or_back() {
        let (mut route, mut nav, w) = setup();
        route_open_escape_options(&mut route, &mut nav, &w);
        let other = Arc::new(());
        let foreign = Arc::downgrade(&other);
        assert!(!route_cancel_options(&mut route, &mut nav, &foreign));
        assert!(!route_back_to_escape(&mut route, &mut nav, &foreign));
        assert_eq!(route.state, InGameRoute::Options);
    }

    #[test]
    fn back_returns_to_main_without_draft_change() {
        let (mut route, mut nav, w) = setup();
        route_open_escape_options(&mut route, &mut nav, &w);
        nav.draft.toggles.insert("TooltipsCheckBox".into(), false);
        assert!(route_back_to_escape(&mut route, &mut nav, &w));
        assert_eq!(route.state, InGameRoute::Escape);
        assert!(!nav.draft.toggles["TooltipsCheckBox"]);
    }

    #[test]
    fn ok_only_transitions_after_successful_caller_commit() {
        let (mut route, mut nav, w) = setup();
        route_open_escape_options(&mut route, &mut nav, &w);
        nav.draft.toggles.insert("TooltipsCheckBox".into(), false);
        // Failed commit: helper refuses; Options, draft, applied intact.
        assert!(!route_ok_options(&mut route, &mut nav, &w, Err("save failed".into())));
        assert_eq!(route.state, InGameRoute::Options);
        assert!(route.is_open());
        assert!(!nav.draft.toggles["TooltipsCheckBox"]);
        assert!(nav.applied.toggles["TooltipsCheckBox"]);
        // Successful caller commit (caller already set applied := draft
        // through the validated commit API); helper only routes back.
        nav.applied = nav.draft.clone();
        assert!(route_ok_options(&mut route, &mut nav, &w, Ok(())));
        assert!(!nav.applied.toggles["TooltipsCheckBox"]);
        assert_eq!(route.state, InGameRoute::Escape);
    }
}
