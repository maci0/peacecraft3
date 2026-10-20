# Script sound restart and stop lifecycle

Applies to script sound handles published through `ObjectHost` and consumed by
`peacecraft3::script_audio`. Spatial placement is out of scope.

## Behavior

- **Repeat `StartSound` on an active handle restarts it.** The earlier voice
  stops and one new voice for that identity plays. Within one batch, a later
  repeat supersedes an earlier one. A repeat no longer refuses, and capacity
  arbitration does not count the replaced voice. If arbitration refuses the
  restart, the earlier voice keeps playing and the refusal is reported.
- **`StopSound` stops the identity's live voice**, looping or not. The host
  first drops that identity's unconsumed `StartSound` requests, so a start
  followed by a stop in the same frame stays silent. The consumer applies
  stops before starts.
- **`kill=true` stops immediately.** Native `kill` releases the backend voice
  after the stop call, so no fade survives it. Handle destruction is host state
  and is not modeled.
- **`fade=true` with `kill=false` is not applied.** Native fade-out ramps the
  voice down by the fade-out rate; this consumer stops at once and
  `ScriptAudioReport::last_partial` names the unapplied ramp. The fade-out rate
  stays retained on the handle.
- **Looping voices** end at `StopSound`, or when their session is replaced.
  **One-shot voices** end when their sink completes.
- **`SetSoundDuration`** remains metadata. It does not trim or stop playback.

## Native evidence

Static decoding of the Legacy client, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`, version
`1.29.2.9231`. Instruction windows are in local evidence only
(`.scratch/campaign-audio-20261020/start-backend.json`, from bounded Capstone
decoding checked against objdump). No original runtime was launched.

- `StartSound` backend `0x5d8fe0` returns early for a null sound. It reads the
  sound object's flags and calls a virtual slot `[vtbl+0x10]`. A nonzero result
  takes a separate path at `0x5d90a4`. That path resets `[+0x1d0]`, applies a
  profile at `0x5cb460`, and issues a new playback call `0x79f8d0`. A zero
  result takes the new-start path instead. The native code therefore reissues
  playback for an active sound; it does not return early.
- Inference: the reissue is a restart (old playback replaced by new). Not
  proven: the meaning of `[vtbl+0x10]` (its vtable is not located), the return
  contract of `0x79f8d0`, and whether the old buffer is cut or overlapped.
- `SetSoundDuration` `0x4a9cc0` then `0x4f70e0` makes no backend call (see
  [campaign-dialogue-audio.md](campaign-dialogue-audio.md)).
- `StopSound` wrapper `0x4ac2d0` (registration matched by `native_setter.py`,
  13 instructions, objdump boundaries matched) resolves the handle through
  `0x482fc0` and calls `0x4f7180(voice, kill, fade)`. Argument order follows the
  stack layout, checked against the callee's `[ebp+8]`/`[ebp+0xc]` reads.
- `0x4f7180`, with a backend voice at `[+0x24]`, calls `0x5db1e0(backend, fade)`.
  If `kill` is set, it then calls `0x5d7570` on the backend and clears the
  pointer, so kill stops at once. The no-backend path (`[+0x28]`, `0x7fbbd0`) is
  not decoded.
- `0x5db1e0`: with `fade=0`, it calls `0x7a06e0` (not decoded; presumed the
  backend stop), sets state 3 via vtable slot `0x18`, and records `[+0x18]=3`. With `fade!=0`, a query
  `0x79f510` gates the fade. A nonzero result enters state 4. A zero result
  takes the same hard stop as `fade=0`.
- State table `0xe90148` (state to entry index) and entries at `0xe90070`
  (16 bytes each, read from the image). State assignment `0x5cce80` reads the
  entry's first two dwords only: flag 1 selects the step at `[+0xc]`, flag 0
  selects `[+0x10]`. Entry 1, which state 4 uses, has flag 0, so the step is
  `[+0x10]`. Entry 1's fourth dword, `0x5b5ea0`, is not called by `0x5cce80`.
- Classification uncertain: the update loop `0x5bf1d0` calls a separate 16-byte
  table at `0xe90068` with `(group index, callback)` pairs: `(1, 0x5b5ea0)`,
  `(4, 0x5b6020)`, `(7, 0x5b5ea0)`, `(5, 0x5b6020)`. So `0x5b5ea0` is a
  group-level pass over the voices in groups 1 and 7, which reads each voice's
  `[+0x14]` step. Whether a voice in state 4 is the only one it affects is not
  established. The group membership of a state 4 voice is also not established.
  Runtime observation settles this; the static tables do not.
- Not established: the fade-out target value (the vtable slot `0x14` result for
  a stopping voice), whether state 2 silences, and what `0x7c6740` (under
  `0x79f510`) reports. The fade ramp is therefore native-qualified in shape only.
  The consumer's immediate stop for `fade` without `kill` is a recreation choice.

## Verification

- `pcx-play` host tests (non-Bevy, private target, `--locked --offline`):
  `cargo test -p pcx-play` passes 820 tests with 24 ignored. The new
  `stop_cancels_only_unconsumed_starts_of_its_sound` checks that stop drops only
  the stopped identity's pending starts, keeps order for the others, records the
  stop, and that a later start queues again.
- `peacecraft3` consumer tests in `crates/peacecraft3/tests/script_audio_session.rs`
  cover repeat restart without a second looping voice, live stop with reported
  unapplied fade, and same-frame start then stop. These need the Bevy build, which
  this change did not run (see the report for the exact command).

## Fade-out ramp status (not implemented)

Decoded, not measured (`.scratch/stopsound/f_5b5ea0.json`, `f_5c7d70.json`):

- The state 4 update `0x5b5ea0` reads the voice volume `v` through `0x79f410`,
  computes `s = v + step` with `step` from the voice `[+0x10]` (state 4 flag 0),
  and sets the new volume to `min_unsigned(s, t)`, where `t` is vtable slot
  `0x14` (`0x5c7d70`, returns `[[obj+0x138]+0x2c]`). It enters state 2 when the
  result equals `t`.
- `min_unsigned` makes a direction ambiguity. With `v == t`, a positive step
  gives `t` again, so state 2 follows at once and no ramp is visible. A negative
  step (unsigned wrap) decreases `v` until `v < |step|`, where the wrapped sum
  exceeds `t` and the result jumps back to `t`.
- Operand checks: every operand in `0x5b5ea0` is a 32-bit dword. The add
  wraps modulo 2^32, and the comparison is unsigned (`cmovb`). `v` comes from
  `0x79f410`, which returns the dword at `[handle_obj+0x144]` (`0x79f410` reads
  no other field). Its units and whether it is attenuation, inverse volume, or
  gain are not established. Miles receives `t` and the result as integers through
  `0x7a0310`. The formula is a min of an integer sum, not a verified gain curve.
- Fade integers: the constructor `0x5d6590` stores the JASS fade-in argument at
  `[+0xc]` and the fade-out argument at `[+0x10]` (`[ebp+0x18]`, `[ebp+0x1c]`).
  It then sets each to `max_unsigned(value, 1)` (`cmova` against 1). Negative
  inputs stay as large unsigned values, which the 32-bit add treats as negative.
  So the sign of the stored step equals the sign of the JASS argument.
- Direction consequence: with a positive fade-out argument (any `k >= 1`), the
  state 4 update raises `v` toward `t` and never ramps down from `t`. With a
  negative argument, `v` falls until the wrap jumps it back to `t`. Neither
  matches a stop fade by itself. Which one the shipped scripts exercise is not
  established.
- Not established: the writer of `[obj+0x138]+0x2c` (the field the start path
  also reads as its volume), and whether state 2 silences a stopped voice. Without
  these, no target or rate is qualified. The consumer stops at once and reports
  the unapplied ramp. No ramp formula is implemented.
- The discriminating evidence is a runtime trace: `StopSound(sound, false, true)`
  on a sound with a positive fade-out argument, recording the voice volume per
  update until silence or the voice ends. A static decode cannot settle it.

## Kill and replay (static)

- `0x4f7180` with `kill` clears the backend pointer `[+0x24]` and does not free
  the sound object. The JASS handle therefore stays valid (host tests cover
  this).
- A later `StartSound` on the same handle runs `0x4f6dc0`. With `[+0x24]`
  cleared, it takes the `[+0x28]` branch to `0x5d8f10`. Both fields are
  object pointers in this path, so the native replay depends on what `[+0x28]`
  holds. That was not resolved, and the consumer's replay after kill is not
  qualified against it.

## Audio update clock (static)

- `0x5dbe60` runs the update loop `0x5bf110`, then re-arms itself through
  `0x840370` with the float `0x3dcccccd` (0.1 s) and callback `0x5dbe60`.
- `0x840370` registers the timer through `0x840640` on the global `0x1160640`,
  then calls `0x846030`. The flag arguments to `0x846030` are literal zeros, but
  which argument selects real or game time was not decoded. Whether `PauseGame`
  stops this timer is therefore open. The legacy clock note
  ([legacy-simulation-clock.md](legacy-simulation-clock.md)) describes the same
  queue family as pausable game time, but this does not prove it for this timer.
- Consequence: a map pause that freezes this timer would also freeze
  native voice fades. The consumer's fades run on `Time<Real>` and do not pause
  with the map. This is a recreation difference until the flag is decoded.

## Out-of-range flag (not played)

`CreateSound` and `CreateSoundFromLabel` carry a `stop_when_out_of_range` input.
Static evidence from the piece02 spatial pass (local only:
`.scratch/spatial-root/EVIDENCE.md` in the piece02 worktree, same Legacy SHA-256):

- A per-frame range check at `0x5be320` runs only through vtable slot 4 of the
  backend voice class (`.rdata` `0xe90738`). It compares the squared distance
  with the stored squared cutoff, and returns 0 when the cutoff is less than or
  equal to the distance.
- Its caller, which consumes that result, is not identified. The action on
  out-of-range (stop, silence, pause, or resume on return) is therefore unknown.

Consequence for lifecycle: the flag has no modelled semantics. The consumer
refuses it as `out-of-range dispatch action is unidentified; not played`, and 3D
voices refuse at start. `StopSound` and restart do not imply an out-of-range
action. Any later spatial consumer must take that action as its own qualified
input.

## Remaining fidelity gaps

- Native `StopSound` backend and its fade/kill effects are undecoded.
- Native restart overlap, buffer cut, and the `[vtbl+0x10]` predicate are
  unresolved.
- Kill-when-done handle destruction is not modeled; the host keeps the handle.
- Spatial voices, out-of-range dispatch, EAX and parameter profiles remain refused
  by the consumer, so no voice exists for a stop or restart to act on.
