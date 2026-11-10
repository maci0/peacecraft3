# Subtitles option: what it gates

Image: Legacy `Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
VERSIONINFO 1.29.2.9231. No original client executed; static bounded reads
only (getter/writer call sites, descriptor table, native registration
table). File offsets below are VA−0x400000 mappings verified against
section headers (.text RVA 0x1000, .rdata RVA 0xa66000).

## Descriptor, read, default (established)

- Descriptor table base VA `0xe68be0` (via getter `0x42b5f0`:
  `shl index,4; [index*16+0xe68bdc]`). 80 rows, stride 16, word0 = name
  pointer, word1 = category, word3 = fallback pointer. Native push-index
  `n` corresponds to importer row `n−1` (row33 `subtitles`, fallback
  `b"0"`; row30 `ambient`, fallback `b"1"`; row28 `sfx`, fallback `b"1"`).
  Getter `0x42b5f0` returns the saved word if the preference record
  exists, else the fallback word via `0xa654c9` and stores it through
  `0x42ca10`.
- Options init `0x531260..0x531ad4`: native index 34 (0x22) is read via
  `0x42b5f0`, compared to 0, and applied to the `SubtitlesCheckBox`
  control (`0x531917`, checkbox delegates `0x878c80/0x878bf0/0x879f40/
  0x87f430`). Indices must be read as native push values: 0x22 = 34 is
  `subtitles`, not importer row 34 (`nomidi`).
- All native-index-34 getter sites in `.text` (bounded push-imm + call
  scan): `0x531917` (options init), `0x62b1e2` and `0x63a1a9` (apply-sync
  functions `0x62ae80` and the `0x63a000` block; checkbox sync only),
  `0x53f93e` (movie/caption config, below) and `0x5f32ea` (transmission
  display, below). No other site reads index 34. In particular the
  four-site ambient sync list does not include these two game consumers.

## Consumer 1: movie/caption config at `0x53f8d0` (established, not dialogue)

- `0x53f8d0` (one caller `0x53f890`, itself called from the movie
  setup path at `0x56ae45`/`0x56e61c`) reads native indices 34, 29 and
  0x11–0x15 (importer rows 33 `subtitles`, 28 `sfx`, 16–20
  `cinematicoverrides/cinematicrefresh/cinematicbpp/cinematicwidth/
  cinematicheight`) into stack slots, inverts the `sfx` word, then
  calls `0x7d0850` with `(esi, edi, [ebp-0x120] inverted-sfx,
  [ebp-0x11c] overrides)`.
- The subtitles word at `[ebp-0x124]` is tested at `0x53f9e0`
  (`cmp …,0; jne 0x53fa0c`). When zero and the `UndeadEd` string check
  at `0x53f9e9` passes, the third argument is `0` instead of `edi`;
  otherwise `edi` is passed. `0x53f670` formats `UI\Captions\%s.txt`
  and `0x53f810` formats `%sMovies/%s.mpq`, so this function is the
  movie caption file setup, not in-game transmission text. It does not
  hide/show `SetCinematicScene` text and does not gate audio.

## Consumer 2: transmission display at `0x5f3290` → `0x5f4930` (established)

- Native registration table (push sig, push name, push func,
  `call 0x8ccc80`): `SetCinematicScene` → `0x4a78a0`,
  `EndCinematicScene` → `0x48fdf0`, `ForceCinematicSubtitles` →
  `0x490770` (sig `(B)V`). `0x4a78a0` resolves its two string
  arguments (speaker, text) through `0x708dc0/0x708d70` then calls
  `0x5f3290`.
- `ForceCinematicSubtitles` impl `0x490770` stores its boolean argument
  into `[ecx+0x2cc]` of the object returned by `0x5ec5e0(0,1)` (the
  same object getter as `EndCinematicScene`). That field is the
  cinematic-subtitle override.
- `0x5f3290` (thiscall, `ecx` = that object): if `[esi+0x18c]` is 1 or
  3 it takes the alternate path `0x651190` (no subtitle read at all);
  otherwise it calls `0x64da40`, then loads `[esi+0x2cc]` (the Force
  override). If that is zero, it reads native index 34 (`subtitles`)
  into that slot at `0x5f32ea` (`push 0x22; call 0x42b5f0`). A nonzero
  Force value therefore takes precedence over the preference; the
  preference is only consulted when Force is zero (never set or set
  false). Callers of the display path `0x5f4930` that do not go
  through `0x5f3290` (e.g. `0x5f4923`, `0x62f0c5`) pass their own flag
  and do not read the preference.
- `0x5f4930` formats the speaker/text pair (`%s|c…%s: |r%s`) and, at
  `0x5f49e6`, tests the flag passed from `0x5f3290`
  (`cmp [ebp+0x18],0; je 0x5f4a0f`): when zero it skips the
  `call [eax+0x70]` on the `[esi+0x428]` frame that publishes the
  formatted string. The `[esi+0x27c]/0x6296c0` subtitle-size path
  runs either way. So the gate suppresses only that one frame
  publication of the formatted transmission string; it does not
  clear the scene, stop voice playback, or affect timed messages.
  The speaker line is part of the same formatted string, so it is
  suppressed with the text on this path; the portrait/border from
  `CinematicPanel.fdf` is a separate path (recreation) and is not
  covered by this evidence.
- Default/domain (established for the control and the consumer read):
  word ≠ 0 = shown, word = 0 = suppressed at `0x5f49e6`/`0x53f9e0`;
  fallback `b"0"` (off) from row33. The Options checkbox therefore
  defaults to off in this image. Per-frame translation to the
  recreation's `PresentationState::cinematic_subtitles: Option<bool>`
  (absent until a script sets it, see `slice-dialogue.md`) must
  keep `None` as "no Force override, use the preference", not as
  shown or hidden; `Some(false)` in the recreation is indistinguishable
  from never-set in the native `0x2cc` field (both are zero), so a
  script `ForceCinematicSubtitles(false)` cannot be distinguished
  from absent there. That is a recreation limitation, not native
  semantics.

## Map to the recreation live producer (what matches, what is missing)

- Live text producer: `campaign_messages::project` +
  `campaign_dialogue::DialogueAssets::paint` project executed
  `SetCinematicScene` (`CinematicScene { speaker, text, … }` from
  `cinematic_scene_host.rs`) onto the CinematicPanel frames. That is
  the only live transmission-text producer; `campaign_messages::paint`
  for `DisplayTimedTextToPlayer` is a separate producer and is not
  gated by this evidence.
- Live override holder: `PresentationState::cinematic_subtitles`
  (`presentation_host.rs`) stores the executed
  `ForceCinematicSubtitles` flag. It is default-`None` and is never
  read by any UI system (search: no reader outside the host and its
  test). The native `0x2cc` field and the recreation field are the
  same concept, but the recreation has no consumer combining it with
  the preference.
- Missing hook (exact): there is no applied-preference reader for
  `subtitles` (no `SubtitlesCheckBox` in `menu_settings.rs`
  `sound_preference`, no `subtitles_default` derivation, and the
  Options screen does not offer the control; `supported()` in
  `menu_screens.rs` does not include it), and `campaign_messages`/
  `campaign_dialogue` do not read `presentation().cinematic_subtitles`
  or any applied subtitles toggle. Gating the panel on the flag
  alone, or on a new storage-only toggle, would invent the
  combination the native code does in `0x5f3290` (Force-nonzero
  wins, else preference, else fallback `0`) and the
  `[esi+0x18c]` 1/3 alternate path that bypasses the gate entirely.
  The `[esi+0x18c]` mode has no identified recreation counterpart.
- Therefore no gate is implemented in this commit. The minimum
  faithful gate, once the missing hook exists, is: in
  `campaign_messages::project` (or `DialogueAssets::paint`), compute
  effective = Force-nonzero ? Force : applied-subtitles-preference
  (default from the selected client's descriptor fallback, `0` here,
  applied-only, draft never gates), and when effective is false
  suppress only the formatted speaker/text publication, keeping
  the scene, portrait/border, voice playback and timed messages.
  Do not gate `DisplayTimedTextToPlayer` text, script audio, or music.
  The movie-caption path (`0x53f8d0`/`UI\Captions\%s.txt`) has no
  recreation producer (no movie captions), so it needs no gate.

## Verification

Static only: bounded scans over `.text` for `push imm; call
0x42b5f0` sites, descriptor table read above, native registration
table read above, and disassembly of `0x42b5f0`, `0x53f8d0`,
`0x5f3290`, `0x5f4930`, `0x490770`, `0x4a78a0`. No runtime launch,
no writes to the preference store, no original bytes committed.
Existing test `cinematic_subtitles_retain_the_executed_flag_without_a_default`
(pcx-play) passes. Untracked probe
`crates/pcx-mpq/examples/pc05_list.rs` preserved untouched.
