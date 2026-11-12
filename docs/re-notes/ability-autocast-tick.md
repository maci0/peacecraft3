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
Their values have not been bound to recreation rules.

`0xb4a4a0` also checks current-order state and ability flag 1, then returns
whether virtual `+0x3e4` returns zero. The usual `+0x3e4`, `0xb4a530`, checks
ability `+0x3c`, owner flag `0x40000000`, virtual `+0x378`, ability flag
`0x200` with the sign-bit exception, ability `+0x44`, virtual `+0x268`, and
the conditional order checks `0xb4b2c0` / `0xb4b230`. Identifying this entire
function as just a mana/cooldown test is insufficient. The precise host mapping
of these fields and virtuals remains open.

## Scan and ranking

`0xb9c8e0` gets the owner position, obtains `+0x44c`, adds through `0x9a01c0`
and multiplies through `0x99ffe0`, then calls `0xb9d020` with its scan context.
Correction to the earlier ON note: `0x9a01c0` is software floating-point
addition, not a target ordering routine. The scan in `0xb9d020` enumerates
through `0x68ea00`, callback `0xb9ce00`, and virtual `+0x454`. Enumeration
order is not yet established, including equal-distance ordering.

The ordinary `+0x454` implementation `0xb9c980` applies `0x67fb80`, rejects
nonzero `+0x3f4`, performs alliance/visibility checks with `0x696a90` and
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
with `0x18`. Nonzero ranked DataC (`0x46eed0`) additionally requires
`0x67ea50`, resolves the target attack component's `+0x70` object, and requires
its owner relation test `0x69bd70` against the caster. These extra conditions
are separate from the ordinary spell's authored `targs`.

`0x67eca0` delegates unit `+0x3f8` to `0xb66f10`. That function checks an
ability counter at `+0x80`, optional flag `0x200`, disabled counts at `+0x3c`,
and exceptions involving `BUsl` and another looked-up ability. A missing
component returns false. Its complete movement-eligibility mapping is open.
`0x67ea50` delegates unit `+0x3f4` to `0xb1e150`: it requires a nonpositive
`+0x3c` counter and a nonzero `flags & 0x180000`. Mapping those attack flags
to canonical host states remains open. Visibility `0x69df20` is also unbound.

## Recreation status

The ordinary targeted Slow effect exists in `unit_curse_host.rs`. The requested
Heal, Inner Fire, Curse, Web, Raise Dead and Black Arrow class codes have no
ordinary targeted cast-effect host in this checkout. The orb's wrapped Black
Arrow is a different class (`ANbs` / `AAsm`) and cannot establish `ANba` casting.

Automatic casting remains refused at the due tick, before acquisition. The
remaining eligibility, visibility, metric and enumeration mappings above must
be established before connecting Slow's existing effect host. This note adds no
implementation, synthetic tests or runtime parity evidence. Default-on activation
remains the earlier open gap.
