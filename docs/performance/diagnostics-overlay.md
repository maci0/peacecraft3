# Diagnostics overlay

F8 shows observed FPS and frame interval with bounded, age-compressed history.
The newest samples remain individual; older pairs retain their peak. The graph
is not a uniform time axis or an elapsed-time performance acceptance gate.

F7 groups display, renderer, scene, retained geometry, textures, device memory
and available campaign diagnostics into wrapping sections. Wide windows use two
columns; narrow windows use one. F8 and F7 occupy separate flex-layout slots.
When F7 wraps below F8 it fits the remaining height, with Page Up / Page Down
paging. Both overlays remain read-only and ignore pointer picking.

These panels are application diagnostics, not original-game UI assets. Their
translucent surfaces, type and spacing are presentation choices. The default
bundled diagnostic font needs no imported game assets.

Geometry counts describe mesh assets, not draw calls or total GPU vertices.
Render-eligible means the mesh permits render-world extraction, not that a draw
was submitted. CPU-released counts assets whose vertex data was extracted.
Retained vertex/index bytes are separate from GPU allocator and whole-adapter
VRAM reports. The current model animation path still skins vertices on the CPU
(`mainmenu_scene::skin_geosets`); deleting those copies would break updates.
Whole-adapter VRAM includes other applications. Missing backend memory counters
remain explicitly unavailable rather than being estimated from CPU assets.

Component tests cover toggles, paging, hidden-state updates, geometry reporting
and bounded peak compression. Asset-free captures at 1280x800 and 480x800 check
layout only; they do not establish original-client parity or campaign GPU cost.
