//! `war3map.w3s` — map-authored sound records.
//!
//! This is the *map* sound table (the records a map's World Editor
//! dialogues create under `gg_snd_*` names). It is a different container
//! from the native Warcraft III savegame format, which also uses the `.w3s`
//! extension and is not parsed here.
use thiserror::Error;

/// File name inside the embedded map archive.
pub const SOUNDS_NAME: &str = "war3map.w3s";

/// The only map-sound version measured in shipped maps.
pub const SOUNDS_VERSION: u32 = 1;

/// Number of 32-bit words following the three authored strings in each record.
/// Measured as a fixed 17-word tail across every shipped record.
pub const SOUND_WORD_COUNT: usize = 17;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SoundError {
    #[error("w3s needs {need} bytes, has {have}")]
    Truncated { have: usize, need: usize },
    #[error("unsupported w3s version {version}")]
    UnsupportedVersion { version: u32 },
    #[error("w3s string at {at} has no NUL terminator")]
    UnterminatedString { at: usize },
    #[error("w3s declares {count} records, but at most {max} fit in {length} bytes")]
    TooManyRecords {
        count: u32,
        max: usize,
        length: usize,
    },
    #[error("w3s records consume {end} bytes of {length}")]
    TrailingBytes { end: usize, length: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundRecord {
    /// The map-visible JASS variable name (raw bytes, without the NUL).
    pub name: Vec<u8>,
    /// The authored asset path (raw bytes, without the NUL).
    pub file: Vec<u8>,
    /// The authored EAX environment name (raw bytes, without the NUL).
    pub environment: Vec<u8>,
    /// The 17 authored 32-bit words following the strings, in file order.
    pub words: [u32; SOUND_WORD_COUNT],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sounds {
    pub version: u32,
    pub records: Vec<SoundRecord>,
}

impl SoundRecord {
    /// The first authored word; shipped files use values such as 0 or 4 here.
    pub fn flags(&self) -> u32 {
        self.words[0]
    }
}

impl Sounds {
    pub fn parse(data: &[u8]) -> Result<Self, SoundError> {
        if data.len() < 8 {
            return Err(SoundError::Truncated {
                have: data.len(),
                need: 8,
            });
        }
        let version = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let count = u32::from_le_bytes(data[4..8].try_into().unwrap());

        if version != SOUNDS_VERSION {
            return Err(SoundError::UnsupportedVersion { version });
        }

        // At least three one-byte strings plus their terminators and the
        // fixed word tail per record.
        let per_record_min = 3 + SOUND_WORD_COUNT * 4;
        let max_records = data.len() / per_record_min;
        if count as usize > max_records {
            return Err(SoundError::TooManyRecords {
                count,
                max: max_records,
                length: data.len(),
            });
        }

        let mut cursor = 8usize;
        let mut records = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let name = read_string(data, &mut cursor)?;
            let file = read_string(data, &mut cursor)?;
            let environment = read_string(data, &mut cursor)?;

            if cursor + SOUND_WORD_COUNT * 4 > data.len() {
                return Err(SoundError::Truncated {
                    have: data.len() - cursor,
                    need: SOUND_WORD_COUNT * 4,
                });
            }
            let mut words = [0u32; SOUND_WORD_COUNT];
            for slot in words.iter_mut() {
                *slot = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
                cursor += 4;
            }

            records.push(SoundRecord {
                name,
                file,
                environment,
                words,
            });
        }

        if cursor != data.len() {
            return Err(SoundError::TrailingBytes {
                end: cursor,
                length: data.len(),
            });
        }

        Ok(Self { version, records })
    }
}

fn read_string(data: &[u8], cursor: &mut usize) -> Result<Vec<u8>, SoundError> {
    if *cursor > data.len() {
        return Err(SoundError::Truncated {
            have: data.len(),
            need: *cursor + 1,
        });
    }
    let tail = &data[*cursor..];
    let nul = tail
        .iter()
        .position(|&b| b == 0)
        .ok_or(SoundError::UnterminatedString { at: *cursor })?;
    let out = tail[..nul].to_vec();
    *cursor += nul + 1;
    Ok(out)
}

impl SoundRecord {
    /// Access a raw authored word by index.
    pub fn word(&self, index: usize) -> Option<u32> {
        self.words.get(index).copied()
    }
}
