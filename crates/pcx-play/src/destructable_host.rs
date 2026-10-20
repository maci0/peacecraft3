//! Executed destructable-creation arguments on the shared arena.
//!
//! One allocation path serves the four declared create natives. A record keeps
//! the executed id, finite reals, variation, whether that native supplied `z`,
//! and whether the native requested the dead form. It is not a mesh, terrain
//! height, life total, pathing cell, or world object.
use super::super::{Entry, Error, Failure, below};
use super::ObjectHost;
use pcx_script::span::Span;
use pcx_script::{HandleId, Value};

const KIND: &str = "destructable";
const BASE: &[&str] = &["integer", "real", "real", "real", "real", "integer"];
const WITH_Z: &[&str] = &["integer", "real", "real", "real", "real", "real", "integer"];

struct Shape {
    name: &'static str,
    parameters: &'static [&'static str],
    /// The native's parameter list includes `z`. Absence is not a height.
    z: bool,
    /// The native is one of the `CreateDead*` declarations. Not a life total.
    dead: bool,
}

const SHAPES: &[Shape] = &[
    Shape {
        name: "CreateDestructable",
        parameters: BASE,
        z: false,
        dead: false,
    },
    Shape {
        name: "CreateDestructableZ",
        parameters: WITH_Z,
        z: true,
        dead: false,
    },
    Shape {
        name: "CreateDeadDestructable",
        parameters: BASE,
        z: false,
        dead: true,
    },
    Shape {
        name: "CreateDeadDestructableZ",
        parameters: WITH_Z,
        z: true,
        dead: true,
    },
];

/// Caller bound on creation records. Not a world or rendering limit.
#[derive(Clone, Copy, Debug)]
pub struct DestructableBudget {
    pub max_records: usize,
}

/// One executed creation. Coordinates, facing and scale are the call's finite reals.
/// Facing is retained in the declared real, not converted from the source comment.
/// `z` is `None` when the creating native has no `z` parameter. `dead` is the
/// source-requested dead form, not a computed life or a later kill.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DestructableCreation {
    pub objectid: i32,
    pub x: f32,
    pub y: f32,
    pub z: Option<f32>,
    pub face: f32,
    pub scale: f32,
    pub variation: i32,
    pub dead: bool,
}

/// Script-changed runtime state; `dead` starts from the creation variant.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DestructableState {
    pub invulnerable: bool,
    pub dead: bool,
    pub hidden: bool,
}

const RUNTIME: [(&str, &[&str], &str); 7] = [
    (
        "SetDestructableInvulnerable",
        &["destructable", "boolean"],
        "nothing",
    ),
    ("IsDestructableInvulnerable", &["destructable"], "boolean"),
    ("KillDestructable", &["destructable"], "nothing"),
    ("ShowDestructable", &["destructable", "boolean"], "nothing"),
    ("GetDestructableX", &["destructable"], "real"),
    ("GetDestructableY", &["destructable"], "real"),
    ("GetDestructableTypeId", &["destructable"], "integer"),
];

pub(super) struct Admission {
    max_records: usize,
    records: usize,
    source: String,
    spans: [Option<Span>; SHAPES.len()],
}

impl ObjectHost {
    /// Admit destructable creation on the shared arena.
    ///
    /// `CreateDestructable` is required. Each of the other three create natives
    /// is admitted only when that program declares it with the same source and
    /// the matching parameter list. A program that omits a variant still
    /// admits the base native. The type must extend `widget`. New records are
    /// refused at `budget.max_records` before allocation. A non-finite real is
    /// refused as this recreation's policy; that is not a claim about the
    /// shipped native.
    pub fn with_destructables(mut self, budget: DestructableBudget) -> Result<Self, Error> {
        let fail = |reason| self.inner.error(SHAPES[0].name, reason);
        if self.destructables.is_some() {
            return Err(fail(Failure::DestructableState("already admitted")));
        }
        std::alloc::Layout::array::<DestructableCreation>(budget.max_records)
            .map_err(|_| fail(Failure::Capacity))?;
        if !below(&self.inner.program, KIND, "widget") {
            return Err(fail(Failure::Signature));
        }
        let mut spans = [None; SHAPES.len()];
        let mut source = String::new();
        for (index, shape) in SHAPES.iter().enumerate() {
            let present = self.inner.program.natives().get(shape.name).is_some();
            if index == 0 && !present {
                return Err(fail(Failure::Signature));
            }
            if !present {
                continue;
            }
            Self::contract(
                &self.inner,
                shape.name,
                shape.parameters,
                KIND,
                false,
                Some(KIND),
            )?;
            let native = self
                .inner
                .program
                .natives()
                .get(shape.name)
                .ok_or_else(|| self.inner.error(shape.name, Failure::Signature))?;
            if index == 0 {
                source
                    .try_reserve_exact(native.source.len())
                    .map_err(|_| fail(Failure::Capacity))?;
                source.push_str(&native.source);
            } else if native.source != source {
                return Err(self.inner.error(shape.name, Failure::Source));
            }
            spans[index] = Some(native.span);
        }
        for (name, args, result) in RUNTIME {
            let Some(native) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, result, false, Some(KIND))?;
            if native.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.destructables = Some(Admission {
            max_records: budget.max_records,
            records: 0,
            source,
            spans,
        });
        Ok(self)
    }

    /// Base `CreateDestructable` declaration retained at admission.
    /// The record itself is not a world object.
    pub fn destructable_declaration(&self) -> Result<(&str, Span), Error> {
        let admitted = self.admitted()?;
        let span = admitted.spans[0].ok_or_else(|| {
            self.inner
                .error(SHAPES[0].name, Failure::DestructableState("not admitted"))
        })?;
        Ok((&admitted.source, span))
    }

    /// Span of an admitted create variant. `Ok(None)` means that native was
    /// absent from the program, so calls stay unsupported.
    pub fn destructable_variant_declaration(
        &self,
        name: &str,
    ) -> Result<Option<(&str, Span)>, Error> {
        let index = SHAPES
            .iter()
            .position(|shape| shape.name == name)
            .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
        let admitted = self.admitted()?;
        Ok(admitted.spans[index].map(|span| (admitted.source.as_str(), span)))
    }

    /// Executed creation for an identity of this family. Other families are refused.
    pub fn destructable_creation(&self, id: HandleId) -> Result<DestructableCreation, Error> {
        match self.inner.entry(id)? {
            Entry::Destructable { creation, .. } => Ok(*creation),
            _ => Err(self.inner.error(SHAPES[0].name, Failure::WrongFamily)),
        }
    }

    pub fn executed_destructables(&self) -> impl Iterator<Item = DestructableCreation> + '_ {
        self.inner.arena.iter().filter_map(|entry| match entry {
            Entry::Destructable { creation, .. } => Some(*creation),
            _ => None,
        })
    }

    pub fn destructable_state(&self, id: HandleId) -> Result<DestructableState, Error> {
        match self.inner.entry(id)? {
            Entry::Destructable { state, .. } => Ok(*state),
            _ => Err(self.inner.error(SHAPES[0].name, Failure::WrongFamily)),
        }
    }

    pub(super) fn destructable_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if RUNTIME.iter().any(|&(n, ..)| n == name) {
            self.destructables.as_ref()?;
            self.inner.program.natives().get(name)?;
            return Some(self.destructable_runtime(name, args));
        }
        let index = SHAPES.iter().position(|shape| shape.name == name)?;
        self.destructables.as_ref()?.spans[index]?;
        Some(self.create_destructable(index, args))
    }

    /// A null destructable is ignored by setters and reads as zero/false, as
    /// with other widget natives in this host.
    fn destructable_runtime(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let (target, flag) = match args {
            [target] => (target, None),
            [target, Value::Bool(flag)] => (target, Some(*flag)),
            _ => return Err(self.inner.error(name, Failure::Signature)),
        };
        let id = match target {
            Value::Handle(id) => *id,
            Value::Null => {
                return Ok(match name {
                    "IsDestructableInvulnerable" => Value::Bool(false),
                    "GetDestructableX" | "GetDestructableY" => Value::Real(0.0),
                    "GetDestructableTypeId" => Value::Int(0),
                    _ => Value::Null,
                });
            }
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        };
        if !matches!(
            self.inner
                .entry(id)
                .map_err(|e| self.inner.error(name, e.failure))?,
            Entry::Destructable { .. }
        ) {
            return Err(self.inner.error(name, Failure::WrongFamily));
        }
        let Ok(Entry::Destructable { creation, state }) = self.inner.entry_mut(id) else {
            unreachable!("checked destructable entry");
        };
        Ok(match (name, flag) {
            ("SetDestructableInvulnerable", Some(flag)) => {
                state.invulnerable = flag;
                Value::Null
            }
            ("ShowDestructable", Some(flag)) => {
                state.hidden = !flag;
                Value::Null
            }
            ("KillDestructable", None) => {
                state.dead = true;
                Value::Null
            }
            ("IsDestructableInvulnerable", None) => Value::Bool(state.invulnerable),
            ("GetDestructableX", None) => Value::Real(creation.x),
            ("GetDestructableY", None) => Value::Real(creation.y),
            ("GetDestructableTypeId", None) => Value::Int(creation.objectid),
            _ => return Err(self.inner.error(name, Failure::Signature)),
        })
    }

    fn admitted(&self) -> Result<&Admission, Error> {
        self.destructables.as_ref().ok_or_else(|| {
            self.inner
                .error(SHAPES[0].name, Failure::DestructableState("not admitted"))
        })
    }

    fn create_destructable(&mut self, index: usize, args: &[Value]) -> Result<Value, Error> {
        let shape = &SHAPES[index];
        let fail = |reason| self.inner.error(shape.name, reason);
        if self.unit_placement.is_some() {
            return Err(fail(Failure::DestructableState(
                "stationary collision cache must be rebuilt before creating destructables",
            )));
        }
        let creation = creation_from(shape, args).map_err(fail)?;
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
        let id = self.inner.allocate(
            Entry::Destructable {
                creation,
                state: DestructableState {
                    dead: creation.dead,
                    ..DestructableState::default()
                },
            },
            shape.name,
        )?;
        let Some(admitted) = self.destructables.as_mut() else {
            return Err(self
                .inner
                .error(shape.name, Failure::DestructableState("not admitted")));
        };
        admitted.records += 1;
        Ok(Value::Handle(id))
    }
}

fn creation_from(shape: &Shape, args: &[Value]) -> Result<DestructableCreation, Failure> {
    let mismatch =
        Failure::DestructableState("arguments do not match the destructable native declaration");
    if shape.z {
        let [
            Value::Int(objectid),
            x,
            y,
            z,
            face,
            scale,
            Value::Int(variation),
        ] = args
        else {
            return Err(mismatch);
        };
        Ok(DestructableCreation {
            objectid: *objectid,
            x: finite_real(x)?,
            y: finite_real(y)?,
            z: Some(finite_real(z)?),
            face: finite_real(face)?,
            scale: finite_real(scale)?,
            variation: *variation,
            dead: shape.dead,
        })
    } else {
        let [
            Value::Int(objectid),
            x,
            y,
            face,
            scale,
            Value::Int(variation),
        ] = args
        else {
            return Err(mismatch);
        };
        Ok(DestructableCreation {
            objectid: *objectid,
            x: finite_real(x)?,
            y: finite_real(y)?,
            z: None,
            face: finite_real(face)?,
            scale: finite_real(scale)?,
            variation: *variation,
            dead: shape.dead,
        })
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
