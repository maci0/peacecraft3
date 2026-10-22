//! Active per-unit buff state and the source-backed `UnitRemoveBuffs` /
//! `UnitRemoveBuffsEx` removal filters.
//!
//! Buff identity is the shipped `Units\AbilityBuffData.slk` rawcode. Polarity,
//! resistance, timed-life, aura and auto-dispel classification are native
//! predicates on the buff class rather than SLK cells (the buff tables carry
//! `alias`, `code`, `isEffect`, `race`, and editor metadata only): the selected
//! client's shared removal helper `0x6D87A0` evaluates vtable slots `0x1E0`
//! (negative), `0x1E4` (positive), `0x1E8` (aura), `0x334` (timed life),
//! `0x1D8` (magic), `0x1DC` (physical), and `0x338` (auto-dispel) while
//! walking the unit's ability chain, and removes each matching buff. The basic
//! `UnitRemoveBuffs` wrapper calls the helper with timed-life/aura enabled and
//! magic/physical/auto-dispel disabled; `UnitRemoveBuffsEx` passes its own
//! booleans in the helper's stack order (physical, magic, timed-life, aura,
//! auto-dispel, remove-positive, remove-negative).
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
            "unit", "boolean", "boolean", "boolean", "boolean", "boolean",
            "boolean", "boolean",
        ],
        "nothing",
    ),
];

/// Native classification of one active buff, matching the shared removal
/// helper's predicate slots. Consumers that apply a buff carry its rawcode and
/// these traits from the behavior that adds it; the buff tables' `code`
/// column remains the source of the rawcode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BuffTraits {
    pub positive: bool,
    pub negative: bool,
    pub magic: bool,
    pub physical: bool,
    pub timed_life: bool,
    pub aura: bool,
    pub auto_dispellable: bool,
}

/// One active buff on a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveBuff {
    rawcode: [u8; 4],
    traits: BuffTraits,
}

impl ActiveBuff {
    pub const fn new(rawcode: [u8; 4], traits: BuffTraits) -> Self {
        Self { rawcode, traits }
    }

    /// The shipped `Units\AbilityBuffData.slk` `code` cell spelling this buff.
    pub const fn rawcode(self) -> [u8; 4] {
        self.rawcode
    }

    pub const fn traits(self) -> BuffTraits {
        self.traits
    }
}

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

    fn live_unit(&self, native: &str, unit: HandleId) -> Result<(), Error> {
        let fail = |failure| self.inner.error(native, failure);
        match self.inner.entry(unit).map_err(|e| fail(e.failure))? {
            Entry::RemovedUnit | Entry::Unit { .. } => Ok(()),
            _ => Err(fail(Failure::WrongFamily)),
        }
    }

    /// Attach an active buff to a live unit. Future buff-applying effect hosts
    /// call this with the shipped buff rawcode and the class traits above.
    pub fn apply_unit_buff(&mut self, unit: HandleId, buff: ActiveBuff) -> Result<(), Error> {
        self.live_unit("unit buff", unit)?;
        self.unit_buffs.active.entry(unit).or_default().push(buff);
        Ok(())
    }

    /// The active buffs on a live unit, most recently applied first.
    pub fn active_unit_buffs(&self, unit: HandleId) -> Result<Vec<ActiveBuff>, Error> {
        self.live_unit("unit buff", unit)?;
        Ok(self
            .unit_buffs
            .active
            .get(&unit)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .to_vec())
    }

    pub(super) fn forget_unit_buffs(&mut self, unit: HandleId) {
        self.unit_buffs.active.remove(&unit);
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
                (REMOVE, [unit, Value::Bool(positive), Value::Bool(negative)]) => (
                    unit.clone(),
                    RemoveOptions::basic(*positive, *negative),
                ),
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
                Entry::RemovedUnit => return Ok(Value::Null),
                Entry::Unit { .. } => {}
                _ => return Err(self.inner.error(name, Failure::WrongFamily)),
            }
            if let Some(active) = self.unit_buffs.active.get_mut(&id) {
                active.retain(|buff| !should_remove(buff, &options));
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
        let positive = buff(*b"BHab", BuffTraits { positive: true, ..Default::default() });
        let negative = buff(*b"Bcor", BuffTraits { negative: true, ..Default::default() });
        let neither = buff(*b"Bncl", BuffTraits::default());
        let both = buff(*b"Bfro", BuffTraits { positive: true, negative: true, ..Default::default() });

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
        let timed = buff(*b"BTLf", BuffTraits { positive: true, timed_life: true, ..Default::default() });
        let aura = buff(*b"BHab", BuffTraits { positive: true, aura: true, ..Default::default() });
        let magic = buff(*b"Bmgs", BuffTraits { positive: true, magic: true, ..Default::default() });
        let physical = buff(*b"Bphy", BuffTraits { positive: true, physical: true, ..Default::default() });
        let dispellable = buff(*b"Bdis", BuffTraits { positive: true, auto_dispellable: true, ..Default::default() });
        let opaque = buff(*b"Bnod", BuffTraits { positive: true, ..Default::default() });

        let keep_timed = RemoveOptions {
            remove_positive: true,
            remove_negative: false,
            magic: false,
            physical: false,
            timed_life: false,
            aura: false,
            auto_dispel: false,
        };
        let keep_aura = RemoveOptions { timed_life: false, aura: false, ..keep_timed };
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
        let require_magic = RemoveOptions { physical: false, ..require_resist };
        assert!(should_remove(&magic, &require_magic));
        assert!(!should_remove(&physical, &require_magic));

        let require_auto = RemoveOptions { auto_dispel: true, ..keep_timed };
        assert!(should_remove(&dispellable, &require_auto));
        assert!(!should_remove(&opaque, &require_auto));
        assert!(should_remove(&opaque, &keep_timed));
    }
}
