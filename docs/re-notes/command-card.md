# Command card

The command card is still being implemented. Learned Holy Light and Water
Elemental buttons now have a receipt-bound renderer and input consumer;
tooltips and readiness presentation remain unfinished.

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

The shared `ability_profile` reader now applies scalar W3A profile modifications
through the loaded AbilityMetaData mapping. Last authored modification wins;
empty values remove the stock value. UTF-8, string wire/type, family and metadata
applicability are checked. Unknown fields, multi-slot fields and ranked profile
modifications refuse explicitly. Stock values are not substituted for those
unsupported map records.

Both live hero ability reads and startup's Holy Light model read use that reader.
This keeps a changed `Targetart` path paired with the model whose sequence data
startup admits. Holy Light input now reports profile errors instead of dropping
them. Synthetic checks cover replacement hotkeys and art paths, repeated and
empty modifications, and malformed/indexed refusal. This does not mount the
command card or implement ranked tooltip expansion.

The selected `HumanAbilityFunc.txt` records for `AHhb` and `AHwe` carry `Art`,
`Buttonpos` and `Order`. `HumanAbilityStrings.txt` carries `Hotkey` and ranked
`Tip`/`UberTip` lists. The existing object catalog preserves each assignment,
its parse errors, archive and edition/version. Rendering must consume those
records, including source token substitutions, rather than supplying icon paths,
button positions or tooltip text from code. Map profile overrides must not be
silently replaced with stock presentation.

## Remaining work

The geometry reader alone does not establish native/rendered parity. The button
checkpoint below connects the two implemented learned commands, source icon
reads and sampler admission, pointer consumption and dispatch. Tooltips,
cooldown/readiness presentation and other command types remain unfinished.
Native command ordering, disabled states and the rendered console relationship
still need runtime comparison.

## Learned button data (2026-10-21)

`ObjectHost::ability_button(unit, ability)` now projects the learned rank,
player availability, authored `Art` and metadata-mapped `Buttonpos`. Unlearned
or player-unavailable abilities return no button. This is presentation data,
not a mana, cooldown or target-readiness decision; accepted orders must still
go through the gameplay command path.

The two position components are identified through AbilityMetaData profile
slots, not field fourcc literals or copied coordinates. Each scalar map
modification replaces only its own component. A valid map override does not
require a stock coordinate to exist. Malformed types, nonzero selectors,
negative coordinates and unsupported mapping attributes refuse. Negative
coordinates may have native hiding semantics; no such rule is claimed here.
The caller must pass the position through the selected grid's `slot` bounds
check before drawing it.

Synthetic tests cover component mapping, map replacement, absent stock values,
malformed inputs and learned-rank projection. The explicit original gate
`ability_profile::tests::original_button_positions_agree_with_authored_profiles_and_client_grid`
compares both implemented Human hero spells with their supplied profile text
and checks the selected client grid. It requires `PCX_NATIVE_CHAPTER_INSTALL`
and `PCX_WAR3_EXE`; no measured coordinates are embedded in the test.

## Button rendering and dispatch (2026-10-22)

`campaign_command_card` mounts the two implemented learned abilities under the
console frame selected by the skin. The mounted HUD must match the exact
published context and binding revision. A new session removes the old root and
only its owned image assets. Geometry remains relative to the source viewport.
Duplicate authored slots refuse rather than inventing button priority.

The selected executable links the command-bar creation (`0x5e805d`) through
the child-button constructor (`0x61d24d`). The command-grid lookup at `0x5eafb6`
feeds the art setter at `0x5eafd5`, its wrapper at `0x5d3caf`, and texture
loader at `0x5d3e60`. The GameUI field written at `0x5e8074` must match the
field read at `0x5eafab`; the reader checks their relationship. Texture
addressing comes from flags at `0x8750db` and `0x875141`, interpreted through
the loader mask at `0x887ce5`. Both paths must agree. These static links qualify
the addressing input, not native filtering or rendered equivalence.

Icons use bounded, receipt-carrying reads and decoding. Successful and failed
image reads are cached per session. Stable selection, learned ranks and
player availability reuse presentation data rather than rebuilding nodes each
frame. Button presses consume world pointer input. Activation rechecks the
session, local ownership, selection, controls and current command availability.
Targeted spells arm the existing target-click path; immediate spells use the
same native and accepted-command log as their hotkeys.

Synthetic session/pointer tests and a selected-client geometry test cover
stale witnesses, pointer consumption, preflight of failed replacement and
owned-asset teardown. The image assets in that teardown test are synthetic;
it does not establish displayed original icon fidelity. The explicit Human01
input gate uses the real chapter startup, with operator setup to enable control
and select/learn Holy Light. It is component input evidence, not progression
through the authored cinematic.

Ranked tooltips and substitutions, cooldown/mana presentation, disabled and
pressed art, remaining commands, and native/rendered comparison remain open.

Private-target validation: 1,086 core tests pass (31 ignored), the selected
layout gate passes, and 639 feature-enabled library/bin app tests pass (50
ignored). The explicit Human01 input gate passes after the system-parameter
cleanup. Strict all-target Clippy with `profile-gpu,test-fixtures`, affected
WASM checks and touched-file formatting pass. Logs are retained locally under
`.scratch/command-card-source/card-validation` and `card-validation-fixed`;
the first chain's Clippy failure is preserved, with its corrected rerun in
`card-clippy-2.log`. No original asset or screenshot is committed.
