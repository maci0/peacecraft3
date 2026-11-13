# Timed unit facing on the native clock

Selected input is Legacy `1.29.2.9231`, executable SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
This checkpoint implements the handler's bounded direct-facing branch. Longer
duration controllers and full native turn parity remain open.

## Source and actual callers

The `SetUnitFacingTimed` registration at `0x49d476` binds `0x4ab440`, retained
in dossier `ev_e5e3c4f14214af284f9c355fa024ddf441d2bcee115f28664a2b6e9f0dc35cdc`.
Its initial comparison reads the duration threshold through the operand at
`0x4ab447`. A duration above that threshold branches to a controller path;
equal or smaller durations use the same angle conversion and logical-facing
setter as `SetUnitFacing`. The profile decoder reads the threshold from the
selected image, joins both degree-factor operands and multiplication calls,
and checks both setter calls including their false reset argument. It does not
copy a measured duration into a production constant.

The bounded source verifier and read hashes remain in
`/tmp/pc3-sol04-timed-facing-source-receipt.json`; cached native procedures are
in `/tmp/pc3-sol04-timed-turn-native-evidence.json`. No new original-client run
is claimed for this checkpoint. Existing logical/visual source evidence is in
[Slow qualification](slow-autocast-qualification.md).

Actual Counter startup on unchanged Human01 and a later Counter action on
Undead02 previously refused at Blizzard.j's timed-facing wrapper. Their local
logs are `/tmp/pc3-sol09-normal-counter-human01-third.log` and
`/tmp/pc3-sol09-normal-counter-undead02-first.log`. A separate read of the
supplied War3Local map scripts finds both short and longer-duration calls in
Human01, and longer calls in Undead02. Input hashes, MPQ offsets and call sites
remain in `/tmp/pc3-sol04-timed-turn-caller/caller-receipt.json`. Those script
calls establish required scope, not which later mission calls have executed.

## Canonical behavior and verification

For an admitted native heading, the qualified branch uses the existing facing
preparation and commit path. It changes logical heading, allocates no linear
turn job, and leaves public orientation to the existing physics visual phase.
The profile capability is required before publication. A duration above its
limit still refuses before unit or clock mutation. Legacy facing behavior
retains its separate existing path.

`timed_facing_threshold_follows_relocated_source_and_rejects_branch_or_setter_drift`
uses nonstock thresholds and relocated operands, and rejects changed comparisons,
degree conversion, reset argument, setter linkage and nonfinite/truncated input.
`counter_preentry_timed_facing_in_globals_config_and_main_keeps_native_visual_state`
selects Counter before VM globals and executes each startup phase through the
public timed-facing native. Public orientation remains at its previous value
until physics publishes it.

`counter_timed_facing_instant_boundary_preserves_logical_visual_phases_and_refusal_atomicity`
passes negative, zero, immediately-below-limit and equal-limit durations through
a public session. It drives actual Counter inputs using the admitted fixture's
quantum, verifies later visual publication without spell events or buffs, and
checks that the next representable duration above the limit preserves both the
unit record and full clock state on refusal. These are source-derived synthetic
caller checks, not original-client timing observations.

Frozen-source validation in
`/tmp/pc3-sol04-timed-facing-validation-1794559224667871464/report.json` passes
all three named timed-facing checks, 16 heading checks (including the retained
65 power, 44 visual and 169 arccosine witnesses), all 47 Curse checks and 164
activity checks. Strict native/WASM Clippy and both library builds pass, with
unchanged gameplay-source hashes. Earlier fixture admission/signature failures
and the initially insufficient Counter input remain in their separate logs.

## Remaining controller path

The longer branch computes the shortest arc from public visual facing, divides the
requested duration by a source string-derived period, and derives an absolute
per-frame rate. `0x800140` constructs a static target point from projected
position plus source-scaled sine/cosine components, records a rate override and
submits a cluster relation. The existing source math policy already binds those
trigonometric routines; host trigonometry or a linear interpolation would lose
the native arithmetic and point quantization.

Controller ownership, replacement/cancellation, moving/paused units, arrival and
order-resume interactions still need their canonical join before this branch is
enabled. This checkpoint adds only an immutable source-profile field and no new
resident turn or save state. An unchanged-map Counter rerun must report its exact
next frontier; this branch alone does not establish normal startup completion.

## Negative-angle admission

An actual Human01 Counter rerun after the short-branch checkpoint reached the
setter's separate negative-angle guard. Its failure is retained in
`/tmp/pc3-sol09-normal-counter-human01-short-timed.log`. Undead02 instead reaches
the still-unsupported longer-duration branch after publishing its initial units;
that separate failure is in
`/tmp/pc3-sol09-normal-counter-undead02-short-timed.log`.

The existing canonical normalizer already implements the source negative branch.
The selected setter calls `0x8b14b0` then `0x8bc3d0`, which stores the result of
`0x8aead0` into logical facing. That normalizer first applies the source remainder
for magnitudes at least a full turn, then adds the source full-turn operand when
the signed result is below the initialized numerical zero. The decoder now joins
those setter calls, remainder and addition calls, full-turn operands, and zero
initializer. The redundant negative-angle refusal is removed; the shared prepare
boundary explicitly requires a finite angle. No arithmetic, resident state or
Legacy turn behavior changes. Selected-window hashes are retained in
`/tmp/pc3-sol04-negative-facing-source-receipt.json`.

The nonstock normalization regression checks negative and signed-zero results
with the fixture's deliberately different reciprocal table. The new
`negative_normalization_binds_relocated_operands_setter_and_arithmetic_calls`
rejects changed branch, field, relocated operand, initializer and arithmetic
relationships. The public Counter boundary regression now includes negative and
multiple-revolution angles and both zero signs, with full unit/clock preservation
on nonfinite input and unsupported longer duration. The globals/config/main test
uses negative angles in all three startup phases. Frozen-source validation in
`/tmp/pc3-sol04-negative-facing-validation-1794560271249514917/report.json`
passes three timed checks, 17 heading checks including the retained native math
witnesses, 47 Curse checks, 164 activity checks, strict native/WASM Clippy and both
library builds. This is source and recreation-caller evidence; it is not a new
original-client negative-angle capture or a completed Human01 startup.

## Bounded long-controller runtime controls

Four private original-client controls retain 1,024 short-clock observations in
`/tmp/pc3-sol04-timed-controller-runtime-receipt.json`. Source-map, script, CRC,
roundtrip and syntax receipts are in its referenced manifest. Each capture checks
the nonce, ordered complete sample sequence, edition, fixed position and current
order. All four clients independently report stopped/PID zero/no OOM; the fresh
ancestor queue is empty in
`/tmp/pc3-sol04-timed-controller-independent-stop-receipt.json`.

The stationary origin and translated controls preserve position and current
order. A replacement timed request changes the turn direction. A direct facing
setter leaves the timed relation active; a later Stop cancels the relation while
the visual orientation settles afterward. These controls cover a stationary
worker only. The symmetric requested direction does not discriminate every
position-quantization case. Moving/paused units and complete order-resume behavior
remain open, and no long controller is enabled by this receipt alone.

The replacement also corrected the earlier logical-facing rate interpretation:
the handler reads virtual slot `+0x1c` on the unit's `+0x26c` wrapper. The selected
constructor installs its table at `0x6759e6`; that slot resolves `0x7fe970`, which
reads public orientation `+0xc8`. Logical physics still advances `+0x8c`. The rate
read point must preserve that distinction when the controller is implemented.
