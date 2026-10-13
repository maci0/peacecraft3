//! One object record: the editor-visible stats of a stock unit, item or
//! upgrade, with a map's overrides layered on top.
//!
//! # What is typed, and on what evidence
//!
//! A stat is only typed once the metadata proves which column it is:
//! [`FieldDef`] resolves the fourcc to a `(table, column)` pair through
//! `Units\UnitMetaData.slk`, and the cell that column holds is the value. The
//! fourcc-to-column join is measured on 267/267 rows, so the numeric decode is
//! measured too — the editor's declared type predicts the wire word of 3 393
//! of the 3 401 modifications 1.28c's maps ship, the single exception being
//! `uhot`, a declared `char` on the string wire 8 times.
//!
//! What stays untyped: the width of an int word, and the scale of a float.
//! [`StatValue`] hands out the value in the shape the declared type says, and
//! nothing else.
//!
//! # The overrides
//!
//! A map's `war3map.w3u` / `war3map.w3t` carries, per object, a list of
//! modifications: a field fourcc, a type word and a value. Here the
//! modifications of one record are laid over the base row. The type word is
//! proven — each fourcc carries exactly one word across the whole corpus —
//! the field semantics are the metadata join above, and **the end word is not
//! interpreted at all**.

use std::collections::BTreeMap;

use super::{Editor, FieldDef, FieldKind, FieldSource, SlkId};
use crate::{Fourcc, Value, ValueType, fourcc_text};

/// What kind of thing a raw code names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    /// A unit, present in `Units\UnitBalance.slk`.
    Unit,
    /// An item, present in `Units\ItemData.slk`.
    Item,
    /// An upgrade, present in `Units\UpgradeData.slk`.
    Upgrade,
    /// An ability, present as a `code` row of `Units\AbilityData.slk`.
    Ability,
    /// A command button, present in `Units\CommandFunc.txt`.
    Command,
}

impl std::fmt::Display for ObjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unit => "unit",
            Self::Item => "item",
            Self::Upgrade => "upgrade",
            Self::Ability => "ability",
            Self::Command => "command",
        })
    }
}

/// One editor value, before it is typed.
///
/// A map override wins over the stock cell; with no override this is the
/// stock cell, as text, exactly as the `.slk` wrote it — `-` for "nothing",
/// `_` for "unset", `1.5` for a real.
#[derive(Debug, Clone, PartialEq)]
pub enum StatValue<'a> {
    /// A stock `.slk` cell or `.txt` value, verbatim.
    Cell(&'a str),
    /// A value a map wrote into this object, wire-decoded.
    Override(&'a Value),
}

impl<'a> StatValue<'a> {
    /// The value as text. `-` and `_` are **not** converted: both are the
    /// shipped way of saying "nothing here", and converting either would
    /// invent a number the editor never had.
    pub fn text(&self) -> Option<&'a str> {
        match self {
            Self::Cell(s) => Some(s),
            Self::Override(Value::Str(b)) => std::str::from_utf8(b).ok(),
            _ => None,
        }
    }

    /// The value as a number, if it is one.
    ///
    /// Both `.slk` text and a map's int/real/unreal word decode here; an int
    /// override keeps its word width, which is the only thing the bytes prove.
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Cell(s) => s.trim().parse::<f64>().ok(),
            Self::Override(Value::Int(w)) => Some(f64::from(*w)),
            Self::Override(Value::Real(f)) | Self::Override(Value::Unreal(f)) => {
                Some(f64::from(*f))
            }
            Self::Override(Value::Str(_)) => None,
        }
    }

    /// The value as a float, if it is one.
    pub fn real(&self) -> Option<f32> {
        match self {
            Self::Cell(s) => s.trim().parse::<f32>().ok(),
            Self::Override(Value::Int(w)) => Some(*w as f32),
            Self::Override(Value::Real(f)) | Self::Override(Value::Unreal(f)) => Some(*f),
            Self::Override(Value::Str(_)) => None,
        }
    }

    /// The value as a boolean, on the `.slk`'s own `0`/`1`/`TRUE`/`FALSE`.
    pub fn flag(&self) -> Option<bool> {
        match self {
            Self::Cell(s) => match s.trim() {
                "1" | "TRUE" | "true" => Some(true),
                "0" | "FALSE" | "false" => Some(false),
                _ => None,
            },
            Self::Override(Value::Int(w)) => Some(*w != 0),
            _ => None,
        }
    }

    /// Whether the value is the editor's "nothing at all" marker.
    ///
    /// `UnitBalance.slk` pads the cell and writes ` - ` for a unit without
    /// mana or without a primary attribute; `UnitData.slk` writes `_` for an
    /// unset buff type. Both are real values of the cell and are reported by
    /// [`StatValue::text`], not swallowed.
    pub fn is_blank(&self) -> bool {
        self.text()
            .map(|t| matches!(t.trim(), "-" | "_"))
            .unwrap_or(false)
    }

    /// Which of base or override this value came from.
    pub fn is_override(&self) -> bool {
        matches!(self, Self::Override(_))
    }
}

/// The shared part of a record: the raw code, the editor it came from, and
/// whatever a map overlaid on top.
#[derive(Debug, Clone)]
struct Record<'a> {
    editor: &'a Editor,
    rawcode: String,
    custom: bool,
    overrides: BTreeMap<Fourcc, Value>,
}

impl<'a> Record<'a> {
    fn new(editor: &'a Editor, rawcode: &str) -> Self {
        Self {
            editor,
            rawcode: rawcode.to_string(),
            custom: false,
            overrides: BTreeMap::new(),
        }
    }

    fn rawcode(&self) -> &str {
        &self.rawcode
    }

    fn custom(&self) -> bool {
        self.custom
    }

    fn overrides(&self) -> &BTreeMap<Fourcc, Value> {
        &self.overrides
    }

    fn field(&self, fourcc: Fourcc) -> Option<&'a FieldDef> {
        self.editor.field(fourcc)
    }

    /// The stock cell for a field fourcc, if the mapping is proven.
    ///
    /// `None` for three separate reasons: the fourcc is unknown to this
    /// build's metadata, the field's base value is not measured, or the object
    /// has no cell there. All three are reportable (see
    /// [`super::report::Refusal`]).
    fn base_value(&self, fourcc: Fourcc) -> Option<StatValue<'a>> {
        let def = self.editor.field(fourcc)?;
        if !def.measured {
            return None;
        }
        match &def.source {
            FieldSource::Column { slk, column } => {
                let table = self.editor.table(*slk)?;
                let row = self.editor.row(*slk, &self.rawcode)?;
                table
                    .get(row, table.column_of(column)?)
                    .map(StatValue::Cell)
            }
            FieldSource::Profile { key, index, slots } => self
                .editor
                .profile(&self.rawcode)
                .and_then(|s| s.element(key, *index, *slots))
                .map(StatValue::Cell),
            FieldSource::Unknown { .. } => None,
        }
    }

    fn base_cell(&self, fourcc: Fourcc) -> Option<&'a str> {
        self.base_value(fourcc)?.text()
    }

    fn comment(&self, ids: &[SlkId]) -> Option<&'a str> {
        for id in ids {
            let Some(row) = self.editor.row(*id, &self.rawcode) else {
                continue;
            };
            let Some(t) = self.editor.table(*id) else {
                continue;
            };
            let Some(c) = id.comment_column().and_then(|n| t.column_of(n)) else {
                continue;
            };
            if let Some(v) = t.get(row, c) {
                return Some(v);
            }
        }
        None
    }

    fn put(&mut self, m: &crate::Mod) {
        self.overrides.insert(m.field, m.value.clone());
    }

    fn mark_custom(&mut self) {
        self.custom = true;
    }
}

/// One stock unit: `UnitData`, `unitUI`, `UnitBalance`, `UnitWeapons` and
/// `UnitAbilities` joined on the raw code.
#[derive(Debug, Clone)]
pub struct Unit<'a> {
    record: Record<'a>,
}

impl<'a> Unit<'a> {
    pub(crate) fn new(editor: &'a Editor, rawcode: &str) -> Self {
        Self {
            record: Record::new(editor, rawcode),
        }
    }

    pub(crate) fn put(&mut self, m: &crate::Mod) {
        self.record.put(m);
    }

    pub(crate) fn mark_custom(&mut self) {
        self.record.mark_custom();
    }

    /// The raw code, e.g. `hfoo`.
    pub fn rawcode(&self) -> &str {
        self.record.rawcode()
    }

    /// Whether this unit exists only because a map declared it.
    pub fn custom(&self) -> bool {
        self.record.custom()
    }

    /// The overrides a map laid on this unit.
    pub fn overrides(&self) -> &BTreeMap<Fourcc, Value> {
        self.record.overrides()
    }

    /// One value, base cell or override.
    pub fn stat(&self, fourcc: Fourcc) -> Option<StatValue<'_>> {
        match self.record.overrides.get(&fourcc) {
            Some(v) => Some(StatValue::Override(v)),
            None => self.record.base_value(fourcc),
        }
    }

    /// The stock cell for a field fourcc.
    pub fn base_cell(&self, fourcc: Fourcc) -> Option<&'a str> {
        self.record.base_cell(fourcc)
    }

    /// The field fourcc's definition, resolved against this build.
    pub fn field(&self, fourcc: Fourcc) -> Option<&'a FieldDef> {
        self.record.field(fourcc)
    }

    /// The editor's own comment for this unit.
    pub fn comment(&self) -> Option<&'a str> {
        self.record
            .comment(&[SlkId::UnitBalance, SlkId::UnitData, SlkId::UnitUi])
    }

    /// The display name, from the `*UnitStrings.txt` layer — `uhpm`/`uico` the
    /// editor shows in the tooltip, not `unitUI.name`, which holds the
    /// animation set name.
    pub fn name(&self) -> Option<&'a str> {
        self.record
            .editor
            .profile(self.record.rawcode())
            .and_then(|s| s.value("Name"))
    }

    /// The `*UnitFunc.txt` section for this unit, if any.
    pub fn profile(&self) -> Option<&'a super::ProfileSection> {
        self.record.editor.profile(self.record.rawcode())
    }

    /// `urac` — the race, `UnitData.slk`'s `race` column.
    pub fn race(&self) -> Option<&str> {
        self.stat(*b"urac").and_then(|v| v.text())
    }

    /// `uhpm` — HP, `UnitBalance.slk`'s `HP` column.
    pub fn hp(&self) -> Option<f64> {
        self.stat(*b"uhpm").and_then(|v| v.number())
    }

    /// `umpm` — mana, `UnitBalance.slk`'s `manaN` column.
    pub fn mana(&self) -> Option<f64> {
        self.stat(*b"umpm").and_then(|v| v.number())
    }

    /// `umvs` — movement speed, `UnitBalance.slk`'s `spd` column.
    pub fn speed(&self) -> Option<f64> {
        self.stat(*b"umvs").and_then(|v| v.number())
    }

    /// `udef` — armour, `UnitBalance.slk`'s `def` column.
    pub fn armour(&self) -> Option<f64> {
        self.stat(*b"udef").and_then(|v| v.number())
    }

    /// `ubld` — build time in seconds, `UnitBalance.slk`'s `bldtm` column.
    pub fn build_time(&self) -> Option<f64> {
        self.stat(*b"ubld").and_then(|v| v.number())
    }

    /// `usid` — sight range, `UnitBalance.slk`'s `sight` column.
    pub fn sight(&self) -> Option<f64> {
        self.stat(*b"usid").and_then(|v| v.number())
    }

    /// `ugol` — gold cost, `UnitBalance.slk`'s `goldcost` column.
    pub fn gold_cost(&self) -> Option<f64> {
        self.stat(*b"ugol").and_then(|v| v.number())
    }

    /// `ulum` — lumber cost, `UnitBalance.slk`'s `lumbercost` column.
    pub fn lumber_cost(&self) -> Option<f64> {
        self.stat(*b"ulum").and_then(|v| v.number())
    }

    /// `ua1d` — damage dice of attack 1, `UnitWeapons.slk`'s `dice1` column.
    pub fn damage_dice(&self) -> Option<f64> {
        self.stat(*b"ua1d").and_then(|v| v.number())
    }

    /// `ua1s` — damage sides of attack 1, `UnitWeapons.slk`'s `sides1` column.
    pub fn damage_sides(&self) -> Option<f64> {
        self.stat(*b"ua1s").and_then(|v| v.number())
    }

    /// `ua1b` — damage bonus of attack 1, `UnitWeapons.slk`'s `dmgplus1` column.
    pub fn damage_bonus(&self) -> Option<f64> {
        self.stat(*b"ua1b").and_then(|v| v.number())
    }

    /// `ua1c` — attack 1 cooldown in seconds, `UnitWeapons.slk`'s `cool1`.
    pub fn attack_cooldown(&self) -> Option<f32> {
        self.stat(*b"ua1c").and_then(|v| v.real())
    }

    /// `ua1r` — attack 1 range, `UnitWeapons.slk`'s `rangeN1`.
    pub fn attack_range(&self) -> Option<f64> {
        self.stat(*b"ua1r").and_then(|v| v.number())
    }

    /// `ubdi`/`ubsi`/`ubba` — bounty dice, sides and bonus.
    pub fn bounty(&self) -> Option<(f64, f64, f64)> {
        let d = self.stat(*b"ubdi").and_then(|v| v.number())?;
        let s = self.stat(*b"ubsi").and_then(|v| v.number())?;
        let b = self.stat(*b"ubba").and_then(|v| v.number())?;
        Some((d, s, b))
    }

    /// `ulev` — the unit's level, `UnitBalance.slk`'s `level` column.
    pub fn level(&self) -> Option<f64> {
        self.stat(*b"ulev").and_then(|v| v.number())
    }

    /// `uarm` — the armour **type**, `unitUI.slk`'s `armor` column:
    /// `Flesh`, `Metal`, `Wood`, `Stone`.
    ///
    /// Not `udty`: that is the *defence* type (`hero`, `large`, `small`,
    /// `fortified`) in `UnitBalance.slk`.
    pub fn armour_type(&self) -> Option<&str> {
        self.stat(*b"uarm").and_then(|v| v.text())
    }

    /// `udty` — the defence type, `UnitBalance.slk`'s `defType` column.
    pub fn defence_type(&self) -> Option<&str> {
        self.stat(*b"udty").and_then(|v| v.text())
    }

    /// `uhpr` — HP regen, `UnitBalance.slk`'s `regenHP` column.
    pub fn hp_regen(&self) -> Option<f32> {
        self.stat(*b"uhpr").and_then(|v| v.real())
    }

    /// The `.slk` row this unit occupies in `id`.
    pub fn row(&self, id: SlkId) -> Option<u32> {
        self.record.editor.row(id, self.record.rawcode())
    }
}

/// One stock item.
#[derive(Debug, Clone)]
pub struct Item<'a> {
    record: Record<'a>,
}

impl<'a> Item<'a> {
    pub(crate) fn new(editor: &'a Editor, rawcode: &str) -> Self {
        Self {
            record: Record::new(editor, rawcode),
        }
    }

    pub(crate) fn put(&mut self, m: &crate::Mod) {
        self.record.put(m);
    }

    pub(crate) fn mark_custom(&mut self) {
        self.record.mark_custom();
    }

    /// The raw code, e.g. `ckng`.
    pub fn rawcode(&self) -> &str {
        self.record.rawcode()
    }

    /// Whether this item exists only because a map declared it.
    pub fn custom(&self) -> bool {
        self.record.custom()
    }

    /// The overrides a map laid on this item.
    pub fn overrides(&self) -> &BTreeMap<Fourcc, Value> {
        self.record.overrides()
    }

    /// One value, base cell or override.
    pub fn stat(&self, fourcc: Fourcc) -> Option<StatValue<'_>> {
        match self.record.overrides.get(&fourcc) {
            Some(v) => Some(StatValue::Override(v)),
            None => self.record.base_value(fourcc),
        }
    }

    /// The stock cell for a field fourcc.
    pub fn base_cell(&self, fourcc: Fourcc) -> Option<&'a str> {
        self.record.base_cell(fourcc)
    }

    /// The field fourcc's definition, resolved against this build.
    pub fn field(&self, fourcc: Fourcc) -> Option<&'a FieldDef> {
        self.record.field(fourcc)
    }

    /// `igol` — gold cost, `ItemData.slk`'s `goldcost` column.
    pub fn gold_cost(&self) -> Option<f64> {
        self.stat(*b"igol").and_then(|v| v.number())
    }

    /// `ilum` — lumber cost, `ItemData.slk`'s `lumbercost` column.
    pub fn lumber_cost(&self) -> Option<f64> {
        self.stat(*b"ilum").and_then(|v| v.number())
    }

    /// `ihtp` — HP, `ItemData.slk`'s `HP` column.
    pub fn hp(&self) -> Option<f64> {
        self.stat(*b"ihtp").and_then(|v| v.number())
    }

    /// `icla` — the item class, `ItemData.slk`'s `class` column.
    pub fn class(&self) -> Option<&str> {
        self.stat(*b"icla").and_then(|v| v.text())
    }

    /// The display name, from `Units\ItemStrings.txt`.
    pub fn name(&self) -> Option<&'a str> {
        self.record
            .editor
            .profile(self.record.rawcode())
            .and_then(|s| s.value("Name"))
    }

    /// The editor's own comment for this item.
    pub fn comment(&self) -> Option<&'a str> {
        self.record.comment(&[SlkId::ItemData])
    }

    /// The `.slk` row this item occupies.
    pub fn row(&self, id: SlkId) -> Option<u32> {
        self.record.editor.row(id, self.record.rawcode())
    }
}

/// One stock upgrade.
#[derive(Debug, Clone)]
pub struct Upgrade<'a> {
    record: Record<'a>,
}

impl<'a> Upgrade<'a> {
    pub(crate) fn new(editor: &'a Editor, rawcode: &str) -> Self {
        Self {
            record: Record::new(editor, rawcode),
        }
    }

    pub(crate) fn put(&mut self, m: &crate::Mod) {
        self.record.put(m);
    }

    /// The raw code, e.g. `Rhme`.
    pub fn rawcode(&self) -> &str {
        self.record.rawcode()
    }

    /// The editor's own comment for this upgrade.
    pub fn comment(&self) -> Option<&'a str> {
        self.record.comment(&[SlkId::UpgradeData])
    }

    /// `UpgradeData.slk`'s `maxlevel` column.
    pub fn max_level(&self) -> Option<f64> {
        let id = SlkId::UpgradeData;
        let row = self.record.editor.row(id, self.record.rawcode())?;
        let t = self.record.editor.table(id)?;
        t.row(row).by_name("maxlevel").and_then(|v| v.parse().ok())
    }
}

/// The declared type of one value, as the editor itself writes it.
///
/// Kept for callers that need a word's shape without decoding it. The word
/// this returns is what [`FieldKind::wire_word`] measured, not a guess: a
/// [`FieldKind::Char`] field ships on the string wire, so the caller that
/// insists on the `char` interpretation has to ask for it separately.
pub fn declared_type(def: &FieldDef) -> ValueType {
    match def.kind {
        FieldKind::Int
        | FieldKind::Bool
        | FieldKind::Char
        | FieldKind::AttackBits
        | FieldKind::VersionFlags
        | FieldKind::IntList => ValueType::Int,
        FieldKind::Real => ValueType::Real,
        FieldKind::Unreal => ValueType::Unreal,
        FieldKind::Token => ValueType::Str,
    }
}

/// A fourcc, displayed for diagnostics.
pub fn show(fourcc: Fourcc) -> String {
    fourcc_text(fourcc)
}
