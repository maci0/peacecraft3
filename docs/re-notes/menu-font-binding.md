# The menu font's face → file binding

A `FrameFont "MasterFont", 0.013, ""` record in the FDF names a **face**, not a
file. The file comes from the shipped skin table, and that is where the binding is
read from — the face name is not mapped to a path by any constant in this project.

Read-only, via the prebuilt `target/release/mpqxtract`; no build was run.

## The binding

`UI\war3skins.txt` carries a `// font references` block inside its `[Default]`
section:

```text
[Default]
...
// font references
MasterFont=Fonts\FRIZQT__.TTF
MessageFont=Fonts\FRIZQT__.TTF
ChatFont=Fonts\FRIZQT__.TTF
EscMenuTextFont=Fonts\FRIZQT__.TTF
InfoPanelTextFont=Fonts\FRIZQT__.TTF
TextTagFont=Fonts\FRIZQT__.TTF
```

So **`MasterFont` → `Fonts\FRIZQT__.TTF`**, authored in `[Default]`.

The table is sectioned — `[Main] [Human] [Orc] [NightElf] [Undead] [Default]` — and
the font keys are defined **only in `[Default]`**. The race sections define their
own console art and none of the font faces, so every race resolves the face through
`[Default]`. That is a fallback, and it is the one the shipped data relies on.

## Provenance

| File | Bytes | SHA-256 (first 24) |
| --- | --- | --- |
| `War3\UI\war3skins.txt` | 44,153 | `7b685b0d2c1129d63de0b116` |
| `War3x\UI\war3skins.txt` | 44,153 | `7b685b0d2c1129d63de0b116` (byte-identical to War3) |
| `War3xLocal\UI\war3skins.txt` | 44,353 | `476a485dc692a3312fdd48b2` |
| `War3\Fonts\FRIZQT__.TTF` | 62,316 | `8f798feb09a0e9dc97daf0a5` |

`Fonts\` ships seven faces (`FRIZQT__.TTF`, `NIM_____.ttf`, `THOWR___.ttf`,
`DFHeiMd.ttf`, `dffn_b31.ttf`, `dfst-m3u.ttf`, `tt5500m_.ttf`); the skin table names
`FRIZQT__.TTF` for all six font keys.

## Duplicate and override ambiguity

Measured over all three tables: **`MasterFont` appears exactly once**, and so does
every other font key — there is **no duplicate and no override** in the shipped
data, so the ambiguity the resolver guards against does not arise here.

The resolver refuses it anyway, because the engine's precedence is **not
established**:

* **Every block with the consulted section name is aggregated**, not just the
  first. A repeated `[Default]` therefore cannot hide a second definition: a face
  defined in two blocks of the same section is `Duplicate` even when the two values
  agree, and a conflicting pair is `Duplicate` with both values reported.
* A face that resolves to an **empty value** is `EmptyValue`, never a binding with
  no file.
* An **empty section name** (`[]`) or an **empty key** (`=value`) is `Malformed`.
* A face defined twice inside one block is likewise `Duplicate`.

**The requested-section-first order is caller policy, not the game's order.** No
native observation of the engine's font lookup was made; that a race section may
override `[Default]` is an inference from the table's shape. What is proven is the
data: the keys, their sections, and the file they name.

## API

`crates/peacecraft3/src/menu_skin.rs` — pure `std`, no Bevy, no Cargo change.

```rust
let table = menu_skin::SkinTable::parse(&text)?;         // SkinError::Malformed
let b = table.unique_file(face)?;                  // SkinError::UnknownFace / ::Duplicate
b.file      // "Fonts\FRIZQT__.TTF"
b.section   // "Default" — where the winning entry came from
```

* `face` is the caller's `TextStyle::font`; the module has **no** `MasterFont`
  mapping of its own.
* Production lookup requires exactly one definition across the complete table.
  It chooses no section order. The section-first helper remains test-only policy.
* **All** blocks with the consulted name are aggregated, so a repeated section
  cannot hide a duplicate.
* A face the table does not define is `UnknownFace`; two entries in the winning
  section are `Duplicate { section, face, values }`; an empty value is
  `EmptyValue`. No default face or file is substituted.
* Section identity is preserved on the result, so a caller can tell an override
  from a fallback.

## Probe against the shipped files

The module run over all three real tables (isolated `rustc`, no cargo): 6 sections
each; `MasterFont`, `MessageFont` and `InfoPanelTextFont` each resolve to
`Fonts\FRIZQT__.TTF` **from `[Default]`** for `Human`, `Orc`, `NightElf`, `Undead`
and `Default` alike; each face appears **1×** across the table.

## Not established — do not infer

* **`FIXEDSIZE` is unresolved native behaviour.** The flag is parsed from
  `FontFlags`, but what it does (a reference-resolution pixel size, versus the
  viewport-relative size the FDF's height implies) is **not** determined by the
  name or by the table. It is reported as parsed-but-unapplied, not interpreted.
* **The engine's actual lookup order is inference, not measurement.** That a race
  section can override `[Default]` is inferred from the table's shape; no native
  observation of the font lookup was made. What is proven is the data: the keys,
  their sections, and the file they name.
* **Native shadow equivalence remains unverified.** The current renderer passes
  authored offsets through `FrameShadow` into viewport-relative `TextShadow`
  placement; this does not prove matching native glyph rasterization.

## Runtime integration (2026-10-14)

Each menu entry supplies its own parsed `TextStyle.font`. The GUI caches decoded
fonts by face and clears both the skin and font cache on asset revisions. Browser
hydration parses the same FDF/skin inputs and registers each derived file under
its canonical path and derived served alias. The static loader calls the Rust
`menu_font_files` and `menu_font_alias` exports, avoiding a copied face-to-file list.
Missing, duplicate and empty bindings are errors, with no substitute font path.

Twelve Node tests and 21 actual WASM assertions pass, including an alternate
synthetic face/file and duplicate definitions across skin sections. The actual
running-app import resolves all six original labels' font paths from the supplied
FDF and selected skin table, verifies alias bytes, and preserves label/scene
refresh, click audio, resize and transaction rollback. The final captures are
under `.scratch/browser-iso-app-import/after-font-opacity/attempt-3-ordered-fonts/`.
Only local Blob reads occur after import; no HTTP requests, uploads or sockets.
The locally repacked source fixture is not a retail installer ISO acceptance gate.

## Next, if the native order must be proven

The engine-side binding is in the Legacy executable. The concrete next probe is to
break where the FDF's `FrameFont` face is resolved against the skin table — the
same path that resolves `MainMenuLogo_V%d` through `FUN_005c30a0` (see
`docs/re-notes/glue-coordinate-runtime.md`), which already checks CustomSkin, then
the active skin, then the default skin. Reading that function's section lookup
would settle the override order; a repeated REA pass on the 14.8 MB image has
timed out before and is not the plan.

## Popup title shadow inheritance (2026-10-16)

Popup label style now resolves omitted shadow colour and offset through the same
actual-title and enclosing-frame lookup used by its font. Explicit child records,
including zero offset or zero alpha, keep precedence. Malformed inherited records
refuse instead of becoming invisible defaults. Numeric parsing stays on the
existing FDF path; `FrameShadow` placement and the other text colours are unchanged.

The previous implementation fails the synthetic title-only shadow regression.
The correction passes all 418 application tests (15 explicit ignores), strict
native/WASM Clippy, rustdoc and both builds. Twenty-six parser/style cases execute
under WASI, including inheritance, explicit zero, absence and malformed parent
records. This is style/component-path verification; no new original-game or
rendered pixel comparison was run. Root evidence is in
`.scratch/cursor50-extra-20261016/root-034/`.

## Popup title vertical alignment (2026-10-17)

Popup values now use middle alignment when their selected text frame omits
`FontJustificationV` along its inheritance chain. An authored value still wins;
malformed values keep the existing style error. Ordinary labels and horizontal
alignment are unchanged. This class default follows the pinned
[reference TEXT implementation](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/parsers/fdf/GameUI.java#L536),
not a measured native dispatcher. No inset or screenshot coordinate was copied.

The component regression covers omission, inherited top alignment, ordinary
labels and malformed input. All 356 application tests, strict native/browser
Clippy and the browser build pass. An actual browser import of the selected
original panel/FDF inputs places the resolution value exactly at its control's
vertical center. Cancel returns to Main. The first capture also exposed a
category-animation regression that hid the right panel. A separate
[per-layer correction](menu-panel-navigation.md#options-owner) restores that
housing in a fresh capture. Overall Options parity, typography and native
rasterization remain open.
Evidence: `.scratch/options-category-root-20261017/`.

## Font quantization experiment (2026-10-17)

Uniformly truncating the current physical font size was tested against fresh
unchanged-font and experimental browser captures at the original witness's
1280x960 extent. Both imports used the same local source closure; both captures
reported stable state, seventeen local code GETs and no uploads or page errors.
The fixture font matches the qualified install member, while the original
runtime's winning archive remains unobserved.

The change improves some label dimensions but moves Gameplay and Cancel widths
away from the reference where the previous path was closer. The heavier fringe
also remains. The experiment was rejected and both Rust files restored exactly;
no quantization helper or font refresh system remains in the product. Source
`FrameFont` sizing, shadow and layout remain on their previous paths.

The later exact-Legacy investigation below establishes nearest-pixel rounding
in one native font path. It does not rehabilitate this uniform-truncation
experiment or establish equivalence of the Bevy and native glyph renderers.

The pinned renderer's integer cast is useful implementation evidence, but its
[menu viewport is established at startup](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/WarsmashGdxMenuScreen.java#L108)
and its [resize path updates that viewport](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/WarsmashGdxMenuScreen.java#L546).
That alone does not establish re-quantizing current physical pixels on each
resize or density change. The candidate passed 365 app tests and strict
native/browser checks; those tests do not outweigh the mixed visible result.
The restored product is the previously verified 363-test implementation.

Evidence: `.scratch/menu-font-experiment-20261017/decision.json`, the two private
code-only packages and `right-label-metrics.json`. Masks and regions are image
analysis only; no measured coordinates or font sizes were copied into product
constants. A scheduled resize fixture was not run because the candidate was
withheld. Typography, text rasterization, control spacing and full native visual
parity remain open. Neither package nor original content was published.

## Text line boxes and dependent anchors (2026-10-17)

The measured TEXT height feeds FDF bottom-anchor chains. Bevy's default paragraph
leading inflated that height, so the vertical error accumulated down Video
Options. Menu label roots and every colour/hotkey span now use a one-font-size
line box. The selected font size, face, colours, shadow and authored anchor
relationships remain unchanged. This is a recreation layout convention, not a
claim that the original font metrics or multiline algorithm use that ratio.
No measured screen coordinate was added to the renderer.

A fresh 1280x960 browser comparison against the retained Wine/Xvfb original frame
moves the nine left gold labels from errors of 6–39 pixels to within one pixel
vertically under the same analysis mask. Static right-button ink masks retain
the baseline dimensions and counts. The Main menu was also visually reviewed;
the popup value remains centered and its five source-derived rows still open.
Both new captures report stable menu state. The run retains seventeen local
code GETs, no page errors, no sockets and no uploads. The original runtime's
winning font member and full typography parity remain unobserved.

All 363 application tests pass (18 explicit gates ignored), along with strict
native/browser Clippy, the browser build and formatting. The first comparison
attempt failed after the old screenshot path reset the emulated screen bound;
that harness defect was corrected separately, and the immutable failure remains.
Evidence is in `.scratch/options-text-anchors-20261017/`, especially
`browser-artifact-2/browser/`, `anchor-metrics.json` and
`right-label-metrics.json`. Multiline native spacing, font rasterization, panel
shading, unsupported settings and full Options parity remain open. No publication.

## Exact Legacy font sizing and glyph flags (2026-11-03)

Binding: locally supplied `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The deep-analysis provider refused its launcher before analysis; bounded Rizin
and objdump reads of this exact file supplied the static observations below.
No older `Game.dll` analysis was substituted. Rizin's string inventory warned
that it skipped an oversized section; its positive GxuFont result is useful,
but the inventory cannot support absence claims.

### Static observations

- `GxuFont.cpp` is at `0xebcc30`; its reference at `0x7e25d7` is an allocator
  callback. The string alone does not identify a HUD glyph-rendering call.
- Font creation `0x7e2c40` allocates the font object and calls `0x7ebe20`.
  That function stores the requested normalized height at `+0x1c8`, clamps it
  against `2 / [0x114e934]`, and stores the result at `+0x1cc`.
- `0x7e3de0(0, height)` multiplies by the viewport-height integer
  `[0x114e934]`, adds a signed half, then uses `cvttss2si`. For positive
  font heights this is nearest-pixel rounding, not truncation of the raw
  scaled height. The nonzero first-argument path returns its input unchanged.
- `0x7f0be0` repeats that sizing on refresh, caps the integer at 64, and stores
  it at `+0xb0/+0xb4`. Font flag bit 0 subtracts two from `+0xb4` when the
  height exceeds two. It derives `+0x1c0` from the face's signed ascender
  and descender and the chosen pixel height, with the observed positive
  nearest-integer result. It calls `0x9f9460(face, pixel_height, 0)`.
- The latter function mirrors the nonzero size onto the zero size, clamps
  each to 1..65535, shifts by six and calls a size-request function. This
  matches the FreeType pixel-size API's shape. Face layout also matches the
  FreeType structures, including units-per-em, ascender, descender and glyph.
- The glyph-admission caller at `0x7eac90` sets its final boolean argument
  to `!(font_flags >> 1) & 1`. `0x7f1c60` constructs load flags
  `0xa8 + 2 * (argument != 0)`, calls the glyph loader `0x9f6ad0`, then
  calls the renderer `0x9f8dc0` with render mode zero. Under the FreeType
  definitions, `0xa8` combines NO_BITMAP, FORCE_AUTOHINT and PEDANTIC;
  `0xaa` additionally sets NO_HINTING. Therefore the **default zero font
  flags select the unhinted branch**, despite FORCE_AUTOHINT also being set.
  This does not justify enabling Bevy hinting for the default HUD text.
- `0x7f1cc0` retains floating-point advance and bearing quantities from the
  glyph slot, multiplying its fixed-point integers by the conversion operand
  at `0xebcdf0`. Downstream placement/kerning remains to be traced; integer
  font height does not establish integer glyph advances.

Function names inferred from FreeType API/structure correspondence above are
not exported symbols recovered from this stripped executable. The instruction
addresses, branch operands and field accesses are direct byte observations.

### Runtime corroboration

A fresh private reference run loaded unchanged RoC Prologue02 at 1280×960,
96 DPI, using its own Xvfb display and verified PulseAudio null sink. A separate
PE32 helper opened the game with read/query rights only and traversed the
bounded native font list rooted at `0x10dfd34`. It performed no memory writes,
code injection or source-map/script changes. The isolated container was stopped
after collection.

All 13 observed font records agree with the positive nearest-pixel height and
baseline derivations. Examples:

| Normalized requested height | Pixel height | Baseline field | Font flags |
| --- | --- | --- | --- |
| 0.0166666657 | 16 | 13 | 0 |
| 0.0216666665 | 21 | 17 | 0 and 1 |
| 0.0249999985 | 24 | 19 | 0 and 1 |
| 0.0399999991 | 38 | 30 | 0 and 1 |

The observed face has units-per-em 1000, ascender 965, descender -250 and
height 1215, matching the locally extracted FRIZQT face's metrics. Matching
metrics do not prove which archive member supplied the runtime face, nor which
font-object record belongs to a particular text-frame instance. The resource
FDF's 0.01 height maps to the 16-pixel case under the existing 0.6 UI conversion;
it provides no reason to shrink the upkeep font to 15 pixels.

Local evidence: `.scratch/native-font-metrics-20261103/report.json`, the bounded
disassemblies, helper source/executables, and
`.scratch/pixel-native-fonts-20261103/fonts-control-v2.txt`. The report retains
input, output and runtime file hashes. Rizin GxuFont string Evidence is
`ev_86e94dd8a0f2e4db2dee9561071cf5edd08cd74d022179de768d8a05ced95ef9`.
System FreeType 2.15.0 raster probes with the source face and both observed
flag combinations remain diagnostic: they do not prove equivalence to the
bundled renderer. Native glyph bytes, kerning, screen-space placement and full
original/recreation raster parity remain open. No product font size, baseline
offset or hinting mode was changed from these observations alone.

## Native glyph bytes and active spacing mode (2026-11-03)

A further fresh, isolated Prologue02 run retained the existing read-only process
probe and added bounded glyph-cache lookups. `0x7edd30` provides the lookup
relationship: mask at font `+0x6c`, bucket base at `+0x60`, twelve-byte buckets,
key in the entry's first word, and the shared glyph record at entry `+0x3c`.
Only already-cached characters were read; no game functions were invoked.
The first read occurred before all upkeep characters were cached. The later
HUD read found every character, and only that complete read supplies the
comparison. The container was stopped after collection.

For the 16-pixel, zero-flag face, the retained bitmaps are:

| Character | Bitmap dimensions | Alpha bytes |
| --- | --- | --- |
| N | 12×13 | 156 |
| U | 12×13 | 156 |
| e | 9×10 | 90 |
| k | 9×13 | 117 |
| o | 10×10 | 100 |
| p | 10×13 | 130 |

All **749 alpha bytes** match independently rendered unhinted FreeType output
exactly, using the locally extracted source face, pixel size 16 and the observed
`0xaa` load flags. This holds for both system FreeType 2.15.0 and the Rust
wrapper's bundled FreeType 2.13.2. Space is a separate, entirely zero-filled
4×16 native placeholder; standalone FreeType returns an empty space bitmap.
No font sizes or glyph coordinates were fitted to screenshots for this check.
This qualifies these glyph-cache bytes, not complete text rendering or all
glyphs/font formats.

The same run reads `[0x114e968] == 0`, selecting the integer-width branch in
the inspected measurement path. `0x7e94e0` maps a pair to glyph indices,
requests unscaled kerning (mode 2), discards positive kerning, multiplies the
retained nonpositive value by font `+0x1d0`, and adds
`min(previous_bitmap_width, previous_advance)` in this mode. Except for three
explicit pair overrides in its table at `0xebcd98`, it passes that result
through the ceiling operation at `0xa7dc10`. This establishes a different
spacing policy from ordinary fractional shaped advances. The override records
and the other text flag/mode branches must remain explicit in any port.
Final measured width also passes through ceiling in `0x7ec990`. This is not
evidence that every rendered glyph position is independently rounded.

Native shared glyph fields retain fractional advance/bearing values as well
as vertical clipping operands. Correction: the earlier draw-x/draw-y labels for
shared-glyph `+0x40/+0x44` were incorrect. The producer at `0x7f1bf0`, called
from `0x7f1e5e`, reads the glyph bitmap top, baseline, font pixel height and
bitmap rows. Its outputs reach these fields through the temporary glyph's
`+0x1c/+0x20`; they are not horizontal/vertical pen coordinates. For the sampled
`p`, baseline 13, bitmap top 10, font height 16 and rows 13 produce -1 and 3
through the bottom-clipping branch. A horizontal -1 correction based on this
field would therefore be wrong. Downstream clipping/placement still needs
qualification; do not introduce a uniform glyph-position rounding patch from
these measurement routines alone.

Local evidence is in `.scratch/pixel-native-glyphs-20261103/glyph-comparison.json`,
with digests for every source/trace/bitmap and the private launch/display/audio
receipts. Bounded lookup, measurement and pair-spacing disassemblies are in
`.scratch/native-font-metrics-20261103/`. A separate local portability probe
confirms that the current FreeType Rust wrapper does not build unchanged for
`wasm32-unknown-unknown`: its bundled C needs target headers, and after providing
them the wrapper still references unavailable libc allocator functions. The
ordinary and configured build failures remain in
`.scratch/freetype-portability/wasm-*-build.log`. No product dependency,
native-only renderer substitution or browser fallback was added. The shared
native/WASM raster integration and rendered placement correction remain open.

The renderer remains within the existing Bevy text stack; adding FreeType is
disfavored because of dependency size. The UI `Text` requirement now selects
`FontHinting::Disabled`, matching the observed Legacy default font flags. An
explicit per-entity hinting override remains supported. This corrects the
rasterization policy; it does not establish final glyph placement or pixel
parity. The experimental native/browser FreeType integration was removed.

The subsequent Bevy-only capture passes the mounted HUD gate and repeated-frame
byte equality. The exact full-screen comparison has 1,099,223 differing pixels
of 1,228,800 and mean absolute channel error 14.791306, versus 1,099,015 and
14.811383 before the hinting correction. These mixed diagnostic results do not
prove better pixel parity. Local full diff, mask, overlay, side-by-side and
comparison receipt: `.scratch/pixel-unhinted-bevy-20261103/`. Original animation
phases remain unmatched. README images now come from this Bevy-only run.

### Bevy checkpoint layout receipt (2026-11-03)

The mounted HUD capture now retains `selected-text-layouts.json` at the selected
unit checkpoint, before later interactions. The local run in
`.scratch/pixel-bevy-layout-20261103/` passes and retains 145 text-root records,
with font-size expressions, layout extent/scale, span text, glyph positions and
atlas bounds/offsets. The repeated screenshot remains byte-identical. These are
Bevy runtime observations, not evidence of the native final draw coordinates.

Upkeep resolves to `Vh(1.6666665)` and a 86×16 layout. Its glyph quad left edges
relative to the text block are approximately 0, 12.48, 22.576, 26.752, 38.768,
48.8, 56.768, 66.08 and 75.392. Native pair measurement steps for the same string
are 12, 10, 4, 12, 10, 8, 9 and 9. Thus ordinary fractional shaping differs from
the observed integer-width measurement policy; nearest-pixel font sizing alone
cannot establish spacing parity. Do not replace the positions with these sample
numbers. Port the source font metrics/kerning and edition-qualified measurement
policy, then qualify the native final placement/clipping path before asserting
rendered equality. The existing Bevy rasterizer remains in use.

### Native pair spacing reaches quad emission (2026-11-03)

A bounded raw relative-call scan of the pinned Legacy image identifies three
candidate calls to `0x7e94e0`. Disassembly confirms two in the existing
measurement paths and one at `0x7ec4b9` inside `0x7ec0f0`. This last function
allocates per-page position arrays and writes four position and UV entries for
each admitted glyph. Thus this pair-spacing policy also feeds quad generation;
it is not only a text-width estimate. This is static control/data-flow evidence,
not an observed execution of the particular upkeep string.

The emitter selects `0x7e94e0` or the alternative `0x7e9680` using string flag
bit `0x10`. It combines the pair result with extra character spacing, scales by
rounded string height divided by the font's `+0x1bc` pixel height, and, unless
string flag `0x80` is set, applies signed nearest-integer rounding before adding
the delta to its pen accumulator (`0x7ec4c6..0x7ec531`). It starts a quad from
that pen; the nonzero spacing-mode branch additionally adds the ceiling of the
cached bearing (`0x7ec53e..0x7ec572`). Glyph bitmap width determines the quad
width, with truncation in the pixel-coordinate branch (`0x7ec57a..0x7ec5b3`).
The source's device-dependent half-pixel adjustments are applied at
`0x7ec619..0x7ec684`; they must not be copied into Bevy without qualifying the
coordinate-system translation. The cached `+0x40` clipping operand then shifts
all four Y coordinates (`0x7ec68a..0x7ec6d5`), corroborating the corrected
vertical interpretation.

The three pair-table records are now decoded locally from the PE mapping.
The routine applies their operand after ceiling the ordinary pair calculation;
they are additive exceptions, not substitute advances. Do not embed the records
in production: derive the table address, count, stride and values from admitted
source instructions/data, with synthetic parser tests and explicit edition
provenance. The live string flag bits, scale, device correction branch and
native final vertex values remain to observe before claiming the screenshot's
complete native placement path.

Retained read-only evidence: `ev_f86f779016745ac994ebfe5c2363585d4026e3e4219178d75aa8e050828a7e43`
for the first emitter window and
`ev_d53661044570e2ff35058ba9955fde6677fbe63efca58152a271810d2bef01e5`
for vertex/UV emission. Their full receipts, raw objdump cross-check
`quad-emission.asm`, relative-call candidates and `pair-overrides.json` remain in
`.scratch/native-font-metrics-20261103/`. The source SHA-256 is unchanged.
Rizin's relocation labels sometimes obscure branch destinations; raw instruction
bytes were cross-checked instead of treating those labels as native symbols.

### Pair-policy derivation in source (2026-11-03)

`pcx_play::font_pair_policy::PairPolicy` now admits the reviewed image with
explicit source/edition/version provenance and reads the pair table address,
record count/stride and fallback from the inspected instruction window. Pair
keys and addends come from the resulting bounded PE range. No original pair
records are stored in the repository. The finishing operation ceilings the
ordinary step and then adds the selected exception, preserving the native
order. Unsupported images, instruction shapes, extents, duplicate keys and
non-finite records refuse instead of substituting values.

Two synthetic tests pass with deliberately different table contents and count;
the explicit local-original test also passes. Strict native Clippy, WASM check
and native/WASM library builds pass. This module does not yet replace Bevy
shaping: source-font ordinary metrics, live string flags, scaling and placement
remain to connect and verify. No screenshot recapture is warranted by this
parser-only change, and no new pixel-parity claim follows from its tests.

### Source-font pair metrics (2026-11-03)

The pair-policy module now parses the supplied static font's character map,
horizontal advances and qualified horizontal format-0 kerning. `integer_step`
combines the rasterizer-provided bitmap width with the unhinted advance,
clamps positive kerning away, and uses the admitted pair policy's
ceiling/addend order. It does not substitute outline bounds for bitmap widths.
The font carries source/edition/version and a SHA-256 digest. Variable fonts,
unsupported kerning combinations and malformed ranges refuse explicitly.

The direct parser dependency is `ttf-parser` 0.25.1, already present in the
workspace lockfile through the existing font stack; no package or rasterizer
was added. The lockfile change is only its direct dependency edge for pcx-play.
A wholly synthetic SFNT fixture checks source metrics, negative and positive
kerning, missing glyphs and the combined integer-step calculation. The explicit
local native receipt test checks all six retained advances at the pixel size
read from the native font record: all six match exactly. The original image
policy test also passes (five focused tests total). Strict native Clippy and
native/WASM library builds pass.

This result qualifies the tested metrics, not every glyph/font/size. Bevy's
ordinary shaping has not yet been replaced: the selected live string flags,
raster bitmap widths, scaling and final layout integration remain to connect.
Local reference envelope: `.scratch/native-font-metrics-20261103/rust-metrics-reference.json`.
No new screenshots or pixel-parity claim follow from this metrics-only change.

### Resource-bar integration and rendered check (2026-11-03)

Resource labels now admit `PairPolicy` from the selected client image. The
profile also derives the font minimum, cap and empty-glyph bitmap-width rule
from their instruction/data operands. The empty glyph path at `0x7f1d8d`
computes a rounded-up fraction of the font pixel height, with a zero-result
fallback to that height; treating the atlas's empty space rectangle as width
zero would incorrectly collapse spaces. Synthetic sizing operands differ from
the source and verify this derivation.

Before Bevy text measurement, the labels resolve their authored font height
against the target's physical viewport and use the derived integer pixel size.
After Bevy atlas/layout generation, the supported single-line, one-character
per glyph path computes pen steps from source font metrics/kerning, actual
bitmap widths (or the derived empty-glyph width) and pair exceptions. It
updates glyph X positions and intrinsic text width within the authored frame.
Unsupported clusters/multiline layouts report a resource diagnostic. Work runs
only when Bevy rebuilds the text layout; the correction bypasses its change
flag so it does not recursively retrigger itself. Publication now follows the
spacing system so failures remain observable.

The final headless/muted run in
`.scratch/pixel-resource-spacing-final-20261103/` passes and repeats byte for
byte. Upkeep is `Px(16)` with a 84×16 layout and quad left edges 0, 12, 22, 26,
38, 48, 56, 65 and 74. These numbers are observations, not production constants.
The original/current full-screen comparison has 1,099,095 differing pixels of
1,228,800 and mean absolute channel error 14.771229, versus 1,099,223 and
14.791306 before this integration. The original animation phases remain
unqualified; no pixels are excluded or registered in that comparison.

A separate inspection-box translation diagnostic (-1,+1) leaves 22 differing
pixels. No such offset was applied: authored frame anchoring, final coordinate
rounding and baseline placement remain to qualify, as do small raster/blend
byte differences. The native live string flags and all alternate spacing modes
are not established by this rendered check. This is an integer-width
recreation path, not proof of complete native text behavior.

Six policy/metrics tests (including the explicit local-original checks), four
resource-frame layout tests, strict native/WASM Clippy and native/WASM
application builds pass. The first capture and final capture agree exactly on
the selected frame. README images and provenance now use the final capture.

### Residual glyph bytes resolve to the native alpha test (2026-11-03)

The 22-pixel registered inspection-box residual is not 22 raster mismatches.
Nineteen pixels lie in the inner glyph area: recreation green coverage 1..3
against native zero. Three remaining pixels are on the textured border. The
font draw initializes its 24-byte render packet's selector at `0x7ed64a` and
submits that packet through `0x7d0d60`/the previously qualified virtual setter.
Its source-derived selector reaches renderer `+0x3ac`. The backend's nonzero
selector path enables alpha test/blend and GreaterEqual (`0x7df01f..0x7df07c`),
then `0x7d3380` reads the reference from its source table. For the admitted font
draw this derives 4/255. No measured cutoff was put into production.

`PairPolicy::font_alpha_cutoff` now reads that selector, table address and byte
reference from the supplied executable, checking ranges and instruction shapes.
A synthetic different-selector/different-value test checks the derivation and
out-of-byte-range rejection. Seven focused tests pass, including explicit local
font/image checks; strict native Clippy and native/WASM library builds pass.
The runtime shader does not consume this cutoff yet. It must compare combined
texture/material alpha and preserve surviving alpha: clearing low atlas bytes
would be incorrect for faded or translucent text.

Applying only the derived alpha test in the separately registered *inner*
glyph diagnostic reduces 19 differing pixels to zero. This isolates a rendering
state difference for this sample; it does not establish full-screen equality,
placement, every glyph or animated/faded text. Full-screen comparison still has
no registration, masks or tolerance. Source/diagnostic receipt:
`.scratch/native-font-metrics-20261103/font-alpha-policy.json`; individual
residual bytes: `.scratch/pixel-resource-spacing-final-20261103/upkeep-residual-pixels.json`.
The last README capture predates this parser-only extension and is unchanged.

### Centering trace frontier

Raw-byte disassembly now traces the quad emitter's caller at `0x7e9ab0` back
to `0x7e9840`. It measures the current line, selects a justification branch,
negates the whole/half line width, and passes it through `0x7e3e50` when string
flag `0x80` is clear before emission. After emitting, it calls `0x7ec790`, which
combines the string's frame/justification operands and normalizes its offsets
against the viewport. This is not evidence for applying a uniform (-1,+1)
pixel offset, or for replacing all rounding with floor. The live frame/string
operands and final application of those normalized offsets remain to qualify.
Local bounded windows are `string-origin-emission.asm` and
`string-centering.asm` in the native-font evidence directory.

The Rizin provider returned a cleanup error rather than a successful complete
receipt for the centering window (`ev_e02672a0155287a645af0bf70522e53038d1eed688669a6dc765db06febafe43`
is marked partial by that response). Raw objdump reads of the same pinned
executable supply the bounded cross-check; the failed provider request was not
repeated and no unrelated process was terminated.

### Runtime combined-alpha test (2026-11-03)

The resource labels now bind the admitted byte reference to opt-in
`bevy_ui::widget::TextAlphaTest`. The published Bevy 0.20 UI renderer is patched
locally: text/shadow extraction carries the reference in unused vertex-flag
bits and the shared shader rejects combined texture/text alpha below the
normalized reference. Surviving alpha is unchanged. Vertex stride, atlas bytes
and batching keys are unchanged; component changes/removal invalidate cached
glyph draws. Other text roots have not yet opted in. No FreeType backend or
new package is introduced.

The isolated synthetic GPU test passes for cutoff updates, faded text,
preservation of admitted coverage and removal/restoration. The ordinary
HUD-texture GPU gate also passes. Strict native/WASM Clippy and native/WASM
application builds pass. The mounted headless/muted capture in
`.scratch/pixel-font-alpha-20261103/` passes, all 13 acquisition clock receipts
hold their clocks, and selected-unit repeats are byte-identical. README images
and provenance now come from that run.

The unregistered whole-screen diagnostic changes from 1,099,095 to 1,099,069
differing pixels, while mean absolute channel error changes from 14.771229 to
14.771360. This mixed result is not a whole-screen parity claim; original
animation phases remain unmatched. The separately registered inner-glyph check
has two differences, both native green byte 4 versus recreation zero. Thus the
previous *post-framebuffer byte* threshold model's zero differences did not
prove the actual shader comparison boundary. Native sampling/comparison
precision and the one-pixel placement offset remain open. Do not patch an
epsilon or quantization rule from those two pixels alone.

The [Direct3D 9 render-state specification](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3drenderstatetype)
confirms that ALPHAREF stores an eight-bit reference, but this alone does not
establish the native incoming-alpha sampling/comparison precision. The runtime
currently uses the normalized floating-point comparison. Retained local checks:
`font-alpha-gpu.log`, `font-alpha-default-ui-gpu.log`,
`font-alpha-runtime-clippy-{native,wasm}.log`, and
`font-alpha-runtime-build-{native,wasm}.log` under `.scratch/`.

### Default font atlas sampling (3 November 2026)

Shipped-byte evidence from the admitted Legacy 1.29.2.9231 executable traces
atlas creation at `0x7edff3..0x7ee02f`: the font flag's bit 2 supplies the low
texture flag. Creation through `0x7d11f0`, the D3D backend `0x7df940`, common
creator `0x7d4590` and constructor `0x7d2bf0` preserves that flag at texture+0x20.
The sampler dispatch at `0x7def47..0x7def82` indexes three source table fields
using texture flags & 3. The operand locations `0x7def50`, `0x7def61` and
`0x7def74` supply the table addresses read by the policy. The default record
selects point magnification/minification and no mip filtering. The enum meanings
are documented by [D3DTEXTUREFILTERTYPE](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dtexturefiltertype)
and [D3DSAMPLERSTATETYPE](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dsamplerstatetype).
The policy reads the operands and values at runtime; no table contents are copied
into production constants. Resource styles with nonempty FontFlags refuse this
scoped default-font path until their behavior is qualified.

Resource labels now select Bevy's per-root nearest atlas mode. This resolves the
two previous cutoff-boundary differences without an epsilon, alpha quantization
or FreeType dependency. Runtime evidence in `.scratch/pixel-font-point-20261103/`
shows zero differing upkeep inner-glyph pixels after diagnostic registration by
(-1,+1). That translation is never applied to layout or full-screen acceptance;
the actual one-pixel placement offset remains unexplained. The unregistered
full screen has 1,099,071 differing pixels and mean absolute channel error
14.77136257595486. Its two-pixel count increase reflects the preserved glyph
coverage at the still-offset placement, not synchronized parity.

Eight policy tests, five resource tests (one additional local-source test
explicitly ignored), two isolated GPU tests, strict native/WASM Clippy and both
application builds pass. The sampler GPU test covers changes and removal; the
alpha test covers faded text and surviving coverage. The mounted capture passes,
all 13 clock receipts preserve their clocks, and selected-frame repeats match
byte for byte. All runs were headless and muted. Current README images and
provenance use this run. Other text roots and native animation/RNG identity remain
open. Local static evidence is retained under `.scratch/native-font-metrics-20261103/`.

### Final-origin rounding trace (3 November 2026)

The source helpers at `0x7e3de0` (Y) and `0x7e3e50` (X) both multiply by
their viewport extent, add a sign-selected half and truncate toward zero. They
use height and capped width respectively; neither helper is a floor operation.
The operand bytes were independently decoded from the admitted image and kept
in `string-origin-rounding-operands.json`; bounded disassembly is in
`string-origin-rounding.asm`. The cached drawing path at `0x7ef9cd..0x7efa1c`
applies these conversions to the normalized justified origins, then constructs
the translation matrix. This is static evidence, not a live upkeep vertex read.

The half-pixel correction globals also have bounded source references retained
in `device-halfpixel-references.json`. Initialization at `0x7ec348..0x7ec3ca`
selects zero or negative-half corrections from a device property, with one
component always negative-half. These are backend coordinate adjustments;
they do not justify copying a (-1,+1) screenshot translation into layout.

The existing selected-text receipt now retains each text root's ancestor
centers, rounded extents and unrounded extents. The isolated capture at
`.scratch/pixel-font-origin-20261103/` shows upkeep's authored frame is
121.2500076×26.2500019 pixels before layout rounding, 121×26 afterward, with
center (1214.5,18). Its 84×16 text child then centers at (1215,18). The authored
FDF's unrounded horizontal center at this resolution is 1214.375. Thus the
recreation rounds the frame before centering its child, whereas the traced
native path rounds the justified origin. This identifies a concrete rounding
order discrepancy to resolve; it does not yet prove live native frame operands
or explain the vertical difference. Production positions are unchanged.

The mounted capture and feature-enabled strict native Clippy pass. All five
images match the prior point-sampled run byte for byte; repeated selected frames
match and all 13 acquisition clock receipts hold their clocks. No README image
replacement is needed for this diagnostic-only extension.

### Native placement observed and integrated (3 November 2026)

The new isolated reference run `.scratch/pixel-native-origins-20261103/`
reaches Prologue02 gameplay. A bounded read-only process probe identifies the
upkeep string through its text pointer and a font-list membership check, then
reads the string record and its 36 emitted vertices. Retained provenance,
probe source/executable hashes, frame operands and vertices are in
`origin-evidence.json`. The run uses private Xvfb, software D3D9 and a PulseAudio
null sink; it was stopped afterward. No game bytes were modified or uploaded.

Observed frame coordinates agree with the authored FDF: left 1153.75, width
121.2499619, height 26.2500751 pixels, with bottom at Y-up 928.7499046. The
justified origin is (1214.375,949.8749542); source rounding produces (1214,950).
The live string flags are 0x4, selecting the previously traced integer pair and
pixel-coordinate branches. The first quad runs from (-42.5,-16.5) to
(-30.5,-0.5). These are live operands and vertices, rather than a screenshot fit.

The native backend initializes its D3D9 glyph corrections from immediate
operands at `0x7ec3ac` and `0x7ec36a`. The policy now reads those operands.
Converting upward Y and integer pixel centers to Bevy's downward Y and
half-integer pixel centers gives the renderer offset; the half-integer center
is a raster convention, not authored content. See Microsoft's
[Direct3D 9 pixel mapping](https://learn.microsoft.com/en-us/windows/win32/direct3d9/directly-mapping-texels-to-pixels)
and [modern pixel coordinates](https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-coordinates).
The font baseline is derived from the source font's hhea ascender/descender
ratio and rounded as in `0x7f0c7d..0x7f0ceb`; the explicit native metrics receipt
now verifies that baseline as well as the six advances.

Resource labels preserve their normalized authored frames through justification
and bind the resulting leaf text transforms after Bevy layout. Their measured
frame widths therefore cannot introduce a second centering round. Glyph Y
positions use the derived baseline and Bevy's rasterized atlas offsets. This is
a recreation implementation choice preserving the observed coordinates; it does
not introduce a fitted translation, replace atlas bytes or add FreeType.

The mounted capture in `.scratch/pixel-font-placement-20261103/` passes. At the
actual screen coordinates, all 528 pixels in the union of the two upkeep glyph
ink sets match exactly: no registration, tolerance or alpha postprocessing. The
surrounding fixed diagnostic box has one brown border-art difference at
(1239,28), native RGB (0,0,0), recreation (3,2,1). This scoped glyph result does
not exclude that pixel from the full-screen gate. The entire unregistered frame
has 1,097,868 / 1,228,800 differing pixels, down by 1,203; mean absolute channel
error is 14.71206787109375. Native animation/RNG identity, border art and other
text roots remain open. The selected-frame repeat is byte-identical and all 13
clock receipts hold their clocks.

Ten policy tests and seven resource tests pass, including explicit local source
fixtures. Synthetic coverage includes independent origin/extent rounding at two
viewport scales, source baseline metrics and changed source correction operands.
Strict native/WASM Clippy and both application builds pass. Current README
screenshots and provenance use this run. No full-screen pixel-parity claim.

### Unit-panel default raster settings shared (2026-11-03)

The unit panel previously used fractional `FontSize::Vh` and the default filtered
atlas without the source alpha cutoff. Its source-resolved fonts and authored
heights now use the existing `PairPolicy` raster-height conversion, default
atlas filter and alpha reference through `hud_render::SourceFont`. The resource
bar uses that same component, replacing its private sizing pass while retaining
its own qualified spacing, baseline and placement pass. Sizing runs before UI
content, updates rich spans with their root and avoids changing unchanged font
components. Invalid size/viewport input is reported with source context.

The selected panel's SimpleString `Font` records are the two-field form. The
normalizer keeps those authored records rather than requiring a nonexistent
third argument. The default raster path admits that form and the explicit empty
`FrameFont` flag form; nondefault/malformed flags refuse. Face paths, heights,
style colours and shadows still come from the receipt-bound FDF/profile reader.
No font rasterizer dependency, asset constant or screenshot-fitted position was
added. The selected client remains Legacy 1.29.2.9231 with the SHA-256 qualified
above; source assets and extracted inspection copies remain local.

Eight synthetic unit-panel checks pass, including default/nondefault font-form
validation. The explicit selected-Human01 panel selection/retirement gate passes
and checks that the actual text roots bind the selected policy, filter and alpha
reference. Six resource checks pass with one explicit local-input ignore. The
final isolated, muted Prologue02 capture in
`.scratch/pixel-panel-raster-verified-20261103` passes and asserts physical pixel
sizing and rich-span size propagation. Strict native/WASM Clippy and both app
builds pass.

Against the unchanged 1280x960 original reference, exact whole-frame differences
fall from 984,904 to 982,482 / 1,228,800 and MAE from 12.3603662109375 to
12.249673936631945. Exactly 5,936 recreation pixels change, within
[363,780,782,958]. All 13 clocks hold and the selected repeat is byte-identical.
The selected frame also matches the preceding completed capture byte-for-byte.
All 2,640 upkeep diagnostic pixels remain exact after the shared-code extraction.
Five README captures and the full-frame overlay, diff and side-by-side were
reviewed; hashes and provenance are refreshed.

This qualifies the shared raster settings and their data flow, not panel text
parity. Panel-specific string flags, integer/fractional pair branches, baselines,
shadow geometry and exact text origins remain to be qualified. Native animation
and RNG identity, other text roots and remaining world/UI differences remain
open. Parser tests and scoped glyph results do not establish full-screen parity.

### Unit-panel simple-string branch and origins (2026-11-03)

Static inspection of the same SHA-qualified Legacy client connects the
`CSimpleFontString` constructor at `0x886400` (default flags written at
`0x8864ac`), its builder at `0x887f40`, wrapper at `0x95ff00`, flag translator at
`0x95f690`, Gx string creation at `0x7e2d90` and initialization at `0x7ebf20`.
The supported constructor/translator operands select integer pair advances and
pixel-coordinate placement without the alternate pair mode. The translator's
possible outputs cannot select the fractional placement bit. The policy reader
now validates those operands; nondefault widget font flags refuse. This is
shipped-code evidence, not an observation of live panel flags.

The unit-panel rule reader derives the constructor's horizontal/vertical font
enums from the wrapper's tests and output operands. Health/mana retain their
explicit matching horizontal and vertical setter inputs. Authored FDF
justification takes precedence; missing records use the native constructor
rather than the generic renderer's left/top fallback. Stat strings retain their
automatic width and left anchor, the status string its automatic width and right
anchor, and title/class strings their source center relationship.

Resource labels and panel labels now share the existing integer pair advance,
font-byte baseline and independent origin/extent rounding calculations. Panel
bottom-origin rectangles are converted to normalized screen-origin anchors
before leaf placement after Bevy layout. A synthetic two-resolution check covers
that conversion; authored/missing justification and changed native operands
have separate fixture coverage. No screenshot-derived translation, generated
font data or additional rasterizer dependency is used. Local static inspection
windows and their hashes are retained in
`.scratch/panel-text-placement-20261103/native-evidence.json`.

The corrected isolated Prologue02 capture in
`.scratch/pixel-panel-origins-fixed-20261103/` passes. Exact unregistered full-frame
RGB differences fall by 1,830 from 982,482 to 980,652 / 1,228,800, with mean
absolute channel error falling from 12.249673936631945 to 12.104810112847222.
Only 3,501 recreation pixels change, within [363,780,771,957]. All 13 acquisition
clocks hold, repeated selected images are byte-identical and all 2,640 upkeep
box pixels remain exact. The full diff, overlay, side-by-side and five README
captures were reviewed. Nine synthetic panel checks plus the explicit
selected-Human01 replay, nine rule tests (one local-input ignore), nine policy
tests (two local-input ignores) and six resource checks (one local-input ignore)
pass. Strict native/WASM Clippy and both application builds pass.

This qualifies the default branch and source-derived placement inputs, not all
panel glyph/shadow pixels. Class/attribute readouts, other text roots and world/UI
rendering still have residuals. Live panel flags and native animation/RNG identity
remain unobserved. The full-screen pixel-perfect objective remains open.

### Unit-panel region precision and automatic sizes (2026-11-03)

A fresh private original-client run retains eight selected-panel simple strings'
region bounds, automatic dimensions, widget flags and wrapped Gx string inputs.
The read-only probe is bounded to 2 GiB and 128 matches; it observes eight
matches after 409,444,352 bytes. The run uses the same SHA-qualified client in
Wine/Xvfb with software graphics, a private PulseAudio null sink, no network and
no host display/audio/GPU mount. Its owned container is stopped. This qualifies
these observed string inputs, not native Windows rendering or tick identity.
Source/runtime receipts and hashes remain local in
`.scratch/panel-text-residuals-20261103/native-evidence.json`.

The class line had matching glyph shapes displaced one pixel horizontally;
the agility value and Intelligence label had matching shapes displaced one
pixel vertically. The native wrapper at `0x95ff00` converts rectangle edges
before Gx justification at `0x7ec790`. Automatic title/class widths therefore
must resolve their actual left/right edges before computing the center; using
the entire info-panel rectangle changes rounding around half-pixel boundaries.
The dimension wrappers at `0x960450` and `0x9605f0` also distinguish automatic
string sizes from their rasterized bitmap bounds.

The region getters at `0x872540` and `0x872940`, with point arithmetic at
`0x873bb0`/`0x873c30`, retain an f32 midpoint between x87 calculations. A
single top anchor can consequently produce an opposite edge differing from
one direct height subtraction. The panel now retains source coordinates,
resolves those edge/midpoint operations, converts both edges into Gx space and
then applies justification and pixel rounding. Stat chains use the resolved
label/value bottoms. Title/class, portrait readouts, status and the title-linked
experience bar preserve their authored relationships. The source operands and
native observations drive this correction; no screenshot displacement or glyph
patch is applied. Synthetic fixtures exercise intrinsic left/center/right
anchors, vertical justification, two resolutions and half-pixel cancellation.

Twelve panel checks pass, including eleven synthetic checks and the explicit
selected-Human01 selection/retirement replay. The final isolated Prologue02
capture in `.scratch/pixel-panel-edges-final-20261103/`, strict native/WASM
Clippy and both application builds pass. Exact full-frame RGB differences fall
by 1,764 from 980,652 to 978,888 / 1,228,800; mean channel error falls from
12.104810112847222 to 11.98810763888889. Exactly 1,880 recreation pixels change,
within [366,809,781,958]. All 13 acquisition clocks hold, the selected repeat is
byte-identical and the frame matches the preceding completed capture. All 2,640
upkeep diagnostic pixels remain exact. The full diff, overlay, side-by-side and
five refreshed README images were reviewed.

The class-line diagnostic rectangle now has zero differences. Several labels
still have seven residual pixels at the lowercase `g` descender, with repeated
coverage differences across its occurrences. That pattern suggests a remaining
glyph-raster coverage issue; native cache bytes for this glyph have not yet been
compared directly. Portrait-frame/background residuals remain under readouts,
as do other UI/world/text differences and native animation/RNG identity. These
pixels stay in the full-screen count. No FreeType dependency was added.

### Bitmap-space implied quadratic midpoints (2026-11-03)

The repeated seven-pixel `g` residual is now qualified against the native font
cache. A private read-only run observes the 14-pixel, default-flag
`Fonts\FRIZQT__.TTF` face with baseline 11 and a 9×12 `g` bitmap. All 108 bytes
match both the installed FreeType 2.15.0 and the existing local bundled 2.13.2
probe; unpatched Swash differs at seven bytes. The selected executable and font
retain their previously qualified SHA identities. The owned Wine/Xvfb/software
renderer/null-audio/no-network container is stopped. Source/runtime evidence and
all original/generated font data remain local in
`.scratch/panel-glyph-coverage-20261103/`.

Untranslated outline commands agree exactly between Skrifa/Swash and FreeType.
The FreeType bitmap convention translates the raw outline to its bitmap bounds
before decomposition. Two implied quadratic endpoints below this glyph's
baseline consequently round one 26.6 unit lower than Skrifa's ordinary
untranslated decomposition. This explains the observed native bitmap; it does
not claim identical internal native execution from the cache observation alone.
The recreation now uses that bitmap-compatible rounding for fixed 26.6 path
coordinates in the already-used Skrifa 0.44.0 dependency. Integer unscaled and
HarfBuzz floating coordinates retain their existing midpoint behavior. A widened
sum handles the entire fixed-coordinate range without overflow. No glyph bytes,
character-specific correction, screenshot fit or FreeType dependency is added.
This intentionally qualifies the application's bitmap path, not general vector
outline equivalence with untranslated FreeType paths.

The corrected Rust output matches all 108 native `g` bytes. A separate 570-case
printable-ASCII matrix over six pixel sizes reduces differences against system
FreeType from 100 bytes across 40 cases to 31 bytes across 20 cases, with no
regressions in that set. The remaining cases are not declared equivalent to the
native cache. Six focused path tests pass: four existing contour-order checks
and two synthetic checks for whole-pixel translation and full-range midpoint
arithmetic. The focused harness avoids unavailable upstream font-test-data;
the complete upstream font suite was not run. Two isolated text alpha/sampling
GPU tests, strict native/WASM Clippy and both application builds pass.

The production capture `.scratch/pixel-bitmap-midpoints-20261103/` passes. Exact
full-screen RGB differences fall by 28 from 978,888 to 978,860 / 1,228,800; MAE
falls from 11.98810763888889 to 11.988053656684027. The 28 repeated lowercase-g
coverage residuals disappear. In total 1,281 recreation pixels change within
[469,25,1279,914], including the upper Log glyph and a treetop region; equal
clock receipts alone do not establish cross-run presentation identity. All 13 clocks hold, repeated bytes match and
all 2,640 upkeep diagnostic pixels remain exact. Thirteen selected-panel text
rectangles outside portrait readouts now have zero RGB differences. The portrait
readout rectangles retain background/frame residuals. Full diff, overlay,
side-by-side and refreshed README captures were reviewed. Other text/UI/world
rendering and native animation/RNG identity remain open; the full-screen count
excludes no pixels.

A second independent current-build run in
`.scratch/pixel-bitmap-midpoints-verified-20261103/` passes and matches all five
published-state PNGs byte-for-byte. The pre-patch/current comparison also changes
a small treetop region and the upper Log glyph. Those pixels remain included;
the matched clocks do not establish presentation identity across those builds.
