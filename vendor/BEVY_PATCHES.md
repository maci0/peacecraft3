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
