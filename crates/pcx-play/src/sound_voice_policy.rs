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

/// Source category routing and real-time interpolation operands.
#[derive(Clone, Debug)]
pub struct GroupPolicy {
    masks: Vec<u32>,
    unity: f32,
    duration: std::time::Duration,
    period: std::time::Duration,
}
impl GroupPolicy {
    pub fn read(reset: &[u8], set: &[u8], masks: &[u8], period: f32) -> Result<Self, String> {
        let bad = || "unsupported sound category interpolation windows".to_owned();
        if reset.len() != 34
            || reset[..3] != [0xc7, 0x46, 0x08]
            || reset[15..18] != [0xc7, 0x46, 0x10]
            || reset[28..30] != [0x83, 0xff]
            || set.len() != 5
            || set[0] != 0x68
        {
            return Err(bad());
        }
        let unity = f32::from_le_bytes(reset[3..7].try_into().unwrap());
        let millis = u32::from_le_bytes(reset[18..22].try_into().unwrap());
        let count = usize::from(reset[30]) / 4;
        if !usize::from(reset[30]).is_multiple_of(4)
            || count == 0
            || masks.len() != count * 4
            || !unity.is_finite()
            || unity <= 0.0
            || !period.is_finite()
            || period <= 0.0
            || set[1..] != reset[18..22]
        {
            return Err(bad());
        }
        let masks: Vec<_> = masks
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| u32::from_le_bytes(*v))
            .collect();
        let mut used = 0;
        for mask in &masks {
            if used & mask != 0 {
                return Err(bad());
            }
            used |= mask;
        }
        let period = std::time::Duration::try_from_secs_f32(period).map_err(|_| bad())?;
        if period.is_zero() {
            return Err(bad());
        }
        Ok(Self {
            masks,
            unity,
            duration: std::time::Duration::from_millis(u64::from(millis)),
            period,
        })
    }
    pub fn masks(&self) -> &[u32] {
        &self.masks
    }
    pub fn unity(&self) -> f32 {
        self.unity
    }
    pub fn duration(&self) -> std::time::Duration {
        self.duration
    }
    pub fn period(&self) -> std::time::Duration {
        self.period
    }
}

#[cfg(test)]
mod group_tests {
    use super::*;
    #[test]
    fn category_operands_masks_and_malformed_windows_are_checked() {
        let mut reset = vec![0; 34];
        reset[..3].copy_from_slice(&[0xc7, 0x46, 0x08]);
        reset[3..7].copy_from_slice(&0.75f32.to_le_bytes());
        reset[15..18].copy_from_slice(&[0xc7, 0x46, 0x10]);
        reset[18..22].copy_from_slice(&250u32.to_le_bytes());
        reset[28..31].copy_from_slice(&[0x83, 0xff, 8]);
        let mut set = vec![0x68];
        set.extend(250u32.to_le_bytes());
        let masks = [4u32.to_le_bytes(), 16u32.to_le_bytes()].concat();
        let p = GroupPolicy::read(&reset, &set, &masks, 0.05).unwrap();
        assert_eq!(p.unity(), 0.75);
        assert_eq!(p.duration().as_millis(), 250);
        assert_eq!(p.masks(), [4, 16]);
        for len in 0..reset.len() {
            assert!(GroupPolicy::read(&reset[..len], &set, &masks, 0.05).is_err());
        }
        set[1] ^= 1;
        assert!(GroupPolicy::read(&reset, &set, &masks, 0.05).is_err());
        set[1] ^= 1;
        let duplicate = [4u32.to_le_bytes(), 4u32.to_le_bytes()].concat();
        assert!(GroupPolicy::read(&reset, &set, &duplicate, 0.05).is_err());
        assert!(GroupPolicy::read(&reset, &set, &masks, f32::NAN).is_err());
        reset[3..7].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(GroupPolicy::read(&reset, &set, &masks, 0.05).is_err());
    }
}
