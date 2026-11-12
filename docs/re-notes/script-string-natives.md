# String native decoding

Image: Warcraft III.exe 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Static assembly observations; no original-client execution.

## Length and case

Registration at `0x49b563` selects `StringLength` handler `0x4ac5d0`.
It returns zero for null/empty strings. Otherwise `0x708dc0` recognizes a
trigger-string reference and `0x708d70` resolves its table entry before
`0x433910` counts bytes to the first NUL. This is byte length, not a Unicode
scalar count. Trigger-reference integer parsing and lookup failures still need
qualification before implementing the native. No implementation is included.

`StringCase` registration at `0x49b577` selects `0x4ac4c0`. It shares the
trigger-string resolution, copies through `0x433500` with bound `0x400`, then
selects `0x434600` for true or `0x4339e0` for false. Copy termination and
non-ASCII conversion still need decoding; this native remains refused.

`StringHash` registration at `0x49b58b` selects `0x4ac5a0`. Null/empty returns
zero; otherwise it calls `0x433890`. The full hash is not yet implemented.

## Real formatting correction

`R2S` handler `0x4a4b10` calls `0x9a0870` with width zero and precision three.
`R2SW` handler `0x4a4b80` forwards width and precision to that same routine.
The earlier chapter-startup note's suggestion of a simple `%.3f` formatter
is insufficient: the selected handler actually uses a custom conversion.
At `0x9a0888` a negative sign is emitted only when the sign bit is set and
the remaining bits are nonzero, so negative zero has no minus sign.
The ordinary magnitude branch defaults negative precision to six and caps it
at nine (`0x9a0980`..`0x9a09e4`), computes a fractional rounded integer through
the game's real arithmetic helpers, and handles carry at `0x9a0abb`.
Negative precision also removes trailing fractional zeros down to one digit
(`0x9a0ac4`..`0x9a0af2`). Large magnitudes take a separate path at `0x9a08b1`.
Exact arithmetic-helper rounding and exceptional values remain unqualified;
both formatting natives remain refused rather than use Rust formatting.

## Remaining scope

S2I/S2R, substring boundaries, localization natives and typed player counts
remain unimplemented. No native/runtime parity or campaign validation is claimed.
An ordinary-string length implementation passed 155 script tests with zero failures
and five ignores, and the strict play/app Clippy check passed. It was discarded
for the release because the required play and downstream suites had not completed.
These validation results describe the discarded candidate, not native fidelity.
