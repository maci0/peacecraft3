//! Play published `ObjectHost` `StartSound` requests through Bevy audio.
//! `MapScriptSession` exposes only a successfully published host, so a failed
//! `replace` cannot drain its candidate queue.
//! The queue stores sound identities, not setting snapshots. Each consumed
//! start samples that identity's current creation inputs, properties and
//! parameter assignment.
//! Playback is the recorded non-spatial, zero-fade profile. 3D, EAX, distance,
//! channel, fade, duration, parameter rows and unmapped volume/pitch are
//! reported and are not replaced with another mix. `PlaybackSettings::LOOP` and
//! `Remove` select lifetime only. Their backend gain and speed are not source
//! measurements.
//! PCM uses `DecoderWave`'s fmt/data-first view. MP3 keeps its encoded bytes.
//! The voice receipt stays the original resource digest.
//! A voice's session is `MapScriptSession::session_witness`, the same `Arc`
//! `CallbackTicket` stores. Looping voices stay while that witness is current.
//! A finished one-shot is despawned and its source handle is dropped.

use crate::map_script_session::MapScriptSession;
use bevy::audio::AudioSinkPlayback;
use bevy::prelude::*;
use pcx_play::label_sounds::PcmResource;
use pcx_play::map_startup_host::MapStartupHost;
use pcx_play::pcm_decoder::DecoderWave;
use pcx_play::playercolor_host::ObjectHost;
use pcx_play::playercolor_host::SoundProperties;
use pcx_play::scripts::AssetIdentity;
use pcx_script::HandleId;
use std::{collections::HashSet, sync::Weak};
use tracing::warn;

/// Optional caller caps for one batch and the retained source cache.
/// Without an override, both ceilings come from the published Host's admitted
/// encoded bank bytes. This does not bound backend decoder or total heap usage.
#[derive(Resource, Clone, Copy, Debug)]
pub struct ScriptAudioLimits {
    pub max_staged_bytes: usize,
    pub max_live_bytes: usize,
}

#[derive(Resource, Default)]
pub struct ScriptAudioReport {
    pub admission_error: Option<String>,
    pub last_refusal: Option<String>,
    pub refusal_count: u64,
    pub played: u64,
}

#[derive(Clone, Component)]
pub struct ScriptVoice {
    pub session: Weak<()>,
    pub handle: HandleId,
    pub looping: bool,
    pub fade_in_rate: i32,
    pub fade_out_rate: i32,
    pub encoded_mp3: bool,
    pub source: Handle<AudioSource>,
    pub member: String,
    pub source_label: String,
    pub origin_source: String,
    pub origin_edition: String,
    pub origin_version: String,
    pub sha256: [u8; 32],
}

#[derive(Resource, Default)]
struct ScriptAudioMemory {
    sources: Vec<LiveSource>,
    playing: HashSet<HandleId>,
}

struct LiveSource {
    sha256: [u8; 32],
    encoded_mp3: bool,
    bytes: usize,
    handle: Handle<AudioSource>,
    voices: u32,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptAudioPlayback;

pub fn install(app: &mut App) {
    app.init_resource::<ScriptAudioReport>()
        .init_resource::<ScriptAudioMemory>()
        .init_resource::<Assets<AudioSource>>()
        .init_resource::<MapScriptSession<MapStartupHost>>()
        .add_systems(
            Update,
            play_published_script_sounds
                .in_set(ScriptAudioPlayback)
                .after(crate::campaign_startup::CampaignStartupSet),
        );
}

fn play_published_script_sounds(
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut commands: Commands,
    mut assets: ResMut<Assets<AudioSource>>,
    mut report: ResMut<ScriptAudioReport>,
    mut memory: ResMut<ScriptAudioMemory>,
    limits: Option<Res<ScriptAudioLimits>>,
    voices: Query<(Entity, &ScriptVoice, Option<&AudioSink>, Has<AudioPlayer>)>,
) {
    let current = session.session_witness();
    reclaim_script_voices(
        &mut commands,
        &mut assets,
        &mut memory,
        &voices,
        current.as_ref(),
    );
    let Some(witness) = current else {
        return;
    };
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects_mut();
    if objects.pending_sound_starts().is_empty() {
        return;
    }
    let limits = match limits.as_deref().copied() {
        Some(limits) => limits,
        None => match objects.sound_resource_bytes() {
            Ok(bytes) => ScriptAudioLimits {
                max_staged_bytes: bytes,
                max_live_bytes: bytes,
            },
            Err(error) => {
                remember(&mut report, &error.to_string());
                return;
            }
        },
    };
    if let Err(error) = publish_queued_starts(
        objects,
        &witness,
        limits,
        &mut memory,
        &mut commands,
        &mut assets,
        &mut report,
    ) {
        remember(&mut report, &error);
    }
}

fn reclaim_script_voices(
    commands: &mut Commands,
    assets: &mut Assets<AudioSource>,
    memory: &mut ScriptAudioMemory,
    voices: &Query<(Entity, &ScriptVoice, Option<&AudioSink>, Has<AudioPlayer>)>,
    current: Option<&Weak<()>>,
) {
    let mut finished = Vec::new();
    memory.playing.clear();
    for (entity, voice, sink, has_player) in voices.iter() {
        let current_voice = current.is_some_and(|session| Weak::ptr_eq(session, &voice.session));
        let sink_empty = sink.map(AudioSinkPlayback::empty);
        if !current_voice || oneshot_completed(voice.looping, has_player, sink_empty) {
            finished.push((entity, voice.source.clone()));
        } else {
            memory.playing.insert(voice.handle);
        }
    }
    for (entity, source) in finished {
        memory.release(assets, &source);
        commands.entity(entity).despawn();
    }
}

fn oneshot_completed(looping: bool, has_player: bool, sink_empty: Option<bool>) -> bool {
    if looping {
        return false;
    }
    if sink_empty == Some(true) {
        return true;
    }
    !has_player
}

fn remember(report: &mut ScriptAudioReport, error: &str) {
    if report.admission_error.as_deref() != Some(error) {
        warn!("{error}");
        report.admission_error = Some(error.to_string());
    }
}

enum Classified {
    Play(PlayPlan),
    Refuse(HandleId, String),
}

struct PlayPlan {
    id: HandleId,
    encoded_mp3: bool,
    looping: bool,
    fade_in_rate: i32,
    fade_out_rate: i32,
    sha256: [u8; 32],
    byte_len: usize,
}

impl PlayPlan {
    fn key(&self) -> ([u8; 32], bool) {
        (self.sha256, self.encoded_mp3)
    }
}

struct Observed<'a> {
    encoded_mp3: bool,
    byte_len: usize,
    sha256: [u8; 32],
    looping: bool,
    is_3d: bool,
    stop_when_out_of_range: bool,
    fade_in_rate: i32,
    fade_out_rate: i32,
    eax: &'a str,
    duration: Option<i32>,
}

struct Prepared {
    plan: PlayPlan,
    bytes: Option<Vec<u8>>,
}

fn publish_queued_starts(
    objects: &mut ObjectHost,
    session: &Weak<()>,
    limits: ScriptAudioLimits,
    memory: &mut ScriptAudioMemory,
    commands: &mut Commands,
    assets: &mut Assets<AudioSource>,
    report: &mut ScriptAudioReport,
) -> Result<(), String> {
    if objects.pending_sound_starts().is_empty() {
        return Ok(());
    }
    let mut ids = Vec::new();
    ids.try_reserve_exact(objects.pending_sound_starts().len())
        .map_err(|_| "script audio start list allocation failed".to_string())?;
    ids.extend_from_slice(objects.pending_sound_starts());
    let mut classified = Vec::new();
    classified
        .try_reserve_exact(ids.len())
        .map_err(|_| "script audio start list allocation failed".to_string())?;
    let mut seen = HashSet::new();
    seen.try_reserve(ids.len())
        .map_err(|_| "script audio start identity allocation failed".to_string())?;
    for id in ids {
        if memory.playing.contains(&id) || !seen.insert(id) {
            classified.push(Classified::Refuse(
                id,
                "repeat StartSound on an active handle has no admitted playback behavior".into(),
            ));
        } else {
            classified.push(classify(objects, id)?);
        }
    }
    let mut staged = 0usize;
    let mut added_live = 0usize;
    let mut seen_new = Vec::<([u8; 32], bool)>::new();
    for item in &classified {
        let Classified::Play(plan) = item else {
            continue;
        };
        if memory.reusable(plan) || seen_new.iter().any(|key| key == &plan.key()) {
            continue;
        }
        staged = add_bytes(staged, plan.byte_len)?;
        added_live = add_bytes(added_live, plan.byte_len)?;
        seen_new.push(plan.key());
    }
    if staged > limits.max_staged_bytes {
        return Err("script audio staging limit exceeded".to_string());
    }
    let live = memory
        .live_bytes()?
        .checked_add(added_live)
        .ok_or_else(|| "script audio byte total overflowed".to_string())?;
    if live > limits.max_live_bytes {
        return Err("script audio live limit exceeded".to_string());
    }
    let mut prepared = Vec::new();
    prepared
        .try_reserve_exact(classified.len())
        .map_err(|_| "script audio start list allocation failed".to_string())?;
    for item in classified {
        match item {
            Classified::Refuse(id, reason) => prepared.push(Err((id, reason))),
            Classified::Play(plan) => {
                let bytes = if memory.reusable(&plan)
                    || prepared.iter().any(|earlier| {
                        earlier.as_ref().ok().is_some_and(|ready: &Prepared| {
                            ready.plan.key() == plan.key() && ready.bytes.is_some()
                        })
                    }) {
                    None
                } else {
                    Some(load_bytes(objects, &plan)?)
                };
                prepared.push(Ok(Prepared { plan, bytes }));
            }
        }
    }
    let mut ready = Vec::new();
    ready
        .try_reserve_exact(prepared.len())
        .map_err(|_| "script audio start list allocation failed".to_string())?;
    let mut committed = Vec::new();
    committed
        .try_reserve_exact(prepared.len())
        .map_err(|_| "script audio rollback list allocation failed".to_string())?;
    memory
        .sources
        .try_reserve(prepared.len())
        .map_err(|_| "script audio source cache allocation failed".to_string())?;
    memory
        .playing
        .try_reserve(prepared.len())
        .map_err(|_| "script audio active identity allocation failed".to_string())?;
    for item in prepared {
        match item {
            Err(refusal) => ready.push(Err(refusal)),
            Ok(prepared) => match commit_source(objects, assets, memory, session, prepared) {
                Ok(voice) => {
                    committed.push(voice.source.clone());
                    ready.push(Ok(voice));
                }
                Err(error) => {
                    for handle in committed {
                        memory.release(assets, &handle);
                    }
                    return Err(error);
                }
            },
        }
    }
    objects.take_sound_starts();
    for item in ready {
        match item {
            Err((handle, reason)) => {
                let token = handle.token();
                warn!("script sound {token} refused: {reason}");
                report.last_refusal = Some(format!("sound {token}: {reason}"));
                report.refusal_count = report.refusal_count.saturating_add(1);
            }
            Ok(voice) => {
                let playback = if voice.looping {
                    PlaybackSettings::LOOP
                } else {
                    PlaybackSettings::REMOVE
                };
                let source = voice.source.clone();
                memory.playing.insert(voice.handle);
                commands.spawn((AudioPlayer(source), playback, voice));
                report.played = report.played.saturating_add(1);
            }
        }
    }
    report.admission_error = None;
    Ok(())
}

fn classify(objects: &ObjectHost, id: HandleId) -> Result<Classified, String> {
    if let Ok(state) = objects.file_mp3_state(id) {
        return plan_file(
            objects,
            id,
            Observed {
                encoded_mp3: true,
                byte_len: state.resource.bytes().len(),
                sha256: state.resource.sha256(),
                looping: state.looping,
                is_3d: state.is_3d,
                stop_when_out_of_range: state.stop_when_out_of_range,
                fade_in_rate: state.fade_in_rate,
                fade_out_rate: state.fade_out_rate,
                eax: state.eax_setting,
                duration: state.duration,
            },
        );
    }
    if let Ok(state) = objects.file_sound_state(id) {
        return plan_file(
            objects,
            id,
            Observed {
                encoded_mp3: false,
                byte_len: state.resource.bytes().len(),
                sha256: state.resource.sha256(),
                looping: state.looping,
                is_3d: state.is_3d,
                stop_when_out_of_range: state.stop_when_out_of_range,
                fade_in_rate: state.fade_in_rate,
                fade_out_rate: state.fade_out_rate,
                eax: state.eax_setting,
                duration: state.duration,
            },
        );
    }
    let state = objects.sound_state(id).map_err(|error| error.to_string())?;
    let Some(flags) = state.creation_flags else {
        return Ok(Classified::Refuse(
            id,
            "CreateMIDISound records no looping or spatial input".to_string(),
        ));
    };
    plan_file(
        objects,
        id,
        Observed {
            encoded_mp3: false,
            byte_len: state.resource.bytes().len(),
            sha256: state.resource.sha256(),
            looping: flags.looping,
            is_3d: flags.is_3d,
            stop_when_out_of_range: flags.stop_when_out_of_range,
            fade_in_rate: state.fade_in_rate,
            fade_out_rate: state.fade_out_rate,
            eax: "",
            duration: state.duration,
        },
    )
}

fn plan_file(
    objects: &ObjectHost,
    id: HandleId,
    observed: Observed<'_>,
) -> Result<Classified, String> {
    if let Some(reason) = creation_block(
        observed.is_3d,
        observed.stop_when_out_of_range,
        observed.eax,
        observed.fade_in_rate,
        observed.fade_out_rate,
        observed.duration,
    ) {
        return Ok(Classified::Refuse(id, reason.to_string()));
    }
    let properties = *objects
        .sound_properties(id)
        .map_err(|error| error.to_string())?;
    if let Some(reason) = property_block(&properties) {
        return Ok(Classified::Refuse(id, reason.to_string()));
    }
    if let Some(reason) = parameter_block(objects, id)? {
        return Ok(Classified::Refuse(id, reason));
    }
    Ok(Classified::Play(PlayPlan {
        id,
        encoded_mp3: observed.encoded_mp3,
        looping: observed.looping,
        fade_in_rate: observed.fade_in_rate,
        fade_out_rate: observed.fade_out_rate,
        sha256: observed.sha256,
        byte_len: observed.byte_len,
    }))
}

fn creation_block(
    is_3d: bool,
    stop_when_out_of_range: bool,
    eax: &str,
    fade_in_rate: i32,
    fade_out_rate: i32,
    duration: Option<i32>,
) -> Option<&'static str> {
    if is_3d {
        return Some("3D placement is not played");
    }
    if stop_when_out_of_range {
        return Some("out-of-range stop is not played");
    }
    if !eax.is_empty() {
        return Some("EAX profile is not played");
    }
    if fade_in_rate != 0 || fade_out_rate != 0 {
        return Some("fade timing is not played");
    }
    if duration.is_some() {
        return Some("duration override is not played");
    }
    None
}

fn property_block(properties: &SoundProperties) -> Option<&'static str> {
    if properties.channel.is_some() {
        return Some("channel routing is not played");
    }
    if properties.volume.is_some() {
        return Some("integer volume has no recorded linear gain");
    }
    if properties.pitch.is_some() {
        return Some("pitch has no recorded playback speed");
    }
    if properties.distance_cutoff.is_some()
        || properties.distances.is_some()
        || properties.cone_angles.is_some()
        || properties.cone_orientation.is_some()
        || properties.position.is_some()
        || properties.velocity.is_some()
    {
        return Some("distance or 3D property is not played");
    }
    None
}

fn parameter_block(objects: &ObjectHost, id: HandleId) -> Result<Option<String>, String> {
    match objects
        .sound_parameter_assignment(id)
        .map_err(|error| error.to_string())?
    {
        None => Ok(None),
        Some(assignment) => {
            let profile = assignment.source.profile;
            Ok(Some(format!("parameter profile {profile} is not played")))
        }
    }
}

fn load_bytes(objects: &ObjectHost, plan: &PlayPlan) -> Result<Vec<u8>, String> {
    let bytes = if plan.encoded_mp3 {
        let state = objects
            .file_mp3_state(plan.id)
            .map_err(|error| error.to_string())?;
        copy_exact(state.resource.bytes())?
    } else if let Ok(state) = objects.file_sound_state(plan.id) {
        decoder_bytes(state.resource)?
    } else {
        let state = objects
            .sound_state(plan.id)
            .map_err(|error| error.to_string())?;
        decoder_bytes(state.resource.resource())?
    };
    if bytes.len() > plan.byte_len {
        return Err("script audio decoder view exceeded the staged byte plan".to_string());
    }
    Ok(bytes)
}

fn commit_source(
    objects: &ObjectHost,
    assets: &mut Assets<AudioSource>,
    memory: &mut ScriptAudioMemory,
    session: &Weak<()>,
    prepared: Prepared,
) -> Result<ScriptVoice, String> {
    let Prepared { plan, bytes } = prepared;
    let (member, source_label, origin_source, origin_edition, origin_version) =
        receipt(objects, &plan)?;
    let source = if let Some(bytes) = bytes {
        let handle = assets.add(AudioSource {
            bytes: bytes.into(),
        });
        memory.insert_new(plan.sha256, plan.encoded_mp3, plan.byte_len, handle.clone());
        handle
    } else {
        memory.clone_existing(plan.key())?
    };
    Ok(ScriptVoice {
        session: session.clone(),
        handle: plan.id,
        looping: plan.looping,
        fade_in_rate: plan.fade_in_rate,
        fade_out_rate: plan.fade_out_rate,
        encoded_mp3: plan.encoded_mp3,
        source,
        member,
        source_label,
        origin_source,
        origin_edition,
        origin_version,
        sha256: plan.sha256,
    })
}

fn receipt(
    objects: &ObjectHost,
    plan: &PlayPlan,
) -> Result<(String, String, String, String, String), String> {
    if plan.encoded_mp3 {
        let state = objects
            .file_mp3_state(plan.id)
            .map_err(|error| error.to_string())?;
        text_receipt(
            state.resource.member(),
            state.resource.source_label(),
            state.resource.origin(),
        )
    } else if let Ok(state) = objects.file_sound_state(plan.id) {
        text_receipt(
            state.resource.member(),
            state.resource.source_label(),
            state.resource.origin(),
        )
    } else {
        let state = objects
            .sound_state(plan.id)
            .map_err(|error| error.to_string())?;
        text_receipt(
            state.resource.member(),
            state.resource.source_label(),
            state.resource.origin(),
        )
    }
}

fn text_receipt(
    member: &str,
    source_label: &str,
    origin: &AssetIdentity,
) -> Result<(String, String, String, String, String), String> {
    Ok((
        owned(member)?,
        owned(source_label)?,
        owned(&origin.source)?,
        owned(&origin.edition)?,
        owned(&origin.version)?,
    ))
}

fn decoder_bytes(resource: &PcmResource) -> Result<Vec<u8>, String> {
    DecoderWave::read(resource, resource.bytes().len())
        .map(|view| view.into_bytes())
        .map_err(|error| error.to_string())
}

fn copy_exact(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len())
        .map_err(|_| "script audio source copy allocation failed".to_string())?;
    out.extend_from_slice(bytes);
    Ok(out)
}

fn owned(text: &str) -> Result<String, String> {
    let mut out = String::new();
    out.try_reserve_exact(text.len())
        .map_err(|_| "script audio receipt allocation failed".to_string())?;
    out.push_str(text);
    Ok(out)
}

fn add_bytes(sum: usize, len: usize) -> Result<usize, String> {
    sum.checked_add(len)
        .ok_or_else(|| "script audio byte total overflowed".to_string())
}

impl ScriptAudioMemory {
    fn live_bytes(&self) -> Result<usize, String> {
        self.sources
            .iter()
            .try_fold(0usize, |sum, source| add_bytes(sum, source.bytes))
    }

    fn reusable(&self, plan: &PlayPlan) -> bool {
        self.sources
            .iter()
            .any(|source| source.sha256 == plan.sha256 && source.encoded_mp3 == plan.encoded_mp3)
    }

    fn insert_new(
        &mut self,
        sha256: [u8; 32],
        encoded_mp3: bool,
        bytes: usize,
        handle: Handle<AudioSource>,
    ) {
        self.sources.push(LiveSource {
            sha256,
            encoded_mp3,
            bytes,
            handle,
            voices: 1,
        });
    }

    fn clone_existing(&mut self, key: ([u8; 32], bool)) -> Result<Handle<AudioSource>, String> {
        let slot = self
            .sources
            .iter_mut()
            .find(|source| source.sha256 == key.0 && source.encoded_mp3 == key.1)
            .ok_or_else(|| "script audio source cache missed a planned reuse".to_string())?;
        slot.voices = slot
            .voices
            .checked_add(1)
            .ok_or_else(|| "script audio voice count overflowed".to_string())?;
        Ok(slot.handle.clone())
    }

    fn release(&mut self, assets: &mut Assets<AudioSource>, handle: &Handle<AudioSource>) {
        let Some(index) = self
            .sources
            .iter()
            .position(|source| &source.handle == handle)
        else {
            return;
        };
        if self.sources[index].voices > 1 {
            self.sources[index].voices -= 1;
            return;
        }
        let slot = self.sources.swap_remove(index);
        assets.remove(&slot.handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::audio::PlaybackMode;
    use bevy::ecs::system::RunSystemOnce;
    use pcx_play::file_sounds::{Assets as SoundAssets, Budget, Mp3Budget, SoundInput};
    use pcx_play::label_sounds::{Budget as LabelBudget, SoundBank, SoundInput as LabelInput};
    use pcx_play::playercolor_host::{EnumHost, Limits};
    use pcx_play::scripts::ScriptAsset;
    use pcx_play::sound_parameters::{Bank, Budget as ParameterBudget};
    use pcx_script::{Host, Prelude, Value};
    use pcx_slq::CoordinateLimits;
    use std::sync::Arc;

    const COMMON: &str = "\
type agent extends handle
type sound extends agent
type timer extends agent
type playercolor extends handle
constant native ConvertPlayerColor takes integer i returns playercolor
native CreateTimer takes nothing returns timer
native DestroyTimer takes timer t returns nothing
native CreateMIDISound takes string label, integer a, integer b returns sound
native CreateSound takes string file, boolean repeating, boolean spatial, boolean stop, integer a, integer b, string eax returns sound
native StartSound takes sound s returns nothing
native SetSoundDuration takes sound s, integer duration returns nothing
native SetSoundChannel takes sound s, integer channel returns nothing
native SetSoundVolume takes sound s, integer volume returns nothing
native SetSoundPitch takes sound s, real pitch returns nothing
native SetSoundDistanceCutoff takes sound s, real cutoff returns nothing
native SetSoundDistances takes sound s, real min, real max returns nothing
native SetSoundConeAngles takes sound s, real inside, real outside, integer outsideVolume returns nothing
native SetSoundConeOrientation takes sound s, real x, real y, real z returns nothing
native SetSoundPosition takes sound s, real x, real y, real z returns nothing
native SetSoundVelocity takes sound s, real x, real y, real z returns nothing
native SetSoundParamsFromLabel takes sound s, string label returns nothing
globals
constant playercolor C=ConvertPlayerColor(37)
endglobals
";

    #[derive(Resource)]
    struct Fixture {
        host: ObjectHost,
    }

    #[derive(Resource)]
    struct Witness(Weak<()>);

    #[derive(Component)]
    struct Bystander;

    fn origin() -> AssetIdentity {
        AssetIdentity {
            edition: "synthetic".into(),
            version: "test".into(),
            source: "synthetic:archive".into(),
        }
    }

    fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut bytes = tag.to_vec();
        bytes.extend(u32::try_from(body.len()).unwrap().to_le_bytes());
        bytes.extend(body);
        if !body.len().is_multiple_of(2) {
            bytes.push(0);
        }
        bytes
    }

    fn format_body(len: usize) -> Vec<u8> {
        let mut format = Vec::new();
        format.extend(1u16.to_le_bytes());
        format.extend(1u16.to_le_bytes());
        format.extend(8_000u32.to_le_bytes());
        format.extend(16_000u32.to_le_bytes());
        format.extend(2u16.to_le_bytes());
        format.extend(16u16.to_le_bytes());
        format.resize(len, 0x11);
        format
    }

    fn riff(chunks: &[&[u8]]) -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        let size = 4 + chunks.iter().map(|chunk| chunk.len()).sum::<usize>();
        bytes.extend(u32::try_from(size).unwrap().to_le_bytes());
        bytes.extend(b"WAVE");
        for chunk in chunks {
            bytes.extend_from_slice(chunk);
        }
        bytes
    }

    fn pcm(format_len: usize, leading_junk: bool) -> Vec<u8> {
        let format = chunk(b"fmt ", &format_body(format_len));
        let data = chunk(b"data", &[0x10, 0x20, 0x30, 0x40]);
        if leading_junk {
            let junk = chunk(b"JUNK", &[0x01, 0x02, 0x03]);
            riff(&[&junk, &data, &format])
        } else {
            riff(&[&format, &data])
        }
    }

    fn file_input<'a>(member: &'a str, label: &'a str, bytes: &'a [u8]) -> SoundInput<'a> {
        SoundInput {
            member,
            asset: ScriptAsset { label, bytes },
            origin: origin(),
        }
    }

    fn file_budget(bytes: usize, resources: usize) -> Budget {
        Budget {
            resources,
            resource_bytes: bytes,
            total_bytes: bytes.saturating_mul(resources),
            metadata_bytes: 512,
        }
    }

    fn host(assets: SoundAssets) -> ObjectHost {
        let common = pcx_script::parse_script(COMMON).unwrap();
        let map = pcx_script::parse_script("").unwrap();
        let program = Arc::new(
            pcx_script::resolve_with(&map, &[Prelude::new("synthetic:common", &common)]).unwrap(),
        );
        ObjectHost::checked(
            EnumHost::checked_selected(
                program,
                "synthetic:files",
                &["ConvertPlayerColor"],
                Limits {
                    max_handles: 8,
                    admission_steps: 64,
                    expression_depth: 8,
                    expression_work: 64,
                },
            )
            .unwrap(),
        )
        .unwrap()
        .with_file_sounds(assets, 64)
        .unwrap()
        .with_sound_starts(4)
        .unwrap()
        .with_sound_duration()
        .unwrap()
        .with_sound_properties()
        .unwrap()
    }

    fn create(
        member: &str,
        looping: bool,
        spatial: bool,
        stop: bool,
        fade_in: i32,
        fade_out: i32,
        eax: &str,
    ) -> Vec<Value> {
        vec![
            Value::Str(member.into()),
            Value::Bool(looping),
            Value::Bool(spatial),
            Value::Bool(stop),
            Value::Int(fade_in),
            Value::Int(fade_out),
            Value::Str(eax.into()),
        ]
    }

    fn start(host: &mut ObjectHost, args: &[Value]) -> HandleId {
        let value = host.native("CreateSound", args).unwrap();
        let Value::Handle(id) = value.clone() else {
            panic!("sound");
        };
        host.native("StartSound", &[value]).unwrap();
        id
    }

    fn world_for(host: ObjectHost, limits: ScriptAudioLimits) -> World {
        let mut world = World::new();
        world.init_resource::<Assets<AudioSource>>();
        world.init_resource::<ScriptAudioReport>();
        world.init_resource::<ScriptAudioMemory>();
        world.insert_resource(limits);
        world.insert_resource(Witness(Arc::downgrade(&Arc::new(()))));
        world.insert_resource(Fixture { host });
        world
    }

    fn publish(world: &mut World) -> Result<(), String> {
        world
            .run_system_once(
                |mut fixture: ResMut<Fixture>,
                 witness: Res<Witness>,
                 mut commands: Commands,
                 mut assets: ResMut<Assets<AudioSource>>,
                 mut report: ResMut<ScriptAudioReport>,
                 mut memory: ResMut<ScriptAudioMemory>,
                 limits: Res<ScriptAudioLimits>| {
                    publish_queued_starts(
                        &mut fixture.host,
                        &witness.0,
                        *limits,
                        &mut memory,
                        &mut commands,
                        &mut assets,
                        &mut report,
                    )
                },
            )
            .unwrap()
    }

    fn count<T: bevy::prelude::Component>(world: &mut World) -> usize {
        let mut query = world.query::<&T>();
        query.iter(world).count()
    }

    fn limits_for(bytes: usize) -> ScriptAudioLimits {
        ScriptAudioLimits {
            max_staged_bytes: bytes,
            max_live_bytes: bytes,
        }
    }

    #[test]
    fn oneshot_completion_ignores_looping_and_unfinished_players() {
        assert!(!oneshot_completed(true, false, Some(true)));
        assert!(!oneshot_completed(false, true, Some(false)));
        assert!(!oneshot_completed(false, true, None));
        assert!(oneshot_completed(false, true, Some(true)));
        assert!(oneshot_completed(false, false, None));
    }

    #[test]
    fn one_pcm_start_keeps_the_decoder_view_and_does_not_replay() {
        let bytes = pcm(16, true);
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets);
        let id = start(
            &mut object_host,
            &create(r"Media\a.wav", true, false, false, 0, 0, ""),
        );
        let source = object_host.file_sound_state(id).unwrap();
        let expected = DecoderWave::read(source.resource, source.resource.bytes().len())
            .unwrap()
            .into_bytes();
        let digest = source.resource.sha256();
        assert_ne!(expected, bytes);
        let mut world = world_for(object_host, limits_for(expected.len()));
        publish(&mut world).unwrap();
        let mut voices = world.query::<(&AudioPlayer, &PlaybackSettings, &ScriptVoice)>();
        let observed: Vec<_> = voices
            .iter(&world)
            .map(|(player, playback, voice)| (player.0.clone(), *playback, voice.clone()))
            .collect();
        assert_eq!(observed.len(), 1);
        let (audio_handle, playback, voice) = &observed[0];
        let audio = world
            .resource::<Assets<AudioSource>>()
            .get(audio_handle.id())
            .unwrap();
        assert_eq!(audio.bytes.as_ref(), expected.as_slice());
        assert!(matches!(playback.mode, PlaybackMode::Loop));
        assert!(!playback.spatial);
        assert!(playback.duration.is_none());
        assert!(voice.looping);
        assert_eq!((voice.fade_in_rate, voice.fade_out_rate), (0, 0));
        assert!(!voice.encoded_mp3);
        assert_eq!(voice.member, r"Media\a.wav");
        assert_eq!(voice.sha256, digest);
        assert_eq!(world.resource::<ScriptAudioReport>().played, 1);
        publish(&mut world).unwrap();
        assert_eq!(count::<ScriptVoice>(&mut world), 1);
        assert_eq!(world.resource::<ScriptAudioReport>().played, 1);
        assert!(
            world
                .resource::<Fixture>()
                .host
                .pending_sound_starts()
                .is_empty()
        );
    }

    #[test]
    fn distinct_handles_share_one_source_inside_the_caller_limit() {
        let bytes = pcm(16, false);
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets);
        start(
            &mut object_host,
            &create(r"Media\a.wav", false, false, false, 0, 0, ""),
        );
        start(
            &mut object_host,
            &create(r"Media\a.wav", false, false, false, 0, 0, ""),
        );
        let mut world = world_for(object_host, limits_for(bytes.len()));
        publish(&mut world).unwrap();
        let mut voices = world.query::<&ScriptVoice>();
        let handles: Vec<_> = voices
            .iter(&world)
            .map(|voice| voice.source.clone())
            .collect();
        assert_eq!(handles.len(), 2);
        assert_eq!(handles[0], handles[1]);
        assert!(!voices.iter(&world).any(|voice| voice.looping));
        assert_eq!(world.resource::<Assets<AudioSource>>().iter().count(), 1);
        let mode = {
            let mut playback = world.query::<&PlaybackSettings>();
            playback.iter(&world).next().unwrap().mode
        };
        assert!(matches!(mode, PlaybackMode::Remove));
    }

    #[test]
    fn staged_limit_does_not_copy_or_drain_a_playable_start() {
        let bytes = pcm(16, false);
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets);
        let id = start(
            &mut object_host,
            &create(r"Media\a.wav", false, false, false, 0, 0, ""),
        );
        let mut world = world_for(
            object_host,
            ScriptAudioLimits {
                max_staged_bytes: bytes.len() - 1,
                max_live_bytes: bytes.len(),
            },
        );
        let error = publish(&mut world).unwrap_err();
        assert!(error.contains("staging limit"), "{error}");
        assert_eq!(count::<ScriptVoice>(&mut world), 0);
        assert_eq!(world.resource::<Assets<AudioSource>>().iter().count(), 0);
        assert_eq!(
            world.resource::<Fixture>().host.pending_sound_starts(),
            &[id]
        );
    }

    #[test]
    fn unsupported_profile_does_not_need_staging_budget() {
        let bytes = pcm(16, false);
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets);
        start(
            &mut object_host,
            &create(r"Media\a.wav", false, true, false, 0, 0, ""),
        );
        let mut world = world_for(
            object_host,
            ScriptAudioLimits {
                max_staged_bytes: 0,
                max_live_bytes: 0,
            },
        );
        publish(&mut world).unwrap();
        assert_eq!(count::<ScriptVoice>(&mut world), 0);
        assert_eq!(world.resource::<Assets<AudioSource>>().iter().count(), 0);
        assert!(
            world
                .resource::<Fixture>()
                .host
                .pending_sound_starts()
                .is_empty()
        );
        assert!(
            world
                .resource::<ScriptAudioReport>()
                .last_refusal
                .as_deref()
                .is_some_and(|text| text.contains("3D placement"))
        );
    }

    #[test]
    fn failed_pcm_admission_does_not_drain_or_publish_any_start() {
        let good = pcm(16, false);
        let bad = pcm(20, false);
        let assets = SoundAssets::read(
            &[
                file_input(r"Media\good.wav", "synthetic:good", &good),
                file_input(r"Media\bad.wav", "synthetic:bad", &bad),
            ],
            file_budget(good.len().max(bad.len()), 2),
        )
        .unwrap();
        let mut object_host = host(assets);
        let good_id = start(
            &mut object_host,
            &create(r"Media\good.wav", false, false, false, 0, 0, ""),
        );
        let bad_id = start(
            &mut object_host,
            &create(r"Media\bad.wav", false, false, false, 0, 0, ""),
        );
        let mut world = world_for(object_host, limits_for(good.len() + bad.len()));
        let error = publish(&mut world).unwrap_err();
        assert!(error.contains("synthetic:bad"), "{error}");
        assert_eq!(count::<ScriptVoice>(&mut world), 0);
        assert_eq!(world.resource::<Assets<AudioSource>>().iter().count(), 0);
        assert_eq!(
            world.resource::<Fixture>().host.pending_sound_starts(),
            &[good_id, bad_id]
        );
    }

    #[test]
    fn removing_a_finished_player_reclaims_only_the_oneshot() {
        let bytes = pcm(16, false);
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets);
        start(
            &mut object_host,
            &create(r"Media\a.wav", false, false, false, 0, 0, ""),
        );
        start(
            &mut object_host,
            &create(r"Media\a.wav", true, false, false, 0, 0, ""),
        );
        let mut world = world_for(object_host, limits_for(bytes.len()));
        world.spawn(Bystander);
        publish(&mut world).unwrap();
        let oneshot = {
            let mut voices = world.query::<(Entity, &ScriptVoice)>();
            voices
                .iter(&world)
                .find(|(_, voice)| !voice.looping)
                .map(|(entity, _)| entity)
                .unwrap()
        };
        world.entity_mut(oneshot).remove::<AudioPlayer>();
        let witness = world.resource::<Witness>().0.clone();
        world
            .run_system_once(
                move |mut commands: Commands,
                      mut assets: ResMut<Assets<AudioSource>>,
                      mut memory: ResMut<ScriptAudioMemory>,
                      voices: Query<(
                    Entity,
                    &ScriptVoice,
                    Option<&AudioSink>,
                    Has<AudioPlayer>,
                )>| {
                    reclaim_script_voices(
                        &mut commands,
                        &mut assets,
                        &mut memory,
                        &voices,
                        Some(&witness),
                    );
                },
            )
            .unwrap();
        assert_eq!(count::<Bystander>(&mut world), 1);
        let looping: Vec<_> = {
            let mut voices = world.query::<&ScriptVoice>();
            voices.iter(&world).map(|voice| voice.looping).collect()
        };
        assert_eq!(looping, vec![true]);
        assert_eq!(world.resource::<Assets<AudioSource>>().iter().count(), 1);
    }

    #[test]
    fn one_mp3_start_keeps_encoded_bytes_and_the_source_receipt() {
        let encoded = include_bytes!("../../pcx-play/tests/fixtures/synthetic-mp3/mono-xing.mp3");
        let assets = SoundAssets::read_encoded(
            &[file_input(r"Media\tone.bin", "synthetic:mp3", encoded)],
            file_budget(encoded.len(), 1),
            Mp3Budget {
                max_input_allocation_bytes: encoded.len() + pcx_play::mp3_frame::DECODER_RING_BYTES,
                max_decoder_steps: 512,
                max_packets: 128,
                max_frames: 200_000,
                max_samples: 400_000,
            },
        )
        .unwrap();
        let mut object_host = host(assets);
        let id = start(
            &mut object_host,
            &create(r"Media\tone.bin", false, false, false, 0, 0, ""),
        );
        let digest = object_host.file_mp3_state(id).unwrap().resource.sha256();
        assert_eq!(object_host.sound_resource_bytes().unwrap(), encoded.len());
        let mut world = world_for(object_host, limits_for(encoded.len()));
        publish(&mut world).unwrap();
        let mut voices = world.query::<(&AudioPlayer, &ScriptVoice)>();
        let observed: Vec<_> = voices
            .iter(&world)
            .map(|(player, voice)| (player.0.clone(), voice.clone()))
            .collect();
        let (audio_handle, voice) = &observed[0];
        let audio = world
            .resource::<Assets<AudioSource>>()
            .get(audio_handle.id())
            .unwrap();
        assert_eq!(audio.bytes.as_ref(), encoded);
        assert!(voice.encoded_mp3);
        assert_eq!(voice.sha256, digest);
        assert_eq!(voice.source_label, "synthetic:mp3");
        use bevy::audio::Decodable;
        let (count, audible) = audio
            .decoder()
            .take(4096)
            .fold((0, false), |(count, audible), sample| {
                (count + 1, audible || sample != 0.0)
            });
        assert_eq!(count, 4096);
        assert!(audible);
    }

    #[test]
    fn parameter_assignment_and_midi_are_refused_without_a_player() {
        let bytes = pcm(16, false);
        let table = b"\
ID;PWXL;N;E\r\n\
C;X1;Y1;K\"Selector\"\r\n\
C;X2;Y1;K\"Volume\"\r\n\
C;X3;Y1;K\"Pitch\"\r\n\
C;X4;Y1;K\"PitchVariance\"\r\n\
C;X5;Y1;K\"Priority\"\r\n\
C;X6;Y1;K\"Channel\"\r\n\
C;X7;Y1;K\"MinDistance\"\r\n\
C;X8;Y1;K\"MaxDistance\"\r\n\
C;X9;Y1;K\"DistanceCutoff\"\r\n\
C;X10;Y1;K\"Flags\"\r\n\
C;X11;Y1;K\"EAXFlags\"\r\n\
C;X1;Y2;K\"Voice\"\r\n\
C;X2;Y2;K\"4\"\r\n\
C;X3;Y2;K\"1.5\"\r\n\
C;X4;Y2;K\"0\"\r\n\
C;X5;Y2;K\"1\"\r\n\
C;X6;Y2;K\"0\"\r\n\
C;X7;Y2;K\"0\"\r\n\
C;X8;Y2;K\"100\"\r\n\
C;X9;Y2;K\"100\"\r\n\
C;X10;Y2;K\"ROW\"\r\n\
C;X11;Y2;K\"None\"\r\n\
E\r\n";
        let bank = Bank::read(
            ScriptAsset {
                label: "synthetic:parameters",
                bytes: table,
            },
            origin(),
            "Selector",
            &["Voice"],
            ParameterBudget {
                table_bytes: table.len(),
                coordinates: CoordinateLimits {
                    max_row: 8,
                    max_column: 16,
                },
                definitions: 1,
                metadata_bytes: 256,
            },
        )
        .unwrap();
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets).with_sound_parameters(bank).unwrap();
        let value = object_host
            .native(
                "CreateSound",
                &create(r"Media\a.wav", false, false, false, 0, 0, ""),
            )
            .unwrap();
        object_host
            .native(
                "SetSoundParamsFromLabel",
                &[value.clone(), Value::Str("Voice".into())],
            )
            .unwrap();
        object_host.native("StartSound", &[value]).unwrap();
        let mut world = world_for(object_host, limits_for(0));
        publish(&mut world).unwrap();
        assert_eq!(count::<ScriptVoice>(&mut world), 0);
        assert!(
            world
                .resource::<ScriptAudioReport>()
                .last_refusal
                .as_deref()
                .is_some_and(|text| text.contains("parameter profile"))
        );

        let label = "\
ID;PWXL;N;E\r\n\
C;X1;Y1;K\"label\"\r\n\
C;X2;Y1;K\"DirectoryBase\"\r\n\
C;X3;Y1;K\"FileNames\"\r\n\
C;X1;Y2;K\"Tone\"\r\n\
C;X2;Y2;K\"Media\\\"\r\n\
C;X3;Y2;K\"a.wav\"\r\n\
E\r\n";
        let bank = SoundBank::read(
            ScriptAsset {
                label: "synthetic:labels",
                bytes: label.as_bytes(),
            },
            origin(),
            "label",
            &[LabelInput {
                label: "Tone",
                member: r"Media\a.wav",
                asset: ScriptAsset {
                    label: "synthetic:pcm",
                    bytes: &bytes,
                },
                origin: origin(),
            }],
            LabelBudget {
                table_bytes: label.len(),
                coordinates: CoordinateLimits {
                    max_row: 8,
                    max_column: 8,
                },
                sounds: 1,
                resource_bytes: bytes.len(),
                total_resource_bytes: bytes.len(),
                metadata_bytes: 256,
            },
        )
        .unwrap();
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets).with_label_sounds(bank).unwrap();
        assert_eq!(object_host.sound_resource_bytes().unwrap(), bytes.len() * 2);
        let value = object_host
            .native(
                "CreateMIDISound",
                &[Value::Str("Tone".into()), Value::Int(0), Value::Int(0)],
            )
            .unwrap();
        object_host.native("StartSound", &[value]).unwrap();
        let mut world = world_for(object_host, limits_for(0));
        publish(&mut world).unwrap();
        assert_eq!(count::<ScriptVoice>(&mut world), 0);
        assert!(
            world
                .resource::<ScriptAudioReport>()
                .last_refusal
                .as_deref()
                .is_some_and(|text| text.contains("CreateMIDISound"))
        );
    }

    #[test]
    fn empty_published_session_retires_script_voices_and_leaves_other_entities() {
        let mut world = World::new();
        world.init_resource::<Assets<AudioSource>>();
        world.init_resource::<ScriptAudioReport>();
        world.init_resource::<ScriptAudioMemory>();
        world.init_resource::<MapScriptSession<MapStartupHost>>();
        let source = world
            .resource_mut::<Assets<AudioSource>>()
            .add(AudioSource {
                bytes: Vec::<u8>::new().into(),
            });
        world.spawn((
            Bystander,
            ScriptVoice {
                session: Weak::new(),
                handle: HandleId::new(std::num::NonZeroU64::new(1).unwrap()),
                looping: false,
                fade_in_rate: 0,
                fade_out_rate: 0,
                encoded_mp3: false,
                source,
                member: String::new(),
                source_label: String::new(),
                origin_source: String::new(),
                origin_edition: String::new(),
                origin_version: String::new(),
                sha256: [0; 32],
            },
        ));
        world.spawn(Bystander);
        world.run_system_once(play_published_script_sounds).unwrap();
        assert_eq!(count::<ScriptVoice>(&mut world), 0);
        assert_eq!(count::<Bystander>(&mut world), 1);
    }
    #[test]
    fn canonical_pcm_uses_the_installed_decoder_without_a_device() {
        use bevy::audio::{Decodable, Source};
        let bytes = pcm(16, true);
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets);
        start(
            &mut object_host,
            &create(r"Media\a.wav", false, false, false, 0, 0, ""),
        );
        let mut world = world_for(object_host, limits_for(bytes.len()));
        publish(&mut world).unwrap();
        let source = world
            .resource::<Assets<AudioSource>>()
            .iter()
            .next()
            .unwrap()
            .1;
        let mut decoder = source.decoder();
        assert_eq!(decoder.channels().get(), 1);
        assert_eq!(decoder.sample_rate().get(), 8_000);
        let expected = [
            i16::from_le_bytes([0x10, 0x20]),
            i16::from_le_bytes([0x30, 0x40]),
        ];
        for sample in expected {
            assert_eq!(decoder.next(), Some(f32::from(sample) / 32_768.0));
        }
        assert_eq!(decoder.next(), None);
    }
    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen::prelude::wasm_bindgen]
    pub fn browser_script_audio_decoder_gate() -> u32 {
        oneshot_completion_ignores_looping_and_unfinished_players();
        one_pcm_start_keeps_the_decoder_view_and_does_not_replay();
        distinct_handles_share_one_source_inside_the_caller_limit();
        staged_limit_does_not_copy_or_drain_a_playable_start();
        unsupported_profile_does_not_need_staging_budget();
        failed_pcm_admission_does_not_drain_or_publish_any_start();
        removing_a_finished_player_reclaims_only_the_oneshot();
        one_mp3_start_keeps_encoded_bytes_and_the_source_receipt();
        parameter_assignment_and_midi_are_refused_without_a_player();
        empty_published_session_retires_script_voices_and_leaves_other_entities();
        canonical_pcm_uses_the_installed_decoder_without_a_device();
        11
    }
}
