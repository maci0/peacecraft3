//! `IssueTargetOrder(unit, "entangleinstant", goldMine)`: the Entangle ability (`Aent`,
//! `CAbilityEntangle`) converting a gold mine. Decoded from the reviewed client, see
//! `docs/re-notes/entangle-mine-conversion.md`.
//!
//! The created type is the ability's own `UnitID1` row (`egol` in the shipped data), not a
//! constant. The conversion (`0xb947a0`, run from event `0xd018c` at `0xb95790`):
//!
//! - the target mine is hidden;
//! - a unit of that type is created for the caster's owner at the mine's position, facing the
//!   `Misc/BuildingAngle` quarter, through the same factory as `CreateUnit` (`0x687ca0`);
//! - the entangled mine takes the mine's remaining gold (`0xb27980`, `Agld +0x70`) and links the
//!   hidden mine, which returns when the entangled mine dies (the haunted-mine link).
//!
//! Open, so refused: the non-instant form (the casting stage), the approach and placement test
//! of the caster (`0xb94b70`, `0xb955d0`), mana and cooldown (the shipped row has none), a target
//! that is not a plain gold mine, and releasing workers inside the mine (`0xb63060`).
use super::{Error, Failure, ObjectHost};
use pcx_objects::editor::FieldKind;
use pcx_script::{HandleId, Value};

const NATIVE: &str = "IssueTargetOrder";
/// Order text of the instant form (`0xd00b4`).
pub(super) const INSTANT_ORDER: &str = "entangleinstant";
/// Order text of the timed form (`0xd00b3`).
pub(super) const TIMED_ORDER: &str = "entangle";
/// Class code of Entangle.
const ENTANGLE_CLASS: &str = "Aent";
/// The gold mine resource ability of a plain mine.
const MINE_ALIAS: &str = "Agld";
const DEGREES_PER_QUARTER: f32 = 90.0;

impl ObjectHost {
    fn entangle_error(&self, why: impl Into<String>) -> Error {
        self.inner.error(NATIVE, Failure::UnitActivity(why.into()))
    }

    /// The caster's learned ability whose class code is `Aent`, with its rank.
    fn entangle_ability(&self, caster: HandleId) -> Result<Option<([u8; 4], i32)>, Error> {
        let Some(abilities) = self.executed_unit(caster)?.abilities.as_ref() else {
            return Ok(None);
        };
        let learned: Vec<([u8; 4], i32)> =
            abilities.levels().filter(|(_, rank)| *rank > 0).collect();
        for (alias, rank) in learned {
            let parent = self
                .ability_parent_code(alias)
                .map_err(|why| self.entangle_error(format!("entangle: {why}")))?;
            let code = self
                .ability_data_text(parent, "code")
                .map_err(|why| self.entangle_error(format!("entangle: {why}")))?;
            if code.trim() == ENTANGLE_CLASS {
                return Ok(Some((alias, rank)));
            }
        }
        Ok(None)
    }

    /// `IssueTargetOrder(caster, "entangleinstant", mine)`. Returns false when the caster has no
    /// Entangle ability or the target is not a gold mine resource (the validators `0x6a19a0` and
    /// `0xb94700` refuse); refuses loudly for the undecoded cases in the module header.
    pub(super) fn order_entangle_instant(
        &mut self,
        caster: HandleId,
        mine: HandleId,
    ) -> Result<bool, Error> {
        let Some((alias, rank)) = self.entangle_ability(caster)? else {
            return Ok(false);
        };
        let (mine_position, gold, mine_is_plain) = {
            let Some(record) = self.live(mine) else {
                return Ok(false);
            };
            let Some(gold) = record.resource_amount else {
                return Ok(false);
            };
            let plain = super::unit_creation::lists_admitted_alias(&record.definition, MINE_ALIAS);
            (record.position, gold, plain)
        };
        if !mine_is_plain {
            return Err(self.entangle_error(
                "entangle of a resource unit without Agld (entangled or haunted mine) is not decoded",
            ));
        }
        let mine_position =
            mine_position.ok_or_else(|| self.entangle_error("entangle target has no position"))?;
        let cell = |stem: &str, kind: FieldKind| {
            self.hero_ability_level_cell(alias, stem, rank, kind)
                .map_err(|why| self.entangle_error(format!("entangle {stem}{rank}: {why}")))
        };
        let number = |stem: &str, kind: FieldKind| -> Result<f32, Error> {
            cell(stem, kind)?
                .trim()
                .parse::<f32>()
                .map_err(|_| self.entangle_error(format!("entangle {stem}{rank} is not a number")))
        };
        for stem in ["Cost", "Cool"] {
            let kind = if stem == "Cost" {
                FieldKind::Int
            } else {
                FieldKind::Unreal
            };
            if number(stem, kind)? != 0.0 {
                return Err(self.entangle_error(format!(
                    "entangle {stem}{rank} is not zero; its charge is not decoded"
                )));
            }
        }
        let range = number("Rng", FieldKind::Unreal)?;
        let text = cell("UnitID", FieldKind::Token)?;
        let kind = <[u8; 4]>::try_from(text.trim().as_bytes())
            .map_err(|_| self.entangle_error(format!("entangle UnitID{rank} {text:?}")))?;
        let (position, owner) = {
            let record = self.executed_unit(caster)?;
            (
                record
                    .position
                    .ok_or_else(|| self.entangle_error("entangle caster has no position"))?,
                record.owner,
            )
        };
        // Recreation reading of the in-range test (`0xb94b70` is not decoded): the gap between
        // the collision circles must not exceed `Rng`. Out of range would walk or re-root the
        // caster, which is not decoded.
        let gap = (position[0] - mine_position[0]).hypot(position[1] - mine_position[1])
            - self.unit_collision_radius(caster, NATIVE)?
            - self.unit_collision_radius(mine, NATIVE)?;
        if gap > range {
            return Err(self.entangle_error(format!(
                "entangle target is {gap} beyond collision edges, outside Rng {range}; the approach is not decoded"
            )));
        }
        let facing = self
            .unit_placement
            .as_ref()
            .and_then(|placement| placement.rules().footprint_geometry())
            .map(|geometry| f32::from(geometry.building_quarter()) * DEGREES_PER_QUARTER)
            .ok_or_else(|| self.entangle_error("Misc/BuildingAngle is not admitted"))?;
        self.hide_linked_mine(mine)?;
        let created = self.prepare_unit_creation(
            &[
                Value::Handle(owner),
                Value::Int(i32::from_be_bytes(kind)),
                Value::Real(mine_position[0]),
                Value::Real(mine_position[1]),
                Value::Real(facing),
            ],
            None,
            super::unit_creation::Birth::Ordinary,
        );
        let mut prepared = match created {
            Ok(prepared) => prepared,
            Err(error) => {
                self.show_linked_mine(mine)?;
                return Err(error);
            }
        };
        if prepared.record.resource_amount.is_none() {
            self.show_linked_mine(mine)?;
            return Err(self.entangle_error("entangled mine type has no resource amount"));
        }
        prepared.record.resource_amount = Some(gold);
        prepared.record.linked_mine = Some(mine);
        if let Err(error) = self.commit_unit_creation(prepared) {
            self.show_linked_mine(mine)?;
            return Err(error);
        }
        Ok(true)
    }
}
