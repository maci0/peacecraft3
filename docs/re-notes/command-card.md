# Command card

The command card is still being implemented. The geometry reader is separate
from the renderer and does not mount buttons yet.

## Selected-client evidence

The selected Legacy executable has VERSIONINFO `1.29.2.9231` and SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Static reads use that complete local image, not the older compatibility DLL or
the IDA launcher decompilations. Extracted inputs and disassembly stay local in
the ignored `.scratch/command-card-source/` directory.

`UI\FrameDef\UI\ConsoleUI.fdf` in the selected base archive defines console
textures, without command-button frames. The client supplies the command grid:

- MSVC RTTI identifies `CCommandBar` through type descriptor `0x10f8c98`,
  complete-object locator `0xff1060` and vtable `0xe95c60`.
- The constructor at `0x61d130` installs that vtable at `0x61d173`.
  The gameplay UI creates it at `0x5e805d`, passing its console parent from
  field `+0x468`. That parent is loaded through the skin key pushed at
  `0x5e7ca5` and retained at `0x5e7cbd`.
- The constructor passes the grid dimensions at `0x61d1c4`/`0x61d1c6` to
  `0x88d050`; the two creation loops compare the same dimensions at
  `0x61d357`/`0x61d364`. Allocation and loop operands must agree.
- Width and height operands at `0x61d1d6`/`0x61d1e5` go to setters
  `0x873b40`/`0x873830`, which write frame width/height at `+0x58`/`+0x5c`.
- The anchor call at `0x61d20c` supplies the bottom-right point on both frames,
  with X/Y operands at `0x61d200`/`0x61d1f8`. Other anchor pairs are unsupported
  by this reader rather than treated as the same layout.
- Each grid child installs the `CCommandButton` vtable `0xe9528c` at
  `0x61d2a9`, then reads its own width and height from `0x61d2e1`/`0x61d2f0`.
- `0x88c970` stores the child at the row/column pair supplied by the loops.
  The grid's layout body at `0x88cb50` divides its extent by the row/column
  dimensions and anchors each child at its top-left, with negative Y steps.
  Its four inset fields are initialized to zero by `0x88c7a0`; the command
  constructor does not set different insets.

`pcx_play::command_layout::Rules` reads the operands from the supplied PE at
runtime, including the parent skin key and grid's default insets, retains input
identity and hash, and derives slot rectangles in logical
UI coordinates. The viewport references are the same selected-client reads as
the existing inventory reader. Instruction locations identify this reviewed
build; they are not copied geometry values. Synthetic fixtures use different
dimensions and operands.

The focused suite passes eight tests, including the explicitly enabled local
selected-client gate. Strict `pcx-play` all-target Clippy and the WASM library
check pass on the same frozen inputs. Receipts are retained locally in
`.scratch/command-card-source/layout-results.json`. These checks prove the reader
and its refusal paths, not displayed buttons.

## Ability presentation inputs

The selected `HumanAbilityFunc.txt` records for `AHhb` and `AHwe` carry `Art`,
`Buttonpos` and `Order`. `HumanAbilityStrings.txt` carries `Hotkey` and ranked
`Tip`/`UberTip` lists. The existing object catalog preserves each assignment,
its parse errors, archive and edition/version. Rendering must consume those
records, including source token substitutions, rather than supplying icon paths,
button positions or tooltip text from code. Map profile overrides must not be
silently replaced with stock presentation.

## Remaining work

The geometry reader alone does not establish native/rendered parity. The
command-card renderer, source-qualified icon/sampler reads, learned-rank view,
tooltips, cooldown/availability presentation, pointer consumption and dispatch
through the existing accepted-command log remain to be connected and tested.
The existing Holy Light and Water Elemental hotkeys remain their input paths.
Native command ordering, disabled states and the rendered console relationship
still need runtime comparison.
