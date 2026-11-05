# In-game Help and Tips

The selected Legacy 1.29.2.9231 executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
This is static evidence, not observed runtime or rendered acceptance.

## Shipped definitions

`UI\FrameDef\UI\EscMenuMainPanel.fdf` already contains Help and Tips pages,
text areas and their buttons. Their sizes and anchors belong to those frames;
no separate guessed page composition is needed. `EscMenuTemplates.fdf` supplies
text-area line height, line gap, inset, scrollbar and backdrop relationships.
Its FrameFont record is commented out. The recreation currently rejects TEXTAREA
in the shared control-item builder, so simply adding another escape-menu page
would fail admission rather than produce a working Help/Tips dialog.

The selected local archive supplies `UI\HelpStrings.txt` as plain rich text and
`UI\TipStrings.txt` as sectioned, numbered tip strings with a declared count.
Those contents must be read at runtime; they must not be copied into source.

## Native content binding

- `0x630d14` resolves HelpTextArea; `0x630d6b` passes the HelpStrings path to the
  file loader. On successful non-null load, `0x630d83..0x630d8a` passes those
  bytes to the text-area setter `0x885c00`, then frees the temporary file buffer.
  This establishes the file-to-widget relationship, beyond filename inference.
- `0x630c71` resolves TipsTextArea. The initializer reads TipCount and initializes
  the current section/tip positions before calling `0x65a690`.
- `0x65a690` formats the displayed tip key from current index plus one, selects
  a section through the pointer table at `0xe945dc`, reads the tip, and passes
  the result to the same text-area setter. Do not assume that a single section
  in one supplied file proves the complete section-navigation policy.

Local evidence is retained in `.scratch/menu-help-tips-20261105/`:
`source-identity.json`, corrected string references, `menu-bindings.asm` and
`text-area-tip-body.asm`. The initial incorrect-address search and rejected
command are retained separately and do not support these findings.

## Remaining implementation and qualification

Use the source-derived native text-area font/style binding when projecting
content; the live check below qualifies the default-slot branch for the selected
RoC source. The commented template record does not authorize an invented font. Implement
source-derived text-area geometry, rich text, clipping and scrollbar behavior,
then connect Help close/back and Tips previous/next/close through the existing
session-bound modal input. Verify boundary/section behavior rather than guessing
wrap or random initial-tip rules. Preserve pause and underlying input isolation.

Synthetic parser/layout tests and explicit original-input gates are necessary,
but rendered full-frame original comparisons are still required. These findings
do not enable either page or establish pixel parity.

## Font inheritance and row construction (2026-11-05)

Further static inspection narrows the missing font binding:

- The native TEXTAREA registration table pairs the string at `0xec4b50` with
  factory `0x856030`; that factory creates the descriptor through `0x869530`.
  The descriptor parses line-height, gap and inset fields independently of font.
- Runtime text-area construction is `0x884e20`. Its row layout at `0x885320`
  creates font-string rows through `0x87d5f0`, sets their height from the
  text-area line-height field, and positions rows using line height plus gap.
  It applies a constructor-provided shadow, rather than requiring a shadow
  record on the commented template font line. These operands must be parsed
  from the selected executable before use, never copied into constants.
- Wrapping in `0x885c20` gets its font through `0x86cf80`: it follows frame
  parent pointers until a non-null font is found, then falls back through
  `0x959b90(0)` if no parent supplies one. Height accessor `0x86cfb0` separately
  checks explicit height, a local font object's height, parent inheritance,
  then the default height slot. Thus wrapping height, row height and font
  selection are separate inputs; the FDF line-height record cannot stand in
  for all three.
- Default font initialization is not the final font identity: `0x42eecc..0x42ef30`
  obtains `Misc/FontTexture`, resolves its path, and overrides both default font
  slots through `0x959bf0`. The selected `UI\MiscUI.txt` supplies that key.
  This must retain the selected configuration/source precedence; the executable
  bootstrap filename does not justify a hard-coded MasterFont binding.

At the end of this static pass, the actual Help/Tips ancestor font and height
chain still needed qualification; the live check below resolves it for RoC.
The source-derived default slot is a fallback, not proof that the runtime uses it
for these frames. Remaining tip section/boundary policy and rendered scrollbars
are also open. No page was enabled by this static pass.

Additional local evidence: `textarea-layout-objdump.asm`,
`textarea-populate.asm`, `textarea-font-accessors.asm`,
`textarea-height-accessor.asm`, `default-font-override-caller.asm` and
`default-font-override-setter.asm`. Rizin's formatted output can substitute
relocation labels for actual call destinations; structured instruction decoding,
raw relative-call bytes and independent objdump agree on the relevant targets.
Formatted relocation labels alone are not evidence of a different callee.

## Isolated live font and navigation check (2026-11-05)

A fresh private Wine/Xvfb run of the selected RoC client entered the unchanged
Prologue02 map, skipped its cinematic, opened Game Menu and Help, and traversed
Tips. The display was private, the PulseAudio sink was verified as a null sink,
the container had no network or host display/audio/device mounts. All original
assets, extracted map bytes, captures and memory-read outputs remain local.

A bounded read-only probe identified the Help/Tips text areas by their loaded
source text before layout, then retained their parent chains after Help opened.
Both areas and every recorded ancestor had null explicit font and zero explicit
height. Their stable object identities connected the pre-layout and open-page
reads. Default font slots were populated and their live heights matched the
source table. This qualifies the default-slot branch for this selected RoC run;
it does not prove all editions or custom frame hierarchies use that branch.
The probe's vtable, parent/font/height/content offsets and default-slot addresses
were derived from checked instruction bytes and supplied as explicit inputs.
No process memory was written.

Full-frame captures establish these navigation observations for the supplied
single-section tip file: a fresh game UI starts at its first authored tip; Back
from first wraps to last; Next from last wraps to first; advancing and closing
then reopening retains the selected tip. Help displays the authored text with
its scrollbar. Native wheel increments and all scrollbar interactions still
need measurement. Multiple tip sections were not exercised.

Font path resolution `0x7af9e0` obtains both directory and path-format strings
from executable operands; these must be read when constructing the configured
FontTexture path. Default-height conversion `0xab2a90` uses executable axis
factors, distinct from the FDF text-area row height. Do not substitute either a
measured glyph size or the row height for that conversion.

Evidence and SHA-256 receipts: `.scratch/menu-help-tips-live-20261105/` and the
private `.scratch/menu-help-tips-reference-20261105/captures/`. The reference is
stopped after capture; its final state is retained in `stopped-status.json`.
These are original-client runtime observations, not a paired recreation capture,
not tick identity, and not menu pixel acceptance. Implementation and rendered
comparison remain open; the earlier ancestor-font question is now qualified
for this source/edition rather than an unknown.
