//! Selected-source lookup fallbacks, not a claim about native registry behavior.
use super::{AudioSettings, Range};
use crate::{fdf::Fdf, menu_screens::Widget};
use pcx_play::{neutral_player::Sha256Digest, options_defaults::Defaults, scripts::AssetIdentity};
use std::collections::BTreeMap;

pub(crate) struct Seed {
    pub(super) audio: AudioSettings,
    pub(super) values: BTreeMap<String, f32>,
    pub(super) toggles: BTreeMap<String, bool>,
    pub(super) ranges: BTreeMap<String, Range>,
    pub(super) origin: AssetIdentity,
    pub(super) image_sha256: Sha256Digest,
}

pub(super) fn prepare(defaults: &Defaults<'_>, fdf: &Fdf) -> Result<Seed, String> {
    prepare_with(
        &defaults.binding().origin,
        defaults.binding().image_sha256,
        fdf,
        |key| defaults.get(key).map(|entry| entry.fallback),
    )
}

fn prepare_with<'a>(
    origin: &AssetIdentity,
    image_sha256: Sha256Digest,
    fdf: &Fdf,
    fallback: impl Fn(&str) -> Option<&'a [u8]>,
) -> Result<Seed, String> {
    let candidate = || {
        let scalar = |key: &str| -> Result<i32, String> {
            let bytes = fallback(key).ok_or_else(|| format!("{key}: missing source fallback"))?;
            let text = std::str::from_utf8(bytes)
                .map_err(|_| format!("{key}: source fallback is not UTF-8"))?;
            text.parse::<i32>()
                .map_err(|_| format!("{key}: source fallback is not a complete signed i32"))
        };
        let toggle = |key: &str, frame: &str| -> Result<bool, String> {
            if !matches!(
                crate::menu_screens::widget(fdf, frame).map_err(|e| format!("{key}: {e}"))?,
                Some(Widget::Checkbox)
            ) {
                return Err(format!("{key}: {frame} is not a supported checkbox"));
            }
            match scalar(key)? {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(format!(
                    "{key}: source toggle is outside the supported 0/1 profile"
                )),
            }
        };
        let slider = |key: &str, frame: &str| -> Result<(f32, Range, f32), String> {
            let Some(Widget::Slider { min, max, step, .. }) =
                crate::menu_screens::widget(fdf, frame).map_err(|e| format!("{key}: {e}"))?
            else {
                return Err(format!("{key}: {frame} is not a supported slider"));
            };
            let units = scalar(key)?;
            let value = units as f32;
            if f64::from(value) != f64::from(units) {
                return Err(format!(
                    "{key}: source units cannot be represented exactly by the slider"
                ));
            }
            let range = Range { min, max, step };
            let gain = range
                .gain(value)
                .map_err(|e| format!("{key}: {frame}: {e}"))?;
            Ok((value, range, gain))
        };

        // Recreation glue binds the four obvious audio semantics to existing controls.
        let music_enabled = toggle("music", "MusicCheckBox")?;
        let effects_enabled = toggle("sfx", "SoundCheckBox")?;
        let (music_value, music_range, music_gain) = slider("musicvolume", "MusicVolumeSlider")?;
        let (effects_value, effects_range, effects_gain) =
            slider("sfxvolume", "SoundVolumeSlider")?;
        Ok(Seed {
            audio: AudioSettings {
                music_gain,
                effects_gain,
                music_enabled,
                effects_enabled,
            },
            values: BTreeMap::from([
                ("MusicVolumeSlider".into(), music_value),
                ("SoundVolumeSlider".into(), effects_value),
            ]),
            toggles: BTreeMap::from([
                ("MusicCheckBox".into(), music_enabled),
                ("SoundCheckBox".into(), effects_enabled),
            ]),
            ranges: BTreeMap::from([
                ("MusicVolumeSlider".into(), music_range),
                ("SoundVolumeSlider".into(), effects_range),
            ]),
            origin: origin.clone(),
            image_sha256,
        })
    };
    candidate().map_err(|e: String| {
        format!(
            "Options audio fallback [{}/{}, {}]: {e}",
            origin.edition, origin.version, origin.source
        )
    })
}

#[cfg(test)]
#[path = "original_options_audio_tests.rs"]
mod tests;
