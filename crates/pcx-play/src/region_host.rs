//! Region requests and subscriptions on the existing arena; no cell raster or delivery.
use super::super::{Entry, Error, Failure};
use super::{ObjectHost, trigger_admission::loaded_type};
use pcx_script::{HandleId, Value};

const CREATE: &str = "CreateRegion";
const ADD: &str = "RegionAddRect";
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

pub(super) struct Admission {
    budget: RegionBudget,
    regions: usize,
    additions: usize,
    events: usize,
}

impl ObjectHost {
    /// Admit selected creation, rectangle-addition and enter-region contracts.
    /// Leave-region registration needs its own loaded declaration.
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
            if name == LEAVE && self.inner.program.natives().get(name).is_none() {
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
        });
        Ok(self)
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

    pub fn region_rect_requests(&self, id: HandleId) -> Result<&[RegionRectRequest], Error> {
        self.region_admission()?;
        match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(CREATE, e.failure))?
        {
            Entry::Region { additions } => Ok(additions),
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
        self.region_rect_requests(*region)
            .map_err(|e| fail(e.failure))?;
        let bounds = self
            .rectangle_bounds(*rectangle)
            .map_err(|e| fail(e.failure))?;
        let a = self.region_admission()?;
        if a.additions >= a.budget.max_rect_additions {
            return Err(fail(Failure::Budget("region rectangle additions")));
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
        self.regions.as_mut().expect("checked admission").additions += 1;
        Ok(Value::Null)
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
