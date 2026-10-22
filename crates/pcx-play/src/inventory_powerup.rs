//! Source-derived hero attribute abilities: automatic powerups (permanent strength on use) and
//! passive held-item bonuses (strength, agility, armor and flat attack damage while held). Both
//! read ability classes from the reviewed client's registration operands and amounts from
//! selected ability metadata with W3A overlays.
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
/// Registration call for the passive flat attack-damage class (source `AIat`, base `APbo`).
const DAMAGE_WINDOW: u32 = 0xbc9c80;
/// Registration call for the life-regeneration class (source `Arel`, base `AREB`).
const LIFE_REGEN_WINDOW: u32 = 0xbca040;
/// Registration call for the mana-regeneration class (source `AIrm`, base `Arem`).
const MANA_REGEN_WINDOW: u32 = 0xbc9ec0;
/// Registration call for the healing class (source `AIhe`). Its base comes from a call, so the
/// window is longer than the immediate-base shape. Reviewed at `0xbf0520`.
pub(crate) const HEAL_WINDOW: u32 = 0xbf0520;
const HEAL_WINDOW_LEN: usize = 25;
const BASE_FN_LEN: usize = 6;
pub(crate) const REGISTER_FN: u32 = 0x7f6360;
const CLASS_WINDOW_LEN: usize = 24;
pub(crate) const PUSH_IMM32: u8 = 0x68;
pub(crate) const PUSH_EAX: u8 = 0x50;
pub(crate) const CALL_REL32: u8 = 0xe8;
pub(crate) const MOV_EAX_IMM32: u8 = 0xb8;
pub(crate) const RET: u8 = 0xc3;
pub(crate) const CLEANUP_RET: [u8; 4] = [0x83, 0xc4, 0x0c, RET];
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

/// Flat attributes, armor, attack damage and regeneration from passive held items.
/// Unit strength also includes retained powerups. Armor points are added to unit armor
/// while held; damage is a flat addition to the unit's attack damage.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AttributeBonus {
    pub strength: i32,
    pub agility: i32,
    pub armor: i32,
    pub damage: i32,
    /// Life points per game second while held, separate from base regeneration eligibility.
    pub life_regen: i32,
    /// Fraction of the unit's normal mana regeneration added while held.
    pub mana_regen_fraction: f32,
}

#[derive(Clone, Debug)]
struct Column {
    field: [u8; 4],
    data: u32,
    aliases: Vec<String>,
}

/// A real-valued metadata column, such as the mana-regeneration fraction.
#[derive(Clone, Debug)]
struct FractionColumn {
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
    damage_class: String,
    strength: Column,
    agility: Column,
    intelligence: Column,
    armor: Column,
    damage: Column,
    /// Healing registration, attached by `with_heal`. Absent until the healing window is read.
    heal: Option<AmountClass>,
    life_regen: Option<AmountClass>,
    mana_regen: Option<FractionClass>,
}

/// One admitted scalar ability class and its metadata-selected amount column.
#[derive(Clone, Debug)]
struct AmountClass {
    class: String,
    amount: Column,
}

/// One admitted real-valued ability class and its metadata-selected amount column.
#[derive(Clone, Debug)]
struct FractionClass {
    class: String,
    amount: FractionColumn,
}

impl Rules {
    /// Reads registration windows from the reviewed client image and selected metadata.
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
        let damage = window(DAMAGE_WINDOW)?;
        let life_regen = window(LIFE_REGEN_WINDOW)?;
        let mana_regen = window(MANA_REGEN_WINDOW)?;
        let heal = pe
            .read_from(HEAL_WINDOW, true)
            .map_err(|e| fail(&e.to_string()))?
            .get(..HEAL_WINDOW_LEN)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| fail("truncated healing registration window"))?;
        let base_va = call_target(&heal, 5).map_err(|e| fail(&e))?;
        let base_code = pe
            .read_from(base_va, true)
            .map_err(|e| fail(&e.to_string()))?
            .get(..BASE_FN_LEN)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| fail("truncated healing base function"))?;
        Self::decode(
            tables,
            overlay,
            origin,
            digest,
            [&powerup, &passive, &armor, &damage],
        )
        .and_then(|rules| rules.with_life_regen(&life_regen))
        .and_then(|rules| rules.with_mana_regen(&mana_regen))
        .and_then(|rules| rules.with_heal(&heal, &base_code))
        .map_err(|e| fail(&e))
    }

    /// Attaches the healing class. The class key is an immediate in the window; the base comes
    /// from the reviewed base function, which the caller reads from the image.
    pub(crate) fn with_heal(mut self, window: &[u8], base_code: &[u8]) -> Result<Self, String> {
        let (class, _) = class_from_call_window(window, base_code)?;
        if class == self.powerup_class
            || class == self.passive_class
            || class == self.armor_class
            || class == self.damage_class
            || self
                .life_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self
                .mana_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
        {
            return Err("healing registration must name a distinct class".into());
        }
        let amount = column(&self.tables, b"Ihpg")?;
        self.heal = Some(AmountClass { class, amount });
        Ok(self)
    }

    pub(crate) fn with_life_regen(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, _) = class_from_window(window)?;
        if self.life_regen.is_some()
            || [
                &self.powerup_class,
                &self.passive_class,
                &self.armor_class,
                &self.damage_class,
            ]
            .contains(&&class)
            || self.heal.as_ref().is_some_and(|heal| heal.class == class)
            || self
                .mana_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
        {
            return Err("life regeneration registration must name a distinct class".into());
        }
        let amount = column(&self.tables, b"Ihpr")?;
        self.life_regen = Some(AmountClass { class, amount });
        Ok(self)
    }

    /// Attaches the real-valued mana-regeneration class.
    pub(crate) fn with_mana_regen(mut self, window: &[u8]) -> Result<Self, String> {
        let (class, _) = class_from_window(window)?;
        if self.mana_regen.is_some()
            || [
                &self.powerup_class,
                &self.passive_class,
                &self.armor_class,
                &self.damage_class,
            ]
            .contains(&&class)
            || self
                .life_regen
                .as_ref()
                .is_some_and(|regen| regen.class == class)
            || self.heal.as_ref().is_some_and(|heal| heal.class == class)
        {
            return Err("mana regeneration registration must name a distinct class".into());
        }
        let amount = fraction_column(&self.tables, b"Imrp")?;
        self.mana_regen = Some(FractionClass { class, amount });
        Ok(self)
    }

    /// Healing amount (`DataA1` at level 1 through the `Ihpg` admission) of an item whose ability
    /// list names the healing class. `None` for items with no abilities or no healing ability.
    /// Mixed lists and healing that is not admitted refuse.
    pub fn heal_effect(&self, definition: &Definition) -> Result<Option<i32>, String> {
        let item = self.item_row(definition)?;
        let abilities = item_text(item, "abilList")?;
        let aliases: Vec<&str> = abilities
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty() && *a != "_")
            .collect();
        // A passive or unusable item has no activation, so it is not a use: no effect, no refusal.
        if aliases.is_empty() || !item_flag(item, "usable")? {
            return Ok(None);
        }
        let Some(heal) = &self.heal else {
            return Err("healing registration is not admitted".into());
        };
        let mut healing = Vec::new();
        for alias in &aliases {
            if self.ability_code(alias)? == heal.class {
                healing.push(*alias);
            }
        }
        match healing[..] {
            [] => Ok(None),
            [alias] if aliases.len() == 1 => {
                self.single_level(alias)?;
                if !heal.amount.aliases.iter().any(|a| a == alias) {
                    return Err(format!("healing metadata does not admit ability {alias}"));
                }
                let amount = self.amount(alias, &heal.amount)?;
                if amount < 0 {
                    return Err(format!("negative healing amount on {}", item.rawcode()));
                }
                Ok(Some(amount))
            }
            _ => Err(format!(
                "mixed healing ability list on {} is unsupported",
                item.rawcode()
            )),
        }
    }

    /// Whether a healing item's single ability needs a selected target: its authored `targs1` is
    /// not `_`. A missing `targs1` cell refuses, since the selection convention is not guessed.
    pub fn heal_requires_target(&self, definition: &Definition) -> Result<bool, String> {
        let item = self.item_row(definition)?;
        let abilities = item_text(item, "abilList")?;
        let [alias] = abilities.split(',').map(str::trim).collect::<Vec<_>>()[..] else {
            return Err("healing target needs exactly one ability".into());
        };
        let targets = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?
            .cell(SlkId::AbilityData, "targs1")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .to_owned();
        Ok(targets != "_")
    }

    /// Cast range (`Rng1`, level 1) of an item's single ability, from the source row. Overrides
    /// of the range field are not applied here, and that is not qualified.
    pub fn heal_range(&self, definition: &Definition) -> Result<f32, String> {
        let item = self.item_row(definition)?;
        let abilities = item_text(item, "abilList")?;
        let [alias] = abilities.split(',').map(str::trim).collect::<Vec<_>>()[..] else {
            return Err("healing range needs exactly one ability".into());
        };
        let text = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?
            .cell(SlkId::AbilityData, "Rng1")
            .map_err(|e| e.to_string())?
            .text
            .trim()
            .to_owned();
        text.parse::<f32>()
            .ok()
            .filter(|r| r.is_finite() && *r >= 0.0)
            .ok_or_else(|| format!("range {text} of {alias} is not a finite distance"))
    }

    pub(crate) fn decode(
        tables: Arc<DefaultTables>,
        overlay: Overlay,
        origin: AssetIdentity,
        image_sha256: Sha256Digest,
        windows: [&[u8]; 4],
    ) -> Result<Self, String> {
        let [powerup_window, passive_window, armor_window, damage_window] = windows;
        let (powerup_class, powerup_base) = class_from_window(powerup_window)?;
        // The passive, armor and damage bases (`APbo`) are native classes with no ability row, so
        // they are not checked against metadata.
        let (passive_class, _) = class_from_window(passive_window)?;
        let (armor_class, _) = class_from_window(armor_window)?;
        let (damage_class, _) = class_from_window(damage_window)?;
        if powerup_class == passive_class
            || powerup_class == armor_class
            || powerup_class == damage_class
            || passive_class == armor_class
            || passive_class == damage_class
            || armor_class == damage_class
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
        let damage = column(&tables, b"Iatt")?;
        Ok(Self {
            strength,
            agility,
            intelligence,
            armor,
            damage,
            origin,
            image_sha256,
            tables,
            overlay,
            powerup_class,
            passive_class,
            armor_class,
            damage_class,
            heal: None,
            life_regen: None,
            mana_regen: None,
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

    /// Passive attributes, armor, attack damage and regeneration. `None` for powerups
    /// and usable items: a usable item is held without activation, so its abilities add no passive
    /// attribute here. Any passive ability outside the admitted classes, or an intelligence
    /// bonus, refuses.
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
            if code == self.damage_class {
                if !self.damage.aliases.iter().any(|a| a == alias) {
                    return Err(format!("damage metadata does not admit ability {alias}"));
                }
                let points = self.amount(alias, &self.damage)?;
                if points < 0 {
                    return Err(format!("negative damage on {}", item.rawcode()));
                }
                held.damage = held
                    .damage
                    .checked_add(points)
                    .ok_or("passive damage overflows")?;
                continue;
            }
            if let Some(regen) = &self.life_regen
                && code == regen.class
            {
                if !regen.amount.aliases.iter().any(|a| a == alias) {
                    return Err(format!(
                        "life regeneration metadata does not admit ability {alias}"
                    ));
                }
                let points = self.amount(alias, &regen.amount)?;
                if points < 0 {
                    return Err(format!("negative life regeneration on {}", item.rawcode()));
                }
                held.life_regen = held
                    .life_regen
                    .checked_add(points)
                    .ok_or("passive life regeneration overflows")?;
                continue;
            }
            if let Some(regen) = &self.mana_regen
                && code == regen.class
            {
                if !regen.amount.aliases.iter().any(|a| a == alias) {
                    return Err(format!(
                        "mana regeneration metadata does not admit ability {alias}"
                    ));
                }
                let fraction = self.fraction(alias, &regen.amount)?;
                if fraction < 0.0 {
                    return Err(format!("negative mana regeneration on {}", item.rawcode()));
                }
                let sum = held.mana_regen_fraction + fraction;
                if !sum.is_finite() {
                    return Err("passive mana regeneration overflows".into());
                }
                held.mana_regen_fraction = sum;
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

    /// Authored real fraction at the effect level, after the map's W3A override for that field.
    fn fraction(&self, alias: &str, column: &FractionColumn) -> Result<f32, String> {
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
                return Err("indexed mana regeneration override unsupported".into());
            }
            return match entry.value {
                pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value)
                    if value.is_finite() =>
                {
                    Ok(value)
                }
                _ => Err("nonreal mana regeneration override".into()),
            };
        }
        let ability = self
            .tables
            .object(Family::Ability, alias)
            .map_err(|e| e.to_string())?;
        let letter =
            char::from_u32(u32::from(b'A') + column.data - 1).ok_or("mana regeneration column")?;
        let cell = ability
            .cell(SlkId::AbilityData, &format!("Data{letter}{EFFECT_LEVEL}"))
            .map_err(|e| e.to_string())?;
        match cell.typed(FieldKind::Real).map_err(|e| e.to_string())? {
            Value::Real(value) if value.is_finite() => Ok(value as f32),
            _ => Err("mana regeneration field is not real".into()),
        }
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

/// Target of the `call rel32` at `at` inside a healing window, given the window's own address.
fn call_target(window: &[u8], at: usize) -> Result<u32, String> {
    let fail = || "unsupported healing registration call".to_string();
    if window.get(at) != Some(&CALL_REL32) {
        return Err(fail());
    }
    let rel: [u8; 4] = window
        .get(at + 1..at + 5)
        .ok_or_else(fail)?
        .try_into()
        .map_err(|_| fail())?;
    let next = i64::from(HEAL_WINDOW) + at as i64 + 5;
    u32::try_from(next + i64::from(i32::from_le_bytes(rel))).map_err(|_| fail())
}

/// Source class from a healing registration window with a computed base. The window pushes the
/// third argument, calls the base function, pushes its result, pushes the source class, calls the
/// registration function (`0x7f6360`), cleans up and returns. The base function must be
/// `mov eax, imm32; ret`, and its immediate is the base class.
fn class_from_call_window(window: &[u8], base_code: &[u8]) -> Result<(String, String), String> {
    let fail = || "unsupported healing registration window".to_string();
    if window.len() != HEAL_WINDOW_LEN
        || window[0] != PUSH_IMM32
        || window[5] != CALL_REL32
        || window[10] != PUSH_EAX
        || window[11] != PUSH_IMM32
        || window[21..] != CLEANUP_RET
        || call_target(window, 16)? != REGISTER_FN
        || base_code.len() != BASE_FN_LEN
        || base_code[0] != MOV_EAX_IMM32
        || base_code[5] != RET
    {
        return Err(fail());
    }
    Ok((fourcc_le(&window[12..16])?, fourcc_le(&base_code[1..5])?))
}

/// Four alphanumeric characters from a little-endian immediate.
fn fourcc_le(bytes: &[u8]) -> Result<String, String> {
    let fail = || "unsupported registration immediate".to_string();
    let mut text: [u8; 4] = bytes.try_into().map_err(|_| fail())?;
    if !text.iter().all(u8::is_ascii_alphanumeric) {
        return Err(fail());
    }
    text.reverse();
    String::from_utf8(text.to_vec()).map_err(|_| fail())
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

/// A real-valued metadata column: the same `Data` mapping as [`column`], with a real or unreal
/// authored type instead of an integer type.
fn fraction_column(tables: &DefaultTables, id: &[u8; 4]) -> Result<FractionColumn, String> {
    let name = std::str::from_utf8(id).map_err(|_| "invalid fraction metadata id")?;
    let metadata = tables
        .metadata(*id)
        .ok_or_else(|| format!("{name} metadata missing"))?;
    let text = |key: &str| {
        metadata
            .attribute(key)
            .map(|c| c.text.trim().to_owned())
            .ok_or_else(|| format!("{name} metadata {key} missing"))
    };
    if text("field")? != "Data"
        || text("slk")? != "AbilityData"
        || !matches!(text("type")?.as_str(), "real" | "unreal")
    {
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
    Ok(FractionColumn {
        field: *id,
        data,
        aliases,
    })
}

#[cfg(test)]
#[path = "inventory_powerup_tests.rs"]
mod tests;
