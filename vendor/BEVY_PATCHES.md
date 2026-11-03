# Bevy 0.20 patches

`bevy_picking`, `bevy_ui` and `bevy_sprite` start from their published 0.20.0
sources and retain the MIT/Apache license notices. The workspace patches those three crates
to resolve batched pointer inputs at each event's location. Without that change,
the final frame hover can miss a fresh click or attribute it to another control.
Cargo cache stamps and unused per-library lockfiles are omitted; the workspace
lockfile owns dependency resolution.

The application opts in with `OrderedPointerInput` after `DefaultPlugins`.
UI, sprite and window picking share their normal hit geometry with registered
event-local resolvers. Observers run through the existing pointer state machine;
their deferred changes are applied before the next input. Final live hover also
reconciles stationary drag targets, without inventing movement events or replaying
layout. Applications without the marker retain the normal frame-based path.
Two UI documentation corrections remove a feature-disabled type link and an
obsolete lint expectation; they do not change runtime code. The demo already
ships the identical Bevy MIT notice in `licenses/bevy-particle-shader.txt`.

The current application installs those three backends. Mesh picking is compiled
but its plugin is not installed. Any added mesh or custom backend must register
an equivalent event-local resolver before using the marker. Resolution work grows
with the actual inputs and registered backends; no input-count cutoff is imposed.

Regression coverage lives in `menu_picking_tests.rs` and
`menu_picking_slider_tests.rs` under the application's private settings module.
It covers real window-input/picking/widget dispatch, capture, cancellation,
touch, multiple pointers, observer changes, backend priority, and stationary
targets. These tests do not establish original-client behavior or rendered
fidelity. See [Options input](../docs/re-notes/options-slider-input.md) for the
application and browser verification boundary.

## Main texture view formats

`bevy_render` also starts from published 0.20.0 and retains its MIT/Apache notices.
Its main-target texture descriptor declares alternate sRGB views only when the
adapter reports `DownlevelFlags::VIEW_FORMATS`. WebGL cannot declare these views;
without the guard numeric menu targets fail texture validation and render blank.
The target format, pixel bytes and compositing rules are unchanged. The vendored crate retains the published style edition for formatting. A synthetic
capability regression covers both supported format pairs and formats without an
alternate view. Actual browser rendering is a separate application gate.

## Render submission counters

`bevy_render` also carries `src/render_stats.rs`: process-wide relaxed atomic counters for the
draw calls (`draw*`, single indirect draws), `multi_draw_*` calls, pipelines and bind groups
actually set through `TrackedRenderPass` (redundant sets are skipped there and not counted), and
for `RenderDevice` bind group, buffer (with requested bytes) and texture creation. They are read
with `bevy::render::render_stats::snapshot()`; a difference of two snapshots counts one interval.
The patch adds one call to each of 15 existing functions (`render_phase/draw_state.rs`,
`renderer/render_device.rs`) and changes no rendering behaviour. Only the render CPU workload
(`docs/performance/render-cpu.md`) reads them. Bind groups made through `wgpu` handles directly
and buffer writes through `RenderQueue` are not counted.

## UI text alpha test

`bevy_ui_render` also starts from published 0.20.0, with its MIT/Apache notices
retained. `TextAlphaTest` on a UI text root selects an eight-bit GreaterEqual
reference for combined atlas/text alpha; surviving alpha is preserved. Text
and shadow extraction pack the reference into unused high vertex-flag bits,
so vertex stride and draw batching stay unchanged. Component changes/removal
invalidate cached draws. Without the component the reference is zero and
rendering follows the ordinary path. The resource-bar labels opt in with the
selected executable's derived reference; no source cutoff is baked into Bevy.

The isolated synthetic GPU regression in `menu_colour_gpu_tests.rs` exercises
threshold updates, faded text, surviving coverage and removal. Original
rendered acceptance remains a separate full-screen comparison.

`TextAtlasFilter` optionally selects nearest atlas texels at mip zero for a text
root and its shadows. Extraction carries the selection in unused flag bit 8192;
the shader uses a bounded texel load. Shared atlas images, sampler objects,
vertex stride and batch keys are unchanged. Component changes/removal invalidate
cached draws; absent components preserve ordinary linear sampling. Resource
labels derive the default selection from the admitted executable's sampler
operands. An isolated GPU test verifies changes and removal restore the baseline.

## Bitmap-space implied curve points

`skrifa` remains published version 0.44.0, already used by Swash/Bevy. Its Rust
sources, generated tables and MIT/Apache notices are retained here. No additional
font backend or runtime dependency is added.

The observed game bitmap agrees with the FreeType bitmap convention: translate
a glyph to its bitmap bounds before outline decomposition. For fixed 26.6 coordinates, that whole-pixel translation
makes implied quadratic midpoints round down, including points below the source
baseline or left of its origin. Skrifa's ordinary outline conversion instead
truncates those negative midpoints toward zero before bitmap translation. The
patch gives fixed 26.6 path coordinates the bitmap-compatible midpoint operation;
unscaled integer and HarfBuzz floating coordinates retain their existing rules.
The widened sum also avoids overflow across the complete fixed-coordinate range.
This is an application bitmap compatibility choice, not a claim that all vector
outline consumers need the same rounding rule.

Two synthetic path tests cover whole-pixel translation and coordinate bounds;
the four existing path-order tests remain. `tests/bitmap_path.rs` is a focused
`rustc --test` harness for those source modules, avoiding the published crate's
unavailable upstream font-test-data fixtures. The application native/WASM checks
and full-frame capture validate the integration. Source-font raster probes stay
local; no game font or generated glyph bitmap is included in this patch.
