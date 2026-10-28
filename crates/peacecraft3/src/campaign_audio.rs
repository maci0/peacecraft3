//! Campaign music from the selected sources.
//!
//! The script Host retains the executed `SetMapMusic`/`ClearMapMusic` and
//! `PlayThematicMusic`/`EndThematicMusic` requests. This module plays them:
//! thematic music names a file, map music names a `UI\war3skins.txt` playlist key
//! (`<name>_V<edition>`, the suffix the menu uses for `GlueMusic_V*`) in the local
//! player's race section, then `[Default]`. Provenance and unverified points:
//! `docs/re-notes/campaign-audio.md`.

use crate::menu_audio::{AuthoredVolume, CampaignMusic};
use crate::menu_skin::SkinTable;
use bevy::prelude::*;
use pcx_assets::Resolver;
use pcx_play::map_startup_host::MapStartupHost;
use peacecraft3::{
    browser_configuration::SelectedMapSources, campaign_startup::CampaignSeed,
    map_requests::MapRequests, map_script_session::MapScriptSession,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Weak};

const SKIN: &str = "UI\\war3skins.txt";
/// Application read bounds: the skin text limit matches the HUD; the track, table and
/// sound-file limits match the menu audio.
const MAX_SKIN_BYTES: u64 = 1 << 20;
const MAX_TRACK_BYTES: u64 = 64 << 20;
const MAX_TABLE_BYTES: u64 = 8 << 20;
const MAX_SOUND_BYTES: u64 = 16 << 20;
const TABLE_COORDINATES: pcx_slq::CoordinateLimits = pcx_slq::CoordinateLimits {
    max_row: 100_000,
    max_column: 1024,
};
/// Application memory bound for sound files kept for reuse.
const SOUND_CACHE_BYTES: usize = 32 << 20;
/// Most distinct problem reports kept for de-duplication.
const MAX_REPORTS: usize = 256;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CampaignAudioSet;

pub(crate) fn install(app: &mut App) {
    app.init_resource::<CampaignAudio>()
        .init_resource::<Assets<AudioSource>>()
        .add_systems(
            Update,
            drive_music
                .in_set(CampaignAudioSet)
                .after(peacecraft3::campaign_startup::CampaignStartupSet),
        );
}

/// A map-music request exactly as the script executed it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MapMusic {
    name: String,
    random: bool,
    index: i32,
}

/// What is being heard: a thematic file or a map playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Request {
    Thematic(String),
    Map(MapMusic),
}

/// Deterministic presentation randomness (splitmix64), seeded from the campaign seed.
struct Rng(u64);

impl Rng {
    fn below(&mut self, bound: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        usize::try_from((z ^ (z >> 31)) % bound as u64).expect("a value below a usize bound")
    }
}

/// Source reader and race context for one published session.
struct Bank {
    reader: Arc<Resolver>,
    skin: SkinTable,
    sections: Vec<String>,
    expansion: bool,
    rng: Rng,
    /// The script's sound-volume range (`SetSoundVolumeBJ`), which table `Volume` cells map onto.
    volume_maximum: Result<f32, String>,
    tables: BTreeMap<&'static str, pcx_slq::Table>,
    sounds: BTreeMap<String, Arc<[u8]>>,
    sounds_bytes: usize,
}

/// One `UI\SoundInfo` row: the columns every sound table shares.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SoundRow {
    name: String,
    /// Archive paths of the alternatives; one is drawn per start.
    members: Vec<String>,
    /// `Volume` as a fraction of the script's range.
    volume: f32,
    pitch: f32,
    pitch_variance: f32,
    channel: i32,
    flags: Vec<String>,
}

/// Plays one sound row; the entity ends with its sound.
#[derive(Component)]
pub(crate) struct GameSound {
    pub name: String,
    channel: i32,
}

impl Bank {
    fn read(&self, member: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.reader
            .resolve_bounded(member, limit)
            .map(|asset| asset.bytes)
            .map_err(|error| format!("{member}: {error}"))
    }

    /// A sound file's bytes, kept for reuse within the cache bound.
    fn sound_bytes(&mut self, member: &str) -> Result<Arc<[u8]>, String> {
        let key = member.to_ascii_lowercase();
        if let Some(bytes) = self.sounds.get(&key) {
            return Ok(Arc::clone(bytes));
        }
        let bytes: Arc<[u8]> = self.read(member, MAX_SOUND_BYTES)?.into();
        if self
            .sounds_bytes
            .checked_add(bytes.len())
            .is_some_and(|total| total <= SOUND_CACHE_BYTES)
        {
            self.sounds_bytes += bytes.len();
            self.sounds.insert(key, Arc::clone(&bytes));
        }
        Ok(bytes)
    }

    /// The unique row of a sound table named `name` (ASCII case-insensitive).
    fn row(&mut self, table: &'static str, name: &str) -> Result<SoundRow, String> {
        if !self.tables.contains_key(table) {
            let bytes = self.read(table, MAX_TABLE_BYTES)?;
            let parsed = pcx_slq::Table::parse_bounded(&bytes, TABLE_COORDINATES)
                .map_err(|error| format!("{table}: {error}"))?;
            self.tables.insert(table, parsed);
        }
        let table_data = &self.tables[table];
        let fail = |reason: &str| format!("{table} row {name:?}: {reason}");
        let mut hits = table_data.data_rows().filter(|row| {
            row.by_name("SoundName")
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        });
        let row = hits.next().ok_or_else(|| fail("not found"))?;
        if hits.next().is_some() {
            return Err(fail("defined more than once"));
        }
        let sounds = pcx_slq::SoundTable(table_data);
        // A `_` entry is a disabled placeholder, not a file.
        let members: Vec<String> = sounds
            .file_names(row.index())
            .iter()
            .enumerate()
            .filter(|(_, file)| **file != "_")
            .filter_map(|(index, _)| sounds.sound_path(row.index(), index))
            .collect();
        if members.is_empty() {
            return Err(fail("lists no file"));
        }
        let number = |column: &str| -> Result<f32, String> {
            row.by_name(column)
                .and_then(|cell| cell.trim().parse::<f32>().ok())
                .filter(|value| value.is_finite())
                .ok_or_else(|| fail(&format!("{column} is missing or not a number")))
        };
        let maximum = self.volume_maximum.clone()?;
        let volume = number("Volume")?;
        if !(0.0..=maximum).contains(&volume) {
            return Err(fail("Volume is outside the script's sound range"));
        }
        let pitch = number("Pitch")?;
        let pitch_variance = number("PitchVariance")?;
        if pitch <= 0.0 || pitch_variance < 0.0 {
            return Err(fail("Pitch must be positive and PitchVariance nonnegative"));
        }
        let channel = row
            .by_name("Channel")
            .and_then(|cell| cell.trim().parse::<i32>().ok())
            .ok_or_else(|| fail("Channel is missing or not an integer"))?;
        let flags = row
            .by_name("Flags")
            .unwrap_or_default()
            .split([',', '|'])
            .map(str::trim)
            .filter(|flag| !matches!(*flag, "" | "0"))
            .map(str::to_owned)
            .collect();
        Ok(SoundRow {
            name: name.to_owned(),
            members,
            volume: volume / maximum,
            pitch,
            pitch_variance,
            channel,
            flags,
        })
    }

    /// The playlist for a map-music name: first race section defining the key.
    fn playlist(&self, name: &str) -> Result<Vec<String>, String> {
        let key = format!("{name}_V{}", u32::from(self.expansion));
        for wanted in &self.sections {
            let Some(section) = self.skin.sections().iter().find(|s| &s.name == wanted) else {
                continue;
            };
            let mut hits = section
                .entries
                .iter()
                .filter(|(entry, _)| entry.eq_ignore_ascii_case(&key));
            let Some((_, value)) = hits.next() else {
                continue;
            };
            if hits.next().is_some() {
                return Err(format!("{SKIN} [{wanted}] defines {key} more than once"));
            }
            let tracks: Vec<String> = value
                .split(';')
                .map(str::trim)
                .filter(|track| !track.is_empty())
                .map(str::to_owned)
                .collect();
            if tracks.is_empty() {
                return Err(format!("{SKIN} [{wanted}] {key} lists no track"));
            }
            return Ok(tracks);
        }
        Err(format!("{SKIN} defines no {key} in {:?}", self.sections))
    }
}

/// Position inside a playlist. Random lists draw the next track; indexed lists
/// start at the authored index and then follow list order.
struct Playlist {
    request: MapMusic,
    tracks: Vec<String>,
    next: usize,
}

impl Playlist {
    fn start(request: MapMusic, tracks: Vec<String>, rng: &mut Rng) -> Result<Self, String> {
        let next = if request.random {
            rng.below(tracks.len())
        } else {
            usize::try_from(request.index)
                .ok()
                .filter(|&index| index < tracks.len())
                .ok_or_else(|| {
                    format!(
                        "map music index {} is outside the {}-track list",
                        request.index,
                        tracks.len()
                    )
                })?
        };
        Ok(Self {
            request,
            tracks,
            next,
        })
    }

    fn take(&mut self, rng: &mut Rng) -> String {
        let track = self.tracks[self.next].clone();
        self.next = if self.request.random && self.tracks.len() > 1 {
            // The track just played is not drawn again.
            (self.next + 1 + rng.below(self.tracks.len() - 1)) % self.tracks.len()
        } else {
            (self.next + 1) % self.tracks.len()
        };
        track
    }
}

#[derive(Resource, Default)]
pub(crate) struct CampaignAudio {
    session: Option<Weak<()>>,
    bank: Option<Bank>,
    /// Source preparation failed for this session; it is reported once, not retried.
    prepare_failed: bool,
    /// The request whose track was last started; `None` after a stop or a clear.
    playing: Option<Request>,
    playlist: Option<Playlist>,
    /// A thematic file that played to its end; map music covers the rest of the request.
    finished_thematic: Option<String>,
    /// The request that failed to start; it is not retried until the script changes it.
    failed: Option<Request>,
    /// Accepted player commands already voiced.
    pub(crate) commands_seen: usize,
    /// Problems already logged, so a repeating one is reported once.
    reported: BTreeSet<String>,
    /// Row flags already reported as not applied.
    unapplied_flags: BTreeSet<String>,
    pub last_error: Option<String>,
}

/// Row flags the player applies. `WANT3D` and the rest of the row flags are reported once.
const FLAG_CHANNEL_PREEMPT: &str = "CHANNELFULLPREEMPT";
const FLAG_RANDOM_PITCH: &str = "RANDOMPITCH";
const FLAG_NO_DUPLICATES: &str = "NODUPLICATES";
const APPLIED_FLAGS: [&str; 3] = [FLAG_CHANNEL_PREEMPT, FLAG_RANDOM_PITCH, FLAG_NO_DUPLICATES];
/// Resolution of the pitch draw for `RANDOMPITCH`.
const PITCH_STEPS: usize = 1 << 16;

#[derive(Debug, PartialEq, Eq)]
enum Step {
    Keep,
    Stop,
    Play(String),
}

impl CampaignAudio {
    /// Chooses the next action for the script's music state and whether music is audible.
    fn step(
        &mut self,
        thematic: Option<&str>,
        map: Option<MapMusic>,
        audible: bool,
    ) -> Result<Step, String> {
        if thematic.is_none() {
            self.finished_thematic = None;
        }
        let want = thematic
            .filter(|path| self.finished_thematic.as_deref() != Some(path))
            .map(|path| Request::Thematic(path.to_owned()))
            .or_else(|| map.clone().map(Request::Map));
        if map.is_none() {
            self.playlist = None;
        }
        if self.failed.is_some() && self.failed != want {
            self.failed = None;
        }
        if audible {
            if self.playing == want {
                return Ok(Step::Keep);
            }
            self.playing = None;
            return Ok(Step::Stop);
        }
        let Some(request) = want else {
            self.playing = None;
            return Ok(Step::Keep);
        };
        if self.failed.as_ref() == Some(&request) {
            return Ok(Step::Keep);
        }
        self.failed = Some(request.clone());
        let step = self.start(&request);
        if step.is_ok() {
            self.failed = None;
        }
        step
    }

    fn start(&mut self, request: &Request) -> Result<Step, String> {
        match request {
            Request::Thematic(path) => {
                if self.playing.as_ref() == Some(request) {
                    // Started earlier and silent now: it played to its end.
                    self.finished_thematic = Some(path.clone());
                    self.playing = None;
                    return Ok(Step::Keep);
                }
                self.playing = Some(request.clone());
                Ok(Step::Play(path.clone()))
            }
            Request::Map(map) => {
                let bank = self
                    .bank
                    .as_mut()
                    .ok_or("campaign audio sources are not ready")?;
                if self.playlist.as_ref().map(|list| &list.request) != Some(map) {
                    let tracks = bank.playlist(&map.name)?;
                    self.playlist = Some(Playlist::start(map.clone(), tracks, &mut bank.rng)?);
                }
                let playlist = self.playlist.as_mut().expect("playlist just started");
                self.playing = Some(request.clone());
                Ok(Step::Play(playlist.take(&mut bank.rng)))
            }
        }
    }

    /// Prepares the session's sources once. A failure is reported and not retried.
    pub(crate) fn ensure_bank(
        &mut self,
        sources: &Sources,
        session: &mut MapScriptSession<MapStartupHost>,
    ) {
        if self.bank.is_some() || self.prepare_failed {
            return;
        }
        let Some(reader) = published_reader(&sources.requests, &sources.sources, session) else {
            return;
        };
        let Some(host) = session.host() else { return };
        let race = host
            .objects()
            .local_race_preference_constant()
            .map_err(|error| error.to_string());
        let maximum = volume_maximum(session);
        let seed = sources.seed.prepared.unwrap_or_default();
        match prepare_bank(reader, race, seed, maximum) {
            Ok(bank) => self.bank = Some(bank),
            Err(error) => {
                self.prepare_failed = true;
                self.report(error);
            }
        }
    }

    /// Logs a problem the first time it occurs and keeps the latest for inspection.
    pub(crate) fn report(&mut self, error: String) {
        if self.reported.len() < MAX_REPORTS && self.reported.insert(error.clone()) {
            warn!("campaign audio unavailable: {error}");
        }
        self.last_error = Some(error);
    }

    /// Starts the named row of a sound table at the user's effects volume.
    ///
    /// `CHANNELFULLPREEMPT` ends the sounds on the row's channel, `NODUPLICATES` skips a
    /// start while the row is audible and `RANDOMPITCH` varies the pitch by up to
    /// `PitchVariance`. Positioning (`WANT3D`) and the remaining flags are not applied and
    /// are reported once per flag.
    pub(crate) fn play(
        &mut self,
        table: &'static str,
        name: &str,
        effects: f32,
        commands: &mut Commands,
        assets: &mut Assets<AudioSource>,
        live: &Query<(Entity, &GameSound)>,
    ) -> Result<(), String> {
        let bank = self
            .bank
            .as_mut()
            .ok_or("campaign audio sources are not ready")?;
        let row = bank.row(table, name)?;
        for flag in &row.flags {
            if !APPLIED_FLAGS.contains(&flag.as_str()) && self.unapplied_flags.insert(flag.clone())
            {
                warn!("campaign audio: row flag {flag} (first seen on {name}) is not applied");
            }
        }
        let flagged = |wanted: &str| row.flags.iter().any(|flag| flag == wanted);
        if flagged(FLAG_NO_DUPLICATES) && live.iter().any(|(_, sound)| sound.name == row.name) {
            return Ok(());
        }
        let member = &row.members[bank.rng.below(row.members.len())];
        let bytes = bank.sound_bytes(member)?;
        let mut speed = row.pitch;
        if flagged(FLAG_RANDOM_PITCH) {
            if row.pitch_variance >= row.pitch {
                return Err(format!(
                    "{table} row {name:?}: PitchVariance must stay below Pitch for RANDOMPITCH"
                ));
            }
            let draw = bank.rng.below(PITCH_STEPS) as f32 / PITCH_STEPS as f32;
            speed += row.pitch_variance * (2.0 * draw - 1.0);
        }
        if flagged(FLAG_CHANNEL_PREEMPT) {
            for (entity, sound) in live {
                if sound.channel == row.channel {
                    commands.entity(entity).despawn();
                }
            }
        }
        commands.spawn((
            AudioPlayer(assets.add(AudioSource { bytes })),
            PlaybackSettings::DESPAWN
                .with_volume(bevy::audio::Volume::Linear(row.volume * effects))
                .with_speed(speed),
            GameSound {
                name: row.name,
                channel: row.channel,
            },
            AuthoredVolume(row.volume),
        ));
        Ok(())
    }
}

/// Source reader of the published session, under the same publication checks as the HUD.
fn published_reader(
    requests: &MapRequests,
    sources: &SelectedMapSources,
    session: &MapScriptSession<MapStartupHost>,
) -> Option<Arc<Resolver>> {
    let context = session.context()?;
    let bindings = requests.bindings.as_ref().filter(|bindings| {
        requests
            .published_receipts
            .as_ref()
            .is_some_and(|receipts| Arc::ptr_eq(receipts, &bindings.receipts))
            && requests
                .admission
                .shared_context()
                .is_some_and(|admitted| Arc::ptr_eq(&admitted, context))
    })?;
    sources.for_receipts(&bindings.receipts)
}

fn prepare_bank(
    reader: Arc<Resolver>,
    race: Result<Option<String>, String>,
    seed: i32,
    volume_maximum: Result<f32, String>,
) -> Result<Bank, String> {
    let text = reader
        .resolve_bounded(SKIN, MAX_SKIN_BYTES)
        .map_err(|error| format!("{SKIN}: {error}"))?;
    let text = String::from_utf8(text.bytes).map_err(|_| format!("{SKIN} is not UTF-8"))?;
    let skin = SkinTable::parse(&text).map_err(|error| format!("{SKIN}: {error:?}"))?;
    let sections = crate::campaign_hud::race_sections(&skin, race?)?;
    Ok(Bank {
        expansion: reader.mode().addon == pcx_assets::Addon::Tft,
        reader,
        skin,
        sections,
        rng: Rng(u64::from(seed.cast_unsigned())),
        volume_maximum,
        tables: BTreeMap::new(),
        sounds: BTreeMap::new(),
        sounds_bytes: 0,
    })
}

/// The script's sound-volume range, read from the retained `SetSoundVolumeBJ` source.
fn volume_maximum(session: &MapScriptSession<MapStartupHost>) -> Result<f32, String> {
    let context = session
        .context()
        .ok_or("published script context missing")?;
    let helper = context
        .scripts
        .program()
        .function("SetSoundVolumeBJ")
        .ok_or("sound-volume helper missing")?;
    let source = context
        .scripts
        .source(&helper.source)
        .ok_or_else(|| format!("{}: retained sound-volume source missing", helper.source))?;
    let scale = peacecraft3::sound_volume::VolumeScale::checked(source.as_bytes())
        .map_err(|error| format!("{}: {error}", helper.source))?;
    Some(scale.maximum)
        .filter(|maximum| maximum.is_finite() && *maximum > 0.0)
        .ok_or_else(|| format!("{}: invalid sound-volume range", helper.source))
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Sources<'w> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    seed: Res<'w, CampaignSeed>,
    pub(crate) settings: Res<'w, crate::menu_settings::Backend>,
}

fn drive_music(
    mut commands: Commands,
    mut audio: ResMut<CampaignAudio>,
    mut session: ResMut<MapScriptSession<MapStartupHost>>,
    mut assets: ResMut<Assets<AudioSource>>,
    sources: Sources,
    music: Query<(Entity, &CampaignMusic)>,
) {
    let witness = session.session_witness();
    let same = match (&audio.session, &witness) {
        (Some(previous), Some(current)) => Weak::ptr_eq(previous, current),
        (None, None) => true,
        _ => false,
    };
    if !same {
        *audio = CampaignAudio {
            session: witness.clone(),
            ..default()
        };
        for (entity, _) in &music {
            commands.entity(entity).despawn();
        }
    }
    if witness.is_none() {
        return;
    }
    let (thematic, map) = match session.host() {
        Some(host) => {
            let objects = host.objects();
            (
                objects.thematic_music().map(str::to_owned),
                objects.map_music_selection().map(|selection| MapMusic {
                    name: selection.name.clone(),
                    random: selection.random,
                    index: selection.index,
                }),
            )
        }
        None => return,
    };
    if thematic.is_some() || map.is_some() {
        audio.ensure_bank(&sources, &mut session);
    }
    let audible = !music.is_empty();
    let step = audio.step(thematic.as_deref(), map, audible);
    match step {
        Ok(Step::Keep) => {}
        Ok(Step::Stop) => {
            for (entity, track) in &music {
                debug!("campaign music stops {}", track.member);
                commands.entity(entity).despawn();
            }
        }
        Ok(Step::Play(member)) => {
            let bytes = audio
                .bank
                .as_ref()
                .ok_or_else(|| "campaign audio sources are not ready".to_owned())
                .and_then(|bank| bank.read(&member, MAX_TRACK_BYTES));
            match bytes {
                Ok(bytes) => {
                    let volume = sources.settings.audio.effective_music();
                    commands.spawn((
                        AudioPlayer(assets.add(AudioSource {
                            bytes: bytes.into(),
                        })),
                        PlaybackSettings::DESPAWN.with_volume(bevy::audio::Volume::Linear(volume)),
                        CampaignMusic { member },
                        AuthoredVolume(1.0),
                    ));
                }
                Err(error) => {
                    audio.failed = audio.playing.take();
                    audio.report(error);
                }
            }
        }
        Err(error) => audio.report(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    const SKIN_TEXT: &str = "[Human]\nMusic_V0=a.mp3;b.mp3;c.mp3\nMusic_V1=x.mp3;a.mp3\n[Orc]\nMusic_V1=o.mp3\n[Default]\nFallback_V1=d.mp3\n";

    const TABLE: &str = "UI\\SoundInfo\\Test.slk";
    const HEADER: [&str; 9] = [
        "SoundName",
        "FileNames",
        "DirectoryBase",
        "Volume",
        "Pitch",
        "PitchVariance",
        "Priority",
        "Channel",
        "Flags",
    ];

    fn bank(expansion: bool, sections: &[&str]) -> Bank {
        Bank {
            reader: synthetic_reader(),
            skin: SkinTable::parse(SKIN_TEXT).unwrap(),
            sections: sections.iter().map(|s| (*s).to_owned()).collect(),
            expansion,
            rng: Rng(7),
            volume_maximum: Ok(127.0),
            tables: BTreeMap::new(),
            sounds: BTreeMap::new(),
            sounds_bytes: 0,
        }
    }

    fn slk(rows: &[[&str; 9]]) -> Vec<u8> {
        let mut text = String::from("ID;PWXL;N;E\r\n");
        for (y, row) in std::iter::once(HEADER)
            .chain(rows.iter().copied())
            .enumerate()
        {
            for (x, cell) in row.iter().enumerate() {
                text.push_str(&format!("C;X{};Y{};K\"{cell}\"\r\n", x + 1, y + 1));
            }
        }
        text.push_str("E\r\n");
        text.into_bytes()
    }

    fn rows() -> Vec<u8> {
        slk(&[
            [
                "TestWhat",
                "TestWhat1.wav,TestWhat2.wav,",
                "Units\\Test\\",
                "127",
                "1",
                "0.01",
                "1000",
                "1",
                "WANT3D,CHANNELFULLPREEMPT,RANDOMPITCH",
            ],
            [
                "TestYes",
                "TestYes1.wav",
                "Units\\Test",
                "64",
                "1",
                "0",
                "1000",
                "3",
                "NODUPLICATES",
            ],
            [
                "Disabled",
                "_",
                "Units\\Test\\",
                "10",
                "1",
                "0",
                "1",
                "2",
                "0",
            ],
            [
                "TooLoud",
                "TestYes1.wav",
                "Units\\Test\\",
                "200",
                "1",
                "0",
                "1",
                "2",
                "0",
            ],
            [
                "BadPitch",
                "TestYes1.wav",
                "Units\\Test\\",
                "10",
                "1",
                "-1",
                "1",
                "2",
                "0",
            ],
            [
                "WidePitch",
                "TestYes1.wav",
                "Units\\Test\\",
                "10",
                "1",
                "2",
                "1",
                "2",
                "RANDOMPITCH",
            ],
            [
                "Twice",
                "TestYes1.wav",
                "Units\\Test\\",
                "10",
                "1",
                "0",
                "1",
                "2",
                "0",
            ],
            [
                "Twice",
                "TestYes1.wav",
                "Units\\Test\\",
                "10",
                "1",
                "0",
                "1",
                "2",
                "0",
            ],
        ])
    }

    struct Memory {
        info: pcx_assets::SourceInfo,
        files: Vec<(String, Vec<u8>)>,
    }

    impl pcx_assets::ContentSource for Memory {
        fn info(&self) -> &pcx_assets::SourceInfo {
            &self.info
        }
        fn paths(&self) -> Result<Vec<String>, pcx_assets::SourceError> {
            Ok(self.files.iter().map(|(path, _)| path.clone()).collect())
        }
        fn read(
            &self,
            member: &str,
        ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
            self.read_bounded(member, u64::MAX)
        }
        fn read_bounded(
            &self,
            member: &str,
            limit: u64,
        ) -> Result<Option<pcx_assets::SourceContent>, pcx_assets::SourceError> {
            let Some((_, bytes)) = self.files.iter().find(|(path, _)| path == member) else {
                return Ok(None);
            };
            if bytes.len() as u64 > limit {
                return Err(pcx_assets::SourceError::Limit {
                    requested: bytes.len() as u64,
                    limit,
                });
            }
            Ok(Some(pcx_assets::SourceContent {
                bytes: bytes.clone(),
                declared_length: bytes.len() as u64,
                locator: member.to_owned(),
            }))
        }
    }

    fn synthetic_reader() -> Arc<Resolver> {
        let mut files: Vec<(String, Vec<u8>)> =
            ["a.mp3", "b.mp3", "c.mp3", "x.mp3", "o.mp3", "d.mp3"]
                .iter()
                .map(|name| ((*name).to_owned(), name.as_bytes().to_vec()))
                .collect();
        files.push((TABLE.into(), rows()));
        for name in ["TestWhat1", "TestWhat2", "TestYes1"] {
            files.push((
                format!("Units\\Test\\{name}.wav"),
                format!("pcm:{name}").into_bytes(),
            ));
        }
        Arc::new(
            Resolver::new(
                pcx_assets::GameMode {
                    build: pcx_assets::Build::V1_29b,
                    addon: pcx_assets::Addon::Tft,
                    assets: pcx_assets::AssetMode::Vanilla,
                    behaviour: pcx_assets::Behaviour::Vanilla,
                },
                None,
                vec![pcx_assets::ArchiveLayer {
                    priority: 1,
                    source: Box::new(Memory {
                        info: pcx_assets::SourceInfo {
                            id: "music-fixture".into(),
                            kind: pcx_assets::LayerKind::Mpq,
                            origin: "music-fixture".into(),
                            version: Some("synthetic".into()),
                        },
                        files,
                    }),
                }],
            )
            .unwrap(),
        )
    }

    fn map(random: bool, index: i32) -> MapMusic {
        MapMusic {
            name: "music".into(),
            random,
            index,
        }
    }

    fn audio_with(bank: Bank) -> CampaignAudio {
        CampaignAudio {
            bank: Some(bank),
            ..default()
        }
    }

    #[test]
    fn playlist_is_the_race_section_entry_for_the_edition() {
        let human = bank(false, &["Human", "Default"]);
        assert_eq!(
            human.playlist("music").unwrap(),
            ["a.mp3", "b.mp3", "c.mp3"]
        );
        let expansion = bank(true, &["Human", "Default"]);
        assert_eq!(expansion.playlist("Music").unwrap(), ["x.mp3", "a.mp3"]);
        let orc = bank(true, &["Orc", "Default"]);
        assert_eq!(orc.playlist("music").unwrap(), ["o.mp3"]);
        assert_eq!(
            bank(true, &["Human", "Default"])
                .playlist("fallback")
                .unwrap(),
            ["d.mp3"]
        );
    }

    #[test]
    fn missing_or_duplicated_playlists_are_errors() {
        assert!(bank(false, &["Orc", "Default"]).playlist("music").is_err());
        let mut duplicated = bank(true, &["Human"]);
        duplicated.skin = SkinTable::parse("[Human]\nMusic_V1=a.mp3\nMusic_V1=b.mp3\n").unwrap();
        assert!(
            duplicated
                .playlist("music")
                .unwrap_err()
                .contains("more than once")
        );
        let mut empty = bank(true, &["Human"]);
        empty.skin = SkinTable::parse("[Human]\nMusic_V1=;\n").unwrap();
        assert!(empty.playlist("music").unwrap_err().contains("no track"));
    }

    #[test]
    fn indexed_lists_start_at_the_index_and_follow_list_order() {
        let mut rng = Rng(1);
        let tracks: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let mut list = Playlist::start(map(false, 1), tracks.clone(), &mut rng).unwrap();
        let order: Vec<_> = (0..4).map(|_| list.take(&mut rng)).collect();
        assert_eq!(order, ["b", "c", "a", "b"]);
        assert!(Playlist::start(map(false, 3), tracks.clone(), &mut rng).is_err());
        assert!(Playlist::start(map(false, -1), tracks, &mut rng).is_err());
    }

    #[test]
    fn random_lists_never_repeat_the_previous_track_and_are_seeded() {
        let tracks: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        let draw = |seed| {
            let mut rng = Rng(seed);
            let mut list = Playlist::start(map(true, 0), tracks.clone(), &mut rng).unwrap();
            (0..32).map(|_| list.take(&mut rng)).collect::<Vec<_>>()
        };
        let first = draw(9);
        assert_eq!(first, draw(9));
        assert!(first.windows(2).all(|pair| pair[0] != pair[1]));
        assert_ne!(first, draw(10));
        let mut rng = Rng(3);
        let mut single = Playlist::start(map(true, 0), vec!["only".into()], &mut rng).unwrap();
        assert_eq!(single.take(&mut rng), "only");
        assert_eq!(single.take(&mut rng), "only");
    }

    #[test]
    fn map_music_plays_the_playlist_and_advances_when_a_track_ends() {
        let mut audio = audio_with(bank(false, &["Human", "Default"]));
        let request = || Some(map(false, 0));
        assert_eq!(
            audio.step(None, request(), false),
            Ok(Step::Play("a.mp3".into()))
        );
        assert_eq!(audio.step(None, request(), true), Ok(Step::Keep));
        assert_eq!(
            audio.step(None, request(), false),
            Ok(Step::Play("b.mp3".into()))
        );
    }

    #[test]
    fn clearing_map_music_stops_it_and_a_new_selection_replaces_it() {
        let mut audio = audio_with(bank(false, &["Human", "Default"]));
        assert_eq!(
            audio.step(None, Some(map(false, 0)), false),
            Ok(Step::Play("a.mp3".into()))
        );
        assert_eq!(audio.step(None, None, true), Ok(Step::Stop));
        assert_eq!(audio.step(None, None, false), Ok(Step::Keep));
        assert_eq!(
            audio.step(None, Some(map(false, 2)), false),
            Ok(Step::Play("c.mp3".into()))
        );
        assert_eq!(audio.step(None, Some(map(false, 1)), true), Ok(Step::Stop));
        assert_eq!(
            audio.step(None, Some(map(false, 1)), false),
            Ok(Step::Play("b.mp3".into()))
        );
    }

    #[test]
    fn thematic_music_covers_map_music_then_hands_back_once_it_ends() {
        let mut audio = audio_with(bank(false, &["Human", "Default"]));
        let selection = || Some(map(false, 0));
        assert_eq!(
            audio.step(None, selection(), false),
            Ok(Step::Play("a.mp3".into()))
        );
        assert_eq!(audio.step(Some("t.mp3"), selection(), true), Ok(Step::Stop));
        assert_eq!(
            audio.step(Some("t.mp3"), selection(), false),
            Ok(Step::Play("t.mp3".into()))
        );
        assert_eq!(audio.step(Some("t.mp3"), selection(), true), Ok(Step::Keep));
        // The file ended: the map playlist resumes with its next track, and the file stays done.
        assert_eq!(
            audio.step(Some("t.mp3"), selection(), false),
            Ok(Step::Keep)
        );
        assert_eq!(
            audio.step(Some("t.mp3"), selection(), false),
            Ok(Step::Play("b.mp3".into()))
        );
        assert_eq!(audio.step(Some("t.mp3"), selection(), true), Ok(Step::Keep));
        // Ending and requesting the same file again replays it.
        assert_eq!(audio.step(None, selection(), true), Ok(Step::Keep));
        assert_eq!(audio.step(Some("t.mp3"), selection(), true), Ok(Step::Stop));
        assert_eq!(
            audio.step(Some("t.mp3"), selection(), false),
            Ok(Step::Play("t.mp3".into()))
        );
    }

    #[test]
    fn a_failed_request_is_reported_once_and_retried_only_after_it_changes() {
        let mut audio = audio_with(bank(false, &["Orc"]));
        let missing = || Some(map(false, 0));
        assert!(audio.step(None, missing(), false).is_err());
        assert_eq!(audio.step(None, missing(), false), Ok(Step::Keep));
        assert!(audio.step(None, Some(map(false, 1)), false).is_err());
    }

    #[test]
    fn music_before_the_sources_are_ready_is_an_error_not_silence() {
        let mut audio = CampaignAudio::default();
        assert!(audio.step(None, Some(map(true, 0)), false).is_err());
    }

    #[test]
    fn bank_reads_through_the_resolver_with_a_byte_bound() {
        let bank = bank(true, &["Human"]);
        assert_eq!(bank.read("a.mp3", 16).unwrap(), b"a.mp3");
        assert!(bank.read("a.mp3", 2).is_err());
        assert!(bank.read("missing.mp3", 16).is_err());
    }

    #[test]
    fn rows_resolve_files_volume_and_flags_from_the_table() {
        let mut bank = bank(true, &["Human"]);
        let what = bank.row(TABLE, "testwhat").unwrap();
        assert_eq!(
            what.members,
            ["Units\\Test\\TestWhat1.wav", "Units\\Test\\TestWhat2.wav"]
        );
        assert_eq!(what.volume, 1.0);
        assert_eq!(
            (what.pitch, what.pitch_variance, what.channel),
            (1.0, 0.01, 1)
        );
        assert_eq!(what.flags, ["WANT3D", "CHANNELFULLPREEMPT", "RANDOMPITCH"]);
        let yes = bank.row(TABLE, "TestYes").unwrap();
        assert_eq!(yes.members, ["Units\\Test\\TestYes1.wav"]);
        assert_eq!(yes.volume, 64.0 / 127.0);
        assert_eq!(yes.flags, ["NODUPLICATES"]);
    }

    #[test]
    fn unusable_rows_are_errors_naming_the_row() {
        let mut bank = bank(true, &["Human"]);
        for (name, reason) in [
            ("Missing", "not found"),
            ("Twice", "more than once"),
            ("Disabled", "no file"),
            ("TooLoud", "Volume"),
            ("BadPitch", "Pitch"),
        ] {
            let error = bank.row(TABLE, name).unwrap_err();
            assert!(error.contains(reason) && error.contains(name), "{error}");
        }
        assert!(bank.row("UI\\SoundInfo\\Absent.slk", "TestYes").is_err());
        bank.volume_maximum = Err("no range".into());
        assert_eq!(bank.row(TABLE, "TestYes").unwrap_err(), "no range");
    }

    #[test]
    fn sound_files_are_cached_within_the_bound() {
        let mut bank = bank(true, &["Human"]);
        let first = bank.sound_bytes("Units\\Test\\TestYes1.wav").unwrap();
        let again = bank.sound_bytes("units\\test\\testyes1.wav").unwrap();
        assert!(Arc::ptr_eq(&first, &again));
        assert_eq!(bank.sounds_bytes, first.len());
        bank.sounds_bytes = SOUND_CACHE_BYTES;
        bank.sound_bytes("Units\\Test\\TestWhat1.wav").unwrap();
        assert_eq!(bank.sounds.len(), 1);
    }

    fn world(seed: u64) -> World {
        let mut world = World::new();
        world.init_resource::<Assets<AudioSource>>();
        let mut audio = audio_with(bank(true, &["Human"]));
        audio.bank.as_mut().unwrap().rng = Rng(seed);
        world.insert_resource(audio);
        world
    }

    fn play(world: &mut World, name: &str) -> Result<(), String> {
        let name = name.to_owned();
        world
            .run_system_once(
                move |mut audio: ResMut<CampaignAudio>,
                      mut commands: Commands,
                      mut assets: ResMut<Assets<AudioSource>>,
                      live: Query<(Entity, &GameSound)>| {
                    audio.play(TABLE, &name, 0.5, &mut commands, &mut assets, &live)
                },
            )
            .unwrap()
    }

    fn voices(world: &mut World) -> Vec<(String, f32, f32)> {
        let mut query = world.query::<(&GameSound, &PlaybackSettings, &AuthoredVolume)>();
        let mut found: Vec<_> = query
            .iter(world)
            .map(|(sound, settings, authored)| {
                let bevy::audio::Volume::Linear(volume) = settings.volume else {
                    panic!("linear volume expected");
                };
                assert_eq!(
                    authored.0 * 0.5,
                    volume,
                    "start volume is gain times effects"
                );
                (sound.name.clone(), volume, settings.speed)
            })
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        found
    }

    #[test]
    fn a_row_becomes_one_audio_entity_with_its_source_bytes() {
        let mut world = world(1);
        play(&mut world, "TestYes").unwrap();
        let found = voices(&mut world);
        assert_eq!(found, [("TestYes".to_owned(), 64.0 / 127.0 * 0.5, 1.0)]);
        let mut players = world.query::<&AudioPlayer>();
        let handle = players.single(&world).unwrap().0.clone();
        let assets = world.resource::<Assets<AudioSource>>();
        assert_eq!(&*assets.get(&handle).unwrap().bytes, b"pcm:TestYes1");
    }

    #[test]
    fn random_pitch_stays_within_the_variance_and_follows_the_seed() {
        let mut speeds = Vec::new();
        for seed in [11, 11, 12] {
            let mut world = world(seed);
            play(&mut world, "TestWhat").unwrap();
            let found = voices(&mut world);
            let (_, _, speed) = found[0];
            assert!((0.99..=1.01).contains(&speed), "{speed}");
            speeds.push(speed);
        }
        assert_eq!(speeds[0], speeds[1]);
        assert_ne!(speeds[0], speeds[2]);
    }

    #[test]
    fn channel_preemption_cuts_the_previous_sound_on_the_channel() {
        let mut world = world(2);
        play(&mut world, "TestWhat").unwrap();
        play(&mut world, "TestYes").unwrap();
        assert_eq!(voices(&mut world).len(), 2, "other channels coexist");
        play(&mut world, "TestWhat").unwrap();
        let found = voices(&mut world);
        let names: Vec<_> = found.iter().map(|v| v.0.as_str()).collect();
        assert_eq!(names, ["TestWhat", "TestYes"]);
        assert_eq!(world.query::<&GameSound>().iter(&world).count(), 2);
    }

    #[test]
    fn a_pitch_variance_that_would_stop_playback_is_an_error() {
        let mut world = world(6);
        let error = play(&mut world, "WidePitch").unwrap_err();
        assert!(error.contains("PitchVariance"), "{error}");
        assert_eq!(voices(&mut world).len(), 0);
    }

    #[test]
    fn a_no_duplicates_row_does_not_start_while_audible() {
        let mut world = world(3);
        play(&mut world, "TestYes").unwrap();
        play(&mut world, "TestYes").unwrap();
        assert_eq!(voices(&mut world).len(), 1);
    }

    #[test]
    fn flags_that_are_not_applied_are_remembered_for_one_report() {
        let mut world = world(4);
        play(&mut world, "TestWhat").unwrap();
        play(&mut world, "TestWhat").unwrap();
        let audio = world.resource::<CampaignAudio>();
        assert_eq!(audio.unapplied_flags.iter().collect::<Vec<_>>(), ["WANT3D"]);
    }

    #[test]
    fn playing_before_the_sources_are_ready_is_an_error() {
        let mut world = World::new();
        world.init_resource::<Assets<AudioSource>>();
        world.init_resource::<CampaignAudio>();
        assert!(play(&mut world, "TestYes").is_err());
    }

    #[test]
    #[ignore = "requires PCX_NATIVE_CHAPTER_* selecting an installed chapter"]
    fn the_installed_sound_tables_resolve_row_by_row() {
        let mut app = crate::native_campaign::tests::started_app();
        app.init_resource::<crate::menu_settings::Backend>()
            .init_resource::<CampaignAudio>();
        app.world_mut()
            .run_system_once(
                |mut audio: ResMut<CampaignAudio>,
                 mut session: ResMut<MapScriptSession<MapStartupHost>>,
                 sources: Sources| audio.ensure_bank(&sources, &mut session),
            )
            .unwrap();
        let mut audio = app.world_mut().resource_mut::<CampaignAudio>();
        assert_eq!(audio.last_error, None);
        let bank = audio
            .bank
            .as_mut()
            .expect("the published session's sources");
        let mut failures = Vec::new();
        for table in [
            "UI\\SoundInfo\\UISounds.slk",
            "UI\\SoundInfo\\UnitAckSounds.slk",
        ] {
            bank.row(table, "probe").unwrap_err();
            let names: Vec<String> = bank.tables[table]
                .data_rows()
                .filter_map(|row| row.by_name("SoundName").map(str::to_owned))
                .collect();
            let mut files = 0;
            for name in &names {
                match bank.row(table, name) {
                    Ok(row) => {
                        for member in &row.members {
                            files += 1;
                            if let Err(error) = bank.sound_bytes(member) {
                                failures.push(error);
                            }
                        }
                    }
                    Err(error) => failures.push(error),
                }
            }
            println!("{table}: {} rows, {files} files", names.len());
        }
        for failure in failures.iter().take(40) {
            println!("unresolved: {failure}");
        }
        println!("{} unresolved in total", failures.len());
        for known in ["PeasantWhat", "FootmanYes", "ArthasYesAttack"] {
            bank.row("UI\\SoundInfo\\UnitAckSounds.slk", known).unwrap();
        }
        bank.row("UI\\SoundInfo\\UISounds.slk", "InterfaceError")
            .unwrap();
    }

    #[test]
    fn repeated_reports_are_logged_once_and_the_latest_is_kept() {
        let mut audio = CampaignAudio::default();
        audio.report("a".into());
        audio.report("a".into());
        audio.report("b".into());
        assert_eq!(audio.reported.len(), 2);
        assert_eq!(audio.last_error.as_deref(), Some("b"));
    }
}
