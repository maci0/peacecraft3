# Executed local text messages

The AI-common [diagnostic text family](campaign-ai-debug-text.md) has checked
silent handlers in the selected release client. Those calls preserve the local
message state described here.

`ObjectHost` optionally admits `DisplayTimedTextToPlayer` and
`ClearTextMessages` from the same declaration source as `CreateTimer` and
`Player`. The production campaign startup attaches this state before script
initialization. Missing declarations remain unsupported; incompatible declarations
refuse the candidate without replacing the published session.

Timed text validates an existing player identity and the explicitly selected
local-client input. A different valid player receives no local message. There is
no inferred player slot, adopted handle, wall clock or default duration. Finite
nonnegative durations use the admitted logical game clock, with visibility over
`[start, deadline)`. Invalid arguments and exhausted caller budgets preserve the
previous messages. Clearing removes the retained messages and advances the same
checked revision. Expired messages release their live-record and aggregate-byte
budget on the next insertion. The application's limits are resource policy,
not recovered original message limits.

The campaign startup derives the message frame geometry from the selected client
image during map preparation, retains it on the published `ObjectHost`, and refuses
a second admission. The renderer consumes that host's admitted layout on the
published campaign UI camera; it does not read the launcher or re-derive geometry.
It reads `UI\\MiscUI.txt` / `FontHeights/WorldFrameUnitMessage`, then
`UI\\war3skins.txt` / `Default/MessageFont`, and resolves the resulting font through
the reader belonging to the session's binding receipts. Each read is bounded and
its receipt retained. Missing, malformed or unsupported inputs produce an explicit
consumer error. Session replacement removes the old text and font, and the next
root is painted only from the replacement host's layout. Changing the launcher
cannot rebind an already published session. Clearing or logical expiration removes
the visible text; menu suspension hides it. Text colors and line breaks use the
existing HUD rich-text parser.

## Evidence and limits

Shipped-byte evidence: the selected `MiscUI.txt` describes these heights as frame
units for programmatically created UI, and identifies `WorldFrameUnitMessage` as
trigger-based in-game dialog. The skin table names the font file. These values
are parsed, not copied into constants.

Static native evidence: in the selected 1.29.2.9231 executable, the real
`DisplayTimedTextToPlayer` registration reaches `0x48fbe0`, which resolves the
player, checks `0x4be2a0`, obtains the supplied text and forwards the real x/y and
duration to `0x5f4860` with an all-ones color word. That consumer addresses the UI
message frame at `+0x428`. Its initializer at `0x5e85ac` names
`WorldFrameUnitMessage` and then `MessageFont`. `ClearTextMessages` at `0x48a9b0`
reaches the UI clear routine `0x5eb380`. These are bounded static reads, not a new
original-game run or proof of every branch. Private source identity, windows,
checks and original-map-prefix receipts are under
`.scratch/campaign-message-root-20261019/`.

The placed box is derived from the same image, not from copied measurements.
`pcx_play::message_layout::Rules` validates the reviewed image hash, then bounded-reads
the constructor at `0x5e7ca5`-`0x5e863c`, the dynamic consumer at `0x5f4860`, and the
UI-to-normalized conversion at `0xab297d`/`0xab29a0`. It reads the skin keys
(`SimpleConsole`, `WorldFrameMessage`, `WorldFrameUnitMessage`), the frame fields
(`+0x468`, `+0x424`, `+0x428`), both anchor pairs (bottom-centre to top-centre),
the console `SetAllPoints` call target `0x873750`, the unit `SetPoint` target
`0x873a10`, the initializer widths, heights and offsets, and the `.rdata` constants
`0.8`, `0.6`, `0.0005`, `0.098125` and `0.452`. It refuses a mismatched hash,
instruction, key, field, anchor pair, call target or initializer/consumer relation.

Native placement for script `(x, y)` is then, in logical UI units with positive Y
upwards and negative inputs clamped at zero:

- unit left `= message_centre_x + x * (0.8 - message_right + 0.098125) - 0.098125 - 0.25`
- unit bottom `= message_top + y * (0.452 - message_top - 0.0005) + 0.0005`
- unit size `0.5 x 0.25`, wrapping at `0.5` logical width

At `(0, 0)` this reproduces the constructor's own `WorldFrameUnitMessage`
rectangle: left `0.051875`, bottom `0.2305`, width `0.5`, height `0.25` in a
`0.8 x 0.6` UI. The renderer converts that rectangle to percentages of the
campaign view and stacks rich-text lines upward from its bottom edge. No
fallback rectangle is invented when the derivation fails; the consumer reports
the error and keeps the previous frame.

Reconstruction choices remain visible: text is stacked upward from the latest
executed normalized position; the native single-frame position therefore applies
to the whole retained block. Per-message historical positions, fade-out timing,
chat-history integration and an original rendered comparison remain open.
Automatic-duration `DisplayTextToPlayer` and sender-prefixed
`DisplayTimedTextFromPlayer` remain unsupported. Invalid/unknown player handling
is a safety refusal, not a claim about native null-handle behavior.

Synthetic Host tests cover local filtering, actual VM argument conversion,
logical-time chunking, clearing, capacity, failed arguments and declaration
qualification. Factory tests cover the published session and rejected
replacement. Synthetic host tests prove that two published hosts keep their own
decoder-derived layouts and that a second admission is refused. A selected-install
component test injects diagnostic timed text through each published host's own
`GetLocalPlayer` and `DisplayTimedTextToPlayer` natives, then covers launcher
replacement and a lookalike session without that session's publication. Its
synthetic stimulus is not mission evidence; it is ignored because it requires
explicit local inputs. Renderer checks cover rich-text spans and relative sizing; the
explicit Vulkan test covers colors and clearing through 4K. Native/WASM builds
and actual WASI/Cargo-WASM execution are recorded separately in the private
verification receipt. These checks do not establish chapter playability,
original/native/WASM visual parity or complete game determinism.

## Line endings in map strings (2026-10-27)

`war3map.wts` stores multi-line strings with CR LF (Prologue02 `TRIGSTR_030`, the
MAIN QUEST message: five lines, 0x0D 0x0A between them). `paint` in
`campaign_messages.rs` replaces `\r\n` with `\n` before the rich-text parse. Before
the change each break rendered twice, so the quest text was spaced two lines apart
(58 px against the original client's 24 px at 1280x960). Pinned by
`crlf_in_message_text_is_one_line_break`. A lone CR is not touched.

## Paired comparison, Prologue02 (2026-10-27)

The recreation capture steps the script clock to the original client's schedule,
derived from its action log: Escape skip at 11.361 s and capture at 23.743 s after
the Return that started the map (`PCX_PAIRED_SKIP_AT`, `PCX_PAIRED_CAPTURE_AT`
read by `mounted_campaign_rendering_state`; runner and receipts in
`.scratch/integration-20261027/`). That treats the Wine client's game clock as real
time from the Return, which was not measured.

Observed in `captures-15` against `matched-skip-control.png`:

- The quest message block sits in the same screen area; text height differs by
  about 1.1x in glyph width (`Establish a Base`: 157 px stock, 175 px recreation).
  Glyph metrics, margins and wrapping are not qualified (line pitch is now equal).
- The recreation already shows the narrator transmission over the console; the
  original does not yet at the same nominal time, so the two clocks or the
  trigger wait are not aligned. This is a timing difference, not a layout claim.

Line pitch: the authored `WorldFrameUnitMessage` height (0.015 frame units, 24 px
at 1280x960) is the line pitch, as for the other HUD fonts, so the line and each
span carry `LineHeight::RelativeToFont(1.0)`. Bevy's default of 1.2 had spaced the
lines 28.7 px apart. Measured in `captures-18`: five rows at 150, 174, 198, 222
and 246 px, 24 px apart, equal to the original capture's pitch. Glyph width is
still about 1.1x the original (the em size is taken as the height; see
"Glyph size" below), and the block sits lower than the
original's.

## Glyph size: what the executable and font establish (2026-10-27)

Stock `Establish a Base` is 155 px wide at 1280x960 (white-pixel ink bounds; the
recreation measures 171 px by the same method), so the stock glyphs are about 0.9x
the recreation's. Linear scaling of the font's advances puts the stock em near
21.5 to 22 px, not 24.

Font metrics (`Fonts\FRIZQT__.TTF`, 62,316 bytes, from `War3.mpq`): `head`
unitsPerEm 1000; `hhea` ascent 965, descent -250, lineGap 0; `OS/2` typo and win
values equal those. Cell height is 1.215 em. Neither 1/1.215 (0.823) nor
ascent/(ascent+descent) (0.794) is 0.9, so a cell-height versus em conversion does
not explain the difference. The recreation's em of 24 px (Bevy `font_size` is the
em) already equals the authored height at this resolution.

Static reading of the selected image (SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`) gives the size
path for this font:

- `0x5e85ac..0x5e85ef`: `FontHeights/WorldFrameUnitMessage` and `MessageFont` go to
  `0x88c210(file, height, flags)` with the literal flags `1` (`push 1` at `0x5e85d1`).
  `0x88c210` stores them at string-object `+0x130/+0x134/+0x138`.
- `0x887470` multiplies the height by the object's scale (`+0x60`, set to 1.0 in the
  frame constructor at `0x872234`) and calls `0x9600c0`, which divides by 0.6
  (`.rdata 0xe84160`, the UI height), clamps to at most 1.0 and passes the screen
  fraction on.
- `0x7f0be0`: the fraction is raised to at least 2 px, converted by `0x7e3de0` to
  `round(fraction * H)` (`H` is `[0x114e934]`, written at `0x7e3a0f` from the
  device viewport rect; its runtime value was not read) and capped at 64 (`0x40`).
  The result goes to `FT_Set_Pixel_Sizes` (`0x9f9460` scales by 64 and requests the
  size), so FreeType's em is that integer. The ascent and descent from the face
  record (`+0x46`, `+0x48`) only give the baseline fraction `ascent/(ascent+descent)`
  at `+0x1c0`.
- `0x7f0c47..0x7f0c64`: when flag bit 0 is set and the size exceeds 2, a second size
  `+0xb4 = size - 2` is stored. `0x7eac90` sets the face to `+0xb4` before the glyph
  render call `0x7f1cc0`.

Reading: with `H = 960` the string size is 24, and a flag-1 font rasterizes glyphs at
22 px, 0.917 of 24. That is close to the measured 0.9 and to the 21.5 to 22 px
estimate. Not applied, because:

- the `-2` is absolute pixels and the recreation sizes every HUD font in `Vh`, so the
  rule needs the output pixel height at paint and a resize path; the other HUD fonts
  would need their own flags read first (the FDF `FontFlags` to this argument
  mapping is not decoded);
- `H` at run time and whether the layout advances use 22 or 24 (the advance scale
  at `+0x1d0` is read while the face is at 24) are not read;
- it is a static reading of one image, not an original-client run.

No code or constant changed for glyph size. A debugger read of `+0xb0`, `+0xb4` and
`[0x114e934]` for this string in the original client would settle it.
