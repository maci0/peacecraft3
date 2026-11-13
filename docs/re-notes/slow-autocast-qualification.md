# Slow automatic-cast qualification

Selected client: Legacy `1.29.2.9231`, executable SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
These observations concern controlled diagnostic maps, not an unchanged mission
playthrough or complete native combat parity.

## Inputs and isolation

The source map is the locally supplied `(2)BootyBay.w3m`, SHA256
`e1c5984189cd046b112ae59b870cf1d585b2b2445368858ea64c7685d4180268`.
The copied diagnostic builder preserves its terrain and starting setup, disables
AI/victory, discovers the starting worker type and town-hall position, then
removes existing units and places controlled actors. Ability, class, buff,
orders, range and caster candidates come from the supplied unprefixed War3x
tables/profiles. Their extraction namespace alone does not establish the map's
active dataset. The runtime logs separately confirm RoC through `VersionGet`.

For alias `ACsw`, the diagnostic picks the lexically first source caster whose
ability list includes that alias, whose auto list includes its `Aslo` class, and
whose source `InBeta` cell is enabled: `ngrk`. This selection is a fixture choice,
not a production alias/unit mapping. Opponent types are discovered from the
original starting workers. Distances, health/mana fractions, observation counts
and explicit orders are experiment controls.

Each map's script and `(attributes)` script CRC are round-tripped through the
existing MPQ extractor. Source terrain identity is checked. All original assets,
diagnostic maps and extracted records remain local under `/tmp`. Each run uses a
fresh copied installation, network-disabled Docker, private Xvfb and a private
Pulse null sink; no host display/audio sockets are mounted. The receipt records
the executable, container image, harness and input fingerprints.

## First bounded observations

All seven first-round arms captured 128 ordered snapshots without overflow or
refusal and their owned clients stopped. Local receipt:
`/tmp/pc3-sol04-slow-native-status.json`; manifest:
`/tmp/pc3-sol04-slow-native-probes-1794512494542539729/manifest.json`.

The disabled control emitted no spell events. A manual Slow on an idle opponent
emitted CHANNEL and CAST inside the order call, before its accepted return.
EFFECT observed the old mana and no Slow buff. Later FINISH and ENDCAST observed
the debited mana and applied buff, and their spell-target identity was null.
The supplied caster's `UnitWeapons.castpt` and `castbsw` match the respective
windup and post-effect interval; they must remain source reads.

With automatic Slow enabled, idle opponents and moving-only opponents produced
no casts in this bounded setup. Opponents attacking the caster produced two
casts, with the second choosing the other opponent after the first carried Slow.
This is consistent with the selected zero-DataC attack gate, not evidence for a
generic nearest-hostile rule. Attack, turn, motion projection, cooldown and retry
timing all affect these traces; their aggregate event times do not isolate a
retry constant.

The two arms labelled as ties reverse creation order at mirrored positions.
Both initially select the negative-X opponent. Turning and projected movement
remain potential confounders, so this does not qualify equal-metric enumeration
or justify a coordinate/handle tie rule.

These probes used a long-running diagnostic timer. The paired timer audit in
`/tmp/pc3-sol02-clock-audit/report.txt` demonstrates that its elapsed query loses
precision relative to a short timer at the same callback. Consequently the
printed event times do not establish a universal scheduling grid. No deadline
rounding constant is inferred from them. Event nesting, old/new mana, target
identity and buff observations remain independently useful.

## Callback and heading controls

Three additional four-arm runs completed with 128 snapshots per arm and owned
clients stopped. Their local receipts are
`/tmp/pc3-sol04-slow-callback-native-status.json`,
`/tmp/pc3-sol04-slow-payment-native-status.json` and
`/tmp/pc3-sol04-slow-heading-native-status.json`.

Stop and caster death inside EFFECT emit a nested ENDCAST while the original
target and old mana are still observable. Payment occurs after the outer EFFECT
returns; neither applies Slow, starts cooldown nor emits FINISH. Removing the
ability has the same retained-payment behavior, with ability rank already zero
inside nested ENDCAST. Removing the caster clears the job without applying an
effect to its target. Removing only the target retains payment, cooldown and
normal FINISH/ENDCAST. Reissuing Slow at another target inside EFFECT returns
success but preserves the original cast and target.

Changing mana during EFFECT demonstrates that payment reads the postcallback
pool. A pool below the cost is clamped at its minimum while the accepted effect
and cooldown still commit. Positive rank changes and replacement/readdition of
the ability during EFFECT have not been qualified.

Heading controls place the same target east, north and west. The north/west
casts begin later, and public facing continues changing during the windup.
Public facing is therefore not evidence for an instantaneous logical-heading
threshold. The previous direct host's instant face assignment remains a fidelity
gap; neither a coordinate fit nor a permanent refusal closes turning behavior.

### Paired short-clock heading controls

The five-arm follow-up completed 640 ordered snapshots and 25 spell events.
Receipt: `/tmp/pc3-sol04-slow-heading-paired-verified.json`; manifest:
`/tmp/pc3-sol04-slow-heading-paired-probes-1794518018248068581/manifest.json`.
Each event records both an eight-second timer and the earlier long timer. The
short timer reports the east EFFECT at `0.470000` and FINISH at `1.030000`;
the corresponding long queries report `0.470093` and `1.030029`. These are query
differences, not a scheduling quantum.

North CHANNEL occurs at short time `0.110000`; west and north with the native
turn-speed getter multiplied by a synthetic half occur at `0.199999`. Halving
the native property-window getter leaves north CHANNEL at `0.110000`. The setter
takes the getter's raw radians; passing half the UnitData degree cell would be
a different experiment. Public north facing is still `10.313240` degrees inside
CHANNEL. Each owned container independently reports stopped/PID zero, disabled
network, read-only root, dropped capabilities and only its private harness/run
mounts. None mounted a host display or audio socket.

### Logical heading and the physics frame

Selected code distinguishes logical heading at CPoPos `+0x8c` from public
orientation at `+0xc8`, with visual angular velocity at `+0xcc`. Public
`GetUnitFacing` resolves the latter through `0x7fe970`; `GetUnitTurnSpeed` reads
`+0xb8`. The stationary turn path `0x8b00c0` uses that turn-speed field to clamp
one logical step. Its arrival helper `0x8aeb50` checks the independent spell
window before that step. The property window at `+0xbc` instead gates walking
in `0x8b0b90`. The controlled half-property-window result is consistent with
these separate roles.

Physics dispatch `0x8b5220` runs logical cluster work before `0x8b0900` visual
orientation. In the one-unit stationary path, `0x8aa740` records arrival before
`0x8ac620` commits the logical step, and `0x8ac3e0`/`0x8ad5a0` performs the
arrived relation cleanup afterward. The cleanup calls `0x8b15a0` and delivers
the movement completion message before visual interpolation. Thus the delayed
spell cannot be implemented by comparing public facing after a visual update.
The canonical timer-queue cursor now drives a retained logical/visual request;
equal-expiry ordering across callback families remains unqualified.

The physics timer is armed by `0x4ee0c0` from the global operand initialized by
`0x40d030` through native decimal conversion `0x9a1760`. The heading reader joins
that initializer destination to the timer operand, and follows the arrival
window, angle tolerance, half-turn and full-turn operands in their consumers.
It uses the native decimal operation order: convert the combined digits, then
scale through the existing selected reciprocal/multiply policy. It does not
parse the period with the host floating-point parser.

The `unit_activity::heading` rules expose the stationary logical step with
check-before-turn arrival and the separate visual phase. Fresh campaign
preparation attaches the policies to canonical activity. The native-clock route
retains the physics request's period and raw queue point. Logical turning resumes
the existing Slow order, queued callbacks drain, then visual interpolation and
periodic rearming complete the same retained phase. Public facing reads the
visual angle. Legacy clock callers retain their existing facing behavior.

The ordinary source profile follows the metadata registrations for turn speed,
property window and orientation selector, including their selected conversion
operands and typed marker default. Canonical creation, form changes, upgrades and
polymorph restoration retain these properties. Form replacement preserves the
logical angle, visual angle, angular velocity and target relation; capability
mismatch refuses before mutation. Movement and attack writers keep logical and
visual state coherent, but their instantaneous logical facing remains existing
recreation debt. Script timed turns on this clock still refuse explicitly.

The initial request anchor has a separate read-only native observation. The root
chain is `[[0x113b6f4]+0x1c]+0x2a8`, joined through `0x520d30`, `0x47ab10` and
`0x4ee0c0`. In the corrected capture, 1,838 physics records validate the timer
vtable, request back-reference and event; the first known request is already
armed while the game queue is zero, with deadline equal to its period. The 210
unavailable prefix records remain unknown. An earlier capture of root `+0x2a8`
without the game-state hop was entirely unknown and supplies no phase evidence.
Receipt: `/tmp/pc3-sol04-physics-initial-anchor-receipt.json`.

### Orientation arithmetic controls

The curve reader follows all sixteen selector entries in constructor
`0x4109f0`. It evaluates the constructor's reviewed stack/register forms and
decimal conversion, rather than copying its coefficients. The power path follows
`0x9a1be0`, logarithm `0x9a1960`/`0x9a0350`, and exponential
`0x9a14c0`/`0x9a0460` with the existing software arithmetic. A private native
`Pow` control retains 65 binary32 results across a synthetic unit interval;
all match exactly. Witness: `/tmp/pc3-sol04-heading-power-witness.json`.

A second private observation resolves the script handle through the selected
game-state handle pool and AGILE object registry to CPoPos. The helper uses only
process query/read access, verifies the position object's setter/reset vtable
entries, and reads clock and orientation fields twice around each sample. It
does not call, inject into, suspend or modify the client. Of 1,200 readable
samples, 1,178 have identical brackets. Adjacent-pair exclusions retain 44
unstable brackets, 82 changed logical headings and 1,029 unchanged orientations.
The remaining 44 transitions have a stable logical heading, selector and queue
cycle, and an interval shorter than the source physics period. All 44 angle and
angular-velocity results match the reconstructed visual step bit for bit.
Witness: `/tmp/pc3-sol04-heading-visual-witness.json`; raw observations:
`/tmp/pc3-sol04-heading-rpm-reference-read-facing-1794522844194114247/native-heading.jsonl`.
The owned client is stopped; its receipt independently checks private mounts,
disabled network, read-only root and dropped capabilities.

These observations caught two arithmetic-order errors that ordinary floating
point tests missed. The shortest-arc helper `0x8a19b0` subtracts current minus
goal before sign reversal/wrap; reversing operands changes the selected signed
truncation result. Angle normalization `0x8aead0` uses remainder `0x9a1a90`
when the magnitude reaches a revolution. Its fractional reciprocal product is
not equivalent to subtracting a revolution. The nonnegative remainder sequence
is shared with timer query arithmetic, and heading admission joins its table and
arithmetic calls to the selected numeric policy. This does not qualify general
JASS modulo behavior.

The direct `SetUnitFacing` control also exhibits later logical-heading writes
while the caster has no public order and fixed position. Those transitions are
retained as unknown controller/idle work, not fitted as a constant visual goal.
This control therefore qualifies the isolated visual step, not the complete
idle/order-resume phase or its writers.

### Direction and callback controls

The source vector-heading path uses software square root, reciprocal division
and table arccosine (`0x998640` / `0x9a05f0`), rather than script `Atan2`.
The reader retains the regular and near-edge table relationships, interpolation
operands, mirrored-negative branch and source pi operand. A fresh private
original-client control records 169 arccosine cases across the range, both branch
boundaries and values near either endpoint. The corrected witness is
`/tmp/pc3-sol04-heading-acos-v2-witness.json`. The first encoder used JASS
approximate equality for zero and censored two small positive results; its failed
comparison is retained and does not justify changing the native arithmetic.

Four public session regressions use source arithmetic with deliberately nonstock
unit properties, cadence and queue cycle. They cover order resumption before the
visual phase, canonical EFFECT-before-payment/buff settlement, Stop during the
turn, sticky callback failure without replay, and request rebasing across two
cycles. A separate nonstock form-profile regression verifies property restoration
without losing dynamic heading state. These are implementation/lifecycle checks,
not evidence for native timing with those synthetic values.

Checked runtime receipt:
`/tmp/pc3-sol04-heading-runtime-validation-1794528041563949235/report.json`.
All 15 check groups passed against unchanged source hashes: 15 heading tests
(including 169 native arccosine values, 65 power values and 44 stable orientation
transitions), four public physics/Slow tests, one form-profile test, 27 morph,
35 curse, 10 structure-upgrade, seven clock, 164 activity, 17 timer and four
timer-query tests, plus the retained 216-value timer witness. Strict native/WASM
Clippy and both library builds passed. Earlier fixture failures, the censored
encoder control and the superseded Clippy failure remain in their local logs.

This route depends on the candidate native-clock platform. Normal campaign
adoption remains separate while script/AI wake domains are qualified. Native
clock selection must occur before startup script orders when that platform is
adopted: a later first paced tick cannot undo synchronous legacy events already
published during startup. The public diagnostics explicitly select the native
driver before issuing orders. Existing Slow effect/backswing deadlines also
retain their previous clock representation; cross-family equal-expiry ordering,
moving-turn behavior and positive automatic selection are not established here.

## Selected-code findings and remaining gates

Slow candidate filter `0xbf7c20`, evidence
`ev_e9dcd1079529fac94010a7c184f236580704a43cbe956974964894b80055ca9b`,
first applies the common filter, mobility gate and existing Slow-class check.
A nonzero DataC accepts after those gates; equality with the selected zero
requires attack-component state and its current-target owner relation. The old
autocast note reversed the DataC branch. The complete owner-relation and
disabled-component mapping remains to be qualified.

The shared comparator's strict-smaller branch retains the incumbent at equal
metrics (`0xb9cb20`, evidence
`ev_ca0b223a71c43c3188fd639dc162a57e40409a4defb09706476dc62a458bf0f3`).
The outer filter calls `0x7ff1e0` with its projection argument enabled. That path
uses clock-derived interpolation (`0x8ae7c0`/`0x8b7660`) and the spatial base plus
scaled delta (`0x613110`) before the normalized squared metric. Reusing an
unqualified center-distance or collision-edge approximation would lose this
relationship. The shared enumeration order, visibility and readiness still
require their own canonical mapping.

The shared Slow order path now queues CHANNEL/CAST synchronously and EFFECT
before payment/buff application. Retained payment survives interruption; exact
game-clock boundaries deliver EFFECT and source-backed backswing completion.
Other Curse classes retain their previous behavior. Public-order synthetic
regressions cover normal phases, stop/death, retarget/target removal,
ability/caster removal, skipped triggers, callback errors and pause/resume.
The callback-error and skipped-trigger tests establish host cleanup guarantees,
not original-client script-error parity.

Selected-code dossiers are retained locally in
`/tmp/pc3-sol04-slow-native-evidence.json`. The Slow numeric gate joins the ranked
getter `0x46eed0`, data loader `0x6ace00`, typed numeric getter `0x95d220` and the
comparison/default initializer. Nonnumeric table markers use the bound numeric
default; malformed authored numeric values remain explicit errors. Automatic
positive selection, common visibility/readiness, equal-metric enumeration,
complete moving/idle turning and full mission behavior remain open. Parser and synthetic tests alone
do not establish native parity.

## Bounded implementation receipt

`advance_slow_autocast` proves no candidate only when the bound ranked numeric
gate requires an accepted attack target and every canonical unit has admitted
attack state without one. Each visited unit consumes work. Missing attack state,
an attacking unit, a nonzero gate and unqualified caster lifecycle refuse.
This family evaluator does not select a nearest target or consume an error as a
successful evaluation. The common periodic dispatcher and its native timer queue
domain remain separate integration work; this change alone does not enable
positive automatic casts.

Local validation receipt:
`/tmp/pc3-sol04-validation-1794517450346458864/report.json`. On the isolated
worktree, `cargo test -p pcx-play --lib combat::curses:: --jobs 2 --offline`
passed all 32 tests. The autocast filter with `--include-ignored` passed 46 tests,
including both selected-image gates and these new nonstock derivations:
`relocated_numeric_gate_and_ranked_column_follow_nonstock_source_operands`,
`inconsistent_ranked_column_default_and_attack_branch_refuse` and
`empty_acquisition_data_uses_the_bound_scalar_and_nonzero_is_not_idle_rejection`.
Exact named public Slow regressions include
`slow_public_order_effect_callback_precedes_payment_and_source_timed_finish`,
`slow_effect_payment_reads_callback_mana_and_the_bound_nonstock_minimum`,
`slow_effect_ability_withdrawal_or_caster_removal_does_not_leave_or_replay_jobs`,
`slow_effect_callback_error_restores_context_and_does_not_replay_payment`,
`slow_marker_policy_preserves_strict_readers_and_refuses_malformed_numeric_data`
and `slow_no_target_evaluation_charges_every_unit_and_refuses_a_potential_attacker`.

Native and `wasm32-unknown-unknown` library Clippy checks with warnings denied,
both library builds, touched-file formatting and whitespace checks passed. The
runner verified unchanged fingerprints of all gameplay Rust sources before and
after each Cargo check, and required named new tests to run. Tests used a
verified private Xvfb and ALSA null configuration. This receipt covers the
bounded lifecycle and negative evaluator, not unchanged-map or rendered parity.
