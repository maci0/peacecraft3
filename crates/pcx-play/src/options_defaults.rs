//! Opt-in, source-qualified preference fallbacks, not effective startup settings.
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{NeutralError, RuleInputBudget, Sha256Digest, sha256};
use crate::neutral_rule_input::Pe;
use crate::scripts::AssetIdentity;

#[path = "options_defaults_discovery.rs"]
mod discovery;
pub use discovery::DiscoveryBudget;

/// Independently reviewed row and its directly referenced strings in one image.
#[derive(Clone, Debug)]
pub struct RowBinding {
    pub index: u32,
    pub row: SourceWindow,
    pub name: SourceWindow,
    pub fallback: SourceWindow,
}
/// Fingerprints establish identity; review establishes the getter/descriptor roles.
/// Rows are an explicit partial selection, never an assertion of table completeness.
/// `discover` instead qualifies a complete binding through the supported opcode
/// and descriptor profile; that is not independent verification of every build.
#[derive(Clone, Debug)]
pub struct Binding {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub getter: SourceWindow,
    pub rows: Vec<RowBinding>,
}
#[derive(Debug, thiserror::Error)]
#[error("Options defaults {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input or selection budget exceeded")]
    Budget,
    #[error("missing identity or incorrect complete-image fingerprint")]
    Identity,
    #[error("invalid selected PE window: {0}")]
    Image(#[source] NeutralError),
    #[error("unsupported preference getter shape")]
    Getter,
    #[error("invalid, duplicate or inconsistent selected row {index}")]
    Row { index: u32 },
    #[error("selected row {index} has a null fallback pointer")]
    NullFallback { index: u32 },
    #[error("no complete supported preference table found")]
    DiscoveryNone,
    #[error("multiple supported preference table candidates")]
    DiscoveryAmbiguous,
}
#[derive(Debug)]
pub struct Entry<'a> {
    pub index: u32,
    pub name: &'a str,
    /// Original descriptor bytes; non-pointer words remain uninterpreted.
    pub raw: &'a [u8],
    /// Exact source bytes excluding the terminating NUL, without numeric conversion.
    pub fallback: &'a [u8],
}
#[derive(Debug)]
pub struct Defaults<'a> {
    binding: Binding,
    index_limit: u32,
    entries: Vec<Entry<'a>>,
}
impl<'a> Defaults<'a> {
    /// Supports the reviewed x86 string-getter profile. The signed native upper
    /// check does not validate negative indexes; public selections are unsigned.
    /// `profile_bytes` bounds selected source bytes; entry metadata is separately
    /// bounded by the supported positive signed-byte index limit. No registry
    /// failure, first-run state or display-capability policy is inferred.
    pub fn read(image: &'a [u8], binding: Binding, budget: RuleInputBudget) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: binding.origin.source.clone(),
            problem,
        };
        if image.len() > budget.image_bytes || binding.rows.len() > budget.profile_bytes {
            return Err(fail(Problem::Budget));
        }
        let mut selected = binding.getter.len as usize;
        for row in &binding.rows {
            for w in [&row.row, &row.name, &row.fallback] {
                selected = selected
                    .checked_add(w.len as usize)
                    .filter(|n| *n <= budget.profile_bytes)
                    .ok_or_else(|| fail(Problem::Budget))?;
            }
        }
        if selected > budget.profile_bytes {
            return Err(fail(Problem::Budget));
        }
        if [
            &binding.origin.edition,
            &binding.origin.version,
            &binding.origin.source,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
            || sha256(image) != binding.image_sha256
        {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget).map_err(|e| fail(Problem::Image(e)))?;
        let code = pe
            .window(&binding.getter, true)
            .map_err(|e| fail(Problem::Image(e)))?;
        let (index_limit, stride, field) = getter(code).ok_or_else(|| fail(Problem::Getter))?;
        if binding.rows.is_empty() || binding.rows.len() > index_limit as usize {
            return Err(fail(Problem::Budget));
        }
        let mut entries: Vec<Entry<'a>> = Vec::new();
        entries
            .try_reserve_exact(binding.rows.len())
            .map_err(|_| fail(Problem::Budget))?;
        let mut base = None;
        for row in &binding.rows {
            let bad = || fail(Problem::Row { index: row.index });
            let row_base = row
                .index
                .checked_mul(stride)
                .and_then(|offset| row.row.va.checked_sub(offset))
                .ok_or_else(bad)?;
            let fallback_at = field.checked_sub(row_base).ok_or_else(bad)?;
            // The supported descriptor has a name pointer and two opaque words
            // before the fallback field. Its stride comes from the getter.
            if row.index >= index_limit
                || row.row.len != stride
                || fallback_at < 3 * 4
                || fallback_at % 4 != 0
                || fallback_at.checked_add(4).is_none_or(|end| end > stride)
                || base.replace(row_base).is_some_and(|old| old != row_base)
                || entries.iter().any(|e| e.index == row.index)
            {
                return Err(bad());
            }
            let raw = pe
                .window(&row.row, false)
                .map_err(|e| fail(Problem::Image(e)))?;
            let pointer = |at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
            if pointer(0) != row.name.va {
                return Err(bad());
            }
            if pointer(fallback_at as usize) == 0 {
                return Err(fail(Problem::NullFallback { index: row.index }));
            }
            if pointer(fallback_at as usize) != row.fallback.va {
                return Err(bad());
            }
            let name = pe
                .window(&row.name, false)
                .map_err(|e| fail(Problem::Image(e)))?;
            let name = nul(name)
                .and_then(|b| std::str::from_utf8(b).ok())
                .filter(|s| !s.is_empty())
                .ok_or_else(bad)?;
            if entries.iter().any(|e| e.name == name) {
                return Err(bad());
            }
            let fallback = pe
                .window(&row.fallback, false)
                .map_err(|e| fail(Problem::Image(e)))?;
            entries.push(Entry {
                index: row.index,
                name,
                raw,
                fallback: nul(fallback).ok_or_else(bad)?,
            });
        }
        Ok(Self {
            binding,
            index_limit,
            entries,
        })
    }
    pub fn binding(&self) -> &Binding {
        &self.binding
    }
    /// Positive-domain upper bound from the getter, not a count of admitted rows.
    pub fn index_limit(&self) -> u32 {
        self.index_limit
    }
    pub fn entries(&self) -> &[Entry<'a>] {
        &self.entries
    }
    pub fn get(&self, name: &str) -> Option<&Entry<'a>> {
        self.entries.iter().find(|e| e.name == name)
    }
}
fn nul(bytes: &[u8]) -> Option<&[u8]> {
    bytes.strip_suffix(&[0]).filter(|b| !b.contains(&0))
}

/// Exact supported control flow; all addresses, call displacements, limit and
/// stride operands are read from the selected source rather than target constants.
fn getter(bytes: &[u8]) -> Option<(u32, u32, u32)> {
    let mut b = bytes;
    fn expect(b: &mut &[u8], expected: &[u8]) -> Option<()> {
        *b = b.strip_prefix(expected)?;
        Some(())
    }
    fn word(b: &mut &[u8]) -> Option<u32> {
        let (head, tail) = b.split_at_checked(4)?;
        *b = tail;
        Some(u32::from_le_bytes(head.try_into().ok()?))
    }
    expect(
        &mut b,
        &[
            0x55, 0x8b, 0xec, 0x53, 0x56, 0x8b, 0x75, 8, 0x8b, 0xd9, 0x83, 0xfe,
        ],
    )?;
    let limit = i8::from_le_bytes([*b.first()?]);
    b = &b[1..];
    if limit <= 0 {
        return None;
    }
    expect(
        &mut b,
        &[
            0x7c, 8, 0x5e, 0x33, 0xc0, 0x5b, 0x5d, 0xc2, 12, 0, 0x57, 0x68,
        ],
    )?;
    word(&mut b)?;
    expect(&mut b, &[0x56, 0xe8])?;
    word(&mut b)?;
    expect(
        &mut b,
        &[
            0x85, 0xc0, 0x74, 5, 0x8d, 0x78, 0x18, 0xeb, 0x14, 0x8b, 0xc6, 0x8b, 0xcb, 0xc1, 0xe0,
        ],
    )?;
    let shift = u32::from(*b.first()?);
    b = &b[1..];
    let stride = 1u32.checked_shl(shift)?;
    expect(&mut b, &[0x8b, 0xb8])?;
    let field = word(&mut b)?;
    expect(&mut b, &[0x57, 0x56, 0xe8])?;
    word(&mut b)?;
    expect(&mut b, &[0xff, 0x75, 0x10, 0x57, 0xff, 0x75, 12, 0xe8])?;
    word(&mut b)?;
    expect(
        &mut b,
        &[
            0x83, 0xc4, 12, 0xb8, 1, 0, 0, 0, 0x5f, 0x5e, 0x5b, 0x5d, 0xc2, 12, 0,
        ],
    )?;
    b.is_empty().then_some((limit as u32, stride, field))
}

#[cfg(test)]
#[path = "options_defaults_tests.rs"]
mod tests;
