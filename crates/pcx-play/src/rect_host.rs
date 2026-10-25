//! Executed rectangular bounds share the session's bounded identity arena.
use super::super::{Entry, Error, Failure};
use super::ObjectHost;
use pcx_script::{HandleId, Value};

impl ObjectHost {
    /// Admit rectangle operations from the loaded declarations.
    /// Location-based movement additionally needs admitted locations at execution.
    pub fn with_rectangles(mut self) -> Result<Self, Error> {
        if self.rectangles {
            return Err(self
                .inner
                .error("Rect", Failure::RectState("already admitted")));
        }
        for (name, parameters, result) in [
            ("Rect", &["real", "real", "real", "real"][..], "rect"),
            ("RemoveRect", &["rect"][..], "nothing"),
            (
                "SetRect",
                &["rect", "real", "real", "real", "real"][..],
                "nothing",
            ),
            ("MoveRectTo", &["rect", "real", "real"][..], "nothing"),
            ("GetRectCenterX", &["rect"][..], "real"),
            ("GetRectCenterY", &["rect"][..], "real"),
            ("GetRectMinX", &["rect"][..], "real"),
            ("GetRectMinY", &["rect"][..], "real"),
            ("GetRectMaxX", &["rect"][..], "real"),
            ("GetRectMaxY", &["rect"][..], "real"),
        ] {
            Self::contract(&self.inner, name, parameters, result, false, Some("rect"))?;
        }
        if let Some(native) = self.inner.program.natives().get("MoveRectToLoc") {
            Self::contract(
                &self.inner,
                "MoveRectToLoc",
                &["rect", "location"],
                "nothing",
                false,
                Some("location"),
            )?;
            if native.source
                != self
                    .inner
                    .program
                    .natives()
                    .get("Rect")
                    .expect("admitted rectangle declaration")
                    .source
            {
                return Err(self.inner.error("MoveRectToLoc", Failure::Source));
            }
        }
        self.rectangles = true;
        Ok(self)
    }

    /// Admit the full map extent independently of the playable camera rectangle.
    pub fn with_world_bounds(mut self, bounds: [f32; 4]) -> Result<Self, Error> {
        let [x0, y0, x1, y1] = bounds;
        if !self.rectangles
            || self.world_bounds.is_some()
            || !bounds.iter().all(|v| v.is_finite())
            || x0 >= x1
            || y0 >= y1
        {
            return Err(self.inner.error(
                "GetWorldBounds",
                Failure::RectState(
                    "ordered finite full-map bounds and rectangle admission required",
                ),
            ));
        }
        if let Some(native) = self.inner.program.natives().get("GetWorldBounds") {
            Self::contract(
                &self.inner,
                "GetWorldBounds",
                &[],
                "rect",
                false,
                Some("rect"),
            )?;
            if native.source
                != self
                    .inner
                    .program
                    .natives()
                    .get("Rect")
                    .expect("admitted")
                    .source
            {
                return Err(self.inner.error("GetWorldBounds", Failure::Source));
            }
        }
        self.world_bounds = Some(bounds);
        Ok(self)
    }

    /// Bounds in executed min-x, min-y, max-x, max-y order.
    pub fn rectangle_bounds(&self, id: HandleId) -> Result<[f32; 4], Error> {
        match self.inner.entry(id)? {
            Entry::Rect {
                bounds: Some(bounds),
            } => Ok(*bounds),
            Entry::Rect { bounds: None } => Err(self.inner.error("Rect", Failure::Identity)),
            _ => Err(self.inner.error("Rect", Failure::WrongFamily)),
        }
    }

    pub(super) fn rect_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !self.rectangles
            || !matches!(
                name,
                "Rect"
                    | "GetWorldBounds"
                    | "RemoveRect"
                    | "SetRect"
                    | "MoveRectTo"
                    | "MoveRectToLoc"
                    | "GetRectCenterX"
                    | "GetRectCenterY"
                    | "GetRectMinX"
                    | "GetRectMinY"
                    | "GetRectMaxX"
                    | "GetRectMaxY"
            )
        {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(self.rectangle(name, args))
    }

    fn rectangle(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |detail| self.inner.error(name, Failure::RectState(detail));
        if name == "GetWorldBounds" {
            if !args.is_empty() {
                return Err(self.inner.error(name, Failure::Signature));
            }
            let bounds = self
                .world_bounds
                .ok_or_else(|| fail("full-map extent is not admitted"))?;
            return self
                .inner
                .allocate(
                    Entry::Rect {
                        bounds: Some(bounds),
                    },
                    name,
                )
                .map(Value::Handle);
        }
        if name == "Rect" {
            let bounds = bounds(args).ok_or_else(|| fail("ordered finite bounds required"))?;
            return self
                .inner
                .allocate(
                    Entry::Rect {
                        bounds: Some(bounds),
                    },
                    name,
                )
                .map(Value::Handle);
        }
        let (Value::Handle(id), rest) = args
            .split_first()
            .ok_or_else(|| fail("rectangle handle required"))?
        else {
            return Err(fail("rectangle handle required"));
        };
        let current = self
            .rectangle_bounds(*id)
            .map_err(|e| self.inner.error(name, e.failure))?;
        let updated = match (name, rest) {
            ("RemoveRect", []) => None,
            ("SetRect", values) if values.len() == 4 => {
                Some(bounds(values).ok_or_else(|| fail("ordered finite bounds required"))?)
            }
            ("MoveRectTo" | "MoveRectToLoc", values) => {
                let [x, y] = match (name, values) {
                    ("MoveRectTo", [x, y]) => {
                        let center = [x, y].map(|v| v.as_real().filter(|v| v.is_finite()));
                        let [Some(x), Some(y)] = center else {
                            return Err(fail("finite center required"));
                        };
                        [x, y]
                    }
                    ("MoveRectToLoc", [location]) => self.location_point(location, name)?,
                    _ => return Err(fail("arguments do not match the rectangle declaration")),
                };
                // Wider intermediates avoid overflow for otherwise representable bounds.
                let half_x = (f64::from(current[2]) - f64::from(current[0])) * 0.5;
                let half_y = (f64::from(current[3]) - f64::from(current[1])) * 0.5;
                let moved = [
                    f64::from(x) - half_x,
                    f64::from(y) - half_y,
                    f64::from(x) + half_x,
                    f64::from(y) + half_y,
                ]
                .map(|v| v as f32);
                if moved.iter().any(|v| !v.is_finite()) {
                    return Err(fail("moved bounds exceed real range"));
                }
                Some(moved)
            }
            ("GetRectMinX", []) => return Ok(Value::Real(current[0])),
            ("GetRectMinY", []) => return Ok(Value::Real(current[1])),
            ("GetRectMaxX", []) => return Ok(Value::Real(current[2])),
            ("GetRectMaxY", []) => return Ok(Value::Real(current[3])),
            ("GetRectCenterX", []) => {
                return Ok(Value::Real(
                    ((f64::from(current[0]) + f64::from(current[2])) * 0.5) as f32,
                ));
            }
            ("GetRectCenterY", []) => {
                return Ok(Value::Real(
                    ((f64::from(current[1]) + f64::from(current[3])) * 0.5) as f32,
                ));
            }
            _ => return Err(fail("arguments do not match the rectangle declaration")),
        };
        let at = usize::try_from(id.token() - 1)
            .map_err(|_| self.inner.error(name, Failure::Identity))?;
        self.inner.arena[at] = Entry::Rect { bounds: updated };
        Ok(Value::Null)
    }
}

fn bounds(values: &[Value]) -> Option<[f32; 4]> {
    let [a, b, c, d] = values else {
        return None;
    };
    let [Some(a), Some(b), Some(c), Some(d)] =
        [a, b, c, d].map(|v| v.as_real().filter(|v| v.is_finite()))
    else {
        return None;
    };
    (a <= c && b <= d).then_some([a, b, c, d])
}

#[cfg(test)]
#[path = "rect_host_tests.rs"]
mod tests;
