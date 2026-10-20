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
