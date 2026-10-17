//! Stored camera-setup fields and destination. Nothing here moves a camera or advances time.
use super::super::{Entry, Error, Failure};
use super::ObjectHost;
use pcx_script::{HandleId, Value};

/// One executed field assignment. The enumerand is the admitted `ConvertCameraField` value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraFieldRecord {
    pub enumerand: i32,
    pub value: f32,
    pub duration: f32,
}

/// One executed destination. Unset setups do not receive a constructed point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraDestination {
    pub x: f32,
    pub y: f32,
    pub duration: f32,
}

pub(super) struct Admission {
    max_fields: usize,
}

const FIELD_NATIVE: &str = "ConvertCameraField";
const SETUP_KIND: &str = "camerasetup";
const FIELD_KIND: &str = "camerafield";

impl ObjectHost {
    /// Admit setup allocation on the shared arena.
    /// `max_fields` bounds distinct field records stored on one setup.
    pub fn with_camera_setups(mut self, max_fields: usize) -> Result<Self, Error> {
        if self.camera_setups.is_some() {
            return Err(self.inner.error(
                "CreateCameraSetup",
                Failure::CameraSetupObject("already admitted"),
            ));
        }
        std::alloc::Layout::array::<CameraFieldRecord>(max_fields)
            .map_err(|_| self.inner.error("CreateCameraSetup", Failure::Capacity))?;
        let field = self
            .inner
            .conversions()
            .find(|c| c.name == FIELD_NATIVE)
            .ok_or_else(|| {
                self.inner.error(
                    FIELD_NATIVE,
                    Failure::CameraSetupObject(
                        "ConvertCameraField is not admitted in this namespace",
                    ),
                )
            })?;
        if field.kind != FIELD_KIND {
            return Err(self.inner.error(FIELD_NATIVE, Failure::Signature));
        }
        let field_source = field.source.clone();
        for (name, parameters, result) in [
            ("CreateCameraSetup", &[][..], SETUP_KIND),
            (
                "CameraSetupSetField",
                &[SETUP_KIND, FIELD_KIND, "real", "real"][..],
                "nothing",
            ),
            (
                "CameraSetupSetDestPosition",
                &[SETUP_KIND, "real", "real", "real"][..],
                "nothing",
            ),
        ] {
            self.bind_setup_native(name, parameters, result, &field_source)?;
        }
        for (name, parameters, result) in [
            ("CameraSetupGetField", &[SETUP_KIND, FIELD_KIND][..], "real"),
            ("CameraSetupGetDestPositionX", &[SETUP_KIND][..], "real"),
            ("CameraSetupGetDestPositionY", &[SETUP_KIND][..], "real"),
        ] {
            if self.inner.program.natives().get(name).is_some() {
                self.bind_setup_native(name, parameters, result, &field_source)?;
            }
        }
        self.camera_setups = Some(Admission { max_fields });
        Ok(self)
    }

    fn bind_setup_native(
        &self,
        name: &str,
        parameters: &[&str],
        result: &str,
        field_source: &str,
    ) -> Result<(), Error> {
        Self::contract_in(
            &self.inner,
            name,
            parameters,
            result,
            false,
            Some(SETUP_KIND),
            false,
        )?;
        let native = self
            .inner
            .program
            .natives()
            .get(name)
            .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
        if native.source != field_source {
            return Err(self.inner.error(name, Failure::Source));
        }
        Ok(())
    }

    pub fn camera_setup_field(
        &self,
        setup: HandleId,
        field: HandleId,
    ) -> Result<(f32, f32), Error> {
        let enumerand = self.field_enumerand(&Value::Handle(field), "CameraSetupGetField")?;
        let record = self
            .setup_fields(setup, "CameraSetupGetField")?
            .iter()
            .find(|record| record.enumerand == enumerand)
            .copied()
            .ok_or_else(|| {
                self.inner.error(
                    "CameraSetupGetField",
                    Failure::CameraSetupObject("field is unset"),
                )
            })?;
        Ok((record.value, record.duration))
    }

    pub fn camera_setup_destination(&self, setup: HandleId) -> Result<CameraDestination, Error> {
        let destination = match self.setup_entry(setup, "CameraSetupGetDestPositionX")? {
            Entry::CameraSetup { destination, .. } => *destination,
            _ => {
                return Err(self
                    .inner
                    .error("CameraSetupGetDestPositionX", Failure::WrongFamily));
            }
        };
        destination.ok_or_else(|| {
            self.inner.error(
                "CameraSetupGetDestPositionX",
                Failure::CameraSetupObject("destination is unset"),
            )
        })
    }

    pub(super) fn camera_setup_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if self.camera_setups.is_none()
            || !matches!(
                name,
                "CreateCameraSetup"
                    | "CameraSetupSetField"
                    | "CameraSetupGetField"
                    | "CameraSetupSetDestPosition"
                    | "CameraSetupGetDestPositionX"
                    | "CameraSetupGetDestPositionY"
            )
        {
            return None;
        }
        self.inner.program.natives().get(name)?;
        Some(self.camera_setup(name, args))
    }

    fn camera_setup(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |detail| self.inner.error(name, Failure::CameraSetupObject(detail));
        if name == "CreateCameraSetup" {
            if !args.is_empty() {
                return Err(fail("arguments do not match the camera setup declaration"));
            }
            return self
                .inner
                .allocate(
                    Entry::CameraSetup {
                        fields: Vec::new(),
                        destination: None,
                    },
                    name,
                )
                .map(Value::Handle);
        }
        let (Value::Handle(setup), rest) = args
            .split_first()
            .ok_or_else(|| fail("camera setup handle required"))?
        else {
            return Err(fail("camera setup handle required"));
        };
        let at = self.setup_index(*setup, name)?;
        match name {
            "CameraSetupGetField" => {
                let [field] = rest else {
                    return Err(fail("arguments do not match the camera setup declaration"));
                };
                let enumerand = self.field_enumerand(field, name)?;
                let record = self
                    .setup_fields(*setup, name)?
                    .iter()
                    .find(|record| record.enumerand == enumerand);
                record
                    .map(|record| Value::Real(record.value))
                    .ok_or_else(|| fail("field is unset"))
            }
            "CameraSetupGetDestPositionX" | "CameraSetupGetDestPositionY" => {
                if !rest.is_empty() {
                    return Err(fail("arguments do not match the camera setup declaration"));
                }
                let Entry::CameraSetup { destination, .. } = &self.inner.arena[at] else {
                    return Err(self.inner.error(name, Failure::WrongFamily));
                };
                let Some(destination) = destination else {
                    return Err(fail("destination is unset"));
                };
                Ok(Value::Real(if name.ends_with('X') {
                    destination.x
                } else {
                    destination.y
                }))
            }
            "CameraSetupSetField" => {
                let [field, value, duration] = rest else {
                    return Err(fail("arguments do not match the camera setup declaration"));
                };
                let enumerand = self.field_enumerand(field, name)?;
                let value =
                    finite_real(value).ok_or_else(|| fail("finite value and duration required"))?;
                let duration = finite_real(duration)
                    .ok_or_else(|| fail("finite value and duration required"))?;
                let max_fields = self
                    .camera_setups
                    .as_ref()
                    .ok_or_else(|| fail("already admitted"))?
                    .max_fields;
                let decision = match &self.inner.arena[at] {
                    Entry::CameraSetup { fields, .. } => {
                        if let Some(index) = fields
                            .iter()
                            .position(|record| record.enumerand == enumerand)
                        {
                            FieldWrite::Update(index)
                        } else if fields.len() >= max_fields {
                            FieldWrite::Full
                        } else {
                            FieldWrite::Insert
                        }
                    }
                    _ => FieldWrite::Foreign,
                };
                match decision {
                    FieldWrite::Foreign => Err(self.inner.error(name, Failure::WrongFamily)),
                    FieldWrite::Full => Err(self.inner.error(name, Failure::Capacity)),
                    FieldWrite::Update(index) => {
                        let Entry::CameraSetup { fields, .. } = &mut self.inner.arena[at] else {
                            return Err(self.inner.error(name, Failure::WrongFamily));
                        };
                        let record = &mut fields[index];
                        record.value = value;
                        record.duration = duration;
                        Ok(Value::Null)
                    }
                    FieldWrite::Insert => {
                        let reserved = {
                            let Entry::CameraSetup { fields, .. } = &mut self.inner.arena[at]
                            else {
                                return Err(self.inner.error(name, Failure::WrongFamily));
                            };
                            fields.try_reserve(1)
                        };
                        if reserved.is_err() {
                            return Err(self.inner.error(name, Failure::Capacity));
                        }
                        let Entry::CameraSetup { fields, .. } = &mut self.inner.arena[at] else {
                            return Err(self.inner.error(name, Failure::WrongFamily));
                        };
                        fields.push(CameraFieldRecord {
                            enumerand,
                            value,
                            duration,
                        });
                        Ok(Value::Null)
                    }
                }
            }
            "CameraSetupSetDestPosition" => {
                let [x, y, duration] =
                    finite_reals(rest).ok_or_else(|| fail("finite destination required"))?;
                let Entry::CameraSetup { destination, .. } = &mut self.inner.arena[at] else {
                    return Err(self.inner.error(name, Failure::WrongFamily));
                };
                *destination = Some(CameraDestination { x, y, duration });
                Ok(Value::Null)
            }
            _ => Err(fail("arguments do not match the camera setup declaration")),
        }
    }

    fn field_enumerand(&self, value: &Value, name: &str) -> Result<i32, Error> {
        let Value::Handle(id) = value else {
            return Err(self.inner.error(
                name,
                Failure::CameraSetupObject(
                    "camera field handle required; raw values are not a field in this namespace",
                ),
            ));
        };
        let kind = FIELD_KIND;
        match self.inner.entry(*id) {
            Ok(Entry::Symbolic {
                kind: actual,
                enumerand,
            }) if actual == kind => Ok(*enumerand),
            Ok(_) => Err(self.inner.error(name, Failure::WrongFamily)),
            Err(error) => Err(self.inner.error(name, error.failure)),
        }
    }

    fn setup_index(&self, id: HandleId, name: &str) -> Result<usize, Error> {
        let at = usize::try_from(id.token() - 1)
            .map_err(|_| self.inner.error(name, Failure::Identity))?;
        match self.inner.arena.get(at) {
            Some(Entry::CameraSetup { .. }) => Ok(at),
            Some(_) => Err(self.inner.error(name, Failure::WrongFamily)),
            None => Err(self.inner.error(name, Failure::Identity)),
        }
    }

    fn setup_entry(&self, id: HandleId, name: &str) -> Result<&Entry, Error> {
        let at = self.setup_index(id, name)?;
        self.inner
            .arena
            .get(at)
            .ok_or_else(|| self.inner.error(name, Failure::Identity))
    }

    fn setup_fields(&self, id: HandleId, name: &str) -> Result<&[CameraFieldRecord], Error> {
        match self.setup_entry(id, name)? {
            Entry::CameraSetup { fields, .. } => Ok(fields),
            _ => Err(self.inner.error(name, Failure::WrongFamily)),
        }
    }
}

enum FieldWrite {
    Update(usize),
    Insert,
    Full,
    Foreign,
}

fn finite_real(value: &Value) -> Option<f32> {
    match value {
        Value::Real(value) if value.is_finite() => Some(*value),
        _ => None,
    }
}

fn finite_reals<const N: usize>(values: &[Value]) -> Option<[f32; N]> {
    let mut out = [0.0; N];
    if values.len() != N {
        return None;
    }
    for (out, value) in out.iter_mut().zip(values) {
        let Value::Real(value) = value else {
            return None;
        };
        if !value.is_finite() {
            return None;
        }
        *out = *value;
    }
    Some(out)
}

#[cfg(test)]
#[path = "camera_setup_host_tests.rs"]
mod tests;
