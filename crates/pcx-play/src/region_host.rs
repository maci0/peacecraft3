//! Region requests, removal and continuous-rectangle membership on the
//! existing arena. Regions store rectangle identities and their call-time
//! bounds; there is no cell raster. Enter/leave delivery and point queries
//! use inclusive continuous containment over those bounds. Native cell-cover
//! equivalence stays unestablished (see `docs/re-notes/slice-regions.md`).
use super::super::{Entry, Error, Failure};
use super::{ObjectHost, trigger_admission::loaded_type};
use pcx_script::{HandleId, Value};
use std::num::NonZeroU64;

const CREATE: &str = "CreateRegion";
const ADD: &str = "RegionAddRect";
const REMOVE: &str = "RemoveRegion";
const CLEAR: &str = "RegionClearRect";
const POINT: &str = "IsPointInRegion";
const ENTER: &str = "TriggerRegisterEnterRegion";
const LEAVE: &str = "TriggerRegisterLeaveRegion";

#[derive(Clone, Copy, Debug)]
pub struct RegionBudget {
    pub max_regions: usize,
    /// Aggregate call records across all regions, including repeated additions.
    pub max_rect_additions: usize,
    pub max_events: usize,
}

/// Executed rectangle identity and bounds at the call, not a native cell cover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionRectRequest {
    pub rectangle: HandleId,
    pub bounds: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionEventKind {
    Enter,
    Leave,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionEventRegistration {
    pub trigger: HandleId,
    pub region: HandleId,
    pub filter: Option<HandleId>,
    pub kind: RegionEventKind,
}

enum RegistrationOrder {
    Compact(Vec<u64>),
    Wide(Vec<(HandleId, usize)>),
}

impl RegistrationOrder {
    fn len(&self) -> usize {
        match self {
            Self::Compact(order) => order.len(),
            Self::Wide(order) => order.len(),
        }
    }

    fn get(&self, index: usize) -> (HandleId, usize) {
        match self {
            Self::Compact(order) => {
                let packed = order[index];
                (
                    HandleId::new(NonZeroU64::new(packed >> 32).expect("recorded region")),
                    packed as u32 as usize,
                )
            }
            Self::Wide(order) => order[index],
        }
    }
}

pub(super) struct Admission {
    budget: RegionBudget,
    regions: usize,
    additions: usize,
    events: usize,
    retain_registration_order: bool,
    registration_order: Option<Box<RegistrationOrder>>,
}

impl ObjectHost {
    /// Admit selected creation, rectangle-addition, removal, clearing,
    /// point-membership and enter-region contracts. Leave-region registration
    /// needs its own loaded declaration. Removal, clearing and point queries
    /// are optional: a map that does not declare them keeps the older
    /// admission, like the existing leave-event rule.
    pub fn with_regions(mut self, budget: RegionBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        if self.regions.is_some() {
            return Err(fail(Failure::RegionState("already admitted")));
        }
        if !self.rectangles || self.triggers.is_none() {
            return Err(fail(Failure::RegionState(
                "rectangles and triggers must be admitted",
            )));
        }
        std::alloc::Layout::array::<Vec<RegionRectRequest>>(budget.max_regions)
            .and_then(|_| std::alloc::Layout::array::<RegionRectRequest>(budget.max_rect_additions))
            .and_then(|_| std::alloc::Layout::array::<RegionEventRegistration>(budget.max_events))
            .map_err(|_| fail(Failure::Capacity))?;
        let source = &self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .expect("checked host")
            .source;
        for (name, params, result, family) in [
            (CREATE, &[][..], "region", "region"),
            (ADD, &["region", "rect"][..], "nothing", "region"),
            (REMOVE, &["region"][..], "nothing", "region"),
            (CLEAR, &["region", "rect"][..], "nothing", "region"),
            (POINT, &["region", "real", "real"][..], "boolean", "region"),
            (
                ENTER,
                &["trigger", "region", "boolexpr"][..],
                "event",
                "event",
            ),
            (
                LEAVE,
                &["trigger", "region", "boolexpr"][..],
                "event",
                "event",
            ),
        ] {
            if (name == LEAVE || name == REMOVE || name == CLEAR || name == POINT)
                && self.inner.program.natives().get(name).is_none()
            {
                continue;
            }
            Self::contract(&self.inner, name, params, result, false, Some(family))?;
            if self
                .inner
                .program
                .natives()
                .get(name)
                .is_none_or(|n| &n.source != source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        for name in ["Rect", "CreateTrigger"] {
            if self
                .inner
                .program
                .natives()
                .get(name)
                .is_none_or(|n| &n.source != source)
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        for name in ["region", "rect", "boolexpr"] {
            loaded_type(&self.inner, CREATE, name, "agent", source)?;
        }
        self.regions = Some(Admission {
            budget,
            regions: 0,
            additions: 0,
            events: 0,
            retain_registration_order: false,
            registration_order: None,
        });
        Ok(self)
    }

    /// Retain the global order of rectangle additions from before map initialization.
    pub(crate) fn enable_region_registration_order(&mut self) -> Result<(), Error> {
        let Some(admission) = self.regions.as_ref() else {
            return Ok(());
        };
        if admission.additions != 0 || admission.retain_registration_order {
            return Err(self.inner.error(
                CREATE,
                Failure::RegionState("region history must start before additions"),
            ));
        }
        self.regions
            .as_mut()
            .expect("checked admission")
            .retain_registration_order = true;
        Ok(())
    }

    pub(crate) fn region_registration_order_enabled(&self) -> bool {
        self.regions
            .as_ref()
            .is_some_and(|admission| admission.retain_registration_order)
    }

    /// Enumerate additions in execution order, retaining their call-time bounds.
    /// Additions of removed regions are skipped: removal keeps their bounds
    /// for the retained counts but they no longer describe a live region.
    /// Clearing rewrites this history so surviving indices stay dense.
    pub fn region_registrations(
        &self,
    ) -> Result<impl Iterator<Item = Result<(HandleId, RegionRectRequest), Error>> + '_, Error>
    {
        let admission = self.region_admission()?;
        if !admission.retain_registration_order {
            return Err(self.inner.error(
                CREATE,
                Failure::RegionState("region registration history was not enabled"),
            ));
        }
        let order = admission.registration_order.as_deref();
        Ok(
            (0..order.map_or(0, RegistrationOrder::len)).filter_map(move |position| {
                let (region, index) = order.expect("nonempty history").get(position);
                if matches!(self.inner.entry(region), Ok(Entry::RemovedRegion { .. })) {
                    return None;
                }
                Some(
                    self.region_rect_requests(region)?
                        .get(index)
                        .copied()
                        .map(|request| (region, request))
                        .ok_or_else(|| {
                            self.inner.error(
                                CREATE,
                                Failure::RegionState("region registration history is inconsistent"),
                            )
                        }),
                )
            }),
        )
    }

    fn region_admission(&self) -> Result<&Admission, Error> {
        self.regions.as_ref().ok_or_else(|| {
            self.inner
                .error(CREATE, Failure::RegionState("not admitted"))
        })
    }
    pub fn retained_regions(&self) -> Result<usize, Error> {
        Ok(self.region_admission()?.regions)
    }
    pub fn retained_region_rect_requests(&self) -> Result<usize, Error> {
        Ok(self.region_admission()?.additions)
    }
    pub fn retained_region_events(&self) -> Result<usize, Error> {
        Ok(self.region_admission()?.events)
    }

    /// Enumerate retained live regions and the rectangle additions executed
    /// for each. Removed regions are skipped; cleared rectangles were
    /// physically removed from the retained lists.
    pub fn executed_regions(
        &self,
    ) -> Result<impl Iterator<Item = (HandleId, &[RegionRectRequest])> + '_, Error> {
        self.region_admission()?;
        Ok(self
            .inner
            .arena
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                if let Entry::Region { additions } = entry {
                    Some((
                        self.inner.id(i, CREATE).expect("allocated identity"),
                        additions.as_slice(),
                    ))
                } else {
                    None
                }
            }))
    }

    pub fn region_rect_requests(&self, id: HandleId) -> Result<&[RegionRectRequest], Error> {
        self.region_admission()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(CREATE, e.failure))?
        {
            Entry::Region { additions } => Ok(additions),
            Entry::RemovedRegion { .. } => Err(self.inner.error(CREATE, Failure::Identity)),
            _ => Err(self.inner.error(CREATE, Failure::WrongFamily)),
        }
    }
    pub fn region_event_registration(
        &self,
        id: HandleId,
    ) -> Result<RegionEventRegistration, Error> {
        self.region_admission()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(ENTER, e.failure))?
        {
            Entry::RegisteredRegionEvent(record) => Ok(*record),
            _ => Err(self.inner.error(ENTER, Failure::WrongFamily)),
        }
    }
    pub(super) fn region_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.regions.as_ref()?;
        match name {
            CREATE => Some(self.create_region(args)),
            ADD => Some(self.region_add_rect(args)),
            REMOVE if self.inner.program.natives().get(REMOVE).is_some() => {
                Some(self.remove_region(args))
            }
            CLEAR if self.inner.program.natives().get(CLEAR).is_some() => {
                Some(self.clear_region_rect(args))
            }
            POINT if self.inner.program.natives().get(POINT).is_some() => {
                Some(self.is_point_in_region(args))
            }
            ENTER => Some(self.register_region_event(name, args)),
            LEAVE if self.inner.program.natives().get(LEAVE).is_some() => {
                Some(self.register_region_event(name, args))
            }
            _ => None,
        }
    }
    fn create_region(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(CREATE, reason);
        if !args.is_empty() {
            return Err(fail(Failure::RegionState("expected no arguments")));
        }
        let a = self.region_admission()?;
        if a.regions >= a.budget.max_regions {
            return Err(fail(Failure::Budget("region records")));
        }
        let id = self.inner.allocate(
            Entry::Region {
                additions: Vec::new(),
            },
            CREATE,
        )?;
        self.regions.as_mut().expect("checked admission").regions += 1;
        Ok(Value::Handle(id))
    }
    fn region_add_rect(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(ADD, reason);
        let [Value::Handle(region), Value::Handle(rectangle)] = args else {
            return Err(fail(Failure::RegionState("expected region and rectangle")));
        };
        let index = self
            .region_rect_requests(*region)
            .map_err(|e| fail(e.failure))?
            .len();
        let bounds = self
            .rectangle_bounds(*rectangle)
            .map_err(|e| fail(e.failure))?;
        let a = self.region_admission()?;
        if a.additions >= a.budget.max_rect_additions {
            return Err(fail(Failure::Budget("region rectangle additions")));
        }
        let retain = a.retain_registration_order;
        let compact = self.inner.max_handles as u64 <= u64::from(u32::MAX)
            && a.budget.max_rect_additions as u64 <= u64::from(u32::MAX);
        let packed = if retain && compact {
            let token = u32::try_from(region.token()).map_err(|_| fail(Failure::Capacity))?;
            let index = u32::try_from(index).map_err(|_| fail(Failure::Capacity))?;
            Some((u64::from(token) << 32) | u64::from(index))
        } else {
            None
        };
        if retain {
            let admission = self.regions.as_mut().expect("checked admission");
            let order = admission.registration_order.get_or_insert_with(|| {
                Box::new(if compact {
                    RegistrationOrder::Compact(Vec::new())
                } else {
                    RegistrationOrder::Wide(Vec::new())
                })
            });
            let reserved = match order.as_mut() {
                RegistrationOrder::Compact(order) => order.try_reserve(1),
                RegistrationOrder::Wide(order) => order.try_reserve(1),
            };
            if reserved.is_err() {
                return Err(self.inner.error(ADD, Failure::Capacity));
            }
        }
        let Entry::Region { additions } = self.inner.entry_mut(*region).expect("checked region")
        else {
            unreachable!("checked region")
        };
        if additions.try_reserve(1).is_err() {
            return Err(self.inner.error(ADD, Failure::Capacity));
        }
        additions.push(RegionRectRequest {
            rectangle: *rectangle,
            bounds,
        });
        let admission = self.regions.as_mut().expect("checked admission");
        match admission.registration_order.as_deref_mut() {
            Some(RegistrationOrder::Compact(order)) => order.push(packed.expect("compact history")),
            Some(RegistrationOrder::Wide(order)) => order.push((*region, index)),
            None => {}
        }
        admission.additions += 1;
        // Additions before the placement cache exists are replayed when it is built.
        if let Some(pool) = self
            .unit_placement
            .as_mut()
            .and_then(crate::unit_placement::Placement::spatial_mut)
        {
            pool.region_added(*region, bounds);
        }
        Ok(Value::Null)
    }
    /// Retire a live region. Its call-time bounds stay retained for the
    /// registration history and the aggregate counts, but membership, new
    /// additions and event registration refuse afterward. A second removal
    /// refuses like a second `RemoveRect`.
    fn remove_region(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(REMOVE, reason);
        let [Value::Handle(region)] = args else {
            return Err(fail(Failure::RegionState("expected region")));
        };
        match self.inner.entry(*region).map_err(|e| fail(e.failure))? {
            Entry::Region { .. } => {}
            Entry::RemovedRegion { .. } => return Err(fail(Failure::Identity)),
            _ => return Err(fail(Failure::WrongFamily)),
        }
        let at = usize::try_from(region.token() - 1)
            .map_err(|_| self.inner.error(REMOVE, Failure::Identity))?;
        let taken = match self.inner.entry_mut(*region).expect("checked region") {
            Entry::Region { additions } => std::mem::take(additions),
            _ => unreachable!("checked region"),
        };
        self.inner.arena[at] = Entry::RemovedRegion { additions: taken };
        if let Some(pool) = self
            .unit_placement
            .as_mut()
            .and_then(crate::unit_placement::Placement::spatial_mut)
        {
            pool.region_removed(*region);
        }
        Ok(Value::Null)
    }
    /// Drop every stored addition that references the given rectangle handle.
    /// This is handle-identity removal, not a geometric subtraction: the
    /// native cell cover of an arbitrary clear rectangle is unmeasured (see
    /// `docs/re-notes/slice-regions.md`), so only the exact rectangle passed
    /// to `RegionAddRect` is withdrawn. A region with no such addition is a
    /// silent no-op. The registration history is rewritten so surviving
    /// indices stay dense.
    fn clear_region_rect(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(CLEAR, reason);
        let [Value::Handle(region), Value::Handle(rectangle)] = args else {
            return Err(fail(Failure::RegionState("expected region and rectangle")));
        };
        match self.inner.entry(*region).map_err(|e| fail(e.failure))? {
            Entry::Region { .. } => {}
            Entry::RemovedRegion { .. } => return Err(fail(Failure::Identity)),
            _ => return Err(fail(Failure::WrongFamily)),
        }
        // Both handles are validated before any retained list changes.
        self.rectangle_bounds(*rectangle)
            .map_err(|e| fail(e.failure))?;
        let len = self
            .region_rect_requests(*region)
            .expect("checked region")
            .len();
        for index in (0..len).rev() {
            let request = self.region_rect_requests(*region).expect("checked region")[index];
            if request.rectangle != *rectangle {
                continue;
            }
            let Entry::Region { additions } =
                self.inner.entry_mut(*region).expect("checked region")
            else {
                unreachable!("checked region")
            };
            additions.remove(index);
            self.forget_registration(*region, index);
            if let Some(pool) = self
                .unit_placement
                .as_mut()
                .and_then(crate::unit_placement::Placement::spatial_mut)
            {
                pool.region_rect_cleared(*region, request.bounds);
            }
        }
        Ok(Value::Null)
    }
    /// Rewrite the global registration history after clearing one addition:
    /// positions pointing at the cleared index are dropped and later indices
    /// of the same region shift down. Clearing is rare, so this stays a
    /// linear rewrite rather than a second index.
    fn forget_registration(&mut self, region: HandleId, index: usize) {
        let Some(order) = self
            .regions
            .as_mut()
            .expect("checked admission")
            .registration_order
            .as_deref_mut()
        else {
            return;
        };
        match order {
            RegistrationOrder::Compact(packed) => {
                packed.retain(|entry| {
                    (*entry >> 32) != region.token() || (*entry as u32 as usize) != index
                });
                for entry in packed.iter_mut() {
                    if (*entry >> 32) == region.token() && (*entry as u32 as usize) > index {
                        *entry -= 1;
                    }
                }
            }
            RegistrationOrder::Wide(entries) => {
                entries.retain(|(held, held_index)| *held != region || *held_index != index);
                for (held, held_index) in entries.iter_mut() {
                    if *held == region && *held_index > index {
                        *held_index -= 1;
                    }
                }
            }
        }
    }
    /// Inclusive continuous containment over the retained call-time bounds,
    /// the same predicate the enter/leave delivery uses. Whether the native
    /// tests this rectangle or its unmeasured cell cover stays open.
    fn is_point_in_region(&self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(POINT, reason);
        let [Value::Handle(region), x, y] = args else {
            return Err(fail(Failure::RegionState(
                "expected region and two coordinates",
            )));
        };
        let (Some(x), Some(y)) = (x.as_real(), y.as_real()) else {
            return Err(fail(Failure::RegionState(
                "expected region and two finite coordinates",
            )));
        };
        if !x.is_finite() || !y.is_finite() {
            return Err(fail(Failure::RegionState(
                "expected region and two finite coordinates",
            )));
        }
        let additions = self
            .region_rect_requests(*region)
            .map_err(|e| fail(e.failure))?;
        let inside = additions.iter().any(|request| {
            let [min_x, min_y, max_x, max_y] = request.bounds;
            x >= min_x && x <= max_x && y >= min_y && y <= max_y
        });
        Ok(Value::Bool(inside))
    }
    fn register_region_event(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(name, reason);
        let [Value::Handle(trigger), Value::Handle(region), filter] = args else {
            return Err(fail(Failure::RegionState(
                "expected trigger, region and nullable filter",
            )));
        };
        if !matches!(
            self.inner.entry(*trigger).map_err(|e| fail(e.failure))?,
            Entry::Trigger { .. }
        ) {
            return Err(fail(Failure::WrongFamily));
        }
        self.region_rect_requests(*region)
            .map_err(|e| fail(e.failure))?;
        let filter = match filter {
            Value::Null => None,
            Value::Handle(id) => {
                self.checked_boolean_wrapper(*id, name)?;
                Some(*id)
            }
            _ => {
                return Err(fail(Failure::RegionState(
                    "expected nullable filter identity",
                )));
            }
        };
        let a = self.region_admission()?;
        if a.events >= a.budget.max_events {
            return Err(fail(Failure::Budget("region event registrations")));
        }
        let record = RegionEventRegistration {
            trigger: *trigger,
            region: *region,
            filter,
            kind: if name == ENTER {
                RegionEventKind::Enter
            } else {
                RegionEventKind::Leave
            },
        };
        let id = self
            .inner
            .allocate(Entry::RegisteredRegionEvent(record), name)?;
        self.regions.as_mut().expect("checked admission").events += 1;
        Ok(Value::Handle(id))
    }
}

#[cfg(test)]
#[path = "region_host_tests.rs"]
mod tests;
