# Executed start-location priorities

`ObjectHost::with_start_priorities(max_entries)` explicitly admits the four
start-priority interfaces after map-config admission. Exact supplied declarations,
scalar argument/return types, constant flags, source ambiguity and script-function
overrides are checked through the existing contract validator. `startlocprio`
values must be admitted symbolic identities from the same arena. No numeric rank,
priority weight, slot count or map relationship is embedded in the implementation.

`SetStartLocPrioCount` stores a declared list length for an already defined start.
`SetStartLocPrio` assigns a target start and priority identity to a declared list
slot. Both source and target must refer to defined start locations. The two getters
return the stored target index or symbolic priority. An unset count, unset slot,
invalid index, wrong family or malformed call refuses with source context; no
implicit target or priority is fabricated. Relationships reference the start
indices, so changing a point's coordinates does not copy stale geometry into them.

The explicit limit bounds total declared list slots across all starts, including
unset entries. Zero-count lists remain distinguishable from absent lists. Their
number is bounded by the existing defined-start limit. Platform layout and
aggregate counts are checked before allocation. Failed growth or assignment
preserves existing lists. Resizing replaces the backing vector so prior large
allocations are not retained across shrink/grow cycles; allocation overhead and
the old/new replacement overlap remain separate from the logical-entry budget.

Resize currently preserves assigned prefix entries, truncates removed entries
and leaves new entries unset. That is an explicit recreation choice pending
native resize/default traces. Priority weights, native implicit values and random
or team placement are not inferred. The `shortcut` comment identifies this
boundary; successful config execution is not native placement proof.

## Verification

Nine synthetic tests cover explicit admission, missing/zero counts, sparse
relationships and coordinate updates, unset/bad indices and families, aggregate
budget atomicity, shrink/grow behavior, platform layout, exact declarations,
function overrides, actual VM setters/getters, and malformed calls. All 236 native
tests pass with five explicit original-input ignores, strict native/WASM
all-target/all-feature Clippy and WASM library generation.
Five actual 32-bit WASM checks pass (mask 31, zero imports), covering platform
layout refusal, compiled config/query execution, aggregate and huge-count refusal,
typed/target isolation, and preserved/dropped/unset resize entries. Commands,
artifact hashes and actual captured output are local in
`.scratch/start-priorities-root/`.

With the same qualified Human01 map, common/BJ, catalog and independently
approved scalar inputs described in
[selected-map-neutral-host.md](selected-map-neutral-host.md), global initialization
still passes. The authored `config` now completes after 538 observed native calls.
Its two priority lists declare five slots, of which three are explicitly assigned;
the two remaining entries stay visibly unset. All counts and assigned relationships
match actual intercepted calls, not a copied expected list.

The same VM then enters the authored map `main` and refuses `GetCameraMargin`
at map-script line 7781. The trace contains 539 calls including that refusal,
with five players, four timers, three groups and 432 symbolic identities
(444 total arena entries). Program, admission and localization ownership remain
unchanged; repeated globals do not issue calls or replace objects. Original input
hashes remain unchanged. No original payload, profile, name, coordinate or
priority list is published.

This proves headless execution of this qualified map's globals and config only.
Native priority defaults/order, actual placement, completed main, world rendering,
campaign completion and ordinary launcher Host/profile installation remain open.
No native game process was run.
