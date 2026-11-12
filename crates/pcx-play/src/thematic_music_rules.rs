//! Selected-client thematic start and seek operands; no audio device is opened.
use crate::{menu_lighting::REVIEWED_IMAGE, neutral_player::sha256, pe_image::PeSections};
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    initial_position: i32,
    ignored_position: i32,
}

impl Rules {
    pub fn read(image: &[u8]) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != REVIEWED_IMAGE {
            return Err("unsupported thematic music client".into());
        }
        let pe = PeSections::parse(image)?;
        Self::decode(REVIEWED_IMAGE, |va, size| {
            pe.va_bytes(va, size)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("thematic music image window {va:#x}"))
        })
    }

    fn decode(
        image_sha256: [u8; 32],
        read: impl Fn(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let window = |va, size| {
            let bytes = read(va, size)?;
            if bytes.len() != size {
                return Err("truncated thematic music window".to_owned());
            }
            Ok(bytes)
        };
        let plain = window(0x4a4200, 24)?;
        let extended = window(0x4a4220, 25)?;
        let setter = window(0x5da9f0, 34)?;
        let relative = |bytes: &[u8], at: usize, base: u32| {
            (bytes.get(at) == Some(&0xe8)).then(|| {
                base.wrapping_add(at as u32 + 5)
                    .wrapping_add(u32::from_le_bytes(
                        bytes[at + 1..at + 5].try_into().unwrap(),
                    ))
            })
        };
        if plain[..7] != [0x55, 0x8b, 0xec, 0x8b, 0x4d, 8, 0x6a]
            || extended[..9] != [0x55, 0x8b, 0xec, 0xff, 0x75, 12, 0x8b, 0x4d, 8]
            || relative(&plain, 8, 0x4a4200).is_none()
            || relative(&plain, 8, 0x4a4200) != relative(&extended, 9, 0x4a4220)
            || relative(&plain, 14, 0x4a4200) != Some(0x5d9170)
            || relative(&extended, 15, 0x4a4220) != Some(0x5d9170)
            || setter[..4] != [0x55, 0x8b, 0xec, 0xa1]
            || setter[8..17] != [0x85, 0xc0, 0x74, 0x14, 0x8b, 0x4d, 8, 0x83, 0xf9]
            || setter[18..24] != [0x74, 12, 0x51, 0xff, 0x70, 8]
            || relative(&setter, 24, 0x5da9f0) != Some(0x7a00e0)
        {
            return Err("thematic music call chain differs".into());
        }
        let rules = Self {
            image_sha256,
            initial_position: i32::from(plain[7] as i8),
            ignored_position: i32::from(setter[17] as i8),
        };
        rules.position(rules.initial_position)?;
        Ok(rules)
    }

    pub(super) fn initial_position(&self) -> Result<Option<Duration>, String> {
        self.position(self.initial_position)
    }

    pub(super) fn position(&self, millis: i32) -> Result<Option<Duration>, String> {
        if millis == self.ignored_position {
            return Ok(None);
        }
        let millis = u64::try_from(millis)
            .map_err(|_| "negative thematic seek through Miles is unqualified")?;
        Ok(Some(Duration::from_millis(millis)))
    }
}

#[cfg(test)]
#[path = "thematic_music_rules_tests.rs"]
pub(crate) mod tests;
