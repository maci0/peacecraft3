//! W3R records from native reader/writer; no invented fields for older versions.
use crate::{Error, Map};

pub const REGIONS_NAME: &str = "war3map.w3r";

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RegionError {
    #[error("unsupported region version {0}")]
    UnsupportedVersion(u32),
    #[error("region data at {at} needs {need} bytes")]
    Truncated { at: usize, need: usize },
    #[error("region string at {at} has no NUL terminator")]
    UnterminatedString { at: usize },
    #[error("region records end at {end}, but file has {length} bytes")]
    TrailingBytes { end: usize, length: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegionBounds {
    Integer([i32; 4]),
    Float([f32; 4]),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// Four authored bounds in stored order, with their version's numeric type.
    pub bounds: RegionBounds,
    /// Authored bytes; no guessed encoding or localization replacement.
    pub name: Vec<u8>,
    pub creation_id: Option<i32>,
    pub weather_id: Option<[u8; 4]>,
    pub ambient_sound: Option<Vec<u8>>,
    /// Stored colour word; channel interpretation remains a caller concern.
    pub colour: Option<[u8; 4]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Regions {
    pub version: u32,
    pub regions: Vec<Region>,
}

impl Regions {
    pub fn parse(data: &[u8]) -> Result<Self, RegionError> {
        let mut cursor = Cursor { data, at: 0 };
        let version = cursor.u32()?;
        if version > 5 {
            return Err(RegionError::UnsupportedVersion(version));
        }
        let count = cursor.u32()?;
        let mut regions = Vec::new();
        for _ in 0..count {
            let words = [
                cursor.word()?,
                cursor.word()?,
                cursor.word()?,
                cursor.word()?,
            ];
            let bounds = if version < 2 {
                RegionBounds::Integer(words.map(i32::from_le_bytes))
            } else {
                RegionBounds::Float(words.map(f32::from_le_bytes))
            };
            let name = cursor.string()?;
            let creation_id = if version > 0 {
                Some(cursor.u32()? as i32)
            } else {
                None
            };
            let weather_id = if version > 2 {
                Some(cursor.word()?)
            } else {
                None
            };
            let ambient_sound = if version > 3 {
                Some(cursor.string()?)
            } else {
                None
            };
            let colour = if version > 4 {
                Some(cursor.word()?)
            } else {
                None
            };
            regions.push(Region {
                bounds,
                name,
                creation_id,
                weather_id,
                ambient_sound,
                colour,
            });
        }
        if cursor.at != data.len() {
            return Err(RegionError::TrailingBytes {
                end: cursor.at,
                length: data.len(),
            });
        }
        Ok(Self { version, regions })
    }
}

struct Cursor<'a> {
    data: &'a [u8],
    at: usize,
}
impl Cursor<'_> {
    fn word(&mut self) -> Result<[u8; 4], RegionError> {
        let word = self
            .data
            .get(self.at..self.at + 4)
            .ok_or(RegionError::Truncated {
                at: self.at,
                need: 4,
            })?
            .try_into()
            .unwrap();
        self.at += 4;
        Ok(word)
    }
    fn u32(&mut self) -> Result<u32, RegionError> {
        self.word().map(u32::from_le_bytes)
    }
    fn string(&mut self) -> Result<Vec<u8>, RegionError> {
        let tail = &self.data[self.at..];
        let length = tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or(RegionError::UnterminatedString { at: self.at })?;
        let bytes = tail[..length].to_vec();
        self.at += length + 1;
        Ok(bytes)
    }
}
impl Map<'_> {
    pub fn regions(&self) -> Result<Regions, Error> {
        let data = self.extract(REGIONS_NAME)?;
        Ok(Regions::parse(&data)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(version: u32) -> Vec<u8> {
        let mut data = Vec::new();
        for word in [version, 1] {
            data.extend(word.to_le_bytes());
        }
        for value in [-2i32, -1, 3, 4] {
            if version < 2 {
                data.extend(value.to_le_bytes());
            } else {
                data.extend((value as f32).to_le_bytes());
            }
        }
        data.extend(b"authored\xff\0");
        if version > 0 {
            data.extend((-7i32).to_le_bytes());
        }
        if version > 2 {
            data.extend(b"RAIN");
        }
        if version > 3 {
            data.extend(b"ambience\0");
        }
        if version > 4 {
            data.extend([1, 2, 3, 4]);
        }
        data
    }
    #[test]
    fn versioned_fields_are_present_only_when_authored() {
        for version in 0..=5 {
            let parsed = Regions::parse(&fixture(version)).unwrap();
            let region = &parsed.regions[0];
            assert_eq!(region.name, b"authored\xff");
            assert_eq!(region.creation_id, (version > 0).then_some(-7));
            assert_eq!(region.weather_id, (version > 2).then_some(*b"RAIN"));
            assert_eq!(
                region.ambient_sound,
                (version > 3).then(|| b"ambience".to_vec())
            );
            assert_eq!(region.colour, (version > 4).then_some([1, 2, 3, 4]));
        }
    }
    #[test]
    fn incomplete_and_trailing_records_are_refused() {
        let data = fixture(5);
        for length in 0..data.len() {
            assert!(Regions::parse(&data[..length]).is_err());
        }
        let mut longer = data;
        longer.push(0);
        assert!(matches!(
            Regions::parse(&longer),
            Err(RegionError::TrailingBytes { .. })
        ));
    }
}
