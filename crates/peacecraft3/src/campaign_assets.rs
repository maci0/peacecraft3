//! Source preparation follows executed paths, never a chapter-specific asset list.
use pcx_assets::Resolver;
use pcx_play::{
    day_night::{Assets, Budget, ModelInput},
    scripts::{AssetIdentity, ScriptAsset},
};

/// Session-owned cache; the same source model is retained on every executed unit.
pub(crate) struct UnitModels {
    reader: std::sync::Arc<Resolver>,
    edition: String,
    models: std::collections::BTreeMap<String, std::sync::Arc<pcx_play::unit_model::Model>>,
    remaining_bytes: usize,
}

impl UnitModels {
    pub(crate) fn new(reader: std::sync::Arc<Resolver>, edition: String) -> Self {
        Self {
            reader,
            edition,
            models: Default::default(),
            remaining_bytes: 128 << 20,
        }
    }

    pub(crate) fn load(
        &mut self,
        definition: &pcx_sim::world::Definition,
    ) -> Result<Option<std::sync::Arc<pcx_play::unit_model::Model>>, String> {
        let Some(path) = pcx_play::unit_model::model_path(definition)? else {
            return Ok(None);
        };
        let key = path.replace('\\', "/").to_ascii_lowercase();
        if let Some(model) = self.models.get(&key) {
            return Ok(Some(std::sync::Arc::clone(model)));
        }
        if self.models.len() >= 512 {
            return Err("unit model count budget exceeded".into());
        }
        let asset = self
            .reader
            .resolve_bounded(&path, self.remaining_bytes.min(16 << 20) as u64)
            .map_err(|e| format!("{path}: {e}"))?;
        let (_, origin) = identity(&asset, &self.edition, 16 << 10)?;
        let size = asset.bytes.len();
        let model = std::sync::Arc::new(pcx_play::unit_model::Model::read(
            origin,
            path,
            asset.bytes.into(),
            16 << 20,
        )?);
        self.remaining_bytes = self
            .remaining_bytes
            .checked_sub(size)
            .ok_or("unit model aggregate byte budget exceeded")?;
        self.models.insert(key, std::sync::Arc::clone(&model));
        Ok(Some(model))
    }
}

pub(crate) fn day_night(
    reader: &Resolver,
    edition: &str,
    paths: [&str; 2],
    budget: Budget,
) -> Result<Assets, String> {
    if paths.iter().any(|path| path.len() > budget.path_bytes) || edition.len() > budget.path_bytes
    {
        return Err("SetDayNightModels: source path/identity budget exceeded".into());
    }
    let paths = if paths[0] == paths[1] {
        &paths[..1]
    } else {
        &paths[..]
    };
    if paths.len() > budget.models {
        return Err("SetDayNightModels: source model count budget exceeded".into());
    }
    let mut remaining = budget.total_bytes;
    let mut loaded = Vec::new();
    for path in paths {
        let member = pcx_mdl::binary_model_path(path);
        let limit = budget.model_bytes.min(remaining);
        let asset = reader
            .resolve_bounded(&member, limit as u64)
            .map_err(|error| format!("SetDayNightModels {path} -> {member}: {error}"))?;
        remaining = remaining.checked_sub(asset.bytes.len()).ok_or_else(|| {
            format!("SetDayNightModels {path}: aggregate source byte budget exceeded")
        })?;
        let (label, origin) = identity(&asset, edition, budget.path_bytes)
            .map_err(|error| format!("SetDayNightModels {path}: {error}"))?;
        loaded.push((asset, label, origin));
    }
    let inputs: Vec<_> = paths
        .iter()
        .zip(&loaded)
        .map(|(path, (asset, label, origin))| ModelInput {
            script_path: path,
            asset: ScriptAsset {
                label,
                bytes: &asset.bytes,
            },
            origin: origin.clone(),
        })
        .collect();
    Assets::read(&inputs, budget).map_err(|error| format!("SetDayNightModels: {error}"))
}

pub(crate) fn file_sound(
    reader: &Resolver,
    edition: &str,
    member: &str,
    budget: pcx_play::file_sounds::Budget,
    mp3: pcx_play::file_sounds::Mp3Budget,
) -> Result<pcx_play::file_sounds::Assets, String> {
    // Application metadata cap also bounds adapter identity formatting before copies.
    let metadata = budget.metadata_bytes.min(16 << 10);
    if member.len() > metadata || edition.len() > metadata || budget.resources == 0 {
        return Err("CreateSound: source path/identity/count budget exceeded".into());
    }
    let asset = reader
        .resolve_bounded(member, budget.resource_bytes.min(budget.total_bytes) as u64)
        .map_err(|error| format!("CreateSound {member}: {error}"))?;
    let (label, origin) = identity(&asset, edition, metadata)
        .map_err(|error| format!("CreateSound {member}: {error}"))?;
    pcx_play::file_sounds::Assets::read_encoded(
        &[pcx_play::file_sounds::SoundInput {
            member,
            asset: ScriptAsset {
                label: &label,
                bytes: &asset.bytes,
            },
            origin,
        }],
        budget,
        mp3,
    )
    .map_err(|error| format!("CreateSound {member}: {error}"))
}

pub(crate) struct SoundSources {
    tables: Vec<ParameterTable>,
    environments: Vec<pcx_play::sound_environment::SoundEnvironment>,
}

struct ParameterTable {
    asset: pcx_assets::ResolvedAsset,
    label: String,
    origin: AssetIdentity,
    table: pcx_slq::Table,
    column: String,
    profile: Option<pcx_play::sound_parameters::Profile>,
}

impl SoundSources {
    pub(crate) fn empty_label_bank(
        &self,
        budget: pcx_play::label_sounds::Budget,
    ) -> Result<pcx_play::label_sounds::SoundBank, String> {
        let source = self
            .tables
            .iter()
            .find(|source| {
                source.table.headers().any(|(_, name)| name == "FileNames")
                    && source
                        .table
                        .headers()
                        .any(|(_, name)| name == "DirectoryBase")
            })
            .ok_or("selected sources contain no sound constructor table")?;
        pcx_play::label_sounds::SoundBank::read(
            ScriptAsset {
                label: &source.label,
                bytes: &source.asset.bytes,
            },
            source.origin.clone(),
            &source.column,
            &[],
            budget,
        )
        .map_err(|e| e.to_string())
    }

    /// SoundInfo is the lookup namespace; actual table names/columns/rows come
    /// from the selected inventory and table headers, not a chapter manifest.
    pub(crate) fn read(
        reader: &Resolver,
        edition: &str,
        budget: pcx_play::sound_parameters::BanksBudget,
    ) -> Result<Self, String> {
        let mut tables = Vec::new();
        let mut environments = Vec::new();
        let mut remaining = budget.total_table_bytes;
        for (visited, path) in reader
            .paths()
            .filter(|p| p.as_str().starts_with("ui/soundinfo/") && p.as_str().ends_with(".slk"))
            .enumerate()
        {
            if visited >= budget.banks {
                return Err("sound parameter table count budget exceeded".into());
            }
            let asset = reader
                .resolve_bounded(path.as_str(), budget.table_bytes.min(remaining) as u64)
                .map_err(|e| e.to_string())?;
            remaining = remaining
                .checked_sub(asset.bytes.len())
                .ok_or("sound parameter aggregate table budget exceeded")?;
            let (label, origin) = identity(&asset, edition, budget.metadata_bytes.min(16 << 10))?;
            let table = pcx_slq::Table::parse_bounded(&asset.bytes, budget.coordinates)
                .map_err(|e| format!("{path}: {e}"))?;
            let has = |field| table.headers().any(|(_, name)| name == field);
            if has("EnvironmentType") {
                let found = pcx_play::sound_environment::from_table(&table)
                    .map_err(|e| format!("{path}: {e}"))?;
                for environment in found {
                    if environments.iter().any(
                        |known: &pcx_play::sound_environment::SoundEnvironment| {
                            known.name == environment.name
                        },
                    ) {
                        return Err(format!(
                            "{path}: duplicate sound environment {}",
                            environment.name
                        ));
                    }
                    environments.push(environment);
                }
                continue;
            }
            // Lookup/reference tables can carry SoundLabel without sound parameters.
            if !has("Volume") && !has("Flags") {
                continue;
            }
            let columns: Vec<_> = table
                .headers()
                .filter(|(_, name)| matches!(*name, "SoundName" | "SoundLabel"))
                .collect();
            if columns.is_empty() {
                continue;
            }
            let [(_, column)] = columns.as_slice() else {
                return Err(format!("{path}: expected one sound-label column"));
            };
            let column = column.to_string();
            let full = [
                "Pitch",
                "PitchVariance",
                "Priority",
                "Channel",
                "MinDistance",
                "MaxDistance",
                "DistanceCutoff",
                "EAXFlags",
            ];
            let profile = if has("Volume") && has("Flags") && full.iter().all(|field| has(*field)) {
                Some(pcx_play::sound_parameters::Profile::Full)
            } else if full.iter().all(|field| !has(*field))
                && ["Volume", "Flags", "version", "DirectoryBase", "FileNames"]
                    .iter()
                    .all(|field| has(*field))
            {
                Some(pcx_play::sound_parameters::Profile::Dialogue)
            } else {
                None
            };
            tables.push(ParameterTable {
                asset,
                label,
                origin,
                table,
                column,
                profile,
            });
        }
        Ok(Self {
            tables,
            environments,
        })
    }

    /// Environment records from every `EnvironmentType` table in the selected sources.
    pub(crate) fn environments(&self) -> &[pcx_play::sound_environment::SoundEnvironment] {
        &self.environments
    }

    fn source(&self, label: &str, metadata: usize, pcm: bool) -> Result<&ParameterTable, String> {
        if label.trim().is_empty() || label.contains('\0') || label.len() > metadata.min(16 << 10) {
            return Err("sound parameter label/metadata budget or identity failure".into());
        }
        let mut owners = self.tables.iter().filter(|source| {
            (!pcm || source.table.headers().any(|(_, name)| name == "FileNames"))
                && source
                    .table
                    .data_rows()
                    .any(|row| row.by_name(&source.column) == Some(label))
        });
        let source = owners
            .next()
            .ok_or_else(|| format!("sound parameter label is absent: {label}"))?;
        if owners.next().is_some() {
            return Err(format!("ambiguous sound parameter label: {label}"));
        }
        Ok(source)
    }

    pub(crate) fn label_sound(
        &self,
        reader: &Resolver,
        edition: &str,
        label: &str,
        budget: pcx_play::label_sounds::Budget,
    ) -> Result<pcx_play::label_sounds::SoundBank, String> {
        let source = self.source(label, budget.metadata_bytes, true)?;
        let mut rows = source
            .table
            .data_rows()
            .filter(|r| r.by_name(&source.column) == Some(label));
        let row = rows.next().ok_or("missing label row")?;
        if rows.next().is_some() {
            return Err(format!("ambiguous sound label row: {label}"));
        }
        let names = row.by_name("FileNames").ok_or("missing sound file names")?;
        let mut files = names.split(',').filter(|f| !f.is_empty());
        let file = files.next().ok_or("missing sound file name")?;
        if file == "_" || files.next().is_some() {
            return Err(format!(
                "{label}: disabled or multiple files need variant selection"
            ));
        }
        let base = row
            .by_name("DirectoryBase")
            .ok_or("missing sound directory")?;
        if base
            .len()
            .checked_add(file.len())
            .and_then(|n| n.checked_add(1))
            .is_none_or(|n| n > budget.metadata_bytes.min(16 << 10))
        {
            return Err("label sound path byte budget exceeded".into());
        }
        let member = pcx_slq::SoundTable(&source.table)
            .sound_path(row.index(), 0)
            .ok_or("missing sound path")?;
        let asset = reader
            .resolve_bounded(
                &member,
                budget.resource_bytes.min(budget.total_resource_bytes) as u64,
            )
            .map_err(|e| format!("{label} -> {member}: {e}"))?;
        let (resource_label, origin) =
            identity(&asset, edition, budget.metadata_bytes.min(16 << 10))?;
        pcx_play::label_sounds::SoundBank::read(
            ScriptAsset {
                label: &source.label,
                bytes: &source.asset.bytes,
            },
            source.origin.clone(),
            &source.column,
            &[pcx_play::label_sounds::SoundInput {
                label,
                member: &member,
                asset: ScriptAsset {
                    label: &resource_label,
                    bytes: &asset.bytes,
                },
                origin,
            }],
            budget,
        )
        .map_err(|e| e.to_string())
    }

    pub(crate) fn bind(
        &self,
        label: &str,
        budget: pcx_play::sound_parameters::BanksBudget,
    ) -> Result<pcx_play::sound_parameters::Banks, String> {
        let source = self.source(label, budget.metadata_bytes, false)?;
        let profile = source
            .profile
            .ok_or_else(|| format!("{}: unsupported sound parameter schema", source.label))?;
        pcx_play::sound_parameters::Banks::read(
            &[pcx_play::sound_parameters::Input {
                asset: ScriptAsset {
                    label: &source.label,
                    bytes: &source.asset.bytes,
                },
                origin: &source.origin,
                label_column: &source.column,
                labels: &[label],
                profile,
            }],
            budget,
        )
        .map_err(|e| e.to_string())
    }
}

pub(crate) fn identity(
    asset: &pcx_assets::ResolvedAsset,
    edition: &str,
    limit: usize,
) -> Result<(String, AssetIdentity), String> {
    let source = &asset.provenance.source;
    let bytes = [&source.id, &source.origin, &asset.provenance.locator]
        .iter()
        .try_fold(0usize, |size, value| size.checked_add(value.len()))
        .and_then(|size| size.checked_add(source.version.as_ref().map_or(0, String::len)));
    if edition.len() > limit || bytes.is_none_or(|bytes| bytes > limit) {
        return Err("source identity budget exceeded".into());
    }
    Ok((
        format!("{}::{}", source.origin, asset.provenance.locator),
        AssetIdentity {
            edition: edition.to_owned(),
            version: source
                .version
                .clone()
                .unwrap_or_else(|| "unversioned source".into()),
            source: serde_json::to_string(&asset.provenance).map_err(|error| error.to_string())?,
        },
    ))
}

#[cfg(test)]
mod preparation_tests {
    use super::*;
    #[test]
    fn absent_constructor_table_is_not_replaced_with_synthetic_sound_content() {
        let sources = SoundSources { tables: Vec::new() };
        let budget = pcx_play::label_sounds::Budget {
            table_bytes: 1024,
            coordinates: pcx_slq::CoordinateLimits {
                max_row: 16,
                max_column: 16,
            },
            sounds: 1,
            resource_bytes: 1024,
            total_resource_bytes: 1024,
            metadata_bytes: 1024,
        };
        let error = sources
            .empty_label_bank(budget)
            .expect_err("missing source table");
        assert_eq!(error, "selected sources contain no sound constructor table");
    }
}
