//! Executed `CreateUnit` joined to the caller's already selected catalog.
//!
//! The catalog value is the same `Arc` the caller holds. This module does not
//! load SLK, overlays, or map files. Record budget and arena capacity are
//! checked before `resolve` and before the per-unit `Source` clone. Trigger
//! creation does not read preplaced owners. The integer rawcode is
//! `i32::to_be_bytes`. `base_rawcode` is stored as the catalog returned it.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_objects::editor::ObjectKind;
use pcx_script::span::Span;
use pcx_script::{HandleId, Value};
use pcx_sim::world::{Definition, ObjectCatalog, ResolutionProblem, Source};
use std::sync::Arc;

const NATIVE: &str = "CreateUnit";
const SET_RANGE: &str = "SetUnitAcquireRange";
const KIND: &str = "unit";

/// Caller bounds. Not a world, population, or rendering limit.
#[derive(Clone, Copy, Debug)]
pub struct UnitBudget {
    pub max_records: usize,
    /// Maximum `Definition.fields` retained from one successful resolve.
    pub max_retained_fields: usize,
    /// Maximum authored modifications retained from one successful resolve.
    pub max_modifications: usize,
}

/// Checked catalog result plus the executed call. `executed_owner` is the
/// player number from this call, not a preplaced control word.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutedUnit {
    pub definition: Definition,
    pub executed_owner: i32,
    pub source: Source,
    pub x: f32,
    pub y: f32,
    pub face: f32,
    /// Executed `SetUnitAcquireRange` real. `None` means the native has not
    /// run for this handle. `Some(0.0)` is an executed zero, not that absence.
    pub acquire_range: Option<f32>,
}

pub(super) struct Admission {
    catalog: Arc<dyn ObjectCatalog + Send + Sync>,
    source: Source,
    declaration_source: String,
    span: Span,
    max_records: usize,
    max_retained_fields: usize,
    max_modifications: usize,
    records: usize,
    /// Present when this program declares `SetUnitAcquireRange` on the same source.
    acquire_span: Option<Span>,
}

impl ObjectHost {
    /// Admit `CreateUnit` against an already selected catalog and source.
    ///
    /// `catalog` must be the caller's existing arc. `source` must be an
    /// identity the caller already selected, such as `ImportedWorld.source`.
    /// Empty install, edition, or map is refused. `unit` must extend `widget`.
    pub fn with_unit_catalog(
        mut self,
        catalog: Arc<dyn ObjectCatalog + Send + Sync>,
        source: Source,
        budget: UnitBudget,
    ) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(NATIVE, reason);
        if self.units.is_some() {
            return Err(fail(Failure::UnitState("already admitted")));
        }
        if source.install.is_empty() || source.edition.is_empty() || source.map.is_empty() {
            return Err(fail(Failure::UnitState("source identity is incomplete")));
        }
        std::alloc::Layout::array::<ExecutedUnit>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if !below(&self.inner.program, KIND, "widget") {
            return Err(fail(Failure::Signature));
        }
        Self::contract(
            &self.inner,
            NATIVE,
            &["player", "integer", "real", "real", "real"],
            KIND,
            false,
            Some(KIND),
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(NATIVE)
            .ok_or_else(|| fail(Failure::Signature))?;
        let mut declaration_source = String::new();
        declaration_source
            .try_reserve_exact(native.source.len())
            .map_err(|_| fail(Failure::Capacity))?;
        declaration_source.push_str(&native.source);
        let acquire_span = self.acquire_declaration(&declaration_source)?;
        self.units = Some(Admission {
            catalog,
            source,
            declaration_source,
            span: native.span,
            max_records: budget.max_records,
            max_retained_fields: budget.max_retained_fields,
            max_modifications: budget.max_modifications,
            records: 0,
            acquire_span,
        });
        Ok(self)
    }

    /// Same catalog allocation passed to [`Self::with_unit_catalog`].
    pub fn unit_catalog(&self) -> Result<Arc<dyn ObjectCatalog + Send + Sync>, Error> {
        Ok(Arc::clone(&self.admitted_units()?.catalog))
    }

    /// Declaration retained at admission. The span is the native's own span.
    pub fn unit_declaration(&self) -> Result<(&str, Span), Error> {
        let admitted = self.admitted_units()?;
        Ok((&admitted.declaration_source, admitted.span))
    }

    /// Checked result for this unit handle. Other families are refused.
    pub fn executed_unit(&self, id: HandleId) -> Result<&ExecutedUnit, Error> {
        match self.inner.entry(id)? {
            Entry::Unit { record } => Ok(record),
            _ => Err(self.inner.error(NATIVE, Failure::WrongFamily)),
        }
    }

    pub(super) fn unit_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.units.as_ref()?;
        if name == NATIVE {
            return Some(self.create_unit(args));
        }
        if name == SET_RANGE && self.admitted_units().ok()?.acquire_span.is_some() {
            return Some(self.set_acquire_range(args));
        }
        None
    }

    fn admitted_units(&self) -> Result<&Admission, Error> {
        self.units
            .as_ref()
            .ok_or_else(|| self.inner.error(NATIVE, Failure::UnitState("not admitted")))
    }

    /// `Ok(None)` when the program does not declare the setter. A present
    /// declaration must return `nothing`, take `unit` then `real`, and share
    /// `CreateUnit`'s source. Parameter kinds are the loaded native's contract.
    fn acquire_declaration(&self, create_source: &str) -> Result<Option<Span>, Error> {
        let Some(native) = self.inner.program.natives().get(SET_RANGE) else {
            return Ok(None);
        };
        Self::contract(
            &self.inner,
            SET_RANGE,
            &["unit", "real"],
            "nothing",
            false,
            Some(KIND),
        )?;
        if native.source != create_source {
            return Err(self.inner.error(SET_RANGE, Failure::Source));
        }
        Ok(Some(native.span))
    }

    fn set_acquire_range(&mut self, args: &[Value]) -> Result<Value, Error> {
        let (id, range) = {
            let fail = |reason| self.inner.error(SET_RANGE, reason);
            let [Value::Handle(unit), range] = args else {
                return Err(fail(Failure::UnitState(
                    "arguments do not match the unit native declaration",
                )));
            };
            let range = finite_real(range).map_err(fail)?;
            match self
                .inner
                .entry(*unit)
                .map_err(|error| self.inner.error(SET_RANGE, error.failure))?
            {
                Entry::Unit { .. } => {}
                _ => return Err(fail(Failure::WrongFamily)),
            }
            (*unit, range)
        };
        match self.inner.entry_mut(id)? {
            Entry::Unit { record } => {
                record.acquire_range = Some(range);
                Ok(Value::Null)
            }
            _ => Err(self.inner.error(SET_RANGE, Failure::WrongFamily)),
        }
    }

    fn create_unit(&mut self, args: &[Value]) -> Result<Value, Error> {
        let record = {
            let fail = |reason| self.inner.error(NATIVE, reason);
            let [Value::Handle(player), Value::Int(unitid), x, y, face] = args else {
                return Err(fail(Failure::UnitState(
                    "arguments do not match the unit native declaration",
                )));
            };
            let x = finite_real(x).map_err(fail)?;
            let y = finite_real(y).map_err(fail)?;
            let face = finite_real(face).map_err(fail)?;
            let rawcode = unitid.to_be_bytes();
            let owner = match self
                .inner
                .entry(*player)
                .map_err(|error| self.inner.error(NATIVE, error.failure))?
            {
                Entry::Player { number } => *number,
                _ => return Err(fail(Failure::WrongFamily)),
            };
            let (records, max_records) = {
                let admitted = self.admitted_units()?;
                (admitted.records, admitted.max_records)
            };
            if records >= max_records {
                return Err(fail(Failure::Budget("unit creation")));
            }
            if self.inner.handles() >= self.inner.max_handles {
                return Err(fail(Failure::Capacity));
            }
            let catalog = Arc::clone(&self.admitted_units()?.catalog);
            let (max_fields, max_modifications) = {
                let admitted = self.admitted_units()?;
                (admitted.max_retained_fields, admitted.max_modifications)
            };
            let definition = match catalog.resolve(rawcode) {
                Ok(definition) => definition,
                Err(problem) => {
                    return Err(fail(Failure::UnitCatalog {
                        requested: rawcode,
                        problem: Box::new(problem),
                    }));
                }
            };
            if definition.rawcode != rawcode {
                return Err(fail(Failure::UnitCatalog {
                    requested: rawcode,
                    problem: Box::new(ResolutionProblem::RawcodeMismatch {
                        requested: rawcode,
                        resolved: definition.rawcode,
                    }),
                }));
            }
            if definition.kind != ObjectKind::Unit {
                return Err(fail(Failure::UnitCatalog {
                    requested: rawcode,
                    problem: Box::new(ResolutionProblem::WrongFamily {
                        expected: ObjectKind::Unit,
                        actual: definition.kind,
                    }),
                }));
            }
            if definition.fields.len() > max_fields {
                return Err(fail(Failure::UnitState(
                    "resolved definition exceeds the retained-field budget",
                )));
            }
            if definition.modifications.len() > max_modifications {
                return Err(fail(Failure::UnitState(
                    "resolved definition exceeds the modification budget",
                )));
            }
            let source = self.admitted_units()?.source.clone();
            ExecutedUnit {
                definition,
                executed_owner: owner,
                source,
                x,
                y,
                face,
                acquire_range: None,
            }
        };
        let id = self.inner.allocate(Entry::Unit { record }, NATIVE)?;
        let admitted = self
            .units
            .as_mut()
            .ok_or_else(|| self.inner.error(NATIVE, Failure::UnitState("not admitted")))?;
        admitted.records += 1;
        Ok(Value::Handle(id))
    }
}

fn finite_real(value: &Value) -> Result<f32, Failure> {
    match value {
        Value::Real(real) if real.is_finite() => Ok(*real),
        Value::Real(_) => Err(Failure::UnitState(
            "recreation policy: finite real required",
        )),
        _ => Err(Failure::UnitState(
            "arguments do not match the unit native declaration",
        )),
    }
}
