//! Gameplay checkboxes other than Tooltips and Disable Mouse Scroll.
//! Only CustomKeys is interactive: CustomKeys is read from
//! Navigation.applied by `campaign_custom_keys`, which loads `CustomKeys.txt`
//! once per session and applies per-section/field/rank hotkey overrides to
//! the command card. HealthBars, Subgroup, FormationToggle and AutosaveReplay
//! have no admitted runtime consumer and stay read-only preview in
//! `original_options_preview`; they are not enabled here.
//! Source defaults come from the selected client's option table via
//! `Options defaults`: the control-key join is the reviewed initializer join
//! in `docs/re-notes/original-options-initialization.md` (also
//! `original_options_preview::join`). Only exact `0`/`1` fallbacks admit.
use super::*;
use pcx_play::options_defaults::Defaults;

pub(crate) const CONTROLS: [(&str, &str); 1] = [("CustomKeysCheckBox", "customkeys")];
pub(crate) fn supports(name: &str) -> bool {
    CONTROLS.iter().any(|(c, _)| *c == name)
}
fn key(name: &str) -> Option<&'static str> {
    CONTROLS.iter().find(|(c, _)| *c == name).map(|(_, k)| *k)
}
fn checkbox(raw: &[u8]) -> Result<bool, String> {
    match raw {
        b"0" => Ok(false),
        b"1" => Ok(true),
        _ => Err("fallback outside 0/1 checkbox profile".into()),
    }
}
pub(crate) fn defaults(
    launcher: Option<&crate::launch_gui::Launcher>,
) -> Result<std::collections::BTreeMap<String, bool>, String> {
    let install = launcher.and_then(crate::launch_gui::selected_install);
    crate::menu_lighting::with_client_image(install.as_deref(), |source, image| {
        let origin = pcx_play::scripts::AssetIdentity {
            source: source.into(),
            edition: "selected menu client".into(),
            version: "reviewed Legacy 1.29.2.9231 gameplay options profile".into(),
        };
        let defaults = Defaults::discover(image, origin, crate::campaign_game_input::DISCOVERY)
            .map_err(|e| e.to_string())?;
        let mut out = std::collections::BTreeMap::new();
        for (control, _) in CONTROLS {
            let raw = defaults
                .get(key(control).unwrap())
                .ok_or(format!("{control}: source preference missing"))?
                .fallback;
            out.insert(
                control.to_string(),
                checkbox(raw).map_err(|e| format!("{control}: {e}"))?,
            );
        }
        Ok(out)
    })
}
pub(super) fn attach(
    commands: &mut Commands,
    entity: Entity,
    control: &SettingControl,
    nav: &mut Navigation,
    seed: bool,
) -> Result<(), String> {
    if !matches!(control.widget, Widget::Checkbox) {
        return Err("incompatible gameplay control".into());
    }
    let applied = *nav
        .applied
        .toggles
        .entry(control.name.clone())
        .or_insert(seed);
    let draft = *nav
        .draft
        .toggles
        .entry(control.name.clone())
        .or_insert(applied);
    commands.entity(entity).insert(Checkbox);
    if draft {
        commands.entity(entity).insert(Checked);
    } else {
        commands.entity(entity).remove::<Checked>();
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joins_match_preview() {
        for (c, k) in CONTROLS {
            assert_eq!(crate::menu_settings::preview::join_for_test(c), Some(k));
        }
    }
    #[test]
    fn attach_draft_cancel_save_reload() {
        let mut world = World::new();
        let entity = world.spawn_empty().id();
        let mut nav = Navigation::default();
        let control = SettingControl {
            name: "CustomKeysCheckBox".into(),
            widget: Widget::Checkbox,
        };
        attach(&mut world.commands(), entity, &control, &mut nav, false).unwrap();
        world.flush();
        assert!(world.get::<Checked>(entity).is_none());
        world.entity_mut(entity).insert(control);
        world.insert_resource(nav);
        world.insert_resource(Backend::default());
        world.add_observer(super::super::checkbox_changed);
        world.trigger(ValueChange {
            source: entity,
            value: true,
            is_final: true,
        });
        world.flush();
        let mut nav = world.remove_resource::<Navigation>().unwrap();
        assert!(!nav.applied.toggles["CustomKeysCheckBox"]);
        assert!(nav.draft.toggles["CustomKeysCheckBox"]);
        nav.action("CancelButton");
        assert!(!nav.draft.toggles["CustomKeysCheckBox"]);
        nav.draft.toggles.insert("CustomKeysCheckBox".into(), true);
        let mut backend = world.remove_resource::<Backend>().unwrap();
        let mut saved = String::new();
        super::super::commit(&mut nav, &mut Window::default(), &mut backend, |t| {
            saved = t.into();
            Ok(())
        })
        .unwrap();
        assert!(
            super::super::decode_preferences(&saved)
                .unwrap()
                .settings
                .toggles["CustomKeysCheckBox"]
        );
        let control = SettingControl {
            name: "CustomKeysCheckBox".into(),
            widget: Widget::Checkbox,
        };
        attach(&mut world.commands(), entity, &control, &mut nav, false).unwrap();
        world.flush();
        assert!(
            world.get::<Checked>(entity).is_some(),
            "saved true overrides source false"
        );
    }
    #[test]
    fn saved_applied_state_drives_consumer_gate() {
        // campaign_custom_keys::enabled is the consumer gate: applied true
        // loads CustomKeys.txt, applied false skips it. Replicate the gate
        // outcomes without file I/O by exercising the same saved-wins rule
        // through decode_preferences: a persisted true must round-trip and
        // remain distinct from an absent toggle.
        let mut nav = Navigation::default();
        nav.draft
            .toggles
            .insert("CustomKeysCheckBox".into(), true);
        let mut backend = Backend::default();
        let mut saved = String::new();
        super::super::commit(&mut nav, &mut Window::default(), &mut backend, |text| {
            saved = text.into();
            Ok(())
        })
        .unwrap();
        let prefs = super::super::decode_preferences(&saved).unwrap();
        assert_eq!(prefs.settings.toggles.get("CustomKeysCheckBox"), Some(&true));
        assert_eq!(
            crate::campaign_custom_keys::enabled_for_test(true, Some(b"0")),
            Ok(true)
        );
        assert_eq!(
            crate::campaign_custom_keys::enabled_for_test(false, Some(b"1")),
            Ok(false)
        );
        // HealthBars etc. were never admitted: decoding them must refuse.
        let mut bad = saved.clone();
        bad = bad.replacen("CustomKeysCheckBox", "HealthBarsCheckBox", 1);
        assert!(super::super::decode_preferences(&bad).is_err());
    }
    #[test]
    fn applied_true_loads_file_and_reaches_command_card_button() {
        use crate::campaign_command_card::{CommandCard, Kind as CardKind};
        use pcx_play::command_hotkey::{CustomKeys, Field, Hotkey};
        use pcx_script::HandleId;
        let path = std::path::PathBuf::from("user");
        let read = |p: &std::path::Path| {
            assert_eq!(p, path);
            Ok(b"[hfoo]\nHotkey=T\n".to_vec())
        };
        let loaded = crate::campaign_custom_keys::applied_load_for_test(
            Some(true),
            Some(b"0"),
            std::slice::from_ref(&path),
            read,
        )
        .unwrap()
        .unwrap();
        assert_eq!(loaded, CustomKeys::parse("[hfoo]\nHotkey=T\n").unwrap());
        let no_read = |_: &std::path::Path| -> std::io::Result<Vec<u8>> {
            panic!("applied false must not read CustomKeys.txt")
        };
        assert_eq!(
            crate::campaign_custom_keys::applied_load_for_test(
                Some(false),
                Some(b"1"),
                std::slice::from_ref(&path),
                no_read,
            )
            .unwrap(),
            None
        );
        let unit = HandleId::new(std::num::NonZeroU64::new(7).unwrap());
        let train = CardKind::Train(i32::from_be_bytes(*b"uNEW"));
        let mut card = CommandCard::default();
        card.show_buttons(unit, &[(train, Some(Hotkey::letter('F').unwrap()))]);
        let resolved = match loaded.get("hfoo", Field::Hotkey) {
            Some(custom) => pcx_play::command_hotkey::at_rank(custom, 0),
            None => Some(Hotkey::letter('F').unwrap()),
        };
        assert_eq!(resolved, Some(Hotkey::letter('T').unwrap()));
        card.show_buttons(unit, &[(train, resolved)]);
        assert_eq!(card.button_for_keys(unit, &[0x54]).unwrap().kind, train);
        assert_eq!(card.button_for_keys(unit, &[0x46]), None);
    }
    #[test]
    fn malformed_fallback_refuses() {
        assert!(checkbox(b"2").is_err());
        assert!(checkbox(b"").is_err());
        assert_eq!(checkbox(b"1"), Ok(true));
    }
    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL with selected Legacy client"]
    fn original_gameplay_defaults_admit() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original install required"),
        );
        let image = std::fs::read(install.join("Warcraft III.exe")).unwrap();
        let origin = pcx_play::scripts::AssetIdentity {
            source: install.display().to_string(),
            edition: "selected menu client".into(),
            version: "reviewed Legacy 1.29.2.9231 gameplay options profile".into(),
        };
        let d = Defaults::discover(&image, origin, crate::campaign_game_input::DISCOVERY).unwrap();
        for (c, k) in CONTROLS {
            assert!(checkbox(d.get(k).unwrap().fallback).is_ok(), "{c}");
        }
    }
}
