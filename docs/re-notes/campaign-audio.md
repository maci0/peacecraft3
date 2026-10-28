# Campaign in-game audio

Report: "there is no audio ingame, just in the intro cinematics". This note lists
which sound categories reach a Bevy audio entity during campaign play, with the
evidence class for each row. Evidence classes: shipped bytes (original data read
from the supplied install), native code (static reading of the client or
`Blizzard.j`/`common.j`), runtime observation (the headless probe below),
inference (a recreation choice with no direct evidence).

## Method

`campaign_audio_probe::human01_opening_audio_reaches_entities`
(`crates/peacecraft3/src/campaign_audio_probe.rs`) starts the unchanged Human01
chapter from the supplied Legacy 1.29 install through the production startup, drives
240 s of simulation, runs `script_audio` each second and prints the Host's retained
music state and the voices that became `AudioPlayer` entities. It adds no audio
plugin, so no device is opened. The run uses a private `XDG_DATA_HOME`, no
`DISPLAY`/`WAYLAND_DISPLAY`, an unreachable `PULSE_SERVER` and an ALSA null device
(`.scratch/runtest.sh`).

`AudioPlayer` creation sites in production code: `menu_sprites.rs` (hanging panel
events), `intro.rs` (movie sound), `menu_audio.rs` (menu music, click) and
`script_audio.rs` (script voices). There is no other site.

## Findings before the fix

| Category | Reaches an audio entity in campaign play | Evidence | Gap |
|---|---|---|---|
| Script voices (`CreateSound*` + `StartSound`: dialogue VO, `Hint`, `QuestNew`, `Rescue`) | yes | runtime: 16 voice starts over 14 members in Human01 (11 `H01Arthas*`/`H01Uther*` mp3, 3 `Sound\Interface` wav); code `script_audio.rs` spawn, installed at `launch_gui.rs` | refusals below |
| Day ambient (`SetAmbientDaySound` -> `CreateMIDISound`) | no | runtime: `sound 1872: CreateMIDISound records no looping or spatial input` | refused before any playback |
| 3D positioned script sounds | no | runtime: two sounds refused in Human01; code `creation_block` | native attenuation unmeasured |
| Script sounds with an EAX string, `stop when out of range`, fade/duration overrides | no | code `creation_block`; not hit by Human01 | refused outright |
| Stacked ambient sounds (`RegisterStackedSound`) | no | code: requests are queued by `stacked_sound_host.rs`, no consumer in `peacecraft3`; not issued by Human01 in the probe | no consumer |
| Map music (`SetMapMusic`, `ClearMapMusic`) | no | runtime: Host retains `("music", random=false, index=0)`; code `map_music_host.rs` stores only | no consumer, no playlist resolver |
| Thematic music (`PlayThematicMusic`/`EndThematicMusic`) | no | runtime: `Sound\Music\mp3Music\Comradeship.mp3` retained during the opening, cleared after the dialogue | no consumer |
| Other music natives (`PlayMusic`, `PlayMusicEx`, `StopMusic`, `ResumeMusic`, `SetMusicVolume`, `PlayThematicMusicEx`, `Set*PlayPosition`) | no | native code: declared at `common.j` 2295-2306; no Host handler in `pcx-play` | not implemented |
| Menu music and click | menu only | `menu_audio.rs`; commit 3311cd7 pauses menu music while the campaign camera is active | by design |
| Unit selection and acknowledgement | no | code: no spawn site; shipped bytes: `UnitUI.slk` `unitSound` plus 1155 `UnitAckSounds.slk` rows named `<unitSound>What/Pissed/Yes/YesAttack/Ready/Warcry` | no consumer |
| UI click, error, quest, alert and race voices (`NoGold`, `UnderAttack`, ...) | only the three the map scripts start | shipped bytes: `UISounds.slk` 134 rows; `war3skins.txt` `[Human]` etc. map `NoGoldSound=NoGoldHuman` | the order path returns a boolean, so no refusal reason exists to voice; attack events are not observed |
| Unit combat, birth, death, footsteps (`SNDX` model events) | no | shipped bytes: `AnimLookups.slk` 613, `AnimSounds.slk` 593, `UnitCombatSounds.slk` 60 rows; code: campaign actors do not read `EVTS` | no consumer |
| Ability and spell sounds | no | shipped bytes: `AbilitySounds.slk` 57 rows | no consumer |
| Unit movement and doodad ambience | no | shipped bytes: `AmbienceSounds.slk` 90 rows | no consumer |
| EAX environment (`NewSoundEnvironment`) | modelled, no effect | code: `sound_environment_host.rs`; the audio backend has no reverb | no backend |

Root cause of the report: the only campaign audio the recreation produces is
script-started voices. Music, unit voices, interface sounds and everything
animation-driven have no consumer, so a map that starts no dialogue is silent.

## Source facts used by the fixes

- `UI\war3skins.txt` race sections carry playlists `Music_V0`/`Music_V1`
  (`;`-separated paths; `V0` Reign of Chaos, `V1` Frozen Throne, the same suffix the
  menu uses for `GlueMusic_V*`) and the race voice keys (`NoGoldSound`,
  `UnderAttackSound`, ...).
- `UI\SoundInfo\MiscData.txt` `[Sound]`: `Attenuation=3.0`, `MinDist=1200`,
  `MaxDist=20000`, `MusicFadeRate=16`, `MusicFadeInterval=500`; `[Listener]`
  distance tables.
- All `UI\SoundInfo` tables share the columns `SoundName`, `FileNames`
  (comma-separated list), `DirectoryBase`, `Volume`, `Pitch`, `PitchVariance`,
  `Priority`, `Channel`, `Flags`, `MinDistance`, `MaxDistance`, `DistanceCutoff`,
  `EAXFlags`.
- Archive members decode to PCM WAV or MP3 through the existing resolver; sampled
  `Units\Human\Arthas\ArthasWhat1.wav` and `Sound\Interface\Error.wav` are 16-bit
  mono 22050 Hz PCM.

## Music (`campaign_audio.rs`)

`drive_music` reads the Host's `thematic_music()` and `map_music_selection()` each
frame and keeps at most one `CampaignMusic` track entity.

- `PlayThematicMusic(path)`: the path is read from the session resolver and played
  once. While it is requested it covers map music; when the file ends the map
  playlist resumes at its next track, and the file is not restarted until
  `EndThematicMusic` clears the request. Playing once and resuming the map list are
  inferences; the native thematic path is not read.
- `SetMapMusic(name, random, index)`: the playlist is the skin key
  `<name>_V<edition>` in the local player's race section, then `[Default]`
  (`V0` for the Reign of Chaos mode, `V1` for the Frozen Throne mode, the suffix
  the menu already uses). The race section is the one the HUD uses
  (`RACE_PREF_*` of the local player). An indexed request starts at `index` and
  follows list order; a random request draws, never repeating the previous track,
  from a splitmix64 stream seeded with the campaign seed. A missing key, a
  repeated key, an empty list or an index outside the list is a loud error, not
  silence. The key convention and the list-order/draw policy are inferences: the
  native resolver was not read. `ClearMapMusic` stops the track.
- Volume is the user's music setting (`CampaignMusic` shares the menu music bus in
  `apply_user_volume`) at authored gain 1.0. `MusicFadeRate`/`MusicFadeInterval` from
  `MiscData.txt` are not applied: tracks start and stop instantly.
- Not implemented: `PlayMusic`, `PlayMusicEx`, `StopMusic`, `ResumeMusic`,
  `SetMusicVolume`, `PlayThematicMusicEx`, `SetMusicPlayPosition`,
  `SetThematicMusicPlayPosition` (no Host handler).

Verification: ten synthetic unit tests (`campaign_audio::tests`: skin lookup,
edition suffix, duplicates, playlist order, seeded draws, thematic cover and
hand-back, clear, failure latch, bounded reads) and the Human01 probe
(`human01_opening_audio_reaches_entities`), which asserts that the retained
`Comradeship.mp3` request becomes a `CampaignMusic` entity at once, that the
`[Human]` `Music_V0` entry `Human1.mp3` follows when the dialogue ends, and that no
second track overlaps. Entities are checked, not a device.

## Sound rows (`campaign_audio.rs`) and unit voices (`campaign_ack.rs`)

`CampaignAudio::play` starts one named row of a `UI\SoundInfo` table through the
session resolver. Columns come from the table: `FileNames` (one alternative drawn
per start from the seeded stream, `_` placeholders skipped), `DirectoryBase`
(exactly one separator inserted), `Volume` divided by the script's sound range
(`SetSoundVolumeBJ`, the range `script_audio` and the menu already use), `Pitch`,
`PitchVariance`, `Channel`, `Flags`. A row that is missing, defined twice,
list-less, or outside the range is an error naming the row. Voices run at
`Volume` times the user's effects setting (`AuthoredVolume` on the existing
effects bus) and end with their sound.

Flags applied: `CHANNELFULLPREEMPT` (ends the other game sounds on the row's
channel; this is the stated meaning of the flag name and the script mixer's
channel rule, not a read of the player), `NODUPLICATES` (skip while the row is
audible), `RANDOMPITCH` (speed is `Pitch` plus a draw within `PitchVariance`; the
band shape is an inference). Every other flag (`WANT3D`, `NODUPEUSERNAMES`,
`DYNAMICOCCLUSION`, ...) is logged once and not applied: voices are not
positioned or attenuated.

Unit voices: a `Select` command plays `<unitSound>What`, a move/smart/patrol order
`<unitSound>Yes`, an attack order or a smart order on an enemy
`<unitSound>YesAttack`, where `unitSound` is the unit's UnitUI cell. The suffix
names are shipped bytes (`PeasantWhat`, `FootmanYes`, `ArthasYesAttack`, ...); which
command plays which is an inference. Only the local player's units speak, one
voice per batch of commands. Not implemented: `Pissed` (repeat-click count
unknown), `Ready` (no production-complete event is observed), `Warcry`, and
acknowledgement of spell orders.

Data check (`the_installed_sound_tables_resolve_row_by_row`, Legacy 1.29 install):
all 134 `UISounds.slk` rows and all 1155 `UnitAckSounds.slk` rows parse; 3114 of
3149 ack files are present. The 35 absent entries are named by the table but not
shipped (all `HeroRanger*` rows, `MoonPriestessWarcry1`, `DreadLordMorphed*`,
`IceTrollPissed5`, `OrcLumberMill`, `PigFarmWhat1`, `DraeneiHutWhat1`,
`SacrificialAltarWhat1`, and two malformed paths). Picking one of those
alternatives is reported once and plays nothing.

Verification: `campaign_audio::tests` use a synthetic archive and table (row
parsing, errors, volume/pitch/seed, preemption, duplicates, flag reports, cache
bound) and `campaign_ack::tests` cover the command-to-row mapping. The Human01
probe selects Arthas and orders a move through the command log and asserts the
`ArthasWhat` and `ArthasYes` rows become entities with their source bytes.

## Status

| Category | State |
|---|---|
| Map and thematic music | connected (music section) |
| Unit selection and order acknowledgement | connected for `What`, `Yes`, `YesAttack` (unit voices section) |
