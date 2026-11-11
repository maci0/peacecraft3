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

## Open

### Arbitrary facing

Refused by `unit_placement::candidate` (`only quarter-turn footprint facing is
admitted`) for a CreateUnit whose pathing texture is set and whose facing is not a
multiple of 90. Affects RoC NightElf04, TFT UndeadX01, NightElfX07 and NightElfX08.

Decoded from `Warcraft III.exe` 1.29.2.9231 (SHA-256 checked before reading):

- CreateUnit native `0x48ee00` (name `0xe719e8`, registered at `0x49d20f`) calls
  `0x663ca0`, which calls the creation routine `0x687ca0` (observed).
- `0x687ca0` takes the unit's pathing texture object from `0x5c3150`, then takes
  the quarter from `Misc/BuildingAngle` through `0x46dea0`. That routine truncates
  the value to a multiple of 90 toward zero, adds 360 when negative, and the
  classifier compares it with the constants 0, 90 and 180 (observed). Those
  constants are set at startup by initializers `0x401300`, `0x401740` and `0x401430`
  (observed). The quarter is BuildingAngle / 90.
- The unit's facing is converted by `0x99ffe0` and written to the fourth
  component of the pose record at `var_1a8`, which is passed to the unit's virtual
  method at vtable slot `0x1ac` (observed). It does not select the footprint
  quarter in `0x687ca0`.
- The per-unit pathing rotation in the stationary-grid routine at `0x4e1130`
  (stamp loop `0x4e1500` to `0x4e16b2`) takes its quarter from BuildingAngle through
  the same classifier, then calls `0x4dffc0` with that quarter (observed). Its
  caller `0x603850` is a unit-position update path (inferred from its field
  accesses).

Inferred: a structure's footprint orientation follows `Misc/BuildingAngle`, and
the facing argument does not affect it.

Not decoded: the quarter passed to the final traversal `0x4dfb40` at `0x4e1725`
is the caller's `0x9c(%esi)` field (observed), and no writer of that field is
traced. A footprint orientation rule cannot be finalized until that is known.

Needed for implementation: (a) read `Misc/BuildingAngle` in production, (b) the
CreateUnit position snap of `0x687ca0` (below), (c) the source of `+0x9c`.

### Odd-dimension pathing texture

`PathTextures\24x24Unflyable.tga` (War3.mpq) has a TGA header of width 29, height 24,
24 bpp, uncompressed. `campaign_placement::decode_kind` refuses odd width or height
with `footprint cell budget or unsupported empty/odd dimensions`.

Decoded for the CreateUnit path:

- Snap `0x4e00f0` (called from `0x687ca0` when a pathing texture exists, with the
  BuildingAngle quarter). Quarter 0 and 2: the x axis uses texture height and the y
  axis uses width. Quarter 1 and 3: the swap applies (observed, `0x4e0109` to
  `0x4e0128`). Per axis: clamp, truncate to a multiple of 64 toward zero, add 32
  when the axis dimension has bit 1 set, add 16 when it is odd. Both offsets take
  sign -1 when the coordinate is above 0 and +1 otherwise (observed,
  `0x4e01c1` to `0x4e0272`).
- Clamp `0x7f4a60`: x in `[o+0x70, o+0x78 - 32]`, y in `[o+0x74, o+0x7c - 32]`, where
  `o = [0x115e9ec]` and the margin 32 is set by `0x4015b0` (observed). The writer of
  `o` (`0x7f557c`) is not traced. Equivalence to the recreation's world bounds is
  unverified.
- Stationary traversal `0x4dfb40`: texel offsets are scaled by `(w - 1) * 16`, with
  the constant 16 at `0x113bde4` set by `0x403b40` (observed). For odd `w` that is a
  whole number of 32-unit cells, `(w - 1) / 2` texels.

Inferred: the odd-width footprint is centred at a half cell, so texel `t` maps to
cell offset `t - (w - 1) / 2`, which is `t - w / 2` in integer division. The float
to cell rounding after the traversal is not decoded.

Needed: the clamp object writer, the final quarter source (above), and the cell
rounding. Until then odd dimensions stay refused.

Affects HumanX04Interlude and RoC NightElf07.

### Green texel outside the map

`campaign_placement::project_native_texture` refuses when a texel with the green
flag projects outside the map. `unit_placement::candidate` refuses with
`native footprint outside map` for any nonzero texel outside the grid. The two
paths disagree: the campaign path silently drops non-green texels outside the map.

Decoded: the texture constructor `0x4deb67` marks a texture green when any source
pixel has nonzero green, and the registration selector `0x6c0943` picks three or
four entry masks from that mark (earlier notes). Not decoded: how the entry linker
`0x804cba` treats a texel outside the grid, so neither refusal policy is
established. The behaviour is unchanged in this pass. Reconciling the two paths
needs the linker decode.

Affects HumanX06 and NightElfX05.

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

## Probe results (2026-11-11, placement decode)

No recreation code changed in this pass, so before and after are the same. Each
row is the first refusal of `selected_install_locked_chapter_startup_probe`.

| Chapter | Before | After |
|---|---|---|
| RoC NightElf04 | `CreateUnit`: only quarter-turn footprint facing is admitted | unchanged |
| TFT UndeadX01 | `CreateUnit`: only quarter-turn footprint facing is admitted | unchanged |
| TFT NightElfX07 | `CreateUnit`: only quarter-turn footprint facing is admitted | unchanged |
| TFT NightElfX08 | `CreateUnit`: only quarter-turn footprint facing is admitted | unchanged |
| TFT HumanX04Interlude | `CreateUnit`: `PathTextures\24x24Unflyable.tga` odd dimensions | unchanged |
| RoC NightElf07 | `CreateUnit`: `PathTextures\24x24Unflyable.tga` odd dimensions | unchanged |
| TFT NightElfX05 | `CreateUnit`: native texture green channel outside map | unchanged |
| TFT HumanX06 | `CreateUnit`: native texture green channel outside map | unchanged |

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
