# Mission data loading audit

Mission content belongs to the selected installation and map, not a source-code
campaign table. Synthetic fixture names and explicit local-asset test inputs are
not production fallbacks.

| Input | Production consumer |
|---|---|
| Edition campaign index | `native_campaign::read_campaigns` selects the edition's index member; `parse_campaign_index` reads CampaignList order, sections, flags, labels, backgrounds and mission rows. |
| Selected mission map path | `native_campaign::configuration` resolves the selected row's map through the pinned CampaignRecipe; it checks that selecting the map data namespace does not change those map bytes. |
| Non-map indexed target | Target type comes from the authored file format. A model-cinematic entry stays in the source list and reports unimplemented standalone playback instead of being coerced into a map archive. |
| Map data namespace | CampaignRecipe reads `war3map.w3i` from those exact map bytes and derives the namespace through the selected client rules. |
| Terrain, pathing, placement and map information | Shared map admission parses the map's own W3E/WPM/DOO/W3I members and validates their bounds/alignment. |
| Object values and map overrides | Shared catalog admission resolves the selected source tables/profiles plus the map's object overrides; production mechanics consume resolved definitions. |
| Script behavior | The selected common/Blizzard preludes and map script or trigger-graph path create the actual startup world. There is no production Human01/Human02 setup switch. |
| Next chapter | Executed map `SetNextLevel`/exit requests supply Continue's path. Continue does not increment a hard-coded chapter list. |
| Fresh difficulty without a profile | The selected client supplies the fallback ordinal; the loaded script's ConvertGameDifficulty declaration maps it to its authored constant. The campaign difficulty picker and persisted profile choice remain unfinished. |

The source paths naming file formats and engine entry points are lookup
contracts. Individual map filenames, chapter counts, content strings, authored
rules and measured asset values must not be embedded as substitute data.
Missing or malformed inputs report their source instead of selecting a known
stock mission as a fallback.

This is a loading audit, not a campaign-completion claim. Profile unlocks and
progression persistence remain unfinished. Extra campaign mission-row fields
retained as uninterpreted by the parser have not been qualified as unlock rules;
do not infer their meaning from their position. The current gameplay frontier
and mission-completion gaps remain in TRACKER.md and the core mechanics plan.

## Checks

The synthetic campaign-index test changes campaign order, availability, titles
and map filenames and checks that the resulting mission rows change with them.
The difficulty test selects a nonstock ordinal and symbol while a differently
numbered Normal symbol is also present; a missing ordinal refuses.

The explicit local-asset gate
`native_campaign::tests::selected_install_all_indexed_entries_resolve_their_authored_target_types`
requires `PCX_NATIVE_CHAPTER_INSTALL`. For each supported edition it reads the
real index and checks every authored target. Map entries resolve their source
archive, parse their own W3I and validate data-namespace selection. Model entries
resolve through the shared MDL-to-binary-member rule and parse their MDX container.
It derives this inventory from the input; no expected chapter count, stock map
list or named cinematic exception is embedded. This checks data loading, not
every map's script completion, standalone cinematic playback or rendering.

The first all-map candidate passed the RoC rows but failed on a TFT model
cinematic. That failure exposed an incorrect assumption that every mission row
names a map. The revised check covers both formats rather than omitting the
non-map row. Its failed receipt is preserved separately.

The revised selected-install run passed with 42 RoC map headers, 37 TFT map
headers and one model cinematic. These are observed input inventories, not
production constants or test expectations. The integrated source passes 1,758
core tests, 711 application tests, strict native/WASM checks, the desktop build and the existing
Human02 opening-minute regression. The latter begins through the existing
victory stimulus and does not prove a first-mission playthrough. Source freezes
and checks remain local under `.scratch/core-mechanics/missions-r2-final/`.
