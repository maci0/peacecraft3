//! Bounded source-capacity arbitration for admitted non-spatial voices.
use bevy::prelude::*;
use pcx_play::map_startup_host::sound_playback::VoicePolicy;
use pcx_script::HandleId;

#[derive(Clone, Component)]
pub(super) struct Voice {
    pub id: HandleId,
    pub channel: i32,
    pub priority: u32,
    pub member: String,
    pub serial: u64,
    pub preempt_channel: bool,
    pub preempt_list: bool,
}

pub(super) struct Active {
    pub voice: Voice,
    pub entity: Entity,
    pub source: Handle<AudioSource>,
}

pub(super) fn admission<'a>(
    existing: impl Iterator<Item = &'a Voice> + Clone,
    candidate: &Voice,
    policy: &VoicePolicy,
) -> Result<Option<HandleId>, String> {
    let weakest = |voices: Vec<&Voice>| -> Option<HandleId> {
        voices
            .into_iter()
            .min_by_key(|voice| (voice.priority, voice.serial))
            .map(|voice| voice.id)
    };
    let all: Vec<_> = existing.clone().collect();
    if all.len() >= policy.list_capacity() {
        if candidate.preempt_list {
            let eligible = all
                .into_iter()
                .filter(|voice| voice.priority <= candidate.priority)
                .collect();
            if let Some(id) = weakest(eligible) {
                return Ok(Some(id));
            }
        }
        return Err(format!(
            "source sound-list capacity {} reached; no eligible list preemption",
            policy.list_capacity()
        ));
    }
    let channel = policy
        .channel(candidate.channel)
        .ok_or("sound channel missing from source policy")?;
    let same: Vec<_> = existing
        .clone()
        .filter(|voice| voice.channel == candidate.channel)
        .collect();
    if same.len() >= channel.capacity {
        if candidate.preempt_channel {
            let eligible = same
                .into_iter()
                .filter(|voice| voice.priority < candidate.priority)
                .collect();
            if let Some(id) = weakest(eligible) {
                return Ok(Some(id));
            }
        }
        return Err(format!(
            "source channel {} capacity {} reached; no lower-priority channel preemption",
            candidate.channel, channel.capacity
        ));
    }
    if existing
        .filter(|voice| voice.member == candidate.member)
        .count()
        >= policy.filename_capacity()
    {
        return Err(format!(
            "source filename capacity {} reached for {:?}; oldest-preemption flag required",
            policy.filename_capacity(),
            candidate.member
        ));
    }
    Ok(None)
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::tests::mix_policy;
    use super::*;
    fn voice(id: u64, priority: u32) -> Voice {
        Voice {
            id: HandleId::new(std::num::NonZeroU64::new(id).unwrap()),
            channel: 0,
            priority,
            member: format!("{id}.wav"),
            serial: id,
            preempt_channel: true,
            preempt_list: true,
        }
    }
    #[test]
    pub(in crate::script_audio) fn list_and_channel_priority_rules_use_distinct_comparisons() {
        let old = [voice(1, 7), voice(2, 9)];
        let same = voice(3, 7);
        assert_eq!(
            admission(old.iter(), &same, &mix_policy(5, 2, 4)),
            Ok(Some(old[0].id)),
            "list permits equal priority"
        );
        assert!(
            admission(old.iter(), &same, &mix_policy(2, 5, 4)).is_err(),
            "channel requires strictly greater priority"
        );
        assert_eq!(
            admission(old.iter(), &voice(3, 8), &mix_policy(2, 5, 4)),
            Ok(Some(old[0].id))
        );
        assert!(admission(old.iter(), &voice(3, 6), &mix_policy(5, 2, 4)).is_err());
        let mut disabled = voice(3, 10);
        disabled.preempt_channel = false;
        disabled.preempt_list = false;
        assert!(admission(old.iter(), &disabled, &mix_policy(5, 2, 4)).is_err());
        assert!(admission(old.iter(), &disabled, &mix_policy(2, 5, 4)).is_err());
    }
    #[test]
    pub(in crate::script_audio) fn filename_capacity_and_deterministic_ties_preserve_flags() {
        let mut old = [voice(2, 7), voice(1, 7)];
        assert_eq!(
            admission(old.iter(), &voice(3, 7), &mix_policy(5, 2, 4)),
            Ok(Some(old[1].id))
        );
        old[1].member = old[0].member.clone();
        let mut incoming = voice(3, 100);
        incoming.member = old[0].member.clone();
        assert!(
            admission(old.iter(), &incoming, &mix_policy(5, 5, 2))
                .unwrap_err()
                .contains("filename capacity 2")
        );
        incoming.member = "different.wav".into();
        assert_eq!(
            admission(old.iter(), &incoming, &mix_policy(5, 5, 2)),
            Ok(None)
        );
    }
}
