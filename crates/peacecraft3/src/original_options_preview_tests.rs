use super::*;
use bevy::{
    ecs::system::RunSystemOnce,
    ui_widgets::{Activate, Button, Checkbox, Slider, ValueChange},
};
use pcx_play::{
    neutral_player::RuleInputBudget,
    options_defaults::{Defaults, DiscoveryBudget},
    scripts::AssetIdentity,
};
fn put(d: &mut [u8], at: usize, v: u32) {
    d[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn named_fixture(
    base: u32,
    limit: u8,
    first_name: &str,
    second_name: &str,
    first: &[u8],
    second: &[u8],
) -> (Vec<u8>, AssetIdentity, DiscoveryBudget) {
    let mut d = vec![0; 0x800];
    d[..2].copy_from_slice(b"MZ");
    d[0x80..0x84].copy_from_slice(b"PE\0\0");
    for (at, v) in [(0x84, 0x14cu16), (0x86, 2), (0x94, 0x60), (0x98, 0x10b)] {
        d[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }
    for (at, v) in [
        (0x3c, 0x80),
        (0xb4, base),
        (0xd0, 0x3000),
        (0x100, 0x200),
        (0x104, 0x1000),
        (0x108, 0x200),
        (0x10c, 0x200),
        (0x11c, 0x60000020),
        (0x128, 0x400),
        (0x12c, 0x2000),
        (0x130, 0x400),
        (0x134, 0x400),
        (0x144, 0x40000040),
    ] {
        put(&mut d, at, v);
    }
    // Synthetic call destinations and operands; no game payload in the fixture.
    let mut code = vec![
        0x55, 0x8b, 0xec, 0x53, 0x56, 0x8b, 0x75, 8, 0x8b, 0xd9, 0x83, 0xfe, limit, 0x7c, 8, 0x5e,
        0x33, 0xc0, 0x5b, 0x5d, 0xc2, 12, 0, 0x57, 0x68,
    ];
    code.extend_from_slice(&(base + 0x2800).to_le_bytes());
    code.extend_from_slice(&[
        0x56, 0xe8, 1, 0, 0, 0, 0x85, 0xc0, 0x74, 5, 0x8d, 0x78, 0x18, 0xeb, 0x14, 0x8b, 0xc6,
        0x8b, 0xcb, 0xc1, 0xe0, 4, 0x8b, 0xb8,
    ]);
    code.extend_from_slice(&(base + 0x200c).to_le_bytes());
    code.extend_from_slice(&[
        0x57, 0x56, 0xe8, 2, 0, 0, 0, 0xff, 0x75, 0x10, 0x57, 0xff, 0x75, 12, 0xe8, 3, 0, 0, 0,
        0x83, 0xc4, 12, 0xb8, 1, 0, 0, 0, 0x5f, 0x5e, 0x5b, 0x5d, 0xc2, 12, 0,
    ]);
    d[0x220..0x220 + code.len()].copy_from_slice(&code);
    let mut cursor = 0x600;
    for index in 0..limit as usize {
        let unused = format!("unused.{index}");
        let (name, raw) = if index == 4 {
            (first_name, first)
        } else if index == 1 {
            (second_name, second)
        } else {
            (unused.as_str(), b"0".as_slice())
        };
        let at = 0x400 + index * 16;
        put(&mut d, at, base + 0x2000 + (cursor - 0x400) as u32);
        d[cursor..cursor + name.len()].copy_from_slice(name.as_bytes());
        cursor += name.len() + 1;
        put(&mut d, at + 4, index as u32 + 39);
        put(&mut d, at + 8, index as u32 + 71);
        put(&mut d, at + 12, base + 0x2000 + (cursor - 0x400) as u32);
        d[cursor..cursor + raw.len()].copy_from_slice(raw);
        cursor += raw.len() + 1;
    }
    (
        d,
        AssetIdentity {
            edition: "synthetic".into(),
            version: "fixture".into(),
            source: "synthetic:preferences.exe".into(),
        },
        DiscoveryBudget {
            image: RuleInputBudget {
                image_bytes: 4096,
                profile_bytes: 4096,
                sections: 2,
            },
            string_bytes: 256,
            candidate_limit: 2,
            work_bytes: 16384,
        },
    )
}

fn widget(frame: &str, kind: &str) -> Widget {
    let source = format!(
        r#"Frame "MENU" "Items" {{
MenuItem "LABEL_Z", -3,
MenuItem "LABEL_A", -3,
}}
Frame "{kind}" "{frame}" {{
PopupMenuFrame "Items",
SliderMinValue 0,
SliderMaxValue 100000000,
SliderInitialValue 5,
SliderStepSize 3,
}}
"#
    );
    crate::menu_screens::widget(&crate::fdf::Fdf::parse(&source).unwrap(), frame)
        .unwrap()
        .unwrap()
}
fn snapshot(frame: &str, key: &str, raw: &[u8], kind: &str, source_key: &str) -> Snapshot {
    let (image, binding, budget) = named_fixture(0x300000, 7, key, "unused", raw, b"0");
    let defaults = Defaults::discover(&image, binding, budget).unwrap();
    Snapshot::checked(
        &defaults,
        [(frame, &widget(frame, kind))],
        "LABEL_Z \"Zulu\",\nLABEL_A \"E|Cffffffffx|Rample\",\n",
        source_key.into(),
    )
}
#[test]
fn popup_order_markup_identity_and_source_key_are_preserved() {
    let s = snapshot(
        "ModelDetailMenu",
        "modeldetail",
        b"1",
        "POPUPMENU",
        "source-A",
    );
    assert!(s.matches("source-A"));
    assert!(!s.matches("source-B"));
    assert_eq!(
        s.value("ModelDetailMenu").unwrap().popup.as_deref(),
        Some("Example")
    );
    assert_eq!(
        s.identity.as_ref().unwrap().0.source,
        "synthetic:preferences.exe"
    );
    assert!(s.notice().contains("not applied"));
    let changed = snapshot(
        "ModelDetailMenu",
        "modeldetail",
        b"0",
        "POPUPMENU",
        "source-B",
    );
    assert_eq!(
        changed.value("ModelDetailMenu").unwrap().popup.as_deref(),
        Some("Zulu")
    );
}
#[test]
fn checkbox_and_slider_restricted_profiles_preserve_raw_units() {
    for (raw, expected) in [
        (b"0".as_slice(), Some(false)),
        (b"1", Some(true)),
        (b"2", None),
        (b"01", None),
        (b"-1", None),
        (b" 1", None),
        (b"", None),
        (b"\xff", None),
    ] {
        let s = snapshot("TooltipsCheckBox", "tooltips", raw, "CHECKBOX", "A");
        assert_eq!(
            s.value("TooltipsCheckBox").and_then(|v| v.checkbox),
            expected
        );
    }
    for (raw, expected) in [
        (b"7".as_slice(), Some(7.0)),
        (b"16777216", Some(16777216.0)),
        (b"16777217", None),
        (b"2147483648", None),
        (b"100000001", None),
        (b"-1", None),
        (b"1.0", None),
        (b" 7", None),
    ] {
        let s = snapshot("GammaSlider", "gamma", raw, "SLIDER", "A");
        assert_eq!(s.value("GammaSlider").and_then(|v| v.slider), expected);
    }
}
#[test]
fn localization_ambiguity_missing_markup_empty_and_budget_refuse() {
    let (image, binding, budget) = named_fixture(0x300000, 7, "modeldetail", "unused", b"0", b"0");
    let d = Defaults::discover(&image, binding, budget).unwrap();
    let w = widget("ModelDetailMenu", "POPUPMENU");
    for strings in [
        "",
        "LABEL_Z \"one\",\nLABEL_Z \"two\",",
        "LABEL_Z \"bad|Cff\",",
        "LABEL_Z \"\",",
        "LABEL_Z \"value\" garbage,",
    ] {
        assert!(
            Snapshot::checked(&d, [("ModelDetailMenu", &w)], strings, "A".into())
                .value("ModelDetailMenu")
                .is_none()
        );
    }
    let huge = format!("LABEL_Z \"{}\",", "x".repeat(LABEL_BYTES + 1));
    assert!(
        Snapshot::checked(&d, [("ModelDetailMenu", &w)], &huge, "A".into())
            .value("ModelDetailMenu")
            .is_none()
    );
    let mut remaining = 0;
    assert!(
        selected(
            &d,
            &w,
            "ModelDetailMenu",
            "LABEL_Z \"text\",",
            &mut remaining
        )
        .is_err()
    );
    assert_eq!(remaining, 0);
    let empty = Widget::Popup { choices: vec![] };
    assert!(
        selected(
            &d,
            &empty,
            "ModelDetailMenu",
            "LABEL_Z \"text\",",
            &mut 1024
        )
        .is_err()
    );
}
#[test]
fn source_replacement_and_unavailable_reset_art_without_behavior() {
    let mut world = World::new();
    let owner = world.spawn(Button).id();
    let w = Widget::Checkbox;
    attach(
        &mut world.commands(),
        owner,
        "TooltipsCheckBox",
        &w,
        &snapshot("TooltipsCheckBox", "tooltips", b"1", "CHECKBOX", "A"),
    );
    world.flush();
    assert!(world.get::<Checked>(owner).is_some());
    assert!(world.get::<Checkbox>(owner).is_none());
    assert!(world.get::<Slider>(owner).is_none());
    let a = world.get::<AccessibilityNode>(owner).unwrap();
    assert!(a.is_disabled());
    assert!(a.description().unwrap().contains("not applied"));
    assert!(a.toggled().is_none());
    attach(
        &mut world.commands(),
        owner,
        "TooltipsCheckBox",
        &w,
        &snapshot("TooltipsCheckBox", "tooltips", b"0", "CHECKBOX", "B"),
    );
    world.flush();
    assert!(world.get::<Checked>(owner).is_none());
    let sw = widget("GammaSlider", "SLIDER");
    attach(
        &mut world.commands(),
        owner,
        "GammaSlider",
        &sw,
        &snapshot("GammaSlider", "gamma", b"7", "SLIDER", "A"),
    );
    world.flush();
    assert_eq!(world.get::<SliderValue>(owner).unwrap().0, 7.0);
    assert!(world.get::<Slider>(owner).is_none());
    attach(
        &mut world.commands(),
        owner,
        "GammaSlider",
        &sw,
        &Snapshot::unavailable("B".into(), "No admitted client defaults."),
    );
    world.flush();
    assert!(world.get::<SliderValue>(owner).is_none());
    assert!(world.get::<SliderRange>(owner).is_none());
    assert!(!world.get::<Control>(owner).unwrap().available);
    assert!(
        world
            .get::<AccessibilityNode>(owner)
            .unwrap()
            .description()
            .unwrap()
            .contains("unavailable")
    );
}
#[test]
fn active_controls_and_omitted_fields_never_become_previews() {
    let mut world = World::new();
    let owner = world.spawn((Button, Checked, SliderValue(37.0))).id();
    for frame in [
        "MusicCheckBox",
        "SoundCheckBox",
        "MusicVolumeSlider",
        "SoundVolumeSlider",
        "ResolutionMenu",
    ] {
        attach(
            &mut world.commands(),
            owner,
            frame,
            &Widget::Checkbox,
            &Snapshot::default(),
        );
        world.flush();
        assert!(world.get::<Control>(owner).is_none());
        assert!(world.get::<InteractionDisabled>(owner).is_none());
        assert!(world.get::<Checked>(owner).is_some());
        assert_eq!(world.get::<SliderValue>(owner).unwrap().0, 37.0);
    }
    for frame in [
        "GamePortEditBox",
        "FixedAspectRatioCheckBox",
        "SpellFilterMenu",
        "unknown",
    ] {
        assert!(!eligible(frame));
    }
}
#[test]
fn genuine_event_handlers_cannot_mutate_disabled_preview_settings() {
    use super::super::{Backend, checkbox_changed, slider_changed};
    let mut app = App::new();
    app.init_resource::<crate::menu_screens::Navigation>()
        .init_resource::<Backend>()
        .add_observer(slider_changed)
        .add_observer(checkbox_changed);
    crate::button_actions::install(&mut app);
    let checkbox = app
        .world_mut()
        .spawn((
            Button,
            crate::menu_screens::SettingControl {
                name: "TooltipsCheckBox".into(),
                widget: Widget::Checkbox,
            },
        ))
        .id();
    attach(
        &mut app.world_mut().commands(),
        checkbox,
        "TooltipsCheckBox",
        &Widget::Checkbox,
        &snapshot("TooltipsCheckBox", "tooltips", b"1", "CHECKBOX", "A"),
    );
    app.world_mut().flush();
    let sw = widget("GammaSlider", "SLIDER");
    let slider = app
        .world_mut()
        .spawn((
            Button,
            crate::menu_screens::SettingControl {
                name: "GammaSlider".into(),
                widget: sw.clone(),
            },
        ))
        .id();
    attach(
        &mut app.world_mut().commands(),
        slider,
        "GammaSlider",
        &sw,
        &snapshot("GammaSlider", "gamma", b"7", "SLIDER", "A"),
    );
    app.world_mut().flush();
    let before = {
        let nav = app.world().resource::<crate::menu_screens::Navigation>();
        format!("{:?}/{:?}", nav.applied, nav.draft)
    };
    let audio_before = {
        let audio = &app.world().resource::<Backend>().audio;
        (
            audio.music_gain.to_bits(),
            audio.effects_gain.to_bits(),
            audio.music_enabled,
            audio.effects_enabled,
        )
    };
    app.world_mut().trigger(ValueChange {
        source: checkbox,
        value: false,
        is_final: true,
    });
    app.world_mut().trigger(ValueChange {
        source: slider,
        value: 99.0,
        is_final: true,
    });
    app.world_mut().trigger(Activate { entity: checkbox });
    app.world_mut().trigger(Activate { entity: slider });
    for entity in [checkbox, slider] {
        app.world_mut()
            .trigger(bevy::picking::events::PointerClick {
                entity,
                pointer: bevy::picking::events::Pointer::new(
                    bevy::picking::pointer::PointerId::Mouse,
                    bevy::picking::pointer::Location {
                        target: bevy::camera::NormalizedRenderTarget::None {
                            width: 1,
                            height: 1,
                        },
                        position: Vec2::ZERO,
                    },
                ),
                button: bevy::picking::pointer::PointerButton::Primary,
                hit: bevy::picking::backend::HitData::new(entity, 0.0, None, None),
                count: 1,
                duration: std::time::Duration::ZERO,
            });
    }

    app.world_mut().flush();
    let nav = app.world().resource::<crate::menu_screens::Navigation>();
    assert_eq!(format!("{:?}/{:?}", nav.applied, nav.draft), before);
    let audio = &app.world().resource::<Backend>().audio;
    assert_eq!(
        (
            audio.music_gain.to_bits(),
            audio.effects_gain.to_bits(),
            audio.music_enabled,
            audio.effects_enabled
        ),
        audio_before
    );
    assert!(
        app.world()
            .resource::<crate::button_actions::ButtonActions>()
            .entities
            .is_empty()
    );
    assert!(app.world().get::<Checked>(checkbox).is_some());
    assert_eq!(app.world().get::<SliderValue>(slider).unwrap().0, 7.0);
    app.world_mut()
        .run_system_once(crate::button_actions::clear)
        .unwrap();
}

#[test]
fn actual_slider_label_uses_preview_or_unavailable_instead_of_draft_or_initial() {
    let mut world = World::new();
    world.init_resource::<crate::menu_screens::Navigation>();
    world
        .resource_mut::<crate::menu_screens::Navigation>()
        .draft
        .values
        .insert("GammaSlider".into(), 2.0);
    let widget = widget("GammaSlider", "SLIDER");
    let owner = world
        .spawn((
            Button,
            crate::menu_screens::SettingControl {
                name: "GammaSlider".into(),
                widget: widget.clone(),
            },
        ))
        .id();
    attach(
        &mut world.commands(),
        owner,
        "GammaSlider",
        &widget,
        &snapshot("GammaSlider", "gamma", b"7", "SLIDER", "A"),
    );
    world.flush();
    let text = world
        .spawn((Text::new("stale"), crate::menu_screens::SettingValue(owner)))
        .id();
    world.run_system_once(super::super::labels).unwrap();
    assert_eq!(world.get::<Text>(text).unwrap().0, "7");
    attach(
        &mut world.commands(),
        owner,
        "GammaSlider",
        &widget,
        &Snapshot::unavailable("B".into(), "No admitted source defaults."),
    );
    world.flush();
    world.run_system_once(super::super::labels).unwrap();
    assert_eq!(world.get::<Text>(text).unwrap().0, "Unavailable");
    let nav = world.resource::<crate::menu_screens::Navigation>();
    assert_eq!(nav.draft.values["GammaSlider"], 2.0);
    assert!(nav.applied.values.is_empty());
}

#[test]
fn actual_label_consumer_shows_parsed_preview_without_draft_selection() {
    let mut world = World::new();
    world.init_resource::<crate::menu_screens::Navigation>();
    let widget = widget("ModelDetailMenu", "POPUPMENU");
    let owner = world
        .spawn((
            Button,
            crate::menu_screens::SettingControl {
                name: "ModelDetailMenu".into(),
                widget: widget.clone(),
            },
        ))
        .id();
    attach(
        &mut world.commands(),
        owner,
        "ModelDetailMenu",
        &widget,
        &snapshot("ModelDetailMenu", "modeldetail", b"1", "POPUPMENU", "A"),
    );
    world.flush();
    let text = world
        .spawn((Text::new("stale"), crate::menu_screens::SettingValue(owner)))
        .id();
    world.run_system_once(super::super::labels).unwrap();
    assert_eq!(world.get::<Text>(text).unwrap().0, "Example");
    assert_eq!(
        world.get::<Control>(owner).unwrap().popup.as_deref(),
        Some("Example")
    );
    let nav = world.resource::<crate::menu_screens::Navigation>();
    assert!(nav.applied.values.is_empty());
    assert!(nav.draft.values.is_empty());
}

#[test]
fn notice_error_priority_and_idle_restore_are_explicit() {
    let s = snapshot("TooltipsCheckBox", "tooltips", b"1", "CHECKBOX", "A");
    let c = Control {
        popup: None,
        available: true,
    };
    assert_eq!(
        notice("Save failed", true, true, Some(&c), &s),
        "Save failed"
    );
    assert_eq!(notice("", true, true, Some(&c), &s), "PeaceCraft settings");
    assert_eq!(notice("", false, true, Some(&c), &s), IDLE_NOTICE);
    assert_eq!(notice("", false, true, None, &s), IDLE_NOTICE);
    assert_eq!(notice("", false, false, None, &s), "");
    let unavailable = Snapshot::unavailable("B".into(), "Archive-only input");
    assert_eq!(
        notice("", false, true, None, &unavailable),
        "Defaults unavailable."
    );
    let unavailable_control = Control {
        popup: None,
        available: false,
    };
    assert!(
        unavailable
            .description("GammaSlider")
            .contains("Archive-only input")
    );
    assert_eq!(
        notice("", false, true, Some(&unavailable_control), &unavailable),
        "Setting unavailable."
    );
}
#[test]
fn all_retained_disabled_joins_are_source_derived() {
    for (frame, key, kind) in [
        ("ModelDetailMenu", "modeldetail", "POPUPMENU"),
        ("AnimQualityMenu", "animquality", "POPUPMENU"),
        ("TextureQualityMenu", "texquality", "POPUPMENU"),
        ("ParticlesMenu", "particles", "POPUPMENU"),
        ("LightsMenu", "lights", "POPUPMENU"),
        ("ShadowsMenu", "unitshadows", "POPUPMENU"),
        ("OcclusionMenu", "occlusion", "POPUPMENU"),
        ("ChatSupportMenu", "chatsupport", "POPUPMENU"),
        ("ProviderMenu", "provider", "POPUPMENU"),
        ("MouseScrollSlider", "mousescroll", "SLIDER"),
        ("KeyScrollSlider", "keyscroll", "SLIDER"),
        ("GammaSlider", "gamma", "SLIDER"),
        (
            "MouseScrollDisableCheckBox",
            "mousescrolldisable",
            "CHECKBOX",
        ),
        ("TooltipsCheckBox", "tooltips", "CHECKBOX"),
        ("HealthBarsCheckBox", "healthbars", "CHECKBOX"),
        ("SubgroupCheckBox", "subgrouporder", "CHECKBOX"),
        ("FormationToggleCheckBox", "formationtoggle", "CHECKBOX"),
        ("CustomKeysCheckBox", "customkeys", "CHECKBOX"),
        ("AutosaveReplayCheckBox", "autosaveReplay", "CHECKBOX"),
        ("AmbientCheckBox", "ambient", "CHECKBOX"),
        ("MovementCheckBox", "movement", "CHECKBOX"),
        ("UnitCheckBox", "unit", "CHECKBOX"),
        ("SubtitlesCheckBox", "subtitles", "CHECKBOX"),
        ("EnviroCheckBox", "environmental", "CHECKBOX"),
        ("PositionalCheckBox", "positional", "CHECKBOX"),
    ] {
        assert!(eligible(frame));
        let s = snapshot(frame, key, b"1", kind, "A");
        let value = s.value(frame).unwrap();
        match kind {
            "POPUPMENU" => assert_eq!(value.popup.as_deref(), Some("Example")),
            "SLIDER" => assert_eq!(value.slider, Some(1.0)),
            _ => assert_eq!(value.checkbox, Some(true)),
        }
    }
}
#[test]
fn duplicate_controls_and_empty_dynamic_lists_are_explicitly_unavailable() {
    let (image, binding, budget) = named_fixture(0x300000, 7, "provider", "unused", b"0", b"0");
    let d = Defaults::discover(&image, binding, budget).unwrap();
    let empty = Widget::Popup { choices: vec![] };
    let s = Snapshot::checked(&d, [("ProviderMenu", &empty)], "", "A".into());
    assert!(s.value("ProviderMenu").is_none());
    assert!(s.description("ProviderMenu").contains("no authored choice"));
    let duplicate = Snapshot::checked(
        &d,
        [("ProviderMenu", &empty), ("ProviderMenu", &empty)],
        "",
        "A".into(),
    );
    assert!(duplicate.identity.is_none());
    assert!(duplicate.description("ProviderMenu").contains("duplicate"));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn optional_discovery_failure_and_exact_synthetic_client_callback_are_separate() {
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let path = std::env::temp_dir().join(format!(
        "peacecraft3-preview-synthetic-{}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    let temp = Temp(path);
    let w = Widget::Checkbox;
    let absent = super::super::source_preview(
        Some(&temp.0),
        true,
        [("TooltipsCheckBox", &w)],
        Some(""),
        "A".into(),
    );
    assert!(absent.identity.is_none());
    let (image, _, _) = named_fixture(0x300000, 7, "tooltips", "unused", b"1", b"0");
    std::fs::write(temp.0.join("Warcraft III.exe"), &image).unwrap();
    let ready = super::super::source_preview(
        Some(&temp.0),
        true,
        [("TooltipsCheckBox", &w)],
        Some(""),
        "B".into(),
    );
    assert_eq!(
        ready.value("TooltipsCheckBox").unwrap().checkbox,
        Some(true)
    );
    assert!(ready.matches("B"));
    std::fs::write(
        temp.0.join("Warcraft III.exe"),
        b"malformed synthetic image",
    )
    .unwrap();
    let malformed = super::super::source_preview(
        Some(&temp.0),
        true,
        [("TooltipsCheckBox", &w)],
        Some(""),
        "C".into(),
    );
    assert!(malformed.identity.is_none());
    assert!(
        malformed
            .description("TooltipsCheckBox")
            .contains("could not be admitted")
    );
    // The optional API returns a display status, not an audio/preference error.
    assert!(malformed.matches("C"));
}

#[test]
fn widget_collection_uses_authoritative_override_and_refuses_bad_later_item() {
    let source = r#"Frame "SLIDER" "GammaSlider" {
SliderMinValue 9,
SliderMaxValue 2,
SliderInitialValue 100,
SliderStepSize 0,
}
Frame "SLIDER" "GammaSlider" {
SliderMinValue 0,
SliderMaxValue 10,
SliderInitialValue 4,
SliderStepSize 3,
}
"#;
    let fdf = crate::fdf::Fdf::parse(source).unwrap();
    let controls = widgets(&fdf).unwrap();
    assert_eq!(controls.len(), 1);
    assert!(matches!(controls[0].1, Widget::Slider { initial: 4.0, .. }));
    let broken = crate::fdf::Fdf::parse(
        r#"Frame "MENU" "Items" {
MenuItem "LABEL_Z", -3,
MenuItem BROKEN, -3,
}
Frame "POPUPMENU" "ModelDetailMenu" {
PopupMenuFrame "Items",
}
"#,
    )
    .unwrap();
    assert!(widgets(&broken).is_err());
}

/// Explicit local-input gate. Values and localized captions stay in memory;
/// failure messages deliberately exclude their contents. This is source/component
/// admission evidence, not effective first-run state or rendered-pixel proof.
#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires explicit local selected install and expected complete client SHA256"]
fn original_disabled_preview_matches_selected_source_and_widget_bindings() {
    use crate::menu_screens::{Navigation, Page, Screen, SettingControl, SettingValue};
    let install = std::env::var_os("PEACECRAFT3_OPTIONS_INSTALL")
        .expect("set PEACECRAFT3_OPTIONS_INSTALL to the explicitly selected install");
    let install = std::path::Path::new(&install);
    let expected_sha = std::env::var("PEACECRAFT3_OPTIONS_IMAGE_SHA256")
        .expect("set PEACECRAFT3_OPTIONS_IMAGE_SHA256 to its independently reviewed digest");
    assert!(
        expected_sha.len() == 64 && expected_sha.bytes().all(|b| b.is_ascii_hexdigit()),
        "expected complete image digest must be hexadecimal SHA256"
    );
    let (fdf, _) = crate::launch_gui::read_screen_fdf(Some(install), Screen::Options(Page::Video))
        .unwrap_or_else(|_| panic!("selected Options FDF/include graph refused"));
    // Same selected archive priority and member as MenuArtCache's native strings
    // path, with test-local packed/decoded preflight before member allocation.
    const STRINGS_BYTES: usize = 8 * 1024 * 1024;
    let mut archives = pcx_mpq::MpqSet::open_wc3(install)
        .unwrap_or_else(|_| panic!("selected archive set refused"));
    let (_, block) = archives
        .locate(crate::mainmenu::GLOBAL_STRINGS_FDF)
        .unwrap_or_else(|| panic!("selected localization member missing"));
    assert!(
        u64::from(block.compressed_size) <= STRINGS_BYTES as u64
            && u64::from(block.file_size) <= STRINGS_BYTES as u64,
        "selected localization member exceeds test policy"
    );
    let strings = archives
        .extract(crate::mainmenu::GLOBAL_STRINGS_FDF)
        .unwrap_or_else(|_| panic!("selected localization member refused"));
    let strings = std::str::from_utf8(&strings)
        .unwrap_or_else(|_| panic!("selected localization member is not UTF8"));
    let controls = widgets(&fdf).unwrap_or_else(|_| panic!("source widget collection refused"));
    assert!(!controls.is_empty(), "no source preview controls admitted");

    // Compare against the actual per-page control-layer admission, not a second
    // handwritten inventory. Indexed FDF overrides must retain the same binding.
    let viewport = (crate::mainmenu::FRAME_W, crate::mainmenu::FRAME_H);
    let mut rendered = BTreeMap::new();
    for page in [Page::Video, Page::Sound, Page::Gameplay] {
        let screen = Screen::Options(page);
        let (items, _) =
            crate::glue_menu::control_items_for(&fdf, screen.root(), viewport, |name| {
                screen.includes(&fdf, name)
            })
            .unwrap_or_else(|_| panic!("source page control-layer admission refused"));
        for item in items {
            if let crate::glue_menu::Kind::Widget(widget) = item.kind
                && eligible(&item.frame)
                && let Some(previous) = rendered.insert(item.frame, widget.clone())
            {
                assert!(
                    previous == widget,
                    "page binding changed across source pages"
                );
            }
        }
    }
    assert!(
        rendered.len() == controls.len(),
        "source binding sets differ"
    );
    for (frame, widget) in &controls {
        assert!(
            rendered.get(*frame) == Some(widget),
            "source widget identity differs"
        );
        assert!(
            crate::menu_screens::widget(&fdf, frame)
                .is_ok_and(|binding| binding.as_ref() == Some(widget)),
            "indexed source widget changed"
        );
    }
    let source_key = format!(
        "local-original-preview:{}:V1",
        expected_sha.to_ascii_lowercase()
    );
    let snapshot = super::super::source_preview(
        Some(install),
        true,
        controls.iter().map(|(name, widget)| (*name, widget)),
        Some(strings),
        source_key.clone(),
    );
    assert!(
        snapshot.matches(&source_key),
        "candidate source key differs"
    );
    let (origin, sha) = snapshot
        .identity
        .as_ref()
        .unwrap_or_else(|| panic!("selected client has no admitted preview identity"));
    let actual_sha: String = sha.iter().map(|byte| format!("{byte:02x}")).collect();
    assert!(
        actual_sha == expected_sha.to_ascii_lowercase(),
        "complete client digest differs"
    );
    assert!(
        origin.source == install.join("Warcraft III.exe").display().to_string(),
        "client role differs"
    );

    // Independently project the restricted profile from actual raw fallback bytes
    // and parsed widgets. Do not use selected()/Snapshot::checked as the oracle.
    let expected = super::super::with_source_defaults(install, true, |defaults| {
        assert!(
            defaults.binding().image_sha256 == *sha,
            "client changed during gate"
        );
        assert!(
            defaults.binding().origin == *origin,
            "client origin changed during gate"
        );
        let mut result = BTreeMap::new();
        let mut remaining = TOTAL_LABEL_BYTES;
        for (frame, widget) in &controls {
            let (key, kind) = join(frame).expect("source binding has a preview join");
            let raw = defaults.get(key).map(|entry| entry.fallback);
            let integer = raw
                .filter(|raw| !raw.is_empty() && raw.iter().all(u8::is_ascii_digit))
                .and_then(|raw| std::str::from_utf8(raw).ok())
                .and_then(|raw| raw.parse::<usize>().ok());
            let value = match (kind, widget) {
                (Kind::Checkbox, Widget::Checkbox) => raw
                    .and_then(|raw| match raw {
                        b"0" => Some(false),
                        b"1" => Some(true),
                        _ => None,
                    })
                    .map(|checkbox| Value {
                        checkbox: Some(checkbox),
                        ..default()
                    }),
                (
                    Kind::Slider,
                    Widget::Slider {
                        min,
                        max,
                        initial,
                        step,
                    },
                ) => integer
                    .and_then(|value| i32::try_from(value).ok())
                    .filter(|&value| f64::from(value as f32) == f64::from(value))
                    .map(|value| value as f32)
                    .filter(|value| {
                        min.is_finite()
                            && max.is_finite()
                            && (max - min).is_finite()
                            && min < max
                            && step.is_finite()
                            && *step > 0.0
                            && initial.is_finite()
                            && (*min..=*max).contains(initial)
                            && (*min..=*max).contains(value)
                    })
                    .map(|slider| Value {
                        slider: Some(slider),
                        ..default()
                    }),
                (Kind::Popup, Widget::Popup { choices }) => integer
                    .and_then(|index| choices.get(index))
                    .and_then(|key| localized(strings, key).ok())
                    .filter(|text| text.len() <= LABEL_BYTES && text.len() <= remaining)
                    .and_then(|text| {
                        crate::menu_text::Label::parse(text)
                            .ok()
                            .map(|label| (text.len(), label.visible()))
                    })
                    .filter(|(_, visible)| !visible.trim().is_empty())
                    .map(|(len, popup)| {
                        remaining -= len;
                        Value {
                            popup: Some(popup),
                            ..default()
                        }
                    }),
                _ => None,
            };
            result.insert(*frame, value);
        }
        Ok(result)
    })
    .unwrap_or_else(|_| panic!("independent selected client discovery refused"))
    .unwrap_or_else(|| panic!("selected client discovery became unavailable"));

    let mut world = World::new();
    world.init_resource::<Navigation>();
    let before = world.resource::<Navigation>().draft.clone();
    let applied = world.resource::<Navigation>().applied.clone();
    let mut owners = Vec::new();
    for (frame, widget) in &controls {
        let owner = world.spawn(Node::default()).id();
        crate::menu_screens::spawn_widget(&mut world.commands(), owner, frame, widget, &fdf);
        attach(&mut world.commands(), owner, frame, widget, &snapshot);
        let rect = fdf
            .rect(frame, viewport)
            .unwrap_or_else(|_| panic!("source widget rectangle refused"));
        crate::glue_menu::decorate_setting(
            &mut world.commands(),
            owner,
            &fdf,
            frame,
            Vec2::new(rect.width, rect.height),
        )
        .unwrap_or_else(|_| panic!("source widget art descriptors refused"));
        // Slider/check art has no SettingValue child in production; this extra
        // text probe exercises the actual shared caption system without a font.
        let text = world.spawn((Text::new(""), SettingValue(owner))).id();
        owners.push((*frame, widget, owner, text));
    }
    world.flush();
    world.run_system_once(super::super::labels).unwrap();
    let mut admitted = 0;
    for (frame, widget, owner, text) in owners {
        let value = expected.get(frame).and_then(Option::as_ref);
        assert!(
            snapshot.value(frame) == value,
            "source restricted-profile projection differs"
        );
        admitted += usize::from(value.is_some());
        assert!(
            world.get::<InteractionDisabled>(owner).is_some(),
            "preview enabled a control"
        );
        assert!(
            world.get::<Checkbox>(owner).is_none() && world.get::<Slider>(owner).is_none(),
            "preview installed behavior"
        );
        assert!(
            world
                .get::<SettingControl>(owner)
                .is_some_and(|c| c.name == frame && c.widget == *widget),
            "attached widget binding differs"
        );
        assert!(
            world.get::<Checked>(owner).is_some() == (value.and_then(|v| v.checkbox) == Some(true)),
            "source checkbox art state differs"
        );
        assert!(
            world.get::<SliderValue>(owner).map(|v| v.0) == value.and_then(|v| v.slider),
            "source slider art state differs"
        );
        assert!(
            world.get::<SliderRange>(owner).is_some() == value.and_then(|v| v.slider).is_some(),
            "unavailable slider retained an art range"
        );
        let caption = match widget {
            Widget::Popup { .. } => value
                .and_then(|v| v.popup.clone())
                .unwrap_or_else(|| "Unavailable".into()),
            Widget::Slider { .. } => value
                .and_then(|v| v.slider)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "Unavailable".into()),
            Widget::Checkbox => if value.and_then(|v| v.checkbox) == Some(true) {
                "×"
            } else {
                ""
            }
            .into(),
        };
        assert!(
            world
                .get::<Text>(text)
                .is_some_and(|text| text.0 == caption),
            "source caption consumer differs"
        );
        let mut labels = world.query::<(&SettingValue, &Text)>();
        let actual_labels = labels
            .iter(&world)
            .filter(|(binding, _)| binding.0 == owner);
        let mut count = 0;
        for (_, text) in actual_labels {
            assert!(text.0 == caption, "actual source popup caption differs");
            count += 1;
        }
        let source_caption = matches!(widget, Widget::Popup { .. })
            && fdf
                .string(frame, "PopupTitleFrame")
                .and_then(|title| fdf.button_text(&title))
                .is_some();
        assert!(
            count == 1 + usize::from(source_caption),
            "source caption binding absent or duplicated"
        );
        let mut thumbs = world.query::<(&ChildOf, &bevy::ui_widgets::SliderThumb)>();
        let thumb_count = thumbs
            .iter(&world)
            .filter(|(parent, _)| parent.parent() == owner)
            .count();
        let source_thumb = matches!(widget, Widget::Slider { .. })
            && fdf.string(frame, "SliderThumbButtonFrame").is_some();
        assert!(
            thumb_count == usize::from(source_thumb),
            "source thumb descriptor binding differs"
        );
        assert!(
            world
                .get::<Control>(owner)
                .is_some_and(|control| control.available == value.is_some()),
            "preview availability differs"
        );
        assert!(
            world
                .get::<AccessibilityNode>(owner)
                .is_some_and(|node| node.is_disabled()
                    && node
                        .description()
                        .is_some_and(|text| text.contains("not applied"))),
            "accessible preview qualification absent"
        );
    }
    assert!(admitted > 0, "no source profiles exercised");
    let nav = world.resource::<Navigation>();
    assert!(
        nav.draft == before && nav.applied == applied,
        "preview changed real preferences"
    );
    eprintln!(
        "source-preview image_sha256={actual_sha} origin={} localization_member={} bindings={} admitted={} refused={}",
        origin.source,
        crate::mainmenu::GLOBAL_STRINGS_FDF,
        controls.len(),
        admitted,
        controls.len() - admitted
    );
}
