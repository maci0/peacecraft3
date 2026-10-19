# The expansion menu's panel and logo

The expansion main menu is missing two pieces the original shows: the top-left
**Frozen Throne logo**, and the **chain-hung riveted panel** the buttons sit on.
A reference still from the original menu is at
`.scratch/video-reference/classic-1285-menu.jpg`.

Probe: `tools/diag/scene_geosets.py`. Extracted art lives under
`web/assets/menu-exp/` (ignored — see `.gitignore`).

## The logo — found, and its entrance is in the model

The expansion `UI\FrameDef\Glue\MainMenu.fdf` (in `War3x.mpq`) declares the
sprite and **comments out** its art:

```text
Frame "SPRITE" "WarCraftIIILogo" {
    //BackgroundArt "UI\Glues\MainMenu\WarCraftIIILogo\WarCraftIIILogo.mdl",
    SetPoint TOPLEFT, "MainMenuFrame", TOPLEFT, 0.13, 0.04,
}
```

The frame and its anchor survive. `War3x.mpq` supplies what the commented line
withheld:

```
UI\Glues\MainMenu\WarCraftIIILogo_exp\WarCraftIIILogo_exp.mdx   1,820 bytes
UI\Glues\MainMenu\WarCraftIIILogo_exp\WarCraftIIILogo_expbeta.mdx
UI\Glues\MainMenu\WarCraftIIILogo_exp\War3xLogo.blp
UI\Glues\MainMenu\WarCraftIIILogo_exp\12plus.blp
```

Its one texture is `ReplaceableTextures\WorldEditUI\WarcraftIIIFTLogo.blp`
(BLP1 content 0, 512×256, alpha 8), which decodes to the *Warcraft III: The
Frozen Throne* artwork — icy blue "WARCRAFT", metallic numeral, "THE FROZEN
THRONE" band — on a transparent field (26.6% fully transparent, 46.7% opaque,
26.7% partial). That is the logo the reference shows at top-left.

The model is one geoset of **four vertices**. Parsed from its `VRTX`:

```
minX -0.1120899990   maxX  0.1114600003     width  0.2235500
minY -0.0582000017   maxY  0.0533000007     height 0.1115000
z    0.0000000000    on all four            aspect 2.0049
```

All vertices are planar at z=0. Screen-space placement is supported by the
native CSpriteFrame default projection trace, not by planarity. The
extents are near-symmetric about the origin but not exactly, and the format
stores **no sprite origin** — the quad says what the artwork's rectangle is, not
where its pivot sits.

The model's `MODL` chunk carries the same rectangle independently, which is worth
recording because its **radius field is misleading on its own**:

```
MODL 372 bytes, name "WarCraftIIILogo_exp"
  radius    (+340) 0.0
  minExtent (+344) (-0.1120899990, -0.0582000017, 0.0)
  maxExtent (+356) ( 0.1114600003,  0.0533000007, 0.0)
  tail      (+368) 0x00000096 = 150
```

The radius is `0.0`, but the min/max extents are **not** zero and match the
`VRTX` bounds exactly. A zero *radius* is therefore not a zero *extent*, and an
earlier reading of this model that called its bounding box "zero-extent" was
conflating the two: the box is real and its size is `0.2236 × 0.1115`, while only
the single radius scalar is zero. The extents are the authority here; the radius
is not.

### The entrance is the model's own Bézier

`SEQS` and `MTLS`, as parsed:

```
SEQS  Birth 0..500 (500 ms) · Birth Alternate 1333..3167 ·
      Stand 5000..60000 (looping) · Death 65000..65500
MTLS  KMTA, 7 keys, interpolation 3 (cubic Bézier)
```

The two entry keys are `frame 0 value 0 in 0.0 out 0.5333330035` and
`frame 500 value 1 in 0.8666669726 out 1.222219944`. A key on disk is
`frame(u32) value(f32) inTan(f32) outTan(f32)`; measured, the first tangent is
the key's **in** tangent (the shipped first key holds `in 0.0`, which is what a
zero first key looks like).

The segment is a cubic Bézier whose control **values** are
`(k0.value, k0.out_tan, k1.in_tan, k1.value)`:

```
v(t) = (1-t)³p0 + 3(1-t)²t·p1 + 3(1-t)t²·p2 + t³p3
```

which gives **0.65** at the midpoint. That is deliberately not `0.5`: a linear
ramp gives 0.5, and the Hermite reading — treating the stored numbers as
derivatives, `h00·v0 + h10·m0 + h01·v1 + h11·m1` — gives **0.458**. The three
readings are distinguishable, and the tests assert the midpoint is 0.65 and
more than 0.1 away from both wrong values.

`pcx-mdl`'s `Interpolation::Bezier` currently dispatches to the same `hermite`
as `Interpolation::Hermite` (`crates/pcx-mdl/src/lib.rs`, the bone sampler), so
the crate reads the logo's tangents as derivatives and would produce 0.458.
That divergence is **recorded, not silently joined**: the entrance parses the
`KMTA` block itself with the measured layout, and the crate change is left to a
separate decision because it also moves every bone animation.

Past `Birth` the entrance **holds** at the `Stand` value. Following the raw
track would drop it back to 0 at frame 1333, in the `Birth Alternate` segment,
which the entrance does not play.

### Selection and placement evidence

The shipped skin table maps `MainMenuLogo_V1` to the expansion logo. The original main-menu initializer constructs `MainMenuLogo_V%d` and resolves it through the skin lookup before assigning sprite art. This establishes the lookup mechanism and table entries; the live edition-selector value was not measured.

The original default sprite projection adds raw model vertices to the frame's LEFT/BOTTOM origin. With unit model scale and zero glue bias, the FDF anchor and measured bounds give normalized left 0.0223875, top 0.0303333, width 0.2794375 and height 0.1858333. Those conditions and the exact executable trace are recorded in [runtime evidence](glue-coordinate-runtime.md).

## Integration and evidence boundary

The module is declared in `main.rs`. `launch_gui` loads the selected installation's TFT logo model and texture, parses its actual `SEQS`, `VRTX` and `KMTA`, and places a relative-flow image through the original default LEFT/BOTTOM sprite projection. Its UV pairs are `(0,0), (0,1), (1,0), (1,1)`, matching the image's full rectangle. The root menu uses two children in the same relative grid cell: button flow and a non-pickable logo layer. No absolute UI positions or guessed width are used.

The 500 ms entrance samples the parsed cubic Bézier and preserves fractional frame time. It resets when the menu is created and holds the final Birth value. Asset selection is supported by the shipped `MainMenuLogo_V1` skin mapping and the native glue's `MainMenuLogo_V%d` lookup. Placement is supported by the original default projection; its live model scale and global glue bias were not sampled. See [original runtime evidence](glue-coordinate-runtime.md).

The background fills the display separately from the authored UI viewport, as the original background initialization explicitly sets the full-width flag. Widescreen controls follow the centered authored branch; square/5:4 displays retain the original direct conversion. Portrait browser containment is a compatibility policy, not an observed original-client mode.

The hanging panel is the authored `SpriteLayers` models; see [panel origin](menu-panel-origin.md). Its `MainMenu Birth` entrance, geoset alpha and sound events are rendered from shipped data; see [sprite rendering](menu-sprite-rendering.md). The visual-fidelity gate remains open.


## Edition switch logo lifetime (2026-10-19)

The controls and scene already followed the selected edition, but a live logo
prevented its initializer from reading the new skin entry. Source-selection
changes now retire that logo and clear its entrance before initialization. The
model, texture, quad, anchor and Birth curve still come from the selected assets.

A synthetic dispatcher regression checks both switch directions and unchanged
selection. An explicit local Legacy test checks TFT/RoC/TFT image replacement.
All 411 native menu tests, strict native/browser checks and both builds pass.
A private native render journal uses the production dispatcher, the selected
Legacy installation and embedded shaders: all three captures show the expected
logo. The first private probe had the wrong embedded-shader crate namespace and
is rejected. Evidence: `.scratch/menu-edition-root-20261019/verification-1.json`.

This does not fix the separate RoC PRE2 batch refusal at its model-space burst
emitter. Original runtime, fresh browser visual and 4K parity remain open.
