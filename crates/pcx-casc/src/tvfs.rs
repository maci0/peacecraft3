//! Checked TVFS v1 byte paths and explicit configured-manifest mounts.
//! CFT keys are joined to ENCODING; opaque CFT suffixes are never executed.
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use crate::encoding::Encoding;
use crate::storage::Storage;
use crate::{Error, Evidence, Failure, Key, Provenance, bytes, error, hex, integer, parse_key};

const FOLDER: u32 = 0x8000_0000;
const LOWERCASE: u32 = 8;
const WRITE: u32 = 2;

#[derive(Debug, Clone)]
pub struct Header {
    pub version: u8,
    pub header_size: u8,
    pub ekey_size: u8,
    pub patch_key_size: u8,
    pub flags: u32,
    pub max_depth: u16,
    pub path_table: Range<usize>,
    pub vfs_table: Range<usize>,
    pub cft_table: Range<usize>,
    pub espec_table: Option<Range<usize>>,
    pub cft_offset_width: usize,
}

#[derive(Debug, Clone)]
pub struct Span {
    pub file_offset: u32,
    pub size: u32,
    pub vfs_offset: usize,
    pub cft_offset: usize,
    pub key_prefix: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum FileRecord {
    Spans(Vec<Span>),
    Unsupported(Error),
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Authored byte identity. Non-UTF8 names are retained, never replaced.
    pub path: Vec<u8>,
    pub node_offset: usize,
    pub record: FileRecord,
}

#[derive(Debug)]
pub struct Manifest {
    pub evidence: Evidence,
    pub header: Header,
    pub entries: BTreeMap<Vec<u8>, Entry>,
    pub observed_depth: u16,
    data: Vec<u8>,
    cft_boundaries: BTreeSet<usize>,
}

struct Frame {
    cursor: usize,
    end: usize,
    prefix: Vec<u8>,
    saved: usize,
    depth: u16,
}

impl Manifest {
    pub fn parse(data: &[u8], source: Provenance, path_budget: u64) -> Result<Self, Error> {
        let name = &source.member;
        let base = bytes(data, 0, 38, name)?;
        if &base[..4] != b"TVFS" || base[4] != 1 {
            return Err(unsupported(
                name,
                0,
                "TVFS signature/version",
                format!("{} version {}", hex(&base[..4]), base[4]),
                "establish this manifest's version-specific header",
            ));
        }
        let flags = integer(&base[8..12], true) as u32;
        if flags & !0x0f != 0 || !matches!(base[6], 9 | 16) || base[7] != base[6] {
            return Err(unsupported(
                name,
                6,
                "TVFS flags/key widths",
                format!("flags={flags:#x} widths={}/{}", base[6], base[7]),
                "establish additional flag/key record framing",
            ));
        }
        let expected_header = if flags & WRITE != 0 { 46 } else { 38 };
        if usize::from(base[5]) != expected_header {
            return Err(unsupported(
                name,
                5,
                "TVFS header extension",
                base[5].to_string(),
                "identify all extension fields before interpreting table references",
            ));
        }
        let raw_header = bytes(data, 0, expected_header, name)?;
        let table = |offset: usize| -> Result<Range<usize>, Error> {
            let start = integer(bytes(raw_header, offset, 4, name)?, true) as usize;
            let size = integer(bytes(raw_header, offset + 4, 4, name)?, true) as usize;
            if start < expected_header {
                return Err(error(
                    name,
                    offset,
                    Failure::Malformed("TVFS table overlaps header".into()),
                ));
            }
            bytes(data, start, size, name)?;
            Ok(start..start + size)
        };
        let path_table = table(12)?;
        let vfs_table = table(20)?;
        let cft_table = table(28)?;
        let espec_table = if flags & WRITE != 0 {
            Some(table(38)?)
        } else {
            None
        };
        let mut ranges = vec![path_table.clone(), vfs_table.clone(), cft_table.clone()];
        ranges.extend(espec_table.iter().cloned());
        ranges.sort_by_key(|range| range.start);
        if ranges.windows(2).any(|pair| pair[0].end > pair[1].start) {
            return Err(error(
                name,
                12,
                Failure::Malformed("overlapping TVFS tables".into()),
            ));
        }
        let max_depth = integer(&base[36..38], true) as u16;
        let header = Header {
            version: base[4],
            header_size: base[5],
            ekey_size: base[6],
            patch_key_size: base[7],
            flags,
            max_depth,
            cft_offset_width: offset_width(cft_table.len()),
            path_table,
            vfs_table,
            cft_table,
            espec_table,
        };
        let mut entries = BTreeMap::new();
        let mut cft_boundaries = BTreeSet::new();
        let mut stack = vec![Frame {
            cursor: header.path_table.start,
            end: header.path_table.end,
            prefix: Vec::new(),
            saved: 0,
            depth: 0,
        }];
        let mut observed_depth = 0;
        let mut stored_path_bytes = 0u64;
        let mut stack_path_bytes = 0u64;
        while let Some(mut frame) = stack.pop() {
            stack_path_bytes -= frame.prefix.len() as u64;
            observed_depth = observed_depth.max(frame.depth);
            if frame.cursor == frame.end {
                if frame.prefix.len() != frame.saved {
                    return Err(error(
                        name,
                        frame.cursor,
                        Failure::Malformed("dangling TVFS path fragment".into()),
                    ));
                }
                continue;
            }
            let node_offset = frame.cursor;
            let scope = &data[..frame.end];
            if bytes(scope, frame.cursor, 1, name)?[0] == 0 {
                frame.prefix.push(b'/');
                frame.cursor += 1;
            }
            if bytes(scope, frame.cursor, 1, name)?[0] != 0xff {
                let length = usize::from(scope[frame.cursor]);
                frame.cursor += 1;
                frame
                    .prefix
                    .extend_from_slice(bytes(scope, frame.cursor, length, name)?);
                frame.cursor += length;
            }
            if frame.cursor < frame.end && scope[frame.cursor] == 0 {
                frame.prefix.push(b'/');
                frame.cursor += 1;
            }
            if frame.cursor < frame.end && scope[frame.cursor] == 0xff {
                let value = integer(bytes(scope, frame.cursor + 1, 4, name)?, true) as u32;
                frame.cursor += 5;
                if value & FOLDER != 0 {
                    let size = (value & !FOLDER) as usize;
                    let child_size = size.checked_sub(4).ok_or_else(|| {
                        error(
                            name,
                            node_offset,
                            Failure::Malformed(
                                "folder extent is shorter than its value word".into(),
                            ),
                        )
                    })?;
                    let child_end = frame
                        .cursor
                        .checked_add(child_size)
                        .filter(|&end| end <= frame.end)
                        .ok_or_else(|| {
                            error(
                                name,
                                node_offset,
                                Failure::Malformed("folder extent exceeds parent scope".into()),
                            )
                        })?;
                    let depth = frame
                        .depth
                        .checked_add(1)
                        .filter(|&depth| depth <= max_depth)
                        .ok_or_else(|| {
                            error(
                                name,
                                node_offset,
                                Failure::Malformed("tree exceeds authored maximum depth".into()),
                            )
                        })?;
                    let child = Frame {
                        cursor: frame.cursor,
                        end: child_end,
                        saved: frame.prefix.len(),
                        prefix: frame.prefix.clone(),
                        depth,
                    };
                    frame.cursor = child_end;
                    frame.prefix.truncate(frame.saved);
                    stack_path_bytes += frame.prefix.len() as u64 + child.prefix.len() as u64;
                    crate::blte::enforce_limit(
                        stack_path_bytes + stored_path_bytes,
                        path_budget,
                        name,
                        node_offset,
                    )?;
                    stack.push(frame);
                    stack.push(child);
                    continue;
                }
                if frame.prefix.is_empty() || frame.prefix.contains(&0) {
                    return Err(error(
                        name,
                        node_offset,
                        Failure::Malformed("empty or NUL-containing TVFS path".into()),
                    ));
                }
                let record = parse_spans(data, &header, value as usize, name, &mut cft_boundaries)?;
                stored_path_bytes = stored_path_bytes
                    .checked_add(frame.prefix.len() as u64 * 2)
                    .ok_or_else(|| {
                        error(
                            name,
                            node_offset,
                            Failure::Malformed("path-byte accounting overflow".into()),
                        )
                    })?;
                crate::blte::enforce_limit(
                    stored_path_bytes + stack_path_bytes,
                    path_budget,
                    name,
                    node_offset,
                )?;
                let path = frame.prefix.clone();
                if entries
                    .insert(
                        path.clone(),
                        Entry {
                            path,
                            node_offset,
                            record,
                        },
                    )
                    .is_some()
                {
                    return Err(error(
                        name,
                        node_offset,
                        Failure::Malformed("duplicate TVFS byte path".into()),
                    ));
                }
                frame.prefix.truncate(frame.saved);
            } else if frame.cursor < frame.end {
                frame.prefix.push(b'/');
            }
            stack_path_bytes += frame.prefix.len() as u64;
            crate::blte::enforce_limit(
                stored_path_bytes + stack_path_bytes,
                path_budget,
                name,
                node_offset,
            )?;
            stack.push(frame);
        }
        Ok(Self {
            evidence: Evidence::of(source, data),
            header,
            entries,
            observed_depth,
            data: data.to_vec(),
            cft_boundaries,
        })
    }

    /// Uninterpreted CFT bytes remain available with bounds fixed by references.
    pub fn cft_record(&self, span: &Span) -> Result<&[u8], Error> {
        let table = bytes(
            &self.data,
            self.header.cft_table.start,
            self.header.cft_table.len(),
            &self.evidence.source.member,
        )?;
        let end = self
            .cft_boundaries
            .range((
                std::ops::Bound::Excluded(span.cft_offset),
                std::ops::Bound::Unbounded,
            ))
            .next()
            .copied()
            .unwrap_or(table.len());
        let length = end.checked_sub(span.cft_offset).ok_or_else(|| {
            error(
                &self.evidence.source.member,
                span.cft_offset,
                Failure::Malformed("invalid CFT range".into()),
            )
        })?;
        bytes(table, span.cft_offset, length, &self.evidence.source.member)
    }

    pub fn control_semantics(&self) -> Error {
        unsupported(
            &self.evidence.source.member,
            self.header.cft_table.start,
            "TVFS CFT patch/write controls",
            format!(
                "flags {:#x}; CFT suffix and encoding-specifier bytes retained",
                self.header.flags
            ),
            "trace native patch/ESpec selection; this reader only opens the hash-verified base representation",
        )
    }
}

fn parse_spans(
    data: &[u8],
    header: &Header,
    offset: usize,
    name: &str,
    boundaries: &mut BTreeSet<usize>,
) -> Result<FileRecord, Error> {
    let vfs = bytes(data, header.vfs_table.start, header.vfs_table.len(), name)?;
    let count = bytes(vfs, offset, 1, name)?[0];
    if !(1..=224).contains(&count) {
        return Ok(FileRecord::Unsupported(unsupported(
            name,
            header.vfs_table.start + offset,
            "TVFS span state",
            count.to_string(),
            "establish deleted/other-file state semantics; no spans are invented",
        )));
    }
    let width = 8 + header.cft_offset_width;
    let raw = bytes(vfs, offset + 1, usize::from(count) * width, name)?;
    let cft = bytes(data, header.cft_table.start, header.cft_table.len(), name)?;
    let mut spans = Vec::with_capacity(usize::from(count));
    for (i, raw) in raw.chunks_exact(width).enumerate() {
        let cft_offset = integer(&raw[8..], true) as usize;
        let prefix = bytes(cft, cft_offset, usize::from(header.ekey_size), name)?.to_vec();
        boundaries.insert(cft_offset);
        spans.push(Span {
            file_offset: integer(&raw[..4], true) as u32,
            size: integer(&raw[4..8], true) as u32,
            vfs_offset: header.vfs_table.start + offset + 1 + i * width,
            cft_offset,
            key_prefix: prefix,
        });
    }
    Ok(FileRecord::Spans(spans))
}

fn offset_width(size: usize) -> usize {
    ((usize::BITS - size.leading_zeros()).div_ceil(8) as usize).max(1)
}

pub(crate) fn unsupported(
    name: &str,
    offset: usize,
    feature: &'static str,
    evidence: String,
    next_probe: &'static str,
) -> Error {
    error(
        name,
        offset,
        Failure::Unsupported {
            feature,
            evidence,
            next_probe,
        },
    )
}

#[derive(Debug)]
pub struct ConfiguredManifest {
    pub config_fields: Vec<String>,
    pub ekey: Key,
    pub ckey: Key,
    pub encoded_size: u64,
    pub manifest: Manifest,
}

#[derive(Debug, Clone)]
pub struct Link {
    pub config_fields: Vec<String>,
    pub manifest: Evidence,
    pub ekey: Key,
    pub ckey: Key,
    pub path: Vec<u8>,
    pub node_offset: usize,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ResolvedPath {
    pub path: Vec<u8>,
    pub build_config: Evidence,
    pub encoding: Evidence,
    pub links: Vec<Link>,
    pub ekey: Key,
    pub ckey: Key,
    pub decoded_size: u64,
}

#[derive(Debug, Clone)]
struct Representation {
    ekey: Key,
    ckey: Key,
    size: u64,
}

#[derive(Debug)]
pub struct Resolver {
    pub build_config: Evidence,
    pub encoding: Evidence,
    manifests: BTreeMap<Key, ConfiguredManifest>,
    root: Key,
    representations: BTreeMap<Vec<u8>, Vec<Representation>>,
}

impl Resolver {
    pub(crate) fn load(storage: &Storage, encoding: &Encoding) -> Result<Self, Error> {
        if storage.build.info.source.build_key != encoding.evidence.source.build_key
            || storage.build.info.source.install != encoding.evidence.source.install
        {
            return Err(error(
                "TVFS",
                0,
                Failure::Malformed("ENCODING belongs to a different install/build".into()),
            ));
        }
        let config = &storage.build.config;
        let mut manifests: BTreeMap<Key, ConfiguredManifest> = BTreeMap::new();
        let mut root = None;
        for (name, pair) in &storage.build.config_values {
            let is_numbered = name.strip_prefix("vfs-").is_some_and(|number| {
                !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
            });
            if name != "vfs-root" && !is_numbered {
                continue;
            }
            if pair.len() != 2 {
                return Err(error(
                    &config.source.member,
                    0,
                    Failure::Malformed(format!("{name} is not a CKey/EKey pair")),
                ));
            }
            let ckey = parse_key(&pair[0], &config.source.member)?;
            let ekey = parse_key(&pair[1], &config.source.member)?;
            if name == "vfs-root" {
                root = Some(ekey);
            }
            if let Some(existing) = manifests.get_mut(&ekey) {
                if existing.ckey != ckey {
                    return Err(error(
                        &config.source.member,
                        0,
                        Failure::Malformed("conflicting configured manifest key pair".into()),
                    ));
                }
                storage.verify_config_lengths(
                    &format!("{name}-size"),
                    existing.manifest.evidence.size,
                    existing.encoded_size,
                )?;
                existing.config_fields.push(name.clone());
                continue;
            }
            let content = encoding
                .contents
                .get(&ckey)
                .filter(|content| content.ekeys.contains(&ekey))
                .ok_or_else(|| {
                    error(
                        &config.source.member,
                        0,
                        Failure::NotFound(format!(
                            "{name} configured pair is absent from ENCODING"
                        )),
                    )
                })?;
            let file = storage.read_ekey(ekey)?;
            crate::verify_md5(
                &file.decoded.bytes,
                ckey,
                &file.decoded.evidence.source.member,
                0,
            )?;
            if file.decoded.bytes.len() as u64 != content.decoded_size {
                return Err(error(
                    &config.source.member,
                    0,
                    Failure::Malformed(format!("{name} ENCODING size disagreement")),
                ));
            }
            if file.decoded.ekey != ekey {
                return Err(error(
                    &config.source.member,
                    0,
                    Failure::Malformed(format!(
                        "{name} EKey differs from its ENCODING representation"
                    )),
                ));
            }
            storage.verify_config_size(&format!("{name}-size"), &file)?;
            let encoded_size = file.decoded.evidence.size;
            let manifest = Manifest::parse(
                &file.decoded.bytes,
                file.decoded.evidence.source,
                storage.limits.decoded_bytes,
            )?;
            manifests.insert(
                ekey,
                ConfiguredManifest {
                    config_fields: vec![name.clone()],
                    ekey,
                    ckey,
                    encoded_size,
                    manifest,
                },
            );
        }
        let root = root.ok_or_else(|| {
            error(
                &config.source.member,
                0,
                Failure::Malformed("missing configured vfs-root".into()),
            )
        })?;
        let widths: BTreeSet<_> = manifests
            .values()
            .map(|manifest| usize::from(manifest.manifest.header.ekey_size))
            .collect();
        let mut representations: BTreeMap<Vec<u8>, Vec<Representation>> = BTreeMap::new();
        for (&ckey, content) in &encoding.contents {
            for &ekey in &content.ekeys {
                for &width in &widths {
                    representations
                        .entry(ekey[..width].to_vec())
                        .or_default()
                        .push(Representation {
                            ekey,
                            ckey,
                            size: content.decoded_size,
                        });
                }
            }
        }
        Ok(Self {
            build_config: config.clone(),
            encoding: encoding.evidence.clone(),
            manifests,
            root,
            representations,
        })
    }

    pub fn manifests(&self) -> &BTreeMap<Key, ConfiguredManifest> {
        &self.manifests
    }

    /// Exact authored namespace. ':' explicitly enters a configured TVFS mount.
    /// ASCII folding is applied only under the manifest's lowercase flag.
    pub fn resolve(&self, path: &str) -> Result<ResolvedPath, Error> {
        self.resolve_bytes(path.as_bytes())
    }

    pub fn resolve_bytes(&self, path: &[u8]) -> Result<ResolvedPath, Error> {
        if path.is_empty() || path.contains(&0) {
            return Err(error(
                "TVFS",
                0,
                Failure::Malformed("empty or NUL-containing lookup path".into()),
            ));
        }
        let components: Vec<_> = path.split(|&byte| byte == b':').collect();
        let mut current = self.root;
        let mut visited = BTreeSet::new();
        let mut links = Vec::new();
        for (i, component) in components.iter().enumerate() {
            if component.is_empty() {
                return Err(error(
                    "TVFS",
                    0,
                    Failure::Malformed("empty mount/path component".into()),
                ));
            }
            if !visited.insert(current) {
                return Err(unsupported(
                    "TVFS",
                    0,
                    "TVFS link cycle",
                    hex(&current),
                    "inspect the cycle's configured mount graph",
                ));
            }
            let configured = self
                .manifests
                .get(&current)
                .ok_or_else(|| error("TVFS", 0, Failure::NotFound(hex(&current))))?;
            let manifest = &configured.manifest;
            let mut lookup = component.to_vec();
            if manifest.header.flags & LOWERCASE != 0 {
                lookup.make_ascii_lowercase();
            }
            let entry = manifest.entries.get(&lookup).ok_or_else(|| {
                error(
                    &manifest.evidence.source.member,
                    manifest.header.path_table.start,
                    Failure::NotFound(format!("TVFS byte path {}", hex(&lookup))),
                )
            })?;
            let spans = match &entry.record {
                FileRecord::Spans(spans) => spans,
                FileRecord::Unsupported(refusal) => return Err(refusal.clone()),
            };
            if spans.len() != 1 {
                return Err(unsupported(
                    &manifest.evidence.source.member,
                    entry.node_offset,
                    "TVFS multi-span execution",
                    spans.len().to_string(),
                    "establish span slicing/assembly semantics before joining bytes",
                ));
            }
            let span = &spans[0];
            let candidates = self.representations.get(&span.key_prefix).ok_or_else(|| {
                error(
                    &manifest.evidence.source.member,
                    span.vfs_offset,
                    Failure::NotFound(format!("ENCODING EKey prefix {}", hex(&span.key_prefix))),
                )
            })?;
            if candidates.len() != 1 {
                return Err(unsupported(
                    &manifest.evidence.source.member,
                    span.vfs_offset,
                    "ambiguous TVFS EKey prefix",
                    format!(
                        "{} representations for {}",
                        candidates.len(),
                        hex(&span.key_prefix)
                    ),
                    "use an independently verified full key/CKey to disambiguate; never select the first match",
                ));
            }
            let representation = &candidates[0];
            if span.file_offset != 0 || u64::from(span.size) != representation.size {
                return Err(unsupported(
                    &manifest.evidence.source.member,
                    span.vfs_offset,
                    "TVFS partial-span execution",
                    format!(
                        "offset {} length {} ENCODING size {}",
                        span.file_offset, span.size, representation.size
                    ),
                    "establish referenced-file slicing semantics with native evidence",
                ));
            }
            links.push(Link {
                config_fields: configured.config_fields.clone(),
                manifest: manifest.evidence.clone(),
                ekey: configured.ekey,
                ckey: configured.ckey,
                path: entry.path.clone(),
                node_offset: entry.node_offset,
                span: span.clone(),
            });
            if i + 1 < components.len() {
                if !self.manifests.contains_key(&representation.ekey) {
                    return Err(unsupported(
                        &manifest.evidence.source.member,
                        entry.node_offset,
                        "unconfigured TVFS mount",
                        hex(&representation.ekey),
                        "establish a build-config link before treating a file as a nested manifest",
                    ));
                }
                current = representation.ekey;
            } else {
                return Ok(ResolvedPath {
                    path: path.to_vec(),
                    build_config: self.build_config.clone(),
                    encoding: self.encoding.clone(),
                    links,
                    ekey: representation.ekey,
                    ckey: representation.ckey,
                    decoded_size: representation.size,
                });
            }
        }
        Err(error(
            "TVFS",
            0,
            Failure::Malformed("lookup has no leaf".into()),
        ))
    }
}

#[cfg(test)]
pub(crate) mod tests;
