use crate::menu_skin::SkinTable;
use bevy::{
    pbr::{DistanceFog, FogFalloff},
    prelude::Color,
};
use std::collections::BTreeMap;

/// Source values selected by the same explicit edition ordinal as the glue skin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MenuFog {
    pub color_argb: [u8; 4],
    pub density: f32,
    pub start: f32,
    pub end: f32,
}

impl MenuFog {
    pub fn checked(table: &SkinTable, edition: usize) -> Result<Self, String> {
        let mut sections = table
            .sections()
            .iter()
            .filter(|section| section.name == "MenuZFog");
        let section = sections.next().ok_or("missing [MenuZFog]")?;
        if sections.next().is_some() {
            return Err("duplicate [MenuZFog]".into());
        }
        let mut fields = BTreeMap::new();
        for (key, value) in &section.entries {
            if !["Style", "Color", "Density", "Start", "End"].contains(&key.as_str()) {
                continue;
            }
            let numbers = value
                .split(',')
                .map(|value| {
                    value
                        .trim()
                        .parse::<f32>()
                        .map_err(|_| format!("[MenuZFog] {key}: invalid number"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if numbers.iter().any(|number| !number.is_finite()) {
                return Err(format!("[MenuZFog] {key}: non-finite value"));
            }
            if fields.insert(key.as_str(), numbers).is_some() {
                return Err(format!("[MenuZFog] duplicate {key}"));
            }
        }
        let row = |key| {
            fields
                .get(key)
                .ok_or_else(|| format!("[MenuZFog] missing {key}"))
        };
        let styles = row("Style")?;
        let columns = styles.len();
        if edition >= columns {
            return Err(format!("[MenuZFog] no edition column {edition}"));
        }
        for key in ["Density", "Start", "End"] {
            if row(key)?.len() != columns {
                return Err(format!("[MenuZFog] {key}: edition column count differs"));
            }
        }
        let colors = row("Color")?;
        if colors.len()
            != columns
                .checked_mul(4)
                .ok_or("[MenuZFog] Color count overflow")?
        {
            return Err("[MenuZFog] Color: expected one four-byte group per edition".into());
        }
        if styles[edition] != 0.0 {
            return Err(format!("[MenuZFog] unsupported style {}", styles[edition]));
        }
        let mut color_argb = [0; 4];
        for (channel, value) in color_argb
            .iter_mut()
            .zip(&colors[edition * 4..edition * 4 + 4])
        {
            if !(0.0..=f32::from(u8::MAX)).contains(value) || value.fract() != 0.0 {
                return Err("[MenuZFog] Color: expected integral byte channels".into());
            }
            *channel = *value as u8;
        }
        let fog = Self {
            color_argb,
            density: row("Density")?[edition],
            start: row("Start")?[edition],
            end: row("End")?[edition],
        };
        if fog.density < 0.0 || fog.start < 0.0 || fog.end <= fog.start {
            return Err("[MenuZFog] invalid density or distance interval".into());
        }
        Ok(fog)
    }

    pub fn distance_fog(self) -> DistanceFog {
        let [alpha, red, green, blue] = self.color_argb;
        // Recreation convention: grouped ARGB and style zero linear; native/render acceptance pending.
        DistanceFog {
            color: Color::srgba_u8(red, green, blue, alpha),
            falloff: FogFalloff::Linear {
                start: self.start,
                end: self.end,
            },
            directional_light_color: Color::NONE,
            directional_light_exponent: 0.0,
        }
    }
}

/// Unfogged and additive surfaces must not introduce the fog color into the scene.
pub fn material_fog_enabled(shading_flags: u32, filter_mode: u32) -> bool {
    // Reconstruction convention corroborated by the retained additive draw's fog-off state.
    shading_flags & 0x20 == 0 && filter_mode != 3
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table(source: &str) -> SkinTable {
        SkinTable::parse(source).unwrap()
    }
    fn fixture() -> String {
        "[MenuZFog]\nStyle=0,0\nDensity=.17,.29\nStart=13,23\nEnd=73,97\nColor=101,37,59,83,109,43,71,89\n".into()
    }
    #[test]
    fn explicit_edition_selects_all_fields_and_argb_channels() {
        let source = table(&fixture());
        let roc = MenuFog::checked(&source, 0).unwrap();
        let tft = MenuFog::checked(&source, 1).unwrap();
        assert_eq!((roc.start, roc.end, roc.density), (13.0, 73.0, 0.17));
        assert_eq!((tft.start, tft.end, tft.density), (23.0, 97.0, 0.29));
        assert_eq!(tft.color_argb, [109, 43, 71, 89]);
        let fog = tft.distance_fog();
        assert_eq!(fog.color, Color::srgba_u8(43, 71, 89, 109));
        assert!(matches!(
            fog.falloff,
            FogFalloff::Linear {
                start: 23.0,
                end: 97.0
            }
        ));
        assert_eq!(fog.directional_light_color, Color::NONE);
        assert!(MenuFog::checked(&source, 2).is_err());
    }
    #[test]
    fn missing_duplicate_malformed_and_unsupported_inputs_refuse() {
        assert!(MenuFog::checked(&table("[Other]\nStyle=0"), 0).is_err());
        for (old, new) in [
            ("Style=0,0", "Style=0,1"),
            ("Density=.17,.29", "Density=.17"),
            ("Start=13,23", "Start=13,NaN"),
            ("End=73,97", "End=73,23"),
            (
                "Color=101,37,59,83,109,43,71,89",
                "Color=101,37,59,83,109,43,71",
            ),
            (
                "Color=101,37,59,83,109,43,71,89",
                "Color=101,37,59,83,109,43,71,256",
            ),
            ("Density=.17,.29", "Density=.17,-.29"),
            ("End=73,97", "End=73,97\nEnd=73,97"),
        ] {
            assert!(
                MenuFog::checked(&table(&fixture().replace(old, new)), 1).is_err(),
                "{new}"
            );
        }
        assert!(MenuFog::checked(&table(&(fixture() + "[MenuZFog]\nStyle=0")), 1).is_err());
        assert!(MenuFog::checked(&table(&fixture().replace("Density=.17,.29\n", "")), 1).is_err());
    }
    #[test]
    fn unshaded_does_not_suppress_fog_and_additive_never_adds_fog_color() {
        for filter_mode in [0, 1, 2, 4, 5, 6] {
            assert!(material_fog_enabled(0, filter_mode));
            assert!(material_fog_enabled(0x01, filter_mode));
            assert!(material_fog_enabled(0x10, filter_mode));
            assert!(!material_fog_enabled(0x20, filter_mode));
            assert!(!material_fog_enabled(0x21, filter_mode));
            assert!(!material_fog_enabled(0xa1, filter_mode));
        }
        for flags in [0, 1, 0x10, 0x20, 0x21, 0xa1] {
            assert!(!material_fog_enabled(flags, 3));
        }
    }
}
