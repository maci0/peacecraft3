# Source-backed label sounds in the shared script arena

`label_sounds::SoundBank::read` admits an explicitly selected sound table, label
column and resource bindings. It reuses `pcx-slq::Table` and `SoundTable` for the
existing `DirectoryBase`/`FileNames` schema. Source identities, labels, table and
resource fingerprints, complete PCM resource bytes and every parsed table cell
are retained. Volume, pitch, channel, flags and priority remain authored cells;
no implicit values, normalization, random variant or channel policy is invented.

Count, source-table bytes, resource bytes, aggregate resource bytes, metadata
text and platform layouts are checked across the complete input set before table
decoding or resource copies. Coordinate ceilings precede cell insertion through
the existing parser. These are logical input/retention limits, not a total heap,
RSS or decoder-work guarantee. No filesystem, executable discovery or network
operation is introduced in the library.

The selected label/header must be unique. Duplicate cells, trailing records and
error-severity table anomalies refuse; informational capacity mismatches and
other non-lossy annotations remain available on the parsed table. In particular,
the advisory `B` record never supplies missing rows or allocation dimensions.
Missing fields, unsupported multi-file/disabled entries, empty identities and
member-path mismatches refuse with source context. Labels and member bindings
are exact; there is no case, extension, namespace or archive-priority fallback.

## PCM and object state

The small `PcmWave` inspector validates exact RIFF size, chunk extents and word
padding, unique format/data chunks, required fields, integer mono/stereo 8/16-bit
PCM, rate/alignment consistency and whole sample frames. Unknown ancillary chunks
remain in the complete bytes. Data may precede the format chunk. Sample rate,
channels, bit depth, sample range and frame count come from the resource, not
constants copied from a recording. This profile follows Microsoft's
[RIFF framing](https://learn.microsoft.com/en-us/windows/win32/xaudio2/resource-interchange-file-format--riff-)
and [WAVEFORMATEX contract](https://learn.microsoft.com/en-us/windows/win32/api/mmreg/ns-mmreg-waveformatex).
Other codecs, extensible/multichannel formats and MIDI/DLS synthesis are not
implemented by this inspector.

`ObjectHost::with_label_sounds(bank)` validates the exact nonconstant
`CreateMIDISound(string,integer,integer)->sound` declaration, sound ancestry,
source ambiguity and script overrides. Each executed creation issues a distinct
sound identity in the existing non-recycling arena. Repeated labels share the
admitted resource buffer, not identities. Capacity failure, bad arguments and
unknown/oversized labels do not issue identities. Generic reserved `sound`
objects do not acquire resource state. `sound_state(id)` retains the resource
and the exact raw fade-rate arguments. Fade units/effective clamps, lifetime,
playback, looping, channel arbitration and renderer/backend output remain open.

## Bounded start requests

`with_sound_starts(max_pending)` separately admits the exact nonconstant
`StartSound(sound)->nothing` declaration and an explicit pending-request limit.
It requires an admitted bank and checks platform layout before use. Starts accept
only actual resource-backed sound objects from this arena. Nulls, other families,
generic reserved sound identities and queue-capacity failures do not enqueue.
Every successful call is retained in script order, including repeated starts of
the same identity. No extra sound identities or resource buffers are created.

`pending_sound_starts()` exposes the ordered requests; `take_sound_starts()`
transfers them to an explicit consumer while the Host retains identities and
resources. The consumer owns delivery/retry after transfer. This is an internal
recreation command boundary, not native scheduling evidence. It does not mark a
sound as playing or install an audio device. A game audio consumer must still
interpret the admitted source settings and implement playback. Stop/kill,
completion callbacks, fade units, channel arbitration and restart behavior remain
unsupported. The default Host still refuses StartSound until explicitly admitted.

## Qualified local source result

The retained Human01 execution calls `CreateMIDISound` from `SetAmbientDaySound`
in Blizzard.j. The selected label occurs in both `MIDISounds.slk` and
`AmbientMusic.slk`. The legacy table references absent MIDI/DLS resources; its
settings differ from the ambient table. A bounded census of the supplied
`War3.mpq` sound tables finds an actual single-WAV binding in `AmbientMusic.slk`.
This is an explicit recreation source choice, not proof of the native active
table or cross-edition fallback.

Selected fingerprints:

- `AmbientMusic.slk`: `a023b06a3a31d2a0c84e2d05192bc604188ab0371e3355bd1f63954191b35473`
- decoded WAV: `bd5d464fd7f9b3147a6210980c2f40b991b673080d97f24342745d1e93bb3f6f`
- explicit local Huffman-table input DLL: `eecc42b05a91413dee10588a654c6cca05836e7513af6c7c568a56124bf97d0a`

The WAV is decoded through the bounded archive API with an explicit table derived
from the supplied DLL; no original Huffman weights are compiled into the game.
This codec source does not identify neighboring archives' release versions.

Offline inspection of the qualified Legacy executable
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
finds the first named `CreateMIDISound` registration with `(SII)Hsound;`.
The normal-flow entry is `0x48e5d0..0x48e681` (177 bytes), forwarding the resolved
label/rates through `0x4f6c40..0x4f6c8e` (78 bytes) to
`0x5d6590..0x5d69a6` (1,046 bytes). Independent objdump boundaries agree with
Capstone for all 62, 32 and 306 instructions. The producer searches two runtime
lookup objects and resolves a filename through another helper. Those objects'
table initialization and filename helper were not inspected. The alternate
registration, active family, exception handlers and runtime sound output remain
unproved. No game, Wine, provider or REA session was run.

## Verification and startup boundaries

### Creation baseline

Ten synthetic tests cover PCM dimensions/order/padding, malformed and overflowing
RIFF spans, profile/rate/alignment errors, exact input limits, preflight ordering,
platform layouts, advisory capacity, duplicate values/rows, metadata/resource
retention, missing/multi-file bindings, exact declarations, shared identity
capacity, wrong families and actual owned-VM creation/failure retention. All 262
play tests pass with five explicit original-input ignores; strict native/WASM
all-target/all-feature Clippy, library generation, rustdoc and formatting pass.
All ten checks also execute as actual 32-bit WASM: mask 1023, zero imports.

The same retained Human01 Program/admission/localization now creates one sound
whose label, resource and rates match the captured call. Arena identities remain
distinct: 445 total. After 550 cumulative native calls, `main` reaches
`GetFloatGameState` through Blizzard.j's `GetTimeOfDay`, span `55083..55124`.
Globals and `config` still complete. No time-of-day default is fabricated to
start playback. The compatibility catalog still has 94 unresolved records;
launcher/default Host admission and campaign completion remain open.

Actual commands, immutable failed attempts, source bindings and captured results
are local in `.scratch/midi-sound-root/`. Original PCM, table and PE payloads
are never written or published by the gate. Source-profile selection, clock
initialization, fades, audio playback and audible/native parity remain separate.


### Start-request verification

Four additional tests cover ordered/repeated requests, queue exhaustion without
mutation, explicit transfer and reuse, preserved PCM/identity ownership, wrong and
reserved families, malformed arguments, admission/signature/override failures,
32-bit platform limits and retained VM requests after a later unsupported call.
All fourteen sound tests execute as actual 32-bit WASM: mask 16383, zero imports.
The complete play suite now passes 273 tests with five explicit original-input
ignores. Strict native/WASM all-target/all-feature Clippy, library builds,
rustdoc and formatting pass.

A fresh same-Program Human01 gate explicitly supplies *both* ambient labels from
the selected table. The night PCM resource is 22,946,888 bytes, SHA-256
`5a12f28a9a208cded1fb1b91157f8c61bc89b4bcbdc5bce6ed9f8d27b2132765`.
Its member path and format/frame count are derived from the table and decoded
resource, with the same explicit bounded Huffman source as the day resource.
No decoded payload is saved. Using the opt-in startup snapshot described in
`script-startup-clock.md`, the authored script reads time twice, creates both
sounds and issues one start request for the actual night sound identity. Every
resource, fade argument and queued identity matches the captured source calls.

Globals and `config` still complete; the run now reaches `SetMapMusic` in map
script span `384467..384503`, after 554 cumulative native calls and 446 arena
identities. The common declaration is at `157693..157798`. A baseline run with
both PCM bindings but without start-request admission independently stopped at
`StartSound`, Blizzard.j span `115532..115569`.

Commands, immutable failed strict-lint attempts and actual corrected native/WASM
results are local in `.scratch/sound-start-root/`. Neither an audio device nor a
browser was used. Audible/native parity, running-clock behavior, renderer/backend
consumption, default launcher admission and campaign completion remain open.

### Explicit label-constructor resources

`ObjectHost::with_label_sound_creation` separately admits caller-selected
`Vec<SoundBank>` inputs for the loaded non-constant `CreateSoundFromLabel`
interface (`string, boolean, boolean, boolean, integer, integer -> sound`).
Each bank has already passed its source, table, resource and metadata budgets.
Empty bank lists and duplicate labels across selected banks refuse. The native
and the entire authored sound ancestry must share the admitted `CreateTimer`
source; a function override, wrong declaration or foreign type source refuses.
The Host does not choose archives, tables or fallback resources.

Each successful constructor allocates one real sound on the existing bounded
arena and binds the exact label to its selected table row and PCM resource.
`SoundState.bank` exposes that source table; `resource` retains the exact source
member and fingerprint. Repeated creation produces distinct identities sharing
the immutable binding. `creation_flags: Some(LabelSoundFlags)` retains all three
executed booleans, including false, while `None` identifies MIDI creation's
absent inputs. Signed fade integers remain unchanged. The MIDI bank stays
independent, including when it contains the same label with a different resource.

Duration, start queues, parameter assignments, property setters and stacked
requests use this same resource-backed identity. Spatial setters require an
explicit true creation `is_3d` input from either constructor; MIDI creation has
no such boolean. Property fields still describe executed setters, not inferred
effective values from the retained table. Table settings, mixer precedence,
looping, range stopping, fades and audible playback need an audio consumer and
remain separate fidelity gates.

The current selected-program qualifier scans all loaded function bodies and global
initializers with bounded work, depth and label storage. It finds 11 distinct
literal labels and no nonliteral constructor sites. The explicit local Legacy
input set binds nine labels through `War3.mpq::UI\SoundInfo\UISounds.slk` and two
through `War3.mpq::UI\SoundInfo\AmbienceSounds.slk`. Their SHA-256 fingerprints
are `6e9732d982610e16ad9b9e7f88f5444f7cb133cf6ab4ecf56c0185c40b1ceeff`
and `22cfa5a88ee69210c3c7f3a8417d465286f507fcb93fd551015a9e4ad6f59d42`.
All 11 single-file bindings pass the existing bounded PCM reader. The Warning
member is present in the explicitly selected expansion archive, not the base
archive. Archive candidates are checked for unequal copies rather than selecting
an assumed precedence. This is a recreation source selection, not proof of the
native lookup winner or an archive's release inferred from its filename.

Using the same unchanged Human01 Program, admission and captured startup inputs,
globals and `config` complete. `main` executes 11 constructor calls across nine
labels, including repeated quest notifications. Every resulting record's label,
source row/resource, three booleans and both fade integers match the executed
arguments. The two ambience resources are admitted but not created before the
next refusal. Previous player, unit, force, sound and source-context checks still
pass; the shared arena contains 880 identities, 155 units and four triggers.
The next unsupported native is `TriggerRegisterTimerExpireEvent`, Blizzard span
`143744..143837`, common declaration `72705..72793`. The timer and trigger are
existing identities; no registration or callback delivery is fabricated.

Four synthetic regressions cover actual owned-VM creation, distinct MIDI/source
banks, nonzero bank selection, exact flags and signed fades, duration/start
consumers, 3D assignment, source/contract/type ancestry, duplicates and atomic
argument/label/arena refusals. All 357 play tests pass natively and through actual
WASI execution, with five explicit original-input ignores. Strict native/browser
Clippy checks pass. Immutable qualification attempts, source/artifact hashes and
the successful selected-program gate are local in
`.scratch/label-constructor-root-20261017/`. No original payload is committed.
The application Host factory, audio playback, callback scheduling, campaign
completion and rendered/native parity remain open.


### Executed source preparation

`admit_label_sound_loader` optionally extends already admitted constructors with
one exact label binding per request. The Host checks constructor arguments and
cached labels first. New banks append without moving any earlier flattened
resource index; existing handles and PCM buffers remain valid. MIDI creation and
explicit-flag creation retain separate resource lists. `sound_bank` still exposes
the initial explicit MIDI bank; `sound_state` identifies each object's actual
source bank. Failed or wrong bindings do not allocate a sound or replace old data.

The campaign caller shares one retained SoundInfo index with parameter loading.
PCM constructor ownership requires the authored `FileNames` definition schema;
legacy MIDI/DLS records with matching labels do not own PCM bindings. No table
filename or chapter label list selects a winner. Multiple PCM owners, disabled
rows and multiple-file rows refuse. This extends the existing PCM-backed native
admission; it does not implement MIDI/DLS decoding or establish the original
engine's active format policy. Paths derive from the selected row's directory
and file fields. Only the requested resource is read and decoded.

Application ceilings are 130 total retained banks, 32 MiB of table input copies,
128 sound bindings, 64 MiB per PCM file, 128 MiB aggregate file input and 1 MiB
metadata. There are 128 uncached label-source attempts per Host; cached labels
remain usable after exhaustion. Label/path adapter text is capped at 16 KiB.
The shared index uses the parameter loader's table/coordinate/work ceilings and
retains both success and failure. These are application input/work bounds,
not total parser memory bounds or original game settings.

Verification: 484 native Host cases pass with seven explicit ignores, as do the
application suites, 28 actual Cargo WASM cases and 22 targeted WASI label tests.
Strict native/WASM lint/builds and formatting pass. A local source-only probe
qualifies eleven literal constructor labels from the selected script set and
two authored ambient selections: thirteen PCM resources across three table
byte inputs. All file fingerprints match retained evidence. The initial probe
exposed matching PCM and MIDI/DLS labels; the schema filter and synthetic
same-label regression pass. Both large ambient files fit the final loader
policy. Two nonliteral constructor sites are reported without a completeness
claim. Explicit archive mount order remains distinct from native winner/release
proof. No new original runtime, audible output or rendered campaign check ran.
Evidence: `.scratch/campaign-label-source-root-20261019/verification-final-6.json`.
