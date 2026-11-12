# String native decoding

Image: Warcraft III.exe 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Static assembly observations; no original-client execution.

## StringLength implementation and trigger lookup

`StringLength` registration at `0x49b563` selects handler `0x4ac5d0`.
It returns zero for null/empty strings. Otherwise `0x708dc0` recognizes a
trigger-string reference and `0x708d70` resolves its table entry before
`0x433910` counts bytes to the first NUL. This is byte length, not a Unicode
scalar count. `NativeImpl::StringLength` now dispatches through the VM with
signature validation, the existing host WTS seam and byte/NUL counting.

Follow-up static evidence (2026-11-13), from GNU objdump 2.47 against the selected
image with its SHA-256 checked again:

- `0x708dc0` obtains the length of the NUL-terminated prefix at `0xea85f0`
  and compares that many bytes through `0x4339f0` -> `0xa77520`.
  This is an exact, case-sensitive prefix comparison.
- The suffix goes to `0x434390`. Only an initial minus is consumed; neither
  whitespace nor plus is skipped. Decimal digits accumulate with wrapping
  32-bit multiply/add, stopping at the first other byte. No digits means zero.
  A negative suffix negates the wrapped accumulator.
- The handler treats the parser result `-1` as no reference. Other results
  are looked up once, including zero, negative bit patterns and wrapped ids.
  A WTS body is not recursively resolved and no code-tag expansion occurs
  in this handler. The new `native_trigstr_id` intentionally differs from
  `trigstr_id`, which still describes the editor's exact identifier syntax.
- `0x708d70` returns the empty string at `0xe68b80` on a missing entry.
  The host API currently cannot distinguish an unavailable WTS source from
  an absent entry, so this implementation retains the existing explicit
  missing-WTS error rather than silently treating missing source data as empty.

The VM still expands canonical trigger literals and code tags eagerly, before
native dispatch. Ordinary literals and direct/computed native arguments are
covered; a WTS body containing another trigger reference can therefore resolve
an extra time when entered through that earlier literal seam. Moving expansion
to consumers is separate work affecting every string consumer. This change does
not claim complete native string parity.

At the start of inspection, REA reported the configured Ghidra installation
unavailable (missing application properties/decompiler); these observations are shell-tool
static assembly evidence, not REA Evidence records or original-client execution.
The executable and disassembly remain local; no source asset bytes are committed.

## Case, substring and hash qualification

`StringCase` registration at `0x49b577` selects `0x4ac4c0`. It shares the
trigger-string resolution above, copies through `0x433500` with bound `0x400`,
then selects `0x434600` for true or `0x4339e0` for false. The copy helper uses
`min(strlen(source), bound - 1)` and always appends NUL for a nonzero bound;
it does not preserve a UTF-8 boundary. Null/empty input returns a null string
handle. Non-ASCII conversion and locale behavior still need decoding; the
native remains refused.

`SubString` registration at `0x49b54f` selects `0x4ac8f0`, which forwards to
`0x4699c0`. That helper counts bytes, narrows indices and source length to
16-bit values and uses signed comparisons (`0x469a18`..`0x469a37`) before
constructing the result by temporarily writing a NUL at the end position.
Negative/reversed indices and large lengths need further qualification; an
ordinary Rust character slice or unconditional unsigned clamp is unsupported.
This native remains refused.

`StringHash` registration at `0x49b58b` selects `0x4ac5a0`. Null/empty returns
zero; otherwise it calls `0x433890`, without the trigger-table lookup used by
length/case. The helper uppercases ASCII a..z, replaces slash with backslash,
and admits at most `0x3ff` bytes into its stack buffer before calling
`0x4347f0` with zero as its third argument. Non-ASCII bytes pass through
this preprocessing.
The full hash is not yet implemented, so this native remains refused.

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

S2I/S2R, remaining substring boundaries, localization natives and typed player counts
remain unimplemented. No native/runtime parity or campaign validation is claimed.

## Validation (2026-11-13)

Validation uses this worktree's `crates/pcx-script` source and the isolated
`CARGO_TARGET_DIR=/tmp/pc3-fidelity-sol61-01-target`; shared-cache logs are
superseded. `cargo test -p pcx-script --jobs 2 --offline` passes: 158 tests,
zero failures, five explicit original-input/doc ignores across all targets.
The new VM regressions cover null/empty text, UTF-8 byte counting, NUL termination,
computed trigger references, suffix parsing/overflow, one-level lookup, raw tags,
empty WTS bodies, missing/malformed sources, signature mismatches and recovery
after an error. Two standalone text-module tests also pass with warnings denied.
These are synthetic parser/VM checks, not native runtime or campaign parity.

Strict `cargo clippy -p pcx-script --all-targets --jobs 2 --offline -- -D warnings`
checks pass in that isolated target directory for native and
`wasm32-unknown-unknown` targets. Formatting and diff checks pass.

The isolated `cargo build -p pcx-script --target x86_64-unknown-linux-gnu
--target wasm32-unknown-unknown --jobs 2 --offline` also passes for both libraries.

Main integration also runs both complete affected package suites using caches
independent of the parallel worktrees: 3,293 passed, zero failed, 148 explicit
ignores. All four new byte-length, computed-reference, declaration/error and
prefix-parsing regressions appear by name and pass. Strict native/WASM
all-target Clippy, native/WASM library builds and the native campaign-probe
rebuild pass. Per-command exit receipts and source hashes are retained locally
under `/tmp/pc3-string-main-validation/`; these checks do not close the
literal-expansion, missing-entry or native runtime comparison gaps above.
