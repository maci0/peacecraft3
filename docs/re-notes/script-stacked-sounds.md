# Stacked ambient sound requests

`ObjectHost::with_stacked_sounds(max_pending)` admits `RegisterStackedSound` and
`UnregisterStackedSound` from the loaded program's declarations. Both use the
existing sound ancestry, positional ABI and declaration-source checks. Admission
requires a sound bank, refuses a second admission, and checks the requested
queue layout before allocation.

Each request retains the action, existing sound handle, executed `byPosition`
boolean, rectangle width/height, and the last explicitly assigned sound position
at that instant. Requests preserve execution order and duplicates. Later
`SetSoundPosition` calls cannot overwrite an earlier queued position. No new
sound identity, independent resource pool or invented origin is created.

Position-based requests require an explicitly assigned position. Dimensions must
be finite and nonnegative; zero dimensions remain valid. These are checked
recreation input policies, not a claim about native handling of invalid inputs.
Null/unknown/wrong-family handles and malformed argument lists refuse. A full
queue refuses without discarding earlier work. Each append reserves fallibly.

`pending_stacked_sound_requests` exposes pending work and
`take_stacked_sound_requests` transfers it to a future consumer. The bound limits
pending requests, not the lifetime number of operations after draining. Draining
does not destroy the sounds, their decoded/encoded resources, or property state.
Register and unregister remain explicit requests: this adapter does not guess
coalescing, reference counts, rectangle matching, listener distance or playback
lifetimes.

## Verification

Four synthetic groups check duplicate/order retention, position snapshots across
movement, MPEG resource/identity preservation, draining and refill, exact/zero
queue bounds, allocation-layout refusal, both source contracts, missing positions,
invalid dimensions and identity families, and actual JASS VM execution through
the next unsupported native. All four execute under Wasmtime on
`wasm32-wasip1`. The full `pcx-play` suite passes 364 tests with six explicit
ignores. Strict native/WASM Clippy and native/WASM application checks pass.

The [selected Human01 execution gate](script-sound-properties.md) was rerun
against current production rlibs with stacked-sound admission added. It retains
the same source identities, prelude hashes, independently approved native
profiles and explicit compatibility object catalog. Globals and `config` still
complete, and all 58 file sound creations remain intact.

`main` now queues seven stacked-sound requests and reaches 142 rectangle
creations. The gate compares every queued action, sound identity, boolean,
dimension bit pattern, position snapshot and request order with the actual
executed calls. All retained rectangle coordinates still match their executed
arguments. The next refusal is `CreateCameraSetup`, map span `88339..88360`,
declared at `common.j:151112..151189`.

Real commands, logs and artifact receipts stay local in
`.scratch/cursor-gameplay-20261016/root-stacked-sounds/`. Original assets and
their extracted contents are not included.

## Remaining audio integration

No audio device consumes these requests yet. Ordinary sound starts and stacked
requests currently have separate queues; an audio consumer must preserve their
combined execution order before audible fidelity can be claimed. Effective
label/native parameter precedence, spatial units, listener mapping, native
ambient behavior and campaign playback remain open. Launcher Host/bank wiring
is also unfinished.
