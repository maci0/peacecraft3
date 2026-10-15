# Source-backed sound-label parameters

`sound_parameters::Bank::read` admits one caller-selected SLK, its exact label
column and a bounded set of labels. It reuses the checked table parser used by
label-sound creation. Source label, edition/version identity, whole-table SHA-256,
raw table and selected row indices remain available. No filename is selected or
PCM resource loaded by parameter admission, including rows with several files.

Before parsing, it checks table bytes, coordinate limits, definition count,
aggregate input metadata and platform allocation layouts. Selected headers,
labels and rows must be unique; duplicate cells, malformed tables and trailing
records refuse. Selected rows must provide integer Volume/Priority/Channel and
finite real Pitch/PitchVariance/MinDistance/MaxDistance/DistanceCutoff. These
numbers remain raw: parsing does not establish gain normalization, distance
units, channel routing, random pitch, attenuation or native clamping. Flags and
EAXFlags remain opaque source text. Unselected rows need not meet this numeric
profile. This is an explicit checked import profile, not a claim that native
code rejects the same malformed rows. Limits cover logical inputs and layouts,
not total parser heap or RSS.

`ObjectHost::with_sound_parameters(bank)` requires resource-backed sound
admission and the exact nonconstant
`SetSoundParamsFromLabel(sound,string)->nothing` contract. It rejects duplicate
admission, ambiguous declarations, script overrides and wrong sound ancestry.
An executed call records the selected definition on that actual shared-arena
sound identity. `sound_parameters(id)` exposes its last assignment and raw
values; `sound_parameter_bank()` provides the source receipt and table. Label
and direct-file sound objects use the same bank. Generic reserved sounds, nulls,
wrong families, invalid identities, oversized labels and missing definitions
refuse without changing prior state or issuing identities. Reassigning a label
preserves the original PCM resource and creation inputs. Those inputs are not
asserted to be effective flags after parameter application.

This is parameter state, not audible playback. The start queue still contains
ordered identity requests, not timestamped settings snapshots. An audio consumer
must qualify how changes between starts are sampled, preserve order and repeats,
and implement supported effective settings rather than assuming that the final
label state describes every earlier request. Playback must follow successful
`MapScriptSession` publication; draining a candidate during globals/startup
would leak side effects from a failed replacement.

## Qualified evidence and limits

The supplied Legacy executable is independently identified by SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
14,858,960 bytes, own VERSIONINFO `1.29.2.9231`. Bounded static inspection finds
the first named registration and the following complete bodies. Capstone
instruction boundaries agree with an independent objdump decode. No executable,
DLL or game process was loaded.

| Body, end exclusive | Bytes | Code SHA-256 |
| --- | ---: | --- |
| `0x4a9cf0..0x4a9d18` | 40 | `06e7c06f51481b642ff93e189af31eeb5a1e06a4a3f7405de1a859ffe5ce11bb` |
| `0x4f7000..0x4f701a` | 26 | `4c41e055d7002e6befc860b342babcf842170535bd24f5111a22fe3ff4457ad0` |
| `0x5da690..0x5da797` | 263 | `d75e5876964381354524fd1c64565000d40c87704fd4837a7181c017ea3c7bab` |

The entry resolves the sound and label, then dispatches through the sound
object. The last body attempts two runtime lookups via `0x5cf970`. Their active
table roles/order are not established here. On success it calls the uninspected
copy/apply target `0x5bcbc0`, preserving old definition fields `+0x1c/+0x20`,
`+0x28` and old bit zero of `+0x40`. Field names and that bit's meaning are not
established. Therefore source flags are not silently treated as an override of
all creation booleans. This static chain supports label-parameter application;
it does not prove the imported numeric profile, active runtime lookup source,
effective settings, audible result or native rejection behavior.

The bounded local table census finds the requested label uniquely in the
explicitly selected `War3.mpq::UI\SoundInfo\UnitAckSounds.slk`, SHA-256
`7fee560faabca6b4217a0e277176d24023069682e4511d792c6942a90d2ef475`,
row 754. The same retained Human01 Program/admission/localization successfully
executes that assignment on sound identity 447 while retaining its originally
bound PCM and creation arguments. No table values, flags, file lists or payloads
are copied into production constants. Runtime table lookup is explicit caller
policy; no native archive or table priority is inferred.

The genuine next refusal is `SetSoundDuration` at map-script span
`42189..42243`, common declaration `158715..158808`: 557 cumulative native calls,
447 arena identities. Globals and config still complete; main remains incomplete.
The session is not published as a successful game. No playback backend or default
launcher Host is wired by this change.

## Verification

Nine synthetic parameter groups cover exact provenance/raw values, multi-file
independence, budgets/platform layouts, missing/invalid/nonfinite fields,
ambiguities, atomic assignment, both sound variants, reserved/wrong families,
contract admission and retained VM state after a later failure. At introduction, all 293 play
checks pass, with five explicit ignores. Strict affected native/WASM
all-target/all-feature Clippy and library builds pass. All fourteen prior
label-sound, seven file-sound and nine parameter groups execute as actual
32-bit WASM: 30 groups, mask `1073741823`, zero imports. Native rustdoc and
format checks also pass.

Exact commands, failures and corrected results stay local in
`.scratch/sound-params-root/`. Original scripts, tables, PCM and executable
payloads are neither committed nor uploaded. Native runtime, audio-device,
browser playback and campaign completion remain unverified.
