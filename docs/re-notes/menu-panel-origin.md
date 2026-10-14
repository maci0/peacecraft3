# Menu panel origin: the skin table and the sprite-layer panel

Read-only record of two bounded lookups that between them settle where the
expansion menu's logo and hanging panel come from. Nothing was rendered to write
this; every path, size and hash below is from the shipped archives.

Sources (all `War3x.mpq` unless noted; extracted with `target/release/mpqxtract`,
no rebuild):

- `UI\war3skins.txt` — the **art-key table**, 44,153 bytes, in `War3.mpq`,
  `War3x.mpq` and `War3xLocal.mpq`;
- `UI\FrameDef\Glue\MainMenu.fdf` — the expansion menu's frame tree;
- `UI\Glues\SpriteLayers\` — the panel models and their textures.

## 1. The logo substitution is settled by shipped data

`UI\war3skins.txt` maps art keys to models. The logo keys, verbatim:

```text
MainMenuButton=UI\Glues\MainMenu\MainMenuButton\MainMenuButton.mdl
MainMenuLogo_V0=UI\Glues\MainMenu\WarCraftIIILogo\WarCraftIIILogo.mdl
MainMenuLogo_V1=UI\Glues\MainMenu\WarCraftIIILogo_exp\WarCraftIIILogo_exp.mdl
MainMenuLogo_V1Beta=UI\Glues\MainMenu\WarCraftIIILogo_exp\WarCraftIIILogo_expbeta.mdl
CampaignLogo=UI\Glues\SinglePlayer\CampaignWarCraftIIILogo\CampaignWarCraftIIILogo.mdl
```

This is what the native glue resolves when it assigns `MainMenuLogo_V%d`. Two
consequences:

- **`MainMenuLogo_V1` names exactly the model this investigation identified**,
  `UI\Glues\MainMenu\WarCraftIIILogo_exp\WarCraftIIILogo_exp.mdl`. The earlier
  note carried the substitution as *inference* — a shipped sibling beside a
  commented-out reference. It is now **shipped-data evidence**: the art key the
  runtime is told to use for the expansion points at that file, the table also carries a separate Beta model entry. The `%d` is the edition index — `V0` is Reign of
  Chaos, `V1` the expansion — which is why one FDF serves both.
- The FDF's commented `BackgroundArt` line is **literally `MainMenuLogo_V0`'s
  value**: `UI\Glues\MainMenu\WarCraftIIILogo\WarCraftIIILogo.mdl`. The line was
  not deleted because it is correct for RoC; the edition-specific override is the
  skin key, not the FDF.

One detail the loader must respect: the skin table names **`.mdl`**, but the
archives ship **`.mdx`** (`WarCraftIIILogo_exp.mdx` exists; `…_exp.mdl` does
not). The `.mdl` spelling is nominal and the engine resolves it to the shipped
`.mdx`.

`MainMenuButton`'s target, `UI\Glues\MainMenu\MainMenuButton\MainMenuButton.mdl`,
**does not exist** in any archive — see §3.

## 2. The panel is the `SpriteLayers` models

The chain-hung riveted plate is **not** an FDF backdrop and **not** the per-button
`StandardMenuButtonBaseBackdrop`. It is a pair of models under
`UI\Glues\SpriteLayers\`:

```text
UI\Glues\SpriteLayers\TopLeftPanel.mdx
UI\Glues\SpriteLayers\TopRightPanel.mdx
UI\Glues\SpriteLayers\Expansion\TopLeftPanel-Expansion.mdx
UI\Glues\SpriteLayers\Expansion\TopRightPanel-Expansion.mdx
```

The main-menu buttons sit on the **right**, so the panel of interest is
`Expansion\TopRightPanel-Expansion.mdx`. Parsed from its bytes:

```
chunks  VERS 4 · MODL 372 · SEQS 8184 · MTLS 816 · TEXS 2144 ·
        GEOS 135912 · GEOA 23004 · BONE 168044 · PIVT 828 · EVTS 584
geometry  57 geosets, 780 vertices
          x [0.4243, 0.9734]   y [-0.1072, 1.2526]   z [497.896, 498.621]
          largest piece 0.4003 x 0.6000; two pieces are thin strips (<0.02 wide)
```

These are raw, unposed bounds; they do not establish visible screen coverage or
projection. The native glue separately creates this overlay with camera `−1`;
see [the coordinate trace](glue-coordinate-runtime.md). The RoC sibling `TopRightPanel.mdx` is the same shape with **49 geosets / 744
vertices**, the expansion adding eight pieces.

Its `TEXS` names the panel texture family directly:

| # | flags | path |
| --- | --- | --- |
| 0 | `0x0` | `Textures\GlueScreen-Panel-Rail.blp` |
| 1 | `0x3` | `Textures\GlueScreen-Panel-Rail.blp` |
| 2 | `0x3` | `UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-MetalPieces-Expansion.blp` |
| 3 | `0x3` | `UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-MetalBorder-Expansion.blp` |
| 4 | `0x1` | `Textures\GlueScreen-Panel-BNetChannel.blp` |
| 5 | `0x0` | `UI\Glues\SpriteLayers\Expansion\GlueScreen-Panel-ExpansionDetail.blp` |
| 6 | `0x0` | `Textures\GlueScreen-Panel-GearRail.blp` |
| 7 | `0x3` | `Textures\GlueScreen-Panel-MetalPieces.blp` |

`MetalPieces`, `MetalBorder` and `ExpansionDetail` are the plate; `Rail`,
`GearRail` and `BNetChannel` are the rails and the channel strip. Flags `0x3`
(wrap both axes) dominate — the same wrapping the scene fix already handles.

The `TEXS` records begin with a 4-byte `slot` then the path, so the path is at
**+4**, not +0 (`pcx-mdl::Texture` reads it there; a throwaway reader that took
`rec[..260]` got an empty string, which is what made the paths look blank on the
first pass).

The panel's sequence inventory includes `MainMenu Birth/Stand/Death`,
`RealmSelection …`, `SinglePlayer …`, `Options …`, `Battlenet…` and `Morph`
pairs. MainMenu durations are **1000 / 1000 / 667 ms**; other sequences differ.
This establishes an authored entrance, not the complete native dispatcher.

### The composition keys, and the same edition index

The same `UI\war3skins.txt` carries the **sprite-layer composition** — every layer
of the glue screen, keyed by the same `_V%d` edition index as the logo:

```text
GlueSpriteLayerBackground_V0=UI\Glues\MainMenu\MainMenu3D\MainMenu3D.mdl
GlueSpriteLayerBackground_V1=UI\Glues\MainMenu\MainMenu3D_exp\MainMenu3D_exp.mdl
GlueSpriteLayerBackground_V1Beta=UI\Glues\MainMenu\MainMenu3D_Beta\MainMenu3D_Beta.mdl
GlueSpriteLayerTopLeft_V0=UI\Glues\SpriteLayers\TopLeftPanel.mdl
GlueSpriteLayerTopLeft_V1=UI\Glues\SpriteLayers\Expansion\TopLeftPanel-Expansion.mdl
GlueSpriteLayerTopLeft_V1Beta=UI\Glues\SpriteLayers\TopLeftPanel.mdl
GlueSpriteLayerTopRight_V0=UI\Glues\SpriteLayers\TopRightPanel.mdl
GlueSpriteLayerTopRight_V1=UI\Glues\SpriteLayers\Expansion\TopRightPanel-Expansion.mdl
GlueSpriteLayerTopRight_V1Beta=UI\Glues\SpriteLayers\TopRightPanel.mdl
GlueSpriteLayerCenter_V0=UI\Glues\BattleNet\BattlenetChatGlue\CenterPanel.mdl
GlueSpriteLayerCenter_V1=UI\Glues\BattleNet\BattlenetChatGlue\Expansion\CenterPanel-Expansion.mdl
GlueSpriteLayerCenter_V1Beta=UI\Glues\BattleNet\BattlenetChatGlue\CenterPanel.mdl
```

This settles two things at once:

- **The scene substitution is proven, not inferred.**
  `GlueSpriteLayerBackground_V1` names `MainMenu3D_exp` — exactly the model the
  scene note identified. The runtime is *told* the expansion background, by key,
  with no suffix concatenation.
- **The panel's selection is answered.**
  `GlueSpriteLayerTopRight_V1` names
  `UI\Glues\SpriteLayers\Expansion\TopRightPanel-Expansion.mdl` — precisely the
  panel parsed in §2. The expansion panel is selected by the **same edition
  index**, `GlueSpriteLayerTopRight_V%d`, and the mechanism is identical to the
  logo's. `V1Beta` falls back to the base panel, so only `V1` is the expansion
  art.

`GlueSpriteLayerCenter` is the **Battle.net chat** center panel
(`BattlenetChatGlue\CenterPanel`), a separate layer from the menu's.

Two install-level facts about the block: `War3.mpq` and `War3x.mpq` carry the
**same** table (byte-identical, §5). `War3xLocal.mpq`'s copy is a different file
(200 bytes larger, different digest) but its `GlueSpriteLayer` block — and its
`MainMenuLogo` block — are identical, shifted down by four lines. So the edition
index controls these lookups. The static trace does not establish its live
value in a running 1.29b client.

The `.mdl`/`.mdx` gap from §1 applies to every one of these keys: they name
`.mdl`, the archives ship `.mdx`.

## 3. Reproducible absences

These were looked for and are **not present**; recording them stops the next pass
re-searching the same ground.

- **`MainMenuButtonsTop` / `MainMenuButtonsBottom` / `MainMenuBorder` do not
  exist.** No `Frame "SPRITE"` of those names in any dumped FDF, and no such
  asset in the `War3x`/`War3` listfiles. The expansion FDF's only `SPRITE`s are
  `WarCraftIIILogo` (art commented out) and two progress bars. The panel is not
  reached through an FDF sprite name.
- **`MainMenuButton.mdl` does not exist**, though `UI\war3skins.txt` names it.
  Both `.mdl` and `.mdx` spellings were checked across all five local archives
  and are absent; see `menu-sprite-rendering.md`. Whether the runtime tolerates
  this dangling key or it is vestigial is not established.
- **There is no `MainMenu.fdf` in `War3.mpq`.** The only copy is in `War3x.mpq`
  (the RoC install has none), which is consistent with one FDF serving both
  editions via the skin key.
- The scene's own textures contain no plate: `MainMenu3D_Exp`'s 49 textures are
  terrain, sky, water and elements, with `Chains_silver.blp` the only UI-like one.
  This texture name does not identify which layer produced any pictured chain.
  The separate `SpriteLayers` model contains the panel texture family above.

## 4. Sprite placement, as traced natively

With default camera `−1`, unit model scale and zero global glue bias, the
original sprite projection trace gives the logo quad placement

```text
X = 0.13 + vertexX
Y = 0.5285 + vertexY
```

i.e. the FDF anchor as a **coordinate**, plus the model's own vertex offsets —
**not** a fit to any screenshot. This is the origin question the module
deliberately left open; the anchor's `0.13` is the `LEFT` term, and the vertical
term is a bottom-anchored `0.5285`, so the FDF's `0.04` is not itself the final
offset. Placement remains the caller's, now against a traced rule rather than a
guess.

## 5. Provenance

| file | bytes | sha256 (first 24) |
| --- | --- | --- |
| `War3\UI\war3skins.txt` | 44,153 | `7b685b0d2c1129d6…` |
| `War3x\UI\war3skins.txt` | 44,153 | `7b685b0d2c1129d6…` (identical to War3) |
| `War3xLocal\UI\war3skins.txt` | 44,353 | `476a485dc692a331…` |
| `War3\UI\FrameDef\FrameDef.toc` | 2,763 | `e3b79847814f2117…` |
| `War3x\UI\FrameDef\FrameDef.toc` | 2,763 | `e3b79847814f2117…` (identical) |
| `War3\…\WarCraftIIILogo\WarCraftIIILogo.mdx` | 1,820 | `97f400788363da5e…` |
| `War3x\…\WarCraftIIILogo_exp\WarCraftIIILogo_exp.mdx` | 1,820 | `578245c64bc103b4…` |
| `War3x\…\WarCraftIIILogo_exp\WarCraftIIILogo_expbeta.mdx` | 2,552 | `d25e5618ffaa5ceb…` |
| `War3x\UI\Glues\SpriteLayers\TopRightPanel.mdx` | 319,376 | `06bcbb6d578f86b0…` |
| `War3x\…\SpriteLayers\Expansion\TopRightPanel-Expansion.mdx` | 339,976 | `b07996b34b7ddfd1…` |
| `War3x\…\SpriteLayers\Expansion\TopLeftPanel-Expansion.mdx` | 360,016 | `707a40bf2af86c7f…` |
| `War3x\…\Expansion\GlueScreen-Panel-MetalBorder-Expansion.blp` | 12,106 | `2238da1f1f726ffe…` |
| `War3x\…\Expansion\GlueScreen-Panel-MetalPieces-Expansion.blp` | 175,942 | `f4f825f8b997b93c…` |
| `War3x\…\Expansion\GlueScreen-Panel-ExpansionDetail.blp` | 175,942 | `6796c3a729426d2d…` |
| `War3x\…\Expansion\GlueScreen-Panel-Rail.blp` | 6,650 | `ba2a918a8328cd64…` |
| `War3x\…\Expansion\GlueScreen-Panel-GearRail.blp` | 23,026 | `4bfd9e745cdde47b…` |

`MetalPieces-Expansion.blp` and `ExpansionDetail.blp` share a size but are **not**
identical (different digests).

## 6. Open, and next probes

- Subsequent probes decoded the material map and retained `GEOA` geoset IDs:
  51 records address a subset of 57 geosets, so positional pairing is incorrect.
  The hierarchy is complete: 65 bones plus four event objects, no helpers, and
  no bone parent outside the bone set. See `menu-sprite-rendering.md` for the
  reproducible probe and its interpolation limits. Browser panel rendering and
  native quaternion interpolation remain unverified.
- `TopLeftPanel-Expansion.mdx` (360,016 bytes) is the left-hand sprite layer;
  `menu_sprites.rs` loads it beside the right one.
- **Panel selection: answered** — `GlueSpriteLayerTopRight_V1` (§2). **Scene
  selection: answered** — `GlueSpriteLayerBackground_V1` (§2). Both `_V%d`.
- The dangling `MainMenuButton` key: confirm against a 1.28c install whether it
  ever had a file.
