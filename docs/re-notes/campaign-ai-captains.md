# Campaign captain targets

The selected Legacy 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Its owned `common.ai` declares `CreateCaptains()` and
`SetCaptainHome(integer, real, real)`, both returning nothing. The unchanged
Human02 opening-minute gate now passes `CreateCaptains` and refuses
`SetNewHeroes` at 7.218750 seconds after passing gathering setup.

## Selected-code relationships

The primary creation handler reaches `0xabd160`. It selects an indexed record
through the same association machinery used by gathering, supplies two scalar
pairs to two created objects, and stores two encoded references. Those references
are not X/Y words. The matched runtime class key does not establish that the
objects are map units, script groups or locations.

For a newly indexed unit, `0xac7000` samples its canonical position and
`0xac7e90` supplies both record pairs from that result. Creation reads the
record pairs at call time. The public home setter reaches `0xac26d0`; each
source selector bit chooses one reference and forwards its supplied pair to
`0xad0630`, which uses the same property setters as initialization. The selector
tests are byte `TEST` instructions; their immediate operands are not sign
extended.

`CaptainIsHome` resolves the first reference and compares that object's current
position with its stored pair. This does not identify the selected anchor unit
as the moving captain. `ResetCaptainLocs` first refreshes association state and
can read updated values for the second pair, so immutable creation homes would
not model its full behavior. The numeric property vtable and conversion, group
membership, callback delivery and the second object's full lifecycle remain
unqualified.

## Bounded recreation

The privately integrated host uses one canonical campaign-player registry with two optional
home targets. Creation resolves a valid current association or selects from the
existing source-qualified canonical-unit path, including when gathering has not
yet been stopped. A genuinely empty world leaves targets absent; unsupported
candidate provenance refuses rather than inventing coordinates. Sampling the
current canonical position instead of reproducing the native cached association
record is an explicit initial-unit recreation choice. Later record refreshes
and movement before repeated creation need further qualification.
Repeated creation currently refuses rather than inventing reset or object
replacement behavior.

Home assignment uses the image-derived selector bits. Selected absent targets
refuse before any write; preflighting both targets is a recreation error boundary,
not a claim of native rollback. No selected bits means no home mutation.
Creation and assignment use the runtime's canonical player context; mutable
`common.ai` globals do not establish ownership.

Creation does not clone units, create script groups or substitute the anchor's
current position for a moving captain. The later grouped timed-life consumer
uses these declared targets for its supported members and enables count/empty
queries. The later [combat query](campaign-ai-captain-combat.md) reads separately
published canonical target state. Readiness, home queries and reset remain
unsupported until their shared-world relationships are modeled.

## Validation

Seven policy tests pass with the original-image gate enabled. They include
unsigned byte masks, selector branch targets, creation/reference-slot agreement,
exact budgets and malformed overflowing or overlapping reference pairs. Host
admission also refuses malformed selector masks before storing policy.

The published map-session fixture creates captains before `StopGathering` and
checks both programs' separate targets before and after sleep. Combined checks
pass 1,604 core tests, 610 app tests, strict native and WASM Clippy, formatting,
selected Human01 startup and Human02 Continue startup. The opening-minute gate
advances past creation and stops at `SetNewHeroes`. Receipts remain local under
`.scratch/campaign-flee-integration/captain-*`. These are source and recreation
execution checks; no new original-client or rendered captain test was run.

Mapped windows, exact byte hashes, source callers and the known/unknown trail
remain local under `.scratch/campaign-basics-native/create-captains*.md`.
No original assets are tracked.

## Public membership observation

Further selected-image validation binds the primary `CaptainGroupSize` and
`CaptainIsEmpty` registrations to their NUL-terminated names and signatures.
Their result paths resolve the first captain reference, then respectively read
or compare with zero the same counter updated by the unit-association path.
The reader checks those operand relationships against the decoded first
reference slot; it does not substitute a collection of all same-owner units.

Synthetic tests change the reference and counter operands coherently, reject
broken joins and string terminators, and reject every truncated window. All
nine focused policy tests pass, including the explicit selected-image test.
The later [grouped timed-life integration](campaign-ai-group-timed-life.md)
enables these queries over canonical first-captain membership. It includes
matched original-client count observations and a bounded lifecycle/movement
consumer; general roster operations and exact native formation remain open.

## Recruitment type-pass correction

Both `AddDefenders` and `AddAssault` use the shared captain recruitment path.
It now preserves separate requested-type and directed-alternate passes. The
previous combined predicate could choose an earlier-created alternate ahead of
an equally healthy exact candidate. It could also decrement a zero request for
an alternate member when the native exact-type completion branch would already
have returned true.

Selected-code evidence is the same Legacy 1.29.2.9231 image identified above.
A fresh bounded GNU objdump disassembly of `0xabb130..0xabb258` confirms these
joins: exact existing-member count at `0xabb1aa`, completion at `0xabb1b4`,
alternate count at `0xabb1c7`, then completion at `0xabb1d1`. Calls at
`0xabb1de`, `0xabb1f9`, `0xabb212` and `0xabb22c` pass respectively healthy
exact, healthy alternate, fallback exact and fallback alternate. Each intervening
zero branch reaches the shared successful return. The bounded member counter
`0xabc8c0..0xabca5d` still stops at zero after its decrement at `0xabca08`;
there is no general nonpositive-request shortcut.

The policy reader now checks those type, mode and completion-branch relationships
in the selected instruction window. Alias rawcodes, threshold and decrement
remain decoded operands. Synthetic malformed-pass mutations refuse admission.
The REA Ghidra provider was unavailable because its configured local installation
was incomplete; the direct disassembly used the supplied executable without
executing it or acquiring assets.

Canonical synthetic regressions use a nonstock directed alias, exact candidates
created after alternates, all four health combinations, repeated requests,
death/recruitment, attack count/readiness/fullness, and unsupported exact-candidate
atomic refusal. They share the existing guard/combat fixture and production
recruitment function; distinct synthetic types are created through the public
`CreateUnit` path with that fixture's admitted movement/combat metadata. They do not establish native callback cadence
or original-client runtime parity. The earlier original-client defender observations
remain the runtime evidence for ordinary count/partial/negative behavior; no new
original-client pass-order observation was run.


## Retained inactive toggle admission

The unchanged Undead04 five-minute continuation refused `ReturnGuardPosts` for
AI player 9, canonical unit `HandleId(2731)` (`hhes`), at control time 1.5.
The recorded current order was zero and the retained ability state was
`Some((Adef, false))`; cached, summoned, paused and effect-history checks were
negative. The local receipt is
`/tmp/pc3-horizon-sweep/roc-Undead04.w3m/test.log`.

`unit_host` initializes admitted toggle-capable definitions with an inactive
state at creation. That field's presence therefore does not establish active
ability behavior. Shared movement/speed and flee admission already test its
active bit. Captain recruitment and explicit guard return now use that same
distinction: inactive retained state permits the ordinary canonical path;
active behavior remains an explicit unsupported state. This is a recreation
admission correction supported by the source/default relationship and existing
shared activity semantics, not a new native guard-controller parity claim.

The regression admits a nonstock Defend alias and technology from synthetic
tables, creates its unit through the normal catalog, switches it on/off through
`IssueImmediateOrder`, and checks both guard return and recruitment. Active
refusal preserves the record and membership; after switching off, routing and
membership succeed with the retained inactive state intact. No ability state,
unit, order ID or authored value is invented to advance the mission. The actual
Undead04 continuation still needs rerunning after integration.


## Correction validation

The isolated worktree build passes all five named recruitment/toggle regressions
and ten captain-policy tests; its explicitly selected original-image reader also
passes. The existing guard suite passes 131 tests, including the new regressions,
and the AI caller suite passes 52 tests with one unrelated asset-gated test
ignored. Strict native and WASM Clippy with all targets and warnings denied pass,
as do both `pcx-play` library builds. Tests ran with a verified private offscreen
display and null audio sink. Formatting and diff checks pass.

Receipts remain local under `/tmp/pc3-lane07-validation/5/`, with exact commands,
worktree path and exit codes in `results.json`. This run uses an isolated target
at `/tmp/pc3-fidelity-sol61-07-target`; the earlier shared-target result omitted
new tests and is not accepted as branch validation. Test-only debug symbols were
disabled for `pcx-play`; assertions remained enabled. Earlier fixture setup
failures are retained in the preceding local runs. No new original-client or
browser runtime observation was made, and no complete campaign or tick-parity
claim follows from these checks.
