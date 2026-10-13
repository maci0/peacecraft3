//! Local archive lookup only. No HTTP, CDN fallback, writes, or path guesses.
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::blte::{Decoded, Limits, enforce_limit};
use crate::encoding::Encoding;
use crate::index::Index;
use crate::{Error, Evidence, Failure, Key, Provenance, error, hex, parse_key, verify_md5};

#[derive(Debug)]
pub struct Build {
    pub info: Evidence,
    pub config: Evidence,
    pub fields: BTreeMap<String, String>,
    pub config_values: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub enum EnvelopeKey {
    MatchesRequestedKey(Key),
    /// A partial/zeroed envelope does not identify a file. BLTE does that.
    Unverified {
        stored: Key,
        requested: Key,
    },
}

#[derive(Debug)]
pub struct LocalEnvelope {
    pub ekey_claim: EnvelopeKey,
    pub total_size: u32,
    /// Preserved without inventing flags or span-checksum meanings.
    pub opaque_span_fields: [u8; 10],
}

#[derive(Debug)]
pub struct FileBytes {
    pub index: Evidence,
    pub index_entry_offset: usize,
    pub archive: PathBuf,
    pub archive_offset: u64,
    pub local_envelope: LocalEnvelope,
    pub decoded: Decoded,
}

#[derive(Debug)]
pub struct PathBytes {
    pub resolved: crate::tvfs::ResolvedPath,
    pub file: FileBytes,
}

impl FileBytes {
    pub fn envelope_key_semantics(&self) -> Result<Key, Error> {
        match self.local_envelope.ekey_claim {
            EnvelopeKey::MatchesRequestedKey(key) => Ok(key),
            EnvelopeKey::Unverified { stored, requested } => Err(error(
                &self.archive.to_string_lossy(),
                self.archive_offset as usize,
                Failure::Unsupported {
                    feature: "local envelope key claim",
                    evidence: format!(
                        "stored {}; requested {}; identity independently verified by full BLTE hash",
                        hex(&stored),
                        hex(&requested)
                    ),
                    next_probe: "trace why the local writer truncates/zeros this key; never use the claim as file identity",
                },
            )),
        }
    }

    pub fn span_fields_semantics(&self) -> Error {
        error(&self.archive.to_string_lossy(), self.archive_offset as usize + 20, Failure::Unsupported {
            feature: "local span flags/checksum semantics",
            evidence: "ten opaque bytes retained; EKey, size and BLTE chunk hashes checked; decoded content hash checked by read_content".into(),
            next_probe: "trace this build's local 30-byte span envelope and its two guarded words",
        })
    }
}

#[derive(Debug)]
pub struct Storage {
    pub build: Build,
    pub indexes: Vec<Index>,
    pub data_part_sizes: BTreeMap<u64, u64>,
    pub limits: Limits,
    install: PathBuf,
}

impl Storage {
    pub fn open(install: &Path, limits: Limits) -> Result<Self, Error> {
        let info_path = install.join(".build.info");
        let info_bytes = read_bounded(&info_path, limits.encoded_bytes)?;
        let fields = parse_build_info(&info_bytes, &info_path.to_string_lossy())?;
        let get = |name: &str| {
            fields.get(name).ok_or_else(|| {
                error(
                    ".build.info",
                    0,
                    Failure::Malformed(format!("missing {name}")),
                )
            })
        };
        let version = get("Version")?.clone();
        let product = get("Product")?.clone();
        let build_key = parse_key(get("Build Key")?, ".build.info")?;
        let source = |member: String| Provenance {
            install: install.display().to_string(),
            version: version.clone(),
            product: product.clone(),
            build_key: Some(build_key),
            member,
        };
        let info = Evidence::of(source(".build.info".into()), &info_bytes);
        let key_hex = hex(&build_key);
        let config_member = format!(
            "Data/config/{}/{}/{}",
            &key_hex[..2],
            &key_hex[2..4],
            key_hex
        );
        let config_bytes = read_bounded(&install.join(&config_member), limits.encoded_bytes)?;
        verify_md5(&config_bytes, build_key, &config_member, 0)?;
        let config_values = parse_config(&config_bytes, &config_member)?;
        let config = Evidence::of(source(config_member), &config_bytes);
        let mut paths = std::fs::read_dir(install.join("Data/data"))
            .map_err(|err| io_error(&install.join("Data/data"), err))?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|err| io_error(&install.join("Data/data"), err))
            })
            .collect::<Result<Vec<_>, _>>()?;
        paths.sort();
        let mut selected: BTreeMap<u8, (u32, PathBuf)> = BTreeMap::new();
        let mut data_part_sizes = BTreeMap::new();
        for path in &paths {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if let Some(suffix) = name.strip_prefix("data.")
                && suffix.len() == 3
                && suffix.bytes().all(|byte| byte.is_ascii_digit())
            {
                let number: u64 = suffix
                    .parse()
                    .map_err(|_| error(name, 0, Failure::Malformed("data part number".into())))?;
                data_part_sizes.insert(
                    number,
                    path.metadata().map_err(|err| io_error(path, err))?.len(),
                );
            }
            if !name.ends_with(".idx") {
                continue;
            }
            let (bucket, generation) = index_filename(name)?;
            if selected
                .get(&bucket)
                .is_none_or(|(old, _)| generation > *old)
            {
                selected.insert(bucket, (generation, path.clone()));
            }
        }
        if selected.len() != 16 {
            return Err(error(
                "Data/data",
                0,
                Failure::Malformed(format!(
                    "expected the format's 16 bucket indexes, found {}",
                    selected.len()
                )),
            ));
        }
        let mut indexes = Vec::new();
        for (bucket, (_, path)) in selected {
            let raw = read_bounded(&path, limits.encoded_bytes)?;
            let member = path
                .strip_prefix(install)
                .map_err(|err| error("Data/data", 0, Failure::Malformed(err.to_string())))?
                .display()
                .to_string();
            let index = Index::parse(&raw, source(member))?;
            if index.header.bucket != bucket {
                return Err(error(
                    &index.evidence.source.member,
                    10,
                    Failure::Malformed("header bucket differs from filename".into()),
                ));
            }
            for entry in &index.entries {
                let length = data_part_sizes.get(&entry.segment).ok_or_else(|| {
                    error(
                        &index.evidence.source.member,
                        entry.index_offset,
                        Failure::NotFound(format!("data.{:03}", entry.segment)),
                    )
                })?;
                if entry
                    .offset
                    .checked_add(entry.encoded_size)
                    .is_none_or(|end| end > *length)
                {
                    return Err(error(
                        &index.evidence.source.member,
                        entry.index_offset,
                        Failure::Malformed("entry span exceeds data-part file size".into()),
                    ));
                }
            }
            indexes.push(index);
        }
        Ok(Self {
            build: Build {
                info,
                config,
                fields,
                config_values,
            },
            indexes,
            data_part_sizes,
            limits,
            install: install.to_path_buf(),
        })
    }

    pub fn read_ekey(&self, ekey: Key) -> Result<FileBytes, Error> {
        let mut last_error = None;
        for index in &self.indexes {
            for entry in index
                .entries
                .iter()
                .filter(|entry| ekey.starts_with(&entry.key_prefix))
            {
                let archive = self
                    .install
                    .join("Data/data")
                    .join(format!("data.{:03}", entry.segment));
                let name = archive.to_string_lossy();
                enforce_limit(
                    entry.encoded_size,
                    self.limits.encoded_bytes,
                    &name,
                    entry.offset as usize,
                )?;
                let length = usize::try_from(entry.encoded_size).map_err(|_| {
                    error(
                        &name,
                        0,
                        Failure::Malformed("encoded length does not fit address space".into()),
                    )
                })?;
                if length < 30 {
                    return Err(error(
                        &name,
                        entry.offset as usize,
                        Failure::Truncated {
                            need: 30,
                            have: length,
                        },
                    ));
                }
                let mut file = File::open(&archive).map_err(|err| io_error(&archive, err))?;
                // Recheck bounds at read time: the installation could have changed.
                let file_size = file
                    .metadata()
                    .map_err(|err| io_error(&archive, err))?
                    .len();
                if entry
                    .offset
                    .checked_add(entry.encoded_size)
                    .is_none_or(|end| end > file_size)
                {
                    return Err(error(
                        &name,
                        entry.offset as usize,
                        Failure::Malformed("span no longer fits data part".into()),
                    ));
                }
                file.seek(SeekFrom::Start(entry.offset))
                    .map_err(|err| io_error(&archive, err))?;
                let mut data = vec![0; length];
                file.read_exact(&mut data)
                    .map_err(|err| io_error(&archive, err))?;
                let mut envelope_key: Key = data[..16].try_into().unwrap();
                envelope_key.reverse();
                let ekey_claim = if envelope_key == ekey {
                    EnvelopeKey::MatchesRequestedKey(envelope_key)
                } else {
                    EnvelopeKey::Unverified {
                        stored: envelope_key,
                        requested: ekey,
                    }
                };
                let total_size = u32::from_le_bytes(data[16..20].try_into().unwrap());
                if u64::from(total_size) != entry.encoded_size {
                    return Err(error(
                        &name,
                        entry.offset as usize + 16,
                        Failure::Malformed("local envelope size differs from index span".into()),
                    ));
                }
                let local_envelope = LocalEnvelope {
                    ekey_claim,
                    total_size,
                    opaque_span_fields: data[20..30].try_into().unwrap(),
                };
                let mut source = index.evidence.source.clone();
                source.member =
                    format!("Data/data/data.{:03}@{:#x}+30", entry.segment, entry.offset);
                let decoded = match crate::blte::decode(&data[30..], ekey, source, self.limits) {
                    Ok(decoded) => decoded,
                    Err(err) => {
                        last_error = Some(err);
                        continue;
                    }
                };
                return Ok(FileBytes {
                    index: index.evidence.clone(),
                    index_entry_offset: entry.index_offset,
                    archive,
                    archive_offset: entry.offset,
                    local_envelope,
                    decoded,
                });
            }
        }
        Err(last_error.unwrap_or_else(|| error("Data/data", 0, Failure::NotFound(hex(&ekey)))))
    }

    pub fn load_encoding(&self) -> Result<Encoding, Error> {
        let pair = self
            .build
            .config_values
            .get("encoding")
            .filter(|value| value.len() == 2)
            .ok_or_else(|| {
                error(
                    &self.build.config.source.member,
                    0,
                    Failure::Malformed("encoding must supply CKey and EKey".into()),
                )
            })?;
        let ckey = parse_key(&pair[0], &self.build.config.source.member)?;
        let ekey = parse_key(&pair[1], &self.build.config.source.member)?;
        let file = self.read_ekey(ekey)?;
        verify_md5(
            &file.decoded.bytes,
            ckey,
            &file.decoded.evidence.source.member,
            0,
        )?;
        self.verify_config_size("encoding-size", &file)?;
        Encoding::parse(&file.decoded.bytes, file.decoded.evidence.source)
    }

    pub fn read_content(&self, encoding: &Encoding, ckey: Key) -> Result<FileBytes, Error> {
        if encoding.evidence.source.build_key != self.build.info.source.build_key
            || encoding.evidence.source.install != self.build.info.source.install
        {
            return Err(error(
                "ENCODING",
                0,
                Failure::Malformed(
                    "encoding provenance belongs to a different installation/build".into(),
                ),
            ));
        }
        let record = encoding
            .contents
            .get(&ckey)
            .ok_or_else(|| error("ENCODING", 0, Failure::NotFound(hex(&ckey))))?;
        enforce_limit(
            record.decoded_size,
            self.limits.decoded_bytes,
            "ENCODING",
            record.page_offset,
        )?;
        let mut last_error = None;
        for &ekey in &record.ekeys {
            match self.read_ekey(ekey) {
                Ok(file) => {
                    if file.decoded.bytes.len() as u64 != record.decoded_size {
                        return Err(error(
                            &file.decoded.evidence.source.member,
                            0,
                            Failure::Malformed("content size differs from ENCODING record".into()),
                        ));
                    }
                    verify_md5(
                        &file.decoded.bytes,
                        ckey,
                        &file.decoded.evidence.source.member,
                        0,
                    )?;
                    return Ok(file);
                }
                Err(err) => last_error = Some(err),
            }
        }
        Err(last_error.unwrap_or_else(|| error("ENCODING", 0, Failure::NotFound(hex(&ckey)))))
    }

    /// Load checked configured TVFS manifests once for repeated path lookups.
    pub fn load_tvfs(&self, encoding: &Encoding) -> Result<crate::tvfs::Resolver, Error> {
        crate::tvfs::Resolver::load(self, encoding)
    }

    /// Convenience lookup; consumers doing repeated reads should reuse load_tvfs.
    pub fn path_to_ekey(&self, encoding: &Encoding, path: &str) -> Result<Key, Error> {
        Ok(self.load_tvfs(encoding)?.resolve(path)?.ekey)
    }

    /// Preserve path/config/manifest evidence beside the fully checked content.
    pub fn read_path(
        &self,
        resolver: &crate::tvfs::Resolver,
        path: &str,
    ) -> Result<PathBytes, Error> {
        self.read_path_bytes(resolver, path.as_bytes())
    }

    /// Preserve opaque byte paths; no guessed filesystem encoding conversion.
    pub fn read_path_bytes(
        &self,
        resolver: &crate::tvfs::Resolver,
        path: &[u8],
    ) -> Result<PathBytes, Error> {
        let resolved = resolver.resolve_bytes(path)?;
        if resolved.build_config != self.build.config {
            return Err(error(
                "TVFS",
                0,
                Failure::Malformed(
                    "resolver belongs to a different install/build configuration".into(),
                ),
            ));
        }
        let file = self.read_ekey(resolved.ekey)?;
        if file.decoded.bytes.len() as u64 != resolved.decoded_size {
            return Err(error(
                &file.decoded.evidence.source.member,
                0,
                Failure::Malformed("path/span/ENCODING content size disagreement".into()),
            ));
        }
        verify_md5(
            &file.decoded.bytes,
            resolved.ckey,
            &file.decoded.evidence.source.member,
            0,
        )?;
        Ok(PathBytes { resolved, file })
    }

    pub fn verify_config_size(&self, field: &str, file: &FileBytes) -> Result<(), Error> {
        self.verify_config_lengths(
            field,
            file.decoded.bytes.len() as u64,
            file.decoded.evidence.size,
        )
    }

    pub(crate) fn verify_config_lengths(
        &self,
        field: &str,
        decoded: u64,
        encoded: u64,
    ) -> Result<(), Error> {
        let values = self
            .build
            .config_values
            .get(field)
            .filter(|values| values.len() == 2)
            .ok_or_else(|| {
                error(
                    &self.build.config.source.member,
                    0,
                    Failure::Malformed(format!("missing decoded/encoded size pair {field}")),
                )
            })?;
        let parsed = values
            .iter()
            .map(|value| {
                value.parse::<u64>().map_err(|_| {
                    error(
                        &self.build.config.source.member,
                        0,
                        Failure::Malformed(format!("invalid size in {field}")),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if parsed[0] != decoded || parsed[1] != encoded {
            return Err(error(
                &self.build.config.source.member,
                0,
                Failure::Malformed(format!("{field} differs from verified bytes")),
            ));
        }
        Ok(())
    }
}

pub fn parse_build_info(data: &[u8], name: &str) -> Result<BTreeMap<String, String>, Error> {
    let text = std::str::from_utf8(data).map_err(|err| {
        error(
            name,
            0,
            Failure::Malformed(format!("build-info encoding: {err}")),
        )
    })?;
    let mut lines = text.lines();
    let headers = lines
        .next()
        .ok_or_else(|| error(name, 0, Failure::Malformed("empty build-info".into())))?
        .split('|')
        .map(|field| {
            field.split_once('!').map(|(name, _)| name).ok_or_else(|| {
                error(
                    name,
                    0,
                    Failure::Malformed("untyped build-info header".into()),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if headers
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != headers.len()
    {
        return Err(error(
            name,
            0,
            Failure::Malformed("duplicate build-info columns".into()),
        ));
    }
    let active_column = headers
        .iter()
        .position(|&field| field == "Active")
        .ok_or_else(|| error(name, 0, Failure::Malformed("missing Active column".into())))?;
    let mut active = None;
    for line in lines.filter(|line| !line.trim().is_empty()) {
        let values: Vec<_> = line.split('|').collect();
        if values.len() != headers.len() {
            return Err(error(
                name,
                0,
                Failure::Malformed("build-info row/header width mismatch".into()),
            ));
        }
        match values[active_column] {
            "0" => continue,
            "1" => {}
            value => {
                return Err(error(
                    name,
                    0,
                    Failure::Malformed(format!("invalid active flag {value}")),
                ));
            }
        }
        if active.is_some() {
            return Err(error(
                name,
                0,
                Failure::Malformed("multiple active build-info rows".into()),
            ));
        }
        active = Some(
            headers
                .iter()
                .zip(values)
                .map(|(header, value)| (header.to_string(), value.to_string()))
                .collect(),
        );
    }
    active.ok_or_else(|| {
        error(
            name,
            0,
            Failure::Malformed("no active build-info row".into()),
        )
    })
}

pub fn parse_config(data: &[u8], name: &str) -> Result<BTreeMap<String, Vec<String>>, Error> {
    let text = std::str::from_utf8(data).map_err(|err| {
        error(
            name,
            0,
            Failure::Malformed(format!("config encoding: {err}")),
        )
    })?;
    let mut values = BTreeMap::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, raw) = line.split_once('=').ok_or_else(|| {
            error(
                name,
                i,
                Failure::Malformed("config assignment lacks equals sign".into()),
            )
        })?;
        let key = key.trim();
        if key.is_empty()
            || values
                .insert(
                    key.into(),
                    raw.split_whitespace().map(str::to_string).collect(),
                )
                .is_some()
        {
            return Err(error(
                name,
                i,
                Failure::Malformed("empty or duplicate config key".into()),
            ));
        }
    }
    Ok(values)
}

fn index_filename(name: &str) -> Result<(u8, u32), Error> {
    let stem = name
        .strip_suffix(".idx")
        .filter(|stem| stem.len() == 10 && stem.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| {
            error(
                name,
                0,
                Failure::Unsupported {
                    feature: "index filename",
                    evidence: name.into(),
                    next_probe: "establish filename generation and bucket fields for this variant",
                },
            )
        })?;
    let bucket = u8::from_str_radix(&stem[..2], 16)
        .map_err(|_| error(name, 0, Failure::Malformed("index bucket".into())))?;
    if bucket >= 16 {
        return Err(error(
            name,
            0,
            Failure::Malformed("bucket exceeds the 16-bucket format".into()),
        ));
    }
    let generation = u32::from_str_radix(&stem[2..], 16)
        .map_err(|_| error(name, 2, Failure::Malformed("index generation".into())))?;
    Ok((bucket, generation))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, Error> {
    let mut file = File::open(path).map_err(|err| io_error(path, err))?;
    let length = file.metadata().map_err(|err| io_error(path, err))?.len();
    enforce_limit(length, limit, &path.to_string_lossy(), 0)?;
    let mut data = Vec::new();
    file.by_ref()
        .take(limit.saturating_add(1))
        .read_to_end(&mut data)
        .map_err(|err| io_error(path, err))?;
    enforce_limit(data.len() as u64, limit, &path.to_string_lossy(), 0)?;
    Ok(data)
}

fn io_error(path: &Path, err: std::io::Error) -> Error {
    error(&path.to_string_lossy(), 0, Failure::Io(err.to_string()))
}
