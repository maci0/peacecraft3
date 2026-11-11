//! `IsUnitType` from each unit's resolved definition and live state. Type
//! names resolve through the script's own `UNIT_TYPE_*` constants. Status
//! types without a modeled effect (stunned, ...) read false. Natural sleep uses live unit state.
//!
//! `IsUnitIdType` answers the same names from a unit type's own record, decoded in
//! `docs/re-notes/chapter-startup-refusals.md` (handler `0x499b10`).
use super::{Entry, Error, Failure, MissileSpeed, ObjectHost};
use pcx_objects::editor::{FieldKind, SlkId};
use pcx_script::Value;
use pcx_sim::world::{Definition, FieldValue};

const NATIVE: &str = "IsUnitType";
const TYPE_ID_NATIVE: &str = "IsUnitIdType";

/// What `IsUnitIdType` reads for one `unittype`: the handler's jump table at `0x499ca8`.
#[derive(Clone, Copy)]
enum TypeTest {
    /// Case 0: the first rawcode character is `A` to `Z` (`0x499060`).
    Hero,
    /// Case 2: the type's structure flag (`0x5cc9f0`).
    Structure,
    /// Case 3: the movement code is 2.
    Flying,
    /// Case 4: the movement code is 13.
    Ground,
    /// Case 5: bit 2 of the enabled weapons' target masks.
    AttacksFlying,
    /// Case 6: bit 1 of the enabled weapons' target masks.
    AttacksGround,
    /// Case 7: an enabled weapon of type 1 whose target mask is not excluded.
    MeleeAttacker,
    /// Case 8: an enabled weapon of type 2 to 8 whose target mask is not excluded.
    RangedAttacker,
    /// Cases 9, 10, 14 to 20: the `type` token list holds the token.
    Class,
    /// Cases 1, 11 to 13, 21 to 26: the handler answers false without reading the type.
    Live,
}

const TYPE_TESTS: [(&str, TypeTest); 27] = [
    ("UNIT_TYPE_HERO", TypeTest::Hero),
    ("UNIT_TYPE_DEAD", TypeTest::Live),
    ("UNIT_TYPE_STRUCTURE", TypeTest::Structure),
    ("UNIT_TYPE_FLYING", TypeTest::Flying),
    ("UNIT_TYPE_GROUND", TypeTest::Ground),
    ("UNIT_TYPE_ATTACKS_FLYING", TypeTest::AttacksFlying),
    ("UNIT_TYPE_ATTACKS_GROUND", TypeTest::AttacksGround),
    ("UNIT_TYPE_MELEE_ATTACKER", TypeTest::MeleeAttacker),
    ("UNIT_TYPE_RANGED_ATTACKER", TypeTest::RangedAttacker),
    ("UNIT_TYPE_GIANT", TypeTest::Class),
    ("UNIT_TYPE_SUMMONED", TypeTest::Class),
    ("UNIT_TYPE_STUNNED", TypeTest::Live),
    ("UNIT_TYPE_PLAGUED", TypeTest::Live),
    ("UNIT_TYPE_SNARED", TypeTest::Live),
    ("UNIT_TYPE_UNDEAD", TypeTest::Class),
    ("UNIT_TYPE_MECHANICAL", TypeTest::Class),
    ("UNIT_TYPE_PEON", TypeTest::Class),
    ("UNIT_TYPE_SAPPER", TypeTest::Class),
    ("UNIT_TYPE_TOWNHALL", TypeTest::Class),
    ("UNIT_TYPE_ANCIENT", TypeTest::Class),
    ("UNIT_TYPE_TAUREN", TypeTest::Class),
    ("UNIT_TYPE_POISONED", TypeTest::Live),
    ("UNIT_TYPE_POLYMORPHED", TypeTest::Live),
    ("UNIT_TYPE_SLEEPING", TypeTest::Live),
    ("UNIT_TYPE_RESISTANT", TypeTest::Live),
    ("UNIT_TYPE_ETHEREAL", TypeTest::Live),
    ("UNIT_TYPE_MAGIC_IMMUNE", TypeTest::Live),
];

/// `movetp` tokens and the codes `0x691a80` stores at `type+0x1a8` (table `0x10dccc0`); any
/// other token reads 0.
const MOVEMENT_CODES: [(&str, u32); 7] = [
    ("fly", 2),
    ("hover", 8),
    ("foot", 1),
    ("horse", 4),
    ("unbuild", 0x40),
    ("float", 0x10),
    ("amph", 0x20),
];
const FLYING_MOVEMENT_CODE: u32 = 2;
/// The compare at `0x499b80`. No code in `MOVEMENT_CODES` equals it.
const GROUND_MOVEMENT_CODE: u32 = 0xd;

/// `weapTp` tokens and the numbers `0x6ac680` stores at `type+0xf8+4*slot`; the placeholders
/// read 0.
const WEAPON_TYPE_CODES: [(&str, u32); 8] = [
    ("normal", 1),
    ("missile", 2),
    ("artillery", 3),
    ("instant", 4),
    ("msplash", 5),
    ("mbounce", 6),
    ("mline", 7),
    ("aline", 8),
];
const MELEE_WEAPON_CODE: u32 = 1;
const RANGED_WEAPON_CODES: std::ops::RangeInclusive<u32> = 2..=8;

/// `targs` tokens and the mask bits of table `0x10dce70` (parser `0x6ae660`); a token the
/// table lacks adds nothing. Only the names the shipped unit rows use are listed here.
const TARGET_BITS: [(&str, u32); 33] = [
    ("air", 0x4),
    ("alive", 0x0100_0000),
    ("allies", 0x4000),
    ("ally", 0x4000),
    ("ancient", 0x8000_0000),
    ("bridge", 0x400),
    ("dead", 0x0200_0000),
    ("decoration", 0x200),
    ("debris", 0x100),
    ("enemies", 0x1_0000),
    ("enemy", 0x1_0000),
    ("ground", 0x2),
    ("friend", 0x6000),
    ("hero", 0x40_0000),
    ("invulnerable", 0x20_0000),
    ("item", 0x20),
    ("mechanical", 0x0800_0000),
    ("neutral", 0x8000),
    ("none", 0x1),
    ("nonancient", 0x4000_0000),
    ("nonhero", 0x80_0000),
    ("nonsapper", 0x1000_0000),
    ("notself", 0x1_e000),
    ("organic", 0x0400_0000),
    ("player", 0x2000),
    ("sapper", 0x2000_0000),
    ("self", 0x1000),
    ("structure", 0x8),
    ("totem", 0x10),
    ("tree", 0x40),
    ("vulnerable", 0x10_0000),
    ("wall", 0x80),
    ("ward", 0x10),
];
const GROUND_TARGET_BIT: u32 = 0x2;
const AIR_TARGET_BIT: u32 = 0x4;
/// Masks that exactly equal `none`, `tree`, `wall` or `debris` do not count as a melee or
/// ranged attack (`0x499430`, `0x4997f0`).
const EXCLUDED_TARGET_MASKS: [u32; 4] = [0x1, 0x40, 0x80, 0x100];
/// `weapsOn` bit of attack slot 1 and 2 (`0x499ba0`, `0x499bb3`).
const ATTACK_SLOT_BITS: [i32; 2] = [1, 2];

/// One enabled attack slot of a unit type.
struct TypeAttack {
    weapon_code: u32,
    targets: u32,
}

impl TypeAttack {
    fn counts(&self) -> bool {
        !EXCLUDED_TARGET_MASKS.contains(&self.targets)
    }
}

fn movement_code(definition: &Definition) -> u32 {
    let token = super::unit_positions::text(definition, SlkId::UnitData, "movetp").unwrap_or("");
    MOVEMENT_CODES
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(token))
        .map_or(0, |&(_, code)| code)
}

fn target_mask(list: &str) -> u32 {
    list.split([' ', ',', '-', '\t'])
        .filter(|token| !token.is_empty())
        .filter_map(|token| {
            TARGET_BITS
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(token))
        })
        .fold(0, |mask, &(_, bit)| mask | bit)
}

fn weapon_code(token: &str, name: &str) -> Result<u32, String> {
    if matches!(token, "" | "_" | "-") {
        return Ok(0);
    }
    WEAPON_TYPE_CODES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(token))
        .map(|&(_, code)| code)
        .ok_or_else(|| format!("unit field {name} {token:?} has no decoded weapon type number"))
}

/// The attack slots the type's `weapsOn` enables, in slot order.
fn type_attacks(definition: &Definition) -> Result<Vec<TypeAttack>, String> {
    let enabled = super::combat::column(definition, SlkId::UnitWeapons, "weapsOn")?
        .map(|v| super::combat::number(v, "weapsOn"))
        .transpose()?
        .flatten()
        .unwrap_or(0.0) as i32;
    let mut attacks = Vec::new();
    for (slot, bit) in (1u8..).zip(ATTACK_SLOT_BITS) {
        if enabled & bit == 0 {
            continue;
        }
        let word = |column: String| -> Result<String, String> {
            Ok(
                match super::combat::column(definition, SlkId::UnitWeapons, &column)? {
                    Some(v) => super::combat::text(v, &column)?.to_owned(),
                    None => String::new(),
                },
            )
        };
        let kind_column = format!("weapTp{slot}");
        let kind = word(kind_column.clone())?;
        attacks.push(TypeAttack {
            weapon_code: weapon_code(&kind, &kind_column)?,
            targets: target_mask(&word(format!("targs{slot}"))?),
        });
    }
    Ok(attacks)
}

fn reaches(attacks: &[TypeAttack], bit: u32) -> bool {
    attacks.iter().any(|a| a.targets & bit != 0)
}

fn attacks_air(attacks: &[TypeAttack]) -> bool {
    reaches(attacks, AIR_TARGET_BIT)
}

fn attacks_ground(attacks: &[TypeAttack]) -> bool {
    reaches(attacks, GROUND_TARGET_BIT)
}

fn melee(attacks: &[TypeAttack]) -> bool {
    attacks
        .iter()
        .any(|a| a.weapon_code == MELEE_WEAPON_CODE && a.counts())
}

fn ranged(attacks: &[TypeAttack]) -> bool {
    attacks
        .iter()
        .any(|a| RANGED_WEAPON_CODES.contains(&a.weapon_code) && a.counts())
}

pub(super) fn structure(definition: &Definition) -> Result<bool, String> {
    let field =
        super::unit_positions::field(definition, SlkId::UnitBalance, "isbldg", &[FieldKind::Bool])
            .map_err(|e| format!("UnitBalance/isbldg: {e}"))?;
    match field {
        FieldValue::Cell(value) if value.trim() == "0" => Ok(false),
        FieldValue::Cell(value) if value.trim() == "1" => Ok(true),
        FieldValue::Override(pcx_objects::Value::Int(0)) => Ok(false),
        FieldValue::Override(pcx_objects::Value::Int(1)) => Ok(true),
        _ => Err("UnitBalance/isbldg requires a source boolean".into()),
    }
}

/// Whether the unit moves through the air: its `UnitData` `movetp` is `fly`.
pub(super) fn flying(definition: &Definition) -> bool {
    super::unit_positions::text(definition, SlkId::UnitData, "movetp").unwrap_or("") == "fly"
}

/// Script type names whose suffix is the `UnitBalance/type` source token.
const CLASSES: [&str; 8] = [
    "UNIT_TYPE_GIANT",
    "UNIT_TYPE_UNDEAD",
    "UNIT_TYPE_MECHANICAL",
    "UNIT_TYPE_PEON",
    "UNIT_TYPE_SAPPER",
    "UNIT_TYPE_TOWNHALL",
    "UNIT_TYPE_ANCIENT",
    "UNIT_TYPE_TAUREN",
];

pub(super) fn class(definition: &Definition, global: &str) -> Result<bool, String> {
    let token = global
        .strip_prefix("UNIT_TYPE_")
        .ok_or("unit class requires a source type name")?;
    let list = super::unit_positions::text(definition, SlkId::UnitBalance, "type")
        .map_err(|e| format!("UnitBalance/type: {e}"))?;
    Ok(list
        .split(',')
        .any(|part| part.trim().eq_ignore_ascii_case(token)))
}

impl ObjectHost {
    /// Living, shown `UNIT_TYPE_PEON` units of player `owner` that report no current order
    /// and are not run by the harvest controller, in creation order. Recreation choice:
    /// the client's idle-worker list is not decoded.
    pub fn idle_workers(&self, owner: i32) -> Result<Vec<pcx_script::HandleId>, String> {
        let mut idle = Vec::new();
        for (id, unit) in self.live_units(owner)? {
            if !class(&unit.definition, "UNIT_TYPE_PEON")? {
                continue;
            }
            if !self.harvest_runs(id)
                && self.current_unit_order(id).map_err(|e| e.to_string())? == 0
            {
                idle.push(id);
            }
        }
        Ok(idle)
    }

    /// Living, shown `UNIT_TYPE_TOWNHALL` units of player `owner` with their camera
    /// points, in creation order. The Backspace list of the client holds the own Town
    /// Hall (`input-camera.md`); its class test and order are not decoded.
    pub fn town_halls(&self, owner: i32) -> Result<Vec<(pcx_script::HandleId, [f32; 2])>, String> {
        let mut halls = Vec::new();
        for (id, unit) in self.live_units(owner)? {
            if class(&unit.definition, "UNIT_TYPE_TOWNHALL")? {
                halls.push((id, unit.position.unwrap_or([unit.x, unit.y])));
            }
        }
        Ok(halls)
    }

    /// Where the camera centres on `unit`.
    pub fn unit_camera_point(&self, unit: pcx_script::HandleId) -> Result<[f32; 2], String> {
        let unit = self.executed_unit(unit).map_err(|e| e.to_string())?;
        Ok(unit.position.unwrap_or([unit.x, unit.y]))
    }

    /// Living, shown units of player `owner` in creation order.
    fn live_units(
        &self,
        owner: i32,
    ) -> Result<Vec<(pcx_script::HandleId, &super::ExecutedUnit)>, String> {
        let mut found = Vec::new();
        for (id, unit) in self.executed_units() {
            let present = unit.shown
                && !unit.combat.as_ref().is_some_and(|c| c.dead)
                && !unit.vitals.as_ref().is_some_and(|v| v.life <= 0.0);
            if present
                && self
                    .player_settings(unit.owner)
                    .map_err(|e| e.to_string())?
                    .number
                    == owner
            {
                found.push((id, unit));
            }
        }
        Ok(found)
    }

    /// `IsUnitIdType(rawcode, kind)`: the `kind` test answered from the unit type's own record.
    /// A rawcode the catalog lacks has no record: the handler's record reads give 0, so every
    /// test but `UNIT_TYPE_HERO` (rawcode shape only) is false. The structure flag of such an id
    /// is not decoded and refuses.
    fn unit_id_type(&self, args: &[Value]) -> Result<Value, Error> {
        let refuse = |why: String| self.inner.error(TYPE_ID_NATIVE, Failure::UnitActivity(why));
        let [Value::Int(rawcode), Value::Handle(kind)] = args else {
            return Err(self.inner.error(TYPE_ID_NATIVE, Failure::Signature));
        };
        let Entry::Symbolic {
            kind: family,
            enumerand,
        } = self.inner.entry(*kind)?
        else {
            return Err(self.inner.error(TYPE_ID_NATIVE, Failure::WrongFamily));
        };
        if family != "unittype" {
            return Err(self.inner.error(TYPE_ID_NATIVE, Failure::WrongFamily));
        }
        let Some(&(global, test)) = TYPE_TESTS.iter().find(|(global, _)| {
            self.source_constant_enumerand(global, "unittype")
                .is_ok_and(|v| v == *enumerand)
        }) else {
            return Ok(Value::Bool(false));
        };
        let requested = rawcode.to_be_bytes();
        let definition = match test {
            TypeTest::Hero => {
                return Ok(Value::Bool(requested[0].is_ascii_uppercase()));
            }
            TypeTest::Live => return Ok(Value::Bool(false)),
            _ => match self
                .unit_catalog()
                .map_err(|e| self.inner.error(TYPE_ID_NATIVE, e.failure))?
                .resolve(requested)
            {
                Ok(definition) => Some(definition),
                Err(
                    pcx_sim::world::ResolutionProblem::MissingRawcode(_)
                    | pcx_sim::world::ResolutionProblem::InvalidRawcode,
                ) => None,
                Err(problem) => {
                    return Err(self.inner.error(
                        TYPE_ID_NATIVE,
                        Failure::UnitCatalog {
                            requested,
                            problem: Box::new(problem),
                        },
                    ));
                }
            },
        };
        let Some(definition) = definition else {
            return match test {
                TypeTest::Structure => Err(refuse(format!(
                    "{global} of a rawcode with no catalog row is not decoded"
                ))),
                _ => Ok(Value::Bool(false)),
            };
        };
        let value = match test {
            TypeTest::Structure => structure(&definition),
            TypeTest::Flying => Ok(movement_code(&definition) == FLYING_MOVEMENT_CODE),
            TypeTest::Ground => Ok(movement_code(&definition) == GROUND_MOVEMENT_CODE),
            TypeTest::AttacksFlying => type_attacks(&definition).map(|a| attacks_air(&a)),
            TypeTest::AttacksGround => type_attacks(&definition).map(|a| attacks_ground(&a)),
            TypeTest::MeleeAttacker => type_attacks(&definition).map(|a| melee(&a)),
            TypeTest::RangedAttacker => type_attacks(&definition).map(|a| ranged(&a)),
            TypeTest::Class => class(&definition, global),
            TypeTest::Hero | TypeTest::Live => Ok(false),
        };
        value.map(Value::Bool).map_err(refuse)
    }

    pub(super) fn unit_type_native(
        &self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !matches!(name, NATIVE | TYPE_ID_NATIVE) || self.units.is_none() {
            return None;
        }
        self.inner.program.natives().get(name)?;
        if name == TYPE_ID_NATIVE {
            return Some(self.unit_id_type(args));
        }
        Some((|| {
            let [unit, Value::Handle(kind)] = args else {
                return Err(self.inner.error(name, Failure::Signature));
            };
            let Entry::Symbolic {
                kind: family,
                enumerand,
            } = self.inner.entry(*kind)?
            else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            if family != "unittype" {
                return Err(self.inner.error(name, Failure::WrongFamily));
            }
            let record = match unit {
                Value::Null => return Ok(Value::Bool(false)),
                Value::Handle(id) => match self.inner.entry(*id)? {
                    Entry::Unit { record } => record,
                    Entry::RemovedUnit { .. } => return Ok(Value::Bool(false)),
                    _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                },
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            let is = |global: &str| {
                self.source_constant_enumerand(global, "unittype")
                    .is_ok_and(|v| v == *enumerand)
            };
            let definition = &record.definition;
            let weapon = record.combat.as_ref().and_then(|c| c.weapon);
            let value = if is("UNIT_TYPE_HERO") {
                record.illusion.is_none() && definition.rawcode[0].is_ascii_uppercase()
            } else if is("UNIT_TYPE_DEAD") {
                record.combat.as_ref().is_some_and(|c| c.dead)
                    || record.vitals.is_some_and(|v| v.life <= 0.0)
            } else if is("UNIT_TYPE_SLEEPING") {
                record.is_sleeping()
            } else if is("UNIT_TYPE_SUMMONED") {
                record.summoned
            } else if is("UNIT_TYPE_STRUCTURE") {
                structure(definition)
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e)))?
            } else if is("UNIT_TYPE_FLYING") {
                record
                    .movement()
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e.into())))?
                    == crate::unit_placement::Movement::Air
            } else if is("UNIT_TYPE_GROUND") {
                record
                    .movement()
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e.into())))?
                    != crate::unit_placement::Movement::Air
            } else if is("UNIT_TYPE_MELEE_ATTACKER") {
                weapon.is_some_and(|w| w.missile_speed == MissileSpeed::Instant)
            } else if is("UNIT_TYPE_RANGED_ATTACKER") {
                weapon.is_some_and(|w| w.missile_speed != MissileSpeed::Instant)
            } else if let Some(&global) = CLASSES.iter().find(|global| is(global)) {
                class(definition, global)
                    .map_err(|e| self.inner.error(NATIVE, Failure::UnitActivity(e)))?
            } else {
                false
            };
            Ok(Value::Bool(value))
        })())
    }
}
