# Authored original-menu fog

The original background now requires `UI\MiscData.txt` alongside its skin and
model. `menu_fog::MenuFog` parses the `MenuZFog` section through the existing
checked skin-table parser. The same explicit edition ordinal that selects the
skin's background selects its fog column. Color groups, density and distance
intervals remain source data; none of their observed values is a code constant.

Missing/duplicate fields, invalid UTF-8/numbers, invalid intervals, unsupported
styles and a missing edition column report an asset-qualified error. A lone RoC
column is never broadcast into TFT. The background camera receives the selected
`DistanceFog`; UI and logo cameras do not. Shared geoset construction reads each
checked source layer's independently documented Unfogged bit, so Unshaded alone
does not disable fog. Additive filter mode 3 suppresses fog color as a separate
reconstruction convention; other modes still honor the source flag. Missing
material/layer state refuses before scene writes.

Browser hydration imports canonical and served aliases together and validates
fog for every supplied background edition before replacing the committed asset
store. Rejected candidates preserve the previous revision, paths and bytes.
Clear/replacement uses the existing revision and scene ownership paths, without
an independent fog cache. Desktop reads reuse the configured installation policy;
unequal archive candidates remain a refusal, not an inferred archive priority.
The member and generated originals remain local, never in the public package.

## Evidence and implementation convention

Cached exact Legacy executable evidence
`ev_f47689b3e4b855d205277e54d8b9c615d2b52270a280438bd43525f39038f869`
links the background factory, edition selector and named MenuZFog records.
The qualified target expansion member is 9,954 bytes, SHA-256
`d097d3cbb4a03719a97591900b45ab42fdb847385819c3d5e7dd7b47d7d9b516`.
The base and expansion observations remain separate source receipts; equality
does not prove native archive priority or identify adjacent archive releases.

Grouped ARGB and style zero as linear falloff are explicit recreation
conventions. Density is retained but unused by that linear choice. Bevy applies
radial camera distance; exact original Z/range distance, color transfer, style
interpretation and visible match remain acceptance questions. No brightness,
ambient floor, screenshot fit or directional fog glow is introduced here.
Existing scene-light calibration and animation gaps are separate work.

## Retained runtime comparison

One bounded retained trace window around the qualified TFT background records
fog disabling before a source Additive layer, even though that layer does not
carry Unfogged. Adding fog color to this surface introduces a gray contribution.
The material policy therefore suppresses fog color for format mode 3, without
special-casing geoset IDs, texture names or measured material values. This is a
reconstruction convention supported by that draw, not proof of every additive
material's native behavior. The earlier sky draw precedes this transition and
its inherited fog enable/start/mode remain unknown. Later orthographic UI fog
settings are not transferred to the background.

The trace span is bytes `[6930706,7979282)`, SHA-256
`6f53c2bf189c2f4cec3814779fc3515b379423b56e3b42bdab51a976ffaa1ec6`.
Receipts and complete bounded events are in
`.scratch/menu-fog-runtime-correction/`. No live rendering was rerun for this
static retained-trace comparison.

## Checks and fixture requirements

The integrated application has 272 passing native tests and eight explicit
original-input ignores. Synthetic ECS tests verify the actual background camera
component, untouched UI camera and independent Unfogged/Unshaded material flags, plus source Additive fog suppression
without changing authored alpha or lighting.
Strict all-target/all-feature native and WASM Clippy pass. The paired original
RoC/TFT scene gate checks actual camera fog against its supplied config. A release
build and matching bindgen produce an asset-free 23-file package. That actual web
WASM passes 49 current import-worker checks, plus four fog-refusal/rollback cases,
valid single-column RoC gating, paired-edition enabling and canonical/alias clear.
These run in Node without DOM, server or network. Commands, outputs and source
hashes are in `.scratch/menu-fog-root-integration/` and `.scratch/menu-fog-release/`.
A fresh local Main/Video Options/Cancel/Main browser run with the qualified
original fixture passes four captures and records zero uploads, sockets or page
errors. That run predates the additive correction and showed excessive haze;
it is not final fog appearance acceptance. The corrected release now separately passes the same four-capture workflow,
49 actual WASM worker cases and four fog-refusal cases, with zero uploads,
sockets or page errors. All four captures were reviewed; haze remains a visible
fidelity question rather than a closed native match. The combined application
has 273 passing tests and eight explicit ignores after the cutout correction.
New receipts are in `.scratch/menu-cutout-root/` and
`.scratch/menu-cutout-release/`; public publication is tracked separately.
No native visual parity is claimed.

Old prepared 115-member menu fixtures lack the newly required configuration.
Regenerate them from their explicitly selected original sources before a fresh
browser run. Do not silently reuse them or patch in observed fog constants.
The original fixture generator now follows that required member. Public Pages
contains code and WASM only; users must supply their assets locally.
