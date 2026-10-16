# Explicit model-viewer terrain texture selection

The viewer takes an explicit model member. An optional `--tile FOURCC` selects
one exact four-byte `tileID` from the supplied installation's
`TerrainArt\\Terrain.slk`. Without that option it draws no terrain preview.
The `viewer::run` model-only API remains; `run_with_terrain` takes the optional ID.
The two terrain path helpers now require the ID, and `terrain_plane_mesh` takes
minimum/maximum bounds plus a diagnostic source and returns a checked result.

Selection preserves authored case and spelling. Missing or duplicate parsed
rows, missing/duplicate column names, missing selected values, parser errors and
error/post-terminator anomalies refuse. It does not pick the sorted first row or
trim/case-fold an identifier. Directory/file values provide the named BLP path.
Archive/table/texture failures retain their installation/member/ID context and
underlying causes through `TerrainTileError`. Requested texture and model-bound
preview validation happen before creating a window. No fallback color is supplied.

The preview's horizontal bounds and elevation come directly from the selected
model's extent record. Non-finite/inverted bounds and zero-area XY footprints
are errors; a flat Z range is allowed. Counter-clockwise triangles face the same
direction as their +Z normals, matching Bevy's ordinary back-face culling.

The shared graphics model loader also rejects non-finite MODL extent components
before importing geometry, with the source and axis in the decode error. The raw
MDX header reader still preserves those words for inspection. Finite flat, empty
and inverted records remain unchanged at this boundary; the footprint consumer
keeps its stricter ordering and nonzero XY requirements. In particular, four
shipped particle/effect headers in each supplied install have inverted bounds,
so a global ordered-box rule would reject original assets. The explicit header
census read 3,234 compatibility-install and 3,249 Legacy-install models: none
had non-finite header bounds. This uses the selected archive-reader policy and
does not establish native archive priority or rendered behavior.

This is a whole-texture, unlit model-viewer convention. It does not establish
terrain atlas cell/variant selection, native UVs/sampling/material behavior,
W3E world-height calibration, ground blending, cliffs, water or gameplay camera
semantics. Model texture/camera/light/animation behavior outside the optional
preview is unchanged. No original game bytes or native runtime were used for
the synthetic selection/bounds tests.

The shared SYLK parser currently retains the first repeated value at one cell
coordinate. This selector validates the resulting rows and headers; it does not
establish a precedence or reject that lower-level repeated-cell case. Runtime
map terrain binding remains separate: active parsed map contexts do not yet
retain explicitly sourced terrain textures/receipts, and the viewer is not used
as a substitute for that integration.

The existing per-update animation step is a deterministic viewer policy, not an
asset-authored frame rate or a native clock. Camera/light/material and model
error fallbacks outside the selected terrain path remain separate review debt.

Validation covers typed selection/read/decode refusals, exact authored row/value
preservation, caller bounds and triangle orientation. The public cases also
execute as actual 32-bit WebAssembly. A separate wholly synthetic GPU draw uses
the production quad and a reversed-winding control with one shared camera,
texture and default back-face culling: the corrected control draws and the
reversed control stays clear. This proves that preview orientation on the
reported Vulkan device, not original terrain rendering or game fidelity.
