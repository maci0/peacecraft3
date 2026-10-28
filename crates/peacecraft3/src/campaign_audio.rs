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
use std::sync::{Arc, Weak};

const SKIN: &str = "UI\\war3skins.txt";
/// Application read bounds: the skin text limit matches the HUD, the track limit the menu.
const MAX_SKIN_BYTES: u64 = 1 << 20;
const MAX_TRACK_BYTES: u64 = 64 << 20;

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
}

impl Bank {
    fn read(&self, member: &str, limit: u64) -> Result<Vec<u8>, String> {
        self.reader
            .resolve_bounded(member, limit)
            .map(|asset| asset.bytes)
            .map_err(|error| format!("{member}: {error}"))
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
    pub last_error: Option<String>,
}

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
    })
}

#[derive(bevy::ecs::system::SystemParam)]
struct Sources<'w> {
    requests: Res<'w, MapRequests>,
    sources: Res<'w, SelectedMapSources>,
    seed: Res<'w, CampaignSeed>,
    settings: Res<'w, crate::menu_settings::Backend>,
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
    if audio.bank.is_none() && !audio.prepare_failed && (thematic.is_some() || map.is_some()) {
        let reader = published_reader(&sources.requests, &sources.sources, &session);
        let race = session
            .host()
            .map(|host| host.objects().local_race_preference_constant())
            .map(|race| race.map_err(|error| error.to_string()));
        if let (Some(reader), Some(race)) = (reader, race) {
            match prepare_bank(reader, race, sources.seed.prepared.unwrap_or_default()) {
                Ok(bank) => audio.bank = Some(bank),
                Err(error) => {
                    audio.prepare_failed = true;
                    report(&mut audio, error);
                }
            }
        }
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
                    report(&mut audio, error);
                }
            }
        }
        Err(error) => report(&mut audio, error),
    }
}

fn report(audio: &mut CampaignAudio, error: String) {
    if audio.last_error.as_deref() != Some(&error) {
        warn!("campaign music unavailable: {error}");
        audio.last_error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKIN_TEXT: &str = "[Human]\nMusic_V0=a.mp3;b.mp3;c.mp3\nMusic_V1=x.mp3;a.mp3\n[Orc]\nMusic_V1=o.mp3\n[Default]\nFallback_V1=d.mp3\n";

    fn bank(expansion: bool, sections: &[&str]) -> Bank {
        Bank {
            reader: synthetic_reader(),
            skin: SkinTable::parse(SKIN_TEXT).unwrap(),
            sections: sections.iter().map(|s| (*s).to_owned()).collect(),
            expansion,
            rng: Rng(7),
        }
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
        let files = ["a.mp3", "b.mp3", "c.mp3", "x.mp3", "o.mp3", "d.mp3"]
            .iter()
            .map(|name| ((*name).to_owned(), name.as_bytes().to_vec()))
            .collect();
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
}
