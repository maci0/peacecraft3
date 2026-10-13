//! Bounded local CASC reader. See the crate README for source-byte measurements,
//! format references, supported branches and explicit residuals.
#![deny(warnings)]

pub mod blte;
pub mod encoding;
pub mod index;
mod jenkins;
pub mod storage;
pub mod tvfs;

pub type Key = [u8; 16];

/// Build identity is read from .build.info, never inferred from a directory name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub install: String,
    pub version: String,
    pub product: String,
    pub build_key: Option<Key>,
    pub member: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub source: Provenance,
    pub size: u64,
    /// Content identity, not a release identity or security boundary.
    pub md5: Key,
}

impl Evidence {
    pub fn of(source: Provenance, data: &[u8]) -> Self {
        Self {
            source,
            size: data.len() as u64,
            md5: md5::compute(data).0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{source_name} at {offset:#x}: {kind}")]
pub struct Error {
    pub source_name: String,
    pub offset: u64,
    pub kind: Failure,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    #[error("need {need} bytes, have {have}")]
    Truncated { need: usize, have: usize },
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("{algorithm} checksum mismatch: expected {expected}, got {actual}")]
    Checksum {
        algorithm: &'static str,
        expected: String,
        actual: String,
    },
    #[error("unsupported {feature}: {evidence}; next probe: {next_probe}")]
    Unsupported {
        feature: &'static str,
        evidence: String,
        next_probe: &'static str,
    },
    #[error("requested {needed} bytes exceeds caller limit {limit}")]
    Limit { needed: u64, limit: u64 },
    #[error("I/O: {0}")]
    Io(String),
    #[error("no local entry for {0}")]
    NotFound(String),
}

pub(crate) fn error(source: &str, offset: usize, kind: Failure) -> Error {
    Error {
        source_name: source.into(),
        offset: offset as u64,
        kind,
    }
}

pub(crate) fn bytes<'a>(
    data: &'a [u8],
    offset: usize,
    len: usize,
    source: &str,
) -> Result<&'a [u8], Error> {
    let end = offset
        .checked_add(len)
        .ok_or_else(|| error(source, offset, Failure::Malformed("offset overflow".into())))?;
    data.get(offset..end).ok_or_else(|| {
        error(
            source,
            offset,
            Failure::Truncated {
                need: len,
                have: data.len().saturating_sub(offset),
            },
        )
    })
}

pub(crate) fn integer(data: &[u8], big_endian: bool) -> u64 {
    if big_endian {
        data.iter()
            .fold(0, |value, &byte| value << 8 | u64::from(byte))
    } else {
        data.iter()
            .rev()
            .fold(0, |value, &byte| value << 8 | u64::from(byte))
    }
}

pub(crate) fn verify_md5(
    data: &[u8],
    expected: Key,
    source: &str,
    offset: usize,
) -> Result<(), Error> {
    let actual = md5::compute(data).0;
    if actual != expected {
        return Err(error(
            source,
            offset,
            Failure::Checksum {
                algorithm: "MD5",
                expected: hex(&expected),
                actual: hex(&actual),
            },
        ));
    }
    Ok(())
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn parse_key(text: &str, source: &str) -> Result<Key, Error> {
    if text.len() != 32 || !text.is_ascii() {
        return Err(error(
            source,
            0,
            Failure::Malformed("key must be 32 ASCII hex digits".into()),
        ));
    }
    let mut key = [0; 16];
    for (i, byte) in key.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16)
            .map_err(|_| error(source, 2 * i, Failure::Malformed("invalid hex key".into())))?;
    }
    Ok(key)
}

#[cfg(test)]
mod tests;
