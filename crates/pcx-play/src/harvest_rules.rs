//! Harvest rules read from the selected object/data tables.
//!
//! This is a **source-read** module: it parses and validates the authored
//! `Ahar` (Harvest) fields, worker/unit capability, destructable tree
//! classification and the depot `Artn` return flags. It runs no gameplay
//! harvesting algorithm — that is deferred until a native trace qualifies it.
//!
//! Rows read (shipped tables):
//! - `Units\AbilityData.slk` `Ahar`: `DataA` (damage to tree), `DataB`
//!   (lumber capacity), `DataC` (gold capacity), `Rng`, `Area`, `Dur`.
//! - `Units\UnitAbilities.slk` worker `abilList` and building `abilList`.
//! - `Units\DestructableData.slk` tree `targType`/`HP`.
//! - `Artn` return ability (aliases `Argd`/`Argl`/`Arlm`) `DataA` (accepts
//!   gold), `DataB` (accepts lumber).
use pcx_objects::editor::defaults::{DefaultTables, Family};
use pcx_objects::editor::SlkId;

#[derive(Debug, Clone, PartialEq)]
pub struct HarvestRules {
    pub source: String,
    pub has_harvest: bool,
    pub damage: f32,
    pub lumber_capacity: i32,
    pub gold_capacity: i32,
    pub search_radius: f32,
    pub range: f32,
    pub work_duration: f32,
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
    Missing { rawcode: String, column: String },
    Malformed { rawcode: String, column: String, detail: String },
}

impl std::fmt::Display for HarvestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing { rawcode, column } => {
                write!(f, "harvest source missing {column} for {rawcode}")
            }
            Self::Malformed { rawcode, column, detail } => {
                write!(f, "harvest source malformed {column} for {rawcode}: {detail}")
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

fn cell_text(tables: &DefaultTables, family: Family, rawcode: &str, slk: SlkId, column: &str) -> Result<String, HarvestError> {
    let object = tables.object(family, rawcode).map_err(|_| HarvestError::Missing {
        rawcode: rawcode.to_string(),
        column: column.to_string(),
    })?;
    object.cell(slk, column).map_err(|_| HarvestError::Missing {
        rawcode: rawcode.to_string(),
        column: column.to_string(),
    }).map(|cell| cell.text.trim().to_owned())
}

fn number_i32(_tables: &DefaultTables, rawcode: &str, text: &str, column: &str) -> Result<i32, HarvestError> {
    text.parse::<i32>().map_err(|detail| HarvestError::Malformed {
        rawcode: rawcode.to_string(),
        column: column.to_string(),
        detail: detail.to_string(),
    })
}

fn number_f32(_tables: &DefaultTables, rawcode: &str, text: &str, column: &str) -> Result<f32, HarvestError> {
    let value: f32 = text.parse::<f32>().map_err(|detail| HarvestError::Malformed {
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

fn ability_level_cell(tables: &DefaultTables, rawcode: &str, stem: &str, level: u32) -> Result<String, HarvestError> {
    let object = tables.object(Family::Ability, rawcode).map_err(|_| HarvestError::Missing {
        rawcode: rawcode.to_string(),
        column: format!("{stem}{level}"),
    })?;
    object.level_cell(stem, level).map_err(|_| HarvestError::Missing {
        rawcode: rawcode.to_string(),
        column: format!("{stem}{level}"),
    }).map(|cell| cell.text.trim().to_owned())
}

fn ability_codes(tables: &DefaultTables, rawcode: &str) -> Result<Vec<String>, HarvestError> {
    let list = cell_text(tables, Family::Unit, rawcode, SlkId::UnitAbilities, "abilList")?;
    Ok(list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect())
}

impl HarvestRules {
    pub fn from_tables(tables: &DefaultTables, worker_code: [u8; 4]) -> Result<Self, HarvestError> {
        let raw = String::from_utf8_lossy(&worker_code).into_owned();
        let abilities = ability_codes(tables, &raw)?;
        let has_harvest = abilities.iter().any(|a| a.eq_ignore_ascii_case("Ahar"));
        let damage = number_f32(tables, "Ahar", &ability_level_cell(tables, "Ahar", "DataA", 1)?, "DataA1")?;
        let lumber_capacity = number_i32(tables, "Ahar", &ability_level_cell(tables, "Ahar", "DataB", 1)?, "DataB1")?;
        let gold_capacity = number_i32(tables, "Ahar", &ability_level_cell(tables, "Ahar", "DataC", 1)?, "DataC1")?;
        let range = number_f32(tables, "Ahar", &ability_level_cell(tables, "Ahar", "Rng", 1)?, "Rng1")?;
        let search_radius = number_f32(tables, "Ahar", &ability_level_cell(tables, "Ahar", "Area", 1)?, "Area1")?;
        let work_duration = number_f32(tables, "Ahar", &ability_level_cell(tables, "Ahar", "Dur", 1)?, "Dur1")?;
        Ok(Self {
            source: source_of(tables),
            has_harvest,
            damage,
            lumber_capacity,
            gold_capacity,
            search_radius,
            range,
            work_duration,
        })
    }
}

impl TreeClass {
    pub fn from_tables(tables: &DefaultTables, tree_code: [u8; 4]) -> Result<Self, HarvestError> {
        let raw = String::from_utf8_lossy(&tree_code).into_owned();
        let targ = cell_text(tables, Family::Destructable, &raw, SlkId::DestructableData, "targType")?;
        let hp_text = cell_text(tables, Family::Destructable, &raw, SlkId::DestructableData, "HP")?;
        let hp = number_f32(tables, &raw, &hp_text, "HP")?;
        Ok(Self {
            source: source_of(tables),
            hp,
            is_tree: targ.eq_ignore_ascii_case("tree"),
        })
    }
}

impl DepotReturn {
    pub fn from_tables(tables: &DefaultTables, building_code: [u8; 4]) -> Result<Self, HarvestError> {
        let raw = String::from_utf8_lossy(&building_code).into_owned();
        let mut accepts_gold = false;
        let mut accepts_lumber = false;
        for ability in ability_codes(tables, &raw)? {
            let code = cell_text(tables, Family::Ability, &ability, SlkId::AbilityData, "code")?;
            if !code.eq_ignore_ascii_case("Artn") {
                continue;
            }
            let gold = number_i32(tables, &ability, &ability_level_cell(tables, &ability, "DataA", 1)?, "DataA1")?;
            let lumber = number_i32(tables, &ability, &ability_level_cell(tables, &ability, "DataB", 1)?, "DataB1")?;
            accepts_gold |= gold != 0;
            accepts_lumber |= lumber != 0;
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
