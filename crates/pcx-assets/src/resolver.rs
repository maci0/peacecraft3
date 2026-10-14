use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    ArchiveLayer, AssetPath, GameMode, LayerKind, OverrideDirectory, PathError, SourceContent,
    SourceError, SourceInfo,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("layer {layer:?} has an invalid path: {source}")]
    LayerPath {
        layer: String,
        #[source]
        source: PathError,
    },
    #[error("unknown layer kind {kind:?}")]
    UnknownLayerKind { kind: String },
    #[error("unknown layer {layer:?}")]
    UnknownLayer { layer: String },
    #[error("missing layer {layer:?} at {location:?}")]
    MissingLayer {
        layer: String,
        location: Option<PathBuf>,
    },
    #[error("no asset layers were supplied")]
    NoLayers,
    #[error("layer {layer:?} has missing or empty {field} provenance")]
    InvalidIdentity { layer: String, field: &'static str },
    #[error("archive layer {layer:?} must be MPQ or CASC, not an override")]
    InvalidArchiveKind { layer: String },
    #[error("layer id {layer:?} is ambiguous")]
    AmbiguousLayer { layer: String },
    #[error("archive priority {priority} is ambiguous between {layers:?}")]
    AmbiguousPriority { priority: u32, layers: [String; 2] },
    #[error("path {path} is ambiguous in layer {layer:?}: {candidates:?}")]
    AmbiguousPath {
        layer: String,
        path: AssetPath,
        candidates: Vec<String>,
    },
    #[error("override root {location:?} is not a directory")]
    InvalidOverrideRoot { location: PathBuf },
    #[error("path {location:?} cannot be represented losslessly as UTF-8")]
    PathEncoding { location: PathBuf },
    #[error("layer {layer:?} contains an unsafe/non-regular entry at {location:?}")]
    UnsafeEntry { layer: String, location: PathBuf },
    #[error("override path {location:?} aliases reserved index.json metadata")]
    ReservedPath { location: PathBuf },
    #[error("asset {path} is absent from every mounted layer")]
    MissingAsset { path: AssetPath },
    #[error("layer {layer:?} listed {path} but returned no content")]
    MissingContent { layer: String, path: AssetPath },
    #[error("layer {layer:?}, path {path:?}: {source}")]
    Source {
        layer: String,
        path: Option<AssetPath>,
        #[source]
        source: SourceError,
    },
    #[error(
        "layer {layer:?}, asset {path} is truncated: expected {expected}, received {actual} bytes"
    )]
    TruncatedInput {
        layer: String,
        path: AssetPath,
        expected: u64,
        actual: u64,
    },
    #[error(
        "layer {layer:?}, asset {path} has excess bytes: expected {expected}, received {actual}"
    )]
    LengthMismatch {
        layer: String,
        path: AssetPath,
        expected: u64,
        actual: u64,
    },
    #[error("layer {layer:?}, asset {path}: cannot allocate {length} bytes")]
    Allocation {
        layer: String,
        path: AssetPath,
        length: u64,
    },
    #[error("layer {layer:?}, location {location:?}: {source}")]
    Io {
        layer: String,
        location: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot encode asset index: {0}")]
    IndexJson(#[from] serde_json::Error),
}

/// The winning layer and exact locator supplied by it, not a guessed edition.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Provenance {
    pub source: SourceInfo,
    /// None for the overriding filesystem layer; otherwise caller-supplied.
    pub archive_priority: Option<u32>,
    pub locator: String,
    pub byte_length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAsset {
    pub path: AssetPath,
    pub bytes: Vec<u8>,
    pub provenance: Provenance,
}

/// Project-owned metadata format. This contains provenance, never asset bytes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AssetIndex {
    pub schema_version: u32,
    pub mode: String,
    /// Winning provenance for every path in the mounted inventories.
    pub assets: BTreeMap<AssetPath, Provenance>,
}

impl AssetIndex {
    pub fn to_json(&self) -> Result<String, Error> {
        let mut json = serde_json::to_string_pretty(self)?;
        json.push('\n');
        Ok(json)
    }
}

struct MountedArchive {
    priority: u32,
    info: SourceInfo,
    source: Box<dyn crate::ContentSource>,
    members: BTreeMap<AssetPath, String>,
}

/// Resolution is override first, then caller-defined descending archive priority.
/// Mode is recorded, never used to invent an archive or gameplay-data policy.
pub struct Resolver {
    mode: GameMode,
    overrides: Option<OverrideDirectory>,
    archives: Vec<MountedArchive>,
    paths: BTreeSet<AssetPath>,
}

impl Resolver {
    pub fn new(
        mode: GameMode,
        overrides: Option<OverrideDirectory>,
        mut archives: Vec<ArchiveLayer>,
    ) -> Result<Self, Error> {
        if overrides.is_none() && archives.is_empty() {
            return Err(Error::NoLayers);
        }
        let mut identities = BTreeSet::new();
        if let Some(overrides) = &overrides {
            identities.insert(overrides.info.id.clone());
        }
        let mut priorities = BTreeMap::new();
        for archive in &archives {
            let info = archive.source.info();
            info.validate()?;
            if info.kind == LayerKind::Override {
                return Err(Error::InvalidArchiveKind {
                    layer: info.id.clone(),
                });
            }
            if !identities.insert(info.id.clone()) {
                return Err(Error::AmbiguousLayer {
                    layer: info.id.clone(),
                });
            }
            if let Some(previous) = priorities.insert(archive.priority, info.id.clone()) {
                return Err(Error::AmbiguousPriority {
                    priority: archive.priority,
                    layers: [previous, info.id.clone()],
                });
            }
        }
        archives.sort_by_key(|archive| std::cmp::Reverse(archive.priority));
        let mut mounted = Vec::new();
        let mut paths = BTreeSet::new();
        if let Some(overrides) = &overrides {
            paths.extend(overrides.paths().cloned());
        }
        for archive in archives {
            let info = archive.source.info().clone();
            let mut members = BTreeMap::new();
            let originals = archive.source.paths().map_err(|source| Error::Source {
                layer: info.id.clone(),
                path: None,
                source,
            })?;
            for member in originals {
                let path = AssetPath::new(&member).map_err(|source| Error::LayerPath {
                    layer: info.id.clone(),
                    source,
                })?;
                if let Some(previous) = members.insert(path.clone(), member.clone()) {
                    return Err(Error::AmbiguousPath {
                        layer: info.id.clone(),
                        path,
                        candidates: vec![previous, member],
                    });
                }
            }
            paths.extend(members.keys().cloned());
            mounted.push(MountedArchive {
                priority: archive.priority,
                info,
                source: archive.source,
                members,
            });
        }
        Ok(Self {
            mode,
            overrides,
            archives: mounted,
            paths,
        })
    }

    pub fn mode(&self) -> GameMode {
        self.mode
    }

    pub fn paths(&self) -> impl Iterator<Item = &AssetPath> {
        self.paths.iter()
    }

    pub fn layer_info(&self, id: &str) -> Result<&SourceInfo, Error> {
        if let Some(overrides) = &self.overrides
            && overrides.info.id == id
        {
            return Ok(&overrides.info);
        }
        self.archives
            .iter()
            .find(|archive| archive.info.id == id)
            .map(|archive| &archive.info)
            .ok_or_else(|| Error::UnknownLayer { layer: id.into() })
    }

    pub fn resolve(&self, value: &str) -> Result<ResolvedAsset, Error> {
        let path = AssetPath::new(value)?;
        if let Some(overrides) = &self.overrides
            && overrides.contains(&path)
        {
            let content = overrides.read(&path)?;
            return resolved(path, content, &overrides.info, None);
        }
        for archive in &self.archives {
            if let Some(member) = archive.members.get(&path) {
                let content = archive
                    .source
                    .read(member)
                    .map_err(|source| Error::Source {
                        layer: archive.info.id.clone(),
                        path: Some(path.clone()),
                        source,
                    })?
                    .ok_or_else(|| Error::MissingContent {
                        layer: archive.info.id.clone(),
                        path: path.clone(),
                    })?;
                return resolved(path, content, &archive.info, Some(archive.priority));
            }
        }
        Err(Error::MissingAsset { path })
    }

    /// Resolve and validate every inventoried path. Any failed winner aborts;
    /// an error is never hidden by falling back to a lower layer.
    pub fn index(&self) -> Result<AssetIndex, Error> {
        let mut assets = BTreeMap::new();
        for path in &self.paths {
            let asset = self.resolve(path.as_str())?;
            assets.insert(path.clone(), asset.provenance);
        }
        Ok(AssetIndex {
            schema_version: 1,
            mode: self.mode.to_string(),
            assets,
        })
    }

    /// Generate override/index.json, replacing the prior index only after all
    /// paths have resolved and a complete temporary file has been written.
    pub fn write_index(&self) -> Result<PathBuf, Error> {
        let overrides = self.overrides.as_ref().ok_or_else(|| Error::MissingLayer {
            layer: "override directory".into(),
            location: None,
        })?;
        overrides.check_root()?;
        let json = self.index()?.to_json()?;
        let destination = overrides.root.join("index.json");
        match fs::symlink_metadata(&destination) {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                return Err(Error::UnsafeEntry {
                    layer: overrides.info.id.clone(),
                    location: destination,
                });
            }
            Ok(_) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::Io {
                    layer: overrides.info.id.clone(),
                    location: destination,
                    source,
                });
            }
        }
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
        let temporary_path = overrides.root.join(format!(
            ".pcx-assets-index-{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let io_error = |location: &PathBuf, source| Error::Io {
            layer: overrides.info.id.clone(),
            location: location.clone(),
            source,
        };
        let temporary = {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)
                .map_err(|source| io_error(&temporary_path, source))?;
            let temporary = TempIndex(temporary_path);
            file.write_all(json.as_bytes())
                .map_err(|source| io_error(&temporary.0, source))?;
            file.sync_all()
                .map_err(|source| io_error(&temporary.0, source))?;
            temporary
        };
        fs::rename(&temporary.0, &destination).map_err(|source| io_error(&destination, source))?;
        Ok(destination)
    }
}

struct TempIndex(PathBuf);
impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn resolved(
    path: AssetPath,
    content: SourceContent,
    info: &SourceInfo,
    archive_priority: Option<u32>,
) -> Result<ResolvedAsset, Error> {
    check_length(
        &info.id,
        &path,
        content.declared_length,
        content.bytes.len() as u64,
    )?;
    if content.locator.trim().is_empty() {
        return Err(Error::InvalidIdentity {
            layer: info.id.clone(),
            field: "content locator",
        });
    }
    Ok(ResolvedAsset {
        path,
        bytes: content.bytes,
        provenance: Provenance {
            source: info.clone(),
            archive_priority,
            locator: content.locator,
            byte_length: content.declared_length,
        },
    })
}

pub(crate) fn check_length(
    layer: &str,
    path: &AssetPath,
    expected: u64,
    actual: u64,
) -> Result<(), Error> {
    if actual < expected {
        Err(Error::TruncatedInput {
            layer: layer.into(),
            path: path.clone(),
            expected,
            actual,
        })
    } else if actual > expected {
        Err(Error::LengthMismatch {
            layer: layer.into(),
            path: path.clone(),
            expected,
            actual,
        })
    } else {
        Ok(())
    }
}
