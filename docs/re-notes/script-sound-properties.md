# Executed sound properties

`ObjectHost::with_sound_properties` admits the loaded declarations for
`SetSoundChannel`, `SetSoundVolume`, `SetSoundPitch`, `SetSoundDistanceCutoff`,
`SetSoundDistances`, `SetSoundConeAngles`, `SetSoundConeOrientation`,
`SetSoundPosition` and `SetSoundVelocity`. Positional parameter types, return
type, constant qualifier, declaration source and sound ancestry use the existing
contract checker. Admission requires a sound bank and refuses a second admission.

Each sound's existing shared-arena entry now owns `SoundProperties`. Its fields
are absent until these calls assign them. Repeated calls replace only the named
property on that identity. Label parameters, duration, encoded/PCM resources,
creation flags, fade inputs and EAX text remain intact. This applies to both PCM
and MPEG direct-file resources and source-backed label sounds.
The getter exposes the last explicit assignments for a future audio consumer.

No defaults, channel mapping, volume clamp, pitch range, distance units, cone
conversion, coordinate conversion or effective mixer behavior are inferred.
Integer arguments remain exact integers; real arguments retain their executed
finite `f32` values. Non-finite or incorrectly typed inputs refuse before any
field changes. Null, unknown and incorrect-family handles refuse in the same
arena; no replacement sound or handle pool is created.

The selected `common.j` comments mark distances, cone angles/orientation,
position and velocity as valid only for sounds created with 3D enabled. These
calls require an explicit true `is_3d` input from `CreateSound` or
`CreateSoundFromLabel`. Two-dimensional sounds and MIDI creation, whose interface
supplies no such boolean, refuse spatial assignment. This does not guess a bit
in a label table or claim native behavior for invalid calls. See
[explicit label-constructor resources](script-label-sounds.md#explicit-label-constructor-resources).

## Checks and original execution

Four synthetic groups exercise all nine setters, independent PCM/MPEG identities,
raw out-of-range values without invented clamping, signed-zero coordinate bits,
integer promotion, unchanged resources, non-spatial label properties, contextual
refusals, atomic failed mutations, admission/signature checks, and actual VM
execution through a subsequent unsupported call. All four also execute through
Wasmtime on `wasm32-wasip1`. The full `pcx-play` suite passes 360 tests with six
explicit ignores. Strict native/WASM Clippy and native/WASM application compiler
checks pass.

The [same selected Human01 gate](script-rectangles.md) was rerun with current
production rlibs. Globals and `config` still complete, all 58 file sounds and 98
rectangles remain admitted, and the executed `SetSoundPosition` now succeeds.
Its stored coordinates are compared by exact `f32` bits with that call's
arguments. The next executed refusal is `RegisterStackedSound`, map span
`83807..83879`, declared at `common.j:159284..159412`.

Commands, real logs, artifact identities and gate outputs stay local in
`.scratch/cursor-gameplay-20261016/root-sound-properties/`. The selected original
inputs and compatibility-catalog limitations are unchanged from the preceding
gate. No source audio, original script text or decoded waveform is shipped.

This is retained execution state. Sound playback, stacked ambient registration,
effective label/native assignment precedence, listener/coordinate mapping and
audible/native parity remain open. Launcher bank preparation and the composed
gameplay Host factory are also not closed by these tests.

## Unit attachment

`AttachSoundToUnit` is checked when the unit catalog is admitted after sound
properties, and must share the unit declaration source. It stores the executed
unit handle in `SoundProperties::attached_unit`, not copied coordinates.
`ObjectHost::sound_attached_position` reads that unit's current accepted
position, so the value follows movement, reveal relocation and reattachment.
A successful `SetSoundPosition` clears the attachment; a refused one leaves both
fields unchanged.

Static reading of the local Legacy executable (sha256 `3f2ed012...`, bounded
instruction windows, not a full
control-flow proof): the native wrapper skips a null sound or unit without
detaching, applies no 3D guard, and keeps a live unit reference in the sound
backend. Explicit `SetSoundPosition` releases that reference. The recreation
follows these rules, but refuses stale and wrong-family handles instead of
ignoring them. No height or attenuation rule is established, so the getter
returns ground X/Y only.

Human01 attaches two explicitly 2D dialogue sounds (`SetSoundDuration`, dialogue
label parameters, fades 10) and later 3D unit sounds. The application consumer in
`script_audio.rs` still refuses 3D, fades, duration and parameter profiles per
sound, so these sounds stay silent and appear in the refusal report. Spatial
playback, listener mapping and audible parity remain open.
