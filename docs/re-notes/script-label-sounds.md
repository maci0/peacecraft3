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

## Verification and next boundary

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
