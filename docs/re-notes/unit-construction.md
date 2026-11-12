# Canonical Human construction

## Evidence and source admission

The selected Legacy client is identified by SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The local classic source receipt resolves War3Local, War3, then Deprecated
archives, and compares the supplied unit/Misc records with that resolution.
Earlier observations referenced `.scratch/human-construction-probe/`; that
directory was not retained in the current checkout. New local runtime receipts
and their separate scope are recorded below.

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

That slice initially made `SetUnitOwner` on an incomplete site explicitly refuse
before producer cleanup or ownership mutation. Previously it could retain the old owner's
participant and fail the next construction update. Native incomplete-site
transfer behavior was then unqualified; the refusal was a capability boundary,
not a claim that the original client rejects ownership changes. The later Human
qualification below replaces that blanket refusal. Transferring a
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

## Human ownership and assistance qualification, 2026-11-13

Four new controlled original-client runs use the selected 1.29.2.9231 executable
above and the supplied `Maps/(2)BootyBay.w3m`, SHA-256
`e1c5984189cd046b112ae59b870cf1d585b2b2445368858ea64c7685d4180268`.
The tracked `tools/original-game-reference/reference.py` and `session.sh` run
the Windows client under Wine, private Xvfb and a verified PulseAudio null sink.
The container has no network, host display/audio socket or hardware device.
This is original executable behavior under Wine, not native Windows validation.
Every container was stopped after collection.

The controlled map keeps the supplied terrain and main setup, disables melee AI
and victory termination, and uses synthetic funded banks. It discovers the town
hall type, position and repair workers from the native starting units, then
orders the same building through `IssuePointOrderById`. No measured cost, life,
duration or world coordinate is used to configure the probe. The modified script
and MPQ attribute checksum round-trip through the independent local extractor;
terrain bytes are unchanged. An earlier stale script checksum caused native
ERROR 131; those failed runs are preserved and excluded from this evidence.

The local manifest is `/tmp/pc3-sol06-native-probes/manifest.json` (nonce
`1794510342`). The runtime receipt beside it records exact map/script/trace
digests, executable identity, actions, checkpoints and diagnostic slopes.
`/tmp/pc3-sol06-summarize-native.py` independently verifies the four loaded map
and executable hashes, trace identities, one START per run, ordered samples
1–128, retained site identity, bank preservation and stopped intervals. It
passed. Baseline, assist, site-transfer and worker-transfer collected 524, 531,
541 and 540 records respectively, with quarter-second samples. All generated
maps, extracted content and raw records remain local under `/tmp`.

| Controlled action | Observed original behavior | Implementation boundary |
| --- | --- | --- |
| Change the paid Human site's owner at game time 10 | Same site and life; both banks unchanged. Owner-event and immediate-after-call samples see the new site owner while the old owner's worker still reports repair. By 10.25 its order is zero and life has reached a stable plateau. | Preserve participation synchronously; ordinary work revalidates ownership and releases it without refund. Exact delayed native cadence is open. |
| Old owner's worker requests repair at time 14 | Returns false; the plateau persists. | Owner mismatch refuses repair enrollment. |
| Transfer that worker to the site's new owner and repair at time 18 | Repair returns true and work resumes without another full debit, refund or replacement site. | Reuse the existing paid site and elapsed work. |
| Transfer the worker while its site keeps its owner | Order is already zero inside the owner-event callback and immediately afterward. Work stops after residual projection settles; neither bank changes. Returning the worker and issuing repair resumes work. | Existing worker-transfer interruption is synchronous. |
| Enroll a second Human repair worker | Both retain repair orders. After its approach, measured life growth increases from about 7.499 to 11.999 per second and incremental gold/lumber charges appear. | Assistance is real; the existing single-participant path is still incomplete. These measurements are not implementation constants. |
| Stop, re-enroll, then kill the helper | The primary keeps working. Stop and death each remove the helper's added growth and subsequent charges. Re-enrollment restores both. | Assistance must add/remove its own retained contribution, without replacing the primary. |

The site-transfer run also exposes a distinct limit: stopping the transferred
worker at time 26 initially clears its order, but by time 28 its order is smart
and construction resumes. Both map slots are authored user-controlled and the
melee AI startup call is disabled. The source of this later acquisition is not
qualified. Neither this run nor the recreation promises that STOP permanently
suppresses all automatic work. The quarter-second collection does not establish
tick timing, completion timing, cancellation after transfer, alliance behavior,
damaged repairs or resource exhaustion.

Bounded x86 inspection of the same image supplies complementary native-code
evidence. The real `SetUnitOwner` wrapper at `0x4ab930` calls `0x69d140`, which
cleans the transferred unit's generic orders before writing its owner and
publishing internal event `0xd01a2`. The build-in-progress event handler forwards
that event through `0xb91560`/`0xb91930` to the retained originating build
ability; HumanBuild's corresponding method at `0xb72430` is a no-op. That no-op
alone does not prove the overall lifecycle. Repair's periodic update
`0xbb72d0`/`0xbb6ce0` revalidates the target through `0xbb57a0`, including the
worker/site owner comparison. Unit virtual slot `0xec` independently reads the
owner ordinal at unit offset `0x160`.

Repair startup `0xbb5cc0` distinguishes reclaiming an unattended incomplete site
from assisting a site with a retained builder. Assistance retains four scalar
contributions and adds them to the build-in-progress accumulators via
`0xb92220`. Stop `0xbb6fa0` removes those exact retained contributions via
`0xb924a0`; it does not replace the primary participant. Ability metadata maps
Rep1/Rep2/Rep3/Rep4 to DataA/B/C/D: repair cost ratio, repair time ratio,
powerbuild cost and powerbuild rate. These records must be read through the
selected ability tables and map overlays.

The native time-source join remains unresolved: `0x67c3d0` reads the established
cached build duration, while the distinct getter `0x69b8b0`, used with repair
DataB, reads the adjacent cache field. The `reptm` source loader at `0x6b423f`
is distinct from `bldtm` at `0x6b4227`; the full join to the repair cache getter
is not yet proven. The repair timer's runtime-initialized interval and full
four-scalar arithmetic/charging order also remain open. Substituting `bldtm` or
fitting rates to these traces would conceal those gaps. Static disassembly and
receipts are local; the attempted Ghidra analysis timed out and supplied no
analysis evidence.

The bounded implementation now admits settled, worker-required Human site
transfers and releases an owner-mismatched participant on the next ordinary
construction update. It excludes that transient mismatch from completion wakes.
It preserves site life, elapsed work and paid cost, and permits the same-owner
repair path to resume. Autonomous, linked, initializing, canceled and faulted
sites retain explicit transfer refusals. Ownership-event listener delivery
remains subject to the existing general `SetUnitOwner` capability boundary.
This update does not invent a repair polling interval: its activity boundary is
a recreation choice, not proof of the observed native delay.

The next assistance slice needs a primary participant plus independently retained
helper contributions, using source DataC/DataD and the qualified repair time
getter before enabling charges. Enrollment must validate at request and arrival;
stop, death, removal, target retirement and owner revalidation must remove only
the departing participant's contribution. Nonstock synthetic ratios and distinct
build/repair durations must test the getter distinction, multiple helpers,
fractional charging, depleted banks and completion cleanup. Until those inputs
and arithmetic are qualified, the current additional-worker refusal remains an
explicit fidelity gap.

Validation of the Human transfer slice used
`/tmp/pc3-fidelity-sol61-06-target`: the construction selection passed 58 tests,
with one explicit local-source test ignored. Named regressions exercised paid
site transfer/revalidation/repair/completion, death before revalidation, the
autonomous transfer refusal and existing worker-transfer cleanup. The autonomous
death fixture now issues a public request before arrival so its authored style
is actually selected. Native and WASM all-target Clippy with `-D warnings` and
both library builds passed. The receipt
`/tmp/pc3-sol06-phase2-verification.json` records the exact source hashes, commands'
logs and successful exit statuses; those Rust sources were unchanged throughout
the successful validation. These checks do not establish native polling parity,
rendered behavior or full-workspace integration.

Independent main integration executes every named transfer/death/autonomous
regression in the complete affected script/trigger/gameplay suites: 3,444
passed, zero failed, 159 explicit ignores. Strict native/WASM all-target and
all-feature Clippy, both library builds, formatting, both application session
targets (26 cases) and the application test-binary rebuild pass. The corrected
autonomous setup is included. Source hashes and per-command receipts are under
`/tmp/pc3-strings-construction-main-validation/` and
`/tmp/pc3-strings-construction-app-validation/`. This qualifies the sampled
Human transfer slice; it does not implement assistant builders or prove exact
native polling, automatic reacquisition or cancellation after transfer.
