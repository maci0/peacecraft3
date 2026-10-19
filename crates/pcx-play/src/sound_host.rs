use super::{Entry, Error, Failure, ObjectHost};
use crate::label_sounds::{BoundSound, Budget, LoadBudget, PcmResource, SoundBank};
pub(super) type Loader = Box<dyn FnMut(&str, Budget) -> Result<SoundBank, String> + Send + Sync>;
use pcx_script::{HandleId, Value};

/// Exact boolean inputs of CreateSoundFromLabel; not effective mixer settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabelSoundFlags {
    pub looping: bool,
    pub is_3d: bool,
    pub stop_when_out_of_range: bool,
}

#[derive(Debug)]
pub struct SoundState<'a> {
    pub identity: HandleId,
    /// Exact last SetSoundDuration argument; unset until explicitly assigned.
    pub duration: Option<i32>,
    /// Selected source table, including the bound row's authored settings.
    pub bank: &'a SoundBank,
    pub resource: &'a BoundSound,
    /// None for CreateMIDISound, whose interface supplies no boolean flags.
    pub creation_flags: Option<LabelSoundFlags>,
    /// Exact authored integers; no fade units, clamping or scheduling are inferred.
    pub fade_in_rate: i32,
    pub fade_out_rate: i32,
}
impl ObjectHost {
    fn label_banks(&self, name: &str) -> Option<&[SoundBank]> {
        match name {
            "CreateMIDISound" => self.sounds.as_deref(),
            "CreateSoundFromLabel" => self.label_sound_creation.as_deref(),
            _ => None,
        }
    }
    fn label_sound_remaining(
        &self,
        name: &str,
        budget: LoadBudget,
    ) -> Result<(usize, Budget), Error> {
        let fail = || {
            self.inner
                .error(name, Failure::Budget("label sound inputs"))
        };
        std::alloc::Layout::array::<SoundBank>(budget.banks)
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.total_table_bytes))
            .and_then(|_| std::alloc::Layout::array::<BoundSound>(budget.inputs.sounds))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.inputs.table_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.inputs.resource_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.inputs.total_resource_bytes))
            .and_then(|_| std::alloc::Layout::array::<u8>(budget.inputs.metadata_bytes))
            .map_err(|_| fail())?;
        let mut banks = budget.banks;
        let mut tables = budget.total_table_bytes;
        let mut remaining = budget.inputs;
        for bank in self
            .sounds
            .iter()
            .flatten()
            .chain(self.label_sound_creation.iter().flatten())
        {
            let (table, metadata) = bank.input_usage();
            if table > budget.inputs.table_bytes {
                return Err(fail());
            }
            banks = banks.checked_sub(1).ok_or_else(fail)?;
            tables = tables.checked_sub(table).ok_or_else(fail)?;
            remaining.sounds = remaining
                .sounds
                .checked_sub(bank.sounds().len())
                .ok_or_else(fail)?;
            remaining.metadata_bytes = remaining
                .metadata_bytes
                .checked_sub(metadata)
                .ok_or_else(fail)?;
            for sound in bank.sounds() {
                if sound.bytes().len() > budget.inputs.resource_bytes {
                    return Err(fail());
                }
                remaining.total_resource_bytes = remaining
                    .total_resource_bytes
                    .checked_sub(sound.bytes().len())
                    .ok_or_else(fail)?;
            }
        }
        remaining.table_bytes = remaining.table_bytes.min(tables);
        remaining.resource_bytes = remaining.resource_bytes.min(remaining.total_resource_bytes);
        Ok((banks, remaining))
    }
    pub fn admit_label_sound_loader(
        &mut self,
        budget: LoadBudget,
        loader: impl FnMut(&str, Budget) -> Result<SoundBank, String> + Send + Sync + 'static,
    ) -> Result<(), Error> {
        let name = "CreateSoundFromLabel";
        if self.label_sound_loader.is_some()
            || (self.sounds.is_none() && self.label_sound_creation.is_none())
        {
            return Err(self.inner.error(
                name,
                Failure::SoundState("label constructors required; loader must be admitted once"),
            ));
        }
        self.label_sound_remaining(name, budget)?;
        self.label_sound_loader = Some((Box::new(loader), budget));
        Ok(())
    }
    fn prepare_label_sound(&mut self, name: &str, label: &str) -> Result<usize, Error> {
        let fail = |reason| self.inner.error(name, reason);
        let banks = self
            .label_banks(name)
            .ok_or_else(|| fail(Failure::Signature))?;
        let limit = banks
            .iter()
            .map(SoundBank::label_limit)
            .max()
            .unwrap_or(0)
            .max(
                self.label_sound_loader
                    .as_ref()
                    .map_or(0, |(_, b)| b.inputs.metadata_bytes),
            );
        if label.trim().is_empty() || label.contains('\0') {
            return Err(fail(Failure::SoundState("empty or NUL-containing label")));
        }
        if label.len() > limit {
            return Err(fail(Failure::Budget("sound label")));
        }
        if let Some(index) = banks
            .iter()
            .flat_map(|b| b.sounds())
            .position(|s| s.label() == label)
        {
            return Ok(index);
        }
        let budget = self
            .label_sound_loader
            .as_ref()
            .map(|(_, b)| *b)
            .ok_or_else(|| fail(Failure::MissingSoundLabel(label.into())))?;
        let (banks_left, remaining) = self.label_sound_remaining(name, budget)?;
        if banks_left == 0
            || remaining.sounds == 0
            || remaining.table_bytes == 0
            || remaining.resource_bytes == 0
        {
            return Err(fail(Failure::Budget("label sound inputs")));
        }
        let loader = &mut self
            .label_sound_loader
            .as_mut()
            .ok_or_else(|| fail(Failure::Signature))?
            .0;
        let next = loader(label, remaining)
            .map_err(|e| self.inner.error(name, Failure::SoundSource(e)))?;
        let (table, metadata) = next.input_usage();
        if next.sounds().len() != 1 || next.sounds()[0].label() != label {
            return Err(self.inner.error(
                name,
                Failure::SoundState("loader must return one matching label"),
            ));
        }
        if table > remaining.table_bytes
            || metadata > remaining.metadata_bytes
            || next.sounds()[0].bytes().len() > remaining.resource_bytes
        {
            return Err(self
                .inner
                .error(name, Failure::Budget("label sound inputs")));
        }
        let banks = match name {
            "CreateMIDISound" => self.sounds.as_mut(),
            "CreateSoundFromLabel" => self.label_sound_creation.as_mut(),
            _ => None,
        }
        .ok_or_else(|| self.inner.error(name, Failure::Signature))?;
        let index = banks.iter().map(|b| b.sounds().len()).sum();
        banks
            .try_reserve(1)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        // Append only: flattening existing bank resources must keep every old index.
        banks.push(next);
        Ok(index)
    }
    pub(super) fn has_sound_resources(&self) -> bool {
        self.sounds.is_some() || self.label_sound_creation.is_some() || self.file_sounds.is_some()
    }
    /// Admit caller-selected table/resource banks independently of CreateMIDISound.
    /// Each bank is bounded before this call; duplicate labels across banks refuse.
    /// The constructor and inherited sound types must share CreateTimer's source.
    pub fn with_label_sound_creation(mut self, banks: Vec<SoundBank>) -> Result<Self, Error> {
        let name = "CreateSoundFromLabel";
        let fail = |reason| self.inner.error(name, reason);
        if self.label_sound_creation.is_some() {
            return Err(fail(Failure::SoundState("already admitted")));
        }
        Self::contract(
            &self.inner,
            name,
            &[
                "string", "boolean", "boolean", "boolean", "integer", "integer",
            ],
            "sound",
            false,
            Some("sound"),
        )?;
        let program = &self.inner.program;
        let native = program
            .natives()
            .get(name)
            .ok_or_else(|| fail(Failure::Signature))?;
        let timer = program
            .natives()
            .get("CreateTimer")
            .ok_or_else(|| fail(Failure::Signature))?;
        if native.source != timer.source {
            return Err(fail(Failure::Source));
        }
        let mut kind = native.ret.kind.name();
        for _ in 0..=program.types().count() {
            if kind == "handle" {
                break;
            }
            let ty = program
                .types()
                .find(|ty| ty.name == kind)
                .ok_or_else(|| fail(Failure::Signature))?;
            if ty.source != native.source {
                return Err(fail(Failure::Source));
            }
            kind = &ty.parent;
        }
        if kind != "handle" {
            return Err(fail(Failure::Signature));
        }
        if banks.is_empty() {
            return Err(fail(Failure::SoundState("source banks required")));
        }
        let mut records = 0usize;
        for (at, bank) in banks.iter().enumerate() {
            records = records
                .checked_add(bank.sounds().len())
                .ok_or_else(|| fail(Failure::Capacity))?;
            for sound in bank.sounds() {
                if banks[..at]
                    .iter()
                    .any(|previous| previous.sounds().iter().any(|s| s.label() == sound.label()))
                {
                    return Err(fail(Failure::SoundState("ambiguous source label")));
                }
            }
        }
        self.label_sound_creation = Some(banks);
        Ok(self)
    }
    /// Admits raw authored duration state, not a timer or playback-length override.
    pub fn with_sound_duration(mut self) -> Result<Self, Error> {
        let name = "SetSoundDuration";
        if self.sound_duration || !self.has_sound_resources() {
            return Err(self.inner.error(
                name,
                Failure::SoundState("sound resources required; duration must be admitted once"),
            ));
        }
        Self::contract(
            &self.inner,
            name,
            &["sound", "integer"],
            "nothing",
            false,
            Some("sound"),
        )?;
        self.sound_duration = true;
        Ok(self)
    }
    pub fn sound_duration(&self, id: HandleId) -> Result<Option<i32>, Error> {
        match self.inner.entry(id)? {
            Entry::Sound { duration, .. } | Entry::FileSound { duration, .. } => Ok(*duration),
            _ => Err(self.inner.error("SetSoundDuration", Failure::WrongFamily)),
        }
    }
    pub(super) fn sound_duration_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name != "SetSoundDuration" || !self.sound_duration {
            return None;
        }
        Some((|| {
            let fail = |reason| self.inner.error(name, reason);
            let [Value::Handle(id), Value::Int(value)] = args else {
                return Err(fail(Failure::SoundState(
                    "expected sound identity and integer duration",
                )));
            };
            self.sound_admitted(*id).map_err(|e| fail(e.failure))?;
            let at = usize::try_from(id.token() - 1).map_err(|_| fail(Failure::Identity))?;
            let (Entry::Sound { duration, .. } | Entry::FileSound { duration, .. }) =
                &mut self.inner.arena[at]
            else {
                return Err(self.inner.error(name, Failure::WrongFamily));
            };
            *duration = Some(*value);
            Ok(Value::Null)
        })())
    }
    pub fn with_label_sounds(mut self, bank: SoundBank) -> Result<Self, Error> {
        if self.sounds.is_some() {
            return Err(self
                .inner
                .error("CreateMIDISound", Failure::SoundState("already admitted")));
        }
        Self::contract(
            &self.inner,
            "CreateMIDISound",
            &["string", "integer", "integer"],
            "sound",
            false,
            Some("sound"),
        )?;
        self.sounds = Some(vec![bank]);
        Ok(self)
    }
    /// Initial explicit MIDI bank. Per-object sources are available through `sound_state`.
    pub fn sound_bank(&self) -> Option<&SoundBank> {
        self.sounds.as_ref()?.first()
    }
    /// Encoded bytes already retained by the admitted sound banks, without copying.
    /// Repeated labels or equal files are conservatively counted separately.
    pub fn sound_resource_bytes(&self) -> Result<usize, Error> {
        let labels = self
            .sounds
            .iter()
            .flatten()
            .chain(self.label_sound_creation.iter().flatten())
            .flat_map(SoundBank::sounds)
            .map(|sound| sound.bytes().len());
        let files = self.file_sound_assets();
        let pcm = files
            .into_iter()
            .flat_map(|bank| bank.resources())
            .map(|sound| sound.bytes().len());
        let mp3 = files
            .into_iter()
            .flat_map(|bank| bank.mp3_resources())
            .map(|sound| sound.bytes().len());
        labels
            .chain(pcm)
            .chain(mp3)
            .try_fold(0usize, |total, bytes| {
                total.checked_add(bytes).ok_or_else(|| {
                    self.inner
                        .error("StartSound", Failure::Budget("admitted sound bytes"))
                })
            })
    }

    /// Queues sound starts for an explicit audio consumer. Admission does not
    /// install a playback device or infer channel, fade, looping or restart rules.
    pub fn with_sound_starts(mut self, max_pending: usize) -> Result<Self, Error> {
        let name = "StartSound";
        if !self.has_sound_resources() || self.sound_start_limit.is_some() {
            return Err(self.inner.error(
                name,
                Failure::SoundState("sound bank required; start queue must not be admitted twice"),
            ));
        }
        Self::contract(
            &self.inner,
            name,
            &["sound"],
            "nothing",
            false,
            Some("sound"),
        )?;
        std::alloc::Layout::array::<HandleId>(max_pending)
            .map_err(|_| self.inner.error(name, Failure::Capacity))?;
        self.sound_start_limit = Some(max_pending);
        Ok(self)
    }
    /// Every successful request remains in execution order, including repeats.
    pub fn pending_sound_starts(&self) -> &[HandleId] {
        &self.sound_starts
    }
    /// Transfers pending work to the consumer; sound identities/resources remain
    /// owned by this Host. The consumer owns retry/failure handling after transfer.
    pub fn take_sound_starts(&mut self) -> Vec<HandleId> {
        std::mem::take(&mut self.sound_starts)
    }
    pub fn sound_state(&self, identity: HandleId) -> Result<SoundState<'_>, Error> {
        let fail = |reason| self.inner.error("CreateMIDISound", reason);
        let Entry::Sound {
            resource,
            creation_flags,
            fade_in_rate,
            fade_out_rate,
            ..
        } = self.inner.entry(identity)?
        else {
            return Err(fail(Failure::WrongFamily));
        };
        let banks = if creation_flags.is_some() {
            self.label_sound_creation.as_deref()
        } else {
            self.sounds.as_deref()
        }
        .ok_or_else(|| fail(Failure::SoundState("not admitted")))?;
        let (bank, resource) = banks
            .iter()
            .flat_map(|bank| bank.sounds().iter().map(move |sound| (bank, sound)))
            .nth(*resource)
            .ok_or_else(|| fail(Failure::Identity))?;
        Ok(SoundState {
            identity,
            bank,
            creation_flags: *creation_flags,
            duration: self.sound_duration(identity)?,
            resource,
            fade_in_rate: *fade_in_rate,
            fade_out_rate: *fade_out_rate,
        })
    }
    pub(crate) fn sound_admitted(&self, identity: HandleId) -> Result<(), Error> {
        match self.inner.entry(identity)? {
            Entry::FileSound { mp3: true, .. } => self.file_mp3_state(identity).map(|_| ()),
            Entry::Sound { .. } | Entry::FileSound { mp3: false, .. } => {
                self.sound_resource(identity).map(|_| ())
            }
            _ => Err(self.inner.error("sound resource", Failure::WrongFamily)),
        }
    }
    /// Common checked PCM access for both label and direct-file sound objects.
    pub fn sound_resource(&self, identity: HandleId) -> Result<&PcmResource, Error> {
        match self.inner.entry(identity)? {
            Entry::Sound { .. } => Ok(self.sound_state(identity)?.resource.resource()),
            Entry::FileSound { .. } => Ok(self.file_sound_state(identity)?.resource),
            _ => Err(self.inner.error("sound resource", Failure::WrongFamily)),
        }
    }
    pub(super) fn sound_native(
        &mut self,
        name: &str,
        args: &[Value],
    ) -> Option<Result<Value, Error>> {
        if name == "StartSound" {
            let limit = self.sound_start_limit?;
            return Some((|| {
                let [Value::Handle(id)] = args else {
                    return Err(self
                        .inner
                        .error(name, Failure::SoundState("expected sound identity")));
                };
                self.sound_admitted(*id)
                    .map_err(|e| self.inner.error(name, e.failure))?;
                if self.sound_starts.len() >= limit {
                    return Err(self
                        .inner
                        .error(name, Failure::Budget("pending sound starts")));
                }
                self.sound_starts
                    .try_reserve(1)
                    .map_err(|_| self.inner.error(name, Failure::Capacity))?;
                self.sound_starts.push(*id);
                Ok(Value::Null)
            })());
        }
        self.label_banks(name)?;
        let fail = |reason| self.inner.error(name, reason);
        let (label, fade_in_rate, fade_out_rate, creation_flags) = match (name, args) {
            ("CreateMIDISound", [Value::Str(label), Value::Int(a), Value::Int(b)]) => {
                (label, *a, *b, None)
            }
            (
                "CreateSoundFromLabel",
                [
                    Value::Str(label),
                    Value::Bool(looping),
                    Value::Bool(is_3d),
                    Value::Bool(stop),
                    Value::Int(a),
                    Value::Int(b),
                ],
            ) => (
                label,
                *a,
                *b,
                Some(LabelSoundFlags {
                    looping: *looping,
                    is_3d: *is_3d,
                    stop_when_out_of_range: *stop,
                }),
            ),
            _ => {
                return Some(Err(fail(Failure::SoundState(
                    "arguments do not match the sound constructor",
                ))));
            }
        };
        let resource = match self.prepare_label_sound(name, label) {
            Ok(index) => index,
            Err(error) => return Some(Err(error)),
        };
        Some(
            self.inner
                .allocate(
                    Entry::Sound {
                        resource,
                        creation_flags,
                        properties: super::SoundProperties::default(),
                        parameters: None,
                        duration: None,
                        fade_in_rate,
                        fade_out_rate,
                    },
                    name,
                )
                .map(Value::Handle),
        )
    }
}
