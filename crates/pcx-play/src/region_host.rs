//! Region rectangle history and membership. Selected-image cell rules apply
//! ordered geometric additions/clears; unqualified hosts retain continuous bounds.
use super::super::{Entry, Error, Failure};
use super::{ObjectHost, trigger_admission::loaded_type};
use pcx_script::{HandleId, Value, ast::Expr};
use std::num::NonZeroU64;

const CREATE: &str = "CreateRegion";
const ADD: &str = "RegionAddRect";
const REMOVE: &str = "RemoveRegion";
const CLEAR: &str = "RegionClearRect";
const POINT: &str = "IsPointInRegion";
const LOCATION: &str = "IsLocationInRegion";
const UNIT: &str = "IsUnitInRegion";
const ADD_CELL: &str = "RegionAddCell";
const ADD_CELL_LOCATION: &str = "RegionAddCellAtLoc";
const CLEAR_CELL: &str = "RegionClearCell";
const CLEAR_CELL_LOCATION: &str = "RegionClearCellAtLoc";
const ENTER: &str = "TriggerRegisterEnterRegion";
const LEAVE: &str = "TriggerRegisterLeaveRegion";

#[derive(Clone, Copy, Debug)]
pub struct RegionBudget {
    pub max_regions: usize,
    /// Aggregate rectangle additions; selected cell geometry also bounds all edits.
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

#[derive(Clone, Copy)]
struct CellEdit {
    region: HandleId,
    bounds: [f32; 4],
    cells: [i32; 4],
    added: bool,
}

pub(super) struct Admission {
    budget: RegionBudget,
    regions: usize,
    additions: usize,
    events: usize,
    retain_registration_order: bool,
    registration_order: Option<Box<RegistrationOrder>>,
    cell_edits: Vec<CellEdit>,
    event_ids: [Option<HandleId>; 2],
}

impl ObjectHost {
    /// Admit selected creation, rectangle-addition, removal, clearing,
    /// point-membership, optional cell edits and enter-region contracts. Cell
    /// edits require selected numeric cell rules at execution. Leave-region registration
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
            .and_then(|_| std::alloc::Layout::array::<CellEdit>(budget.max_rect_additions))
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
            (LOCATION, &["region", "location"][..], "boolean", "region"),
            (UNIT, &["region", "unit"][..], "boolean", "region"),
            (
                ADD_CELL,
                &["region", "real", "real"][..],
                "nothing",
                "region",
            ),
            (
                CLEAR_CELL,
                &["region", "real", "real"][..],
                "nothing",
                "region",
            ),
            (
                ADD_CELL_LOCATION,
                &["region", "location"][..],
                "nothing",
                "region",
            ),
            (
                CLEAR_CELL_LOCATION,
                &["region", "location"][..],
                "nothing",
                "region",
            ),
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
            if matches!(
                name,
                LEAVE
                    | REMOVE
                    | CLEAR
                    | POINT
                    | LOCATION
                    | UNIT
                    | ADD_CELL
                    | CLEAR_CELL
                    | ADD_CELL_LOCATION
                    | CLEAR_CELL_LOCATION
            ) && self.inner.program.natives().get(name).is_none()
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
        if self.inner.program.natives().get(UNIT).is_some() {
            loaded_type(&self.inner, UNIT, "unit", "widget", source)?;
        }
        for name in [LOCATION, ADD_CELL_LOCATION, CLEAR_CELL_LOCATION] {
            if self.inner.program.natives().get(name).is_some() {
                loaded_type(&self.inner, name, "location", "agent", source)?;
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
        let event_ids = [
            self.read_region_event_id(ENTER, "EVENT_GAME_ENTER_REGION")?,
            if self.inner.program.natives().get(LEAVE).is_some() {
                self.read_region_event_id(LEAVE, "EVENT_GAME_LEAVE_REGION")?
            } else {
                None
            },
        ];
        self.regions = Some(Admission {
            budget,
            regions: 0,
            additions: 0,
            events: 0,
            retain_registration_order: false,
            registration_order: None,
            cell_edits: Vec::new(),
            event_ids,
        });
        Ok(self)
    }

    fn read_region_event_id(
        &mut self,
        native: &str,
        global_name: &str,
    ) -> Result<Option<HandleId>, Error> {
        let fail = |reason| self.inner.error(native, reason);
        let Some(global) = self
            .inner
            .program
            .globals()
            .iter()
            .find(|g| g.name == global_name)
        else {
            return Ok(None);
        };
        let source = &self
            .inner
            .program
            .natives()
            .get(native)
            .expect("checked region declaration")
            .source;
        if !global.is_constant
            || global.array
            || global.ty.array
            || global.ty.kind.name() != "gameevent"
            || global.source != *source
        {
            return Err(fail(Failure::Initializer(global_name.into())));
        }
        Self::contract_in(
            &self.inner,
            "ConvertGameEvent",
            &["integer"],
            "gameevent",
            true,
            None,
            false,
        )?;
        if self
            .inner
            .program
            .natives()
            .get("ConvertGameEvent")
            .is_none_or(|n| n.source != *source)
        {
            return Err(fail(Failure::Source));
        }
        loaded_type(&self.inner, native, "gameevent", "eventid", source)?;
        let Some(Expr::Call { callee, args, .. }) = &global.init else {
            return Err(fail(Failure::Initializer(global_name.into())));
        };
        if !matches!(callee.as_ref(), Expr::Ident(name, _) if name.as_str() == "ConvertGameEvent")
            || args.len() != 1
        {
            return Err(fail(Failure::Initializer(global_name.into())));
        }
        let number = super::super::integer(&args[0], 64, &mut 4096).map_err(fail)?;
        let Value::Handle(event) = self
            .inner
            .convert_for("ConvertGameEvent", &[Value::Int(number)])?
        else {
            unreachable!("converted game event")
        };
        Ok(Some(event))
    }

    pub(super) fn region_event_id(&self, kind: RegionEventKind) -> Option<HandleId> {
        self.regions
            .as_ref()
            .and_then(|r| r.event_ids[usize::from(kind == RegionEventKind::Leave)])
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
    /// Unqualified clearing rewrites indices; selected cell edits retain the history.
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
                    self.region_rect_requests(region)
                        .ok()?
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
    /// physically removed only by unqualified handle-identity clearing.
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
            LOCATION if self.inner.program.natives().get(LOCATION).is_some() => {
                Some(self.is_location_in_region(args))
            }
            UNIT if self.inner.program.natives().get(UNIT).is_some() => {
                Some(self.is_unit_in_region(args))
            }
            ADD_CELL | CLEAR_CELL | ADD_CELL_LOCATION | CLEAR_CELL_LOCATION
                if self.inner.program.natives().get(name).is_some() =>
            {
                Some(self.edit_region_cell(name, args))
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
    fn edit_region_cell(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let [Value::Handle(region), rest @ ..] = args else {
            return Err(self.inner.error(
                name,
                Failure::RegionState("expected region and cell coordinates or location"),
            ));
        };
        self.region_rect_requests(*region)
            .map_err(|e| self.inner.error(name, e.failure))?;
        let [x, y] = if matches!(name, ADD_CELL_LOCATION | CLEAR_CELL_LOCATION) {
            let [location] = rest else {
                return Err(self
                    .inner
                    .error(name, Failure::RegionState("expected region and location")));
            };
            self.location_point(location, name)?
        } else {
            let [x, y] = rest else {
                return Err(self.inner.error(
                    name,
                    Failure::RegionState("expected region and two finite coordinates"),
                ));
            };
            let (Some(x), Some(y)) = (x.as_real(), y.as_real()) else {
                return Err(self.inner.error(
                    name,
                    Failure::RegionState("expected region and two finite coordinates"),
                ));
            };
            if !x.is_finite() || !y.is_finite() {
                return Err(self.inner.error(
                    name,
                    Failure::RegionState("expected region and two finite coordinates"),
                ));
            }
            [x, y]
        };
        let added = matches!(name, ADD_CELL | ADD_CELL_LOCATION);
        let bounds = [x, y, x, y];
        let edit = self
            .prepare_region_cell_edit(*region, bounds, added, name)?
            .ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::RegionState("source-backed region cell rules required"),
                )
            })?;
        self.regions
            .as_mut()
            .expect("admitted region")
            .cell_edits
            .push(edit);
        if let Some(pool) = self
            .unit_placement
            .as_mut()
            .and_then(crate::unit_placement::Placement::spatial_mut)
        {
            if added {
                pool.region_added(*region, bounds);
            } else {
                pool.region_cells_cleared(*region, bounds);
            }
        }
        Ok(Value::Null)
    }

    fn prepare_region_cell_edit(
        &mut self,
        region: HandleId,
        bounds: [f32; 4],
        added: bool,
        name: &str,
    ) -> Result<Option<CellEdit>, Error> {
        let Some(rules) = self.numeric.as_ref().and_then(|n| n.region_cells()) else {
            return Ok(None);
        };
        let world = self.world_bounds.ok_or_else(|| {
            self.inner.error(
                name,
                Failure::RegionState("source world extent required for cell geometry"),
            )
        })?;
        let cells = rules
            .cover(bounds, [world[0], world[1]])
            .map_err(|why| self.inner.error(name, Failure::ScriptMath(why)))?;
        let admission = self.regions.as_mut().expect("admitted region");
        if admission.cell_edits.len() >= admission.budget.max_rect_additions {
            return Err(self.inner.error(name, Failure::Budget("region cell edits")));
        }
        if admission.cell_edits.try_reserve(1).is_err() {
            return Err(self.inner.error(name, Failure::Capacity));
        }
        Ok(Some(CellEdit {
            region,
            bounds,
            cells,
            added,
        }))
    }

    pub(crate) fn region_cell_edits(
        &self,
    ) -> Option<impl Iterator<Item = (HandleId, [f32; 4], bool)> + '_> {
        self.numeric.as_ref()?.region_cells()?;
        Some(
            self.regions
                .as_ref()?
                .cell_edits
                .iter()
                .filter(|e| matches!(self.inner.entry(e.region), Ok(Entry::Region { .. })))
                .map(|e| (e.region, e.bounds, e.added)),
        )
    }

    pub(super) fn region_contains_point(
        &self,
        region: HandleId,
        point: [f32; 2],
        name: &str,
    ) -> Result<bool, Error> {
        let additions = self
            .region_rect_requests(region)
            .map_err(|e| self.inner.error(name, e.failure))?;
        if let Some(rules) = self.numeric.as_ref().and_then(|n| n.region_cells()) {
            let world = self.world_bounds.ok_or_else(|| {
                self.inner.error(
                    name,
                    Failure::RegionState("source world extent required for cell geometry"),
                )
            })?;
            if point[0] < world[0]
                || point[1] < world[1]
                || point[0] >= world[2]
                || point[1] >= world[3]
            {
                return Ok(false);
            }
            let [x, y] = rules
                .point(point, [world[0], world[1]])
                .map_err(|why| self.inner.error(name, Failure::ScriptMath(why)))?;
            return Ok(self
                .regions
                .as_ref()
                .expect("admitted region")
                .cell_edits
                .iter()
                .rev()
                .find(|e| {
                    e.region == region
                        && x >= e.cells[0]
                        && y >= e.cells[1]
                        && x <= e.cells[2]
                        && y <= e.cells[3]
                })
                .is_some_and(|e| e.added));
        }
        Ok(additions.iter().any(|r| {
            point[0] >= r.bounds[0]
                && point[1] >= r.bounds[1]
                && point[0] <= r.bounds[2]
                && point[1] <= r.bounds[3]
        }))
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
        let edit = self.prepare_region_cell_edit(*region, bounds, true, ADD)?;
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
        if let Some(edit) = edit {
            admission.cell_edits.push(edit);
        }
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
        for &index in &self.inner.region_event_slots {
            if let Entry::RegisteredRegionEvent(record) = &mut self.inner.arena[index]
                && record.region == *region
            {
                record.filter = None;
            }
        }
        if let Some(pool) = self
            .unit_placement
            .as_mut()
            .and_then(crate::unit_placement::Placement::spatial_mut)
        {
            pool.region_removed(*region);
        }
        Ok(Value::Null)
    }
    /// Selected cells are cleared using current bounds, independent of identity.
    /// Unqualified hosts retain their earlier handle-identity withdrawal behavior.
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
        let bounds = self
            .rectangle_bounds(*rectangle)
            .map_err(|e| fail(e.failure))?;
        if let Some(edit) = self.prepare_region_cell_edit(*region, bounds, false, CLEAR)? {
            self.regions
                .as_mut()
                .expect("admitted region")
                .cell_edits
                .push(edit);
            if let Some(pool) = self
                .unit_placement
                .as_mut()
                .and_then(crate::unit_placement::Placement::spatial_mut)
            {
                pool.region_cells_cleared(*region, bounds);
            }
            return Ok(Value::Null);
        }
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
    /// Share membership with location queries and event polling.
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
        self.region_contains_point(*region, [x, y], POINT)
            .map(Value::Bool)
    }
    fn is_unit_in_region(&self, args: &[Value]) -> Result<Value, Error> {
        let (region, unit) = match args {
            [Value::Handle(region), Value::Handle(unit)] => (*region, *unit),
            [Value::Null, Value::Null | Value::Handle(_)] | [Value::Handle(_), Value::Null] => {
                return Ok(Value::Bool(false));
            }
            _ => {
                return Err(self.inner.error(
                    UNIT,
                    Failure::RegionState("expected region and nullable unit"),
                ));
            }
        };
        self.region_rect_requests(region)
            .map_err(|e| self.inner.error(UNIT, e.failure))?;
        let point = match self
            .inner
            .entry(unit)
            .map_err(|e| self.inner.error(UNIT, e.failure))?
        {
            Entry::Unit { record } => record.position.ok_or_else(|| {
                self.inner.error(
                    UNIT,
                    Failure::RegionState("accepted unit position required"),
                )
            })?,
            Entry::RemovedUnit {
                location: Some((at, point)),
                ..
            } if self.timer_game_time() == Some(*at) => *point,
            Entry::RemovedUnit {
                removed_at: Some(at),
                ..
            } if self.timer_game_time().is_some_and(|now| now > *at) => {
                return Ok(Value::Bool(false));
            }
            Entry::RemovedUnit { .. } => {
                return Err(self.inner.error(
                    UNIT,
                    Failure::RegionState("accepted removal clock and position required"),
                ));
            }
            _ => return Err(self.inner.error(UNIT, Failure::WrongFamily)),
        };
        self.region_contains_point(region, point, UNIT)
            .map(Value::Bool)
    }

    fn is_location_in_region(&self, args: &[Value]) -> Result<Value, Error> {
        let [Value::Handle(region), location] = args else {
            return Err(self.inner.error(
                LOCATION,
                Failure::RegionState("expected region and location"),
            ));
        };
        let [x, y] = self.location_point(location, LOCATION)?;
        self.is_point_in_region(&[Value::Handle(*region), Value::Real(x), Value::Real(y)])
            .map_err(|error| self.inner.error(LOCATION, error.failure))
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
        let members = self.current_region_units(*region)?;
        let id = self
            .inner
            .allocate(Entry::RegisteredRegionEvent(record), name)?;
        self.regions.as_mut().expect("checked admission").events += 1;
        self.seed_region_event(id, members);
        Ok(Value::Handle(id))
    }
}

#[cfg(test)]
#[path = "region_host_tests.rs"]
mod tests;
