# Script 3D sound playback: blocked

`script_audio.rs` refuses every `CreateSound` voice created with `spatial = true`.
The refusal reason is `3D placement is not played: native attenuation, cutoff and
listener mapping are unmeasured`. This is the state of the code. No positioned
playback exists.

## Executed inputs the host retains

`SetSoundPosition`, `SetSoundDistances`, `SetSoundDistanceCutoff`,
`SetSoundVelocity`, `SetSoundConeAngles`, `SetSoundConeOrientation` and
`AttachSoundToUnit` store their executed values on the sound identity
(`pcx-play/src/sound_properties_host.rs`). Their arguments are finite reals or
integers with the declared arity. Nothing else in the repo consumes them.

## Not established

- **Attenuation curve.** How `MinDistance` and `MaxDistance` map to gain (shape,
  whether gain reaches zero, whether it is clamped below the minimum). No
  original-asset or engine-derived evidence exists.
- **Cutoff.** Whether `DistanceCutoff` silences the voice, stops it, or only
  changes the curve. `stop_when_out_of_range` is a separate creation flag with no
  established link to it.
- **Listener.** Which point is the listener: camera eye, camera target, or the
  hero. The campaign camera is a recreation convention, not a native fact.
- **Ear spacing and pan.** Bevy's `SpatialListener` default offsets are an engine
  convention. The native pan model is unmeasured.
- **Distance precedence.** Whether label `MinDistance`/`MaxDistance`/`DistanceCutoff`
  apply when no executed setter ran, and which wins when both exist. The source
  catalog columns exist (`pcx-play/src/sound_parameters.rs`); their semantics do not.
- **Velocity, cones, attached-unit height.** Doppler, cone attenuation and the
  height of an attached unit are unmeasured.

## Fade gap (not a native rule)

`FadeIn` and `advance_fades` read `AudioSink`. A positioned sink would be a
`SpatialAudioSink`, so a fade on that voice would not advance. This matters only
after 3D is enabled. Adapting both sink types needs the native fade semantics first,
since no evidence sets the 3D ramp.

## Evidence needed to unblock

A native reading of the Legacy executable's 3D sound update path (the code that
consumes the executed distances and position) and one original runtime observation of
a 3D sound's gain against listener distance. Either needs a root Ghidra slot. The
result must state the curve, cutoff, listener source and pan model, with the
executable's SHA256 (`3f2ed012...`) and the function addresses. Until then, keep the
refusal.

## Caller needs for other workers

- Sound lifecycle: a 3D start stays refused until the evidence above lands. The
  refusal must remain explicit in `ScriptAudioReport`.
- Environment: no listener or camera API is exposed to sound playback. Any listener
  choice needs root's decision and the evidence above.
