//! Play published `ObjectHost` `StartSound` requests through Bevy audio.
//! `MapScriptSession` exposes only a successfully published host, so a failed
//! `replace` cannot drain its candidate queue.
//! The queue stores sound identities, not setting snapshots. Each consumed
//! start samples that identity's current creation inputs, properties and
//! parameter assignment.
//! Non-spatial dialogue and explicit gain/pitch use source-backed volume and
//! audio-update timing when supplied. 3D, EAX, distance and unhandled parameter
//! profiles remain refused; channel categories follow source masks and real-time transitions.
//! Native duration stays metadata and never trims a waveform. `PlaybackSettings::LOOP` and
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
use std::{collections::HashMap, sync::Weak};
use tracing::warn;
#[path = "script_audio_mix.rs"]
mod mix;
use pcx_play::map_startup_host::sound_playback::VoicePolicy;

/// Per-voice source gain, including the source-timed fade. User gain multiplies it.
#[derive(Component)]
pub struct ScriptAudioGain(pub f32);
#[derive(Component)]
struct AuthoredGain(f32);
/// Category gain clamps at the channel bus before multiplying the individual voice.
#[derive(Component)]
pub struct ScriptAudioCategory {
    base: f32,
    scale: f32,
    unity: Option<f32>,
}
impl ScriptAudioCategory {
    pub fn with_user_gain(&self, authored: f32, user: f32) -> f32 {
        match self.unity {
            Some(unity) if self.base > 0.0 => {
                authored / self.base * (self.base * user * self.scale).clamp(0.0, unity)
            }
            Some(_) => 0.0,
            None => authored * user,
        }
    }
}

fn apply_category_gains(
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut voices: Query<(
        &AuthoredGain,
        Option<&FadeIn>,
        &ScriptAudioMetadata,
        &mut ScriptAudioGain,
        &mut ScriptAudioCategory,
    )>,
    mut report: ResMut<ScriptAudioReport>,
) {
    let Some(host) = session.host() else { return };
    for (base, fade, metadata, mut gain, mut category_gain) in &mut voices {
        let authored = fade.map_or(base.0, FadeIn::sample);
        let category = metadata.channel.map_or(Ok(1.0), |channel| {
            host.objects().volume_group_channel_gain(channel)
        });
        match category {
            Ok(scale) => {
                gain.0 = authored;
                category_gain.scale = scale;
                category_gain.unity = host
                    .sound_playback()
                    .map(|rules| rules.group_policy().unity());
            }
            Err(error) => {
                gain.0 = 0.0;
                if report.last_refusal.as_ref() != Some(&error) {
                    remember(&mut report, &error);
                }
            }
        }
    }
}

/// Native duration is query metadata; it does not trim the encoded resource.
#[derive(Component)]
pub struct ScriptAudioMetadata {
    pub duration: Option<i32>,
    pub channel: Option<i32>,
}

#[derive(Clone)]
struct PlaybackProfile {
    maximum: f32,
    update_seconds: f32,
    minimum_fade_step: u32,
    mix: Option<std::sync::Arc<VoicePolicy>>,
}

#[derive(Component)]
struct FadeIn {
    elapsed: f64,
    period: f64,
    increment: f64,
    target: f32,
}

fn advance_fades(
    time: Res<Time<Real>>,
    mut voices: Query<(&mut FadeIn, &mut ScriptAudioGain, &AudioSink)>,
) {
    for (mut fade, mut gain, sink) in &mut voices {
        gain.0 = fade.advance(time.delta_secs_f64(), sink.is_paused());
    }
}

impl FadeIn {
    fn advance(&mut self, delta: f64, paused: bool) -> f32 {
        if !paused {
            self.elapsed += delta;
        }
        self.sample()
    }
    fn sample(&self) -> f32 {
        ((self.elapsed / self.period).floor() * self.increment).min(f64::from(self.target)) as f32
    }
}

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
    pub last_partial: Option<String>,
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
    /// One live voice per sound identity in the current session.
    playing: HashMap<HandleId, LiveVoice>,
    profile: Option<(Weak<()>, Result<PlaybackProfile, String>)>,
    mixed: Vec<mix::Active>,
    next_serial: u64,
}

struct LiveVoice {
    entity: Entity,
    source: Handle<AudioSource>,
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

type ScriptVoices<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static ScriptVoice,
        Option<&'static AudioSink>,
        Has<AudioPlayer>,
        Option<&'static mix::Voice>,
    ),
>;

pub fn install(app: &mut App) {
    app.init_resource::<ScriptAudioReport>()
        .init_resource::<Time<Real>>()
        .init_resource::<ScriptAudioMemory>()
        .init_resource::<Assets<AudioSource>>()
        .init_resource::<MapScriptSession<MapStartupHost>>()
        .add_systems(
            Update,
            play_published_script_sounds
                .in_set(ScriptAudioPlayback)
                .after(crate::campaign_startup::CampaignStartupSet),
        )
        .add_systems(
            Update,
            advance_fades
                .in_set(ScriptAudioPlayback)
                .after(play_published_script_sounds),
        )
        .add_systems(
            Update,
            apply_category_gains
                .in_set(ScriptAudioPlayback)
                .after(advance_fades),
        );
}

fn play_published_script_sounds(
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut commands: Commands,
    mut assets: ResMut<Assets<AudioSource>>,
    mut report: ResMut<ScriptAudioReport>,
    mut memory: ResMut<ScriptAudioMemory>,
    limits: Option<Res<ScriptAudioLimits>>,
    voices: ScriptVoices,
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
        memory.profile = None;
        return;
    };
    if !memory
        .profile
        .as_ref()
        .is_some_and(|(previous, _)| Weak::ptr_eq(previous, &witness))
    {
        let profile = (|| -> Result<PlaybackProfile, String> {
            let context = session
                .context()
                .cloned()
                .ok_or("published script context missing")?;
            let helper = context
                .scripts
                .program()
                .function("SetSoundVolumeBJ")
                .ok_or_else(|| {
                    format!(
                        "{:?}: sound-volume helper missing",
                        context.sources.blizzard
                    )
                })?;
            let source = context.scripts.source(&helper.source).ok_or_else(|| {
                format!("{}: retained sound-volume source missing", helper.source)
            })?;
            let maximum = crate::sound_volume::VolumeScale::checked(source.as_bytes())
                .map_err(|e| format!("{}: {e}", helper.source))?
                .maximum;
            let rules = session
                .host()
                .and_then(|host| host.sound_playback())
                .ok_or_else(|| format!("{:?}: source audio timing missing", context.sources.map))?;
            Ok(PlaybackProfile {
                maximum,
                update_seconds: rules.update_seconds(),
                minimum_fade_step: rules.minimum_fade_step(),
                mix: Some(rules.voice_policy()),
            })
        })();
        memory.profile = Some((witness.clone(), profile));
    }
    let profile_error = memory
        .profile
        .as_ref()
        .and_then(|(_, value)| value.as_ref().err())
        .cloned();
    let Some(host) = session.host() else {
        return;
    };
    let objects = host.objects_mut();
    // Stops run first: the host already dropped any unconsumed start of a stopped identity.
    stop_sounds(
        &objects.take_sound_stops(),
        &mut commands,
        &mut assets,
        &mut memory,
        &mut report,
    );
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
    let previous_refusals = report.refusal_count;
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
    if report.refusal_count != previous_refusals
        && let Some(error) = profile_error
        && let Some(reason) = &mut report.last_refusal
    {
        reason.push_str(&format!("; playback profile unavailable: {error}"));
        warn!("{reason}");
    }
}

fn reclaim_script_voices(
    commands: &mut Commands,
    assets: &mut Assets<AudioSource>,
    memory: &mut ScriptAudioMemory,
    voices: &ScriptVoices,
    current: Option<&Weak<()>>,
) {
    let mut finished = Vec::new();
    memory.playing.clear();
    memory.mixed.clear();
    for (entity, voice, sink, has_player, mixed) in voices.iter() {
        let current_voice = current.is_some_and(|session| Weak::ptr_eq(session, &voice.session));
        let sink_empty = sink.map(AudioSinkPlayback::empty);
        if !current_voice || oneshot_completed(voice.looping, has_player, sink_empty) {
            finished.push((entity, voice.source.clone()));
        } else {
            memory.playing.insert(
                voice.handle,
                LiveVoice {
                    entity,
                    source: voice.source.clone(),
                },
            );
            if let Some(mixed) = mixed {
                memory.mixed.push(mix::Active {
                    voice: mixed.clone(),
                    entity,
                    source: voice.source.clone(),
                });
            }
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

#[derive(Clone)]
struct PlayPlan {
    id: HandleId,
    encoded_mp3: bool,
    looping: bool,
    fade_in_rate: i32,
    fade_out_rate: i32,
    sha256: [u8; 32],
    byte_len: usize,
    gain: Option<f32>,
    speed: Option<f32>,
    duration: Option<i32>,
    profile: Option<PlaybackProfile>,
    channel: Option<i32>,
    mix: Option<mix::Voice>,
    victim: Option<HandleId>,
    channel_gain: f32,
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
    member: &'a str,
}

struct Prepared {
    plan: PlayPlan,
    bytes: Option<Vec<u8>>,
}

/// Silences each stopped identity's live voice, looping or not. Native
/// `StopSound(kill=true)` releases the backend voice after any fade call, so
/// kill stops at once. A fade without kill starts the native fade-out ramp,
/// which this consumer does not apply; the voice stops at once and the report says so.
fn stop_sounds(
    stops: &[(HandleId, bool, bool)],
    commands: &mut Commands,
    assets: &mut Assets<AudioSource>,
    memory: &mut ScriptAudioMemory,
    report: &mut ScriptAudioReport,
) {
    for &(id, kill, fade) in stops {
        if let Some(live) = memory.playing.remove(&id) {
            memory.mixed.retain(|active| active.entity != live.entity);
            commands.entity(live.entity).despawn();
            memory.release(assets, &live.source);
        }
        if fade && !kill {
            let token = id.token();
            report.last_partial = Some(format!(
                "sound {token}: StopSound fade-out ramp not applied; stopped immediately"
            ));
        }
    }
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
    let profile = memory
        .profile
        .as_ref()
        .filter(|(witness, _)| Weak::ptr_eq(witness, session))
        .and_then(|(_, value)| value.as_ref().ok())
        .cloned();
    let mut ids = Vec::new();
    ids.try_reserve_exact(objects.pending_sound_starts().len())
        .map_err(|_| "script audio start list allocation failed".to_string())?;
    ids.extend_from_slice(objects.pending_sound_starts());
    let mut classified = Vec::new();
    classified
        .try_reserve_exact(ids.len())
        .map_err(|_| "script audio start list allocation failed".to_string())?;
    let mut simulated: Vec<_> = memory
        .mixed
        .iter()
        .map(|active| active.voice.clone())
        .collect();
    let mut serial = memory.next_serial;
    for id in ids {
        // A repeat replaces its identity's voice, so that voice never counts against capacity.
        let mut classified_sound = classify(objects, id, profile.clone())?;
        if let Classified::Play(plan) = &mut classified_sound
            && let Some(voice) = &mut plan.mix
            && let Some(policy) = profile.as_ref().and_then(|profile| profile.mix.as_ref())
        {
            voice.serial = serial;
            serial = serial
                .checked_add(1)
                .ok_or("script audio start serial overflowed")?;
            match mix::admission(
                simulated.iter().filter(|other| other.id != id),
                voice,
                policy,
            ) {
                Ok(victim) => {
                    plan.victim = victim;
                    simulated.retain(|other| other.id != id);
                    if let Some(victim) = victim {
                        simulated.retain(|other| other.id != victim);
                    }
                    simulated.push(voice.clone());
                }
                Err(reason) => classified_sound = Classified::Refuse(id, reason),
            }
        }
        classified.push(classified_sound);
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
            Ok(prepared) => {
                let plan = prepared.plan.clone();
                match commit_source(objects, assets, memory, session, prepared) {
                    Ok(voice) => {
                        committed.push(voice.source.clone());
                        ready.push(Ok((voice, plan)));
                    }
                    Err(error) => {
                        for handle in committed {
                            memory.release(assets, &handle);
                        }
                        return Err(error);
                    }
                }
            }
        }
    }
    memory.next_serial = serial;
    objects.take_sound_starts();
    for item in ready {
        match item {
            Err((handle, reason)) => {
                let token = handle.token();
                warn!("script sound {token} refused: {reason}");
                report.last_refusal = Some(format!("sound {token}: {reason}"));
                report.refusal_count = report.refusal_count.saturating_add(1);
            }
            Ok((voice, plan)) => {
                // A repeat StartSound restarts its identity: the earlier voice stops here.
                if let Some(old) = memory.playing.remove(&voice.handle) {
                    memory.mixed.retain(|active| active.entity != old.entity);
                    commands.entity(old.entity).despawn();
                    memory.release(assets, &old.source);
                }
                if let Some(victim) = plan.victim
                    && let Some(at) = memory
                        .mixed
                        .iter()
                        .position(|active| active.voice.id == victim)
                {
                    let old = memory.mixed.swap_remove(at);
                    commands.entity(old.entity).despawn();
                    memory.release(assets, &old.source);
                    memory.playing.remove(&victim);
                }
                let mut playback = if voice.looping {
                    PlaybackSettings::LOOP
                } else {
                    PlaybackSettings::REMOVE
                };
                let source = voice.source.clone();
                if let Some(speed) = plan.speed {
                    playback.speed = speed;
                }
                let handle = voice.handle;
                let rate = voice.fade_in_rate;
                let mut entity = commands.spawn((
                    AudioPlayer(source.clone()),
                    playback,
                    voice,
                    ScriptAudioMetadata {
                        duration: plan.duration,
                        channel: plan.channel,
                    },
                ));
                memory.playing.insert(
                    handle,
                    LiveVoice {
                        entity: entity.id(),
                        source: source.clone(),
                    },
                );
                if let Some(mixed) = plan.mix {
                    entity.insert(mixed.clone());
                    memory.mixed.push(mix::Active {
                        voice: mixed,
                        entity: entity.id(),
                        source,
                    });
                }
                if let Some(gain) = plan.gain {
                    entity.insert((
                        ScriptAudioGain(gain),
                        AuthoredGain(gain),
                        ScriptAudioCategory {
                            base: plan.channel_gain,
                            scale: 1.0,
                            unity: None,
                        },
                    ));
                    if let Some(profile) = plan.profile {
                        let step = (rate as u32).max(profile.minimum_fade_step);
                        entity.insert((
                            ScriptAudioGain(0.0),
                            FadeIn {
                                elapsed: 0.0,
                                period: f64::from(profile.update_seconds),
                                increment: f64::from(step) / f64::from(profile.maximum)
                                    * f64::from(plan.channel_gain),
                                target: gain,
                            },
                        ));
                    }
                }
                report.played = report.played.saturating_add(1);
            }
        }
    }
    report.admission_error = None;
    Ok(())
}

fn classify(
    objects: &ObjectHost,
    id: HandleId,
    profile: Option<PlaybackProfile>,
) -> Result<Classified, String> {
    if let Ok(state) = objects.file_mp3_state(id) {
        return plan_file(
            objects,
            id,
            profile,
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
                member: state.resource.member(),
            },
        );
    }
    if let Ok(state) = objects.file_sound_state(id) {
        return plan_file(
            objects,
            id,
            profile,
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
                member: state.resource.member(),
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
        profile,
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
            member: state.resource.resource().member(),
        },
    )
}

fn plan_file(
    objects: &ObjectHost,
    id: HandleId,
    profile: Option<PlaybackProfile>,
    observed: Observed<'_>,
) -> Result<Classified, String> {
    if let Some(reason) = creation_block(
        observed.is_3d,
        observed.stop_when_out_of_range,
        observed.eax,
        if profile.is_some() {
            0
        } else {
            observed.fade_in_rate
        },
        if profile.is_some() {
            0
        } else {
            observed.fade_out_rate
        },
        if profile.is_some() {
            None
        } else {
            observed.duration
        },
    ) {
        return Ok(Classified::Refuse(id, reason.to_string()));
    }
    let properties = *objects
        .sound_properties(id)
        .map_err(|error| error.to_string())?;
    let mut supported = properties;
    let mut gain = None;
    let mut speed = None;
    let mut duration = None;
    let mut channel = None;
    let mut mixed = None;
    let mut channel_gain = 1.0;
    if let Some(profile) = profile.as_ref() {
        if observed.fade_in_rate < 0 || observed.fade_out_rate < 0 {
            return Ok(Classified::Refuse(
                id,
                "negative fade rates are not played".into(),
            ));
        }
        let assignment = objects
            .sound_parameter_assignment(id)
            .map_err(|e| e.to_string())?;
        let volume = match assignment {
            Some(assignment) => match assignment.parameters {
                pcx_play::sound_parameters::ParameterValues::Dialogue(parameters) => {
                    let mut preempt_channel = false;
                    let mut preempt_list = false;
                    for flag in parameters.flags.split(['|', ',']).map(str::trim) {
                        match flag {
                            "0" if parameters.flags == "0" => {}
                            "CHANNELFULLPREEMPT" => preempt_channel = true,
                            "LISTFULLPREEMPT" => preempt_list = true,
                            _ => {
                                return Ok(Classified::Refuse(
                                    id,
                                    format!(
                                        "dialogue parameter label {:?}, row {}, source {} ({:?}): unsupported flag {:?} in Flags={:?}",
                                        parameters.label,
                                        parameters.row,
                                        assignment.source.source_label,
                                        assignment.source.origin,
                                        flag,
                                        parameters.flags
                                    ),
                                ));
                            }
                        }
                    }
                    if (preempt_channel || preempt_list) && profile.mix.is_none() {
                        return Ok(Classified::Refuse(
                            id,
                            format!(
                                "dialogue parameter label {:?}, row {}, source {} ({:?}): Flags={:?} require source channel/list preemption capacities",
                                parameters.label,
                                parameters.row,
                                assignment.source.source_label,
                                assignment.source.origin,
                                parameters.flags
                            ),
                        ));
                    }
                    if let Some(policy) = &profile.mix {
                        let channel = properties.channel.unwrap_or(policy.default_channel());
                        let Some(rule) = policy.channel(channel) else {
                            return Ok(Classified::Refuse(
                                id,
                                format!("sound channel {channel} missing from source policy"),
                            ));
                        };
                        mixed = Some(mix::Voice {
                            id,
                            channel,
                            priority: policy
                                .default_priority()
                                .clamp(rule.minimum_priority, rule.maximum_priority),
                            member: observed.member.replace('\\', "/").to_ascii_lowercase(),
                            serial: 0,
                            preempt_channel,
                            preempt_list,
                        });
                    }
                    Some(parameters.values.volume)
                }
                pcx_play::sound_parameters::ParameterValues::Full(parameters) => {
                    return Ok(Classified::Refuse(
                        id,
                        format!(
                            "full numeric parameter label {:?}, row {}, source {} ({:?}): effective priority/channel, pitch variance and attenuation are not implemented; values={:?}, Flags={:?}, EAXFlags={:?}",
                            parameters.label,
                            parameters.row,
                            assignment.source.source_label,
                            assignment.source.origin,
                            parameters.values,
                            parameters.flags,
                            parameters.eax_flags,
                        ),
                    ));
                }
            },
            None => None,
        };
        if mixed.is_none()
            && let Some(policy) = &profile.mix
        {
            let channel = properties.channel.unwrap_or(policy.default_channel());
            let Some(rule) = policy.channel(channel) else {
                return Ok(Classified::Refuse(
                    id,
                    format!("sound channel {channel} missing from source policy"),
                ));
            };
            mixed = Some(mix::Voice {
                id,
                channel,
                priority: policy
                    .default_priority()
                    .clamp(rule.minimum_priority, rule.maximum_priority),
                member: observed.member.replace('\\', "/").to_ascii_lowercase(),
                serial: 0,
                preempt_channel: false,
                preempt_list: false,
            });
        }
        if let Some(mixed) = &mixed
            && let Some(policy) = &profile.mix
        {
            if let Err(error) = objects.volume_group_channel_gain(mixed.channel) {
                return Ok(Classified::Refuse(id, error));
            }
            channel_gain = policy
                .channel(mixed.channel)
                .ok_or("source channel disappeared")?
                .gain;
        }
        if let Some(volume) = properties
            .volume
            .or(volume)
            .or_else(|| profile.mix.as_ref().map(|policy| policy.default_volume()))
        {
            let volume = volume as f32;
            if !(0.0..=profile.maximum).contains(&volume) {
                return Ok(Classified::Refuse(
                    id,
                    "sound volume exceeds source range".into(),
                ));
            }
            gain = Some(volume / profile.maximum * channel_gain);
        } else if observed.fade_in_rate != 0 || observed.fade_out_rate != 0 {
            return Ok(Classified::Refuse(
                id,
                "fade requires an explicit source target volume".into(),
            ));
        }
        speed = properties
            .pitch
            .or_else(|| profile.mix.as_ref().map(|policy| policy.default_pitch()));
        if speed.is_some_and(|pitch| pitch <= 0.0) {
            return Ok(Classified::Refuse(
                id,
                "nonpositive pitch is not played".into(),
            ));
        }
        duration = observed.duration;
        channel = supported
            .channel
            .take()
            .or_else(|| mixed.as_ref().map(|voice| voice.channel));
        supported.volume = None;
        supported.pitch = None;
    } else if let Some(reason) = parameter_block(objects, id)? {
        return Ok(Classified::Refuse(id, reason));
    }
    if let Some(reason) = property_block(&supported) {
        return Ok(Classified::Refuse(id, reason.to_string()));
    }
    Ok(Classified::Play(PlayPlan {
        id,
        encoded_mp3: observed.encoded_mp3,
        looping: observed.looping,
        fade_in_rate: observed.fade_in_rate,
        fade_out_rate: observed.fade_out_rate,
        sha256: observed.sha256,
        byte_len: observed.byte_len,
        gain,
        speed,
        duration,
        profile,
        channel,
        mix: mixed,
        victim: None,
        channel_gain,
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
        return Some(
            "3D placement is not played: native attenuation, cutoff and listener mapping are unmeasured",
        );
    }
    if stop_when_out_of_range {
        // The per-frame range check is measured; its dispatch action is not.
        return Some("out-of-range dispatch action is unidentified; not played");
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
    use std::collections::HashSet;
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
        profile: Option<PlaybackProfile>,
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
        world.insert_resource(Fixture {
            host,
            profile: None,
        });
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
                    let Fixture { host, profile } = &mut *fixture;
                    memory.profile = profile
                        .as_ref()
                        .map(|profile| (witness.0.clone(), Ok(profile.clone())));
                    publish_queued_starts(
                        host,
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

    fn profile() -> PlaybackProfile {
        PlaybackProfile {
            maximum: 100.0,
            update_seconds: 0.25,
            minimum_fade_step: 3,
            mix: None,
        }
    }

    pub(super) fn mix_policy(
        channel_capacity: u32,
        list_capacity: u8,
        filename_capacity: u8,
    ) -> std::sync::Arc<VoicePolicy> {
        let mut defaults = Vec::new();
        for (field, value) in [0x2c, 0x30, 0x34, 0x38, 0x3c, 0x40].into_iter().zip([
            100,
            1.0f32.to_bits(),
            0,
            7,
            0,
            0,
        ]) {
            defaults.extend([0xc7, 0x41, field]);
            defaults.extend(value.to_le_bytes());
        }
        let channels: Vec<_> = [channel_capacity, 0, 100, 1.0f32.to_bits(), 0u32]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        std::sync::Arc::new(
            VoicePolicy::read(
                ScriptAsset {
                    label: "synthetic:defaults",
                    bytes: &defaults,
                },
                ScriptAsset {
                    label: "synthetic:channels",
                    bytes: &channels,
                },
                ScriptAsset {
                    label: "synthetic:list",
                    bytes: &[0x83, 0xf8, list_capacity],
                },
                ScriptAsset {
                    label: "synthetic:filename",
                    bytes: &[0x83, 0x78, 0x18, filename_capacity],
                },
                origin(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn category_gain_clamps_channel_after_user_multiplication_before_voice() {
        let mut category = ScriptAudioCategory {
            base: 0.5,
            scale: 2.0,
            unity: Some(1.0),
        };
        assert_eq!(category.with_user_gain(0.2, 0.5), 0.2);
        assert_eq!(category.with_user_gain(0.2, 1.0), 0.4);
        category.scale = 4.0;
        assert_eq!(category.with_user_gain(0.2, 1.0), 0.4);
        category.scale = -1.0;
        assert_eq!(category.with_user_gain(0.2, 0.5), 0.0);
        category.scale = 0.5;
        assert_eq!(category.with_user_gain(0.2, 0.25), 0.025);
    }
    #[test]
    fn fade_clock_preserves_pauses_and_frame_partition() {
        let fade = || FadeIn {
            elapsed: 0.0,
            period: 0.25,
            increment: 0.1,
            target: 0.8,
        };
        let mut whole = fade();
        let mut chunks = fade();
        assert_eq!(whole.advance(0.75, false), 0.3);
        for delta in [0.125, 0.25, 0.375] {
            chunks.advance(delta, false);
        }
        assert_eq!(chunks.sample(), whole.sample());
        assert_eq!(chunks.advance(100.0, true), 0.3);
        assert_eq!(chunks.elapsed, 0.75);
        assert_eq!(chunks.advance(100.0, false), 0.8);
    }

    #[test]
    fn dialogue_source_volume_and_flag_refusals_preserve_receipt() {
        for (flags, with_policy) in [
            ("0", false),
            ("CHANNELFULLPREEMPT|LISTFULLPREEMPT", false),
            ("CHANNELFULLPREEMPT|LISTFULLPREEMPT", true),
        ] {
            use pcx_play::sound_parameters::{Banks, BanksBudget, Input, Profile};
            let mut slk = String::from("ID;PWXL;N;E\r\n");
            let rows = [
                [
                    "Selector",
                    "DirectoryBase",
                    "FileNames",
                    "Volume",
                    "Flags",
                    "version",
                ],
                ["Speech", "Voice", "a.wav", "40", flags, "3"],
            ];
            for (y, row) in rows.iter().enumerate() {
                for (x, value) in row.iter().enumerate() {
                    slk.push_str(&format!("C;X{};Y{};K\"{value}\"\r\n", x + 1, y + 1));
                }
            }
            slk.push_str("E\r\n");
            let bytes = pcm(16, false);
            let assets = SoundAssets::read(
                &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
                file_budget(bytes.len(), 1),
            )
            .unwrap();
            let banks = Banks::read(
                &[Input {
                    asset: ScriptAsset {
                        label: "synthetic:dialogue",
                        bytes: slk.as_bytes(),
                    },
                    origin: &origin(),
                    label_column: "Selector",
                    labels: &["Speech"],
                    profile: Profile::Dialogue,
                }],
                BanksBudget {
                    banks: 1,
                    table_bytes: slk.len(),
                    total_table_bytes: slk.len(),
                    coordinates: CoordinateLimits {
                        max_row: 8,
                        max_column: 8,
                    },
                    definitions: 1,
                    metadata_bytes: 512,
                },
            )
            .unwrap();
            let mut object_host = host(assets).with_sound_parameter_banks(banks).unwrap();
            let id = start(
                &mut object_host,
                &create(r"Media\a.wav", false, false, false, 0, 0, ""),
            );
            object_host
                .native(
                    "SetSoundParamsFromLabel",
                    &[Value::Handle(id), Value::Str("Speech".into())],
                )
                .unwrap();
            let mut world = world_for(object_host, limits_for(bytes.len()));
            let mut playback_profile = profile();
            if with_policy {
                playback_profile.mix = Some(mix_policy(4, 8, 4));
            }
            world.resource_mut::<Fixture>().profile = Some(playback_profile);
            publish(&mut world).unwrap();
            if flags != "0" && !with_policy {
                assert_eq!(count::<ScriptVoice>(&mut world), 0);
                let reason = world
                    .resource::<ScriptAudioReport>()
                    .last_refusal
                    .as_deref()
                    .unwrap();
                for expected in [
                    "Speech",
                    "row 2",
                    "synthetic:dialogue",
                    "synthetic:archive",
                    "CHANNELFULLPREEMPT|LISTFULLPREEMPT",
                    "preemption",
                ] {
                    assert!(reason.contains(expected), "missing {expected:?}: {reason}");
                }
                continue;
            }
            let mut query = world.query::<(&ScriptVoice, &FadeIn)>();
            let (voice, fade) = query.single(&world).unwrap();
            assert_eq!(voice.source_label, "synthetic:pcm");
            assert_eq!(fade.target, 0.4);
            assert_eq!(
                fade.increment, 0.03,
                "zero input uses source-decoded minimum step"
            );
            let assignment = world
                .resource::<Fixture>()
                .host
                .sound_parameter_assignment(id)
                .unwrap()
                .unwrap();
            assert_eq!(assignment.source.source_label, "synthetic:dialogue");
            assert_eq!(assignment.source.origin, &origin());
        }
    }

    #[test]
    fn source_list_preemption_retires_entities_and_shared_source_references() {
        use pcx_play::sound_parameters::{Banks, BanksBudget, Input, Profile};
        let table = b"ID;PWXL;N;E\r\nC;X1;Y1;K\"Selector\"\r\nC;X2;Y1;K\"DirectoryBase\"\r\nC;X3;Y1;K\"FileNames\"\r\nC;X4;Y1;K\"Volume\"\r\nC;X5;Y1;K\"Flags\"\r\nC;X6;Y1;K\"version\"\r\nC;X1;Y2;K\"Speech\"\r\nC;X2;Y2;K\"Voice\"\r\nC;X3;Y2;K\"a.wav\"\r\nC;X4;Y2;K\"50\"\r\nC;X5;Y2;K\"LISTFULLPREEMPT|CHANNELFULLPREEMPT\"\r\nC;X6;Y2;K\"0\"\r\nE\r\n";
        for batch in [false, true] {
            let bytes = pcm(16, false);
            let assets = SoundAssets::read(
                &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
                file_budget(bytes.len(), 1),
            )
            .unwrap();
            let banks = Banks::read(
                &[Input {
                    asset: ScriptAsset {
                        label: "synthetic:dialogue",
                        bytes: table,
                    },
                    origin: &origin(),
                    label_column: "Selector",
                    labels: &["Speech"],
                    profile: Profile::Dialogue,
                }],
                BanksBudget {
                    banks: 1,
                    table_bytes: table.len(),
                    total_table_bytes: table.len(),
                    coordinates: CoordinateLimits {
                        max_row: 8,
                        max_column: 8,
                    },
                    definitions: 1,
                    metadata_bytes: 512,
                },
            )
            .unwrap();
            let mut object_host = host(assets).with_sound_parameter_banks(banks).unwrap();
            let mut ids = Vec::new();
            for _ in 0..2 {
                let id = start(
                    &mut object_host,
                    &create(r"Media\a.wav", false, false, false, 2, 2, ""),
                );
                object_host
                    .native(
                        "SetSoundParamsFromLabel",
                        &[Value::Handle(id), Value::Str("Speech".into())],
                    )
                    .unwrap();
                ids.push(id);
            }
            let mut world = world_for(object_host, limits_for(bytes.len()));
            let mut playback_profile = profile();
            playback_profile.mix = Some(mix_policy(4, 2, 4));
            world.resource_mut::<Fixture>().profile = Some(playback_profile);
            if !batch {
                publish(&mut world).unwrap();
                assert_eq!(count::<ScriptVoice>(&mut world), 2);
            }
            let id = {
                let mut fixture = world.resource_mut::<Fixture>();
                let id = start(
                    &mut fixture.host,
                    &create(r"Media\a.wav", false, false, false, 2, 2, ""),
                );
                fixture
                    .host
                    .native(
                        "SetSoundParamsFromLabel",
                        &[Value::Handle(id), Value::Str("Speech".into())],
                    )
                    .unwrap();
                id
            };
            publish(&mut world).unwrap();
            let mut voices = world.query::<&ScriptVoice>();
            let surviving: HashSet<_> = voices.iter(&world).map(|voice| voice.handle).collect();
            assert_eq!(surviving, HashSet::from([ids[1], id]));
            assert_eq!(world.resource::<Assets<AudioSource>>().iter().count(), 1);
            assert_eq!(world.resource::<ScriptAudioMemory>().sources[0].voices, 2);
            assert_eq!(world.resource::<ScriptAudioReport>().played, 3);
            assert_eq!(world.resource::<ScriptAudioReport>().refusal_count, 0);
        }
    }

    #[test]
    fn out_of_range_flag_refuses_with_the_unidentified_action_named() {
        assert_eq!(creation_block(false, false, "", 0, 0, None), None);
        assert_eq!(
            creation_block(false, true, "", 0, 0, None),
            Some("out-of-range dispatch action is unidentified; not played")
        );
        assert_eq!(
            creation_block(true, true, "", 0, 0, None),
            Some(
                "3D placement is not played: native attenuation, cutoff and listener mapping are unmeasured"
            )
        );
    }

    #[test]
    fn explicit_nonspatial_settings_publish_source_gain_and_timed_fade() {
        let bytes = pcm(16, false);
        let assets = SoundAssets::read(
            &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
            file_budget(bytes.len(), 1),
        )
        .unwrap();
        let mut object_host = host(assets);
        let id = start(
            &mut object_host,
            &create(r"Media\a.wav", false, false, false, 10, 7, ""),
        );
        for (native, value) in [
            ("SetSoundVolume", Value::Int(50)),
            ("SetSoundChannel", Value::Int(4)),
            ("SetSoundPitch", Value::Real(1.5)),
            ("SetSoundDuration", Value::Int(1)),
        ] {
            object_host
                .native(native, &[Value::Handle(id), value])
                .unwrap();
        }
        let mut world = world_for(object_host, limits_for(bytes.len()));
        world.resource_mut::<Fixture>().profile = Some(profile());
        publish(&mut world).unwrap();
        let mut query = world.query::<(
            &ScriptVoice,
            &ScriptAudioGain,
            &mut FadeIn,
            &PlaybackSettings,
            &ScriptAudioMetadata,
        )>();
        let (voice, gain, mut fade, playback, metadata) = query.single_mut(&mut world).unwrap();
        assert_eq!(voice.handle, id);
        assert_eq!(gain.0, 0.0);
        assert_eq!(playback.speed, 1.5);
        assert_eq!(
            playback.duration, None,
            "native duration is metadata, not a waveform crop"
        );
        assert_eq!(metadata.duration, Some(1));
        assert_eq!(metadata.channel, Some(4));
        fade.elapsed = 0.249;
        assert_eq!(fade.sample(), 0.0);
        fade.elapsed = 0.25;
        assert_eq!(fade.sample(), 0.1);
        fade.elapsed = 100.0;
        assert_eq!(fade.sample(), 0.5);
        assert_eq!(world.resource::<ScriptAudioReport>().played, 1);
        assert!(world.resource::<ScriptAudioReport>().last_partial.is_none());
    }

    #[test]
    fn source_profile_refuses_missing_volume_and_invalid_effective_numbers() {
        for (native, value, expected) in [
            ("SetSoundPitch", Value::Real(0.0), "pitch"),
            ("SetSoundVolume", Value::Int(101), "volume"),
            ("SetSoundChannel", Value::Int(4), "target volume"),
        ] {
            let bytes = pcm(16, false);
            let assets = SoundAssets::read(
                &[file_input(r"Media\a.wav", "synthetic:pcm", &bytes)],
                file_budget(bytes.len(), 1),
            )
            .unwrap();
            let mut object_host = host(assets);
            let id = start(
                &mut object_host,
                &create(r"Media\a.wav", false, false, false, 10, 7, ""),
            );
            object_host
                .native(native, &[Value::Handle(id), value])
                .unwrap();
            if native == "SetSoundPitch" {
                object_host
                    .native("SetSoundVolume", &[Value::Handle(id), Value::Int(50)])
                    .unwrap();
            }
            let mut world = world_for(object_host, limits_for(0));
            world.resource_mut::<Fixture>().profile = Some(profile());
            publish(&mut world).unwrap();
            assert_eq!(count::<ScriptVoice>(&mut world), 0);
            assert!(
                world
                    .resource::<ScriptAudioReport>()
                    .last_refusal
                    .as_deref()
                    .unwrap()
                    .contains(expected)
            );
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
                .is_some_and(|text| text
                    .contains("native attenuation, cutoff and listener mapping are unmeasured"))
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
                      voices: ScriptVoices| {
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires PCX_SCRIPT_AUDIO_{CLIENT,BJ,TABLE,MAP,MEDIA,LABEL} local original inputs"]
    fn local_original_dialogue_voice_decodes_through_published_consumer() {
        use bevy::audio::Decodable;
        use pcx_play::sound_parameters::{Banks, BanksBudget, Input, Profile};
        use pcx_script::{Expr, Item, Literal, Stmt};
        let path = |name: &str| {
            std::env::var(format!("PCX_SCRIPT_AUDIO_{name}"))
                .expect("explicit local original input")
        };
        let client = path("CLIENT");
        let bj = path("BJ");
        let table_path = path("TABLE");
        let map_path = path("MAP");
        let media_path = path("MEDIA");
        let label = path("LABEL");
        let identity = |source: String| AssetIdentity {
            source,
            edition: "explicit local original selection".into(),
            version:
                "client digest qualified; adjacent asset release not independently established"
                    .into(),
        };
        let image = std::fs::read(&client).unwrap();
        let rules = pcx_play::map_startup_host::sound_playback::Rules::reviewed_legacy(
            &image,
            identity(client),
        )
        .unwrap();
        let maximum = crate::sound_volume::VolumeScale::checked(&std::fs::read(&bj).unwrap())
            .unwrap()
            .maximum;
        let raw = std::fs::read(&table_path).unwrap();
        let banks = Banks::read(
            &[Input {
                asset: ScriptAsset {
                    label: &table_path,
                    bytes: &raw,
                },
                origin: &identity(table_path.clone()),
                label_column: "SoundName",
                labels: &[&label],
                profile: Profile::Dialogue,
            }],
            BanksBudget {
                banks: 1,
                table_bytes: raw.len(),
                total_table_bytes: raw.len(),
                coordinates: CoordinateLimits {
                    max_row: 100_000,
                    max_column: 64,
                },
                definitions: 1,
                metadata_bytes: 64 << 10,
            },
        )
        .unwrap();
        let original_map = std::fs::read_to_string(&map_path).unwrap();
        let script = pcx_script::parse_script(&original_map).unwrap();
        let init = script
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name == "InitSounds" => {
                    function.body.as_ref()
                }
                _ => None,
            })
            .expect("original InitSounds");
        let variables: Vec<_> = init
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Call { callee, args, .. } if callee.name == "SetSoundParamsFromLabel" => {
                    match args.as_slice() {
                        [
                            Expr::Ident(variable, _),
                            Expr::Literal(Literal::Str(name), _),
                        ] if name.decoded() == label => Some(variable.name.as_str()),
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect();
        assert_eq!(variables.len(), 1, "unambiguous original label assignment");
        let creations: Vec<_> = init
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Set {
                    target: Expr::Ident(variable, _),
                    value: Expr::Call { callee, args, .. },
                    ..
                } if variable.name == variables[0] => match callee.as_ref() {
                    Expr::Ident(name, _) if name.name == "CreateSound" => Some(args),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(creations.len(), 1, "unambiguous original CreateSound");
        let arguments: Vec<_> = creations[0]
            .iter()
            .map(|arg| match arg {
                Expr::Literal(Literal::Str(value), _) => Value::Str(value.decoded()),
                Expr::Literal(Literal::Bool(value), _) => Value::Bool(*value),
                Expr::Literal(Literal::Int(value), _) => {
                    Value::Int(i32::try_from(value.value).unwrap())
                }
                _ => panic!("unsupported original creation expression"),
            })
            .collect();
        let Value::Str(member) = &arguments[0] else {
            panic!("original sound filename")
        };
        let encoded = std::fs::read(&media_path).unwrap();
        let asset_origin = identity(format!("{media_path} explicitly bound to {member}"));
        let assets = SoundAssets::read_encoded(
            &[SoundInput {
                member,
                asset: ScriptAsset {
                    label: &media_path,
                    bytes: &encoded,
                },
                origin: asset_origin,
            }],
            file_budget(encoded.len(), 1),
            Mp3Budget {
                max_input_allocation_bytes: encoded.len() + pcx_play::mp3_frame::DECODER_RING_BYTES,
                max_decoder_steps: 2 << 20,
                max_packets: 64 << 10,
                max_frames: 4 << 20,
                max_samples: 8 << 20,
            },
        )
        .unwrap();
        let mut object_host = host(assets).with_sound_parameter_banks(banks).unwrap();
        let id = start(&mut object_host, &arguments);
        object_host
            .native(
                "SetSoundParamsFromLabel",
                &[Value::Handle(id), Value::Str(label)],
            )
            .unwrap();
        let mut world = world_for(object_host, limits_for(encoded.len()));
        world.resource_mut::<Fixture>().profile = Some(PlaybackProfile {
            maximum,
            update_seconds: rules.update_seconds(),
            minimum_fade_step: rules.minimum_fade_step(),
            mix: Some(rules.voice_policy()),
        });
        publish(&mut world).unwrap();
        assert_eq!(world.resource::<ScriptAudioReport>().played, 1);
        assert_eq!(
            world.resource::<ScriptAudioReport>().refusal_count,
            0,
            "{:?}",
            world.resource::<ScriptAudioReport>().last_refusal
        );
        let mut voices = world.query::<(&ScriptVoice, &mix::Voice, &FadeIn)>();
        let (voice, mixed, fade) = voices.single(&world).unwrap();
        assert_eq!(voice.member, member.as_str());
        assert!(mixed.preempt_channel && mixed.preempt_list);
        assert!(fade.target > 0.0);
        let audio = world
            .resource::<Assets<AudioSource>>()
            .get(&voice.source)
            .unwrap();
        let (samples, nonzero) = audio
            .decoder()
            .take(8 << 20)
            .fold((0usize, false), |(count, audible), sample| {
                (count + 1, audible || sample != 0.0)
            });
        assert!(
            samples > 0 && nonzero,
            "selected original voice decoded silence"
        );
        println!(
            "selected original voice published and decoded: {samples} samples; source flags retained and capacity policy admitted"
        );
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
        explicit_nonspatial_settings_publish_source_gain_and_timed_fade();
        source_profile_refuses_missing_volume_and_invalid_effective_numbers();
        dialogue_source_volume_and_flag_refusals_preserve_receipt();
        fade_clock_preserves_pauses_and_frame_partition();
        source_list_preemption_retires_entities_and_shared_source_references();
        mix::tests::list_and_channel_priority_rules_use_distinct_comparisons();
        mix::tests::filename_capacity_and_deterministic_ties_preserve_flags();
        category_gain_clamps_channel_after_user_multiplication_before_voice();
        19
    }
}
