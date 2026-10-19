# Source-backed direct-file sounds

`file_sounds::Assets::read` admits exact caller-provided filename/member bindings
and qualified local bytes. It checks the complete set's count, per-file bytes,
aggregate bytes, identity/metadata bytes, duplicate names and platform layouts
before waveform decoding or source copies. Missing identities and embedded NULs
refuse. These are logical input/retention bounds, not a total heap or decoder-work
promise. There is no case, extension, namespace or archive-priority fallback.

Both direct-file assets and the existing label bank now own `PcmResource`, using
the same checked `PcmWave` inspector and byte-copy path. Complete bytes, member,
source label, edition/version identity, SHA-256 and inspected PCM metadata remain
associated. Label-backed `BoundSound` still exposes its existing label, row and
resource getters; no table rows, labels or row numbers are invented for direct
files. Ancillary WAV chunks remain in the complete source bytes.

`ObjectHost::with_file_sounds(assets,max_eax_bytes)` separately admits the exact
nonconstant `CreateSound(string,boolean,boolean,boolean,integer,integer,string)`
contract returning `sound`. It checks source ambiguity, script overrides, sound
ancestry and the caller's EAX-text platform limit. Executed calls issue distinct
sound identities in the same non-recycling arena as label sounds, timers,
groups, players and symbolic identities. Repeated filenames share resource bytes,
not object identities. Filename/EAX budgets, missing exact members, wrong argument
types, NUL-containing EAX text and arena exhaustion refuse without issuing IDs.

`file_sound_state(id)` retains the actual resource, looping/is3D/out-of-range
booleans, signed fade-rate integers and EAX text exactly as executed. These are
creation inputs, not effective native units or defaults. The existing label-only
`sound_state` API remains available. `sound_resource(id)` provides checked common
PCM access for both resource-backed variants. A generic reserved `sound` handle
has no PCM resource and cannot be started.

The existing separately admitted start queue accepts both variants in one script
execution order, preserving repeated requests and resource ownership. This does
not install an audio device, mark sounds as playing or implement effective fades,
EAX, channel settings, stop/kill, distance behavior or completion callbacks. Those
need a real audio consumer. The default launcher still does not admit this Host
automatically.

## Qualified original input and verification

The same retained Human01 Program requests its first direct sound through
`CreateSound` at map script span `41999..42103`; the common declaration is
`155322..155512`. The explicit supplied Legacy `War3.mpq` resource is 53,750 bytes,
SHA-256 `85edb43f42bfaece78355568e7c7fd7ed21282bce172ebb07069b7571bdc7cd4`.
It inspects as mono 16-bit PCM, with sample rate and 26,815 frames derived from
its bytes. Decoding uses the existing bounded archive API and an explicitly
qualified Huffman-table input, not compiled-in original weights. The source DLL
and neighboring archive have separate identities; this does not establish their
release equivalence. No decoded payload is saved.

Seven new synthetic groups cover complete bytes/provenance, whole-set preflight,
exact lookup and identity refusal, shared buffers/distinct handles, all raw
creation inputs, malformed arguments, EAX/arena/platform limits, exact contracts,
script overrides, reserved identities, mixed label/file starts and retained VM
state after a later unsupported call. All fourteen prior label-sound groups and
seven file-sound groups execute as actual 32-bit WASM: mask 2097151, zero imports.
The complete play suite passes 284 tests with five explicit original-input
ignores. Strict native/WASM all-target/all-feature Clippy, library builds,
rustdoc and formatting pass.

The same Human01 admission, Program and localization still complete globals and
`config`; two ambient objects, the night start request and map-music selection
remain intact. The new direct sound's flags/rates/EAX and resource match the actual
captured call. The gate reaches `SetSoundParamsFromLabel` at map span
`42109..42183` after 556 cumulative native calls and 447 arena identities. The
next common declaration is `156001..156095`. This is bounded Host/data evidence,
not audio output, native runtime equivalence or campaign completion.

Actual source fingerprints, command logs, failed synthetic-fixture attempts and
corrected native/WASM results remain local in `.scratch/file-sound-root/`.
Original scripts, executable/DLL, sound tables and PCM bytes are never committed
or uploaded.

## Explicit duration state

`ObjectHost::with_sound_duration()` separately admits the exact nonconstant
`SetSoundDuration(sound,integer)->nothing` contract after sound resources are
admitted. It rejects duplicate admission, wrong/ambiguous/overridden declarations
and wrong sound ancestry. Both resource-backed sound variants begin with no
assigned duration. An executed call changes only that identity's optional raw
integer; `sound_duration(id)`, `SoundState::duration` and
`FileSoundState::duration` expose it. Reassignment preserves the last argument,
including zero or negative values, without guessing clamping or native sentinel
behavior. Invalid/reserved/wrong-family identities or wrong argument types fail
before mutation. No additional arena identity or PCM buffer is allocated.

Duration state is independent of inspected PCM frames/rate and survives a later
label-parameter assignment. This does not trim, stretch or resample PCM, schedule
completion, implement the script `GetSoundDuration` native, or qualify native
exceptional-input behavior. No asset duration or conversion scale is copied into
production. The default Host still refuses this setter until explicitly admitted.

The same Human01 Program/admission/localization now executes its first authored
duration assignment while retaining the parameter row, sound identity and PCM.
The next failure is an unadmitted filename on the following `CreateSound` call,
map-script span `42279..42383`: 558 cumulative native calls and 447 arena
identities. `CreateSound` itself is implemented; its caller-provided file bank
currently lacks that resource. A bounded source-only census of the remainder of
`InitSounds` identifies repeated creation/label/duration setup, so the next
binding set should derive from that function rather than a hand-written list.
Globals and config still pass; main/session publication and campaign remain open.

Three new synthetic groups cover both sound variants, per-identity replacement,
resource preservation, invalid/oversized identities, atomic failures, explicit
contract admission and retained VM state after a later failure. The existing
parameter cross-variant test also verifies that label assignment preserves the
executed duration. All 296 play tests pass with five explicit ignores; affected
strict native/WASM Clippy, library builds, native rustdoc and formatting pass.
Actual 32-bit WASM executes all 33 label/file/duration/parameter groups with zero
imports. The two export masks are `16777215` and `511`. Commands, rejected private
harness compiles and corrected results stay local in `.scratch/sound-setup-root/`.
No playback device, browser audio, default launcher or native-runtime proof is
claimed; no original payload is committed or uploaded.

## Executed-path source loading

`ObjectHost::admit_file_sound_loader` optionally supplies missing exact filename
bindings after ordinary argument validation. It requires prior file-sound
admission, validates the existing bank against its aggregate budget, and refuses
duplicate loader admission. The callback receives remaining resource, input-byte
and metadata capacity. A full bank refuses before invoking it. A successful
callback must return exactly the requested binding; append validates the combined
bank before mutation. Existing PCM/MP3 index spaces and retained byte buffers stay
stable. Cached filenames allocate a new sound identity without reading or decoding
the source again. Explicit banks without a loader keep their existing contract.

Campaign startup installs this callback on its exact retained binding reader.
Executed filenames, including computed strings, determine source lookup. Full
resolver provenance, selected edition, supplied version (or explicit unversioned
identity), original bytes and codec reports accompany each binding. Missing,
malformed or oversized input refuses; a failed replacement keeps the prior VM and
its original source reader. No chapter-specific file list, silence or transcoding
is substituted. Label constructors and parameter tables remain separate inputs.

The application policy allows 128 source attempts, 128 retained file bindings,
16 MiB per member, 128 MiB aggregate input and 1 MiB aggregate metadata. Adapter
identity formatting is capped at 16 KiB. MP3 inspection has explicit input-copy,
decoder-step, packet, frame and sample limits. These bounds are resource policies,
not game values or total heap bounds. Failed source attempts consume work;
cached bindings do not. No effective fades/EAX, native decoder equality or
audibility is established by source loading.

The new Host/factory cases check mixed-codec append, stable old byte buffers and
identities, cache reuse, cumulative input limits, wrong loader bindings, invalid
arguments, bounded diagnostics, failed source work and the retained reader after
a refused replacement. Native Host/application suites, 21 actual Cargo WASM cases,
25 targeted WASI cases, strict native/WASM lint/builds and formatting pass. The
first strict check rejected a test assertion idiom; its corrected candidate and
the failed receipt are retained separately.

A source-only check derives the direct file arguments from the local Human01
`InitSounds` AST and reads all 58 members through this production adapter: 23
PCM and 35 MP3. All member lengths and SHA-256 values match the retained earlier
source receipts. The map, codec PE and four selected archive fingerprints stay
unchanged across the check. Archive mount order is an explicit lookup policy;
native winners and release equivalence are unestablished. The two lighting inputs
also pass the shared identity adapter. No original bytes are saved or published,
and no chapter or original game is run. Evidence:
`.scratch/campaign-sound-source-root-20261019/`.
