# Sound environments (`NewSoundEnvironment`)

`ObjectHost::with_sound_environments(records, properties)` admits `NewSoundEnvironment`
from the loaded declaration (`takes string environmentName returns nothing`, same contract
checker as the other natives). Records come from every `UI\SoundInfo` table whose header
row contains `EnvironmentType` (`SoundSources::read`, `crates/peacecraft3/src/campaign_assets.rs`).
Property rows come from the selected `Warcraft III.exe` (`pcx_play::sound_environment::eax_properties`).
An empty record set is not admitted, so the native stays an explicit refusal.

## What the host does

- `pcx_play::sound_environment::from_table` derives one record per `EnvironmentType` row:
  name, `RoomType`, `DisplayText` and fourteen reverb cells. Cells parse as finite `f32`;
  a missing column, malformed number, empty or repeated name is refused.
- `eax_properties(exe)` reads fourteen rows from the executable. The row table is found
  from code: the unique cluster of `[edi*4 + base + {0,4,8,12,16}]` displacements inside
  `.rdata`, stride 20. The name table is the unique `push [edx*4 + disp]` displacement in
  `.text`. Rows are read while the kind is 0 (float: `[min, max]` as f32 at +4/+8) or 1
  (int: `[min, max]` as i32 at +12/+16). Any other kind ends the table. The read names must
  equal the fourteen EAX names in `Reverb` field order, or the table is refused.
- `apply(record, properties)` clamps each cell to its row's range. The first row
  (`EAX effect volume`) is the native's own global; the other thirteen are provider
  preference writes. Int rows require integral cells.
- The call selects the record whose name equals the argument exactly. Unknown names,
  wrong arguments and values that fail `apply` refuse and keep the previous selection.
  Before the first successful call, `sound_environment()` and `sound_environment_applied()`
  return `None`.

`sound_environment_applied()` is not a delivery. The host has no 3D provider, so the
provider writes are values the native would pass to Miles when a provider is open. Nothing
is sent.

## Native path (static, `Warcraft III.exe`, sha256 `3f2ed012…570eed`)

Decoded with bounded capstone scripts (no execution). Local output lives under
`.scratch/sound-env-static/`; the scripts read the spatial-root helper module read-only.

| Address | Role | Evidence |
|---|---|---|
| `0x4a39c0` | `NewSoundEnvironment` body: calls `0x469010` on the argument, stores the result back as the argument, tail-jumps to `0x5d83e0` | `NewSoundEnvironment.json` |
| `0x469010` | Returns `[arg+8]->[+0x1c]` or `0` | `body-469010.json` |
| `0x5d83e0` | Shared body. Copies the name to `0x113cd98` (`0x104`) when pending flag `0x113d624` is set. Reads `GetTickCount` (IAT `0xe661a8`) against last change `0x113d628` plus `0x10dc6bc`. If inside that window, copies the name and schedules `0x5be250` through `0x840370`, storing the handle at `0x113d624`. Otherwise applies now and stores the tick in `0x113d628`. | `body-5d83e0.json` |
| `0x5be250` | Scheduled apply: clears `0x113d624`, looks up the name in table `0x113d504` (`0x5cf8e0`), calls `0x7a0090` (room) and `0x79ff90` (properties) | `body-5be250.json` |
| `0x7a0090` | If provider handle `0x1140650` is nonzero and `0x1140aa4` bit 0 is clear: `AIL_set_3D_room_type` (IAT `0xe66904`, arg = room index) | `body-7a0090.json` |
| `0x79ff90` | Loop over 12-byte items `{index, float, int}`: clamps to row range (table `0xeb7500`, stride 20); index 0 to global `0x10dd990`; others `AIL_set_3D_provider_preference` (IAT `0xe66908`) with name from `0x10dd9f0`. No provider, no call. | `body-79ff90.json` |
| `0x840370` | Schedules the callback. Callee not decoded. Its first argument is the window value divided by `0xe78350` (`1000.0` in the image), so the unit is likely seconds from milliseconds (inference). | `exe-values.json` |

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

Row 14 has kind `0x7f800000` and ends the table. The ranges above were read from the image
with `.scratch/sound-env-static/read_exe_values.py` and match the gate test
`shipped_executable_yields_the_fourteen_eax_rows_in_order`.

The EAX names match the `Reverb` field order of `UI\SoundInfo\EnvironmentSounds.slk`. The
`RoomType` cells (`MOUNTAINS`, `PLAIN`, `PSYCHOTIC`) are EAX preset names. The
`0x1a` bound in `0x5be250` (26 entries) is consistent with the EAX preset list. The
name-to-index map in table `0x113d504` is not decoded.

## Not established

- **Room type.** The host does not apply it. The native passes an index in `0..26` to
  `AIL_set_3D_room_type`, but the mapping from `RoomType` names to indices lives in
  table `0x113d504`, which is not decoded.
- **Cooldown window.** The window value is `0x10dc6bc`, initialised from a dword at
  `0x10dc6d4` (`17680068`), and written by the function at `0x5d7000` from its first
  argument (`-1` keeps the current value). The host applies immediately and has no window.
- **Item list composition.** The item list's source is the record's `+0x20`/`+0x24`
  fields. Whether it always contains all fourteen rows in this order is not established.
  The host assumes it does.
- **Provider.** No 3D provider is opened in this host. Whether a real Miles provider
  opens on the target machine is not established.
- **Audio.** No audio consumer renders reverb. `TRACKER.md` still lists EAX as an explicit
  refusal. That remains true until a consumer uses these values.
- **Case sensitivity and unknown names.** The native's behavior is not established. This
  host compares names exactly and refuses unknown names.

## Tests

- `crates/pcx-play/src/sound_environment_tests.rs`: SLK parsing, row/name derivation on a
  synthetic image (`pe_image::tests::synthetic_image`), refusal of wrong names, counts and
  inverted ranges, clamp and apply behavior, and two explicit gates:
  - `shipped_environment_table_yields_its_authored_records`: the local `EnvironmentSounds.slk`.
  - `shipped_executable_yields_the_fourteen_eax_rows_in_order`: the local `Warcraft III.exe`.
  Run them with
  `PEACECRAFT3_LEGACY_DIR=<Legacy dir> cargo test -p pcx-play -- --ignored shipped`.
- `crates/pcx-play/src/sound_environment_host_tests.rs`: admission, selection, clamped
  values, atomic refusal, second admission, signature mismatch.
- `crates/pcx-play/src/pe_image_tests.rs`: section lookup, bounded VA reads, malformed headers.
  `item_class_table` now reads sections through the same reader; its tests cover the move.
