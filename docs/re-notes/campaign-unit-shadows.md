# Campaign unit and building shadows

Stock draws a shadow under units and buildings that `war3map.shd` does not
contain. The clone drew none. `pcx_play::unit_shadow` reads the UnitUI shadow
fields and `campaign_unit_shadows.rs` draws them as terrain-clipped decals.
This is a recreation of the observed look, not decoded native behaviour; every
placement rule below is labelled with its evidence and its open question.

## Source data (shipped bytes)

Checked against the selected 1.29 `Units\UnitUI.slk` (837 rows) and
`UnitMetaData.slk`:

| UnitUI column | Type (UnitMetaData) | Values |
| --- | --- | --- |
| `unitShadow` | `shadowImage` | `Shadow` (550 rows), `ShadowFlyer` (62), `shadow` (11), `_` (213) |
| `buildingShadow` | `shadowTexture` | `_` (628), else 208 rows over 59 names such as `ShadowGreatHall`, `BuildingShadowLarge` |
| `shadowW`, `shadowH` | `real` | 35 to 500 on every one of the 623 rows naming a `unitShadow`; 5 other rows author them without one |
| `shadowX`, `shadowY` | `real` | 17 to 200; `shadowX != shadowY` on 6 rows, `shadowW != shadowH` on 6 rows |
| `shadowOnWater` | `bool` | `1` on every row, not used (no water is drawn yet) |

- Every named image resolves to `ReplaceableTextures\Shadows\<name>.blp`; all
  names in UnitUI have a member there (names are case-insensitive).
- 13 rows name both images (Tree of Life, Ancient Protector and Ancient of War
  families); both are drawn.
- `Shadow.blp` is 64x64 BLP1 alpha-only (RGB 0, alpha 0 to 191, edge alpha 0).
  `ShadowFlyer.blp` is 64x64, symmetric, alpha 0 to 179, edge alpha at most 3; `Shadow.blp` is skewed along the lower-left to upper-right diagonal.
- Building masks are 4 to 20 texels square (`ShadowGreatHall` and
  `BuildingShadowLarge` 13, `ShadowHouse` and `BuildingShadowSmall` 9, `ShadowCastle`
  18), alpha only 0 or 255, hard edged, like the `0x00`/`0xFF` cells of
  `war3map.shd`. Several masks have 255 on their edge texels, and some are
  even-sized.
- `UI\MiscData.txt` `FogOfWar/ShadowImageColor=170,0,0,0` (ARGB).

## Not in the baked mask

Prologue02 `war3map.shd` is zero in a 21x21 cell window around the preplaced
Great Hall (128, 1280), Altar of Storms (288, 736) and Gold Mine (768, 1728).
Across the 53 Frozen Throne campaign maps in `War3x.mpq`, stamping each preplaced
building's mask at its position hits non-zero `war3map.shd` cells for 5.6% of mask
cells (5261 of 94762), below the whole-file shadow share, so buildings are not
baked. Drawing the dynamic building shadow therefore does not double it. This
used a byte scan of `war3mapUnits.doo` for the building raw codes with a
plausibility filter on the following floats, not the full object parser. The
check covered those maps; ROC campaign maps were checked only for Prologue02.

## What stock draws (runtime observation)

Reference: `.scratch/shadow-fix-20261027/projection-original/captures/matched-skip-control.png`
against the clone capture `.scratch/visual-match-20261027/captures-11/selected-unit.png`
(1280x960, same camera). The camera is the one in that capture's `render-state.json`
(translation 572.0, 1922.5, -429.6, pitch -56 degrees, vertical field of view 0.599
rad over a 1280x720 viewport at y offset 32); projecting the doodad positions with it
reproduces the visible positions of the Great Hall, Gold Mine and Orc Burrow.

Ground brightness agrees between the captures (ratio of mean luminance 1.00 on open
ground), so the per-pixel ratio stock/clone isolates what stock adds:

- A dark soft blob under the walking peon: 60 to 70 px wide and about 50 px
  high on screen, brightness ratio 0.45 to 0.8, elongated from lower left to
  upper right.
- Dark regions east and north-east of the Great Hall, the Gold Mine and the Orc
  Burrow, outside their silhouettes.
- No clear signal under Thrall (the red ubersplat covers it).

## Recreation choices

| Choice | Evidence | Open question |
| --- | --- | --- |
| Texture path `ReplaceableTextures\Shadows\<name>.blp` | Only archive member matching every UnitUI name | none |
| `shadowW` and `shadowH` are the image's full world width and height | Peon `Shadow`, W=120: the opaque part is about 0.8 W; at 0.72 px per world unit that is 69 px, matching the 60 to 70 px blob | none for size |
| `shadowX`, `shadowY` give the unit origin's distance from the image's lower-left corner, so the image centre is at `(W/2 - X, H/2 - Y)` from the unit | Reads the field names literally; the blob centre lies on the north-east side of the peon's feet, the direction of its positive offset | The stock peon world position in the capture is unknown, so the 10 unit offset of the peon (and the sign) is not measured. Thrall (W=160, X=80) and most rows have zero offset. A capture of a stationary unit with `shadowX != W/2` settles it |
| Unit images: row 0 is the north edge | Correlating the blob against the predicted `Shadow.blp` placement over 240 x 240 unit of translations gives 0.74 for row 0 north and 0.52 for row 0 south | one capture, one unit |
| Building masks: one texel is one `war3map.shd` cell (`TILE_XY / SHADOW_CELLS_PER_TILE` = 32), mask centred on the building origin, row 0 is the south edge | The masks are 0/255 like the shadow map; the predicted mask contour matches the east edge of the dark region at the Great Hall (within a few pixels) and the north-east lobe of the Gold Mine, and row 0 north would put the lobe south-east of the origin, where stock shows no darkening | Odd masks (13) centred on a cell corner could be snapped half a cell (16 units) either way; the whole-frame correlation scan cannot separate this because the building covers the shadow |
| Colour is `ShadowImageColor` rgb and its alpha multiplies the image alpha (peak 191/255 * 170/255 = 0.50) | Stock darkening at the blob edge is ratio 0.74 to 0.82 and 0.45 to 0.65 inside | The blob centre is hidden by the unit; flyers (`ShadowFlyer`, peak alpha 179/255) are not measured |
| Decals blend over each other | Not measured | The shadow map takes the maximum of overlapping cells; stock may do the same for unit images |
| Drawn at the unit's ground position, no flyer height offset | Not measured | Flyer shadow placement |

Not drawn: shadows of hidden or dead units, fog interaction, `shadowOnWater`,
the video option that turns unit shadows off, and any camera or time of day
dependence of the shadow direction. The images are fixed in world orientation.

## Implementation

- `crates/pcx-play/src/unit_shadow.rs`: reads `unitShadow`, `buildingShadow`
  and the four `real` columns through the selected metadata (map edits
  included), resolves texture paths, and computes the world footprint and
  texture coordinates. Missing, non-positive or non-finite placement for a named
  `unitShadow` is an error naming the column.
- `crates/peacecraft3/src/campaign_unit_shadows.rs`: per executed unit, caches
  the parsed images by `definition_revision`, loads each image once with its
  RGB replaced by white (the shadow colour comes from `base_color`), projects the
  footprint's enclosing square with `campaign_selection::projection::project`
  and rewrites the UVs. Outside the rectangle of a non-square `unitShadow` the
  square's texture coordinates leave 0..1; the sampler clamps to the edge texels,
  whose alpha is at most 3/255. A session change clears every entity and cache.
  Errors go to `CampaignUnitShadows.last_error`.
- The decal depth bias (16) places shadows under ubersplats (32) and selection
  circles (64).

## Tests

Synthetic tests with non-stock values cover field parsing (cells, map overrides,
empty markers, malformed values, wrong metadata), the unit and building
footprints, and the rewritten UVs of a projected mesh over a synthetic terrain.
They do not establish rendered parity.

## Evidence files

Local only, under `.scratch/shadows/`: extracted shadow images, per-map scan
scripts and output, ratio and contour images, and the correlation scans
(`ratio.png`, `ov-hall.png`, `ov-mine.png`, `peonscan.py`, `allmaps.py`).
