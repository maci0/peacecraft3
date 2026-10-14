use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{AssetPath, Error};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerKind {
    Override,
    Mpq,
    Casc,
}

impl FromStr for LayerKind {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "override" => Ok(Self::Override),
            "mpq" => Ok(Self::Mpq),
            "casc" => Ok(Self::Casc),
            _ => Err(Error::UnknownLayerKind { kind: value.into() }),
        }
    }
}

/// Adapter-supplied identity, not a version inferred from a directory or mode.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SourceInfo {
    pub id: String,
    pub kind: LayerKind,
    /// Archive/install identity or the canonical override directory.
    pub origin: String,
    /// Exact detected version/source label, or explicitly unknown/unversioned.
    pub version: Option<String>,
}

impl SourceInfo {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        for (field, value) in [("id", self.id.as_str()), ("origin", self.origin.as_str())] {
            if value.trim().is_empty() {
                return Err(Error::InvalidIdentity {
                    layer: self.id.clone(),
                    field,
                });
            }
        }
        if self
            .version
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(Error::InvalidIdentity {
                layer: self.id.clone(),
                field: "version",
            });
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source layer is unavailable: {detail}")]
    Unavailable { detail: String },
    #[error("source input is malformed or unsupported: {detail}")]
    InvalidInput { detail: String },
    #[error("source input is truncated: expected {expected} bytes, received {actual}")]
    Truncated { expected: u64, actual: u64 },
    #[error("source member is ambiguous: {detail}")]
    Ambiguous { detail: String },
    #[error("source member requires {requested} decoded bytes, exceeding caller limit {limit}")]
    Limit { requested: u64, limit: u64 },
    #[error("source cannot enforce caller read budget {limit} before payload allocation/decode")]
    BudgetUnsupported { limit: u64 },
    #[error(transparent)]
    MpqExtraction(Box<pcx_mpq::ExtractionError>),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Complete decoded bytes plus the source-declared size and exact member locator.
/// The resolver validates length rather than trusting an incomplete adapter read.
#[derive(Debug, Clone)]
pub struct SourceContent {
    pub bytes: Vec<u8>,
    pub declared_length: u64,
    pub locator: String,
}

/// Read-only archive boundary. Each instance represents one explicitly ordered
/// layer. `paths` returns the mounted logical names; `read` receives those names
/// verbatim. An adapter retains the exact archive member behind each logical name.
/// The mounted inventory may be an explicitly declared subset. An unavailable
/// inventory must return an error, not silently claim an empty archive.
pub trait ContentSource: Send + Sync {
    fn info(&self) -> &SourceInfo;
    fn paths(&self) -> Result<Vec<String>, SourceError>;
    fn read(&self, member: &str) -> Result<Option<SourceContent>, SourceError>;
    /// Refuse known oversized payloads before allocating/decoding them.
    /// Implementations must enforce the ceiling through their payload reader.
    /// There is no allocating fallback to read for adapters without this contract.
    fn read_bounded(
        &self,
        _member: &str,
        limit: u64,
    ) -> Result<Option<SourceContent>, SourceError> {
        Err(SourceError::BudgetUnsupported { limit })
    }
}

pub struct ArchiveLayer {
    /// Higher numbers win. No ordering is inferred from filenames or asset mode.
    pub priority: u32,
    pub source: Box<dyn ContentSource>,
}

struct OverrideEntry {
    file: PathBuf,
    length: u64,
}

/// A real, recursively scanned directory tree. Paths are indexed at mount time;
/// remount to discover new names. Existing files are read on demand, not converted.
/// Root `index.json` and `.pcx-assets-index-*` are reserved generated metadata.
/// Symlinks and non-regular entries below the selected root are refused.
pub struct OverrideDirectory {
    pub(crate) root: PathBuf,
    pub(crate) info: SourceInfo,
    entries: BTreeMap<AssetPath, OverrideEntry>,
}

impl OverrideDirectory {
    pub fn open(
        root: impl AsRef<Path>,
        id: impl Into<String>,
        version: Option<String>,
    ) -> Result<Self, Error> {
        let id = id.into();
        let configured_root = root.as_ref();
        let root = fs::canonicalize(configured_root).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                Error::MissingLayer {
                    layer: id.clone(),
                    location: Some(configured_root.into()),
                }
            } else {
                Error::Io {
                    layer: id.clone(),
                    location: configured_root.into(),
                    source,
                }
            }
        })?;
        let metadata = fs::metadata(&root).map_err(|source| Error::Io {
            layer: id.clone(),
            location: root.clone(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(Error::InvalidOverrideRoot { location: root });
        }
        let origin = root
            .to_str()
            .ok_or_else(|| Error::PathEncoding {
                location: root.clone(),
            })?
            .to_owned();
        let info = SourceInfo {
            id,
            kind: LayerKind::Override,
            origin,
            version,
        };
        info.validate()?;
        let mut this = Self {
            root: root.clone(),
            info,
            entries: BTreeMap::new(),
        };
        let mut directories = vec![root];
        while let Some(directory) = directories.pop() {
            let entries =
                fs::read_dir(&directory).map_err(|source| this.io_error(&directory, source))?;
            for entry in entries {
                let entry = entry.map_err(|source| this.io_error(&directory, source))?;
                let file = entry.path();
                let metadata =
                    fs::symlink_metadata(&file).map_err(|source| this.io_error(&file, source))?;
                if metadata.file_type().is_symlink() || !(metadata.is_dir() || metadata.is_file()) {
                    return Err(Error::UnsafeEntry {
                        layer: this.info.id.clone(),
                        location: file,
                    });
                }
                let relative = file
                    .strip_prefix(&this.root)
                    .map_err(|_| Error::UnsafeEntry {
                        layer: this.info.id.clone(),
                        location: file.clone(),
                    })?;
                let original = relative.to_str().ok_or_else(|| Error::PathEncoding {
                    location: file.clone(),
                })?;
                if relative.components().count() == 1
                    && (original == "index.json" || original.starts_with(".pcx-assets-index-"))
                {
                    if !metadata.is_file() {
                        return Err(Error::UnsafeEntry {
                            layer: this.info.id.clone(),
                            location: file,
                        });
                    }
                    continue;
                }
                let path = AssetPath::new(original).map_err(|source| Error::LayerPath {
                    layer: this.info.id.clone(),
                    source,
                })?;
                if path.as_str() == "index.json" {
                    return Err(Error::ReservedPath { location: file });
                }
                if metadata.is_dir() {
                    directories.push(file);
                } else {
                    if let Some(previous) = this.entries.get(&path) {
                        return Err(Error::AmbiguousPath {
                            layer: this.info.id.clone(),
                            path,
                            candidates: vec![
                                previous.file.display().to_string(),
                                file.display().to_string(),
                            ],
                        });
                    }
                    this.entries.insert(
                        path,
                        OverrideEntry {
                            file,
                            length: metadata.len(),
                        },
                    );
                }
            }
        }
        Ok(this)
    }

    pub fn info(&self) -> &SourceInfo {
        &self.info
    }

    pub fn paths(&self) -> impl Iterator<Item = &AssetPath> {
        self.entries.keys()
    }

    pub(crate) fn contains(&self, path: &AssetPath) -> bool {
        self.entries.contains_key(path)
    }

    pub(crate) fn read(&self, path: &AssetPath) -> Result<SourceContent, Error> {
        self.read_with_budget(path, None)
    }
    pub(crate) fn read_bounded(
        &self,
        path: &AssetPath,
        limit: u64,
    ) -> Result<SourceContent, Error> {
        self.read_with_budget(path, Some(limit))
    }
    fn read_with_budget(
        &self,
        path: &AssetPath,
        limit: Option<u64>,
    ) -> Result<SourceContent, Error> {
        self.check_root()?;
        let entry = self
            .entries
            .get(path)
            .ok_or_else(|| Error::MissingContent {
                layer: self.info.id.clone(),
                path: path.clone(),
            })?;
        // Recheck ancestors so a replaced directory cannot silently redirect a read.
        let relative = entry
            .file
            .strip_prefix(&self.root)
            .map_err(|_| Error::UnsafeEntry {
                layer: self.info.id.clone(),
                location: entry.file.clone(),
            })?;
        let mut location = self.root.clone();
        for component in relative.components() {
            location.push(component);
            let metadata = fs::symlink_metadata(&location)
                .map_err(|source| self.io_error(&location, source))?;
            let valid = if location == entry.file {
                metadata.is_file()
            } else {
                metadata.is_dir()
            };
            if metadata.file_type().is_symlink() || !valid {
                return Err(Error::UnsafeEntry {
                    layer: self.info.id.clone(),
                    location,
                });
            }
        }
        let file = File::open(&entry.file).map_err(|source| self.io_error(&entry.file, source))?;
        let metadata = file
            .metadata()
            .map_err(|source| self.io_error(&entry.file, source))?;
        if !metadata.is_file() {
            return Err(Error::UnsafeEntry {
                layer: self.info.id.clone(),
                location: entry.file.clone(),
            });
        }
        crate::resolver::check_length(&self.info.id, path, entry.length, metadata.len())?;
        if let Some(limit) = limit
            && entry.length > limit
        {
            return Err(Error::Source {
                layer: self.info.id.clone(),
                path: Some(path.clone()),
                source: SourceError::Limit {
                    requested: entry.length,
                    limit,
                },
            });
        }
        let capacity = entry
            .length
            .checked_add(1)
            .and_then(|length| usize::try_from(length).ok())
            .ok_or_else(|| Error::Allocation {
                layer: self.info.id.clone(),
                path: path.clone(),
                length: entry.length,
            })?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| Error::Allocation {
                layer: self.info.id.clone(),
                path: path.clone(),
                length: entry.length,
            })?;
        file.take(capacity as u64)
            .read_to_end(&mut bytes)
            .map_err(|source| self.io_error(&entry.file, source))?;
        crate::resolver::check_length(&self.info.id, path, entry.length, bytes.len() as u64)?;
        Ok(SourceContent {
            bytes,
            declared_length: entry.length,
            locator: entry
                .file
                .to_str()
                .ok_or_else(|| Error::PathEncoding {
                    location: entry.file.clone(),
                })?
                .into(),
        })
    }

    fn io_error(&self, location: &Path, source: std::io::Error) -> Error {
        Error::Io {
            layer: self.info.id.clone(),
            location: location.into(),
            source,
        }
    }

    pub(crate) fn check_root(&self) -> Result<(), Error> {
        let metadata =
            fs::symlink_metadata(&self.root).map_err(|source| self.io_error(&self.root, source))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(Error::UnsafeEntry {
                layer: self.info.id.clone(),
                location: self.root.clone(),
            });
        }
        Ok(())
    }
}
