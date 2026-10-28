//! Upgrade effect slots and the pool arithmetic of the effects whose application is established.
//!
//! Source: `UpgradeData` columns named by `UpgradeMetaData` rows that carry an `effectType`
//! (`EffectID`, `Base`, `Mod`, `Code` per `effectIndex`), the map's `w3q` edits on those fields,
//! and the effect ids of `UpgradeEffectMetaData`. Application semantics come from the reviewed
//! 1.29.2.9231 executable; see `docs/re-notes/upgrade-effects.md`.
use crate::{
    inventory_powerup_host::{reprice_flat_life_scaled, round_life_with},
    neutral_player::sha256,
    neutral_rule_input::Pe,
    player_handicap::LifeRounding,
};
use pcx_objects::editor::{
    FieldKind, FieldSource, Overlay, SlkId,
    defaults::{DefaultTables, Family},
};
use std::collections::{BTreeMap, BTreeSet};

pub type EffectId = [u8; 4];

/// A level value is `base + modifier * (level - 1)`, and zero at level zero (`0x6acc50`).
const FIRST_LEVEL: i32 = 1;
const NO_EFFECT_CELL: &str = "_";
const EMPTY_CELL: &str = "-";

/// One effect of an upgrade: the `effectN`, `baseN`, `modN` and `codeN` columns of an `UpgradeData` row.
#[derive(Clone, Debug, PartialEq)]
pub struct Slot {
    /// The `effectIndex` of the slot, from 1.
    pub number: u8,
    pub effect: EffectId,
    pub base: f32,
    pub modifier: f32,
    pub code: String,
}

impl Slot {
    /// The effect's value at a researched level.
    pub fn value(&self, level: i32) -> f32 {
        if level == 0 {
            0.0
        } else {
            self.base + self.modifier * (level - FIRST_LEVEL) as f32
        }
    }

    /// The change applied when a player's level moves from `from` to `to`.
    pub fn delta(&self, from: i32, to: i32) -> f32 {
        self.value(to) - self.value(from)
    }
}

/// How the host treats an effect id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// `rmnx`: maximum mana, current mana keeps its ratio.
    MaxMana,
    /// `rhpx`: maximum life scaled by the owner's health factor, current life keeps its ratio.
    MaxLife,
    /// `rmnr`: mana regeneration rate.
    ManaRegen,
    /// Application not established. `additive` records that the decoded routine adds the
    /// value delta, so a zero delta changes nothing.
    Pending {
        reason: &'static str,
        additive: bool,
    },
}

const NOT_DECODED: &str = "the native application routine is not decoded";
const NOT_IN_METADATA: &str =
    "the effect id is absent from UpgradeEffectMetaData and its routine is not decoded";

/// The application status of an effect id. `defined` is whether `UpgradeEffectMetaData` names it.
pub fn classify(effect: &EffectId, defined: bool) -> Effect {
    let pending = |reason, additive| Effect::Pending { reason, additive };
    match effect {
        b"rmnx" => Effect::MaxMana,
        b"rhpx" => Effect::MaxLife,
        b"rmnr" => Effect::ManaRegen,
        b"ratd" | b"ratx" => pending(
            "the native routine converts the delta to an integer and updates selected weapons; the selection mask's source-field and enum mappings are not decoded",
            true,
        ),
        b"rmvx" | b"rmov" => pending(
            "the native routine adds a movement bonus whose interaction with the speed limits and the slow buff is not decoded",
            true,
        ),
        b"rhpo" => pending(
            "the native routine scales a unit value from a virtual query that is not decoded",
            true,
        ),
        b"rman" => pending(
            "the native routine scales a unit type value whose UnitBalance column is not decoded",
            true,
        ),
        b"rarm" => pending(
            "the native routine adds the level change times a unit type value to the armor bonus; the armor model has no upgrade term",
            false,
        ),
        _ if defined => pending(NOT_DECODED, false),
        _ => pending(NOT_IN_METADATA, false),
    }
}

/// Effects of every upgrade that has at least one slot, plus the effect ids the metadata defines.
#[derive(Clone, Debug, Default)]
pub struct Table {
    defined: BTreeSet<EffectId>,
    upgrades: BTreeMap<i32, Vec<Slot>>,
    flags: BTreeMap<i32, (bool, bool)>,
}

impl Table {
    pub(crate) fn flags(&self, upgrade: i32) -> Option<(bool, bool)> {
        self.flags.get(&upgrade).copied()
    }
    pub fn slots(&self, upgrade: i32) -> &[Slot] {
        self.upgrades.get(&upgrade).map_or(&[], Vec::as_slice)
    }

    pub fn is_defined(&self, effect: &EffectId) -> bool {
        self.defined.contains(effect)
    }

    pub fn classify(&self, effect: &EffectId) -> Effect {
        classify(effect, self.is_defined(effect))
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn upgrade_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.upgrades.keys().copied()
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn synthetic(defined: &[EffectId], upgrades: Vec<(i32, Vec<Slot>)>) -> Self {
        Self {
            defined: defined.iter().copied().collect(),
            flags: upgrades
                .iter()
                .map(|(id, _)| (*id, (false, false)))
                .collect(),
            upgrades: upgrades.into_iter().collect(),
        }
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn synthetic_flags(mut self, upgrade: i32, inherit: bool, global: bool) -> Self {
        self.flags.insert(upgrade, (inherit, global));
        self
    }

    /// Read the slot layout from `UpgradeMetaData`, the effect ids from `UpgradeEffectMetaData`,
    /// and every listed upgrade's slots from its row and the map's edits.
    pub fn read(
        tables: &DefaultTables,
        overlay: &Overlay,
        upgrades: &[i32],
    ) -> Result<Self, String> {
        let layout = read_layout(tables)?;
        let defined = read_defined(tables)?;
        let mut out = BTreeMap::new();
        let mut flags = BTreeMap::new();
        let flag_column = |name: &str| -> Result<Column, String> {
            let mut fields = tables.editor().fields().iter().filter(|field| {
                field.from == SlkId::UpgradeMetaData
                    && matches!(&field.source, FieldSource::Column { slk: SlkId::UpgradeData, column } if column == name)
            });
            let field = fields
                .next()
                .ok_or_else(|| format!("missing {name} metadata"))?;
            if fields.next().is_some() || field.kind != FieldKind::Bool || field.index != -1 {
                return Err(format!("ambiguous or unsupported {name} metadata"));
            }
            Ok(Column {
                fourcc: field.fourcc,
                name: name.into(),
            })
        };
        let inherit = flag_column("inherit")?;
        let global = flag_column("global")?;
        for &id in upgrades {
            let code = id.to_be_bytes();
            let alias = std::str::from_utf8(&code).map_err(|e| e.to_string())?;
            let (base, mods) = overlay
                .new_object(alias)
                .unwrap_or((alias, overlay.on(alias)));
            let object = tables
                .object(Family::Upgrade, base)
                .map_err(|e| format!("{alias}: {e}"))?;
            let find = |column: &Column| -> Result<Option<&pcx_objects::Mod>, String> {
                let Some(edit) = mods.iter().rev().find(|m| m.field == column.fourcc) else {
                    return Ok(None);
                };
                if edit.selectors.is_some_and(|s| s != [0, 0]) {
                    return Err(format!(
                        "{alias}: unsupported {} edit selector",
                        column.name
                    ));
                }
                Ok(Some(edit))
            };
            let text = |column: &Column| -> Result<String, String> {
                if let Some(edit) = find(column)? {
                    let pcx_objects::Value::Str(bytes) = &edit.value else {
                        return Err(format!("{alias}: {} edit is not a string", column.name));
                    };
                    return Ok(std::str::from_utf8(bytes)
                        .map_err(|e| format!("{alias}: {}: {e}", column.name))?
                        .to_string());
                }
                let cell = object
                    .cell(SlkId::UpgradeData, &column.name)
                    .map_err(|e| format!("{alias}: {e}"))?;
                if cell.parse_error.is_some() {
                    return Err(format!("{alias}: malformed {}", column.name));
                }
                Ok(cell.text.trim().to_string())
            };
            let number = |column: &Column| -> Result<f32, String> {
                if let Some(edit) = find(column)? {
                    return match edit.value {
                        pcx_objects::Value::Int(v) => Ok(v as i32 as f32),
                        pcx_objects::Value::Real(v) | pcx_objects::Value::Unreal(v) => Ok(v),
                        _ => Err(format!("{alias}: {} edit is not numeric", column.name)),
                    };
                }
                let cell = text(column)?;
                if cell.is_empty() || cell == EMPTY_CELL {
                    return Ok(0.0);
                }
                cell.parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("{alias}: {} is not a finite number", column.name))
            };
            let flag = |column: &Column| -> Result<bool, String> {
                if let Some(edit) = find(column)? {
                    if edit.value_type != pcx_objects::ValueType::Int {
                        return Err(format!("{alias}: {} edit is not an integer", column.name));
                    }
                    return match edit.value {
                        pcx_objects::Value::Int(value) => Ok(value != 0),
                        _ => Err(format!("{alias}: {} edit is not an integer", column.name)),
                    };
                }
                text(column)?
                    .parse::<i32>()
                    .map(|value| value != 0)
                    .map_err(|_| format!("{alias}: {} is not an integer", column.name))
            };
            flags.insert(id, (flag(&inherit)?, flag(&global)?));
            let mut slots = Vec::new();
            for (index, columns) in &layout {
                let effect = text(&columns.effect)?;
                if effect.is_empty() || effect == NO_EFFECT_CELL || effect == EMPTY_CELL {
                    continue;
                }
                let effect: EffectId = effect
                    .as_bytes()
                    .try_into()
                    .map_err(|_| format!("{alias}: effect {index} is not a four-byte id"))?;
                let code = text(&columns.code)?;
                slots.push(Slot {
                    number: *index,
                    effect,
                    base: number(&columns.base)?,
                    modifier: number(&columns.modifier)?,
                    code: if code == EMPTY_CELL {
                        String::new()
                    } else {
                        code
                    },
                });
            }
            if !slots.is_empty() {
                out.insert(id, slots);
            }
        }
        Ok(Self {
            defined,
            upgrades: out,
            flags,
        })
    }
}

struct Column {
    fourcc: [u8; 4],
    name: String,
}

struct Columns {
    effect: Column,
    base: Column,
    modifier: Column,
    code: Column,
}

fn read_layout(tables: &DefaultTables) -> Result<BTreeMap<u8, Columns>, String> {
    let table = tables
        .editor()
        .table(SlkId::UpgradeMetaData)
        .ok_or("missing UpgradeMetaData")?;
    let mut parts: BTreeMap<u8, BTreeMap<String, Column>> = BTreeMap::new();
    for row in table.data_rows() {
        let Some(kind) = row.by_name("effectType").filter(|k| !k.trim().is_empty()) else {
            continue;
        };
        let cell = |name| {
            row.by_name(name)
                .map(str::trim)
                .ok_or_else(|| format!("UpgradeMetaData row {}: missing {name}", row.index()))
        };
        if cell("slk")? != "UpgradeData" {
            return Err(format!(
                "UpgradeMetaData row {}: effect field outside UpgradeData",
                row.index()
            ));
        }
        let index: u8 = cell("effectIndex")?
            .parse()
            .map_err(|_| format!("UpgradeMetaData row {}: effectIndex", row.index()))?;
        let fourcc: [u8; 4] = cell("ID")?
            .as_bytes()
            .try_into()
            .map_err(|_| format!("UpgradeMetaData row {}: ID", row.index()))?;
        let column = Column {
            fourcc,
            name: cell("field")?.to_string(),
        };
        if parts
            .entry(index)
            .or_default()
            .insert(kind.trim().to_string(), column)
            .is_some()
        {
            return Err(format!(
                "UpgradeMetaData: duplicate {kind} for slot {index}"
            ));
        }
    }
    parts
        .into_iter()
        .map(|(index, mut by_kind)| {
            let mut take = |kind: &str| {
                by_kind
                    .remove(kind)
                    .ok_or_else(|| format!("UpgradeMetaData: slot {index} has no {kind} field"))
            };
            let columns = Columns {
                effect: take("EffectID")?,
                base: take("Base")?,
                modifier: take("Mod")?,
                code: take("Code")?,
            };
            if let Some(extra) = by_kind.keys().next() {
                return Err(format!("UpgradeMetaData: slot {index} has unknown {extra}"));
            }
            Ok((index, columns))
        })
        .collect()
}

fn read_defined(tables: &DefaultTables) -> Result<BTreeSet<EffectId>, String> {
    let table = tables
        .editor()
        .table(SlkId::UpgradeEffectMetaData)
        .ok_or("missing UpgradeEffectMetaData")?;
    table
        .data_rows()
        .filter_map(|row| row.by_name("effectID"))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(|id| {
            id.as_bytes()
                .try_into()
                .map_err(|_| format!("UpgradeEffectMetaData: effectID {id:?}"))
        })
        .collect()
}

/// Reviewed executable routines the implemented effects rely on: virtual address, length, SHA-256.
const PROFILE: &[(u32, usize, &str)] = &[
    // Level value `base + mod * (level - 1)`.
    (
        0x6acc50,
        0x78,
        "cc7f1885e1d663eb30206e2614c0daf309efd14800a02d8b8ed13d102d061188",
    ),
    // `rmnx` effect id getter, then its apply routine.
    (
        0x6a4420,
        0x6,
        "c55d5bec93583c2723e0af5de8a890a562258e384dbf9f81676123f806be4468",
    ),
    (
        0x6a4430,
        0x73,
        "a4824ec0009f9ff8bfa35072451dd515f3399461e1aabd9d3c93286c65926673",
    ),
    // `rhpx` effect id getter, then its apply routine.
    (
        0x6a3960,
        0x6,
        "e4a81c2ac9cc6ebfa79e2cbc0601585ca50c519bb3f16359d43a0f9ffca1ffba",
    ),
    (
        0x6a3970,
        0x90,
        "15d6ab48a8087cadc0717babce64446a2d2d886e26a8182ada794ed8c93b03c9",
    ),
    // `rmnr` id, apply routine, property update and regeneration refresh.
    (
        0x6a45f0,
        0x6,
        "010ecd052b98df9a59c088e3913426a72f0b01a56c6ecf814d33805b2a22f6d4",
    ),
    (
        0x6a4600,
        0x44,
        "2ebaf5409abc104631c8209d40769e14aa1e44a39100ccef004b30b7cb59406d",
    ),
    (
        0x680310,
        0x3d,
        "ca4e2d9a9a4e8bd3d39325bfdd00724972b80bab1b07da797ccbf74d03225012",
    ),
    (
        0x6a1070,
        0x130,
        "b689f6d89124d5822839a5334653da762a4a1b7f4842dd39dbd37946b7338c86",
    ),
    // Maximum-mana change, then maximum-life change (mode 1 keeps the current/maximum ratio).
    (
        0x680350,
        0x1d9,
        "26fae0aa5139038d8aafd33c62cd73c8bfa7e6a3e5a3fe36dd9c170e6ad89acd",
    ),
    (
        0x6be8c0,
        0x265,
        "47a470d70048b5f3fae182304b4e82f61f1c94814060da26ce5b11926aac481a",
    ),
    // Level change applies each effect to the owner's units; creation applies level zero to the current level.
    (
        0x6dd240,
        0x33,
        "1a380da2912d0dd01c1deea24ad7c2261a9137f8e956698ea19b02bc948c0143",
    ),
    (
        0x6dd280,
        0x69,
        "712cf978701a5201f23be5641c3044a8e635d6feff8d97338c84ac22082178ee",
    ),
];

/// Refuse an executable whose upgrade-effect routines differ from the reviewed ones.
pub(crate) fn verify_profile(pe: &Pe<'_>) -> Result<(), String> {
    for &(va, len, digest) in PROFILE {
        let bytes = pe
            .read_from(va, true)
            .map_err(|e| e.to_string())?
            .get(..len)
            .ok_or_else(|| format!("truncated upgrade-effect routine {va:#x}"))?;
        let actual: String = sha256(bytes).iter().map(|b| format!("{b:02x}")).collect();
        if actual != digest {
            return Err(format!("unsupported upgrade-effect routine {va:#x}"));
        }
    }
    Ok(())
}

/// Maximum-mana change with the current mana keeping its ratio (`0x680350` mode 1). The change
/// is rounded first, and a zero change stops before any write.
pub fn change_max_mana(
    mana: f32,
    max_mana: f32,
    delta: f32,
    rounding: Option<LifeRounding>,
) -> Result<(f32, f32), String> {
    if !mana.is_finite() || !max_mana.is_finite() || !delta.is_finite() {
        return Err("non-finite mana value".into());
    }
    if delta == 0.0 {
        return Ok((mana, max_mana));
    }
    if max_mana < 0.0 || !(0.0..=max_mana).contains(&mana) {
        return Err("mana pools must satisfy 0 <= mana <= max_mana".into());
    }
    let next_max = max_mana + round_life_with(delta, rounding);
    if !next_max.is_finite() || next_max < 0.0 {
        return Err("new maximum mana is negative".into());
    }
    let ratio = if max_mana > 0.0 { mana / max_mana } else { 0.0 };
    Ok((
        round_life_with(next_max * ratio, rounding).clamp(0.0, next_max),
        next_max,
    ))
}

/// Maximum-life change scaled by the owner's health factor (`0x6a3970`), then `0x6be8c0` mode 1:
/// rounded change, ratio kept, a living unit keeps at least one hit point.
pub fn change_max_life(
    life: f32,
    max_life: f32,
    delta: f32,
    factor: f32,
    rounding: Option<LifeRounding>,
    dead: bool,
) -> Result<(f32, f32), String> {
    let (life, max_life) = reprice_flat_life_scaled(life, max_life, delta, factor, rounding)?;
    Ok(crate::inventory_powerup_host::clamp_life_pools(
        life, max_life, dead,
    ))
}

/// One unapplied slot with the reason, kept by the host for the player and upgrade.
#[derive(Clone, Debug, PartialEq)]
pub struct Unapplied {
    pub player: i32,
    pub upgrade: i32,
    pub slot: u8,
    pub effect: EffectId,
    pub from: i32,
    pub to: i32,
    pub reason: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(effect: &EffectId, base: f32, modifier: f32) -> Slot {
        Slot {
            number: 1,
            effect: *effect,
            base,
            modifier,
            code: String::new(),
        }
    }

    #[test]
    fn level_value_is_zero_at_zero_and_steps_by_the_modifier() {
        let s = slot(b"rmnx", 30.0, 7.5);
        assert_eq!(s.value(0), 0.0);
        assert_eq!(s.value(1), 30.0);
        assert_eq!(s.value(3), 45.0);
        assert_eq!(s.delta(1, 3), 15.0);
        assert_eq!(s.delta(3, 0), -45.0);
    }

    #[test]
    fn max_mana_keeps_the_ratio_and_rounds_the_change_first() {
        assert_eq!(change_max_mana(50.0, 200.0, 100.0, None), Ok((75.0, 300.0)));
        assert_eq!(change_max_mana(0.0, 0.0, 40.0, None), Ok((0.0, 40.0)));
        assert_eq!(change_max_mana(10.0, 20.0, 0.0, None), Ok((10.0, 20.0)));
        // 0.4 rounds to zero, which still recomputes the current from the ratio.
        assert_eq!(change_max_mana(7.0, 20.0, 0.4, None), Ok((7.0, 20.0)));
        assert_eq!(
            change_max_mana(150.0, 300.0, -100.0, None),
            Ok((100.0, 200.0))
        );
        assert!(change_max_mana(10.0, 20.0, -25.0, None).is_err());
    }

    #[test]
    fn max_life_scales_by_the_factor_and_keeps_a_living_floor() {
        assert_eq!(
            change_max_life(60.0, 100.0, 50.0, 1.0, None, false),
            Ok((90.0, 150.0))
        );
        assert_eq!(
            change_max_life(60.0, 100.0, 50.0, 0.5, None, false),
            Ok((75.0, 125.0))
        );
        assert_eq!(
            change_max_life(1.0, 1000.0, 10.0, 1.0, None, false),
            Ok((1.0, 1010.0))
        );
        assert_eq!(
            change_max_life(0.0, 100.0, 10.0, 1.0, None, true),
            Ok((0.0, 110.0))
        );
        assert_eq!(
            change_max_life(60.0, 100.0, 50.0, 0.0, None, false),
            Ok((60.0, 100.0))
        );
    }

    fn sylk(headers: &[&str], rows: &[&[&str]]) -> Vec<u8> {
        let mut text = String::from("ID;PWXL;N;E\r\n");
        for (y, row) in std::iter::once(headers)
            .chain(rows.iter().copied())
            .enumerate()
        {
            for (x, value) in row.iter().enumerate() {
                text.push_str(&format!("C;X{};Y{};K\"{}\"\r\n", x + 1, y + 1, value));
            }
        }
        text.push_str("E\r\n");
        text.into_bytes()
    }

    /// Two-slot layout with nonstock field ids, a three-row upgrade sheet and two effect ids.
    fn tables(effect_meta: &[&[&str]]) -> DefaultTables {
        let layout = [
            ("Qe01", "fxa", "EffectID", "1"),
            ("Qb01", "fba", "Base", "1"),
            ("Qm01", "fma", "Mod", "1"),
            ("Qc01", "fca", "Code", "1"),
            ("Qe02", "fxb", "EffectID", "2"),
            ("Qb02", "fbb", "Base", "2"),
            ("Qm02", "fmb", "Mod", "2"),
            ("Qc02", "fcb", "Code", "2"),
        ];
        let mut meta_rows: Vec<Vec<&str>> = layout
            .iter()
            .map(|(id, field, kind, index)| {
                vec![
                    *id,
                    *field,
                    "UpgradeData",
                    "-1",
                    "string",
                    "data",
                    *kind,
                    *index,
                ]
            })
            .collect();
        meta_rows.push(vec![
            "Qinh",
            "inherit",
            "UpgradeData",
            "-1",
            "bool",
            "stats",
            "",
            "",
        ]);
        meta_rows.push(vec![
            "Qglb",
            "global",
            "UpgradeData",
            "-1",
            "bool",
            "stats",
            "",
            "",
        ]);
        let meta_rows: Vec<&[&str]> = meta_rows.iter().map(Vec::as_slice).collect();
        let meta = sylk(
            &[
                "ID",
                "field",
                "slk",
                "index",
                "type",
                "category",
                "effectType",
                "effectIndex",
            ],
            &meta_rows,
        );
        let data = sylk(
            &[
                "upgradeid",
                "fxa",
                "fba",
                "fma",
                "fca",
                "fxb",
                "fbb",
                "fmb",
                "fcb",
                "inherit",
                "global",
            ],
            &[
                &[
                    "Z001", "wxyz", "10", "2.5", "-", "_", "-", "-", "-", "1", "0",
                ],
                &["Z002", "_", "-", "-", "-", "_", "-", "-", "-", "0", "1"],
                &[
                    "Z003", "pqrs", "-", "-", "Acde", "wxyz", "7", "-", "-", "0", "0",
                ],
            ],
        );
        let effects = sylk(&["ID", "effectID", "dataType"], effect_meta);
        let units = sylk(&["unitID"], &[]);
        DefaultTables::from_bytes(
            pcx_objects::editor::defaults::Identity {
                install: "synthetic catalog".into(),
                edition_version: "synthetic version".into(),
            },
            &[
                (SlkId::UpgradeData.path(), &data),
                (SlkId::UpgradeMetaData.path(), &meta),
                (SlkId::UpgradeEffectMetaData.path(), &effects),
                (SlkId::UnitData.path(), &units),
            ],
            &[],
            pcx_objects::editor::EditorReadLimits {
                max_listed_paths: 16,
                max_asset_bytes: 8192,
                max_aggregate_input_bytes: 65536,
            },
        )
        .unwrap()
    }

    fn edit(field: &[u8; 4], value: pcx_objects::Value) -> pcx_objects::Mod {
        pcx_objects::Mod {
            field: *field,
            value_type: match value {
                pcx_objects::Value::Str(_) => pcx_objects::ValueType::Str,
                pcx_objects::Value::Int(_) => pcx_objects::ValueType::Int,
                _ => pcx_objects::ValueType::Unreal,
            },
            selectors: Some([0, 0]),
            value,
            end_word: 0,
        }
    }

    const EFFECT_META: &[&[&str]] = &[&["aaab", "wxyz", "Base"], &["aaac", "pqrs", "Mod"]];

    fn ids() -> Vec<i32> {
        ["Z001", "Z002", "Z003", "Z009"]
            .iter()
            .map(|id| i32::from_be_bytes(id.as_bytes().try_into().unwrap()))
            .collect()
    }

    #[test]
    fn slots_follow_the_metadata_layout_and_stock_cells() {
        let tables = tables(EFFECT_META);
        let ids = &ids()[..3];
        let table = Table::read(&tables, &Overlay::default(), ids).unwrap();
        let z = |id: &[u8; 4]| i32::from_be_bytes(*id);
        assert_eq!(
            table.slots(z(b"Z001")),
            [Slot {
                number: 1,
                effect: *b"wxyz",
                base: 10.0,
                modifier: 2.5,
                code: String::new()
            }]
        );
        assert!(
            table.slots(z(b"Z002")).is_empty(),
            "an all-empty row has no slots"
        );
        assert_eq!(
            table.slots(z(b"Z003")),
            [
                Slot {
                    number: 1,
                    effect: *b"pqrs",
                    base: 0.0,
                    modifier: 0.0,
                    code: "Acde".into()
                },
                Slot {
                    number: 2,
                    effect: *b"wxyz",
                    base: 7.0,
                    modifier: 0.0,
                    code: String::new()
                }
            ]
        );
        assert!(table.is_defined(b"wxyz") && table.is_defined(b"pqrs"));
        assert_eq!(table.flags(z(b"Z001")), Some((true, false)));
        assert_eq!(table.flags(z(b"Z002")), Some((false, true)));
        assert!(!table.is_defined(b"zzzz"));
    }

    #[test]
    fn map_edits_and_custom_upgrades_override_the_cells() {
        let tables = tables(EFFECT_META);
        let mut overlay = Overlay::default();
        overlay.mods.insert(
            "Z001".into(),
            vec![
                edit(b"Qb01", pcx_objects::Value::Unreal(99.0)),
                edit(b"Qb01", pcx_objects::Value::Unreal(33.0)),
                edit(b"Qe02", pcx_objects::Value::Str(b"pqrs".to_vec())),
                edit(b"Qb02", pcx_objects::Value::Int(4)),
                edit(b"Qinh", pcx_objects::Value::Int(0)),
                edit(b"Qglb", pcx_objects::Value::Int(1)),
            ],
        );
        overlay.custom.insert(
            "Z009".into(),
            (
                "Z003".into(),
                vec![edit(b"Qm02", pcx_objects::Value::Real(1.5))],
            ),
        );
        let table = Table::read(&tables, &overlay, &ids()).unwrap();
        let z = |id: &[u8; 4]| i32::from_be_bytes(*id);
        let first = table.slots(z(b"Z001"));
        assert_eq!(
            (first[0].base, first[0].modifier),
            (33.0, 2.5),
            "the last edit wins"
        );
        assert_eq!((first[1].effect, first[1].base), (*b"pqrs", 4.0));
        assert_eq!(table.flags(z(b"Z001")), Some((false, true)));
        let custom = table.slots(z(b"Z009"));
        assert_eq!((custom[1].base, custom[1].modifier), (7.0, 1.5));
    }

    #[test]
    fn malformed_slots_and_unsupported_edit_selectors_refuse() {
        let tables = tables(EFFECT_META);
        let mut overlay = Overlay::default();
        overlay.mods.insert(
            "Z001".into(),
            vec![edit(b"Qe01", pcx_objects::Value::Str(b"toolong".to_vec()))],
        );
        let error = Table::read(&tables, &overlay, &ids()[..1]).unwrap_err();
        assert!(error.contains("four-byte"), "{error}");
        overlay.mods.get_mut("Z001").unwrap()[0] = pcx_objects::Mod {
            selectors: Some([2, 0]),
            ..edit(b"Qb01", pcx_objects::Value::Unreal(1.0))
        };
        let error = Table::read(&tables, &overlay, &ids()[..1]).unwrap_err();
        assert!(error.contains("selector"), "{error}");
        overlay.mods.clear();
        overlay.mods.insert(
            "Z001".into(),
            vec![edit(b"Qb01", pcx_objects::Value::Str(b"x".to_vec()))],
        );
        let error = Table::read(&tables, &overlay, &ids()[..1]).unwrap_err();
        assert!(error.contains("not numeric"), "{error}");
        overlay.mods.insert(
            "Z001".into(),
            vec![pcx_objects::Mod {
                selectors: Some([1, 0]),
                ..edit(b"Qinh", pcx_objects::Value::Int(1))
            }],
        );
        let error = Table::read(&tables, &overlay, &ids()[..1]).unwrap_err();
        assert!(
            error.contains("inherit") && error.contains("selector"),
            "{error}"
        );
    }

    #[test]
    fn classification_separates_established_pending_and_undefined_effects() {
        assert_eq!(classify(b"rmnx", true), Effect::MaxMana);
        assert_eq!(classify(b"rhpx", true), Effect::MaxLife);
        assert_eq!(classify(b"rmnr", true), Effect::ManaRegen);
        assert!(matches!(
            classify(b"ratd", true),
            Effect::Pending { additive: true, .. }
        ));
        assert!(matches!(
            classify(b"rtma", true),
            Effect::Pending {
                additive: false,
                reason: NOT_DECODED
            }
        ));
        assert!(matches!(
            classify(b"zzzz", false),
            Effect::Pending {
                additive: false,
                reason: NOT_IN_METADATA
            }
        ));
    }
}
