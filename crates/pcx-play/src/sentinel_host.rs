//! `IssueTargetOrder(unit, "sentinel", tree)`: the Night Elf Sentinel (`Aesn`, `CAbilitySentinel`).
//! Decoded from the reviewed client, see `docs/re-notes/ability-sentinel.md`.
//!
//! The cast (`0xc4d350`) spends one charge, `DataD` (`0x46ef80`, "Number of Owls"), and launches
//! the owl missile (`Mesn`) at the tree. When the missile reaches a tree that is not dead
//! (`0xc4d8f0`), the owl effect (`Xesn`) attaches to it: a sight area owned by the caster's owner,
//! centered on the tree, radius `DataB` ("Hovering Sight Radius"). The effect ends when the tree
//! delivers its life-change, stop or removal event (`0xc4de30`); this host ends it at every
//! destructable life write.
//!
//! Open, so refused: target validation other than a live, visible tree whose `targs` list the
//! tree and its stance, an exhausted ability, the approach when the tree is beyond `Rng`, and the
//! missile flight. The flight time and the in-flight radius (`DataA`) are not decoded, so the owl
//! attaches at the cast.
use super::{Error, Failure, ObjectHost, VisionOrigin, VisionShape, VisionSource, VisionState};
use pcx_objects::editor::FieldKind;
use pcx_script::HandleId;
use std::collections::BTreeMap;

const NATIVE: &str = "IssueTargetOrder";
/// Class code of the Sentinel ability.
const SENTINEL_CLASS: &str = "Aesn";
/// `DataB`: hovering sight radius. `DataD`: number of owls (charges).
const HOVER_RADIUS_STEM: &str = "DataB";
const CHARGES_STEM: &str = "DataD";

/// One owl attached to a tree.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Owl {
    tree: HandleId,
    /// The caster's owner when the effect was created (`0xc4dd60`).
    owner: HandleId,
    center: [f32; 2],
    radius: f32,
}

#[derive(Default)]
pub(super) struct Sentinels {
    owls: Vec<Owl>,
    /// Charges left per caster ability, set from `DataD` on the first cast.
    charges: BTreeMap<(HandleId, [u8; 4]), i32>,
}

impl ObjectHost {
    fn sentinel_error(&self, why: impl Into<String>) -> Error {
        self.inner.error(NATIVE, Failure::UnitActivity(why.into()))
    }

    /// The caster's learned ability whose class code is `Aesn`, with its rank.
    fn sentinel_ability(&self, caster: HandleId) -> Result<Option<([u8; 4], i32)>, Error> {
        let Some(abilities) = self.executed_unit(caster)?.abilities.as_ref() else {
            return Ok(None);
        };
        let learned: Vec<([u8; 4], i32)> =
            abilities.levels().filter(|(_, rank)| *rank > 0).collect();
        for (alias, rank) in learned {
            let parent = self
                .ability_parent_code(alias)
                .map_err(|why| self.sentinel_error(format!("sentinel: {why}")))?;
            let code = self
                .ability_data_text(parent, "code")
                .map_err(|why| self.sentinel_error(format!("sentinel: {why}")))?;
            if code.trim() == SENTINEL_CLASS {
                return Ok(Some((alias, rank)));
            }
        }
        Ok(None)
    }

    /// `IssueTargetOrder(caster, "sentinel", tree)`. Returns false when the caster has no Sentinel
    /// ability; refuses loudly for the undecoded cases in the module header.
    pub(super) fn order_sentinel(
        &mut self,
        caster: HandleId,
        tree: HandleId,
    ) -> Result<bool, Error> {
        let Some((alias, rank)) = self.sentinel_ability(caster)? else {
            return Ok(false);
        };
        let state = self.destructable_state(tree).map_err(|_| {
            self.sentinel_error("sentinel target validation of a non-destructable is not decoded")
        })?;
        let creation = self.destructable_creation(tree)?;
        if state.dead || state.hidden || !state.life.is_some_and(|life| life > 0.0) {
            return Err(self.sentinel_error(
                "sentinel target validation of a dead or hidden tree is not decoded",
            ));
        }
        let is_tree = self
            .destructable_catalog
            .as_ref()
            .ok_or_else(|| self.sentinel_error("destructable catalog not admitted"))?
            .stats_if_destructable(creation.objectid)
            .map_err(|why| self.sentinel_error(why))?
            .is_some_and(|stats| stats.tree);
        let cell = |stem: &str, kind: FieldKind| {
            self.hero_ability_level_cell(alias, stem, rank, kind)
                .map_err(|why| self.sentinel_error(format!("sentinel {stem}{rank}: {why}")))
        };
        let number = |stem: &str, kind: FieldKind| -> Result<f32, Error> {
            let value = cell(stem, kind)?.trim().parse::<f32>().map_err(|_| {
                self.sentinel_error(format!("sentinel {stem}{rank} is not a number"))
            })?;
            if value.is_finite() && value >= 0.0 {
                Ok(value)
            } else {
                Err(self.sentinel_error(format!(
                    "sentinel {stem}{rank} must be finite and nonnegative"
                )))
            }
        };
        let targs = cell("targs", FieldKind::Token)?;
        let lists = |token: &str| targs.split(',').any(|listed| listed.trim() == token);
        let stance = if state.invulnerable { "invu" } else { "vuln" };
        if !is_tree || !lists("tree") || !lists(stance) {
            return Err(self.sentinel_error(
                "sentinel target validation of anything but a tree whose targs list it and its stance is not decoded",
            ));
        }
        let (position, owner) = {
            let record = self.executed_unit(caster)?;
            (
                record
                    .position
                    .ok_or_else(|| self.sentinel_error("sentinel caster has no position"))?,
                record.owner,
            )
        };
        let range = number("Rng", FieldKind::Unreal)?;
        let gap = (position[0] - creation.x).hypot(position[1] - creation.y);
        if gap > range {
            return Err(self.sentinel_error(format!(
                "sentinel tree is {gap} away, outside Rng {range}; the approach is not decoded"
            )));
        }
        // The level hook (`0xc4d230`) loads the charge count as DataD truncated toward zero.
        let charges = number(CHARGES_STEM, FieldKind::Int)?.trunc() as i32;
        let left = *self
            .sentinels
            .charges
            .get(&(caster, alias))
            .unwrap_or(&charges);
        if left <= 0 {
            return Err(
                self.sentinel_error("a Sentinel with no owls left (flag 0x80000) is not decoded")
            );
        }
        let radius = number(HOVER_RADIUS_STEM, FieldKind::Unreal)?;
        let sentinels = &mut self.sentinels;
        if sentinels.owls.try_reserve(1).is_err() {
            return Err(self.sentinel_error("sentinel allocation"));
        }
        sentinels.charges.insert((caster, alias), left - 1);
        sentinels.owls.push(Owl {
            tree,
            owner,
            center: [creation.x, creation.y],
            radius,
        });
        self.wake_natural_sleep(caster);
        self.cancel_patrol(caster);
        Ok(true)
    }

    /// The tree delivered a life-change, stop or removal event: its owls end (`0xc4de30`).
    pub(super) fn end_sentinels_on(&mut self, tree: HandleId) {
        self.sentinels.owls.retain(|owl| owl.tree != tree);
    }

    /// Trees that carry an owl, with its owner, center and sight radius, in cast order.
    pub fn sentinel_owls(&self) -> impl Iterator<Item = (HandleId, HandleId, [f32; 2], f32)> + '_ {
        self.sentinels
            .owls
            .iter()
            .map(|owl| (owl.tree, owl.owner, owl.center, owl.radius))
    }

    /// Sight area of every owl `viewer` owns or is shared vision from.
    pub(super) fn sentinel_vision(
        &self,
        viewer: HandleId,
        sharing: &mut impl FnMut(HandleId) -> Result<bool, Error>,
    ) -> Result<Vec<VisionSource>, Error> {
        let mut sources = Vec::new();
        for owl in &self.sentinels.owls {
            if owl.owner != viewer && !sharing(owl.owner)? {
                continue;
            }
            sources
                .try_reserve(1)
                .map_err(|_| self.sentinel_error("vision allocation failed"))?;
            sources.push(VisionSource {
                shape: VisionShape::Circle {
                    center: owl.center,
                    radius: owl.radius,
                },
                state: VisionState::Visible,
                after_units: false,
                origin: VisionOrigin::Sentinel { tree: owl.tree },
                source_height: None,
            });
        }
        Ok(sources)
    }
}
