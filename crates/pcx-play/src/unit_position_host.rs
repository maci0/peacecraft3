use super::{Entry, Error, ExecutedUnit, Failure, NativeCallback, ObjectHost};
use crate::unit_placement::{Placement, Prepared};
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_script::{HandleId, ScriptCall, Value, value::FunctionId};
use pcx_sim::world::{Definition, FieldValue};

pub(super) struct Enumeration {
    native: &'static str,
    group: HandleId,
    candidates: Vec<HandleId>,
    next: usize,
    predicate: FunctionId,
}

impl ObjectHost {
    /// Admit an initialization collision cache before any unit is created.
    /// This does not alias authored placements to executed identities.
    pub fn admit_unit_placement(&mut self, placement: Placement) -> Result<(), Error> {
        let fail = |detail| self.inner.error("CreateUnit", Failure::UnitState(detail));
        if self.unit_placement.is_some() || self.units.is_none() {
            return Err(fail(
                "unit catalog required, placement must be admitted once",
            ));
        }
        if self
            .inner
            .arena
            .iter()
            .any(|e| matches!(e, Entry::Unit { .. }))
        {
            return Err(fail("placement must precede executed units"));
        }
        if self
            .units
            .as_ref()
            .is_none_or(|u| u.source() != placement.source())
        {
            return Err(fail("placement and unit catalog sources differ"));
        }
        for (name, parameters, result) in [
            ("GetUnitX", &["unit"][..], "real"),
            ("GetUnitY", &["unit"][..], "real"),
            (
                "GroupEnumUnitsInRect",
                &["group", "rect", "boolexpr"][..],
                "nothing",
            ),
            (
                "GroupEnumUnitsOfPlayer",
                &["group", "player", "boolexpr"][..],
                "nothing",
            ),
        ] {
            if name == "GroupEnumUnitsOfPlayer" && self.inner.program.natives().get(name).is_none()
            {
                continue;
            }
            Self::contract(
                &self.inner,
                name,
                parameters,
                result,
                matches!(name, "GetUnitX" | "GetUnitY"),
                None,
            )?;
            if self
                .inner
                .program
                .natives()
                .get(name)
                .expect("checked declaration")
                .source
                != self
                    .inner
                    .program
                    .natives()
                    .get("CreateUnit")
                    .expect("admitted unit declaration")
                    .source
            {
                return Err(self.inner.error(name, Failure::Source));
            }
        }
        if self.inner.program.natives().get("GetFilterUnit").is_some() {
            Self::contract(
                &self.inner,
                "GetFilterUnit",
                &[],
                "unit",
                true,
                Some("unit"),
            )?;
            if self
                .inner
                .program
                .natives()
                .get("GetFilterUnit")
                .expect("checked declaration")
                .source
                != self
                    .inner
                    .program
                    .natives()
                    .get("CreateUnit")
                    .expect("unit admission")
                    .source
            {
                return Err(self.inner.error("GetFilterUnit", Failure::Source));
            }
        }
        if self.group_limit.is_none() || !self.rectangles {
            return Err(fail("group and rectangle admission required"));
        }
        self.unit_placement = Some(placement);
        Ok(())
    }

    pub(super) fn prepare_unit_position(
        &mut self,
        record: &ExecutedUnit,
    ) -> Result<Option<Prepared>, Error> {
        let Some(placement) = &mut self.unit_placement else {
            return Ok(None);
        };
        let fail = |text| self.inner.error("CreateUnit", Failure::UnitState(text));
        let (radius, path) = geometry(&record.definition).map_err(fail)?;
        placement
            .prepare([record.x, record.y], radius, path, record.face)
            .map(Some)
            .map_err(|e| {
                self.inner
                    .error("CreateUnit", Failure::UnitPlacement(e.to_string()))
            })
    }

    fn rectangle_candidates(
        &self,
        group: &Value,
        rect: HandleId,
    ) -> Result<(HandleId, Vec<HandleId>), Error> {
        let name = "GroupEnumUnitsInRect";
        let group = self.inner.id(self.group_index(group, name)?, name)?;
        let [left, bottom, right, top] = self
            .rectangle_bounds(rect)
            .map_err(|e| self.inner.error(name, e.failure))?;
        let mut found = Vec::new();
        for (index, entry) in self.inner.arena.iter().enumerate() {
            if let Entry::Unit { record } = entry {
                if !record.shown {
                    continue;
                }
                let [x, y] = record.position.ok_or_else(|| {
                    self.inner
                        .error(name, Failure::UnitState("accepted unit position missing"))
                })?;
                // shortcut: centre containment and creation order; qualify native edge/order traces before widening fidelity claims.
                if left <= x && x <= right && bottom <= y && y <= top {
                    found
                        .try_reserve(1)
                        .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                    found.push(self.inner.id(index, name)?);
                }
            }
        }
        Ok((group, found))
    }

    pub(super) fn begin_rectangle_call(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Result<Option<ScriptCall>, Error> {
        let name = match name {
            "GroupEnumUnitsInRect" => "GroupEnumUnitsInRect",
            "GroupEnumUnitsOfPlayer" => "GroupEnumUnitsOfPlayer",
            _ => return Ok(None),
        };
        if self.unit_placement.is_none() || self.inner.program.natives().get(name).is_none() {
            return Ok(None);
        }
        if matches!(args, [_, _, Value::Null]) {
            return Ok(None);
        }
        let [group, Value::Handle(subject), Value::Handle(filter)] = args else {
            return Err(self.inner.error(
                name,
                Failure::UnitState("group, query handle and boolean expression required"),
            ));
        };
        let predicate = match self.inner.entry(*filter)? {
            Entry::Filter { .. } => self.filter_callback(*filter)?.function,
            Entry::Condition { .. } => self.condition_callback(*filter)?.function,
            _ => return Err(self.inner.error(name, Failure::WrongFamily)),
        };
        let (group, candidates) = if name == "GroupEnumUnitsOfPlayer" {
            self.player_candidates(group, *subject)?
        } else {
            self.rectangle_candidates(group, *subject)?
        };
        self.begin_unit_enumeration(name, group, candidates, Some(predicate))
    }

    fn player_candidates(
        &self,
        group: &Value,
        player: HandleId,
    ) -> Result<(HandleId, Vec<HandleId>), Error> {
        let name = "GroupEnumUnitsOfPlayer";
        let group = self.inner.id(self.group_index(group, name)?, name)?;
        self.player_settings(player)
            .map_err(|e| self.inner.error(name, e.failure))?;
        let mut candidates = Vec::new();
        for (index, entry) in self.inner.arena.iter().enumerate() {
            // shortcut: snapshot extant units in creation order; qualify native mutation/order traces before changing this policy.
            if matches!(entry, Entry::Unit { record } if record.owner == player) {
                candidates
                    .try_reserve(1)
                    .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                candidates.push(self.inner.id(index, name)?);
            }
        }
        Ok((group, candidates))
    }

    /// Replace `group` with `candidates` that pass `predicate`, in order. Without a
    /// predicate every candidate joins immediately.
    pub(super) fn begin_unit_enumeration(
        &mut self,
        name: &'static str,
        group: HandleId,
        candidates: Vec<HandleId>,
        predicate: Option<FunctionId>,
    ) -> Result<Option<ScriptCall>, Error> {
        let Some(predicate) = predicate.filter(|_| !candidates.is_empty()) else {
            let retained = self.members.iter().filter(|(g, _)| *g != group).count();
            if candidates.len() > self.group_limit.unwrap_or(0).saturating_sub(retained) {
                return Err(self.inner.error(name, Failure::Budget("group membership")));
            }
            self.members
                .try_reserve(candidates.len())
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            self.members.retain(|(g, _)| *g != group);
            self.members
                .extend(candidates.into_iter().map(|unit| (group, unit)));
            return Ok(Some(ScriptCall::Return(Value::Null)));
        };
        if self.callbacks.len() >= pcx_script::eval::MAX_CALL_DEPTH as usize {
            return Err(self
                .inner
                .error(name, Failure::Budget("nested native callbacks")));
        }
        self.callbacks
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        self.members.retain(|(g, _)| *g != group);
        // shortcut: new births during a predicate join the next query; qualify mutable spatial iteration before extending it.
        self.callbacks.push(NativeCallback::Rectangle(Enumeration {
            native: name,
            group,
            candidates,
            next: 1,
            predicate,
        }));
        Ok(Some(ScriptCall::Call(predicate)))
    }

    pub(super) fn continue_rectangle_call(&mut self, value: Value) -> Result<ScriptCall, Error> {
        let name = match self.callbacks.last() {
            Some(NativeCallback::Rectangle(frame)) => frame.native,
            _ => "unit enumeration",
        };
        let Value::Bool(accepted) = value else {
            return Err(self
                .inner
                .error(name, Failure::UnitState("predicate did not return boolean")));
        };
        let Some(NativeCallback::Rectangle(frame)) = self.callbacks.last() else {
            return Err(self
                .inner
                .error(name, Failure::UnitState("no active unit predicate")));
        };
        let group = frame.group;
        let candidate = frame.candidates[frame.next - 1];
        match self.inner.entry(group)? {
            Entry::Group { live: true } => {}
            _ => return Err(self.inner.error(name, Failure::Identity)),
        }
        if accepted && !self.members.contains(&(group, candidate)) {
            if self.members.len() >= self.group_limit.unwrap_or(0) {
                return Err(self.inner.error(name, Failure::Budget("group membership")));
            }
            self.members
                .try_reserve(1)
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            self.members.push((group, candidate));
        }
        let Some(NativeCallback::Rectangle(frame)) = self.callbacks.last_mut() else {
            unreachable!("active rectangle predicate");
        };
        if frame.next < frame.candidates.len() {
            frame.next += 1;
            return Ok(ScriptCall::Call(frame.predicate));
        }
        self.callbacks.pop();
        Ok(ScriptCall::Return(Value::Null))
    }

    pub(super) fn unit_position_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.unit_placement.as_ref()?;
        if name == "GetFilterUnit" {
            self.inner.program.natives().get(name)?;
            return Some(if args.is_empty() {
                Ok(self
                    .callbacks
                    .iter()
                    .rev()
                    .find_map(|frame| match frame {
                        NativeCallback::Rectangle(frame) => {
                            Some(Value::Handle(frame.candidates[frame.next - 1]))
                        }
                        _ => None,
                    })
                    .or_else(|| self.range_filter_unit().map(Value::Handle))
                    .or_else(|| self.events.subject.unit.map(Value::Handle))
                    .unwrap_or(Value::Null))
            } else {
                Err(self
                    .inner
                    .error(name, Failure::UnitState("no arguments expected")))
            });
        }
        if !matches!(
            name,
            "GetUnitX" | "GetUnitY" | "GroupEnumUnitsInRect" | "GroupEnumUnitsOfPlayer"
        ) {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some((|| {
            let fail = |detail| self.inner.error(name, Failure::UnitState(detail));
            if matches!(name, "GetUnitX" | "GetUnitY") {
                let [Value::Handle(id)] = args else {
                    return Err(fail("one executed unit required"));
                };
                let position = self
                    .executed_unit(*id)
                    .map_err(|e| self.inner.error(name, e.failure))?
                    .position
                    .ok_or_else(|| fail("accepted unit position missing"))?;
                return Ok(Value::Real(position[usize::from(name == "GetUnitY")]));
            }
            let [group, Value::Handle(subject), Value::Null] = args else {
                return Err(fail(
                    "filtered unit enumeration requires the owned VM callback path",
                ));
            };
            let (group, found) = if name == "GroupEnumUnitsOfPlayer" {
                self.player_candidates(group, *subject)?
            } else {
                self.rectangle_candidates(group, *subject)?
            };
            let retained = self.members.iter().filter(|(g, _)| *g != group).count();
            if found.len() > self.group_limit.unwrap_or(0).saturating_sub(retained) {
                return Err(self.inner.error(name, Failure::Budget("group membership")));
            }
            self.members
                .try_reserve(found.len())
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            self.members.retain(|(g, _)| *g != group);
            self.members
                .extend(found.into_iter().map(|unit| (group, unit)));
            Ok(Value::Null)
        })())
    }
}

pub(super) fn field<'a>(
    definition: &'a Definition,
    slk: SlkId,
    column: &str,
    kinds: &[FieldKind],
) -> Result<&'a FieldValue, &'static str> {
    let mut found = definition.fields.iter().filter(|(id, f)| f.metadata.as_ref().is_some_and(|m|
        m.fourcc == **id && m.from == SlkId::UnitMetaData && m.index == -1
        && kinds.contains(&m.kind)
        && matches!(&m.source, FieldSource::Column { slk: s, column: c } if *s == slk && c == column)));
    let (_, value) = found.next().ok_or("missing placement catalog field")?;
    if found.next().is_some() {
        return Err("ambiguous placement catalog field");
    }
    value
        .value
        .as_ref()
        .map_err(|_| "unsupported placement catalog field")
}

pub(super) fn text<'a>(
    definition: &'a Definition,
    slk: SlkId,
    column: &str,
) -> Result<&'a str, &'static str> {
    match field(definition, slk, column, &[FieldKind::Token])? {
        FieldValue::Cell(value) => Ok(value),
        FieldValue::Override(pcx_objects::Value::Str(value)) => {
            std::str::from_utf8(value).map_err(|_| "non-UTF8 placement field")
        }
        _ => Err("text placement field required"),
    }
}

pub(super) fn geometry(definition: &Definition) -> Result<(Option<f32>, &str), &'static str> {
    let movement = text(definition, SlkId::UnitData, "movetp")?;
    let path = text(definition, SlkId::UnitData, "pathTex")?;
    let radius = match movement {
        "_" | "none" | "" => None,
        "foot" | "horse" => {
            let value = field(
                definition,
                SlkId::UnitBalance,
                "collision",
                &[FieldKind::Real, FieldKind::Unreal],
            )?;
            let radius = match value {
                FieldValue::Cell(value) => value.parse::<f32>().ok(),
                FieldValue::Override(
                    pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value),
                ) => Some(*value),
                _ => None,
            }
            .filter(|v| v.is_finite() && *v >= 0.0)
            .ok_or("finite collision radius required")?;
            Some(radius)
        }
        _ => return Err("movement type is outside admitted creation placement"),
    };
    Ok((radius, path))
}
