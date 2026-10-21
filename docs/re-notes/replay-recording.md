# Replay recording

Status: the native replay container and session recorder are implemented in
`crates/pcx-net`. The original Warcraft III W3G reader and a bounded writer are
implemented for the admitted structures. Header and block checksums are verified
and regenerated. Per-slot values and the encoded startup tail remain uninterpreted;
original-client playback and production recording are not qualified.

## Native container

`pcx_net::replay` (`crates/pcx-net/src/replay.rs`) stores what the sim consumed,
not a derived summary. Layout, little endian:

| Field | Bytes | Notes |
|-------|-------|-------|
| magic | 8 | `PCX3REPL` |
| format version | 1 | `1`; playback refuses other values |
| seed | 8 | the `Session` seed, same `pcx_sim::Rng::new` input |
| map SHA-1 | 20 | supplied by the caller; not checked against map bytes |
| start snapshot | 4 + n | `encode_world_payload` at tick 0 |
| tick count | 8 | |
| per tick | 1 + s + 1 + entries | `s` disconnect-stopped players, then per input: player, u32 length, `encode_input_payload` bytes |
| body checksum | 20 | SHA-1 of every preceding byte; checked before parsing |
| final digest | 20 | SHA-1 over final tick (u64) and `encode_world_payload`; checked after playback |

Playback verifies the body checksum, reseeds the RNG, applies each tick's disconnect stops
(`stop_owned_moves`, the same call `Session::advance` makes), steps the sim,
and compares the final digest. Any changed byte is refused: the body checksum catches it before parsing.
The map SHA-1 is returned for the caller to compare with the loaded map.

The disconnect stop is recorded because it is not a sim command: it clears move
targets of every unit the leaving player owns, whatever its selection.

## Limits

- Recording starts only at tick 0 (`Session::start_recording`). The RNG state
  after earlier ticks is not in the snapshot, so playback would diverge.
- Admitted movement (`World::movement`) is not stored. The caller passes the
  same admission to `play`; the final digest refuses a different one. A replay
  with movement played with `None` is therefore `Divergence`, not a silent
  change in behavior.
- The map SHA-1 is caller-supplied. Computing it from the loaded map bytes is
  the caller's job.
- Build identity is the format version plus the snapshot payload version. No
  game build identity is written, because the native replay only replays the
  recreation's own sim.

## Gates

- `crates/pcx-net/src/replay.rs` unit tests: coordinate bounds, player range,
  repeated player, tampered digest.
- `crates/pcx-net/tests/replay_roundtrip.rs`: live session with joins, leaves,
  a disconnect stop, and a select/stop; every truncation; trailing bytes; bad
  magic; unknown version; every single-byte flip outside the identity bytes.

No original asset or original replay is read by these tests.

## Initial W3G writer blocker

At this checkpoint no `.w3g` sample was available locally. A file search for `*.w3g` and `*.w3x`
under `wc3/`, the main checkout's `.scratch/`, and the Legacy install found only
`.w3x` maps, and the install has no `Replays` directory. The repo has no W3G header or block layout evidence
either. A writer built from memory would invent header fields, zlib block
framing, and player record layout, so it is not written.

To unblock: a Legacy 1.29b replay (`.w3g`) recorded from the selected executable
(SHA-256 `3f2ed012...570eed`), plus a documented decoder for its header and
blocks checked against that file. The writer then needs a round-trip gate over
that original sample.

## Integration

The Save Replay control in the score screen is owned by the integrator. It needs:

1. `Session::start_recording(map_sha1)` at tick 0 for the match.
2. `Session::stop_recording()` at match end; `None` means nothing was recorded,
   and the control must refuse, not report success.
3. A save path for the returned bytes, and `replay::play` to verify a file
   before reporting it saved.

## Initial gameplay replay gap

The production campaign does not run `pcx_sim::World`. Its state is the
script host and object model (`crates/peacecraft3/src/map_script_session.rs`,
`advance_unit_activity`). `Session` records only the harness sim, so it cannot
replay real campaign play. Capturing production play needs the host's
authoritative command and tick stream, its RNG seed and initial world, and the
map identity. The later production command-log section adds part of this boundary; a complete
production replay writer and playback remain open.

`crates/pcx-net/src/classic_frame/action.rs` decodes W3GS action records
(player id, opaque payload). It is reusable for framing action blocks, but it
does not decode orders, so it does not supply the gameplay command stream.

`tools/original-game-reference/replay.py` replays a scripted input journal
against the original and the browser. It is a test driver, not a game replay.

## Original sample: evidence and runbook

Shipped bytes of `Warcraft III.exe` (SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`) contain
`Warcraft III recorded game`, `Replay/LastReplay.w3g`, `TempReplay.w3g`,
`%s%s.w3g`, `%s_%s_replay`, `SaveReplayPanel`, `SaveReplayFileEditBox`, and
`ScoreScreenSaveReplayButton`. These show the file name and save UI. They do
not show that a match writes the file, or the file layout.

Runbook (not run; needs a root GUI slot):

1. `docker build -t peacecraft-original-reference:local tools/original-game-reference`
2. `reference.py prepare --source '<Legacy tree>' --run-dir .scratch/replay-sample-1`
3. `reference.py launch --run-dir .scratch/replay-sample-1 --width 1280 --height 960 --dpi 96`
4. Drive a custom single-player match with `key`/`click`. Capture each step, and
   finish with Save Replay on the score screen if it is offered.
5. The Wine prefix exists only inside the container (`/run/reference/prefix`).
   Copy `Replay/LastReplay.w3g` out with a new `export-replay` subcommand. The
   harness has no copy-out command today, so this must be added first.
6. Record the file's SHA-256, the executable hash, the map file hash, and the
   action log in the run receipt.

## Integrator API

- `Session::start_recording(map_sha1: [u8; 20]) -> Result<(), SessionError>`,
  at tick 0, before the first `advance`.
- `Session::stop_recording() -> Option<Vec<u8>>`. `None` means nothing was
  recording. The Save control must refuse, not report success.
- `pcx_net::replay::play(bytes, movement: Option<Arc<AdmittedMovement>>) -> Result<Replay, ReplayError>`.
  Verify a saved file with it before reporting success.

## Evidence still needed

- A recorded `LastReplay.w3g` from the selected executable, and its layout
  checked against that file.
- The production command and tick stream, with seed and initial-world capture,
  so the Save control records real campaign play.

## Production command log

`pcx_play::player_command_log::PlayerCommandLog` holds the accepted player
commands of the prepared campaign session. It is data only.

- Recording point: once per command, after the authoritative native accepts it
  (`crates/peacecraft3/src/campaign_input.rs`). Selection records after
  `select_units`. Orders record per unit after `IssueTargetOrder` or
  `IssuePointOrder` returns `Ok`. End-of-cinematic records after
  `fire_local_player_event` returns `Ok`. Script-issued natives are not recorded.
- Fault model: the first record that cannot be kept (missing start, missing or
  non-finite clock, clock running backwards, entry bound reached) faults the log.
  The fault is sticky, the game keeps running, and `last_error` shows the fault.
  The log records nothing after a fault. Commands accepted after a fault are
  unrecorded, so the log can never be saved as a replay.
- Bound: `MAX_LOG_ENTRIES` entries per session.
- Start identity: `StartIdentity::from_context` reads the admitted
  `ParsedMapContext`. It carries the seed, the map label, the SHA-256 of the
  admitted map bytes (`map_sha256`), and the existing `sources` identities for
  the map, common, Blizzard and catalog archives. The FNV-1a `map_digest` is a
  reproducibility checksum only and is not used as identity.
- Save readiness: `save_readiness()` refuses with `Faulted`, `NoStart`, or
  `NoInitialSnapshot`. The last is always returned for now.

Not captured: the initial state a replay starts from. Deterministic playback
from map init needs the campaign VM, globals, object and timer state at the
first accepted command, and the seed-driven RNG state. The native `Session`
container does not carry any of these, so it cannot substitute. Until the
snapshot exists, Save must refuse with `NoInitialSnapshot`.

## Original sample qualification (2026-10-20)

Source: `Warcraft III.exe` Legacy 1.29.2.9231 (SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`), custom
1v1 on Booty Bay, player `replay1` (Random race, Team 1) against Computer
(Normal) (Random, Team 2), 100% handicap. Match ended by F10 menu, End Game,
Quit Mission. The game wrote `Replay/LastReplay.w3g` at match end, before any
Save Replay click. Sample: 6041 bytes, SHA-256
`7055a5ac8ef45a8e176d7bb7d9abea9baac597761b47cbfec4e94bd57574f5a7`. Export
receipt: `.scratch/replay-sample-1/replays/match-1.json` (local, not committed).

Verified against the bytes:

- Bytes 0–27: `Warcraft III recorded game` `1a 00`.
- Header size 68 (`0x44`). Compressed-file size 6041 equals the file length.
- Header version 1. Decompressed size 10570. Two blocks.
- Bytes 48–51: `PX3W`. Game version 29 (1.29). Build 6060. Flags `0x0000`.
- Replay length 73625 ms, which matches the score screen's 0:01:13.
- Block framing: u16 compressed size, u16 uncompressed size (8192), u32 field,
  then a zlib stream beginning `78 01`. Block 0 is at 68 with compressed size
  4560. Block 1 is at 4636 with compressed size 1397. The file ends at 6041.
- Each block inflates to 8192 bytes. The content ends at 10570 across both
  blocks. The rest is zero padding, so the header's decompressed size is the
  real content length.
- Records are continuous across the block boundary. Block 0 ends with a
  `1f 02 00 …` time-slot record, and block 1 starts mid-record.
- Inflated block 0 starts `10 01 00 00 00 01 "replay1" 00 01 00 "Local Game" 00 …`.
  This is observed, not yet decoded against a record table.

At initial capture, not yet established (later results below supersede this list):

- The block u32 field (`7f65725c` in block 0). It matches none of adler32 or
  crc32 over the inflated or the compressed bytes (`.scratch/w3g_checksum.py`).
- Header bytes 64–67 (`3d d5 55 eb`). Meaning unknown.
- Action payload meaning (`w3g_actions.txt` is not in this repository).
- Whether a writer can reproduce the unknown fields.

At that checkpoint the W3G writer remained unimplemented. The later structural
reader/writer and checksum qualification below supersede that blocker; native
playback fidelity remains open.

## W3G reader (structure only)

`crates/pcx-net/src/w3g.rs` reads the layout from the public notes
`scopatz/w3g` `w3g_format.txt` (CC0-1.0). Provenance: fetched on 2026-10-21
from `https://github.com/scopatz/w3g/blob/master/w3g_format.txt`; document version
1.18, dated 2007-06-26, authors blue and nagger (deepnode.de / samods forum).
The preserved copy `w3g_format_reference.txt` matches the fetched source. The
reader decodes the header, zlib blocks, startup records, and replay data blocks
0x17, 0x1E/0x1F, 0x22, 0x2F. Every item is read by its id and length. Shapes
outside the notes are refused, not skipped: ladder player records, chat 0x20,
block 0x23, leave reasons outside 0x01/0x0C/0x0E, and any startup value that
differs from the notes.

Verified on the Legacy 1.29.2.9231 sample:

- Header CRC32 over the 68 bytes with its own field zeroed equals the stored
  value. The reader checks it.
- Startup: `0x110`, host record (`replay1`, custom), game name `Local Game`,
  encoded string, PlayerCount 24, GameType 9, LanguageID 0, no player list,
  GameStart record (2 slots, seed `0x04b3ff52`, select mode 0, 2 start spots),
  blocks `1a`, `1b`, `1c`. The replay data begins at 156. The reader derives this
  from the decoded prefix, not from a stored offset.
- Replay data: 939 time slots, 939 checksums, one countdown-free leave record
  (reason 12, player 1, result 7, trailer 1). The walk ends exactly at the
  content end.

Deviation from the notes: the first encoded string is the map path
(`Maps/(2)BootyBay.w3m`), not a bare map name. After the creator and an empty
string come 20 bytes the notes do not describe. They are not the map's SHA-1
(checked against the installed map file). They contain zero bytes. The reader
keeps them raw as `encoded_tail`. The field is null-delimited, so this cannot
move the record boundaries.

Block checksum qualification (2026-10-21): the u32 at block offset 4 consists of
two little-endian u16 checksums. Each folds an IEEE CRC32 with
`(crc ^ (crc >> 16)) & 0xffff`. The first covers the eight-byte block header with
both checksums zeroed; the second covers the compressed payload. An independent
Python zlib calculation matches both fields of the retained original sample.
The reader checks both before inflation, and the writer recomputes both after
compression. Format references are the
[gowarcraft3 compressor](https://github.com/nielsAD/gowarcraft3/blob/master/file/w3g/compress.go)
and the independent
[ReplayMap-Fixer implementation](https://github.com/l-Fingon-l/ReplayMap-Fixer/blob/main/replay.cpp).
No source implementation was copied.

Both original blocks yield their declared output using sync-flush decoding and
do not reach zlib end-of-stream. The writer preserves this framing; lack of a
final trailer is not by itself corruption in this format.

Not established, kept raw:
- The u32 value in each checksum record (`0x22`). The sample has 939 such values,
  all distinct-looking (935 unique) and without a derivable relation to the seed.
- `encoded_tail` (20 bytes).
- The game-settings bit meanings and the 4-byte map checksum inside them.

The meanings of the per-slot values and encoded tail are unresolved. Their
variation is not evidence that they are nondeterministic.

### Bounded writer

`crates/pcx-net/src/w3g.rs::write` serializes a parsed `Replay` back to `.w3g`
bytes. It recomputes header and block checksums and compresses the authoritative
`content` bytes. Parsed startup/record fields must agree with those bytes or the
writer refuses instead of silently losing an edit. Per-slot values and the
startup tail remain unchanged in the content. Recompression can change the block
checksums. Length conversions are checked, and output allocations refuse capacity
failure. The writer's block buffer size is an encoding choice, not a game rule.
The written file is not yet proven playable by the original client.

The W3G header records game version and build, not the executable's cryptographic
identity. The game-start record carries a seed, map path and a four-byte checksum;
these do not replace the production SHA-256 source identities. Per-slot values
cannot yet be generated from the recreation's authoritative state, so this writer
does not provide production campaign recording or command-only replay fidelity.

Structural proof, not fidelity. A parsed replay does not show that the game
reproduces it. The writer carries the opaque fields; it does not claim to match
the original client's sync algorithm.

Gates: `cargo test -p pcx-net --lib w3g` covers checksum regeneration without
source fields, contradictory parsed-view refusal, recompressed multiple blocks,
every single-byte block/header corruption, truncations, record values, padding
and the structural parser cases. Local original gate:
`PCX_W3G_SAMPLE=<path> cargo test -p pcx-net --test w3g_sample -- --ignored`.
The sample is never committed.
