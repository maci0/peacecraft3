# Campaign HUD top bar: Allies, Log and the time-of-day clock

Evidence classes: shipped bytes (S), native binary strings (N), stock-client
capture (C, `matched-skip-control.png`, Prologue02 orc base), recreation choice (R).

## Upper button bar

`UpperButtonBar.fdf` (S) chains four SIMPLEBUTTON frames from the bar's top left:
`UpperButtonBarQuestsButton`, `...MenuButton`, `...AlliesButton`, `...ChatButton`,
each 0.085 wide with `NormalText`, `DisabledText` and `HighlightText` keys. The
earlier code spawned only the first two, so the remaining frames showed the
console art with no label.

`campaign_upper_buttons.rs` now spawns all four from the same FDF path: anchors
and art regions from the FDF, label styles from `UpperButtonBarButtonTextTemplate`
and its disabled/highlight variants, strings from `GlobalStrings.fdf`.

- Allies (`KEY_ALLIES`, `ALLIES`): spawned with `InteractionDisabled`, so it shows
  the authored disabled art region and `UpperButtonBarButtonDisabledTextTemplate`
  (grey 0.4). The enable rule is read from `Warcraft III.exe` (N, disassembly with
  rizin, image base 0x400000), not assumed:
  - `CUpperButtonBar` (vtable `.?AVCUpperButtonBar@@`) enables Allies at 0x0062e440
    only when three tests pass: the dword at `this+0x170` is 0, the function at
    0x005ad700 returns nonzero, and the function at 0x005edaa0 returns 0. The
    setter at 0x0062d650 stores `this+0x170` and then re-enables or disables the same button; its two further calls are relocation targets rendered as `pe_` labels, so they are presumed, not read, to be the same two predicates.
  - 0x005ad700 reads global object 13, field +8, offset 0x2650 (a four character
    tag) and returns 0 when it equals `LOOP` (0x4c4f4f50) or `NONE` (0x4e4f4e45).
    The same function picks the fourth button's label (see below), so Allies is
    enabled exactly when the bar shows Chat.
  - `campaign_upper_buttons.rs` names the tags (`PROVIDER_LOOPBACK`,
    `PROVIDER_NONE`), derives Allies, the Chat/Log choice and the Log action from
    one `networked(provider)` test, and passes `CAMPAIGN_PROVIDER = LOOP`: the
    campaign HUD only serves the local single-player session. Reading `LOOP` as
    the local session is an inference from the name; the code that writes the tag
    is not traced.
  - Open: what sets `this+0x170`, and what 0x005edaa0 tests on the local player
    (it returns a field at +0x78 of a record found through the local player slot). Both only matter for a
    networked session, so neither changes the campaign result.
  C shows Allies disabled in a campaign, which agrees with the rule.
- Fourth button: the FDF names the frame `UpperButtonBarChatButton` with
  `KEY_CHAT`/`CHAT`. At 0x006350af the client calls the same 0x005ad700 test: nonzero
  keeps `KEY_CHAT`/`CHAT`, zero loads `KEY_LOG`/`LOG` (N). The bar substitutes the
  Log strings for a local session. The tooltip code around 0x0063ae98 picks
  `UPPER_BUTTON_CHAT_TIP`/`_UBER` or `UPPER_BUTTON_LOG_TIP`/`_UBER` with the same test.
- Log opens the message log dialog (`UI\FrameDef\UI\LogDialog.fdf`, class
  `CLogDialog` in the client) and F12 (`KEY_LOG`, read from `GlobalStrings.fdf`)
  does the same: see [campaign-upper-controls.md](campaign-upper-controls.md). Allies
  and Chat have no dialog here: Allies is disabled for a local session, so its
  `KEY_ALLIES` key (F11) has nothing to open. `AllianceDialog.fdf` is not built.
  Menu's key (F10) is consumed by the campaign camera's menu toggle, and Quests'
  key (F9) by this module.

Tooltips (`UPPER_BUTTON_*_TIP`/`_UBER`) are not shown for any of the four buttons.

## Time-of-day indicator

`war3skins.txt` (S) keys `TimeOfDayIndicator` per race section, for example
`UI\Console\Orc\OrcUI-TimeIndicator.mdl`. No FDF frame references it, and the
client has a `CTimeOfDayIndicator` class (N). `campaign_time_indicator.rs` reads
the model path from the local player's race section, loads
`OrcUI-TimeIndicator.mdx` and draws it.

Model facts (S, read with `mdlx` and a bone-track probe):

- MODL extent is x 0.337..0.465, y 0.536..0.615, z 0.3736..0.677. Centre
  (0.401, 0.575) of the 0.8 x 0.6 UI frame is the stock clock position (about
  x 640, y 35 of 1280 x 960 in C). The model is therefore authored directly in UI
  frame units, y up. The console frame leaves a gap at x 0.343..0.459
  (`ConsoleUI.fdf` ConsoleTexture02 pieces) that the clock fills.
- Two sequences: `Stand` 0..60000 and `Stand Alternate` 67500..127500, both 60000
  ms. `GeoSphere01` rotates one full turn over `Stand` in 7500 ms steps (eight
  steps, 45 degrees each), and the sun/moon geoset alphas switch at 15000 and
  45000. With 60000 ms equal to 24 hours that is 2500 ms per hour, switching at
  hours 6 and 18, matching dawn and dusk. `Stand Alternate` repeats the same keys
  shifted by 67500 and is not used.
- Texture slots are all fixed paths (six `.blp` files); no replaceable texture.
- Layer shading flags are 33 or 97: unshaded, with `0x40` (no depth test) on the
  opaque disc and the ring. Filter modes: additive glows, one opaque disc, one
  alpha-keyed frame.

Drawing path: the same model pipeline as the unit portrait. Geosets are
expanded per material layer, skinned from the sequence frame, and rendered by an
orthographic camera that frames exactly the MODL extent (z range sets the clip
planes) into a transparent render target. An image node over the same rectangle,
placed by percentages of the 0.8 x 0.6 frame like the other HUD roots, shows it.
The node carries `CampaignInterface`, so cinematic mode hides it, and the camera
only runs while the node has layout size.

Frame selection (R, same convention as the day/night model's sun in
`pcx_play::day_night`): frame = first sequence start + span x `day_fraction()`.
`day_fraction()` is the object host's hour over the cycle length, so hour 0 maps
to frame 0. Before the clock is admitted nothing is drawn. Global-sequence clocks
(sparkle, pulse) run from the scene's creation in real time.

### Open questions

- The native choice of `Stand` versus `Stand Alternate` and whether the native
  code maps hour 0 to frame 0 are unverified. The 24 hour reading rests on the
  keyed rotation and the 6/18 alpha switches, not on a native trace.
- The render target is straight-alpha composited. Blending into a transparent
  target yields premultiplied colour, so partially transparent edge pixels are
  slightly darker than the original, and additive glow over fully transparent
  pixels is lost. A premultiplied UI material would remove both; not built.
- The ring overhangs the world, so the target is transparent where the model draws
  nothing. Pixel parity against C has not been measured.
- The cutout reference (`menu_lighting::parameters`) is not bound, as in the score
  screen and menu sprites; the alpha-keyed frame uses the material's own cutoff.
- The extent is the MODL header bounds. Animated scaling bones (`Plane01`,
  `sunrise/set`) may draw outside it and would clip.
- No GPU capture was taken for this change; the synthetic tests cover the frame
  mapping, extent placement, target sizing and button construction only.

## Resource string frame alignment (2026-11-03)

Shipped-byte evidence: the selected Legacy 1.29.2.9231 RoC install's
`UI\FrameDef\UI\ResourceBar.fdf` supplies each string's anchor, frame height,
font, and horizontal justification; upkeep also has an explicit frame width.
The previous consumer placed unwrapped text directly on that sized node. Bevy's
`NoWrap` text measurement is unbounded, so `TextLayout` justification did not
center the glyph run inside the source width.

Each resource string now has a source-sized positioning frame and an intrinsic,
nonshrinking text child. Flex placement follows the parsed horizontal and
vertical justification. Simple strings without an explicit vertical record use
middle alignment, a recreation convention supported by the observed original
resource bar; explicit top/bottom records take precedence. Text roots and colour
spans use the existing HUD one-font-size line-box convention. No measured pixel
offset, font size, width or height was added. The synthetic test varies source
records and checks sizing, ownership and all supported alignment directions.

Runtime evidence: an isolated/muted production Prologue02 render and a retained
original Wine/Xvfb render use the same 1280×960 extent and selected install
(executable SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`).
The upkeep green-glyph left edge now matches; its top is still one pixel high,
its width one pixel larger, and its glyph weight differs. Enabling font hinting
had no effect on this capture and was not retained. Font rasterization parity
remains open; centering the source frame does not prove native font metrics.

The comparison covers the **entire screen**, without registration, resizing,
tolerance, normalization or excluded pixels. It finds 1,099,199 differing RGB
pixels out of 1,228,800, spanning all 48 tiles of a 160×160 diagnostic grid.
The tile counts sum exactly to the full-frame count. Side-by-side, 50% overlay,
raw absolute RGB differences, binary difference mask, annotated full-screen
grid, source hashes and measurements remain local under
`.scratch/readme-full-screen-comparison-20261103/`. The earlier before-fix report
is retained separately. The world, minimap, portrait, clock and other text
also differ. Camera, animation, simulation and pointer state are not synchronized,
so those pixels cannot all be assigned to layout or rendering errors.

Validation: four resource-bar synthetic tests and the mounted HUD capture pass;
the original-asset parser gate remains explicitly ignored. All five fresh README
images were visually inspected. Strict native all-target and WASM Clippy,
native/WASM application builds and formatting pass. This is not full-screen
pixel parity or a full campaign/item diagnostic pass.

### Upper-button image quads (2026-11-03)

Read-only shipped-byte and Wine runtime evidence now distinguishes the generic
image path from the font emitter. For the selected Legacy 1.29.2.9231 RoC image,
RTTI and constructor stores identify CSimpleTexture's vtable at `0xec8f64`.
The rectangle writer at `0x8877a0` copies floating frame edges into four XYZ
vertices at object+0x98; UVs are at +0xc8. The batch collector at `0x887250`
forwards these arrays, and `0x886cb0` binds/draws them. No integer frame rounding
or font-specific half-pixel correction appears in this rectangle writer.

The isolated run `.scratch/pixel-native-textures-20261103/` reads sixteen upper
texture objects: normal, pushed, disabled and highlight for all four buttons.
Their fractional positions and UVs agree with the authored frame/atlas records.
The texture handle reaches its GPU object through +0x0c, as confirmed by
`0x972730`; GPU dimensions agree with the source BLP header. The low sampler
selector bits choose the source table's linear magnification/minification and
disabled mip filtering. This qualifies the retained base-only upper atlas and
explains why the prior mip experiment was rejected. No sampler value was added
to production constants.

Separate projection/view cache reads retain 32 orthographic observations.
They include the centered UI view and another translated view; cache state
changes within a frame, so these reads are not an atomic capture of a named draw.
Raw observations, source and executable hashes, bounded probe sources, launch
and null-audio receipts are retained locally in `quad-evidence.json`. The native
container was stopped. Static windows remain in `.scratch/native-ui-quads-20261103/`.

The recreation now preserves the authored fractional bounds for these four
button images and their hover layers after Bevy layout. The D3D9 integer-center
to Bevy half-integer-center conversion is a raster convention, not a measured
asset offset; see [D3D9 pixel mapping](https://learn.microsoft.com/en-us/windows/win32/direct3d9/directly-mapping-texels-to-pixels).
Zero-sized hidden layouts remain hidden. Text labels retain their separate text
path; generic console/backdrop images have not yet opted into this conversion.

The mounted headless/muted capture in `.scratch/pixel-upper-quads-20261103/`
passes, including upper-control hover/pressed/disabled behavior. The entire
unregistered 1280×960 RGB comparison falls from 1,097,868 to 1,091,515 differing
pixels, a reduction of 6,353. Mean absolute channel error falls from
14.71206787109375 to 14.553592664930555. The upper-control diagnostic region
[0,0,550,40] falls from 19,670 to 13,317 differing pixels and MAE 23.5605303 to
14.7089697. No registration, tolerance or excluded pixels are used in the full
comparison. Upkeep remains unchanged and matched; full-frame parity, remaining
UI/font/color differences and native animation/RNG identity remain open.

Seventeen upper-control tests, strict native/WASM Clippy and both application
builds pass. The new synthetic check exercises fractional bounds and pixel-center
conversion at two viewport scales. Selected-frame repeats are byte-identical,
all thirteen acquisition clock receipts hold, and all five README screenshots
were visually reviewed and refreshed from this run. Browser rendering and other
native graphics backends were not exercised by this Vulkan/Wine comparison.

The upper image conversion now lives in `hud_render::SourceImageRect` and is
shared with source-planned console images. The current full-frame comparison
is 1,017,908 differing pixels; the entire fixed upkeep diagnostic box matches.
See [console coordinate evidence](gameplay-hud-fidelity.md#console-image-coordinates-preserved-2026-11-03)
for the thirteen native console matches, validation and remaining scope.

## Upper label raster policy (2026-11-03)

The selected Legacy 1.29.2.9231 executable (SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`)
now supplies upper labels' default raster-size policy, alpha cutoff and atlas
sampling through the existing source-font component. `UpperButtonBar.fdf` still
supplies each state's font face, authored height, colours and text; rich colour
spans receive the root's physical font size. The three state style chains are
validated as default font records before assets are committed. Unsupported font
or widget flags refuse. This reuses the panel validator in the shared HUD code;
no source values or glyph data are copied into constants. No FreeType is added.

The synthetic four-button test verifies all twelve state roots' policy identity,
normalized source height, sampling and alpha binding. The correctly filtered
upper-controls/message-log suite passes 22 tests with one explicit original-FDF
input ignore. Eleven panel checks pass with one explicit local-chapter ignore;
strict native/WASM Clippy and both app builds pass. Production captures exercise
normal, hover, pressed and disabled upper states. Their original placement and
pressed-offset path remains in use: this change qualifies raster settings, not
native pair spacing, baseline or frame placement.

Exactly 1,804 recreation pixels change inside `[28,11,513,26]`. The full-screen
exact RGB difference count falls by 200 to 978,207 / 1,228,800, but mean absolute
channel error rises from 11.939232313368056 to 11.940990125868055. The diagnostic
upper strip `[0,0,550,40]` changes from 9,620 differences / MAE 11.55060606060606
to 9,420 / MAE 11.648787878787878. Both metrics are retained: fewer unequal
pixels does not establish complete improvement or parity. Upkeep's 2,640 pixels
and 13 panel text diagnostic regions remain exact. Full-frame diff, mask,
overlay and side-by-side remain local in `.scratch/pixel-upper-source-font-20261103/`.
Native tick/animation/RNG identity and remaining UI/world fidelity remain open.

## Upper string widths, baselines and frame origins (2026-11-03)

A fresh private Wine/Xvfb/software-rendered run of the same SHA-qualified
Legacy 1.29.2.9231 RoC client retains twelve upper `CSimpleFontString` states
through a bounded read-only process-memory probe. This includes hidden normal,
highlight and disabled strings. The Gx observations show the supported default
integer-coordinate branch, centered horizontal/vertical modes, automatic width
and one authored font-height frame. The normal/highlight strings share geometry.
Native frame tops and normalized font inputs agree with centering the source
font-height frame on its FDF button. The local probe and raw widget/Gx fields
are retained in `.scratch/upper-string-evidence-20261103/native-evidence.json`.
The no-network container used a private 1280×960 Xvfb and null audio sink and is
verified stopped. This is Wine runtime evidence, not native Windows or tick
identity evidence.

Upper labels now use the shared source pair-advance/baseline calculation, keep
rich hotkey colours, and have explicit no-wrap layout. The first capture caught
the generic wrapping path; that failed run is not the published baseline.
Spacing without source frame placement still left the origin wrong. The final
path retains automatic source widths and resolves the source font-height frame
around each button's authored center before pixel rounding. Horizontal/vertical
defaults come from the admitted constructor operand, retained in the font policy;
explicit FDF justification takes precedence. Inventory shares the same flag
interpretation helper without changing its geometry. Pressed offsets retain
source units before the shared renderer positions the leaf; the old rounded
wrapper is no longer its pixel-origin input. No screenshot-fitted offset or
source font-size constant is added.

All twelve resulting line widths agree with the live native observations: the
normal/hotkey Quests/Menu/Allies/Log strings are 86/80/75/66 pixels, and the
plain disabled forms are 54/43/42/29. These are diagnostic outputs, not layout
inputs. Synthetic tests cover all twelve frame/policy bindings, no-wrap layout,
source pressed displacement and disabled/released restoration. Twenty-two upper
and message-log tests, 23 Inventory tests, nine synthetic font-policy checks,
one explicit supplied-executable policy check, strict native/WASM Clippy and
both app builds pass. Original-FDF, chapter and separate native-advance inputs
remain explicit test requirements where not supplied.

The final exact 1280×960 RGB comparison falls by 2,024 pixels to 976,183 /
1,228,800, MAE 11.755233289930555. Exactly 2,381 recreation pixels change inside
`[28,11,513,27]`. The upper diagnostic strip `[0,0,550,40]` falls from 9,420
unequal pixels / MAE 11.648787878787878 to 7,396 / MAE 1.2734242424242423.
All 2,640 upkeep pixels and thirteen panel text rectangles remain exact. Two
fresh production captures pass, all 26 clock receipts hold, selected repeats
match and all five published states independently match byte-for-byte. The
full-frame diff, mask, overlay and side-by-side remain local in
`.scratch/pixel-upper-layout-20261103/`. Remaining blended-edge/backing-image,
other UI/world differences and native animation/RNG identity remain open;
this is not a full-screen or all-state native parity claim. No FreeType added.
