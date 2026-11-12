# Canonical Human construction

## Evidence and source admission

The selected Legacy client is identified by SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The local classic source receipt resolves War3Local, War3, then Deprecated
archives, and compares the supplied unit/Misc records with that resolution.
Original inputs, instrumented maps, captures and receipts remain under the
ignored `.scratch/human-construction-probe/` tree.

`construction_rules` follows public native registrations and the independently
reviewed x86 virtual-method roles to join max/current life, the build-in-progress
class constructor and initialization multiplication operand. It reads the
initial life fraction from the mapped operand, rather than a measured constant.
The HumanBuild class-name getter, virtual code getter and constructor derive the
ability identifier independently. The rules retain executable/common digests,
edition/version identities, ordered Misc sources and typed refund/drain inputs.
Unsupported discovery stays an explicit construction capability refusal; it does
not prevent unrelated map startup.

The worker's authored `Builds`, building cost/duration/food/prerequisites,
collision, movement and placement fields use the retained catalog. The build
menu reads the class ability's unranked profiles through existing typed profile
readers and the admitted map overlay. Matching its source AbilityData race with
the worker's UnitData race, together with Builds membership, is the current
recreation association. Native implicit enrollment ordering remains unqualified.
The generic table/overlay path does not require a hero skill capability.

## Original runtime observations

Five bounded original-client arms preserve the same loaded-map baseline and
source inputs. Their raw PreloadGen records, strict collectors and hash receipts
are local. The observations are distinct from parser tests:

- A public point order immediately debited the bank before worker arrival. Its
  boolean result was false despite committed travel and real construction.
- START callbacks observed the canonical building at full life. Subsequent
  samples observed fractional initialization and ordinary construction growth;
  undamaged completion followed the source build duration. Food capacity began
  at completion, with no second cost debit.
- Public STOP after START halted work; no drain was observed over the bounded
  stopped interval. The Misc drain setting alone does not establish a Human
  stopped-site drain rule.
- Actual UI cancellation of an undamaged site returned the source refund-rate
  share, delivered CANCEL with zero life, then retired the same building.
- Public STOP and public KillUnit before arrival returned the complete reserved
  cost, with no site, START, CANCEL or FINISH event during the observation window.

These arms do not qualify damaged refunds, multiple builders, native placement
snapping, arbitrary terrain requirements or construction presentation.

## Canonical implementation

A pending request belongs to the existing worker record. Its paid reservation
transfers to the existing building record on arrival. There is no second unit
store, and public gameplay does not call the internal arrival factory directly.
Readiness distinguishes ordinary unavailable choices from malformed admission.
Pending requests reserve production limits without providing completed-unit
technology. A changed placement aborts the reservation without publishing a site.

Preview and arrival share footprint/grid/occupancy and authored placement checks.
The worker's own occupancy is excluded. The `unbuildable` mapping uses the WPM
format's no-build byte flag, independently documented in the
[w3z-editor format constants](https://github.com/LeoYawoo/w3z-editor/blob/master/War3TypesAndConstants.h),
not a building-specific default. The initial approach uses the source
footprint/collision bounds and worker collision, quantized to the admitted grid.
Source `bj_UNIT_FACING` supplies the recreation's default facing; its equivalence
to native construction facing is unqualified. Preview and creation use the same
quarter-turn footprint orientation. Mobile structure construction refuses.

`IssuePointOrderById` preserves the observed false result. The worker's monotonic
request revision is the committed-command witness for local input recording.
The source registered repair target order resumes a stopped, paid site with one
worker. Ordinary movement/STOP and supported attack/follow interruption detach
participation; accepted harvest publication uses the same interruption helper.
Pending death/removal cleanup releases its reservation; active
worker/site retirement removes participation without creating a replacement.

START uses normal event contexts and response getters. After the last START
callback, initialization re-reads the current maximum life and applies the
admitted fraction. Legal removal/cancellation during a callback skips settlement.
Already executed callback effects are not rolled back by later numeric faults.
Food, technology, training, casting, automatic combat and unit caching exclude
incomplete sites. FINISH clears the marker before its callbacks observe benefits.

## Limits

Ordinary work uses source-duration interpolation; native polling/sub-tick
arithmetic is not claimed identical. Native snap/approach fidelity and the
source-default-facing equivalence need further qualification. The Human and unattended
Undead styles, stationary footprints, supported placement tokens and zero building
food reservation are accepted. Shoreline placement, other styles and multiple
builders refuse. Worker style is selected by matching the authored worker race and build list
to the ability's source race; native implicit enrollment is not established. The unattended
worker is released at site creation, while the probe only bounds the original release between
successive one-second samples. A worker with an existing build request/participation must stop
before beginning another. Damaged cancellation refunds remain an explicit refusal,
including damage healed before the next work sample. Fractional refund rounding
uses the admitted numeric product and truncation; native rounding parity is not
qualified by the integer-valued cancellation observation. The
native order association of the UI Cancel button is not claimed; the canonical
cancel operation has its own tested lifecycle. Construction actor/animation
presentation and a complete recreation Human02 playthrough remain separate work.

Synthetic tests exercise public point-order reservation/travel/events/completion,
STOP and death refunds, repair resumption, placement invalidation, production
limits, callback life changes/removal/cancellation, incomplete benefits and
unranked nonstock menu profiles. A separate nonstock test covers unattended site
completion after release of its builder. The original-client observation of an Undead
builder and its site through completion is recorded in [AI construction](ai-construction.md).
These tests do not establish rendered behavior.

## Terminal sites and repair enrollment, 2026-11-13

The completion scheduler now excludes a site with nonpositive life or a dead
combat record. Previously an unattended site's retained incomplete marker kept
producing future completion deadlines after `KillUnit`, even though work
projection correctly refused to grow the dead site. Death keeps the paid cost
and does not publish FINISH or provide completion benefits. Repair arrival also
checks the canonical target before attaching participation: a dead, canceled,
initializing or autonomous site cannot acquire a worker, and an arriving worker
cannot replace the site's current worker. A refused arrival clears its
zero-cost travel request through the existing arrival path.

`SetUnitOwner` on an incomplete site now explicitly refuses before producer
cleanup or ownership mutation. Previously it could retain the old owner's
participant and fail the next construction update. Native incomplete-site
transfer behavior remains unqualified; this refusal is a capability boundary,
not a claim that the original client rejects ownership changes. Transferring a
worker continues to refund its pre-arrival reservation to the original owner or
stop its participation in an already paid site.

The evidence boundary remains the selected 1.29.2.9231 executable digest above,
the retained source-derived costs/durations and the Human and Undead original
observations described here and in `ai-construction.md`. No new original-client
run establishes death, competing repair arrival or owner-transfer timing in this
change. These are canonical-state/scheduler consistency fixes and an explicit
unsupported-state refusal. Synthetic regressions exercise terminal wake removal,
repair enrollment, unchanged paid costs, lack of FINISH, atomic transfer refusal
and worker transfer before and after arrival; they do not establish native tick
parity or rendered behavior.

Validation used the branch's isolated target directory: `cargo test -p pcx-play
--lib construction --jobs 2 --offline` passed 56 tests with the explicit
local-source test ignored; running `selected_original_construction_rules` with
its supplied local inputs and `--ignored` passed separately. All five new
lifecycle regressions ran and passed. Native and `wasm32-unknown-unknown`
`cargo check -p pcx-play --lib` passed with the workspace's warnings-as-errors
policy. Every run used the verified private display and null audio sink.

Independent main integration validates all five named regressions in the full
`pcx-play` package suite: 3,140 passed, zero failed, 143 explicit ignores.
Strict native/WASM all-target Clippy, native/WASM library builds, formatting
and the native campaign-probe rebuild pass. Per-command exit receipts and
unchanged source hashes remain under `/tmp/pc3-construction-main-validation/`.
These gates establish canonical lifecycle consistency, not native transfer,
multiple-builder behavior, polling cadence or rendered parity.
