# Default difficulty (`GetDefaultDifficulty`, `SetDefaultDifficulty`)

## Source
- Selected `common.j:1805-1806`: `native GetDefaultDifficulty takes nothing returns gamedifficulty`
  and `native SetDefaultDifficulty takes gamedifficulty g returns nothing`.
- Authored callers, selected `blizzard.j`: 5201, 5216, 5255 and 5326. The defeat quit
  (`CustomDefeatQuitBJ`, 5320-5328) runs `PauseGame(false)`, `SetGameDifficulty(GetDefaultDifficulty())`
  and `EndGame(true)`.

## Native fallback (selected `Warcraft III.exe`, Legacy 1.29.2.9231)
- The name string, signature `()Hgamedifficulty;`, and primary handler are registered as
  push-immediate arguments before the registrar call. Registration is located by name, not by a
  recorded address.
- The primary handler calls the singleton lookup. When the lookup fails, it returns the immediate
  of `mov eax, imm32` before `ret`. That immediate is the fallback ordinal.
- `pcx_play::default_difficulty_image::native_default_fallback` derives the ordinal from the selected
  image: name occurs once, exactly one registration carries the expected signature, and the handler
  has the fallback shape. Any other image fails loud.
- The ordinal is validated against the selected `ConvertGameDifficulty` domain at admission. It is
  not copied from a constant.

## Model
- `ObjectHost` keeps the default separately from the current difficulty. `SetGameDifficulty` does not
  change the default, and `SetDefaultDifficulty` does not change the current difficulty.
- Admission (`admit_default_difficulty`) validates the declarations, the source anchor, and the
  domain before storing. A failed admission leaves no value.
- `SetDefaultDifficulty` accepts only a `gamedifficulty` handle. A refused call changes nothing.

## Observations and app source
- `DefaultDifficultyObservation::NoProfile { native_fallback }` (`map_startup_host.rs`) is the only
  source. The campaign builder derives `native_fallback` from the selected client image it already
  reads (`campaign_inputs::prepare_new_game`).
- The app reads no `.w3p` or Battle.net registry profile. The native reads a profile entry through
  the `Difficulty` key. Static evidence: the registry prefix `Software\Battle.net\` and the
  `/Campaigns` and `.w3p` strings appear in the selected image. The app does not enumerate them.

## Not built (explicit unsupported)
- Saved-profile default. A player whose profile stores `Difficulty` gets the native fallback here,
  which does not match the original. This is a known fidelity gap, not verified parity.
- The `.w3p` entry format and its value decoding (`0x96A110`, `0x9692C0`, `0x4B2AB0` in the selected
  image) are unread. Loading them is future work and needs a profile source qualified from the
  selected install.

## Tests
- Synthetic PE tests (`default_difficulty_image_tests.rs`): derivation, name uniqueness, signature,
  handler shape, non-PE and truncated inputs.
- Gate `selected_executable_yields_the_native_fallback` (ignored; needs `PCX_WAR3_EXE`) checks the
  reviewed image.
- Startup settings tests cover no-profile admission, current-versus-default separation, and refusal
  of foreign sources and signatures.
- App: ordinary Quit runs `SetGameDifficulty(GetDefaultDifficulty())` before `EndGame(true)`, and the
  session clears in the same update.
