# Campaign dialogue audio consumer

The published-session consumer now admits non-spatial dialogue rows and explicit
volume/pitch settings when the startup Host supplies a source-qualified playback
profile. It reuses the existing checked `VolumeScale` reader against the exact
retained source named by the resolved `SetSoundVolumeBJ` declaration. Missing or
unsupported helpers/timing remain explicit per-request refusals, with their source
context retained. The shared parser is exposed by the application library; menu
sprite and click consumers keep their existing source-derived volume behavior.

Dialogue rows must have the admitted dialogue schema and `Flags` equal to `0`.
The selected Human01 dialogue rows use channel/list saturation-preemption flags,
so those actual mission voices remain refused. Refusals now identify the exact
label, source table and edition/version, row and raw flags. The consumer has no
source-qualified channel/list capacity, priority comparison or victim-selection
rules; accepting the flags without that behavior would discard authored state.
Their `Volume` supplies the target gain unless the sound has an explicit volume
assignment. Full numeric profiles and other dialogue flags remain refused. The
linear gain conversion is the existing recreation convention, not a native mixer
measurement. Positive explicit pitch becomes playback speed. The original sound,
parameter row/table, script and client identities remain on the published Host;
voices preserve the original resource receipt. Spatial attachment to a unit does
not turn a sound explicitly created as 2D into a spatial voice.

## Selected native code evidence

Bounded static decoding of the separately identified Legacy client executable
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
VERSIONINFO `1.29.2.9231`, establishes these paths. Capstone instruction boundaries
were checked against independent objdump output. No original runtime was launched
for this investigation.

- `CreateSound` registration resolves to `0x48e860`, forwarding through
  `0x4f6b20` into `0x5d6590`. The backend stores the supplied fade integers in
  object fields `+0xc/+0x10` and applies an unsigned minimum from an instruction
  operand at `0x5d67cf`.
- `StartSound` resolves to `0x4ac260`, through `0x4f6dc0` into `0x5d8fe0`.
  The selected initial start enters state 1. State assignment at `0x5cce80`
  selects the stored fade-in step into `+0x14`.
- The state's update callback `0x5b5ea0` adds that step to backend volume,
  capped at its target, then enters the normal playing state. The shared update
  loop `0x5bf110` invokes the state callbacks.
- `0x5dbe60` runs the update loop and re-arms itself through `0x840370`. Its
  floating-point timer operand is read from the instruction at `0x5dbe77`.
  This inspected client supplies a tenth-second update period. Production reads
  that operand from the selected image rather than embedding the measured value.
- `SetSoundDuration` resolves to `0x4a9cc0` then `0x4f70e0`. The complete setter
  sets metadata bits and stores the argument on the script sound object; it
  makes no playback/backend call. The consumer retains that value as
  `ScriptAudioMetadata::duration` and does not trim the encoded resource.

The production timing reader binds those instruction locations to the reviewed
whole-image digest, checks PE executable mapping and instruction forms, and
validates positive finite period/nonzero minimum before producing immutable
numeric rules. The supplied client identity remains attached to the rules.
Synthetic tests vary both operands and reject truncation, invalid instructions,
nonfinite/nonpositive periods, zero minimum and unqualified images.

## Playback and remaining fidelity questions

A source-backed voice starts with zero authored gain and advances in source-sized
steps. Elapsed time starts once its real audio sink exists and stops while that
sink is paused. Frame chunks produce the same step count. The source-backed
minimum applies even when a script supplied zero. Negative fade arguments remain
refused instead of adopting the native unsigned-wrap behavior. Fade-out inputs
remain retained; this change does not add a StopSound consumer or apply a fade-out
to a naturally completed waveform.

A voice's `ScriptAudioGain` multiplies the existing effects preference gain. Mute
and later user preference changes therefore preserve the source gain/envelope.
The source staging/cache budgets, decoder views, duplicate-handle handling and
session retirement remain shared with the existing consumer.

Explicit channel identifiers remain metadata. They currently share the effects
bus and produce an explicit partial-playback warning/report instead of silencing
the supported voice. Category-specific mixing remains unimplemented. 3D placement,
attenuation, EAX, full parameter profiles, native gain curves, assignment precedence
when a label follows an explicit property, global audio-update phase, device/sample
alignment, physical audibility and original/native/browser parity remain open.

Synthetic consumer checks cover dialogue rows/provenance, explicit settings,
metadata-only duration, zero-rate minimum derivation, missing target volume,
invalid gain/pitch, stepped gain and pause/frame partition. The existing registered
user-volume check now exercises authored gain multiplication and mute as well.
The browser decoder gate includes these four additional cases (15 total).
Commands, instruction findings and build output remain local under
`.scratch/campaign-audio-20261020/`; no original contents are committed.
