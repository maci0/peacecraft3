# Automatic-cast tick: partial native decode

Reviewed executable: Warcraft III.exe 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Addresses below are virtual addresses with image base `0x400000`.
These are static native-code observations, not runtime parity results.

## Tick and readiness

`0xb9d120` tests ability flag `0x100000` before obtaining the retry delay
through virtual `+0x470`. That flag exits without rearming. Otherwise ability
flag `0x20`, an unset active bit `0x80000`, failed readiness `+0x3e0`, owner
flag `0x1000000` at `+0x164`, owner flag `0x4000000` at `+0x20`, or positive
owner `+0x3a4` skips acquisition and rearms using the retry delay.

A non-null acquisition result sets ability flags `0x220000`. Cast flags
`+0x354` select the immediate path `0xb4b0f0` for bit 1, otherwise the targeted
path `0xb2b080` for bit 4. The tick then clears `0x20000` and rearms with the
greater of `+0x3e8` and `+0x474`. The retry getter `0xb9cbb0` reads global
`0x112d7c8`; the minimum-after-cast getter `0xb9cbd0` reads `0x112d810`.
The getters and their integer-to-real initializers now bind `TickTiming` in
`autocast_rules.rs`; the called retry route remains unfinished.

`0xb4a4a0` also checks current-order state and ability flag 1, then returns
whether virtual `+0x3e4` returns zero. The usual `+0x3e4`, `0xb4a530`, checks
ability `+0x3c`, owner flag `0x40000000`, virtual `+0x378`, ability flag
`0x200` with the sign-bit exception, ability `+0x44`, virtual `+0x268`, and
the conditional order checks `0xb4b2c0` / `0xb4b230`. Identifying this entire
function as just a mana/cooldown test is insufficient. The precise host mapping
of these fields and virtuals remains open.

### Suspend and reactivate are separate from ordinary retry

Fresh selected-image Ghidra evidence confirms that `0xb9c750` sets ability
flag `0x100000` and cancels the embedded timer at `+0x128` through `0x7fbbd0`.
The cancellation clears the timer's retained request pointer. Its counterpart
`0xb9c710` clears that flag, asks virtual `+0x43c` whether the ability is active,
and only then schedules the source first-delay operand. The getter at
`0xb81150` reads active flag `0x80000`. This is not a periodic retry through
an inactive interval.

The lifecycle is not qualified as cooldown-only. `0xb9c3d0` calls the suspend
virtual before detaching its owner relationships. `0xb9c440` can set the same
flag during attachment, depending on owner state. The event sink `0xb9c5f0`
also dispatches suspend/reactivate virtuals for several internal event kinds;
their complete canonical meanings remain open. A generic constructor's zero
write at `0x46d0d0` does not establish the later attached instance's flag.
An empty recreation cooldown map therefore cannot prove ordinary eligibility.

Evidence IDs: suspend `ev_27a135bd769a8690da7db4243bcd6176567ca6ca86448c905841accee1361af0`,
reactivate `ev_64002184cae7ef0128b1696ebc7b05e54f4b31bb5e001eaed29c48871e8df4e7`,
detach `ev_9bbe6d7e08a1990b5ef6a931a3c470fea41e35fbcb0c10cb4e0b2804b26b4e80`,
attach `ev_20e7a5e1b51ad7ca72c2bc2921d3a9f4c60d8eeadc471be925dba5ef1248160d`,
event sink `ev_e36ff7aae68f41c68270ce3d99431b11befafa288a54be870f42927352e8178a`.
Complete local evidence and a reusable analysis snapshot are retained at
`/tmp/pc3-sol09-autocast-lifecycle-evidence-v1.json` and
`/tmp/pc3-sol09-autocast-lifecycle-snapshot-v1.json`. This is native-code evidence,
not a new original-client runtime comparison or a completed autocast loop.

## Scan and ranking

`0xb9c8e0` gets the owner position, obtains `+0x44c`, adds through `0x9a01c0`
and multiplies through `0x99ffe0`, then calls `0xb9d020` with its scan context.
Correction to the earlier ON note: `0x9a01c0` is software floating-point
addition, not a target ordering routine. The scan in `0xb9d020` enumerates
through `0x68ea00`, callback `0xb9ce00`, and virtual `+0x454`. Enumeration
order is not yet established, including equal-distance ordering.

The ordinary `+0x454` implementation `0xb9c980` applies `0x67fb80`, rejects
nonzero `+0x3f4`, performs alliance checks with `0x696a90` and
`0x69df20`, then requires `+0x458`. It obtains a candidate metric through
`0x7ff1e0` and compares against the incumbent through `+0x460`.
The metric's coordinate transformation and enumeration range remain unbound;
center distance from another spell is not evidence for this path.

The shared comparator `0xb9cb20` first calls `+0x46c`. If true, it selects
the candidate strictly when the incumbent metric is greater; equal metrics
retain the incumbent. Otherwise it compares each metric with the reference
argument: metrics at or below the reference beat those above; among two at or
below it the greater metric wins; among two above it the smaller wins. Equal
metrics choose the candidate in the former case and the incumbent in the latter.
This alternate branch must not be replaced by a nearest-target rule.

`+0x46c` at `0xb811c0` returns the logical negation of `+0x464`.
Slow's `+0x464`, `0xbf7c10`, returns zero, so Slow uses the strict smaller-metric
branch. No lower-handle tie policy has been established for this scan.

## Slow filter and remaining host gaps

Slow vtable `0xf6b1f8`: readiness `0xb4a4a0`, candidate filter `0xbf7c20`,
comparator `0xb9cb20`, self fallback `0xbf7c10`, ranking switch `0xb811c0`.

`0xbf7c20` first requires the common filter `0xb9ca70`. Unless ability flag 2
is set, it requires `0x67eca0(target, 0, 0)`. It rejects an existing `Bslo`
ability through `0x6d6570`. Owner flag 4 adds a target player-index comparison
with `0x18`. Ranked DataC equal to the selected zero (`0x46eed0`) additionally requires
`0x67ea50`, resolves the target attack component's `+0x70` object, and requires
its owner relation test `0x69bd70` against the caster. These extra conditions
are separate from the ordinary spell's authored `targs`.

The former nonzero-DataC description was reversed. The selected instruction
branch and decompilation (`ev_e9dcd1079529fac94010a7c184f236580704a43cbe956974964894b80055ca9b`)
accept a nonzero DataC after the preceding gates. The relation test reads the
caster's `ALLIANCE_HELP_RESPONSE` mask for the resolved attack victim's owner;
it is distinct from passive alliance and hostility.
See [the bounded Slow observations](slow-autocast-qualification.md).

`0x67eca0` delegates unit `+0x3f8` to `0xb66f10`. That function checks an
ability counter at `+0x80`, optional flag `0x200`, disabled counts at `+0x3c`,
and exceptions involving `BUsl` and another looked-up ability. A missing
component returns false. Its complete movement-eligibility mapping is open.
`0x67ea50` delegates unit `+0x3f4` to `0xb1e150`: it requires a nonpositive
`+0x3c` counter and a nonzero `flags & 0x180000`. Mapping those attack flags
to canonical host states remains open. The `0x69df20` relationship is shared
spells, not fog visibility; its full predicate mapping remains open.

## Recreation status

The ordinary targeted Slow effect exists in `unit_curse_host.rs`. The requested
Heal, Inner Fire, Curse, Web, Raise Dead and Black Arrow class codes have no
ordinary targeted cast-effect host in this checkout. The orb's wrapped Black
Arrow is a different class (`ANbs` / `AAsm`) and cannot establish `ANba` casting.

Automatic casting remains refused at the due tick, before acquisition. The
remaining eligibility, visibility, metric and enumeration mappings above must
be established before connecting Slow's existing effect host. This note adds no
implementation or runtime parity evidence for acquisition. Default-on first-evaluation scheduling
is now connected through unit creation (see `ability-autocast-on.md`); it exposes this same refusal
at the decoded deadline and does not close the automatic-casting gap.

## Restoration acquisition follow-up (2026-11-13)

A recreation horizon run of unchanged RoC `Undead02Interlude.w3m` reaches an `Arst`
automatic-acquisition refusal; this is a recreation failure, not an original-client observation.
The local receipt is `/tmp/pc3-horizon-sweep/roc-Undead02Interlude.w3m/test.log`.
The local Legacy install's `War3x.mpq` member `Units/UndeadAbilityFunc.txt` identifies its
presentation as a repair action with an acolyte work loop. This is shipped-profile evidence,
not a qualified healing-spell implementation or proof of the active map's dataset selection.

Read-only PE mapping and `objdump` inspection of the same hash-qualified image above identify
`CAbilityRestoration`: name getter `0xbb69e0`, class vtable `0xf4b670`. Its acquisition `+0x450`
is the shared `0xb9c8e0`; outer candidate filter `+0x454` is `0xb9c980`; inner filter `+0x458`
is `0xbb5ad0`; comparator `+0x460` is `0xb9cb20`. The ranking switch `+0x46c` (`0xbb4e00`)
returns one, selecting the shared strict-smaller-metric branch. That does not establish that
the metric is center distance or establish equal-metric enumeration order.

The inner filter first calls `0xb9ca70`, then has a configuration-dependent target `+0x164`
flag test, a comparison against global `0x112d88c` after `0x697030`, position/object tests
through `0x7fcc50`, `0x7ff5d0`, `0x696c10` and `0x7f6a00`, and final target-flag tests.
Those values and predicates are not yet mapped to canonical host state. `+0x45c` (`0xbb5c00`)
returns one. Cast-flags getter `+0x354` (`0xbb4e20`) has bit 4 set and bit 1 clear, so the
shared tick selects its targeted dispatch, rather than the immediate branch. Readiness is
still the shared `0xb4a4a0` / `0xb4a530`; the scan-range getter `+0x44c` (`0xb81170`) calls
`0x46f840`. No ordinary Restoration cast-effect host exists here. Acquisition, source-field
bindings, effect execution and retry scheduling therefore remain unsupported; substituting
Slow's filter or a generic nearest-target heal would not be supported by this inspection.

## Heal and Neutral Heal acquisition follow-up (2026-11-13)

Current unchanged recreation horizons expose `Ahea` at its first due evaluation in
`/tmp/pc3-ai-horizon-main/Undead04.log`, and `ACsw` in
`/tmp/pc3-undead03-before-range/Undead03.log` (the latter precedes range-enumeration intake).
These are concrete recreation frontiers, not original-client observations. Undead05's earlier
`Anh2` creation refusal and its class correction are recorded in `ability-autocast-on.md`.

The reviewed image's Heal and Neutral Heal classes share candidate filter `0xbed1a0` and
comparator `0xbed270`. The filter calls the common `0xb9ca70`, applies a configuration-dependent
target-flag check, explicitly calls shared-spells relationship `0x69df20`, tests a player-property
query `0x697030`, and excludes a target carrying `Bhea` from a different source caster. The
ordinary target check `0xbed0c0` compares maximum minus current life against global `0x112d810`
and applies its own configuration and existing-heal-source checks.

The comparator is **not** the shared nearest-metric comparator: it first divides candidates
according to their metrics relative to a reference, and when both satisfy that reference it
calls `0x635990` on each. That routine divides virtual `+0x12c` by `+0x130`, the current/max-life
ratio qualified in `destructable-death-delivery.md` and `hero-vital-level-updates.md`. Thus a generic
nearest-unit rule would lose a native health-ratio preference. Metric/range transformation,
equal-ranking enumeration, the full common target filter, visibility and readiness remain to
be mapped before faithful automatic Heal execution can be connected.

The shared Heal effect routine `0xbed2f0` reads ranked DataA through `0x46ed70`, conditionally
scales it through a configuration lookup, applies a life update via `0x690770`, updates timing,
and creates or refreshes `Bhea` with source-caster identity. A one-shot call to the canonical heal
helper would omit this retained effect/timing state. Actual acquisition, cost/effect execution,
channels and cancellation therefore remain open; no guessed target selection or silent no-op
was added by the Neutral Heal class correction.

## Alliance field-role correction (2026-11-13)

Selected native-code evidence distinguishes two relationships previously left
ambiguous here. `0x69bd70 -> 0x47d060` reads player mask `+0x58`. The public
alliance getter/setter agree on a `+0x38` base and a 16-byte stride; resolving
the resulting index against the selected `common.j` identifies
`ALLIANCE_HELP_RESPONSE`. Public `IsUnitAlly` reads `+0x38` instead.

`0x69df20 -> 0x47fb00 -> 0x47faa0` reads `+0x78`, which the same derivation
identifies as `ALLIANCE_SHARED_SPELLS`. It must not be implemented using a
fog-visibility query. The earlier `0x67fb80` gate remains separate. These
field roles do not establish the full common target predicate, default masks
or automatic Heal acquisition.

The local verification receipt is
`/tmp/pc3-sol09-autocast-relations-receipt.json`: all 31 bounded image windows
match their selected instruction relationships, two nonstock field layouts
resolve correctly and a misaligned layout refuses. Inputs are image SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
version 1.29.2.9231, and local `common.j` SHA-256
`3c21e998bf80d1e04718bd3f2ee7ef2bf1b83359e5b303a56cf59cd61a2afdcd`.
This is native-code and shipped-script evidence; no new casting behavior is
implemented by this correction.
