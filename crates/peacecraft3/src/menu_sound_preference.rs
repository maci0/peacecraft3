//! Unit-acknowledgement sound preference (`UnitCheckBox` → `unit`).
//!
//! Source key `unit` (see `docs/re-notes/original-options-initialization.md`)
//! gates the `campaign_ack` producer. The fallback is read from the selected
//! client's descriptor table at runtime; no default value is copied here.

use super::*;
use pcx_play::neutral_player::RuleInputBudget;
use pcx_play::options_defaults::{Defaults, DiscoveryBudget};

pub(crate) const UNIT_TOGGLE: &str = "UnitCheckBox";
pub(crate) const UNIT_KEY: &str = "unit";

const DISCOVERY: DiscoveryBudget = DiscoveryBudget {
    image: RuleInputBudget {
        image_bytes: 32 << 20,
        profile_bytes: 128 << 10,
        sections: 96,
    },
    string_bytes: 4096,
    candidate_limit: 32,
    work_bytes: 64 << 20,
};

pub(crate) fn supports(name: &str) -> bool {
    name == UNIT_TOGGLE
}

/// Whether the applied `unit` preference enables the campaign_ack producer.
/// Absent applied state (no saved pref, no source seed yet) means enabled:
/// the original default is `1` in the qualified client, and an unseeded
/// checkbox must not silence voices. Only an explicit applied `false` gates.
pub(crate) fn unit_voices_enabled(settings: Option<&Settings>) -> bool {
    settings
        .and_then(|s| s.toggles.get(UNIT_TOGGLE))
        .copied()
        .unwrap_or(true)
}

/// Source fallback for the `unit` descriptor: only exact `0`/`1` admitted.
pub(crate) fn unit_default(
    image: &[u8],
    origin: pcx_play::scripts::AssetIdentity,
) -> Result<bool, String> {
    let defaults = Defaults::discover(image, origin, DISCOVERY).map_err(|e| e.to_string())?;
    let raw = defaults
        .get(UNIT_KEY)
        .ok_or("unit preference key is absent from the descriptor table")?
        .fallback;
    checkbox(raw)
}

fn checkbox(raw: &[u8]) -> Result<bool, String> {
    match raw {
        b"0" => Ok(false),
        b"1" => Ok(true),
        _ => Err("unit fallback is outside the zero/one checkbox profile".into()),
    }
}

pub(crate) fn attach(
    commands: &mut Commands,
    entity: Entity,
    control: &SettingControl,
    nav: &mut Navigation,
    seed: bool,
) -> Result<(), String> {
    if !matches!(control.widget, Widget::Checkbox) || control.name != UNIT_TOGGLE {
        return Err("incompatible unit-sound control".into());
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
    fn absent_applied_state_keeps_voices_enabled_only_explicit_false_gates() {
        assert!(unit_voices_enabled(None));
        assert!(unit_voices_enabled(Some(&Settings::default())));
        let mut s = Settings::default();
        s.toggles.insert(UNIT_TOGGLE.into(), true);
        assert!(unit_voices_enabled(Some(&s)));
        s.toggles.insert(UNIT_TOGGLE.into(), false);
        assert!(!unit_voices_enabled(Some(&s)));
    }

    #[test]
    fn checkbox_profile_is_exact_zero_or_one() {
        assert_eq!(checkbox(b"0"), Ok(false));
        assert_eq!(checkbox(b"1"), Ok(true));
        for raw in [b"" as &[u8], b"2", b"true", b" 1"] {
            assert!(checkbox(raw).is_err(), "{raw:?}");
        }
    }

    #[test]
    fn unit_checkbox_draft_cancel_save_and_reload() {
        let mut world = World::new();
        let control = SettingControl {
            name: UNIT_TOGGLE.into(),
            widget: Widget::Checkbox,
        };
        let entity = world.spawn_empty().id();
        let mut nav = Navigation::default();
        attach(&mut world.commands(), entity, &control, &mut nav, true).unwrap();
        world.flush();
        assert!(world.get::<Checked>(entity).is_some());
        world.entity_mut(entity).insert(control);
        world.insert_resource(nav);
        world.insert_resource(Backend::default());
        world.add_observer(super::super::checkbox_changed);
        world.trigger(bevy::ui_widgets::ValueChange {
            source: entity,
            value: false,
            is_final: true,
        });
        world.flush();
        let mut nav = world.remove_resource::<Navigation>().unwrap();
        assert!(nav.applied.toggles[UNIT_TOGGLE]);
        assert!(!nav.draft.toggles[UNIT_TOGGLE]);
        // Draft false must not gate the producer until applied.
        assert!(unit_voices_enabled(Some(&nav.applied)));
        nav.action("CancelButton");
        assert!(nav.draft.toggles[UNIT_TOGGLE]);
        nav.draft.toggles.insert(UNIT_TOGGLE.into(), false);
        assert!(
            unit_voices_enabled(Some(&nav.applied)),
            "draft must not gate"
        );
        let mut backend = world.remove_resource::<Backend>().unwrap();
        let mut saved = String::new();
        super::super::commit(&mut nav, &mut Window::default(), &mut backend, |text| {
            saved = text.into();
            Ok(())
        })
        .unwrap();
        let reloaded = super::super::decode_preferences(&saved).unwrap();
        assert!(!reloaded.settings.toggles[UNIT_TOGGLE]);
        assert!(!unit_voices_enabled(Some(&reloaded.settings)));
    }
}
