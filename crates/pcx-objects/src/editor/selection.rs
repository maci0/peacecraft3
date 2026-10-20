//! Selected `UI\MiscData.txt` `[SelectionCircle]` settings.
use super::defaults::{parse_config_profile, profile_elements};

/// One size band: circles up to `max_size` (the last band has none) use these textures.
#[derive(Clone, Debug, PartialEq)]
pub struct CircleBand {
    pub max_size: Option<f32>,
    pub texture: String,
    pub dotted: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectionCircles {
    /// Multiplies a unit's UnitUI `scale` to give its circle size.
    pub scale_factor: f32,
    pub bands: Vec<CircleBand>,
    /// ARGB as authored.
    pub friend: [u8; 4],
    pub neutral: [u8; 4],
    pub enemy: [u8; 4],
}

impl SelectionCircles {
    pub fn read(bytes: &[u8], member: &str, max_bands: usize) -> Result<Self, String> {
        let parsed = parse_config_profile(bytes, member).map_err(|e| e.to_string())?;
        if let Some(error) = parsed.unresolved().first() {
            return Err(error.to_string());
        }
        let raw = |key: &str| -> Result<Option<Vec<String>>, String> {
            let fields = parsed.raw_fields("selectioncircle", &key.to_ascii_lowercase());
            match fields.as_slice() {
                [] => Ok(None),
                [(_, raw)] => profile_elements(raw)
                    .map(|v| Some(v.into_iter().map(str::to_owned).collect()))
                    .map_err(|e| format!("{member}: {e}")),
                _ => Err(format!("{member}: duplicate SelectionCircle/{key}")),
            }
        };
        let scalar = |key: &str| -> Result<f32, String> {
            raw(key)?
                .and_then(|v| v.first().and_then(|s| s.parse::<f32>().ok()))
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("{member}: missing or invalid SelectionCircle/{key}"))
        };
        let text = |key: &str| -> Result<String, String> {
            raw(key)?
                .and_then(|v| v.into_iter().next())
                .ok_or_else(|| format!("{member}: missing SelectionCircle/{key}"))
        };
        let color = |key: &str| -> Result<[u8; 4], String> {
            let values = raw(key)?.ok_or_else(|| format!("{member}: missing {key}"))?;
            let parsed: Option<Vec<u8>> = values.iter().map(|v| v.parse().ok()).collect();
            parsed
                .and_then(|v| <[u8; 4]>::try_from(v).ok())
                .ok_or_else(|| format!("{member}: SelectionCircle/{key} needs four bytes"))
        };
        let count = scalar("NumSizes")?;
        if count < 1.0 || count.fract() != 0.0 || count as usize > max_bands {
            return Err(format!(
                "{member}: SelectionCircle/NumSizes is out of range"
            ));
        }
        let count = count as usize;
        let mut bands = Vec::with_capacity(count);
        for index in 0..count {
            let max_size = if index + 1 < count {
                Some(scalar(&format!("Size{index:02}"))?)
            } else {
                None
            };
            bands.push(CircleBand {
                max_size,
                texture: text(&format!("Texture{index:02}"))?,
                dotted: text(&format!("TextureDotted{index:02}"))?,
            });
        }
        Ok(Self {
            scale_factor: scalar("ScaleFactor")?,
            bands,
            friend: color("ColorFriend")?,
            neutral: color("ColorNeutral")?,
            enemy: color("ColorEnemy")?,
        })
    }

    /// The band for a circle of `size`.
    pub fn band(&self, size: f32) -> &CircleBand {
        self.bands
            .iter()
            .find(|b| b.max_size.is_none_or(|max| size <= max))
            .unwrap_or_else(|| self.bands.last().expect("at least one band"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_scale_and_colors_come_from_the_selected_section() {
        let source = b"[SelectionCircle]\nNumSizes=2\nSize00=50\nTexture00=Small\nTextureDotted00=SmallDot\nTexture01=Large\nTextureDotted01=LargeDot\nScaleFactor=9\nColorFriend=1,2,3,4\nColorNeutral=5,6,7,8\nColorEnemy=9,10,11,12\n";
        let circles = SelectionCircles::read(source, "fixture", 8).unwrap();
        assert_eq!(circles.scale_factor, 9.0);
        assert_eq!(circles.band(50.0).texture, "Small");
        assert_eq!(circles.band(51.0).dotted, "LargeDot");
        assert_eq!(circles.enemy, [9, 10, 11, 12]);
        assert!(SelectionCircles::read(source, "fixture", 1).is_err());
        let broken =
            String::from_utf8_lossy(source).replace("ColorEnemy=9,10,11,12", "ColorEnemy=9,10");
        assert!(SelectionCircles::read(broken.as_bytes(), "fixture", 8).is_err());
    }
}
