//! Minimap opacity fields from the selected UI profile, not fitted colors.
use super::defaults::{parse_config_profile, profile_elements};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinimapFogAlpha {
    pub masked: u8,
    pub fogged: u8,
    pub visible: u8,
}

impl MinimapFogAlpha {
    pub fn read(bytes: &[u8], max_bytes: usize) -> Result<Self, String> {
        if bytes.len() > max_bytes {
            return Err("UI\\MiscData.txt: minimap fog profile byte budget".into());
        }
        let profile = parse_config_profile(bytes, "UI\\MiscData.txt").map_err(|e| e.to_string())?;
        if let Some(error) = profile.unresolved().first() {
            return Err(error.to_string());
        }
        let alpha = |key: &str| -> Result<u8, String> {
            let fields = profile.raw_fields("fogofwar", &key.to_ascii_lowercase());
            let [(_, raw)] = fields.as_slice() else {
                return Err(format!("UI\\MiscData.txt: expected one FogOfWar/{key}"));
            };
            let values = profile_elements(raw).map_err(|e| e.to_string())?;
            let [value] = values.as_slice() else {
                return Err(format!("UI\\MiscData.txt: scalar {key} required"));
            };
            value
                .parse()
                .map_err(|_| format!("UI\\MiscData.txt: byte {key} required"))
        };
        Ok(Self {
            masked: alpha("FogColorBlackMaskAlpha")?,
            fogged: alpha("FogColorFogMaskAlpha")?,
            visible: alpha("FogColorVisibleAlpha")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opacity_is_authored_and_bad_profiles_refuse() {
        let profile = b"[FogOfWar]\nFogColorBlackMaskAlpha=7\nFogColorFogMaskAlpha=91\nFogColorVisibleAlpha=223\n";
        assert_eq!(
            MinimapFogAlpha::read(profile, 1024).unwrap(),
            MinimapFogAlpha {
                masked: 7,
                fogged: 91,
                visible: 223
            }
        );
        assert!(MinimapFogAlpha::read(profile, 1).is_err());
        for bad in ["256", "-1", "1,2", "bad"] {
            let profile = String::from_utf8_lossy(profile).replace("=91", &format!("={bad}"));
            assert!(MinimapFogAlpha::read(profile.as_bytes(), 1024).is_err());
        }
        let duplicate = [profile.as_slice(), b"FogColorVisibleAlpha=19\n"].concat();
        assert!(MinimapFogAlpha::read(&duplicate, 1024).is_err());
    }
}
