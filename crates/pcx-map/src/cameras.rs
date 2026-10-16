//! Authored W3C framing from mdx-m3-viewer commit 2ff0bc00c6363f425016e23d88c0fb2929d3b3cc.
//! Header version does not select the profile. No native camera behavior is inferred.
use crate::{Map, MapMemberError};
use pcx_mpq::checked::ReadBudget;
use std::ops::Range;

pub const CAMERAS_NAME: &str = "war3map.w3c";

/// Select from explicit source/build provenance, never from the W3C version word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraProfile {
    /// Primary implementation's buildVersion <= 131 branch.
    Base,
    /// Primary implementation's buildVersion > 131 branch: three words after the name.
    LocalAngles,
}
impl CameraProfile {
    fn suffix_bytes(self) -> usize {
        match self {
            Self::Base => 0,
            Self::LocalAngles => 12,
        }
    }
}

/// Caller framing policy; max_bytes also caps decoded allocation before extraction.
#[derive(Debug, Clone, Copy)]
pub struct CameraLimits {
    pub max_bytes: usize,
    pub max_records: u32,
    /// Excludes the NUL terminator.
    pub max_name_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CameraError {
    #[error("camera {what} at byte {at} exceeds caller limit {limit}")]
    Limit {
        what: &'static str,
        at: usize,
        limit: usize,
    },
    #[error("camera span at byte {at} needs {need} bytes, only {have} remain")]
    Truncated { at: usize, need: usize, have: usize },
    #[error(
        "{count} camera records at byte {at} each need at least {minimum_record_bytes} bytes; only {have} remain"
    )]
    CountFraming {
        count: u32,
        at: usize,
        minimum_record_bytes: usize,
        have: usize,
    },
    #[error("camera name at byte {at} has no NUL terminator")]
    UnterminatedName { at: usize },
    #[error("camera records end at byte {end}, member length is {length}")]
    TrailingBytes { end: usize, length: usize },
}

#[derive(Debug, thiserror::Error)]
#[error("{source_label}: {member} ({profile:?}): {cause}")]
pub struct CameraLoadError {
    pub source_label: String,
    pub member: &'static str,
    pub profile: CameraProfile,
    #[source]
    pub cause: CameraLoadCause,
}
#[derive(Debug, thiserror::Error)]
pub enum CameraLoadCause {
    #[error("extraction: {0}")]
    Extraction(#[source] Box<MapMemberError>),
    #[error("camera member is missing")]
    MissingMember,
    #[error("framing: {0}")]
    Framing(#[from] CameraError),
}

impl From<MapMemberError> for CameraLoadCause {
    fn from(source: MapMemberError) -> Self {
        Self::Extraction(Box::new(source))
    }
}

/// Immutable checked borrowed member. Parsing and iteration allocate no storage.
#[derive(Debug, Clone, Copy)]
pub struct CameraFile<'a> {
    raw: &'a [u8],
    version: i32,
    record_count: u32,
    profile: CameraProfile,
}
impl<'a> CameraFile<'a> {
    pub fn parse(
        raw: &'a [u8],
        profile: CameraProfile,
        limits: CameraLimits,
    ) -> Result<Self, CameraError> {
        if raw.len() > limits.max_bytes {
            return Err(CameraError::Limit {
                what: "member bytes",
                at: 0,
                limit: limits.max_bytes,
            });
        }
        let mut cursor = Cursor { raw, at: 0 };
        let version = i32::from_le_bytes(cursor.take(4)?.try_into().unwrap());
        let record_count = u32::from_le_bytes(cursor.take(4)?.try_into().unwrap());
        if record_count > limits.max_records {
            return Err(CameraError::Limit {
                what: "record count",
                at: 4,
                limit: limits.max_records as usize,
            });
        }
        // Division avoids count multiplication overflow and rejects impossible loops before traversal.
        let minimum = 41 + profile.suffix_bytes();
        if u64::from(record_count) > ((raw.len() - 8) / minimum) as u64 {
            return Err(CameraError::CountFraming {
                count: record_count,
                at: 8,
                minimum_record_bytes: minimum,
                have: raw.len() - 8,
            });
        }
        for _ in 0..record_count {
            cursor.record(profile, limits.max_name_bytes)?;
        }
        if cursor.at != raw.len() {
            return Err(CameraError::TrailingBytes {
                end: cursor.at,
                length: raw.len(),
            });
        }
        Ok(Self {
            raw,
            version,
            record_count,
            profile,
        })
    }
    pub fn version(&self) -> i32 {
        self.version
    }
    pub fn record_count(&self) -> u32 {
        self.record_count
    }
    pub fn profile(&self) -> CameraProfile {
        self.profile
    }
    pub fn raw(&self) -> &'a [u8] {
        self.raw
    }
    pub fn records(&self) -> impl Iterator<Item = CameraRecord<'a>> + 'a {
        let mut cursor = Cursor {
            raw: self.raw,
            at: 8,
        };
        let profile = self.profile;
        (0..self.record_count).map(move |_| {
            cursor
                .record(profile, usize::MAX)
                .expect("immutable validated camera record")
        })
    }
}

#[derive(Debug)]
pub struct CameraRecord<'a> {
    /// Half-open member-relative byte range, including terminator and any extension.
    pub span: Range<usize>,
    /// Ten unmodified f32 words in primary source order: `target[3]`, rotation,
    /// angleOfAttack, distance, roll, fieldOfView, farClippingPlane, nearClippingPlane.
    pub words: &'a [u8],
    pub name_span: Range<usize>,
    pub name: &'a [u8],
    /// Raw localPitch/localYaw/localRoll words, absent for Base.
    pub local_angles: Option<&'a [u8]>,
}

struct Cursor<'a> {
    raw: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, need: usize) -> Result<&'a [u8], CameraError> {
        let have = self.raw.len() - self.at;
        if need > have {
            return Err(CameraError::Truncated {
                at: self.at,
                need,
                have,
            });
        }
        let start = self.at;
        self.at += need;
        Ok(&self.raw[start..self.at])
    }
    fn record(
        &mut self,
        profile: CameraProfile,
        name_limit: usize,
    ) -> Result<CameraRecord<'a>, CameraError> {
        let start = self.at;
        let words = self.take(40)?;
        let name_start = self.at;
        let tail = &self.raw[name_start..];
        let search = tail.len().min(name_limit.saturating_add(1));
        let length = match tail[..search].iter().position(|&b| b == 0) {
            Some(n) => n,
            None if tail.len() > name_limit => {
                return Err(CameraError::Limit {
                    what: "name bytes",
                    at: name_start,
                    limit: name_limit,
                });
            }
            None => return Err(CameraError::UnterminatedName { at: name_start }),
        };
        let name = self.take(length)?;
        self.take(1)?;
        let local_angles = match profile {
            CameraProfile::Base => None,
            CameraProfile::LocalAngles => Some(self.take(12)?),
        };
        Ok(CameraRecord {
            span: start..self.at,
            words,
            name_span: name_start..name_start + length,
            name,
            local_angles,
        })
    }
}

/// Owns only the already extracted member and the caller's moved provenance label.
#[derive(Debug)]
pub struct MapCameras {
    raw: Vec<u8>,
    source_label: String,
    version: i32,
    record_count: u32,
    profile: CameraProfile,
}
impl MapCameras {
    pub fn source_label(&self) -> &str {
        &self.source_label
    }
    pub fn view(&self) -> CameraFile<'_> {
        CameraFile {
            raw: &self.raw,
            version: self.version,
            record_count: self.record_count,
            profile: self.profile,
        }
    }
}
impl Map<'_> {
    /// Explicit shared extraction budget; the camera byte cap applies before decoding.
    pub fn cameras(
        &self,
        source_label: String,
        profile: CameraProfile,
        limits: CameraLimits,
        read_budget: ReadBudget,
    ) -> Result<MapCameras, CameraLoadError> {
        let checked = || -> Result<_, CameraLoadCause> {
            let read_budget = ReadBudget {
                max_decoded_bytes: read_budget.max_decoded_bytes.min(limits.max_bytes),
                ..read_budget
            };
            let raw = self
                .extract_checked(&source_label, CAMERAS_NAME, read_budget)?
                .ok_or(CameraLoadCause::MissingMember)?;
            let view = CameraFile::parse(&raw, profile, limits)?;
            let (version, record_count) = (view.version, view.record_count);
            Ok((raw, version, record_count))
        };
        match checked() {
            Ok((raw, version, record_count)) => Ok(MapCameras {
                raw,
                source_label,
                version,
                record_count,
                profile,
            }),
            Err(cause) => Err(CameraLoadError {
                source_label,
                member: CAMERAS_NAME,
                profile,
                cause,
            }),
        }
    }
}
