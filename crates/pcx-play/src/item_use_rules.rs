//! Source-derived item charges and cooldown groups.
//!
//! Field ids are the `UnitMetaData.slk` rows for `ItemData`: `iuse` (uses), `iper` (perishable),
//! `iicd` (ignoreCD), `icid` (cooldownID) and `iabi` (abilList). The duration is the item's one
//! ability's level-1 `acdn` (`Cool`) with W3A overrides applied. Values come from the definition
//! fields and the ability overlay, so the selected install is the only source.
use pcx_objects::Value as OverrideValue;
use pcx_objects::editor::defaults::{DefaultTables, Family};
use pcx_objects::editor::{FieldKind, Overlay};
use pcx_sim::world::{Definition, FieldProblem, FieldValue};
use std::sync::Arc;

const USES: [u8; 4] = *b"iuse";
const PERISHABLE: [u8; 4] = *b"iper";
const IGNORE_COOLDOWN: [u8; 4] = *b"iicd";
const COOLDOWN_ID: [u8; 4] = *b"icid";
const ABILITY_LIST: [u8; 4] = *b"iabi";
const COOLDOWN: [u8; 4] = *b"acdn";
/// `uses` marker for an item without charges.
const UNCHARGED: &str = "-";
/// Ability level whose cooldown the item uses.
const COOLDOWN_LEVEL: u32 = 1;

/// Charge model from the source `uses` cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charges {
    /// No charge count and no consumption on use.
    Uncharged,
    /// Authored initial count; each use consumes one.
    Charged(u32),
}

/// A cooldown group and its duration in game seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct Cooldown {
    pub group: String,
    pub seconds: f64,
    /// `ignoreCD`: the item is not refused while its group is cooling down.
    pub ignores_wait: bool,
}

#[derive(Clone, Debug)]
pub struct Rules {
    tables: Arc<DefaultTables>,
    overlay: Overlay,
}

impl Rules {
    pub fn new(tables: Arc<DefaultTables>, overlay: Overlay) -> Self {
        Self { tables, overlay }
    }

    /// The initial charge model. A missing or non-count `uses` cell refuses.
    pub fn charges(&self, definition: &Definition) -> Result<Charges, String> {
        let text = field_text(definition, USES, "uses")?;
        if text == UNCHARGED {
            return Ok(Charges::Uncharged);
        }
        text.parse()
            .map(Charges::Charged)
            .map_err(|_| format!("uses {text} is not a charge count"))
    }

    pub fn perishable(&self, definition: &Definition) -> Result<bool, String> {
        flag(definition, PERISHABLE, "perishable")
    }

    /// `None` when the item names no cooldown group. A missing `cooldownID` cell is that case.
    pub fn cooldown(&self, definition: &Definition) -> Result<Option<Cooldown>, String> {
        let group = match definition.fields.get(&COOLDOWN_ID).map(|f| &f.value) {
            None | Some(Err(FieldProblem::Missing)) => return Ok(None),
            Some(_) => field_text(definition, COOLDOWN_ID, "cooldownID")?,
        };
        if group.is_empty() {
            return Ok(None);
        }
        let abilities = field_text(definition, ABILITY_LIST, "abilList")?;
        let [alias] = abilities
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty() && *a != "_" && *a != UNCHARGED)
            .collect::<Vec<_>>()[..]
        else {
            return Err(format!("cooldown group {group} needs exactly one ability"));
        };
        Ok(Some(Cooldown {
            group,
            seconds: self.level_cooldown(alias)?,
            ignores_wait: flag(definition, IGNORE_COOLDOWN, "ignoreCD")?,
        }))
    }

    /// Level-1 `Cool` of an ability alias, after the map's W3A override for that field.
    fn level_cooldown(&self, alias: &str) -> Result<f64, String> {
        let mods = self
            .overlay
            .new_object(alias)
            .map(|(_, m)| m)
            .unwrap_or_else(|| self.overlay.on(alias));
        if let Some(entry) = mods
            .iter()
            .rev()
            .find(|m| m.field == COOLDOWN && m.selectors.is_none_or(|s| s[0] == COOLDOWN_LEVEL))
        {
            if entry.selectors.is_some_and(|s| s[1] != 0) {
                return Err(format!(
                    "indexed cooldown override on {alias} is unsupported"
                ));
            }
            let OverrideValue::Real(seconds) = entry.value else {
                return Err(format!("noninteger cooldown override on {alias}"));
            };
            return finite_seconds(f64::from(seconds), alias);
        }
        let object = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?;
        let cell = object
            .level_cell("Cool", COOLDOWN_LEVEL)
            .map_err(|e| e.to_string())?;
        match cell.typed(FieldKind::Real).map_err(|e| e.to_string())? {
            pcx_objects::editor::defaults::Value::Real(seconds) => finite_seconds(seconds, alias),
            _ => Err(format!("cooldown of {alias} is not real")),
        }
    }
}

fn finite_seconds(seconds: f64, alias: &str) -> Result<f64, String> {
    if seconds.is_finite() && seconds >= 0.0 {
        Ok(seconds)
    } else {
        Err(format!("cooldown of {alias} is not a nonnegative duration"))
    }
}

/// Authored text of a field: a base cell or a typed map override, trimmed.
fn field_text(definition: &Definition, fourcc: [u8; 4], name: &str) -> Result<String, String> {
    let field = definition
        .fields
        .get(&fourcc)
        .ok_or_else(|| format!("item {name} is absent"))?;
    match &field.value {
        Ok(FieldValue::Cell(text)) => Ok(text.trim().to_owned()),
        Ok(FieldValue::Override(OverrideValue::Int(n))) => Ok(n.to_string()),
        Ok(FieldValue::Override(OverrideValue::Str(bytes))) => String::from_utf8(bytes.clone())
            .map(|s| s.trim().to_owned())
            .map_err(|_| format!("item {name} is not text")),
        _ => Err(format!("item {name} is not derived")),
    }
}

fn flag(definition: &Definition, fourcc: [u8; 4], name: &str) -> Result<bool, String> {
    match field_text(definition, fourcc, name)?.as_str() {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("item {name} is not a boolean flag")),
    }
}

#[cfg(test)]
#[path = "item_use_rules_tests.rs"]
mod tests;
