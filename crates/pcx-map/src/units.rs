//! Placed units/items, using native 0x551cc0/0x5520d0 framing.
use crate::{Error, Map};
pub const UNITS_NAME: &str = "war3mapUnits.doo";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UnitPlacementError {
    #[error("unit placement data at {at} needs {need} bytes")]
    Truncated { at: usize, need: usize },
    #[error("bad unit placement magic {0:?}")]
    BadMagic([u8; 4]),
    #[error("unsupported unit placement version {version}, subversion {subversion}")]
    UnsupportedVersion { version: u32, subversion: u32 },
    #[error("unsupported unit random-selection kind {0}")]
    RandomKind(u32),
    #[error("unit records end at {end}, but file has {length} bytes")]
    TrailingBytes { end: usize, length: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RandomUnit {
    None,
    /// Native kind zero's packed word, without guessed bit-field meanings.
    Packed(u32),
    Table([u32; 2]),
    Choices(Vec<[u32; 2]>),
    /// Native reads no payload for other kinds; their meaning is unverified.
    Unclassified(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitPlacement {
    pub type_id: [u8; 4],
    pub variation: u32,
    pub position: [f32; 3],
    pub angle: f32,
    pub scale: [f32; 3],
    pub flags: u8,
    /// Native two-byte control field followed by an unclassified word.
    pub control_word: u16,
    pub state_word: u32,
    /// Authored signed words; native runtime conversion is not applied here.
    pub life: i32,
    pub mana: i32,
    pub item_table: Option<i32>,
    pub item_sets: Vec<Vec<[u32; 2]>>,
    pub post_drop_word: u32,
    pub post_drop_float: f32,
    pub progression_word: u32,
    pub progression_words: Option<[u32; 3]>,
    /// Native inventory word pairs, retained in file order.
    pub inventory: Vec<[u32; 2]>,
    /// Native ability word triples, retained in file order.
    pub abilities: Vec<[u32; 3]>,
    pub random: RandomUnit,
    /// The two authored words following random-selection data.
    pub tail_words: [u32; 2],
    pub creation_number: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Units {
    pub version: u32,
    pub subversion: u32,
    pub instances: Vec<UnitPlacement>,
}

impl Units {
    pub fn parse(data: &[u8]) -> Result<Self, UnitPlacementError> {
        let mut c = Cursor { data, at: 0 };
        let magic = c.bytes()?;
        if &magic != b"W3do" {
            return Err(UnitPlacementError::BadMagic(magic));
        }
        let version = c.word()?;
        let subversion = c.word()?;
        if !matches!(version, 7 | 8) || !matches!(subversion, 9 | 11) {
            return Err(UnitPlacementError::UnsupportedVersion {
                version,
                subversion,
            });
        }
        let count = c.word()?;
        let mut instances = Vec::new();
        for _ in 0..count {
            let type_id = c.bytes()?;
            let variation = c.word()?;
            let position = [c.float()?, c.float()?, c.float()?];
            let angle = c.float()?;
            let scale = [c.float()?, c.float()?, c.float()?];
            let flags = c.bytes::<1>()?[0];
            let control_word = u16::from_le_bytes(c.bytes()?);
            let state_word = c.word()?;
            let life = c.word()? as i32;
            let mana = c.word()? as i32;
            let item_table = if subversion > 10 {
                Some(c.word()? as i32)
            } else {
                None
            };
            let set_count = c.word()?;
            let mut item_sets = Vec::new();
            for _ in 0..set_count {
                item_sets.push(c.pairs()?);
            }
            let post_drop_word = c.word()?;
            let post_drop_float = c.float()?;
            let progression_word = c.word()?;
            let progression_words = if subversion > 9 {
                Some([c.word()?, c.word()?, c.word()?])
            } else {
                None
            };
            let inventory = c.pairs()?;
            let ability_count = c.word()?;
            let mut abilities = Vec::new();
            for _ in 0..ability_count {
                abilities.push([c.word()?, c.word()?, c.word()?]);
            }
            let random = match c.word()? {
                u32::MAX => RandomUnit::None,
                0 => RandomUnit::Packed(c.word()?),
                1 => RandomUnit::Table([c.word()?, c.word()?]),
                2 => RandomUnit::Choices(c.pairs()?),
                kind => RandomUnit::Unclassified(kind),
            };
            let tail_words = [c.word()?, c.word()?];
            let creation_number = c.word()?;
            instances.push(UnitPlacement {
                type_id,
                variation,
                position,
                angle,
                scale,
                flags,
                control_word,
                state_word,
                life,
                mana,
                item_table,
                item_sets,
                post_drop_word,
                post_drop_float,
                progression_word,
                progression_words,
                inventory,
                abilities,
                random,
                tail_words,
                creation_number,
            });
        }
        if c.at != data.len() {
            return Err(UnitPlacementError::TrailingBytes {
                end: c.at,
                length: data.len(),
            });
        }
        Ok(Self {
            version,
            subversion,
            instances,
        })
    }

    /// Reconstruct parsed authored fields for exact local fixture comparisons.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = b"W3do".to_vec();
        for word in [self.version, self.subversion, self.instances.len() as u32] {
            out.extend(word.to_le_bytes());
        }
        for unit in &self.instances {
            out.extend(unit.type_id);
            out.extend(unit.variation.to_le_bytes());
            for value in unit
                .position
                .into_iter()
                .chain([unit.angle])
                .chain(unit.scale)
            {
                out.extend(value.to_bits().to_le_bytes());
            }
            out.push(unit.flags);
            out.extend(unit.control_word.to_le_bytes());
            for word in [unit.state_word, unit.life as u32, unit.mana as u32] {
                out.extend(word.to_le_bytes());
            }
            if let Some(table) = unit.item_table {
                out.extend(table.to_le_bytes());
            }
            out.extend((unit.item_sets.len() as u32).to_le_bytes());
            for set in &unit.item_sets {
                write_pairs(&mut out, set);
            }
            out.extend(unit.post_drop_word.to_le_bytes());
            out.extend(unit.post_drop_float.to_bits().to_le_bytes());
            out.extend(unit.progression_word.to_le_bytes());
            if let Some(stats) = unit.progression_words {
                for word in stats {
                    out.extend(word.to_le_bytes());
                }
            }
            write_pairs(&mut out, &unit.inventory);
            out.extend((unit.abilities.len() as u32).to_le_bytes());
            for ability in &unit.abilities {
                for word in ability {
                    out.extend(word.to_le_bytes());
                }
            }
            match &unit.random {
                RandomUnit::None => out.extend(u32::MAX.to_le_bytes()),
                RandomUnit::Unclassified(kind) => out.extend(kind.to_le_bytes()),
                RandomUnit::Packed(word) => {
                    out.extend(0u32.to_le_bytes());
                    out.extend(word.to_le_bytes());
                }
                RandomUnit::Table(words) => {
                    out.extend(1u32.to_le_bytes());
                    for word in words {
                        out.extend(word.to_le_bytes());
                    }
                }
                RandomUnit::Choices(pairs) => {
                    out.extend(2u32.to_le_bytes());
                    write_pairs(&mut out, pairs);
                }
            }
            for word in unit.tail_words.into_iter().chain([unit.creation_number]) {
                out.extend(word.to_le_bytes());
            }
        }
        out
    }
}

fn write_pairs(out: &mut Vec<u8>, pairs: &[[u32; 2]]) {
    out.extend((pairs.len() as u32).to_le_bytes());
    for pair in pairs {
        for word in pair {
            out.extend(word.to_le_bytes());
        }
    }
}
struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}
impl Cursor<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], UnitPlacementError> {
        let end = self
            .at
            .checked_add(N)
            .ok_or(UnitPlacementError::Truncated {
                at: self.at,
                need: N,
            })?;
        let value = self
            .data
            .get(self.at..end)
            .ok_or(UnitPlacementError::Truncated {
                at: self.at,
                need: N,
            })?
            .try_into()
            .unwrap();
        self.at = end;
        Ok(value)
    }
    fn word(&mut self) -> Result<u32, UnitPlacementError> {
        self.bytes().map(u32::from_le_bytes)
    }
    fn float(&mut self) -> Result<f32, UnitPlacementError> {
        self.word().map(f32::from_bits)
    }
    fn pairs(&mut self) -> Result<Vec<[u32; 2]>, UnitPlacementError> {
        let count = self.word()?;
        let mut pairs = Vec::new();
        for _ in 0..count {
            pairs.push([self.word()?, self.word()?]);
        }
        Ok(pairs)
    }
}
impl Map<'_> {
    pub fn units(&self) -> Result<Units, Error> {
        let data = self.extract(UNITS_NAME)?;
        Ok(Units::parse(&data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(random_kind: u32, random_body: &[u32]) -> Vec<u8> {
        let mut data = b"W3do".to_vec();
        for word in [8u32, 11, 1] {
            data.extend(word.to_le_bytes());
        }
        data.extend(b"UNIT");
        data.extend(2u32.to_le_bytes());
        for value in [1f32, 2., 3., 0.75, 1., 2., 3.] {
            data.extend(value.to_bits().to_le_bytes());
        }
        data.push(5);
        data.extend(24u16.to_le_bytes());
        for word in [
            0x1122u32,
            (-10i32) as u32,
            (-20i32) as u32,
            u32::MAX,
            1,
            1,
            33,
            0x41424344,
            100,
        ] {
            data.extend(word.to_le_bytes());
        }
        data.extend(123.5f32.to_bits().to_le_bytes());
        for word in [
            1u32,
            2,
            3,
            4,
            1,
            5,
            0x11223344,
            1,
            0x41424344,
            1,
            6,
            random_kind,
        ] {
            data.extend(word.to_le_bytes());
        }
        for word in random_body {
            data.extend(word.to_le_bytes());
        }
        for word in [u32::MAX, 17, 7] {
            data.extend(word.to_le_bytes());
        }
        data
    }
    #[test]
    fn variable_lists_and_random_variants_keep_record_boundaries() {
        for (kind, words, expected) in [
            (u32::MAX, vec![], RandomUnit::None),
            (0, vec![0x01020304], RandomUnit::Packed(0x01020304)),
            (1, vec![4, 8], RandomUnit::Table([4, 8])),
            (
                2,
                vec![1, 25, 0x41424344],
                RandomUnit::Choices(vec![[25, 0x41424344]]),
            ),
        ] {
            let data = fixture(kind, &words);
            let parsed = Units::parse(&data).unwrap();
            let unit = &parsed.instances[0];
            assert_eq!(unit.position, [1., 2., 3.]);
            assert_eq!(unit.control_word, 24);
            assert_eq!(unit.life, -10);
            assert_eq!(unit.item_sets, [vec![[33, 0x41424344]]]);
            assert_eq!(unit.inventory, [[5, 0x11223344]]);
            assert_eq!(unit.abilities, [[0x41424344, 1, 6]]);
            assert_eq!(unit.random, expected);
            assert_eq!(unit.creation_number, 7);
            assert_eq!(parsed.canonical_bytes(), data);
        }
    }
    #[test]
    fn unknown_random_layout_and_truncated_lists_are_errors() {
        assert_eq!(
            Units::parse(&fixture(3, &[])).unwrap().instances[0].random,
            RandomUnit::Unclassified(3)
        );
        let data = fixture(2, &[1, 25, 0x41424344]);
        for length in 0..data.len() {
            assert!(Units::parse(&data[..length]).is_err());
        }
    }
}
