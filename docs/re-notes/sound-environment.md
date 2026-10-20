# Sound environments (`NewSoundEnvironment`)

`ObjectHost::with_sound_environments(SoundEnvironmentInput)` admits the native from the
loaded declaration (`takes string environmentName returns nothing`). The input holds the
records from every `UI\SoundInfo` table with an `EnvironmentType` header
(`SoundSources::read`, `crates/peacecraft3/src/campaign_assets.rs`), the executable
tables (`pcx_play::sound_environment::tables`), and the window in milliseconds
(`NATIVE_WINDOW_MS`). Admission applies every record against the tables, so a record
that cannot be written (fractional int cell, missing row) refuses admission. Without
admission the native stays an explicit refusal.

## Host state

The host follows the native's control flow. Times are game seconds
(`timer_game_time`, zero when no timer clock exists). The executable's scale constant
`1000.0` (`0xe78350`) converts the window from milliseconds.

- A call at time `t` first runs any callback due by `t`. Then:
  - `in_window` is true when `last_change` is set and `t <= last_change + window`.
  - When a callback is pending or `in_window`, the name is buffered.
  - `in_window`: a callback is queued at `t + window` and the pending flag is set. The
    outcome is `Scheduled { fire_at }`. `last_change` is not updated.
  - Otherwise the named record is applied now and `last_change = t`.
- A callback at its time `f` clears the pending flag, applies the buffered name, and
  sets `last_change = f`. Queued callbacks each run. The first one clears the flag even
  while another is queued.
- An unknown name applies nothing, but still sets `last_change` to the call time. The
  outcome is `NoRecord`.
- An applied record's outcome depends on the provider state (`ProviderState`):
  - `Unqualified` (no report yet): `Validated(values)`. Nothing is sent.
  - `Closed`: `ProviderClosed(values)`. The native skips every provider call.
  - `Open(id)`: `Delivered { provider, values }`. The effect-volume global and the
    provider writes are sent. The room write follows the room rule below.
- `sound_environment()` is the record the last known-name lookup selected.
  `sound_environment_delivery()` is the latest outcome, scheduled or applied.
  `sound_environment_applied()` is `Some` only for `Delivered`.
- `advance_sound_environments()` runs once per unit-activity tick
  (`unit_activity_host.rs`). Each call also runs callbacks due by the current time.

A new host starts with no state, which is the session-replacement path. No reset path
is decoded. A provider change does not re-deliver an earlier record. Whether the native
re-applies on reopen is not established.

## Native path (static, `Warcraft III.exe`, sha256 `3f2ed012…570eed`)

Decoded with bounded capstone scripts (no execution). Output lives under
`.scratch/sound-env-static/`.

| Address | Role |
|---|---|
| `0x4a39c0` | Native body. Passes its argument through `0x469010` and tail-jumps to `0x5d83e0`. |
| `0x5d83e0` | Shared body: window test against `0x113d628` plus `0x10dc6bc`, `GetTickCount` (IAT `0xe661a8`), buffer copy into `0x113cd98`, scheduling through `0x840370`, direct apply. |
| `0x5be250` | Callback: clears `0x113d624`, looks up the buffered name, applies, sets the tick. |
| `0x5cf8e0` | Name lookup in table `0x113d504`. The table is filled from parsed `EnvironmentSounds` rows by `0x5cd2d0`. |
| `0x5caf40` (record builder) | Reads 14 reverb cells as float and int, then the room name. The room index is the name's position in the 26-entry array at `0xe900e0`, or `-1` when absent (loop bound `cmp esi, 0x1a`). |
| `0x7a0090` | When provider `0x1140650` is open and `0x1140aa4` bit 0 is clear: `AIL_set_3D_room_type` (IAT `0xe66904`). Index `-1` fails `cmp eax, 0x1a; jae` and skips the call. |
| `0x79ff90` | Returns at once when no provider is open. Otherwise clamps each item to its row range. Index 0 writes the global `0x10dd990`; the others call `AIL_set_3D_provider_preference` (IAT `0xe66908`) with the name from `0x10dd9f0`. |
| `0x5d6f70` | Scheduler setter: stores its first argument into `0x10dc6bc`. Callers at `0x42eea5` and `0x534cc9` pass `0x1388` (5000 ms). |

The room names are the executable's own strings, `GENERIC` through `PSYCHOTIC`. One
spelling is `STONEOOM` (index 5), as the image contains it. The pointer array is found
from the unique `GENERIC` string and its unique pointer.

The property rows, read from the image:

| Index | Name | Kind | Range |
|---|---|---|---|
| 0 | EAX effect volume | float | 0 .. 1 |
| 1 | EAX decay time | float | 0.1 .. 20 |
| 2 | EAX damping | float | 0 .. 2 |
| 3 | EAX environment size | float | 1 .. 100 |
| 4 | EAX2 environment diffusion | float | 0 .. 1 |
| 5 | EAX2 room | int | -10000 .. 0 |
| 6 | EAX2 room HF | int | -10000 .. 0 |
| 7 | EAX2 decay HF Ratio | float | 0.1 .. 20 |
| 8 | EAX2 reflections | int | -10000 .. 1000 |
| 9 | EAX2 reflections delay | float | 0 .. 0.3 |
| 10 | EAX2 reverb | int | -10000 .. 2000 |
| 11 | EAX2 reverb delay | float | 0 .. 0.1 |
| 12 | EAX2 room rolloff | float | 0.1 .. 10 |
| 13 | EAX2 air absorption | float | -100 .. 0 |

Row 14 has kind `0x7f800000` and ends the table. The gate
`shipped_executable_yields_the_environment_tables_and_window_call_sites` checks every row
name, the room table, and the two window call sites (`68 88 13 00 00`, then `call 0x5d6f70`).

## Evidence classes

- Decoded from code: the paths above, the room lookup and its `-1` skip, the window
  value and its two call sites, the callback and pending-flag handling, the provider
  gates.
- Read from the image: the property rows and the room names.
- Inference: the window unit. The scale constant `1000` suggests milliseconds to
  seconds, and game timers are in seconds. Callback order for equal times. The record
  builder is assumed to produce all 14 items in column order.

## Not established

- **Room gate.** The byte at `0x1140aa4` has writers at `0x79ecf3` and `0x79fe96`. Their
  meaning is not decoded. A `Delivered` outcome may still hide a room write the native
  skips.
- **Clock.** The native uses `GetTickCount`, which is wall time. The host uses game time.
  This is a recreation choice. A scheduled apply may land at a different moment from the
  native's wall-clock callback.
- **Initial window.** `last_change` starts unset, so the first call always applies at
  once. The native's zero-initialised tick would open a window only for the first 5 s of
  process time. Not checked against a live run.
- **Name comparison.** `0x433490` tail-calls `0xa77598`, then `0xa77604`. The case rule
  is not decoded. Shipped table names are uppercase, so the rule does not show in them.
- **Window source.** The value is gate-verified at its two call sites, not derived at
  runtime. Deriving it needs call-site analysis.
- **Provider identity.** `ProviderId` is an opaque number the audio owner supplies. The
  identity and lifecycle contract with the audio worker (02) is pending root
  coordination.
- **Audio.** No audio consumer applies these values. `TRACKER.md` still lists EAX as an
  explicit refusal.

## Tests

- `sound_environment_tests.rs`: SLK parsing; row and room derivation on a synthetic
  image (`pe_image::tests::synthetic_image`); anchor and shape refusals; clamp and apply;
  an unknown room; a fractional int cell and a mismatched table. Ignored gates:
  `shipped_environment_table_yields_its_authored_records` (the local
  `EnvironmentSounds.slk`) and `shipped_executable_yields_the_environment_tables_and_window_call_sites`
  (the local `Warcraft III.exe`). Run with
  `PEACECRAFT3_LEGACY_DIR=<Legacy dir> cargo test -p pcx-play -- --ignored shipped`.
- `sound_environment_host_tests.rs`: admission refusals; no-provider validation; closed
  and open delivery; scheduling inside the window; the pending flag with two queued
  callbacks; a call past the window while a callback is queued; unknown names and their
  effect on the window; atomic refusal of wrong arguments.
- `pe_image_tests.rs`: section lookup, repeated names refused only when queried, VA
  reads, malformed headers.
- `item_class_table` reads sections through `pe_image`. Its existing tests cover the
  ambiguous and overflow cases. The table derived from the real executable is identical
  before and after the move; its `image_sha256` begins `3f2ed012`.
