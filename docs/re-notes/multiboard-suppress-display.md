# `MultiboardSuppressDisplay`: a flag on the game UI object

Image: Legacy `Warcraft III.exe`, SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
(1.29.2.9231). Static reads only (`objdump`, `rizin`); no client was run. Observed means read from the
image, inferred means a reading of it.

## Decode

- Registration (observed): the name string is at `0xe76190`, signature `(B)V` at `0xe700b0`. The
  table call at `0x4a0204` pushes the handler `0x4a39a0` and calls `0x8ccc80`. A second table at
  `0xcbb4ec` registers the unsupported stub `0xcbda30`.
- Handler `0x4a39a0` (observed): gets the game UI object with `0x5ec5e0(0, 1)`, the object that
  `ForceCinematicSubtitles` writes through (`subtitles-option-consumer.md`). With no object it
  returns without an error. Otherwise it calls `0x5f5360(flag)`.
- `0x5f5360` (observed): stores the argument in `[ui+0x3f0]` and calls `0x5f3f90([ui+0x3ec])`,
  which re-applies the current multiboard (`[ui+0x3ec]`).
- `0x5f3f90` (observed): hides the previous board, then shows the new one only while `[ui+0x3f0]` is
  zero (`cmp [edi+0x3f0],0` at `0x5f3fb8`, jump to `0x5f409f` when set). Installing a board
  (`0x5f40cf`) and the board refresh `0x5f3ce0` (`0x5f3d22`) test the same word. A set flag keeps
  every multiboard hidden, including ones created or displayed later, until a script passes false.
- The flag is the stored boolean argument itself and is not tied to a particular board.
- Default (inferred): zero, because no multiboard is suppressed before a script asks. The
  constructor of the UI object was not read.

## Recreation

`PresentationState::multiboard_suppressed` (`crates/pcx-play/src/presentation_host.rs`) holds the
last executed boolean, `None` until a script sets it, like `cinematic_subtitles`. The host has no
multiboard natives and the recreation draws no multiboard, so nothing reads the flag yet: it is
stored faithfully and not rendered. A renderer of multiboards must show none while it is `Some(true)`.
A host without the presentation admission still refuses the native as unsupported.

Test: `multiboard_suppression_retains_the_executed_flag_without_a_default`
(`presentation_host_tests.rs`).
