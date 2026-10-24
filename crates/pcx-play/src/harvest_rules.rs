//! Harvest rules read from the selected object/data tables.
//!
//! This is a **source-read** module: it parses and validates the authored
//! `Ahar` (Harvest) fields, worker/unit capability, destructable tree
//! classification and the depot `Artn` return flags. It runs no gameplay
//! harvesting algorithm. These readers consume default tables only; runtime
//! admission must resolve or explicitly refuse relevant map overrides.
//!
//! Identities and class codes are case-sensitive, matching the shipped
//! fourcc/code cells; `-` and `_` ability-list markers mean absence, never a lookup.
//! Missing harvest capability is reported as `None`, not as numbers from an
//! absent `Ahar` record.
use pcx_objects::editor::defaults::{DefaultTables, Family};
use pcx_objects::editor::{FieldKind, FieldSource, Overlay, SlkId};
use pcx_sim::world::{Definition, FieldValue};

const AHAR: &str = "Ahar";
const ARTN: &str = "Artn";
const SENTINEL: &str = "-";

#[derive(Debug, Clone, PartialEq)]
pub struct HarvestRules {
    pub source: String,
    pub ahar_alias: String,
    pub damage: f32,
    pub lumber_capacity: i32,
    pub gold_capacity: i32,
    pub search_radius: f32,
    pub range: f32,
    pub work_duration: f32,
    /// The worker's tree-targeting weapon slot, when admitted by `UnitWeapons`.
    pub work_weapon: Option<WorkWeapon>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkWeapon {
    pub slot: u32,
    pub cool: f32,
    pub damage_point: f32,
    pub backswing: f32,
    pub range: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeClass {
    pub source: String,
    pub hp: f32,
    pub is_tree: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepotReturn {
    pub source: String,
    pub accepts_gold: bool,
    pub accepts_lumber: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GoldMineRules {
    pub source: String,
    pub ability_alias: String,
    pub work_duration: f32,
    pub max_workers: u32,
}

impl GoldMineRules {
    /// Read the selected mine ability and its typed map overrides. Worker cargo
    /// remains the worker's `Ahar` gold capacity, not a mine-owned amount.
    pub fn from_definition(
        tables: &DefaultTables,
        abilities: &Overlay,
        definition: &Definition,
    ) -> Result<Option<Self>, HarvestError> {
        let mut result = None;
        for alias in definition_abil_codes(definition)? {
            if resolve_ability_code(tables, Some(abilities), &alias)?.as_deref() != Some("Agld") {
                continue;
            }
            if result.is_some() {
                return Err(HarvestError::Ambiguous {
                    rawcode: fourcc(definition.rawcode)?,
                    column: "abilList".into(),
                    detail: "several gold-mine abilities".into(),
                });
            }
            let duration = overlay_ability_cell(
                tables,
                abilities,
                &alias,
                AbilityValue::Data(2),
                1,
                FieldKind::Unreal,
            )?;
            let work_duration = parse_f32(&alias, &duration, "DataB1")?;
            let maximum = overlay_ability_cell(
                tables,
                abilities,
                &alias,
                AbilityValue::Data(3),
                1,
                FieldKind::Int,
            )?;
            let max_workers = parse_nonneg_i32(&alias, &maximum, "DataC1")?;
            if work_duration <= 0.0 || max_workers == 0 {
                return Err(HarvestError::Malformed {
                    rawcode: alias,
                    column: "DataB1/DataC1".into(),
                    detail: "positive mining duration and worker capacity required".into(),
                });
            }
            result = Some(Self {
                source: source_of(tables),
                ability_alias: alias,
                work_duration,
                max_workers: max_workers as u32,
            });
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HarvestError {
    Missing {
        rawcode: String,
        column: String,
        detail: String,
    },
    Malformed {
        rawcode: String,
        column: String,
        detail: String,
    },
    Ambiguous {
        rawcode: String,
        column: String,
        detail: String,
    },
}

impl std::fmt::Display for HarvestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing {
                rawcode,
                column,
                detail,
            } => {
                write!(f, "harvest source missing {column} for {rawcode}: {detail}")
            }
            Self::Malformed {
                rawcode,
                column,
                detail,
            } => {
                write!(
                    f,
                    "harvest source malformed {column} for {rawcode}: {detail}"
                )
            }
            Self::Ambiguous {
                rawcode,
                column,
                detail,
            } => {
                write!(
                    f,
                    "harvest source ambiguous {column} for {rawcode}: {detail}"
                )
            }
        }
    }
}

fn source_of(tables: &DefaultTables) -> String {
    format!(
        "{}; {}",
        tables.identity().install,
        tables.identity().edition_version
    )
}

fn fourcc(rawcode: [u8; 4]) -> Result<String, HarvestError> {
    std::str::from_utf8(&rawcode)
        .map(str::to_owned)
        .map_err(|detail| HarvestError::Malformed {
            rawcode: format!("{rawcode:?}"),
            column: "rawcode".to_string(),
            detail: detail.to_string(),
        })
}

fn object_text(
    tables: &DefaultTables,
    family: Family,
    rawcode: &str,
    slk: SlkId,
    column: &str,
) -> Result<String, HarvestError> {
    let object = tables
        .object(family, rawcode)
        .map_err(|underlying| HarvestError::Missing {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: underlying.to_string(),
        })?;
    if object.rawcode() != rawcode {
        return Err(HarvestError::Missing {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: format!("resolved identity {} differs in case", object.rawcode()),
        });
    }
    object
        .cell(slk, column)
        .map_err(|underlying| HarvestError::Missing {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: underlying.to_string(),
        })
        .map(|cell| cell.text.trim().to_owned())
}

fn upgrade_overlay_source<'a>(
    upgrades: &'a Overlay,
    alias: &'a str,
) -> (&'a str, &'a [pcx_objects::Mod]) {
    match upgrades.new_object(alias) {
        Some((base, mods)) => (base, mods),
        None => (alias, upgrades.on(alias)),
    }
}

#[derive(Clone, Copy)]
enum UpgradeCellKind {
    Token,
    Integer,
    Int,
}

impl UpgradeCellKind {
    fn accepts(self, kind: FieldKind) -> bool {
        match self {
            Self::Token => matches!(kind, FieldKind::Char | FieldKind::Token),
            Self::Integer => matches!(kind, FieldKind::Int | FieldKind::Real | FieldKind::Unreal),
            Self::Int => kind == FieldKind::Int,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::Integer => "numeric",
            Self::Int => "integer",
        }
    }
}

fn upgrade_cell(
    tables: &DefaultTables,
    upgrades: &Overlay,
    alias: &str,
    column: &str,
    expected: UpgradeCellKind,
) -> Result<String, HarvestError> {
    let (base, mods) = upgrade_overlay_source(upgrades, alias);
    let matching: Vec<_> = tables
        .editor()
        .fields()
        .iter()
        .filter(|field| {
            field.from == SlkId::UpgradeMetaData
                && matches!(
                    &field.source,
                    FieldSource::Column {
                        slk: SlkId::UpgradeData,
                        column: name
                    } if name == column
                )
        })
        .collect();
    let field = match matching.as_slice() {
        [] if mods.is_empty() => {
            return object_text(tables, Family::Upgrade, base, SlkId::UpgradeData, column);
        }
        [] => {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: column.to_string(),
                detail: "missing upgrade metadata for a column with map modifications".into(),
            });
        }
        [field] => *field,
        _ => {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: column.to_string(),
                detail: "upgrade column maps to more than one metadata field".into(),
            });
        }
    };
    if !matches!(field.index, -1 | 0) {
        return Err(HarvestError::Malformed {
            rawcode: alias.to_string(),
            column: column.to_string(),
            detail: format!("unsupported indexed upgrade metadata: {}", field.index),
        });
    }
    if !expected.accepts(field.kind) {
        return Err(HarvestError::Malformed {
            rawcode: alias.to_string(),
            column: column.to_string(),
            detail: format!(
                "upgrade metadata declares {:?}, expected a {} value",
                field.kind,
                expected.name()
            ),
        });
    }
    if let Some(edit) = mods.iter().rev().find(|edit| edit.field == field.fourcc) {
        if edit.selectors != Some([0, 0]) {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: column.to_string(),
                detail: format!(
                    "unsupported upgrade override selectors {:?}; a first-level scalar override requires [0, 0]",
                    edit.selectors
                ),
            });
        }
        if !value_type_matches(edit.value_type, field.kind) {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: column.to_string(),
                detail: format!(
                    "upgrade override type {:?} differs from metadata kind {:?}",
                    edit.value_type, field.kind
                ),
            });
        }
        return value_text(alias, column, &edit.value);
    }
    object_text(tables, Family::Upgrade, base, SlkId::UpgradeData, column)
}

fn upgrade_cell_if_present(
    tables: &DefaultTables,
    upgrades: &Overlay,
    alias: &str,
    column: &str,
    expected: UpgradeCellKind,
) -> Result<Option<String>, HarvestError> {
    match upgrade_cell(tables, upgrades, alias, column, expected) {
        Ok(value) => Ok(Some(value)),
        Err(HarvestError::Missing {
            column: missing, ..
        }) if missing == column => Ok(None),
        Err(error) => Err(error),
    }
}

fn parse_upgrade_integer(
    rawcode: &str,
    text: &str,
    column: &str,
    subject: &str,
) -> Result<i32, HarvestError> {
    let value = text
        .parse::<f64>()
        .map_err(|detail| HarvestError::Malformed {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: format!("{subject} must be a finite integer, got {text:?}: {detail}"),
        })?;
    if !value.is_finite() || value.fract() != 0.0 {
        return Err(HarvestError::Malformed {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: format!("{subject} must be a finite integer, got {text:?}"),
        });
    }
    if value < f64::from(i32::MIN) || value > f64::from(i32::MAX) {
        return Err(HarvestError::Malformed {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: format!("{subject} {text:?} is outside the i32 range"),
        });
    }
    Ok(value as i32)
}

fn parse_i32(rawcode: &str, text: &str, column: &str) -> Result<i32, HarvestError> {
    text.parse::<i32>()
        .map_err(|detail| HarvestError::Malformed {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: detail.to_string(),
        })
}

fn parse_nonneg_i32(rawcode: &str, text: &str, column: &str) -> Result<i32, HarvestError> {
    let value = parse_i32(rawcode, text, column)?;
    if value >= 0 {
        Ok(value)
    } else {
        Err(HarvestError::Malformed {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: "must be nonnegative".to_string(),
        })
    }
}

fn parse_f32(rawcode: &str, text: &str, column: &str) -> Result<f32, HarvestError> {
    let value: f32 = text
        .parse::<f32>()
        .map_err(|detail| HarvestError::Malformed {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: detail.to_string(),
        })?;
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err(HarvestError::Malformed {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: "must be finite and nonnegative".to_string(),
        })
    }
}

/// Boolean `AbilityData` cell: only the literal `0`/`1` are accepted.
fn parse_exact_bool(rawcode: &str, text: &str, column: &str) -> Result<bool, HarvestError> {
    match text {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(HarvestError::Malformed {
            rawcode: rawcode.to_string(),
            column: column.to_string(),
            detail: format!("expected 0 or 1, got {text:?}"),
        }),
    }
}

fn split_csv(text: &str) -> impl Iterator<Item = &str> {
    text.split(',').map(str::trim).filter(|s| !s.is_empty())
}

fn ability_object<'a>(
    tables: &'a DefaultTables,
    alias: &str,
) -> Result<pcx_objects::editor::defaults::Object<'a>, HarvestError> {
    if alias.len() != 4 {
        return Err(HarvestError::Malformed {
            rawcode: alias.to_string(),
            column: "alias".to_string(),
            detail: "ability identity must be four bytes".to_string(),
        });
    }
    let object = tables
        .object(Family::Ability, alias)
        .map_err(|underlying| HarvestError::Missing {
            rawcode: alias.to_string(),
            column: "alias".to_string(),
            detail: underlying.to_string(),
        })?;
    if object.rawcode() != alias {
        return Err(HarvestError::Missing {
            rawcode: alias.to_string(),
            column: "alias".to_string(),
            detail: format!("resolved identity {} differs in case", object.rawcode()),
        });
    }
    Ok(object)
}

fn ability_level_cell(
    tables: &DefaultTables,
    alias: &str,
    stem: &str,
    level: u32,
) -> Result<String, HarvestError> {
    ability_object(tables, alias)?
        .level_cell(stem, level)
        .map_err(|underlying| HarvestError::Missing {
            rawcode: alias.to_string(),
            column: format!("{stem}{level}"),
            detail: underlying.to_string(),
        })
        .map(|cell| cell.text.trim().to_owned())
}

fn unit_abil_codes(tables: &DefaultTables, rawcode: &str) -> Result<Vec<String>, HarvestError> {
    let list = object_text(
        tables,
        Family::Unit,
        rawcode,
        SlkId::UnitAbilities,
        "abilList",
    )?;
    Ok(split_csv(&list).map(str::to_owned).collect())
}

/// One authored harvest input: a scalar metadata field or an ordinal `Data`
/// field. `Data` rows use the metadata `data` column and the authored level
/// suffix; the fourcc is metadata-selected, not guessed from `DataA1`.
#[derive(Debug, Clone, Copy)]
enum AbilityValue<'a> {
    Data(u8),
    Scalar(&'a str),
}

fn metadata_text<'a>(
    metadata: pcx_objects::editor::defaults::Metadata<'a>,
    attribute: &str,
) -> &'a str {
    metadata
        .attribute(attribute)
        .map(|cell| cell.text.trim())
        .unwrap_or("")
}

fn scalar_attribute(text: &str) -> bool {
    matches!(text, "" | "0" | "-" | "_")
}

fn metadata_applies(metadata: pcx_objects::editor::defaults::Metadata<'_>, base: &str) -> bool {
    let include = metadata_text(metadata, "useSpecific");
    if !include.is_empty() && !include.split(',').any(|name| name.trim() == base) {
        return false;
    }
    let exclude = metadata_text(metadata, "notSpecific");
    !exclude.split(',').any(|name| name.trim() == base)
}

/// Ability cell after map overrides. Metadata supplies the field identity,
/// ordinal and applicability; the map overlay supplies a typed level/value
/// selector. Unrelated edits survive, while an edit that would change the
/// selected harvest field is either applied or refused with context.
fn overlay_ability_cell(
    tables: &DefaultTables,
    abilities: &Overlay,
    alias: &str,
    value: AbilityValue<'_>,
    level: u32,
    kind: pcx_objects::editor::FieldKind,
) -> Result<String, HarvestError> {
    let diagnostic = match value {
        AbilityValue::Data(ordinal) => format!("Data{ordinal}"),
        AbilityValue::Scalar(stem) => stem.to_string(),
    };
    let alias_bytes = fourcc_text(alias)?;
    let (base, modifications) = crate::ability_profile::base(tables, abilities, alias_bytes)
        .map_err(|detail| HarvestError::Malformed {
            rawcode: alias.to_string(),
            column: diagnostic.clone(),
            detail,
        })?;
    let mut selected = None;
    for field in tables
        .editor()
        .fields()
        .iter()
        .filter(|field| field.from == SlkId::AbilityMetaData)
    {
        let Some(metadata) = tables.metadata(field.fourcc) else {
            continue;
        };
        let name = metadata_text(metadata, "field");
        let data = metadata_text(metadata, "data");
        let matches = match value {
            AbilityValue::Data(ordinal) => {
                name.eq_ignore_ascii_case("Data")
                    && data.parse::<u8>().is_ok_and(|value| value == ordinal)
            }
            AbilityValue::Scalar(stem) => name.eq_ignore_ascii_case(stem) && scalar_attribute(data),
        };
        if !matches
            || !scalar_attribute(metadata_text(metadata, "appendIndex"))
            || !metadata_applies(metadata, base)
        {
            continue;
        }
        if selected.replace(field.fourcc).is_some() {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: diagnostic,
                detail: "multiple applicable ability metadata fields".into(),
            });
        }
    }
    let Some(fourcc) = selected else {
        return Err(HarvestError::Missing {
            rawcode: alias.to_string(),
            column: diagnostic,
            detail: format!("no applicable ability metadata field for {base}"),
        });
    };
    let definition = tables
        .editor()
        .field(fourcc)
        .ok_or_else(|| HarvestError::Missing {
            rawcode: alias.to_string(),
            column: diagnostic.clone(),
            detail: "selected metadata field is not in the editor schema".into(),
        })?;
    if definition.kind != kind {
        return Err(HarvestError::Malformed {
            rawcode: alias.to_string(),
            column: diagnostic.clone(),
            detail: format!(
                "metadata field kind {:?} differs from expected {kind:?}",
                definition.kind
            ),
        });
    }
    let column = match value {
        AbilityValue::Data(ordinal) => {
            let letter = (1..=9)
                .contains(&ordinal)
                .then(|| char::from(b'A' + ordinal - 1))
                .ok_or_else(|| HarvestError::Malformed {
                    rawcode: alias.to_string(),
                    column: diagnostic.clone(),
                    detail: "Data ordinal outside A..I".into(),
                })?;
            format!("Data{letter}{level}")
        }
        AbilityValue::Scalar(stem) => format!("{stem}{level}"),
    };
    if let Some(entry) = modifications.iter().rev().find(|entry| {
        entry.field == fourcc
            && entry
                .selectors
                .is_some_and(|selectors| selectors[0] == level)
    }) {
        let Some([_, data]) = entry.selectors else {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: diagnostic,
                detail: "ability override lacks indexed selectors".into(),
            });
        };
        if data != 0 {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: diagnostic,
                detail: format!("unsupported data selector {data}"),
            });
        }
        if !value_type_matches(entry.value_type, kind) {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: diagnostic,
                detail: format!(
                    "override type {:?} differs from metadata kind {kind:?}",
                    entry.value_type
                ),
            });
        }
        return value_text(alias, &diagnostic, &entry.value);
    }
    if modifications
        .iter()
        .any(|entry| entry.field == fourcc && entry.selectors.is_none())
    {
        return Err(HarvestError::Malformed {
            rawcode: alias.to_string(),
            column: diagnostic,
            detail: "ability override lacks indexed selectors".into(),
        });
    }
    let object = ability_object(tables, base)?;
    let cell = object
        .cell(SlkId::AbilityData, &column)
        .map_err(|underlying| HarvestError::Missing {
            rawcode: alias.to_string(),
            column: column.clone(),
            detail: underlying.to_string(),
        })?;
    let typed = cell
        .typed(kind)
        .map_err(|underlying| HarvestError::Malformed {
            rawcode: alias.to_string(),
            column: column.clone(),
            detail: underlying.to_string(),
        })?;
    typed_text(alias, &column, typed)
}

fn typed_text(
    alias: &str,
    column: &str,
    value: pcx_objects::editor::defaults::Value<'_>,
) -> Result<String, HarvestError> {
    use pcx_objects::editor::defaults::Value;
    Ok(match value {
        Value::Integer(number) => number.to_string(),
        Value::Boolean(value) => u8::from(value).to_string(),
        Value::Real(number) => number.to_string(),
        Value::Text(text) => text.trim().to_owned(),
        Value::IntegerList(_) | Value::Marker(_) => {
            return Err(HarvestError::Malformed {
                rawcode: alias.to_string(),
                column: column.to_string(),
                detail: "harvest input is not a scalar authored value".into(),
            });
        }
    })
}

fn render_value(
    definition: &Definition,
    column: &str,
    value: &pcx_objects::Value,
) -> Result<String, HarvestError> {
    value_text(&fourcc(definition.rawcode)?, column, value)
}

fn value_type_matches(
    value_type: pcx_objects::ValueType,
    kind: pcx_objects::editor::FieldKind,
) -> bool {
    use pcx_objects::ValueType;
    match kind {
        pcx_objects::editor::FieldKind::Int
        | pcx_objects::editor::FieldKind::Bool
        | pcx_objects::editor::FieldKind::AttackBits
        | pcx_objects::editor::FieldKind::VersionFlags
        | pcx_objects::editor::FieldKind::IntList => value_type == ValueType::Int,
        pcx_objects::editor::FieldKind::Real => value_type == ValueType::Real,
        pcx_objects::editor::FieldKind::Unreal => value_type == ValueType::Unreal,
        pcx_objects::editor::FieldKind::Char | pcx_objects::editor::FieldKind::Token => {
            value_type == ValueType::Str
        }
    }
}

fn value_text(
    alias: &str,
    column: &str,
    value: &pcx_objects::Value,
) -> Result<String, HarvestError> {
    match value {
        pcx_objects::Value::Str(bytes) => {
            std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|detail| HarvestError::Malformed {
                    rawcode: alias.to_string(),
                    column: column.to_string(),
                    detail: detail.to_string(),
                })
        }
        pcx_objects::Value::Int(number) => Ok(number.to_string()),
        pcx_objects::Value::Real(number) | pcx_objects::Value::Unreal(number) => {
            Ok(number.to_string())
        }
    }
}

fn fourcc_text(text: &str) -> Result<[u8; 4], HarvestError> {
    let bytes = text.as_bytes();
    if bytes.len() != 4 {
        return Err(HarvestError::Malformed {
            rawcode: text.to_string(),
            column: "rawcode".into(),
            detail: "rawcode must be four ascii bytes".into(),
        });
    }
    let mut code = [0; 4];
    code.copy_from_slice(bytes);
    Ok(code)
}

fn definition_text(
    definition: &Definition,
    slk: SlkId,
    column: &str,
) -> Result<String, HarvestError> {
    let fields: Vec<_> = definition
        .fields
        .values()
        .filter(|field| {
            field.metadata.as_ref().is_some_and(|metadata| {
                matches!(
                    &metadata.source,
                    FieldSource::Column { slk: field_slk, column: name }
                        if *field_slk == slk && name == column
                )
            })
        })
        .collect();
    if fields.is_empty() {
        return Err(HarvestError::Missing {
            rawcode: fourcc(definition.rawcode)?,
            column: column.into(),
            detail: format!("no {slk:?} {column} field in selected definition"),
        });
    }
    if fields.len() > 1 {
        return Err(HarvestError::Malformed {
            rawcode: fourcc(definition.rawcode)?,
            column: column.into(),
            detail: "ambiguous selected definition field".into(),
        });
    }
    match &fields[0].value {
        Ok(FieldValue::Cell(text)) => Ok(text.trim().to_owned()),
        Ok(FieldValue::Override(value)) => render_value(definition, column, value),
        Err(problem) => Err(HarvestError::Missing {
            rawcode: fourcc(definition.rawcode)?,
            column: column.into(),
            detail: format!("selected field unavailable: {problem:?}"),
        }),
    }
}

pub(crate) fn definition_upgrade_codes(
    definition: &Definition,
) -> Result<Vec<String>, HarvestError> {
    match definition_text(definition, SlkId::UnitBalance, "upgradeList") {
        Ok(list) => Ok(split_csv(&list)
            .filter(|code| !matches!(*code, "-" | "_"))
            .map(str::to_owned)
            .collect()),
        Err(HarvestError::Missing { .. }) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn definition_abil_codes(definition: &Definition) -> Result<Vec<String>, HarvestError> {
    let list = definition_text(definition, SlkId::UnitAbilities, "abilList")?;
    Ok(split_csv(&list).map(str::to_owned).collect())
}

fn definition_work_weapon(definition: &Definition) -> Result<Option<WorkWeapon>, HarvestError> {
    let raw = fourcc(definition.rawcode)?;
    let weaps_on = match definition_text(definition, SlkId::UnitWeapons, "weapsOn") {
        Ok(text) => text,
        Err(HarvestError::Missing { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    let mask = parse_i32(&raw, &weaps_on, "weapsOn")?;
    if mask < 0 || mask & !0x3 != 0 {
        return Err(HarvestError::Malformed {
            rawcode: raw,
            column: "weapsOn".into(),
            detail: "weapon-slot mask must be a nonnegative 2-bit mask".into(),
        });
    }
    let mut found: Option<WorkWeapon> = None;
    for slot in [1u32, 2] {
        if mask & (1 << (slot - 1)) == 0 {
            continue;
        }
        let targs = match definition_text(definition, SlkId::UnitWeapons, &format!("targs{slot}")) {
            Ok(text) => text,
            Err(HarvestError::Missing { .. }) => continue,
            Err(error) => return Err(error),
        };
        if !split_csv(&targs).any(|target| target == "tree") {
            continue;
        }
        if found.is_some() {
            return Err(HarvestError::Ambiguous {
                rawcode: raw,
                column: format!("targs{slot}"),
                detail: "multiple weapon slots target trees".into(),
            });
        }
        let cool = parse_f32(
            &raw,
            &definition_text(definition, SlkId::UnitWeapons, &format!("cool{slot}"))?,
            &format!("cool{slot}"),
        )?;
        let damage_point = parse_f32(
            &raw,
            &definition_text(definition, SlkId::UnitWeapons, &format!("dmgpt{slot}"))?,
            &format!("dmgpt{slot}"),
        )?;
        let backswing = parse_f32(
            &raw,
            &definition_text(definition, SlkId::UnitWeapons, &format!("backSw{slot}"))?,
            &format!("backSw{slot}"),
        )?;
        let range = parse_f32(
            &raw,
            &definition_text(definition, SlkId::UnitWeapons, &format!("rangeN{slot}"))?,
            &format!("rangeN{slot}"),
        )?;
        found = Some(WorkWeapon {
            slot,
            cool,
            damage_point,
            backswing,
            range,
        });
    }
    Ok(found)
}

fn resolve_ability_code(
    tables: &DefaultTables,
    abilities: Option<&Overlay>,
    alias: &str,
) -> Result<Option<String>, HarvestError> {
    if matches!(alias, SENTINEL | "_") {
        return Ok(None);
    }
    let base = match abilities {
        Some(overlay) => {
            crate::ability_profile::base(tables, overlay, fourcc_text(alias)?)
                .map_err(|detail| HarvestError::Malformed {
                    rawcode: alias.to_string(),
                    column: "code".to_string(),
                    detail,
                })?
                .0
        }
        None => alias,
    };
    let code = ability_object(tables, base)?
        .cell(SlkId::AbilityData, "code")
        .map_err(|underlying| HarvestError::Missing {
            rawcode: alias.to_string(),
            column: "code".to_string(),
            detail: underlying.to_string(),
        })?
        .text
        .trim()
        .to_owned();
    if code == SENTINEL {
        return Ok(None);
    }
    if code.len() != 4 || !code.is_ascii() {
        return Err(HarvestError::Malformed {
            rawcode: alias.to_string(),
            column: "code".to_string(),
            detail: format!("ability code must be four ascii bytes, got {code:?}"),
        });
    }
    Ok(Some(code))
}

/// The sole alias whose resolved code equals `Ahar`, if any.
fn find_ahar_alias(
    tables: &DefaultTables,
    abilities: Option<&Overlay>,
    aliases: &[String],
) -> Result<Option<String>, HarvestError> {
    let mut found: Option<String> = None;
    for alias in aliases {
        let Some(code) = resolve_ability_code(tables, abilities, alias)? else {
            continue;
        };
        if code != AHAR {
            continue;
        }
        if found.is_some() {
            return Err(HarvestError::Ambiguous {
                rawcode: alias.clone(),
                column: "abilList".to_string(),
                detail: "multiple abilities resolve to Ahar".to_string(),
            });
        }
        found = Some(alias.clone());
    }
    Ok(found)
}

fn work_weapon_from_tables(
    tables: &DefaultTables,
    worker_code: &str,
) -> Result<Option<WorkWeapon>, HarvestError> {
    let weaps_on = match object_text(
        tables,
        Family::Unit,
        worker_code,
        SlkId::UnitWeapons,
        "weapsOn",
    ) {
        Ok(text) => text,
        Err(HarvestError::Missing { .. }) => return Ok(None),
        Err(other) => return Err(other),
    };
    let mask = parse_i32(worker_code, &weaps_on, "weapsOn")?;
    if mask < 0 || mask & !0x3 != 0 {
        return Err(HarvestError::Malformed {
            rawcode: worker_code.to_string(),
            column: "weapsOn".to_string(),
            detail: "weapon-slot mask must be a nonnegative 2-bit mask".to_string(),
        });
    }
    let mut found: Option<WorkWeapon> = None;
    for slot in [1u32, 2] {
        if mask & (1 << (slot - 1)) == 0 {
            continue;
        }
        let targs = object_text(
            tables,
            Family::Unit,
            worker_code,
            SlkId::UnitWeapons,
            &format!("targs{slot}"),
        )?;
        if !split_csv(&targs).any(|t| t == "tree") {
            continue;
        }
        if found.is_some() {
            return Err(HarvestError::Ambiguous {
                rawcode: worker_code.to_string(),
                column: format!("targs{slot}"),
                detail: "multiple weapon slots target trees".to_string(),
            });
        }
        let cool = parse_f32(
            worker_code,
            &object_text(
                tables,
                Family::Unit,
                worker_code,
                SlkId::UnitWeapons,
                &format!("cool{slot}"),
            )?,
            &format!("cool{slot}"),
        )?;
        let damage_point = parse_f32(
            worker_code,
            &object_text(
                tables,
                Family::Unit,
                worker_code,
                SlkId::UnitWeapons,
                &format!("dmgpt{slot}"),
            )?,
            &format!("dmgpt{slot}"),
        )?;
        let backswing = parse_f32(
            worker_code,
            &object_text(
                tables,
                Family::Unit,
                worker_code,
                SlkId::UnitWeapons,
                &format!("backSw{slot}"),
            )?,
            &format!("backSw{slot}"),
        )?;
        let range = parse_f32(
            worker_code,
            &object_text(
                tables,
                Family::Unit,
                worker_code,
                SlkId::UnitWeapons,
                &format!("rangeN{slot}"),
            )?,
            &format!("rangeN{slot}"),
        )?;
        found = Some(WorkWeapon {
            slot,
            cool,
            damage_point,
            backswing,
            range,
        });
    }
    Ok(found)
}

impl HarvestRules {
    /// `Ok(None)` when the worker has no `Ahar` capability.
    pub fn from_tables(
        tables: &DefaultTables,
        worker_code: [u8; 4],
    ) -> Result<Option<Self>, HarvestError> {
        let raw = fourcc(worker_code)?;
        let aliases = unit_abil_codes(tables, &raw)?;
        let work_weapon = work_weapon_from_tables(tables, &raw)?;
        Self::from_alias_list(tables, None, &raw, &aliases, work_weapon)
    }

    /// Rules from the map-selected unit definition, with ability overrides.
    pub fn from_definition(
        tables: &DefaultTables,
        abilities: &Overlay,
        definition: &Definition,
    ) -> Result<Option<Self>, HarvestError> {
        let raw = fourcc(definition.rawcode)?;
        let aliases = definition_abil_codes(definition)?;
        let work_weapon = definition_work_weapon(definition)?;
        Self::from_alias_list(tables, Some(abilities), &raw, &aliases, work_weapon)
    }

    /// Apply the authored `UpgradeData` effects that change this worker's
    /// harvest inputs. Only the `rlum` lumber-capacity effect is consumed here;
    /// active combat, armor and movement upgrades remain unit-runtime state.
    ///
    /// `level` returns the player's applied level for a fourcc tech integer.
    /// Absent or inactive levels are zero. A malformed active effect or a level
    /// beyond the authored maximum is a contextual refusal, never a default.
    pub fn apply_lumber_harvesting_upgrades(
        &mut self,
        tables: &DefaultTables,
        upgrades: &Overlay,
        definition: &Definition,
        mut level: impl FnMut(i32) -> Result<i32, HarvestError>,
    ) -> Result<(), HarvestError> {
        let worker = fourcc(definition.rawcode)?;
        for alias in definition_upgrade_codes(definition)? {
            let code = fourcc_text(&alias)?;
            let active = level(i32::from_be_bytes(code))?;
            if active <= 0 {
                continue;
            }
            let (base, _) = upgrade_overlay_source(upgrades, &alias);
            let upgrade = tables.object(Family::Upgrade, base).map_err(|underlying| {
                HarvestError::Missing {
                    rawcode: alias.clone(),
                    column: "upgradeid".into(),
                    detail: underlying.to_string(),
                }
            })?;
            if upgrade.rawcode() != base {
                return Err(HarvestError::Missing {
                    rawcode: alias.clone(),
                    column: "upgradeid".into(),
                    detail: format!("resolved identity {} differs in case", upgrade.rawcode()),
                });
            }
            let maximum = upgrade_cell(tables, upgrades, &alias, "maxlevel", UpgradeCellKind::Int)?;
            let maximum = parse_i32(&alias, &maximum, "maxlevel")?;
            if active > maximum {
                return Err(HarvestError::Malformed {
                    rawcode: alias.clone(),
                    column: "maxlevel".into(),
                    detail: format!("applied level {active} exceeds authored maximum {maximum}"),
                });
            }
            for slot in 1..=4 {
                let Some(effect) = upgrade_cell_if_present(
                    tables,
                    upgrades,
                    &alias,
                    &format!("effect{slot}"),
                    UpgradeCellKind::Token,
                )?
                else {
                    continue;
                };
                if effect != "rlum" {
                    continue;
                }
                let base = upgrade_cell(
                    tables,
                    upgrades,
                    &alias,
                    &format!("base{slot}"),
                    UpgradeCellKind::Integer,
                )?;
                let increment = upgrade_cell(
                    tables,
                    upgrades,
                    &alias,
                    &format!("mod{slot}"),
                    UpgradeCellKind::Integer,
                )?;
                let base = parse_upgrade_integer(
                    &alias,
                    &base,
                    &format!("base{slot}"),
                    "lumber-capacity base effect",
                )?;
                let increment = parse_upgrade_integer(
                    &alias,
                    &increment,
                    &format!("mod{slot}"),
                    "lumber-capacity per-level effect",
                )?;
                let delta = increment
                    .checked_mul(active - 1)
                    .and_then(|scaled| base.checked_add(scaled))
                    .ok_or_else(|| HarvestError::Malformed {
                        rawcode: alias.clone(),
                        column: format!("base{slot}"),
                        detail: "lumber-capacity effect overflows".into(),
                    })?;
                self.lumber_capacity =
                    self.lumber_capacity.checked_add(delta).ok_or_else(|| {
                        HarvestError::Malformed {
                            rawcode: worker.clone(),
                            column: "Lumber Capacity".into(),
                            detail: "active upgrade capacity overflows".into(),
                        }
                    })?;
            }
        }
        Ok(())
    }

    fn from_alias_list(
        tables: &DefaultTables,
        abilities: Option<&Overlay>,
        raw: &str,
        aliases: &[String],
        work_weapon: Option<WorkWeapon>,
    ) -> Result<Option<Self>, HarvestError> {
        let Some(ahar_alias) = find_ahar_alias(tables, abilities, aliases)? else {
            return Ok(None);
        };
        let cell = |value: AbilityValue<'_>, kind: pcx_objects::editor::FieldKind, column: &str| {
            match abilities {
                Some(overlay) => overlay_ability_cell(tables, overlay, &ahar_alias, value, 1, kind),
                None => match value {
                    AbilityValue::Data(ordinal) => {
                        let stem = format!("Data{}", char::from(b'A' + ordinal - 1));
                        ability_level_cell(tables, &ahar_alias, &stem, 1)
                    }
                    AbilityValue::Scalar(stem) => ability_level_cell(tables, &ahar_alias, stem, 1),
                }
                .map_err(|error| match error {
                    HarvestError::Missing {
                        rawcode, detail, ..
                    } => HarvestError::Missing {
                        rawcode,
                        column: column.to_string(),
                        detail,
                    },
                    other => other,
                }),
            }
        };
        let damage = parse_i32(
            &ahar_alias,
            &cell(
                AbilityValue::Data(1),
                pcx_objects::editor::FieldKind::Int,
                "DataA1",
            )?,
            "DataA1",
        )? as f32;
        let lumber_capacity = parse_nonneg_i32(
            &ahar_alias,
            &cell(
                AbilityValue::Data(2),
                pcx_objects::editor::FieldKind::Int,
                "DataB1",
            )?,
            "DataB1",
        )?;
        let gold_capacity = parse_nonneg_i32(
            &ahar_alias,
            &cell(
                AbilityValue::Data(3),
                pcx_objects::editor::FieldKind::Int,
                "DataC1",
            )?,
            "DataC1",
        )?;
        let range = parse_f32(
            &ahar_alias,
            &cell(
                AbilityValue::Scalar("Rng"),
                pcx_objects::editor::FieldKind::Unreal,
                "Rng1",
            )?,
            "Rng1",
        )?;
        let search_radius = parse_f32(
            &ahar_alias,
            &cell(
                AbilityValue::Scalar("Area"),
                pcx_objects::editor::FieldKind::Unreal,
                "Area1",
            )?,
            "Area1",
        )?;
        let work_duration = parse_f32(
            &ahar_alias,
            &cell(
                AbilityValue::Scalar("Dur"),
                pcx_objects::editor::FieldKind::Unreal,
                "Dur1",
            )?,
            "Dur1",
        )?;
        let _ = raw;
        Ok(Some(Self {
            source: source_of(tables),
            ahar_alias,
            damage,
            lumber_capacity,
            gold_capacity,
            search_radius,
            range,
            work_duration,
            work_weapon,
        }))
    }
}

impl TreeClass {
    pub fn from_tables(tables: &DefaultTables, tree_code: [u8; 4]) -> Result<Self, HarvestError> {
        let raw = fourcc(tree_code)?;
        let targ = object_text(
            tables,
            Family::Destructable,
            &raw,
            SlkId::DestructableData,
            "targType",
        )?;
        let hp_text = object_text(
            tables,
            Family::Destructable,
            &raw,
            SlkId::DestructableData,
            "HP",
        )?;
        let hp = parse_f32(&raw, &hp_text, "HP")?;
        Ok(Self {
            source: source_of(tables),
            hp,
            is_tree: split_csv(&targ).any(|t| t == "tree"),
        })
    }
}

impl DepotReturn {
    pub fn from_tables(
        tables: &DefaultTables,
        building_code: [u8; 4],
    ) -> Result<Self, HarvestError> {
        let raw = fourcc(building_code)?;
        let aliases = unit_abil_codes(tables, &raw)?;
        Self::from_alias_list(tables, None, &aliases)
    }

    /// Depot flags from the map-selected building definition.
    pub fn from_definition(
        tables: &DefaultTables,
        abilities: &Overlay,
        definition: &Definition,
    ) -> Result<Self, HarvestError> {
        let aliases = definition_abil_codes(definition)?;
        Self::from_alias_list(tables, Some(abilities), &aliases)
    }

    fn from_alias_list(
        tables: &DefaultTables,
        abilities: Option<&Overlay>,
        aliases: &[String],
    ) -> Result<Self, HarvestError> {
        let mut accepts_gold = false;
        let mut accepts_lumber = false;
        for alias in aliases {
            let Some(code) = resolve_ability_code(tables, abilities, alias)? else {
                continue;
            };
            if code != ARTN {
                continue;
            }
            let cell = |value: AbilityValue<'_>,
                        kind: pcx_objects::editor::FieldKind,
                        column: &str| match abilities {
                Some(overlay) => overlay_ability_cell(tables, overlay, alias, value, 1, kind),
                None => match value {
                    AbilityValue::Data(ordinal) => {
                        let stem = format!("Data{}", char::from(b'A' + ordinal - 1));
                        ability_level_cell(tables, alias, &stem, 1)
                    }
                    AbilityValue::Scalar(stem) => ability_level_cell(tables, alias, stem, 1),
                }
                .map_err(|error| match error {
                    HarvestError::Missing {
                        rawcode, detail, ..
                    } => HarvestError::Missing {
                        rawcode,
                        column: column.to_string(),
                        detail,
                    },
                    other => other,
                }),
            };
            let gold = parse_exact_bool(
                alias,
                &cell(
                    AbilityValue::Data(1),
                    pcx_objects::editor::FieldKind::Bool,
                    "DataA1",
                )?,
                "DataA1",
            )?;
            let lumber = parse_exact_bool(
                alias,
                &cell(
                    AbilityValue::Data(2),
                    pcx_objects::editor::FieldKind::Bool,
                    "DataB1",
                )?,
                "DataB1",
            )?;
            accepts_gold |= gold;
            accepts_lumber |= lumber;
        }
        Ok(Self {
            source: source_of(tables),
            accepts_gold,
            accepts_lumber,
        })
    }
}

#[cfg(test)]
#[path = "harvest_rules_tests.rs"]
mod tests;
