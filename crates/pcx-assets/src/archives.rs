//! Concrete checked readers, with no implicit inventory or module selection.
use std::{collections::BTreeMap, sync::Mutex};

use crate::{AssetPath, ContentSource, LayerKind, SourceContent, SourceError, SourceInfo};

/// Caller-selected logical identity and exact archive/TVFS member.
/// This declares a mounted subset, not the archive's complete inventory.
pub struct NamedMember {
    pub asset: String,
    pub member: String,
}

struct Mapping {
    asset: String,
    member: String,
}

fn mappings(names: Vec<NamedMember>) -> Result<BTreeMap<AssetPath, Mapping>, SourceError> {
    let mut result = BTreeMap::new();
    for name in names {
        let path = AssetPath::new(&name.asset).map_err(invalid)?;
        if name.member.is_empty() || name.member.chars().any(char::is_control) {
            return Err(invalid("empty or control-bearing archive member"));
        }
        if let Some(previous) = result.insert(
            path.clone(),
            Mapping {
                asset: name.asset,
                member: name.member,
            },
        ) {
            return Err(SourceError::Ambiguous {
                detail: format!(
                    "logical asset {path} is mapped more than once (including {:?})",
                    previous.member
                ),
            });
        }
    }
    Ok(result)
}

fn invalid(error: impl std::fmt::Display) -> SourceError {
    SourceError::InvalidInput {
        detail: error.to_string(),
    }
}

/// One MPQ archive, with an explicit named subset, caller-supplied version label
/// and decoded member budget. Multiple locale/platform matches are refused.
/// Mutex serialization permits the existing seek-based reader behind ContentSource.
pub struct MpqSource {
    info: SourceInfo,
    archive: Mutex<pcx_mpq::MpqArchive>,
    members: BTreeMap<AssetPath, Mapping>,
    extraction: Option<(
        pcx_mpq::ExtractionBudget,
        Option<pcx_mpq::huffman::HuffmanTable>,
    )>,
}

impl MpqSource {
    pub fn new(
        archive: pcx_mpq::MpqArchive,
        id: String,
        version: Option<String>,
        names: Vec<NamedMember>,
        decoded_byte_limit: u64,
    ) -> Result<Self, SourceError> {
        let info = SourceInfo {
            id,
            kind: LayerKind::Mpq,
            origin: archive
                .path()
                .to_str()
                .ok_or_else(|| invalid("MPQ origin is not UTF-8"))?
                .into(),
            version,
        };
        info.validate().map_err(invalid)?;
        let members = mappings(names)?;
        for mapping in members.values() {
            let hash = pcx_mpq::hash::hash_string(&mapping.member);
            let candidates = archive
                .hash_entries()
                .iter()
                .filter(|entry| {
                    entry.name1 == hash.name1
                        && entry.name2 == hash.name2
                        && archive
                            .block(entry.block_index as usize)
                            .is_ok_and(|block| block.exists())
                })
                .collect::<Vec<_>>();
            if candidates.len() > 1 {
                return Err(SourceError::Ambiguous {
                    detail: format!(
                        "MPQ member {:?} has multiple locale/platform candidates: {candidates:?}; no locale selection policy supplied",
                        mapping.member
                    ),
                });
            }
            if !archive.contains(&mapping.member) {
                return Err(invalid(format!(
                    "declared MPQ member {:?} is absent from {}",
                    mapping.member, info.origin
                )));
            }
            let requested = archive
                .find_block(&mapping.member)
                .ok_or_else(|| invalid("declared MPQ block is unavailable"))?
                .file_size as u64;
            if requested > decoded_byte_limit {
                return Err(SourceError::Limit {
                    requested,
                    limit: decoded_byte_limit,
                });
            }
        }
        Ok(Self {
            info,
            archive: Mutex::new(archive),
            members,
            extraction: None,
        })
    }
    /// Add explicit packed/decoded policy and caller-owned codec input.
    pub fn new_bounded(
        archive: pcx_mpq::MpqArchive,
        id: String,
        version: Option<String>,
        names: Vec<NamedMember>,
        budget: pcx_mpq::ExtractionBudget,
        huffman: Option<pcx_mpq::huffman::HuffmanTable>,
    ) -> Result<Self, SourceError> {
        let mut source = Self::new(archive, id, version, names, budget.max_decoded_bytes as u64)?;
        source.extraction = Some((budget, huffman));
        Ok(source)
    }
}

impl ContentSource for MpqSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        Ok(self.members.values().map(|m| m.asset.clone()).collect())
    }
    fn read(&self, asset: &str) -> Result<Option<SourceContent>, SourceError> {
        let path = AssetPath::new(asset).map_err(invalid)?;
        let Some(mapping) = self.members.get(&path) else {
            return Ok(None);
        };
        let mut archive = self.archive.lock().map_err(|_| SourceError::Unavailable {
            detail: "MPQ reader lock is poisoned".into(),
        })?;
        let declared_length = archive
            .find_block(&mapping.member)
            .ok_or_else(|| invalid("declared MPQ block disappeared"))?
            .file_size as u64;
        let bytes = archive.extract(&mapping.member).map_err(invalid)?;
        Ok(Some(SourceContent {
            bytes,
            declared_length,
            locator: mapping.member.clone(),
        }))
    }
    fn read_bounded(&self, asset: &str, limit: u64) -> Result<Option<SourceContent>, SourceError> {
        let Some((mut budget, huffman)) = self.extraction.as_ref().map(|(b, h)| (*b, h)) else {
            return Err(SourceError::BudgetUnsupported { limit });
        };
        let path = AssetPath::new(asset).map_err(invalid)?;
        let Some(mapping) = self.members.get(&path) else {
            return Ok(None);
        };
        budget.max_decoded_bytes = limit.min(budget.max_decoded_bytes as u64) as usize;
        let mut archive = self.archive.lock().map_err(|_| SourceError::Unavailable {
            detail: "MPQ reader lock is poisoned".into(),
        })?;
        // A per-source table wins; otherwise the codec source imported with the archive.
        let imported = archive.codec_table();
        let codec = huffman.as_ref().or(imported.as_deref());
        let bytes = archive
            .extract_bounded(&mapping.member, budget, codec)
            .map_err(|source| SourceError::MpqExtraction(Box::new(source)))?;
        let declared_length = archive
            .find_block(&mapping.member)
            .ok_or_else(|| invalid("declared MPQ block disappeared"))?
            .file_size as u64;
        Ok(Some(SourceContent {
            declared_length,
            bytes,
            locator: mapping.member.clone(),
        }))
    }
}

/// One verified CASC build. Every logical asset maps to an explicit stored TVFS
/// namespace; no basename, HD, module, locale or mode lookup policy is supplied.
pub struct CascSource {
    info: SourceInfo,
    storage: pcx_casc::storage::Storage,
    resolver: pcx_casc::tvfs::Resolver,
    members: BTreeMap<AssetPath, Mapping>,
    identities: BTreeMap<AssetPath, pcx_casc::tvfs::ResolvedPath>,
}

impl CascSource {
    pub fn new(
        storage: pcx_casc::storage::Storage,
        resolver: pcx_casc::tvfs::Resolver,
        id: String,
        names: Vec<NamedMember>,
    ) -> Result<Self, SourceError> {
        if storage.build.config != resolver.build_config {
            return Err(invalid("TVFS resolver belongs to a different CASC build"));
        }
        let source = &storage.build.info.source;
        let info = SourceInfo {
            id,
            kind: LayerKind::Casc,
            origin: source.install.clone(),
            version: Some(source.version.clone()),
        };
        info.validate().map_err(invalid)?;
        let members = mappings(names)?;
        let mut identities = BTreeMap::new();
        for (path, mapping) in &members {
            identities.insert(
                path.clone(),
                resolver.resolve(&mapping.member).map_err(invalid)?,
            );
        }
        Ok(Self {
            info,
            storage,
            resolver,
            members,
            identities,
        })
    }

    /// Full build/config/encoding/manifest chain retained beside the mapped asset.
    pub fn identity(
        &self,
        asset: &str,
    ) -> Result<Option<&pcx_casc::tvfs::ResolvedPath>, SourceError> {
        Ok(self
            .identities
            .get(&AssetPath::new(asset).map_err(invalid)?))
    }
}

impl ContentSource for CascSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        Ok(self.members.values().map(|m| m.asset.clone()).collect())
    }
    fn read(&self, asset: &str) -> Result<Option<SourceContent>, SourceError> {
        self.read_with_limit(asset, None)
    }
    fn read_bounded(&self, asset: &str, limit: u64) -> Result<Option<SourceContent>, SourceError> {
        self.read_with_limit(asset, Some(limit))
    }
}

impl CascSource {
    fn read_with_limit(
        &self,
        asset: &str,
        limit: Option<u64>,
    ) -> Result<Option<SourceContent>, SourceError> {
        let path = AssetPath::new(asset).map_err(invalid)?;
        let Some(mapping) = self.members.get(&path) else {
            return Ok(None);
        };
        let file = match limit {
            Some(limit) => self
                .storage
                .read_path_bounded(&self.resolver, &mapping.member, limit)
                .map_err(|source| SourceError::CascRead(Box::new(source)))?,
            None => self
                .storage
                .read_path(&self.resolver, &mapping.member)
                .map_err(invalid)?,
        };
        let manifests = file
            .resolved
            .links
            .iter()
            .map(|link| {
                serde_json::json!({
                        "config_fields": link.config_fields,
                        "manifest": link.manifest.source.member,
                        "manifest_md5": pcx_casc::hex(&link.manifest.md5),
                "ekey": pcx_casc::hex(&link.ekey), "ckey": pcx_casc::hex(&link.ckey),
                "path_bytes": link.path, "node_offset": link.node_offset,
                "span": { "file_offset": link.span.file_offset, "size": link.span.size,
                    "vfs_offset": link.span.vfs_offset, "cft_offset": link.span.cft_offset,
                    "key_prefix": pcx_casc::hex(&link.span.key_prefix) }
                    })
            })
            .collect::<Vec<_>>();
        let locator = serde_json::json!({ "tvfs": mapping.member, "ekey": pcx_casc::hex(&file.resolved.ekey), "ckey": pcx_casc::hex(&file.resolved.ckey), "build_config": pcx_casc::hex(&file.resolved.build_config.md5), "encoding": pcx_casc::hex(&file.resolved.encoding.md5), "manifests": manifests, "archive": file.file.archive, "offset": file.file.archive_offset }).to_string();
        Ok(Some(SourceContent {
            bytes: file.file.decoded.bytes,
            declared_length: file.resolved.decoded_size,
            locator,
        }))
    }
}
