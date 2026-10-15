//! Explicit sound-label parameter definitions, separate from PCM file bindings.
pub use crate::label_sounds::{Error, Problem};
use crate::label_sounds::{checked_table, identity};
use crate::neutral_player::{Sha256Digest, sha256};
use crate::scripts::{AssetIdentity, ScriptAsset};
use pcx_slq::{CoordinateLimits, Row, Table};

#[derive(Clone, Copy)]
pub struct Budget {
    pub table_bytes: usize,
    pub coordinates: CoordinateLimits,
    pub definitions: usize,
    /// Aggregate selected labels, column, source label and identity strings.
    pub metadata_bytes: usize,
}
/// Raw source numbers. No gain normalization, distance units, clamping or
/// native flag/channel mapping is implied by parsing these fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Values {
    pub volume: i32,
    pub pitch: f32,
    pub pitch_variance: f32,
    pub priority: i32,
    pub channel: i32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub distance_cutoff: f32,
}
#[derive(Debug)]
struct Definition {
    row: u32,
    values: Values,
}
#[derive(Debug)]
pub struct Parameters<'a> {
    pub label: &'a str,
    pub row: u32,
    pub values: &'a Values,
    /// Opaque authored declarations, not effective flags after object creation.
    pub flags: &'a str,
    pub eax_flags: &'a str,
}
#[derive(Debug)]
pub struct Bank {
    table: Table,
    source_label: String,
    origin: AssetIdentity,
    sha256: Sha256Digest,
    label_column: String,
    definitions: Vec<Definition>,
    label_limit: usize,
}
fn values(row: &Row<'_>) -> Result<Values, Problem> {
    let error = |field| Problem::Parameter {
        row: row.index(),
        field,
    };
    let integer = |field| {
        row.by_name(field)
            .and_then(|s| s.parse::<i32>().ok())
            .ok_or_else(|| error(field))
    };
    let real = |field| {
        row.by_name(field)
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|n| n.is_finite())
            .ok_or_else(|| error(field))
    };
    for field in ["Flags", "EAXFlags"] {
        if row.by_name(field).is_none_or(|s| s.contains('\0')) {
            return Err(error(field));
        }
    }
    Ok(Values {
        volume: integer("Volume")?,
        pitch: real("Pitch")?,
        pitch_variance: real("PitchVariance")?,
        priority: integer("Priority")?,
        channel: integer("Channel")?,
        min_distance: real("MinDistance")?,
        max_distance: real("MaxDistance")?,
        distance_cutoff: real("DistanceCutoff")?,
    })
}
impl Bank {
    /// Caller selects one table and its exact label column. Only selected rows
    /// must satisfy this numeric profile; no file selection or extraction occurs.
    pub fn read(
        input: ScriptAsset<'_>,
        origin: AssetIdentity,
        label_column: &str,
        labels: &[&str],
        budget: Budget,
    ) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: input.label.into(),
            problem,
        };
        std::alloc::Layout::array::<u8>(budget.table_bytes)
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.metadata_bytes))
            .and_then(|_| std::alloc::Layout::array::<Definition>(budget.definitions))
            .map_err(|_| fail(Problem::Budget))?;
        if input.bytes.len() > budget.table_bytes || labels.len() > budget.definitions {
            return Err(fail(Problem::Budget));
        }
        let metadata = [
            input.label,
            label_column,
            &origin.source,
            &origin.edition,
            &origin.version,
        ];
        if !identity(&origin)
            || metadata
                .iter()
                .chain(labels)
                .any(|s| s.trim().is_empty() || s.contains('\0'))
        {
            return Err(fail(Problem::Identity));
        }
        metadata
            .iter()
            .chain(labels)
            .try_fold(0usize, |sum, s| {
                sum.checked_add(s.len())
                    .filter(|&n| n <= budget.metadata_bytes)
            })
            .ok_or_else(|| fail(Problem::Budget))?;
        for (i, label) in labels.iter().enumerate() {
            if labels[..i].contains(label) {
                return Err(fail(Problem::Ambiguous));
            }
        }
        let table = checked_table(
            input,
            budget.coordinates,
            &[
                label_column,
                "Volume",
                "Pitch",
                "PitchVariance",
                "Priority",
                "Channel",
                "MinDistance",
                "MaxDistance",
                "DistanceCutoff",
                "Flags",
                "EAXFlags",
            ],
        )?;
        let mut definitions = Vec::new();
        definitions
            .try_reserve_exact(labels.len())
            .map_err(|_| fail(Problem::Budget))?;
        for label in labels {
            let mut rows = table
                .data_rows()
                .filter(|r| r.by_name(label_column) == Some(label));
            let row = rows.next().ok_or_else(|| fail(Problem::Missing))?;
            if rows.next().is_some() {
                return Err(fail(Problem::Ambiguous));
            }
            definitions.push(Definition {
                row: row.index(),
                values: values(&row).map_err(fail)?,
            });
        }
        Ok(Self {
            table,
            source_label: input.label.into(),
            origin,
            sha256: sha256(input.bytes),
            label_column: label_column.into(),
            definitions,
            label_limit: budget.metadata_bytes,
        })
    }
    pub fn table(&self) -> &Table {
        &self.table
    }
    pub fn source_label(&self) -> &str {
        &self.source_label
    }
    pub fn origin(&self) -> &AssetIdentity {
        &self.origin
    }
    pub fn sha256(&self) -> Sha256Digest {
        self.sha256
    }
    pub fn get(&self, index: usize) -> Option<Parameters<'_>> {
        let d = self.definitions.get(index)?;
        Some(Parameters {
            label: self.table.by_name(d.row, &self.label_column)?,
            row: d.row,
            values: &d.values,
            flags: self.table.by_name(d.row, "Flags")?,
            eax_flags: self.table.by_name(d.row, "EAXFlags")?,
        })
    }
    pub(crate) fn find(&self, label: &str) -> Option<usize> {
        self.definitions
            .iter()
            .position(|d| self.table.by_name(d.row, &self.label_column) == Some(label))
    }
    pub(crate) fn label_limit(&self) -> usize {
        self.label_limit
    }
}

#[cfg(test)]
#[path = "sound_parameter_tests.rs"]
mod tests;
