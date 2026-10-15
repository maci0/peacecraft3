//! Caller-selected sound-table bindings, preserving authored settings without defaults.
use crate::neutral_player::{Sha256Digest, sha256};
use crate::scripts::{AssetIdentity, ScriptAsset};
use pcx_slq::{CoordinateLimits, SoundTable, Table};
#[path = "pcm_wave.rs"]
mod wave;
pub use wave::{PcmWave, WaveError};

pub struct SoundInput<'a> {
    pub label: &'a str,
    /// Exact derived archive path; source_label may also include the archive identity.
    pub member: &'a str,
    pub asset: ScriptAsset<'a>,
    pub origin: AssetIdentity,
}
#[derive(Clone, Copy)]
pub struct Budget {
    pub table_bytes: usize,
    pub coordinates: CoordinateLimits,
    pub sounds: usize,
    pub resource_bytes: usize,
    pub total_resource_bytes: usize,
    /// Aggregate labels, member paths, source labels and identity strings.
    pub metadata_bytes: usize,
}
#[derive(Debug, thiserror::Error)]
#[error("sound input {source_context}: {problem}")]
pub struct Error {
    pub source_context: String,
    pub problem: Problem,
}
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    #[error("input count, byte or platform layout budget exceeded")]
    Budget,
    #[error("empty source identity, label or path")]
    Identity,
    #[error("SLK: {0}")]
    Table(#[source] pcx_slq::Error),
    #[error("ambiguous/noncanonical table, selected header, binding or row")]
    Ambiguous,
    #[error("required selected row or file field is missing")]
    Missing,
    #[error("multiple files or disabled file entry require an explicit implementation")]
    UnsupportedFiles,
    #[error("provided member does not match the source-derived file path")]
    Member,
    #[error("{0}")]
    Wave(#[source] WaveError),
}
#[derive(Debug)]
pub struct BoundSound {
    label: String,
    row: u32,
    member: String,
    source_label: String,
    origin: AssetIdentity,
    sha256: Sha256Digest,
    bytes: Vec<u8>,
    pcm: PcmWave,
}
impl BoundSound {
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn row(&self) -> u32 {
        self.row
    }
    pub fn member(&self) -> &str {
        &self.member
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
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn pcm(&self) -> &PcmWave {
        &self.pcm
    }
}
#[derive(Debug)]
pub struct SoundBank {
    table: Table,
    source_label: String,
    origin: AssetIdentity,
    sha256: Sha256Digest,
    sounds: Vec<BoundSound>,
    label_limit: usize,
}
fn identity(origin: &AssetIdentity) -> bool {
    [&origin.source, &origin.edition, &origin.version]
        .iter()
        .all(|s| !s.trim().is_empty())
}
impl SoundBank {
    /// Exact selected table/label column, with the existing FileNames/DirectoryBase schema.
    /// Logical input limits do not promise a total decoder-heap or RSS bound.
    pub fn read(
        table: ScriptAsset<'_>,
        origin: AssetIdentity,
        label_column: &str,
        inputs: &[SoundInput<'_>],
        budget: Budget,
    ) -> Result<Self, Error> {
        let fail = |source: &str, problem| Error {
            source_context: source.into(),
            problem,
        };
        std::alloc::Layout::array::<BoundSound>(budget.sounds)
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.table_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.resource_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.total_resource_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.metadata_bytes))
            .map_err(|_| fail(table.label, Problem::Budget))?;
        if table.bytes.len() > budget.table_bytes || inputs.len() > budget.sounds {
            return Err(fail(table.label, Problem::Budget));
        }
        if table.label.trim().is_empty() || label_column.trim().is_empty() || !identity(&origin) {
            return Err(fail(table.label, Problem::Identity));
        }
        let mut metadata = 0usize;
        let mut add = |s: &str| -> Result<(), Error> {
            metadata = metadata
                .checked_add(s.len())
                .filter(|&n| n <= budget.metadata_bytes)
                .ok_or_else(|| fail(table.label, Problem::Budget))?;
            Ok(())
        };
        for s in [
            table.label,
            label_column,
            &origin.source,
            &origin.edition,
            &origin.version,
        ] {
            add(s)?;
        }
        let mut total = 0usize;
        for (i, input) in inputs.iter().enumerate() {
            if !identity(&input.origin)
                || [input.label, input.member, input.asset.label]
                    .iter()
                    .any(|s| s.trim().is_empty())
            {
                return Err(fail(input.asset.label, Problem::Identity));
            }
            if inputs[..i].iter().any(|old| old.label == input.label) {
                return Err(fail(input.asset.label, Problem::Ambiguous));
            }
            for s in [
                input.label,
                input.member,
                input.asset.label,
                &input.origin.source,
                &input.origin.edition,
                &input.origin.version,
            ] {
                add(s)?;
            }
            total = total
                .checked_add(input.asset.bytes.len())
                .filter(|&n| n <= budget.total_resource_bytes)
                .ok_or_else(|| fail(input.asset.label, Problem::Budget))?;
            if input.asset.bytes.len() > budget.resource_bytes {
                return Err(fail(input.asset.label, Problem::Budget));
            }
        }
        let decoded = Table::parse_bounded(table.bytes, budget.coordinates)
            .map_err(|e| fail(table.label, Problem::Table(e)))?;
        if decoded.anomalies().iter().any(|a| {
            a.severity == pcx_slq::Severity::Error
                || matches!(
                    a.kind,
                    pcx_slq::AnomalyKind::DuplicateCell | pcx_slq::AnomalyKind::StrayDataAfterEnd
                )
        }) {
            return Err(fail(table.label, Problem::Ambiguous));
        }
        let required = [label_column, "DirectoryBase", "FileNames"];
        for (i, name) in required.iter().enumerate() {
            if required[..i].contains(name)
                || decoded.headers().filter(|(_, h)| h == name).count() != 1
            {
                return Err(fail(table.label, Problem::Ambiguous));
            }
        }
        let mut sounds = Vec::new();
        sounds
            .try_reserve_exact(inputs.len())
            .map_err(|_| fail(table.label, Problem::Budget))?;
        for input in inputs {
            let mut matches = decoded
                .data_rows()
                .filter(|r| r.by_name(label_column) == Some(input.label));
            let row = matches
                .next()
                .ok_or_else(|| fail(table.label, Problem::Missing))?;
            if matches.next().is_some() {
                return Err(fail(table.label, Problem::Ambiguous));
            }
            let mut names = row
                .by_name("FileNames")
                .ok_or_else(|| fail(table.label, Problem::Missing))?
                .split(',')
                .filter(|s| !s.is_empty());
            let file = names
                .next()
                .ok_or_else(|| fail(table.label, Problem::Missing))?;
            if file == "_" || names.next().is_some() {
                return Err(fail(table.label, Problem::UnsupportedFiles));
            }
            let base = row
                .by_name("DirectoryBase")
                .ok_or_else(|| fail(table.label, Problem::Missing))?;
            base.len()
                .checked_add(file.len())
                .and_then(|n| n.checked_add(1))
                .filter(|&n| n <= budget.metadata_bytes)
                .ok_or_else(|| fail(table.label, Problem::Budget))?;
            let member = SoundTable(&decoded)
                .sound_path(row.index(), 0)
                .ok_or_else(|| fail(table.label, Problem::Missing))?;
            if member != input.member {
                return Err(fail(input.asset.label, Problem::Member));
            }
            let pcm = PcmWave::inspect(input.asset.bytes)
                .map_err(|e| fail(input.asset.label, Problem::Wave(e)))?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(input.asset.bytes.len())
                .map_err(|_| fail(input.asset.label, Problem::Budget))?;
            bytes.extend_from_slice(input.asset.bytes);
            sounds.push(BoundSound {
                label: input.label.into(),
                row: row.index(),
                member,
                source_label: input.asset.label.into(),
                origin: input.origin.clone(),
                sha256: sha256(&bytes),
                bytes,
                pcm,
            });
        }
        Ok(Self {
            table: decoded,
            source_label: table.label.into(),
            origin,
            sha256: sha256(table.bytes),
            sounds,
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
    pub fn sounds(&self) -> &[BoundSound] {
        &self.sounds
    }
    pub(crate) fn label_limit(&self) -> usize {
        self.label_limit
    }
}

#[cfg(test)]
#[path = "label_sound_tests.rs"]
mod tests;
