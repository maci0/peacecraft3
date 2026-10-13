# Original glue frame coordinate evidence

Inspected 2026-10-14 using REA 6.3.0 and Ghidra 12.1.3. These are observations of shipped machine code and recovered pseudocode, not a recording of native execution. Screenshots are corroboration only; the coordinate result below does not depend on them.

## Exact artifacts

| Artifact | SHA-256 | VERSIONINFO |
| --- | --- | --- |
| Installed Legacy `Warcraft III.exe` | `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed` | Fixed FileVersion and ProductVersion **1.29.2.9231**; string FileVersion `1.29.2.9231`, ProductVersion `1.29.2.9231 (9d2368a59e)` |
| `wc3/game/app/game.dll` | `6d21bd9a0f9fbc8446f455c9e89ac994fed68174426fe608fcb9baefd4dec53c` | Fixed FileVersion and ProductVersion **1.26.0.6401**; string FileVersion `1, 26, 0, 6401`, ProductVersion `Version 1.26.0` |

Both records were parsed from the PE VERSIONINFO resource, including fixed and string records. These facts identify the individual PEs, not every archive/movie in a potentially mixed installation tree. The earlier 1.28c label for this particular `game.dll`, and a hard-coded 9232 label for this particular Legacy executable, are not established by their resources.

All addresses below belong to the Legacy executable, preferred image base `0x00400000`. For a relocated runtime image use `actual_module_base + (VA - 0x00400000)`.

## Frame rendering really uses 0.8 by 0.6

Ghidra confirms `FUN_00ab2970` at VA `0x00ab2970`, body through `0x00ab29c0`:

```c
out[0] = in[0] / 0.8f;
out[2] = in[2] / 0.8f;
out[1] = in[1] / 0.6f;
out[3] = in[3] / 0.6f;
```

The actual constants at VA `0x00e84160` are bytes `9a 99 19 3f cd cc 4c 3f`: little-endian float32 0.6 and 0.8. `FUN_00ab29d0` performs the corresponding point conversion; `FUN_00ab2a30` performs the inverse rectangle conversion. This is executable arithmetic, not merely a table containing plausible numbers.

The rectangle conversion has two direct calls, at `0x0086c80e` and `0x0086cb03`, inside `FUN_0086c5d0` (`0x0086c5d0..0x0086cbe9`). Original MSVC RTTI identifies `CFrame`'s vtable at `0x00ec7818`; its slot 9 contains `0x0086c5d0`. The same inherited routine appears in the `CTextFrame` and `CSpriteFrame` vtables. It passes converted bounds to `FUN_007d14d0`, which dispatches to the renderer object's vtable offset `+0x48`. This connects the conversion directly to frame presentation.

Evidence: rectangle conversion `ev_bdb8fc32fcbcc78fe0939dd74b61ba4ca8d62ea2b37f03dc0f8456fd620d2421`; CFrame presentation `ev_f1503c9aee834c09216755d892d64784a61357179a0ef3b1de28f3df420915bc`; constant bytes `ev_2edd8a0b5d59ed106a2ebb5b29e94361504a72a4cd1ccf32b5821191f99fd804`; renderer dispatch `ev_e645a99a2e642424060e59aa92f4debc02b953a1484a72b98214205c4730aff5`.

## Widescreen behavior is conditional

`FUN_0086c5d0` obtains display dimensions through `FUN_007d0820` and compares their ratio with `1.3333333` (4:3). When the aspect ratio is at most 4:3, or frame flag bit 0 at `this + 0x0c` is set, it uses the direct 0.8/0.6 conversion above.

For a wider display with that bit clear, its bounds conversion is:

```text
extra = (aspect / (4/3)) * 0.8 - 0.8
x_normalized = (x + extra/2) / (0.8 + extra)
y_normalized = y / 0.6
```

An authored X range 0..0.8 therefore occupies a centered 4:3 region: its normalized width is `(4/3)/aspect`. This is a concrete pillarbox branch. Later in the same function, globals at `0x010e8ec4` and `0x01161ae0`, frame bits `0x48`, a byte at `this + 0x75`, and a dynamic-type check select additional content viewport or expanded-bounds behavior. The investigation did **not** measure those live values for MainMenuFrame, so it does not prove that every screen always takes one branch.

`FUN_00ab28d0` independently implements the inverse wide-window position mapping, `x' = (x/0.8)*(0.8+extra) - extra/2`, leaving Y unchanged. `CLayoutFrame` routine `FUN_00872f80` calls it when a byte at its `this + 0x69` is set. This is evidence for authored extension support, not authorization to assume that the original main menu enables it.

### Background and authored UI take different paths

The original glue initializer `FUN_0055f370` creates `SpriteBackground`, anchors it to its parent with SetAllPoints, assigns the resolved `GlueSpriteLayerBackground_V%d` art with camera argument **0**, and explicitly adds flag bits **0 and 1** to the sprite frame at `+0x0c`. The same routine creates the top-left/top-right overlay sprites, assigns their skin art with camera argument **−1**, and explicitly adds **bit 1 only**. Therefore the background is deliberately eligible for the direct full-window conversion even on a wide display. The overlay construction does not explicitly add that bypass.

For MainMenuFrame itself, the name table at `0x010dc0b0` leads to factory `0x00540400`, constructor `0x00544f90`, CFrame base constructor `0x00875d00`, and CLayer constructor `0x0086b100`. CLayer initializes flags at `+0x0c` and `+0x10` to zero. The inspected main-menu initialization `FUN_00559c60` adds bit 1; it does not explicitly add bit 0. The parent-assignment routine `FUN_0086fe00` changes inherited bit `0x40`, not bit 0. These are bounded static paths, not proof that no other runtime write can change the fields. In particular, expressions that merge the `+0x10` flags and the previously identified globals still need a live observation.

For the reconstruction, the evidence supports separating the **full-window original 3D background** from the **authored UI coordinate space**. A centered 4:3 region is a conservative implementation of the observed wide-window branch for ordinary UI, not a requirement to put the entire scene inside a 4:3 box or to reject other resolutions. At aspect ratios at most 4:3, the inspected branch directly maps the authored X/Y ranges; uniform containment on portrait screens would be an application policy, not this original formula. Exact parity at all resolutions requires reproducing the remaining flag-dependent expansion and viewport selection. Independent full-window scaling of ordinary UI on a wide display misses the measured branch.

Evidence: glue background/overlay construction `ev_f47689b3e4b855d205277e54d8b9c615d2b52270a280438bd43525f39038f869`; main-menu constructor `ev_e8a999611a4d3744d1c6fc0713b6729eb71525ef6b1489d732c225ce28062a38`; CLayer flag defaults `ev_57f1c1235a7908f8db51c56197b5d99a3feb7d912db228c6fcf297f77fb43879`; parent flag update `ev_30496bfe9a22f6c317ec1770af9b6b57e9f15a2ca9a8f1084c737f378477332f`.

## FDF offsets reach relative anchors unchanged

`FUN_00899c50` is the frame setup path that binds parsed anchor records to named frames. Its error message is `Error, cannot find frame "%s" to bind point to!`. For each 0x1c-byte anchor record it calls:

```c
FUN_00873a10(record[2], relative_frame, record[3], record[0], record[1], 1);
```

There is no negation or normalization of the two stored offsets here. `FUN_00873a10` replaces an anchor, calls `FUN_00893910`, and queues layout invalidation. The constructor explicitly names `CFramePointRelative`, installs its original vtable, and stores the relative frame, relative-point enum, X offset and Y offset unchanged.

At `CFramePointRelative` vtable `0x00ec9a2c`, `FUN_00893970` evaluates horizontal anchors as left/center/right plus stored X offset times scale. `FUN_00893a70` evaluates vertical anchors as top/center/bottom plus stored Y offset times scale. Positive Y adds in the engine's upward coordinate system. Therefore a positive `+0.005` from the previous backdrop's bottom lifts the next top by 0.005: the successive authored step is **0.064 − 0.005**, not 0.064 + 0.005. Negative Y from a top anchor moves downward. The viewport conversion into an application with Y-down coordinates must perform the final direction change consistently.

When there are no explicit anchors and the parsed SetAllPoints flag is set, `FUN_00899c50` calls `FUN_00873750` on the parent layout. That routine clears existing points and creates two zero-offset `CFramePointRelative` records at opposite corners (enums 0 and 8), rather than inventing fixed screen coordinates.

Evidence: FDF handoff `ev_0fb93a3c0d2a351c4f6299bbb071e3afd875df5fc9a074a04757635e83bbca62`; relative-point constructor `ev_81f9ec9373354ffcb986d57d67a0ba517dabbbec2a740d70ac2b1f1ff3e8ebdd`; horizontal evaluator `ev_b63dabf639707b222a8bbe76a2f7bb9e660c5cbb26f91a0edeb96ba67ab87e6c`; vertical evaluator `ev_b556fedb315933a74196d3e1e7727a670b8d994da91352a0a391a6f0476bcd74`; SetAllPoints `ev_15c61dc7f140761427be978eecdd066787ef113032cd0c7c50fd1b86eb72f2dd`.

The literal names `SetPoint`, `SetAllPoints` and `FrameFont` also exist in the Legacy lexer table, at `0x00ec6610`, `0x00ec6600` and `0x00ec6274`. Their presence alone was not used as handler proof. The full lexical token-to-enum parser was not reconstructed in this pass.

## Font height follows the same vertical extent

The common frame setup path calls `FUN_00899a00`; it passes the stored font name, font height at definition offset `+0x8c`, and flags at `+0x94` to `FUN_0086f990`. That setter calls `FUN_009600c0`, which converts the supplied height through `FUN_00ab2a10`: **height / 0.6**. It then passes that normalized height to the font backend `FUN_007e2c40`.

Thus an authored font height 0.013 corresponds to `(0.013/0.6) * authored_viewport_height`, not 1.3% of arbitrary full-window height. Font face selection, glyph metrics, antialiasing, shadow and fixed-size flag behavior still require their own parity checks; this result establishes the height conversion, not identical rasterization.

Evidence: FDF font handoff `ev_c9eeea85bf2fa542b6dacf32d6adf66a1cb610fbfeb1b6a551f8957a6c4953b8`; frame setter `ev_3a2700ec8b4564f8c95befc80a9ac3a451458b1e6c0d7ec654f00f9a0f693043`; font creation `ev_446d5c89805ec85157f54893eb9afb5be5d634220b15f1f10f0c84592679bbb3`.

## Sprite/logo placement: bounds and actual default projection

Original RTTI identifies `CSpriteFrame` vtables at `0x00ec7994` and `0x00ec7a8c`. Its layout width/height getters (`FUN_008781b0`, `FUN_00878100`) first honor explicit dimensions; otherwise they obtain the model's scaled bounds through `FUN_00965c00` and return max minus min. The bounds routine gets model bounds via `FUN_009195d0` and applies the model object's scale.

The extracted TFT logo model (`/tmp/opencode/tftlogo/logo_exp.mdx`, matching `web/assets/menu-exp/warcraftiiilogo_exp.mdx`) has SHA-256 `578245c64bc103b4fc2f9cf1b2ae6104935584a91a88d7f03874c63cf9537800`. Its MODL radius is zero, but its **bounds are not zero**. At MODL payload offsets 344 and 356, min/max are approximately `(-0.11209, -0.05820, 0)` and `(0.11146, 0.05330, 0)`. They match its four VRTX positions: width **0.22355**, height **0.11150**. This model has no CAMS chunk. A zero radius must not be mistaken for absent extents.

`FUN_00873b00` stores layout bounds in bottom/left/top/right order at CLayoutFrame `+0x44/+0x48/+0x4c/+0x50`. CFrame embeds that layout at `+0xbc`, so CFrame `+0x100` is bottom and `+0x104` is left. `FUN_008771f0` selects an authored camera when the camera pointer at `+0x14c` exists; otherwise it dispatches to the default projection `FUN_00877220`:

```text
originX = frame.left   + global_01161b4c
originY = frame.bottom + global_01161b50
```

That default path calls `FUN_00973d30`. The latter centers an orthographic projection on the clipping rectangle, using `FUN_007e6480`, then sends an identity model-view translated by `(originX - clipCenterX, originY - clipCenterY, 0)`. It also reverses the depth axis. The clip-center subtraction cancels the projection's center; it does **not** recenter the logo's raw min/max. Consequently the default XY mapping is `frame.left + modelX` and `frame.bottom + modelY`, with the separate model transform and global bias included when present.

For the shipped logo's FDF TOPLEFT offset `(0.13, +0.04)` relative to the authored screen top-left, **conditional on unit model scale, zero glue bias and the default camera path**, frame top is `0.64` and bottom is `0.64 − 0.11150 = 0.52850`. Thus the original quad occupies:

```text
X = 0.13 + vertexX
Y = 0.52850 + vertexY
normalized left   = (0.13 - 0.11209) / 0.8 = 0.0223875
normalized top    = 1 - (0.52850 + 0.05330) / 0.6 = 0.0303333
normalized width  = 0.22355 / 0.8 = 0.2794375
normalized height = 0.11150 / 0.6 = 0.1858333
```

These are ratios derived from the original projection and authored bounds, not a screenshot fit. An equivalent relative-flow quad must preserve this origin convention; subtracting the model minimum or placing the model center on the FDF anchor changes the result. Actual entrance animation, live model scale and glue-bias values were not sampled in this pass.

Evidence: implicit height `ev_8271d88247c1056c09b00a125d770221b0224e7c839acdded526e4b628026c9d`; implicit width `ev_da7c1e9511a2a883997762783a168a99f2be49d778e1e0c87ba054795b428838`; scaled bounds `ev_ad57c50b1802d8c38211c9d5570966e47969650eb9259418dd0c53fda96a8325`; layout storage `ev_5fd35e49a5637480d9241188ee6eacb90a79c23572a34bdfde41e035707413d9`; default origin `ev_ac43f6c39b549459422828db9fdce9f4649b2ef24cd71e6e8fd496eec9de4226`; projection/model-view `ev_efa82af985c14dd4d3f827da026c4b444a53c516b9d48e12cc059e870358c00c`. Original bytes and assembly excerpts are retained alongside `tft-logo-model-bounds.json` under the ignored evidence directory.

## Logo art is selected through the skin table

Both inspected glue initializer paths (`FUN_00557e60` and actual main-menu `FUN_00559c60`) find the `WarCraftIIILogo` frame, construct `MainMenuLogo_V%d` with the integer at `0x010db3e0`, resolve that key through `FUN_005c30a0`, and call the sprite art setter (vtable offset `+0xf0`) with the resulting path, camera argument −1 and callback zero. Their logo-specific segments do not explicitly resize or recenter the frame. This bounded trace does not exclude changes elsewhere.

`FUN_005c30a0` checks CustomSkin when enabled, then the active skin at `0x0113d1c8`, then the default skin. `MainMenuLogo_V%d` is a **skin key**, not a filename. The extracted shipped `war3x-war3skins.txt` and localized counterpart map:

```text
MainMenuLogo_V0 = UI\Glues\MainMenu\WarCraftIIILogo\WarCraftIIILogo.mdl
MainMenuLogo_V1 = UI\Glues\MainMenu\WarCraftIIILogo_exp\WarCraftIIILogo_exp.mdl
MainMenuLogo_V1Beta = UI\Glues\MainMenu\WarCraftIIILogo_exp\WarCraftIIILogo_expbeta.mdl
```

The `%d` call produces a numeric suffix; the Beta key's presence alone does not establish a path that selects it. The live selector value was not measured. Table hashes are `7b685b0d2c1129d63de0b116af0958368e7a053e7c616606c007bc2ec1876c14` (war3x) and `476a485dc692a3312fdd48b292512874d94eacf0b237cd13bf33b1a4fd17bd6f` (localized), from ignored `/tmp/opencode/skins/` extractions; archive/build identity must be retained separately from the executable's VERSIONINFO.

Evidence: first glue initializer `ev_311569d97321b598b3cf56eb568f96be57d7b3c6c3738b922aaa237f620736ad`; main-menu initializer `ev_ca98677f5c61c41e0ae3abd74e77aa60a9424ec8f53c516b3fd1a640b0d25e2a`; skin resolver `ev_8edb0e78d5e9b049fdc81dea76cdd52e5b2d213e6ce4135c2b1f9a1f6f41633c`.

## Evidence retention and a minimal native probe

Complete REA results, raw VERSIONINFO, original-byte scans, disassembly and RTTI records are retained under ignored `.scratch/glue-coordinate-runtime/`. The matching REA snapshot is keyed by the Legacy executable's SHA-256 under `.rea/snapshots/`. No original binaries or screenshots are published with this note. Function names beginning `FUN_` are generated analysis names, not recovered source symbols. Provisional artifact filenames are navigation aids, not semantic proof.

To settle the remaining runtime choices, launch this exact executable in an owned Wine/debug session and inspect the actual MainMenuFrame:

1. Break at the rebased `0x0086c5d0`, identify the frame through its name pointer at CFrame `+0x16c`, and record flags at `+0x0c`, globals `0x010e8ec4`/`0x01161ae0`, and bounds passed through `0x0086c80e`.
2. Repeat at 4:3 and 16:9 to observe which conversion branch the menu actually selects. Preserve the values and call stack; rendered pictures only supplement those records.
3. For `WarCraftIIILogo`, capture actual camera selection, model scale, global bias at `0x01161b4c/0x01161b50` and final transform alongside the bounds. The default static origin is now established; these observations determine whether the conditional numeric quad above is the live result.

A bounded native probe was attempted using the already-existing owned `.scratch/menu-wine` prefix and installed `winedbg`, without modifying the user's Faugus prefix or installing another runtime. The exact executable loaded at `0x00400000`; the debugger set a main-menu initialization breakpoint at `0x0055a1a4`. It did **not** reach the breakpoint during the 25-second limit. The log recorded a MiniDump thread and `DrawCaptions`, but neither identifies the startup cause. No live flag, selector or scale values were obtained. The debug session ended; no original Warcraft process remained in the post-check. Commands and log remain ignored as `native-menu-probe.txt` and `native-menu-probe.log`.

Static frame extent, offset addition, font height and the default sprite projection are established. Actual menu flag/global combinations, live logo scale/bias and exact rasterization remain open. A successful native startup reaching the identified breakpoint is the concrete next probe; repeated launches or a screenshot adjustment cannot replace those values.
