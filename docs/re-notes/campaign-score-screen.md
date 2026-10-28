# Campaign score screen

Status: `EndGame(true)` retains an exit snapshot for the source-backed score
layout, background-model and race-art consumer. OK dismisses the screen; Save Replay
refuses because no production replay writer exists. Player-score admission
retains the selected constants, but every counter remains unqualified and absent.
Rendered placement and original-client parity remain open.

## Source evidence

- Request: `common.j` declares `EndGame(boolean doScoreScreen)`. The defeat
  dialog's `CustomDefeatQuitBJ` calls `EndGame(true)` (`Blizzard.j`). See
  [game exit](script-game-exit.md).
- Layout: `UI\FrameDef\Glue\ScoreScreen.fdf` (17117 bytes, 455 lines, `War3.mpq`
  member). It includes `UI\FrameDef\Glue\StandardTemplates.fdf` (45278 bytes)
  and `UI\FrameDef\UI\EscMenuTemplates.fdf` (16134 bytes). Include resolution is
  not yet verified against a loader.
- Top-level frames: `ScoreScreenFrame` containing `ScoreScreenMainPanel`, with
  background, page tab, 5-column and 4-column header frames, race art, top bar,
  four tab text and button pairs, elapsed time label and value, team toggle and
  color button, Save Replay, Replay Saved, OK and a scroll bar. `SaveReplayPanel`
  is a second top-level frame.
- Strings for the frames exist in `Warcraft III.exe` (for example
  `ScoreScreenBannerWin`, `ScoreScreenBannerLoss`, `ScoreScreenDefeat`,
  `ScoreScreenOkButton`).
- Skin: `UI\war3skins.txt` (`War3.mpq`). `[Default]` names `ScoreScreenDefeat`,
  `ScoreScreenVictory` (the Human model), `ScoreScreenBannerWin/Loss` and
  `ScoreScreenPlayer<Race>` textures. Each race section names
  `ScoreScreenVictory_V0`, `_V1` and `_V1Beta`. The `_V1` models are in `War3x.mpq`.
  The executable carries the key strings `ScoreScreenVictory`, `ScoreScreenDefeat`,
  `ScoreScreenBannerWin`, `ScoreScreenBannerLoss` and `ScoreScreenPlayer%s`.
- Textures and models named by the FDF and skin are listed in `War3.mpq`. The
  `show` subcommand returned identical size and hash for both `War3.mpq` and
  `War3x.mpq` when queried individually, so the archive that supplies each
  member is not yet established.

## Not established

- Score values. Row contents and column meanings are not yet derived. See [player score counters](player-score-counters.md): `GetPlayerScore`
  refuses counters without a verified source.
- Button semantics. OK returns to the caller in the original; Save Replay and
  Replay Saved have no implemented replay writer here, so they must refuse.
- Restart, load and campaign transition after the screen.
- Rendered placement, fonts and text. No rendered comparison exists.

## Integrated request lifetime

The exit consumer retains the request, local result, logical game time and source
context before clearing the session. The score consumer takes each snapshot once.
Source replacement and dismissal release the prior scene. Unsupported score
values remain absent; they are not zero-filled.

## Skin and model art

Source: `src/campaign_score_model.rs`, `src/campaign_score_art.rs`.

- Art reads through the exiting session's selected resolver, never the install
  directory. The screen root carries the resolver as `ArtSource`, so FDF textures
  (backdrop and highlight files) and the model textures decode from it. Decodes
  are dropped when the screen is replaced or dismissed.
- Three `MODEL` frames name `BackgroundArt` in the FDF: `ScoreScreenBackground`,
  `ScoreScreenPageTab` and `ScoreScreenBarTop`. Placement comes from the frame
  geometry; `LayerStyle` `NOSHADING` makes the geosets unlit.
- `ScoreScreenRaceArt` is a `MODEL` frame with `SetAllPoints` and no
  `BackgroundArt`. Its model is not in the FDF. Recreation choice, not proven
  against the executable: it shows the skin's `ScoreScreenVictory` or
  `ScoreScreenDefeat` model by the local game result, drawn in this frame's
  rectangle and draw order (after the tabs, before the top bar).
- Skin lookup, shared with the HUD and menus and equally unproven against the
  executable: the local player's race section (from the executed
  `SetPlayerRacePreference`, captured at exit), then `[Default]`. Within a
  section the edition key is tried first (`_V1` Frozen Throne, `_V0` Reign of
  Chaos), then the unsuffixed key. Victory finds the race section's model;
  defeat finds the `[Default]` model. A duplicate or empty entry refuses.
- Not drawn, each logged: a tie, neutral or absent local result (the skin has no
  key for it), an unset or ambiguous race preference, and any model or texture
  the resolver cannot supply. The other backgrounds are unaffected.
- Archive models `ScoreScreen-BottomBar`, `ScoreScreen-CenterBar` and the
  banner and player-race textures are not named by `ScoreScreen.fdf`. They belong
  to the score rows, which need counters that are not available, so they are
  not drawn.
- `_V1Beta` entries are ignored: no beta edition is selected.
- Tests: synthetic skin tables for selection (edition, race fallback, duplicate,
  missing, tie); synthetic FDFs for frame order and the race art frame. Original
  gate `PCX_SCORE_ART_DIR` (local directory with `UI\war3skins.txt` and the
  victory and defeat models plus textures, archive layout) prepares every race
  and edition for drawing. Rendered output has not been compared.
