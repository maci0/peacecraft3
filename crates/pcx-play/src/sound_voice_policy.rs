//! Source-decoded non-spatial voice defaults and admission capacities.
use crate::{
    neutral_player::{Sha256Digest, sha256},
    scripts::{AssetIdentity, ScriptAsset},
};

#[derive(Clone, Debug)]
pub struct Channel {
    pub capacity: usize,
    pub minimum_priority: u32,
    pub maximum_priority: u32,
    pub gain: f32,
    /// Retained native channel flags; spatial routing is outside this profile.
    pub flags: u32,
}
#[derive(Clone, Debug)]
pub struct VoicePolicy {
    origin: AssetIdentity,
    receipts: Vec<(String, Sha256Digest)>,
    channels: Vec<Channel>,
    default_volume: i32,
    default_pitch: f32,
    default_priority: u32,
    default_channel: i32,
    list_capacity: usize,
    filename_capacity: usize,
}
impl VoicePolicy {
    /// Parse explicit native instruction/table windows; all numeric operands are input data.
    pub fn read(
        definition: ScriptAsset<'_>,
        channels: ScriptAsset<'_>,
        list: ScriptAsset<'_>,
        filename: ScriptAsset<'_>,
        origin: AssetIdentity,
    ) -> Result<Self, String> {
        let fail = |label: &str, reason: &str| {
            format!("sound voice policy {} / {label}: {reason}", origin.source)
        };
        if [&origin.source, &origin.edition, &origin.version]
            .iter()
            .any(|s| s.trim().is_empty())
            || [definition, channels, list, filename]
                .iter()
                .any(|s| s.label.is_empty() || s.bytes.len() > 64 << 10)
        {
            return Err(fail("inputs", "identity/window budget"));
        }
        let mut values = Vec::new();
        if definition.bytes.len() != 42 {
            return Err(fail(
                definition.label,
                "truncated/extra default instructions",
            ));
        }
        for (code, field) in definition
            .bytes
            .as_chunks::<7>()
            .0
            .iter()
            .zip([0x2c, 0x30, 0x34, 0x38, 0x3c, 0x40])
        {
            if code[..3] != [0xc7, 0x41, field] {
                return Err(fail(definition.label, "unsupported default instruction"));
            }
            values.push(u32::from_le_bytes(code[3..].try_into().unwrap()));
        }
        let default_volume = i32::try_from(values[0])
            .map_err(|_| fail(definition.label, "invalid default volume"))?;
        let default_pitch = f32::from_bits(values[1]);
        if default_volume < 0
            || !default_pitch.is_finite()
            || default_pitch <= 0.0
            || values[2] != 0
            || values[5] != 0
        {
            return Err(fail(
                definition.label,
                "unsupported default volume/pitch/variance/flags",
            ));
        }
        let default_channel = i32::try_from(values[4])
            .map_err(|_| fail(definition.label, "invalid default channel"))?;
        if channels.bytes.is_empty() || !channels.bytes.len().is_multiple_of(20) {
            return Err(fail(channels.label, "invalid channel table size"));
        }
        let mut parsed = Vec::new();
        for row in channels.bytes.as_chunks::<20>().0 {
            let words: Vec<_> = row
                .as_chunks::<4>()
                .0
                .iter()
                .map(|v| u32::from_le_bytes(*v))
                .collect();
            let gain = f32::from_bits(words[3]);
            if words[0] == 0 || words[1] > words[2] || !gain.is_finite() || gain < 0.0 {
                return Err(fail(
                    channels.label,
                    "invalid channel capacity/priority/gain",
                ));
            }
            parsed.push(Channel {
                capacity: words[0] as usize,
                minimum_priority: words[1],
                maximum_priority: words[2],
                gain,
                flags: words[4],
            });
        }
        if default_channel as usize >= parsed.len() {
            return Err(fail(
                definition.label,
                "default channel missing from source table",
            ));
        }
        let capacity = |input: ScriptAsset<'_>, prefix: &[u8]| -> Result<usize, String> {
            if input.bytes.len() != prefix.len() + 1
                || !input.bytes.starts_with(prefix)
                || input.bytes[prefix.len()] == 0
            {
                return Err(fail(input.label, "unsupported capacity comparison"));
            }
            Ok(usize::from(input.bytes[prefix.len()]))
        };
        let list_capacity = capacity(list, &[0x83, 0xf8])?;
        let filename_capacity = capacity(filename, &[0x83, 0x78, 0x18])?;
        let receipts = [definition, channels, list, filename]
            .iter()
            .map(|s| (s.label.to_owned(), sha256(s.bytes)))
            .collect();
        Ok(Self {
            origin,
            receipts,
            channels: parsed,
            default_volume,
            default_pitch,
            default_priority: values[3],
            default_channel,
            list_capacity,
            filename_capacity,
        })
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn receipts(&self) -> &[(String, Sha256Digest)] {
        &self.receipts
    }
    pub fn channel(&self, index: i32) -> Option<&Channel> {
        self.channels.get(usize::try_from(index).ok()?)
    }
    pub fn default_volume(&self) -> i32 {
        self.default_volume
    }
    pub fn default_pitch(&self) -> f32 {
        self.default_pitch
    }
    pub fn default_priority(&self) -> u32 {
        self.default_priority
    }
    pub fn default_channel(&self) -> i32 {
        self.default_channel
    }
    pub fn list_capacity(&self) -> usize {
        self.list_capacity
    }
    pub fn filename_capacity(&self) -> usize {
        self.filename_capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inputs() -> (Vec<u8>, Vec<u8>) {
        let mut definition = Vec::new();
        for (field, value) in [0x2c, 0x30, 0x34, 0x38, 0x3c, 0x40].into_iter().zip([
            91,
            1.25f32.to_bits(),
            0,
            12,
            0,
            0,
        ]) {
            definition.extend([0xc7, 0x41, field]);
            definition.extend(value.to_le_bytes());
        }
        let channels = [3, 2, 30, 0.75f32.to_bits(), 0u32]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        (definition, channels)
    }
    fn read(
        definition: &[u8],
        channels: &[u8],
        list: &[u8],
        filename: &[u8],
    ) -> Result<VoicePolicy, String> {
        VoicePolicy::read(
            ScriptAsset {
                label: "synthetic:defaults",
                bytes: definition,
            },
            ScriptAsset {
                label: "synthetic:channels",
                bytes: channels,
            },
            ScriptAsset {
                label: "synthetic:list",
                bytes: list,
            },
            ScriptAsset {
                label: "synthetic:filename",
                bytes: filename,
            },
            AssetIdentity {
                source: "synthetic:client".into(),
                edition: "synthetic".into(),
                version: "test".into(),
            },
        )
    }
    #[test]
    fn values_and_receipts_follow_synthetic_source_windows() {
        let (definition, channels) = inputs();
        let policy = read(
            &definition,
            &channels,
            &[0x83, 0xf8, 9],
            &[0x83, 0x78, 0x18, 2],
        )
        .unwrap();
        assert_eq!(policy.default_volume(), 91);
        assert_eq!(policy.default_pitch(), 1.25);
        assert_eq!(policy.default_priority(), 12);
        assert_eq!(policy.default_channel(), 0);
        assert_eq!(policy.list_capacity(), 9);
        assert_eq!(policy.filename_capacity(), 2);
        let channel = policy.channel(0).unwrap();
        assert_eq!(
            (
                channel.capacity,
                channel.minimum_priority,
                channel.maximum_priority,
                channel.gain
            ),
            (3, 2, 30, 0.75)
        );
        assert!(policy.channel(-1).is_none() && policy.channel(1).is_none());
        assert_eq!(policy.receipts()[0].1, sha256(&definition));
        assert_eq!(policy.origin().source, "synthetic:client");
    }
    #[test]
    fn truncated_or_unsupported_source_policy_is_contextual() {
        let (mut definition, mut channels) = inputs();
        for len in 0..definition.len() {
            assert!(
                read(
                    &definition[..len],
                    &channels,
                    &[0x83, 0xf8, 9],
                    &[0x83, 0x78, 0x18, 2]
                )
                .is_err()
            );
        }
        for len in 0..channels.len() {
            assert!(
                read(
                    &definition,
                    &channels[..len],
                    &[0x83, 0xf8, 9],
                    &[0x83, 0x78, 0x18, 2]
                )
                .is_err()
            );
        }
        for list in [
            &[0x83, 0xf8, 0][..],
            &[0x83, 0xff, 9][..],
            &[0x83, 0xf8][..],
        ] {
            assert!(read(&definition, &channels, list, &[0x83, 0x78, 0x18, 2]).is_err());
        }
        channels[12..16].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert!(
            read(
                &definition,
                &channels,
                &[0x83, 0xf8, 9],
                &[0x83, 0x78, 0x18, 2]
            )
            .unwrap_err()
            .contains("synthetic:channels")
        );
        let (_, channels) = inputs();
        definition[17..21].copy_from_slice(&1u32.to_le_bytes());
        assert!(
            read(
                &definition,
                &channels,
                &[0x83, 0xf8, 9],
                &[0x83, 0x78, 0x18, 2]
            )
            .unwrap_err()
            .contains("variance")
        );
    }
}
