//! `[Misc]` settings from explicitly ordered stock/map profiles.
use super::defaults::{parse_config_profile, profile_elements};

/// An ordered profile integer list, replacing the entire earlier list. Missing
/// elements use the caller's source-derived initializer; excess elements are ignored.
/// Decimal tokens are admitted without float conversion. Other native token
/// forms (base prefixes, FourCC and partial parses) remain explicitly unsupported.
pub fn misc_integer_list(
    profiles: &[(&str, &[u8])],
    key: &str,
    width: usize,
    missing: i32,
    max_bytes: usize,
) -> Result<Option<Vec<i32>>, String> {
    let mut value = None;
    let mut total = 0usize;
    if width > max_bytes / size_of::<i32>() {
        return Err("misc integer list output budget".into());
    }
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
        let fields = profile.raw_fields("misc", &key.to_ascii_lowercase());
        if fields.is_empty() {
            continue;
        }
        let [(_, raw)] = fields.as_slice() else {
            return Err(format!("{member}: duplicate Misc/{key}"));
        };
        let elements = profile_elements(raw).map_err(|e| format!("{member}: {e}"))?;
        let mut list = vec![missing; width];
        for (target, token) in list.iter_mut().zip(elements) {
            // Native base-zero parsing differs for a leading zero; refuse that
            // spelling rather than silently interpreting an octal token as decimal.
            let digits = token.trim_start_matches(['+', '-']);
            if digits.is_empty()
                || !digits.bytes().all(|b| b.is_ascii_digit())
                || (digits.len() > 1 && digits.starts_with('0'))
            {
                return Err(format!(
                    "{member}: unsupported integer token Misc/{key}: {token}"
                ));
            }
            *target = token
                .parse()
                .map_err(|_| format!("{member}: integer overflow Misc/{key}"))?;
        }
        value = Some(list);
    }
    Ok(value)
}

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
    pub sources: Vec<String>,
    pub armor_factor: f32,
    pub primary_damage_per_attribute: f32,
    pub armor_per_agility: f32,
    pub hero_armor_base: f32,
    pub attack_speed_per_agility: f32,
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
            "StrAttackBonus",
            "AgiDefenseBonus",
            "AgiDefenseBase",
            "AgiAttackSpeedBonus",
        ];
        let (scalars, sources) = misc_scalars(profiles, keys, max_bytes)?;
        let [
            armor,
            help,
            creep_help,
            guard,
            max_guard,
            guard_time,
            primary,
            agility_armor,
            hero_armor,
            agility_speed,
        ] = scalars;
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
            sources,
            armor_factor: required(armor, keys[0])?,
            primary_damage_per_attribute: required(primary, keys[6])?,
            armor_per_agility: required(agility_armor, keys[7])?,
            hero_armor_base: required(hero_armor, keys[8])?,
            attack_speed_per_agility: required(agility_speed, keys[9])?,
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

/// Unit and building move-speed limits (`Misc/MinUnitSpeed`, `Misc/MaxUnitSpeed`,
/// `Misc/MinBldgSpeed`, `Misc/MaxBldgSpeed`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoveSpeedLimits {
    pub min_unit: f32,
    pub max_unit: f32,
    pub min_building: f32,
    pub max_building: f32,
}

impl MoveSpeedLimits {
    pub fn read(profiles: &[(&str, &[u8])], max_bytes: usize) -> Result<Self, String> {
        let ([min_unit, max_unit, min_building, max_building], _) = misc_scalars(
            profiles,
            [
                "MinUnitSpeed",
                "MaxUnitSpeed",
                "MinBldgSpeed",
                "MaxBldgSpeed",
            ],
            max_bytes,
        )?;
        let required =
            |value: Option<f32>, key: &str| value.ok_or_else(|| format!("missing Misc/{key}"));
        let limits = Self {
            min_unit: required(min_unit, "MinUnitSpeed")?,
            max_unit: required(max_unit, "MaxUnitSpeed")?,
            min_building: required(min_building, "MinBldgSpeed")?,
            max_building: required(max_building, "MaxBldgSpeed")?,
        };
        if [
            limits.min_unit,
            limits.max_unit,
            limits.min_building,
            limits.max_building,
        ]
        .iter()
        .any(|value| *value < 0.)
        {
            return Err("Misc move speed limit is negative".into());
        }
        if limits.min_unit > limits.max_unit || limits.min_building > limits.max_building {
            return Err("Misc move speed minimum exceeds maximum".into());
        }
        Ok(limits)
    }

    pub fn for_building(self, building: bool) -> (f32, f32) {
        if building {
            (self.min_building, self.max_building)
        } else {
            (self.min_unit, self.max_unit)
        }
    }
}

/// Hero attribute bonuses (`Misc/StrHitPointBonus`, `Misc/IntManaBonus`) and
/// per-attribute regeneration rates (`Misc/StrRegenBonus`, `Misc/IntRegenBonus`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeroVitals {
    pub life_per_strength: f32,
    pub mana_per_intelligence: f32,
    pub life_regen_per_strength: f32,
    pub mana_regen_per_intelligence: f32,
}

impl HeroVitals {
    pub fn read(profiles: &[(&str, &[u8])], max_bytes: usize) -> Result<Self, String> {
        let ([life, mana, life_regen, mana_regen], _) = misc_scalars(
            profiles,
            [
                "StrHitPointBonus",
                "IntManaBonus",
                "StrRegenBonus",
                "IntRegenBonus",
            ],
            max_bytes,
        )?;
        Ok(Self {
            life_per_strength: life.ok_or("missing Misc/StrHitPointBonus")?,
            mana_per_intelligence: mana.ok_or("missing Misc/IntManaBonus")?,
            life_regen_per_strength: life_regen.ok_or("missing Misc/StrRegenBonus")?,
            mana_regen_per_intelligence: mana_regen.ok_or("missing Misc/IntRegenBonus")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_lists_preserve_precision_whole_override_and_source_padding() {
        let profiles = [
            ("stock", b"[Misc]\nCount=16777217,9,11,13\n".as_slice()),
            ("map", b"[Misc]\nCount=3\n"),
        ];
        assert_eq!(
            misc_integer_list(&profiles, "Count", 3, -7, 512).unwrap(),
            Some(vec![3, -7, -7])
        );
        assert_eq!(
            misc_integer_list(&profiles[..1], "Count", 3, -7, 512).unwrap(),
            Some(vec![16777217, 9, 11])
        );
        assert_eq!(
            misc_integer_list(&profiles, "Absent", 3, -7, 512).unwrap(),
            None
        );
        for value in ["0x10", "017", "3.5", "2147483648", "1\nCount=2"] {
            let bytes = format!("[Misc]\nCount={value}\n");
            assert!(misc_integer_list(&[("map", bytes.as_bytes())], "Count", 3, 0, 512).is_err());
        }
        assert!(misc_integer_list(&profiles, "Count", 3, 0, 1).is_err());
    }
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
    fn move_speed_limits_select_unit_and_building_pairs() {
        let stock =
            b"[Misc]\nMinUnitSpeed=10\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300\n";
        let map =
            b"[Misc]\nMinUnitSpeed=120\nMaxUnitSpeed=350\nMinBldgSpeed=40\nMaxBldgSpeed=250\n";
        let limits = MoveSpeedLimits::read(&[("stock", stock), ("map", map)], 512).unwrap();
        assert_eq!(limits.for_building(false), (120.0, 350.0));
        assert_eq!(limits.for_building(true), (40.0, 250.0));
        for invalid in [
            "[Misc]\nMinUnitSpeed=10\nMaxUnitSpeed=400\nMinBldgSpeed=25",
            "[Misc]\nMinUnitSpeed=500\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300",
            "[Misc]\nMinUnitSpeed=-1\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300",
            "[Misc]\nMinUnitSpeed=10\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=-300",
            "[Misc]\nMinUnitSpeed=10,20\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300",
            "[Misc]\nMinUnitSpeed=NaN\nMaxUnitSpeed=400\nMinBldgSpeed=25\nMaxBldgSpeed=300",
            "[Misc]\nMinUnitSpeed=10\nMaxUnitSpeed=inf\nMinBldgSpeed=25\nMaxBldgSpeed=300",
        ] {
            assert!(MoveSpeedLimits::read(&[("bad", invalid.as_bytes())], 512).is_err());
        }
        assert!(MoveSpeedLimits::read(&[], 512).is_err());
    }

    #[test]
    fn combat_rules_read_bonus_rows_in_attack_order() {
        let mut text = String::from(
            "[Misc]\nDefenseArmor=0.5\nCallForHelp=1\nCreepCallForHelp=2\nGuardDistance=3\nMaxGuardDistance=4\nGuardReturnTime=5\nStrAttackBonus=2.5\nAgiDefenseBonus=0.7\nAgiDefenseBase=-3\nAgiAttackSpeedBonus=0.03\n",
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
        assert_eq!(
            (
                rules.primary_damage_per_attribute,
                rules.armor_per_agility,
                rules.hero_armor_base,
                rules.attack_speed_per_agility
            ),
            (2.5, 0.7, -3.0, 0.03)
        );
        let overridden = CombatRules::read(&[("stock", text.as_bytes()),
            ("map", b"[Misc]\nStrAttackBonus=4\nAgiDefenseBonus=0.8\nAgiDefenseBase=-7\nAgiAttackSpeedBonus=0.09\n")], 4096).unwrap();
        assert_eq!(overridden.sources, ["stock", "map"]);
        assert_eq!(
            (
                overridden.primary_damage_per_attribute,
                overridden.armor_per_agility,
                overridden.hero_armor_base,
                overridden.attack_speed_per_agility
            ),
            (4.0, 0.8, -7.0, 0.09)
        );
        for key in [
            "StrAttackBonus",
            "AgiDefenseBonus",
            "AgiDefenseBase",
            "AgiAttackSpeedBonus",
        ] {
            let missing = text
                .lines()
                .filter(|line| !line.starts_with(&format!("{key}=")))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(CombatRules::read(&[("missing", missing.as_bytes())], 4096).is_err());
        }
        let short = text.replace("DamageBonusHero=6,1,2,3,4,5,6,7", "DamageBonusHero=6,1");
        assert!(CombatRules::read(&[("stock", short.as_bytes())], 4096).is_err());
    }

    #[test]
    fn hero_vitals_read_attribute_bonuses() {
        let base = b"[Misc]\nStrHitPointBonus=7\nIntManaBonus=3.5\nStrRegenBonus=0.05\nIntRegenBonus=0.5\n";
        let map = b"[Misc]\nIntManaBonus=4\n";
        let v = HeroVitals::read(&[("stock", base), ("map", map)], 256).unwrap();
        assert_eq!((v.life_per_strength, v.mana_per_intelligence), (7.0, 4.0));
        assert_eq!(
            (v.life_regen_per_strength, v.mana_regen_per_intelligence),
            (0.05, 0.5)
        );
        assert!(HeroVitals::read(&[("bad", b"[Misc]\nIntManaBonus=1\n")], 256).is_err());
        let no_regen = b"[Misc]\nStrHitPointBonus=7\nIntManaBonus=3.5\n";
        assert!(HeroVitals::read(&[("stock", no_regen)], 256).is_err());
    }
}
