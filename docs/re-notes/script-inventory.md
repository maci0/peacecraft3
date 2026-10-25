# Canonical inventory foundation

The selected Legacy executable is identified by SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The unchanged Human01 script creates the ledger on Menag's death, registers a
player-unit pickup event, uses `GetManipulatedItem` to identify it, makes it
nondroppable, and later tests `UnitHasItem` before removing the same item identity.
Returning a constant false would suppress the quest rather than implement it.

## Source observations

`UnitAbilities` supplies each unit's inventory ability. The selected ability
metadata's `inv1` through `inv5` records identify the `AbilityData` data-column
indices and applicable aliases. Their common ability code identifies the inventory
class without copying that code. Shipped `UI/WorldEditStrings.txt` labels these
fields Item Capacity, Drop Items On Death, Can Use Items, Can Get Items and Can
Drop Items. Per-level values come from the selected ability rows, with typed W3A
map overrides retained. A `Data` override applies only when its second selector
equals that field's metadata `data` ordinal; a mismatched pointer refuses instead
of falling back or fabricating a value. The source metadata supplies the capacity
bound.
Missing, ambiguous, malformed or unsupported input refuses admission or use.
A unit with a complete known ability list lacking this component has no slots;
an unadmitted ability list remains unknown.

The selected `MiscGame` profile supplies pickup, drop and give ranges. Ordered
map profiles can override these scalars. The admitted order registry supplies
`smart` and `getitem` identifiers. Source table identities, contributing asset
fingerprints, ordered profile names and the registry's image binding remain with
the rules. No measured slot count, radius or order number is embedded.

Bounded native instruction inspection found the registered `UnitHasItem`
implementation at `0x4af1e0` and inventory-size implementation at `0x4af390` using
the same inventory component guard, `0x67ec60`. Size then reaches `0x6960f0` and
`0xb46390`. `UnitAddItem` at `0x4ae9f0` validates both identities and can detach
an existing carrier before attaching the item. `UnitRemoveItem` at `0x4af650`
uses the shared detach path. The inventory component's pickup path reads
`Misc/PickupItemRange` at `0xb468df`; order names and identifiers are registered
from source strings. These are shipped-byte observations, not original runtime
measurements. Private bounded disassembly receipts stay local under
`.scratch/player-resources-20261020/`.

## Implemented behavior and explicit boundaries

Items retain one shared-arena identity while on the ground or held in a numbered
slot. Removal leaves a typed tombstone. Queries, additions, transfers, forced
script removals and slot removals read this state. Full inventory, unavailable
pickup permission and removed objects refuse acquisition without inventing a
held item. Nondrop and item drop-on-death overrides remain attached to that item.
The public capability projections combine authored inventory and item policies.
Script `UnitRemoveItem` remains a forced detach, distinct from permission to drop
an item through player controls.

Pickup target orders use the existing routed unit motion and source pickup range.
The pending action is replaced by later accepted movement or combat orders;
current-order queries expose its admitted identifier. Arrival transfers the
actual ground item and queues matching player-unit pickup events with item,
unit, player, filter and event identity. Forced detach queues the corresponding
drop event. Source policy, identity and queue-allocation checks precede ownership
changes. Unit death drops items according to inventory and item death policies;
full unit removal puts held items on the ground. Units with nothing held need no
inventory policy lookup merely to die or be removed.

Storage layout, deferred event delivery at script boundaries, full-removal drops
and drop notifications when deleting a held item are recreation choices; exact
native nesting and deletion-edge behavior remain unverified. Stored ground x/y
are retained while an item is held. Initial item invulnerability is unknown until
an explicit setter establishes it. Automatic-use powerup effects refuse before
movement rather than accepting an acquisition that cannot execute. Item use,
unimplemented passive families (life, mana and attack speed), consumable effects,
drag/drop/give orders and inventory HUD interaction remain separate consumers. Ground item rendering/picking is
verified separately from these host tests.

Synthetic tests cover changed capacity and permission overrides, full and empty
slots, transfers, drop/removal identities, wrong families, malformed arguments,
replacement movement, actual routed pickup and delivered response context,
death retention/item death overrides and ordinary-unit lifecycle. The explicit
original source gate requires `PCX_INVENTORY_CATALOG` pointing to the retained
selected source copy and `PCX_PLAYER_RESOURCE_IMAGE` identifying the selected
executable. It derives Hart's policy from original tables and the same registry;
full unchanged Human01 pickup/return execution remains a separate runtime gate.

## UnitAddItemToSlotById

Selected image SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The native is `(Hunit;II)B`. Aligned windows, half-open, from that image:

| Window | VA | SHA-256 |
| --- | --- | --- |
| Registration | `0x49d93f..0x49d953` | `704faf32061316ccf859b0c2e66a7768904bef7a564d1d1993a559f1e47037c2` |
| Handler | `0x4aeba0..0x4aec44` | `d1e6361ac973803cf3efd7c6b7143c226aa4e5b90ca621d6a8c1ff21cebff51f` |
| Slot dispatch | `0x67a310..0x67a3b9` | `64f11e54d7722d0d818988c7f64741c1927ecb02ade9fa0c22125cd4f2d35b61` |
| Ordinary add | `0x67a2a0..0x67a2ff` | `068fb66c1bf3f0f98bcb70cfa8e015b39e923d8090360cf7301c8d0bcc9e5cb7` |
| Slot writer | `0xb45660..0xb458c2` | `36e92acf8623b43983fd7826bd5189ecfacc277c2359fdd288c3bc33d1179b9c` |
| Powerup writer | `0xb47a80..0xb47ad5` | `700a71845a84a8247789e439203cde6bce9bcb2847dc890cb83ad28f063546b4` |

The raw windows and objdump listings stay in
`.scratch/unit-add-item-slot-20261025/`. The original executable was not run.

The handler resolves the unit and returns false for a null pointer. It copies
the unit's current position, then calls the item constructor before the
inventory checks. A null constructor result returns false. A later false return
does not delete the item. Eligibility uses the inventory component and the same
permission call as `UnitAddItem`. The host treats a dead unit, or a source
policy that cannot get items, as that refusal: the new item stays unheld at the
accepted unit position. Other native flag bits in that permission call are not
mapped.

An admitted automatic powerup takes the same grant path as `UnitAddItem` and
ignores the requested slot. The powerup writer passes slot `-1`. A grant the
host cannot prepare returns an error before the record exists. A prepared grant
creates the item unheld, applies the strength pools, queues the pickup, and
returns true.

An ordinary item compares the slot as an unsigned index with the source
inventory capacity. The writer also rejects a slot at or above the minimum of
that capacity and an unclassified converted value. The host uses the source
capacity only. An out-of-range or occupied slot returns false and leaves the
new item unheld at the unit position. There is no search for another slot.
A free in-range slot is attached directly. Passive checks, pickup callback
resolution, and event capacity run before that record is published. A condition
and a filter are both legal pickup boolexprs. Catalog, budget, passive, and
callback failures leave the arena unchanged.

The same Prologue02 opening that used to stop inside this native now passes the
startup and timer horizon on `peacecraft3-becac95980727bee` (26.42s, 6 items, 0
visible). The visible count is the gate's report, not a ground-render result.
The following order frontier is the point `setrally` note in
[immediate orders](script-immediate-orders.md).

## Ground item consumer and current runtime gate

The application projects only ground records through the existing campaign model,
material, skinning, opacity, texture and lighting path. `ItemData` model path,
scale and color columns are selected by the resolved metadata mapping; typed map
edits remain effective. Model bytes and provenance are bounded and cached per
session. Held/removal transitions retire ground actors. Missing models, invalid
metadata, unsupported replaceable textures and degenerate picking extents report
an explicit rendering diagnostic.

Pointer pickup uses the actual model's source bounds transformed to world space;
there is no invented item click radius. A matching ground item takes a `smart`
target order through the same Host route as scripts. Stale session witnesses
cannot pick previous actors. Grounding the model origin without a separate facing
override is an explicit recreation transform choice; native item orientation and
animated picking parity are not yet established.

The v17 native fixture creates a ledger beside the current hero as synthetic test
input, derives its click point from the same selected model bounds, and sends a
physical secondary-pointer click. The item is visibly present before the click,
then absent from the ground, with its original identity held by Arthas in slot
zero. The unchanged campaign's ledger creation/pickup/return path is not proved
by that synthetic producer. Separately, unchanged Human01 now passes `UnitHasItem`
and reaches the fourth diagnostic route rectangle before `RemoveGuardPosition`
refuses. Local screenshots, logs and receipts are under
`.scratch/campaign-objective-20261020/item-gui-v17/` and
`route9-final-v17/`; no original content is committed.
