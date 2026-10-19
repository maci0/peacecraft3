//! Hero progression settings from explicitly ordered stock/map profiles.
use super::defaults::{parse_config_profile, profile_elements};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progression {
    pub max_level: i32,
    pub ability_level_skip: i32,
    pub sources: Vec<String>,
}

impl Progression {
    /// Later supplied profiles override earlier ones; duplicates within a profile refuse.
    pub fn read(profiles: &[(&str, &[u8])], max_bytes: usize) -> Result<Self, String> {
        let mut total = 0usize;
        let mut maximum = None;
        let mut skip = None;
        let mut sources = Vec::new();
        for &(member, bytes) in profiles {
            total = total
                .checked_add(bytes.len())
                .and_then(|n| n.checked_add(member.len()))
                .ok_or("hero profile size overflow")?;
            if member.is_empty() || total > max_bytes {
                return Err(format!("{member}: hero profile byte budget or identity"));
            }
            let profile = parse_config_profile(bytes, member).map_err(|e| e.to_string())?;
            if let Some(error) = profile.unresolved().first() {
                return Err(error.to_string());
            }
            for (key, target, minimum) in [
                ("maxherolevel", &mut maximum, 1),
                ("heroabilitylevelskip", &mut skip, 0),
            ] {
                let fields = profile.raw_fields("misc", key);
                if fields.is_empty() {
                    continue;
                }
                let [(_, raw)] = fields.as_slice() else {
                    return Err(format!("{member}: duplicate Misc/{key}"));
                };
                let values = profile_elements(raw).map_err(|e| format!("{member}: {e}"))?;
                let [value] = values.as_slice() else {
                    return Err(format!("{member}: expected scalar Misc/{key}"));
                };
                *target = Some(
                    value
                        .parse::<i32>()
                        .ok()
                        .filter(|v| *v >= minimum)
                        .ok_or_else(|| format!("{member}: invalid Misc/{key}"))?,
                );
            }
            sources.push(member.to_owned());
        }
        Ok(Self {
            max_level: maximum.ok_or("missing Misc/MaxHeroLevel")?,
            ability_level_skip: skip.ok_or("missing Misc/HeroAbilityLevelSkip")?,
            sources,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_profiles_supply_progression_and_map_overrides() {
        let base = b"[Misc]\nMaxHeroLevel=19\nHeroAbilityLevelSkip=3 // fixture\n";
        let map = b"[Misc]\nMaxHeroLevel=27\n";
        let p = Progression::read(&[("stock", base), ("map", map)], 256).unwrap();
        assert_eq!((p.max_level, p.ability_level_skip), (27, 3));
        assert_eq!(p.sources, ["stock", "map"]);
        assert!(Progression::read(&[("stock", base)], 1).is_err());
        for invalid in [
            "[Misc]\nMaxHeroLevel=0\nHeroAbilityLevelSkip=2",
            "[Misc]\nMaxHeroLevel=3\nHeroAbilityLevelSkip=-1",
            "[Misc]\nMaxHeroLevel=3\nMaxHeroLevel=3\nHeroAbilityLevelSkip=2",
            "[Misc]\nMaxHeroLevel=3,4\nHeroAbilityLevelSkip=2",
            "[Misc]\nMaxHeroLevel=3",
        ] {
            assert!(Progression::read(&[("bad", invalid.as_bytes())], 256).is_err());
        }
    }
}
