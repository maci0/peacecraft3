# Locked-chapter startup refusals: MDX names, stationary keys, footprint textures

Probe: `native_campaign::tests::selected_install_locked_chapter_startup_probe`,
one run per chapter against the selected install, headless through
`.scratch/run-asset-gate.sh`. Install data was read with `mpqxtract get`
into the worktree `.scratch/` only.

## Fixed

### MDX sequence names

`Units\NightElf\MalFurion\MalFurion.mdx` (War3x.mpq, 1188-byte SEQS payload,
9 records) stores record 7 as `Stand Ready`, then `\r\r\r\r\r\r\n`, then NUL
padding in its 80-byte name field. All intervals are ordered. `unit_model`
refused the model because it rejected any name with a control character. The
shipped chapters NightElfX04Interlude, NightElfX06 and NightElfX08Finale use
this unit. `004-mdx.md` already records that shipped SEQS anomalies are
reported, not refused.

Fix: drop the control-character rule. The stored bytes stay the name, and
`select` matches them by whitespace token. The interval check stays.

Tests (pcx-play `unit_model`): `sequence_names_keep_the_stored_bytes_and_still_match_by_token`,
`inverted_sequence_interval_is_still_refused`.

### Stationary collision keys

`UndeadX08.w3x` (TFT, War3x.mpq) `war3map.doo`, version 8, 3576 records. Two
`ITw2` walls (flags 0x02, life 100) share creation number 197 at
(-6112, -2528, -2.6) and (-6112, -2720, -0.6). Stationary layers were keyed by
creation number for placed destructables, so the second wall hit
`duplicate stationary key`.

Fix: every stationary layer and spatial pool owner is keyed by the arena
handle, the key script-created destructables already used. Removed:
`StationaryKey`, `Texture::Placed`, `PLACED_TAG`, `set_placed_active`. The
collision sync takes the handle only.

Correction: the `creation_numbers_are_unique` doc claimed uniqueness on the 132
shipped files. That measurement covers `.w3m`/`.w3n` only (`pcx_map::shipped_maps`).
The doc and the `creation_number` field doc now name UndeadX08 as a counterexample.

Test (pcx-play `destructable_host_tests`): `placed_destructables_sharing_a_creation_number_toggle_their_own_layers`.
It failed before the fix at the first post-kill cell check (cell 100 still 0x02)
and passes after.

### Absent weapon damage point

RoC Undead03 `war3map.j:1016` creates `nhem` (base `UnitWeapons` row `weapsOn` 0).
The map's `war3map.w3u` entry sets `uaen` (`attackBits`, the weapon-enable mask)
to 3 and sets attack 1 dice, sides, base damage, range, cooldown and target
types. Neither the base row nor the map sets `dmgpt1`, so the cell is empty.
`read_combat` required it at creation, and the nested order issued during
`CreateUnit` refused.

Fix: `dmgpt1` is read as an absent-capable number, like `Missilespeed`. Creation
and orders succeed. A swing refuses with `ABSENT_DAMAGE_POINT`, at the unit swing
and the ground-attack swing. The original's timing for an absent damage point is
not established, so no default is used.

Test (pcx-play `combat_tests`): `weapon_without_shipped_damage_point_refuses_only_at_swing`.
It failed before the fix at creation with `unit field dmgpt1 is empty`.

### Integer death type override

TFT UndeadX02 `war3map.j:2431` creates `n000`, a custom `nzom` whose
`war3map.w3u` entry sets `udea` (`UnitData/deathType`) to the integer word 3.
The base cell is the text `2`. The map writes the mask as type 0 (`int`), but
`unit_decay_host` read deathType through the token-text helper, which refuses a
non-string override. `CreateUnit` refused with `unit animation: text placement field required`.

Fix: deathType reads a text cell or string override as before, and an integer
override as the mask word. The mask check is unchanged: a bit outside the
admitted raise and decay bits still refuses with `unsupported UnitData/deathType mask`.

Test (pcx-play `unit_corpse_tests`): `map_integer_death_type_override_is_the_decay_mask`.
It failed before the fix with `text placement field required`.

### Repeated decorative doodad creation numbers

TFT UndeadX07a `war3map.doo` (version 8, 1166 instances) repeats creation numbers
in the range 0..=1164, and the original loads it. Admission refused any repeat with
`duplicate placed creation number`, and the renderer keyed actors by creation number.
The creation number is an editor label, as the destructable fix already established
for UndeadX08. Admission now keeps every row in file order, and the renderer keys
actors and failures by row index.

Test (pcx-play `doodad_host_tests`): `duplicate_creation_numbers_are_distinct_placements`.
It failed before the fix with `duplicate placed creation number`.

### Absent hero ability list cell

TFT HumanX06Finale `war3map.j:191` creates `N000`, a custom `Nklj` (Kil'jaeden).
The base `Units\UnitAbilities.slk` row for `Nklj` has no `heroAbilList` cell at
all, while its `abilList` cell is `_`. The probe reported the wrapped
`ability tables ...` message; the inner error is `hero skills: unsupported
UnitAbilities/heroAbilList`, from `ability_list` refusing a missing cell.

Fix: a missing cell is the empty list, as `training_command` and
`unit_animation_rarity` already read a missing list cell. Creation succeeds; a
learn of an ability outside the list grants nothing.

Test (pcx-play `unit_hero_tests`): `absent_hero_ability_cell_is_an_empty_list`.
It failed before the fix at `CreateUnit`.

## Footprint placement

Decoded from `Warcraft III.exe` 1.29.2.9231 (SHA-256 checked before reading). "Observed"
means read from the machine code; "inferred" is marked as such.

### Routines CreateUnit reaches

- `CreateUnit` (`0x48ee00`, name `0xe719e8`, registered at `0x49d20f`) multiplies the facing
  by the degree-to-radian constant `0x10db0f8` (`0x99ffe0`) without normalizing, then calls
  `0x663ca0` and `0x687ca0` (observed).
- `0x687ca0` takes the unit type's pathing texture (`0x5c3150`). With a texture it snaps the
  creation position with `0x4e00f0`, using the `Misc/BuildingAngle` quarter (below), and stores
  the snapped position in the creation record. The facing radians go into the record unchanged
  (observed).
- `0x66efc0`, CUnit vtable slot `0x1ac`, calls slot `0x148` (`0x6bd2a0`), the footprint
  registration (observed). It reads the facing from the unit's pose (`this+0x26c`, slot `0xb8`,
  then slot `0x1c`; `GetUnitFacing` at `0x497650` reads the same slot and multiplies by
  `0x10db11c`), classifies it with `0x4c6ab0`, snaps the pose position again with `0x4e00f0` and
  that quarter, walks the texture with `0x4dfb40` (callback `0x6bff00`) and registers with
  `0x805090` (observed). `0x6c06e0` (`0x6c0883`, `0x6c09a1`) contains the same sequence; its
  callers were not traced.

### Two quarters

- Creation snap quarter: `0x46dea0` reads `Misc/BuildingAngle`, truncates it to an integer,
  reduces it modulo 360 (`mov ecx,0x168` at `0x46dec3`), rounds toward zero to a multiple of
  90 (`push 0x5a` at `0x401740`) and adds 360 when negative. `0x687ca0` maps 0, 90, 180 to
  quarters 0, 1, 2 and everything else to 3 (observed). The shipped value 270 gives 3.
- Registered footprint quarter: `0x4c6ab0` takes the facing in radians. Within 0.001
  (`0x10db054`) of 0 gives 0, of pi/2 (`0x10db10c`) gives 1, of pi (`0x10db110`) gives 2, and any
  other angle gives 3, with no test for 3 pi/2 (observed). A facing of 45 or 270 degrees therefore
  registers quarter 3, and a created unit's facing selects the quarter, unlike the snap.
- That the pose slot `0x1c` returns the creation facing unchanged is inferred from
  `GetUnitFacing` reading it. How the pose normalizes an angle outside one turn is not decoded.

### The `0x9c(%esi)` field (open item 1)

`0x4e1130` is not a CreateUnit routine. Its only caller `0x603850` is slot 2 of the
`CPlacementBox` vtable (`0xe94028`, RTTI `.?AVCPlacementBox@@`), the building cursor box of the
placement UI. `+0x9c` is written once, by the constructor `0x5f7c50` (`0x5f7cb4`, fourth
argument), whose only caller is `0x6627c5` in `0x662520` (reached from `0x661590` and
`0x5fb52f`). `0x662520` takes the value from slot `0x234` of the ordering build ability, or
from `0x46dea0` when there is none. Every build ability vtable (`CAbilityBaseBuild`,
`CAbilityBuild`, Human, Orc, NightElf, Undead, Naga, Neutral, Item) has `0x46ab40` in slot
`0x234`, which calls `0x46dea0` and returns its value (observed). The cursor box quarter is
`BuildingAngle` in every path, with the same 0, 90, 180, else 3 classification.

### Snap `0x4e00f0` and the clamp object (open item 2)

- Arguments: texture (ecx), pointer to x, pointer to y, quarter. Native dimensions are
  `[+0x8, +0xc]`. `0x4deb67` loads the texture transposed: `+0x8` is the TGA height and `+0xc`
  the TGA width, with pixel index `height * column + row` (observed). The first axis is world x.
  For quarters 0 and 2 the x axis uses `+0x8` and y uses `+0xc`; for 1 and 3 they exchange.
- Per axis: clamp with `0x7f4a60`, truncate to an integer, remove the C remainder modulo 64
  (`and ecx,0x8000003f` at `0x4e01a1`), add 32 (`shl 5` at `0x4e01cf`, `0x4e01e6`) when bit 1 of
  the axis dimension is set, then add 16 (`push 0x10` at `0x403b40`) when the dimension is odd.
  Both additions take the sign of the clamped coordinate (negative only when below zero; zero
  and positive coordinates add) (observed).
- Clamp `0x7f4a60`: x to `[o+0x70, o+0x78 - 32]`, y to `[o+0x74, o+0x7c - 32]`; the margin is
  the real built by `push 0x20` at `0x4015b2` (observed).
- `o` is the 0x90-byte singleton `0x115e9ec`, allocated and constructed by `0x7f5520`
  (constructor `0x7f3600`, vtable `0xebcf64`; stored at `0x7f557c`). Its minimum fields start at
  zero (`0x7f369a`, `0x7f36a2`). The rectangle is written only by `0x7f4fe0` (`0x7f5039` to
  `0x7f504a`; `[+0x70]` from `rect[1]`, `[+0x74]` from `rect[0]`, `[+0x78]` from `rect[3]`,
  `[+0x7c]` from `rect[2]`), called once from `0x704c7e` with the rectangle `0x733c40` builds from
  the first and last records of the `+0xe0` array of the object `0x707750` returns. Not decoded: the numeric derivation
  inside `0x733c40` (packed 9-bit fields scaled by 128 through `0x11403cc`).
- Inferred: the same rectangle is the cell origin in `0x804be0` (cell = `(clamped position - [o+0x70]) / 32`),
  so it equals the pathing grid origin, and its upper corner the grid's far edge. The
  recreation uses the pathing grid extent.
- Traversal `0x4dfb40`: with `hx = (w - 1) * 16` and `hy = (h - 1) * 16` (constant 16 from
  `0x403b40`, step 32 from `0x403b30`) the start and steps per quarter are: 0: start
  `(x - hx, y - hy)`, along `+x`, rows `+y`; 1: `(x + hy, y - hx)`, along `+y`, rows `-x`;
  2: `(x + hx, y + hy)`, along `-x`, rows `-y`; 3: `(x - hy, y + hx)`, along `-y`, rows `+x`
  (observed, `w = +0x8`, `h = +0xc`). Texel centres sit at `(index - (dimension - 1) / 2)` cells
  from the snapped position: whole cells for an odd dimension, half cells for an even one.

### Texel outside the map (open item 3)

`0x804cba` is an instruction inside `0x804be0`, the pixel linker called by the texture callback
`0x6bff00` for every texel with nonzero flags. `0x804be0` first clamps the texel position with
`0x7f4a60`, then divides by the cell size and links the pixel to each entry whose mask matches
(observed). A texel outside the map is linked to the nearest border cell, green or not. Neither
refusal nor dropping is native.

## Footprint placement in the recreation

- `footprint_geometry.rs` holds the decoded rules. The snap operands (`0x4e01a1`, `0x4e01cf`,
  `0x4e01e6`, `0x403b40`, `0x4015b0`), the turn constants (`0x46dec3`, `0x401740`) and the facing
  constants are read from the fingerprinted image; `Misc/BuildingAngle` is read from the stock
  and map Misc profiles and is required. `Rules::with_footprint_geometry` attaches them
  (`campaign_inputs::prepare_new_game`).
- `CreateUnit` snaps the request by the BuildingAngle quarter, orients the registered
  footprint by the facing quarter, snaps the position again with that quarter and anchors the
  texels on the containing cell. Texel offsets rotate by the doubled-centre rule
  (`rotate_offset`), which reproduces the previous even-dimension formulas and handles odd and
  mixed dimensions. Texel cells outside the map clamp to the border (`clamp_cell`) in the unit
  path, the stationary path and the native pixel projection.
- A facing outside `[0, 360)` is refused (pose normalization not decoded).
- Map-loaded destructables and doodads keep the nearest-quarter rule and the rounded anchor on
  even axes. An odd axis is snapped first (`stationary_anchor`) and anchored on the cell that
  contains the snapped coordinate. `0x6bd2a0` is slot `0x148` of CWidget, CUnit, CDestructable and
  CItem alike (observed), so destructables take the snap; shipped HumanX04Interlude and NightElf07
  place the 29-wide texture on multiples of 64, which the snap moves by 16.
- Construction footprints (`building_raster`, `construction_position_live`) still refuse odd
  dimensions: the construction anchor and its snap parity are not decoded.
- Without the attached geometry (synthetic fixtures) the previous quarter-turn facing refusal
  stays.
- The plain footprint raster links a texel when the red channel has the bit that `0x4deb57`
  tests (`TexturePolicy::blocks_movement`), replacing the former 0 or 255 refusal.
  `PathTextures\24x24Unflyable.tga` (29 by 24, bottom-left origin) carries red values such as
  3, 0x4b and 0x17 in some texels, so it relies on this.

### Not decoded

- The quarter a map-loaded destructable or doodad registers with (the pose facing through
  `0x4c6ab0`, as for units); they keep the nearest-quarter rule.
- The pose's normalization of a facing outside `[0, 360)`; `0x733c40`'s numeric derivation; and
  the equality of the clamp rectangle with the pathing grid (inferred above).
- The AI and player construction anchor for odd footprints.
- That the texture's pixel order reaches the recreation's flags in the transposed order the
  loader uses. The recreation's existing row-by-column mapping agrees with the traversal above;
  no native capture checks it.


## Probe results

| Chapter | Before | After |
|---|---|---|
| TFT NightElfX08Finale | CreateUnit: MalFurion.mdx `invalid animation interval/name` | pass CreateUnit; `SetCampaignAvailable` unsupported by this host |
| TFT NightElfX06 | CreateUnit: MalFurion.mdx; then `IssueImmediateOrder` `frostarmoroff` | pass CreateUnit and `frostarmoroff` (autocast OFF, `CAbilityFrostArmorAuto`); `GroupAddUnit` refused: arguments do not match the group native declaration (`Scripts\Blizzard.j:4322`) |
| TFT NightElfX06Interlude | `IssueImmediateOrder` `manashieldoff` | superseded: `manashieldoff` now binds (see the 2026-11-11 Mana Shield table below) |
| TFT HumanX01 (after chat registration) | `TriggerRegisterPlayerChatEvent` unsupported by this host | passes registration (`war3map.j:7343`, see `script-trigger-admission.md`); next first error `UnitSetUsesAltIcon` unsupported (`Scripts\Blizzard.j:4270`), no repo evidence |
| TFT NightElfX06 (after null-unit fix) | `GroupAddUnit` with null unit (`udg_CaravanUnitArray[7]`, see `script-group-arena.md`) | passes `GroupAddUnit`; next first error `IssueTargetOrder` `autoharvestlumber` refused: worker lacks the attack work sequence (`Scripts\Blizzard.j:7808`), unit-activity semantics, not started |
| TFT NightElfX06Interlude | `IssueImmediateOrder` `manashieldoff` | unchanged: toggle, not bound (see `ability-autocast-off.md`) |
| RoC Undead03 | `IssueImmediateOrder` `restorationoff` (`war3map.j:880`) | pass `restorationoff` (autocast OFF, `CAbilityRestoration`); `CreateUnit` (`war3map.j:1016`) refused: nested `IssueTargetOrder` (`Scripts\common.j:106649`) `unit field dmgpt1 is empty`; not autocast, cause not traced |
| TFT UndeadX07b | `IssueImmediateOrder` `burrow` | unchanged: morph, not bound (see `ability-autocast-off.md`) |
| TFT NightElfX04Interlude | CreateUnit: MalFurion.mdx | exit 0 (probe passes) |
| TFT NightElfX05 | `native texture green channel outside map` | unchanged (open) |
| TFT HumanX06 | `native texture green channel outside map` | unchanged (open) |
| TFT HumanX04Interlude | `PathTextures\24x24Unflyable.tga` odd dimensions | unchanged (open) |
| RoC NightElf07 | `PathTextures\24x24Unflyable.tga` odd dimensions | unchanged (open) |
| TFT UndeadX08 | `duplicate stationary key` | pass placement; `UnitAddItemToSlotById` refused: unsupported passive ability `AIgd` with code `AIfb` on `gldo` |
| RoC Undead02 | `UnitAddItemToSlotById` refused: unsupported passive ability `AIfb` with code `AIfb` on `ofir` (`war3map.j:897`) | unchanged (orb refusal, see passive-held-items.md) |
| TFT UndeadX01Interlude | `UnitAddItemToSlotById` refused: `AIfb` on `ofir` (`war3map.j:159`) | unchanged (orb refusal) |
| TFT HumanX05 | `UnitAddItemToSlotById` refused: unsupported passive ability `AIcb` with code `AIcb` on `ocor` (`war3map.j:1550`) | unchanged (orb refusal) |

## Probe results (2026-11-11)

Each row is the first refusal of `selected_install_locked_chapter_startup_probe`
on the selected install, before and after this batch.

| Chapter | Before | After |
|---|---|---|
| RoC Orc03 | `Blizzard.j:3996` `SetDestructableMaxLife` unsupported | unchanged: native semantics not established |
| RoC Human07 | `Blizzard.j:2133` `AddUnitAnimationProperties` unsupported | unchanged: selection effect not established |
| RoC Undead06 | `CreateUnit`: `units\orc\HeroFarSeer\HeroFarSeer.mdx` invalid animation interval | unchanged: record 0 is genuinely inverted (see below) |
| TFT NightElfX08Finale | `Blizzard.j:6913` `SetCampaignAvailable` unsupported | passes `SetCampaignAvailable`; `Blizzard.j:6915` `ForceCampaignSelectScreen` unsupported |
| TFT NightElfX08Finale (after campaign-select record) | `Blizzard.j:6915` `ForceCampaignSelectScreen` unsupported | probe passes (exit 0). The call sets a host flag (`campaign_select_requested`, `game_exit_host.rs`); no consumer returns to the selection screen, so that return is not implemented |
| TFT UndeadX07c | `Blizzard.j:2128` `SetDoodadAnimationRect`: `ZZcd` unsupported `vertR` selector | passes the doodad; `Blizzard.j:4841` `SetPlayerAlliance` refused: arguments do not match the declaration |
| RoC Undead03 | `CreateUnit` (`war3map.j:1016`): `unit field dmgpt1 is empty` | passes `CreateUnit`; `IssueImmediateOrder` `repairoff` (`war3map.j:1019`) unsupported by this host |
| TFT UndeadX02 | `CreateUnit` (`war3map.j:2431`): `unit decay: unit animation: text placement field required` | passes `CreateUnit`; `UnitAddItemToSlotById` (`war3map.j:2470`) refused: unsupported passive ability `AIfb` on `ofir` (orb refusal, see passive-held-items.md) |
| TFT UndeadX07a | `Doodad("duplicate placed creation number")` at startup | passes doodad admission; `IssueImmediateOrder` (`war3map.j:1469`) refused: `autocast enabling/effects not implemented for [65, 67, 119, 98]` (`ACwb`) |
| TFT HumanX06Finale | `CreateUnit` (`war3map.j:191`): hero skills `unsupported UnitAbilities/heroAbilList` | passes `CreateUnit`; `ForceCampaignSelectScreen` (`Blizzard.j:6915`) unsupported |
| RoC NightElf03 | (not previously probed) | `IssueImmediateOrder` `innerfireon` (`war3map.j:675`) refused: `autocast enabling/effects not implemented for [65, 105, 110, 102]` (`Ainf`) |
| TFT UndeadX07c (after null-player fix) | `Blizzard.j:4841` `SetPlayerAlliance` refused: other player is null (`udg_AP3_Nerubians`, never assigned) | probe passes (exit 0); see `script-player-setup.md` |

`HeroFarSeer.mdx` (War3.mpq, 133,824 bytes) SEQS record 0 is named `nothing`,
with interval start `0xFFFFFF59` (signed -167) and end `0`. Every other record
is ordered. The reader keeps refusing the model, as the shipped-anomaly policy
above requires. Whether the engine tolerates such a record is unmeasured.

## Probe results (2026-11-11, Mana Shield and Immolation OFF)

Each row is the first refusal of `selected_install_locked_chapter_startup_probe`
on the selected install. The toggle OFF evidence is in `ability-autocast-off.md`.

| Chapter | Before | After |
|---|---|---|
| RoC NightElf06 | `IssueImmediateOrder` `unimmolation` (`war3map.j:827`) | pass `unimmolation`; `UnitAddItemToSlotById` refused: unsupported passive ability `AIfb` on `ofir` (`war3map.j:960`), orb refusal (see passive-held-items.md) |
| TFT NightElfX04 | `IssueImmediateOrder` `manashieldoff` (`war3map.j:1047`) | pass `manashieldoff`; `IssueTargetOrder` refused: `entangleinstant` is not the attack target order (`Scripts\Blizzard.j:7795`) |
| TFT NightElfX06Interlude | `IssueImmediateOrder` `manashieldoff` (`war3map.j:360`) | exit 0 (probe passes) |

## Probe results (2026-11-11, Repair OFF)

Each row is the first refusal of `selected_install_locked_chapter_startup_probe`
on the selected install. "Before" is `main` at `98ba0dc` without the Repair class.
The Repair and Town Bell evidence is in `ability-autocast-off.md`.

| Chapter | Before | After |
|---|---|---|
| RoC Undead03 | `IssueImmediateOrder` `repairoff` (`war3map.j:1019`) unsupported | pass `repairoff`; `IssueImmediateOrder` `townbelloff` (`war3map.j:1020`) unsupported: Town Bell not bound |
| RoC NightElf03 | `IssueImmediateOrder` `innerfireon` (`war3map.j:675`) refused: autocast ON | unchanged: ON refused |
| TFT UndeadX07a | `IssueImmediateOrder` `webon` (`war3map.j:1469`, class `ACwb`) refused: autocast ON | unchanged: ON refused |

## Probe results (2026-11-11, footprint placement)

Each row is the first refusal of `selected_install_locked_chapter_startup_probe`.

| Chapter | Before | After |
|---|---|---|
| RoC NightElf04 | `CreateUnit`: only quarter-turn footprint facing is admitted | passes `CreateUnit`; `IssueImmediateOrder` `unroot` (`war3map.j:1150`) unsupported by this host |
| TFT UndeadX01 | `CreateUnit`: only quarter-turn footprint facing is admitted | passes `CreateUnit`; `UnitAddItemToSlotById` refused: unsupported passive ability `AIdf` with code `AIsb` on `odef` (orb refusal) |
| TFT NightElfX07 | `CreateUnit`: only quarter-turn footprint facing is admitted | passes `CreateUnit`; `IssueImmediateOrder` `unroot` (`war3map.j:1157`) unsupported by this host |
| TFT NightElfX08 | `CreateUnit`: only quarter-turn footprint facing is admitted | passes `CreateUnit`; `IssueImmediateOrder` `unroot` (`war3map.j:1094`) unsupported by this host |
| TFT HumanX04Interlude | `CreateUnit`: `PathTextures\24x24Unflyable.tga` odd dimensions | exit 0 (probe passes, `executed_units=26`) |
| RoC NightElf07 | `CreateUnit`: `PathTextures\24x24Unflyable.tga` odd dimensions | passes `CreateUnit`; `UnitAddItemToSlotById` (`war3map.j:1037`) refused: unsupported passive ability `AIrc` on `ankh` |
| TFT NightElfX05 | `CreateUnit`: native texture green channel outside map | passes `CreateUnit`; `UnitAddItemToSlotById` refused: unsupported passive ability `AIgd` with code `AIfb` on `gldo` |
| TFT HumanX06 | `CreateUnit`: native texture green channel outside map | passes `CreateUnit`; `UnitAddItem` refused: unsupported passive ability `AIgd` with code `AIfb` on `gldo` |

## Probe results (2026-11-11, max life, animation properties, alt icon)

Each row is the first refusal of `selected_install_locked_chapter_startup_probe`.
"Before" is the refusal recorded in the rows above for `main`; the native named
there was not re-measured on the old binary. "After" is the run on this branch.

| Chapter | Before | After |
|---|---|---|
| RoC Orc03 | `Blizzard.j:3996` `SetDestructableMaxLife` unsupported | passes `SetDestructableMaxLife`; next `Blizzard.j:1321` `SetCameraTargetController` unsupported by this host |
| RoC Human07 | `Blizzard.j:2133` `AddUnitAnimationProperties` unsupported | unchanged: refuses at `Blizzard.j:2133` (list mechanics decoded, selector coupling open, see `unit-animation-properties.md`) |
| TFT HumanX01 | `Blizzard.j:4270` `UnitSetUsesAltIcon` unsupported | passes `UnitSetUsesAltIcon` (state only, see `unit-alt-icon.md`); next `war3map.j:7476` `SetAltMinimapIcon` unsupported by this host |
| RoC NightElf03 | (not probed before) | first refusal unchanged: `IssueImmediateOrder` `innerfireon` (`war3map.j:675`, autocast ON); the `SetDestructableMaxLifeBJ` at `war3map.j:2394` is not reached |
| RoC Orc08 | (not probed before) | probe passes (exit 0) |
| TFT HumanX03 | (not probed before) | `SetPlayerTechResearched` refused at `war3map.j:12489`; its `SetDestructableMaxLifeBJ` calls are not separately observed |

## Probe results (2026-11-11, native decode lane)

First refusal of `selected_install_locked_chapter_startup_probe` per chapter. "Before" is the
run on `f9c282e`, "after" is the final commit of this lane.

| Chapter | Before | After |
|---|---|---|
| RoC Undead05Interlude | `SetSoundParamsFromLabel`: sound parameter label absent `felhoundYes` | pass; `WaygateActivate` unsupported by this host (`Scripts\common.j:108999`) |
| TFT UndeadX07Interlude | `CreateUnit`: `regenMana` negative refused | unchanged: drain rule not decoded (see unit-regeneration.md) |
| RoC Undead03 | `IssueImmediateOrder` `townbelloff` (`war3map.j:1020`) | unchanged: no-militia branch decoded, not implemented (see ability-autocast-off.md) |
| TFT NightElfX06 | `IssueTargetOrder` `autoharvestlumber` "worker lacks the attack work sequence" | same message: the target form now returns false, but a harvest started by another route still enters the work animation (route not traced) |
| TFT NightElfX04 | `IssueTargetOrder` `entangleinstant`: "only the attack target order is supported" | unchanged refusal, precise message: gold mine conversion not decoded |

Open after this lane:

- `SetDoodadAnimationRect` footprint branch: the cell-count origin (`+0xa0`/`+0xa4`) is undecoded.
- Town Bell militia conversion: the callback and dispatcher are undecoded.
- Entangle mine conversion: undecoded.
- The `WaygateActivate` native (RoC Undead05): not started.

The orb refusals above (`AIfb`, `AIgd`, `AIcb`) no longer occur: orbs are admitted, and the eight chapters now stop at other refusals, listed in the locked-chapter probe table of [passive-held-items.md](passive-held-items.md).

## Held-item passive probe (2026-11-11, `AImm` lane)

First refusal of `selected_install_locked_chapter_startup_probe` before (sweep6 logs) and after.
`AImm` (Pendant of Mana `pmna`) is admitted, see "Maximum-mana family" in
[passive-held-items.md](passive-held-items.md). The other four classes stay refused with a reason;
their decode is under "Open held classes" in the same file.

| Chapter | Before | After |
|---|---|---|
| TFT OrcX03a | `AIbm` (code `AImm`) on `pmna` | `AIob` on `ofro` (orb of frost, not a decoded class) |
| TFT HumanX06 | `AIbm` (code `AImm`) on `pmna` | `AIsr` on `brac` (not a decoded class) |
| TFT UndeadX01 | `AIdf` (code `AIsb`) on `odef` | unchanged, now with the wrapped-ability reason |
| TFT UndeadX01Interlude | `AIdf` (code `AIsb`) on `odef` | unchanged, now with the wrapped-ability reason |
| TFT HumanX05 | `AIll` (code `AIsb`) on `oli2` | unchanged, now with the wrapped-ability reason |
| RoC NightElf07 | `AIrc` on `ankh` | unchanged, now with the reincarnation reason |
| RoC Human09 | `AIrc` on `ankh` | unchanged, now with the reincarnation reason |
| RoC Undead02 | `AIms` on `bspd` | unchanged, now with the speed-multiplier reason |
| TFT UndeadX02 | `AIpb` on `oven` | unchanged, now with the poison reason |

## Probe results (2026-11-11, night elf target orders)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is `c8032ac5`, "after"
is this lane. Decode and rules: [entangle-mine-conversion.md](entangle-mine-conversion.md),
[worker-harvest-orders.md](worker-harvest-orders.md), [ability-autocast-off.md](ability-autocast-off.md).

| Chapter | Before | After |
|---|---|---|
| RoC NightElf06 | `IssueTargetOrder` `entangleinstant`: conversion not decoded | probe passes (exit 0, `executed_units=211`) |
| TFT NightElfX04 | `IssueTargetOrder` `entangleinstant`: conversion not decoded | passes `entangleinstant`; `IssueImmediateOrder` `rechargeon` on `emow` unsupported (autocast ON class, not started) |
| RoC NightElf02 | `IssueTargetOrder` `load`: only the attack target order is supported | passes `load` (returns false: the Orc Burrow has no cargo-load ability); native `UnitId` unsupported (`Blizzard.j:2998`) |
| TFT NightElfX06 | `autoharvestlumber`: worker lacks the attack work sequence | passes the work animation; wisp harvest refused: unit has no `Ahar` harvest capability (`Awha` is a different harvest class, not started) |
| RoC Undead03 | `IssueImmediateOrder` `townbelloff` unsupported | passes `townbelloff` (no militia, returns false); `SetUnitState` refused: arguments do not match the unit native declaration |

## 2026-11-11 autocast ON orders

`IssueImmediateOrder` ON orders for the decoded classes now bind (`ability-autocast-on.md`; `CAbilityBlackArrow` and `CAbilityCurse` joined the bound classes). Probe: `selected_install_locked_chapter_startup_probe`, first refusal before and after.

| Chapter | Before | After |
| --- | --- | --- |
| RoC NightElf03 | `innerfireon` (`Ainf`) | passes `innerfireon`; `curseon` (`ACcs`, `Acrs`) then bound too; `SetUnitState` refused: arguments do not match the unit native declaration (`Scripts\common.j:91204`) |
| RoC Undead02Interlude | `raisedeadon` (`ACrd`, code `Arai`) | passes; `UnitAddItemToSlotById` refused: unsupported passive ability `AIlb` on `olig` |
| TFT UndeadX07a | `webon` (`ACwb`) | passes; probe exits 0 (`executed_units=64`) |
| TFT NightElfX03 | `blackarrowon` unsupported immediate order | passes; `SetDoodadAnimationRect` refused: footprint doodad rectangle selection needs the native footprint cell-count origin (`script-doodads.md`) |

## Probe results (2026-11-11, root and burrow lane)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is the sweep-6 log, "after"
is this branch. Decode, addresses and the rules implemented: [unit-root-burrow.md](unit-root-burrow.md).

| Chapter | Before | After |
|---|---|---|
| RoC NightElf01 | `IssueImmediateOrder` `unroot` on `etol` (`war3map.j:810`) unsupported | probe passes (`executed_units=105`, published) |
| RoC NightElf04 | `IssueImmediateOrder` `unroot` on `etoa` unsupported | probe passes (`executed_units=275`, published) |
| TFT NightElfX07 | `IssueImmediateOrder` `unroot` on `eaow` (`war3map.j:1157`) unsupported | passes every `unroot`; `IssueImmediateOrder` at `war3map.j:1520` refused: autocast enabling for `Aweb` not implemented |
| TFT NightElfX08 | `IssueImmediateOrder` `unroot` on `etrp` (`war3map.j:1094`) unsupported | passes `unroot`; `CreateUnit` at `war3map.j:1282` refused: `UnitBalance` `regenMana` required finite |
| TFT UndeadX04 | `IssueImmediateOrder` `burrow` on `u000` (`war3map.j:1607`) unsupported | refused with a precise message: morph flags `0x1` of class `Abur`, form swap time for a form without the immediate bit not decoded |
| TFT UndeadX07b | `IssueImmediateOrder` `burrow` on `nanb` (`war3map.j:1620`) unsupported | passes (no ability handles the order, native returns false); `GetDestructableOccluderHeight` unsupported (`Blizzard.j:4027`) |

Open after this lane:

- `burrow` on `Abur`: who posts event `0xd01c4`, hence when the form changes for `DataB=1`.
- Uprooted movement, the point order `root`, and the creation-time rooted attack mask (`0xb8f470`).

## Frost orb and spell resistance admitted

| Chapter | Before | After |
|---|---|---|
| TFT OrcX03a | `AIob` on `ofro` | `AIms` on `bspd` (speed-multiplier reason) |
| TFT HumanX06 | `AIsr` on `brac` | `AIau` (code `AUau`) on `lgdh` (not a decoded class) |
| RoC Undead02 | `AIms` on `bspd` | unchanged |

## Probe results (2026-11-11, local UI, camera, guard and stock natives)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is the sweep on
`c8032ac5`, "after" is the final commit of this lane. Decodes are in
[local-ui-natives.md](local-ui-natives.md), [script-unit-guards.md](script-unit-guards.md),
[script-stock-slots.md](script-stock-slots.md) and
[script-immediate-orders.md](script-immediate-orders.md).

| Chapter | Before | After |
|---|---|---|
| TFT HumanX01 | `SetAltMinimapIcon` unsupported (`war3map.j:7476`) | passes startup (189 units) |
| TFT UndeadX08 | `SetAltMinimapIcon` unsupported (`war3map.j:8396`) | passes startup (280 units); the `RecycleGuardPosition` calls are admitted |
| TFT UndeadX06 | `SetReservedLocalHeroButtons` unsupported (`war3map.j:2182`) | `UnitAddItem`: unsupported passive ability `AIsx` with code `AIas` on `gcel` |
| RoC Orc03 | `SetCameraTargetController` unsupported (`Blizzard.j:1321`) | passes startup (222 units) after `SetCameraField` |
| TFT NightElfX05 | `RecycleGuardPosition` unsupported (`war3map.j:2473`) | `IssueTargetOrder` `autoharvestlumber`: worker lacks the attack work sequence |
| TFT UndeadX03 | `AddUnitToStock` unsupported (`Blizzard.j:3847`) | `AddUnitToStock`: shop slot count is not assigned (no `SetUnitTypeSlots`; the constructor default is not read from the image) |
| TFT HumanX03 | `IssueImmediateOrder` `defend` on `hfoo` unsupported | passes startup (182 units) after `SetUnitCreepGuard` |

Open after this lane:

- Unit shop default slot count: the `Asud` constructor writes `0xb` at `0xb353d8`; deriving it from the image is not done.
- `RecycleGuardPosition` ability list flag (`0x68e1a0`) and the global option word tested by `0x46e030` (mask `0x100`) are undecoded.
- Camera target controller: per-frame follow and the calls that release it are undecoded; target reads refuse while one is set.

## Probe results (2026-11-11, negative mana regeneration lane)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is the sweep-6 log. Decode and addresses: [unit-regeneration.md](unit-regeneration.md#native-negative-mana-rate).

| Chapter | Before | After |
|---|---|---|
| TFT UndeadX07Interlude | `CreateUnit` refused: finite nonnegative unit `UnitBalance` `regenMana` required (`ubsp`, -3) | probe passes (`executed_units=42`) |
| TFT NightElfX08 | `CreateUnit` at `war3map.j:1282` refused: `regenMana` required finite | passes; `SetReservedLocalHeroButtons` unsupported by this host (`war3map.j:2441`) |

## Map-defined objects (2026-11-11)

Details in [custom-object-resolution.md](custom-object-resolution.md). Probe run per chapter
on the lane worktree, before is the `sweep6` log.

| Chapter | Before | After |
|---|---|---|
| TFT HumanX02 | `CreateUnit` (`war3map.j:1969`): hero skills `Ability: no shipped object A001` (`nbal` lists an id with no object data) | passes; next `CreateUnit` for `ubsp` (`war3map.j:1987`): `UnitBalance regenMana` required finite nonnegative, shipped row is `-3` (drain rule undecoded) |
| TFT UndeadX05 | `UnitAddItem` (`Blizzard.j:2640`): `Item: no shipped object I003` (custom item over `oven`) | passes the custom item; next `UnitAddItem` same call: `unsupported passive ability AIpb with code AIpb on oven` (Orb of Venom on-hit poison undecoded) |

Also fixed without a probe change: the placement catalog read `w3h` (buffs) as the
destructable member; it now reads `w3b`. Custom buffs (`w3h`) and custom item sale rows stay
unresolved on their paths; no indexed chapter reaches them.

## 2026-11-11 rooted attack mask, Aweb ON and occluder height

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is the probe at `4364677f`
(re-run for NightElfX07 and UndeadX07b; NightElf01 and NightElf04 passed there per the root lane),
"after" is this branch. Decode: [unit-root-burrow.md](unit-root-burrow.md),
[ability-autocast-on.md](ability-autocast-on.md),
[destructable-occluder-height.md](destructable-occluder-height.md).

| Chapter | Before | After |
|---|---|---|
| RoC NightElf01 | passes (`executed_units=105`) | passes (`executed_units=105`) |
| RoC NightElf04 | passes (`executed_units=275`) | passes (`executed_units=275`) |
| TFT NightElfX07 | passes (`executed_units=279`); the `Aweb` refusal in the root lane's row predates the ON binding | passes (`executed_units=279`) |
| TFT UndeadX07b | `GetDestructableOccluderHeight` unsupported (`Blizzard.j:4027`) | `native sight wide occluder unsupported: DTrf` (vision rebuild; `DTrf` is the `Elevator` destructable with `occH` 0 and `fatLOS` 1, so the height above zero comes from `SetDestructableOccluderHeight`) |

An unarmed rootable building (`edob`, `edos`: `weapTp1` a placeholder) rooted under `Aro1`
`DataA=1` would refuse creation if the mask resolved attack 1 strictly; it enables nothing instead.

Open after this lane:

- `fatLOS` (wide) occluders with a height above zero: creation sets unit flag `0x400` and registers
  through `0x6faea0` (`0x6ce2fc`..`0x6ce324`); the footprint it fills is not decoded.
- Both attacks enabled at once (`weapsOn=3` or a `DataA` of 3): the combat model holds one weapon.

## Probe results (2026-11-11, wisp harvest, UnitId, null SetUnitState)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is `main`
at `87bf80be`, "after" is this lane. Decodes: [wisp-harvest.md](wisp-harvest.md),
[unit-id-native.md](unit-id-native.md), "Null unit handle" in [unit-state-maxima.md](unit-state-maxima.md).

| Chapter | Before | After |
|---|---|---|
| RoC NightElf02 | `UnitId` unsupported (`Blizzard.j:2998`) | probe passes (exit 0, `executed_units=135`) |
| TFT NightElfX06 | `harvest` target order on a wisp: unit has no `Ahar` harvest capability (`Blizzard.j:7808`) | probe passes (exit 0, `executed_units=188`) |
| RoC Undead03 | `SetUnitState`: arguments do not match the unit native declaration (`Blizzard.j:3315`, `SetUnitManaBJ` of a null `udg_Arthas`) | `GetPlayerUnitCount` unsupported (`war3map.j:5454`), not started |

## Probe results (2026-11-11, shop default slots and held attack speed)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is the base of this lane (`0ff16d9c`), "after" is its final commit. Decodes: [script-stock-slots.md](script-stock-slots.md#default-slot-count) and the attack speed section of [passive-held-items.md](passive-held-items.md).

| Chapter | Before | After |
|---|---|---|
| TFT UndeadX03 | `AddUnitToStock`: shop slot count is not assigned | passes startup (`executed_units=208`) |
| TFT UndeadX06 | `UnitAddItem`: unsupported passive ability `AIsx` with code `AIas` on `gcel` | `UnitAddItem`: unsupported passive ability `AIdf` with code `AIsb` on `odef` (the orb wraps Black Arrow; not a close sibling, see the `AIsb` entry in passive-held-items.md) |

Open after this lane:

- A unit without an `Asid` or `Asud` ability stores stock here; the native returns silently for it.
- Attack speed: that agility and the cooldown consumer use the same multiplier object, and the illusion gate, are inferred or not modelled (see passive-held-items.md).

## Movement speed model and boots

First refusal of `selected_install_locked_chapter_startup_probe`, before (the table above) and after the speed model (`unit-move-speed-model.md`) and the `AIms` admission. Boots bind when `Misc/MoveSpeedBonusesStack` is 0 (root `MiscGame.txt`, `Melee_V0`); the chapters run with `Custom_V0` or `Custom_V1`, which set it to 1, so they stay on the undecoded stacking path.

| Chapter | Before | After |
|---|---|---|
| TFT OrcX03a | `AIms` on `bspd` (speed-multiplier reason) | `UnitAddItemToSlotById` refused: `AIms` on `bspd`, `Misc/MoveSpeedBonusesStack` is set, where the summed bonus record reaches the speed is not decoded (`passive-held-items.md`) |
| RoC Undead02 | `AIms` on `bspd` | same refusal as OrcX03a |
| TFT HumanX06 | `AIau` (code `AUau`) on `lgdh` (not a decoded class) | `UnitAddItem` refused: `AIau` on `lgdh`, with the decoded parts named (class `aura`, buff `BUau`); the aura scan, buff duration and regeneration addition are open |

## Probe results (2026-11-11, Orb of Venom and AIlb lane)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is the refusal recorded
above for each chapter (not re-run on a baseline build); "after" is a probe on this branch.
Decode and addresses: [passive-held-items.md](passive-held-items.md) (`AIpb`, `AIlb`).

| Chapter | Before | After |
|---|---|---|
| TFT UndeadX02 | `AIpb` on `oven` unsupported | `IssueImmediateOrder` (`war3map.j:2600`): unit activity morph flags `0x1` of class `Abur` is not decoded |
| TFT UndeadX05 | `UnitAddItem`: `AIpb` on `oven` (custom item `I003`) unsupported | `UnitAddItem`: unsupported passive ability `AIsx` with code `AIas` on `gcel` |
| RoC Undead02Interlude | `UnitAddItemToSlotById`: `AIlb` on `olig` unsupported | unchanged chapter stop, now with the reason: `AIlb` casts `AIlp`, a Purge (`Aprg`, `Bprg`) the recreation lacks |

## 2026-11-11 player unit count, null `SetUnitState`, sight override test

First refusal of `selected_install_locked_chapter_startup_probe`; "before" is this branch's base
`b496c8f5`. Decode: [script-unit-world.md](script-unit-world.md) (Player unit count),
[destructable-occluder-height.md](destructable-occluder-height.md).

| Chapter | Before | After |
|---|---|---|
| RoC Undead03 | `SetUnitState` (`Blizzard.j:3315`, `SetUnitLifeBJ`): `arguments do not match the unit native declaration` (null unit; handler `0x4abba0` returns when the unit does not resolve) | passes (`executed_units=185`) |
| TFT UndeadX07b | `native sight wide occluder unsupported: DTrf` | unchanged (message now names the undecoded gate) |

`GetPlayerUnitCount` was not the first refusal at this base: the probe stops earlier on the null
`SetUnitState`, and after that fix startup completes, so the `war3map.j:5454` call is not
reached by the startup probe. The native is implemented and unit-tested (see the note above).
`GetPlayerStructureCount(player, true)` now also counts queued structure-upgrade slots.

Still refused: wide (`fatLOS`) occluders with a height above zero (gate and footprint source
undecoded, see the occluder note) and `GetPlayerTypedUnitCount` (name table undecoded).

## Boots under the stacking rule sets, aura decode

First refusal of `selected_install_locked_chapter_startup_probe` (one run each, selected Legacy install, headless), before and after the change that admits `AIms` under `Misc/MoveSpeedBonusesStack` (the flag does not change the formula, see `passive-held-items.md`) and records the `AIau` decode.

| Chapter | Before | After |
|---|---|---|
| RoC Undead02 | `UnitAddItemToSlotById` refused: `AIms` on `bspd`, `Misc/MoveSpeedBonusesStack` is set | startup passes: `executed_units=123`, `published=true` (first refusal moves past this chapter's startup) |
| TFT OrcX03a | `UnitAddItemToSlotById` refused: `AIms` on `bspd`, `Misc/MoveSpeedBonusesStack` is set | `UnitAddItemToSlotById` refused: `AIva` on `modt` (unsupported passive ability) |
| TFT HumanX06 | `UnitAddItem` refused: `AIau` (code `AUau`) on `lgdh` | same item, new message: the buff halves are decoded; the range event timing, buff end after leaving the area and the stacking test against the hero aura are not |

## 2026-11-11 spell orbs (`AIsb`)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is the `AIsb` pickup
refusal at `b496c8f5` named for these chapters (the 2026-11-10 rows above record the same refusal;
not re-run), "after" is this branch. Decode: [passive-held-items.md](passive-held-items.md),
"Spell orbs".

| Chapter | Before | After |
|---|---|---|
| TFT UndeadX01 | `UnitAddItem`: `AIdf` (code `AIsb`) on `odef` | passes the pickup; next `UnitAddItem`: `AIpb` on `oven` (Orb of Venom, poison undecoded) |
| TFT UndeadX01Interlude | `UnitAddItemToSlotById`: `AIdf` on `odef` | passes the pickup; next `UnitAddItemToSlotById`: `AIpb` on `oven` |
| TFT UndeadX06 | `UnitAddItem`: `AIdf` on `odef` | passes the pickup; next `UnitAddItem`: `AIsx` (code `AIas`) on `gcel` (attack-speed bonus, undecoded) |
| TFT HumanX05 | `UnitAddItem`: `AIll` on `oli2` | passes the pickup; next `MultiboardSuppressDisplay` unsupported by this host (`Blizzard.j`) |

The probes stop at startup, so no swing happens. Once a hero with the orb swings: the proc roll
needs a seeded random state; Black Arrow marks and a marked unit's death refuses (summon
placement undecoded); a Purge proc (`oli2`) refuses at the hit. The movement-speed code is not
touched, because the Purge slow stays refused.

The "next" refusals above were recorded before `AIpb` (Orb of Venom) and `AIsx` (Gloves of Haste)
landed on main; both are implemented there now, and these chapters were not re-probed after the merge.

## 2026-11-11 footprint doodad selection and absent unit models

Same exe (`Warcraft III.exe` 1.29.2.9231, SHA-256 `3f2ed012...0eed`). Footprint decode:
[script-doodads.md](script-doodads.md).

### `IntentionallyLeftBlank.mdx`

- Unit `nzin` (UnitUI row `z3`, `file` `Doodads\Terrain\LOSBlocker\IntentionallyLeftBlank`) is
  created by OrcX01 and OrcX02 from stock data (no map edit of `nzin`). `mpqxtract show` for the
  `.mdx` and `.mdl` spelling finds no entry in the install's `War3.mpq`, `War3x.mpq`,
  `War3xLocal.mpq`, `War3Local.mpq`, `Deprecated.mpq`, nor in the 1.28c `wc3/game/app` `war3.mpq`,
  `War3x.mpq`, `War3xlocal.mpq`, `War3Patch.mpq`, nor in the two map archives. Only
  `LOSBlocker.mdx` ships in that directory (`War3.mpq`, 1296 bytes). The file is absent, not
  misspelled.
- Model loader `0x913660` (path stripped of its extension, cache lookup `0x915370`, file reader
  `0x8ec3c0` -> `0x8ec840`): `0x8ec840` opens the `.mdx`, retries the other extension (`0x8ec7d0`),
  and when both fail formats a system message through the error sink (virtual `+0x10`,
  `0x8ec475`) and returns 0. The cache node keeps a null model (`0x9137e8`) and `0x914590`
  returns 0 for it (`0x9145c4`..`0x9145da`). Observed.
- The sprite model setter `0x967210` stores that result at `[sprite+0x20]` (`0x967306`) and returns
  without error on null (`0x96731a` -> `0x96746e`); accessors on a sprite without a model return
  early (`0x967490`: `[+0x20]` null). Observed.
- Not traced: the CUnit path that reaches `0x967210` with the unit's `file` string. That unit
  creation continues is inferred from the null-tolerant setter and from OrcX01/OrcX02 creating
  `nzin` from stock data; no runtime observation of the original exists.
- Recreation: `UnitModels` (`campaign_assets.rs`) treats a name no layer holds as "no model": the
  unit is created without animation and the renderer skips it. A missing `.mdx` whose `.mdl`
  exists, and every layer, budget and read failure, still refuse.

### Probe results

First refusal of `selected_install_locked_chapter_startup_probe`; "before" is `2822f6bc`.

| Chapter | Before | After |
|---|---|---|
| RoC Orc07 | `SetDoodadAnimationRect`: footprint doodad rectangle selection undecoded | pass (`executed_units=136`) |
| RoC Undead05 | same `SetDoodadAnimationRect` refusal | `autoharvestlumber`: unit has no `Ahar` harvest capability |
| TFT NightElfX03 | same `SetDoodadAnimationRect` refusal | pass (`executed_units=214`) |
| TFT OrcX01 | `CreateUnit`: `IntentionallyLeftBlank.mdx` absent from every mounted layer | `UnitModifySkillPoints` unsupported by this host |
| TFT OrcX02 | same `CreateUnit` refusal | `UnitModifySkillPoints` unsupported by this host |

Still open: BLP pathing texture sizes for doodads (`0x71a7c0` `.blp` branch), the `0x1000000`/`+0x14c`
alive-or-dead path choice in `0x75b150`, and the unit-side caller of the sprite model setter.

## 2026-11-11 delayed `Abur` burrow

First refusal of `selected_install_locked_chapter_startup_probe`; "before" is this branch's base
`2822f6bc`, run with `.scratch/chapter-sweep.sh`. Decode, poster of event `0xd01c4` and the
custom unit `u000`: [unit-root-burrow.md](unit-root-burrow.md).

| Chapter | Before | After |
|---|---|---|
| TFT UndeadX02 | `IssueImmediateOrder` `burrow` (`war3map.j:2600`): morph flags `0x1` of class `Abur` not decoded | the burrow orders at `war3map.j:2600` to `2607` are accepted; `UnitAddItemToSlotById` (`war3map.j:2621`) refused: passive ability `AIdf` (code `AIsb`) on `odef`, the wrapped orb ability is not modeled. The orders at `2644` to `2653` follow it and were not reached. |
| TFT UndeadX04 | `IssueImmediateOrder` `burrow` on `u000` (`war3map.j:1607`): same refusal | passes (`executed_units=298`, published) |
| TFT UndeadX07b | `native sight wide occluder unsupported: DTrf` | unchanged |
| RoC Human02Interlude (Raven Form, immediate path) | passes (`executed_units=40`) | passes (`executed_units=40`) |

Whether the probe advances the game clock past `Dur` (`1.45` s) was not checked; the swap is covered
by unit tests (`burrow_*` in `unit_morph_host_tests.rs`).

## Moon Well autocast, Sentinel, alarm ignore and animation properties

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" is the refusal recorded
for the chapter by the earlier lanes (not re-run on a baseline build); "after" is a probe on this
branch. Decodes: [ability-autocast-on.md](ability-autocast-on.md) (`CAbilityManaBattery`),
[ability-sentinel.md](ability-sentinel.md), [unit-alarm-ignore.md](unit-alarm-ignore.md),
[unit-animation-properties.md](unit-animation-properties.md).

| Chapter | Before | After |
|---|---|---|
| TFT NightElfX04 | `IssueImmediateOrder` `rechargeon` on `emow` unsupported | passes `rechargeon`; `SetUnitUseFood` (`Blizzard.j:3862`, `SetUnitUseFoodBJ`) is an unsupported native |
| TFT NightElfX08 | `rechargeon` on `emow` unsupported | passes (`executed_units=297`) |
| RoC NightElf03 | `IssueTargetOrder` `sentinel`: only the attack target order is supported | same stop, now naming the undecoded Sentinel cast (`CAbilitySentinel`) |
| TFT UndeadX05 | `UnitIgnoreAlarm` unsupported native | passes (`executed_units=252`) |
| RoC Human07 | `AddUnitAnimationProperties` unsupported (`Blizzard.j:2133`) | passes (`executed_units=121`) |

`CAbilityManaBattery` (`Ambt`, `Amb2`) was missing from the bound autocast classes; it shares the
base ON/OFF handlers. Its data authors only `Order=recharge`, so the toggle ids come from the class.

`SetUnitUseFood` handler `0x4abc70` (not implemented): with a live unit it reads flag `0x200` of
`[unit+0x16c]`. Enabling with the flag clear passes the unit type's food cost (`0x6902a0`) to the
owner-side routine `0x4b57d0` (the counter it adjusts is not decoded) and sets `0x300`. Disabling with the flag set passes it negated
and clears `0x300`. The unit's initial flag state and the player food model are not decoded here.

## 2026-11-11 skill points, Ahrl harvest, unit user data

Same exe (`Warcraft III.exe` 1.29.2.9231, SHA-256 `3f2ed012...0eed`). Decodes:
[hero-skill-points.md](hero-skill-points.md), [ghoul-harvest.md](ghoul-harvest.md),
[unit-user-data.md](unit-user-data.md).

- `UnitModifySkillPoints` (`0x4af4b0` -> `0x697730` -> `0xb3f670`): a decrease is limited to the
  unspent points, an increase to the unlearned ranks of the hero skill slots, and the result is
  true whenever the first limit leaves a nonzero delta. `UnitStripHeroLevel` (`0x4af870` ->
  `0x69ea60` -> `0xb40c10`) is not decoded and stays unsupported.
- `autoharvestlumber` in RoC Undead05 is issued to a `ugho` (Ghoul, `Acan,Ahrl,Aiun`). `Ahrl` is
  `CAbilityHarvestLumber` (vtable `0xf2ba54`, loader `0xb771a0`): `DataA` damage, `DataB` lumber
  capacity, no `DataC`. `Ahar` (loader `0xb77c40`) extends it with the gold capacity. The immediate
  validator `0xb75d00` accepts `autoharvestlumber` on the lumber capacity only.
- OrcX01 reached `SetUnitUserData` next: `0x497d50` and `0x4abcf0` read and write `[unit+0x488]`.
  A read of a never-written value refuses (creation default not traced).

First refusal of `selected_install_locked_chapter_startup_probe`; "before" is `4f8724e5`.

| Chapter | Before | After |
|---|---|---|
| RoC Undead05 | `autoharvestlumber`: unit `ugho` has no `Ahar` harvest capability | pass (`executed_units=194`) |
| TFT OrcX01 | `UnitModifySkillPoints` unsupported by this host | pass (`executed_units=239`) |
| TFT OrcX02 | `UnitModifySkillPoints` unsupported by this host | `UnitAddItem`: damage metadata does not admit ability `AItx` |

`AItx` ("AttackBonus +20", item, code `AIat`) is not in the damage metadata alias list of
`inventory_powerup.rs`; whether the class reads `DataA` for it was not decoded, so OrcX02 stays
refused there.

## 2026-11-11 `Sentinel` and `SetUnitUseFood`

First refusal of `selected_install_locked_chapter_startup_probe`; "before" is the refusal recorded
above for the chapter, "after" is a probe on this branch with `.scratch/run-asset-gate.sh`. Decodes:
[ability-sentinel.md](ability-sentinel.md), [unit-use-food.md](unit-use-food.md).

| Chapter | Before | After |
|---|---|---|
| RoC NightElf03 | `IssueTargetOrder` `sentinel`: the Sentinel cast (`CAbilitySentinel`) not decoded | passes (`executed_units=112`) |
| TFT NightElfX04 | `SetUnitUseFood` (`Blizzard.j:3862`) unsupported native | passes (`executed_units=224`) |

The Sentinel order is accepted without the missile flight: the owl's sight circle (`DataB`) attaches
at the cast, not at impact, and the in-flight radius (`DataA`) is not modeled. A target that is not a
live tree, a tree beyond `Rng1`, and an exhausted Sentinel still refuse.

## 2026-11-11 AItx and the unit user data default

Same exe (`Warcraft III.exe` 1.29.2.9231, SHA-256 `3f2ed012...0eed`).

- `AItx` ("AttackBonus +20", code `AIat`, `DataA1` 20) is the only `AIat`-code row that the `Iatt`
  `useSpecific` list omits (`AbilityMetaData.slk`, War3.mpq and War3x.mpq identical). The `AIat`
  attach `0xbc2a20` formats `"%s%d"` with `DataA` and level 1 and reads that cell through
  `0x6b06a0`; it never consults the metadata list. The damage branch of `held_attributes` now admits
  by class code, so `AItx` holds +20. The `DataA` column and the W3A override pointer still come
  from `Iatt`. Other classes with the same gap, still refused by their alias lists: `AIde` rows
  `AId0`, `AId7`, `AId8`; `AIab` rows `AIx3`, `AIx4`; `AIml` row `AIlz`; `AImm` rows `AI2m`, `AImv`,
  `AImz`. Not decoded whether each is held by a shipped item.
- Unit user data: the CUnit constructor `0x6756d0` stores `eax` (zeroed by `xor eax,eax` at
  `0x675cc6`) to `[unit+0x488]` at `0x675d0d`. The store at `0x6a7cda` belongs to another class
  (constructor `0x6a71e0`, allocation size `0x8fc`, vtable `0xea0388`). An unwritten unit reads 0;
  the refusal is gone. The only other CUnit accesses are the getter `0x497d64` and setter `0x4abd05`,
  so nothing clears the field before the handle stops resolving.

| Chapter | Before | After |
|---|---|---|
| TFT OrcX02 | `UnitAddItem`: damage metadata does not admit ability `AItx` | `UnitAddItem`: unsupported passive ability `AIva` with code `AIva` on `klmm` |

`AIva` (an item vampiric aura, also behind OrcX03a) needs the aura and life-steal classes decoded
and stays refused.

## Probe results (2026-11-11, Purge and Ankh lane)

First refusal of `selected_install_locked_chapter_startup_probe`. "Before" was probed on the
unmodified base `c30b6d7b` (a saved test binary), "after" on this branch. Decode and addresses:
[passive-held-items.md](passive-held-items.md) (`AIrc`, "Purge").

| Chapter | Before | After |
|---|---|---|
| RoC NightElf07 | `UnitAddItemToSlotById` (`war3map.j:1037`): `AIrc` on `ankh` unsupported | passes the pickup; next `UnitAddItemToSlotById` (`war3map.j:1041`): unsupported passive ability `Adt1` (code `Adet`) on `gemt` |
| RoC Human09 | `UnitAddItem`: `AIrc` on `ankh` unsupported | passes (`executed_units=199`) |
| RoC Undead02Interlude | `UnitAddItemToSlotById`: `AIlb` on `olig` unsupported (the Purge reason) | passes `olig`; next `UnitAddItemToSlotById`: a second orb on one holder is unsupported (the firing order of several orbs is not decoded) |
| TFT HumanX05 | `MultiboardSuppressDisplay` unsupported by this host | unchanged: the probe stops on the multiboard native before any swing, so the `AIll` hit is not reached |

Not reached by a startup probe, so refused or open in code: a death of a unit that holds an ankh
and a hero reincarnation ability, several ankhs, or a flier; Purge pauses (`DataD`, `DataE`, so the
shipped `AIpg` row of `AIll`), a Purge on a summoned enemy (magic-type damage path), and a Purge
cast on a unit with a buff whose magic bit is unread. Open: `Adet` (`gemt`) and the multi-orb order.

## 2026-11-11 Held classes by class code, `AIva`, aura buff lifetime

Same exe (`Warcraft III.exe` 1.29.2.9231, SHA-256 `3f2ed012...0eed`). Decodes and addresses:
[passive-held-items.md](passive-held-items.md) ("Held classes are admitted by class code", `AIva`,
"Aura buff lifetime").

- Every held class attach reads `DataA` of its row by class code (`AIde`, `AIml`, `AImm`, `AIab`, `Arel`,
  `AIrm`, `AIas`, powerup `AIsm`, healing `AIhe`). The `useSpecific` gate is removed for all of them.
- `AIva` is a vampiric attack on the holder's hits (no `Area1`, no buff), now implemented.
- `BUau` ends 4.0 s after its last application (timer `0xd01c4`). Whether the range registration
  repeats for units that stay in range is not decoded, so `AUau` stays refused.

First refusal of `selected_install_locked_chapter_startup_probe` (`.scratch/run-asset-gate.sh`):

| Chapter | Before | After |
|---|---|---|
| TFT OrcX02 | `UnitAddItem`: unsupported passive ability `AIva` on `klmm` | `UnitAddItem`: unsupported passive ability `AIad` (code `AHad`) on `blba` |
| TFT OrcX03a | `UnitAddItemToSlotById`: unsupported passive ability `AIva` on `modt` | same `AIad` on `blba` |
| TFT HumanX06 | `UnitAddItem`: `AIau` (code `AUau`) on `lgdh` | same item, message now names the 4.0 s buff lifetime and the open range repeat |
