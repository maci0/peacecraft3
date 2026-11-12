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
turning and full mission behavior remain open. Parser and synthetic tests alone
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
