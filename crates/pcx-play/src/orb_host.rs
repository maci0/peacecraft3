//! On-hit effects of held orbs (`AIfb` Orb of Fire and Orb of Kil'jaeden, `AIcb` Orb of
//! Corruption, `AIob` Orb of Frost, `AIpb` Orb of Venom). The decoded rules and their evidence are in
//! `docs/re-notes/passive-held-items.md`.
//!
//! Every orb adds its `DataA1` to the holder's attack damage like any `AIat` item (that part is
//! the held flat damage). On a hit that the orb arms, the reviewed client then runs the orb's own
//! effect before it applies the attack damage:
//!
//! - Fire: `DataA1` damage to every living enemy of the attacker within `Area1` of the target,
//!   sparing the target. The damage is universal type, so armor and magic immunity do not apply,
//!   and its attack type is the spells row of the damage-bonus table.
//! - Corruption: armor penalty `DataB1` on the target for `Dur1` (`HeroDur1` for a hero). A
//!   second hit replaces the penalty and restarts the timer; the penalty never stacks.
//! - Frost: the target's move speed falls by `FrostMoveSpeedDecrease` and its attack speed by
//!   `FrostAttackSpeedDecrease` (`MiscGame.txt`) for `Dur1` (`HeroDur1` for a hero). A second hit
//!   restarts the timer; the decreases never stack.
//!
//! - Venom: one poison stack per holder on the target, refreshed by that holder's later hits. The
//!   stacks tick together once a second (first tick 0.01 s after the first stack) for the sum of
//!   their damage per second, universal damage of the spells attack type, until the last stack
//!   ends.
//!
//! The orb arms when the target's class is listed in the orb row's `targs1` (so a structure is
//! outside the stock orbs). Which unit-state flags the client also tests are open; see the note.
use super::combat::Weapon;
use super::unit_buff::{ActiveBuff, BuffTraits};
use super::{Error, Failure, ObjectHost};
use crate::inventory_powerup::{
    BlackArrow, Orb, OrbKind, OrbTargets, ProcCounters, ProcKind, SpellOrb, WrappedAbility,
    WrappedTargets,
};
use pcx_script::HandleId;
use pcx_sim::world::Definition;
use std::collections::BTreeMap;

/// Attack-type name of the splash record, whose index in the client's damage-bonus table is zero.
const SPLASH_ATTACK_TYPE: &str = "spells";
const NAME: &str = "orb hit";
/// Seconds between poison ticks after the first: slot `+0x364` of the periodic buff base
/// (`0x6ef820`) returns the global `0x112d810`, which the static initializer `0x401460` sets to
/// the integer 1.
const POISON_TICK_INTERVAL: f32 = 1.0;
/// A unit with this buff takes no poison tick: the tick routine `0x6efd30` looks it up through
/// the class getter `0xb3ba50` and returns when it is present.
const POISON_SUPPRESSING_BUFF: [u8; 4] = *b"BUsl";

struct ActivePenalty {
    buff: [u8; 4],
    points: i32,
    expires_at: f64,
}

struct ActiveFrost {
    buff: [u8; 4],
    /// Attack speed fraction removed from the unit.
    attack: f32,
    expires_at: f64,
    /// Movement decrease added to the unit's speed multiplier, `None` when the unit has no
    /// speed host.
    movement: Option<f32>,
}

/// One holder's poison on a unit. A later hit by the same holder restarts `expires_at`; a hit by
/// another holder adds a stack.
struct PoisonStack {
    source: HandleId,
    damage: f32,
    expires_at: f64,
}

/// The poison on one unit: the stacks, and the periodic damage they add up to.
struct ActivePoison {
    buff: [u8; 4],
    stacks: Vec<PoisonStack>,
    /// The unit the tick damage is credited to: the holder of the first stack, then of the
    /// latest stack added.
    credit: HandleId,
    next_tick: f64,
}

/// The proc state of one holder's spell orb: the counters survive until the orb item changes.
struct SpellProc {
    item: HandleId,
    counters: ProcCounters,
    /// A swing rolled a proc and no hit has used it yet.
    held: bool,
}

/// A target under Black Arrow: the mark buff and what its death would summon.
struct BlackArrowMark {
    buff: [u8; 4],
    expires_at: f64,
    summon: [u8; 4],
    count: i32,
}

/// Where an item's abilities stand in the unit's orb walk.
#[derive(Clone, Copy, Default)]
struct OrbOrder {
    /// The slot value of the item's abilities (`[ability+0xa4]`): the slot of the latest drag
    /// move, zero until one.
    slot: usize,
    /// Counts up as items enter inventories; the newest ability is the head of the ability list.
    acquired: u64,
}

#[derive(Default)]
pub(super) struct State {
    order: BTreeMap<HandleId, OrbOrder>,
    acquired: u64,
    penalties: BTreeMap<HandleId, ActivePenalty>,
    frosts: BTreeMap<HandleId, ActiveFrost>,
    poisons: BTreeMap<HandleId, ActivePoison>,
    procs: BTreeMap<HandleId, SpellProc>,
    marks: BTreeMap<HandleId, BlackArrowMark>,
}

impl ObjectHost {
    fn orb_error(&self, reason: impl Into<String>) -> Error {
        self.inner.error(NAME, Failure::UnitActivity(reason.into()))
    }

    /// Armor removed from the unit by an active orb penalty.
    pub(crate) fn orb_armor_penalty(&self, unit: HandleId) -> f32 {
        self.orbs
            .penalties
            .get(&unit)
            .map_or(0.0, |penalty| penalty.points as f32)
    }

    /// Attack speed fraction removed from the unit by slows and frost. Each is its own additive
    /// decrease, so a slowed unit that is also frosted loses both.
    pub(crate) fn attack_speed_penalty(&self, unit: HandleId) -> f32 {
        self.curse_attack_penalty(unit)
            + self
                .orbs
                .frosts
                .get(&unit)
                .map_or(0.0, |frost| frost.attack)
    }

    /// Whether an orb of frost owns the unit's move speed.
    pub(crate) fn unit_frosted(&self, unit: HandleId) -> bool {
        self.orbs.frosts.contains_key(&unit)
    }

    /// An item enters a unit's inventory (pickup, creation into a slot, purchase, restore): its
    /// abilities are new objects, so the slot value they carry is back to zero and they join the
    /// head of the unit's ability list.
    pub(super) fn orb_item_acquired(&mut self, item: HandleId) {
        self.orbs.acquired += 1;
        self.orbs.order.insert(
            item,
            OrbOrder {
                slot: 0,
                acquired: self.orbs.acquired,
            },
        );
    }

    /// The inventory drag order moved `item` to `slot` and `other`, when the slot was occupied,
    /// to `other_slot`: each item's abilities take the new slot value (`0x6c6ad0`, called from
    /// `0xb46fe4` and `0xb47007`).
    pub(super) fn orb_items_traded(
        &mut self,
        (item, slot): (HandleId, usize),
        other: Option<(HandleId, usize)>,
    ) {
        for (held, slot) in std::iter::once((item, slot)).chain(other) {
            self.orbs.order.entry(held).or_default().slot = slot;
        }
    }

    /// The orb the attacker's hit on `target` arms, if any. The client walks the unit's orb
    /// abilities by tier, the item's slot value plus a constant, and within a tier from the
    /// newest ability to the oldest, and the first whose arm check passes arms and ends the
    /// walk (`0x697620`, `0xc9e440`, `0xc9e4a0`): one orb arms per hit. The arm check is the
    /// target class test against the orb row's `targs1`.
    pub(super) fn orb_for_hit(
        &self,
        source: HandleId,
        target: HandleId,
    ) -> Result<Option<Orb>, Error> {
        let mut held = Vec::new();
        for (item, record) in self.executed_items() {
            if record.held_by.is_none_or(|(owner, _)| owner != source) {
                continue;
            }
            if let Some(orb) = self.held_attributes(item)?.orb {
                let order = self.orbs.order.get(&item).copied().unwrap_or_default();
                held.push((order.slot, std::cmp::Reverse(order.acquired), orb));
            }
        }
        held.sort_by_key(|&(slot, acquired, _)| (slot, acquired));
        let victim = &self.executed_unit(target)?.definition;
        for (_, _, orb) in held {
            if self.targets_cover(&orb.targets, victim)? {
                return Ok(Some(orb));
            }
        }
        Ok(None)
    }

    /// The weapon `unit` attacks `target` with. A held orb with `DataE1` nonzero enables the
    /// unit's second attack (`0xc9e3a0`); against an air target that weapon 1 cannot reach, a
    /// second weapon that names air replaces it, with its own range, cooldown, damage and
    /// missile. An orb with `DataE1` 0 leaves weapon 1 in place.
    pub(super) fn weapon_against(
        &self,
        unit: HandleId,
        first: Weapon,
        target: HandleId,
    ) -> Result<Weapon, Error> {
        if first.reaches_air != Some(false)
            || !super::unit_types::flying(&self.executed_unit(target)?.definition)
            || !self
                .unit_hero_bonus(unit)?
                .orb
                .is_some_and(|orb| orb.air_switch)
        {
            return Ok(first);
        }
        let second = super::combat::read_weapon(&self.executed_unit(unit)?.definition, 2)
            .map_err(|e| self.orb_error(e))?;
        Ok(second
            .filter(|weapon| weapon.reaches_air == Some(true))
            .unwrap_or(first))
    }

    /// Whether a row's `targs1` classes list the unit class of `victim`: structure, air (a
    /// `movetp` of `fly`) or ground.
    fn targets_cover(&self, targets: &OrbTargets, victim: &Definition) -> Result<bool, Error> {
        let structure = super::unit_types::structure(victim).map_err(|e| self.orb_error(e))?;
        let class = if structure {
            targets.structure
        } else if super::unit_types::flying(victim) {
            targets.air
        } else {
            targets.ground
        };
        // `organic` is the class bit a mechanical unit lacks.
        let organic = !targets.organic
            || !super::unit_types::class(victim, "UNIT_TYPE_MECHANICAL")
                .map_err(|e| self.orb_error(e))?;
        Ok(class && organic)
    }

    /// The vampiric attack (`AIva`) of a landed attack hit: the holder gains `DataA1` of the
    /// damage, capped at the life the target still has, before the hit lowers that life. The hook
    /// `0xbc69b0` runs from the damage routine `0x6677c0` for every attack hit, melee or missile,
    /// when the target's class is in the row's `targs1` and its owner is an enemy. An illusion on
    /// either side heals only when `Misc/IllusionsCanRestoreLife` is set. The target-side veto
    /// `0x682180` is not modelled beyond that, and neither are the ability disabled counter and
    /// virtual `+0x268` of the arming routine `0xbc6ad0`.
    pub(super) fn life_steal_hit(
        &mut self,
        source: HandleId,
        target: HandleId,
        damage: f32,
    ) -> Result<(), Error> {
        if !damage.is_finite() || damage <= 0.0 {
            return Ok(());
        }
        let Some(steal) = self.unit_hero_bonus(source)?.life_steal else {
            return Ok(());
        };
        let Some(victim) = self.weapon_target(target) else {
            return Ok(());
        };
        let holder = self.executed_unit(source)?;
        if !self.hostile_owners(holder.owner, victim.owner)
            || !self.targets_cover(&steal.targets, &victim.definition)?
            || (!steal.illusions_restore
                && (holder.illusion.is_some() || victim.illusion.is_some()))
        {
            return Ok(());
        }
        let remaining = victim.vitals.map_or(0.0, |vitals| vitals.life);
        self.heal_unit(source, damage.min(remaining) * steal.fraction)
    }

    /// Whether the orb changes the target's armor before the attack damage is computed.
    pub(super) fn orb_corrupts(orb: &Orb) -> bool {
        matches!(orb.kind, OrbKind::Corruption { .. })
    }

    /// Applies or refreshes the corruption penalty, the frost decreases or the poison of
    /// `source` on `target`, or casts the ability a held spell orb procced. The fire orb does
    /// nothing here.
    pub(super) fn orb_debuff(
        &mut self,
        orb: &Orb,
        source: HandleId,
        target: HandleId,
    ) -> Result<(), Error> {
        if let OrbKind::Poison {
            damage_per_second,
            duration,
            hero_duration,
            first_tick_delay,
            buff,
        } = orb.kind
        {
            let now = self
                .timer_game_time()
                .ok_or_else(|| self.orb_error("clock missing"))?;
            let hero = self.executed_unit(target)?.definition.rawcode[0].is_ascii_uppercase();
            let expires_at = now + f64::from(if hero { hero_duration } else { duration });
            let stack = PoisonStack {
                source,
                damage: damage_per_second,
                expires_at,
            };
            return self.orb_poison(target, buff, stack, now + f64::from(first_tick_delay));
        }
        if let OrbKind::Spell(spell) = orb.kind {
            return self.orb_spell_hit(&spell, source, target);
        }
        if let OrbKind::Lightning(purge) = orb.kind {
            return self.cast_purge(&purge, source, target);
        }
        if let OrbKind::Frost {
            move_decrease,
            attack_decrease,
            duration,
            hero_duration,
            buff,
        } = orb.kind
        {
            let now = self
                .timer_game_time()
                .ok_or_else(|| self.orb_error("clock missing"))?;
            let hero = self.executed_unit(target)?.definition.rawcode[0].is_ascii_uppercase();
            let expires_at = now + f64::from(if hero { hero_duration } else { duration });
            return self.orb_frost(target, buff, (move_decrease, attack_decrease), expires_at);
        }
        let OrbKind::Corruption {
            armor_penalty,
            duration,
            hero_duration,
            buff,
        } = orb.kind
        else {
            return Ok(());
        };
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.orb_error("clock missing"))?;
        let hero = self.executed_unit(target)?.definition.rawcode[0].is_ascii_uppercase();
        let expires_at = now + f64::from(if hero { hero_duration } else { duration });
        let refresh = self.orbs.penalties.contains_key(&target);
        if !refresh {
            self.attach_unit_buff(
                target,
                ActiveBuff::new(
                    buff,
                    BuffTraits {
                        negative: true,
                        ..BuffTraits::default()
                    },
                ),
            )?;
        }
        self.orbs.penalties.insert(
            target,
            ActivePenalty {
                buff,
                points: armor_penalty,
                expires_at,
            },
        );
        Ok(())
    }

    /// A first frost hit attaches the buff and subtracts the movement decrease from the target's
    /// speed multiplier; a hit on a frosted unit restarts the timer only, because the decreases
    /// are the same `Misc` values every time (the client removes and re-adds them). A slow adds
    /// its own decrease to the same multiplier. A polymorphed unit refuses: the speed writes of
    /// the polymorph type change are not decoded.
    fn orb_frost(
        &mut self,
        target: HandleId,
        buff: [u8; 4],
        (movement, attack): (f32, f32),
        expires_at: f64,
    ) -> Result<(), Error> {
        if let Some(frost) = self.orbs.frosts.get_mut(&target) {
            frost.expires_at = expires_at;
            return Ok(());
        }
        if self.unit_polymorphed(target) {
            return Err(self.orb_error(
                "frost on a polymorphed unit is not modelled: the speed writes of the polymorph type change are not decoded",
            ));
        }
        self.attach_unit_buff(
            target,
            ActiveBuff::new(
                buff,
                BuffTraits {
                    negative: true,
                    ..BuffTraits::default()
                },
            ),
        )?;
        let applied = match self.shift_unit_speed(target, -movement, NAME) {
            Ok(applied) => applied,
            Err(error) => {
                self.detach_unit_buff(target, buff);
                return Err(error);
            }
        };
        self.orbs.frosts.insert(
            target,
            ActiveFrost {
                buff,
                attack,
                expires_at,
                movement: applied.then_some(movement),
            },
        );
        Ok(())
    }

    /// Takes the movement decrease back out of the speed multiplier and removes the buff.
    fn end_frost(&mut self, unit: HandleId) -> Result<(), Error> {
        let Some(frost) = self.orbs.frosts.remove(&unit) else {
            return Ok(());
        };
        if let Some(movement) = frost.movement {
            self.shift_unit_speed(unit, movement, NAME)?;
        }
        self.detach_unit_buff(unit, frost.buff);
        Ok(())
    }

    /// One poison hit. The holder's own stack only restarts its timer; a new holder adds a stack
    /// and becomes the damage credit. The first stack attaches the buff and starts the tick
    /// schedule at `first_tick`; later stacks leave the schedule alone.
    fn orb_poison(
        &mut self,
        target: HandleId,
        buff: [u8; 4],
        stack: PoisonStack,
        first_tick: f64,
    ) -> Result<(), Error> {
        if let Some(poison) = self.orbs.poisons.get_mut(&target) {
            if poison.buff != buff {
                return Err(
                    self.orb_error("poisons of different buffs on one unit are not modelled")
                );
            }
            match poison.stacks.iter_mut().find(|s| s.source == stack.source) {
                Some(own) => own.expires_at = stack.expires_at,
                None => {
                    poison.credit = stack.source;
                    poison.stacks.push(stack);
                }
            }
            return Ok(());
        }
        self.attach_unit_buff(
            target,
            ActiveBuff::new(
                buff,
                BuffTraits {
                    negative: true,
                    // The poison buffs read magic 0 (`+0x1d8`), see the re-note.
                    magic_decoded: true,
                    ..BuffTraits::default()
                },
            ),
        )?;
        self.orbs.poisons.insert(
            target,
            ActivePoison {
                buff,
                credit: stack.source,
                stacks: vec![stack],
                next_tick: first_tick,
            },
        );
        Ok(())
    }

    /// Runs the poison events of `unit` that are due at `now` in time order: the stack expiries
    /// and the ticks. The last expiry ends the poison.
    fn advance_poison(&mut self, unit: HandleId, now: f64) -> Result<(), Error> {
        loop {
            let Some(poison) = self.orbs.poisons.get_mut(&unit) else {
                return Ok(());
            };
            let expiry = poison
                .stacks
                .iter()
                .map(|stack| stack.expires_at)
                .fold(f64::INFINITY, f64::min);
            if expiry <= now && expiry <= poison.next_tick {
                poison.stacks.retain(|stack| stack.expires_at > expiry);
                if poison.stacks.is_empty() {
                    self.end_orb_poison(unit);
                    return Ok(());
                }
            } else if poison.next_tick <= now {
                let total: f32 = poison.stacks.iter().map(|stack| stack.damage).sum();
                let credit = poison.credit;
                poison.next_tick += f64::from(POISON_TICK_INTERVAL);
                self.poison_tick(unit, credit, total)?;
            } else {
                return Ok(());
            }
        }
    }

    /// One poison tick: `total` damage of the spells attack type and universal damage type, so
    /// armor does not apply, credited to `credit`. A sleeping unit (buff `BUsl`) takes none
    /// (`0x6efd30`), and neither does a unit that is not a valid target.
    fn poison_tick(&mut self, unit: HandleId, credit: HandleId, total: f32) -> Result<(), Error> {
        if self
            .unit_buffs(unit)?
            .iter()
            .any(|buff| buff.rawcode() == POISON_SUPPRESSING_BUFF)
        {
            return Ok(());
        }
        let Some(victim) = self.weapon_target(unit) else {
            return Ok(());
        };
        let Some(combat) = victim.combat.as_ref() else {
            return Ok(());
        };
        let defense = combat.defense;
        if self.executed_unit(credit).is_err() {
            return Err(self.orb_error("the unit that poisoned this target no longer exists"));
        }
        let damage = self
            .attack_type_damage(SPLASH_ATTACK_TYPE, total, defense)
            .ok_or_else(|| self.orb_error("the damage-bonus table has no spells row"))?;
        // The tick record has the spells attack type, which spell-resistance items scale.
        let taken = self
            .unit_hero_bonus(unit)?
            .spell_damage_taken
            .unwrap_or(1.0);
        self.apply_damage(credit, unit, damage * taken, true)
    }

    fn end_orb_poison(&mut self, unit: HandleId) {
        if let Some(poison) = self.orbs.poisons.remove(&unit) {
            self.detach_unit_buff(unit, poison.buff);
        }
    }

    /// Fire orb splash around `target`: the full `DataA1` to each living enemy of `source`
    /// within `Area1` plus its collision radius of the target's position, never the target.
    pub(super) fn orb_splash(
        &mut self,
        orb: &Orb,
        source: HandleId,
        target: HandleId,
    ) -> Result<(), Error> {
        let OrbKind::Fire { area } = orb.kind else {
            return Ok(());
        };
        let Some(center) = self.weapon_target(target).and_then(|t| t.position) else {
            return Ok(());
        };
        let owner = self.executed_unit(source)?.owner;
        let victims: Vec<_> = self.executed_units().map(|(id, _)| id).collect();
        let mut hits = Vec::new();
        for id in victims {
            if id == target {
                continue;
            }
            let Some(victim) = self.weapon_target(id) else {
                continue;
            };
            let (Some(position), Some(combat)) = (victim.position, victim.combat.as_ref()) else {
                continue;
            };
            if !self.hostile_owners(owner, victim.owner) {
                continue;
            }
            if !self.targets_cover(&orb.targets, &victim.definition)? {
                continue;
            }
            let defense = combat.defense;
            let reach = area + self.unit_collision_radius(id, NAME)?;
            if super::combat::distance(position, center) > reach {
                continue;
            }
            let damage = self
                .attack_type_damage(SPLASH_ATTACK_TYPE, orb.damage as f32, defense)
                .ok_or_else(|| self.orb_error("the damage-bonus table has no spells row"))?;
            // The splash record has the spells attack type, which spell-resistance items scale.
            let taken = self.unit_hero_bonus(id)?.spell_damage_taken.unwrap_or(1.0);
            hits.push((id, damage * taken));
        }
        for (id, damage) in hits {
            self.apply_damage(source, id, damage, true)?;
        }
        Ok(())
    }

    /// Ends every penalty and frost whose timer has run out.
    pub(super) fn advance_orbs(&mut self, now: f64) -> Result<(), Error> {
        let due: Vec<HandleId> = self
            .orbs
            .penalties
            .iter()
            .filter(|(_, penalty)| penalty.expires_at <= now)
            .map(|(unit, _)| *unit)
            .collect();
        for unit in due {
            self.end_orb_penalty(unit);
        }
        let frozen: Vec<HandleId> = self
            .orbs
            .frosts
            .iter()
            .filter(|(_, frost)| frost.expires_at <= now)
            .map(|(unit, _)| *unit)
            .collect();
        for unit in frozen {
            self.end_frost(unit)?;
        }
        let poisoned: Vec<HandleId> = self.orbs.poisons.keys().copied().collect();
        for unit in poisoned {
            self.advance_poison(unit, now)?;
        }
        let marked: Vec<HandleId> = self
            .orbs
            .marks
            .iter()
            .filter(|(_, mark)| mark.expires_at <= now)
            .map(|(unit, _)| *unit)
            .collect();
        for unit in marked {
            if let Some(mark) = self.orbs.marks.remove(&unit) {
                self.detach_unit_buff(unit, mark.buff);
            }
        }
        Ok(())
    }

    fn end_orb_penalty(&mut self, unit: HandleId) {
        if let Some(penalty) = self.orbs.penalties.remove(&unit) {
            self.detach_unit_buff(unit, penalty.buff);
        }
    }

    /// A buff removal (`UnitRemoveBuffs`) ends the penalty its buff carries.
    pub(super) fn orb_buff_removed(
        &mut self,
        unit: HandleId,
        rawcode: [u8; 4],
    ) -> Result<(), Error> {
        if self
            .orbs
            .penalties
            .get(&unit)
            .is_some_and(|penalty| penalty.buff == rawcode)
        {
            self.orbs.penalties.remove(&unit);
        }
        if self
            .orbs
            .frosts
            .get(&unit)
            .is_some_and(|frost| frost.buff == rawcode)
        {
            self.end_frost(unit)?;
        }
        if self
            .orbs
            .poisons
            .get(&unit)
            .is_some_and(|poison| poison.buff == rawcode)
        {
            self.orbs.poisons.remove(&unit);
        }
        if self
            .orbs
            .marks
            .get(&unit)
            .is_some_and(|mark| mark.buff == rawcode)
        {
            self.orbs.marks.remove(&unit);
        }
        self.purge_buff_removed(unit, rawcode)
    }

    /// A dead or removed unit keeps no penalty, frost or poison.
    pub(super) fn forget_orb_unit(&mut self, unit: HandleId) {
        self.orbs.poisons.remove(&unit);
        self.orbs.penalties.remove(&unit);
        self.orbs.frosts.remove(&unit);
        self.orbs.procs.remove(&unit);
        self.orbs.marks.remove(&unit);
    }
}

/// Spell orbs (`AIsb`): the swing-start proc roll and the cast of the wrapped ability. The decode
/// is in `docs/re-notes/passive-held-items.md`, "`AIsb`".
impl ObjectHost {
    /// The held spell orb of `unit`: its item, orb and spell data.
    fn held_spell_orb(&self, unit: HandleId) -> Result<Option<(HandleId, Orb, SpellOrb)>, Error> {
        for (item, record) in self.executed_items() {
            if record.held_by.is_none_or(|(owner, _)| owner != unit) {
                continue;
            }
            if let Some(orb) = self.held_attributes(item)?.orb
                && let OrbKind::Spell(spell) = orb.kind
            {
                return Ok(Some((item, orb, spell)));
            }
        }
        Ok(None)
    }

    /// Proc kind of a target: a hero by its rawcode, else a summoned unit.
    fn proc_kind(&self, target: HandleId) -> Result<ProcKind, Error> {
        let record = self.executed_unit(target)?;
        Ok(if record.definition.rawcode[0].is_ascii_uppercase() {
            ProcKind::Hero
        } else if record.summoned {
            ProcKind::Summon
        } else {
            ProcKind::Unit
        })
    }

    /// A swing of `unit` at `target` starts: a held spell orb rolls its proc (`AIsb` slot 73,
    /// `0xc9f2d0`). Nothing is rolled when the target is outside the orb's classes. A held proc
    /// stays until a hit uses it or the attack stops.
    pub(super) fn orb_swing_start(
        &mut self,
        unit: HandleId,
        weapon_range: f32,
        target: HandleId,
    ) -> Result<(), Error> {
        let Some((item, orb, spell)) = self.held_spell_orb(unit)? else {
            self.orbs.procs.remove(&unit);
            return Ok(());
        };
        if !self.targets_cover(&orb.targets, &self.executed_unit(target)?.definition)? {
            return Ok(());
        }
        let kind = self.proc_kind(target)?;
        let (mut counters, held) = match self.orbs.procs.get(&unit) {
            Some(proc) if proc.item == item => (proc.counters, proc.held),
            _ => (ProcCounters::attached(), false),
        };
        let chance = counters
            .chance(kind, spell.increments[kind as usize])
            .map_err(|e| self.orb_error(e.to_string()))?;
        let draw = self.draw_random_secondary_real(spell.channel, NAME)?;
        let procced = chance >= draw;
        counters.rolled(kind, procced);
        self.orbs.procs.insert(
            unit,
            SpellProc {
                item,
                counters,
                held: held || procced,
            },
        );
        let range = match spell.wrapped {
            WrappedAbility::BlackArrow(arrow) => Some(arrow.range),
            WrappedAbility::Purge(purge) => Some(purge.range),
            WrappedAbility::Undecoded { .. } => None,
        };
        if procced
            && let Some(range) = range
            && weapon_range > range
        {
            return Err(self.orb_error(format!(
                "a proc of the wrapped ability needs its Rng1 {range} to cover the weapon range \
                 {weapon_range}; the cast range test is not decoded"
            )));
        }
        Ok(())
    }

    /// The attack of `unit` stopped: its held proc is cleared (`AIsb` slot 74, `0xc9f3a0`).
    pub(super) fn orb_attack_stopped(&mut self, unit: HandleId) {
        if let Some(proc) = self.orbs.procs.get_mut(&unit) {
            proc.held = false;
        }
    }

    /// An armed hit of a spell orb lands on `target`. With a proc held and the target valid for
    /// the wrapped ability, the ability is cast before the attack damage (`AIsb` slot 85,
    /// `0xc9f3d0`); the proc is used up. A target the wrapped ability cannot affect leaves the
    /// proc held.
    fn orb_spell_hit(
        &mut self,
        spell: &SpellOrb,
        source: HandleId,
        target: HandleId,
    ) -> Result<(), Error> {
        if !self.orbs.procs.get(&source).is_some_and(|proc| proc.held) {
            return Ok(());
        }
        match spell.wrapped {
            WrappedAbility::BlackArrow(arrow) => {
                if !self.wrapped_target_valid(source, target, &arrow.targets)? {
                    return Ok(());
                }
                self.orb_attack_stopped(source);
                self.black_arrow_mark(&arrow, target)
            }
            WrappedAbility::Purge(purge) => {
                if !self.purge_target_valid(&purge, target)? {
                    return Ok(());
                }
                self.orb_attack_stopped(source);
                self.cast_purge(&purge, source, target)
            }
            WrappedAbility::Undecoded { alias, code } => Err(self.orb_error(format!(
                "a spell orb proc casts {} (class {}), whose cast effect is not decoded: \
                 see docs/re-notes/passive-held-items.md, \"AIsb\"",
                String::from_utf8_lossy(&alias),
                String::from_utf8_lossy(&code)
            ))),
        }
    }

    /// Whether the wrapped ability may be cast on `target`: its class, relationship and
    /// organic tokens against the target and the holder.
    fn wrapped_target_valid(
        &self,
        caster: HandleId,
        target: HandleId,
        targets: &WrappedTargets,
    ) -> Result<bool, Error> {
        let Some(unit) = self.live(target) else {
            return Ok(false);
        };
        let definition = &unit.definition;
        let flying = super::unit_types::flying(definition);
        let structure = super::unit_types::structure(definition).map_err(|e| self.orb_error(e))?;
        let mechanical = super::unit_types::class(definition, "UNIT_TYPE_MECHANICAL")
            .map_err(|e| self.orb_error(e))?;
        let enemy = self.is_enemy(caster, target);
        let ally = self.is_ally(caster, target);
        let related = (targets.enemy && enemy)
            || (targets.neutral && !enemy && !ally)
            || (targets.friend && ally);
        Ok(caster != target
            && related
            && (if flying { targets.air } else { targets.ground })
            && (targets.structure || !structure)
            && !(targets.organic && mechanical))
    }

    /// The Black Arrow cast (`0xc72f40`): a target that is not already a summoned minion gets
    /// the mark buff for `Dur1`, or `HeroDur1` for a hero; a duration of zero casts nothing. A
    /// target that is already marked has its timer restarted.
    fn black_arrow_mark(&mut self, arrow: &BlackArrow, target: HandleId) -> Result<(), Error> {
        let hero = self.executed_unit(target)?.definition.rawcode[0].is_ascii_uppercase();
        let duration = if hero {
            arrow.hero_duration
        } else {
            arrow.duration
        };
        if duration == 0.0 {
            return Ok(());
        }
        if self
            .unit_buffs(target)?
            .iter()
            .any(|buff| buff.rawcode() == arrow.marker)
        {
            return Ok(());
        }
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.orb_error("clock missing"))?;
        let expires_at = now + f64::from(duration);
        if let Some(mark) = self.orbs.marks.get_mut(&target) {
            mark.expires_at = expires_at;
            return Ok(());
        }
        self.attach_unit_buff(
            target,
            ActiveBuff::new(
                arrow.buff,
                BuffTraits {
                    negative: true,
                    // `BNba` slot `+0x1d8` (`0xc72140`) returns 0.
                    magic_decoded: true,
                    ..BuffTraits::default()
                },
            ),
        )?;
        self.orbs.marks.insert(
            target,
            BlackArrowMark {
                buff: arrow.buff,
                expires_at,
                summon: arrow.summon,
                count: arrow.count,
            },
        );
        Ok(())
    }

    /// A marked unit's death makes the summons the client creates through `0xcaf240`, whose
    /// placement is not decoded, so the death refuses before it changes anything.
    pub(super) fn require_black_arrow_unmarked(&self, unit: HandleId) -> Result<(), Error> {
        let Some(mark) = self.orbs.marks.get(&unit) else {
            return Ok(());
        };
        Err(self.orb_error(format!(
            "a unit marked by Black Arrow died: creating {} {} units (placement through \
             0xcaf240) is not decoded",
            mark.count,
            String::from_utf8_lossy(&mark.summon)
        )))
    }
}
