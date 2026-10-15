# Menu background display transfer

The background camera now explicitly selects no filmic tonemapping and disables
Bevy debanding, matching the existing panel-camera source-art policy. Installed
Bevy 0.20 Core3d registers default TonyMcMapface and enabled debanding for a
Camera3d that omits these components. The non-HDR standard material shader applies
that display stage after fog even for an unlit surface, so Unshaded alone does
not preserve the sampled source color.

This change removes an extra renderer transform; it does not replace texture or
vertex colors, alter source alpha, change material blend rules, disable fog or
move the camera. Earlier material/lighting calculations and texture/output sRGB
conversion remain in place. The policy is a recreation choice supported by the
retained primary SD renderer's direct texture/vertex-color output, not proof of
the original client's complete framebuffer transfer or gamma behavior. Lit
geometry and particles share the background camera and therefore the same policy.
UI/logo/panel cameras and their component policies remain separate.

The excessive fog veil is still an independent fidelity question. The current
qualified sky material carries Unshaded, not Unfogged; no texture name or geoset
ID exception is introduced. The source configuration and its current explicit
style/color interpretation remain unchanged. An on-axis radial-versus-axial
change cannot alone remove the previously calculated veil. See
[the authored fog boundary](menu-authored-fog.md).

The source-only analysis and installed primary shader/plugin fingerprints are
retained in `.scratch/menu-sky-cause-20261016/`. That independent ECS check
executes the two Core3d required-component registrations against the exact camera
tuple and a component-removal control; it is not a GPU or original-client run.
Root baseline regressions are retained in
`.scratch/menu-background-color-root-20261016/`: the plain test World shows that
the old factory omitted the policy, while the plugin-registration check identifies
the inherited Tony/Enabled values. Neither result alone is rendered acceptance.

## Checked rendering and integration

The independent non-HDR GPU calibration uses the production MenuMaterial with
synthetic sRGB textures, vertex colors, opaque/alpha/additive/modulate blending,
and an independent analytical fog and sRGB oracle. All 416 source-preserving
channel checks pass on both llvmpipe Vulkan and Radeon RX 7900 XTX Vulkan, within
two 8-bit codes. Maximum errors are below 0.646 and 0.600 codes respectively.
The old Tony/Enabled control changes 147 and 148 RGB channels. Alpha and
Unfogged controls retain their expected behavior. These are actual draws and
readbacks, not a native-client comparison.

The complete app tests, strict native/WASM Clippy, builds and rustdoc pass.
Explicit original scene-import gates pass for both RoC and TFT from each of the
separately selected Legacy and compatibility inputs. The actual release-WASM
worker checks pass 49 baseline and 103 panel assertions. A fresh six-capture
browser run loads the local original fixture, navigates Main/Video Options/Cancel
and Single Player/return, and reaches settled panels with no uploads, sockets
or page errors. All six captures were reviewed. The haze remains visible; no
fog improvement or complete original visual fidelity is claimed.

GPU evidence is retained in `.scratch/menu-background-transfer-gpu-20261016/`;
the independent Radeon run, integration logs and browser receipt are in
`.scratch/menu-background-color-root-20261016/`. The local fixture is a repack,
not retail-installer acceptance. No original assets or captures are distributed.
