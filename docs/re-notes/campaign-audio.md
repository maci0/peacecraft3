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

## Status

Updated per commit below.
