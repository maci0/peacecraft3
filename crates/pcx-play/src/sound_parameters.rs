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
fn preflight(
    input: ScriptAsset<'_>,
    origin: &AssetIdentity,
    label_column: &str,
    labels: &[&str],
    budget: Budget,
) -> Result<usize, Error> {
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
    if !identity(origin)
        || metadata
            .iter()
            .chain(labels)
            .any(|s| s.trim().is_empty() || s.contains('\0'))
    {
        return Err(fail(Problem::Identity));
    }
    let metadata_bytes = metadata
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
    Ok(metadata_bytes)
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
        preflight(input, &origin, label_column, labels, budget)?;
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
/// Explicit source schema; absent dialogue fields never become full values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Full,
    Dialogue,
}
impl std::fmt::Display for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Full => "full numeric",
            Self::Dialogue => "dialogue",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogueValues {
    pub volume: i32,
    pub version: i32,
}
#[derive(Debug)]
pub struct DialogueParameters<'a> {
    pub label: &'a str,
    pub row: u32,
    pub values: &'a DialogueValues,
    pub flags: &'a str,
    /// Source declarations only; no file variant selection is implied.
    pub directory_base: &'a str,
    pub file_names: &'a str,
}
#[derive(Debug)]
pub enum ParameterValues<'a> {
    Full(Parameters<'a>),
    Dialogue(DialogueParameters<'a>),
}
/// Receipt for exactly the table owning an assignment's row.
#[derive(Debug, Clone, Copy)]
pub struct BankSource<'a> {
    pub profile: Profile,
    pub table: &'a Table,
    pub source_label: &'a str,
    pub label_column: &'a str,
    pub origin: &'a AssetIdentity,
    pub sha256: Sha256Digest,
}
#[derive(Debug)]
pub struct Assignment<'a> {
    pub source: BankSource<'a>,
    pub parameters: ParameterValues<'a>,
}
#[derive(Debug)]
struct DialogueDefinition {
    row: u32,
    values: DialogueValues,
}
#[derive(Debug)]
struct DialogueBank {
    table: Table,
    source_label: String,
    origin: AssetIdentity,
    sha256: Sha256Digest,
    label_column: String,
    definitions: Vec<DialogueDefinition>,
}
impl DialogueBank {
    fn read(
        input: ScriptAsset<'_>,
        origin: AssetIdentity,
        column: &str,
        labels: &[&str],
        budget: Budget,
    ) -> Result<Self, Error> {
        preflight(input, &origin, column, labels, budget)?;
        let fail = |problem| Error {
            source_context: input.label.into(),
            problem,
        };
        let table = checked_table(
            input,
            budget.coordinates,
            &[
                column,
                "Volume",
                "Flags",
                "version",
                "DirectoryBase",
                "FileNames",
            ],
        )?;
        // Selecting this schema must not discard authored spatial/numeric fields.
        for field in [
            "Pitch",
            "PitchVariance",
            "Priority",
            "Channel",
            "MinDistance",
            "MaxDistance",
            "DistanceCutoff",
            "EAXFlags",
        ] {
            if table.headers().any(|(_, name)| name == field) {
                return Err(fail(Problem::Parameter { row: 1, field }));
            }
        }
        let mut definitions = Vec::new();
        definitions
            .try_reserve_exact(labels.len())
            .map_err(|_| fail(Problem::Budget))?;
        for label in labels {
            let mut rows = table
                .data_rows()
                .filter(|row| row.by_name(column) == Some(label));
            let row = rows.next().ok_or_else(|| fail(Problem::Missing))?;
            if rows.next().is_some() {
                return Err(fail(Problem::Ambiguous));
            }
            let parameter = |field| {
                fail(Problem::Parameter {
                    row: row.index(),
                    field,
                })
            };
            let integer = |field| {
                row.by_name(field)
                    .and_then(|value| value.parse::<i32>().ok())
                    .ok_or_else(|| parameter(field))
            };
            for field in ["Flags", "DirectoryBase", "FileNames"] {
                if row.by_name(field).is_none_or(|value| value.contains('\0')) {
                    return Err(parameter(field));
                }
            }
            definitions.push(DialogueDefinition {
                row: row.index(),
                values: DialogueValues {
                    volume: integer("Volume")?,
                    version: integer("version")?,
                },
            });
        }
        Ok(Self {
            table,
            source_label: input.label.into(),
            origin,
            sha256: sha256(input.bytes),
            label_column: column.into(),
            definitions,
        })
    }
    fn get(&self, index: usize) -> Option<DialogueParameters<'_>> {
        let definition = self.definitions.get(index)?;
        Some(DialogueParameters {
            label: self.table.by_name(definition.row, &self.label_column)?,
            row: definition.row,
            values: &definition.values,
            flags: self.table.by_name(definition.row, "Flags")?,
            directory_base: self.table.by_name(definition.row, "DirectoryBase")?,
            file_names: self.table.by_name(definition.row, "FileNames")?,
        })
    }
}

/// Every input is an explicit table/column/profile selection; no lookup priority.
#[derive(Clone, Copy)]
pub struct Input<'a> {
    pub asset: ScriptAsset<'a>,
    pub origin: &'a AssetIdentity,
    pub label_column: &'a str,
    pub labels: &'a [&'a str],
    pub profile: Profile,
}
#[derive(Clone, Copy)]
pub struct BanksBudget {
    pub banks: usize,
    pub table_bytes: usize,
    pub total_table_bytes: usize,
    pub coordinates: CoordinateLimits,
    /// Aggregate selected definitions across all banks.
    pub definitions: usize,
    /// Aggregate source labels, columns, identities and selected label text.
    pub metadata_bytes: usize,
}
#[derive(Debug)]
enum SourceBank {
    Full(Bank),
    Dialogue(DialogueBank),
}
impl SourceBank {
    fn source(&self) -> BankSource<'_> {
        match self {
            Self::Full(bank) => BankSource {
                profile: Profile::Full,
                table: &bank.table,
                source_label: &bank.source_label,
                label_column: &bank.label_column,
                origin: &bank.origin,
                sha256: bank.sha256,
            },
            Self::Dialogue(bank) => BankSource {
                profile: Profile::Dialogue,
                table: &bank.table,
                source_label: &bank.source_label,
                label_column: &bank.label_column,
                origin: &bank.origin,
                sha256: bank.sha256,
            },
        }
    }
    fn get(&self, definition: usize) -> Option<Assignment<'_>> {
        let parameters = match self {
            Self::Full(bank) => ParameterValues::Full(bank.get(definition)?),
            Self::Dialogue(bank) => ParameterValues::Dialogue(bank.get(definition)?),
        };
        Some(Assignment {
            source: self.source(),
            parameters,
        })
    }
    fn find(&self, label: &str) -> Option<usize> {
        match self {
            Self::Full(bank) => bank.find(label),
            Self::Dialogue(bank) => bank
                .definitions
                .iter()
                .position(|d| bank.table.by_name(d.row, &bank.label_column) == Some(label)),
        }
    }
}
/// Only admitted definitions mint these internal source-qualified locators.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ParameterKey {
    bank: usize,
    definition: usize,
}
#[derive(Debug)]
pub struct Banks {
    banks: Vec<SourceBank>,
    label_limit: usize,
}
impl Banks {
    /// Whole-set counts, layouts, bytes, identities and label uniqueness precede
    /// table decoding, source copies and installation. Limits are logical inputs,
    /// not a promise of total parser heap/RSS bounds.
    pub fn read(inputs: &[Input<'_>], budget: BanksBudget) -> Result<Self, Error> {
        let fail = |problem| Error {
            source_context: "selected sound parameter banks".into(),
            problem,
        };
        std::alloc::Layout::array::<SourceBank>(budget.banks)
            .and_then(|_| std::alloc::Layout::array::<Definition>(budget.definitions))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.table_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.metadata_bytes))
            .and_then(|_| std::alloc::Layout::array::<ParameterKey>(budget.definitions))
            .and_then(|_| std::alloc::Layout::array::<DialogueDefinition>(budget.definitions))
            .and_then(|_| std::alloc::Layout::array::<&str>(budget.definitions))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.total_table_bytes))
            .map_err(|_| fail(Problem::Budget))?;
        if inputs.len() > budget.banks {
            return Err(fail(Problem::Budget));
        }
        let leaf = Budget {
            table_bytes: budget.table_bytes,
            coordinates: budget.coordinates,
            definitions: budget.definitions,
            metadata_bytes: budget.metadata_bytes,
        };
        let (mut count, mut bytes, mut metadata) = (0usize, 0usize, 0usize);
        for input in inputs {
            let size = preflight(
                input.asset,
                input.origin,
                input.label_column,
                input.labels,
                leaf,
            )?;
            count = count
                .checked_add(input.labels.len())
                .filter(|&n| n <= budget.definitions)
                .ok_or_else(|| fail(Problem::Budget))?;
            bytes = bytes
                .checked_add(input.asset.bytes.len())
                .filter(|&n| n <= budget.total_table_bytes)
                .ok_or_else(|| fail(Problem::Budget))?;
            metadata = metadata
                .checked_add(size)
                .filter(|&n| n <= budget.metadata_bytes)
                .ok_or_else(|| fail(Problem::Budget))?;
        }
        let mut labels = Vec::new();
        labels
            .try_reserve_exact(count)
            .map_err(|_| fail(Problem::Budget))?;
        labels.extend(inputs.iter().flat_map(|input| input.labels.iter().copied()));
        labels.sort_unstable();
        if labels.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(fail(Problem::Ambiguous));
        }
        let mut banks = Vec::new();
        banks
            .try_reserve_exact(inputs.len())
            .map_err(|_| fail(Problem::Budget))?;
        for input in inputs {
            banks.push(match input.profile {
                Profile::Full => SourceBank::Full(Bank::read(
                    input.asset,
                    input.origin.clone(),
                    input.label_column,
                    input.labels,
                    leaf,
                )?),
                Profile::Dialogue => SourceBank::Dialogue(DialogueBank::read(
                    input.asset,
                    input.origin.clone(),
                    input.label_column,
                    input.labels,
                    leaf,
                )?),
            });
        }
        Ok(Self {
            banks,
            label_limit: budget.metadata_bytes,
        })
    }
    pub fn sources(&self) -> impl Iterator<Item = BankSource<'_>> {
        self.banks.iter().map(SourceBank::source)
    }
    pub(crate) fn single(bank: Bank) -> Self {
        let label_limit = bank.label_limit();
        Self {
            banks: vec![SourceBank::Full(bank)],
            label_limit,
        }
    }
    pub(crate) fn single_full(&self) -> Option<&Bank> {
        match self.banks.as_slice() {
            [SourceBank::Full(bank)] => Some(bank),
            _ => None,
        }
    }
    pub(crate) fn get(&self, key: ParameterKey) -> Option<Assignment<'_>> {
        self.banks.get(key.bank)?.get(key.definition)
    }
    pub(crate) fn find(&self, label: &str) -> Option<ParameterKey> {
        // Whole-set uniqueness makes this bounded scan independent of bank order.
        self.banks.iter().enumerate().find_map(|(bank, source)| {
            source
                .find(label)
                .map(|definition| ParameterKey { bank, definition })
        })
    }
    pub(crate) fn label_limit(&self) -> usize {
        self.label_limit
    }
}

#[cfg(test)]
#[path = "sound_parameter_tests.rs"]
mod tests;
