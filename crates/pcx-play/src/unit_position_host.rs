use super::{Entry, Error, ExecutedUnit, Failure, ObjectHost};
use crate::unit_placement::{Placement, Prepared};
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_script::Value;
use pcx_sim::world::{Definition, FieldValue};

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
        ] {
            Self::contract(
                &self.inner,
                name,
                parameters,
                result,
                name != "GroupEnumUnitsInRect",
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
        if self.group_limit.is_none() || !self.rectangles {
            return Err(fail("group and rectangle admission required"));
        }
        self.unit_placement = Some(placement);
        Ok(())
    }

    pub(super) fn prepare_unit_position(
        &self,
        record: &ExecutedUnit,
    ) -> Result<Option<Prepared>, Error> {
        let Some(placement) = &self.unit_placement else {
            return Ok(None);
        };
        let fail = |text| self.inner.error("CreateUnit", Failure::UnitState(text));
        let movement = text(&record.definition, SlkId::UnitData, "movetp").map_err(fail)?;
        let path = text(&record.definition, SlkId::UnitData, "pathTex").map_err(fail)?;
        let radius = match movement {
            "_" | "none" | "" => None,
            "foot" | "horse" => {
                let value =
                    field(&record.definition, SlkId::UnitBalance, "collision").map_err(fail)?;
                let radius = match value {
                    FieldValue::Cell(value) => value.parse::<f32>().ok(),
                    FieldValue::Override(
                        pcx_objects::Value::Real(value) | pcx_objects::Value::Unreal(value),
                    ) => Some(*value),
                    _ => None,
                }
                .filter(|v| v.is_finite() && *v >= 0.0)
                .ok_or_else(|| fail("finite collision radius required"))?;
                Some(radius)
            }
            _ => return Err(fail("movement type is outside admitted creation placement")),
        };
        placement
            .prepare([record.x, record.y], radius, path, record.face)
            .map(Some)
            .map_err(|e| {
                self.inner
                    .error("CreateUnit", Failure::UnitPlacement(e.to_string()))
            })
    }

    pub(super) fn unit_position_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        self.unit_placement.as_ref()?;
        if !matches!(name, "GetUnitX" | "GetUnitY" | "GroupEnumUnitsInRect") {
            return None;
        }
        Some((|| {
            let fail = |detail| self.inner.error(name, Failure::UnitState(detail));
            if name != "GroupEnumUnitsInRect" {
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
            let [group, Value::Handle(rect), Value::Null] = args else {
                return Err(fail(
                    "rectangle enumeration requires an explicit null filter",
                ));
            };
            let index = self.group_index(group, name)?;
            let group = self.inner.id(index, name)?;
            // shortcut: populated-group and filter semantics need explicit qualification before admission.
            if self.members.iter().any(|(g, _)| *g == group) {
                return Err(fail("enumeration into a populated group is not admitted"));
            }
            let [left, bottom, right, top] = self
                .rectangle_bounds(*rect)
                .map_err(|e| self.inner.error(name, e.failure))?;
            let mut found = Vec::new();
            for (index, entry) in self.inner.arena.iter().enumerate() {
                if let Entry::Unit { record } = entry {
                    let [x, y] = record
                        .position
                        .ok_or_else(|| fail("accepted unit position missing"))?;
                    // shortcut: centre containment and creation order; qualify native edge/order traces before widening fidelity claims.
                    if left <= x && x <= right && bottom <= y && y <= top {
                        if self.members.len() + found.len() >= self.group_limit.unwrap_or(0) {
                            return Err(self
                                .inner
                                .error(name, Failure::Budget("group membership")));
                        }
                        found
                            .try_reserve(1)
                            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                        found.push((group, self.inner.id(index, name)?));
                    }
                }
            }
            self.members
                .try_reserve(found.len())
                .map_err(|_| self.inner.error(name, Failure::Capacity))?;
            self.members.extend(found);
            Ok(Value::Null)
        })())
    }
}

fn field<'a>(
    definition: &'a Definition,
    slk: SlkId,
    column: &str,
) -> Result<&'a FieldValue, &'static str> {
    let mut found = definition.fields.iter().filter(|(id, f)| f.metadata.as_ref().is_some_and(|m|
        m.fourcc == **id && m.from == SlkId::UnitMetaData && m.index == -1
        && if slk == SlkId::UnitBalance { matches!(m.kind, FieldKind::Real | FieldKind::Unreal) } else { m.kind == FieldKind::Token }
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

fn text<'a>(definition: &'a Definition, slk: SlkId, column: &str) -> Result<&'a str, &'static str> {
    match field(definition, slk, column)? {
        FieldValue::Cell(value) => Ok(value),
        FieldValue::Override(pcx_objects::Value::Str(value)) => {
            std::str::from_utf8(value).map_err(|_| "non-UTF8 placement field")
        }
        _ => Err("text placement field required"),
    }
}
