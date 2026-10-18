//! Read-only source fallback presentation for settings without runtime owners.
//! This never admits an applied/draft preference or installs widget behavior.
use crate::menu_screens::{Widget, supported};
use bevy::{
    a11y::AccessibilityNode,
    prelude::*,
    ui::{Checked, InteractionDisabled},
    ui_widgets::{SliderRange, SliderValue},
};
use pcx_play::{neutral_player::Sha256Digest, scripts::AssetIdentity};
use std::collections::BTreeMap;

pub(crate) const IDLE_NOTICE: &str = "Preview; not applied.";
const VALID_DESCRIPTION: &str =
    "Source fallback preview only. This setting has no runtime implementation and is not applied.";
// Project-owned resource policies, not original setting values.
#[cfg(any(test, not(target_arch = "wasm32")))]
const LABEL_BYTES: usize = 4096;
#[cfg(any(test, not(target_arch = "wasm32")))]
const TOTAL_LABEL_BYTES: usize = 64 * 1024;
/// Project policy; independent of the number of authored bindings.
pub(crate) const CONTROL_LIMIT: usize = 64;

#[derive(Debug, Default, Clone, PartialEq)]
struct Value {
    popup: Option<String>,
    slider: Option<f32>,
    checkbox: Option<bool>,
}
#[derive(Default)]
pub(crate) struct Snapshot {
    key: String,
    // One independently qualified client identity for the snapshot, not per control.
    identity: Option<(AssetIdentity, Sha256Digest)>,
    entries: BTreeMap<String, Result<Value, &'static str>>,
    unavailable: String,
}
impl Snapshot {
    pub(crate) fn unavailable(key: String, reason: &str) -> Self {
        Self {
            key,
            unavailable: reason.chars().take(240).collect(),
            ..default()
        }
    }
    pub(crate) fn matches(&self, key: &str) -> bool {
        self.key == key
    }
    pub(crate) fn notice(&self) -> String {
        if self.identity.is_some() {
            IDLE_NOTICE.into()
        } else {
            "Defaults unavailable.".into()
        }
    }
    fn value(&self, frame: &str) -> Option<&Value> {
        self.entries.get(frame).and_then(|v| v.as_ref().ok())
    }
    fn description(&self, frame: &str) -> String {
        if self.value(frame).is_some() {
            return VALID_DESCRIPTION.into();
        }
        let reason = self
            .entries
            .get(frame)
            .and_then(|v| v.as_ref().err())
            .copied()
            .unwrap_or(if self.unavailable.is_empty() {
                "No admitted source fallback preview for this control."
            } else {
                &self.unavailable
            });
        format!("This setting is unavailable and is not applied. {reason}")
    }
    #[cfg(any(test, not(target_arch = "wasm32")))]
    pub(crate) fn checked<'a>(
        defaults: &pcx_play::options_defaults::Defaults<'_>,
        controls: impl IntoIterator<Item = (&'a str, &'a Widget)>,
        strings: &str,
        key: String,
    ) -> Self {
        let mut snapshot = Self {
            key,
            identity: Some((
                defaults.binding().origin.clone(),
                defaults.binding().image_sha256,
            )),
            ..default()
        };
        if strings.len() > 8 * 1024 * 1024 {
            return Self::unavailable(
                snapshot.key,
                "The localization input exceeds the preview policy.",
            );
        }
        let mut remaining = TOTAL_LABEL_BYTES;
        for (frame, widget) in controls {
            if supported(frame) || !eligible(frame) {
                continue;
            }
            if snapshot.entries.len() >= CONTROL_LIMIT {
                return Self::unavailable(
                    snapshot.key,
                    "The setting preview count exceeds its policy.",
                );
            }
            let value = selected(defaults, widget, frame, strings, &mut remaining);
            if snapshot.entries.insert(frame.into(), value).is_some() {
                return Self::unavailable(
                    snapshot.key,
                    "The candidate contains duplicate setting controls.",
                );
            }
        }
        snapshot
    }
}

/// Respect Fdf's indexed overrides and bound temporary control metadata. Existing
/// checked widget parsing owns malformed records and authored order.
pub(crate) fn widgets(fdf: &crate::fdf::Fdf) -> Result<Vec<(&str, Widget)>, String> {
    let controls = fdf
        .frames()
        .iter()
        .filter(|frame| {
            fdf.frame(&frame.name)
                .is_some_and(|current| std::ptr::eq(current, *frame))
        })
        .filter(|frame| eligible(&frame.name))
        .take(CONTROL_LIMIT + 1)
        .map(|frame| {
            crate::menu_screens::widget(fdf, &frame.name)
                .and_then(|widget| {
                    widget.ok_or_else(|| crate::fdf::FdfError::UnknownFrame(frame.name.clone()))
                })
                .map(|widget| (frame.name.as_str(), widget))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if controls.len() > CONTROL_LIMIT {
        return Err("The setting preview count exceeds its policy.".into());
    }
    Ok(controls)
}

/// Existing failure/pending notices remain authoritative over transient help.
pub(crate) fn notice(
    status: &str,
    cog: bool,
    options: bool,
    hovered: Option<&Control>,
    snapshot: &Snapshot,
) -> String {
    if !status.is_empty() {
        status.into()
    } else if cog {
        "PeaceCraft settings".into()
    } else if options {
        hovered.map_or_else(String::new, |control| {
            if control.available {
                snapshot.notice()
            } else {
                "Setting unavailable.".into()
            }
        })
    } else {
        String::new()
    }
}

/// Presentation-only metadata; no Checkbox/Slider behavior component is added.
#[derive(Component)]
pub(crate) struct Control {
    pub(crate) popup: Option<String>,
    available: bool,
}
/// Called only while building the same admitted candidate's control tree.
pub(crate) fn attach(
    commands: &mut Commands,
    owner: Entity,
    frame: &str,
    widget: &Widget,
    snapshot: &Snapshot,
) {
    if supported(frame) {
        return;
    }
    // Reset before projecting a changed source. Unavailable is not unchecked/zero.
    commands
        .entity(owner)
        .remove::<(Checked, SliderValue, SliderRange)>();
    let value = snapshot.value(frame);
    if matches!(widget, Widget::Checkbox) && value.and_then(|v| v.checkbox) == Some(true) {
        commands.entity(owner).insert(Checked);
    }
    if let Widget::Slider { min, max, .. } = widget
        && let Some(value) = value.and_then(|v| v.slider)
    {
        commands
            .entity(owner)
            .insert((SliderValue(value), SliderRange::new(*min, *max)));
    }
    let popup = value.and_then(|v| v.popup.clone());
    let description = snapshot.description(frame);
    let accessible_value = if let Some(label) = value.and_then(|v| v.popup.as_deref()) {
        format!("Source fallback preview: {label}")
    } else if let Some(value) = value.and_then(|v| v.slider) {
        format!("Source fallback preview in authored units: {value}")
    } else if let Some(value) = value.and_then(|v| v.checkbox) {
        format!("Source fallback preview: {value}")
    } else {
        "Source fallback unavailable".into()
    };
    commands.entity(owner).insert((
        InteractionDisabled,
        Control {
            popup,
            available: value.is_some(),
        },
    ));
    // Button's required node has been inserted before this queued presentation update.
    // Do not expose Checked as an effective checkbox state in the accessibility tree.
    commands.queue(move |world: &mut World| {
        if let Some(mut node) = world.get_mut::<AccessibilityNode>(owner) {
            node.set_disabled();
            node.set_description(description);
            node.set_value(accessible_value);
            node.clear_toggled();
        }
    });
}

#[derive(Clone, Copy)]
enum Kind {
    Popup,
    Slider,
    Checkbox,
}
pub(crate) fn eligible(frame: &str) -> bool {
    join(frame).is_some()
}
fn join(frame: &str) -> Option<(&'static str, Kind)> {
    use Kind::{Checkbox as C, Popup as P, Slider as S};
    Some(match frame {
        "ModelDetailMenu" => ("modeldetail", P),
        "AnimQualityMenu" => ("animquality", P),
        "TextureQualityMenu" => ("texquality", P),
        "ParticlesMenu" => ("particles", P),
        "LightsMenu" => ("lights", P),
        "ShadowsMenu" => ("unitshadows", P),
        "OcclusionMenu" => ("occlusion", P),
        "ChatSupportMenu" => ("chatsupport", P),
        "ProviderMenu" => ("provider", P),
        "MouseScrollSlider" => ("mousescroll", S),
        "KeyScrollSlider" => ("keyscroll", S),
        "GammaSlider" => ("gamma", S),
        "MouseScrollDisableCheckBox" => ("mousescrolldisable", C),
        "TooltipsCheckBox" => ("tooltips", C),
        "HealthBarsCheckBox" => ("healthbars", C),
        "SubgroupCheckBox" => ("subgrouporder", C),
        "FormationToggleCheckBox" => ("formationtoggle", C),
        "CustomKeysCheckBox" => ("customkeys", C),
        "AutosaveReplayCheckBox" => ("autosaveReplay", C),
        "AmbientCheckBox" => ("ambient", C),
        "MovementCheckBox" => ("movement", C),
        "UnitCheckBox" => ("unit", C),
        "SubtitlesCheckBox" => ("subtitles", C),
        "EnviroCheckBox" => ("environmental", C),
        "PositionalCheckBox" => ("positional", C),
        _ => return None,
    })
}
#[cfg(any(test, not(target_arch = "wasm32")))]
fn selected(
    defaults: &pcx_play::options_defaults::Defaults<'_>,
    widget: &Widget,
    frame: &str,
    strings: &str,
    remaining: &mut usize,
) -> Result<Value, &'static str> {
    let (key, kind) = join(frame).ok_or("This control has no supported preview binding.")?;
    let raw = defaults
        .get(key)
        .ok_or("The source preference key is absent.")?
        .fallback;
    match (kind, widget) {
        (Kind::Checkbox, Widget::Checkbox) => match raw {
            b"0" => Ok(Value {
                checkbox: Some(false),
                ..default()
            }),
            b"1" => Ok(Value {
                checkbox: Some(true),
                ..default()
            }),
            _ => Err("The source value is outside the strict zero/one preview profile."),
        },
        (
            Kind::Slider,
            Widget::Slider {
                min,
                max,
                initial,
                step,
            },
        ) => {
            let integer = decimal(raw)
                .and_then(|v| i32::try_from(v).ok())
                .ok_or("The source value is outside the nonnegative integer preview profile.")?;
            let value = integer as f32;
            if !min.is_finite()
                || !max.is_finite()
                || !(max - min).is_finite()
                || min >= max
                || !step.is_finite()
                || *step <= 0.0
                || !initial.is_finite()
                || !(*min..=*max).contains(initial)
                || f64::from(value) != f64::from(integer)
                || !(*min..=*max).contains(&value)
            {
                return Err(
                    "The source value cannot be represented exactly in the authored slider range.",
                );
            }
            Ok(Value {
                slider: Some(value),
                ..default()
            })
        }
        (Kind::Popup, Widget::Popup { choices }) => {
            let index = decimal(raw)
                .ok_or("The source value is outside the popup index preview profile.")?;
            let key = choices
                .get(index)
                .ok_or("The source popup has no authored choice at that index.")?;
            let text = localized(strings, key)?;
            if text.len() > LABEL_BYTES || text.len() > *remaining {
                return Err("The preview label exceeds its text policy.");
            }
            // Check the borrowed raw text before Label allocates its authored runs.
            let label = crate::menu_text::Label::parse(text)
                .map_err(|_| "The source popup label markup is malformed.")?;
            let visible = label.visible();
            if visible.trim().is_empty() {
                return Err("The source popup label is empty.");
            }
            *remaining -= text.len();
            Ok(Value {
                popup: Some(visible),
                ..default()
            })
        }
        _ => Err("The control kind does not match the source preview binding."),
    }
}
#[cfg(any(test, not(target_arch = "wasm32")))]
fn decimal(raw: &[u8]) -> Option<usize> {
    if raw.is_empty() || !raw.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(raw).ok()?.parse().ok()
}
/// Borrowed, unique complete record lookup; no key-as-label fallback or clone before bounds.
#[cfg(any(test, not(target_arch = "wasm32")))]
fn localized<'a>(strings: &'a str, key: &str) -> Result<&'a str, &'static str> {
    let mut value = None;
    for line in strings.lines() {
        let Some((name, rest)) = line.trim().split_once(char::is_whitespace) else {
            continue;
        };
        if name != key {
            continue;
        }
        if value.is_some() {
            return Err("The source popup label is ambiguous.");
        }
        let (text, tail) = rest
            .trim()
            .strip_prefix('"')
            .and_then(|s| s.split_once('"'))
            .ok_or("The source popup label is malformed.")?;
        if !matches!(tail.trim(), "" | ",") {
            return Err("The source popup label has unsupported trailing content.");
        }
        value = Some(text);
    }
    value.ok_or("The source popup label is missing.")
}

#[cfg(test)]
#[path = "original_options_preview_tests.rs"]
mod tests;
