//! Programmatic trigger-message font settings in the selected UI profiles.
use super::defaults::{parse_config_profile, profile_elements};

#[derive(Clone, Debug, PartialEq)]
pub struct MessageStyle {
    pub font_path: String,
    pub height: f32,
}
impl MessageStyle {
    pub fn read(misc: &[u8], skin: &[u8], max_bytes: usize) -> Result<Self, String> {
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
            "worldframeunitmessage",
            max_bytes,
        )?
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or("UI\\MiscUI.txt: invalid WorldFrameUnitMessage height")?;
        let font_path = field(
            skin,
            "UI\\war3skins.txt",
            "default",
            "messagefont",
            max_bytes,
        )?;
        if font_path.is_empty() || font_path.contains('\0') {
            return Err("UI\\war3skins.txt: invalid MessageFont".into());
        }
        Ok(Self { font_path, height })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
