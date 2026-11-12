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
aliases are omitted too. Ordinary entries follow fresh source creation order;
learned ranks follow the parsed hero slots, as in the inspected native loop.
Absolute native command-card ordering and cooldown-override persistence remain
unqualified. The existing
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
valid old snapshots with a different ordinary traversal order now reconstruct
source creation order after the same alias/rank/cooldown validation.

Initial projection validation: 13 targeted cache tests and 218 affected synthetic inventory/powerup
tests pass in an independent build directory under a private offscreen display
and null audio sink. The original-install attribute gate was explicitly skipped.
The affected WebAssembly library build and touched-file formatting checks pass;
workspace compiler warnings remain errors. These checks do not establish native
runtime or browser execution parity.

### Remove and re-add order correction

The initial projection retained the live order of surviving ordinary aliases.
Removing an ordinary alias and adding it again appends it to the live list, so
the snapshot could preserve transient order even when its aliases and ranks
matched source. That contradicted reconstructing the ordinary state through the
same source factory used by fresh creation (`0x518ce3` calls `0x687ca0`). The
source creation loop at `0x66ffda` instantiates the parsed `abilList`; the cache
writer above does not encode its live traversal order.

The affected observable consumer is `ObjectHost::learned_ability_buttons`, which
iterates these entries, then `campaign_command_card::display_only`, which selects
the first display-only button for each authored `Buttonpos`. Two ordinary aliases
sharing a slot can therefore change the displayed icon and tooltip candidate.
This is a traced recreation behavior, not a new native UI observation.

`cached_remove_readd_restores_fresh_source_button_order` gives two synthetic
ordinary aliases different Art values and the same Buttonpos. Removing and
re-adding the first changes the live first candidate. Store/save and continuation
must reconstruct both the source ability sequence and the fresh ordered button
input, without changing the edited live unit. Both hero and ordinary-unit cases
use the public natives. No command-card implementation change is needed.

The hero case learns two nonstock skills in reverse source-slot order. Its fresh
control learns the same ranks in source order. The test then restores both the
newly captured profile and an older valid PC3UNIT vector carrying the edited
ordinary and learned traversal sequence. Both must match the fresh control.
Capture and restore share the same source/slot projection; restore retains all
existing unsupported/corrupt-data checks before constructing the candidate state.
This is private-record normalization, not original W3V or world-save migration.

Follow-up validation: all 14 targeted cache tests, including the named order
regression and existing invalid-record/refusal tests, and all 219 affected
synthetic inventory/powerup tests pass. The original-install attribute gate stays
explicitly skipped. The isolated WebAssembly library build and touched-file
formatting/diff checks pass with compiler warnings denied. No native runtime or
browser presentation comparison was added.

## Private original-client probes (2026-11-13)

Runtime source is the same executable identity above, under private
Docker/Wine/Xvfb, not native Windows. The runtime image is
`sha256:ab778c3e5be5de42cbe35f7d649cab2c0688630838fbcb1a27e476a0f825ec6e`.
The container has no network, host display/audio sockets or audio devices, drops
all capabilities and uses a read-only root. The only writable bind is its private
run directory. Before launching the client, the copied harness verifies its Xvfb
geometry and a single Pulse null sink. Modified diagnostic map scripts use the
working map-patching recipe, update the script's MPQ attributes CRC, and verify
extraction round trips. They are bounded experiments, not unchanged-map evidence.
Original maps, generated maps, profiles and saves remain local and uncommitted.

The path/Exists probe at
`/tmp/pc3-sol10-save-probes-1794517623829270155/native-status.json` completed.
Its final synthetic trace was checked for each before/after witness:

- A controller-created zero-byte ordinary file returns true from SaveGameExists.
  This corroborates the inspected wrapper's lack of save-header validation.
- Null, empty, parent traversal, nested parent traversal, colon and drive-colon
  names return false from Exists before and after SaveGame. Both SaveGame and
  LoadGame return to the caller for these names, retaining its local value. The
  private custom-save directory contains only the ordinary-file control and the
  valid save control after the run. This does not qualify every path spelling,
  filename root or map-identity transformation.
- A valid control changes Exists from false to true before the next script
  statement. The resulting extensionless file is under the private profile's
  `CustomSaves/PCXSaveProbe` directory; it is 1,357,666 bytes with SHA-256
  `95b601be17e42463214baafc3ca3079128c667434cb87182f4ab7f5a42fc3e23`.
  Only filename, size and digest were recorded for the payload.

A separate timer-callback probe retains a helper's local value across SaveGame,
then completes its enclosing arithmetic assignment. LoadGame returns to the old
caller far enough for a following statement and local-value witness to execute.
The same immediate witnesses occur when the map is launched through the original
Single Player/Custom Game menu with a newly created private profile. No registered
GAME_SAVE callback appears in those captured intervals. These observations do not
establish which VM instruction point was stored or the eventual fate of the old
thread after successful publication.

The extensionless scripted-load attempts encounter an access violation at
`0x8744d7`. REA/Ghidra evidence
`ev_3c05fe74039c00c2e8246e7f800b43602dc6a5556d57f9f4f9ecbfc54d60626a`
resolves the caller at `0x521800` to `0x5214b0`: its failure path builds
`NETERROR_BADMAPDATA` and calls the dialog helper leading to that dereference.
The actual Load Saved Game menu, opened in a new private process and copied
profile, lists the diagnostic directory but omits the extensionless file. These
are filename/admission controls, not successful restore evidence.

### Successful save/load execution boundary

Matching `.w3z` names load successfully in both the direct timer and nested
TriggerExecute arms. Local evidence and checked synthetic witnesses are in
`/tmp/pc3-sol10-save-probes-1794518196713646522/witness-verification.json`.
The two saves are 1,362,856 and 1,362,784 bytes, respectively, with SHA-256
`8cc3dfc6d6922906b620e9eeb04636be3d462cb7cde6646e0fa9c25d36706916` and
`3ecb67fafb5be20fd5d435eccd1b98936a6f86e5d304245b78eb6769be56ea46`.
No payload contents were inspected or retained in the repository.

The helper sets a phase marker to 11, then calls SaveGame while an outer
arithmetic assignment is pending. Live execution returns from SaveGame, sets
the marker to 12, completes the assignment and later calls LoadGame. Its old
caller then executes a following statement setting the marker to 33.

After publication, GAME_LOADED instead observes marker 11 and the uncompleted
assignment's original value. The next two periodic callbacks advance their
counter from 1 to 2 and 3 while retaining those values. The helper's after-save
tail and its active timer caller do not resume during that interval. The nested
case likewise retains its before-trigger marker but does not replay its
after-trigger tail. Saving at tick end or restoring these active frames and
finishing their pending assignment would contradict these observations.

This does not establish omission of all VM execution state. A separate paired
probe parks a trigger action with a local value before saving. Its uninterrupted
control wakes once. After loading, the parked action also wakes once with the
same local value, while its start counter remains one. Twelve later timer
callbacks retain the saved marker and pending assignment values above. Evidence
is in `/tmp/pc3-sol10-save-probes-1794518777723079019/native-status.json`.
Thus these parked map threads survive, even though the active save-calling
execution does not resume. A fresh original process, with prior trace files
excluded from its copied private profile, bootstraps the same map and loads that
existing save. Its new final trace exactly matches the same-process loaded trace,
including the parked local and single wake. The checked receipt is
`sleep-witness-verification.json` in the same directory. The unsuccessful direct
`-loadfile` saved-file experiments are not used as cold-load evidence.

A further paired probe preserves an already evaluated left operand, the sleeping
callee's local, its caller's local and the remaining TriggerAddAction tail. Its
evaluation/start/wake/tail counters each remain one after load. Both uninterrupted
and loaded sleeping actions see their original Begin timer from GetExpiredTimer,
not a later periodic timer. GAME_LOADED instead sees a null expired timer. Split
records avoid the original Preload trace's long-line truncation. Complete local
records are at `/tmp/pc3-sol10-save-probes-1794519945945109409`.

The same split records separate expired identity from timer queries. During
GAME_LOADED the periodic timer reports elapsed equal to its timeout and remaining
zero despite GetExpiredTimer returning null. The earlier one-shot Begin timer
retains its inactive elapsed/remaining values when its sleeping action resumes.
These are finite-real query witnesses, using a bounded sign/exponent/mantissa
normalizer with separate arithmetic controls, not assumptions about the native
clock quantum or a copied query implementation.

### Pending expiry replay after load

Saving inside the first registered expiry subscription distinguishes native
delivery from the active script call tree. At that point the first subscription
has started, but its tail, the second subscription and the direct handler have
not run. GAME_LOADED sees exactly those saved counters.

At the same saved long-timer clock point, a fresh firing then starts again with
the first subscription, followed by the second subscription and direct handler.
An early observer scheduled by GAME_LOADED sees that complete fresh firing;
the following normal periodic firing occurs later. Local evidence is
`/tmp/pc3-sol10-save-probes-1794520242410567573`. This is neither resuming the
old subscription halfway through nor waiting a full new period before firing.

Consequently an internal scheduler that advances a periodic deadline before
calling script cannot save only that advanced Run. Its saved pending-expiry
projection must reproduce the native fresh firing. Idle/menu checkpoint
round trips can compare uninterrupted continuation; a script save during an
active expiry must instead compare this qualified load behavior. Reentrant
restart/resume cases, other event families, AI threads and presentation still
need separate qualification. No result here permits dropping retained spell
payments or unrelated pending native jobs.

### Session delivery cursor foundation

The existing production timer dispatcher now retains its expiry snapshot,
subscription index, condition index/combined result and action/handler phase in
the published session. It still evaluates every condition, skips subscriptions
retired before their turn, clears trigger context before the direct handler and
finishes an expiry even on callback failure. This is recreation-owned dispatch
state, not a claim that original save payloads contain the same cursor.

The two new `timer_delivery_cursor_*` regressions run through the production
factory and published VM. Both pass, as do all eight affected trigger-delivery
tests. Strict native and WebAssembly pcx-play Clippy checks, the WebAssembly
library build, the application test's WebAssembly compile check and strict native
application-test Clippy check pass in the isolated lane target, with unchanged
source hashes. Browser execution remains unchecked.

Full SaveGame/LoadGame remains unsupported in the recreation. No save codec,
VM checkpoint barrier or production save/menu route accompanies this foundation.

### Retained expired-timer response

Parked script-thread contexts now retain the expired-timer identity observed at
their sleep. Resuming installs that response; returning or failing restores the
caller's response. An explicitly retained null differs from having no override,
so it cannot fall through to an unrelated current expiry. The getter retains its
existing handle-family/lifetime validation. No timer query epoch or old clock
point is installed by this context change.

The three new regressions exercise nested trigger calls, a second sleep, the
remaining action tail, explicit null under another prepared expiry, and failure
unwinding through the real owned VM and host. They and all 13 filtered trigger/
runtime tests pass. All eight affected published-session trigger tests also pass
after merging the released baseline. Strict native/WebAssembly pcx-play
all-target Clippy and the WebAssembly library build pass with unchanged source
hashes. This fixes a script-visible continuation field; it does not implement
durable world saves or establish every expired-timer lifetime behavior.
