//! Source-driven PRE2 CPU state; no camera, material, image or GPU side effects.
use pcx_mdl::{Interpolation, KeyTrack, NO_ID, Particle2, Particle2Properties, Pivots, Texture};
use std::{sync::Arc, time::Duration};

// Public primary PRE2 flag values; these are format identifiers, not asset data.
// war3-model model.ts NodeType.ParticleEmitter (also PRE2), pinned primary convention.
pub const PARTICLE_EMITTER_TYPE: u32 = 0x1000;
pub const UNSHADED: u32 = 0x8000;
pub const SORT_FAR_Z: u32 = 0x10000;
pub const LINE: u32 = 0x20000;
pub const UNFOGGED: u32 = 0x40000;
pub const MODEL_SPACE: u32 = 0x80000;
pub const XY_QUAD: u32 = 0x100000;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Failure {
    #[error("malformed {0}")]
    Malformed(&'static str),
    #[error("unsupported {0}")]
    Unsupported(&'static str),
    #[error("{kind} budget needs {needed}, limit {limit}")]
    Budget {
        kind: &'static str,
        needed: u128,
        limit: u128,
    },
    #[error("clock overflow")]
    Clock,
    #[error("nonfinite or unrepresentable particle arithmetic")]
    Arithmetic,
    #[error("particle allocation failed")]
    Allocation,
}

#[derive(Debug, thiserror::Error)]
#[error("{source_name} emitter {emitter:?}: {failure}")]
pub struct Error {
    pub source_name: String,
    pub emitter: Option<u32>,
    pub failure: Failure,
}

/// Explicit caller-owned policies; none is an authored emission/quality default.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_emitters: usize,
    pub max_keys: usize,
    pub particle_capacity: usize,
    pub max_ticks: u64,
    pub max_updates: u64,
    pub tick: Duration,
}

#[derive(Clone, Debug)]
struct Key {
    frame: i32,
    value: [f64; 4],
    incoming: [f64; 4],
    outgoing: [f64; 4],
}
#[derive(Clone, Debug)]
struct Channel {
    keys: Vec<Key>,
    interpolation: Interpolation,
    global: Option<u32>,
    range: [u32; 2],
    constant: bool,
    rotation: bool,
}

fn normalize(mut q: [f64; 4]) -> Result<[f64; 4], Failure> {
    let length = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    if !length.is_finite() || length == 0. {
        return Err(Failure::Malformed("zero/nonfinite quaternion"));
    }
    for v in &mut q {
        *v /= length;
    }
    Ok(q)
}
fn slerp(a: [f64; 4], mut b: [f64; 4], t: f64) -> Result<[f64; 4], Failure> {
    let mut dot = a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    if dot < 0. {
        b = b.map(|v| -v);
        dot = -dot;
    }
    // Numerical interpolation policy, independent of asset records.
    let (left, right) = if dot > 1. - 1e-9 {
        (1. - t, t)
    } else {
        let angle = dot.clamp(-1., 1.).acos();
        (
            ((1. - t) * angle).sin() / angle.sin(),
            (t * angle).sin() / angle.sin(),
        )
    };
    normalize(std::array::from_fn(|i| left * a[i] + right * b[i]))
}
fn rotate(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let [x, y, z, w] = q;
    let [a, b, c] = v;
    let t = [
        2. * (y * c - z * b),
        2. * (z * a - x * c),
        2. * (x * b - y * a),
    ];
    [
        a + w * t[0] + y * t[2] - z * t[1],
        b + w * t[1] + z * t[0] - x * t[2],
        c + w * t[2] + x * t[1] - y * t[0],
    ]
}
fn vector(v: [f64; 3]) -> Result<[f32; 3], Failure> {
    if v.iter()
        .any(|x| !x.is_finite() || x.abs() > f64::from(f32::MAX))
    {
        return Err(Failure::Arithmetic);
    }
    Ok(v.map(|x| x as f32))
}

impl Channel {
    fn checked(
        track: &KeyTrack,
        width: usize,
        rotation: bool,
        clip: [u32; 2],
        globals: &[u32],
    ) -> Result<Self, Failure> {
        let stride = 4 + width
            * 4
            * if track.interpolation.has_tangents() {
                3
            } else {
                1
            };
        if track.width != width
            || track.stride != stride
            || (track.count as usize).checked_mul(stride) != Some(track.raw.len())
        {
            return Err(Failure::Malformed("track shape"));
        }
        let global = if track.global_seq_id == NO_ID {
            None
        } else {
            let d = *globals
                .get(track.global_seq_id as usize)
                .ok_or(Failure::Malformed("global reference"))?;
            if d == 0 {
                return Err(Failure::Malformed("zero global duration"));
            }
            Some(d)
        };
        let range = global.map_or(clip, |duration| [0, duration]);
        let mut keys = Vec::new();
        keys.try_reserve_exact(track.count as usize)
            .map_err(|_| Failure::Allocation)?;
        let mut previous = None;
        for i in 0..track.count as usize {
            let frame = track
                .frame_signed(i)
                .ok_or(Failure::Malformed("signed key frame"))?;
            if previous.is_some_and(|old| old >= frame) {
                return Err(Failure::Malformed("duplicate/unordered signed keys"));
            }
            previous = Some(frame);
            let convert = |value: Option<[f32; 4]>| -> Result<[f64; 4], Failure> {
                let v = value
                    .ok_or(Failure::Malformed("track value/tangent"))?
                    .map(f64::from);
                if !v.iter().all(|v| v.is_finite()) {
                    return Err(Failure::Malformed("nonfinite track"));
                }
                Ok(v)
            };
            let mut key = Key {
                frame,
                value: convert(track.value(i))?,
                incoming: convert(track.in_tangent(i))?,
                outgoing: convert(track.out_tangent(i))?,
            };
            if rotation {
                key.value = normalize(key.value)?;
                if track.interpolation.has_tangents() {
                    key.incoming = normalize(key.incoming)?;
                    key.outgoing = normalize(key.outgoing)?;
                }
            }
            if (global.is_some() && i == 0 && i64::from(frame) > i64::from(range[1]))
                || (i64::from(frame) >= i64::from(range[0])
                    && i64::from(frame) <= i64::from(range[1]))
            {
                keys.push(key);
            }
        }
        let constant = keys
            .first()
            .is_some_and(|first| keys.iter().all(|key| key.value == first.value));
        Ok(Self {
            keys,
            constant,
            interpolation: track.interpolation,
            global,
            range,
            rotation,
        })
    }
    fn sample(&self, frame: u32, elapsed: u64, identity: [f64; 4]) -> Result<[f64; 4], Failure> {
        if self.keys.is_empty() {
            return Ok(identity);
        }
        let at = i64::from(
            self.global
                .map_or(frame, |duration| (elapsed % u64::from(duration)) as u32),
        );
        // Pinned sd.ts sequence selection and cyclic end-gap convention.
        if self.constant {
            return Ok(self.keys[0].value);
        }
        let after = self.keys.partition_point(|key| i64::from(key.frame) <= at);
        let (left, right) = if after == 0 || after == self.keys.len() {
            (&self.keys[self.keys.len() - 1], &self.keys[0])
        } else {
            (&self.keys[after - 1], &self.keys[after])
        };
        if self.interpolation == Interpolation::None {
            return Ok(left.value);
        }
        let mut start_frame = i64::from(left.frame);
        let mut between = i64::from(right.frame) - start_frame;
        if between < 0 {
            between += i64::from(self.range[1]) - i64::from(self.range[0]);
            if at < start_frame {
                start_frame = i64::from(right.frame);
            }
        }
        let t = if between == 0 {
            0.
        } else {
            (at - start_frame) as f64 / between as f64
        };
        let result = if self.rotation {
            if self.interpolation == Interpolation::Linear {
                slerp(left.value, right.value, t)?
            } else {
                // Pinned flowtsohg QuatSd uses gl-matrix spherical cubic interpolation.
                slerp(
                    slerp(left.value, right.value, t)?,
                    slerp(left.outgoing, right.incoming, t)?,
                    2. * t * (1. - t),
                )?
            }
        } else {
            std::array::from_fn(|i| {
                let (a, b, c, d) = (
                    left.value[i],
                    left.outgoing[i],
                    right.incoming[i],
                    right.value[i],
                );
                match self.interpolation {
                    Interpolation::None => a,
                    Interpolation::Linear => a + (d - a) * t,
                    Interpolation::Bezier => {
                        let u = 1. - t;
                        u * u * u * a + 3. * u * u * t * b + 3. * u * t * t * c + t * t * t * d
                    }
                    Interpolation::Hermite => {
                        (2. * t * t * t - 3. * t * t + 1.) * a
                            + (t * t * t - 2. * t * t + t) * b
                            + (t * t * t - t * t) * c
                            + (-2. * t * t * t + 3. * t * t) * d
                    }
                }
            })
        };
        if !result.into_iter().all(f64::is_finite) {
            return Err(Failure::Arithmetic);
        }
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub object_id: u32,
    pub properties: Particle2Properties,
    pub flags: u32,
    pub texture_flags: u32,
    pub pivot: [f32; 3],
    translation: Option<Channel>,
    rotation: Option<Channel>,
    visibility: Option<Channel>,
}
#[derive(Clone, Copy, Debug)]
pub struct Clip {
    pub interval: [u32; 2],
    pub looping: bool,
}

#[derive(Debug)]
pub struct Plan {
    source: String,
    profiles: Vec<Profile>,
    interval: [u32; 2],
    looping: bool,
    native_appearance: Option<NativeAppearance>,
}

#[derive(Clone, Debug)]
struct NativeAppearance {
    source: String,
    phase_scale: f32,
    phase_bias: f32,
    colour_scale: f32,
    packing_bias: f32,
    packing_shift: u32,
}

impl NativeAppearance {
    fn phase(&self, phase: f32) -> f32 {
        phase * self.phase_scale + self.phase_bias
    }
    fn pack(&self, value: f32) -> u8 {
        ((value + self.packing_bias).to_bits() >> self.packing_shift) as u8
    }
    fn endpoint(&self, value: f32) -> u8 {
        let scaled = value * self.colour_scale;
        // CVTTSS2SI's indefinite integer has a zero low byte.
        if !scaled.is_finite()
            || f64::from(scaled) < f64::from(i32::MIN)
            || f64::from(scaled) >= (i64::from(i32::MAX) + 1) as f64
        {
            0
        } else {
            scaled as i32 as u8
        }
    }
    fn rgba(&self, properties: &Particle2Properties, segment: usize, phase: f32) -> [f32; 4] {
        std::array::from_fn(|channel| {
            let value = |index: usize| {
                if channel == 3 {
                    properties.segment_alpha[index]
                } else {
                    self.endpoint(properties.segment_colours[index][channel])
                }
            };
            let left = value(segment);
            let delta = i32::from(value(segment + 1)) - i32::from(left);
            f32::from(self.pack(delta as f32 * phase + f32::from(left))) / f32::from(u8::MAX)
        })
    }
}

impl Plan {
    /// Select coefficients read from the caller's qualified native client image.
    /// The ordinary checked plan retains floating-point channels for data-only callers.
    pub fn with_native_appearance(
        mut self,
        source: &str,
        phase: [f32; 2],
        colour: [f32; 2],
        packing_shift: u32,
    ) -> Result<Self, Error> {
        let [phase_scale, phase_bias] = phase;
        let [colour_scale, packing_bias] = colour;
        let bits = packing_bias.to_bits();
        let exponent = ((bits >> 23) & 0xff) as i32 - 127;
        if source.trim().is_empty()
            || phase.into_iter().chain(colour).any(|v| !v.is_finite())
            || phase_scale < 0.
            || phase_bias < 0.
            || phase_scale + phase_bias > 1.
            || colour_scale <= 0.
            || bits & 0x807f_ffff != 0
            || !(8..=23).contains(&exponent)
            || packing_shift != (23 - exponent) as u32
        {
            return Err(Error {
                source_name: format!("{}; appearance {source}", self.source),
                emitter: None,
                failure: Failure::Malformed("native appearance parameters"),
            });
        }
        if self.profiles.iter().any(|profile| {
            !profile.properties.rows.is_power_of_two()
                || !profile.properties.columns.is_power_of_two()
        }) {
            return Err(Error {
                source_name: format!("{}; appearance {source}", self.source),
                emitter: None,
                failure: Failure::Unsupported("native atlas dimensions must be powers of two"),
            });
        }
        self.native_appearance = Some(NativeAppearance {
            source: source.into(),
            phase_scale,
            phase_bias,
            colour_scale,
            packing_bias,
            packing_shift,
        });
        Ok(self)
    }
    pub fn appearance_source(&self) -> Option<&str> {
        self.native_appearance
            .as_ref()
            .map(|style| style.source.as_str())
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }
    pub fn interval(&self) -> [u32; 2] {
        self.interval
    }
    pub fn looping(&self) -> bool {
        self.looping
    }
    pub fn checked(
        emitters: &[Particle2],
        textures: &[Texture],
        pivots: &Pivots,
        globals: &[u32],
        clip: Clip,
        source: &str,
        limits: Limits,
    ) -> Result<Self, Error> {
        let Clip { interval, looping } = clip;
        let fail = |emitter, failure| Error {
            source_name: source.into(),
            emitter,
            failure,
        };
        if interval[1] < interval[0] {
            return Err(fail(None, Failure::Malformed("clip interval")));
        }
        if emitters.len() > limits.max_emitters {
            return Err(fail(
                None,
                Failure::Budget {
                    kind: "emitters",
                    needed: emitters.len() as u128,
                    limit: limits.max_emitters as u128,
                },
            ));
        }
        let key_count = emitters
            .iter()
            .flat_map(|e| e.node_tracks.iter().chain(&e.tracks))
            .try_fold(0usize, |sum, t| sum.checked_add(t.count as usize))
            .ok_or_else(|| fail(None, Failure::Arithmetic))?;
        if key_count > limits.max_keys {
            return Err(fail(
                None,
                Failure::Budget {
                    kind: "keys",
                    needed: key_count as u128,
                    limit: limits.max_keys as u128,
                },
            ));
        }
        let mut profiles: Vec<Profile> = Vec::new();
        profiles
            .try_reserve_exact(emitters.len())
            .map_err(|_| fail(None, Failure::Allocation))?;
        for emitter in emitters {
            let build = || -> Result<Profile, Failure> {
                if profiles.iter().any(|p| p.object_id == emitter.object_id) {
                    return Err(Failure::Malformed("duplicate emitter ID"));
                }
                if emitter.parent_id != -1 {
                    return Err(Failure::Unsupported("parented emitter"));
                }
                if emitter.flags & 0xff != 0 {
                    return Err(Failure::Unsupported("generic node transform flags"));
                }
                let known = PARTICLE_EMITTER_TYPE
                    | UNSHADED
                    | SORT_FAR_Z
                    | LINE
                    | UNFOGGED
                    | MODEL_SPACE
                    | XY_QUAD
                    | 0xff;
                if emitter.flags & !known != 0 {
                    return Err(Failure::Unsupported("unknown PRE2 flags"));
                }
                if emitter.flags & MODEL_SPACE != 0 {
                    return Err(Failure::Unsupported(
                        "model-space particles outside TFT profile",
                    ));
                }
                let props = emitter.properties();
                if props.squirt != 0 {
                    return Err(Failure::Unsupported("squirt emission outside TFT profile"));
                }
                if props.replaceable_id != 0 {
                    return Err(Failure::Unsupported("replaceable particle texture"));
                }
                if props.filter_mode > 1 || props.head_or_tail > 2 {
                    return Err(Failure::Unsupported("particle filter/head-tail mode"));
                }
                let scalars = [
                    props.speed,
                    props.variation,
                    props.latitude,
                    props.gravity,
                    props.life_span,
                    props.emission_rate,
                    props.width,
                    props.length,
                    props.tail_length,
                    props.time,
                ];
                if !scalars.into_iter().all(f32::is_finite)
                    || props.life_span <= 0.
                    || [
                        props.variation,
                        props.latitude,
                        props.emission_rate,
                        props.width,
                        props.length,
                        props.tail_length,
                    ]
                    .into_iter()
                    .any(|v| v < 0.)
                    || !(0. ..=1.).contains(&props.time)
                {
                    return Err(Failure::Malformed("particle scalars"));
                }
                if props
                    .segment_colours
                    .iter()
                    .flatten()
                    .chain(props.particle_scaling.iter())
                    .any(|v| !v.is_finite() || *v < 0.)
                {
                    return Err(Failure::Malformed("particle segments"));
                }
                props
                    .rows
                    .checked_mul(props.columns)
                    .filter(|n| *n > 0)
                    .ok_or(Failure::Malformed("atlas dimensions"))?;
                let texture = textures
                    .get(props.texture_id as usize)
                    .ok_or(Failure::Malformed("TEXS reference"))?;
                if texture.slot != 0 || texture.path.is_empty() {
                    return Err(Failure::Unsupported(
                        "nonzero TEXS leading word/empty source",
                    ));
                }
                if texture.flags & !(pcx_mdl::TEXTURE_WRAP_WIDTH | pcx_mdl::TEXTURE_WRAP_HEIGHT)
                    != 0
                {
                    return Err(Failure::Unsupported("TEXS sampler flags"));
                }
                let pivot = pivots
                    .position(emitter.object_id)
                    .ok_or(Failure::Malformed("emitter pivot"))?;
                if !pivot.into_iter().all(f32::is_finite) {
                    return Err(Failure::Malformed("nonfinite emitter pivot"));
                }
                let mut translation = None;
                let mut rotation = None;
                let mut visibility = None;
                for track in &emitter.node_tracks {
                    let (slot, width, is_rotation) = match &track.magic {
                        b"KGTR" => (&mut translation, 3, false),
                        b"KGRT" => (&mut rotation, 4, true),
                        _ => return Err(Failure::Unsupported("node track outside KGTR/KGRT")),
                    };
                    if slot.is_some() {
                        return Err(Failure::Malformed("duplicate node channel"));
                    }
                    *slot = Some(Channel::checked(
                        track,
                        width,
                        is_rotation,
                        interval,
                        globals,
                    )?);
                }
                for track in &emitter.tracks {
                    if track.magic != *b"KP2V" || track.interpolation != Interpolation::None {
                        return Err(Failure::Unsupported(
                            "animated scalar outside TFT STEP visibility",
                        ));
                    }
                    if visibility.is_some() {
                        return Err(Failure::Malformed("duplicate visibility channel"));
                    }
                    visibility = Some(Channel::checked(track, 1, false, interval, globals)?);
                }
                Ok(Profile {
                    object_id: emitter.object_id,
                    properties: props,
                    flags: emitter.flags,
                    texture_flags: texture.flags,
                    pivot,
                    translation,
                    rotation,
                    visibility,
                })
            };
            profiles.push(build().map_err(|failure| fail(Some(emitter.object_id), failure))?);
        }
        Ok(Self {
            source: source.into(),
            profiles,
            interval,
            looping,
            native_appearance: None,
        })
    }
    fn error(&self, index: usize, failure: Failure) -> Error {
        Error {
            source_name: self.source.clone(),
            emitter: self.profiles.get(index).map(|p| p.object_id),
            failure,
        }
    }
    pub fn frame(&self, elapsed_ms: u64) -> u32 {
        let duration = u64::from(self.interval[1] - self.interval[0]);
        self.interval[0]
            + if duration == 0 {
                0
            } else if self.looping {
                (elapsed_ms % duration) as u32
            } else {
                elapsed_ms.min(duration) as u32
            }
    }
    pub fn sample_emitter(&self, index: usize, elapsed_ms: u64) -> Result<EmitterSample, Error> {
        let result = || -> Result<EmitterSample, Failure> {
            let profile = self
                .profiles
                .get(index)
                .ok_or(Failure::Malformed("emitter ordinal"))?;
            let frame = self.frame(elapsed_ms);
            let translation = profile
                .translation
                .as_ref()
                .map_or(Ok([0.; 4]), |t| t.sample(frame, elapsed_ms, [0.; 4]))?;
            let rotation = profile
                .rotation
                .as_ref()
                .map_or(Ok([0., 0., 0., 1.]), |t| {
                    t.sample(frame, elapsed_ms, [0., 0., 0., 1.])
                })?;
            let visible = profile
                .visibility
                .as_ref()
                .map_or(Ok([1., 0., 0., 0.]), |t| {
                    t.sample(frame, elapsed_ms, [1., 0., 0., 0.])
                })?[0]
                > 0.;
            Ok(EmitterSample {
                translation: vector([translation[0], translation[1], translation[2]])?,
                rotation: rotation.map(|v| v as f32),
                visible,
            })
        };
        result().map_err(|failure| self.error(index, failure))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmitterSample {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub visible: bool,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    pub emitter: usize,
    pub age: f64,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub gravity: f32,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Work {
    pub ticks: u64,
    pub births: u64,
    pub updates: u64,
    pub expired: u64,
}
#[derive(Debug, Clone, Copy)]
struct Rng(u64);
impl Rng {
    // Injected xorshift stream is explicit project policy, not the game's RNG.
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn symmetric(&mut self, extent: f64) -> f64 {
        (self.unit() * 2. - 1.) * extent
    }
}

pub struct Simulation {
    plan: Arc<Plan>,
    elapsed: Duration,
    pending: Duration,
    particles: Vec<Particle>,
    scratch: Vec<Particle>,
    remainders: Vec<f64>,
    next_remainders: Vec<f64>,
    rng: Rng,
    pub work: Work,
    capacity: usize,
    tick: Duration,
}
impl Simulation {
    pub fn new(plan: Arc<Plan>, seed: u64, limits: Limits) -> Result<Self, Error> {
        let error = |failure| Error {
            source_name: plan.source.clone(),
            emitter: None,
            failure,
        };
        if seed == 0 || limits.tick.is_zero() {
            return Err(error(Failure::Malformed("RNG seed/tick policy")));
        }
        limits
            .particle_capacity
            .checked_mul(std::mem::size_of::<Particle>())
            .and_then(|v| v.checked_mul(2))
            .ok_or_else(|| error(Failure::Allocation))?;
        let mut particles = Vec::new();
        let mut scratch = Vec::new();
        particles
            .try_reserve_exact(limits.particle_capacity)
            .map_err(|_| error(Failure::Allocation))?;
        scratch
            .try_reserve_exact(limits.particle_capacity)
            .map_err(|_| error(Failure::Allocation))?;
        let mut remainders = Vec::new();
        let mut next_remainders = Vec::new();
        remainders
            .try_reserve_exact(plan.profiles.len())
            .map_err(|_| error(Failure::Allocation))?;
        next_remainders
            .try_reserve_exact(plan.profiles.len())
            .map_err(|_| error(Failure::Allocation))?;
        remainders.resize(plan.profiles.len(), 0.);
        next_remainders.resize(plan.profiles.len(), 0.);
        Ok(Self {
            plan,
            elapsed: Duration::ZERO,
            pending: Duration::ZERO,
            particles,
            scratch,
            remainders,
            next_remainders,
            rng: Rng(seed),
            work: Work::default(),
            capacity: limits.particle_capacity,
            tick: limits.tick,
        })
    }
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
    pub fn pending(&self) -> Duration {
        self.pending
    }
    pub fn advance(&mut self, delta: Duration, limits: Limits) -> Result<Work, Error> {
        let plan = &self.plan;
        let error = |failure| Error {
            source_name: plan.source.clone(),
            emitter: None,
            failure,
        };
        if self.tick != limits.tick || self.capacity != limits.particle_capacity {
            return Err(error(Failure::Malformed("simulation/plan/policy identity")));
        }
        let pending = self
            .pending
            .checked_add(delta)
            .ok_or_else(|| error(Failure::Clock))?;
        let ticks = pending.as_nanos() / self.tick.as_nanos();
        if ticks > u128::from(limits.max_ticks) {
            return Err(error(Failure::Budget {
                kind: "ticks",
                needed: ticks,
                limit: u128::from(limits.max_ticks),
            }));
        }
        self.pending = pending;
        let mut work = Work::default();
        for _ in 0..ticks {
            let end = self
                .elapsed
                .checked_add(self.tick)
                .ok_or_else(|| error(Failure::Clock))?;
            let ms = u64::try_from(end.as_millis()).map_err(|_| error(Failure::Clock))?;
            let dt = self.tick.as_secs_f64();
            let mut rng = self.rng;
            let mut births = 0u64;
            let mut expired = 0u64;
            let mut updates =
                u64::try_from(self.particles.len()).map_err(|_| error(Failure::Arithmetic))?;
            let initial_work = work
                .updates
                .checked_add(updates)
                .ok_or_else(|| error(Failure::Arithmetic))?;
            if initial_work > limits.max_updates {
                return Err(error(Failure::Budget {
                    kind: "updates",
                    needed: u128::from(initial_work),
                    limit: u128::from(limits.max_updates),
                }));
            }
            self.scratch.clear();
            self.next_remainders.copy_from_slice(&self.remainders);
            for particle in &self.particles {
                if let Some(next) = advance_particle(
                    *particle,
                    dt,
                    plan.profiles[particle.emitter].properties.life_span,
                )
                .map_err(|failure| plan.error(particle.emitter, failure))?
                {
                    self.scratch.push(next);
                } else {
                    expired += 1;
                }
            }
            for (index, profile) in plan.profiles.iter().enumerate() {
                let sample = plan.sample_emitter(index, ms)?;
                if !sample.visible {
                    continue;
                }
                let emission =
                    self.remainders[index] + f64::from(profile.properties.emission_rate) * dt;
                if !emission.is_finite() || emission < 0. {
                    return Err(plan.error(index, Failure::Arithmetic));
                }
                if emission.floor() > (self.capacity - self.scratch.len()) as f64 {
                    return Err(plan.error(
                        index,
                        Failure::Budget {
                            kind: "particles",
                            needed: (emission.floor() as u128)
                                .saturating_add(self.scratch.len() as u128),
                            limit: self.capacity as u128,
                        },
                    ));
                }
                let count = emission.floor() as usize;
                if count > self.capacity - self.scratch.len() {
                    return Err(plan.error(
                        index,
                        Failure::Budget {
                            kind: "particles",
                            needed: (self.scratch.len() as u128) + (count as u128),
                            limit: self.capacity as u128,
                        },
                    ));
                }
                let count_u64 =
                    u64::try_from(count).map_err(|_| plan.error(index, Failure::Arithmetic))?;
                updates = updates
                    .checked_add(count_u64)
                    .ok_or_else(|| plan.error(index, Failure::Arithmetic))?;
                let total = work
                    .updates
                    .checked_add(updates)
                    .ok_or_else(|| plan.error(index, Failure::Arithmetic))?;
                if total > limits.max_updates {
                    return Err(plan.error(
                        index,
                        Failure::Budget {
                            kind: "updates",
                            needed: u128::from(total),
                            limit: u128::from(limits.max_updates),
                        },
                    ));
                }
                self.next_remainders[index] = emission - count as f64;
                for _ in 0..count {
                    let particle = spawn_particle(index, profile, sample, &mut rng)
                        .map_err(|failure| plan.error(index, failure))?;
                    if let Some(next) = advance_particle(particle, dt, profile.properties.life_span)
                        .map_err(|failure| plan.error(index, failure))?
                    {
                        self.scratch.push(next);
                    } else {
                        expired += 1;
                    }
                }
                births = births
                    .checked_add(count_u64)
                    .ok_or_else(|| plan.error(index, Failure::Arithmetic))?;
            }
            if work
                .updates
                .checked_add(updates)
                .ok_or_else(|| error(Failure::Arithmetic))?
                > limits.max_updates
            {
                return Err(error(Failure::Budget {
                    kind: "updates",
                    needed: u128::from(work.updates) + u128::from(updates),
                    limit: u128::from(limits.max_updates),
                }));
            }
            let next = Work {
                ticks: 1,
                births,
                updates,
                expired,
            };
            let add = |a: u64, b: u64| a.checked_add(b).ok_or_else(|| error(Failure::Arithmetic));
            let cumulative = Work {
                ticks: add(self.work.ticks, 1)?,
                births: add(self.work.births, births)?,
                updates: add(self.work.updates, updates)?,
                expired: add(self.work.expired, expired)?,
            };
            work = Work {
                ticks: add(work.ticks, next.ticks)?,
                births: add(work.births, next.births)?,
                updates: add(work.updates, next.updates)?,
                expired: add(work.expired, next.expired)?,
            };
            std::mem::swap(&mut self.particles, &mut self.scratch);
            std::mem::swap(&mut self.remainders, &mut self.next_remainders);
            self.rng = rng;
            self.elapsed = end;
            self.pending -= self.tick;
            self.work = cumulative;
        }
        Ok(work)
    }
}
fn spawn_particle(
    index: usize,
    profile: &Profile,
    sample: EmitterSample,
    rng: &mut Rng,
) -> Result<Particle, Failure> {
    let props = profile.properties;
    let q = sample.rotation.map(f64::from);
    let offset = rotate(
        q,
        [
            rng.symmetric(f64::from(props.width) * 0.5),
            rng.symmetric(f64::from(props.length) * 0.5),
            0.,
        ],
    );
    let position = vector(std::array::from_fn(|i| {
        f64::from(profile.pivot[i]) + f64::from(sample.translation[i]) + offset[i]
    }))?;
    let lat = f64::from(props.latitude).to_radians();
    let inclination = rng.symmetric(lat);
    let azimuth = if profile.flags & LINE == 0 {
        rng.symmetric(std::f64::consts::TAU)
    } else {
        0.
    };
    let (sin_lat, cos_lat) = inclination.sin_cos();
    let (sin_az, cos_az) = azimuth.sin_cos();
    // The source plane basis turns the polar cone a quarter turn around local Z.
    let direction = rotate(q, [-sin_lat * sin_az, sin_lat * cos_az, cos_lat]);
    let speed = f64::from(props.speed) * (1. + rng.symmetric(f64::from(props.variation)));
    Ok(Particle {
        emitter: index,
        age: 0.,
        position,
        velocity: vector(direction.map(|v| v * speed))?,
        gravity: props.gravity,
    })
}
fn advance_particle(
    mut particle: Particle,
    dt: f64,
    life: f32,
) -> Result<Option<Particle>, Failure> {
    particle.age += dt;
    if particle.age >= f64::from(life) {
        return Ok(None);
    }
    let mut position = std::array::from_fn(|i| {
        f64::from(particle.position[i]) + f64::from(particle.velocity[i]) * dt
    });
    position[2] -= 0.5 * f64::from(particle.gravity) * dt * dt;
    particle.position = vector(position)?;
    particle.velocity = vector([
        f64::from(particle.velocity[0]),
        f64::from(particle.velocity[1]),
        f64::from(particle.velocity[2]) - f64::from(particle.gravity) * dt,
    ])?;
    Ok(Some(particle))
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Rendering obligations are retained, not silently interpreted as GPU support.
/// Known node-type/visual flags remain source-authored. Shader effects are not certified here.
pub struct Appearance {
    pub rgba: [f32; 4],
    pub scale: f32,
    pub head_cell: Option<u32>,
    pub tail_cell: Option<u32>,
    pub tail_start: [f32; 3],
    pub texture_id: u32,
    pub texture_flags: u32,
    pub flags: u32,
    pub priority_plane: i32,
    pub filter_mode: u32,
    pub rows: u32,
    pub columns: u32,
    pub xy_facing: Option<f32>,
}
impl Plan {
    pub fn appearance(&self, particle: &Particle) -> Result<Appearance, Error> {
        let result = || -> Result<Appearance, Failure> {
            let profile = self
                .profiles
                .get(particle.emitter)
                .ok_or(Failure::Malformed("particle emitter"))?;
            let p = profile.properties;
            if !particle
                .position
                .iter()
                .chain(&particle.velocity)
                .all(|v| v.is_finite())
            {
                return Err(Failure::Arithmetic);
            }
            if !particle.age.is_finite()
                || particle.age < 0.
                || particle.age >= f64::from(p.life_span)
            {
                return Err(Failure::Malformed("live particle age"));
            }
            let (segment, t, native_phase) = if let Some(style) = self.native_appearance.as_ref() {
                let age = particle.age as f32;
                let split = p.time * p.life_span;
                let (segment, start, end) = if p.time == 1. || age < split {
                    (0, 0., split)
                } else {
                    (1, split, p.life_span)
                };
                let phase = style.phase((age - start) * (end - start).recip());
                if !phase.is_finite() {
                    return Err(Failure::Arithmetic);
                }
                (segment, f64::from(phase), Some(phase))
            } else {
                let age = particle.age / f64::from(p.life_span);
                let (segment, t) = if age < f64::from(p.time) {
                    (0, age / f64::from(p.time))
                } else {
                    (1, (age - f64::from(p.time)) / (1. - f64::from(p.time)))
                };
                (segment, t, None)
            };
            let lerp = |a: f32, b: f32| f64::from(a) + (f64::from(b) - f64::from(a)) * t;
            let rgba = if let Some(style) = self.native_appearance.as_ref() {
                style.rgba(&p, segment, t as f32).map(f64::from)
            } else {
                [
                    lerp(
                        p.segment_colours[segment][0],
                        p.segment_colours[segment + 1][0],
                    ),
                    lerp(
                        p.segment_colours[segment][1],
                        p.segment_colours[segment + 1][1],
                    ),
                    lerp(
                        p.segment_colours[segment][2],
                        p.segment_colours[segment + 1][2],
                    ),
                    lerp(
                        f32::from(p.segment_alpha[segment]) / 255.,
                        f32::from(p.segment_alpha[segment + 1]) / 255.,
                    ),
                ]
            };
            let head_interval = if segment == 0 {
                p.life_span_uv_anim
            } else {
                p.decay_uv_anim
            };
            let mut tail_interval = if segment == 0 {
                p.tail_uv_anim
            } else {
                p.tail_decay_uv_anim
            };
            if self.native_appearance.is_some() {
                // The reviewed phase object shares its head repeat count with the tail interval.
                tail_interval[2] = head_interval[2];
            }
            let head_cell = (p.head_or_tail != 1).then(|| {
                atlas_cell(
                    head_interval,
                    t,
                    p.rows * p.columns,
                    self.native_appearance.as_ref(),
                )
            });
            let tail_cell = (p.head_or_tail != 0).then(|| {
                atlas_cell(
                    tail_interval,
                    t,
                    p.rows * p.columns,
                    self.native_appearance.as_ref(),
                )
            });
            let tail_start = if tail_cell.is_some() {
                vector(std::array::from_fn(|i| {
                    f64::from(particle.position[i])
                        - f64::from(particle.velocity[i]) * f64::from(p.tail_length)
                }))?
            } else {
                particle.position
            };
            let scale = if let Some(phase) = native_phase {
                f64::from(
                    (p.particle_scaling[segment + 1] - p.particle_scaling[segment]) * phase
                        + p.particle_scaling[segment],
                )
            } else {
                lerp(p.particle_scaling[segment], p.particle_scaling[segment + 1])
            };
            if !rgba
                .iter()
                .chain([&scale])
                .all(|v| v.is_finite() && v.abs() <= f64::from(f32::MAX))
            {
                return Err(Failure::Arithmetic);
            }
            Ok(Appearance {
                rgba: rgba.map(|v| v as f32),
                scale: scale as f32,
                head_cell,
                tail_cell,
                tail_start,
                texture_id: p.texture_id,
                texture_flags: profile.texture_flags,
                flags: profile.flags,
                priority_plane: p.priority_plane,
                filter_mode: p.filter_mode,
                rows: p.rows,
                columns: p.columns,
                // Pinned Particle2.bind XY facing; gravity cannot change XY velocity.
                xy_facing: (profile.flags & XY_QUAD != 0).then(|| {
                    particle.velocity[1].atan2(particle.velocity[0]) - std::f32::consts::PI
                        + std::f32::consts::PI / 8.
                }),
            })
        };
        result().map_err(|failure| self.error(particle.emitter, failure))
    }
}
fn atlas_cell(
    interval: [u32; 3],
    factor: f64,
    cells: u32,
    native: Option<&NativeAppearance>,
) -> u32 {
    let [start, end, repeat] = interval;
    if let Some(style) = native {
        if start > cells || end > cells {
            return 0;
        }
        let phase = if repeat == 1 {
            factor as f32
        } else {
            (factor as f32 * repeat as f32) % 1.
        };
        let (start, end) = (start as i32, end as i32);
        let (first, delta) = if end >= start {
            (start, end.wrapping_sub(start).wrapping_add(1))
        } else {
            (
                start.wrapping_add(1),
                end.wrapping_sub(start).wrapping_sub(1),
            )
        };
        return u32::from(style.pack(delta as f32 * phase + first as f32));
    }
    let span = u64::from(start.abs_diff(end)) + 1;
    let frame = (span as f64 * f64::from(repeat) * factor).floor() % span as f64;
    let cell = if end >= start {
        f64::from(start) + frame
    } else {
        f64::from(start) - frame
    };
    cell.min(f64::from(cells - 1)) as u32
}

impl Appearance {
    /// Source atlas indices, in the retained reference shader's corner order.
    /// Camera-facing world geometry remains the renderer's responsibility.
    pub fn cell_corners(&self, tail: bool) -> Option<[[f32; 2]; 4]> {
        let cell = if tail { self.tail_cell } else { self.head_cell }?;
        let column = cell % self.columns;
        let row = cell / self.columns;
        let left = column as f32 / self.columns as f32;
        let right = (column + 1) as f32 / self.columns as f32;
        let top = row as f32 / self.rows as f32;
        let bottom = (u64::from(row) + 1) as f32 / self.rows as f32;
        Some([[right, top], [left, top], [left, bottom], [right, bottom]])
    }
}
