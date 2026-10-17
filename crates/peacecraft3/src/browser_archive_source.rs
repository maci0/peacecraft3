//! A selected committed browser archive; no cache flattening, archive clone or inferred policy.
use pcx_assets::{
    AssetPath, ContentSource, LayerKind, NamedMember, SourceContent, SourceError, SourceInfo,
};
use pcx_mpq::{BlockEntry, ExtractionBudget, HashEntry, Header, MpqArchive};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveSelection {
    pub index: usize,
    pub committed_revision: u32,
    pub archive_name: String,
    pub archive_bytes: u64,
    pub header: Header,
    /// Opened tables of the instance observed by [`ArchiveSelection::inspect`].
    /// Path, length and header can stay identical across a committed replacement.
    hash_entries: Vec<HashEntry>,
    block_entries: Vec<BlockEntry>,
}
impl ArchiveSelection {
    /// Observe the selected actual instance under the same generation locks.
    pub fn inspect(
        archives: &Mutex<Vec<MpqArchive>>,
        revision: &Mutex<u32>,
        index: usize,
        committed_revision: u32,
    ) -> Result<Self, SourceError> {
        let revision = revision.lock().map_err(|_| SourceError::Unavailable {
            detail: "committed revision lock is poisoned".into(),
        })?;
        if *revision != committed_revision {
            return Err(SourceError::Unavailable {
                detail: format!(
                    "requested revision {committed_revision} is stale; current {}",
                    *revision
                ),
            });
        }
        let archives = archives.lock().map_err(|_| SourceError::Unavailable {
            detail: "committed archive lock is poisoned".into(),
        })?;
        let archive = archives
            .get(index)
            .ok_or_else(|| SourceError::Unavailable {
                detail: format!("selected archive index {index} is absent"),
            })?;
        Ok(Self {
            index,
            committed_revision,
            archive_name: archive
                .path()
                .to_str()
                .ok_or_else(|| invalid("selected archive origin is not UTF-8"))?
                .into(),
            archive_bytes: archive.byte_len(),
            header: archive.header,
            hash_entries: archive.hash_entries().to_vec(),
            block_entries: archive.block_entries().to_vec(),
        })
    }
}
struct Mapping {
    logical: String,
    member: String,
}
pub struct CommittedArchiveSource {
    archives: &'static Mutex<Vec<MpqArchive>>,
    revision: &'static Mutex<u32>,
    selected: ArchiveSelection,
    info: SourceInfo,
    members: BTreeMap<AssetPath, Mapping>,
    budget: ExtractionBudget,
}
fn invalid(detail: impl std::fmt::Display) -> SourceError {
    SourceError::InvalidInput {
        detail: detail.to_string(),
    }
}
impl CommittedArchiveSource {
    /// The owner must bump revision when replacing/removing an archive. Existing
    /// browser commit/drop do; legacy append does not attest inventory generation.
    /// SourceInfo.version is caller-declared, including explicit None for unknown.
    pub fn mount_no_huffman(
        archives: &'static Mutex<Vec<MpqArchive>>,
        revision: &'static Mutex<u32>,
        selected: ArchiveSelection,
        info: SourceInfo,
        names: Vec<NamedMember>,
        budget: ExtractionBudget,
    ) -> Result<Self, SourceError> {
        if info.kind != LayerKind::Mpq
            || info.origin != selected.archive_name
            || info.id.trim().is_empty()
            || info.origin.trim().is_empty()
            || info.version.as_ref().is_some_and(|v| v.trim().is_empty())
        {
            return Err(invalid(
                "supplied MPQ source identity/version must match the explicitly selected archive",
            ));
        }
        let mut members = BTreeMap::new();
        for name in names {
            let key = AssetPath::new(&name.asset).map_err(invalid)?;
            if name.member.is_empty() || name.member.chars().any(char::is_control) {
                return Err(invalid("empty or control-bearing exact member locator"));
            }
            if members
                .insert(
                    key,
                    Mapping {
                        logical: name.asset,
                        member: name.member,
                    },
                )
                .is_some()
            {
                return Err(SourceError::Ambiguous {
                    detail: "logical asset inventory repeats a canonical name".into(),
                });
            }
        }
        let source = Self {
            archives,
            revision,
            selected,
            info,
            members,
            budget,
        };
        source.validate()?;
        Ok(source)
    }
    fn locked(
        &self,
    ) -> Result<
        (
            MutexGuard<'static, u32>,
            MutexGuard<'static, Vec<MpqArchive>>,
        ),
        SourceError,
    > {
        let revision = self.revision.lock().map_err(|_| SourceError::Unavailable {
            detail: "committed browser revision lock is poisoned".into(),
        })?;
        let archives = self.archives.lock().map_err(|_| SourceError::Unavailable {
            detail: "committed browser archive lock is poisoned".into(),
        })?;
        self.check(*revision, &archives)?;
        Ok((revision, archives))
    }
    fn check(&self, revision: u32, archives: &[MpqArchive]) -> Result<(), SourceError> {
        if revision != self.selected.committed_revision {
            return Err(SourceError::Unavailable {
                detail: format!(
                    "selected archive {} revision {} is stale; current revision {revision}",
                    self.selected.index, self.selected.committed_revision
                ),
            });
        }
        let archive =
            archives
                .get(self.selected.index)
                .ok_or_else(|| SourceError::Unavailable {
                    detail: format!(
                        "selected archive index {} is absent at revision {revision}",
                        self.selected.index
                    ),
                })?;
        if archive.path().to_str() != Some(self.selected.archive_name.as_str())
            || archive.byte_len() != self.selected.archive_bytes
            || archive.header != self.selected.header
            || archive.hash_entries() != self.selected.hash_entries
            || archive.block_entries() != self.selected.block_entries
        {
            return Err(SourceError::Unavailable {
                detail: format!(
                    "selected archive {} identity differs at revision {revision}; expected {:?}",
                    self.selected.index, self.selected.archive_name
                ),
            });
        }
        Ok(())
    }
    fn validate(&self) -> Result<(), SourceError> {
        let _held = self.locked()?;
        Ok(())
    }
}
impl ContentSource for CommittedArchiveSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        let _held = self.locked()?;
        Ok(self.members.values().map(|m| m.logical.clone()).collect())
    }
    fn read(&self, logical: &str) -> Result<Option<SourceContent>, SourceError> {
        self.read_bounded(logical, self.budget.max_decoded_bytes as u64)
    }
    fn read_bounded(
        &self,
        logical: &str,
        limit: u64,
    ) -> Result<Option<SourceContent>, SourceError> {
        let (_revision, mut archives) = self.locked()?;
        let key = AssetPath::new(logical).map_err(invalid)?;
        let Some(mapping) = self.members.get(&key) else {
            return Ok(None);
        };
        let archive = &mut archives[self.selected.index];
        let (name1, name2) = pcx_mpq::hash::hash_string_both(&mapping.member);
        if !archive
            .hash_entries()
            .iter()
            .any(|e| !e.is_unpopulated() && e.name1 == name1 && e.name2 == name2)
        {
            return Ok(None);
        };
        let budget = ExtractionBudget {
            max_raw_packed_bytes: self.budget.max_raw_packed_bytes,
            max_decoded_bytes: self
                .budget
                .max_decoded_bytes
                .min(usize::try_from(limit).unwrap_or(usize::MAX)),
        };
        let bytes = archive
            .extract_bounded(&mapping.member, budget, None)
            .map_err(|source| SourceError::MpqExtraction(Box::new(source)))?;
        let declared_length = archive
            .find_block(&mapping.member)
            .ok_or_else(|| invalid("strictly decoded member no longer has a block"))?
            .file_size as u64;
        Ok(Some(SourceContent {
            bytes,
            declared_length,
            locator: mapping.member.clone(),
        }))
    }
}
