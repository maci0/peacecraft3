//! Source-qualified `Condition` wrappers in the existing session arena.
//! Wrapping retains a loaded callback and does not evaluate a predicate.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_script::resolve::{Function, Program};
use pcx_script::span::Span;
use pcx_script::value::FunctionId;
use pcx_script::{HandleId, Value};

const NATIVE: &str = "Condition";
/// Caller bound on retained condition wrappers, independent of shared arena capacity.
#[derive(Clone, Copy, Debug)]
pub struct ConditionBudget {
    pub max_records: usize,
}
/// Borrowed declaration in the Host's immutable Program; no callback body is copied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConditionCallback<'a> {
    pub function: FunctionId,
    pub name: &'a str,
    pub source: &'a str,
    pub span: Span,
}
pub(super) struct Admission {
    max_records: usize,
    retained: usize,
}
impl ObjectHost {
    /// Admit the loaded `code -> conditionfunc` interface and its authored ancestry.
    ///
    /// The native and each inherited type declaration share the source of the
    /// already-admitted `CreateTimer`. Parent edges are read from the Program.
    /// Only loaded functions taking nothing and returning scalar boolean are
    /// supported. Wrappers live until Host/session drop. Predicate evaluation,
    /// trigger-condition registration, boolean operators and destruction remain
    /// unsupported.
    pub fn with_conditions(mut self, budget: ConditionBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(NATIVE, reason);
        if self.conditions.is_some() {
            return Err(fail(Failure::ConditionState("already admitted")));
        }
        std::alloc::Layout::array::<FunctionId>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        Self::contract(
            &self.inner,
            NATIVE,
            &["code"],
            "conditionfunc",
            false,
            Some("conditionfunc"),
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(NATIVE)
            .ok_or_else(|| fail(Failure::Signature))?;
        let timer = self
            .inner
            .program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?;
        if native.source != timer.source {
            return Err(fail(Failure::Source));
        }
        let mut kind = native.ret.kind.name();
        if !below(&self.inner.program, kind, "boolexpr") {
            return Err(fail(Failure::Signature));
        }
        for _ in 0..=self.inner.program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = self
                .inner
                .program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if ty.source != native.source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        self.conditions = Some(Admission {
            max_records: budget.max_records,
            retained: 0,
        });
        Ok(self)
    }
    pub fn retained_conditions(&self) -> Result<usize, Error> {
        self.conditions
            .as_ref()
            .map(|admitted| admitted.retained)
            .ok_or_else(|| {
                self.inner
                    .error(NATIVE, Failure::ConditionState("not admitted"))
            })
    }
    /// Inspect a real wrapper; an ordinary reserved identity of the same type refuses.
    pub fn condition_callback(&self, id: HandleId) -> Result<ConditionCallback<'_>, Error> {
        if self.conditions.is_none() {
            return Err(self
                .inner
                .error(NATIVE, Failure::ConditionState("not admitted")));
        }
        let function = match self
            .inner
            .entry(id)
            .map_err(|e| self.inner.error(NATIVE, e.failure))?
        {
            Entry::Condition { function } => *function,
            Entry::DeadBoolExpr => return Err(self.inner.error(NATIVE, Failure::Identity)),
            _ => return Err(self.inner.error(NATIVE, Failure::WrongFamily)),
        };
        let declared = callback(&self.inner.program, function)
            .map_err(|reason| self.inner.error(NATIVE, reason))?;
        Ok(ConditionCallback {
            function,
            name: &declared.name,
            source: &declared.source,
            span: declared.span,
        })
    }
    pub(super) fn condition_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.conditions.as_ref()?;
        if name != NATIVE {
            return None;
        }
        Some(self.create_condition(args))
    }
    fn create_condition(&mut self, args: &[Value]) -> Result<Value, Error> {
        let [Value::Code(function)] = args else {
            return Err(self.inner.error(
                NATIVE,
                Failure::ConditionState("expected one loaded code callback"),
            ));
        };
        callback(&self.inner.program, *function)
            .map_err(|reason| self.inner.error(NATIVE, reason))?;
        let admitted = self.conditions.as_mut().ok_or_else(|| {
            self.inner
                .error(NATIVE, Failure::ConditionState("not admitted"))
        })?;
        if admitted.retained >= admitted.max_records {
            return Err(self
                .inner
                .error(NATIVE, Failure::Budget("condition wrappers")));
        }
        // shortcut: one token per successful call; revisit if native repeated-code identity requires interning.
        let id = self.inner.allocate(
            Entry::Condition {
                function: *function,
            },
            NATIVE,
        )?;
        admitted.retained += 1;
        Ok(Value::Handle(id))
    }
}
fn callback(program: &Program, id: FunctionId) -> Result<&Function, Failure> {
    let function = program
        .functions()
        .get(id.index())
        .ok_or(Failure::ConditionState("callback is not a loaded function"))?;
    if !function.params.is_empty() || function.ret.array || function.ret.kind.name() != "boolean" {
        return Err(Failure::ConditionState(
            "callback takes nothing returns boolean",
        ));
    }
    Ok(function)
}
