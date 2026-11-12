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
  The native lookup seam now distinguishes an absent map source (error) from
  an absent id in an admitted table (`None`, yielding native empty text).
  Invalid source encoding remains an error with the WTS label and entry id.

The initial implementation in `0f8cc7d5` still expanded canonical literals and
code tags before native dispatch. The migration below removes that earlier
lookup, preserves raw script data and supplies the map source to real consumers.
It does not establish complete native runtime or rendered string parity.

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
The formatter now uses those decoded branches through `RealOps::real_to_string`,
`NumericPolicy::with_formatting`, and the normal campaign client reader. The VM
validates both native signatures and performs its existing integer-to-real
argument conversion before invoking the formatter. An absent selected runtime
or unadmitted formatter remains an error; no host float formatter is used.

The profile reads both wrappers, the formatter, floor/truncation helpers,
rounding/large-limit cells, and integer format strings from the exact selected
image. Buffer capacity comes from the wrapper's buffer/cookie displacements;
default width/precision, negative/default precision, cap, radix/scale identity,
padding minima, separators, sign, fill and infinity marker come from their own
instructions/data. Arithmetic call targets must match the admitted numeric
policy. The existing wide `script_numeric::Binding` remains unchanged.

For ordinary magnitudes below the signed integer boundary, the implementation
extracts the integer/fraction parts, converts the generated scale with the native
integer helper, multiplies/adds through the existing integer real helpers and
truncates the nonnegative rounded result. This preserves the native floor's
observable result. Carry and negative-precision trimming follow their source
branches. The final `%*d.%0*d` uses integer decimal conversion: padding belongs
after an already-emitted minus sign. Precision zero still produces a decimal
separator and one integer zero digit.

The large branch repeatedly multiplies by its source-encoded factor, renders
the truncated integer and appends zeros. It ignores width and uses the raw
precision with a separate minimum, without the ordinary precision cap. The
source maximum-finite comparison emits the encoded infinity marker, including
for infinities. Zero-exponent fractions cannot reach an integer under the
admitted bounded scale/bias, so their fractional integer is zero while a
nonzero negative input retains its sign. NaN behavior remains refused. Every
output branch checks the derived native buffer capacity before allocating;
unsafe oversized native output is not replaced by a truncated string.

### Original-client formatter observation

A private copy of the current construction probe recipe was adapted before
changes; the selected Booty Bay map's main/terrain remain intact and only its
script and `(attributes)` script CRC were updated. The original map digest is
`e1c5984189cd046b112ae59b870cf1d585b2b2445368858ea64c7685d4180268`.
The generated script resolves against the selected common/Blizzard scripts.
Competing melee AI/victory are disabled only in this controlled diagnostic.

A fresh, networkless, device-free Wine/software-Mesa container with private
Xvfb and verified Pulse null sink captured 194 cases plus BEGIN/END, then
stopped. Each row has its ordered index, expression label, explicit string
boundaries and native `StringLength`; the collector checks identity, count,
order and exact byte length. The raw CRLF trace digest is
`b7c68f97df001314282011dbfd5134fd673af578d9588b800ae0c04f5a2df62b`.
Original assets, generated map and trace remain local.

The matrix covers signed/negative zero, dyadic ties, adjacent decimal literals,
carry, negative/zero/over-cap precision, signed padding through width 128, and
large values. Examples: `R2SW(0.125,0,2)` gives `0.13`;
`R2SW(-0.5,8,3)` gives `-    0.500`; precision zero on 0.5 gives `1.0`.
The literal 9.9995 gives default `9.999`, because selected literal/arithmetic
conversion delivers a lower input; reparsing with host floats would lose this.
Large-value precision 16 retains 16 zeros while the ordinary branch caps at 9.

A source-derived integer prototype matches all 194 rows. More importantly, the
explicit local-input regression
`real_formatter_selected_image_matches_original_capture_through_vm` reads the
selected image plus the retained probe receipt, installs the real numeric and
formatter policies, compiles every original expression, and compares the actual
`R2S`/`R2SW` VM results with all 194 captured outputs. This check passed before
the final gates. It remains explicitly ignored without its original local
inputs. This is original-client Wine behavior, not native Windows parity.

The static REA records are `ev_d6ef7de70a29cb3cd3012fc21ee7709660e7b3d8ff4f7cf5d1009ecd4af20655`
(identity) and `ev_8062272af1fb4e713a2ee624caa9b5bc9124fabad2911f9e07250cd01ebbb1b7`
(bounded formatter disassembly). Infinity/subnormal assertions are supported by
native-code reasoning and synthetic tests; they were not extra original-client
runtime cases in the 194-row matrix.

### Formatter validation (2026-11-13)

From `efa0dbce`, the isolated lane target passes complete `pcx-script`,
`pcx-trig`, and `pcx-play` suites with `pcx-play/test-fixtures`: 3,450 passed,
zero failed, 160 explicit ignores. All six new ordinary regressions actually run:
four source-policy/nonstock/capacity/lifecycle tests and two VM normalization/
signature tests. The separately selected original-capture replay passes one
named test comparing all 194 expressions through the real VM callers.

Strict native and WASM Clippy with all targets/features and both explicit-target
all-feature library builds pass. All eight touched Rust source hashes stayed
unchanged through those gates, run under verified private offscreen display and
null audio. Formatting and diff checks pass. No original data/copies are committed.
Application rebuild/mission checks remain downstream integration work; these
library gates do not establish full campaign or rendered parity.

## Remaining scope

S2I/S2R, case/substring boundaries and the full hash remain unimplemented. Additional map-string wrapper families remain refused until their
real native consumers are implemented. No native/runtime parity or campaign
validation is claimed.

## Initial StringLength validation (2026-11-13)

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

Before the raw-literal migration below, main integration ran both complete
affected package suites using caches
independent of the parallel worktrees: 3,293 passed, zero failed, 148 explicit
ignores. All four new byte-length, computed-reference, declaration/error and
prefix-parsing regressions appear by name and pass. Strict native/WASM
all-target Clippy, native/WASM library builds and the native campaign-probe
rebuild pass. Per-command exit receipts and source hashes are retained locally
under `/tmp/pc3-string-main-validation/`; that earlier run did not close the
literal-expansion, missing-entry or native runtime comparison gaps. The next
section supersedes its literal/table-entry behavior.


## Raw literal and native-consumer migration

Source/native-code evidence uses the same selected image above. Additional
static reads cover `0x469010` (raw string backing pointer), `0x4afe80` (the common
map-string wrapper), `0x493970` (GetLocalizedString), `0x493940`
(GetLocalizedHotkey), and the registered entry/call sites below. These observations
are not original-client execution. Disassembly and original bytes remain local.

`0x4afe80` obtains the selected map table, reads the string backing pointer,
returns null for initial null/empty input, and either returns the raw pointer or
performs one `0x708dc0`/`0x708d70` lookup. It does not recursively resolve the
result, query a UI dictionary, or replace `^^` markers. The selected disassembly
contains 31 direct calls to this helper, across 29 registered native handlers.
There are separately 16 inline prefix-parser call sites in 15 registered native
handlers. These sets include unsupported APIs; they are evidence inventories,
not permission to admit new natives.

| Implemented consumer | Native boundary evidence | Map lookup |
| --- | --- | --- |
| DisplayTimedTextToPlayer | `0x48fbe0`, helper call `0x48fc00` after recipient/local checks | Text once |
| SetPlayerName | `0x4a96d0`, helper `0x4a96e8` after player resolution | Name once |
| SetMapName / SetMapDescription | `0x4a9370` / `0x4a92a0`, inline parsers `0x4a93b1` / `0x4a92e1` | Header once |
| QuestSetTitle / QuestSetDescription / QuestItemSetDescription | Helpers `0x4a4aa8` / `0x4a4988` / `0x4a4928` | Text once after object resolution |
| QuestSetIconPath | `0x4a4a30` reads through `0x469010`, copies to field `+0x4c` through `0x468d70` | Raw string, no map lookup |
| DialogSetMessage / DialogAddButton / DialogAddQuitButton | Helpers `0x48f958` / `0x48f738` / `0x48f798` | Caption once after dialog resolution |
| TimerDialogSetTitle | `0x4acfc0`, helper `0x4acfe3` | Title once after dialog resolution |
| LeaderboardAddItem / LeaderboardSetLabel / LeaderboardSetItemLabel | Helpers `0x4a1d0d` / `0x4a2138` / `0x4a1fc3` | Label once after board resolution |
| SetCinematicScene | Entry `0x4a78a0`, parsers `0x4a78cc` and `0x4a7913` | Speaker and text independently, in argument order |
| SetAltMinimapIcon | Entry `0x4a6ea0`, parser `0x4a6ecc` | Path once; empty result leaves prior path |
| TriggerRegisterPlayerChatEvent | Entry `0x4adbb0`, helper `0x4adc29` | Match text once after trigger/player checks |
| GetLocalizedString | Entry `0x493970`, parser `0x4939b2`, lookup `0x4939c2`, dictionary `0x895a00` | Map once, then UI dictionary |
| GetLocalizedHotkey | Entry `0x493940`, raw pointer then profile lookup `0x4304c0` | Raw key; no map lookup |

The item-label call belongs to the registered entry `0x4a1fa0`, not the adjacent
`LeaderboardRemovePlayerItem` entry `0x4a1f60`: those functions have no padding
between them. Entry registrations plus bounded assembly were used to correct
that initial padding-based candidate. No broad policy follows from proximity.

The VM now escape-decodes literals while retaining their exact trigger tokens,
paired markers and payload. The existing AST `decoded()` API continues to drop
tag parts for its existing artifact callers; the new fallible `literal_text()`
is the evaluation path. Escapes inside paired payloads receive normal JASS
validation and decoding. Unsupported escapes or NUL remain errors, including in
manually constructed payloads. Bare lowered trigger identifiers retain their
exact spelling, including leading zeroes. Assignment, concatenation, equality,
user-function parameters/returns and parked argument tails retain raw values.
There is no persistent “already localized” marker: a later explicit native call
can consume a returned trigger token once again, as the native wrappers do.

`native_table_string` distinguishes a missing source from a missing entry.
`MapScripts::table_string`, used by existing presentation readers, retains its
prior API and missing-entry behavior. `native_with_strings` forwards a read-only
lookup service without converting arbitrary native arguments. Both localized
VM wrappers bind that service to their retained selected map. Recording,
selected-neutral and startup hosts preserve their own dispatch before passing
context onward; AI reads only its currently lent world, and preload preserves
its cross-program callback refusal. No source is invented while a world is not
lent. Consumer lookup occurs at its own branch, before resolved-byte budgets
and state retention; invalid-object/local-recipient guards remain ahead of it.

The UI dictionary bank has a distinct internal route for a key after map lookup,
so a returned `TRIGSTR_` prefix is not interpreted twice or rejected as an
unresolved original input. Direct uncontextualized bank lookup still refuses
map references. Hotkey input is independently kept raw. Missing UI keys retain
the checked bank fallback rules. No VM or WTS operation performs general UI tag
formatting. The existing rich-text importer preserves closed markers with notes;
actual native tag-formatting and rendered parity remain separate questions.

A local pre-change proof fixture passed three checks showing the old defect:
StringLength on a literal took two WTS lookups while the same supplied argument
took one; raw returns demanded WTS/tag services prematurely; and legacy AST
decoding dropped marker payloads. These checks demonstrated the old VM behavior,
not original-client parity. The new regressions assert the corrected boundaries
through the actual VM and real admitted host consumers.

## Raw-string migration validation (2026-11-13)

The isolated target above passes the complete `pcx-script`, `pcx-trig` and
`pcx-play` package suites with `pcx-play/test-fixtures`: 3,403 passed, zero failed,
157 explicitly ignored. This includes 3,008 play library tests and all 16 named
`native_string_` consumer/source-forwarding regressions. The native coverage
walk's negative fixture also passes: following context-aware dispatch must still
reject a name appearing only in a declaration table.
The same 16 focused consumer tests also pass with all features enabled, including
the native-dispatch tracing entry point.

The five affected integration targets (`custom_scripts`, `wts_materialization`,
`string_identity`, `map_script_ownership`, `map_scripts_limits`) pass while
retaining malformed-input, encoding, budget, source-diagnostic and owner-drop
checks. Raw return/Capture assertions remain raw; real `StringLength` calls
exercise demanded source/entry/encoding behavior. The first full run lacked
this worktree's local corpus paths and failed five existing trigger asset
checks. The successful rerun explicitly selected the existing local 1.28c and
1.29.2.9231 installations; no tests or asset checks were removed.

Strict all-target, all-feature Clippy checks pass for native and
`wasm32-unknown-unknown`; both package builds pass for explicit
`x86_64-unknown-linux-gnu` and WASM targets. Rust source hashes stayed unchanged
through those checks. Formatting and diff checks pass.

The app's shared script-session fixture now retains raw Capture arguments and
demands invalid WTS text through `StringLength`. Its `map_loading` and
`script_audio_session` integration targets require downstream validation in the
application build; they were not run by these library gates. No original client,
GPU window or audio playback was launched by those library gates. That migration
did not qualify native runtime/rendered parity or case/substring/hash behavior;
the separate formatter investigation above has its own evidence and gates.

Independent main integration runs all affected script, trigger and gameplay
package suites: 3,444 passed, zero failed, 159 explicit ignores. All sixteen
native-consumer/forwarding regressions execute, alongside the construction
transfer cases integrated in the same immutable source run. Strict native/WASM
all-target/all-feature Clippy and both library builds pass. The local corpus
inputs and per-step source hashes are recorded in
`/tmp/pc3-strings-construction-main-validation/`.

The first application session run found two stale fixture expectations: eager
Capture text and the column/span of the now-explicit `call StringLength` error.
Correcting those expectations leaves production/core source unchanged. Both
application integration targets then pass all thirteen session cases each,
including callback-ticket cases; the application test binary also rebuilds.
Precise diagnostic spans, failed-candidate rollback and owner release remain
asserted. Corrected receipts are in
`/tmp/pc3-strings-construction-app-validation/`, whose acceptance record links
the unchanged successful core checks rather than discarding the failed run.

The rebuilt unchanged Human01 and Undead02 pass their 300-second post-control
idle-script horizons. Orc02 and Human08 refuse earlier unsupported `ACcs` and
`ACwb` automatic acquisition respectively. These are current recreation
frontiers, not completed missions or rendered/original-client string parity.
Exact commands and executable identity remain in
`/tmp/pc3-strings-construction-horizons/`.
