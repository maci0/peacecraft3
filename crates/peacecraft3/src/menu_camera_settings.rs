//! Camera preferences retain the selected client's unsigned option units.
use super::*;
use pcx_play::camera_input::ScrollOptions;

pub(crate) fn supports(name: &str) -> bool {
    matches!(
        name,
        "MouseScrollSlider" | "KeyScrollSlider" | "MouseScrollDisableCheckBox"
    )
}
pub(super) fn scalar(name: &str) -> bool {
    matches!(name, "MouseScrollSlider" | "KeyScrollSlider")
}
pub(super) fn units(value: f32) -> Option<u32> {
    (value.is_finite()
        && value >= 0.
        && value.fract() == 0.
        && f64::from(value) <= f64::from(u32::MAX))
    .then_some(value as u32)
}
pub(crate) fn effective(settings: Option<&Settings>, mut fallback: ScrollOptions) -> ScrollOptions {
    if let Some(settings) = settings {
        if let Some(value) = settings
            .values
            .get("MouseScrollSlider")
            .and_then(|v| units(*v))
        {
            fallback.mouse = value;
        }
        if let Some(value) = settings
            .values
            .get("KeyScrollSlider")
            .and_then(|v| units(*v))
        {
            fallback.key = value;
        }
        if let Some(value) = settings.toggles.get("MouseScrollDisableCheckBox") {
            fallback.mouse_disable = u32::from(*value);
        }
    }
    fallback
}
pub(super) fn defaults(
    launcher: Option<&crate::launch_gui::Launcher>,
) -> Result<ScrollOptions, String> {
    let install = launcher.and_then(crate::launch_gui::selected_install);
    crate::menu_lighting::with_client_image(install.as_deref(), |source, image| {
        let rules = pcx_play::camera_input::CameraInputRules::read(
            image,
            pcx_play::scripts::AssetIdentity {
                source: source.into(),
                edition: "selected menu client".into(),
                version: "reviewed Legacy 1.29.2.9231 camera input profile".into(),
            },
        )?;
        Ok(rules.fallback)
    })
}
pub(super) fn attach(
    commands: &mut Commands,
    entity: Entity,
    control: &SettingControl,
    nav: &mut Navigation,
    backend: &mut Backend,
    fallback: &ScrollOptions,
) -> Result<(), String> {
    match &control.widget {
        Widget::Slider { min, max, step, .. } if scalar(&control.name) => {
            let range = Range {
                min: *min,
                max: *max,
                step: *step,
            };
            let raw = if control.name == "KeyScrollSlider" {
                fallback.key
            } else {
                fallback.mouse
            };
            let source = raw as f32;
            let applied = nav
                .applied
                .values
                .get(&control.name)
                .copied()
                .unwrap_or(source);
            let draft = nav
                .draft
                .values
                .get(&control.name)
                .copied()
                .unwrap_or(applied);
            if f64::from(source) != f64::from(raw)
                || !range.valid(applied)
                || !range.valid(draft)
                || units(applied).is_none()
                || units(draft).is_none()
            {
                return Err(format!(
                    "{}: camera preference is outside the authored slider domain",
                    control.name
                ));
            }
            backend.ranges.insert(control.name.clone(), range);
            nav.applied
                .values
                .entry(control.name.clone())
                .or_insert(applied);
            nav.draft
                .values
                .entry(control.name.clone())
                .or_insert(draft);
            commands.entity(entity).insert((
                Slider {
                    track_click: TrackClick::Snap,
                    orientation: SliderOrientation::Horizontal,
                },
                SliderRange::new(*min, *max),
                SliderStep(*step),
                SliderValue(draft),
            ));
            super::slider::attach(commands, entity);
        }
        Widget::Checkbox if control.name == "MouseScrollDisableCheckBox" => {
            let applied = *nav
                .applied
                .toggles
                .entry(control.name.clone())
                .or_insert(fallback.mouse_disable != 0);
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
        }
        _ => return Err(format!("{}: incompatible camera control", control.name)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui_widgets::ValueChange;

    #[test]
    fn unsigned_option_units_reject_fractional_negative_and_overflowing_preferences() {
        assert_eq!(units(19.), Some(19));
        for value in [-1., 1.5, f32::NAN, f32::INFINITY, u32::MAX as f32] {
            assert_eq!(units(value), None);
        }
        let mut settings = Settings::default();
        settings.values.insert("KeyScrollSlider".into(), 19.5);
        assert!(super::super::validate_settings(&settings).is_err());
    }

    #[test]
    fn source_units_attach_without_using_widget_initial_and_apply_cancel_persist() {
        let mut world = World::new();
        let control = world
            .spawn(SettingControl {
                name: "KeyScrollSlider".into(),
                widget: Widget::Slider {
                    min: 7.,
                    max: 31.,
                    initial: 7.,
                    step: 2.,
                },
            })
            .id();
        let toggle = world
            .spawn(SettingControl {
                name: "MouseScrollDisableCheckBox".into(),
                widget: Widget::Checkbox,
            })
            .id();
        let mut nav = Navigation {
            screen: crate::menu_screens::Screen::Options(crate::menu_screens::Page::Gameplay),
            ..default()
        };
        let mut backend = Backend::default();
        let fallback = ScrollOptions {
            mouse: 13,
            key: 19,
            mouse_disable: 0,
        };
        for entity in [control, toggle] {
            let definition = world.get::<SettingControl>(entity).unwrap();
            let owned = SettingControl {
                name: definition.name.clone(),
                widget: definition.widget.clone(),
            };
            attach(
                &mut world.commands(),
                entity,
                &owned,
                &mut nav,
                &mut backend,
                &fallback,
            )
            .unwrap();
        }
        world.flush();
        assert_eq!(world.get::<SliderValue>(control).unwrap().0, 19.);
        assert_eq!(nav.applied.values["KeyScrollSlider"], 19.);
        world.insert_resource(nav);
        world.insert_resource(backend);
        world.add_observer(super::super::slider_changed);
        world.add_observer(super::super::checkbox_changed);
        world.trigger(ValueChange {
            source: control,
            value: 25.,
            is_final: true,
        });
        world.trigger(ValueChange {
            source: toggle,
            value: true,
            is_final: true,
        });
        world.flush();
        let mut nav = world.remove_resource::<Navigation>().unwrap();
        let mut backend = world.remove_resource::<Backend>().unwrap();
        assert_eq!(
            effective(Some(&nav.applied), fallback),
            fallback,
            "draft must not change the live camera"
        );
        nav.action("CancelButton");
        assert_eq!(nav.draft, nav.applied);
        nav.draft.values.insert("KeyScrollSlider".into(), 25.);
        nav.draft
            .toggles
            .insert("MouseScrollDisableCheckBox".into(), true);
        let mut saved = String::new();
        super::super::commit(&mut nav, &mut Window::default(), &mut backend, |text| {
            saved = text.into();
            Ok(())
        })
        .unwrap();
        let reloaded = super::super::decode_preferences(&saved).unwrap();
        let options = effective(Some(&reloaded.settings), fallback);
        assert_eq!(options.key, 25);
        assert_eq!(options.mouse_disable, 1);
        nav.draft.values.insert("KeyScrollSlider".into(), 33.);
        assert!(super::super::prepare(&nav, &backend).is_err());
    }

    #[test]
    #[ignore = "requires PEACECRAFT3_INSTALL with the selected original executable and Options FDF"]
    fn original_gameplay_controls_admit_the_selected_camera_fallbacks() {
        let install = std::path::PathBuf::from(
            std::env::var_os("PEACECRAFT3_INSTALL").expect("explicit original install required"),
        );
        let image = std::fs::read(install.join("Warcraft III.exe")).unwrap();
        let fallback = pcx_play::camera_input::CameraInputRules::read(
            &image,
            pcx_play::campaign_inputs::client_identity(&image, "selected local Options client"),
        )
        .unwrap()
        .fallback;
        let (fdf, _) = crate::launch_gui::read_screen_fdf(
            Some(&install),
            crate::menu_screens::Screen::Options(crate::menu_screens::Page::Gameplay),
        )
        .unwrap();
        let mut world = World::new();
        let mut nav = Navigation::default();
        let mut backend = Backend::default();
        for name in [
            "KeyScrollSlider",
            "MouseScrollSlider",
            "MouseScrollDisableCheckBox",
        ] {
            let widget = crate::menu_screens::widget(&fdf, name).unwrap().unwrap();
            assert!(crate::menu_screens::supported(name));
            let entity = world.spawn_empty().id();
            attach(
                &mut world.commands(),
                entity,
                &SettingControl {
                    name: name.into(),
                    widget,
                },
                &mut nav,
                &mut backend,
                &fallback,
            )
            .unwrap();
        }
        world.flush();
        assert_eq!(effective(Some(&nav.applied), fallback), fallback);
    }
}
