//! `[Misc]` settings from explicitly ordered stock/map profiles.
use super::defaults::{parse_config_profile, profile_elements};

/// Later supplied profiles override earlier ones; duplicates within a profile
/// refuse. Each key must be one finite value; absent keys stay `None`.
pub fn misc_scalars<const N: usize>(
    profiles: &[(&str, &[u8])],
    keys: [&str; N],
    max_bytes: usize,
) -> Result<([Option<f32>; N], Vec<String>), String> {
    let mut total = 0usize;
    let mut values = [None; N];
    let mut sources = Vec::new();
    for &(member, bytes) in profiles {
        total = total
            .checked_add(bytes.len())
            .and_then(|n| n.checked_add(member.len()))
            .ok_or("misc profile size overflow")?;
        if member.is_empty() || total > max_bytes {
            return Err(format!("{member}: misc profile byte budget or identity"));
        }
        let profile = parse_config_profile(bytes, member).map_err(|e| e.to_string())?;
        if let Some(error) = profile.unresolved().first() {
            return Err(error.to_string());
        }
        for (key, target) in keys.iter().zip(values.iter_mut()) {
            let fields = profile.raw_fields("misc", &key.to_ascii_lowercase());
            if fields.is_empty() {
                continue;
            }
            let [(_, raw)] = fields.as_slice() else {
                return Err(format!("{member}: duplicate Misc/{key}"));
            };
            let elements = profile_elements(raw).map_err(|e| format!("{member}: {e}"))?;
            let [value] = elements.as_slice() else {
                return Err(format!("{member}: expected scalar Misc/{key}"));
            };
            *target = Some(
                value
                    .parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("{member}: invalid Misc/{key}"))?,
            );
        }
        sources.push(member.to_owned());
    }
    Ok((values, sources))
}

/// Like [`misc_scalars`] for comma lists of exactly `width` finite values.
pub fn misc_lists<const N: usize>(
    profiles: &[(&str, &[u8])],
    keys: [&str; N],
    width: usize,
    max_bytes: usize,
) -> Result<[Option<Vec<f32>>; N], String> {
    let mut values: [Option<Vec<f32>>; N] = std::array::from_fn(|_| None);
    let mut total = 0usize;
    for &(member, bytes) in profiles {
        total = total
            .checked_add(bytes.len())
            .ok_or("misc profile size overflow")?;
        if member.is_empty() || total > max_bytes {
            return Err(format!("{member}: misc profile byte budget or identity"));
        }
        let profile = parse_config_profile(bytes, member).map_err(|e| e.to_string())?;
        for (key, target) in keys.iter().zip(values.iter_mut()) {
            let fields = profile.raw_fields("misc", &key.to_ascii_lowercase());
            if fields.is_empty() {
                continue;
            }
            let [(_, raw)] = fields.as_slice() else {
                return Err(format!("{member}: duplicate Misc/{key}"));
            };
            let list = profile_elements(raw)
                .map_err(|e| format!("{member}: {e}"))?
                .iter()
                .map(|v| v.parse::<f32>().ok().filter(|v| v.is_finite()))
                .collect::<Option<Vec<_>>>()
                .filter(|list| list.len() == width)
                .ok_or_else(|| format!("{member}: Misc/{key} needs {width} finite values"))?;
            *target = Some(list);
        }
    }
    Ok(values)
}

/// Defense-type order of every `DamageBonus*` list, as the stock profile documents.
pub const DEFENSE_TYPES: [&str; 8] = [
    "small", "medium", "large", "fort", "normal", "hero", "divine", "none",
];
/// Attack types named by the `DamageBonus<Type>` keys.
pub const ATTACK_TYPES: [&str; 7] = [
    "normal", "pierce", "siege", "magic", "chaos", "spells", "hero",
];

#[derive(Clone, Debug, PartialEq)]
pub struct CombatRules {
    pub armor_factor: f32,
    /// `[attack][defense]` in [`ATTACK_TYPES`] by [`DEFENSE_TYPES`] order.
    pub damage_bonus: [[f32; 8]; 7],
    pub call_for_help: f32,
    pub creep_call_for_help: f32,
    pub guard_distance: f32,
    pub max_guard_distance: f32,
    pub guard_return_time: f32,
}

impl CombatRules {
    pub fn read(profiles: &[(&str, &[u8])], max_bytes: usize) -> Result<Self, String> {
        let keys = [
            "DefenseArmor",
            "CallForHelp",
            "CreepCallForHelp",
            "GuardDistance",
            "MaxGuardDistance",
            "GuardReturnTime",
        ];
        let (scalars, _) = misc_scalars(profiles, keys, max_bytes)?;
        let [armor, help, creep_help, guard, max_guard, guard_time] = scalars;
        let lists = misc_lists(
            profiles,
            [
                "DamageBonusNormal",
                "DamageBonusPierce",
                "DamageBonusSiege",
                "DamageBonusMagic",
                "DamageBonusChaos",
                "DamageBonusSpells",
                "DamageBonusHero",
            ],
            DEFENSE_TYPES.len(),
            max_bytes,
        )?;
        let mut damage_bonus = [[0.0; 8]; 7];
        for (row, (list, name)) in damage_bonus.iter_mut().zip(lists.iter().zip(ATTACK_TYPES)) {
            let list = list
                .as_ref()
                .ok_or_else(|| format!("missing Misc/DamageBonus for {name}"))?;
            row.copy_from_slice(list);
        }
        let required = |v: Option<f32>, key: &str| v.ok_or_else(|| format!("missing Misc/{key}"));
        Ok(Self {
            armor_factor: required(armor, keys[0])?,
            damage_bonus,
            call_for_help: required(help, keys[1])?,
            creep_call_for_help: required(creep_help, keys[2])?,
            guard_distance: required(guard, keys[3])?,
            max_guard_distance: required(max_guard, keys[4])?,
            guard_return_time: required(guard_time, keys[5])?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DayCycle {
    /// Game seconds per game day (`Misc/DayLength`).
    pub day_length: f32,
    /// Hours per game day (`Misc/DayHours`).
    pub day_hours: f32,
    pub sources: Vec<String>,
}

impl DayCycle {
    pub fn read(profiles: &[(&str, &[u8])], max_bytes: usize) -> Result<Self, String> {
        let ([length, hours], sources) =
            misc_scalars(profiles, ["DayLength", "DayHours"], max_bytes)?;
        let positive = |v: Option<f32>, key| {
            v.filter(|v| *v > 0.0)
                .ok_or_else(|| format!("missing or invalid Misc/{key}"))
        };
        Ok(Self {
            day_length: positive(length, "DayLength")?,
            day_hours: positive(hours, "DayHours")?,
            sources,
        })
    }
}

/// Hero attribute bonuses (`Misc/StrHitPointBonus`, `Misc/IntManaBonus`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeroVitals {
    pub life_per_strength: f32,
    pub mana_per_intelligence: f32,
}

impl HeroVitals {
    pub fn read(profiles: &[(&str, &[u8])], max_bytes: usize) -> Result<Self, String> {
        let ([life, mana], _) =
            misc_scalars(profiles, ["StrHitPointBonus", "IntManaBonus"], max_bytes)?;
        Ok(Self {
            life_per_strength: life.ok_or("missing Misc/StrHitPointBonus")?,
            mana_per_intelligence: mana.ok_or("missing Misc/IntManaBonus")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_profiles_supply_day_rate_and_map_overrides() {
        let base = b"[Misc]\n// fixture\nDayLength=37.5\nDayHours=11\n";
        let map = b"[Misc]\nDayLength=90\n";
        let c = DayCycle::read(&[("stock", base), ("map", map)], 256).unwrap();
        assert_eq!((c.day_length, c.day_hours), (90.0, 11.0));
        assert_eq!(c.sources, ["stock", "map"]);
        assert!(DayCycle::read(&[("stock", base)], 1).is_err());
        for invalid in [
            "[Misc]\nDayLength=0\nDayHours=2",
            "[Misc]\nDayLength=3\nDayHours=-1",
            "[Misc]\nDayLength=3\nDayLength=3\nDayHours=2",
            "[Misc]\nDayLength=3,4\nDayHours=2",
            "[Misc]\nDayLength=inf\nDayHours=2",
            "[Misc]\nDayLength=3",
        ] {
            assert!(DayCycle::read(&[("bad", invalid.as_bytes())], 256).is_err());
        }
    }

    #[test]
    fn combat_rules_read_bonus_rows_in_attack_order() {
        let mut text = String::from(
            "[Misc]\nDefenseArmor=0.5\nCallForHelp=1\nCreepCallForHelp=2\nGuardDistance=3\nMaxGuardDistance=4\nGuardReturnTime=5\n",
        );
        for (i, name) in [
            "Normal", "Pierce", "Siege", "Magic", "Chaos", "Spells", "Hero",
        ]
        .iter()
        .enumerate()
        {
            text.push_str(&format!("DamageBonus{name}={i},1,2,3,4,5,6,7\n"));
        }
        let rules = CombatRules::read(&[("stock", text.as_bytes())], 4096).unwrap();
        assert_eq!(rules.armor_factor, 0.5);
        assert_eq!(
            rules.damage_bonus[3],
            [3.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
        );
        assert_eq!(rules.guard_return_time, 5.0);
        let short = text.replace("DamageBonusHero=6,1,2,3,4,5,6,7", "DamageBonusHero=6,1");
        assert!(CombatRules::read(&[("stock", short.as_bytes())], 4096).is_err());
    }

    #[test]
    fn hero_vitals_read_attribute_bonuses() {
        let base = b"[Misc]\nStrHitPointBonus=7\nIntManaBonus=3.5\n";
        let map = b"[Misc]\nIntManaBonus=4\n";
        let v = HeroVitals::read(&[("stock", base), ("map", map)], 256).unwrap();
        assert_eq!((v.life_per_strength, v.mana_per_intelligence), (7.0, 4.0));
        assert!(HeroVitals::read(&[("bad", b"[Misc]\nIntManaBonus=1\n")], 256).is_err());
    }
}
