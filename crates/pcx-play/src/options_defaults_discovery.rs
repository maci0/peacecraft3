//! Pattern-qualified discovery; no native startup or saved-preference policy.
use super::{Binding, Defaults, Error, Problem, RowBinding, getter};
use crate::camera_margins::SourceWindow;
use crate::neutral_player::{RuleInputBudget, sha256};
use crate::neutral_rule_input::Pe;
use crate::pe_image::windows_with_byte;
use crate::scripts::AssetIdentity;
use std::alloc::Layout;

// Length of the supported complete opcode profile, not a game-data extent.
const GETTER_BYTES: usize = 91;
/// First byte `getter` requires of its window (`push ebp`).
const GETTER_FIRST_BYTE: u8 = 0x55;
const WORD_BYTES: u32 = 4;

/// Explicit caller limits, independent of the source's decoded ordinal bound.
#[derive(Clone, Copy)]
pub struct DiscoveryBudget {
    pub image: RuleInputBudget,
    /// Maximum inspected bytes per directly referenced string, including NUL.
    pub string_bytes: usize,
    /// Complete opcode-shape matches, including candidates with invalid tables.
    pub candidate_limit: usize,
    /// Logical source bytes charged: all scanned code ranges plus each candidate
    /// table and inspected string bytes. Repeated pointers are charged again.
    /// This bounds deterministic input work, not every CPU operation/heap byte.
    pub work_bytes: usize,
}

impl<'a> Defaults<'a> {
    /// Derives an identity-bound Binding from exactly one complete valid candidate.
    /// The supported descriptor profile places its fallback pointer in the last
    /// word of the decoded stride. This is a pattern-qualified interpretation,
    /// not automatic proof of native roles across versions or first-run settings.
    /// The existing reviewed, partial-selection `read` API remains available.
    pub fn discover(
        image: &'a [u8],
        origin: AssetIdentity,
        budget: DiscoveryBudget,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: origin.source.clone(),
            problem,
        };
        if image.len() > budget.image.image_bytes
            || budget.string_bytes == 0
            || budget.candidate_limit == 0
            || budget.work_bytes == 0
        {
            return Err(fail(Problem::Budget));
        }
        if [&origin.edition, &origin.version, &origin.source]
            .iter()
            .any(|s| s.trim().is_empty())
        {
            return Err(fail(Problem::Identity));
        }
        let pe = Pe::parse(image, budget.image).map_err(|e| fail(Problem::Image(e)))?;
        let image_sha256 = sha256(image);
        let mut work = 0;
        let mut candidates = 0usize;
        let mut found = None;
        for (va, code) in pe.code_ranges() {
            charge(&mut work, code.len(), budget.work_bytes).map_err(fail)?;
            for (offset, bytes) in windows_with_byte(code, 0, GETTER_FIRST_BYTE, GETTER_BYTES) {
                let Some(operands) = getter(bytes) else {
                    continue;
                };
                candidates = candidates
                    .checked_add(1)
                    .filter(|n| *n <= budget.candidate_limit)
                    .ok_or_else(|| fail(Problem::Budget))?;
                let Some(rows) = candidate_rows(&pe, operands, budget, &mut work).map_err(fail)?
                else {
                    continue;
                };
                let getter_va = u32::try_from(offset)
                    .ok()
                    .and_then(|offset| va.checked_add(offset))
                    .ok_or_else(|| fail(Problem::Budget))?;
                let binding = Binding {
                    origin: origin.clone(),
                    image_sha256,
                    getter: source_window(getter_va, bytes).map_err(fail)?,
                    rows,
                };
                if found.replace(binding).is_some() {
                    return Err(fail(Problem::DiscoveryAmbiguous));
                }
            }
        }
        let binding = found.ok_or_else(|| fail(Problem::DiscoveryNone))?;
        Self::read(image, binding, budget.image)
    }
}

fn charge(total: &mut usize, bytes: usize, limit: usize) -> Result<(), Problem> {
    *total = total
        .checked_add(bytes)
        .filter(|n| *n <= limit)
        .ok_or(Problem::Budget)?;
    Ok(())
}

fn source_window(va: u32, bytes: &[u8]) -> Result<SourceWindow, Problem> {
    Ok(SourceWindow {
        va,
        len: u32::try_from(bytes.len()).map_err(|_| Problem::Budget)?,
        digest: sha256(bytes),
    })
}

fn candidate_rows(
    pe: &Pe<'_>,
    (count, stride, field): (u32, u32, u32),
    budget: DiscoveryBudget,
    work: &mut usize,
) -> Result<Option<Vec<RowBinding>>, Problem> {
    // Name pointer, two opaque words, then last-word fallback in this profile.
    if stride < 4 * WORD_BYTES || stride % WORD_BYTES != 0 {
        return Ok(None);
    }
    let Some(base) = field.checked_sub(stride - WORD_BYTES) else {
        return Ok(None);
    };
    let table_bytes = count.checked_mul(stride).ok_or(Problem::Budget)?;
    if base.checked_add(table_bytes).is_none() {
        return Ok(None);
    }
    let table_bytes = usize::try_from(table_bytes).map_err(|_| Problem::Budget)?;
    let mut selected = GETTER_BYTES;
    charge(&mut selected, table_bytes, budget.image.profile_bytes)?;
    charge(work, table_bytes, budget.work_bytes)?;
    let Ok(tail) = pe.read_from(base, false) else {
        return Ok(None);
    };
    let Some(table) = tail.get(..table_bytes) else {
        return Ok(None);
    };
    let count = usize::try_from(count).map_err(|_| Problem::Budget)?;
    Layout::array::<RowBinding>(count).map_err(|_| Problem::Budget)?;
    Layout::array::<&str>(count).map_err(|_| Problem::Budget)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count).map_err(|_| Problem::Budget)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(count)
        .map_err(|_| Problem::Budget)?;
    let stride = usize::try_from(stride).map_err(|_| Problem::Budget)?;
    for (index, raw) in table.chunks_exact(stride).enumerate() {
        let pointer = |at| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
        let Some((name, name_window)) = direct_string(pe, pointer(0), budget, &mut selected, work)?
        else {
            return Ok(None);
        };
        let Some(name) = std::str::from_utf8(name).ok().filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        if names.contains(&name) {
            return Ok(None);
        }
        let Some((_, fallback)) =
            direct_string(pe, pointer(stride - 4), budget, &mut selected, work)?
        else {
            return Ok(None);
        };
        names.push(name);
        let index = u32::try_from(index).map_err(|_| Problem::Budget)?;
        let row_va = base
            .checked_add(index.checked_mul(stride as u32).ok_or(Problem::Budget)?)
            .ok_or(Problem::Budget)?;
        rows.push(RowBinding {
            index,
            row: source_window(row_va, raw)?,
            name: name_window,
            fallback,
        });
    }
    Ok(Some(rows))
}

fn direct_string<'a>(
    pe: &Pe<'a>,
    va: u32,
    budget: DiscoveryBudget,
    selected: &mut usize,
    work: &mut usize,
) -> Result<Option<(&'a [u8], SourceWindow)>, Problem> {
    if va == 0 {
        return Ok(None);
    }
    let Ok(tail) = pe.read_from(va, false) else {
        return Ok(None);
    };
    for (offset, byte) in tail.iter().take(budget.string_bytes).enumerate() {
        charge(selected, 1, budget.image.profile_bytes)?;
        charge(work, 1, budget.work_bytes)?;
        if *byte == 0 {
            return Ok(Some((
                &tail[..offset],
                source_window(va, &tail[..=offset])?,
            )));
        }
    }
    if tail.len() >= budget.string_bytes {
        return Err(Problem::Budget);
    }
    Ok(None)
}

#[cfg(test)]
#[path = "options_defaults_discovery_tests.rs"]
mod tests;
