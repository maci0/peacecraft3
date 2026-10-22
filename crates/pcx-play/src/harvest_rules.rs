//! Harvest rules read from the selected object/data tables.
//!
//! This is a **source-read** module: it parses and validates the authored
//! `Ahar` (Harvest) fields, worker/unit capability, destructable tree
//! classification and the depot `Artn` return flags. It runs no gameplay
//! harvesting algorithm — that is deferred until a native trace qualifies it.
//!
//! Identities and class codes are case-sensitive, matching the shipped
//! fourcc/code cells; the `-` sentinel means absence, never a lookup.
//! Missing harvest capability is reported as `None`, not as numbers from an
//! absent `Ahar` record.
use pcx_objects::editor::SlkId;
use pcx_objects::editor::defaults::{DefaultTables, Family};

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

fn resolve_ability_code(
    tables: &DefaultTables,
    alias: &str,
) -> Result<Option<String>, HarvestError> {
    if alias == SENTINEL {
        return Ok(None);
    }
    let code = ability_object(tables, alias)?
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
    aliases: &[String],
) -> Result<Option<String>, HarvestError> {
    let mut found: Option<String> = None;
    for alias in aliases {
        let Some(code) = resolve_ability_code(tables, alias)? else {
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
        let Some(ahar_alias) = find_ahar_alias(tables, &aliases)? else {
            return Ok(None);
        };
        let damage = parse_f32(
            &ahar_alias,
            &ability_level_cell(tables, &ahar_alias, "DataA", 1)?,
            "DataA1",
        )?;
        let lumber_capacity = parse_nonneg_i32(
            &ahar_alias,
            &ability_level_cell(tables, &ahar_alias, "DataB", 1)?,
            "DataB1",
        )?;
        let gold_capacity = parse_nonneg_i32(
            &ahar_alias,
            &ability_level_cell(tables, &ahar_alias, "DataC", 1)?,
            "DataC1",
        )?;
        let range = parse_f32(
            &ahar_alias,
            &ability_level_cell(tables, &ahar_alias, "Rng", 1)?,
            "Rng1",
        )?;
        let search_radius = parse_f32(
            &ahar_alias,
            &ability_level_cell(tables, &ahar_alias, "Area", 1)?,
            "Area1",
        )?;
        let work_duration = parse_f32(
            &ahar_alias,
            &ability_level_cell(tables, &ahar_alias, "Dur", 1)?,
            "Dur1",
        )?;
        let work_weapon = work_weapon_from_tables(tables, &raw)?;
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
        let mut accepts_gold = false;
        let mut accepts_lumber = false;
        for alias in unit_abil_codes(tables, &raw)? {
            let Some(code) = resolve_ability_code(tables, &alias)? else {
                continue;
            };
            if code != ARTN {
                continue;
            }
            let gold = parse_exact_bool(
                &alias,
                &ability_level_cell(tables, &alias, "DataA", 1)?,
                "DataA1",
            )?;
            let lumber = parse_exact_bool(
                &alias,
                &ability_level_cell(tables, &alias, "DataB", 1)?,
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
