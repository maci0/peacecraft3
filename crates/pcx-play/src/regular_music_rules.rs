//! Source-bound ordinary music transitions and independent audio random policy.
use crate::{
    camera_margins::SourceWindow,
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, sha256},
    pe_image::PeSections,
    script_random::profile::{Binding, Policy},
    scripts::AssetIdentity,
};
use std::sync::Arc;

type Read<'a> = dyn Fn(u32, usize) -> Result<Vec<u8>, String> + 'a;

#[derive(Clone, Debug)]
pub struct Rules {
    pub image_sha256: [u8; 32],
    pub origin: AssetIdentity,
    pub(super) random: Arc<Policy>,
    pub(super) initial_level: u32,
    pub(super) initial_enabled: bool,
    pub(super) random_index: i32,
    pub(super) maximum_level: u32,
    pub(super) name_bytes: usize,
    pub(super) default_fade_ms: u32,
    pub(super) fade_interval_ms: u32,
    pub(super) next_interval_ms: u32,
    pub(super) repeat_attempts: usize,
    pub(super) pitch_mask: u32,
    pub(super) pitch_base: u32,
    pub(super) pitch_one: f32,
    fold: [u8; 3],
    edition_format: [String; 3],
    delimiter: String,
}

fn bytes(read: &Read<'_>, at: u32, count: usize) -> Result<Vec<u8>, String> {
    let result = read(at, count)?;
    if result.len() != count {
        return Err(format!("regular music source truncated at {at:#x}"));
    }
    Ok(result)
}
fn word(read: &Read<'_>, at: u32) -> Result<u32, String> {
    Ok(u32::from_le_bytes(bytes(read, at, 4)?.try_into().unwrap()))
}
fn operand(read: &Read<'_>, at: u32, prefix: &[u8], size: usize) -> Result<Vec<u8>, String> {
    let b = bytes(read, at, prefix.len() + size)?;
    if !b.starts_with(prefix) {
        return Err(format!(
            "regular music source instruction differs at {at:#x}"
        ));
    }
    Ok(b[prefix.len()..].to_vec())
}
fn immediate(read: &Read<'_>, at: u32, prefix: &[u8]) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        operand(read, at, prefix, 4)?.try_into().unwrap(),
    ))
}
fn direct_call(read: &Read<'_>, at: u32, target: u32) -> Result<(), String> {
    let relative = i32::from_le_bytes(operand(read, at, &[0xe8], 4)?.try_into().unwrap());
    if i64::from(at) + 5 + i64::from(relative) != i64::from(target) {
        return Err(format!("regular music source call differs at {at:#x}"));
    }
    Ok(())
}
fn string(read: &Read<'_>, at: u32, limit: usize) -> Result<String, String> {
    let mut out = Vec::new();
    for offset in 0..limit {
        let address = at
            .checked_add(u32::try_from(offset).map_err(|_| "music string offset")?)
            .ok_or("music source string address overflow")?;
        let b = bytes(read, address, 1)?[0];
        if b == 0 {
            return String::from_utf8(out).map_err(|_| "music source format is not UTF-8".into());
        }
        out.push(b);
    }
    Err("unterminated regular music source format".into())
}

impl Rules {
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        if image.len() > 32 << 20 || sha256(image) != REVIEWED_IMAGE {
            return Err("unsupported regular music client image".into());
        }
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err("regular music source identity is incomplete".into());
        }
        let pe = PeSections::parse(image)?;
        let read = |at, len| {
            pe.va_bytes(at, len)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| format!("regular music image window {at:#x}"))
        };
        let window = |va, len| -> Result<SourceWindow, String> {
            Ok(SourceWindow {
                va,
                len,
                digest: sha256(&bytes(&read, va, len as usize)?),
            })
        };
        let random = Policy::read(
            image,
            Binding {
                origin: origin.clone(),
                image_sha256: REVIEWED_IMAGE,
                seed: window(0x9892a0, 119)?,
                draw: window(0x480dd0, 169)?,
                integer: window(0x494e40, 60)?,
                real: window(0x494e80, 177)?,
                set_seed: window(0x4a99a0, 39)?,
                secondary: window(0x69a0d0, 251)?,
                real_constant_init: window(0x4017c0, 16)?,
                math_sub: window(0x9a00b0, 264)?,
                math_add: window(0x9a01c0, 262)?,
                math_mul: crate::campaign_client::numeric_multiplication(image)?,
                math_lzcnt: window(0x9a0330, 29)?,
                math_i32_to_real: window(0x9a1620, 92)?,
            },
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| e.to_string())?;
        Self::decode(&read, origin, Arc::new(random))
    }

    fn decode(read: &Read<'_>, origin: AssetIdentity, random: Arc<Policy>) -> Result<Self, String> {
        // The seed, music and sound consumers must use the same independent stream.
        let stream = immediate(read, 0x5d7013, &[0xb9])?;
        if stream != immediate(read, 0x5d8bee, &[0x68])?
            || stream != immediate(read, 0x5cf3f0, &[0x68])?
            || stream != immediate(read, 0x5c3228, &[0x68])?
            || stream != immediate(read, 0x698e58, &[0x68])?
        {
            return Err("regular music audio random stream relationship differs".into());
        }
        for (site, target) in [
            (0x5d700d, 0x796c90),
            (0x5d7018, 0x9892a0),
            (0x5d8bf3, 0x480dd0),
            (0x5cf3f5, 0x480dd0),
            (0x5c322d, 0x480dd0),
            (0x698e5d, 0x480dd0),
            (0x5ce423, 0x796c90),
            (0x5ce477, 0x796c90),
            (0x8460d3, 0x796c90),
            (0x845d14, 0x796c90),
            (0x845d9f, 0x845500),
            (0x5ce446, 0x840370),
            (0x5db5c8, 0x840370),
        ] {
            direct_call(read, site, target)?;
        }
        if bytes(read, 0x5d8bfb, 2)? != [0xf7, 0xe6]
            || bytes(read, 0x845d75, 13)?
                != [
                    0x8b, 0x46, 0x10, 0x2b, 0xc2, 0x85, 0xc0, 0x0f, 0x8f, 0xd0, 0, 0, 0,
                ]
            || bytes(read, 0x5ce488, 4)? != [0x3b, 0xc1, 0x76, 0x33]
        {
            return Err("regular music draw or wall callback comparison differs".into());
        }
        if immediate(read, 0x5d8b2d, &[0xff, 0x35])? != immediate(read, 0x710f00, &[0xa1])? {
            return Err("regular music version selector relationship differs".into());
        }
        direct_call(read, 0x4334c4, 0xa77598)?;
        let fold = [
            operand(read, 0xa99c23, &[0xb7], 1)?[0],
            operand(read, 0xa99c25, &[0xb3], 1)?[0],
            operand(read, 0xa99c27, &[0xb6], 1)?[0],
        ];
        if fold[0] > fold[1] || fold[1].checked_add(fold[2]).is_none() {
            return Err("unsupported music filename fold".into());
        }
        let name_bytes = immediate(read, 0x5d8b3f, &[0x68])?;
        if name_bytes == 0 || name_bytes > 1 << 20 {
            return Err("regular music source filename bound is unsupported".into());
        }
        let format = string(
            read,
            immediate(read, 0x5d8b3a, &[0x68])?,
            name_bytes as usize,
        )?;
        let (prefix, rest) = format.split_once("%s").ok_or("music edition name format")?;
        let (middle, suffix) = rest
            .split_once("%d")
            .ok_or("music edition version format")?;
        if [prefix, middle, suffix]
            .iter()
            .any(|part| part.contains('%'))
        {
            return Err("unsupported music edition format conversion".into());
        }
        let edition_format = [prefix.into(), middle.into(), suffix.into()];
        let delimiter = string(
            read,
            immediate(read, 0x5d8b86, &[0x68])?,
            name_bytes as usize,
        )?;
        if delimiter.is_empty() || !delimiter.is_ascii() || delimiter.contains('"') {
            return Err("music source delimiters are unsupported".into());
        }
        let fade_global = immediate(read, 0x5d6d30, &[0xc7, 0x05])?;
        if fade_global != immediate(read, 0x5ce482, &[0x8b, 0x0d])? {
            return Err("regular music fade duration relationship differs".into());
        }
        let default_fade_ms = word(read, 0x5d6d36)?;
        // Scheduling converts caller seconds with this source multiplier.
        let scale_at = immediate(read, 0x8460df, &[0xf3, 0x0f, 0x59, 0x05])?;
        let scale = f32::from_bits(word(read, scale_at)?);
        let period = f32::from_bits(immediate(read, 0x5d6d44, &[0xc7, 0x04, 0x24])?);
        let next = f32::from_bits(immediate(read, 0x5db5c1, &[0xc7, 0x04, 0x24])?);
        let interval = |seconds: f32| -> Result<u32, String> {
            let millis = seconds * scale;
            if !millis.is_finite() || millis < 1.0 || millis >= i32::MAX as f32 {
                return Err("unsupported regular music callback interval".into());
            }
            Ok(millis.trunc() as u32)
        };
        if default_fade_ms == 0 || default_fade_ms > i32::MAX as u32 {
            return Err("unsupported regular music default fade duration".into());
        }
        let retry = usize::from(operand(read, 0x5cf407, &[0x83, 0xf8], 1)?[0]);
        let pitch_mask = immediate(read, 0x5c3237, &[0x81, 0xe1])?;
        let pitch_base = immediate(read, 0x5c323d, &[0x81, 0xc9])?;
        let pitch_one = f32::from_bits(word(
            read,
            immediate(read, 0x5c326f, &[0xf3, 0x0f, 0x5c, 0x05])?,
        )?);
        if !pitch_one.is_finite() || pitch_mask & pitch_base != 0 {
            return Err("unsupported regular music shared pitch packing".into());
        }
        let random_index = i32::from(operand(read, 0x4a41b1, &[0x6a], 1)?[0] as i8);
        if random_index >= 0 {
            return Err("music random selector is not negative".into());
        }
        let enabled = word(read, 0x10dc6dc)?;
        if enabled > 1 {
            return Err("unsupported music-enabled initializer".into());
        }
        let initial_enabled = enabled != 0;
        let level_global = immediate(read, 0x5d8dd1, &[0xa1])?;
        let maximum_level = immediate(read, 0x5da426, &[0xb8])?;
        let initial_level = word(read, level_global)?;
        if initial_level > maximum_level {
            return Err("music initial volume exceeds source range".into());
        }
        Ok(Self {
            image_sha256: REVIEWED_IMAGE,
            origin,
            random,
            initial_level,
            initial_enabled,
            random_index,
            maximum_level,
            name_bytes: name_bytes as usize,
            default_fade_ms,
            fade_interval_ms: interval(period)?,
            next_interval_ms: interval(next)?,
            repeat_attempts: retry.checked_add(1).ok_or("music random retry bound")?,
            pitch_mask,
            pitch_base,
            pitch_one,
            fold,
            edition_format,
            delimiter,
        })
    }

    pub(super) fn same_path(&self, a: &str, b: &str) -> bool {
        let fold = |byte: u8| {
            if (self.fold[0]..=self.fold[1]).contains(&byte) {
                byte + self.fold[2]
            } else {
                byte
            }
        };
        a.len() == b.len() && a.bytes().zip(b.bytes()).all(|(a, b)| fold(a) == fold(b))
    }

    pub fn random_index(&self) -> i32 { self.random_index }

    pub fn edition_key(&self, name: &str, edition: i32) -> Result<String, String> {
        let [prefix, middle, suffix] = &self.edition_format;
        let key = format!("{prefix}{name}{middle}{edition}{suffix}");
        if key.len() >= self.name_bytes {
            return Err("regular music edition key exceeds source bound".into());
        }
        Ok(key)
    }
    pub fn tracks(&self, text: &str) -> Result<Vec<String>, String> {
        if text.len() >= self.name_bytes || text.contains('\0') {
            return Err("regular music list exceeds source filename bound".into());
        }
        let mut tracks = Vec::new();
        for part in text
            .split(|c| self.delimiter.contains(c))
            .filter(|part| !part.is_empty())
        {
            tracks.push(part.to_owned());
        }
        if tracks.is_empty() {
            return Err("regular music list has no tracks".into());
        }
        Ok(tracks)
    }
}

#[cfg(test)]
impl Rules {
    pub(super) fn fixture() -> Self {
        Self {
            image_sha256: [0x5e; 32],
            origin: AssetIdentity {
                source: "fixture".into(),
                edition: "nonstock".into(),
                version: "fixture".into(),
            },
            random: Arc::new(crate::script_random::profile::fixture()),
            initial_level: 8,
            maximum_level: 13,
            initial_enabled: true,
            random_index: -3,
            name_bytes: 128,
            default_fade_ms: 31,
            fade_interval_ms: 7,
            next_interval_ms: 19,
            repeat_attempts: 3,
            pitch_mask: 0x007f_ffff,
            pitch_base: 1.0f32.to_bits(),
            pitch_one: 1.0,
            fold: [b'A', b'Z', b'a' - b'A'],
            edition_format: ["test/".into(), "-edition-".into(), ".list".into()],
            delimiter: "|".into(),
        }
    }
}

#[cfg(test)]
#[path = "regular_music_rules_tests.rs"]
mod tests;
