# Renderer screenshots

## 0.19.0 gallery

The five images were captured on 18 November 2026 from the PeaceCraft III
renderer at 1280×960, using locally supplied Legacy 1.29.2.9231 assets in Reign
of Chaos mode. The test ran with offscreen Vulkan software rendering and null
audio.

- `main-menu.png`: Reign of Chaos main menu.
- `chapter-selection.png`: campaign and chapter selection; difficulty selection is unavailable.
- `campaign.png`: selected Thrall in Prologue02, with the unit panel and command card.
- `research-menu.png`: source-derived hero skill menu and readiness states.
- `quest-log.png`: populated quest dialog with Done hovered and a notification behind it.

The capture test passed with `test-fixtures`, verified 13 held-clock receipts,
and produced byte-identical repeats of the held selected-unit frame. All five
images were visually reviewed. See [capture provenance](provenance.json) for
the source revision and PNG hashes.

The gallery has no original-client comparison. Earlier comparisons observed
gaps in quest borders, row status, scrollbars, world and minimap rendering,
font metrics, and cursor edges. Those gaps were outside the 0.19.0 capture
check. See [HUD fidelity notes](../re-notes/gameplay-hud-fidelity.md).

## Historical comparisons

Results follow capture order. The sight-lattice entry ends the first native
reference series. The live-sight and JPEG entry starts the second. Full-screen
counts cover 1,228,800 RGB pixels without masking, alignment, resizing or
tolerance. Simulation tick, animation phase and RNG identity were outside
scope. Synthetic GPU checks covered the cursor overlay and numeric HUD
working channel. The 0.19.0 gallery has separate receipts.

From the inventory-heading checkpoint onward, each two-capture series held
26 clock receipts and matched the five published states. Individual notes
identify the byte-for-byte comparisons.

### First native reference

Resource labels use executable-derived font-size limits and pair exceptions, source font
metrics/kerning and Bevy atlas bitmap widths. Upkeep resolves to an 84-pixel layout with
integer pair spacing. The observed upkeep placement offset is resolved using source
coordinates and backend conversion; no screenshot-derived offset was applied.

Resource labels also use the selected executable's alpha reference in the Bevy text
shader. GPU tests cover combined text/texture alpha, surviving coverage and component
removal. The default resource font also uses executable-derived point sampling. All 528
upkeep glyph ink pixels match at their actual screen coordinates without registration.
The entire surrounding fixed diagnostic box matches in all 2,640 pixels. This scoped
result does not replace the full-screen comparison.

Upper-button images and hover layers preserve authored fractional frame bounds and
convert native pixel-center coordinates. The observed source sampler disables mip
filtering, so the atlas keeps its base-only path. The shared image conversion also
preserves viewport-derived console and HUD-plan backdrop geometry, including supplied
root transforms. Other widget/material paths and upper text labels still needed
correction at this checkpoint. See [upper-button image
evidence](../re-notes/campaign-hud-top-bar.md#upper-button-image-quads-2026-11-03).

Minimap filter-button images retain constructor-derived fractional bounds through the
shared source-image renderer. Their image filters follow native file-loader flag
operands and sampler-table values. State selection, other image-loader samplers and
low-byte residuals remained open.

Command-cache images share source-derived min/mag/mip filtering. Command art and active
highlights also preserve source-derived fractional bounds. This removed 24,889
full-screen differences from the sampler-only result, leaving 992,062.
Rank/text, hit geometry and low-byte image residuals remained open.

Unit-panel icons and experience images use source-derived filtering and fractional
bounds. The hero-icon key list is read from the admitted executable; all three bindings
preserve the prior artwork. This correction removed another 3,819 full-screen
differences, leaving 988,243.

Terrain uses its selected day/night light and source-derived corner normals. Exact
full-frame differences fell by 36 to 988,207, and mean channel error fell from 12.803222 to
12.529152. Native terrain normal/packing and light/shadow pass behavior remained
unqualified.

Terrain sheets preserve supplied BLP mip pixels instead of regenerating them. The
selected-map upload has zero channel differences from the supplied seven cell mip levels
(previously 777,076). The full-frame difference count fell by four to 988,203 and mean
channel error fell to 12.526580. Native terrain LOD/filtering and far-LOD atlas
behavior remained unqualified.

Minimap projection derives its rectangle from executed camera bounds and the native
profile margin references. Fog, markers, outline and pointer mapping share it. The
full-screen difference count fell by 1,456 to 986,747, and mean channel error fell to
12.392503. Native fog-grid sampling, occlusion and raster details remained open.

Minimap dynamic-texture sampling follows the native selector and tables, and the image
retains its fractional source quad. Whole-frame differences fell by 1,843 to 984,904;
mean channel error fell to 12.360366. Repeated frames matched. Fog sampling,
occlusion, special icons and outline rasterization were either different or
unqualified.

Unit-panel labels share the source-derived default font raster size, atlas filtering and
alpha cutoff with resource labels. Whole-frame differences fell by 2,422 to 982,482, and
mean channel error fell to 12.249674. Panel-specific pair spacing, baselines and
placement still needed correction at this checkpoint. No FreeType dependency was added.

Default unit-panel strings share source-derived integer pair advances, font baselines
and pixel-origin rounding with resource labels. Constructor justification defaults,
portrait overrides and automatic stat/status widths preserve source relationships. The
exact whole-frame difference count fell by 1,830 to 980,652 and mean channel error to
12.104810. All 13 clocks held and repeat bytes matched. Exact glyph/shadow parity and
native animation/RNG identity remained open.

Panel text preserves automatic frame widths and source midpoint/edge precision before Gx
justification. This removed another 1,764 full-screen differences, leaving 978,888 and
mean channel error 11.988108. The class-line diagnostic box is exact. Repeated
lowercase-g coverage pixels, portrait backgrounds and other UI/world/text residuals
were counted at this checkpoint. Read-only Wine observations covered eight
selected-panel string inputs. Original tick identity remained unqualified.

Bitmap-space implied curve midpoints in the existing Rust font path reproduce the native
14-pixel lowercase-g bitmap exactly. This removed all 28 repeated coverage differences
from the full frame, leaving 978,860 differences and mean channel error 11.988054.
Thirteen panel text diagnostic rectangles matched exactly. Portrait
backgrounds, other glyph/UI/world differences and native tick identity
remained open.

Inventory heading retains its source fixed frame and constructor justification through
the shared default-font and region-placement path. Only its region changed from the
previous recreation (1,189 pixels); full-frame differences fell by 160 to 978,700, mean
channel error to 11.961337. Selected repeats matched. Upkeep and
the 13 panel text regions remained exact. Heading raster and background
differences remained. See [inventory
evidence](../re-notes/held-inventory-presentation.md#inventory-heading-source-font-and-region-placement-2026-11-03).

Plain source text roots receive integer raster sizing even without child spans.
Inventory reaches the admitted 18-pixel font; all 92 opaque heading pixels match at
native screen coordinates. Whole-frame differences fell by 293 to 978,407 and mean
channel error to 11.939232. Only 811 pixels in the heading region changed. Blended edges,
backing images and the wider fidelity gaps remained. The earlier standalone glyph-cache
match did not prove production sizing; the capture regression check includes plain text
roots.

Upper-button labels use source-derived raster sizing, alpha cutoff and atlas sampling.
Full-frame differing pixels fell by 200 to 978,207, while mean channel error rose
slightly to 11.940990. Only 1,804 upper-label pixels
changed. Native upper-label pair spacing, baseline and frame placement remained open at this
checkpoint; this raster change does not establish complete text parity. See [upper label
evidence](../re-notes/campaign-hud-top-bar.md#upper-label-raster-policy-2026-11-03).

Upper labels use source integer advances, baselines and frame origins with no wrapping.
Twelve resulting string widths agree with read-only native observations. Full-frame
differences fell by 2,024 to 976,183 and mean channel error to 11.755233. Only 2,381
upper-label pixels changed. The upper strip mean absolute channel error fell from
11.648788 to 1.273424; blended/backing-image residuals were still counted. Source pressed
offsets and explicit FDF justification remained intact. Native Windows, full-screen and
animation/RNG parity remained unqualified. See [upper string
evidence](../re-notes/campaign-hud-top-bar.md#upper-string-widths-baselines-and-frame-origins-2026-11-03).

The minimap uses source resource art and mask/tint blits, solid CPU markers,
native grid/float-bit projection, a fixed-point fog scan and a separate textured
hero pass. The retained native-grid test reproduces all 65,536 CPU alpha bytes;
quad size, four projection matrix values and marker scales match the native
records bit-for-bit. Full-frame differing pixels fell from 976,183 to 976,058,
and mean channel error from 11.755233 to 11.714645. Upkeep
and 13 panel text regions remained exact. Fog-grid production/blockers, special
neutral/camp overlays, remembered visibility, shadows and full-screen/tick parity
remained open. See [minimap evidence](../re-notes/campaign-minimap.md#hero-image-pass-and-remaining-sight-trace-2026-11-03).

Blocked sight no longer erases another source's clear visibility or exposes a
fresh unexplored cell as fogged. The overlap regression fails before the fix;
all 24 vision checks, strict native/WASM checks and both builds pass. Only 387
minimap pixels changed. Full-screen differences fell by 61 to 975,997, with mean
channel error 11.714487. Native span/graph reconstruction progressed separately;
the corrected plane labels supersede the earlier 26-cell residual. See
[blocked-source evidence](../re-notes/campaign-minimap.md#blocked-source-preservation-and-sight-graphs-2026-11-04).

The campaign ledger uses the native sight lattice and predecessor graph,
asset-derived occluder heights, and separate flying/modifier spans. Production
heights match all 12,288 retained native cells; the explicit Rust gate matches
209 span rows and propagation through eleven native graphs. Only 2,511 minimap pixels
changed; full-screen differences fell by 1,138 to 974,859, with mean channel error
11.652511. Upkeep and 13 panel text regions remained exact. Remembered visibility,
special overlays, outline rasterization and native tick identity remained open.
See [sight integration](../re-notes/campaign-minimap.md#native-sight-lattice-integration-2026-11-04).

### Second native reference

A later capture series included live day/night sight endpoints and a corrected
13-bit JPEG transform in the existing Rust decoder. The minimap base matches all
65,536 native RGBA pixels; current visibility matches all 1,209 native cells and
heights match all 12,288 cells. Against a newly acquired native reference, the
fresh baseline had 974,765 differing full-screen pixels and these changes reduced
that to 972,999 (mean absolute channel error 11.609237). Explored history had
23 missing cells. Marker memory, outline rasterization and cross-client clock
identity remained open. The base-image result does not establish final
minimap parity. See [live-sight and JPEG evidence](../re-notes/campaign-minimap.md#live-sight-endpoints-and-jpeg-precision-2026-11-04).

Menu panel models retain their authored BLP mip chains through GPU upload,
using the same bounded loader as other model textures. The original-panel byte
regression failed before the fix and passed after it. The change affected
13,909 main-menu pixels. Chapter and gameplay frames retained their previous
hashes, so the gameplay difference count remained 972,999.
Native LOD and filter parity remained unqualified.
See [panel mip evidence](../re-notes/mdx-texture-selection.md#panel-mip-retention-2026-11-04).

A later minimap outline pass uses the finite source frustum after camera projection
refresh and aliased pixel spans instead of a UI border. All 97 outline pixels
match the retained native frame. Only 141 recreation pixels changed; full-screen
differences fell by 116 to 972,883, with mean channel error 11.583758.
Fog grids and CPU minimap bytes were unchanged.
Other camera states/resolutions, explored history and native tick identity
remained open. See [outline evidence](../re-notes/campaign-minimap.md#finite-outline-frustum-and-aliased-line-coverage-2026-11-04).
