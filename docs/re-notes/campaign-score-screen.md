# Campaign score screen

Status: `EndGame(true)` retains an exit snapshot for the source-backed score
layout and background-model consumer. OK dismisses the screen; Save Replay
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
- Skin: `UI\war3skins.txt` is listed in the archives. Its `[Default]` section
  names the defeat model (`ScoreScreenDefeat=UI\Glues\ScoreScreen\ScoreScreen-Defeat\ScoreScreen-Defeat.mdl`).
  Race sections name victory models. The campaign skin choice is not yet
  established.
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
