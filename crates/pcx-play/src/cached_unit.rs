//! Portable recreation-owned unit progress; never an original W3V payload.
use super::{Reader, Writer};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CachedUnit {
    pub rawcode: [u8; 4],
    pub client_sha256: [u8; 32],
    pub install: String,
    pub edition: String,
    pub hero: Option<[i32; 2]>,
    pub experience: Option<crate::hero_experience::State>,
    pub abilities: Vec<([u8; 4], i32)>,
    pub cooldowns: Vec<([u8; 4], i32, u32)>,
    pub permanent_strength: i32,
    /// Life, maximum life, mana, maximum mana as exact f32 bits.
    pub pools: [u32; 4],
    pub items: Vec<CachedItem>,
    /// Point rally. Versions 1 through 3 have none.
    pub rally: Option<[f32; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CachedItem {
    pub slot: usize,
    pub rawcode: [u8; 4],
    pub charges: i32,
    /// Explicit invulnerable, droppable and drop-on-death writes.
    pub flags: [Option<bool>; 3],
}

impl CachedUnit {
    pub fn validate(&self) -> Result<(), String> {
        let [life, maximum, mana, maximum_mana] = self.pools.map(f32::from_bits);
        if self.install.is_empty()
            || self.edition.is_empty()
            || [life, maximum, mana, maximum_mana]
                .iter()
                .any(|n| !n.is_finite())
            || life <= 0.
            || maximum < life
            || mana < 0.
            || maximum_mana < mana
            || self
                .hero
                .is_some_and(|[level, points]| level < 0 || points < 0)
            || self.abilities.iter().any(|(_, level)| *level < 0)
            || self
                .experience
                .is_some_and(|state| state.xp < 0 || self.hero.is_none())
            || self.abilities.windows(2).any(|a| a[0].0 >= a[1].0)
            || self.cooldowns.iter().any(|(alias, rank, bits)| {
                *rank < 1
                    || !self
                        .abilities
                        .iter()
                        .any(|(a, level)| a == alias && *level > 0)
                    || !f32::from_bits(*bits).is_finite()
                    || f32::from_bits(*bits) < 0.0
            })
            || self
                .cooldowns
                .windows(2)
                .any(|a| (a[0].0, a[0].1) >= (a[1].0, a[1].1))
            || self.items.iter().any(|item| item.charges < 0)
            || self.items.windows(2).any(|a| a[0].slot >= a[1].slot)
            || self
                .rally
                .is_some_and(|point| !point[0].is_finite() || !point[1].is_finite())
        {
            return Err("invalid cached unit identity, pools, progress or inventory".into());
        }
        Ok(())
    }

    pub fn encode(&self, limit: usize) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out = Writer {
            bytes: Vec::new(),
            limit,
        };
        out.bytes(b"PC3UNIT\x04")?;
        out.bytes(&self.rawcode)?;
        out.bytes(&self.client_sha256)?;
        out.text(&self.install)?;
        out.text(&self.edition)?;
        out.bytes(&[u8::from(self.hero.is_some())])?;
        if let Some(hero) = self.hero {
            for number in hero {
                out.bytes(&number.to_le_bytes())?;
            }
        }
        out.bytes(&self.permanent_strength.to_le_bytes())?;
        for bits in self.pools {
            out.bytes(&bits.to_le_bytes())?;
        }
        out.number(self.abilities.len())?;
        for (alias, level) in &self.abilities {
            out.bytes(alias)?;
            out.bytes(&level.to_le_bytes())?;
        }
        out.number(self.items.len())?;
        for item in &self.items {
            out.number(item.slot)?;
            out.bytes(&item.rawcode)?;
            out.bytes(&item.charges.to_le_bytes())?;
            for flag in item.flags {
                out.bytes(&[flag.map_or(0, |v| if v { 2 } else { 1 })])?;
            }
        }
        out.number(self.cooldowns.len())?;
        for (alias, rank, bits) in &self.cooldowns {
            out.bytes(alias)?;
            out.bytes(&rank.to_le_bytes())?;
            out.bytes(&bits.to_le_bytes())?;
        }
        out.bytes(&[u8::from(self.experience.is_some())])?;
        if let Some(state) = self.experience {
            out.bytes(&state.xp.to_le_bytes())?;
            out.bytes(&[u8::from(state.suspended)])?;
        }
        out.bytes(&[u8::from(self.rally.is_some())])?;
        if let Some(point) = self.rally {
            for axis in point {
                out.bytes(&axis.to_bits().to_le_bytes())?;
            }
        }
        Ok(out.bytes)
    }

    pub fn decode(bytes: &[u8], limit: usize) -> Result<Self, String> {
        if bytes.len() > limit {
            return Err("cached unit byte policy exceeded".into());
        }
        let mut input = Reader { bytes, cursor: 0 };
        let version = match input.take(8)? {
            b"PC3UNIT\x01" => 1,
            b"PC3UNIT\x02" => 2,
            b"PC3UNIT\x03" => 3,
            b"PC3UNIT\x04" => 4,
            _ => return Err("unsupported cached unit format/version".into()),
        };
        let rawcode = input.take(4)?.try_into().expect("four bytes");
        let client_sha256 = input.take(32)?.try_into().expect("SHA-256 bytes");
        let install = input.text(limit)?;
        let edition = input.text(limit)?;
        let hero = match input.take(1)?[0] {
            0 => None,
            1 => Some([integer(&mut input)?, integer(&mut input)?]),
            _ => return Err("invalid cached hero tag".into()),
        };
        let permanent_strength = integer(&mut input)?;
        let mut pools = [0; 4];
        for bits in &mut pools {
            *bits = u32::from_le_bytes(input.take(4)?.try_into().expect("four bytes"));
        }
        let count = input.number()?;
        if count > (bytes.len() - input.cursor) / 8 {
            return Err("truncated cached abilities".into());
        }
        let mut abilities = Vec::new();
        abilities
            .try_reserve_exact(count)
            .map_err(|_| "cached ability allocation failed")?;
        for _ in 0..count {
            abilities.push((
                input.take(4)?.try_into().expect("four bytes"),
                integer(&mut input)?,
            ));
        }
        let count = input.number()?;
        if count > (bytes.len() - input.cursor) / 15 {
            return Err("truncated cached inventory".into());
        }
        let mut items = Vec::new();
        items
            .try_reserve_exact(count)
            .map_err(|_| "cached inventory allocation failed")?;
        for _ in 0..count {
            let slot = input.number()?;
            let rawcode = input.take(4)?.try_into().expect("four bytes");
            let charges = integer(&mut input)?;
            let mut flags = [None; 3];
            for flag in &mut flags {
                *flag = match input.take(1)?[0] {
                    0 => None,
                    1 => Some(false),
                    2 => Some(true),
                    _ => return Err("invalid cached item flag".into()),
                };
            }
            items.push(CachedItem {
                slot,
                rawcode,
                charges,
                flags,
            });
        }
        let mut cooldowns = Vec::new();
        if version >= 2 {
            let count = input.number()?;
            if count > (bytes.len() - input.cursor) / 12 {
                return Err("truncated cached cooldowns".into());
            }
            cooldowns
                .try_reserve_exact(count)
                .map_err(|_| "cached cooldown allocation failed")?;
            for _ in 0..count {
                cooldowns.push((
                    input.take(4)?.try_into().expect("four bytes"),
                    integer(&mut input)?,
                    u32::from_le_bytes(input.take(4)?.try_into().expect("four bytes")),
                ));
            }
        }
        let experience = if version >= 3 {
            match input.take(1)?[0] {
                0 => None,
                1 => {
                    let xp = integer(&mut input)?;
                    let suspended = match input.take(1)?[0] {
                        0 => false,
                        1 => true,
                        _ => return Err("invalid cached XP suspension".into()),
                    };
                    Some(crate::hero_experience::State { xp, suspended })
                }
                _ => return Err("invalid cached XP tag".into()),
            }
        } else {
            None
        };
        let rally = if version >= 4 {
            match input.take(1)?[0] {
                0 => None,
                1 => {
                    let mut point = [0f32; 2];
                    for axis in &mut point {
                        *axis = f32::from_bits(u32::from_le_bytes(
                            input.take(4)?.try_into().expect("four bytes"),
                        ));
                    }
                    if point.iter().any(|axis| !axis.is_finite()) {
                        return Err("cached rally point is not finite".into());
                    }
                    Some(point)
                }
                _ => return Err("invalid cached rally tag".into()),
            }
        } else {
            None
        };
        if input.cursor != bytes.len() {
            return Err("trailing cached unit bytes".into());
        }
        let result = Self {
            rawcode,
            client_sha256,
            install,
            edition,
            hero,
            experience,
            abilities,
            cooldowns,
            permanent_strength,
            pools,
            items,
            rally,
        };
        result.validate()?;
        Ok(result)
    }
}

fn integer(input: &mut Reader<'_>) -> Result<i32, String> {
    Ok(i32::from_le_bytes(
        input.take(4)?.try_into().expect("four bytes"),
    ))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    pub(crate) fn fixture() -> CachedUnit {
        CachedUnit {
            rawcode: *b"H001",
            client_sha256: [0; 32],
            install: "synthetic".into(),
            edition: "fixture".into(),
            hero: Some([3, 1]),
            experience: None,
            abilities: vec![(*b"A001", 2)],
            cooldowns: vec![(*b"A001", 2, 1.25f32.to_bits())],
            permanent_strength: 7,
            pools: [123.5f32, 750., 0., 50.].map(f32::to_bits),
            rally: None,
            items: vec![CachedItem {
                slot: 2,
                rawcode: *b"I001",
                charges: 4,
                flags: [None, Some(false), Some(true)],
            }],
        }
    }
    #[test]
    fn old_unit_payloads_still_decode_and_cooldown_records_are_validated() {
        let mut saved = fixture();
        saved.cooldowns.clear();
        let mut old = saved.encode(1024).unwrap();
        old[7] = 1;
        // Version one ended before cooldowns, the XP tag, and the rally tag.
        old.truncate(old.len() - 6);
        assert_eq!(CachedUnit::decode(&old, 1024).unwrap(), saved);
        for cooldowns in [
            vec![(*b"A001", 0, 1f32.to_bits())],
            vec![(*b"A001", 1, f32::NAN.to_bits())],
            vec![(*b"B001", 1, 1f32.to_bits())],
            vec![(*b"A001", 1, 1f32.to_bits()); 2],
        ] {
            saved.cooldowns = cooldowns;
            assert!(saved.encode(1024).is_err());
        }
    }

    #[test]
    fn unit_snapshot_roundtrip_is_bounded_and_rejects_truncation_and_invalid_progress() {
        let saved = fixture();
        let bytes = saved.encode(1024).unwrap();
        assert_eq!(CachedUnit::decode(&bytes, 1024).unwrap(), saved);
        for n in 0..bytes.len() {
            assert!(CachedUnit::decode(&bytes[..n], 1024).is_err(), "prefix {n}");
        }
        assert!(saved.encode(bytes.len() - 1).is_err());
        assert!(CachedUnit::decode(&bytes, bytes.len() - 1).is_err());
        let mut extra = bytes.clone();
        extra.push(0);
        assert!(CachedUnit::decode(&extra, 1024).is_err());
        let mut bad = saved.clone();
        bad.pools[0] = f32::NAN.to_bits();
        assert!(bad.encode(1024).is_err());
        bad = saved.clone();
        bad.abilities.push(bad.abilities[0]);
        assert!(bad.encode(1024).is_err());
        bad = saved.clone();
        bad.items.push(bad.items[0].clone());
        assert!(bad.encode(1024).is_err());
        bad = saved;
        bad.items[0].charges = -1;
        assert!(bad.encode(1024).is_err());
    }
    #[test]
    fn xp_and_suspension_roundtrip_and_legacy_versions_keep_absence() {
        let mut saved = fixture();
        saved.experience = Some(crate::hero_experience::State {
            xp: 73,
            suspended: true,
        });
        let bytes = saved.encode(1024).unwrap();
        assert_eq!(CachedUnit::decode(&bytes, 1024).unwrap(), saved);
        let mut old = fixture().encode(1024).unwrap();
        old[7] = 2;
        old.truncate(old.len() - 2);
        let decoded = CachedUnit::decode(&old, 1024).unwrap();
        assert_eq!(decoded.experience, None);
        assert_eq!(decoded, fixture());
        saved.experience.as_mut().unwrap().xp = -1;
        assert!(saved.encode(1024).is_err());
    }

    #[test]
    fn rally_point_roundtrip_and_older_versions_keep_absence() {
        let mut saved = fixture();
        let absent = saved.encode(1024).unwrap();
        assert_eq!(CachedUnit::decode(&absent, 1024).unwrap().rally, None);
        saved.rally = Some([48., 16.]);
        let bytes = saved.encode(1024).unwrap();
        assert_eq!(CachedUnit::decode(&bytes, 1024).unwrap(), saved);
        for n in 0..bytes.len() {
            assert!(CachedUnit::decode(&bytes[..n], 1024).is_err(), "prefix {n}");
        }
        let mut legacy = bytes.clone();
        legacy[7] = 3;
        legacy.truncate(legacy.len() - 9);
        assert_eq!(CachedUnit::decode(&legacy, 1024).unwrap().rally, None);
        saved.rally = Some([f32::NAN, 16.]);
        assert!(saved.encode(1024).is_err());
        let mut tag = absent.clone();
        *tag.last_mut().unwrap() = 3;
        assert!(CachedUnit::decode(&tag, 1024).is_err());
        let mut truncated = absent;
        *truncated.last_mut().unwrap() = 1;
        assert!(CachedUnit::decode(&truncated, 1024).is_err());
        let mut nonfinite = bytes;
        let bits = f32::NAN.to_bits().to_le_bytes();
        let end = nonfinite.len() - 4;
        let start = end - 4;
        nonfinite[start..end].copy_from_slice(&bits);
        assert!(CachedUnit::decode(&nonfinite, 1024).is_err());
    }
}
