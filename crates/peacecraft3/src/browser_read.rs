//! Fallible browser-store reads. Limits are caller copy policy, not native rules.
use bevy::asset::io::memory::Dir;
use pcx_mpq::MpqArchive;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveOrigin {
    pub ordinal: usize,
    pub name: String,
}
impl std::fmt::Display for ArchiveOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "archive {} {:?}", self.ordinal, self.name)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub key: String,
    pub requested_path: String,
    pub revision: u32,
    pub byte_len: usize,
    /// None means the hydrated/served store did not retain source origins.
    /// An archive read retains every equal-content contributing archive.
    pub archives: Option<Vec<ArchiveOrigin>>,
}
#[derive(Debug)]
pub struct AssetRead {
    pub bytes: Vec<u8>,
    pub receipt: Receipt,
}
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("invalid browser asset path {path:?}")]
    InvalidPath { path: String },
    #[error(
        "{key:?} at browser revision {revision}: {byte_len} bytes exceed copy limit {max_bytes}"
    )]
    BudgetExceeded {
        key: String,
        revision: u32,
        byte_len: usize,
        max_bytes: usize,
    },
    #[error(
        "{key:?} at browser revision {revision}: uncached archive read has no bounded extraction contract supplied to this cached-only API ({candidates:?})"
    )]
    ArchiveBudgetUnsupported {
        key: String,
        revision: u32,
        candidates: Vec<ArchiveOrigin>,
    },
    #[error(
        "{key:?} in {origin}: present hash entry cannot be selected by the existing archive reader"
    )]
    InvalidSelection { key: String, origin: ArchiveOrigin },
    #[error("{key:?} in {origin}: extraction failed: {source}")]
    Extraction {
        key: String,
        origin: ArchiveOrigin,
        #[source]
        source: Box<pcx_mpq::Error>,
    },
    #[error(
        "{key:?} at browser revision {revision}: bounded extraction failed in {origin}: {source}"
    )]
    BoundedExtraction {
        requested_path: String,
        key: String,
        revision: u32,
        origin: ArchiveOrigin,
        matching_archives: Vec<ArchiveOrigin>,
        #[source]
        source: Box<pcx_mpq::ExtractionError>,
    },
    #[error(
        "AMBIGUOUS_MEMBER: {key:?} at browser revision {revision}: unequal bytes in archives {first} and {other}; source policy is unresolved"
    )]
    BoundedAmbiguous {
        requested_path: String,
        key: String,
        revision: u32,
        matching_archives: Vec<ArchiveOrigin>,
        first: usize,
        other: usize,
    },
    #[error(
        "AMBIGUOUS_MEMBER: {key:?}: unequal archive content from {first} and {other}; source policy is unresolved"
    )]
    Ambiguous {
        key: String,
        first: ArchiveOrigin,
        other: ArchiveOrigin,
    },
}

/// A known panel-audio codec gap from one unambiguous bounded archive source.
/// Other matching sources remain unverified when extraction stops at an error.
pub fn panel_codec_mask(error: &ReadError) -> Option<u8> {
    let ReadError::BoundedExtraction {
        matching_archives,
        source,
        ..
    } = error
    else {
        return None;
    };
    if matching_archives.len() != 1 {
        return None;
    }
    match source.cause.as_ref() {
        pcx_mpq::ExtractionCause::Mpq(pcx_mpq::Error::Payload(
            pcx_mpq::payload::PayloadError::UnsupportedCompression {
                mask: mask @ (0x01 | 0x40 | 0x41 | 0x80 | 0x81),
            },
        )) => Some(*mask),
        _ => None,
    }
}

#[cfg(test)]
mod panel_codec_tests {
    use super::*;

    fn bounded(cause: pcx_mpq::ExtractionCause, sources: usize) -> ReadError {
        let origin = ArchiveOrigin {
            ordinal: 0,
            name: "synthetic local archive".into(),
        };
        ReadError::BoundedExtraction {
            requested_path: "Synthetic\\Panel.wav".into(),
            key: "synthetic/panel.wav".into(),
            revision: 7,
            origin: origin.clone(),
            matching_archives: (0..sources)
                .map(|ordinal| ArchiveOrigin {
                    ordinal,
                    name: format!("synthetic source {ordinal}"),
                })
                .collect(),
            source: Box::new(pcx_mpq::ExtractionError {
                archive: origin.name.into(),
                member: "Synthetic\\Panel.wav".into(),
                cause: Box::new(cause),
            }),
        }
    }

    fn codec(mask: u8) -> pcx_mpq::ExtractionCause {
        pcx_mpq::ExtractionCause::Mpq(pcx_mpq::Error::Payload(
            pcx_mpq::payload::PayloadError::UnsupportedCompression { mask },
        ))
    }

    #[test]
    fn only_known_single_source_codec_capability_may_omit_a_sound() {
        for mask in [0x01, 0x40, 0x41, 0x80, 0x81] {
            assert_eq!(panel_codec_mask(&bounded(codec(mask), 1)), Some(mask));
            for sources in [0, 2] {
                assert_eq!(panel_codec_mask(&bounded(codec(mask), sources)), None);
            }
        }
        for mask in [0, 0x04, 0x05, 0x82, 0xff] {
            assert_eq!(panel_codec_mask(&bounded(codec(mask), 1)), None);
        }
    }

    #[test]
    fn malformed_missing_ambiguous_and_unbounded_reads_are_not_codec_capability() {
        for cause in [
            pcx_mpq::ExtractionCause::Mpq(pcx_mpq::Error::NotFound("Synthetic.wav".into())),
            pcx_mpq::ExtractionCause::Ambiguous { count: 2 },
            pcx_mpq::ExtractionCause::Mpq(pcx_mpq::Error::Payload(
                pcx_mpq::payload::PayloadError::Huffman(pcx_mpq::huffman::HuffmanError::Corrupt),
            )),
        ] {
            assert_eq!(panel_codec_mask(&bounded(cause, 1)), None);
        }
        let unbounded = ReadError::Extraction {
            key: "Synthetic/Panel.wav".into(),
            origin: ArchiveOrigin {
                ordinal: 0,
                name: "synthetic".into(),
            },
            source: Box::new(pcx_mpq::Error::Payload(
                pcx_mpq::payload::PayloadError::UnsupportedCompression { mask: 0x81 },
            )),
        };
        assert_eq!(panel_codec_mask(&unbounded), None);
    }
}

/// Shares the existing browser key normalization; does not establish MPQ priority.
pub fn normalize(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for part in path.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." || part == ".." {
            continue;
        }
        if !out.is_empty() {
            out.push('/');
        }
        out.push_str(part);
    }
    out
}
fn key(path: &str) -> Result<String, ReadError> {
    let key = normalize(path);
    if key.is_empty() {
        return Err(ReadError::InvalidPath { path: path.into() });
    }
    Ok(key)
}
fn check(key: &str, revision: u32, byte_len: usize, max_bytes: usize) -> Result<(), ReadError> {
    if byte_len > max_bytes {
        return Err(ReadError::BudgetExceeded {
            key: key.into(),
            revision,
            byte_len,
            max_bytes,
        });
    }
    Ok(())
}
fn contains(archive: &MpqArchive, key: &str) -> bool {
    let (name1, name2) = pcx_mpq::hash::hash_string_both(&key.replace('/', "\\"));
    archive
        .hash_entries()
        .iter()
        .any(|entry| !entry.is_unpopulated() && entry.name1 == name1 && entry.name2 == name2)
}
fn origin(ordinal: usize, archive: &MpqArchive) -> ArchiveOrigin {
    ArchiveOrigin {
        ordinal,
        name: archive.path().display().to_string(),
    }
}

/// Checks the borrowed stored slice length before its first Vec copy.
/// A missing cached path is absent only when no committed archive has its hash.
/// This cached-only API refuses present uncached members. Use read_bounded with
/// an explicit archive policy to decode them.
/// The revision is the caller's committed import generation, not a full snapshot
/// guarantee: legacy add_asset/add_archive do not increment that generation.
pub fn read_checked(
    store: &Dir,
    archives: &[MpqArchive],
    revision: u32,
    path: &str,
    max_bytes: usize,
) -> Result<Option<AssetRead>, ReadError> {
    let key = key(path)?;
    if let Some(data) = store.get_asset(Path::new(&key)) {
        let borrowed = data.value();
        check(&key, revision, borrowed.len(), max_bytes)?;
        return Ok(Some(AssetRead {
            bytes: borrowed.to_vec(),
            receipt: Receipt {
                key,
                requested_path: path.into(),
                revision,
                byte_len: borrowed.len(),
                archives: None,
            },
        }));
    }
    let candidates = archives
        .iter()
        .enumerate()
        .filter(|(_, archive)| contains(archive, &key))
        .map(|(ordinal, archive)| origin(ordinal, archive))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        Ok(None)
    } else {
        Err(ReadError::ArchiveBudgetUnsupported {
            key,
            revision,
            candidates,
        })
    }
}

/// Compatible fallible archive decode used by existing hydration/read callers.
/// This preserves decoder errors and equal-source receipts, but file_size alone
/// is not a packed allocation/work budget. It is deliberately not read_checked.
pub fn extract_fallible(
    archives: &mut [MpqArchive],
    revision: u32,
    path: &str,
    max_decoded_bytes: usize,
) -> Result<Option<AssetRead>, ReadError> {
    let key = key(path)?;
    let member = key.replace('/', "\\");
    let mut found: Option<AssetRead> = None;
    for (ordinal, archive) in archives.iter_mut().enumerate() {
        if !contains(archive, &key) {
            continue;
        }
        let source = origin(ordinal, archive);
        let block = archive
            .find_block(&member)
            .ok_or_else(|| ReadError::InvalidSelection {
                key: key.clone(),
                origin: source.clone(),
            })?;
        check(&key, revision, block.file_size as usize, max_decoded_bytes)?;
        let bytes = archive
            .extract(&member)
            .map_err(|cause| ReadError::Extraction {
                key: key.clone(),
                origin: source.clone(),
                source: Box::new(cause),
            })?;
        check(&key, revision, bytes.len(), max_decoded_bytes)?;
        if let Some(prior) = &mut found {
            if prior.bytes != bytes {
                return Err(ReadError::Ambiguous {
                    key,
                    first: prior.receipt.archives.as_ref().expect("archive receipt")[0].clone(),
                    other: source,
                });
            }
            prior
                .receipt
                .archives
                .as_mut()
                .expect("archive receipt")
                .push(source);
        } else {
            found = Some(AssetRead {
                receipt: Receipt {
                    key: key.clone(),
                    requested_path: path.into(),
                    revision,
                    byte_len: bytes.len(),
                    archives: Some(vec![source]),
                },
                bytes,
            });
        }
    }
    Ok(found)
}

/// No default limits or codec. The copy limit intersects the extraction decoded
/// limit before payload allocation; archive backing/tables were already imported.
#[derive(Clone, Copy)]
pub struct BoundedReadPolicy<'a> {
    pub max_copy_bytes: usize,
    pub extraction: pcx_mpq::ExtractionBudget,
    pub huffman: Option<&'a pcx_mpq::huffman::HuffmanTable>,
}
/// Configured Rust caller route; cached bytes keep their unavailable-origin state.
/// Success preserves every equal-content contributing archive. Errors preserve
/// all matching archive labels, without choosing a winner or discovering codecs.
pub fn read_bounded(
    store: &Dir,
    archives: &mut [MpqArchive],
    revision: u32,
    path: &str,
    policy: BoundedReadPolicy<'_>,
) -> Result<Option<AssetRead>, ReadError> {
    if let Some(asset) = read_checked(store, &[], revision, path, policy.max_copy_bytes)? {
        return Ok(Some(asset));
    }
    let key = key(path)?;
    let member = key.replace('/', "\\");
    let matching_archives = archives
        .iter()
        .enumerate()
        .filter(|(_, archive)| contains(archive, &key))
        .map(|(ordinal, archive)| origin(ordinal, archive))
        .collect::<Vec<_>>();
    let budget = pcx_mpq::ExtractionBudget {
        max_raw_packed_bytes: policy.extraction.max_raw_packed_bytes,
        max_decoded_bytes: policy
            .extraction
            .max_decoded_bytes
            .min(policy.max_copy_bytes),
    };
    let mut found: Option<AssetRead> = None;
    for (ordinal, archive) in archives.iter_mut().enumerate() {
        if !contains(archive, &key) {
            continue;
        }
        let origin = origin(ordinal, archive);
        // Archive tables come from the staged codec source at commit. A per-call table
        // in the policy takes precedence; no executable is discovered here.
        let imported = archive.codec_table();
        let codec = policy.huffman.or(imported.as_deref());
        let bytes = match archive.extract_bounded(&member, budget, codec) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Err(ReadError::BoundedExtraction {
                    requested_path: path.into(),
                    key,
                    revision,
                    origin,
                    matching_archives,
                    source: Box::new(error),
                });
            }
        };
        check(&key, revision, bytes.len(), policy.max_copy_bytes)?;
        if let Some(prior) = &mut found {
            if prior.bytes != bytes {
                return Err(ReadError::BoundedAmbiguous {
                    requested_path: path.into(),
                    key,
                    revision,
                    first: prior.receipt.archives.as_ref().expect("archive receipt")[0].ordinal,
                    other: ordinal,
                    matching_archives,
                });
            }
            prior
                .receipt
                .archives
                .as_mut()
                .expect("archive receipt")
                .push(origin);
        } else {
            found = Some(AssetRead {
                receipt: Receipt {
                    requested_path: path.into(),
                    key: key.clone(),
                    revision,
                    byte_len: bytes.len(),
                    archives: Some(vec![origin]),
                },
                bytes,
            });
        }
    }
    Ok(found)
}
