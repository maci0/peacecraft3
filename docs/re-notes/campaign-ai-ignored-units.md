# Campaign AI ignored-unit query

`IgnoredUnits(integer unitid) -> integer` reads the running AI player's current
units outside its managed associations. `common.ai` adds this quantity to an
attack wave's requested amount before comparing against the player's unit count
in `BuildAttackers` and `PrepTime`. Returning a fixed zero would lose that
distinction after a guard is released.

## Selected native evidence

The selected 1.29.2.9231 image is SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The primary registration at `0xab6820` binds the `(I)I` declaration and
`IgnoredUnits` name to `0xab5870`. The handler resolves its owned AI context and
calls `0xabf760`. That query enumerates the context's player and the requested
type, accumulating through callback `0xabca60`. The callback counts a unit when
its `0x6933c0` AI link is absent, or when none of the three association references
at link `+0x30`, `+0x3c` and `+0x48` resolves. It skips a unit with any resolved
association. These are observations of selected code, not runtime measurements.

REA evidence retained locally under `.scratch/ai-ignored-units/`:

| Evidence | Result |
| --- | --- |
| `ev_021ca8c526beaa6f335424f14f13d089f66e1d595b17a9c2f88cdb2007fb7e17` | Context query `0xabf760` |
| `ev_d8964e6dfd6b69a47f95041c1cbcf6c8672a0af789cc9e983136d6188d745ade` | Association predicate `0xabca60` |
| `ev_988392fc620dce16267a9ed32eedcde9b56e5484c66364e294ebf88ad28423c1` | Enumerator `0x68ea00`: player selection and exact-type predicate registration |
| `ev_a749a69a87a3c14d379e98c72f9e9bde73c72355ea879c0578357d97269d9315` | Enumerator's unit-state mask predicate |
| `ev_8ab7cb22b23b31d189236bd4758a5c8848640517b294c3acdf5b5c08c37eb4ed` | Captain-link assignment `0xac69c0` |
| `ev_e3fa8689451457b2f29f4c6cc007dad12b9baad4bd4f4f375516d4f33d52d5b5` | Guard release `0xac1500` |

The default enumeration excludes a native dead-state bit and two further state
bits whose public lifecycle meaning remains unqualified. `IsUnitHidden` reads a
different field (`unit+0x20`), so those mask bits cannot simply be called hidden.
The existing [guard](script-unit-guards.md) and captain-association evidence
supports the host's management distinction without copying encoded references
or native lists.

## Shared-world recreation

The query uses canonical live unit type and owner, unreleased guard membership,
and captain membership. A released guard counts; an admitted captain member
does not. Exact ownership/type matching, death/removal and checked output use
the same count path as the other AI queries. The query does not allocate unit
handles or change orders, memberships, clocks or random state. Invalid/inactive
AI owners and unknown liveness refuse; an unmatched type returns zero.

There is no additional unit store or counter cache. Worlds without an admitted
guard system have no accepted guard memberships; their live non-captain units
count as unmanaged. This is an explicit recreation choice. Autonomous worker
controllers, additional native association classes, and the two unqualified
enumeration state bits remain fidelity work. Explicit map harvest orders are
not relabeled as autonomous AI controller membership.

Tests exercise the real guard-release path, repeat release, owner transfer,
death/removal, captain attachment, inactive-owner refusal and query immutability.
AI declaration/argument tests distinguish this result from total and completed
counts. The published two-program fixture checks live unmanaged counts before
and after sleep. The combined gates retain 1,692 passing core tests, 55
app-library and 638 app-binary tests, the published fixture, selected Human01
startup and victory Continue, strict native/WASM Clippy and the default desktop
build. The unchanged
Human02 opening-minute test still fails, now at `GetUnitBuildTime` at
7.312500 seconds. Logs remain in `.scratch/ai-ignored-units/gates/`; no new
original-client experiment is claimed. Full campaign production and native
association parity remain unfinished.
