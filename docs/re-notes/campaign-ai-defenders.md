# Defensive captain recruitment

`AddDefenders(quantity, unit_type)` returns whether the defensive captain has
the requested number of matching members. Existing matching members contribute
to that desired total. A shortfall retains newly recruited members and returns
false. Recruitment does not add those units to the attack captain's count.

## Source and native evidence

The selected Legacy executable is SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The primary registration at `0xab66c3` binds `AddDefenders\0`, `(II)B\0`
and handler `0xab3a30`. The handler forwards the owned AI context and both
integer arguments. `0xabaa10` selects the second captain reference, the same
pair decoded for captain creation and home changes, then calls `0xabb130`.

Recruitment counts existing members before selecting additional units. It has
directed alternate-type relationships, multiple health-priority/fallback passes,
current-order exclusions, and a signed decrement shared with the existing-member
counter. These operands are decoded by `CaptainPolicy`; none of the unit aliases,
health threshold, excluded order IDs or count step is copied into production
constants. The decoder binds completion branches and canonical association calls
as well as the registration and NUL-terminated strings. Synthetic operands vary
independently; malformed relationships and every truncated window refuse.

Retained REA observations include:

- Registration: `ev_97131512ab6964c43eb4758f5aa79b8b9ec17be5109d3d392948c4e093b55d65`.
- Second reference: `ev_d5b5ea7df8038b585c44ae154e42d46ef52a2c037618f26440127599c5015ffe`.
- Recruitment: `ev_c40630e4b587f5f0565c07447d4d55100e61c4298881b08655480d014cd1a5bc`.
- Unit filter: `ev_c2b8df2c6d306b1177e09d25b18471c849e1056c4f0d8e3149d52d3faaaa4093`.
- Health threshold: `ev_b98c2b4c434a9dc0a8c2b0ea618ff03ddde408c663fd8135378918dc2da3a8cc`.
- Association: `ev_a7bcf7cf384cc8e947018aae53bc30b5a4ca3b064b1c00fedc2589cf48087cbd`.

The analysis session is closed. The evidence is selected-code evidence, not a
claim that the native callback machinery has been reproduced.

## Matched original-client observations

Four Human02-derived diagnostics used the same ordinary footman, authored homes,
seed and timing. Only the quantity and diagnostic identities differed. Each AI
called `AddDefenders` twice before its acknowledgment. Continuous queries packed
both results, both combat selectors and the first-captain count into a bounded
bank value with a progress sequence. The map later ordered the footman to attack
a paused enemy, verified actual damage, and killed that enemy.

All four stopped Wine/Xvfb reference runs reported RoC and passed the collector's
source identity, marker inventory, ordering, progress, damage and terminal checks.
Map/AI parsing, archive-member roundtrips and normalized arm comparison passed.

| Requested | First result | Repeated result | Defensive query during fight | Attack count |
| --- | --- | --- | --- | --- |
| 0 | true | true | false | 0 |
| 1 | true | true | true | 0 |
| 2, only one available | false | false | true | 0 |
| -1 | false | false | true | 0 |

The defensive query returned false after enemy death in every recruited arm.
The negative request is not treated as an empty request: the native counter has
no general nonpositive early return. Zero without members completes immediately;
counting an existing member can make a zero request negative. The latter edge is
supported by the selected counter, not separately measured by these four arms.

Stopped result SHA-256 identities:

- Zero: `32a994f62ab9532546520ee8c1c76ff13c9bfca409a92bbada95b222385ba70b`.
- One: `f5deb6b71d21d886167bedd647199d02cec40070385d29daeddc95e08a8256d7`.
- Partial: `bdb5611ec34d21becac0e1e8d87fd4c908d3d7adfd67dd0b48b28b92bad0180d`.
- Negative: `8176f2b97b020dd7b24da66e2b97df511e4e219f9813bb768e41058764890c56`.

Maps, scripts, raw markers, collectors, receipts and matched results remain local
and ignored under `.scratch/add-defenders/`. None is shipped as game content.

## Recreation boundary

The host uses the canonical unit arena, existing two-role captain registry,
prepared steering, guard homes and combat steps. It preflights recruitment before
publishing membership or orders. Insufficient candidates are a partial result;
unsupported candidate state is a contextual error before publication.

Equal-priority ordering follows canonical creation order. Ordinary recruits use
the existing defensive home and guard-return rules. Native controller scheduling,
formation, alternate priority flags and stealing from another controller remain
fidelity questions. Cached, summoned, effect-bearing and unsupported activity
states do not silently become successful recruits. These engine choices are not
evidence of identical native internals or complete campaign strategy.

## Integration checks

The selected-image reader, 1,737 core tests, 55 app-library tests, 638 app-binary
tests, the published two-program session, selected Human01 startup and Continue
into Human02 pass, as do strict native and WASM Clippy, formatting and the default
desktop build. The unchanged victory
Continue/opening-minute gate now passes `AddDefenders` and refuses
`FillGuardPosts` at 8.312500 seconds. That gate remains a failing campaign
acceptance test; the authored map and AI were not edited to bypass it.
