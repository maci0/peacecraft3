//! Source model and logical animation state shared by unit scripts and rendering.
use crate::scripts::AssetIdentity;
use pcx_objects::editor::{FieldKind, FieldSource, SlkId};
use pcx_sim::world::{Definition, FieldValue};
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub origin: AssetIdentity,
    pub member: String,
    pub bytes: Arc<[u8]>,
    pub sequences: Vec<pcx_mdl::Sequence>,
    pub blend_time_ms: u32,
    pub joints: Vec<pcx_mdl::Bone>,
    pub global_sequences: Vec<u32>,
}

impl Model {
    pub fn read(
        origin: AssetIdentity,
        member: String,
        bytes: Arc<[u8]>,
        maximum_bytes: usize,
    ) -> Result<Self, String> {
        if bytes.len() > maximum_bytes
            || member.trim().is_empty()
            || [&origin.source, &origin.edition, &origin.version]
                .into_iter()
                .any(|s| s.trim().is_empty())
        {
            return Err(format!(
                "{member}: unit model source identity or byte budget"
            ));
        }
        let model = pcx_mdl::Mdlx::parse(&bytes).map_err(|e| format!("{member}: {e}"))?;
        let blend_time_ms = model
            .model(&bytes)
            .map_err(|e| format!("{member}: MODL: {e}"))?
            .blend_time_ms;
        let mut joints = model
            .bones(&bytes)
            .map_err(|e| format!("{member}: BONE: {e}"))?;
        for helper in model
            .nodes(&bytes, pcx_mdl::tag::HELP)
            .map_err(|e| format!("{member}: HELP: {e}"))?
        {
            joints.push(
                helper
                    .transform_bone()
                    .map_err(|e| format!("{member}: HELP {}: {e}", helper.object_id))?,
            );
        }
        let globals = model.payload(&bytes, pcx_mdl::tag::GLBS).unwrap_or(&[]);
        if !globals.len().is_multiple_of(4) {
            return Err(format!("{member}: malformed GLBS"));
        }
        let global_sequences = globals
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| u32::from_le_bytes(*v))
            .collect();
        let sequences = model
            .sequences(&bytes)
            .map_err(|e| format!("{member}: SEQS: {e}"))?;
        if sequences
            .iter()
            .any(|s| s.interval_end < s.interval_start || s.name.chars().any(char::is_control))
        {
            return Err(format!("{member}: invalid animation interval/name"));
        }
        Ok(Self {
            origin,
            member,
            bytes,
            sequences,
            blend_time_ms,
            joints,
            global_sequences,
        })
    }

    fn poses(&self, sample: Sample) -> Result<Vec<pcx_mdl::Pose>, String> {
        let elapsed =
            u32::try_from(sample.elapsed_ms).map_err(|_| "unit global pose clock overflow")?;
        let mut poses = Vec::new();
        poses
            .try_reserve_exact(self.joints.len())
            .map_err(|_| "unit transition pose allocation")?;
        poses.extend(self.joints.iter().map(|bone| {
            pcx_mdl::sample_joint_pose(
                bone,
                sample.frame,
                elapsed,
                &self.global_sequences,
                sample.interval,
            )
        }));
        Ok(poses)
    }

    fn select(&self, name: &str) -> Option<usize> {
        let wanted: Vec<_> = name.split_ascii_whitespace().collect();
        if wanted.is_empty() {
            return None;
        }
        self.sequences
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name.trim()))
            .or_else(|| {
                self.sequences.iter().position(|s| {
                    let tokens: Vec<_> = s
                        .name
                        .split(|c: char| c.is_ascii_whitespace() || c == '-')
                        .filter(|s| !s.is_empty())
                        .collect();
                    tokens.len() >= wanted.len()
                        && wanted
                            .iter()
                            .zip(&tokens)
                            .all(|(a, b)| a.eq_ignore_ascii_case(b))
                })
            })
    }
}

/// Resolve the model through the selected metadata, including scalar map edits.
pub fn model_path(definition: &Definition) -> Result<Option<String>, String> {
    let mut fields = definition.fields.iter().filter(|(_, f)| {
        f.metadata.as_ref().is_some_and(|m|
        matches!(&m.source, FieldSource::Column {slk: SlkId::UnitUi,column} if column == "file"))
    });
    let (id, field) = fields.next().ok_or("missing UnitUI/file metadata")?;
    if fields.next().is_some() {
        return Err("ambiguous UnitUI/file metadata".into());
    }
    let m = field.metadata.as_ref().expect("selected metadata");
    if m.fourcc != *id
        || m.from != SlkId::UnitMetaData
        || m.index != -1
        || m.kind != FieldKind::Token
    {
        return Err("unsupported UnitUI/file metadata".into());
    }
    let path = match &field.value {
        Ok(FieldValue::Cell(text)) => text.as_str(),
        Ok(FieldValue::Override(pcx_objects::Value::Str(text))) => {
            std::str::from_utf8(text).map_err(|_| "model path is not UTF8")?
        }
        _ => return Err("unsupported UnitUI/file value".into()),
    }
    .trim();
    if matches!(path, "" | "_" | "-") {
        return Ok(None);
    }
    if path.len() > 16 << 10 || path.chars().any(char::is_control) || path.contains(',') {
        return Err("unsupported model path or model variation list".into());
    }
    let path = pcx_mdl::binary_model_path(path);
    let name = path.rsplit(['\\', '/']).next().expect("nonempty path");
    if !name.contains('.') {
        Ok(Some(format!("{path}.mdx")))
    } else if name.to_ascii_lowercase().ends_with(".mdx") {
        Ok(Some(path))
    } else {
        Err("unsupported unit model extension".into())
    }
}

/// Recreation bound: the original queue capacity is not established.
const QUEUED_LIMIT: usize = 8;
/// Floating-point guard for authored millisecond boundaries (one nanosecond).
const SCHEDULE_EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq)]
struct QueuedClip {
    sequence: usize,
    requested: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Animation {
    pub model: Arc<Model>,
    pub sequence: Option<usize>,
    engine_properties: Option<(Arc<crate::unit_animation_rarity::Rules>, Vec<String>)>,
    pub created: f64,
    pub started: f64,
    /// Sequence seconds per game second, fixed when the sequence is selected.
    rate: f64,
    phase_at: f64,
    phase_seconds: f64,
    time_scale: f64,
    blend_time: f64,
    blend: Option<Blend>,
    /// Authored clips waiting behind the current sequence, in call order, with
    /// the game time at which each was requested.
    queued: VecDeque<QueuedClip>,
    /// The current sequence came from the queue and still owes a Stand fallback.
    queue_active: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct Blend {
    from: Arc<[pcx_mdl::Pose]>,
    started: f64,
    duration: f64,
    elapsed: f64,
}

#[derive(Clone, Copy)]
pub struct BlendSample<'a> {
    pub from: &'a [pcx_mdl::Pose],
    pub weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub interval: [u32; 2],
    pub frame: u32,
    pub elapsed_ms: u64,
}

impl Animation {
    pub fn new(model: Arc<Model>, now: f64) -> Result<Self, String> {
        if !now.is_finite() || now < 0. {
            return Err("invalid unit model game time".into());
        }
        // shortcut: animation variants use source order; qualify original rarity/RNG selection before claiming visual lockstep.
        let sequence = model
            .select("stand")
            .or_else(|| (!model.sequences.is_empty()).then_some(0));
        let blend_time = f64::from(model.blend_time_ms) / 1000.;
        Ok(Self {
            model,
            sequence,
            engine_properties: None,
            created: now,
            started: now,
            rate: 1.,
            phase_at: now,
            phase_seconds: 0.,
            time_scale: 1.,
            blend_time,
            blend: None,
            queued: VecDeque::new(),
            queue_active: false,
        })
    }
    /// Whether the model declares a sequence matching `name`. `select` ignores misses.
    pub fn has_sequence(&self, name: &str) -> bool {
        self.model.select(name).is_some()
    }
    /// Select `name` at `now`. `unit_speed` is the unit's movement speed for a
    /// moving sequence: the sequence then plays at `unit_speed / move_speed` of its
    /// authored clock, where `move_speed` is the sequence's SEQS ground speed.
    /// `None`, or a zero `move_speed`, keeps the authored clock. A missing
    /// selection leaves the state unchanged.
    pub fn select(&mut self, name: &str, now: f64, unit_speed: Option<f32>) -> Result<(), String> {
        self.select_candidate(self.model.select(name), name, now, unit_speed)
    }
    /// Engine activity respects the current form's source-authored properties.
    /// Explicit script animation selections retain their separate selection policy.
    pub(crate) fn select_engine(
        &mut self,
        name: &str,
        now: f64,
        unit_speed: Option<f32>,
    ) -> Result<(), String> {
        let candidate = match &self.engine_properties {
            None => self.model.select(name),
            Some((rules, props)) => {
                // shortcut: engine variants retain source-first selection; replace after ordinary animation RNG history is qualified.
                rules
                    .engine_candidates(&self.model, name, props)?
                    .into_iter()
                    .next()
            }
        };
        self.select_candidate(candidate, name, now, unit_speed)
    }
    /// Bind source properties for automatic activity without changing explicit script selection.
    pub(crate) fn copy_engine_properties_from(&mut self, source: &Self) {
        self.engine_properties.clone_from(&source.engine_properties);
    }
    pub(crate) fn set_engine_properties(
        &mut self,
        rules: Arc<crate::unit_animation_rarity::Rules>,
        props: Vec<String>,
    ) {
        self.engine_properties = Some((rules, props));
    }
    fn select_candidate(
        &mut self,
        candidate: Option<usize>,
        name: &str,
        now: f64,
        unit_speed: Option<f32>,
    ) -> Result<(), String> {
        if !now.is_finite() || now < self.started || now < self.phase_at {
            return Err("invalid unit animation game time".into());
        }
        if let Some(index) = candidate {
            let rate = match unit_speed {
                None => 1.,
                Some(speed) if !speed.is_finite() || speed <= 0. => {
                    return Err(format!("{name}: playback rate needs a positive unit speed"));
                }
                Some(speed) => {
                    let move_speed = f64::from(self.model.sequences[index].move_speed);
                    if !move_speed.is_finite() || move_speed < 0. {
                        return Err(format!("{name}: invalid SEQS move speed"));
                    }
                    // ponytail: zero ground speed has nothing to scale against; shipped Walk
                    // sequences do carry it, so the authored clock stays in effect (unverified).
                    if move_speed == 0. {
                        1.
                    } else {
                        f64::from(speed) / move_speed
                    }
                }
            };
            let blend = self.prepare_blend(now)?;
            self.sequence = Some(index);
            self.started = now;
            self.rate = rate;
            self.phase_at = now;
            self.phase_seconds = 0.;
            self.blend = blend;
            self.queued.clear();
            self.queue_active = false;
        }
        Ok(())
    }
    pub(crate) fn select_index(&mut self, index: usize, now: f64) -> Result<(), String> {
        if index >= self.model.sequences.len()
            || !now.is_finite()
            || now < self.started
            || now < self.phase_at
        {
            return Err("invalid unit animation index/time".into());
        }
        let blend = self.prepare_blend(now)?;
        self.sequence = Some(index);
        self.started = now;
        self.rate = 1.;
        self.phase_at = now;
        self.phase_seconds = 0.;
        self.blend = blend;
        self.queued.clear();
        self.queue_active = false;
        Ok(())
    }
    /// Engine phase duration is gameplay data; fit its authored clip without changing explicit time scale.
    pub(crate) fn select_for_duration(
        &mut self,
        name: &str,
        now: f64,
        duration: f64,
    ) -> Result<(), String> {
        if !duration.is_finite() || duration < 0. {
            return Err("invalid unit phase duration".into());
        }
        let Some(index) = self.model.select(name) else {
            return Ok(());
        };
        let span = f64::from(self.model.sequences[index].duration()) / 1000.;
        self.select(name, now, None)?;
        if duration > 0. && span > 0. {
            self.rate = span / duration;
        }
        Ok(())
    }
    /// Append `name` to the animation FIFO. Missing names are ignored; storage and
    /// capacity failures leave the animation unchanged. Queued clips play one
    /// authored interval each, including looping sequences, then fall back to
    /// Stand. That one-shot queue policy is a recreation choice pending native
    /// runtime comparison.
    pub fn queue(&mut self, name: &str, now: f64) -> Result<bool, String> {
        if !now.is_finite() || now < self.started || now < self.phase_at {
            return Err("invalid unit animation game time".into());
        }
        let Some(index) = self.model.select(name) else {
            return Ok(false);
        };
        if self.queued.len() == QUEUED_LIMIT {
            return Err("unit animation queue capacity".into());
        }
        self.queued
            .try_reserve(1)
            .map_err(|_| "unit animation queue storage".to_string())?;
        self.queued.push_back(QueuedClip {
            sequence: index,
            requested: now,
        });
        Ok(true)
    }
    pub fn has_pending(&self) -> bool {
        !self.queued.is_empty()
    }
    /// Whether a queued clip is playing or still waiting to play.
    pub fn needs_advance(&self) -> bool {
        self.queue_active || !self.queued.is_empty()
    }
    /// Advance queued intent to authored completion boundaries at or before `now`.
    /// A large step may consume several clips, each starting exactly at the prior
    /// completion time. Renderer sampling remains immutable.
    pub fn advance(&mut self, now: f64) -> Result<(), String> {
        if !now.is_finite() || now < self.started || now < self.created {
            return Err("invalid unit animation game time".into());
        }
        while self.queue_active || !self.queued.is_empty() {
            let Some(index) = self.sequence else {
                let Some(next) = self.queued.front().copied() else {
                    self.queue_active = false;
                    break;
                };
                self.start(next.sequence, self.started.max(next.requested), 1.)?;
                self.queued.pop_front();
                self.queue_active = true;
                continue;
            };
            let Some(at) = (match self.queued.front() {
                Some(next) => self.next_boundary(index, next.requested)?,
                None => self.next_boundary(index, self.started)?,
            }) else {
                break;
            };
            if now < at {
                break;
            }
            if let Some(next) = self.queued.front().copied() {
                self.start(next.sequence, at, 1.)?;
                self.queued.pop_front();
                self.queue_active = true;
            } else if let Some(stand) = self.model.select("stand") {
                self.start(stand, at, 1.)?;
                self.queue_active = false;
            } else {
                self.queue_active = false;
                break;
            }
        }
        Ok(())
    }
    /// The first completion boundary of the current clip at or after `requested`.
    /// A late request waits for the next authored boundary instead of consuming a
    /// clip at a boundary that already elapsed.
    fn next_boundary(&self, index: usize, requested: f64) -> Result<Option<f64>, String> {
        let sequence = self
            .model
            .sequences
            .get(index)
            .ok_or("unit animation sequence missing")?;
        let speed = self.rate * self.time_scale;
        if speed == 0. {
            return Ok(None);
        }
        let requested = requested.max(self.phase_at);
        let duration = f64::from(sequence.duration()) / 1000.;
        if duration == 0. {
            return Ok(Some(requested));
        }
        let phase = self.local_seconds(requested)?;
        let target = if sequence.non_looping {
            if speed > 0. { duration } else { 0. }
        } else if speed > 0. {
            (phase / duration).ceil().max(1.) * duration
        } else {
            (phase / duration).floor().min(-1.) * duration
        };
        let at = (self.phase_at + (target - self.phase_seconds) / speed).max(requested);
        if !at.is_finite() {
            return Err("unit animation clock overflow".into());
        }
        Ok(Some(at))
    }
    fn local_seconds(&self, now: f64) -> Result<f64, String> {
        if !now.is_finite() || now < self.phase_at {
            return Err("invalid unit animation game time".into());
        }
        let phase = self.phase_seconds + (now - self.phase_at) * self.rate * self.time_scale;
        if !phase.is_finite() {
            return Err("unit animation clock overflow".into());
        }
        Ok(phase)
    }
    /// Preserve the visible phase when changing speed; zero freezes local playback.
    pub fn set_time_scale(&mut self, scale: f32, now: f64) -> Result<(), String> {
        if !scale.is_finite() {
            return Err("finite unit time scale required".into());
        }
        self.local_seconds(now)?;
        let mut next = self.clone();
        next.advance(now)?;
        if let Some(blend) = &mut next.blend {
            blend.elapsed = (blend.elapsed + (now - blend.started) * next.time_scale).max(0.);
            blend.started = now;
            if blend.elapsed >= blend.duration {
                next.blend = None;
            }
        }
        next.phase_seconds = next.local_seconds(now)?;
        next.phase_at = now;
        next.time_scale = f64::from(scale);
        *self = next;
        Ok(())
    }
    pub(crate) fn interval_finished(&self, now: f64) -> Result<bool, String> {
        let Some(index) = self.sequence else {
            return Ok(true);
        };
        let duration = f64::from(self.model.sequences[index].duration()) / 1000.;
        let phase = self.local_seconds(now)?;
        Ok(if self.time_scale < 0. {
            phase <= 0.
        } else {
            phase >= duration
        })
    }
    /// Shift every animation timestamp that the pause covered forward by the
    /// paused interval. Requests made during the pause take effect at resume.
    pub(crate) fn resume_after_pause(&mut self, since: f64, now: f64) {
        let resume = |at: f64| {
            if at <= since {
                at + (now - since)
            } else if at < now {
                now
            } else {
                at
            }
        };
        if let Some(blend) = &mut self.blend {
            blend.started = resume(blend.started);
        }
        self.phase_at = resume(self.phase_at);
        self.created = resume(self.created);
        self.started = resume(self.started);
        for entry in &mut self.queued {
            entry.requested = resume(entry.requested);
        }
    }
    fn start(&mut self, index: usize, at: f64, rate: f64) -> Result<(), String> {
        let blend = self.prepare_blend(at)?;
        self.blend = blend;
        self.sequence = Some(index);
        self.started = at;
        self.rate = rate;
        self.phase_at = at;
        self.phase_seconds = 0.;
        Ok(())
    }
    pub fn set_blend_time(&mut self, seconds: f32) -> Result<(), String> {
        if !seconds.is_finite() || seconds < 0. {
            return Err("invalid unit blend time".into());
        }
        // shortcut: positive changes affect future transitions; qualify native retiming before changing an active blend.
        self.blend_time = f64::from(seconds);
        if seconds == 0. {
            self.blend = None;
        }
        Ok(())
    }
    pub fn blend_sample(&self, now: f64) -> Result<Option<BlendSample<'_>>, String> {
        if !now.is_finite() || now < self.started || now < self.phase_at {
            return Err("invalid blend clock".into());
        }
        Ok(self.blend.as_ref().and_then(|blend| {
            let elapsed = blend.elapsed + (now - blend.started) * self.time_scale;
            (elapsed < blend.duration).then_some(BlendSample {
                from: &blend.from,
                weight: (elapsed / blend.duration).clamp(0., 1.) as f32,
            })
        }))
    }
    fn prepare_blend(&self, now: f64) -> Result<Option<Blend>, String> {
        if self.blend_time == 0. || self.model.joints.is_empty() {
            return Ok(None);
        }
        let Some(sample) = self.sample(now)? else {
            return Ok(None);
        };
        let mut poses = self.model.poses(sample)?;
        if let Some(blend) = self.blend_sample(now)? {
            for ((pose, from), bone) in poses.iter_mut().zip(blend.from).zip(&self.model.joints) {
                *pose = pcx_mdl::blend_joint_pose(bone, *from, *pose, blend.weight)
                    .map_err(str::to_owned)?;
            }
        }
        Ok(Some(Blend {
            from: poses.into(),
            started: now,
            duration: self.blend_time,
            elapsed: 0.,
        }))
    }
    pub fn sample(&self, now: f64) -> Result<Option<Sample>, String> {
        if !now.is_finite() || now < self.started || now < self.created {
            return Err("invalid unit animation game time".into());
        }
        let Some(index) = self.sequence else {
            return Ok(None);
        };
        let sequence = self
            .model
            .sequences
            .get(index)
            .ok_or("unit animation sequence missing")?;
        let elapsed_ms = millis(now - self.created)?;
        let local = (self.local_seconds(now)? * 1000. + SCHEDULE_EPSILON * 1000.).floor();
        if local.abs() >= u64::MAX as f64 {
            return Err("unit animation clock overflow".into());
        }
        let duration = u64::from(sequence.duration());
        let offset = if duration == 0 {
            0
        } else if sequence.non_looping {
            local.clamp(0., duration as f64) as u64
        } else {
            local.rem_euclid(duration as f64) as u64
        };
        Ok(Some(Sample {
            interval: [sequence.interval_start, sequence.interval_end],
            frame: sequence.interval_start + offset as u32,
            elapsed_ms,
        }))
    }
}

fn millis(seconds: f64) -> Result<u64, String> {
    let ms = (seconds * 1000. + SCHEDULE_EPSILON * 1000.).floor();
    if ms >= u64::MAX as f64 {
        return Err("unit animation clock overflow".into());
    }
    Ok(ms as u64)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn model() -> Arc<Model> {
        let mut bytes = b"MDLX".to_vec();
        bytes.extend(b"VERS");
        bytes.extend(4u32.to_le_bytes());
        bytes.extend(800u32.to_le_bytes());
        bytes.extend(b"MODL");
        bytes.extend((pcx_mdl::MODL_SIZE as u32).to_le_bytes());
        bytes.extend(vec![0; pcx_mdl::MODL_SIZE]);
        let mut records = Vec::new();
        for (name, start, end, once, move_speed) in [
            ("Stand - 1", 100u32, 900u32, false, 0f32),
            ("Stand Defend", 1200, 1800, false, 0.),
            ("Death", 2000, 2400, true, 0.),
            ("Walk", 3000, 3800, false, 200.),
        ] {
            let mut record = vec![0; pcx_mdl::SEQUENCE_SIZE];
            record[..name.len()].copy_from_slice(name.as_bytes());
            record[80..84].copy_from_slice(&start.to_le_bytes());
            record[84..88].copy_from_slice(&end.to_le_bytes());
            record[88..92].copy_from_slice(&move_speed.to_le_bytes());
            record[92..96].copy_from_slice(&u32::from(once).to_le_bytes());
            records.extend(record);
        }
        bytes.extend(b"SEQS");
        bytes.extend((records.len() as u32).to_le_bytes());
        bytes.extend(records);
        Arc::new(
            Model::read(
                AssetIdentity {
                    source: "synthetic".into(),
                    edition: "test".into(),
                    version: "fixture".into(),
                },
                "fixture.mdx".into(),
                bytes.into(),
                4096,
            )
            .unwrap(),
        )
    }
    pub(crate) fn blended_model() -> Arc<Model> {
        let original = model();
        let mut bytes = original.bytes.to_vec();
        let parsed = pcx_mdl::Mdlx::parse(&bytes).unwrap();
        let header = parsed.chunk(pcx_mdl::tag::MODL).unwrap();
        let end = header.offset + 8 + pcx_mdl::MODL_SIZE;
        bytes[end - 4..end].copy_from_slice(&400u32.to_le_bytes());
        let mut bone = vec![0; 96];
        bone[..4].copy_from_slice(&176u32.to_le_bytes());
        bone[4..8].copy_from_slice(b"head");
        bone[88..92].copy_from_slice(&(-1i32).to_le_bytes());
        bone.extend(b"KGTR");
        bone.extend(4u32.to_le_bytes());
        bone.extend(1u32.to_le_bytes());
        bone.extend((-1i32).to_le_bytes());
        for (t, x) in [(100u32, 10f32), (900, 10.), (2000, 30.), (2400, 30.)] {
            bone.extend(t.to_le_bytes());
            bone.extend([x, 0., 0.].into_iter().flat_map(f32::to_le_bytes));
        }
        bone.extend([0; 8]);
        bytes.extend(b"BONE");
        bytes.extend((bone.len() as u32).to_le_bytes());
        bytes.extend(bone);
        Arc::new(
            Model::read(
                original.origin.clone(),
                original.member.clone(),
                bytes.into(),
                4096,
            )
            .unwrap(),
        )
    }
    fn posed(animation: &Animation, now: f64) -> pcx_mdl::Pose {
        let sample = animation.sample(now).unwrap().unwrap();
        let to = animation.model.poses(sample).unwrap()[0];
        match animation.blend_sample(now).unwrap() {
            None => to,
            Some(blend) => pcx_mdl::blend_joint_pose(
                &animation.model.joints[0],
                blend.from[0],
                to,
                blend.weight,
            )
            .unwrap(),
        }
    }
    #[test]
    fn authored_blend_and_interrupted_transitions_snapshot_the_visible_logical_pose() {
        let model = blended_model();
        assert_eq!(model.blend_time_ms, 400);
        let mut a = Animation::new(model, 0.).unwrap();
        assert_eq!(posed(&a, 0.).translation[0], 10.);
        a.select("Death", 0., None).unwrap();
        assert_eq!(posed(&a, 0.).translation[0], 10.);
        assert_eq!(posed(&a, 0.2).translation[0], 20.);
        a.select("Stand Defend", 0.2, None).unwrap();
        assert_eq!(posed(&a, 0.2).translation[0], 20.);
        assert!((posed(&a, 0.4).translation[0] - 10.).abs() < 1e-5);
        assert_eq!(posed(&a, 0.61).translation[0], 0.);
        a.select("Death", 0.7, None).unwrap();
        a.set_blend_time(0.).unwrap();
        assert_eq!(posed(&a, 0.7).translation[0], 30.);
        let before = a.clone();
        assert!(a.set_blend_time(f32::NAN).is_err());
        assert_eq!(a, before);
    }
    #[test]
    fn queue_blends_are_partition_independent_and_pause_shifts_the_transition() {
        let mut a = Animation::new(blended_model(), 0.).unwrap();
        a.select("Death", 0., None).unwrap();
        a.queue("Stand Defend", 0.).unwrap();
        let mut b = a.clone();
        for t in [0.1, 0.3, 0.4, 0.5] {
            a.advance(t).unwrap();
        }
        b.advance(0.5).unwrap();
        assert_eq!(a, b);
        assert!((posed(&a, 0.5).translation[0] - 22.5).abs() < 1e-5);
        let pose = posed(&a, 0.5);
        a.resume_after_pause(0.5, 4.5);
        assert_eq!(posed(&a, 4.5), pose);
    }
    #[test]
    fn truncated_model_header_does_not_invent_a_default_blend() {
        let original = model();
        let parsed = pcx_mdl::Mdlx::parse(&original.bytes).unwrap();
        let header = parsed.chunk(pcx_mdl::tag::MODL).unwrap();
        let mut bytes = original.bytes.to_vec();
        let end = header.offset + 8 + pcx_mdl::MODL_SIZE;
        bytes.drain(end - 4..end);
        bytes[header.offset + 4..header.offset + 8]
            .copy_from_slice(&((pcx_mdl::MODL_SIZE - 4) as u32).to_le_bytes());
        assert!(
            Model::read(
                original.origin.clone(),
                original.member.clone(),
                bytes.into(),
                4096
            )
            .unwrap_err()
            .contains("MODL")
        );
    }
    #[test]
    fn freezing_during_a_transition_freezes_its_local_pose_and_resumes_from_that_pose() {
        let mut a = Animation::new(blended_model(), 0.).unwrap();
        a.select("Death", 0., None).unwrap();
        a.set_time_scale(0., 0.1).unwrap();
        let pose = posed(&a, 0.1);
        assert_eq!(pose.translation[0], 15.);
        assert_eq!(posed(&a, 10.), pose);
        a.set_time_scale(2., 10.).unwrap();
        assert_eq!(posed(&a, 10.), pose);
        assert!((posed(&a, 10.1).translation[0] - 25.).abs() < 1e-5);
        assert_eq!(posed(&a, 10.2).translation[0], 30.);
        a.set_time_scale(-1., 10.2).unwrap();
        assert!(
            a.blend_sample(10.3).unwrap().is_none(),
            "reversing playback cannot resurrect an old completed blend"
        );
    }

    #[test]
    fn speed_changes_preserve_phase_freeze_resume_reverse_and_independent_global_clock() {
        let mut a = Animation::new(model(), 0.).unwrap();
        a.select("Walk", 1., Some(400.)).unwrap();
        assert_eq!(a.sample(1.1).unwrap().unwrap().frame, 3200);
        a.set_time_scale(0., 1.1).unwrap();
        let phase = a.sample(4.).unwrap().unwrap();
        assert_eq!(phase.frame, 3200);
        assert_eq!(phase.elapsed_ms, 4000);
        a.set_time_scale(0.5, 4.).unwrap();
        assert_eq!(a.sample(4.1).unwrap().unwrap().frame, 3300);
        a.set_time_scale(-1., 4.1).unwrap();
        assert_eq!(a.sample(4.2).unwrap().unwrap().frame, 3100);
        assert_eq!(a.sample(4.4).unwrap().unwrap().frame, 3500);
        a.select("Death", 5., None).unwrap();
        assert_eq!(a.sample(5.2).unwrap().unwrap().frame, 2000);
        assert!(a.interval_finished(5.2).unwrap());
        a.set_time_scale(2., 6.).unwrap();
        a.select("Death", 6., None).unwrap();
        assert_eq!(a.sample(6.1).unwrap().unwrap().frame, 2200);
        let before = a.clone();
        assert!(a.set_time_scale(f32::NAN, 6.1).is_err());
        assert!(a.set_time_scale(1., 5.9).is_err());
        assert_eq!(a, before);
    }
    #[test]
    fn queued_boundaries_follow_playback_speed_and_do_not_advance_while_frozen() {
        let mut a = Animation::new(model(), 0.).unwrap();
        a.select("Death", 0., None).unwrap();
        a.queue("Stand Defend", 0.).unwrap();
        a.set_time_scale(0., 0.1).unwrap();
        a.advance(10.).unwrap();
        assert_eq!(a.sequence, Some(2));
        a.set_time_scale(2., 10.).unwrap();
        a.advance(10.14).unwrap();
        assert_eq!(a.sequence, Some(2));
        a.advance(10.15).unwrap();
        assert_eq!(a.sequence, Some(1));
        assert_eq!(a.sample(10.2).unwrap().unwrap().frame, 1300);
        let finish = a.next_boundary(1, a.started).unwrap().unwrap();
        a.advance(finish - 1e-9).unwrap();
        assert_eq!(a.sequence, Some(1));
        a.advance(finish).unwrap();
        assert_eq!(a.sequence, Some(0));
    }

    #[test]
    fn model_paths_follow_declared_metadata_and_scalar_map_overrides() {
        use pcx_objects::editor::{FieldDef, ObjectKind};
        use pcx_sim::world::ObjectField;
        let field = FieldDef {
            fourcc: *b"M001",
            from: SlkId::UnitMetaData,
            field: "file".into(),
            slk: "UnitUI".into(),
            index: -1,
            declared: "model".into(),
            kind: FieldKind::Token,
            category: "art".into(),
            source: FieldSource::Column {
                slk: SlkId::UnitUi,
                column: "file".into(),
            },
            measured: true,
        };
        let mut definition = Definition {
            rawcode: *b"U001",
            base_rawcode: "U001".into(),
            kind: ObjectKind::Unit,
            fields: std::collections::BTreeMap::from([(
                *b"M001",
                ObjectField {
                    metadata: Some(field),
                    value: Ok(FieldValue::Cell("Units\\Synthetic\\Actor".into())),
                },
            )]),
            modifications: Vec::new(),
        };
        assert_eq!(
            model_path(&definition).unwrap().as_deref(),
            Some("Units\\Synthetic\\Actor.mdx")
        );
        definition.fields.get_mut(b"M001").unwrap().value = Ok(FieldValue::Override(
            pcx_objects::Value::Str(b"Map\\Edited.mdl".to_vec()),
        ));
        assert_eq!(
            model_path(&definition).unwrap().as_deref(),
            Some("Map\\Edited.mdx")
        );
        definition.fields.get_mut(b"M001").unwrap().value = Ok(FieldValue::Cell("_".into()));
        assert_eq!(model_path(&definition).unwrap(), None);
        definition.fields.get_mut(b"M001").unwrap().value =
            Ok(FieldValue::Cell("Map\\A,Map\\B".into()));
        assert!(model_path(&definition).is_err());
        definition
            .fields
            .get_mut(b"M001")
            .unwrap()
            .metadata
            .as_mut()
            .unwrap()
            .index = 0;
        assert!(model_path(&definition).is_err());
    }
    #[test]
    fn selected_sequence_drives_frames_without_wall_time_or_frame_partition_dependence() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        animation.select("sTaNd DeFeNd", 3., None).unwrap();
        assert_eq!(animation.sequence, Some(1));
        assert_eq!(
            animation.sample(3.25).unwrap(),
            Some(Sample {
                interval: [1200, 1800],
                frame: 1450,
                elapsed_ms: 1250
            })
        );
        let mut other = animation.clone();
        for time in [3.1, 3.3, 3.9, 4.25] {
            other.sample(time).unwrap();
        }
        assert_eq!(animation.sample(4.25).unwrap(), other.sample(4.25).unwrap());
        other.select("absent", 4.25, None).unwrap();
        assert_eq!(other, animation);
        animation.select("Death", 4., None).unwrap();
        assert_eq!(animation.sample(9.).unwrap().unwrap().frame, 2400);
        assert!(animation.sample(3.).is_err());
    }
    #[test]
    fn walk_rate_is_unit_speed_over_sequence_move_speed() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        // 400 / 200 = 2: the 250 ms of game time reaches 500 ms of Walk.
        animation.select("walk", 3., Some(400.)).unwrap();
        assert_eq!(animation.sample(3.25).unwrap().unwrap().frame, 3500);
        animation.select("walk", 4., None).unwrap();
        assert_eq!(animation.sample(4.25).unwrap().unwrap().frame, 3250);
        assert!(animation.select("walk", 5., Some(0.)).is_err());
        assert!(animation.select("walk", 5., Some(f32::NAN)).is_err());
        // Death has zero move speed: it keeps the authored clock instead of refusing.
        animation.select("death", 5., Some(400.)).unwrap();
        assert_eq!(animation.sample(5.25).unwrap().unwrap().frame, 2250);
    }
    #[test]
    fn queued_clips_run_fifo_from_authored_boundaries_and_sampling_is_immutable() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        animation.select("Death", 3., None).unwrap();
        assert!(animation.queue("Walk", 3.).unwrap());
        assert!(animation.queue("Stand Defend", 3.).unwrap());
        let before = animation.clone();
        for time in [3.1, 3.2, 3.4, 4.0, 4.9] {
            animation.sample(time).unwrap();
        }
        assert_eq!(animation, before);

        animation.advance(3.2).unwrap();
        assert_eq!(animation.sequence, Some(2));
        assert!(animation.has_pending());
        animation.advance(3.4).unwrap();
        assert_eq!((animation.sequence, animation.started), (Some(3), 3.4));
        assert!(animation.has_pending());
        animation.advance(4.2).unwrap();
        assert_eq!((animation.sequence, animation.started), (Some(1), 4.2));
        assert!(!animation.has_pending());
        animation.advance(4.8).unwrap();
        assert_eq!((animation.sequence, animation.started), (Some(0), 4.8));
        assert_eq!(animation.sample(5.).unwrap().unwrap().frame, 300);

        let mut large = Animation::new(model(), 2.).unwrap();
        large.select("Death", 3., None).unwrap();
        large.queue("Walk", 3.).unwrap();
        large.queue("Stand Defend", 3.).unwrap();
        large.advance(5.).unwrap();
        assert_eq!(large, animation);
    }

    #[test]
    fn late_enqueue_waits_for_the_next_authored_boundary_after_the_request() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        assert!(animation.queue("Death", 4.5).unwrap());
        animation.advance(5.1).unwrap();
        assert_eq!((animation.sequence, animation.started), (Some(0), 2.));
        assert!(animation.has_pending());
        animation.advance(5.2).unwrap();
        assert_eq!((animation.sequence, animation.started), (Some(2), 5.2));
        assert!(!animation.has_pending());
    }

    #[test]
    fn advancing_below_a_boundary_leaves_the_same_time_sampleable() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        animation.select("Death", 3., None).unwrap();
        assert!(animation.queue("Walk", 3.).unwrap());
        let just_below = 3.4 - SCHEDULE_EPSILON / 2.;
        animation.advance(just_below).unwrap();
        assert_eq!((animation.sequence, animation.started), (Some(2), 3.));
        assert!(animation.sample(just_below).is_ok());
        animation.advance(3.4).unwrap();
        assert_eq!((animation.sequence, animation.started), (Some(3), 3.4));
        assert!(animation.sample(3.4).is_ok());
    }

    #[test]
    fn missing_queue_names_retain_state_and_explicit_selection_interrupts_the_fifo() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        animation.select("death", 3., None).unwrap();
        animation.queue("walk", 3.).unwrap();
        let before = animation.clone();
        assert!(!animation.queue("absent", 3.).unwrap());
        assert_eq!(animation, before);
        animation.select("absent", 3.5, None).unwrap();
        assert_eq!(animation, before);
        animation.select("stand defend", 3.5, None).unwrap();
        assert!(!animation.has_pending());
        assert_eq!(animation.sequence, Some(1));
    }

    #[test]
    fn bounded_queue_overflow_refuses_before_mutation() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        for _ in 0..QUEUED_LIMIT {
            assert!(animation.queue("walk", 2.).unwrap());
        }
        let before = animation.clone();
        assert_eq!(
            animation.queue("death", 2.),
            Err("unit animation queue capacity".into())
        );
        assert_eq!(animation, before);
    }

    #[test]
    fn candidate_walk_reselect_restarts_unmeasured_and_invalid_move_speed_refuses() {
        let mut animation = Animation::new(model(), 2.).unwrap();
        animation.select("walk", 3., Some(400.)).unwrap();
        assert_eq!(animation.sample(3.25).unwrap().unwrap().frame, 3500);
        // Candidate, unmeasured: a reselect restarts Walk. Original phase on a new order is unknown.
        animation.select("walk", 3.5, Some(200.)).unwrap();
        assert_eq!(animation.started, 3.5);
        assert_eq!(animation.sample(3.75).unwrap().unwrap().frame, 3250);
        let mut bad = (*model()).clone();
        bad.sequences[3].move_speed = -1.;
        let mut animation = Animation::new(Arc::new(bad), 2.).unwrap();
        assert!(animation.select("walk", 3., Some(400.)).is_err());
        assert!(animation.select("walk", 3., None).is_ok());
    }
}
