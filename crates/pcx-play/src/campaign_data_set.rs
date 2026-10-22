//! Campaign data-set selection and scoped archive mounts read from a reviewed
//! client image plus the selected map header.
//!
//! The map supplies `loading_id` and `flags`; the image supplies the selector
//! operands, dataset names, namespace format, archive names and mount
//! priorities. No probe value is copied into this module.
use crate::{
    install_source::{MAX_AGGREGATE_BYTES, MAX_ASSET_BYTES, MAX_LISTED_PATHS},
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
use pcx_assets::{
    ArchiveLayer, AssetPath, ContentSource, GameMode, LayerKind, Resolver, SourceContent,
    SourceError, SourceInfo,
};
use pcx_map::info::MapInfo;
use pcx_script::{BinOp, Expr, Item, Literal, TypeKind, UnOp};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("campaign data set: {0}")]
    Message(String),
    #[error("campaign edition: {0}")]
    Edition(#[from] EditionError),
    #[error(transparent)]
    Assets(#[from] pcx_assets::Error),
    #[error(transparent)]
    Source(#[from] SourceError),
    #[error(transparent)]
    Mpq(#[from] pcx_mpq::Error),
    #[error(transparent)]
    Huffman(#[from] pcx_mpq::huffman::HuffmanError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct EditionError(String);

impl From<String> for Error {
    fn from(reason: String) -> Self {
        Self::Message(reason)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountSpec {
    pub archive: String,
    pub priority: u32,
}

#[derive(Clone, Debug)]
pub struct ImageRules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    pub selector_clamp: u32,
    pub flag_mask: u32,
    pub namespace_format: String,
    pub datasets: [String; 2],
    pub base: Vec<MountSpec>,
    selected: [Vec<MountSpec>; 2],
    localized_archives: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CampaignDataSet {
    pub dataset: usize,
    pub edition: i32,
    pub prefix: String,
}

#[derive(Clone, Debug)]
pub struct PlannedMember {
    pub asset: String,
    pub member: String,
}

#[derive(Clone, Debug)]
pub struct PlannedLayer {
    pub archive: String,
    pub priority: u32,
    pub prefix: Option<String>,
    pub members: Vec<PlannedMember>,
}

/// One archive opened once, shared by its unprefixed base layer and its
/// selected-namespace layer. Each layer owns the explicit member mapping that
/// belongs at that priority; the archive handle and seek reader are shared.
struct ArchiveInventorySource {
    info: SourceInfo,
    archive: Arc<Mutex<pcx_mpq::MpqArchive>>,
    members: BTreeMap<AssetPath, String>,
    huffman: pcx_mpq::huffman::HuffmanTable,
}

impl ContentSource for ArchiveInventorySource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }
    fn paths(&self) -> Result<Vec<String>, SourceError> {
        Ok(self.members.keys().map(ToString::to_string).collect())
    }
    fn read(&self, asset: &str) -> Result<Option<SourceContent>, SourceError> {
        self.read_bounded(asset, MAX_ASSET_BYTES as u64)
    }
    fn read_bounded(&self, asset: &str, limit: u64) -> Result<Option<SourceContent>, SourceError> {
        let path = AssetPath::new(asset).map_err(|error| SourceError::InvalidInput {
            detail: error.to_string(),
        })?;
        let Some(member) = self.members.get(&path) else {
            return Ok(None);
        };
        let mut archive = self.archive.lock().map_err(|_| SourceError::Unavailable {
            detail: "campaign archive reader lock is poisoned".into(),
        })?;
        let declared_length = archive
            .find_block(member)
            .ok_or_else(|| SourceError::InvalidInput {
                detail: format!("declared campaign member {member:?} disappeared"),
            })?
            .file_size as u64;
        let budget = pcx_mpq::ExtractionBudget {
            max_raw_packed_bytes: MAX_ASSET_BYTES,
            max_decoded_bytes: usize::try_from(limit.min(MAX_ASSET_BYTES as u64))
                .unwrap_or(MAX_ASSET_BYTES),
        };
        let locator = format!("{}::{member}", archive.path().display());
        let bytes = archive
            .extract_bounded(member, budget, Some(&self.huffman))
            .map_err(|error| SourceError::MpqExtraction(Box::new(error)))?;
        Ok(Some(SourceContent {
            bytes,
            declared_length,
            locator,
        }))
    }
}

/// Pinned campaign source generation. The reviewed image, the selected
/// `common.j` edition and the install/mode policy are fixed here, so a later
/// map header selects only the dataset and namespace from values already held;
/// no path or executable is reopened to rediscover them.
#[derive(Clone, Debug)]
pub struct CampaignRecipe {
    dir: PathBuf,
    mode: GameMode,
    rules: ImageRules,
    edition: i32,
    common_sha256: Sha256Digest,
    huffman: pcx_mpq::huffman::HuffmanTable,
    generation: Arc<ArchiveGeneration>,
}

impl CampaignRecipe {
    pub fn pin(
        dir: &Path,
        mode: GameMode,
        rules: ImageRules,
        image: &[u8],
        common: &[u8],
        edition_global: &str,
    ) -> Result<Self, Error> {
        if sha256(image) != rules.image_sha256 {
            return Err(Error::Message(
                "pinned campaign image does not match the admitted image rules".into(),
            ));
        }
        let huffman =
            pcx_mpq::huffman::HuffmanTable::from_pe(image, Path::new(&rules.origin.source))?;
        refuse_war3patch(dir)?;
        refuse_unresolved_locale(dir, &rules)?;
        let generation = Arc::new(open_campaign_inventory(dir, &rules, &huffman)?);
        let selected_common = read_selected_common(&generation, &rules, &huffman)?;
        let common_sha256 = matching_common_sha256(common, &selected_common)?;
        let edition = edition_from_common(&selected_common, edition_global)?;
        rules.selected_mounts(edition)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            mode,
            rules,
            edition,
            common_sha256,
            huffman,
            generation,
        })
    }

    /// Build a complete bounded reader for one admitted map header from the
    /// inventory opened at pin time.
    pub fn open(&self, info: &MapInfo) -> Result<CampaignReader, Error> {
        let selection = self.rules.campaign_data_set(info, self.edition)?;
        let layers = campaign_layers(
            &self.generation,
            &self.rules,
            &self.huffman,
            &selection,
            self.rules.image_sha256,
            self.common_sha256,
        )?;
        let resolver = Resolver::new(self.mode, None, layers)?;
        Ok(CampaignReader {
            recipe: self.clone(),
            selection,
            resolver,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn mode(&self) -> GameMode {
        self.mode
    }
    pub fn rules(&self) -> &ImageRules {
        &self.rules
    }
    pub fn edition(&self) -> i32 {
        self.edition
    }
    pub fn common_sha256(&self) -> Sha256Digest {
        self.common_sha256
    }
    pub fn image_sha256(&self) -> Sha256Digest {
        self.rules.image_sha256
    }
}

pub struct CampaignReader {
    pub recipe: CampaignRecipe,
    pub selection: CampaignDataSet,
    pub resolver: Resolver,
}

impl ImageRules {
    /// Read every rule from the reviewed Legacy image. Values are operands, not
    /// defaults substituted when a window is malformed.
    pub fn read(image: &[u8], origin: AssetIdentity) -> Result<Self, Error> {
        let fail = |reason: &str| Error::Message(reason.into());
        if image.len() > 32 * 1024 * 1024
            || sha256(image) != REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|value| value.trim().is_empty())
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(
            image,
            RuleInputBudget {
                image_bytes: 32 * 1024 * 1024,
                profile_bytes: 64 * 1024,
                sections: 96,
            },
        )
        .map_err(|_| fail("invalid PE image"))?;
        Self::decode(origin, sha256(image), |va, n| {
            pe.read_from(va, false)
                .map_err(|_| "missing reviewed instruction operand".to_string())?
                .get(..n)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| "truncated reviewed instruction operand".to_string())
        })
    }

    fn decode(
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        mut read: impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
    ) -> Result<Self, Error> {
        let selector = read(0x71dbd6, 14)?;
        if selector[..10] != [0x8b, 0x88, 0x44, 0x1e, 0x00, 0x00, 0x33, 0xc0, 0x83, 0xf9]
            || selector[11..] != [0x0f, 0x46, 0xc1]
        {
            return Err(Error::Message(
                "unsupported campaign selector instructions".into(),
            ));
        }
        let selector_clamp = u32::from(selector[10]);

        let first = read(0x7046c5, 9)?;
        let second = read(0x704701, 9)?;
        if first[..7] != [0x8b, 0x80, 0x40, 0x01, 0x00, 0x00, 0x83]
            || first[7] != 0xe0
            || second[..7] != [0x8b, 0x85, 0xbc, 0xcb, 0xff, 0xff, 0x83]
            || second[7] != 0xe0
            || first[8] != second[8]
        {
            return Err(Error::Message(
                "unsupported campaign flag instructions".into(),
            ));
        }
        let flag_mask = u32::from(first[8]);

        let start = read(0x428dd8, 5)?;
        let step = read(0x428e2f, 3)?;
        if start[0] != 0xbe || step[..2] != [0x83, 0xc6] {
            return Err(Error::Message(
                "unsupported base archive priority instructions".into(),
            ));
        }
        let start_priority = u32::from_le_bytes(start[1..].try_into().unwrap());
        let step_priority = u32::from(step[2]);
        let war3_mod_priority = immediate_u32(read(0x428e41, 2)?, 0x6a)?;
        let deprecated_priority = immediate_u32(read(0x428e54, 2)?, 0x6a)?;

        let stride = read(0x428e32, 3)?;
        let limit = read(0x428e35, 3)?;
        if stride[..2] != [0x83, 0xc7] || limit[..2] != [0x83, 0xfe] {
            return Err(Error::Message(
                "unsupported base archive descriptor loop".into(),
            ));
        }
        let descriptor_stride = u32::from(stride[2]);
        let priority_limit = u32::from(limit[2]);
        if step_priority == 0
            || descriptor_stride == 0
            || priority_limit <= start_priority
            || (priority_limit - start_priority) % step_priority != 0
        {
            return Err(Error::Message(
                "unsupported base archive priority range".into(),
            ));
        }
        let mount_rounds = (priority_limit - start_priority) / step_priority;
        if mount_rounds == 0
            || mount_rounds > 8
            || mount_rounds
                .checked_mul(descriptor_stride)
                .is_none_or(|bytes| bytes > 64)
        {
            return Err(Error::Message(
                "unsupported base archive mount round count".into(),
            ));
        }

        let base_descriptors = mov_table_operand(read(0x428de0, 6)?)?;
        let base_handles = lea_operand(read(0x428dea, 6)?)?;
        let local_descriptors = mov_table_operand(read(0x428e0a, 6)?)?;
        let local_handles = lea_operand(read(0x428e19, 6)?)?;
        let mod_descriptor = push_operand(read(0x428e43, 5)?)?;
        let mod_handle = push_operand(read(0x428e3a, 5)?)?;
        let deprecated_descriptor = push_operand(read(0x428e56, 5)?)?;
        let deprecated_handle = push_operand(read(0x428e4d, 5)?)?;

        let mut names = BTreeMap::new();
        for site in [0x428be9, 0x428bff, 0x428c13, 0x428c27] {
            let copy = read(site, 20)?;
            let call = copy[15] == 0xe8 || copy[15..18] == [0x8b, 0xf0, 0xe8];
            if copy[0] != 0x68 || copy[5] != 0x68 || copy[10] != 0x68 || !call {
                return Err(Error::Message(
                    "unsupported archive name initializer".into(),
                ));
            }
            let size = u32::from_le_bytes(copy[1..5].try_into().unwrap());
            if size == 0 {
                return Err(Error::Message(
                    "archive name initializer has an empty buffer".into(),
                ));
            }
            let source = u32::from_le_bytes(copy[6..10].try_into().unwrap());
            let destination = u32::from_le_bytes(copy[11..15].try_into().unwrap());
            let name = c_string_at(&mut read, source, "archive descriptor")?;
            names.entry(destination).or_insert(name);
        }
        let mut localized_archives = Vec::new();
        for (site, fallback_site) in [(0x428c3b, 0x428c83), (0x428c58, 0x428cab)] {
            let localized = read(site, 26)?;
            if localized[0] != 0x68
                || localized[5] != 0x56
                || localized[6] != 0x68
                || localized[11] != 0x68
                || localized[16] != 0x68
                || localized[21] != 0xe8
            {
                return Err(Error::Message(
                    "unsupported localized archive name construction".into(),
                ));
            }
            let suffix_va = u32::from_le_bytes(localized[1..5].try_into().unwrap());
            let format_va = u32::from_le_bytes(localized[7..11].try_into().unwrap());
            let destination = u32::from_le_bytes(localized[17..21].try_into().unwrap());
            let suffix = c_string_at(&mut read, suffix_va, "local archive suffix")?;
            let format = c_string_at(&mut read, format_va, "local archive name format")?;
            if suffix.trim() != suffix || suffix.is_empty() || format != "%s-%s" {
                return Err(Error::Message(
                    "unsupported localized archive name format".into(),
                ));
            }
            if !localized_archives.contains(&suffix) {
                localized_archives.push(suffix);
            }
            let fallback = read(fallback_site, 20)?;
            if fallback[0] != 0x68
                || fallback[5] != 0x68
                || fallback[10] != 0x68
                || fallback[15] != 0xe8
            {
                return Err(Error::Message(
                    "unsupported localized archive fallback".into(),
                ));
            }
            let fallback_va = u32::from_le_bytes(fallback[6..10].try_into().unwrap());
            let fallback_destination = u32::from_le_bytes(fallback[11..15].try_into().unwrap());
            if fallback_destination != destination {
                return Err(Error::Message(
                    "localized archive fallback writes another descriptor".into(),
                ));
            }
            let name = c_string_at(&mut read, fallback_va, "local archive fallback")?;
            names.insert(destination, name);
        }

        let descriptor_bytes = usize::try_from(mount_rounds * descriptor_stride)
            .map_err(|_| Error::Message("archive descriptor table is too large".into()))?;
        let nonlocal_descriptor_bytes = read(base_descriptors, descriptor_bytes)?;
        let local_descriptor_bytes = read(local_descriptors, descriptor_bytes)?;
        let mut base_with_handles = Vec::new();
        for round in 0..mount_rounds {
            let offset = round * descriptor_stride;
            let offset_usize = offset as usize;
            let priority = start_priority + round * step_priority;
            for (descriptor_bytes, handle_base, local) in [
                (nonlocal_descriptor_bytes.as_slice(), base_handles, 0),
                (local_descriptor_bytes.as_slice(), local_handles, 1),
            ] {
                let descriptor = u32::from_le_bytes(
                    descriptor_bytes[offset_usize..offset_usize + 4]
                        .try_into()
                        .map_err(|_| Error::Message("truncated archive descriptor".into()))?,
                );
                if descriptor == 0 {
                    continue;
                }
                let archive = names.get(&descriptor).cloned().ok_or_else(|| {
                    Error::Message(format!(
                        "archive descriptor {descriptor:#x} has no name initializer"
                    ))
                })?;
                let handle = handle_base.checked_add(offset).ok_or_else(|| {
                    Error::Message("archive handle table offset overflowed".into())
                })?;
                base_with_handles.push((
                    handle,
                    MountSpec {
                        archive,
                        priority: priority + local,
                    },
                ));
            }
        }
        let base_name = |descriptor: u32| {
            names.get(&descriptor).cloned().ok_or_else(|| {
                Error::Message(format!(
                    "archive descriptor {descriptor:#x} has no name initializer"
                ))
            })
        };
        base_with_handles.push((
            mod_handle,
            MountSpec {
                archive: base_name(mod_descriptor)?,
                priority: war3_mod_priority,
            },
        ));
        base_with_handles.push((
            deprecated_handle,
            MountSpec {
                archive: base_name(deprecated_descriptor)?,
                priority: deprecated_priority,
            },
        ));
        let by_handle: BTreeMap<u32, MountSpec> = base_with_handles.iter().cloned().collect();
        let base: Vec<_> = base_with_handles
            .into_iter()
            .map(|(_, mount)| mount)
            .collect();

        let selected_priorities = [
            immediate_u32(read(0x42905b, 2)?, 0x6a)?,
            immediate_u32(read(0x42907f, 2)?, 0x6a)?,
            immediate_u32(read(0x42909d, 2)?, 0x6a)?,
            immediate_u32(read(0x4290bb, 2)?, 0x6a)?,
        ];
        let selected_tables = [
            table_operand(read(0x429009, 7)?, 0x0c)?,
            table_operand(read(0x429024, 7)?, 0x0c)?,
            table_operand(read(0x429010, 7)?, 0x1c)?,
            table_operand(read(0x429017, 7)?, 0x3c)?,
        ];
        let table_entries = selected_tables
            .iter()
            .map(|table| read(*table, 8))
            .collect::<Result<Vec<_>, String>>()?;
        let mut selected: [Vec<MountSpec>; 2] = [Vec::new(), Vec::new()];
        for (edition, edition_mounts) in selected.iter_mut().enumerate() {
            for (entries, priority) in table_entries.iter().zip(selected_priorities) {
                let at = edition * 4;
                let handle = u32::from_le_bytes(entries[at..at + 4].try_into().unwrap());
                if handle == 0 {
                    continue;
                }
                let mount = by_handle.get(&handle).ok_or_else(|| {
                    Error::Message(format!(
                        "selected namespace handle {handle:#x} has no base archive"
                    ))
                })?;
                let candidate = MountSpec {
                    archive: mount.archive.clone(),
                    priority,
                };
                if let Some(existing) = edition_mounts
                    .iter_mut()
                    .find(|mount| mount.archive == candidate.archive)
                {
                    if candidate.priority > existing.priority {
                        *existing = candidate;
                    }
                } else {
                    edition_mounts.push(candidate);
                }
            }
        }
        let namespace_format = c_string(&mut read, 0xe68994, "%s_V%d")?;
        let table = read(0x10db190, 8)?;
        let melee_va = u32::from_le_bytes(table[..4].try_into().unwrap());
        let custom_va = u32::from_le_bytes(table[4..].try_into().unwrap());
        let datasets = [
            c_string_at(&mut read, melee_va, "Melee")?,
            c_string_at(&mut read, custom_va, "Custom")?,
        ];

        let rules = Self {
            origin,
            image_sha256,
            selector_clamp,
            flag_mask,
            namespace_format,
            datasets,
            base,
            selected,
            localized_archives,
        };
        rules.validate()?;
        Ok(rules)
    }

    fn validate(&self) -> Result<(), Error> {
        if self.selector_clamp != 2 {
            return Err(Error::Message(
                "reviewed selector accepts a different loading-id range".into(),
            ));
        }
        if self.flag_mask == 0 {
            return Err(Error::Message("campaign flag mask is empty".into()));
        }
        if self.datasets[0] == self.datasets[1] {
            return Err(Error::Message(
                "campaign dataset names are not distinct".into(),
            ));
        }
        if self
            .datasets
            .iter()
            .any(|name| name.trim().is_empty() || name.chars().any(char::is_control))
        {
            return Err(Error::Message("invalid campaign dataset name".into()));
        }
        validate_archive_names(&self.base)?;
        for mounts in &self.selected {
            validate_archive_names(mounts)?;
        }
        let known: BTreeSet<&str> = self.base.iter().map(|m| m.archive.as_str()).collect();
        for name in self
            .selected
            .iter()
            .flatten()
            .map(|mount| mount.archive.as_str())
        {
            if !known.contains(name) {
                return Err(Error::Message(format!(
                    "selected namespace archive {name:?} is not a base archive"
                )));
            }
        }
        for selected in &self.selected {
            let mut priorities = BTreeSet::new();
            for mount in self.base.iter().chain(selected) {
                if !priorities.insert(mount.priority) {
                    return Err(Error::Message(format!(
                        "archive priority {} is ambiguous",
                        mount.priority
                    )));
                }
            }
        }
        format_namespace(&self.namespace_format, &self.datasets[0], 0)?;
        format_namespace(&self.namespace_format, &self.datasets[1], 1)?;
        Ok(())
    }

    pub fn selected_mounts(&self, edition: i32) -> Result<&[MountSpec], Error> {
        usize::try_from(edition)
            .ok()
            .and_then(|edition| self.selected.get(edition))
            .map(Vec::as_slice)
            .ok_or_else(|| {
                Error::Message(format!(
                    "campaign edition enumerand {edition} is unsupported"
                ))
            })
    }

    pub fn campaign_data_set(
        &self,
        info: &MapInfo,
        edition: i32,
    ) -> Result<CampaignDataSet, Error> {
        self.select(info.loading_id, info.flags, edition)
    }

    pub fn select(
        &self,
        loading_id: Option<u32>,
        flags: u32,
        edition: i32,
    ) -> Result<CampaignDataSet, Error> {
        self.selected_mounts(edition)?;
        let raw = loading_id.unwrap_or(u32::MAX);
        let loading_id = if raw > self.selector_clamp { 0 } else { raw };
        let dataset = match loading_id {
            1 => 1,
            2 => 0,
            0 if flags & self.flag_mask != 0 => 0,
            0 => 1,
            _ => {
                return Err(Error::Message(format!(
                    "campaign loading id {loading_id} is unsupported"
                )));
            }
        };
        let prefix = format_namespace(&self.namespace_format, &self.datasets[dataset], edition)?;
        Ok(CampaignDataSet {
            dataset,
            edition,
            prefix,
        })
    }

    /// Resolve a logical member set into layers. `has` reports whether the
    /// selected archive actually holds that exact member spelling.
    pub fn plan_layers(
        &self,
        selection: &CampaignDataSet,
        logicals: &[&str],
        mut has: impl FnMut(&str, &str) -> bool,
    ) -> Result<Vec<PlannedLayer>, Error> {
        self.selected_mounts(selection.edition)?;
        let mut layers = Vec::new();
        let mut seen = BTreeSet::new();
        for (mounts, prefix) in [
            (self.base.as_slice(), None),
            (
                self.selected[selection.edition as usize].as_slice(),
                Some(selection.prefix.as_str()),
            ),
        ] {
            for mount in mounts {
                let mut members = Vec::new();
                for logical in logicals {
                    if logical.is_empty() {
                        return Err(Error::Message("logical member is empty".into()));
                    }
                    let member = match prefix {
                        Some(prefix) => format!("{prefix}\\{}", logical.replace('/', "\\")),
                        None => (*logical).to_string(),
                    };
                    if has(&mount.archive, &member) {
                        members.push(PlannedMember {
                            asset: (*logical).to_string(),
                            member,
                        });
                    }
                }
                if !members.is_empty()
                    && seen.insert((mount.archive.clone(), prefix.map(str::to_string)))
                {
                    layers.push(PlannedLayer {
                        archive: mount.archive.clone(),
                        priority: mount.priority,
                        prefix: prefix.map(str::to_string),
                        members,
                    });
                }
            }
        }
        layers.sort_by_key(|layer| std::cmp::Reverse(layer.priority));
        Ok(layers)
    }
}

/// Derive the `version` enumerand for `global` from one selected `common.j`.
/// The declaration must be a constant `version` initialized by the constant
/// native `ConvertVersion(integer)`.
pub fn edition_from_common(common: &[u8], global: &str) -> Result<i32, EditionError> {
    let fail = |reason: &str| EditionError(reason.into());
    let source = std::str::from_utf8(common).map_err(|_| fail("common script is not UTF-8"))?;
    let script = pcx_script::parse(source).map_err(|e| EditionError(e.render(source)))?;

    let mut types = Vec::new();
    let mut natives = Vec::new();
    collect_declarations(&script.items, &mut types, &mut natives);
    let version_types: Vec<_> = types
        .iter()
        .filter(|ty| ty.name.as_str() == "version")
        .collect();
    if version_types.len() != 1
        || version_types[0].array
        || version_types[0].extends.as_str() != "handle"
    {
        return Err(fail(
            "version must be declared exactly once extending handle",
        ));
    }
    let conversions: Vec<_> = natives
        .iter()
        .filter(|native| native.name.as_str() == "ConvertVersion")
        .collect();
    if conversions.len() != 1 {
        return Err(fail("ConvertVersion must be declared exactly once"));
    }
    let conversion = conversions[0];
    if !conversion.is_constant
        || conversion.params.len() != 1
        || conversion.params[0].ty.kind != TypeKind::Integer
        || conversion.params[0].ty.array
        || conversion.ret.kind.name() != "version"
        || conversion.ret.array
    {
        return Err(fail(
            "ConvertVersion must be constant native ConvertVersion(integer) returns version",
        ));
    }

    let globals: Vec<_> = script
        .globals()
        .into_iter()
        .filter(|decl| decl.name.as_str() == global)
        .collect();
    if globals.len() != 1 {
        return Err(EditionError(format!(
            "{global} must be declared exactly once"
        )));
    }
    let selected = globals[0];
    if !selected.is_constant || selected.ty.array || selected.ty.kind.name() != "version" {
        return Err(EditionError(format!(
            "{global} must be a constant scalar version"
        )));
    }
    let Some(Expr::Call { callee, args, .. }) = &selected.init else {
        return Err(EditionError(format!(
            "{global} must be initialized by ConvertVersion"
        )));
    };
    let (Expr::Ident(callee, _), [argument]) = (callee.as_ref(), args.as_slice()) else {
        return Err(EditionError(format!(
            "{global} must be initialized by one ConvertVersion argument"
        )));
    };
    if callee.as_str() != "ConvertVersion" {
        return Err(EditionError(format!(
            "{global} must be initialized by ConvertVersion"
        )));
    }
    let mut work = 128;
    integer(argument, 16, &mut work)
        .map_err(|reason| EditionError(format!("{global} initializer: {reason}")))
}

fn collect_declarations<'a>(
    items: &'a [Item],
    types: &mut Vec<&'a pcx_script::TypeDecl>,
    natives: &mut Vec<&'a pcx_script::NativeDecl>,
) {
    for item in items {
        match item {
            Item::Type(ty) => types.push(ty),
            Item::Native(native) => natives.push(native),
            Item::Library(library) => {
                collect_declarations(&library.body, types, natives);
            }
            Item::Scope(scope) => {
                collect_declarations(&scope.body, types, natives);
            }
            Item::Struct(body) => {
                collect_declarations(&body.body, types, natives);
            }
            _ => {}
        }
    }
}

fn integer(expr: &Expr, depth: usize, work: &mut usize) -> Result<i32, String> {
    let child = depth.checked_sub(1).ok_or("expression depth")?;
    *work = work.checked_sub(1).ok_or("expression work")?;
    match expr {
        Expr::Literal(Literal::Int(value), _) => Ok(value.value as u32 as i32),
        Expr::Group(inner, _) => integer(inner, child, work),
        Expr::Unary { op, operand, .. } => {
            let value = integer(operand, child, work)?;
            match op {
                UnOp::Neg => Ok(value.wrapping_neg()),
                UnOp::Pos => Ok(value),
                UnOp::Not => Err("boolean operator in integer initializer".into()),
            }
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let left = integer(lhs, child, work)?;
            let right = integer(rhs, child, work)?;
            match op {
                BinOp::Add => Ok(left.wrapping_add(right)),
                BinOp::Sub => Ok(left.wrapping_sub(right)),
                BinOp::Mul => Ok(left.wrapping_mul(right)),
                BinOp::Div => left.checked_div(right).ok_or("division by zero".into()),
                BinOp::Modulo => left.checked_rem(right).ok_or("division by zero".into()),
                _ => Err("non-integer operator in integer initializer".into()),
            }
        }
        _ => Err("non-constant integer expression".into()),
    }
}

fn format_namespace(format: &str, dataset: &str, edition: i32) -> Result<String, Error> {
    let bytes = format.as_bytes();
    let mut out = String::new();
    let mut dataset_used = false;
    let mut edition_used = false;
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'%' {
            out.push(char::from(bytes[at]));
            at += 1;
            continue;
        }
        let Some(next) = bytes.get(at + 1) else {
            return Err(Error::Message("namespace format ends in '%'".into()));
        };
        match next {
            b'%' => out.push('%'),
            b's' if !dataset_used => {
                dataset_used = true;
                out.push_str(dataset);
            }
            b'd' if !edition_used => {
                edition_used = true;
                out.push_str(&edition.to_string());
            }
            _ => {
                return Err(Error::Message(
                    "namespace format must use one %s and one %d".into(),
                ));
            }
        }
        at += 2;
    }
    if !dataset_used || !edition_used {
        return Err(Error::Message(
            "namespace format must use one %s and one %d".into(),
        ));
    }
    Ok(out)
}

fn validate_archive_names(mounts: &[MountSpec]) -> Result<(), Error> {
    for mount in mounts {
        if mount.archive.eq_ignore_ascii_case("War3Patch.mpq") {
            return Err(Error::Message(
                "War3Patch.mpq is outside the reviewed campaign mount path".into(),
            ));
        }
        if mount.archive.is_empty()
            || mount.archive.trim() != mount.archive
            || mount
                .archive
                .chars()
                .any(|c| c.is_control() || c == '/' || c == '\\')
        {
            return Err(Error::Message("invalid archive name operand".into()));
        }
    }
    Ok(())
}

fn mov_table_operand(bytes: Vec<u8>) -> Result<u32, Error> {
    if bytes.len() != 6 || bytes[..2] != [0x8b, 0x8f] {
        return Err(Error::Message(
            "unsupported archive descriptor table operand".into(),
        ));
    }
    Ok(u32::from_le_bytes(bytes[2..6].try_into().unwrap()))
}

fn lea_operand(bytes: Vec<u8>) -> Result<u32, Error> {
    if bytes.len() != 6 || bytes[..2] != [0x8d, 0x87] {
        return Err(Error::Message("unsupported archive handle operand".into()));
    }
    Ok(u32::from_le_bytes(bytes[2..6].try_into().unwrap()))
}

fn push_operand(bytes: Vec<u8>) -> Result<u32, Error> {
    if bytes.len() != 5 || bytes[0] != 0x68 {
        return Err(Error::Message("unsupported archive operand".into()));
    }
    Ok(u32::from_le_bytes(bytes[1..5].try_into().unwrap()))
}

fn table_operand(bytes: Vec<u8>, modrm: u8) -> Result<u32, Error> {
    if bytes.len() != 7 || bytes[..2] != [0x8b, modrm] || bytes[2] != 0x85 {
        return Err(Error::Message(
            "unsupported selected archive table operand".into(),
        ));
    }
    Ok(u32::from_le_bytes(bytes[3..7].try_into().unwrap()))
}

fn digest_hex(digest: &Sha256Digest) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn immediate_u32(bytes: Vec<u8>, opcode: u8) -> Result<u32, Error> {
    if bytes.len() != 2 || bytes[0] != opcode {
        return Err(Error::Message(
            "unsupported archive priority operand".into(),
        ));
    }
    Ok(u32::from(bytes[1]))
}

fn c_string(
    read: &mut impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
    va: u32,
    label: &str,
) -> Result<String, Error> {
    c_string_at(read, va, label)
}

fn c_string_at(
    read: &mut impl FnMut(u32, usize) -> Result<Vec<u8>, String>,
    va: u32,
    label: &str,
) -> Result<String, Error> {
    let bytes = read(va, 256)?;
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| Error::Message(format!("{label} string is unterminated")))?;
    let text = std::str::from_utf8(&bytes[..end])
        .map_err(|_| Error::Message(format!("{label} string is not UTF-8")))?;
    if text.is_empty() {
        return Err(Error::Message(format!("{label} string is empty")));
    }
    Ok(text.to_string())
}

struct OpenedArchive {
    path: PathBuf,
    archive: Arc<Mutex<pcx_mpq::MpqArchive>>,
    raw: Vec<(String, String)>,
}

struct ArchiveGeneration {
    opened: BTreeMap<String, OpenedArchive>,
    raw_paths: usize,
}

impl fmt::Debug for ArchiveGeneration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArchiveGeneration")
            .field("archives", &self.opened.keys().collect::<Vec<_>>())
            .field("raw_paths", &self.raw_paths)
            .finish()
    }
}

/// The reviewed campaign mount path does not reference War3Patch.mpq. Its
/// presence places this tree outside the reviewed source generation.
fn refuse_war3patch(dir: &Path) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("War3Patch.mpq")
        {
            return Err(Error::Message(format!(
                "{} contains War3Patch.mpq, outside the reviewed campaign mount order",
                dir.display()
            )));
        }
    }
    Ok(())
}

fn find_opened<'a>(
    opened: &'a BTreeMap<String, OpenedArchive>,
    name: &str,
) -> Option<&'a OpenedArchive> {
    opened.get(&name.to_ascii_lowercase())
}

fn read_listfile(
    archive: &mut pcx_mpq::MpqArchive,
    huffman: &pcx_mpq::huffman::HuffmanTable,
) -> Result<String, Error> {
    let budget = pcx_mpq::ExtractionBudget {
        max_raw_packed_bytes: MAX_ASSET_BYTES,
        max_decoded_bytes: MAX_ASSET_BYTES,
    };
    let bytes = archive
        .extract_bounded("(listfile)", budget, Some(huffman))
        .map_err(|error| {
            Error::Message(format!(
                "{}: cannot read (listfile): {error}",
                archive.path().display()
            ))
        })?;
    String::from_utf8(bytes).map_err(|_| {
        Error::Message(format!(
            "{}: (listfile) is not UTF-8",
            archive.path().display()
        ))
    })
}

enum ListedMember<'a> {
    Absent,
    Live(&'a pcx_mpq::BlockEntry),
}

/// Distinguishes a stale listfile name from a listed hash entry whose block
/// cannot be used. The former is allowed to remain absent; the latter must not
/// silently lose to a lower-priority archive. Empty, deleted and region-end
/// hash sentinels keep their chain-terminating meaning, and block deletion
/// markers remain legitimate tombstones.
fn listed_member<'a>(
    archive: &'a pcx_mpq::MpqArchive,
    member: &str,
) -> Result<ListedMember<'a>, Error> {
    let hashes = archive.hash_entries();
    if hashes.is_empty() {
        return Ok(ListedMember::Absent);
    }
    let count = hashes.len();
    let (name1, name2) = pcx_mpq::hash::hash_string_both(member);
    let mut slot = pcx_mpq::hash::start_slot(member, count);
    for _ in 0..count {
        let entry = &hashes[slot];
        if entry.is_unpopulated() {
            return Ok(ListedMember::Absent);
        }
        if entry.name1 == name1 && entry.name2 == name2 {
            let index = entry.block_index as usize;
            let block = archive.block_entries().get(index).ok_or_else(|| {
                Error::Message(format!(
                    "{}: listed member {member:?} references block {index} outside the block table",
                    archive.path().display()
                ))
            })?;
            if block.flags & pcx_mpq::flags::DELETE_MARKER != 0 {
                return Ok(ListedMember::Absent);
            }
            if block.flags & pcx_mpq::flags::EXISTING == 0 {
                return Err(Error::Message(format!(
                    "{}: listed member {member:?} references non-existent block {index} (flags {:#010x})",
                    archive.path().display(),
                    block.flags
                )));
            }
            return Ok(ListedMember::Live(block));
        }
        slot += 1;
        if slot == count {
            slot = 0;
        }
    }
    Ok(ListedMember::Absent)
}

fn canonical_members(
    archive: &pcx_mpq::MpqArchive,
    listfile: &str,
) -> Result<Vec<(String, String)>, Error> {
    let mut canonical: BTreeMap<AssetPath, (String, pcx_mpq::BlockEntry)> = BTreeMap::new();
    for line in listfile
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let ListedMember::Live(block) = listed_member(archive, line)? else {
            continue;
        };
        let path = AssetPath::new(line)
            .map_err(|error| Error::Message(format!("{}: {error}", archive.path().display())))?;
        if let Some((prior, prior_block)) = canonical.get(&path) {
            if prior_block.file_pos != block.file_pos
                || prior_block.file_size != block.file_size
                || prior_block.compressed_size != block.compressed_size
            {
                return Err(Error::Message(format!(
                    "{}: {line:?} and {prior:?} are distinct archive members with one platform path",
                    archive.path().display()
                )));
            }
        } else {
            canonical.insert(path, (line.to_string(), *block));
        }
    }
    if canonical.len() > MAX_LISTED_PATHS {
        return Err(Error::Message(format!(
            "{}: listfile exposes {} platform paths, limit is {MAX_LISTED_PATHS}",
            archive.path().display(),
            canonical.len()
        )));
    }
    Ok(canonical
        .into_iter()
        .map(|(path, (member, _))| (path.as_str().to_string(), member))
        .collect())
}

fn stripped_members(
    raw: &[(String, String)],
    prefix: &str,
) -> Result<Vec<(String, String)>, Error> {
    let prefix = AssetPath::new(prefix)
        .map_err(|error| Error::Message(format!("invalid campaign namespace: {error}")))?;
    let prefix = format!("{}/", prefix.as_str());
    let mut stripped: BTreeMap<AssetPath, String> = BTreeMap::new();
    for (asset, member) in raw {
        let Some(rest) = asset.strip_prefix(&prefix) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let path = AssetPath::new(rest)
            .map_err(|error| Error::Message(format!("invalid campaign member path: {error}")))?;
        if let Some(prior) = stripped.get(&path)
            && prior != member
        {
            return Err(Error::Message(format!(
                "namespace {prefix:?} maps both {prior:?} and {member:?} to one path"
            )));
        }
        stripped.insert(path, member.clone());
    }
    Ok(stripped
        .into_iter()
        .map(|(path, member)| (path.as_str().to_string(), member))
        .collect())
}

struct LayerGeneration {
    image_sha256: Sha256Digest,
    common_sha256: Sha256Digest,
    edition: i32,
}

fn build_layer(
    opened: &OpenedArchive,
    priority: u32,
    members: Vec<(String, String)>,
    prefix: Option<&str>,
    huffman: &pcx_mpq::huffman::HuffmanTable,
    generation: &LayerGeneration,
) -> Result<ArchiveLayer, Error> {
    let mut mappings = BTreeMap::new();
    for (asset, member) in members {
        let path = AssetPath::new(&asset)
            .map_err(|error| Error::Message(format!("invalid campaign asset: {error}")))?;
        if mappings.insert(path.clone(), member).is_some() {
            return Err(Error::Message(format!(
                "campaign layer maps {path} more than once"
            )));
        }
    }
    let origin = opened
        .path
        .to_str()
        .ok_or_else(|| Error::Message("campaign archive path is not UTF-8".into()))?;
    let role = prefix
        .map(|prefix| format!("namespace {prefix}"))
        .unwrap_or_else(|| "base".to_string());
    let version = format!(
        "campaign image sha256:{} common sha256:{} edition:{}",
        digest_hex(&generation.image_sha256),
        digest_hex(&generation.common_sha256),
        generation.edition
    );
    let id = format!("campaign archive {origin} priority {priority} {role} {version}");
    let info = SourceInfo {
        id,
        kind: LayerKind::Mpq,
        origin: origin.to_string(),
        version: Some(version),
    };
    if info.origin.trim().is_empty() || info.id.trim().is_empty() {
        return Err(Error::Message(
            "scoped campaign source has empty provenance".into(),
        ));
    }
    Ok(ArchiveLayer {
        priority,
        source: Box::new(ArchiveInventorySource {
            info,
            archive: Arc::clone(&opened.archive),
            members: mappings,
            huffman: huffman.clone(),
        }),
    })
}

fn add_paths(total: usize, added: usize) -> Result<usize, Error> {
    let total = total
        .checked_add(added)
        .ok_or_else(|| Error::Message("campaign path count overflowed".into()))?;
    let limit = MAX_LISTED_PATHS.saturating_mul(8);
    if total > limit {
        return Err(Error::Message(format!(
            "campaign reader exposes more than {limit} paths"
        )));
    }
    Ok(total)
}

fn refuse_unresolved_locale(dir: &Path, rules: &ImageRules) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        for suffix in &rules.localized_archives {
            let lower = name.to_ascii_lowercase();
            let suffix = suffix.to_ascii_lowercase();
            let Some(prefix) = lower.strip_suffix(&suffix) else {
                continue;
            };
            if prefix
                .strip_suffix('-')
                .is_some_and(|locale| !locale.is_empty())
            {
                return Err(Error::Message(format!(
                    "{} contains localized campaign archive {name:?}; runtime locale selection is unresolved",
                    dir.display()
                )));
            }
        }
    }
    Ok(())
}

fn matching_common_sha256(provided: &[u8], selected: &[u8]) -> Result<Sha256Digest, Error> {
    let provided = sha256(provided);
    let selected = sha256(selected);
    if provided != selected {
        return Err(Error::Message(
            "provided common.j does not match the common.j selected from the campaign archives"
                .into(),
        ));
    }
    Ok(selected)
}

fn read_selected_common(
    generation: &ArchiveGeneration,
    rules: &ImageRules,
    huffman: &pcx_mpq::huffman::HuffmanTable,
) -> Result<Vec<u8>, Error> {
    let mut mounts: Vec<_> = rules.base.iter().collect();
    mounts.sort_by_key(|mount| std::cmp::Reverse(mount.priority));
    for mount in mounts {
        let Some(opened) = find_opened(&generation.opened, &mount.archive) else {
            continue;
        };
        let Some((_, member)) = opened
            .raw
            .iter()
            .find(|(asset, _)| asset == "scripts/common.j")
        else {
            continue;
        };
        let mut archive = opened
            .archive
            .lock()
            .map_err(|_| Error::Message("campaign archive reader lock is poisoned".into()))?;
        let budget = pcx_mpq::ExtractionBudget {
            max_raw_packed_bytes: MAX_ASSET_BYTES,
            max_decoded_bytes: MAX_ASSET_BYTES,
        };
        return archive
            .extract_bounded(member, budget, Some(huffman))
            .map_err(|error| {
                Error::Message(format!(
                    "{}: cannot read selected common.j: {error}",
                    opened.path.display()
                ))
            });
    }
    Err(Error::Message(
        "campaign archives do not contain Scripts\\common.j".into(),
    ))
}

fn open_campaign_inventory(
    dir: &Path,
    rules: &ImageRules,
    huffman: &pcx_mpq::huffman::HuffmanTable,
) -> Result<ArchiveGeneration, Error> {
    let mut wanted: Vec<&str> = rules
        .base
        .iter()
        .map(|mount| mount.archive.as_str())
        .collect();
    wanted.sort_unstable();
    wanted.dedup();

    let mut opened = BTreeMap::new();
    let mut listfile_bytes = 0usize;
    let mut raw_paths = 0usize;
    match pcx_mpq::MpqSet::open_named(dir, &wanted) {
        Ok(set) => {
            for mut archive in set.into_members() {
                let path = archive.path().to_path_buf();
                let key = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        Error::Message(format!(
                            "campaign archive {} has no UTF-8 name",
                            path.display()
                        ))
                    })?
                    .to_ascii_lowercase();
                let listfile = read_listfile(&mut archive, huffman)?;
                listfile_bytes = listfile_bytes
                    .checked_add(listfile.len())
                    .ok_or_else(|| Error::Message("campaign listfile size overflowed".into()))?;
                if listfile_bytes > MAX_AGGREGATE_BYTES {
                    return Err(Error::Message(format!(
                        "campaign listfiles exceed {MAX_AGGREGATE_BYTES} bytes"
                    )));
                }
                let raw = canonical_members(&archive, &listfile)?;
                raw_paths = add_paths(raw_paths, raw.len())?;
                opened.insert(
                    key,
                    OpenedArchive {
                        path,
                        archive: Arc::new(Mutex::new(archive)),
                        raw,
                    },
                );
            }
        }
        Err(pcx_mpq::Error::NotFound(_)) => {
            return Err(Error::Message(format!(
                "no campaign archives found in {}",
                dir.display()
            )));
        }
        Err(error) => return Err(Error::Mpq(error)),
    }
    if opened.is_empty() {
        return Err(Error::Message(format!(
            "no campaign archives found in {}",
            dir.display()
        )));
    }
    Ok(ArchiveGeneration { opened, raw_paths })
}

fn campaign_layers(
    generation: &ArchiveGeneration,
    rules: &ImageRules,
    huffman: &pcx_mpq::huffman::HuffmanTable,
    selection: &CampaignDataSet,
    image_sha256: Sha256Digest,
    common_sha256: Sha256Digest,
) -> Result<Vec<ArchiveLayer>, Error> {
    let selected_mounts = rules.selected_mounts(selection.edition)?;
    let layer_generation = LayerGeneration {
        image_sha256,
        common_sha256,
        edition: selection.edition,
    };
    let mut path_count = generation.raw_paths;
    let mut layers = Vec::new();
    for mount in &rules.base {
        let Some(opened) = find_opened(&generation.opened, &mount.archive) else {
            continue;
        };
        layers.push(build_layer(
            opened,
            mount.priority,
            opened.raw.clone(),
            None,
            huffman,
            &layer_generation,
        )?);
    }
    for mount in selected_mounts {
        let Some(opened) = find_opened(&generation.opened, &mount.archive) else {
            continue;
        };
        let members = stripped_members(&opened.raw, &selection.prefix)?;
        if members.is_empty() {
            continue;
        }
        path_count = add_paths(path_count, members.len())?;
        layers.push(build_layer(
            opened,
            mount.priority,
            members,
            Some(&selection.prefix),
            huffman,
            &layer_generation,
        )?);
    }
    Ok(layers)
}

#[cfg(test)]
#[path = "campaign_data_set_tests.rs"]
mod tests;
