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
