//! Total hero-XP thresholds from ordered stock/map profiles.
use super::defaults::{parse_config_profile, profile_elements};

#[derive(Clone, Debug)]
pub struct Experience {
    thresholds: Vec<i32>,
    presentation_ceiling: Result<i32, String>,
    pub sources: Vec<String>,
}
impl Experience {
    pub fn read(
        profiles: &[(&str, &[u8])],
        initial: i32,
        maximum: i32,
        max_bytes: usize,
        max_levels: usize,
    ) -> Result<Self, String> {
        let count = usize::try_from(maximum.checked_sub(initial).ok_or("XP level overflow")?)
            .map_err(|_| "XP level domain")?;
        if initial < 1 || count > max_levels {
            return Err("XP level budget/domain".into());
        }
        let mut fields = [None, None, None, None];
        let mut total = 0usize;
        let mut sources = Vec::new();
        for &(name, bytes) in profiles {
            total = total
                .checked_add(name.len())
                .and_then(|n| n.checked_add(bytes.len()))
                .ok_or("XP profile overflow")?;
            if name.is_empty() || total > max_bytes {
                return Err(format!("{name}: XP profile budget/identity"));
            }
            let profile = parse_config_profile(bytes, name).map_err(|e| e.to_string())?;
            if let Some(error) = profile.unresolved().first() {
                return Err(error.to_string());
            }
            for (index, key) in [
                "needheroxp",
                "needheroxpformulaa",
                "needheroxpformulab",
                "needheroxpformulac",
            ]
            .iter()
            .enumerate()
            {
                let raw = profile.raw_fields("misc", key);
                if raw.is_empty() {
                    continue;
                }
                let [(_, value)] = raw.as_slice() else {
                    return Err(format!("{name}: duplicate Misc/{key}"));
                };
                let values = profile_elements(value).map_err(|e| e.to_string())?;
                if values.is_empty()
                    || values.len() > max_levels.max(1)
                    || (index != 0 && values.len() != 1)
                {
                    return Err(format!("{name}: invalid Misc/{key} shape"));
                }
                fields[index] = Some(
                    values
                        .iter()
                        .map(|v| {
                            v.parse::<i32>()
                                .map_err(|_| format!("{name}: invalid Misc/{key}"))
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            sources.push(name.to_string());
        }
        let [table, a, b, c] = fields;
        let table = table.ok_or("missing Misc/NeedHeroXP")?;
        let (a, b, c) = (
            a.ok_or("missing XP formula A")?[0],
            b.ok_or("missing XP formula B")?[0],
            c.ok_or("missing XP formula C")?[0],
        );
        let mut thresholds = Vec::new();
        thresholds
            .try_reserve_exact(count)
            .map_err(|_| "XP allocation")?;
        let threshold = |index: usize, previous: Option<i32>| -> Result<i32, String> {
            if let Some(value) = table.get(index) {
                return Ok(*value);
            }
            let level = initial
                .checked_add(i32::try_from(index).map_err(|_| "XP index")?)
                .and_then(|n| n.checked_add(1))
                .ok_or("XP level overflow")?;
            let previous = previous.ok_or("XP table is empty")?;
            a.checked_mul(previous)
                .and_then(|n| b.checked_mul(level).and_then(|b| n.checked_add(b)))
                .and_then(|n| n.checked_add(c))
                .ok_or_else(|| "XP recurrence overflow".into())
        };
        for index in 0..count {
            let value = threshold(index, thresholds.last().copied())?;
            if value <= thresholds.last().copied().unwrap_or(0) {
                return Err("XP thresholds must be positive and strictly increasing".into());
            }
            thresholds.push(value);
        }
        // The status bar reads one threshold beyond the gameplay level cap.
        // An unusable presentation endpoint must not reject valid leveling rules.
        let presentation_ceiling = threshold(count, thresholds.last().copied());
        Ok(Self {
            thresholds,
            presentation_ceiling,
            sources,
        })
    }
    pub fn level_for(&self, initial: i32, xp: i32) -> Result<i32, String> {
        if xp < 0 {
            return Err("negative hero XP".into());
        }
        initial
            .checked_add(
                i32::try_from(self.thresholds.partition_point(|v| *v <= xp))
                    .map_err(|_| "XP level overflow")?,
            )
            .ok_or_else(|| "XP level overflow".into())
    }
    /// XP floor and next threshold, including the source endpoint at the level cap.
    pub fn presentation_range(&self, initial: i32, level: i32) -> Result<[i32; 2], String> {
        let offset = level
            .checked_sub(initial)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n <= self.thresholds.len())
            .ok_or("XP level outside source table")?;
        let lower = self.minimum_for(initial, level)?;
        let upper = match self.thresholds.get(offset) {
            Some(value) => *value,
            None => self.presentation_ceiling.clone()?,
        };
        Ok([lower, upper])
    }
    pub fn minimum_for(&self, initial: i32, level: i32) -> Result<i32, String> {
        if level == initial {
            return Ok(0);
        }
        let index = level
            .checked_sub(initial)
            .and_then(|n| n.checked_sub(1))
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("XP level outside source table")?;
        self.thresholds
            .get(index)
            .copied()
            .ok_or_else(|| "XP level outside source table".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recurrence_uses_target_level_and_map_fields_override_independently() {
        let base = b"[Misc]\nNeedHeroXP=17,41\nNeedHeroXPFormulaA=2\nNeedHeroXPFormulaB=3\nNeedHeroXPFormulaC=5\n";
        let p = Experience::read(
            &[("stock", base), ("map", b"[Misc]\nNeedHeroXPFormulaC=7")],
            1,
            4,
            1024,
            20,
        )
        .unwrap();
        assert_eq!(p.thresholds, [17, 41, 101]);
        for (xp, level) in [
            (0, 1),
            (16, 1),
            (17, 2),
            (40, 2),
            (41, 3),
            (100, 3),
            (101, 4),
            (1000, 4),
        ] {
            assert_eq!(p.level_for(1, xp).unwrap(), level);
        }
        assert_eq!(p.minimum_for(1, 3).unwrap(), 41);
        assert_eq!(p.presentation_range(1, 1).unwrap(), [0, 17]);
        assert_eq!(p.presentation_range(1, 3).unwrap(), [41, 101]);
        assert_eq!(p.presentation_range(1, 4).unwrap(), [101, 224]);
        assert!(p.presentation_range(1, 0).is_err());
        assert!(p.presentation_range(1, 5).is_err());
        assert!(p.minimum_for(1, 5).is_err());
        assert!(p.level_for(1, -1).is_err());
        assert!(Experience::read(&[("stock", base)], 1, 4, 1024, 1).is_err());
        for field in [
            "NeedHeroXP=0",
            "NeedHeroXP=7,7",
            "NeedHeroXP=2147483647",
            "NeedHeroXP=bad",
            "NeedHeroXPFormulaA=1,2",
            "NeedHeroXP=2\nNeedHeroXP=3",
        ] {
            assert!(
                Experience::read(
                    &[
                        ("stock", base),
                        ("bad", format!("[Misc]\n{field}").as_bytes())
                    ],
                    1,
                    4,
                    1024,
                    20
                )
                .is_err(),
                "{field}"
            );
        }
    }
    #[test]
    fn presentation_endpoint_does_not_extend_or_reject_gameplay_levels() {
        let profile = b"[Misc]\nNeedHeroXP=23\nNeedHeroXPFormulaA=2147483647\nNeedHeroXPFormulaB=0\nNeedHeroXPFormulaC=0";
        let rules = Experience::read(&[("synthetic", profile)], 3, 4, 1024, 8).unwrap();
        assert_eq!(rules.level_for(3, i32::MAX).unwrap(), 4);
        assert_eq!(rules.presentation_range(3, 3).unwrap(), [0, 23]);
        assert!(
            rules
                .presentation_range(3, 4)
                .unwrap_err()
                .contains("overflow")
        );
        let constant = b"[Misc]\nNeedHeroXP=23\nNeedHeroXPFormulaA=1\nNeedHeroXPFormulaB=0\nNeedHeroXPFormulaC=0";
        let rules = Experience::read(&[("synthetic", constant)], 3, 4, 1024, 8).unwrap();
        assert_eq!(rules.presentation_range(3, 4).unwrap(), [23, 23]);
        assert_eq!(rules.level_for(3, 100).unwrap(), 4);
    }
}
