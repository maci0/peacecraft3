//! Source-derived hero attribute abilities: automatic powerups (permanent strength on use) and
//! passive held-item bonuses (strength and agility while held). Both read ability classes from
//! the reviewed client's registration operands and amounts from selected ability metadata with
//! W3A overlays.
use crate::{
    menu_lighting::REVIEWED_IMAGE,
    neutral_player::{RuleInputBudget, Sha256Digest, sha256},
    neutral_rule_input::Pe,
    scripts::AssetIdentity,
};
use pcx_objects::editor::defaults::{DefaultTables, Family, Object, Value};
use pcx_objects::editor::{FieldKind, Overlay, SlkId};
use pcx_sim::world::Definition;
use std::sync::Arc;

/// Registration call for the powerup class (source `AIsm`, base `AIxm`).
const POWERUP_WINDOW: u32 = 0xbca0e0;
/// Registration call for the passive attribute class (source `AIab`, base `APbo`).
const PASSIVE_WINDOW: u32 = 0xbc9ce0;
/// Registration call for the passive armor class (source `AIde`, base `APbo`).
const ARMOR_WINDOW: u32 = 0xbc9d40;
const CLASS_WINDOW_LEN: usize = 24;
const PUSH_IMM32: u8 = 0x68;
const CALL_REL32: u8 = 0xe8;
const RET: u8 = 0xc3;
const CLEANUP_RET: [u8; 4] = [0x83, 0xc4, 0x0c, RET];
const MAX_IMAGE_BYTES: usize = 32 << 20;
const IMAGE_BUDGET: RuleInputBudget = RuleInputBudget {
    image_bytes: MAX_IMAGE_BYTES,
    profile_bytes: 64 << 10,
    sections: 96,
};
/// Data columns are `DataA`..`DataZ`, so a metadata index names one of 26 letters.
const DATA_COLUMNS: u32 = 26;
/// The item ability's first level. Effect lookup reads `Data{column}{level}` at this level.
const EFFECT_LEVEL: u32 = 1;

/// Permanent hero strength granted by one tome. Only nonnegative authored values are admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StrengthEffect {
    pub(crate) strength: i32,
}

impl StrengthEffect {
    pub fn strength(&self) -> i32 {
        self.strength
    }
}

/// Flat strength, agility and armor from passive held items. Strength and agility also include
/// retained powerup strength in a unit total. Armor points are added to unit armor while held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttributeBonus {
    pub strength: i32,
    pub agility: i32,
    pub armor: i32,
}

#[derive(Clone, Debug)]
struct Column {
    field: [u8; 4],
    data: u32,
    aliases: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub origin: AssetIdentity,
    pub image_sha256: Sha256Digest,
    tables: Arc<DefaultTables>,
    overlay: Overlay,
    powerup_class: String,
    passive_class: String,
    armor_class: String,
    strength: Column,
    agility: Column,
    intelligence: Column,
    armor: Column,
}

impl Rules {
    /// Reads both registration windows from the reviewed client image and the selected tables.
    pub fn read(
        tables: Arc<DefaultTables>,
        overlay: Overlay,
        image: &[u8],
        origin: AssetIdentity,
    ) -> Result<Self, String> {
        let context = format!("inventory attribute rules {}", origin.source);
        let fail = |why: &str| format!("{context}: {why}");
        let digest = sha256(image);
        if image.len() > MAX_IMAGE_BYTES
            || digest != REVIEWED_IMAGE
            || [&origin.source, &origin.edition, &origin.version]
                .iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(fail("unsupported client identity"));
        }
        let pe = Pe::parse(image, IMAGE_BUDGET).map_err(|e| fail(&e.to_string()))?;
        let window = |va: u32| {
            pe.read_from(va, true)
                .map_err(|e| fail(&e.to_string()))?
                .get(..CLASS_WINDOW_LEN)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| fail("truncated class registration window"))
        };
        let powerup = window(POWERUP_WINDOW)?;
        let passive = window(PASSIVE_WINDOW)?;
        let armor = window(ARMOR_WINDOW)?;
        Self::decode(tables, overlay, origin, digest, &powerup, &passive, &armor)
            .map_err(|e| fail(&e))
    }

    pub(crate) fn decode(
        tables: Arc<DefaultTables>,
        overlay: Overlay,
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        powerup_window: &[u8],
        passive_window: &[u8],
        armor_window: &[u8],
    ) -> Result<Self, String> {
        let (powerup_class, powerup_base) = class_from_window(powerup_window)?;
        // The passive and armor bases (`APbo`) are native classes with no ability row, so they are
        // not checked against metadata.
        let (passive_class, _) = class_from_window(passive_window)?;
        let (armor_class, _) = class_from_window(armor_window)?;
        if powerup_class == passive_class
            || powerup_class == armor_class
            || passive_class == armor_class
        {
            return Err("attribute registrations must name distinct classes".into());
        }
        let strength = column(&tables, b"Istr")?;
        if !strength.aliases.contains(&powerup_base) {
            return Err("strength metadata does not admit the registered base class".into());
        }
        let agility = column(&tables, b"Iagi")?;
        let intelligence = column(&tables, b"Iint")?;
        let armor = column(&tables, b"Idef")?;
        Ok(Self {
            strength,
            agility,
            intelligence,
            armor,
            origin,
            image_sha256,
            tables,
            overlay,
            powerup_class,
            passive_class,
            armor_class,
        })
    }

    /// Strength granted by an item, `None` when the item is not an automatic powerup.
    /// Agility or intelligence effects refuse until their native paths are qualified.
    pub fn strength_effect(
        &self,
        definition: &Definition,
    ) -> Result<Option<StrengthEffect>, String> {
        let item = self.item_row(definition)?;
        if !item_flag(item, "powerup")? {
            return Ok(None);
        }
        if !item_flag(item, "usable")?
            || !item_flag(item, "perishable")?
            || item_text(item, "uses")? != "1"
        {
            return Err("powerup item flags or uses are unsupported".into());
        }
        let abilities = item_text(item, "abilList")?;
        let [alias] = abilities.split(',').map(str::trim).collect::<Vec<_>>()[..] else {
            return Err("mixed or empty powerup ability list is unsupported".into());
        };
        if self.ability_code(alias)? != self.powerup_class {
            return Err("powerup ability class does not match the client registration".into());
        }
        self.single_level(alias)?;
        self.admit(alias)?;
        let agility = self.amount(alias, &self.agility)?;
        let intelligence = self.amount(alias, &self.intelligence)?;
        if agility != 0 || intelligence != 0 {
            return Err("agility or intelligence powerup effects are unsupported".into());
        }
        let strength = self.amount(alias, &self.strength)?;
        if strength < 0 {
            return Err("negative strength powerup is unsupported".into());
        }
        Ok(Some(StrengthEffect { strength }))
    }

    /// Passive strength, agility and armor of a held item. `None` for powerups and usable items: a
    /// usable item is held without activation, so its abilities add no passive attribute here.
    /// Any passive ability outside the admitted classes, or an intelligence bonus, refuses.
    pub fn held_attributes(
        &self,
        definition: &Definition,
    ) -> Result<Option<AttributeBonus>, String> {
        let item = self.item_row(definition)?;
        if item_flag(item, "powerup")? || item_flag(item, "usable")? {
            return Ok(None);
        }
        let abilities = item_text(item, "abilList")?;
        let mut held = AttributeBonus::default();
        for alias in abilities
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty() && *a != "_")
        {
            let code = self.ability_code(alias)?;
            self.single_level(alias)?;
            if code == self.armor_class {
                if !self.armor.aliases.iter().any(|a| a == alias) {
                    return Err(format!("armor metadata does not admit ability {alias}"));
                }
                let points = self.amount(alias, &self.armor)?;
                if points < 0 {
                    return Err(format!("negative armor on {}", item.rawcode()));
                }
                held.armor = held
                    .armor
                    .checked_add(points)
                    .ok_or("passive armor overflows")?;
                continue;
            }
            if code != self.passive_class {
                return Err(format!(
                    "unsupported passive ability {alias} with code {code} on {}",
                    item.rawcode()
                ));
            }
            self.admit(alias)?;
            let intelligence = self.amount(alias, &self.intelligence)?;
            if intelligence != 0 {
                return Err(format!(
                    "intelligence passive bonus on {} needs the unqualified mana path",
                    item.rawcode()
                ));
            }
            held.strength = held
                .strength
                .checked_add(self.amount(alias, &self.strength)?)
                .ok_or("passive strength overflows")?;
            held.agility = held
                .agility
                .checked_add(self.amount(alias, &self.agility)?)
                .ok_or("passive agility overflows")?;
        }
        Ok(Some(held))
    }

    fn item_row(&self, definition: &Definition) -> Result<Object<'_>, String> {
        let name = std::str::from_utf8(&definition.rawcode).map_err(|_| "invalid item rawcode")?;
        if self.overlay.new_object(name).is_some() {
            return Err("custom item rows are unsupported".into());
        }
        self.tables
            .object(Family::Item, name)
            .map_err(|e| e.to_string())
    }

    fn ability_code(&self, alias: &str) -> Result<String, String> {
        let ability = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?;
        Ok(ability
            .cell(SlkId::AbilityData, "code")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .to_owned())
    }

    /// The effect reads one authored level, so the source row must declare exactly that level.
    fn single_level(&self, alias: &str) -> Result<(), String> {
        let levels = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?
            .cell(SlkId::AbilityData, "levels")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .parse::<u32>();
        if levels != Ok(EFFECT_LEVEL) {
            return Err(format!(
                "multi-level attribute ability {alias} is unsupported"
            ));
        }
        Ok(())
    }

    /// Every attribute column must admit the ability alias through its `useSpecific` list.
    fn admit(&self, alias: &str) -> Result<(), String> {
        for column in [&self.strength, &self.agility, &self.intelligence] {
            if !column.aliases.iter().any(|a| a == alias) {
                return Err(format!("attribute metadata does not admit ability {alias}"));
            }
        }
        Ok(())
    }

    /// Authored amount at the effect level, after the map's W3A override for that field.
    fn amount(&self, alias: &str, column: &Column) -> Result<i32, String> {
        let mods = self
            .overlay
            .new_object(alias)
            .map(|(_, m)| m)
            .unwrap_or_else(|| self.overlay.on(alias));
        if let Some(entry) = mods
            .iter()
            .rev()
            .find(|m| m.field == column.field && m.selectors.is_none_or(|s| s[0] == EFFECT_LEVEL))
        {
            if entry.selectors.is_some_and(|s| s[1] != 0) {
                return Err("indexed attribute override unsupported".into());
            }
            let pcx_objects::Value::Int(n) = entry.value else {
                return Err("noninteger attribute override".into());
            };
            return i32::try_from(n).map_err(|_| "attribute override exceeds integer".into());
        }
        let ability = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?;
        let letter = char::from_u32(u32::from(b'A') + column.data - 1).ok_or("attribute column")?;
        let cell = ability
            .cell(SlkId::AbilityData, &format!("Data{letter}{EFFECT_LEVEL}"))
            .map_err(|e| e.to_string())?;
        match cell.typed(FieldKind::Int).map_err(|e| e.to_string())? {
            Value::Integer(n) => {
                i32::try_from(n).map_err(|_| "attribute value exceeds integer".into())
            }
            _ => Err("attribute field is not integral".into()),
        }
    }
}

fn item_text(item: Object<'_>, key: &str) -> Result<String, String> {
    item.cell(SlkId::ItemData, key)
        .map(|c| c.text.trim().to_owned())
        .map_err(|e| e.to_string())
}

fn item_flag(item: Object<'_>, key: &str) -> Result<bool, String> {
    match item_text(item, key)?.as_str() {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("item {key} is not a boolean flag")),
    }
}

/// Source and base ability classes from a registration call. The callee (`0x7f6360`) reads cdecl
/// stack arguments: first (`[ebp+8]`, last push) hashes into the source-class registry entry,
/// second (`[ebp+0xc]`) into the base entry; the third is an unnamed pointer.
fn class_from_window(window: &[u8]) -> Result<(String, String), String> {
    let fail = || "unsupported class registration window".to_string();
    // Three pushed immediates, one call, stack cleanup and return.
    if window.len() != CLASS_WINDOW_LEN
        || [window[0], window[5], window[10]] != [PUSH_IMM32; 3]
        || window[15] != CALL_REL32
        || window[20..] != CLEANUP_RET
    {
        return Err(fail());
    }
    let key = |at: usize| -> Result<String, String> {
        let mut text: [u8; 4] = window[at..at + 4].try_into().map_err(|_| fail())?;
        if !text.iter().all(u8::is_ascii_alphanumeric) {
            return Err(fail());
        }
        // Immediates are little-endian, so the characters read in reverse.
        text.reverse();
        String::from_utf8(text.to_vec()).map_err(|_| fail())
    };
    Ok((key(11)?, key(6)?))
}

fn column(tables: &DefaultTables, id: &[u8; 4]) -> Result<Column, String> {
    let name = std::str::from_utf8(id).map_err(|_| "invalid attribute metadata id")?;
    let metadata = tables
        .metadata(*id)
        .ok_or_else(|| format!("{name} metadata missing"))?;
    let text = |key: &str| {
        metadata
            .attribute(key)
            .map(|c| c.text.trim().to_owned())
            .ok_or_else(|| format!("{name} metadata {key} missing"))
    };
    if text("field")? != "Data" || text("slk")? != "AbilityData" || text("type")? != "int" {
        return Err(format!("unsupported {name} metadata mapping"));
    }
    let data: u32 = text("data")?
        .parse()
        .map_err(|_| format!("invalid {name} data index"))?;
    if !(1..=DATA_COLUMNS).contains(&data) {
        return Err(format!("{name} data index outside column alphabet"));
    }
    let aliases = text("useSpecific")?
        .split(',')
        .map(|a| a.trim().to_owned())
        .collect();
    Ok(Column {
        field: *id,
        data,
        aliases,
    })
}

#[cfg(test)]
#[path = "inventory_powerup_tests.rs"]
mod tests;
