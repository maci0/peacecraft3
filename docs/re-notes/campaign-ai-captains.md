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
queries. Readiness, combat, home queries and reset remain unsupported until
their shared-world relationships are modeled.

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
