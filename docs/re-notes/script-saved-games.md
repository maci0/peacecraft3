# Script saved games: native trace and refusal boundary

Evidence: read-only disassembly of the locally supplied Warcraft III.exe,
1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
These are static observations, not original-client runtime observations.

The registration table identifies LoadGame at `0x49e879` → `0x4a2690`,
SaveGame at `0x49e88d` → `0x4a5c50`, and SaveGameExists at
`0x49e8e0` → `0x4a5d90`.

## Observed wrapper behavior

All three test `0x5ad700` and `0x5ad790` and return early when either
returns nonzero. The meaning of these predicates has not been decoded.

SaveGame resolves the script string through `0x469010`, rejects null and
empty text, and rejects parent traversal and colon substring matches through
`0x433c80` (string addresses `0xe6be98`, `0xe6ec10`).
It requires the global game object and checks its field `+0x6d4` is zero.
It sets that field to one before calling path routine `0x4925a0` with a
260-byte output buffer and trailing arguments `(0, 0, 1)`. On success it
calls `0x47d410(path, 0)`, then calls `0x4775c0` on the game object, and
calls `0x4fbe90` only when both results are nonzero. The wrapper itself
has no boolean result contract.

SaveGameExists resolves its argument through `0x4afe80`, invokes the same
path routine with trailing arguments `(0, 0, 1)`, then calls `0x798600`
on the output buffer. Unlike LoadGame and SaveGame, it does not branch
on the path routine's return value. `0x798600` rejects null/empty paths,
calls `0x798c40`, rejects result -1 and attribute bit `0x10`, and returns
one otherwise. This is consistent with a regular-file existence check;
no save header or payload validation occurs in the inspected wrapper.
The filesystem interpretation of the attribute helper remains inferred.

LoadGame rejects a null script-string handle, resolves its text through
`0x469010`, and calls `0x4925a0` with trailing arguments `(0, 1, 1)`.
Only success calls `0x47ca90(path, supplied_boolean, 1)` on the game
object. Compare ChangeLevel's third argument zero in
[the route trace](chapter-startup-refusals.md). A saved-game filename
must therefore not be treated as a fresh-map launch.

## Save payload and path trace

`0x4925a0` rejects null/empty names and the same two substrings. The
branch at `0x49261f` selects path construction according to bit 8 of
the game-info flags. The other branch gets the current map identity
through `0x4775a0` or game-info `+0x24`, applies `0x7afed0` and
`0x4afbe0`, and builds a map-specific custom-save path at `0x49269a`.
The helper `0x79b530` resolves the filesystem path. Argument six selects
that resolved path versus the intermediate path; argument four requests
a trailing backslash; argument five requests existence validation,
using directory helper `0x798240` or regular-file helper `0x798600`.
The map-identity transformation and filesystem root remain unresolved.

The writer uses the same stream object for game virtual method slot
`+0x10` at `0x47d7b3`, then `0x705450` at `0x47d7bd`, and
`0x8ccf60` at `0x47d7c9`. The last routine obtains subsystem 5 via
`0x45df90`, writes its counts and iterates its indexed objects, delegating
each present object to `0x8ceb80`. The names of these subsystems are
not established by this trace. This is evidence of an object graph in
the save, rather than evidence for any particular reconstructed schema.

The writer additionally serializes game `+0x6f4` through `0x4d19a0`
at `0x47d845`, calls `0xac1c70` and `0x5d9af0`, then obtains another
object through `0x5ec5e0(1, 0)`. It writes fields from that object,
calls `0x5f2bb0`, and serializes the objects at its `+0x3fc` and
`+0x400` through `0x615cd0` and `0x64bac0`. Their field meanings and
matching restore routines remain unresolved. No evidence establishes
that the bonus campaign omits any of these sections.

## Remaining work and refusal boundary

The save writer `0x47d410` constructs a data stream beginning at
`0x47d47b`; the wrapper is not a filename-only checkpoint. Its complete
payload, the save path normalization routine, predicate meanings, and
restore dispatch have not been decoded. In particular this trace does
not establish which VM threads, globals, handles, timers, trigger state,
or world state a bonus-campaign save restores.

No implementation change accompanies this note. SaveGame, SaveGameExists
and LoadGame remain unsupported. The missing pieces are save path
resolution and world/VM snapshot restore, including the undecoded sections
above. Saving and revisiting OrcX01/OrcX02 zones remain unsupported.
Returning false unconditionally or relaunching the map would hide missing
state and is not supported by this evidence. The existing ChangeLevel
route probe bypasses SaveGame and cannot validate save or revisit fidelity.

## Campaign unit ability projection (2026-11-13)

The separate recreation-owned game-cache path captured added or removed ordinary
abilities, but even a same-source `RestoreUnit` rejected that saved entry. The
restore validator required the current source's ordinary abilities and refused
extra runtime abilities. A successful store/save could therefore replace a usable
hero checkpoint with one the continuation could not load.

Read-only disassembly of the selected image above narrows the native contract:

- Store path `0x519b50` calls `0x6d8b20` at `0x519c7f`. The latter calls
  `0x6d7260` to aggregate ability contributions, then `0xb40510` on the hero
  component when present. Aggregation is not a copy of the ability list.
- `0xb40860` prepares five eight-byte records. The loop at `0xb408a0` reads
  aliases from hero component `+0xf8`, queries each through `0x6d6570`, and
  stores alias and internal rank plus one at `0xb40947`/`0xb4094e`. Its end
  at `0xb4096c` bounds the records; there is no arbitrary ability-list loop in
  this writer.
- Restore `0x518ca0` creates a fresh unit through `0x687ca0`, then calls
  `0xb40160` on its hero component. At `0xb40351` each nonzero saved alias
  is searched against the receiving hero's five `+0xf8` slots; only a match
  reaches the repeated rank-application call `0xb3f320` at `0xb40388`.

These static observations support keeping hero learn-slot ranks and rebuilding
ordinary abilities from the receiving source. They do not establish persistence
of arbitrary runtime additions/removals. No new original-client run was made.

Capture now projects onto the source ordinary abilities plus live hero learn-slot
ranks. Added ordinary aliases are omitted, removed source aliases are reinstated,
and ordinary ranks come from source creation. Cooldown overrides for omitted
aliases are omitted too. The recreation's existing live sequence is retained
for surviving aliases; missing source entries return at their creation indices.
Native ordering and cooldown-override persistence remain unqualified. The existing
restore validator runs before profile mutation, retaining strict errors for
unsupported persisted progress. The payload version is unchanged.

`cached_ability_edits_rebuild_source_abilities_for_continuation` uses synthetic
source data through public add/remove, store, save and restore natives. It checks
successful stores, identical persisted bytes for transient edits, unchanged live
state and arena, and reconstructed hero and ordinary-unit abilities with a learned
nonstock hero slot in a fresh host. This is a recreation continuation regression
backed by static native evidence, not a
matched native runtime comparison. Explicit permanent-ability edits, original
W3V import and complete world/VM saves remain unsupported. This does not
implement `SaveGame` or `LoadGame`.
The existing restore validator still refuses receiving-map authored ability-list
mismatches; cross-map changes to those definitions are outside this correction.
Previously written snapshots containing unsupported transient edits still refuse;
this correction changes capture, without migrating those old records.

Validation: 13 targeted cache tests and 218 affected synthetic inventory/powerup
tests pass in an independent build directory under a private offscreen display
and null audio sink. The original-install attribute gate was explicitly skipped.
The affected WebAssembly library build and touched-file formatting checks pass;
workspace compiler warnings remain errors. These checks do not establish native
runtime or browser execution parity.
