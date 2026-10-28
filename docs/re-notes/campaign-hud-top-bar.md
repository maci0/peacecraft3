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
