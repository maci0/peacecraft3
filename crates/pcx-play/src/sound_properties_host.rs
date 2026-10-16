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
}

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
        if self.sound_properties || (self.sounds.is_none() && self.file_sounds.is_none()) {
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
        ) && !matches!(self.inner.entry(*id)?, Entry::FileSound { is_3d: true, .. })
        {
            return Err(fail(
                "explicit 3D creation required; label flags are not interpreted",
            ));
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
            ("SetSoundPosition", values) => next.position = Some(reals(values).ok_or_else(bad)?),
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
