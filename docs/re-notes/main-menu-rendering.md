# Main menu rendering — which assets the 1.29b TFT runtime actually selects

Findings on the original main-menu background. The headline is an **asset
selection** error: the scene we render is the *Reign of Chaos* one, while the
project's target is *The Frozen Throne*. Evidence is read from the shipped
archives directly; the probe is `tools/diag/scene_geosets.py`.

## TFT and RoC ship different main-menu scenes

Two distinct scenes exist, in different archives:

| | scene | archive | geosets | materials | textures | sky texture |
|---|---|---|---|---|---|---|
| RoC | `UI\Glues\MainMenu\MainMenu3d\MainMenu3d.mdx` | `War3.mpq` | 33 | 31 | 33 | `white32.blp` (pure white, opaque, unshaded) |
| **TFT** | `UI\Glues\MainMenu\MainMenu3D_Exp\MainMenu3D_Exp.mdx` | `War3x.mpq` | 18 | 14 | 49 | **`SKY2.blp`** (unshaded) |

The expansion scene's own textures name it plainly: `Ice_Ice.blp`,
`Ice_Natural01.blp`, `Iceberg_New.blp`, `SnowyRocksdull2.blp`, `FrostWyrm.blp`,
`Chains_silver.blp`, `ocean_h.01..08.blp`, `SnowyRocksdull2.blp`,
`lensflare1Ax.blp`, `SKY2.blp` — an **iced** scene. The base scene's are the Orc
camp (`Dirt1`, `GrassTuft`, `Catails`, `Banner_orc2`, `Wetrock1`, …). One of
these is The Frozen Throne.

**So the grey/white sky is not a lighting or material defect: it is the wrong
scene.** RoC's sky is geoset 29, a 25-vertex/32-triangle dome 86,000 units
across textured with `white32.blp`, whose every one of 1,024 pixels is
`(255,255,255,255)` — an opaque, unshaded, pure white dome, which is exactly
what a grey-white sky looks like. TFT's sky is `SKY2.blp` on a comparable dome
(geoset 14, diagonal 92,966) and is a real sky texture.

## TFT does not draw the `WarCraftIIILogo` model

The expansion `UI\FrameDef\Glue\MainMenu.fdf` (in `War3x.mpq`, md5
`62d2d414681498ffa093ffd185b333be`; the copy in `War3Local.mpq` is
byte-identical) declares the sprite frame but **comments the art out**:

```
Frame "SPRITE" "WarCraftIIILogo" {
    //BackgroundArt "UI\Glues\MainMenu\WarCraftIIILogo\WarCraftIIILogo.mdl",
```

`UI\FrameDef\FrameDef.toc` in `War3x.mpq` is the expansion's FDF load list, and
it does list `UI\FrameDef\Glue\MainMenu.fdf`, so this *is* the file TFT loads.
Consequences:

- The `WarcraftIII-logo-alpha.blp` artwork verified earlier — which decodes to
  the *Reign of Chaos* logo — is **RoC branding and belongs to the RoC menu**,
  not to TFT's. It is not wrong, it is the wrong menu's logo.
- Whatever serves as TFT's branding is not that model; it is not named in this
  FDF.

## How the scene is selected: **unproven**

Neither `MainMenu3d` nor `MainMenu3D_Exp` appears as a literal in
`Warcraft III.exe`, while `WarCraftIIILogo` does.

**What that does not prove.** A missing literal does not establish that the
runtime assembles the path by concatenation. The path could equally come from
the `.toc`, from an FDF, from a config file, from a table of asset ids, or from
a literal that is present but not byte-aligned as the grep expects. Stating
"the runtime concatenates a suffix" would be an inference from an absence, and
the absence is weak evidence.

**What is established.** The expansion's own archives ship
`MainMenu3D_Exp.mdx` with iced textures and its `FrameDef.toc` names the
expansion `MainMenu.fdf`, which comments out the logo model. That is evidence
about *which assets exist and which FDF TFT loads*; it is not evidence about the
mechanism that picks the scene. Pinning the mechanism needs a runtime
observation — the file TFT actually opens — which this note does not have.

## Layer flag meanings

The two words in `LAYS` — at `+12` and `+16`; `pcx-mdl` names them the other way
round from the format — carry, across both scenes:

```
                 filterMode            shadingFlags
RoC  MainMenu3d   0×18, 1×6, 2×4, 3×1, 4×2   0x00×17, 0x10×8, 0x21×2, 0x90×2, 0xA1×2
TFT  MainMenu3D_Exp  0×7, 1×2, 3×5            0x00×7, 0x01×1, 0x10×2, 0x21×3, 0xA1×1
```

### The flag bits, from primary implementations

Retera's Model Studio — a long-standing community model editor whose source
publishes the format — specifies:

| bit | meaning |
|---|---|
| `0x01` | Unshaded |
| `0x02` | SphereEnvironmentMap |
| `0x04` | — (not named in the reference) |
| `0x10` | **TwoSided** |
| `0x20` | **Unfogged** |
| `0x40` | **NoDepthTest** |
| `0x80` | **NoDepthSet** |

Sources: Retera's Model Studio format code
(<https://www.hiveworkshop.com/threads/retera-model-studio-reforged-hack.320070/>)
and the open-realm MDX format documentation
(<https://corepunch.github.io/open-realm/games/warcraft-3/file-formats/mdx/>).
Both are cited rather than reproduced; if the decoder source is retrieved, link
the file and line as well.

**What this class of evidence is.** A published decoder is a *format*
specification by implementation: it states what the bits mean in the bytes. It is
stronger than correlating flags with geometry, which is why the earlier
"thin surfaces carry `0x10`" observation is now only a **corroboration** and not
the proof. It is *not* evidence of what the original renderer does with each bit —
rendering behaviour, and specifically whether `Unfogged` and `NoDepthSet` are
honoured the way the name suggests, would need the shipped runtime or a capture.

**Two earlier errors this corrects.**

1. `0x10` was guessed to be `NoDepthTest` in an earlier pass. It is `TwoSided`,
   and the geometry that carries it — Catails, Arrow_64, both ElwynnFlowers,
   Banner_orc2, Critter_Bird in RoC; Chains_silver, Ice_Ice in TFT — is exactly
   the thin-surface set, which is the corroboration, not the argument.
2. `0x20` was going to be read as a depth bit. It is `Unfogged`, so the
   screen-filling sky domes (which carry `0x21` = `Unshaded | Unfogged`) are
   **not** a depth-testing problem at all. `NoDepthTest` is `0x40`, which no
   layer in either scene sets. The earlier hypothesis that the domes covered the
   scene by drawing without depth testing is therefore withdrawn.

No culling or depth behaviour has been changed on any of this.

## Minimal proposed fix

1. **Select the scene by target.** For the 1.29b TFT target, load
   `UI\Glues\MainMenu\MainMenu3D_Exp\MainMenu3D_Exp.mdx` from `War3x.mpq`; keep
   `MainMenu3d.mdx` for a base RoC configuration. `MpqSet::extract` already
   searches every member archive, so the file resolves without new plumbing —
   only the path constant changes.
2. **Serve the expansion scene's own textures.** Its 49 `TEXS` entries, not the
   base scene's 33; several are unique to it (`SKY2`, `Ice_Ice`, `FrostWyrm`,
   `ocean_h.*`, `lensflare1Ax`).
3. **Do not render the `WarCraftIIILogo` model for TFT.** Its FDF comments the
   art out. Rendering it puts RoC branding on a TFT menu.
4. Leave the base scene and its logo in place for RoC; the finding is a
   selection bug, not that either asset is wrong.

## Open

- `filterMode` 6 (`Modulate2x`) has no equivalent in Bevy's `AlphaMode`; no
  layer in either scene uses it.
- The shading-flag *names* come from published format implementations. What the
  **original renderer** does with `Unfogged`, `NoDepthSet` and `TwoSided` is not
  established here; that needs the shipped runtime or a capture.
- The expansion scene's `SEQS`, `CAMS`, `LITE` and `PRE2` have not been read
  here; its sky, geometry and materials are what establish the selection.

## The black band at the bottom of the frame

The TFT capture has scene content down to about 87% of the canvas height and
**pure black `(0,0,0)` below it** — the clear colour, not dark terrain.

**What it is, measured.** The expansion scene has exactly **one** camera,
`Camera02good` (the whole `CAMS` chunk is 664 bytes, one record):

```
eye (4205, 506, 415)   target (-1802, 349, 940)   fov 1.0466 rad (60.0°)   near 331   far 70,000
```

The camera sits 6,031 units from its target and its view direction is
`(-0.9959, -0.0261, +0.0872)` — it looks very slightly **upward**. The whole
scene's geometry spans `z -1841 .. 27867`, and the sky dome `SKY2` (geoset 14)
spans `z -1841 .. 21634`: the **lowest geometry in the model is `z = -1841`**.

With a 60° vertical field of view and a near-horizontal view, the bottom edge of
the frame looks about 25° below the horizon. At the dome's distance that is tens
of thousands of units down, far below `z = -1841`. The frame therefore extends
past the bottom of the model, and what is drawn there is the **clear colour**.

So the band is not missing geometry in the decoder's sense and not a texture
fault; it is the camera's frustum reaching below a diorama that has a bottom.

**Candidates, none established.**

1. **The camera is animated and we use its static default.** The 664-byte record
   is about 120 bytes of fixed fields plus roughly 544 bytes of tracks, so
   `Camera02good` moves. This note's numbers are the record's default
   eye/target, which is what the scene renders from. Sampling the camera's own
   tracks, the way bone tracks are sampled, is the first thing to try.
2. **The field of view's axis.** The model stores `1.0466` without saying whether
   it is vertical or horizontal. Read as horizontal, the vertical angle would be
   about 47° at 4:3 — less of the frame below the horizon, a smaller band. Read
   as vertical, as it is here, the band is larger. Nothing measured so far
   decides this.
3. **The clear colour.** WC3 may not clear the menu to black. A capture of the
   original is needed; no amount of reading the model settles it.

Choosing between these needs the shipped runtime or a capture of it. No clear
colour, field-of-view axis or camera sampling has been changed on this evidence.
