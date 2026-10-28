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

/// One packed `A,R,G,B` profile value as RGBA, for a section and key the caller names.
pub fn read_argb(
    bytes: &[u8],
    max_bytes: usize,
    section: &str,
    key: &str,
) -> Result<[u8; 4], String> {
    if bytes.len() > max_bytes {
        return Err("UI\\MiscData.txt: profile color byte budget".into());
    }
    let profile = parse_config_profile(bytes, "UI\\MiscData.txt").map_err(|e| e.to_string())?;
    if let Some(error) = profile.unresolved().first() {
        return Err(error.to_string());
    }
    let fields = profile.raw_fields(&section.to_ascii_lowercase(), &key.to_ascii_lowercase());
    let [(_, raw)] = fields.as_slice() else {
        return Err(format!("UI\\MiscData.txt: expected one {section}/{key}"));
    };
    let values = profile_elements(raw).map_err(|e| e.to_string())?;
    let [a, r, g, b] = values.as_slice() else {
        return Err(format!("UI\\MiscData.txt: ARGB {section}/{key} required"));
    };
    let mut color = [0; 4];
    for (out, value) in color.iter_mut().zip([r, g, b, a]) {
        *out = value
            .parse()
            .map_err(|_| format!("UI\\MiscData.txt: byte {section}/{key} required"))?;
    }
    Ok(color)
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

    #[test]
    fn argb_values_read_by_named_section_and_key_and_refuse_malformed() {
        let profile =
            b"[Frame]\nRimColor=9,200,31,77\nOther=1,2,3,4\n[Elsewhere]\nRimColor=1,1,1,1\n";
        assert_eq!(
            read_argb(profile, 1024, "frame", "RimColor").unwrap(),
            [200, 31, 77, 9]
        );
        assert!(read_argb(profile, 1024, "Frame", "Missing").is_err());
        assert!(read_argb(profile, 4, "Frame", "RimColor").is_err());
        for bad in ["9,200,31", "9,200,31,256", "9,200,31,x", "9,200,31,77,1"] {
            let text = String::from_utf8_lossy(profile).replace("9,200,31,77", bad);
            assert!(
                read_argb(text.as_bytes(), 1024, "Frame", "RimColor").is_err(),
                "{bad}"
            );
        }
        let twice = [profile.as_slice(), b"[Frame]\nRimColor=1,2,3,4\n"].concat();
        assert!(read_argb(&twice, 1024, "Frame", "RimColor").is_err());
    }
}
