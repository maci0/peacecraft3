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

## Open

### Odd-dimension pathing texture

`PathTextures\24x24Unflyable.tga` (War3.mpq) despite its name has a TGA header of
width 29, height 24, 24 bpp, uncompressed. `campaign_placement::decode_kind`
refuses odd width or height with `footprint cell budget or unsupported empty/odd dimensions`.
The refusal is deliberate: `ai-construction.md` (odd texture dimensions "still
refuse rather than gaining invented behavior") records it.

The native construction transform `0x4e00f0` adds 16 toward the sign for an odd
dimension when it places a building (`ai-construction.md`). Nothing decoded
shows how the CreateUnit stationary footprint (texel traversal `0x4dfb40`)
centres an odd width. Centring with `width / 2` would invent that rule.

Needed: decode the centre arithmetic on the stationary footprint path, or
observe the original's collision cells for an odd-width texture.

Affects HumanX04Interlude and RoC NightElf07.

### Green texel outside the map

`campaign_placement::project_native_texture` refuses when a texel with the green
flag projects outside the map. Native evidence in `ai-construction.md`: the
texture constructor `0x4deb67` marks a texture green when any source pixel has
nonzero green (texture level), and the registration selector `0x6c0943` picks
three or four entry masks from that mark. Not decoded: how the entry linker
`0x804cba` treats a texel outside the grid. The notes record this refusal as
deliberate because clipping "would hide the selector input".

Needed: decode the linker's out-of-range behaviour, and confirm the selector
reads the texture-level mark rather than linked cells. Only then can clipping
keep the entry mask.

Affects HumanX06 and NightElfX05.

## Probe results

| Chapter | Before | After |
|---|---|---|
| TFT NightElfX08Finale | CreateUnit: MalFurion.mdx `invalid animation interval/name` | pass CreateUnit; `SetCampaignAvailable` unsupported by this host |
| TFT NightElfX06 | CreateUnit: MalFurion.mdx; then `IssueImmediateOrder` `frostarmoroff` | pass CreateUnit and `frostarmoroff` (autocast OFF, `CAbilityFrostArmorAuto`); `GroupAddUnit` refused: arguments do not match the group native declaration (`Scripts\Blizzard.j:4322`) |
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
