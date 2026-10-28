//! Authored world import. This data is not a native gameplay simulation.
//!
//! Positions and placement state stay in their original representation. No
//! fixed-point rounding, spawn defaults, life conversion or combat rules occur.
use std::collections::{BTreeMap, HashMap};

use pcx_map::{
    GridAlignment, GridAlignmentError, GridDimensions, Map, MapInfo, Pathing, RandomUnit,
    UnitPlacement, Units,
};
use pcx_objects::editor::{Editor, FieldDef, FieldSource, ObjectKind, Overlay, SlkId, StatValue};
use pcx_objects::{Fourcc, Mod, Value};

/// Supplied source identity, rather than an edition inferred from nearby files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub install: String,
    pub edition: String,
    pub map: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("{source_identity:?}: terrain {terrain:?} and pathing {pathing:?}: {error}")]
    GridAlignment {
        source_identity: Source,
        terrain: GridDimensions,
        pathing: GridDimensions,
        #[source]
        error: GridAlignmentError,
    },
    #[error("world import requires explicit install, edition and map provenance")]
    MissingSource,
    #[error("{map}: {member}: {error}")]
    Member {
        map: String,
        member: &'static str,
        error: pcx_map::Error,
    },
}

/// Exact base cell or typed map override, with no inferred numeric scale.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Cell(String),
    Override(Value),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldProblem {
    Missing,
    UnsupportedSource,
    Indexed([u32; 2]),
    UnknownMetadata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectField {
    pub metadata: Option<FieldDef>,
    pub value: Result<FieldValue, FieldProblem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    pub rawcode: Fourcc,
    pub base_rawcode: String,
    pub kind: ObjectKind,
    pub fields: BTreeMap<Fourcc, ObjectField>,
    /// All authored override records, including unsupported/indexed fields.
    pub modifications: Vec<Mod>,
}

/// Why a column does not select exactly one field of a [`Definition`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnError {
    /// No field's metadata names the column.
    Missing,
    /// More than one field's metadata names the column.
    Ambiguous,
}

impl Definition {
    /// The one field whose metadata names `column` as its SLK column, in any table. Scans every
    /// field; use [`ColumnIndex`] for repeated lookups on one unchanged definition.
    pub fn column_field(&self, column: &str) -> Result<(&Fourcc, &ObjectField), ColumnError> {
        let mut matches = self
            .fields
            .iter()
            .filter(|(_, field)| field_column(field) == Some(column));
        let found = matches.next().ok_or(ColumnError::Missing)?;
        match matches.next() {
            Some(_) => Err(ColumnError::Ambiguous),
            None => Ok(found),
        }
    }

    /// Index for repeated [`ColumnIndex::field`] lookups on this definition.
    pub fn column_index(&self) -> ColumnIndex {
        let mut columns: HashMap<String, Option<Fourcc>> = HashMap::new();
        for (id, field) in &self.fields {
            if let Some(column) = field_column(field) {
                // A second field with the same column makes the entry ambiguous (`None`).
                columns
                    .entry(column.to_owned())
                    .and_modify(|slot| *slot = None)
                    .or_insert(Some(*id));
            }
        }
        ColumnIndex { columns }
    }
}

fn field_column(field: &ObjectField) -> Option<&str> {
    match &field.metadata.as_ref()?.source {
        FieldSource::Column { column, .. } => Some(column),
        _ => None,
    }
}

/// Column-to-field map of one [`Definition`], built once by [`Definition::column_index`].
/// It describes the definition as it was when built: rebuild it after any change to
/// `fields` (a unit's definition changes only when its `definition_revision` does).
#[derive(Debug, Clone)]
pub struct ColumnIndex {
    /// The field id for a unique column, `None` for an ambiguous one.
    columns: HashMap<String, Option<Fourcc>>,
}

impl ColumnIndex {
    /// Same result as [`Definition::column_field`] on the definition the index was built from.
    pub fn field<'a>(
        &self,
        definition: &'a Definition,
        column: &str,
    ) -> Result<(&'a Fourcc, &'a ObjectField), ColumnError> {
        match self.columns.get(column) {
            None => Err(ColumnError::Missing),
            Some(None) => Err(ColumnError::Ambiguous),
            Some(Some(id)) => definition
                .fields
                .get_key_value(id)
                .ok_or(ColumnError::Missing),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionProblem {
    InvalidRawcode,
    RawcodeMismatch {
        requested: Fourcc,
        resolved: Fourcc,
    },
    MissingMetadata(String),
    MissingRawcode(String),
    UnsupportedKind(ObjectKind),
    AmbiguousCustomRawcode,
    UnsupportedCustomBase(String),
    WrongFamily {
        expected: ObjectKind,
        actual: ObjectKind,
    },
}

/// Allows synthetic, fully supplied catalogs to exercise import independently
/// of the original install. Production uses [`MetadataCatalog`].
pub trait ObjectCatalog {
    fn resolve(&self, rawcode: Fourcc) -> Result<Definition, ResolutionProblem>;
}

/// Object values are read from supplied SLK/profile metadata and map overlays.
/// Keep unit and item overlays separate: combining them loses family identity.
pub struct MetadataCatalog<'a> {
    pub editor: &'a Editor,
    pub units: &'a Overlay,
    pub items: &'a Overlay,
}

impl ObjectCatalog for MetadataCatalog<'_> {
    fn resolve(&self, rawcode: Fourcc) -> Result<Definition, ResolutionProblem> {
        if self.editor.table(SlkId::UnitMetaData).is_none() {
            return Err(ResolutionProblem::MissingMetadata(
                SlkId::UnitMetaData.path().to_owned(),
            ));
        }
        let name = std::str::from_utf8(&rawcode)
            .ok()
            .filter(|name| name.bytes().all(|byte| byte.is_ascii_graphic()))
            .ok_or(ResolutionProblem::InvalidRawcode)?;
        let custom_unit = self.units.new_object(name);
        let custom_item = self.items.new_object(name);
        if custom_unit.is_some() && custom_item.is_some() {
            return Err(ResolutionProblem::AmbiguousCustomRawcode);
        }
        let (base, expected) = if let Some((base, _)) = custom_unit {
            (base, Some(ObjectKind::Unit))
        } else if let Some((base, _)) = custom_item {
            (base, Some(ObjectKind::Item))
        } else {
            (name, None)
        };
        // Overlay's public reader supports stock bases, not custom chains.
        if base != name
            && (self.units.new_object(base).is_some() || self.items.new_object(base).is_some())
        {
            return Err(ResolutionProblem::UnsupportedCustomBase(base.to_owned()));
        }
        let kind = self
            .editor
            .resolve(base)
            .ok_or_else(|| ResolutionProblem::MissingRawcode(base.to_owned()))?;
        if let Some(expected) = expected
            && expected != kind
        {
            return Err(ResolutionProblem::WrongFamily {
                expected,
                actual: kind,
            });
        }
        let overlay = match kind {
            ObjectKind::Unit => self.units,
            ObjectKind::Item => self.items,
            _ => return Err(ResolutionProblem::UnsupportedKind(kind)),
        };
        let unit = overlay.unit(self.editor, name);
        let item = overlay.item(self.editor, name);
        let stat = |field| match kind {
            ObjectKind::Unit => unit.stat(field),
            ObjectKind::Item => item.stat(field),
            _ => None,
        };
        let modifications = overlay
            .new_object(name)
            .map(|(_, mods)| mods)
            .unwrap_or_else(|| overlay.on(name));
        let mut fields = BTreeMap::new();
        for metadata in self
            .editor
            .fields()
            .iter()
            .filter(|field| field.from == SlkId::UnitMetaData)
        {
            let applicable = match &metadata.source {
                FieldSource::Column { slk, .. } => match kind {
                    ObjectKind::Item => *slk == SlkId::ItemData,
                    ObjectKind::Unit => matches!(
                        slk,
                        SlkId::UnitData
                            | SlkId::UnitUi
                            | SlkId::UnitBalance
                            | SlkId::UnitWeapons
                            | SlkId::UnitAbilities
                    ),
                    _ => false,
                },
                // Retain missing profile entries explicitly; applicability flags
                // are not exposed by the current metadata reader.
                FieldSource::Profile { .. } | FieldSource::Unknown { .. } => true,
            };
            if !applicable {
                continue;
            }
            let value = if let Some(selectors) = modifications
                .iter()
                .rev()
                .find(|entry| entry.field == metadata.fourcc && entry.selectors.is_some())
                .and_then(|entry| entry.selectors)
            {
                Err(FieldProblem::Indexed(selectors))
            } else if let Some(value) = stat(metadata.fourcc) {
                Ok(match value {
                    StatValue::Cell(value) => FieldValue::Cell(value.to_owned()),
                    StatValue::Override(value) => FieldValue::Override(value.clone()),
                })
            } else if matches!(metadata.source, FieldSource::Unknown { .. }) {
                Err(FieldProblem::UnsupportedSource)
            } else {
                Err(FieldProblem::Missing)
            };
            fields.insert(
                metadata.fourcc,
                ObjectField {
                    metadata: Some(metadata.clone()),
                    value,
                },
            );
        }
        // Unknown map fields remain visible even when absent from metadata.
        for entry in modifications {
            fields.entry(entry.field).or_insert_with(|| ObjectField {
                metadata: None,
                value: Err(FieldProblem::UnknownMetadata),
            });
        }
        Ok(Definition {
            rawcode,
            base_rawcode: base.to_owned(),
            kind,
            fields,
            modifications: modifications.to_vec(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordProblem {
    Definition(ResolutionProblem),
    MissingRequiredField(Fourcc),
    RequiredField {
        field: Fourcc,
        problem: FieldProblem,
    },
    NonFinitePlacement,
    /// Native reads these words but their selection semantics are not imported.
    UnverifiedSelection(RandomUnit),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedRecord {
    /// File ordinal: creation_number is authored data, not an allocated handle.
    pub ordinal: usize,
    pub placement: UnitPlacement,
    /// Signed native owner word for units. Items have no unit owner binding.
    /// Neutral/player remapping and MapInfo slot labels are not guessed.
    pub owner: Option<i16>,
    pub definition: Result<usize, ResolutionProblem>,
    pub problems: Vec<RecordProblem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedWorld {
    pub source: Source,
    pub info: MapInfo,
    /// Authored WPM bytes. The fixed-tick step reads them only after
    /// [`crate::admit_movement`] installs an explicit profile on the sim world.
    pub pathing: Pathing,
    pub definitions: Vec<Definition>,
    pub records: Vec<ImportedRecord>,
}

impl ImportedWorld {
    /// Raw import without a terrain/pathing dimension admission policy.
    /// Use [`Self::from_map_aligned`] for an explicitly checked grid join.
    /// `required_fields` checks availability, not numeric or gameplay validity.
    /// Required files fail with map/member context. Unsupported records stay
    /// named and inspectable; they are never silently dropped or substituted.
    pub fn from_map(
        map: &Map<'_>,
        source: Source,
        catalog: &impl ObjectCatalog,
        required_fields: &[Fourcc],
    ) -> Result<Self, ImportError> {
        if source.install.is_empty() || source.edition.is_empty() || source.map.is_empty() {
            return Err(ImportError::MissingSource);
        }
        let read_error = |member, error| ImportError::Member {
            map: source.map.clone(),
            member,
            error,
        };
        let info = map
            .info()
            .map_err(|error| read_error("war3map.w3i", error))?;
        let pathing = map
            .pathing()
            .map_err(|error| read_error("war3map.wpm", error))?;
        let units = map
            .units()
            .map_err(|error| read_error("war3mapUnits.doo", error))?;
        Self::from_authored(source, info, pathing, &units, catalog, required_fields)
    }

    /// Import the same authored map with uniform integer grid validation.
    /// This is explicit project import policy, not native admission or world units.
    /// Existing extraction/parsers have no caller allocation budget; this method
    /// does not claim a total-memory bound. It returns derived metadata separately.
    pub fn from_map_aligned(
        map: &Map<'_>,
        source: Source,
        catalog: &impl ObjectCatalog,
        required_fields: &[Fourcc],
    ) -> Result<(Self, GridAlignment), ImportError> {
        if source.install.is_empty() || source.edition.is_empty() || source.map.is_empty() {
            return Err(ImportError::MissingSource);
        }
        let read_error = |member, error| ImportError::Member {
            map: source.map.clone(),
            member,
            error,
        };
        let terrain = {
            let terrain = map
                .terrain()
                .map_err(|error| read_error("war3map.w3e", error))?;
            GridDimensions {
                width: terrain.width(),
                height: terrain.height(),
            }
        };
        let pathing = map
            .pathing()
            .map_err(|error| read_error("war3map.wpm", error))?;
        let pathing_dimensions = GridDimensions {
            width: pathing.width,
            height: pathing.height,
        };
        let alignment = pcx_map::grid_alignment::terrain_pathing_alignment(
            terrain.width,
            terrain.height,
            pathing.width,
            pathing.height,
        )
        .map_err(|error| ImportError::GridAlignment {
            source_identity: source.clone(),
            terrain,
            pathing: pathing_dimensions,
            error,
        })?;
        let info = map
            .info()
            .map_err(|error| read_error("war3map.w3i", error))?;
        let units = map
            .units()
            .map_err(|error| read_error("war3mapUnits.doo", error))?;
        let world = Self::from_authored(source, info, pathing, &units, catalog, required_fields)?;
        Ok((world, alignment))
    }

    /// Placement data stays separate from the existing chosen fixed-tick world.
    pub fn from_authored(
        source: Source,
        info: MapInfo,
        pathing: Pathing,
        units: &Units,
        catalog: &impl ObjectCatalog,
        required_fields: &[Fourcc],
    ) -> Result<Self, ImportError> {
        if source.install.is_empty() || source.edition.is_empty() || source.map.is_empty() {
            return Err(ImportError::MissingSource);
        }
        let mut definitions = Vec::new();
        let mut by_rawcode = BTreeMap::new();
        let mut records = Vec::new();
        for (ordinal, placement) in units.instances.iter().enumerate() {
            let definition = by_rawcode
                .entry(placement.type_id)
                .or_insert_with(|| {
                    catalog.resolve(placement.type_id).and_then(|definition| {
                        if definition.rawcode != placement.type_id {
                            return Err(ResolutionProblem::RawcodeMismatch {
                                requested: placement.type_id,
                                resolved: definition.rawcode,
                            });
                        }
                        let index = definitions.len();
                        definitions.push(definition);
                        Ok(index)
                    })
                })
                .clone();
            let mut problems = Vec::new();
            let owner = match &definition {
                Ok(index) => {
                    let data = &definitions[*index];
                    for &field in required_fields {
                        match data.fields.get(&field) {
                            None => problems.push(RecordProblem::MissingRequiredField(field)),
                            Some(ObjectField {
                                value: Err(problem),
                                ..
                            }) => problems.push(RecordProblem::RequiredField {
                                field,
                                problem: problem.clone(),
                            }),
                            Some(_) => {}
                        }
                    }
                    (data.kind == ObjectKind::Unit)
                        .then(|| i16::from_le_bytes(placement.control_word.to_le_bytes()))
                }
                Err(problem) => {
                    problems.push(RecordProblem::Definition(problem.clone()));
                    None
                }
            };
            if placement
                .position
                .iter()
                .chain(&placement.scale)
                .copied()
                .chain([placement.angle])
                .any(|value| !value.is_finite())
            {
                problems.push(RecordProblem::NonFinitePlacement);
            }
            if placement.random != RandomUnit::None {
                problems.push(RecordProblem::UnverifiedSelection(placement.random.clone()));
            }
            records.push(ImportedRecord {
                ordinal,
                placement: placement.clone(),
                owner,
                definition,
                problems,
            });
        }
        Ok(Self {
            source,
            info,
            pathing,
            definitions,
            records,
        })
    }
}
