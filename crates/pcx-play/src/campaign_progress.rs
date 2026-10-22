//! Recreation-owned explicit mission availability, not an original profile importer.
use std::collections::BTreeMap;

const MAGIC: &[u8] = b"PCXPROGRESS1\n";
pub(crate) const MAX_ENTRIES: usize = 65536;
pub(crate) const MAX_BYTES: usize = 8 << 20;

/// Only executed assignments are known. Missing entries do not invent fresh-profile unlocks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    missions: BTreeMap<(i32, i32), bool>,
}

impl Progress {
    pub fn mission_available(&self, campaign: i32, mission: i32) -> Option<bool> {
        self.missions.get(&(campaign, mission)).copied()
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

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        let length = self
            .missions
            .len()
            .checked_mul(9)
            .and_then(|size| size.checked_add(MAGIC.len() + 4))
            .filter(|size| *size <= MAX_BYTES)
            .ok_or("campaign progress byte policy exceeded")?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|e| e.to_string())?;
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(
            &u32::try_from(self.missions.len())
                .map_err(|e| e.to_string())?
                .to_le_bytes(),
        );
        for (&(campaign, mission), &available) in &self.missions {
            bytes.extend_from_slice(&campaign.to_le_bytes());
            bytes.extend_from_slice(&mission.to_le_bytes());
            bytes.push(u8::from(available));
        }
        Ok(bytes)
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES || !bytes.starts_with(MAGIC) {
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
        duplicate[MAGIC.len()..MAGIC.len() + 4].copy_from_slice(&2u32.to_le_bytes());
        duplicate.extend_from_slice(&valid[MAGIC.len() + 4..]);
        assert!(Progress::decode(&duplicate).is_err());
        let mut negative = valid;
        negative[MAGIC.len() + 4..MAGIC.len() + 8].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(Progress::decode(&negative).is_err());
    }
}
