//! Canonical content receipt for the catalog and Misc source data a headless startup admits.
//!
//! The receipt keys every member by its normalized archive path (`pcx_assets::AssetPath`) and
//! binds the member's exact admitted bytes by SHA-256, or records that the member was absent.
//! It never keys on the install directory, so the same selected archives at another path produce
//! the same receipt. Two starts that admit different bytes under the same member path produce
//! different receipts, and conflicting observations of one member refuse.
use crate::neutral_player::{Sha256Digest, sha256};
use pcx_assets::AssetPath;
use std::collections::BTreeMap;

/// More members than any catalog this startup accepts; the ceiling bounds receipt work.
pub const MAX_RECEIPT_MEMBERS: usize = 512;
/// One member's content ceiling, matching the campaign catalog's per-member read limit.
pub const MAX_RECEIPT_MEMBER_BYTES: usize = 4 << 20;
/// Aggregate content ceiling, matching the selected source reader's operating ceiling.
pub const MAX_RECEIPT_BYTES: u64 = 512 << 20;

/// Every `Units\` profile table name the campaign catalog admits, as a lowercase leaf.
const CATALOG_PROFILE_SUFFIXES: &[&str] = &[
    "unitfunc.txt",
    "unitstrings.txt",
    "abilityfunc.txt",
    "abilitystrings.txt",
    "upgradefunc.txt",
    "upgradestrings.txt",
    "itemfunc.txt",
    "itemstrings.txt",
];

#[derive(Debug, thiserror::Error)]
pub enum ReceiptError {
    #[error("member {member:?}: {source}")]
    Path {
        member: String,
        #[source]
        source: pcx_assets::PathError,
    },
    #[error("receipt exceeds {MAX_RECEIPT_MEMBERS} members")]
    TooManyMembers,
    #[error("member {0} exceeds {MAX_RECEIPT_MEMBER_BYTES} bytes")]
    MemberTooLarge(String),
    #[error("receipt content exceeds {MAX_RECEIPT_BYTES} bytes")]
    AggregateTooLarge,
    #[error("member {0} was admitted with different content twice")]
    ConflictingContent(String),
    #[error("catalog profile inventory exceeds its work budget")]
    ProfileInventory,
    #[error("catalog admits more than {MAX_CATALOG_PROFILES} profile members")]
    ProfileCount,
    #[error("catalog profile read: {0}")]
    ProfileRead(String),
}
const MAX_CATALOG_PROFILES: usize = 256;

/// Canonical receipt over one admitted source set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceReceipt {
    digest: Sha256Digest,
    members: usize,
    bytes: u64,
}

impl SourceReceipt {
    /// SHA-256 of the canonical member list and each member's content digest (or absence).
    pub fn digest(&self) -> Sha256Digest {
        self.digest
    }
    /// Number of distinct admitted members, present or absent.
    pub fn members(&self) -> usize {
        self.members
    }
    /// Sum of the present members' admitted byte lengths.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Accumulates member observations while the startup reads its selected sources.
#[derive(Debug, Default)]
pub struct SourceReceiptRecorder {
    entries: BTreeMap<AssetPath, Option<Sha256Digest>>,
    bytes: u64,
    error: Option<ReceiptError>,
}

impl SourceReceiptRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this member was already observed under its normalized path.
    pub fn contains(&self, member: &str) -> bool {
        AssetPath::new(member)
            .ok()
            .is_some_and(|key| self.entries.contains_key(&key))
    }

    /// Records the exact bytes an admitted read returned, or the member's absence.
    ///
    /// Observations never stop the caller's read: a refusal is retained and surfaced by
    /// [`SourceReceiptRecorder::finish`], so a malformed receipt cannot silently pass.
    pub fn observe(&mut self, member: &str, content: Option<&[u8]>) {
        if self.error.is_some() {
            return;
        }
        let key = match AssetPath::new(member) {
            Ok(key) => key,
            Err(source) => {
                self.error = Some(ReceiptError::Path {
                    member: member.into(),
                    source,
                });
                return;
            }
        };
        let Some(bytes) = content else {
            match self.entries.get(&key) {
                None => {}
                Some(None) => return,
                Some(Some(_)) => {
                    self.error = Some(ReceiptError::ConflictingContent(member.into()));
                    return;
                }
            }
            if self.entries.len() >= MAX_RECEIPT_MEMBERS {
                self.error = Some(ReceiptError::TooManyMembers);
                return;
            }
            self.entries.insert(key, None);
            return;
        };
        if bytes.len() > MAX_RECEIPT_MEMBER_BYTES {
            self.error = Some(ReceiptError::MemberTooLarge(member.into()));
            return;
        }
        let sum = self.bytes.saturating_add(bytes.len() as u64);
        if sum > MAX_RECEIPT_BYTES {
            self.error = Some(ReceiptError::AggregateTooLarge);
            return;
        }
        let digest = sha256(bytes);
        if let Some(existing) = self.entries.get(&key) {
            if existing.as_ref() != Some(&digest) {
                self.error = Some(ReceiptError::ConflictingContent(member.into()));
            }
            return;
        }
        if self.entries.len() >= MAX_RECEIPT_MEMBERS {
            self.error = Some(ReceiptError::TooManyMembers);
            return;
        }
        self.entries.insert(key, Some(digest));
        self.bytes = sum;
    }

    /// Refuses on any retained problem, otherwise hashes the canonical member list.
    pub fn finish(self) -> Result<SourceReceipt, ReceiptError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let members = self.entries.len();
        let mut text = String::from("pcx-source-receipt-v1\n");
        for (path, digest) in &self.entries {
            match digest {
                Some(digest) => {
                    text.push_str("content ");
                    text.push_str(path.as_str());
                    text.push(' ');
                    push_hex(&mut text, digest);
                    text.push('\n');
                }
                None => {
                    text.push_str("absent ");
                    text.push_str(path.as_str());
                    text.push('\n');
                }
            }
        }
        Ok(SourceReceipt {
            digest: sha256(text.as_bytes()),
            members,
            bytes: self.bytes,
        })
    }
}

/// Whether a normalized archive path is one of the campaign catalog's top-level `Units\`
/// profile members. The campaign catalog reads these tables from its own inventory scan;
/// the receipt selects the same members so their admitted content is bound too.
pub fn is_catalog_profile_member(path: &str) -> bool {
    let Some(leaf) = path.strip_prefix("units/") else {
        return false;
    };
    !leaf.contains('/')
        && CATALOG_PROFILE_SUFFIXES
            .iter()
            .any(|suffix| leaf.ends_with(suffix))
}

/// Reads every catalog profile member not already observed, mirroring the catalog's own
/// inventory budget. `read` extracts one bounded member and returns `None` when absent.
pub fn observe_catalog_profiles<E>(
    recorder: &mut SourceReceiptRecorder,
    paths: impl Iterator<Item = AssetPath>,
    mut read: impl FnMut(&str) -> Result<Option<Vec<u8>>, E>,
) -> Result<(), ReceiptError>
where
    E: std::fmt::Display,
{
    let mut profiles = 0usize;
    for (visited, path) in paths.enumerate() {
        if visited >= crate::install_source::MAX_LISTED_PATHS {
            return Err(ReceiptError::ProfileInventory);
        }
        if !is_catalog_profile_member(path.as_str()) {
            continue;
        }
        profiles += 1;
        if profiles > MAX_CATALOG_PROFILES {
            return Err(ReceiptError::ProfileCount);
        }
        if recorder.contains(path.as_str()) {
            continue;
        }
        let content =
            read(path.as_str()).map_err(|error| ReceiptError::ProfileRead(error.to_string()))?;
        recorder.observe(path.as_str(), content.as_deref());
    }
    Ok(())
}

fn push_hex(text: &mut String, bytes: &[u8]) {
    for byte in bytes {
        text.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        text.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0f)]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(entries: &[(&str, Option<&[u8]>)]) -> SourceReceipt {
        let mut recorder = SourceReceiptRecorder::new();
        for (member, content) in entries {
            recorder.observe(member, *content);
        }
        recorder.finish().expect("receipt")
    }

    #[test]
    fn changed_content_under_one_path_changes_the_receipt() {
        let original = receipt(&[(r"Units\MiscGame.txt", Some(b"[Misc]\r\nFoo=1\r\n"))]);
        let changed = receipt(&[(r"Units\MiscGame.txt", Some(b"[Misc]\r\nFoo=2\r\n"))]);
        assert_ne!(original.digest(), changed.digest());
        assert_eq!(original.members(), changed.members());
    }

    #[test]
    fn member_spelling_case_and_order_do_not_change_the_receipt() {
        let forward = receipt(&[
            (r"Units\MiscGame.txt", Some(b"game")),
            (r"Units\MiscData.txt", Some(b"misc")),
        ]);
        let reordered = receipt(&[
            (r"units/miscdata.TXT", Some(b"misc")),
            (r"UNITS\MISCGAME.TXT", Some(b"game")),
        ]);
        assert_eq!(forward.digest(), reordered.digest());
        assert_eq!(forward.members(), reordered.members());
    }

    #[test]
    fn repeated_observation_of_one_content_is_idempotent() {
        let once = receipt(&[(r"Units\MiscData.txt", Some(b"rules"))]);
        let twice = receipt(&[
            (r"Units\MiscData.txt", Some(b"rules")),
            (r"units\miscdata.txt", Some(b"rules")),
        ]);
        assert_eq!(once.digest(), twice.digest());
        assert_eq!(once.bytes(), twice.bytes());
    }

    #[test]
    fn absent_empty_and_conflicting_members_are_distinct_or_refused() {
        let absent = receipt(&[(r"Units\MiscData.txt", None)]);
        let empty = receipt(&[(r"Units\MiscData.txt", Some(b""))]);
        assert_ne!(absent.digest(), empty.digest());

        let mut recorder = SourceReceiptRecorder::new();
        recorder.observe(r"Units\MiscData.txt", Some(b"one"));
        recorder.observe(r"units\miscdata.txt", Some(b"two"));
        assert!(matches!(
            recorder.finish(),
            Err(ReceiptError::ConflictingContent(_))
        ));
    }

    #[test]
    fn catalog_profile_selection_matches_the_admitted_units_tables() {
        assert!(is_catalog_profile_member("units/humanunitfunc.txt"));
        assert!(is_catalog_profile_member("units/upgradefunc.txt"));
        assert!(is_catalog_profile_member("units/itemstrings.txt"));
        assert!(!is_catalog_profile_member("units/unitdata.slk"));
        assert!(!is_catalog_profile_member("units/sub/unitfunc.txt"));
        assert!(!is_catalog_profile_member("ui/miscdata.txt"));
    }

    #[test]
    fn profile_observation_records_only_unseen_members_and_respects_absence() {
        let paths = [
            AssetPath::new("units/upgradefunc.txt").unwrap(),
            AssetPath::new("units/itemfunc.txt").unwrap(),
            AssetPath::new("units/unitdata.slk").unwrap(),
        ];
        let mut recorder = SourceReceiptRecorder::new();
        recorder.observe("Units\\ItemFunc.txt", Some(b"seen"));
        let mut read = Vec::new();
        observe_catalog_profiles(&mut recorder, paths.into_iter(), |member| {
            read.push(member.to_string());
            Ok::<_, std::io::Error>(Some(b"profile".to_vec()))
        })
        .unwrap();
        assert_eq!(read, ["units/upgradefunc.txt"]);
        let finished = recorder.finish().unwrap();
        assert_eq!(finished.members(), 2);
    }
}
