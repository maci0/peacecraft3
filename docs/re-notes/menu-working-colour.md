# Original-menu working colour channels

Original-menu textures, geoset tints, light colours, particle colours, fog and UI now
share numeric working RGB through sampling and blending. Authored alpha, texture pixels,
sampler flags and material relationships are unchanged. This corrects a composition
mismatch without fitting opacity to screenshots.

## Runtime evidence and interpretation

The selected Legacy executable is SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. A fresh isolated
Wine 10.0 run covers process launch, Main, the blank Options landing, Video, return and
shutdown. Its complete bounded trace contains 2,733,384 lines (248,003,898 bytes),
194,508 render-state setters and 96,368 sampler-state setters, with no unparsed setters.
It records no sRGB-write or sRGB-texture setter, no pixel-shader binding and no
gamma-ramp call. Its 1,264 presents use the ordinary device Present entrypoint, which
has no presentation flag parameter.

[D3D9 gamma
documentation](https://learn.microsoft.com/en-us/windows/win32/direct3d9/gamma)
specifies default-disabled sRGB sampling and writes. The [Wine 10.0 setter trace
implementation](https://github.com/wine-mirror/wine/blob/wine-10.0/dlls/d3d9/device.c)
identifies the observed setter records. Taken together, the complete interval and
documented defaults support preserving numeric source RGB through this menu's sampling
and blending. That is an inference from API evidence, not a direct state query or proof
of native Windows hardware output.

A first browser candidate passed navigation-state checks but rendered blank. Its
validation errors and captures are retained, and it is rejected; fresh runtime
acceptance also checks rendering errors logged through the Rust console.

The first trace reached its line bound and is explicitly rejected as whole-interval
evidence. The second pauses the owned container only during the initial visual-review
handoff, then resumes the same input journal. Its raw decompressed byte count and hash,
EOF, setter coverage, screenshots and stopped-process state are checked. Both traces and
all original content stay local under `.scratch/menu-composition-root-20261018/`.

## Implementation

`menu_composition` marks only cameras whose entire input path uses numeric menu
channels. Menu images use `Rgba8Unorm`; colour constructors upload the source numbers
directly. The background, panel and ordinary UI cameras share the same compositing
space. The final existing Bevy blit decodes that working buffer for the display's sRGB
attachment, preserving displayed numeric values after hardware encoding. No replacement
shader is introduced. A small patch to the vendored Bevy render crate declares alternate
main-target views only when the adapter reports support. Without that guard, WebGL
rejects raw main textures because it lacks VIEW_FORMATS and the canvas is blank. The
original format and compositing equation are retained. Bevy's published formatting and
license notices are preserved. The application regression includes the exact production
capability helper source.

The marker is removed from the UI camera when returning to the custom launcher. Its
normal colour path remains available. Nonfinite material values still refuse before
writes, and updates validate all targets before mutation. Finite values above one remain
unclamped; the former admission check for overflow during sRGB-to-linear conversion no
longer applies because that conversion is absent.

## Verification

The frozen application source is `2f02f7cabd6f3895712aec6e14ab75c33b293d42`, tree digest
`29bc972f7c08d4f898e6e822081e84155e3c9a521c150d4da4c6ad733b805099` in the retained
verification manifest. The ordinary suite passes 647 tests with 24 explicit
local-input/GPU ignores. Strict native and browser-WASM Clippy, native and WASM
application builds and formatting pass.

Five explicit GPU tests pass on both Radeon RX 7900 XTX Vulkan and llvmpipe. The new
composition test covers geometry, the production particle shader, ordinary UI images and
both custom UI material paths, with alpha zero, partial alpha and alpha one. It checks
120 RGB components against an independent equation for both the numeric path and a
linear control. The control differs in 94 checked components. Its two-byte synthetic
comparison allowance is local to the synthetic readback test; full-frame original
comparisons retain zero tolerance. Existing geoset colour, depth, HUD and tooltip checks
also pass. The comparison harness passes 47 Python and five Node/browser tests,
including recognition of renderer errors logged at Rust ERROR severity through the
ordinary browser console.

Fresh native and actual browser-WASM builds complete the same eleven-event Main →
Options landing → Video → Main journal at 1024×768, 1280×960, 1920×1080, 2560×1440 and
3840×2160. All forty new raw captures are visually reviewed, and every browser
after-capture state is the requested settled screen, with no console renderer errors.
Native visual capture uses an isolated ALSA null PCM; audible output is not checked. All
owned processes are stopped.

The local report `.scratch/menu-composition-root-20261018/comparison-2/index.html`
combines those captures with twenty retained original WC3/Wine references. Its sixty
full-frame RGB comparisons all differ. No tolerance, version mask, normalization,
cropping or rescaling is applied. Browser snapshots change during all twenty captures,
so no shared animation frame is established. `verification-final-2.json` checks
source/build/package hashes, source closure, raw extents, image hashes, input journals,
settled state, harness identities and unrelated-file preservation. The original API
trace and the retained original image matrix have separate provenance.

## Open fidelity gates

Exact full-frame equality, native Windows hardware, lighting, fog distance/style,
particle simulation/RNG, animation phase, multiple material layers and complete blend
behaviour remain open. The retained asset closure has explicit mixed-source
compatibility provenance; it does not establish retail archive priority. Missing
client-image defaults and unsupported popup audio codecs remain reported. Parser/GPU
transport tests and menu navigation do not establish deterministic simulation or
original/native/WASM lockstep. The bottom-right version-string footprint remains
unqualified, so no pixels are excluded from comparisons.
