//! `war3map.wpmp` — allied shared data (campaign-wide table overrides).
//!
//! No shipped byte proves a record layout for this member, so this crate
//! decodes no fields. It retains the member's exact bytes with provenance,
//! reports length and hash, and refuses gameplay interpretation loudly at use.
//! A map that carries the member is therefore admitted with its shared data
//! visible, never silently dropped and never parsed against an invented shape.

/// `war3map.wpmp`, the only name the game reads this member under.
pub const WPMP_NAME: &str = "war3map.wpmp";

/// One retained `war3map.wpmp` member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wpmp {
    raw: Vec<u8>,
}

impl Wpmp {
    /// Retain the member's exact bytes. Empty members are refused: an empty
    /// shared-data member carries no provenance and no checkable framing.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.is_empty() {
            return Err(format!("{WPMP_NAME}: empty member carries no shared data"));
        }
        Ok(Self { raw: bytes })
    }

    /// The member's own bytes.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Byte length of the member.
    pub fn len(&self) -> usize {
        self.raw.len()
    }

    /// Whether the member is empty (never true for a parsed value).
    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    /// FNV-1a 64 over the raw member.
    pub fn canonical_hash(&self) -> u64 {
        pcx_mpq::fnv1a64(&self.raw)
    }
}
