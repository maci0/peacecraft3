//! Display palette from ordered UI/map profiles.
use super::defaults::{parse_config_profile, profile_elements};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Original,
    Player,
    Ally,
    Creep,
    Enemy,
}
impl Role {
    fn index(self) -> Option<usize> {
        match self {
            Self::Original => None,
            Self::Player => Some(0),
            Self::Ally => Some(1),
            Self::Creep => Some(2),
            Self::Enemy => Some(3),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Palette {
    pub sources: Vec<String>,
    model: [i32; 4],
    minimap: [[u8; 4]; 4],
}
impl Palette {
    pub fn read(profiles: &[(&str, &[u8])], budget: usize) -> Result<Self, String> {
        let mut model = [None; 4];
        let mut minimap = [None; 4];
        let mut sources = Vec::new();
        let mut remaining = budget;
        for &(source, bytes) in profiles {
            remaining = remaining
                .checked_sub(bytes.len())
                .ok_or("ally-color profile budget")?;
            if source.trim().is_empty() {
                return Err("ally-color profile source is empty".into());
            }
            let profile = parse_config_profile(bytes, source).map_err(|e| e.to_string())?;
            if let Some(error) = profile.unresolved().first() {
                return Err(error.to_string());
            }
            for (index, role) in ["Player", "Ally", "Creep", "Enemy"].iter().enumerate() {
                let key = format!("ColorIndex{role}");
                let values = profile.raw_fields("teamcolorfilter", &key.to_ascii_lowercase());
                match values.as_slice() {
                    [] => {}
                    [(_, text)] => {
                        let fields = profile_elements(text).map_err(|e| e.to_string())?;
                        let [value] = fields.as_slice() else {
                            return Err(format!("{source}: expected scalar {key}"));
                        };
                        let value = value
                            .parse::<i32>()
                            .ok()
                            .filter(|v| *v >= 0)
                            .ok_or_else(|| format!("{source}: invalid {key}"))?;
                        model[index] = Some(value);
                    }
                    _ => return Err(format!("{source}: duplicate {key}")),
                }
                let key = format!(
                    "FogColor{}",
                    if *role == "Creep" {
                        "CreepNormal"
                    } else {
                        role
                    }
                );
                let values = profile.raw_fields("fogofwar", &key.to_ascii_lowercase());
                match values.as_slice() {
                    [] => {}
                    [(_, text)] => {
                        let fields = profile_elements(text).map_err(|e| e.to_string())?;
                        let [a, r, g, b] = fields.as_slice() else {
                            return Err(format!("{source}: expected ARGB {key}"));
                        };
                        let mut color = [0; 4];
                        for (out, value) in color.iter_mut().zip([r, g, b, a]) {
                            *out = value
                                .parse()
                                .map_err(|_| format!("{source}: invalid {key}"))?;
                        }
                        minimap[index] = Some(color);
                    }
                    _ => return Err(format!("{source}: duplicate {key}")),
                }
            }
            sources.push(source.into());
        }
        Ok(Self {
            sources,
            model: model
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or("incomplete TeamColorFilter profile")?
                .try_into()
                .expect("four roles"),
            minimap: minimap
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or("incomplete FogOfWar colors")?
                .try_into()
                .expect("four roles"),
        })
    }
    pub fn model(&self, role: Role) -> Option<i32> {
        role.index().map(|i| self.model[i])
    }
    pub fn minimap(&self, role: Role) -> Option<[u8; 4]> {
        role.index().map(|i| self.minimap[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE:&[u8]=b"[TeamColorFilter]\nColorIndexPlayer=3\nColorIndexAlly=8\nColorIndexCreep=9\nColorIndexEnemy=10\n[FogOfWar]\nFogColorPlayer=17,7,11,13\nFogColorAlly=19,23,29,31\nFogColorCreepNormal=37,41,43,47\nFogColorEnemy=53,59,61,67\n";
    #[test]
    fn nonstock_profiles_keep_separate_model_and_minimap_values_and_map_overrides() {
        let colors = Palette::read(
            &[
                ("ui", SOURCE),
                ("map", b"[TeamColorFilter]\nColorIndexAlly=6\n"),
            ],
            4096,
        )
        .unwrap();
        assert_eq!(colors.model(Role::Player), Some(3));
        assert_eq!(colors.model(Role::Ally), Some(6));
        assert_eq!(colors.minimap(Role::Player), Some([7, 11, 13, 17]));
        assert_eq!(colors.minimap(Role::Enemy), Some([59, 61, 67, 53]));
        assert_eq!(colors.model(Role::Original), None);
        assert_eq!(colors.sources, ["ui", "map"]);
    }
    #[test]
    fn malformed_missing_duplicate_and_over_budget_profiles_refuse() {
        assert!(Palette::read(&[("ui", SOURCE)], 1).is_err());
        assert!(Palette::read(&[], 4096).is_err());
        for (from, to) in [
            ("ColorIndexPlayer=3", "ColorIndexPlayer=-1"),
            ("FogColorAlly=19,23,29,31", "FogColorAlly=19,23,29"),
            (
                "ColorIndexEnemy=10",
                "ColorIndexEnemy=10\nColorIndexEnemy=11",
            ),
            ("FogColorEnemy=53,59,61,67", "FogColorEnemy=256,59,61,67"),
        ] {
            let bad = String::from_utf8_lossy(SOURCE).replace(from, to);
            assert!(Palette::read(&[("ui", bad.as_bytes())], 4096).is_err());
        }
    }
}
