//! Selected-client audio-update timing; no source audio or measured operands.
use crate::{
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::{AssetIdentity, ScriptAsset},
};
#[path = "sound_voice_policy.rs"]
mod voice;
pub use voice::{Channel, GroupPolicy, VoicePolicy};

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    update_seconds: f32,
    minimum_fade_step: u32,
    voice_policy: std::sync::Arc<VoicePolicy>,
    group_policy: std::sync::Arc<GroupPolicy>,
}

impl Rules {
    pub fn group_policy(&self) -> std::sync::Arc<GroupPolicy> {
        self.group_policy.clone()
    }
    pub fn voice_policy(&self) -> std::sync::Arc<VoicePolicy> {
        std::sync::Arc::clone(&self.voice_policy)
    }
    pub fn update_seconds(&self) -> f32 {
        self.update_seconds
    }
    pub fn minimum_fade_step(&self) -> u32 {
        self.minimum_fade_step
    }
    /// Reviewed instruction locations are bindings; their operands remain source data.
    pub fn reviewed_legacy(image: &[u8], origin: AssetIdentity) -> Result<Self, String> {
        let fail = |reason: &str| format!("sound playback {}: {reason}", origin.source);
        if image.len() > 32 << 20
            || sha256(image) != crate::menu_lighting::REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 << 20,
                profile_bytes: 64 << 10,
                sections: 96,
            },
        )
        .map_err(|e| fail(&e.to_string()))?;
        let timer = pe
            .read_from(0x5dbe77, true)
            .map_err(|e| fail(&e.to_string()))?;
        let minimum = pe
            .read_from(0x5d67ce, true)
            .map_err(|e| fail(&e.to_string()))?;
        let (update_seconds, minimum_fade_step) = operands(timer, minimum).map_err(fail)?;
        let window = |va, len, executable| -> Result<&[u8], String> {
            pe.read_from(va, executable)
                .map_err(|e| fail(&e.to_string()))?
                .get(..len)
                .ok_or_else(|| fail("truncated voice policy window"))
        };
        let count = window(0x5d71cf, 2, true)?;
        let pointer = window(0x5d71d7, 5, true)?;
        if count[0] != 0x6a || pointer[0] != 0x68 {
            return Err(fail("unsupported channel initialization"));
        }
        let table = u32::from_le_bytes(pointer[1..].try_into().unwrap());
        let voice_policy = VoicePolicy::read(
            ScriptAsset {
                label: "client sound definition defaults",
                bytes: window(0x5b8cbe, 42, true)?,
            },
            ScriptAsset {
                label: "client sound channel table",
                bytes: window(table, usize::from(count[1]) * 20, false)?,
            },
            ScriptAsset {
                label: "client total sound-list comparison",
                bytes: window(0x7c74fc, 3, true)?,
            },
            ScriptAsset {
                label: "client filename-count comparison",
                bytes: window(0x7c7739, 4, true)?,
            },
            origin.clone(),
        )?;
        let reset = window(0x5d99c5, 34, true)?;
        let mask_load = window(0x5bfa64, 6, true)?;
        if mask_load[..2] != [0x8b, 0x82] {
            return Err(fail("unsupported category mask load"));
        }
        let mask_pointer = u32::from_le_bytes(mask_load[2..].try_into().unwrap());
        let group_policy = GroupPolicy::read(
            reset,
            window(0x4afbb6, 5, true)?,
            window(mask_pointer, usize::from(reset[30]), false)?,
            update_seconds,
        )
        .map_err(|e| fail(&e))?;
        let clamp = window(0x79fa44, 8, true)?;
        if clamp[..4] != [0xf3, 0x0f, 0x10, 0x0d] {
            return Err(fail("unsupported channel gain clamp load"));
        }
        let clamp_pointer = u32::from_le_bytes(clamp[4..].try_into().unwrap());
        let clamp_unity = f32::from_le_bytes(window(clamp_pointer, 4, false)?.try_into().unwrap());
        if clamp_unity != group_policy.unity() {
            return Err(fail("reset unity and channel clamp disagree"));
        }
        Ok(Self {
            origin,
            image_sha256: sha256(image),
            update_seconds,
            minimum_fade_step,
            voice_policy: std::sync::Arc::new(voice_policy),
            group_policy: std::sync::Arc::new(group_policy),
        })
    }
}

fn operands(timer: &[u8], minimum: &[u8]) -> Result<(f32, u32), &'static str> {
    let timer = timer.get(..7).ok_or("truncated audio timer instruction")?;
    let minimum = minimum
        .get(..5)
        .ok_or("truncated fade minimum instruction")?;
    if timer[..3] != [0xc7, 0x04, 0x24] || minimum[0] != 0xba {
        return Err("unsupported audio timer/fade instructions");
    }
    let seconds = f32::from_le_bytes(timer[3..].try_into().unwrap());
    let step = u32::from_le_bytes(minimum[1..].try_into().unwrap());
    if !seconds.is_finite() || seconds <= 0.0 || step == 0 {
        return Err("invalid audio timer/fade operands");
    }
    Ok((seconds, step))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timer_and_fade_operands_follow_synthetic_instructions() {
        let mut timer = vec![0xc7, 0x04, 0x24];
        timer.extend(0.25f32.to_le_bytes());
        let mut minimum = vec![0xba];
        minimum.extend(3u32.to_le_bytes());
        assert_eq!(operands(&timer, &minimum), Ok((0.25, 3)));
        for len in 0..timer.len() {
            assert!(operands(&timer[..len], &minimum).is_err());
        }
        for len in 0..minimum.len() {
            assert!(operands(&timer, &minimum[..len]).is_err());
        }
        for seconds in [0.0, -1.0, f32::INFINITY, f32::NAN] {
            timer[3..].copy_from_slice(&seconds.to_le_bytes());
            assert!(operands(&timer, &minimum).is_err());
        }
        timer[3..].copy_from_slice(&0.5f32.to_le_bytes());
        minimum[1..].copy_from_slice(&0u32.to_le_bytes());
        assert!(operands(&timer, &minimum).is_err());
        minimum[1..].copy_from_slice(&2u32.to_le_bytes());
        timer[0] = 0x90;
        assert!(operands(&timer, &minimum).is_err());
    }
    #[test]
    fn unqualified_client_is_refused_before_instruction_reads() {
        let error = Rules::reviewed_legacy(
            &[],
            AssetIdentity {
                source: "synthetic:client".into(),
                edition: "synthetic".into(),
                version: "test".into(),
            },
        )
        .unwrap_err();
        assert!(error.contains("synthetic:client") && error.contains("identity"));
    }
}
