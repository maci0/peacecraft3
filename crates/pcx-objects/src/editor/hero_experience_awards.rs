//! Kill-award tables and policies from ordered stock/map Misc profiles.
use super::defaults::{parse_config_profile, profile_elements};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Awards {
    normal: Vec<i32>,
    heroes: Vec<i32>,
    creep: Vec<f32>,
    pub range: f32,
    pub global: bool,
    pub buildings: bool,
    pub maximum_drains: bool,
    pub summoned: f32,
    pub sources: Vec<String>,
}
impl Awards {
    pub fn read(
        profiles: &[(&str, &[u8])],
        maximum_hero: i32,
        max_bytes: usize,
        max_levels: usize,
    ) -> Result<Self, String> {
        let keys = [
            "maxunitlevel",
            "grantheroxp",
            "grantnormalxp",
            "herofactorxp",
            "summonedkillfactor",
            "heroexprange",
            "globalexperience",
            "buildingkillsgiveexp",
            "maxlevelheroesdrainexp",
            "grantheroxpformulaa",
            "grantheroxpformulab",
            "grantheroxpformulac",
            "grantnormalxpformulaa",
            "grantnormalxpformulab",
            "grantnormalxpformulac",
        ];
        let mut fields = BTreeMap::new();
        let mut total = 0usize;
        let mut sources = Vec::new();
        for &(name, bytes) in profiles {
            total = total
                .checked_add(name.len())
                .and_then(|n| n.checked_add(bytes.len()))
                .ok_or("XP award input overflow")?;
            if name.is_empty() || total > max_bytes {
                return Err(format!("{name}: XP award input budget/identity"));
            }
            let profile = parse_config_profile(bytes, name).map_err(|e| e.to_string())?;
            if let Some(error) = profile.unresolved().first() {
                return Err(error.to_string());
            }
            for key in keys {
                let raw = profile.raw_fields("misc", key);
                if raw.is_empty() {
                    continue;
                }
                let [(_, raw)] = raw.as_slice() else {
                    return Err(format!("{name}: duplicate Misc/{key}"));
                };
                let values = profile_elements(raw).map_err(|e| e.to_string())?;
                if values.is_empty() || values.len() > max_levels {
                    return Err(format!("{name}: Misc/{key} element budget"));
                }
                fields.insert(
                    key,
                    values.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
                );
            }
            sources.push(name.to_string());
        }
        let values = |key| fields.get(key).ok_or_else(|| format!("missing Misc/{key}"));
        let integer = |key| -> Result<i32, String> {
            let [value] = values(key)?.as_slice() else {
                return Err(format!("Misc/{key}: scalar required"));
            };
            value
                .parse()
                .map_err(|_| format!("Misc/{key}: integer required"))
        };
        let real = |key| -> Result<f32, String> {
            let [value] = values(key)?.as_slice() else {
                return Err(format!("Misc/{key}: scalar required"));
            };
            value
                .parse::<f32>()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or_else(|| format!("Misc/{key}: finite nonnegative real required"))
        };
        let boolean = |key| -> Result<bool, String> {
            match integer(key)? {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(format!("Misc/{key}: boolean required")),
            }
        };
        let series = |key, a, b, c, maximum: i32| -> Result<Vec<i32>, String> {
            let count = usize::try_from(maximum)
                .ok()
                .filter(|n| *n > 0 && *n <= max_levels)
                .ok_or("XP grant level budget/domain")?;
            let table = values(key)?;
            let (a, b, c) = (integer(a)?, integer(b)?, integer(c)?);
            let mut result = Vec::<i32>::new();
            result
                .try_reserve_exact(count)
                .map_err(|_| "XP grant allocation")?;
            for index in 0..count {
                let value = if let Some(value) = table.get(index) {
                    value
                        .parse()
                        .map_err(|_| format!("Misc/{key}: integer grant required"))?
                } else {
                    let previous = *result.last().ok_or("empty XP grant table")?;
                    a.checked_mul(previous)
                        .and_then(|v| {
                            b.checked_mul(index as i32 + 1)
                                .and_then(|b| v.checked_add(b))
                        })
                        .and_then(|v| v.checked_add(c))
                        .ok_or("XP grant recurrence overflow")?
                };
                if value < 0 {
                    return Err(format!("Misc/{key}: negative grant"));
                }
                result.push(value);
            }
            Ok(result)
        };
        let normal = series(
            "grantnormalxp",
            "grantnormalxpformulaa",
            "grantnormalxpformulab",
            "grantnormalxpformulac",
            integer("maxunitlevel")?,
        )?;
        let heroes = series(
            "grantheroxp",
            "grantheroxpformulaa",
            "grantheroxpformulab",
            "grantheroxpformulac",
            maximum_hero,
        )?;
        let creep = values("herofactorxp")?
            .iter()
            .map(|v| {
                v.parse::<f32>()
                    .ok()
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .map(|n| n / 100.0)
                    .ok_or("Misc/HeroFactorXP: nonnegative percentage required")
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            normal,
            heroes,
            creep,
            range: real("heroexprange")?,
            global: boolean("globalexperience")?,
            buildings: boolean("buildingkillsgiveexp")?,
            maximum_drains: boolean("maxlevelheroesdrainexp")?,
            summoned: real("summonedkillfactor")?,
            sources,
        })
    }
    pub fn grant(&self, hero: bool, level: i32) -> Result<i32, String> {
        let table = if hero { &self.heroes } else { &self.normal };
        let index = usize::try_from(level.checked_sub(1).ok_or("XP victim level overflow")?)
            .map_err(|_| "XP victim level must be positive")?;
        table
            .get(index)
            .copied()
            .ok_or_else(|| "XP victim level outside admitted source table".into())
    }
    pub fn creep_factor(&self, level: i32) -> Result<f32, String> {
        let index = usize::try_from(level.checked_sub(1).ok_or("XP hero level overflow")?)
            .map_err(|_| "XP hero level must be positive")?;
        Ok(self.creep[index.min(self.creep.len() - 1)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) const PROFILE: &[u8] = b"[Misc]\nMaxUnitLevel=4\nGrantHeroXP=31,43\nGrantNormalXP=11\nHeroFactorXP=75,25,0\nSummonedKillFactor=0.3\nHeroExpRange=71\nGlobalExperience=1\nBuildingKillsGiveExp=0\nMaxLevelHeroesDrainExp=1\nGrantHeroXPFormulaA=1\nGrantHeroXPFormulaB=3\nGrantHeroXPFormulaC=7\nGrantNormalXPFormulaA=2\nGrantNormalXPFormulaB=3\nGrantNormalXPFormulaC=7\n";
    #[test]
    fn grants_extend_by_source_level_and_ordered_overrides_preserve_policies() {
        let a = Awards::read(
            &[
                ("stock", PROFILE),
                ("map", b"[Misc]\nGrantNormalXPFormulaC=9\nHeroExpRange=103"),
            ],
            4,
            4096,
            8,
        )
        .unwrap();
        assert_eq!(a.normal, [11, 37, 92, 205]);
        assert_eq!(a.heroes, [31, 43, 59, 78]);
        assert_eq!(a.creep_factor(1).unwrap(), 0.75);
        assert_eq!(a.creep_factor(7).unwrap(), 0.0);
        assert_eq!(a.range, 103.0);
        assert!(a.global && a.maximum_drains && !a.buildings);
        assert_eq!(a.summoned, 0.3);
        assert!(a.grant(false, 0).is_err());
        assert!(a.grant(false, 5).is_err());
        for invalid in [
            b"[Misc]\nGlobalExperience=2".as_slice(),
            b"[Misc]\nHeroFactorXP=NaN",
            b"[Misc]\nMaxUnitLevel=99999",
            b"[Misc]\nGrantNormalXPFormulaA=2147483647",
            b"[Misc]\nHeroExpRange=-1",
        ] {
            assert!(Awards::read(&[("stock", PROFILE), ("bad", invalid)], 4, 4096, 8).is_err());
        }
        assert!(Awards::read(&[("stock", PROFILE)], 4, 1, 8).is_err());
    }
}
