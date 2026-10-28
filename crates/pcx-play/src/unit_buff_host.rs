//! Active per-unit buff state and the `UnitRemoveBuffs` /
//! `UnitRemoveBuffsEx` removal filters derived from the selected client's shared
//! helper `0x6D87A0`.
//!
//! The helper walks the unit's ability chain and evaluates vtable slots `0x1E0`
//! (negative), `0x1E4` (positive), `0x1E8` (aura), `0x334` (timed life),
//! `0x1D8` (magic), `0x1DC` (physical), and `0x338` (auto-dispel), removing each
//! matching buff. The basic `UnitRemoveBuffs` wrapper calls the helper with
//! timed-life/aura enabled and magic/physical/auto-dispel disabled;
//! `UnitRemoveBuffsEx` reorders its booleans into the helper's stack order
//! (physical, magic, timed-life, aura, auto-dispel, remove-positive,
//! remove-negative).
//!
//! The shipped `Units\AbilityBuffData.slk` carries only `alias`, `code`,
//! `isEffect`, `race` and editor metadata; it does not encode polarity,
//! resistance, timed-life or dispel class. A buff-producing ability host will
//! supply the rawcode (checked against the buff table spelling when the
//! applying ability is implemented) and the class traits; classification is the
//! applying ability's behavior, not a value invented here or derived from the
//! buff tables. Producers (Slow and Polymorph in `unit_curse_host`) attach and
//! remove buffs through the crate-internal hooks below; the bounded per-unit
//! state backs the removal filters and is not a public API. A buff that
//! `UnitRemoveBuffs` removes is reported to its producer so the effect ends.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;

const REMOVE: &str = "UnitRemoveBuffs";
const REMOVE_EX: &str = "UnitRemoveBuffsEx";

const NATIVES: [(&str, &[&str], &str); 2] = [
    (REMOVE, &["unit", "boolean", "boolean"], "nothing"),
    (
        REMOVE_EX,
        &[
            "unit", "boolean", "boolean", "boolean", "boolean", "boolean", "boolean", "boolean",
        ],
        "nothing",
    ),
];

/// Classification of one active buff as the applying ability declares it,
/// matching the shared removal helper's predicate slots. The buff tables do not
/// encode these bits; a buff-producing ability host supplies them from its own
/// behavior when a producer is implemented.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BuffTraits {
    pub positive: bool,
    pub negative: bool,
    pub magic: bool,
    pub physical: bool,
    pub timed_life: bool,
    pub aura: bool,
    pub auto_dispellable: bool,
}

/// One active buff on a unit, as attached by a producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ActiveBuff {
    rawcode: [u8; 4],
    traits: BuffTraits,
}

impl ActiveBuff {
    pub(crate) const fn new(rawcode: [u8; 4], traits: BuffTraits) -> Self {
        Self { rawcode, traits }
    }

    #[cfg(any(test, feature = "test-fixtures"))]
    pub(crate) const fn rawcode(&self) -> [u8; 4] {
        self.rawcode
    }
}

/// Cap on buffs retained per unit in the test-only producer hook.
pub(crate) const MAX_BUFFS_PER_UNIT: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RemoveOptions {
    remove_positive: bool,
    remove_negative: bool,
    magic: bool,
    physical: bool,
    timed_life: bool,
    aura: bool,
    auto_dispel: bool,
}

impl RemoveOptions {
    fn basic(remove_positive: bool, remove_negative: bool) -> Self {
        Self {
            remove_positive,
            remove_negative,
            magic: false,
            physical: false,
            timed_life: true,
            aura: true,
            auto_dispel: false,
        }
    }
}

/// The shared helper's filter, read from `0x6D87A0`. A buff that is neither
/// positive nor negative is never removable through this path.
fn should_remove(buff: &ActiveBuff, options: &RemoveOptions) -> bool {
    if !buff.traits.positive && !buff.traits.negative {
        return false;
    }
    if !((options.remove_positive && buff.traits.positive)
        || (options.remove_negative && buff.traits.negative))
    {
        return false;
    }
    if !options.aura && buff.traits.aura {
        return false;
    }
    if !options.timed_life && buff.traits.timed_life {
        return false;
    }
    if options.physical && !buff.traits.physical {
        return false;
    }
    if options.magic && !buff.traits.magic {
        return false;
    }
    if options.auto_dispel && !buff.traits.auto_dispellable {
        return false;
    }
    true
}

#[derive(Default)]
pub(super) struct State {
    active: BTreeMap<HandleId, Vec<ActiveBuff>>,
}

impl ObjectHost {
    pub(super) fn unit_buff_declarations(&self, source: &str) -> Result<(), Error> {
        for (name, args, result) in NATIVES {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(&self.inner, name, args, result, false, Some("unit"))?;
                if native.source != source {
                    return Err(self.inner.error(name, Failure::Source));
                }
            }
        }
        Ok(())
    }

    /// `Ok(Some(()))` for a live `unit`, `Ok(None)` for a removed tombstone,
    /// and `Err` for any other family.
    fn unit_liveness(&self, native: &str, unit: HandleId) -> Result<Option<()>, Error> {
        let fail = |failure| self.inner.error(native, failure);
        match self.inner.entry(unit).map_err(|e| fail(e.failure))? {
            Entry::Unit { .. } => Ok(Some(())),
            Entry::RemovedUnit { .. } => Ok(None),
            _ => Err(fail(Failure::WrongFamily)),
        }
    }

    /// Internal, bounded hook for a buff-producing ability host to attach an
    /// active buff to a live unit. The producer supplies the rawcode and class
    /// traits from its own behavior; this is not a public API and does not
    /// claim buff-table-derived classification. Refuses removed units and is
    /// capped at [`MAX_BUFFS_PER_UNIT`] per unit.
    pub(crate) fn attach_unit_buff(
        &mut self,
        unit: HandleId,
        buff: ActiveBuff,
    ) -> Result<(), Error> {
        if self.unit_liveness("unit buff", unit)?.is_none() {
            return Err(self
                .inner
                .error("unit buff", Failure::UnitState("unit removed")));
        }
        let active = self.unit_buffs.active.entry(unit).or_default();
        if active.len() >= MAX_BUFFS_PER_UNIT {
            return Err(self
                .inner
                .error("unit buff", Failure::Budget("unit buff capacity")));
        }
        active.push(buff);
        self.record_placement_event(
            unit,
            crate::ai_placement::PlacementEventKind::BuffApplied(buff.rawcode),
        );
        Ok(())
    }

    /// The active buffs attached to a unit, in application order. A removed unit
    /// reports an empty set (its buffs are forgotten on `RemoveUnit`).
    #[cfg(any(test, feature = "test-fixtures"))]
    pub(crate) fn unit_buffs(&self, unit: HandleId) -> Result<Vec<ActiveBuff>, Error> {
        if self.unit_liveness("unit buff", unit)?.is_none() {
            return Ok(Vec::new());
        }
        Ok(self
            .unit_buffs
            .active
            .get(&unit)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .to_vec())
    }

    /// Removes every active buff of `rawcode` from a unit and reports whether one was present.
    pub(crate) fn detach_unit_buff(&mut self, unit: HandleId, rawcode: [u8; 4]) -> bool {
        let Some(active) = self.unit_buffs.active.get_mut(&unit) else {
            return false;
        };
        let before = active.len();
        active.retain(|buff| buff.rawcode != rawcode);
        let removed = before != active.len();
        if removed {
            self.record_placement_event(
                unit,
                crate::ai_placement::PlacementEventKind::BuffRemoved(rawcode),
            );
        }
        removed
    }

    pub(super) fn forget_unit_buffs(&mut self, unit: HandleId) {
        if let Some(buffs) = self.unit_buffs.active.remove(&unit) {
            for buff in buffs {
                self.record_placement_event(
                    unit,
                    crate::ai_placement::PlacementEventKind::BuffRemoved(buff.rawcode),
                );
            }
        }
    }

    pub(super) fn unit_buff_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !NATIVES.iter().any(|(n, ..)| *n == name)
            || self.inner.program.natives().get(name).is_none()
        {
            return None;
        }
        Some((|| {
            let (unit, options) = match (name, args) {
                (REMOVE, [unit, Value::Bool(positive), Value::Bool(negative)]) => {
                    (unit.clone(), RemoveOptions::basic(*positive, *negative))
                }
                (
                    REMOVE_EX,
                    [
                        unit,
                        Value::Bool(positive),
                        Value::Bool(negative),
                        Value::Bool(magic),
                        Value::Bool(physical),
                        Value::Bool(timed_life),
                        Value::Bool(aura),
                        Value::Bool(auto_dispel),
                    ],
                ) => (
                    unit.clone(),
                    RemoveOptions {
                        remove_positive: *positive,
                        remove_negative: *negative,
                        magic: *magic,
                        physical: *physical,
                        timed_life: *timed_life,
                        aura: *aura,
                        auto_dispel: *auto_dispel,
                    },
                ),
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let id = match unit {
                Value::Null => return Ok(Value::Null),
                Value::Handle(id) => id,
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            };
            match self
                .inner
                .entry(id)
                .map_err(|e| self.inner.error(name, e.failure))?
            {
                Entry::RemovedUnit { .. } => return Ok(Value::Null),
                Entry::Unit { .. } => {}
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }
            let mut removed = Vec::new();
            if let Some(active) = self.unit_buffs.active.get_mut(&id) {
                active.retain(|buff| {
                    let remove = should_remove(buff, &options);
                    if remove {
                        removed.push(buff.rawcode);
                    }
                    !remove
                });
            }
            for &rawcode in &removed {
                self.record_placement_event(
                    id,
                    crate::ai_placement::PlacementEventKind::BuffRemoved(rawcode),
                );
            }
            for rawcode in removed {
                self.curse_buff_removed(id, rawcode)?;
            }
            Ok(Value::Null)
        })())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buff(rawcode: [u8; 4], traits: BuffTraits) -> ActiveBuff {
        ActiveBuff::new(rawcode, traits)
    }

    #[test]
    fn basic_removal_follows_the_helper_polarity_gate() {
        let positive = buff(
            *b"BHab",
            BuffTraits {
                positive: true,
                ..Default::default()
            },
        );
        let negative = buff(
            *b"Bcor",
            BuffTraits {
                negative: true,
                ..Default::default()
            },
        );
        let neither = buff(*b"Bncl", BuffTraits::default());
        let both = buff(
            *b"Bfro",
            BuffTraits {
                positive: true,
                negative: true,
                ..Default::default()
            },
        );

        let only_positive = RemoveOptions::basic(true, false);
        let only_negative = RemoveOptions::basic(false, true);
        let neither_flag = RemoveOptions::basic(false, false);

        assert!(should_remove(&positive, &only_positive));
        assert!(!should_remove(&negative, &only_positive));
        assert!(should_remove(&negative, &only_negative));
        assert!(!should_remove(&positive, &only_negative));
        assert!(!should_remove(&neither, &only_positive));
        assert!(!should_remove(&neither, &only_negative));
        assert!(!should_remove(&positive, &neither_flag));
        assert!(!should_remove(&negative, &neither_flag));
        assert!(should_remove(&both, &only_positive));
        assert!(should_remove(&both, &only_negative));
    }

    #[test]
    fn ex_filters_apply_only_when_the_corresponding_boolean_is_set() {
        let timed = buff(
            *b"BTLf",
            BuffTraits {
                positive: true,
                timed_life: true,
                ..Default::default()
            },
        );
        let aura = buff(
            *b"BHab",
            BuffTraits {
                positive: true,
                aura: true,
                ..Default::default()
            },
        );
        let magic = buff(
            *b"Bmgs",
            BuffTraits {
                positive: true,
                magic: true,
                ..Default::default()
            },
        );
        let physical = buff(
            *b"Bphy",
            BuffTraits {
                positive: true,
                physical: true,
                ..Default::default()
            },
        );
        let dispellable = buff(
            *b"Bdis",
            BuffTraits {
                positive: true,
                auto_dispellable: true,
                ..Default::default()
            },
        );
        let opaque = buff(
            *b"Bnod",
            BuffTraits {
                positive: true,
                ..Default::default()
            },
        );

        let keep_timed = RemoveOptions {
            remove_positive: true,
            remove_negative: false,
            magic: false,
            physical: false,
            timed_life: false,
            aura: false,
            auto_dispel: false,
        };
        let keep_aura = RemoveOptions {
            timed_life: false,
            aura: false,
            ..keep_timed
        };
        assert!(!should_remove(&timed, &keep_timed));
        assert!(!should_remove(&aura, &keep_aura));
        assert!(!should_remove(&timed, &keep_aura));
        assert!(!should_remove(&aura, &keep_timed));

        let require_resist = RemoveOptions {
            remove_positive: true,
            remove_negative: false,
            magic: true,
            physical: true,
            timed_life: true,
            aura: true,
            auto_dispel: false,
        };
        assert!(!should_remove(&magic, &require_resist));
        assert!(!should_remove(&physical, &require_resist));
        let require_magic = RemoveOptions {
            physical: false,
            ..require_resist
        };
        assert!(should_remove(&magic, &require_magic));
        assert!(!should_remove(&physical, &require_magic));

        let require_auto = RemoveOptions {
            auto_dispel: true,
            ..keep_timed
        };
        assert!(should_remove(&dispellable, &require_auto));
        assert!(!should_remove(&opaque, &require_auto));
        assert!(should_remove(&opaque, &keep_timed));
    }
}
