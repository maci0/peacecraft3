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
