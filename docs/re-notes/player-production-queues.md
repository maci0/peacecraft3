# Player production queues: source and behavioral contract

This is the implementation contract for extending the existing replacement
production lifecycle. The shared-state implementation now admits queued player
training orders and explicit queue cancellation, with producer-death refunds.
Presentation, input delivery, removal and transfer remain unfinished. It uses
the canonical ObjectHost units, banks, food accounting, clock, placement and
event delivery, with an optional guard-post binding for replacement jobs.

## Qualified original behavior

Selected executable SHA-256:
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The runtime observations use the original client under Wine/software rendering,
with RoC explicitly selected. They do not establish Windows or browser parity.
Original assets, generated maps, captures and complete analysis records stay
local under `.scratch/player-production/`.

The bounded direct-order probe derives its producer and trainable type from the
retained source setup. It starts no AI and omits inherited unit creation and
initialization triggers. Twelve immediate training requests are issued through
`IssueImmediateOrderById` using the unit type's FourCC. The first seven accept;
the rest refuse without further spending. Every accepted request spends its
authored money cost immediately. Only the active job reserves food and emits
`EVENT_UNIT_TRAIN_START`. Pending entries neither reserve additional food nor
count as living units in `GetPlayerTechCount`. `GetUnitCurrentOrder` remains zero
during this production queue; it must not be used as the queue model.

The public string conversion `OrderId("cancel")` returns zero in this selected
client. Sending that string does not cancel a training job, emit a cancellation
event or refund money. A recreation must not invent a successful script order
for it. Queue-UI cancellation is a separate operation on the same production
state.

The repeat UI run uses the same frozen map bytes, a canonical local-player
producer and captured queue controls. Actions occur before any unit finishes:

| Action | Observed result |
|---|---|
| Enqueue the source queue | All accepted money costs are charged; one active food reservation and one start event. |
| Click the last pending entry | One refund and one cancellation event; active job and start count stay unchanged. |
| Click the active entry | One refund and one cancellation event; the next job starts and emits its start event. Food remains one active reservation. |
| Allow that job to complete | One living unit and finish event; the following job starts. Used food includes the completed unit plus the new active reservation. |

These observations qualify the ordinary single-type case. They do not prove
mixed-type callback ordering, explicit producer removal, ownership transfer,
revival or construction behavior. Separate food-blocked-head and producer-death
observations are recorded below. The earlier manual UI
rehearsal is retained separately; its late pending-entry click is not used as
the bounded cancellation-event proof.

Receipts:

- `queue-probe/native/result.json`: exact 49-marker direct-order inventory.
- `queue-probe-ui-r2/native/result.json`: exact 481-marker UI-repeat inventory.
- `queue-probe-ui-r2/native/actions-observed.json`: before/pending/active action
  rows, with separate input and screenshot receipts.

Both clients were stopped and preserved before collection. Maps/scripts and
staged executable identities are checked. The map and common preludes parse
and resolve; every packed map member round-trips. Collectors reject malformed
wrappers, identities, fields, nonfinite values, incomplete inventories and
invalid terminal sequences. The initial collector's assumption that a public
cancel order must exist was rejected by the actual zero conversion result; the
failed collector is retained rather than treating that request as a success.

## Source bindings for the implementation

Native inspection is retained with the selected image and Ghidra 12.1.3 analysis
identity. Pseudocode is recovered analysis, not original source. Critical
operand relationships also appear in the recorded instruction streams:

- `0x632190` resolves `SimpleBuildQueueBackdrop`, creates a separate active
  control and a bounded pending-control collection. Its pending allocation and
  loop agree. The selected pending count is six; it is source data to decode,
  not a production constant.
- The cancellation walkers at `0xb56f90` and `0xb57390` independently carry the
  selected slot count seven (`0xb56fbf`, `0xb573bd`). Read and cross-check these
  operands against the UI relationship and runtime boundary; do not copy seven
  into a queue limit.
- The ordinary cancellation branch reads `TrainRefundRate` through the source
  profile getter, separately from `ResearchRefundRate` and `ReviveRefundRate`.
  Ordered `Units/MiscGame.txt` and `war3mapMisc.txt` profiles supply this value.
  The implementation must preserve edition, source identity and map overrides.
- The ordinary refund path converts the unit money cost through `0x9a1620`,
  multiplies through `0x99ffe0`, converts the product through `0x9a07b0`, then
  calls the money-credit wrappers. The conversion's inspected mantissa/sign
  operations truncate finite in-range values toward zero. Apply conversion per
  cancelled entry, rather than multiplying a sum of queue costs. Reuse the
  selected numeric policy; do not substitute an assumed full refund or host
  float rounding. Fractional override runtime discrimination remains required.

The complete evidence records are `rea-session-queue-strings.json`,
`rea-queue-ui-function.json`, `rea-refund-function.json`,
`rea-single-cancel-function.json` and `rea-refund-integer-conversion.json`.
No native wrapper layout or linked list needs to be reproduced to implement
the observed queue faithfully.

## Food-blocked pending head observation

A separate preserved `queue-probe-food/native` run lowers the food cap to the
current food used after all seven requests have been accepted. The original
client completes the active unit, keeps the next entry pending, and emits no
second start while at the cap. Restoring the cap starts the next head and adds
its food reservation without another money debit. The strict collector retained
481 markers; `food-transition.json` checks the bounded wait and restore against
those rows and pins the result hash. The run was stopped after collection.
This tests a previously paid pending head, not an initial request made at a full
food cap. Same-callback activation timing and producer interruption still need
separate connected recreation checks.

## Producer death observation

A separate `queue-probe-death/native` run kills the producer after seven paid
requests and before the first finish. Money returns to its initial value, active
food is released, and no train-cancel or finish event is emitted. The bounded
80-tick run retained 161 markers; `death-transition.json` pins the result and
checks the post-death state through the terminal sample. This does not qualify
explicit removal, owner transfer or a fractional refund override. The host now
prepares this distinct death refund before death mutations and
clears production without replaying UI cancellation events. Explicit removal
and owner transfer still need their own qualified lifecycle handling.

## Fractional producer-death refund observation

The separate `queue-probe-death-refund/native` run adds an explicitly experimental
map override, `[Misc] TrainRefundRate=0.37`, while retaining the producer and unit
source setup. Killing the producer refunds 602 gold across seven entries;
truncating the combined cost would yield 608. Food returns to zero and no cancel
event fires. `fractional-refund-transition.json` pins the strict 161-marker
result, derives each cost from the observed debits, and discriminates per-entry
conversion from aggregate conversion. The generated map and original inputs
remain local. This qualifies the positive fractional death-refund case; negative
rates, overflow classes and UI cancellation with this override were not tested.

The host's `cancel_unit_training(producer, slot)` prepares the source refund,
food release, successor activation and combined event capacity before mutation.
Pending cancellation leaves the active deadline unchanged. Active cancellation
starts the successor only when food permits. Invalid slots and repeats return
false. The operation is not exposed as an invented script order. Tests use
nonstock fractional rates and costs, exercise missing numeric-policy refusal,
and distinguish counted living units from pending entries. Hero training refuses
through the source cost classifier until token transitions are implemented.

## Connected implementation and acceptance

1. Generalize the existing training state into a per-producer FIFO with a
   separate pending/active phase and optional guard binding. Keep one unit and
   player store. Decode the source queue bound; retain application work and
   allocation budgets as separate limits.
2. Admit a request through the producer's resolved `Trains` list, unit
   prerequisites, current owner limits and money balance. Spend money once at
   enqueue. Activate only the head, with its food reservation, source `bldtm`,
   game-clock deadline and start event. Do not reserve food for every queued
   entry. Qualify the food-blocked-head case before choosing that transition.
3. Complete through existing canonical unit creation/placement and train-finish
   delivery; activate the next head without publishing duplicate debit or
   callbacks. Use the existing pause and clock boundaries, with interruption
   regressions. Explicit producer-removal/transfer refusal remains an open lifecycle
   gap, not a completed cancellation implementation.
4. Add queue cancellation by index as a shared-world operation used by UI and
   input/replay delivery. Prepare the source-derived per-entry refund, food,
   queue/event capacity and replacement-head work before mutating state.
   Pending cancellation must not restart the active job. Do not map it onto the
   unavailable public `"cancel"` string order.
5. Add command-card train buttons from resolved unit metadata (`Art`,
   `Buttonpos`, tooltips and hotkeys), resource/prerequisite availability and
   the producer's `Trains` relationships. Render the canonical queue and progress
   through source frame geometry, textures and bindings. Keep training's
   reported current-order behavior separate. Building work animation and
   progress presentation need rendered comparison, not parser-only claims.
6. Cover nonstock metadata/cost/time/slot/refund values; queue-full and
   insufficient-resource refusal; callbacks and retries; pending and active
   cancellation; pause; blocked food; placement; producer death/removal and
   transfer. Use real public training commands and UI dispatch in tests. Finish
   with a connected unchanged-map gather/train/cancel/fight scenario and the
   existing main-menu/campaign regression.

Hero training/revival, research, construction and race-specific workers remain
the subsequent parts of the economy stage. This contract does not close that
stage or the first-mission playthrough requirement.

## Research refund policy

Research entries in the shared queue now use `Misc/ResearchRefundRate` for
explicit cancellation, producer-death cleanup and queue-detail refunds. Ordinary
training retains `TrainRefundRate`; structure upgrades use their separate policy
below. Training and research values
come from the ordered source profiles, including map overrides; a missing
research rate refuses a research refund before money, life or queue mutation.
It does not substitute the training rate. Conversion remains per entry and
uses the admitted numeric policy, with finite in-range results truncated toward
zero.

Evidence class: native instruction inspection with GNU objdump of the selected
1.29.2.9231 executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
In the queue cancellation walker `0xb56f90`, the research branch pushes the
`ResearchRefundRate` key at `0xb570d2`, calls the profile getter at `0xb570e0`,
and reads level-priced gold/lumber through `0x6df7e0` / `0x6e0a30` at
`0xb570f8` / `0xb57136`. Integer-to-real, multiply, truncation and credit calls
are `0xb57102` / `0xb5710f` / `0xb57115` / `0xb57121` for gold and
`0xb57140` / `0xb5714d` / `0xb57153` / `0xb5715f` for lumber. The rule reader
checks those call targets and reads the profile-key pointer from the image;
it does not copy either refund rate into production constants.

Synthetic regressions distinguish training rate 0.37 from research rate 0.61,
check cancellation and producer death exactly once without awarding a level,
check the queue-detail refund, and reject a missing research policy without
changing the bank, queue or producer life. Parser checks cover ordered overrides,
missing/malformed rates, truncated instructions, source identity and numeric
conversion. These are recreation and source-binding checks; no new matched
original-client research cancellation/death runtime run was performed. Research
events, player research commands and transfer/removal semantics remain open.

## Structure-upgrade cost and refund policy

The selected executable's upgrade admission method `0xb63580` and cancellation
method `0xb63ad0` distinguish `Misc/RelativeUpgradeCost`. When enabled, the target
unit's gold and lumber fields are the job cost. When disabled, both methods
subtract the original unit type's corresponding fields. Target-price mode skips
original-cost metadata reads, matching the conditional native getter path.
The queue now derives
that amount for each chosen producer before spending, retains the paid amount
in the canonical job, and uses it for explicit cancellation refunds. Stock upgrade loss and transfer
follow the separately qualified policy below.
The target's authored build time remains the job duration; a zero cost does not
make a job instantaneous. Ordinary training and research retain their own cost
and refund paths.

Evidence class: GNU objdump 2.47 instruction/data inspection of the selected
1.29.2.9231 image identified above. Admission reads the mode key at `0xb635c5`
and calls its integer profile getter at `0xb635d2`; target cost calls
`0xb635af` / `0xb635bc` are negated. With the mode disabled, original cost calls
`0xb635e1` / `0xb635ec` are added before bank calls `0xb63622` / `0xb63633`.
Cancellation reads the same key at `0xb63b5d`, obtains target costs at
`0xb63b4b` / `0xb63b56`, and conditionally subtracts original costs through
`0xb63b79` / `0xb63b84`. It calls the upgrade rate method through virtual slot
`+0x350` at `0xb63b38`; the inspected table entry `0xf24f28` points to
`0xb63df0`. This method reads `UpgradeRefundRate` at `0xb63dfd` and also reads
`UpgradeDamageRefundPenalty` at `0xb63e11`. The rule reader checks the profile-key
operands and the relevant getter, cost, conversion, multiplication, truncation
and bank call targets against the pinned image.

The ordered game/map profiles supply all three upgrade fields, with no training
rate substitute. Missing fields and unsupported boolean classes report errors.
An enabled damage penalty refuses admission before spending: its native damage
accumulator is not yet modelled. Negative price deltas likewise refuse as an
unqualified class rather than inventing a clamp or credit policy. With the
penalty disabled, conversion uses the source numeric policy and per-entry
truncation, and current life does not scale the refund.

Nonstock regressions use distinct producer/target prices and separate training,
research and upgrade rates. They exercise both cost modes, affordability at the
derived delta, source duration and completion, damaged life with the penalty
disabled, pause, cancellation/death/repeated cleanup, queue-detail refunds,
missing policy and enabled-penalty refusal. Parser checks cover layered profiles,
invalid/missing flags, negative deltas, overflow and the explicit local original
image/profile binding. These checks do not establish original-client upgrade
runtime parity. Damage-penalty support, transfer/removal, upgrade events and
intermediate type/pool transitions remain open.

Validation of this slice: 86 training/construction lifecycle tests and five
source-rule tests passed in a dedicated target directory, including the
explicit original-image/profile test. Native and WASM library builds and strict
compile checks passed with workspace warnings denied. The final frozen-source
binary reran all 86 lifecycle and five rule tests under a private offscreen
display with null audio; source hashes matched before and after the run.

## Stock structure-upgrade interruption qualification

A separate bounded original-client diagnostic qualifies the stock Human hall
upgrade under the selected 1.29.2.9231 image above, Wine/software rendering and
explicit RoC. It retains the shipped Booty Bay terrain and main initialization,
disables melee AI/victory, pauses other initial workers and runs five controlled
producers. This is a modified diagnostic, not unchanged-map parity or an
encountered campaign frontier.

The local receipt is
`/tmp/pc3-sol61-05-upgrade-qualification/runtime-status.json`; the immutable raw
trace SHA-256 is
`6b4f2843dfaf253779b48998eb6f6f18216aeabb58083e382f2d32b75da14500`.
Its 2,039 strict records include five accepted upgrades, ten lifecycle events,
bit-exact real encodings and one complete terminal inventory. The collector
checks nonce, field shapes, sample/event counts, bit controls, absence of
unexpected damage and a completed control. The client was stopped; independent
harness status reports exited, not running and PID zero. Container-private Xvfb
and a Pulse null sink were verified; no host display/audio device was mounted.
The generated map, original bytes and capture remain local.

Source selection matters. A generic editor query opens expansion/base tables
and does not select the map's gameplay data. The production regression
`campaign_inputs::local_original_upgrade_catalog_and_policy_match_the_roc_runtime_witness`
opens the exact diagnostic map through `CampaignRecipe`, calls the existing
production `defaults` loader and resolves its `MetadataCatalog`. The actual
map-header selection is `Melee_V0`. Its source prices, upgrade mode and refund
rate reproduce the observed debits/refund; its authored duration plus the
observed pause interval lies in the adjacent old/new sample bracket. Matching
`Custom_V0` cells alone were not selection evidence. No production cost/time
constant was added, and no production edition-loader defect was found.

| Operation | Native observation |
| --- | --- |
| Pending upgrade and pause | The same handle retains the original type and pools until completion; pausing suspends progress. |
| Complete a damaged producer | Life percentage is retained: the control's 750/1500 becomes 1000/2000, rather than healing fully. |
| Explicit numeric cancellation | The independently inspected immediate operands at `0xb481b9` and `0xb482b8` identify the accepted cancellation command. One upgrade-cancel event fires, the original type/pools remain, and the source upgrade refund is credited. The public `OrderId("cancel")` still returns zero. |
| Kill an upgrading producer | No refund or upgrade-cancel/finish event. Its death callback observes the target type and target maximum life, with current life zero. |
| Remove an upgrading producer | No refund or upgrade/death callback. The type still reads as the original immediately after removal and becomes invalid on a later sample. No upgrade completes. |
| Transfer an upgrading producer | Neither bank receives a refund. The same paid job continues and finishes under its new owner. |

The recreation now prepares the target definition, model, abilities, pools,
decay, collision placement and checked revision before committing a replacement.
Completion preserves the current life percentage, using the admitted life
rounder when the maximum changes. Unchanged maxima retain current life exactly.
Mana transitions outside the observed zero-pool class refuse before debit.
Missing life-rounding policy also refuses a damaged, changed-maximum transition.
Cancellation keeps the original type/pools. Removal discards upgrade work
without a refund or replacement. Transfer retains the original paid cost and
deadline and changes the canonical job owner only when the unit owner commits.

Upgrade death prepares the target's actual death/explosion animation and decay
before any kill cleanup, commits its prepared inactive placement and target
form, then uses canonical death delivery. Existing XP/reincarnation planning is
retained only for structure-to-structure, nonhero replacements with equal
ability/level, flight domain and experience-level classes; other classes refuse before cleanup.
This does not fix the canonical kill path's preexisting unrelated late failures.
Ordinary training and research keep their independent cleanup/refund policy.
Recipient cancellation uses the retained paid price and the current owner; the
native cancellation method obtains that owner at `0xb63b3e`. A separate native
recipient-cancel run has not been performed.

Synthetic regressions exercise nonstock maxima and source rounding, pause,
recipient completion/cancellation, removal, event-visible target death pools,
repeated cleanup and malformed target pool/placement refusal before mutation.
These tests do not prove every native tick, rounding boundary, renderer or
platform. The original finish event's exact subsecond scheduling remains a
separate clock qualification; the source-duration check uses adjacent samples.
Enabled `UpgradeDamageRefundPenalty`, upgrade events in the recreation,
upgraded/researched pool bonuses and other changed death award/ability classes
remain unfinished. The enabled penalty retains its explicit admission error.
Signed price credits are qualified separately below.

Mixed training/structure-upgrade queues also refuse before spending; their native
admission behavior has not been qualified.

Removed-handle `GetUnitTypeId` still returns a strict host error; the native
delayed zero result is recorded above and is not claimed as implemented.


## Signed upgrade prices and remaining damage accounting

The selected 1.29.2.9231 image remains
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The following qualification is static native-code and shipped authoring evidence,
not a new original-client observation or a campaign encounter. Both original
and target costs can be valid nonnegative authored amounts while their upgrade
difference is negative. `UnitMetaData.slk` admits that nonnegative cost domain.
The original metadata and analysis exports stay local.

The root upgrade admission method at `0xb87ce0` subtracts original gold and wood
when the selected `RelativeUpgradeCost` requests differences, then compares each
signed price with the corresponding bank. It has no negative-price rejection.
The in-progress admission at `0xb63580` negates those differences and credits or
debits the banks. Cancellation at `0xb63ad0` multiplies the signed differences by
the selected upgrade refund rate and truncates each result toward zero. Its bank
path, `0x4b5820` / `0x4b5860` through `0x4b58a0`, clamps an exhausted bank to zero.
A cancellation can therefore charge back part of a previous credit even after
that credit has been spent. It does not require enough money to cancel.

The recreation retains these signed prices in the existing canonical job.
Admission checks both resource requirements before committing either bank.
Explicit cancellation uses the retained price and current owner; underfunded
negative refunds clamp to zero, and positive credits use the existing selected
resource scale and upper bound. Training and research keep their own negative
cost refusals. Missing source policy, invalid bounds and unsupported integer or
scaled overflow refuse before resource or queue mutation. The native arithmetic
can wrap outside this admitted range; that wider class remains unsupported.
Death and removal still discard the job without a refund, and completion keeps
the admission credit. No new job state or codec fields are introduced.

Synthetic public-lifecycle tests cover mixed signed gold/wood prices, independent
affordability, a varied scale and cap, recipient cancellation after spending,
completion, death, removal, repeat cleanup and source/overflow refusal. They do
not establish original-client tick parity or every intermediate bank fraction.
Static evidence is retained locally in
`/tmp/pc3-sol61-05-upgrade-signed-damage/native-evidence-2.json`:
`ev_41909c80a387df5ef8729a1435567b86b751345eff45e65e1218689e7af6be7b`
(admission), `ev_8869393344c0c8603471bd1d608d578c0c1c0693d1d06ebe3b804540f2fd8f8f`
(payment), and `ev_7263d07f11b61939b4a36616e89b1707ddcd04e477e53b49116ee55283bc2710`
(cancellation). Ghidra pseudocode is a derived view; signed comparisons and
arithmetic were checked against the selected image's instructions.

Enabled damage penalties require a different connection from final health loss.
The child constructor at `0xb641c0` initializes its FloatMini tracker at
`+0xa4/+0xa8` from a source-initialized zero. The FloatMini assignment at `0x470120`
stores directly without clamping. Handler `0xb63820` adds damage-event `+0x10`
when the penalty is enabled. Its stage selector `0xb63810` returns the first
phase, dispatched at `0x66796d` before armor, ethereal multipliers and later
ability phases in `0x6677c0`. Counting the recreation's final health damage would
therefore be incorrect. Refund method `0xb63df0` applies the selected rate times
one minus tracker divided by the unit's maximum life, without a local factor
clamp. Zero denominators and unusual signed/nonfinite event classes still need
qualification; no guessed saturation or health-loss substitute is admitted.

The constructor and direct write are qualified, but allocation/reuse across every
cancel/restart path is not yet established. Methods `0xb63fa0`, `0xb64070` and
`0xb64120` serialize, restore and visit the tracker; they are not reset evidence.
The existing combat callers generally supply resolved damage, so enabling this
policy needs its incoming phase connected through real callers first. See
`ev_3e211b0c00ff42c9fcfdb96cf3300cd7c35d00e6dce9b10425765b7d765b8eb8`
and `ev_1d940d7af3445d1869dcb0a4e2a1c796bd49d473be09674839ff7a7a708f3be4`.

`UpgradeInProgressIdChange` selects distinct native pending-form/pool paths in
`0xb63580`, `0xb638f0`, `0xb63ad0` and `0xb63d30`. The Melee_V0 control uses
the old-form pending path; shipped Custom_V0 enables its sibling. The earlier
strict enabled-mode admission guard is superseded by the bounded lifecycle
qualification below. Missing and malformed selectors still refuse before price,
credit or queue mutation. Training and research policies remain independent.
Upgrade start/cancel/finish publication and public numeric cancel dispatch remain
open in the recreation; the original captures record their contract.


Upgrade-only scalar semantics are isolated inside the upgrade policy result.
Textual, nonfinite and list-shaped values for the upgrade refund rate, relative
cost mode, damage penalty or pending-form selector reject structure upgrades
without disabling ordinary training or research admission and cancellation.
Global profile syntax, identity, budget and common refund-rate errors remain
strict failures. Regressions exercise each upgrade key with `bogus`, `NaN` and
`1,0` through the shared parser and actual production callers, checking rejected
upgrades leave the producer, queue, banks, handles and RNG unchanged. This fixes
the outer-parser error introduced by selecting an additional upgrade-only key;
it does not admit any malformed upgrade value. Enabled-mode behavior is qualified separately below.

The explicit local gate
`campaign_inputs::selected_original_upgrade_modes_use_production_profiles_and_isolate_the_pending_selector`
now follows the production recipe, defaults and metadata catalog for three probe
maps. The header's selection bit comes from `ImageRules` and the selected image.
The Melee_V0 / Custom_V0 comparison records each source policy separately; a
second Custom_V0 arm changes only the pending-form selector through the normal
map-misc override. Its catalog and every other selected policy remain equal to
the first Custom_V0 arm. Inputs and the local receipt are bounded and hashed.
Raw mana markers are preserved rather than assigned an assumed default.

These are local diagnostic profiles, not campaign encounter evidence. The gate
qualifies source selection and probe setup only. Map/member CRC round trips,
JASS resolution and collector self-checks precede runtime admission; none proves
an original-client result. The subsequent runtime qualification below retains its own receipts; damage refunds remain unsupported.


## Enabled pending-form lifecycle qualification

The three prepared maps ran separately under the selected 1.29.2.9231 executable
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
Wine and software rendering. Every admission used a private verified offscreen
display, null audio, read-only container and disabled network. Each arm produced
1,819 strict records, ten lifecycle events and five accepted upgrades. Collection
ended before flushing the buffered diagnostic. The collector independently
revalidated all 5,457 records against the frozen map/profile identities and
actual production-catalog prices and refund policy. This is controlled original
runtime evidence under Wine, without a Windows hardware or tick-parity claim.

Immutable local captures and receipts remain under
`/tmp/pc3-sol61-05-upgrade-enabled-prep`. The source join is
`source-receipt.json`; raw validation is `capture-revalidation.json`;
`independent-stop-receipt.json` verifies each owned container exited with PID zero,
no OOM and no remaining reference-container process. Raw trace SHA-256 values:

- Melee_V0: `ab6c9ace7c3ec3580a5c43a0241223804f3b65cd4b37de8ab92d2505ccd253b8`.
- Custom_V0: `4531eb6e68ac6f32798be01b14d8e76067367777101ac2c9b18037f5d5c8f56a`.
- Custom_V0 with the disabled selector:
  `581c482d9044c8874bb5d109ba059e9946a99c222da8cc0e17e7741cf42ea39b`.

The third arm retains the Custom_V0 catalog and every other policy, including its
cost mode; only the pending-form selector changes through the normal map-misc
override. Its observed kind, pools, armor and interruption behavior matches the
Melee_V0 control. This comparison isolates the selector from edition pricing;
the first two arms alone differ on several source axes.

With the selector enabled, the same handle exposes the target type immediately.
Its pending maximum life stays at the original base maximum. Completion adds the
source-scaled target-minus-original base-life difference to both current and
maximum life, preserving the absolute missing life. The disabled control instead
retains the original public type and completes through its qualified life-ratio
path. Pause suspends the paid deadline in either mode.

Enabled-mode cancellation restores the original type and preserves the current
pending pools, then credits the retained paid price at the selected upgrade
refund rate to the current owner. Transfer continues the same job. Death exposes
the already-published target type with the pending original maximum and zero
current life; it does not run the disabled-mode target-pool replacement. Death
and removal discard the job without a refund. No second completion or refund
occurs after interruption.

Native-code evidence supplies the enabled form replacement at `0xb63580`, the
public raw type read at `0x497d30`, and the type write before its ID-change event
at `0x6c06e0`. Completion at `0xb638f0` reads owner-scaled base life through
`0x67b6c0` and uses the additive pool mode in `0x6be8c0`. The source-bound health
rounder remains required. Native armor/defense reads and writes at `0x67b640`,
`0x67bbd0` and `0x680070` explain retaining the original armor during the pending
form and applying the source armor difference at completion. The sampled stock
pair has equal armor; a changed-armor runtime comparison remains unqualified.
Prior native exports remain under
`/tmp/pc3-sol61-05-upgrade-pending-mode/native-evidence-4.json` and its analysis
snapshot. Decompiler output is supporting evidence, not recovered original source.

The called implementation retains `Training.original_kind: Option<i32>` only for
enabled-mode jobs. Every cancellation and completion rebinds that rawcode against
the admitted catalog; the early public target type cannot recover the original
identity. Pending form/placement/ability/pool/decay and payment preparation precede
publication. Cancellation prepares the original replacement and signed refund
before any job, form or bank change. The retained option also belongs in a future
complete training-job save codec; no save support is implied here.

Support currently requires a positive or zero base-HP difference, zero mana,
unchanged pool/regeneration/armor bonuses, matching source ability lists and
levels, nonhero structures and the qualified flight class. The same source policy
continues to reject enabled damage refunds. Unsupported negative-HP, mana,
researched-pool and inventory-capable classes refuse explicitly. Research and
recipient-transfer pool projections validate pending jobs before any multi-unit,
research-level, finance or ownership publication. A changed handicap similarly
refuses before owner-row or unit mutation; its no-op remains available. Public
ability-list changes refuse before ability/Slow-state withdrawal, with native
no-op add/remove behavior preserved. An unadmitted inventory family has no public
item mutation path; later inventory admission must also qualify every published
pending job against its proposed source policy.

Nonstock regressions cover damaged fractional life, pause/deadline retention,
current-owner signed cancellation, death-event target identity and pending maximum,
removal, compatible transfer, bad original-catalog retry, low funds and failed
placement. Unsupported research/transfer/handicap changes check all affected unit
records and player state before recovering by finishing or cancelling the original
job. These tests establish the recreation's error and lifecycle contracts; the
original traces above establish only the recorded original observations.

Remaining qualification includes negative life deltas, mana/researched pool
transitions, changed-ability/reset classes, changed armor in the original runtime,
all upgrade events and numeric cancellation routing, and enabled pre-armor damage
refund accounting with tracker reset/reuse. The original three-arm captures did not record technology queries; their separate
count-manager qualification follows below.


## Pending-upgrade counters and same-callback removal

A later matched Custom_V0 pair changes only `UpgradeInProgressIdChange` and
records public technology and player counts before the request, while pending,
immediately after `RemoveUnit` in that callback, and from two later independent
timer callbacks. Both original arms agree: the original specific count is zero,
its nonspecific count is one, both target counts are zero, and the structure is
incomplete rather than completed. Removal retains these pending counts in the
same callback. Both later samples return zero. The unchanged nonstructure unit
counts are a positive control; source-derived debit and absence of a removal
refund are independently checked. No sampled delay becomes a game rule.

The local frozen input/source/CRC/JASS/collector receipts and 48 strict records
remain under `/tmp/pc3-sol61-05-upgrade-count-removal`. Its two named production
source gates passed before admission. Raw trace SHA-256 values are
`10f601e2d0db27263d471acd8734ae0538915e406620634d539fdcc669d7d149`
and `0ca9c293181b9e13853cb30c7e80d98cdd43bb681173ed5f85bfe2304b39a8e1`.
The independent stop receipt verifies both containers exited with PID zero, no
OOM, unchanged raw captures and an empty reference-container inventory. The
runtime environment and selected executable identity are the same as above.

The source join is consistent with the observations: `0xb63580` reaches
`0x6800b0` with original/target types and incomplete registration in either mode.
`GetPlayerTechCount` at `0x494bb0` reaches `0x513620`, which reads the completed
per-type field; traversing an authored upgrade edge also includes its incomplete
field. `0x5149d0` adds an incomplete count, while `0x513000`/`0x513510` add or
complete a completed count. Fresh Ghidra startup exceeded its deadline during
this follow-up. The additional bounded read-only GNU objdump observations and
selected-image SHA-256 are retained in
`/tmp/pc3-sol61-05-mode1-lifecycle/counter-evidence.json`; they are native-code
evidence, independently corroborated by the original query pair.

The shared query now distinguishes completed providers from pending providers
reached through an `Upgrade` edge. `DependencyOr` keeps its authored relationship
semantics. It uses the canonical job target in both modes, so an early public
type cannot unlock its own technology. Production prerequisites consume this
same query. Player counts exclude the upgrading producer from the completed
base and include its canonical pending job once when requested.

Before discarding an upgrading job, removal retains a typed technology snapshot:
`RemovedUnit.technology: Option<RemovedUnitTechnology>`, with `Inactive`,
`Completed` and `PendingUpgrade([u8; 4])` categories. `None` preserves unknown
liveness. Completed snapshots use the existing public type; pending snapshots
retain the exact job target independently of the old or new public type.
Technology and incomplete player counts consume it only through the existing
removed-unit retirement boundary. Unknown liveness refuses while retained; retired
rows are excluded before demanding their optional category, so they cannot poison
later queries or hide other living providers. Public removed-unit type/life semantics stay
separate. The existing clock-based retirement is a recreation phase model, not
proof of every native reclamation tick. The future arena codec must retain this
category and target along with its existing owner and removal time.


## Pending food columns and food-use transitions

The selected start handler `0xb63580` routes both modes through counter flags
`0x101`. In `0x6800b0`, the source type's used-food term is removed when its bit
is set, and the target's `fused` is enrolled unconditionally. The subsequent
`0xb489a0(owner, original, 1)` reads `0x690270` (`fmade`, type-cache `+0x60`) and
credits the original food capacity. The target food-used reader is `0x6902a0`
(`fused`, cache `+0x5c`). These roles agree with the independent unit-type query
bindings in [food queries](script-unit-food-queries.md) and
[food-use flags](unit-use-food.md). Bounded disassembly and image hashes remain in
`/tmp/pc3-sol61-05-mode1-lifecycle/food-source-evidence.json`.

Canonical pending jobs now supply the target type for used food and the original
type for capacity. The same parsed source amounts serve either pending-form mode;
there is no second counter cache. Start and explicit cancellation re-enroll the
existing food-use bit after all fallible preparation. Completion preserves its
current state: the shared completion path's `AND fffffff6` / `OR c12` leaves that
bit intact. Its later unit virtual `+0x8c(4, 0x200, 1)` is the source primary
animation selector `0x666a20`, not a food-bit setter. The selected vtable word and
callee domain are independently recorded in
`/tmp/pc3-sol03-upgrade-completion-slot8c.json`.

A source-column or catalog error and a checked pending/final aggregate overflow
refuse before payment, job publication, cancellation credit or owner mutation.
Nonstock regressions vary both food columns, pause the job, disable food before
start and again before cancellation, and check completion with the bit still
disabled. Ordinary training/research contracts remain separate.

Two interactions remain deliberately unsupported. In the old-public-type mode,
an actual food-use toggle with different original and target `fused` subtracts
the public type's amount from a target-enrolled native counter. The existing
membership set cannot represent that residual. Such an actual toggle refuses
before mutation; true no-ops remain available. An ownership change with different
original/target used food or capacity also refuses before recipient projection or
finance, pending qualification of the original owner's counter adjustment. The
same-owner no-op and equal-food transfers retain their prior behavior. These are
recreation admission boundaries, not claims that the original rejects the calls.

Changed-food runtime comparisons, food-sensitive order admission and player-state
event timing need further original qualification. Generic same-callback removal
of food contributors also retains existing debt: native food counters can remain
visible until retirement, while `live_food` currently excludes removed records.
The new removed technology snapshot qualifies technology/player unit counts; it
is not a food-counter snapshot and does not claim food-removal parity.
