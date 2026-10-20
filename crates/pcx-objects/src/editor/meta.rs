//! The field schema: which fourcc lands in which column, and what the editor
//! itself says it is.
//!
//! `Units\UnitMetaData.slk` (and the five other `*MetaData.slk`) is the object
//! editor's own machine-readable schema. One row per field fourcc, carrying:
//!
//! | column | meaning |
//! |--------|---------|
//! | `ID`   | the fourcc, e.g. `uhpm` |
//! | `field`| the column name in the table named by `slk`, e.g. `HP` |
//! | `slk`  | which table: `UnitBalance`, `UnitData`, `profile`, `ItemData`, … |
//! | `index`| the comma slot inside a `Profile` value; retained verbatim for SLK rows |
//! | `type` | the editor's declared type: `int`, `bool`, `real`, `unreal`, `unitList`, … |
//! | `category`, `changeFlags`, `minVal`, `maxVal`, `useHero`, … | editor-only |
//!
//! The join this crate relies on is `ID -> (slk, field)` and it is **measured**:
//! all 267 rows of `UnitMetaData.slk` resolve to a row-1 column of the shipped
//! table, or to a key a shipped `.txt` section carries. What is *not*
//! measured by the join, and is therefore refused here, is the numeric width
//! and the scale of every field — see [`FieldKind`].

use pcx_slq::Table;

use super::SlkId;

/// The metadata tables, in the order their rows are read.
///
/// Measured: these six hold **1 357 distinct fourccs and share none between
/// them**, so a fourcc never needs disambiguating between families.
pub const METADATA_SLK: &[SlkId] = &[
    SlkId::UnitMetaData,
    SlkId::AbilityMetaData,
    SlkId::UpgradeMetaData,
    SlkId::UpgradeEffectMetaData,
    SlkId::DestructableMetaData,
    SlkId::MiscMetaData,
    SlkId::AbilityBuffMetaData,
    SlkId::DoodadMetaData,
];

/// Where a field's base value lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldSource {
    /// A named column of one decoded `.slk`.
    Column {
        /// Which table.
        slk: SlkId,
        /// The row-1 header the field names in that table.
        column: String,
    },
    /// A key inside a `[rawcode]` section of the `.txt` layer.
    Profile {
        /// The `key=value` name, lower-cased.
        key: String,
        /// The slot the field names: `-1` for a whole-value field.
        index: i32,
        /// How many slots the key has, measured from the metadata rows.
        slots: u32,
    },
    /// The metadata names a table this crate does not decode — `DoodadData`,
    /// or an empty `slk` on a row that has none. The row is kept, and
    /// [`FieldDef::measured`] refuses to answer for it.
    Unknown {
        /// The `slk` column's value, verbatim.
        slk: String,
        /// The `field` column's value, verbatim.
        field: String,
    },
}

/// The editor's own declared type for one field.
///
/// These are the strings the `type` column of a `*MetaData.slk` actually
/// carries — 45 distinct values across 1 357 rows, not a family imposed from
/// outside. Only the numeric/word shape is interpreted; what each means
/// semantically (which resistance type a `defenseType` string names, which
/// icon an `icon` path points at) stays text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FieldKind {
    /// `int` — a whole number. 83 unit rows.
    Int,
    /// `bool` — 0/1. 39 unit rows.
    Bool,
    /// `char` — a single character. 1 unit row, `uhot` `Hotkey`, which ships
    /// on the **string** wire, not the char wire: see [`FieldKind::wire_word`].
    Char,
    /// `real` — a float. 16 unit rows.
    Real,
    /// `unreal` — a float in a different slot. 43 unit rows.
    Unreal,
    /// `attackBits` — a weapon-enable bit mask. 1 unit row, `uaen`.
    ///
    /// Word 0 like [`FieldKind::Int`], but a mask, not a count.
    AttackBits,
    /// `versionFlags` — an art-version bit mask. 1 unit row, `uver`.
    VersionFlags,
    /// `intList` — a list of whole numbers. 1 unit row, `urqa`.
    IntList,
    /// Everything else: a token, an enum or a list of them, which the editor
    /// writes as text and the container as a string.
    ///
    /// `unitList`, `abilityList`, `targetList`, `model`, `icon`, `soundLabel`,
    /// `attackType`, `combatSound`, `pathingTexture`, `unitClass`, … `tileset`,
    /// `string`, `stringList`, … — 1 176 of the 1 357 rows.
    Token,
}

impl FieldKind {
    /// Read the editor's declared type verbatim.
    ///
    /// Unrecognised values become [`FieldKind::Token`], which is the safe
    /// direction: an unrecognised token could be anything, and the string wire
    /// is what every shipped fourcc with an unrecognised type actually uses.
    pub fn of(declared: &str) -> Self {
        match declared {
            "int" => Self::Int,
            "bool" => Self::Bool,
            "char" => Self::Char,
            "real" => Self::Real,
            "unreal" => Self::Unreal,
            "attackBits" => Self::AttackBits,
            "versionFlags" => Self::VersionFlags,
            "intList" => Self::IntList,
            _ => Self::Token,
        }
    }

    /// The value word of a [`crate::Mod`] this kind is written with.
    ///
    /// This is the **measured** relation: the editor's declared type predicts
    /// the wire word of 3 393 of the 3 401 modifications 1.28c's maps ship. The
    /// one mismatch is `uhot`, declared `char` and shipped as type 3 eight
    /// times. The `char` decode is therefore refused — the word is handed out
    /// as bytes — and the field resolves as a string.
    pub fn wire_word(self) -> u32 {
        match self {
            Self::Int
            | Self::Bool
            | Self::Char
            | Self::AttackBits
            | Self::VersionFlags
            | Self::IntList => crate::TYPE_INT,
            Self::Real => crate::TYPE_REAL,
            Self::Unreal => crate::TYPE_UNREAL,
            Self::Token => crate::TYPE_STRING,
        }
    }

    /// Whether this kind is a number on the editor's own classification.
    pub fn is_numeric(self) -> bool {
        matches!(
            self,
            Self::Int
                | Self::Bool
                | Self::Char
                | Self::Real
                | Self::Unreal
                | Self::AttackBits
                | Self::VersionFlags
                | Self::IntList
        )
    }
}

impl std::fmt::Display for FieldKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match *self {
            Self::Int => "int",
            Self::Bool => "bool",
            Self::Char => "char",
            Self::Real => "real",
            Self::Unreal => "unreal",
            Self::AttackBits => "attackBits",
            Self::VersionFlags => "versionFlags",
            Self::IntList => "intList",
            Self::Token => "token",
        })
    }
}

/// One field fourcc, and everything measured about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    /// The fourcc, e.g. `*b"uhpm"`.
    pub fourcc: crate::Fourcc,
    /// Which metadata table it was read from.
    pub from: SlkId,
    /// The row-1 column name the editor writes it in, verbatim.
    pub field: String,
    /// The `slk` column's value, verbatim (`UnitBalance`, `Profile`, …).
    pub slk: String,
    /// The `index` column's value, retained verbatim even for SLK-backed rows.
    pub index: i32,
    /// The declared type, verbatim.
    pub declared: String,
    /// What the declared type decodes to.
    pub kind: FieldKind,
    /// The editor's `category`, verbatim.
    pub category: String,
    /// Where the base value lives, resolved against what this build ships.
    pub source: FieldSource,
    /// Whether this build actually ships a base value for the field.
    ///
    /// `false` only for the eight `Profile` fourccs whose key no shipped
    /// `.txt` section carries (`urq3`..`urq8`, `urqa`, `urva`).
    pub measured: bool,
}

impl FieldDef {
    /// Read every row of one metadata table.
    pub fn read(t: &Table, id: SlkId) -> Vec<Self> {
        let c_id = t.column_of("ID");
        let c_field = t.column_of("field");
        let c_slk = t.column_of("slk");
        let c_index = t.column_of("index");
        let c_type = t.column_of("type");
        let c_cat = t.column_of("category");
        let (Some(c_id), Some(c_field), Some(c_slk), Some(c_index), Some(c_type), Some(c_cat)) =
            (c_id, c_field, c_slk, c_index, c_type, c_cat)
        else {
            // A metadata table without its schema columns is not readable as
            // a schema; every shipped table carries all six.
            return Vec::new();
        };
        let mut out = Vec::new();
        for row in t.data_rows() {
            let Some(code) = row.get(c_id) else { continue };
            let field = row.get(c_field).unwrap_or_default().to_string();
            let slk = row.get(c_slk).unwrap_or_default().to_string();
            let index = row
                .get(c_index)
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(-1);
            let declared = row.get(c_type).unwrap_or_default().to_string();
            let category = row.get(c_cat).unwrap_or_default().to_string();
            let Some(fourcc) = fourcc_of(code) else {
                continue;
            };
            let kind = FieldKind::of(&declared);
            let source = if slk.eq_ignore_ascii_case("profile") {
                FieldSource::Profile {
                    key: field.to_lowercase(),
                    index,
                    // Filled in by the pass below, once every row of this
                    // table has been read.
                    slots: 1,
                }
            } else {
                match slk_for(&slk) {
                    Some(id) => FieldSource::Column {
                        slk: id,
                        column: field.clone(),
                    },
                    None => FieldSource::Unknown {
                        slk: slk.clone(),
                        field: field.clone(),
                    },
                }
            };
            out.push(Self {
                fourcc,
                from: id,
                field,
                slk,
                index,
                declared,
                kind,
                category,
                source,
                // Filled in by [`super::Editor::open_in`] once the tables and
                // the `.txt` layer are loaded: a field is measured when this
                // build really carries a base value for it.
                measured: false,
            });
        }
        // Which `Profile` keys are comma lists: the ones more than one row
        // of this table names. This is the only measured discriminator.
        let mut by_key: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
        for def in &out {
            if let FieldSource::Profile { key, .. } = &def.source {
                *by_key.entry(key.clone()).or_default() += 1;
            }
        }
        for def in &mut out {
            if let FieldSource::Profile { key, slots, .. } = &mut def.source {
                *slots = by_key.get(key).copied().unwrap_or(1);
            }
        }
        out
    }

    /// The declared type, and what it decodes to.
    pub fn kind(&self) -> FieldKind {
        self.kind
    }

    /// The table the base value lives in, for a SLK-backed field.
    pub fn slk_id(&self) -> Option<SlkId> {
        match self.source {
            FieldSource::Column { slk, .. } => Some(slk),
            FieldSource::Profile { .. } | FieldSource::Unknown { .. } => None,
        }
    }

    /// The row-1 column the base value lives in, for a SLK-backed field.
    pub fn column_name(&self) -> &str {
        &self.field
    }

    /// The row-1 column the base value lives in, for a SLK-backed field.
    pub fn column(&self) -> Option<&str> {
        match &self.source {
            FieldSource::Column { column, .. } => Some(column),
            FieldSource::Profile { .. } | FieldSource::Unknown { .. } => None,
        }
    }

    /// The `slk` column's verbatim value, whatever it names.
    pub fn slk_name(&self) -> &str {
        &self.slk
    }

    /// How many slots this field's `Profile` key has, measured from the
    /// metadata rows that name it.
    ///
    /// `1` for every SLK-backed field and every single-row `Profile` field,
    /// `2` for `Buttonpos`, `Missilearc`, `Missileart`, `MissileHoming` and
    /// `Missilespeed`.
    pub fn slots(&self) -> u32 {
        match &self.source {
            FieldSource::Profile { slots, .. } => *slots,
            _ => 1,
        }
    }
}

impl std::fmt::Display for FieldDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} ({}, {})",
            crate::fourcc_text(self.fourcc),
            self.field,
            self.slk,
            self.kind
        )
    }
}

fn fourcc_of(code: &str) -> Option<crate::Fourcc> {
    let b = code.as_bytes();
    if b.len() != 4 || !b.iter().all(|c| c.is_ascii()) {
        return None;
    }
    Some([b[0], b[1], b[2], b[3]])
}

/// Map the `slk` column's value to a decoded table, by its own short name.
///
/// `profile` never reaches here — the caller turns it into a
/// [`FieldSource::Profile`]. `DoodadData` is the one name that does not match
/// its table's short name (`Doodads`); the schema spells it that way, so the
/// alias is measured rather than inferred. A name nothing decodes comes back
/// `None` and the field is refused, not guessed at.
fn slk_for(name: &str) -> Option<SlkId> {
    let aliases = [("DoodadData", SlkId::Doodads)];
    let n = name.trim();
    if let Some((_, id)) = aliases.iter().find(|(a, _)| a.eq_ignore_ascii_case(n)) {
        return Some(*id);
    }
    SlkId::ALL
        .iter()
        .copied()
        .find(|id| id.name().eq_ignore_ascii_case(n))
}
