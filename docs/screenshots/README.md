# Renderer screenshots

Fresh 0.13.0 captures from PeaceCraft III on 4 November 2026, using the locally supplied Legacy 1.29.2.9231 install in Reign of Chaos mode. The production renderer ran headless at 1280×960 with offscreen Vulkan software rendering and audio disabled. These are rendered demonstrations, not original-client captures or pixel-parity evidence.

- `main-menu.png`: Reign of Chaos main menu.
- `chapter-selection.png`: campaign and chapter selection; difficulty selection is unavailable.
- `campaign.png`: selected Thrall in Prologue02, with the unit panel and command card.
- `research-menu.png`: source-derived hero skill menu and readiness states.
- `quest-log.png`: populated quest dialog with the Done button hovered.

The dedicated `mounted_campaign_hud_and_research_states` capture test passes, including selection, skill-menu states, populated quest dialog interaction and upper-button states. All five images were visually inspected. Screenshot acquisition holds real, virtual, fixed and map clocks during GPU warm-up/readback and restores the update strategy afterward. Two selected-unit captures at the same held checkpoint match byte for byte. The cursor overlay and numeric HUD working-channel path are covered by synthetic GPU checks.

The original-client reference has no observed simulation tick; it must not be called tick-identical. An exact full-screen RGB comparison, without masks, registration, tolerance or resizing, finds 976,183 differing pixels out of 1,228,800. The two same-tick recreation captures have zero differences. Cross-client initial state, tick, animation and RNG identity still need qualification.

Quest border appearance, row/status binding, scrollbar presentation, world/minimap rendering, font metrics and cursor edge parity still have fidelity gaps. The quest notification is visible behind the dialog. Resource labels preserve authored FDF justification through source-derived pixel origins, baselines and raster-coordinate conversion. Other text roots still need metrics and rendering qualification. Screenshot measurements are diagnostic evidence, not layout inputs. See [gameplay HUD fidelity](../re-notes/gameplay-hud-fidelity.md).

Image hashes and capture provenance are retained in `provenance.json`. Original archives, profiles, reference captures, probe outputs and other generated game contents remain local.

The current images use the existing Bevy text renderer with unhinted UI text. No FreeType dependency is included. The full-frame mean absolute channel error is 11.755233; unmatched original animation phases prevent treating this as a synchronized parity gate.

Resource labels now use executable-derived font-size limits and pair exceptions, source font metrics/kerning and Bevy atlas bitmap widths. Upkeep resolves to an 84-pixel layout with integer pair spacing. The observed upkeep placement offset is resolved using source coordinates and backend conversion; no screenshot-derived offset was applied.

Resource labels also use the selected executable's alpha reference in the Bevy text shader. GPU tests cover combined text/texture alpha, surviving coverage and component removal. The default resource font also uses executable-derived point sampling. All 528 upkeep glyph ink pixels now match at their actual screen coordinates without registration. The entire surrounding fixed diagnostic box now matches in all 2,640 pixels. This scoped result does not replace the full-screen comparison.

Upper-button images and hover layers now preserve authored fractional frame bounds and convert native pixel-center coordinates. The observed source sampler disables mip filtering, so the atlas keeps its base-only path. The shared image conversion now also preserves viewport-derived console and HUD-plan backdrop geometry, including supplied root transforms. Other widget/material paths and upper text labels still need correction. See [upper-button image evidence](../re-notes/campaign-hud-top-bar.md#upper-button-image-quads-2026-11-03).

Minimap filter-button images now retain constructor-derived fractional bounds through the shared source-image renderer. Their image filters now follow native file-loader flag operands and sampler-table values. State selection, other image-loader samplers and low-byte residuals remain open.

Command-cache images now share source-derived min/mag/mip filtering. Command art and active highlights now also preserve source-derived fractional bounds. This removes 24,889 full-screen differences from the sampler-only result. Rank/text and hit geometry plus low-byte image residuals remain open.

Unit-panel icons and experience images now use source-derived filtering and fractional bounds. The hero-icon key list is read from the admitted executable; all three bindings preserve the prior artwork. This correction removes another 3,819 full-screen differences.

Terrain now uses its selected day/night light and source-derived corner normals. Exact full-frame differences fall by 36, and mean channel error falls from 12.803222 to 12.529152. Native terrain normal/packing and light/shadow pass behavior remain unqualified.

Terrain sheets now preserve supplied BLP mip pixels instead of regenerating them. The selected-map upload has zero channel differences from the supplied seven cell mip levels (previously 777,076). The full-frame difference count falls by four to 988,203 and mean channel error falls to 12.526580. Native terrain LOD/filtering and far-LOD atlas behavior remain unqualified.

Minimap projection now derives its rectangle from executed camera bounds and the native profile margin references. Fog, markers, outline and pointer mapping share it. The full-screen difference count falls by 1,456 to 986,747, and mean channel error falls to 12.392503. Native fog-grid sampling, occlusion and raster details remain open.

Minimap dynamic-texture sampling now follows the native selector and tables, and the image retains its fractional source quad. Whole-frame differences fall by 1,843 to 984,904; mean channel error falls to 12.360366. Repeated frames remain identical. Fog sampling, occlusion, special icons and outline rasterization still differ or remain unqualified.

Unit-panel labels now share the source-derived default font raster size, atlas filtering and alpha cutoff with resource labels. Whole-frame differences fall by 2,422 to 982,482, and mean channel error falls to 12.249674. Panel-specific pair spacing, baselines and placement remain open. No FreeType dependency was added.

Default unit-panel strings now share source-derived integer pair advances, font baselines and pixel-origin rounding with resource labels. Constructor justification defaults, portrait overrides and automatic stat/status widths preserve source relationships. The exact whole-frame difference count falls by 1,830 to 980,652 and mean channel error to 12.104810. All 13 clocks hold and repeat bytes match. Exact glyph/shadow parity and native animation/RNG identity remain open.

Panel text now preserves automatic frame widths and source midpoint/edge precision before Gx justification. This removes another 1,764 full-screen differences, leaving 978,888 and mean channel error 11.988108. The class-line diagnostic box is exact. Repeated lowercase-g coverage pixels, portrait backgrounds and other UI/world/text residuals remain counted. Eight selected-panel string inputs have read-only Wine observations; original tick identity remains unqualified.

Bitmap-space implied curve midpoints in the existing Rust font path now reproduce the native 14-pixel lowercase-g bitmap exactly. This removes all 28 repeated coverage differences from the full frame, leaving 978,860 differences and mean channel error 11.988054. Thirteen panel text diagnostic rectangles are now exact; portrait backgrounds, other glyph/UI/world differences and native tick identity remain open. No FreeType dependency was added.

Inventory heading now retains its source fixed frame and constructor justification through the shared default-font and region-placement path. Only its region changes from the previous recreation (1,189 pixels); full-frame differences fall by 160 to 978,700, mean channel error to 11.961337. All 26 receipts from two fresh captures hold, selected repeats match, and five published states independently match byte-for-byte. Upkeep and the 13 exact panel text regions remain exact. Heading raster/background differences remain; no screenshot-fitted offset or FreeType dependency was added. See [inventory evidence](../re-notes/held-inventory-presentation.md#inventory-heading-source-font-and-region-placement-2026-11-03).

Plain source text roots now receive integer raster sizing even without child spans. Inventory reaches the admitted 18-pixel font; all 92 opaque heading pixels match at native screen coordinates. Whole-frame differences fall by 293 to 978,407 and mean channel error to 11.939232. Only 811 pixels in the heading region change. Two fresh runs hold all 26 clocks and independently match all five published states. Blended edges, backing images and the wider fidelity gaps remain. The earlier standalone glyph-cache match did not prove production sizing; the capture regression check now includes plain text roots.

Upper-button labels now use source-derived raster sizing, alpha cutoff and atlas sampling. Full-frame differing pixels fall by 200 to 978,207, while mean channel error rises slightly to 11.940990; both metrics remain reported. Only 1,804 upper-label pixels change. Two fresh runs hold 26 clocks and independently match five published states. Native upper-label pair spacing, baseline and frame placement remain open; this raster change does not establish complete text parity. See [upper label evidence](../re-notes/campaign-hud-top-bar.md#upper-label-raster-policy-2026-11-03).

Upper labels now use source integer advances, baselines and frame origins with no wrapping. Twelve resulting string widths agree with read-only native observations. Full-frame differences fall by 2,024 to 976,183 and mean channel error to 11.755233. Only 2,381 upper-label pixels change. The upper strip MAE falls from 11.648788 to 1.273424; blended/backing-image residuals remain counted. Source pressed offsets and explicit FDF justification remain preserved. Two final captures hold all 26 clocks and independently match five published states. Native Windows, full-screen and animation/RNG parity remain unqualified. See [upper string evidence](../re-notes/campaign-hud-top-bar.md#upper-string-widths-baselines-and-frame-origins-2026-11-03).

The minimap now uses source resource art and mask/tint blits, solid CPU markers,
native grid/float-bit projection, a fixed-point fog scan and a separate textured
hero pass. The retained native-grid test reproduces all 65,536 CPU alpha bytes;
quad size, four projection matrix values and marker scales match the native
records bit-for-bit. Full-frame differing pixels fall from 976,183 to 976,058,
and mean channel error from 11.755233 to 11.714645. Two independent production
runs match all five published states; all 26 acquisition clocks hold. Upkeep
and 13 panel text regions remain exact. Fog-grid production/blockers, special
neutral/camp overlays, remembered visibility, shadows and full-screen/tick parity
remain open. See [minimap evidence](../re-notes/campaign-minimap.md#hero-image-pass-and-remaining-sight-trace-2026-11-03).

Blocked sight no longer erases another source's clear visibility or exposes a
fresh unexplored cell as fogged. The overlap regression fails before the fix;
all 24 vision checks, strict native/WASM checks and both builds pass. Only 387
minimap pixels change. Full-screen differences fall by 61 to 975,997, with mean
channel error 11.714487. Two independent captures match five published states
with all 26 clocks held. Native span/graph reconstruction progresses separately;
the corrected plane labels supersede the earlier 26-cell residual. See
[blocked-source evidence](../re-notes/campaign-minimap.md#blocked-source-preservation-and-sight-graphs-2026-11-04).


The campaign ledger now uses the native sight lattice and predecessor graph,
asset-derived occluder heights, and separate flying/modifier spans. Production
heights match all 12,288 retained native cells; the explicit Rust gate matches
209 span rows and propagation through eleven native graphs. Two fresh captures
match five published states with all 26 clocks held. Only 2,511 minimap pixels
change; full-screen differences fall by 1,138 to 974,859, with mean channel error
11.652511. Upkeep and 13 panel text regions remain exact. Remembered visibility,
special overlays, outline rasterization and native tick identity remain open.
See [sight integration](../re-notes/campaign-minimap.md#native-sight-lattice-integration-2026-11-04).


The latest captures include live day/night sight endpoints and a corrected
13-bit JPEG transform in the existing Rust decoder. The minimap base matches all
65,536 native RGBA pixels; current visibility matches all 1,209 native cells and
heights match all 12,288 cells. Against a newly acquired native reference, the
fresh baseline has 974,765 differing full-screen pixels and these changes reduce
that to 972,999 (MAE 11.609237). Earlier comparisons used a different reference.
The full frame still includes every differing pixel. Both fresh runs match five
published states and hold all 26 acquisition clocks. Explored history has 23
missing cells; marker memory, outline geometry/rasterization and cross-client
clock identity remain open. The base-image result does not establish final
minimap parity. See [current evidence](../re-notes/campaign-minimap.md#live-sight-endpoints-and-jpeg-precision-2026-11-04).

Menu panel models now retain their authored BLP mip chains through GPU upload,
using the same bounded loader as other model textures. The original-panel byte
regression fails before and passes after. Two fresh captures match five states
with all 26 clocks held. This changes 13,909 main-menu pixels; chapter/gameplay
frames retain their previous hashes. Native LOD/filter parity remains unqualified.
See [panel mip evidence](../re-notes/mdx-texture-selection.md#panel-mip-retention-2026-11-04).

The latest minimap outline uses the finite source frustum after camera projection
refresh and aliased pixel spans instead of a UI border. All 97 outline pixels
match the retained native frame. Only 141 recreation pixels change; full-screen
differences fall by 116 to 972,883, with mean channel error 11.583758409288194.
The full frame still counts every differing pixel. Two fresh captures match all
five states and hold all 26 clocks; fog grids and CPU minimap bytes are unchanged.
Other camera states/resolutions, explored history and native tick identity remain
open. See [outline evidence](../re-notes/campaign-minimap.md#finite-outline-frustum-and-aliased-line-coverage-2026-11-04).
