# Automatic powerup (attribute tome) re-notes

Scope: powerup items whose source ability grants permanent strength on the hero
(`UnitAddItem` automatic activation). Agility and intelligence effects are refused.

## Evidence

Selected client: `Warcraft III.exe` 1.29.2.9231, SHA-256 `3f2ed012…`, equal to
`menu_lighting::REVIEWED_IMAGE`. Shipped bytes and bounded native code only; no
original runtime observation.

- Registration window `0xbca0e0` (24 bytes): three pushed immediates, `call 0x7f6360`,
  `add esp,0xc`, `ret`. The last push is `"AIsm"` (`0x4149736d`), the middle `"AIxm"`.
- `0x7f6360` reads stack arguments at `[ebp+8]`, `[ebp+0xc]`, `[ebp+0x10]` and ends in a
  plain `ret`; the caller removes arguments. Calling convention: cdecl. `[ebp+8]` (source
  key) and `[ebp+0xc]` (base key) are each hashed through `0x95c860` into registry nodes.
  The base key sets flag `+0x84` and `+0x80`; the third argument (`0x11b67a0`) is stored at
  node `+4`. The class-to-RTTI relationship beyond that node is not verified.
- Source row: `ItemData tstr` has `abilList=AIsm`, `powerup=1`, `usable=1`,
  `perishable=1`, `uses=1`. `AbilityData AIsm` has `levels=1`, `DataC1` is the strength
  amount. `Istr` metadata has `data=3` and `useSpecific` containing `AIsm` and `AIxm`.
- Research receipt (`.scratch/tome-research-20261020/native-receipt.json`) for the route
  `UnitAddItem 0x4ae9f0 → 0x67a2a0 → 0xb47a80 → 0x6c37c0`, the attribute callback
  `0xbc5520 → 0xb3dd30 → 0xb3fd00 → 0x6be8c0`, and pickup/removal `0x68a840`,
  `0x6c4f70`, `0x6c60b0`. This session re-verified only `0x7f6360` and `0xbca0e0`.

## Behavior

1. `UnitAddItem` on a `powerup` item applies the effect before the slot check, so a full
   inventory does not refuse it. Routed pickup (`IssueTargetOrder` smart/get) reaches the
   same path once in range.
2. Strength is retained per unit (`ExecutedUnit::permanent_strength`) and added to derived
   STR for combat and vitals. Level-up deltas cancel it, so it is not counted twice.
3. Life follows the ratio policy: `max' = max + STR × LifePerStrength`,
   `life' = max' × life / max` (native mode 1, `0x635990`). Level-up keeps its absolute
   missing-amount policy (mode 2 analogue).
4. Order of effects: grant, then the pickup event is queued, then the item is claimed.
   It is marked consumed when no queued pickup for it remains. That runs in
   `finish_fired_event` and in the disabled-trigger branch of `begin_fired_event`.
   Queries during delivery see the live item. A claimed item cannot be granted again or
   picked up.
5. Refusals are atomic: mixed or empty ability lists, multi-level or multi-charge rows,
   agility or intelligence amounts, negative strength, custom item rows, and non-hero
   targets return an error before any write. Unit cleanup projects every held item's
   strength removal in held/drop order before cancelling a pending order, dropping an
   item or queueing a drop event. An item whose removal would make the maximum
   nonpositive refuses the whole cleanup without partial drops, even when an earlier
   item removal alone was valid. An authored removal delta whose negation is not
   representable also refuses before any write instead of overflowing.

## Unqualified

These are fidelity gaps in the recreation, not permission gates added for convenience.

- `get_items` (inventory policy) is checked before the powerup path. Whether native
  checks it there is unverified, so a unit whose policy forbids pickup refuses a tome
  that native might still activate.
- Non-hero target: native `b3dd30` returns through a fail path when hero data is null.
  The recreation refuses with an error; the native fail behavior is not verified.
- Native event nesting: the pickup callback runs from the queue, after the grant and
  before consumption, matching the order in `68a840`. Synchronous nesting is not proved.
- `uses` above 1 and multi-level abilities are refused, not modeled.
- Mana is untouched by the strength path; no evidence was gathered for mana in this effect.

## Tests

- `inventory_powerup_tests.rs`: window decoding, source amounts, W3A override level,
  refusals, custom rows, image gate.
- `inventory_powerup_host.rs`: pure life-percentage and retained-strength math.
- `combat_tests.rs` child `inventory_powerup_host_tests.rs`: queued pickup reads the live
  item, claimed grant refused, consumption after finish, full slots, level-up retention,
  percentage policy, mixed and non-hero refusal, multiple queued pickups, disabled trigger.
  Cleanup tests refuse an oversized strength item after an earlier plain item and two
  strength items whose second removal would empty the pool, preserving units, items,
  pending orders and drop events.
- Explicit local gate (`#[ignore]`): `PCX_TOME_CLIENT` and `PCX_TOME_CATALOG`, run with
  `--ignored`.

## Cross-module contracts

- `ObjectHost::live_item` (`item_host.rs`, `pub(super)`) is the canonical item lookup:
  consumed and removed handles read as absent, a non-item handle is a family error.
  Inventory and query paths call it; `item_query_host` must not mirror the policy.
- `SetItemDropID` refuses a consumed powerup (`item_drop_host.rs`). Covered by
  `consumed_powerup_refuses_set_item_drop_id_while_a_live_item_accepts_it`.
- Startup order: `admit_unit_vitals` → `with_inventory` → `with_inventory_powerup`
  (`map_startup_host.rs`). Campaign inputs build the rules from the selected client image
  and the same tables as hero skills (`campaign_inputs.rs`).

## Shared reader

`inventory_powerup::Rules` also reads the passive `AIab` registration (`0xbc9ce0`) and
exposes `held_attributes` for held passive items. See `passive-held-items.md`.
