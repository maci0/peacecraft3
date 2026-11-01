//! Recreation-owned explicit mission availability and completion, not an
//! original profile importer.
use std::collections::BTreeMap;

const MAGIC: &[u8] = b"PCXPROGRESS1\n";
const MAGIC_COMPLETION: &[u8] = b"PCXPROGRESS2\n";
pub(crate) const MAX_ENTRIES: usize = 65536;
pub(crate) const MAX_BYTES: usize = 8 << 20;

/// Only executed assignments are known. Missing entries do not invent fresh-profile unlocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    missions: BTreeMap<(i32, i32), bool>,
    completed: BTreeMap<(i32, i32), bool>,
}

impl Progress {
    pub fn mission_available(&self, campaign: i32, mission: i32) -> Option<bool> {
        self.missions.get(&(campaign, mission)).copied()
    }

    /// Authored `DefaultOpen` availability refined by executed assignments.
    /// An explicit `SetMissionAvailable` value wins; unknown missions fall back
    /// to the campaign index's authored default. Fresh-profile unlocks remain
    /// unknown and are not invented here.
    pub fn availability(&self, campaign: i32, mission: i32, default_open: bool) -> bool {
        self.mission_available(campaign, mission)
            .unwrap_or(default_open)
    }

    pub(crate) fn set(
        &mut self,
        campaign: i32,
        mission: i32,
        available: bool,
    ) -> Result<(), String> {
        if campaign < 0 || mission < 0 {
            return Err("negative campaign/mission indices are not admitted".into());
        }
        if !self.missions.contains_key(&(campaign, mission)) && self.missions.len() >= MAX_ENTRIES {
            return Err("campaign progress entry policy exceeded".into());
        }
        self.missions.insert((campaign, mission), available);
        Ok(())
    }

    /// Record an executed full-mission victory for `(campaign, mission)`.
    /// Only script-observed victories are known; this never invents a win.
    pub fn mark_complete(&mut self, campaign: i32, mission: i32) -> Result<(), String> {
        if campaign < 0 || mission < 0 {
            return Err("negative campaign/mission indices are not admitted".into());
        }
        if !self.completed.contains_key(&(campaign, mission)) && self.completed.len() >= MAX_ENTRIES
        {
            return Err("campaign completion entry policy exceeded".into());
        }
        self.completed.insert((campaign, mission), true);
        Ok(())
    }

    /// True only after an executed `mark_complete` for the same key.
    pub fn is_complete(&self, campaign: i32, mission: i32) -> bool {
        self.completed
            .get(&(campaign, mission))
            .copied()
            .unwrap_or(false)
    }

    pub fn completed_missions(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.completed
            .iter()
            .filter(|(_, done)| **done)
            .map(|(key, _)| *key)
    }

    /// Next uncompleted mission in caller-supplied authored order. The order
    /// slice carries `(campaign, mission)` keys in index order (for example
    /// the admitted `CampaignList` positions); no campaign sequence is
    /// hard-coded here. Returns `None` when every listed mission is complete.
    pub fn next_incomplete(&self, ordered: &[(i32, i32)]) -> Option<(i32, i32)> {
        ordered
            .iter()
            .copied()
            .find(|(campaign, mission)| !self.is_complete(*campaign, *mission))
    }

    /// True when every mission in the caller-supplied authored order is
    /// complete. An empty order never reports completion.
    pub fn order_complete(&self, ordered: &[(i32, i32)]) -> bool {
        !ordered.is_empty() && self.next_incomplete(ordered).is_none()
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        let length = self
            .missions
            .len()
            .checked_add(self.completed.len())
            .and_then(|n| n.checked_mul(9))
            .and_then(|size| size.checked_add(MAGIC_COMPLETION.len() + 8))
            .filter(|size| *size <= MAX_BYTES)
            .ok_or("campaign progress byte policy exceeded")?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(MAGIC_COMPLETION);
        bytes.extend_from_slice(
            &u32::try_from(self.missions.len())
                .map_err(|e| e.to_string())?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(
            &u32::try_from(self.completed.len())
                .map_err(|e| e.to_string())?
                .to_le_bytes(),
        );
        for (&(campaign, mission), &available) in &self.missions {
            bytes.extend_from_slice(&campaign.to_le_bytes());
            bytes.extend_from_slice(&mission.to_le_bytes());
            bytes.push(u8::from(available));
        }
        for (campaign, mission) in self.completed.keys() {
            bytes.extend_from_slice(&campaign.to_le_bytes());
            bytes.extend_from_slice(&mission.to_le_bytes());
            bytes.push(1);
        }
        Ok(bytes)
    }

    fn decode_v1_body(value: &mut Self, body: &[u8]) -> Result<(), String> {
        let count = body.len() / 9;
        if count > MAX_ENTRIES || count.checked_mul(9) != Some(body.len()) {
            return Err("campaign progress count/length policy".into());
        }
        for entry in body.as_chunks::<9>().0 {
            let campaign = i32::from_le_bytes(entry[..4].try_into().unwrap());
            let mission = i32::from_le_bytes(entry[4..8].try_into().unwrap());
            let available = match entry[8] {
                0 => false,
                1 => true,
                _ => return Err("invalid mission availability boolean".into()),
            };
            if value.mission_available(campaign, mission).is_some() {
                return Err("duplicate campaign progress entry".into());
            }
            value.set(campaign, mission, available)?;
        }
        Ok(())
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err("unsupported campaign progress format/byte policy".into());
        }
        if bytes.starts_with(MAGIC_COMPLETION) {
            let header = bytes
                .get(MAGIC_COMPLETION.len()..MAGIC_COMPLETION.len() + 8)
                .ok_or("truncated campaign progress header")?;
            let avail = usize::try_from(u32::from_le_bytes(header[..4].try_into().unwrap()))
                .map_err(|e| e.to_string())?;
            let done = usize::try_from(u32::from_le_bytes(header[4..8].try_into().unwrap()))
                .map_err(|e| e.to_string())?;
            let body = &bytes[MAGIC_COMPLETION.len() + 8..];
            if avail > MAX_ENTRIES
                || done > MAX_ENTRIES
                || avail
                    .checked_add(done)
                    .is_none_or(|n| n.checked_mul(9) != Some(body.len()))
            {
                return Err("campaign progress count/length policy".into());
            }
            let mut value = Self::default();
            let (first, second) = body.split_at(avail * 9);
            for entry in first.as_chunks::<9>().0 {
                let campaign = i32::from_le_bytes(entry[..4].try_into().unwrap());
                let mission = i32::from_le_bytes(entry[4..8].try_into().unwrap());
                let available = match entry[8] {
                    0 => false,
                    1 => true,
                    _ => return Err("invalid mission availability boolean".into()),
                };
                if value.mission_available(campaign, mission).is_some() {
                    return Err("duplicate campaign progress entry".into());
                }
                value.set(campaign, mission, available)?;
            }
            for entry in second.as_chunks::<9>().0 {
                let campaign = i32::from_le_bytes(entry[..4].try_into().unwrap());
                let mission = i32::from_le_bytes(entry[4..8].try_into().unwrap());
                if entry[8] != 1 {
                    return Err("invalid mission completion boolean".into());
                }
                if value.is_complete(campaign, mission) {
                    return Err("duplicate campaign completion entry".into());
                }
                value.mark_complete(campaign, mission)?;
            }
            return Ok(value);
        }
        if !bytes.starts_with(MAGIC) {
            return Err("unsupported campaign progress format/byte policy".into());
        }
        let count = bytes
            .get(MAGIC.len()..MAGIC.len() + 4)
            .ok_or("truncated campaign progress header")?;
        let count = usize::try_from(u32::from_le_bytes(count.try_into().unwrap()))
            .map_err(|e| e.to_string())?;
        let body = &bytes[MAGIC.len() + 4..];
        if count > MAX_ENTRIES || count.checked_mul(9) != Some(body.len()) {
            return Err("campaign progress count/length policy".into());
        }
        let mut value = Self::default();
        Self::decode_v1_body(&mut value, body)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_false_unknown_and_round_trip_remain_distinct() {
        let mut progress = Progress::default();
        assert_eq!(progress.mission_available(3, 7), None);
        progress.set(3, 7, false).unwrap();
        progress.set(9, 2, true).unwrap();
        assert_eq!(
            Progress::decode(&progress.encode().unwrap()).unwrap(),
            progress
        );
        assert_eq!(progress.mission_available(3, 7), Some(false));
        assert_eq!(progress.mission_available(3, 8), None);
        assert!(progress.set(-1, 2, true).is_err());
    }
    #[test]
    fn malformed_counts_entries_and_trailing_data_refuse() {
        let mut progress = Progress::default();
        progress.set(3, 7, true).unwrap();
        let valid = progress.encode().unwrap();
        for end in 0..valid.len() {
            assert!(Progress::decode(&valid[..end]).is_err());
        }
        let mut extra = valid.clone();
        extra.push(0);
        assert!(Progress::decode(&extra).is_err());
        let mut boolean = valid.clone();
        *boolean.last_mut().unwrap() = 2;
        assert!(Progress::decode(&boolean).is_err());
        let mut duplicate = valid.clone();
        duplicate[MAGIC_COMPLETION.len()..MAGIC_COMPLETION.len() + 4]
            .copy_from_slice(&2u32.to_le_bytes());
        duplicate.extend_from_slice(&valid[MAGIC_COMPLETION.len() + 8..]);
        assert!(Progress::decode(&duplicate).is_err());
        let mut negative = valid;
        negative[MAGIC_COMPLETION.len() + 8..MAGIC_COMPLETION.len() + 12]
            .copy_from_slice(&(-1i32).to_le_bytes());
        assert!(Progress::decode(&negative).is_err());
    }
}
