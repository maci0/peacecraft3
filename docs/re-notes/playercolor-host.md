# Source-derived symbolic enum identities

`pcx_play::playercolor_host::EnumHost` is an opt-in concrete, session-owned Host.
The caller explicitly selects native names and supplies `Limits` for handle
capacity, admission steps, expression depth and aggregate expression work to
`checked_selected`. Admission steps count each supplied native name (including
duplicates) and each global visited for each selected contract, before filtering.
They do not bound parsing, string bytes or all CPU operations.
Signature shape alone never enables a native. Each selected declaration must be
a constant scalar integer-to-named-handle-subtype native, without a script
override or multiple declaration sources. Constant conversion globals in that
native's declaring source supply its admitted enumerands. The Program producer
must enforce unique, non-reserved source labels before resolution, as
`MapScripts::compile` does. Under that prerequisite, map-source globals cannot
enlarge this domain. Labels alone do not authenticate original bytes, and the
Host cannot recover origins after a producer collapses labels. No counts,
enumerands, palettes or engine handle IDs
are embedded in the implementation.

Supported conversion arguments are closed integer literal expressions with
parentheses and unary/binary operators evaluated by the existing VM value
operations. Results must be integers; invalid arithmetic, identifiers and calls
are refused. Depth and work limits are project resource policies. No other
function/native executes to derive a domain, and no range or flag combinations
are inferred. Unsupported initializers, incompatible declarations, missing
domains, bad arguments and out-of-domain inputs report contextual errors. These
admitted domains are local import policy; native behavior outside them remains
unestablished.

One bounded arena interns nominal result-family/enumerand pairs. Equal pairs
share identity across selected native aliases; each native still checks its own
domain before looking up that identity. Different families and interleaved object
families have distinct identities. Reservation refuses selected enum families
and their descendants. Capacity failure preserves existing identities, which
remain usable at capacity. This identity allocation is a recreation choice;
the reservation API provides no object behavior. Handle-producing operations in
this Host must use the same arena. Tokens belong to one Host/VM session.

`ColorHost` remains a type alias with the existing color-only `checked` API and
accessors. Its compatibility entry point selects only `ConvertPlayerColor`, uses
a fixed project-policy recursion ceiling and derives an aggregate work bound
from source spans, plus a checked one-name/global-scan admission budget. It still
requires the exact `playercolor` return family. Generic callers must supply explicit `Limits` and native
selection. Symbolic identity supplies no RGB interpretation, native pointer
semantics or gameplay behavior for any admitted family.

`MapScripts::shared_program` shares the exact existing Arc with the Host instead
of compiling or cloning a second Program. The selected-map scalar neutral
adapters can wrap this Host without allocating competing handles.
`MapScripts::owned_vm` still owns localization and diagnostics. Automatic Host
installation and the remaining gameplay contracts are separate work.

## Verification

The existing nine ColorHost tests are retained. Twelve additional synthetic tests
exercise the same public module: family separation, alias identity with distinct
sparse domains, shared arena reservations, closed arithmetic and overflow,
independent expression/admission budgets, exact color-family compatibility,
declaration/source rejection, capacity, actual
compiled global initialization, next-native refusal and sticky failure.

The earlier color-only integration passed 163 native tests with five explicit
original-input ignores, strict native/WASM Clippy and WASM library code
generation. Those results precede the shared enum implementation. The new
twenty-one-test module passes integrated native tests. The affected crate passes
179 native tests with five explicit original-input ignores, strict native/WASM
all-target/all-feature Clippy, and warning-denied WASM library code generation.
These are parser/Host checks, not rendered or native gameplay proof.

The explicit local Human01 gate retains the same admitted map, compiled Program,
localization and independently approved scalar neutral adapters described in
[the neutral adapter note](selected-map-neutral-host.md). Its unchanged common
SHA-256 is `3c21e998bf80d1e04718bd3f2ee7ef2bf1b83359e5b303a56cf59cd61a2afdcd`.
The earlier color-only gate advances 24 source-declared color constants and
refuses `ConvertRace`. A later private shared-enum prototype uses a separately
checked, explicit source-census native policy and verifies the derived domain
digests. It advances 435 enum conversions across 43 selected natives to 432 typed
identities, handles both neutral getters, then refuses `GetBJMaxPlayers` at its
Blizzard global initializer. Repeating initialization returns the same failure
without new calls or identities. These counts are observations, never production
constants. Local commands, diagnostics and hashes are retained in
`.scratch/script-enum-map-initializer/`; they precede production integration.

The integrated Host has now been exercised on the same retained Human01
admission, Program and localization owner, with all three independently approved
scalar adapters. Its source-census policy still admits the same 43 contracts and
432 typed identities. Global initialization makes 439 observed native calls and
then refuses `GetBJPlayerNeutralVictim` at Blizzard line 44. A repeat returns the
same diagnostic without more calls or identities. Actual commands, artifact/input
hashes and logs are in `.scratch/enum-host-root-integration/`; no original input
or profile contents are committed. Counts are diagnostic observations only.

The later same-context gate also admits the independently approved victim
getter, advancing to `GetBJPlayerNeutralExtra` at Blizzard line 45 with the same
symbolic identities and sticky-failure guarantees. The symbolic Host is unchanged;
that additive scalar slice and its receipts are described in
[the selected-map note](selected-map-neutral-host.md#selected-victim-getter).

The gate uses an identified compatibility object catalog with unresolved records.
It does not establish complete target-catalog admission, campaign execution,
rendered interpretation or native exceptional behavior. Original inputs and
executable-rule profiles remain local and unpublished.


The subsequent same-context gate adds independently approved Extra and slot
rules and passes the scalar player-limit declarations. It now refuses
`CreateTimer` at Blizzard line 454 with 442 observed native calls and the same
432 symbolic identities. Retry remains sticky without new calls or allocation.
This is initialization evidence only; timers, script entrypoints and launcher
Host wiring remain open. See the
[selected-map note](selected-map-neutral-host.md#selected-extra-and-player-slot-getters)
for qualified sources and actual gate receipts.
