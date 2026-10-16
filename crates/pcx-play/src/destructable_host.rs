//! Executed `CreateDestructable` arguments on the shared arena.
//!
//! A record retains the declaration's integer id and finite reals. It is not a
//! mesh, life total, pathing cell, or world object. `CreateDestructableZ` and
//! `CreateDeadDestructable` stay unsupported.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_script::span::Span;
use pcx_script::{HandleId, Value};

const NATIVE: &str = "CreateDestructable";
const KIND: &str = "destructable";

/// Caller bound on creation records. Not a world or rendering limit.
#[derive(Clone, Copy, Debug)]
pub struct DestructableBudget {
    pub max_records: usize,
}

/// One executed creation. Coordinates, facing and scale are the call's finite reals.
/// Facing is retained in the declared real, not converted from the source comment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DestructableCreation {
    pub objectid: i32,
    pub x: f32,
    pub y: f32,
    pub face: f32,
    pub scale: f32,
    pub variation: i32,
}

pub(super) struct Admission {
    max_records: usize,
    records: usize,
    source: String,
    span: Span,
}

impl ObjectHost {
    /// Admit `CreateDestructable` on the shared arena.
    ///
    /// The native must return `destructable`, and that type must extend `widget`.
    /// The declaration source and span are kept. New records are refused at
    /// `budget.max_records` before allocation. A non-finite real is refused as
    /// this recreation's policy; that is not a claim about the shipped native.
    pub fn with_destructables(mut self, budget: DestructableBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(NATIVE, reason);
        if self.destructables.is_some() {
            return Err(fail(Failure::DestructableState("already admitted")));
        }
        std::alloc::Layout::array::<DestructableCreation>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if !below(&self.inner.program, KIND, "widget") {
            return Err(fail(Failure::Signature));
        }
        Self::contract(
            &self.inner,
            NATIVE,
            &["integer", "real", "real", "real", "real", "integer"],
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
        let mut source = String::new();
        source
            .try_reserve_exact(native.source.len())
            .map_err(|_| fail(Failure::Capacity))?;
        source.push_str(&native.source);
        self.destructables = Some(Admission {
            max_records: budget.max_records,
            records: 0,
            source,
            span: native.span,
        });
        Ok(self)
    }

    /// Declaration retained at admission. The record itself is not a world object.
    pub fn destructable_declaration(&self) -> Result<(&str, Span), Error> {
        let admitted = self.destructables.as_ref().ok_or_else(|| {
            self.inner
                .error(NATIVE, Failure::DestructableState("not admitted"))
        })?;
        Ok((&admitted.source, admitted.span))
    }

    /// Executed creation for an identity of this family. Other families are refused.
    pub fn destructable_creation(&self, id: HandleId) -> Result<DestructableCreation, Error> {
        match self.inner.entry(id)? {
            Entry::Destructable { creation } => Ok(*creation),
            _ => Err(self.inner.error(NATIVE, Failure::WrongFamily)),
        }
    }

    pub(super) fn destructable_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != NATIVE {
            return None;
        }
        self.destructables.as_ref()?;
        Some(self.create_destructable(args))
    }

    fn create_destructable(&mut self, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(NATIVE, reason);
        let [
            Value::Int(objectid),
            x,
            y,
            face,
            scale,
            Value::Int(variation),
        ] = args
        else {
            return Err(fail(Failure::DestructableState(
                "arguments do not match the destructable native declaration",
            )));
        };
        let x = finite_real(x).map_err(fail)?;
        let y = finite_real(y).map_err(fail)?;
        let face = finite_real(face).map_err(fail)?;
        let scale = finite_real(scale).map_err(fail)?;
        let creation = DestructableCreation {
            objectid: *objectid,
            x,
            y,
            face,
            scale,
            variation: *variation,
        };
        let (records, max_records) = {
            let admitted = self
                .destructables
                .as_ref()
                .ok_or_else(|| fail(Failure::DestructableState("not admitted")))?;
            (admitted.records, admitted.max_records)
        };
        if records >= max_records {
            return Err(fail(Failure::Budget("destructable creation")));
        }
        let id = self
            .inner
            .allocate(Entry::Destructable { creation }, NATIVE)?;
        let Some(admitted) = self.destructables.as_mut() else {
            return Err(self
                .inner
                .error(NATIVE, Failure::DestructableState("not admitted")));
        };
        admitted.records += 1;
        Ok(Value::Handle(id))
    }
}

fn finite_real(value: &Value) -> Result<f32, Failure> {
    match value {
        Value::Real(real) if real.is_finite() => Ok(*real),
        Value::Real(_) => Err(Failure::DestructableState(
            "recreation policy: finite real required",
        )),
        _ => Err(Failure::DestructableState(
            "arguments do not match the destructable native declaration",
        )),
    }
}
