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
        for (name, parameters, result) in [
            ("MoveRectToLoc", &["rect", "location"][..], "nothing"),
            ("RectFromLoc", &["location", "location"][..], "rect"),
            (
                "SetRectFromLoc",
                &["rect", "location", "location"][..],
                "nothing",
            ),
        ] {
            if let Some(native) = self.inner.program.natives().get(name) {
                Self::contract(
                    &self.inner,
                    name,
                    parameters,
                    result,
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
                    return Err(self.inner.error(name, Failure::Source));
                }
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

    /// Admitted full-map extent for world snapshots; `None` is unadmitted.
    pub fn world_bounds_snapshot(&self) -> Option<[f32; 4]> {
        self.world_bounds
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
                    | "RectFromLoc"
                    | "SetRectFromLoc"
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

    fn rectangle_location_bounds(&self, args: &[Value], name: &str) -> Result<[f32; 4], Error> {
        let fail = |message| self.inner.error(name, Failure::RectState(message));
        let [minimum, maximum] = args else {
            return Err(fail("two locations required"));
        };
        let [min_x, min_y] = self.location_point(minimum, name)?;
        let [max_x, max_y] = self.location_point(maximum, name)?;
        bounds(&[min_x, min_y, max_x, max_y].map(Value::Real))
            .ok_or_else(|| fail("finite rectangle coordinates required"))
    }

    fn clamp_rectangle_bounds(&self, coordinates: [f32; 4], name: &str) -> Result<[f32; 4], Error> {
        let Some(rules) = self.numeric.as_ref().and_then(|n| n.rectangle_clamp()) else {
            return Ok(coordinates);
        };
        let world = self.world_bounds.ok_or_else(|| {
            self.inner.error(
                name,
                Failure::RectState(
                    "source-derived world extent is required for rectangle clamping",
                ),
            )
        })?;
        let limits = rules
            .limits(world)
            .map_err(|why| self.inner.error(name, Failure::ScriptMath(why)))?;
        if limits.iter().any(|v| !v.is_finite()) || limits[0] > limits[2] || limits[1] > limits[3] {
            return Err(self.inner.error(
                name,
                Failure::RectState("world extent is smaller than the selected clamp inset"),
            ));
        }
        Ok(std::array::from_fn(|i| {
            let (low, high) = (limits[i % 2], limits[i % 2 + 2]);
            if coordinates[i] < low {
                low
            } else if coordinates[i] > high {
                high
            } else {
                coordinates[i]
            }
        }))
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
        if matches!(name, "Rect" | "RectFromLoc") {
            let bounds = if name == "RectFromLoc" {
                self.rectangle_location_bounds(args, name)?
            } else {
                bounds(args).ok_or_else(|| fail("finite rectangle coordinates required"))?
            };
            let bounds = self.clamp_rectangle_bounds(bounds, name)?;
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
            ("SetRect", values) if values.len() == 4 => Some(self.clamp_rectangle_bounds(
                bounds(values).ok_or_else(|| fail("finite rectangle coordinates required"))?,
                name,
            )?),
            ("SetRectFromLoc", locations) => {
                Some(self.clamp_rectangle_bounds(
                    self.rectangle_location_bounds(locations, name)?,
                    name,
                )?)
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
                let moved =
                    if let Some(rules) = self.numeric.as_ref().and_then(|n| n.rectangle_motion()) {
                        rules
                            .translate(current, [x, y])
                            .map_err(|why| self.inner.error(name, Failure::ScriptMath(why)))?
                    } else {
                        let half_x = (f64::from(current[2]) - f64::from(current[0])) * 0.5;
                        let half_y = (f64::from(current[3]) - f64::from(current[1])) * 0.5;
                        [
                            f64::from(x) - half_x,
                            f64::from(y) - half_y,
                            f64::from(x) + half_x,
                            f64::from(y) + half_y,
                        ]
                        .map(|v| v as f32)
                    };
                if moved.iter().any(|v| !v.is_finite()) {
                    return Err(fail("moved bounds exceed real range"));
                }
                Some(moved)
            }
            ("GetRectMinX", []) => return Ok(Value::Real(current[0])),
            ("GetRectMinY", []) => return Ok(Value::Real(current[1])),
            ("GetRectMaxX", []) => return Ok(Value::Real(current[2])),
            ("GetRectMaxY", []) => return Ok(Value::Real(current[3])),
            ("GetRectCenterX" | "GetRectCenterY", []) => {
                let axis = usize::from(name == "GetRectCenterY");
                let center =
                    if let Some(rules) = self.numeric.as_ref().and_then(|n| n.rectangle_motion()) {
                        rules
                            .centers(current)
                            .map_err(|why| self.inner.error(name, Failure::ScriptMath(why)))?[axis]
                    } else {
                        ((f64::from(current[axis]) + f64::from(current[axis + 2])) * 0.5) as f32
                    };
                return Ok(Value::Real(center));
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
    Some([
        if a < c { a } else { c },
        if b < d { b } else { d },
        if a > c { a } else { c },
        if b > d { b } else { d },
    ])
}

#[cfg(test)]
#[path = "rect_host_tests.rs"]
mod tests;
