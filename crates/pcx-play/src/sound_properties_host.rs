//! Last executed sound-property assignments, without playback defaults or units.
use super::{Entry, Error, Failure, ObjectHost};
use pcx_script::{HandleId, Value};

/// Unset fields have not been assigned by these native calls. Label parameters
/// and creation flags remain separate; this is not an effective mixer state.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SoundProperties {
    pub channel: Option<i32>,
    pub volume: Option<i32>,
    pub pitch: Option<f32>,
    pub distance_cutoff: Option<f32>,
    pub distances: Option<[f32; 2]>,
    pub cone_angles: Option<(f32, f32, i32)>,
    pub cone_orientation: Option<[f32; 3]>,
    pub position: Option<[f32; 3]>,
    pub velocity: Option<[f32; 3]>,
    /// Live executed unit from AttachSoundToUnit; a later SetSoundPosition clears it.
    pub attached_unit: Option<HandleId>,
}

const ATTACH: &str = "AttachSoundToUnit";

const CONTRACTS: &[(&str, &[&str])] = &[
    ("SetSoundChannel", &["sound", "integer"]),
    ("SetSoundVolume", &["sound", "integer"]),
    ("SetSoundPitch", &["sound", "real"]),
    ("SetSoundDistanceCutoff", &["sound", "real"]),
    ("SetSoundDistances", &["sound", "real", "real"]),
    ("SetSoundConeAngles", &["sound", "real", "real", "integer"]),
    (
        "SetSoundConeOrientation",
        &["sound", "real", "real", "real"],
    ),
    ("SetSoundPosition", &["sound", "real", "real", "real"]),
    ("SetSoundVelocity", &["sound", "real", "real", "real"]),
];

impl ObjectHost {
    pub fn with_sound_properties(mut self) -> Result<Self, Error> {
        if self.sound_properties || !self.has_sound_resources() {
            return Err(self.inner.error(
                "SetSoundPosition",
                Failure::SoundState("sound resources required; properties must be admitted once"),
            ));
        }
        for &(name, parameters) in CONTRACTS {
            Self::contract(
                &self.inner,
                name,
                parameters,
                "nothing",
                false,
                Some("sound"),
            )?;
        }
        self.sound_properties = true;
        Ok(self)
    }

    pub fn sound_properties(&self, id: HandleId) -> Result<&SoundProperties, Error> {
        match self.inner.entry(id)? {
            Entry::Sound { properties, .. } | Entry::FileSound { properties, .. } => Ok(properties),
            _ => Err(self.inner.error("sound properties", Failure::WrongFamily)),
        }
    }

    pub(super) fn sound_properties_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if !self.sound_properties || !CONTRACTS.iter().any(|&(expected, _)| expected == name) {
            return None;
        }
        Some(self.assign_sound_property(name, args))
    }

    /// Checked at unit-catalog admission so attachment shares the unit source.
    pub(super) fn sound_attachment_declaration(&self, source: &str) -> Result<(), Error> {
        let Some(native) = self.inner.program.natives().get(ATTACH) else {
            return Ok(());
        };
        Self::contract(
            &self.inner,
            ATTACH,
            &["sound", "unit"],
            "nothing",
            false,
            Some("sound"),
        )?;
        if native.source != source {
            return Err(self.inner.error(ATTACH, Failure::Source));
        }
        Ok(())
    }

    pub(super) fn sound_attachment_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != ATTACH
            || !self.sound_properties
            || self.units.is_none()
            || self.inner.program.natives().get(ATTACH).is_none()
        {
            return None;
        }
        Some((|| {
            let fail = |failure| self.inner.error(ATTACH, failure);
            let (sound, unit) = match args {
                [Value::Handle(sound), Value::Handle(unit)] => (*sound, *unit),
                // Native wrapper evidence: a null sound or unit skips without detaching.
                [
                    Value::Null | Value::Handle(_),
                    Value::Null | Value::Handle(_),
                ] => {
                    return Ok(Value::Null);
                }
                _ => return Err(fail(Failure::Signature)),
            };
            self.sound_admitted(sound).map_err(|e| fail(e.failure))?;
            self.executed_unit(unit).map_err(|e| fail(e.failure))?;
            let at = usize::try_from(sound.token() - 1).map_err(|_| fail(Failure::Identity))?;
            let (Entry::Sound { properties, .. } | Entry::FileSound { properties, .. }) =
                &mut self.inner.arena[at]
            else {
                return Err(self.inner.error(ATTACH, Failure::WrongFamily));
            };
            properties.attached_unit = Some(unit);
            Ok(Value::Null)
        })())
    }

    /// Current accepted ground position of the unit a sound follows, if attached.
    pub fn sound_attached_position(&self, id: HandleId) -> Result<Option<[f32; 2]>, Error> {
        let Some(unit) = self.sound_properties(id)?.attached_unit else {
            return Ok(None);
        };
        if matches!(self.inner.entry(unit)?, Entry::RemovedUnit { .. }) {
            return Ok(None);
        }
        let record = self.executed_unit(unit)?;
        record.position.map(Some).ok_or_else(|| {
            self.inner
                .error(ATTACH, Failure::UnitState("accepted position missing"))
        })
    }

    fn assign_sound_property(&mut self, name: &str, args: &[Value]) -> Result<Value, Error> {
        let fail = |reason| self.inner.error(name, Failure::SoundState(reason));
        let (Value::Handle(id), tail) = args
            .split_first()
            .ok_or_else(|| fail("sound identity required"))?
        else {
            return Err(fail("sound identity required"));
        };
        self.sound_admitted(*id)
            .map_err(|e| self.inner.error(name, e.failure))?;
        if matches!(
            name,
            "SetSoundDistances"
                | "SetSoundConeAngles"
                | "SetSoundConeOrientation"
                | "SetSoundPosition"
                | "SetSoundVelocity"
        ) && !matches!(
            self.inner.entry(*id)?,
            Entry::FileSound { is_3d: true, .. }
                | Entry::Sound {
                    creation_flags: Some(super::LabelSoundFlags { is_3d: true, .. }),
                    ..
                }
        ) {
            return Err(fail("explicit 3D creation required"));
        }
        let mut next = *self.sound_properties(*id)?;
        let bad = || fail("arguments do not match the finite sound-property declaration");
        match (name, tail) {
            ("SetSoundChannel", [Value::Int(value)]) => next.channel = Some(*value),
            ("SetSoundVolume", [Value::Int(value)]) => next.volume = Some(*value),
            ("SetSoundPitch", values) => next.pitch = Some(reals::<1>(values).ok_or_else(bad)?[0]),
            ("SetSoundDistanceCutoff", values) => {
                next.distance_cutoff = Some(reals::<1>(values).ok_or_else(bad)?[0])
            }
            ("SetSoundDistances", values) => next.distances = Some(reals(values).ok_or_else(bad)?),
            ("SetSoundConeAngles", [_, _, Value::Int(volume)]) => {
                let [a, b] = reals(&tail[..2]).ok_or_else(bad)?;
                next.cone_angles = Some((a, b, *volume));
            }
            ("SetSoundConeOrientation", values) => {
                next.cone_orientation = Some(reals(values).ok_or_else(bad)?)
            }
            ("SetSoundPosition", values) => {
                next.position = Some(reals(values).ok_or_else(bad)?);
                next.attached_unit = None;
            }
            ("SetSoundVelocity", values) => next.velocity = Some(reals(values).ok_or_else(bad)?),
            _ => return Err(bad()),
        }
        let at = usize::try_from(id.token() - 1)
            .map_err(|_| self.inner.error(name, Failure::Identity))?;
        let (Entry::Sound { properties, .. } | Entry::FileSound { properties, .. }) =
            &mut self.inner.arena[at]
        else {
            return Err(self.inner.error(name, Failure::WrongFamily));
        };
        *properties = next;
        Ok(Value::Null)
    }
}

fn reals<const N: usize>(values: &[Value]) -> Option<[f32; N]> {
    if values.len() != N {
        return None;
    }
    let mut result = [0.0; N];
    for (out, value) in result.iter_mut().zip(values) {
        *out = value.as_real().filter(|v| v.is_finite())?;
    }
    Some(result)
}
