# Campaign selection circles

The campaign renderer reads the selected source's `UI\MiscData.txt`
`[SelectionCircle]` size bands, texture names, ARGB relationship colors and
`ScaleFactor`. The executed unit's metadata-backed UnitUI `scale` supplies the
unit size. Ground height comes from the admitted terrain at the live unit
position; world `[x, y, z]` becomes Bevy `[x, z, -y]`. Selected hidden/dead units
are excluded. Materials are loaded through the active map's resolver and receipt
identity checks. A replaced or cleared script session discards its material
cache and despawns its circle entities, even if unit handle numbers recur.

A selected unit with missing/malformed scale, missing position, an out-of-terrain
position, missing local relationship, missing texture band or non-finite transform
now leaves a unit-specific `CampaignSelection.last_error` instead of silently
dropping the failed circle; other valid selected units can still render. Texture
read/decode failures retain the requested member path. Missing profile, mismatched
terrain session and unavailable receipt resolver also report explicit errors.
Debug events report selected/wanted/placed count changes, successful source texture
and alpha ranges, and spawned circle transforms with terrain footprint heights.
Diagnostics alone do not prove successful pixels.

The two native `campaign_selection::tests` cover size-band boundaries,
relationship tint choices, and actual retirement of a spawned circle entity and
cached source materials on session clear. The consolidated v7 native application
tests, strict native/browser Clippy and browser build passed (2026-10-20); retained
receipts are under the private `.scratch/campaign-integration-20261020-v7/` run.

## Rendered observation

In that v7 native run, a mouse click selected footman handle 779, and a subsequent
right click issued a move order to open ground. `moving-footman.png` visibly shows
the green selection arc around the moving footman near pixel `(610, 520)`. The
source log records a white circle texture with alpha range `[0, 255]`, friendly
ARGB tint `[255, 0, 255, 0]`, and a diameter derived from that unit's metadata.
This establishes actual submission and visible pixels for this local native run;
initial `selected=1 wanted=1 placed=1` logs alone did not establish it.

At the initial hero/footman locations, the flat circle intersects uneven terrain
and overlaps the actor/model footprint. Moving the same selected footman exposes
the ring without altering its material or authored size. These observations
support occlusion as the earlier visibility limitation, rather than a missing
texture or failed selection admission. They do not establish the relative
contribution of each occluder. The terrain sampler is bilinear whereas the
rendered terrain uses planar triangles, so sampled height may additionally differ
from the actual surface within a tile.

Unqualified recreation choices remain: interpreting scaled size as diameter,
adding `.blp` to the authored texture name, a flat alpha-blended plane at the unit
center's terrain height, and render depth bias. Terrain-conforming projection,
exact original slope behavior and original radius-versus-diameter semantics remain
explicit fidelity work; no screenshot fit or guessed size multiplier was added.
The moving-footman capture is not a matched original-client or browser comparison.
No source bytes or generated original assets are included here. The target is
1.29b; runtime resolver receipts retain selected source provenance rather than
assuming all locally supplied archives share that version.
