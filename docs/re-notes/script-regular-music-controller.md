# Regular music controller candidate

This is an incomplete candidate. The controller, independent audio random stream,
source reader and ordered host journal are available only under `test` or
`test-fixtures`. Explicit diagnostic journal configuration connects regular and
source-qualified mixed music operations to VM dispatch; normal builds retain
their existing regular-playback refusal.
Application playback is not connected. Previously accepted thematic music and
regular volume behavior is unchanged.

## Evidence and candidate behavior

The selected executable is Warcraft III 1.29.2.9231, SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The reader requires this image identity, checks call/global relationships and
derives operands from its bytes. It reuses the canonical random policy and
generator rather than copying random tables or measured values.

The source joins support these candidate rules:

- `5d8af0` resolves an edition-qualified name, tokenizes its source delimiters,
  makes one multiply-high random selection for a negative index (including a
  single track), and reduces a nonnegative index modulo track count.
- `5d8c30` cancels pending callbacks before its same-file check. A playing
  regular stream at offset zero is retained; filename comparison reaches the
  source ASCII folding routine through `433490` and `a99c10`.
- `5d8560` retains a paused regular stream. `5d99f0` resumes its position;
  `5da400` seeks the regular stream independently of thematic music.
- Regular completion `5db520` advances sequentially and schedules a delayed
  next start. Thematic completion `5dbb60` clears its stream and tail-calls
  `5d99f0` with the fade-in mode.
- `840370` arms against `GetTickCount` through `796c90`. Pump `845ce0` captures
  wall time once and removes an entry before invocation. Fade `5ce420` records
  its first actual callback time, rearms from current wall time, and completes
  strictly after its duration. It does not synthesize missed intervals.
- Audio initialization `5d6f70` seeds the independent shared stream from wall
  time. Generic sound-list choice `5cf3a0` differs from music selection: a
  single member draws nothing, and multiple members have bounded repeat retries.

These are native-code observations, not original-client playback observations.
The proposed pre-frame presentation pump is a recreation ordering choice; the
complete map-worker callback relationship and unrelated equal-time callbacks
remain unverified. No original client was launched during this implementation
phase. Prior volume LUT and null-sink results do not prove this playback family.

## Actual checks

Checks ran against `/tmp/pc3-lane07-regular-playback-worktree` with isolated
target `/tmp/pc3-fidelity-sol61-07-playback-target`, two build jobs, a verified
private Xvfb display and ALSA null output:

- `cargo test -p pcx-play --lib regular_ --jobs 2`: 13 passed, three ignored.
  This includes six controller cases, two synthetic source-reader cases and
  five existing thematic/volume cases. Log:
  `/tmp/pc3-lane07-regular-controller-draft/core-second-tests.log`.
- `cargo test -p pcx-play --lib audio_random_ --jobs 2`: three passed. The
  first attempt failed before Cargo because the sandbox blocked Xvfb; that
  failed receipt is retained. The authorized isolated rerun passed at
  `/tmp/pc3-lane07-regular-controller-draft/audio-random-after-isolation.log`.
- `selected_client_supplies_regular_music_controller_rules`, with the local
  selected image explicitly supplied and `--ignored --exact`: one passed.
  Log: `/tmp/pc3-lane07-regular-controller-draft/selected-rules-first.log`.

The final checkpoint adds fixture-only gates after those runs. No strict
native/WASM build, browser execution or app test of this checkpoint was run.
The browser typed getter and its test gate are candidates, not validated runtime
admission. The unlinked `music_wall.rs` prototype is not a called wall producer.

## Work still required

Connect a bounded ordered host journal, startup inputs and the actual application
controller before admitting any new native. The application-global audio epoch
must precede menu choices, survive map changes, use the same stream for supported
sound choices, and retain explicit source/backend identities. Keep wall input
separate from Game/REAL clocks and speed scaling. Browser page-origin time is an
explicit adaptation, not native boot-seed parity.

Browser admission must inspect the typed getter on every boundary: legacy archive
append clears attestation without changing revision; failed/cancelled candidates
retain the old revision and Arc; identical-content reimport changes process
revision while retaining content generation. Absence/errors must retire old HTML
music too. All three cases still need actual generated-WASM execution and a
logical menu/campaign/return history test. No shared runtime or JavaScript audio
ownership changes have been implemented.

Pause/resume cursor retention, decoder seek failures, late/wrapping wall delivery,
fade gain ordering, regular/thematic overlap, source retirement and session
replacement need called app and null-sink tests. The candidate's disabled/pending
filename state and automatic selected-map startup ordering need further source
qualification. Acknowledgement counter/cooldown families, full root persistence,
audible amplitude and native callback jitter remain open.


## Ordered host journal and plain-play source flow (2026-11-17)

The diagnostic host journal preserves every accepted Play/Ex, Stop, Resume and
regular seek in execution order. Its caller supplies a memory bound. Checked
sequence numbers survive prefix retirement; a full queue, invalid argument or
invalid acknowledgement leaves the pending commands intact. The consumer must
acknowledge only after accepting an operation. Resolution failure therefore leaves
its command and later commands available for retry. Existing last-request getters
retain the raw script arguments, including absent plain-play timing parameters.
This does not install a presentation consumer or qualify playback success.

Static inspection of the selected executable at `4a41a0` establishes the
plain-play argument flow: after the guard call, `TEST EAX,EAX` and `JNE` skip the
play call for a nonzero result. Fallthrough pushes the tested zero register for
both timing arguments before string conversion and the bound `5d8af0` call.
The reader validates the branch, pushes and direct calls. The zero is derived
from the tested-register condition; it is not a copied asset record. Synthetic
mutations reject altered branches, pushes and call targets. A nonstock timing
fixture verifies that diagnostic dispatch uses its supplied policy. This is
static instruction/control-flow evidence, not an original-client observation or
a completed runtime replay guard. A fresh deep function analysis timed out;
that attempt supplied no additional evidence.

The controller also validates a replacement thematic path before retiring the
accepted voice or changing its regular fade. Invalid empty, NUL-containing and
source-boundary paths preserve both voices and callback state.

Fresh checks use `/tmp/pc3-continue-music-target`, two build jobs and the private
helper under `/tmp/pc3-continue-music-receipts/`. The helper verifies its Xvfb
display and default ALSA null configuration before launching any test. The first
sandboxed attempt failed before launching tests; the isolated reruns succeeded.
Source hashes are frozen in `core-inputs.sha256` in that directory.

- Final `regular_` filter: 18 passed, zero failures, three explicit local-input
  skips (`regular-final-tests.log`).
- Entire music host/controller/rules/random family: 29 passed, zero failures,
  four explicit local-input skips (`music-family-tests.log`).
- Exact selected-client controller rules gate with the canonical executable
  explicitly supplied: one passed, zero skipped (`selected-source-direct.log`).
- Strict native and WASM `test-fixtures` checks passed (`check-final-core.log`
  and `wasm-check.log`).
- Native and WASM `test-fixtures` library builds passed (`native-build.log`
  and `wasm-build.log`).

These results supersede no earlier playback receipt. The older temporary evidence
paths above are historical and were unavailable in this environment. Retained
sinks, source retirement, menu-to-map epoch ownership and browser execution still
need qualification before production admission.


## Mixed music order (2026-11-17)

The same diagnostic journal now includes Theme, EndTheme, ThemeSeek, Volume,
Select and Clear. Each hook validates and reserves a command before updating its
accepted snapshot. Publication and sequence advancement follow that preflight;
queue exhaustion preserves the accepted thematic revision/position, selection,
regular volume revision/level and pending prefix. Without a journal, the existing
snapshot behavior is unchanged. Missing required policies, incompatible client
image identities and unsupported thematic starts refuse before publication.
Ignored thematic seeks remain source-defined no-ops. This adds no event bus,
clock transport, startup admission or application playback consumer.

The selected image's `SetMapMusic` wrapper at `4a9340` forms its effective index
independently of the plain-play wrapper: OR with an all-ones immediate establishes
a negative selector; CMP/CMOVE replaces it with the authored index only when the
random flag is zero. The reader checks the comparison, conditional move, pushes,
string conversion and `5da520` call, and reads the selector separately. Synthetic
mutations reject changed control/data flow. The raw selection snapshot continues
to retain the authored random flag and index, including when random is true.
This proves the wrapper argument flow, not complete native selection behavior.

A twelve-call VM test consumes the actual ordered commands through the controller
and checks retained regular serial/cursor across a thematic start, independent
seek, volume and selection, thematic end, clear, regular replacement and resume.
The expected start and retirement order is checked as effects, alongside the
intermediate voice states. Overflow tests exercise every mixed snapshot boundary;
missing and mismatched policy tests verify refusal before publication.

With the same private display/null audio helper and two build jobs:

- Music family: 32 passed, four explicit local-input skips
  (`mixed-family-tests.log`).
- Existing map/thematic snapshot family: eight passed, one explicit local-input
  skip (`mixed-map-regression.log`).
- Exact canonical selected-image reader gate: one passed, zero skipped
  (`mixed-selected-source.log`).
- Strict native and WASM fixture checks passed (`mixed-native-check.log`,
  `mixed-wasm-check.log`).
- Native and WASM fixture library builds passed (`mixed-native-build.log`,
  `mixed-wasm-build.log`, structured exits in `mixed-build-status.json`).

The frozen code manifest is `mixed-inputs.sha256` under the same private receipt
directory. These remain diagnostic controller/host results. The actual retained
sink, decoder/seek failure handling, source epoch, browser ownership and normal
startup still require the application qualification described above.
