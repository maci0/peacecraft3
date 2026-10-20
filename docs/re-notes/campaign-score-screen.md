# Campaign score screen

Status: evidence only. No score presentation is implemented. `EndGame(true)`
still ends the session and the exit consumer logs that the score screen is
unsupported.

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

- Score values. Row contents and column meanings are not yet derived. The
  per-player source is partly established: `GetPlayerScore` answers only
  `PLAYER_SCORE_UNITS_KILLED`, see [player score counters](player-score-counters.md).
- Button semantics. OK returns to the caller in the original; Save Replay and
  Replay Saved have no implemented replay writer here, so they must refuse.
- Restart, load and campaign transition after the screen.
- Rendered placement, fonts and text. No rendered comparison exists.

## Handoff needs

- A request snapshot taken before the session is cleared: `score_screen`
  (boolean), the defeat state, and the simulation time. The exit consumer
  owns the clear, so it must retain this snapshot before clearing.
- A score-data source that refuses, rather than zero-fills, any value the
  simulation does not track.
