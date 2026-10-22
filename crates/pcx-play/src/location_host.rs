use super::{Entry, Error, Failure, ObjectHost, below};
use pcx_script::Value;

impl ObjectHost {
    /// Two-dimensional locations on the same bounded arena as units and callbacks.
    /// Height queries require the terrain-height consumer and stay unsupported.
    pub fn with_locations(mut self) -> Result<Self, Error> {
        if self.locations || !below(&self.inner.program, "location", "agent") {
            return Err(self.inner.error("Location", Failure::Signature));
        }
        let source = &self
            .inner
            .program
            .natives()
            .get("Location")
            .ok_or_else(|| self.inner.error("Location", Failure::Signature))?
            .source;
        if self
            .inner
            .program
            .natives()
            .get("CreateUnit")
            .is_some_and(|n| &n.source != source)
        {
            return Err(self.inner.error("Location", Failure::Source));
        }
        for (name, args, result, constant) in [
            ("Location", &["real", "real"][..], "location", false),
            ("RemoveLocation", &["location"][..], "nothing", false),
            (
                "MoveLocation",
                &["location", "real", "real"][..],
                "nothing",
                false,
            ),
            ("GetLocationX", &["location"][..], "real", false),
            ("GetLocationY", &["location"][..], "real", false),
            ("GetUnitLoc", &["unit"][..], "location", true),
        ] {
            let Some(declaration) = self.inner.program.natives().get(name) else {
                continue;
            };
            Self::contract(&self.inner, name, args, result, constant, Some("location"))?;
            if &declaration.source != source {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        self.locations = true;
        Ok(self)
    }

    pub(super) fn location_point(&self, value: &Value, name: &str) -> Result<[f32; 2], Error> {
        if !self.locations {
            return Err(self
                .inner
                .error(name, Failure::UnitActivity("locations not admitted".into())));
        }
        let Value::Handle(id) = value else {
            return Err(self.inner.error(name, Failure::Signature));
        };
        match self
            .inner
            .entry(*id)
            .map_err(|e| self.inner.error(name, e.failure))?
        {
            Entry::Location { point: Some(point) } => Ok(*point),
            Entry::Location { point: None } => Err(self.inner.error(name, Failure::Identity)),
            _ => Err(self.inner.error(name, Failure::WrongFamily)),
        }
    }

    pub(super) fn location_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !self.locations || self.inner.program.natives().get(name).is_none() {
            return None;
        }
        if !matches!(
            name,
            "Location"
                | "RemoveLocation"
                | "MoveLocation"
                | "GetLocationX"
                | "GetLocationY"
                | "GetUnitLoc"
        ) {
            return None;
        }
        Some((|| {
            let finite = |x: f32, y: f32| {
                if x.is_finite() && y.is_finite() {
                    Ok([x, y])
                } else {
                    Err(self.inner.error(
                        name,
                        Failure::UnitActivity("finite location coordinates required".into()),
                    ))
                }
            };
            if name == "Location" {
                let [Value::Real(x), Value::Real(y)] = args else {
                    return Err(self.inner.error(name, Failure::Signature));
                };
                let point = finite(*x, *y)?;
                return self
                    .inner
                    .allocate(Entry::Location { point: Some(point) }, name)
                    .map(Value::Handle);
            }
            if name == "GetUnitLoc" {
                let unit = match args {
                    [Value::Null] => return Ok(Value::Null),
                    [Value::Handle(unit)] => *unit,
                    _ => return Err(self.inner.error(name, Failure::Signature)),
                };
                let point = match self
                    .inner
                    .entry(unit)
                    .map_err(|e| self.inner.error(name, e.failure))?
                {
                    Entry::Unit { record } => record.position.ok_or_else(|| {
                        self.inner.error(
                            name,
                            Failure::UnitActivity("accepted unit position required".into()),
                        )
                    })?,
                    Entry::RemovedUnit { position: Some(point) } => *point,
                    Entry::RemovedUnit { position: None } => return Ok(Value::Null),
                    _ => return Err(self.inner.error(name, Failure::WrongFamily)),
                };
                return self
                    .inner
                    .allocate(Entry::Location { point: Some(point) }, name)
                    .map(Value::Handle);
            }
            if matches!(args, [Value::Null]) && matches!(name, "GetLocationX" | "GetLocationY") {
                return Ok(Value::Real(0.0));
            }
            if name == "RemoveLocation" && matches!(args, [Value::Null]) {
                return Ok(Value::Null);
            }
            if name == "MoveLocation"
                && let [Value::Null, Value::Real(x), Value::Real(y)] = args
            {
                finite(*x, *y)?;
                return Ok(Value::Null);
            }
            let point = match args {
                [_, Value::Real(x), Value::Real(y)] if name == "MoveLocation" => {
                    Some(finite(*x, *y)?)
                }
                [_] if name != "MoveLocation" => None,
                _ => return Err(self.inner.error(name, Failure::Signature)),
            };
            let old = self.location_point(&args[0], name)?;
            if matches!(name, "GetLocationX" | "GetLocationY") {
                return Ok(Value::Real(old[usize::from(name == "GetLocationY")]));
            }
            let Value::Handle(id) = args[0] else {
                unreachable!("checked location")
            };
            if let Entry::Location { point: stored } = self.inner.entry_mut(id)? {
                *stored = point;
            }
            Ok(Value::Null)
        })())
    }
}
