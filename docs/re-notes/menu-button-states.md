# Main-menu button states, as authored

Read-only record of the exact authored state art for the Warcraft III main-menu
buttons: which template each button uses, the art path per state, the highlight
modes, and the numerics (insets, corner sizes, pushed offsets). Nothing was
rendered or extracted to write this; every line below is quoted from shipped FDFs.

Sources (both dumped from `War3x.mpq`):

- `UI\FrameDef\Glue\MainMenu.fdf` → `.scratch` copy, 646 lines, the expansion
  menu's own frame tree;
- `UI\FrameDef\Glue\StandardTemplates.fdf` → `.scratch/standard-templates.fdf`,
  1,185 lines, the shared templates.

## Which template each button uses

From the expansion `MainMenu.fdf`:

| Button | Template |
| --- | --- |
| `SinglePlayerButton`, `BattleNetButton`, `LocalAreaNetworkButton`, `OptionsButton`, `CreditsButton`, `ExitButton` | `StandardButtonTemplate` |
| `EditionButton`, `RealmButton` | `StandardIconicButtonTemplate` |
| `RealmSelectOKButton`, `RealmSelectCancelButton` | `StandardSmallButtonTemplate` |
| `PatchButtonBackdrop`, `ConnectButtonBackdrop`, `MigrationButtonLargeSingleBackdrop` | `StandardMenuSingle{Button,LargeSingle}BaseBackdrop` |
| `MigrationButtonSmall{Right,Left}Backdrop` | `StandardMenuSingleSmallButtonBaseBackdrop` |

Each of the six main buttons sits **inside** a parent `BACKDROP` that inherits
`StandardMenuButtonBaseBackdrop`, e.g. `SinglePlayerBackdrop`, `BattleNetBackdrop`
(lines 81, 133, 184, 199, 214, 229). So there are **two nested plates**: an outer
border plate, and the button's own state background inside it.

### The outer plate

```text
Frame "BACKDROP" "StandardMenuButtonBaseBackdrop" {
    Width 0.256,
    Height 0.064,
    BackdropBlendAll,
    BackdropBackground  "UI\Widgets\Glues\GlueScreen-Button1-Border.blp",
}
```

`BackdropBlendAll` means this plate is drawn with alpha blending rather than
opaquely — the border is a cut-out, not a solid rectangle. The small variant
(`StandardMenuSmallButtonBaseBackdrop`, used by the Realm OK/Cancel) is the same
sizes with `GlueScreen-Button1-SmallBorder.blp`.

## `StandardButtonTemplate` — the state machine

```text
Frame "TEXTBUTTON" "StandardButtonTemplate" {
    Height 0.031,
    Width 0.179,
    ControlStyle "AUTOTRACK|HIGHLIGHTONMOUSEOVER",
    ButtonPushedTextOffset -0.002f -0.003f,

    ControlBackdrop           "ButtonBackdropTemplate"         → StandardButtonBackdropTemplate
    ControlPushedBackdrop     "ButtonPushedBackdropTemplate"   → StandardButtonPushedBackdropTemplate
    ControlDisabledBackdrop   "ButtonDisabledBackdropTemplate" → StandardButtonDisabledBackdropTemplate
//  ControlFocusHighlight     …commented out…
    ControlMouseOverHighlight "ButtonMouseOverHighlightTemplate" → StandardButtonMouseOverHighlightTemplate
}
```

Two things worth noting because they are easy to get wrong:

- The button's own box is **0.179 × 0.031**, smaller than the 0.256 × 0.064 outer
  plate it sits in. The button does not fill its parent plate.
- `ButtonPushedTextOffset -0.002 -0.003` shifts the label when pushed. The label
  moves; the plate does not.
- The keyboard **focus** highlight is **commented out** for text buttons — only
  the mouse-over highlight is live. `StandardIconicButtonTemplate` enables both.

### State art

Each state is a `BACKDROP` with the same tiling, corner and inset geometry, and
differs only in its **background** and **edge** files:

| State | Background | Edge |
| --- | --- | --- |
| normal | `UI\Widgets\Glues\GlueScreen-Button1-BackdropBackground.blp` | `…\GlueScreen-Button1-BackdropBorder.blp` |
| pushed | `…\GlueScreen-Button1-BackdropBackground.blp` (same) | `…\GlueScreen-Button1-BackdropBorder-Down.blp` |
| disabled | `…\GlueScreen-Button1-BackdropBackground-Disabled.blp` | `…\GlueScreen-Button1-BackdropBorder-Disabled.blp` |
| disabled+pushed | `…\GlueScreen-Button1-BackdropBackground-Disabled.blp` (same as disabled) | `…\GlueScreen-Button1-BackdropBorder-DisabledDown.blp` |

So **pushed changes only the edge**, and the disabled family swaps both. The four
edges — `…Border`, `…Border-Down`, `…Border-Disabled`, `…Border-DisabledDown` —
are the complete set the main menu needs.

The geometry, identical across all four (lines 124–162):

```text
BackdropTileBackground,
BackdropCornerFlags "UL|UR|BL|BR|T|L|B|R",
BackdropCornerSize  0.016,
BackdropBackgroundSize  0.032,
BackdropBackgroundInsets 0.004 0.004 0.004 0.004,
```

`BackdropTileBackground` + `BackdropBackgroundSize 0.032` means the background is
a **0.032 tile repeated** to fill, not one stretched image; the edge is drawn
from eight corner/edge pieces of `BackdropCornerSize 0.016`, with the content
inset `0.004` on all four sides. `StandardSmallButton*` uses `0.012` corners and
`0.003` insets instead.

### Highlight (hover) and focus

```text
Frame "HIGHLIGHT" "StandardButtonMouseOverHighlightTemplate" {
    HighlightType "FILETEXTURE",
    HighlightAlphaFile "UI\Widgets\BattleNet\bnet-button01-highlight-mouse.blp",
    HighlightAlphaMode "ADD",
}
Frame "HIGHLIGHT" "StandardButtonFocusHighlightTemplate" {
    HighlightType "FILETEXTURE",
    HighlightAlphaFile "UI\Widgets\Glues\GlueScreen-Button-KeyboardHighlight.blp",
    HighlightAlphaMode "ADD",
}
```

Hover is an **additive** overlay of `bnet-button01-highlight-mouse.blp` — a glow
added on top of the button, not a replacement backdrop. Focus uses
`GlueScreen-Button-KeyboardHighlight.blp`, also additive. `ControlStyle
"AUTOTRACK|HIGHLIGHTONMOUSEOVER"` is what enables the hover highlight; iconic
buttons add `HIGHLIGHTONFOCUS`.

### Text states

```text
Frame "TEXT" "StandardButtonTextTemplate" {
    FrameFont "MasterFont", 0.013, "",
    FontJustificationH JUSTIFYCENTER,
    FontJustificationV JUSTIFYMIDDLE,
    FontFlags "FIXEDSIZE",
    FontColor          0.99 0.827 0.0705 1.0,   // gold
    FontHighlightColor 1.0  1.0    1.0    1.0,  // white on hover
    FontDisabledColor  0.5  0.5    0.5    1.0,  // grey
    FontShadowColor    0.0  0.0    0.0    0.9,
    FontShadowOffset   0.001 -0.001,
}
```

The label is centred both ways, fixed-size, gold normally, **white** on hover,
grey when disabled, with a 0.9-black shadow offset `(0.001, -0.001)`.

## `StandardIconicButtonTemplate`

The two iconic buttons are **0.031 × 0.031** squares beside Single Player and
Battle.net, and each overrides its own art in `MainMenu.fdf`:

```text
Frame "GLUEBUTTON" "EditionButton" INHERITS WITHCHILDREN "StandardIconicButtonTemplate" {
    SetPoint TOPRIGHT, "SinglePlayerBackdrop", TOPRIGHT, -0.012, -0.0165,
    ControlBackdrop         "EditionButtonBackdropTemplate" {
        BackdropBackground  "UI\Widgets\Glues\GlueScreen-ROC-EditionButton-up.blp",
        BackdropBackgroundSize  0.031,
        BackdropBackgroundInsets 0.004 0.004 0.004 0.004,
    }
    ControlPushedBackdrop   … "…-ROC-EditionButton-down.blp"
    ControlDisabledBackdrop … "…-ROC-EditionButton-disabled.blp"
}
```

`RealmButton` is the same shape with
`UI\Widgets\BattleNet\bnet-mainmenu-options-{up,down,disabled}.blp`. These are the
only two buttons with a genuine **three-art state set** (up/down/disabled files)
rather than a background+edge pair.

## Not established here

- **The colour of the plate.** These FDFs give file paths and geometry, never a
  tint, so the button background's actual appearance — the familiar blue — lives
  in `GlueScreen-Button1-BackdropBackground.blp` and is **not** verified by this
  note. The path is evidence; the pixels are not.
- **Whether the FDF is the runtime's authority.** These are the shipped
  definitions, read statically. That the 1.29b executable resolves these exact
  states at run time is inference, in the same way the expansion scene's
  selection is: not observed.
- `ButtonPushedTextOffset`'s sign convention (which way the label moves) is the
  FDF's; the rendered direction has not been checked.
