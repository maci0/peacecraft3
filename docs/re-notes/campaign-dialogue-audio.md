# Campaign dialogue audio consumer

The published-session consumer now admits non-spatial dialogue rows and explicit
volume/pitch settings when the startup Host supplies a source-qualified playback
profile. It reuses the existing checked `VolumeScale` reader against the exact
retained source named by the resolved `SetSoundVolumeBJ` declaration. Missing or
unsupported helpers/timing remain explicit per-request refusals, with their source
context retained. The shared parser is exposed by the application library; menu
sprite and click consumers keep their existing source-derived volume behavior.

Dialogue rows admit `0`, `CHANNELFULLPREEMPT` and `LISTFULLPREEMPT`, including
combinations and either ordering. The selected Human01 dialogue rows use both
ordinary preemption flags and now reach real voice publication. Other flags
remain source-bearing refusals rather than being discarded. Source client
constructor operands supply absent volume, pitch, priority and channel settings;
channel-table operands supply capacity, priority limits and initial channel gain.
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

Channel identifiers now select their source admission capacity, priority clamp
and initial gain. They currently share the effects preference bus and produce an explicit partial-playback warning/report instead of silencing
the supported voice. Category-specific mixing remains unimplemented. 3D placement,
attenuation, EAX, full parameter profiles, native gain curves, assignment precedence
when a label follows an explicit property, global audio-update phase, device/sample
alignment, physical audibility and original/native/browser parity remain open.

Synthetic consumer checks cover dialogue rows/provenance, explicit settings,
metadata-only duration, zero-rate minimum derivation, missing target volume,
invalid gain/pitch, stepped gain and pause/frame partition. The existing registered
user-volume check now exercises authored gain multiplication and mute as well.
The browser decoder gate includes dialogue, arbitration and lifetime checks
(18 total).
Commands, instruction findings and build output remain local under
`.scratch/campaign-audio-20261020/`; no original contents are committed.


## Source capacities and ordinary preemption

The selected startup at `0x5d71cf` supplies the channel count; the pointer operand
at `0x5d71d7` names its channel table. `0x79dbf0` copies twenty-byte records into
the backend channel collection. `0x79ee30` reads each channel's priority floor,
ceiling and flags while constructing a voice. The dialogue defaults come from
`0x5b8cbe..0x5b8ce8`; no constructor volume, pitch, priority or channel operand is
copied into production constants.

The complete admission body `0x7c7380..0x7c7797` checks total active sound-list
capacity, per-channel capacity and filename count in that order. The total and
filename limits are decoded from its comparison instructions at `0x7c74fc` and
`0x7c7739`. Their inspected values are documented in local evidence only; the
runtime reader validates instruction forms and reads them from the qualified
client. The filename lookup at `0x7c7ea0` uses the name hash and string comparison;
`0x7c49d0` decrements the filename count when a playing voice retires.

Flag translation at `0x5bfbc0` associates ordinary channel/list preemption with
these branches. At total-list capacity, ordinary list preemption accepts a
lowest-priority victim whose priority is no greater than the incoming voice.
At channel capacity, ordinary channel preemption requires a strictly lower
victim priority. These branches return after evicting their victim. Filename
saturation is not overridden by either ordinary flag; the separate oldest
variant remains unsupported. The consumer preserves this admission order and
strict/nonstrict comparison, and refuses starts without an eligible victim.
Equal-priority victim ties choose the oldest accepted start deterministically;
that tie rule and slash normalization are stated recreation choices, not proven
native tie behavior.

Arbitration runs against live and earlier queued voices before source staging.
Only a completely prepared batch retires victims. Eviction removes the old
entity and releases its shared encoded-source reference. Finished voices and
session replacement continue to use the existing reclamation path. Capacities
apply to the consumer's admitted script voices; unsupported original spatial,
menu/music-category and other engine voices do not become invented entries in
its accounting.

Synthetic policy tests vary constructor/table/limit operands and check retained
source receipts, truncation, malformed opcodes, invalid ranges/gain and unsupported
variance. Consumer tests cover both flags, missing source capacities, priority
comparisons, filename saturation, tie order and same-batch/cross-batch eviction
with shared-source reclamation.

The explicit ignored original-input test
`local_original_dialogue_voice_decodes_through_published_consumer` requires all
six `PCX_SCRIPT_AUDIO_{CLIENT,BJ,TABLE,MAP,MEDIA,LABEL}` inputs. It selects the
label's original `InitSounds` variable and literal `CreateSound` arguments from
the supplied map AST, loads that exact parameter row and supplied encoded member,
then uses the real consumer and installed decoder. A local selected opening
Human01 voice publishes without refusal and produces nonzero decoded samples
while retaining both preemption flags. The first invocation used relative input
paths and failed after Cargo changed the working directory; an absolute-path
rerun passed. This is source/consumer/decoder evidence, not physical audibility,
audio-device output, original runtime arbitration observation or full campaign
sound parity. Original inputs and decoded content remain local.
