//! Programmatic trigger-message font settings in the selected UI profiles.
use super::defaults::{parse_config_profile, profile_elements};

/// `[FontHeights]` key of the unit message frame, and the skin key of the shared font.
const UNIT_HEIGHT_KEY: &str = "WorldFrameUnitMessage";
const FONT_KEY: &str = "MessageFont";

#[derive(Clone, Debug, PartialEq)]
pub struct MessageStyle {
    pub font_path: String,
    pub height: f32,
}

/// Programmatic `WorldFrameUnitMessage` rectangle in UI frame units from the
/// bottom left of the 0.8 by 0.6 in-game UI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MessageFrame {
    pub left: f32,
    pub bottom: f32,
    pub width: f32,
    pub height: f32,
}
impl MessageFrame {
    pub const UI_WIDTH: f32 = 0.8;
    /// `WorldFrameMessage` is 0.38 by 0.05 and sits with its bottom centre
    /// 0.18 above the UI bottom, so its top is at 0.23 and its right at 0.59.
    const WORLD_BOTTOM: f32 = 0.18;
    const WORLD_WIDTH: f32 = 0.38;
    const WORLD_HEIGHT: f32 = 0.05;
    const WORLD_RIGHT: f32 = Self::UI_WIDTH * 0.5 + Self::WORLD_WIDTH * 0.5;
    /// `WorldFrameUnitMessage` is 0.5 by 0.25 and anchors its bottom centre to
    /// the top centre of `WorldFrameMessage` with the native initializer's
    /// offsets.
    const UNIT_WIDTH: f32 = 0.5;
    const UNIT_HEIGHT: f32 = 0.25;
    const UNIT_X: f32 = -0.098125;
    const UNIT_Y: f32 = 0.0005;
    /// Native vertical extent used when mapping script coordinates.
    const SCRIPT_HEIGHT: f32 = 0.452;
    /// `x` and `y` are `DisplayTimedTextToPlayer` coordinates, with `(0, 0)` at
    /// the top-left of the script-visible area.
    pub fn unit(x: f32, y: f32) -> Self {
        let centre_x = Self::UI_WIDTH * 0.5
            + Self::UNIT_X
            + x * (Self::UI_WIDTH - Self::WORLD_BOTTOM - Self::UNIT_X);
        let bottom = Self::WORLD_BOTTOM
            + Self::WORLD_HEIGHT
            + Self::UNIT_Y
            + y * (Self::SCRIPT_HEIGHT - Self::WORLD_RIGHT - Self::UNIT_Y);
        Self {
            left: centre_x - Self::UNIT_WIDTH * 0.5,
            bottom,
            width: Self::UNIT_WIDTH,
            height: Self::UNIT_HEIGHT,
        }
    }
}
impl MessageStyle {
    pub fn read(misc: &[u8], skin: &[u8], max_bytes: usize) -> Result<Self, String> {
        Self::read_keyed(misc, skin, UNIT_HEIGHT_KEY, FONT_KEY, max_bytes)
    }
    /// Reads the `[FontHeights]` entry `height_key` and the `[Default]` entry `font_key`.
    pub fn read_keyed(
        misc: &[u8],
        skin: &[u8],
        height_key: &str,
        font_key: &str,
        max_bytes: usize,
    ) -> Result<Self, String> {
        fn field(
            bytes: &[u8],
            member: &str,
            section: &str,
            key: &str,
            max: usize,
        ) -> Result<String, String> {
            if bytes.len() > max {
                return Err(format!("{member}: profile exceeds {max} bytes"));
            }
            let parsed = parse_config_profile(bytes, member).map_err(|error| error.to_string())?;
            if let Some(error) = parsed.unresolved().first() {
                return Err(error.to_string());
            }
            let fields = parsed.raw_fields(section, key);
            let [(_, raw)] = fields.as_slice() else {
                return Err(format!("{member}: expected one {section}/{key}"));
            };
            let values = profile_elements(raw).map_err(|error| format!("{member}: {error}"))?;
            let [value] = values.as_slice() else {
                return Err(format!("{member}: expected scalar {section}/{key}"));
            };
            Ok(value.to_string())
        }
        let height = field(
            misc,
            "UI\\MiscUI.txt",
            "fontheights",
            &height_key.to_ascii_lowercase(),
            max_bytes,
        )?
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or_else(|| format!("UI\\MiscUI.txt: invalid {height_key} height"))?;
        let font_path = field(
            skin,
            "UI\\war3skins.txt",
            "default",
            &font_key.to_ascii_lowercase(),
            max_bytes,
        )?;
        if font_path.is_empty() || font_path.contains('\0') {
            return Err(format!("UI\\war3skins.txt: invalid {font_key}"));
        }
        Ok(Self { font_path, height })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unit_messages_use_the_native_programmatic_frame() {
        let origin = MessageFrame::unit(0.0, 0.0);
        assert!((origin.left - 0.051875).abs() < 1e-6);
        assert!((origin.bottom - 0.2305).abs() < 1e-6);
        assert_eq!(origin.width, 0.5);
        assert_eq!(origin.height, 0.25);
        let right = MessageFrame::unit(1.0, 0.0);
        assert!((right.left - 0.77).abs() < 1e-6);
        assert_eq!(right.bottom, origin.bottom);
        let low = MessageFrame::unit(0.0, 1.0);
        assert!((low.bottom - 0.092).abs() < 1e-6);
        assert_eq!(low.left, origin.left);
    }
    #[test]
    fn the_height_and_font_keys_are_chosen_by_the_caller() {
        let misc = b"[FontHeights]\nWorldFrameUnitMessage=0.037\nWorldFrameMessage=0.021\n";
        let skin = b"[Default]\nMessageFont=Fonts\\a.ttf\nOtherFont=Fonts\\b.ttf\n";
        let style =
            MessageStyle::read_keyed(misc, skin, "WorldFrameMessage", "OtherFont", 128).unwrap();
        assert_eq!(style.height, 0.021);
        assert_eq!(style.font_path, "Fonts\\b.ttf");
        let error =
            MessageStyle::read_keyed(misc, skin, "Missing", "MessageFont", 128).unwrap_err();
        assert!(error.contains("missing"), "{error}");
    }
    #[test]
    fn font_and_height_follow_the_selected_profiles_and_refuse_ambiguity() {
        let misc = b"[FontHeights]\nWorldFrameUnitMessage=0.037 // synthetic\n";
        let skin = b"[Default]\nMessageFont=\"Fonts\\synthetic.ttf\"\n";
        assert_eq!(
            MessageStyle::read(misc, skin, 128).unwrap(),
            MessageStyle {
                font_path: "Fonts\\synthetic.ttf".into(),
                height: 0.037
            }
        );
        assert!(MessageStyle::read(misc, skin, 1).is_err());
        for misc in [
            b"[FontHeights]\nWorldFrameUnitMessage=NaN".as_slice(),
            b"[FontHeights]\nWorldFrameUnitMessage=0",
            b"[FontHeights]\nWorldFrameUnitMessage=1\nWorldFrameUnitMessage=1",
            b"[Other]\nWorldFrameUnitMessage=1",
        ] {
            assert!(MessageStyle::read(misc, skin, 128).is_err());
        }
        for skin in [
            b"[Default]\nMessageFont=".as_slice(),
            b"[Default]\nMessageFont=a,b",
            b"[Default]\nMessageFont=a\nMessageFont=b",
            b"[Other]\nMessageFont=a",
        ] {
            assert!(MessageStyle::read(misc, skin, 128).is_err());
        }
    }
}
