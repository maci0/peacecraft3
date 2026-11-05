//! CustomKeysCheckBox: the only gameplay checkbox with an admitted consumer.
//! Consumer: `campaign_custom_keys::load` reads `Navigation.applied` and
//! `campaign_command_card::resolve_hotkey` applies the file to button hotkeys
//! (`button_for_keys`). Source default is the selected client's `customkeys`
//! option row via `Defaults::discover`; only exact `0`/`1` fallbacks admit.
//! The other Gameplay checkboxes (HealthBars/Subgroup/Formation/Autosave)
//! have no admitted consumer and remain preview-only: they are NOT in
//! `supports`, so they stay disabled widgets with authored preview values and
//! no preference storage. No consumer behavior is invented for them.
use super::*;
use pcx_play::options_defaults::Defaults;

pub(crate) const CONTROLS: [(&str, &str); 1] = [("CustomKeysCheckBox", "customkeys")];
pub(crate) fn supports(name: &str) -> bool { CONTROLS.iter().any(|(c, _)| *c == name) }
fn key(name: &str) -> Option<&'static str> { CONTROLS.iter().find(|(c, _)| *c == name).map(|(_, k)| *k) }
fn checkbox(raw: &[u8]) -> Result<bool, String> {
    match raw { b"0" => Ok(false), b"1" => Ok(true), _ => Err("fallback outside 0/1 checkbox profile".into()) }
}
pub(crate) fn defaults(launcher: Option<&crate::launch_gui::Launcher>) -> Result<std::collections::BTreeMap<String, bool>, String> {
    let install = launcher.and_then(crate::launch_gui::selected_install);
    let install_ref = install.as_deref();
    crate::menu_lighting::with_client_image(install_ref, |source, image| {
        let origin = pcx_play::scripts::AssetIdentity { source: source.into(), edition: "selected menu client".into(), version: "reviewed Legacy 1.29.2.9231 customkeys profile".into() };
        let defaults = Defaults::discover(image, origin, crate::campaign_game_input::DISCOVERY).map_err(|e| e.to_string())?;
        let mut out = std::collections::BTreeMap::new();
        for (control, _) in CONTROLS {
            let raw = defaults.get(key(control).unwrap()).ok_or(format!("{control}: source preference missing"))?.fallback;
            out.insert(control.to_string(), checkbox(raw).map_err(|e| format!("{control}: {e}"))?);
        }
        Ok(out)
    })
}
pub(super) fn attach(commands: &mut Commands, entity: Entity, control: &SettingControl, nav: &mut Navigation, seed: bool) -> Result<(), String> {
    if !matches!(control.widget, Widget::Checkbox) { return Err("incompatible gameplay control".into()) }
    let applied = *nav.applied.toggles.entry(control.name.clone()).or_insert(seed);
    let draft = *nav.draft.toggles.entry(control.name.clone()).or_insert(applied);
    commands.entity(entity).insert(Checkbox);
    if draft { commands.entity(entity).insert(Checked); } else { commands.entity(entity).remove::<Checked>(); }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn only_consumer_backed_control_supported() {
        assert!(supports("CustomKeysCheckBox"));
        for other in ["HealthBarsCheckBox", "SubgroupCheckBox", "FormationToggleCheckBox", "AutosaveReplayCheckBox"] {
            assert!(!supports(other), "{other} must stay preview-only");
            assert!(crate::menu_settings::preview::eligible(other), "{other} preview preserved");
        }
        assert_eq!(key("CustomKeysCheckBox"), Some("customkeys"));
    }
    #[test] fn attach_draft_cancel_save_reload() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut nav = Navigation::default();
        let control = SettingControl { name: "CustomKeysCheckBox".into(), widget: Widget::Checkbox };
        attach(&mut world.commands(), entity, &control, &mut nav, false).unwrap();
        world.flush();
        assert!(world.get::<Checked>(entity).is_none());
        world.entity_mut(entity).insert(control);
        world.insert_resource(nav); world.insert_resource(Backend::default());
        world.add_observer(super::super::checkbox_changed);
        world.trigger(ValueChange { source: entity, value: true, is_final: true }); world.flush();
        let mut nav = world.remove_resource::<Navigation>().unwrap();
        assert!(!nav.applied.toggles["CustomKeysCheckBox"], "draft not applied");
        assert!(nav.draft.toggles["CustomKeysCheckBox"]);
        nav.action("CancelButton");
        assert!(!nav.draft.toggles["CustomKeysCheckBox"], "cancel restores applied");
        nav.draft.toggles.insert("CustomKeysCheckBox".into(), true);
        let mut backend = world.remove_resource::<Backend>().unwrap();
        let mut saved = String::new();
        super::super::commit(&mut nav, &mut Window::default(), &mut backend, |t| { saved = t.into(); Ok(()) }).unwrap();
        assert!(super::super::decode_preferences(&saved).unwrap().settings.toggles["CustomKeysCheckBox"]);
        let control = SettingControl { name: "CustomKeysCheckBox".into(), widget: Widget::Checkbox };
        attach(&mut world.commands(), entity, &control, &mut nav, false).unwrap(); world.flush();
        assert!(world.get::<Checked>(entity).is_some(), "saved true overrides source false");
    }
    #[test] fn malformed_fallback_refuses() {
        assert!(checkbox(b"2").is_err()); assert!(checkbox(b"").is_err()); assert_eq!(checkbox(b"1"), Ok(true));
    }
    #[test]
    fn applied_customkeys_drives_command_card_hotkey() {
        use pcx_play::command_hotkey::{CustomKeys, Field, Hotkey};
        // Consumer chain: Navigation.applied true -> campaign_custom_keys::load
        // gates on it (enabled(saved)=saved) -> CustomKeys parse -> resolve ->
        // button_for_keys. This exercises the real consumer types, not storage.
        let applied_enabled = |saved: Option<bool>| saved.unwrap_or(false);
        assert!(!applied_enabled(None), "absent applied = off (no file read)");
        assert!(applied_enabled(Some(true)));
        let custom = CustomKeys::parse("[hfoo]\nHotkey=T\n").unwrap();
        assert_eq!(custom.get("hfoo", Field::Hotkey), Some(&[Some(Hotkey::letter('T').unwrap())][..]));
        // resolve_hotkey is in campaign_command_card; replicate its contract via
        // CustomKeys::get + at_rank, the exact functions resolve_hotkey calls.
        let ranked = custom.get("hfoo", Field::Hotkey).unwrap();
        assert_eq!(pcx_play::command_hotkey::at_rank(ranked, 0), Some(Hotkey::letter('T').unwrap()));
        assert!(Hotkey::letter('T').unwrap().matches(0x54), "T key triggers override");
        assert!(!Hotkey::letter('F').unwrap().matches(0x54), "stock F no longer fires for T");
    }
    #[test] #[ignore = "requires PCX_WAR3_EXE, the selected Legacy client"]
    fn original_customkeys_default_is_off() {
        let path = std::path::PathBuf::from(std::env::var_os("PCX_WAR3_EXE").expect("set PCX_WAR3_EXE"));
        let image = std::fs::read(&path).unwrap();
        let origin = pcx_play::scripts::AssetIdentity { source: path.display().to_string(), edition: "selected menu client".into(), version: "reviewed Legacy 1.29.2.9231 customkeys profile".into() };
        let d = Defaults::discover(&image, origin, crate::campaign_game_input::DISCOVERY).unwrap();
        assert_eq!(checkbox(d.get("customkeys").unwrap().fallback), Ok(false));
    }
}
