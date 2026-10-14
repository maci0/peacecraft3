# The blue behind menu entries, measured

Distinguishing the **blue button fill** from the **chain-hung panel**, from the
shipped textures themselves rather than from their names. Decoded read-only with
`target/release/blpx` for the headers and a small BLP1 paletted decoder
(`.scratch/blue-art/blp1_rgb.py`, ignored) for the pixels; nothing synthetic was
drawn.

## Answer

- The **blue behind a menu entry belongs to the button itself**, in two parts: its
  fill `GlueScreen-Button1-BackdropBackground.blp` is a **flat dark blue
  `RGB(0, 0, 58)`**, and its edge `GlueScreen-Button1-BackdropBorder.blp` is a
  **blue gradient** (`rgb(0,0,48)` … `rgb(0,0,64)`), 77.6% blue-dominant.
- The **grey** is the outer plate `GlueScreen-Button1-Border.blp` (behind *all* the
  buttons) and the **panel**, whose three TFT textures are greys with **0%
  blue-dominant texels**.

So "blue" is the **button's fill and edge**, and the panel's **textures** are
**grey metal**. They are different art, and the panel must not be called blue.
(That grey is a texture result; see the caveats on composited tints.)

## Measured colours

Decoded from mip 0 of each texture; "blue-dominant" means `B > R+16 and B > G+16`.

| Texture | Path | Size | Format | Measured |
| --- | --- | --- | --- | --- |
| Button fill | `UI\Widgets\Glues\GlueScreen-Button1-BackdropBackground.blp` | 32×32 | BLP1 paletted, `flags 0x0` (opaque) | **flat `RGB(0,0,58)`** — 1 palette index for all 1024 texels, **100% blue-dominant** |
| Button edge | `UI\Widgets\Glues\GlueScreen-Button1-BackdropBorder.blp` | 256×32 | BLP1 paletted, alpha 8 | **blue gradient** — `rgb(0,0,48)` 38.2%, `rgb(0,0,32)` 17.4%, `rgb(0,0,16)` 7.4%, `rgb(0,0,64)` 3.4%; **77.6% blue-dominant** |
| Outer plate edge | `UI\Widgets\Glues\GlueScreen-Button1-Border.blp` | 512×128 | BLP1 paletted, alpha 8 | **grey** — `(16,16,16)` 31.1%, `(0,0,0)` 28.4%, `(32,32,32)` 14.5% |
| Panel detail | `UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-ExpansionDetail.blp` | 256×256 | BLP1 paletted, alpha 8 | **grey** — 0% blue; opaque mean `(95.4, 95.4, 95.4)` |
| Panel pieces (TFT) | `UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-MetalPieces-Expansion.blp` | 256×256 | BLP1 paletted, alpha 8 | **grey** — 0% blue; `(0,0,0)` 49.1%, `(16,16,16)` 17.7% |
| Panel border (TFT) | `UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-MetalBorder-Expansion.blp` | 32×128 | BLP1 paletted, alpha 8 | **grey** — 0% blue; `(0,0,0)` 41.9%, `(16,16,16)` 23.5% |
| Panel pieces (RoC) | `Textures\GlueScreen-Panel-MetalPieces.blp` | 256×256 | BLP1 **JPEG** content 0, alpha 8 | (jpeg path, not decoded here) |

The fill is exactly one palette entry, so it is a **solid** colour, not a tiled
gradient: the 32×32 tile is uniform. The blue **gradient** is in the edge file,
which is a separate texture.

## How the fill is drawn — native FDF

The fill belongs to `StandardButtonBackdropTemplate`, which every main-menu button
inherits:

```text
Frame "BACKDROP" "StandardButtonBackdropTemplate" {
    BackdropTileBackground,
    BackdropBackground  "UI\Widgets\Glues\GlueScreen-Button1-BackdropBackground.blp",
    BackdropCornerFlags "UL|UR|BL|BR|T|L|B|R",
    BackdropCornerSize  0.016,
    BackdropBackgroundSize  0.032,
    BackdropBackgroundInsets 0.004 0.004 0.004 0.004,
    BackdropEdgeFile  "UI\Widgets\Glues\GlueScreen-Button1-BackdropBorder.blp",
}
```

So the 32×32 blue tile is **repeated** at a `0.032` background size
(`BackdropTileBackground`), inset `0.004` on all four sides, inside an eight-piece
edge of corner size `0.016`. That `BackdropEdgeFile` is
`GlueScreen-Button1-BackdropBorder.blp` — which measures **blue** (the gradient in
the table above), so **both** the fill and the button's own edge are blue. The
**grey** is the *outer plate* the button sits in:
`StandardMenuButtonBaseBackdrop` separately draws `GlueScreen-Button1-Border.blp`
with `BackdropBlendAll`. So of the four paths:

```
GlueScreen-Button1-BackdropBackground.blp   button fill    BLUE  (flat 0,0,58)
GlueScreen-Button1-BackdropBorder.blp       button edge    BLUE  (gradient)
GlueScreen-Button1-Border.blp               outer plate    GREY
GlueScreen-Panel-*                          panel model    GREY
```

That is per entry. The panel is a different object entirely — the
`TopRightPanel-Expansion.mdx` model whose textures are the grey `GlueScreen-Panel-*`
set (see `menu-sprite-rendering.md`).

## Initial rendering gap

At the initial measurement, the GUI
renders only the **outer plate** art (`BUTTON_ART` →
`GlueScreen-Button1-Border.blp`, the grey one) on main-menu entries. It does **not**
yet render the inner fill (`BUTTON_BG_ART` →
`GlueScreen-Button1-BackdropBackground.blp`) or the inner edge
(`BUTTON_BORDER_ART` → `GlueScreen-Button1-BackdropBorder.blp`). So the blue fill
and blue edge are **not** on screen; only the grey plate is. Do not read this note
as evidence that blue had been restored.

## Normal fill integration (2026-10-14)

The normal fill now uses the original decoded texture, with bounds derived from
the menu's FDF anchor chain and inherited backdrop insets. A relative three-track
grid places it inside the grey outer plate. Bevy's default image mode retained
the intrinsic texture size, producing a small square; `NodeImageMode::Stretch`
fixes that sizing. Every decoded RGBA texel must match before this path draws:
stretching and authored background tiling are pixel-equivalent for a uniform
texture. Non-uniform backgrounds are reported and omitted until tiling is implemented.

The real browser File-input import test uses locally repacked supplied menu
members, not a retail installer ISO. Fresh labels, scene, click audio, resize and
unequal-member rollback pass. Network capture records only two local Blob GETs
after import, with no HTTP requests, uploads, sockets or page errors.

At a 988×892 canvas, the Single Player fill's FDF-derived extent is
174.135×34.193 pixels; the capture measures 174×34. At 988×796 it derives
174.135×30.513 and measures 174×30. All six widths agree with raster extents.
These dimensions are observations, not layout constants. The largest edge
discrepancy is 1.133 pixels; the strict one-pixel diagnostic remains false.
Continuous source coordinates and pixel extents do not certify exact native
pixel alignment. Captures and full diagnostics remain local under
`.scratch/browser-iso-app-import/after-blue-mode-fix/`.

The blue inner edge, state overlays and hanging panel are still not rendered.
The uniform fill fix does not establish their mapping or full menu fidelity.

## Caveats

- **"The panel is grey" is texture evidence, not a composited colour.** Palette
  greys establish the *texels*. Whether the panel renders grey depends on tints
  this note does not fully resolve. What was checked, from the panel model:
  - its 51 `GEOA` records carry **neutral** static colours — 50 at
    `(1.0, 1.0, 1.0)` and one at `(0.7569, 0.7569, 0.7569)` — so the geoset-anim
    static colour introduces **no hue** (and there is no `KGAC` colour track, but
    that alone would not have ruled a *static* tint in or out; the measured static
    values do);
  - the material layers' static alpha is **not established** here: the field the
    decoder reads at layer `+28` is the coordinate id, and it reads `0.0` for
    every shipped layer, so material alpha and any material colour multiplier
    remain unknown;
  - a `CSpriteFrame` tint handoff was **not** read — it would need the executable,
    and the read-only REA probe on the 14.8 MB image timed out (see below).

  So the panel's **textures** are grey and its geoset-anim static colours are
  neutral, but a composited tint from material alpha, blending, or a frame tint is
  not excluded. Treat "grey" as the texture result, not the final pixel.
- These are **texel** measurements. The rendered colour also depends on
  `BackdropBlendAll`, the state edges, and the additive hover/focus overlays
  (`bnet-button01-highlight-mouse.blp`, `GlueScreen-Button-KeyboardHighlight.blp`),
  which this note does not decode. `RGB(0,0,58)` is the fill's own colour, not a
  claim about the composited result.
- The RoC `Textures\GlueScreen-Panel-MetalPieces.blp` is JPEG-content and was not
  decoded here; the TFT `-Expansion` variants are paletted and were.
- Palette alpha byte for the fill is `0`, but `flags 0x0` means alpha bits are
  **off**, so the fill is opaque; the `0` is not transparency.

## Correction: the material layer's static alpha **is** established

An earlier caveat above says the layers' static alpha is "not established" because
the field the decoder reads at layer `+28` is the coordinate id and reads `0.0` for
every shipped layer. That reasoning was sound but the conclusion is now **stale**:
the alpha exists, it is simply at a different word.

`docs/re-notes/format-mdx-materials.md` records native evidence from the Legacy
World Editor (`sha256 d198d2ca47fc29d4901f3de27ff17619824afef5af7e42b9ff17cbe03723d7b7`):
the layer reader `0x68fdb0`
(`ev_3a81e18f3e9583faade57a7d5b621c997ae6bfa687dc90328078b0ed208cc443`) reads an
inclusive size, **five integer fields, then a float**, and the keyword reader
`0x68f4b0` binds `Alpha` to that float. So the layer body is:

```
+12 filterMode    +16 shading flags   +20 textureId
+24 textureAnimId +28 coordId         +32 staticAlpha   <- layer-record +24
```

`+28` being `0.0` for every shipped layer is therefore the **coordinate id**, not a
missing alpha — which is exactly what that word is. Reading `+28` as alpha is the
error, and it is why the field looked unset.

What remains genuinely unestablished is narrower: whether any **material colour
multiplier** or frame tint composites over these texels. The static alpha itself is
now a measured format fact, and the "grey is texture evidence" caveat above stands
on the colour only.

## Native interpolation probe — a diagnosed blocker

The follow-up probe (identify the native Hermite/Bézier sampler address and call
chain) did **not** produce an address. Read-only through REA on the Legacy
executable (`sha256 3f2ed012…`, x86, opened from
`.rea/snapshots/3f2ed012….snap`):

- `rea_search_procedures` (regex `[Ii]nterp|[Aa]nim|Hermite|Bezier|Track`) and
  `rea_search_strings` (regex `[Ii]nterp|[Bb]ezier|[Hh]ermite`) both **timed out**
  on the 14.8 MB image;
- the existing evidence index
  (`.scratch/glue-coordinate-runtime/evidence-index.json`) contains **no**
  interpolation, Hermite or Bézier analysis — its operations are frame/layout,
  anchor, art-loader and coordinate work.

So there is no address to report yet, and this is the concrete next probe rather
than a plan: on the already-open session, either (a) run the procedure search with
a narrower pattern and a longer deadline, or (b) anchor structurally instead of by
name — find the function that switches on a track's interpolation byte at the
track reader and branches to its samplers, or locate the Hermite basis constants
(`2.0`, `3.0` in `2t³−3t²+1`) near the animation update. Name search alone will
not find it: the image is stripped of such symbols.

## Artifacts

Extracted to ignored `.scratch/blue-art/`:

| file | bytes | sha256 (first 32) |
| --- | --- | --- |
| `button-backdrop.blp` | 2,545 | `049d834cad049fc7a72297f6b6bd06d5` |
| `button-backdropborder.blp` | 23,034 | `e3e3fc30fcc8badf73b6cc9a905e3bdc` |
| `button-border.blp` | 175,946 | `5fb1d807e94e84dfdbbbab232980e7ef` |
| `panel-expansiondetail.blp` | 175,942 | `6796c3a729426d2dcfb7ceab1fdc5787` |
| `panel-metalpieces-exp.blp` | 175,942 | `f4f825f8b997b93cdb6f3bc3929127a3` |
| `panel-metalborder-exp.blp` | 12,106 | `2238da1f1f726ffe74dec244c4b9bb06` |
| `panel-metalpieces.blp` | 52,210 | `c4cee5eb946813cf09575ad8abfab9ec` |

Reproduce: `python3 .scratch/blue-art/blp1_rgb.py .scratch/blue-art/button-backdrop.blp`.
