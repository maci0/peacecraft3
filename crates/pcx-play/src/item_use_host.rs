//! Item charges (`GetItemCharges`, `SetItemCharges`) and cooldown groups for executed items.
//!
//! Charge accessors follow the selected binary (`0x492d50`, `0x4a8de0`): a null or removed item
//! reads 0 and a write is a no-op, and a negative write stores 0. A charge count starts from the
//! source `uses` cell on first access and is then kept per item.
//!
//! Use is a synchronous two-step boundary inside one host call. `check_item_use(unit, item)` reads
//! the host game clock, checks that the unit is live and holds the item, and returns a grant. The
//! owner runs the effect and then calls `commit_item_use(grant)`. Commit requires:
//! - the grant came from this host session,
//! - the host game clock still equals the grant's time (no delay or time advance),
//! - the unit, item, holder, charge model and cooldown still match the grant.
//!
//! Any mismatch refuses and writes nothing. Commit starts a cooldown at the host clock, not at a
//! caller-supplied time. A grant is move-only, so one grant commits at most once. The owner removes
//! the item when `UseCommit::remove` is set.
use super::super::{Entry, Error, Failure};
use super::ObjectHost;
use crate::item_use_rules::{Charges, Cooldown, Rules};
use pcx_script::{HandleId, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

const GET: &str = "GetItemCharges";
const SET: &str = "SetItemCharges";

/// Identity of one admitted host. Grants hold a clone of its token and commit compares tokens by
/// pointer. No randomness or global counter is involved, and an address cannot be reused while a
/// grant still holds the token.
#[derive(Clone)]
pub(super) struct Session(Arc<()>);

impl PartialEq for Session {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Session")
    }
}

pub(super) struct ItemUse {
    rules: Rules,
    /// Items with a stored charge count, and cooldown groups with a stored ready time, are each
    /// bounded by this (the item budget's record count).
    max_records: usize,
    session: Session,
    charges: BTreeMap<HandleId, i32>,
    ready_at: BTreeMap<String, f64>,
}

/// Outcome of the readiness check. Nothing is written until `commit_item_use`.
#[derive(Debug, PartialEq)]
pub enum UseCheck {
    Ready(UseGrant),
    /// Game seconds until the item's cooldown group is ready.
    OnCooldown { remaining: f64 },
    NoCharges,
    /// The item is not held by the unit.
    NotHeld,
}

/// A checked use. Move-only. Its fields are the validated unit, item, and the host clock time.
#[derive(Debug, PartialEq)]
pub struct UseGrant {
    session: Session,
    unit: HandleId,
    item: HandleId,
    time: f64,
    charged: bool,
    cooldown: Option<Cooldown>,
    holder: Option<(HandleId, usize)>,
}

impl UseGrant {
    pub fn unit(&self) -> HandleId {
        self.unit
    }

    pub fn item(&self) -> HandleId {
        self.item
    }

    /// The host game clock at check time. Commit requires the clock to be unchanged.
    pub fn time(&self) -> f64 {
        self.time
    }
}

/// Result of a committed use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseCommit {
    /// Charges left, or `None` for an uncharged item.
    pub charges: Option<i32>,
    /// The item is perishable and charged with no charges left. The owner removes it.
    pub remove: bool,
}

impl ObjectHost {
    /// Opt into charges and cooldowns. Requires item admission and a game clock
    /// (`with_timer_runtime`), since readiness and cooldown start both read the host clock.
    pub fn with_item_use(mut self, rules: Rules, max_records: usize) -> Result<Self, Error> {
        if self.item_use.is_some() {
            return Err(self
                .inner
                .error(GET, Failure::ItemUse("item use already admitted".into())));
        }
        if self.items.is_none() {
            return Err(self
                .inner
                .error(GET, Failure::ItemUse("item use requires item admission".into())));
        }
        Self::contract(&self.inner, GET, &["item"], "integer", false, None)?;
        Self::contract(&self.inner, SET, &["item", "integer"], "nothing", false, None)?;
        self.item_use = Some(ItemUse {
            rules,
            max_records,
            session: Session(Arc::new(())),
            charges: BTreeMap::new(),
            ready_at: BTreeMap::new(),
        });
        Ok(self)
    }

    /// Readiness of `unit` using `item` at the host game clock. Refuses when the clock is not
    /// admitted, when the unit or item is absent or the wrong family, and when the item data is
    /// not derived or the item is `ignoreCD`.
    pub fn check_item_use(&self, unit: HandleId, item: HandleId) -> Result<UseCheck, Error> {
        let now = self.use_clock()?;
        self.readiness(unit, item, now)
    }

    /// Applies a checked use after its effect has run. Every check runs before the first write.
    pub fn commit_item_use(&mut self, grant: UseGrant) -> Result<UseCommit, Error> {
        let fail = |reason: String| self.inner.error("item use", Failure::ItemUse(reason));
        let session = self.item_use.as_ref().map(|s| s.session.clone());
        if session.as_ref() != Some(&grant.session) {
            return Err(fail("grant belongs to another host session".into()));
        }
        let now = self.use_clock()?;
        if now != grant.time {
            return Err(fail(format!(
                "grant expired: game clock moved from {} to {now}",
                grant.time
            )));
        }
        let current = match self.readiness(grant.unit, grant.item, now)? {
            UseCheck::Ready(current) => current,
            refused => return Err(fail(format!("stale grant: {refused:?}"))),
        };
        if current != grant {
            return Err(fail("stale grant: item state changed since check".into()));
        }
        let state = self.item_use.as_ref().expect("session matched");
        let record = self.live_item(grant.item)?.expect("ready item is live");
        let perishable = state.rules.perishable(&record.definition).map_err(fail)?;
        let remaining = if grant.charged {
            Some(self.charge_count(grant.item, record)? - 1)
        } else {
            None
        };
        if remaining.is_some()
            && !state.charges.contains_key(&grant.item)
            && state.charges.len() >= state.max_records
        {
            return Err(self.inner.error(GET, Failure::Budget("item charges")));
        }
        if let Some(cooldown) = &grant.cooldown
            && !state.ready_at.contains_key(&cooldown.group)
            && state.ready_at.len() >= state.max_records
        {
            return Err(self.inner.error(GET, Failure::Budget("item cooldown groups")));
        }
        let state = self.item_use.as_mut().expect("session matched");
        if let Some(count) = remaining {
            state.charges.insert(grant.item, count);
        }
        if let Some(cooldown) = &grant.cooldown {
            state
                .ready_at
                .insert(cooldown.group.clone(), now + cooldown.seconds);
        }
        Ok(UseCommit {
            charges: remaining,
            remove: perishable && remaining == Some(0),
        })
    }

    fn use_clock(&self) -> Result<f64, Error> {
        let now = self
            .timer_game_time()
            .ok_or_else(|| self.inner.error("item use", Failure::ItemUse("game clock is not admitted".into())))?;
        if !now.is_finite() {
            return Err(self
                .inner
                .error("item use", Failure::ItemUse("game clock is not finite".into())));
        }
        Ok(now)
    }

    fn readiness(&self, unit: HandleId, item: HandleId, now: f64) -> Result<UseCheck, Error> {
        let fail = |reason: String| self.inner.error("item use", Failure::ItemUse(reason));
        let state = self
            .item_use
            .as_ref()
            .ok_or_else(|| fail("item use is not admitted".into()))?;
        match self.inner.entry(unit)? {
            Entry::Unit { .. } => {}
            Entry::RemovedUnit => return Err(fail("absent unit".into())),
            _ => return Err(self.inner.error("item use", Failure::WrongFamily)),
        }
        let record = self
            .live_item(item)?
            .ok_or_else(|| fail("absent item".into()))?;
        if record.held_by.map(|(holder, _)| holder) != Some(unit) {
            return Ok(UseCheck::NotHeld);
        }
        let charged = matches!(
            state.rules.charges(&record.definition).map_err(fail)?,
            Charges::Charged(_)
        );
        if charged && self.charge_count(item, record)? == 0 {
            return Ok(UseCheck::NoCharges);
        }
        let cooldown = state.rules.cooldown(&record.definition).map_err(fail)?;
        if let Some(cooldown) = &cooldown {
            if cooldown.ignores_wait {
                return Err(fail("ignoreCD cooldown semantics are unverified".into()));
            }
            if let Some(ready) = state.ready_at.get(&cooldown.group).filter(|r| **r > now) {
                return Ok(UseCheck::OnCooldown {
                    remaining: ready - now,
                });
            }
        }
        Ok(UseCheck::Ready(UseGrant {
            session: state.session.clone(),
            unit,
            item,
            time: now,
            charged,
            cooldown,
            holder: record.held_by,
        }))
    }

    pub(super) fn item_use_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.item_use.is_none() || !matches!(name, GET | SET) {
            return None;
        }
        Some(if name == GET {
            self.get_charges(args)
        } else {
            self.set_charges(args)
        })
    }

    fn get_charges(&self, args: &[Value]) -> Result<Value, Error> {
        let item = match args {
            [Value::Null] => return Ok(Value::Int(0)),
            [Value::Handle(item)] => *item,
            _ => {
                return Err(self
                    .inner
                    .error(GET, Failure::ItemUse("expected one item handle".into())));
            }
        };
        let Some(record) = self.live_item(item)? else {
            return Ok(Value::Int(0));
        };
        Ok(Value::Int(self.charge_count(item, record)?))
    }

    /// Native: a null or removed item is a no-op, and a negative count stores 0.
    fn set_charges(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason: String| self.inner.error(SET, Failure::ItemUse(reason));
        let (item, count) = match args {
            [Value::Null, Value::Int(_)] => return Ok(Value::Null),
            [Value::Handle(item), Value::Int(count)] => (*item, *count),
            _ => return Err(fail("expected item handle and integer".into())),
        };
        if self.live_item(item)?.is_none() {
            return Ok(Value::Null);
        }
        let count = count.max(0);
        let state = self.item_use.as_mut().expect("dispatch requires admission");
        if !state.charges.contains_key(&item) && state.charges.len() >= state.max_records {
            return Err(self.inner.error(SET, Failure::Budget("item charges")));
        }
        state.charges.insert(item, count);
        Ok(Value::Null)
    }

    /// The stored count, else the source `uses` cell. An uncharged item reads 0.
    fn charge_count(&self, item: HandleId, record: &super::ExecutedItem) -> Result<i32, Error> {
        let state = self.item_use.as_ref().expect("dispatch requires admission");
        if let Some(count) = state.charges.get(&item) {
            return Ok(*count);
        }
        match state
            .rules
            .charges(&record.definition)
            .map_err(|e| self.inner.error(GET, Failure::ItemUse(e)))?
        {
            Charges::Uncharged => Ok(0),
            Charges::Charged(n) => i32::try_from(n).map_err(|_| {
                self.inner
                    .error(GET, Failure::ItemUse("charge count exceeds integer".into()))
            }),
        }
    }
}

#[cfg(test)]
#[path = "item_use_host_tests.rs"]
mod tests;
