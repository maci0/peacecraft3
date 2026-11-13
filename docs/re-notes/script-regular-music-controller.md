# Regular music controller candidate

This is an incomplete checkpoint, stopped for integration wrap-up. The new
controller, independent audio random stream and source reader are available only
under `test` or `test-fixtures`. They are not connected to native dispatch or
application playback. `PlayMusic`, `PlayMusicEx`, `StopMusic`, `ResumeMusic` and
regular seek retain their existing explicit refusal. Previously accepted thematic
music and regular volume behavior is unchanged.

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
