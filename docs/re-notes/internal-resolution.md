# PeaceCraft internal game resolution

The PeaceCraft configuration panel has session-local 1×, 2×, 3× and 4× choices.
These are application rendering settings, not original game content. They are
absent from the imported main-menu and in-game options definitions. The default
1× path keeps the existing camera and display target.

At higher scales, `internal_resolution.rs` redirects only the campaign scene and
its sky to an offscreen sRGB image. Both physical dimensions are multiplied, so
2× renders four times as many pixels and 4× renders sixteen times as many. A
separate display-sized camera averages each complete N×N pixel block in linear
colour. The display resolution and the original HUD, cinematic overlays and menu
cameras retain their existing targets. No source layout, animation, material,
texture or game-rule values are replaced.

The image target's scale factor preserves the original logical viewport. This
keeps projection, cursor rays and field of view independent of supersampling,
including display DPI. Resizing rebuilds the target; returning to 1×, leaving a
campaign or replacing its camera retires the image, material and compositor.

Before allocating, dimensions are checked against the render device's texture
limit and an application ceiling of 64 megapixels. Unsupported sizes render at
1× and report the reason in diagnostics and the PeaceCraft panel. These limits
are allocation policy, not measured original game values. Higher scales increase
GPU memory and work; the choice lasts for the current application session.

## Verification

Synthetic tests exercise every scale, allocation refusal, the independent
launcher action and its selected state. Explicit headless/muted Vulkan tests
exercise the real render graph: all N×N samples contribute, output colours and
logical cursor rays are retained, sky and scene share the higher-resolution
target while UI retains the display target, and resize/exit retire owned assets.
The production launcher is rendered and activated at widths 480, 880 and 1280.
Local receipts and screenshots stay in the ignored
`.scratch/internal-resolution-20261025/` directory.

This optional supersampling path does not resolve the separately tracked sky
illumination and sight-occlusion fidelity gaps. It makes no original-client pixel
parity claim for those existing gaps.
