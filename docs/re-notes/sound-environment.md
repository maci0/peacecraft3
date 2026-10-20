# Sound environments (`NewSoundEnvironment`)

`ObjectHost::with_sound_environments(records)` admits `NewSoundEnvironment` from the
loaded declaration (`takes string environmentName returns nothing`, same contract
checker as the other natives). Records come from every `UI\SoundInfo` table whose
header row contains `EnvironmentType`, read by `SoundSources::read` in
`crates/peacecraft3/src/campaign_assets.rs`. An empty record set is not admitted, so
the native stays an explicit refusal.

`pcx_play::sound_environment::from_table` derives one `SoundEnvironment` per data row:
`EnvironmentType` (name), `RoomType`, `DisplayText` and fourteen reverb cells
(`EffectVolume` through `AirAbsorption`). Cell text is parsed as a finite `f32`; a
missing column, malformed or non-finite number, empty or repeated name is refused.
Nothing is defaulted.

The call selects the record whose name equals the argument exactly. Unknown names,
non-string arguments and wrong arity refuse and leave the previous selection in place.
Before the first successful call, `ObjectHost::sound_environment()` returns `None`.

## Evidence

| Claim | Evidence | Class |
|---|---|---|
| Declaration `native NewSoundEnvironment takes string environmentName returns nothing` | selected `Scripts\common.j` line 2263 (local copy under `.scratch/ledger-route-20261020/`) | shipped-byte |
| `EnvironmentSounds.slk` is 1,239 bytes, rows Default, lake, psychotic, Dungeon; column set above | local copy `.scratch/campaign-bootstrap-root-20261020/.../ui/soundinfo/environmentsounds.slk`, `crates/pcx-slq/src/lib.rs` test `environment_sounds_is_the_smallest_sound_table_and_has_four_rows` | shipped-byte |
| reverb column names (`DecayTime`, `Reflections`, `Reverb`, `RoomRolloff`) resemble the EAX 1.0 reverb property set; `eaxdefs.slk` names EAX2 fields | column names in `UI\SoundInfo\EnvironmentSounds.slk` and `eaxdefs.slk` | inference from names |
| Rise of the Naga startup refuses at `war3map.j:10269:5`, the 37-byte `call NewSoundEnvironment(...)` span, argument not read | `.scratch/gui-item-20261020/runs/checkpoint-2/gui.log` refusal line | runtime observation; argument is inferred from span length |
| Argument is `"Default"` in the Lordaeron probe map | `.scratch/original-rng-map-probe-20261016/source-war3map.j:1950` | shipped-byte (other map) |

## Not established

- Native behavior of `NewSoundEnvironment`: no native-code evidence was taken for
  this slice. Whether the selection applies to sounds created later, to sounds already
  playing, or to ambience is unknown.
- Whether the native matches names case-insensitively, and what it does with an
  unknown name (this host refuses it).
- Reverb application. No audio consumer renders these parameters yet. Until one does,
  the environment is retained and observable only through `sound_environment()`.
  `TRACKER.md` still lists EAX as an explicit refusal; that remains true.
- `DisplayText` use by the native or the UI is not established.

## Tests

- `crates/pcx-play/src/sound_environment_tests.rs`: synthetic SLK parsing through
  `Table::parse`; refusals; ignored explicit gate
  `shipped_environment_table_yields_its_authored_records`, run with
  `PEACECRAFT3_INSTALL=<Legacy dir> cargo test -p pcx-play -- --ignored shipped_environment`.
- `crates/pcx-play/src/sound_environment_host_tests.rs`: admission, selection,
  atomic refusal, second admission, signature mismatch.
