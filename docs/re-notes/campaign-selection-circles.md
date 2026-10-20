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

## Terrain-conforming projection

The circle image now conforms to the admitted rendered ground. Each intersected
tile uses the ground renderer's `[00,10,11]` and `[00,11,01]` triangles; each triangle
is clipped against the source-sized circle square, and the resulting polygon is
triangulated without crossing a ground-triangle boundary. Clip intersections
interpolate the original corner positions. No vertical lift or bilinear surface
approximation is added. The existing render depth bias resolves coplanarity.

UV coordinates still span the original square, with V increasing toward world
negative Y, matching the previous Plane3d texture orientation. Clipping at map
edges crops the texture rather than stretching it. The source-derived diameter,
texture-band choice and relationship tint are unchanged. Each circle owns its
mesh; position/size changes replace that mesh's contents, unchanged transforms
reuse it, deselection removes it, and session replacement retires all owned mesh
assets. The circle's transform retains the existing center height, while local
vertex elevations cancel that center to land on the actual rendered triangles.

The renderer refuses a footprint intersecting more than 256 terrain tiles before
allocating geometry. This is an explicit rendering-work bound, not a game rule or
an authored size. Two input triangles per tile, four clip planes, and at most five
fan triangles per clipped polygon bound output to 30 vertices per visited tile.
Synthetic tests check non-coplanar slopes across multiple tiles, vertices and
triangle centroids against the rendered surface, complete footprint area, source
UV orientation/extents, cropped map-edge UVs, malformed sizes, and deterministic
geometry at the tile-work cap. ECS cleanup covers both session clear and empty
selection, including actual mesh-asset retirement. These new checks await the
coordinated build and rendered validation; the v7 observation above predates this
projection change.

Unqualified recreation choices remain: interpreting scaled size as diameter,
adding `.blp` to the authored texture name, and render depth bias. Exact original
projection behavior and original radius-versus-diameter semantics remain explicit
fidelity work; no screenshot fit or guessed size multiplier was added. Projection
matches the currently rendered ground; walkable doodad surfaces and future cliff
geometry require their actual surface data before this can cover those cases.
The moving-footman capture is not a matched original-client or browser comparison.
No source bytes or generated original assets are included here. The target is
1.29b; runtime resolver receipts retain selected source provenance rather than
assuming all locally supplied archives share that version.
