# Authored map-music selection

`ObjectHost::with_map_music_selection(max_name_bytes)` separately admits the
nonconstant `SetMapMusic(string,boolean,integer)->nothing` contract in the retained
Program. No selection exists before an executed call. `map_music_selection()` then
exposes the exact name, random flag and index from the latest successful call.
No playlist, active track, playback position, index origin or default is invented.
In particular, the authored index is retained even when the random flag is true;
it is not reported as a selected track index.

The caller supplies a logical UTF-8 byte limit, checked for platform layout at
admission and before copying each request. This is a reconstruction input policy,
not the native buffer capacity or a total heap bound. Wrong argument types,
embedded NULs, incompatible/overridden/ambiguous declarations and exhausted
limits refuse. Invalid calls leave the previous selection intact. Copying is
fallible and completes before the state changes. Empty selection text is retained
as empty text, without assigning it a clearing or fallback meaning.

This is selection storage for a future music consumer. Track expansion, starting
or stopping music, random selection/RNG work and music completion are not
implemented here. The existing source-backed sound-start queue remains separate;
no audio device or playback state is fabricated. Default launcher admission is
still open.

## Source evidence and scope

The same retained Human01 Program actually calls `SetMapMusic` with the string
`Music`, random true and index zero. The call is in map script span
`384467..384503`; its common declaration is `157693..157798`. A bounded census of
selected `War3.mpq` listfile-named `UI\SoundInfo\*.slk` tables decoded 694,041
bytes without finding an exact `Music` data-cell match. This is a bounded negative,
not proof that music is absent, that a particular table/column is the resolver,
or that another namespace has priority.

Static inspection uses only the local Legacy executable with SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
own VERSIONINFO `1.29.2.9231`. The first named `SetMapMusic` registration has
signature `(SBI)V` and selects `0x4a9340..0x4a9362`: 34 bytes, 13 instructions,
SHA-256 `79debb40a793e49002543118d1f313c880c0758dd2b2731d32d87611e6f21c49`.
Its wrapper uses a sentinel instead of the supplied index when random is true,
resolves the string and calls `0x5da520`.

That complete callee is `0x5da520..0x5da547`: 39 bytes, 12 instructions, SHA-256
`a7931c9bb85466fe53ed47cb37e0d0070273f7ceb58517366153259853f83490`.
It calls another, uninspected helper, copies the supplied text to a global buffer
and stores the supplied effective index. Capstone and independent objdump
boundaries agree for both bodies. No measured address, capacity or sentinel is
embedded in production code. The uninspected helper's effects, later resolver,
alternate registration family and actual playback remain unknown. Nothing was
executed from the game or a DLL.

## Verification

Four synthetic test groups cover exact selection replacement, absence of implicit
state or track expansion, UTF-8 byte boundaries, atomic failure, NULs, malformed
arguments, duplicate admission, signature/override failures, platform layout and
retained-VM state after a later unsupported call. All four execute in actual
32-bit WASM: mask 15, zero imports. The full play suite passes 277 tests with five
explicit original-input ignores. Strict native/WASM all-target/all-feature
Clippy, library builds, rustdoc and formatting pass.

The same original-map gate still completes globals and `config`, creates both
source-backed ambient sounds, retains the night sound's start request, and now
retains the executed map-music selection. It reaches `CreateSound` in map script
span `41999..42103` after 555 cumulative native calls and 446 arena identities.
That next declaration is common span `155322..155512`. This gate proves retained
Host inputs and script progression, not audible behavior or campaign completion.

Actual commands, source-qualified static ranges, table census and captured
native/WASM results remain local in `.scratch/map-music-root/`. No original
script, table, executable or audio contents are committed or uploaded.
