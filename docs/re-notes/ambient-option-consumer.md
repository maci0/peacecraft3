# Ambient option: what it gates

Image: Legacy `Warcraft III.exe`, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
VERSIONINFO 1.29.2.9231. No original client executed; static
bounded reads only (getter/writer call sites, descriptor table,
channel/mask tables). File offsets below are VA−0x400000 mappings
verified against section headers.

## Descriptor, read, write (established)

- Descriptor table base VA `0xe68be0` (via getter `0x42b5f0`:
  `shl index,4; [index*16+0xe68bdc]` plus the `+4` field word the
  checked importer uses). 80 rows, stride 16, word0 = name pointer,
  word1 = category, word3 = fallback pointer. Native push-index
  `n` corresponds to importer row `n−1` (row30 `ambient`, fallback
  `b"1"`; row29 `sfxvolume`, fallback `b"100"`; row27
  `musicvolume`, fallback `b"40"`).
- Options init `0x531260..0x531ad4`: native index 31 (0x1f) is read
  via `0x42b5f0`, compared to 0, and applied to the `AmbientCheckBox`
  control (`[edi+0x28c]`, `0x878c80/0x878bf0/0x879f40/0x87f430`
  checkbox delegates). Indices 32/33/34 do the same for
  movement/unit/subtitles controls. Index 30 is the sfx-volume
  slider (`0x87f090`), index 29 the sfx checkbox.
- Options write `0x538440..0x53882f`: index 30 written from the
  sfx-volume slider float, index 31 written from the ambient
  checkbox checked-state (`0x878c80` → `0x42ca10`). A second
  in-game writer at `0x64bb00` writes the same index set from its
  own control block. `0x42ca10` stores the word in the selected
  preference record; no audio call follows.
- All native-index-31 getter sites in `.text` (bounded push-imm +
  call scan): `0x531837` (init), `0x531d63` (change-check vs
  control state, then `0x42aab0` dispatch refresh), `0x62b00a`
  and `0x63a0cc` (apply-sync functions `0x62ae80` and the
  `0x63a000` block; called from `0x63b002`/`0x63e424` dispatch
  stubs). At every site the only live effect is syncing the
  checkbox control with the preference word. No site passes the
  value to a sound function, volume-group call, or channel gain.
  `0x42aab0` is a category dispatch refresh (reads descriptor
  category word, calls string getter `0x42b590` for category 0 or
  word getter `0x42b5f0` then `0x42ab70` for category 1); it
  does not start audio.

## Producer/channel relationship (established vs inference)

- `ScriptAudioCategory` (recreation) is only a gain combiner:
  authored voice gain × user gain, with `volume_group_channel_gain`
  supplying the category scale and `GroupPolicy::unity` the clamp.
  It has no checkbox input and no enable/disable semantics.
- Native channel/mask evidence read from the same image:
  `GroupPolicy` reset window `0x5d99c5` (unity 1.0, duration
  2000 ms), mask load `0x5bfa64` → mask table VA `0x10dc6f0`
  = `[0x200,0x101e,0x20,0x800,0x100,0x80,0x400,0x4000]` (8 groups),
  clamp unity at `0xe69584` = 1.0. Voice channel table at
  `0xe8ff10` (16 channels, gains all 1.0 in this image). None of
  these tables is indexed by the ambient preference, and no
  ambient-index reader feeds any mask/channel/gain path above.
  Inference (not established): the ambient checkbox most plausibly
  gates the engine's ambient/movement producer (shipped
  `AmbienceSounds.slk`, 90 rows), which has no consumer in this
  recreation and no traced native producer function. That
  producer, its channel, and whether the checkbox mutes it,
  scales it, or suppresses its creation, remain unread.
- Contrast: subtitles (native index 34) *does* have game
  consumers outside Options (`0x53f93e`, `0x5f32ea` → `0x5f4930`
  display path). Ambient (31), movement (32) and unit (33) have
  only the four sync sites listed above.

## Next existing backend hook (exact, no new engine)

There is no existing recreation producer to gate. The concrete
missing consumer is: a Bevy consumer for (a) stacked ambient
requests (`pending_stacked_sound_requests` /
`take_stacked_sound_requests`, `stacked_sound_host.rs`) and/or
(b) `AmbienceSounds.slk` movement/doodad rows, neither of which
exists today. Until such a producer exists, gating anything
would invent a gate. Do not gate `ScriptAudioCategory` (it would
mute script dialogue/music/effects, which the option does not
cover) and do not add a second checkbox.

If a source-qualified producer is later found, the minimum gate
would be: read the selected client's `ambient` fallback/saved
word through the existing `Defaults` path (domain: word ≠ 0 =
enabled, init `cmp …,0` / `je`), default fallback `b"1"` (on),
and suppress only that producer's voice starts — with a producer
test. That domain/default is established for the *control*; its
application to a producer is inference until the producer is
identified.

## Verification

Static only: bounded scans over `.text` for `push imm; call
0x42b5f0/0x42ca10/0x42aab0` sites, descriptor/mask/channel table
reads above, and the existing checked importer. No runtime
launch, no writes to the preference store, no original bytes
committed. Untracked probe `crates/pcx-mpq/examples/pc05_list.rs`
preserved untouched.
