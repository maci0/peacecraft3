# Item charges and cooldown groups

Scope: `GetItemCharges`, `SetItemCharges`, and the state a usable item consumes. Item activation
(`UnitUseItem` and its effects) is not here. The activation owner calls the two-step boundary below.

## Evidence

Shipped bytes and bounded static native code of the selected executable (SHA-256 `3f2ed012…`,
resource 1.29.2.9231). No original runtime observation.

- JASS declarations: `common.j:1450-1451`. `common.j:2533-2537` declares unit ability cooldown natives
  only, and no JASS native reads an item cooldown.
- Name strings `GetItemCharges` (VA `0xe73128`) and `SetItemCharges` (VA `0xe73138`), with signature
  strings `(Hitem;)I` and `(Hitem;I)V`. Each has a registration row (signature, name, handler) passed
  to the registrar `0x8ccc80`.
- Binding 1 (block at `0x49e4b0`): `GetItemCharges` → handler `0x492d50`. The handler resolves the
  handle through `0x482050`, returns 0 when the handle is null or unresolved, and otherwise returns the
  integer at object `+0x18c`. `SetItemCharges` → handler `0x4a8de0`. The handler returns without a
  write for a null handle. For a negative argument it stores 0. It stores the count at `+0x18c`, sets
  bit `0x1000` at `+0x20` when the stored count is 0 and clears it otherwise, and calls `0x5f6e50`.
- Binding 2 (block at `0xcb9798`): the same two names register with handler `0xcbda30`, a function
  that returns 0 with no other effect. The registrar `0x8c8f30` looks up an existing name first. Which
  binding is effective depends on registration order, and that order is not established here. This is
  an open point. The implementation below follows binding 1.
- Field ids (`UnitMetaData.slk`, `slk` = `ItemData`): `iuse` = `uses` (`X11`), `iusa` = `usable`,
  `iper` = `perishable`, `iicd` = `ignoreCD`, `icid` = `cooldownID` (`X9`, `abilCode`), `iabi` =
  `abilList`.
- `ItemData` `phea` (line 3993 of the selected `itemdata.slk`): `abilList` `AIh1`, `cooldownID`
  `AIhe`, `uses` 1, `perishable` 1.
- `AbilityData` alias `AIh1` (line 56691): code `AIhe`, `Cool1` 20. Three other rows share the code
  `AIhe` with `Cool1` 40, 60 and 0. The duration key is therefore the item's own ability alias, not
  the cooldown code.
- `uses` over 273 rows: 161 are `-` (uncharged), 112 are integers. One row, `wneg`, has `ignoreCD` 1.
- Census gate `local_original_item_use_census_derives_every_row` (`--ignored`, `PCX_ITEM_USE_CATALOG`)
  reads the selected `units` directory.

## Behavior

1. Accessors match the binary. A null or removed item reads 0 and a write is a no-op. A negative
   write stores 0. A count starts from the authored `uses` cell on first access, and a write replaces
   it. A non-item handle refuses.
2. Readiness (`check_item_use`, no writes). Refusals: `NoCharges` for a charged item at 0,
   `OnCooldown { remaining }` while the group runs, and an error for an `ignoreCD` item, a non-finite
   time, an absent item, or underived data.
3. Commit (`commit_item_use(grant)`). Re-runs readiness at the grant's time and requires the item to
   equal the grant: same holder, charge model and cooldown. Otherwise it refuses with a `stale grant`
   error and writes nothing. On success it consumes one charge, sets the group's ready time to
   `now + Cool1` for this holder, and returns `UseCommit { charges, remove }`. `remove` is set when the item is
   perishable and charged with 0 left.
4. Grants are move-only. A grant commits at most once, and a second grant for the same group or last
   charge refuses after the first commit. Dropping a grant changes nothing.
5. Duration: `Cool1` at level 1, with the map's W3A `acdn` override. An item with a group needs
   exactly one ability in `abilList`.
6. A malformed `uses`, `perishable` or ability value refuses loudly. A missing `cooldownID` cell means
   no group.

## Unverified (recreation choices, not established)

- Group sharing: items held by one unit with the same `cooldownID` wait on one
  ready time. Different holders have independent records. This implements the
  unit-local reset boundary described below; propagation between same-group item
  abilities and cooldown behavior when an item changes holders remain unverified
  against the original. No transfer-parity claim is made.
- Charge consumption at commit, after the effect. The native order is not measured.
- `remove` on the last charge of a perishable item. The binary sets bit `0x1000` at zero charges. Its
  consumer is not identified, and the owner performs the removal here.
- `ignoreCD`: refused, not modeled. Both readings of the flag are plausible.
- Binding precedence (see Evidence). If binding 2 is effective, `GetItemCharges` always returns 0.

## Cross-module contracts

### Unit reset and holder scope (2026-10-22)

Static reads of the selected `Warcraft III.exe` 1.29.2.9231 (SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`):
`UnitResetCooldown`'s primary handler `0x4af760` resolves its unit and calls
`0x6d8a10`. The body walks that unit's ability list, uses the spell type tag from
`0xb4b180` and the type relation helper `0x7f55f0`, then calls `0xb4a8e0` on
each spell instance. That helper resets the active instance timer at `+0xd4`,
without changing the configured duration cell. A final call to `0x5f5d70`
notifies the interface. Selected-image MSVC RTTI identifies `CAbilityItemHeal`
as a descendant of `CAbilitySimpleSpell` and `CAbilitySpell`; its remaining/end
slots share the base spell methods. `CAbilityInventory` does not inherit Spell.
These establish the unit/spell-instance traversal, not a runtime observation of
the active registrar, item transfer, or group synchronization.

The spell-start path at `0xb4a940`, when its item flag is set, gets the owner
unit's inventory ability through the `AInv` type getter `0xb463c0` and lookup
`0x6d6570`. It then calls `0xb47070` on that inventory instance. That method
stores a timer and item-type group word in the inventory's own arrays, compares
held items' group words (`0x6c4d10`), and starts matching item abilities through
`0x6c6ba0` and their `+0x3ac` timer-start method (`0xb4a1c0` for ItemHeal).
This is additional static evidence for inventory-local group synchronization;
it does not establish transfer/pickup timing or prove that the recreation's
single deadline reproduces every native timer interaction.

The old single global group deadline was an unqualified recreation choice that
coupled all holders. Records are now keyed by unit and derived group, with the
same aggregate caller budget over unit/group pairs. Readiness performs borrowed
map lookups without cloning the group key. Reset removes only that unit's groups;
removal also releases them. Charge records and grant validation are unchanged.
This is the current recreation model, not proof that native item timers use the
same keys or preserve/drop a deadline on transfer in the same way.

The ordinary unit admission now validates the optional `UnitResetCooldown`
declaration against `CreateUnit`'s source, signature and family; hero-skill
admission is not required. Reset clears all currently modeled spell and item
deadlines for the named unit. It does not refund charges or mana, undo healing,
change configured durations, advance time/RNG, or cancel a pending cast. A
pending spell can start its cooldown later at its normal cast point. Null and
removed units are no-ops; malformed arguments refuse before mutation.

Private disassembly, RTTI scripts and hash receipts stay under
`.scratch/unit-cooldown-reset/`. No original assets are committed. Native runtime
group/transfer timing and full campaign acceptance remain open.

The private target's final reset increment passes 1,108 core tests (31 ignored),
589 app tests (51 ignored), the installed Human01 declaration/rank gate, strict
core/app all-target Clippy and affected WASM checks. Tests cover both spell
deadlines, configured-duration preservation, pending summons, argument refusal,
ordinary units without hero rules, independent holders, duplicate grants within
one holder, real potion reuse and bounded-record cleanup on reset/removal.
The earlier failed core receipt retained one test's superseded expectation that
different holders invalidate each other's grants; its final form keeps the
same-holder rejection and separately checks independent-holder commits.

- `ObjectHost::with_item_use(rules, max_records)` (`item_use_host.rs`) requires item admission.
  `object_host.rs` dispatches the natives. `map_startup_host.rs` admits it inside the item budget, from
  `MapStartupInputs::item_use`.
- `campaign_inputs.rs`: `prepare` builds the default inputs with `item_use: None`, which assembly
  refuses. `prepare_new_game`, the only caller, sets `Some(Rules::new(defend tables, hero overlay))`
  before assembly. The campaign item budget (`budgets()`) is set, so this path is live.
- `item_use_rules::Rules::new(tables, overlay)` reads the same tables and W3A overlay as the hero skill
  and powerup rules. It takes no image, since every value comes from the selected rows.
- Public API: `UseCheck`, `UseGrant`, `UseCommit` (re-exported from `playercolor_host`).
  `check_item_use(&self, HandleId, f64) -> Result<UseCheck, Error>` and
  `commit_item_use(&mut self, UseGrant) -> Result<UseCommit, Error>`. `Error` is the object host error.

## Tests

- `item_use_rules_tests.rs`: charge model, perishable flag, group and duration, absent group,
  `ignoreCD` flag, mixed ability refusal, W3A `acdn` override by level and index, refusal of negative
  or indexed overrides, and the gated census.
- `item_use_host_tests.rs`: admission order, get and set natives, null and removed handles, negative
  store, malformed `uses`, shared group, last-charge removal, duplicate and stale grants (charges,
  cooldown, holder, consumed, removed), effect refusal, `ignoreCD` refusal, non-finite time, and the
  charge budget.
- Holder, consumed and removed cases write the record state directly, as `RemoveItem`, the powerup
  path and `UnitAddItem` do. The inventory host's own transfer path is not exercised here.
