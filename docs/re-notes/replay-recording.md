# Replay recording

Status: the native replay container and session recorder are implemented in
`crates/pcx-net`. The original Warcraft III W3G writer is not implemented.

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

## W3G writer: blocked

No `.w3g` sample is available locally. A file search for `*.w3g` and `*.w3x`
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

## Gameplay replay gap

The production campaign does not run `pcx_sim::World`. Its state is the
script host and object model (`crates/peacecraft3/src/map_script_session.rs`,
`advance_unit_activity`). `Session` records only the harness sim, so it cannot
replay real campaign play. Capturing production play needs the host's
authoritative command and tick stream, its RNG seed and initial world, and the
map identity. None of those is exported yet.

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
