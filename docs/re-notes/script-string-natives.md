# String native decoding

Image: Warcraft III.exe 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Static assembly and explicitly scoped original-client observations below.

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
handle. The called policy below qualifies the initial C byte profile; other effective
locales and operating-system NLS conversion remain unqualified.

`SubString` registration at `0x49b54f` selects `0x4ac8f0`, which forwards to
`0x4699c0`. That helper counts bytes, narrows indices and source length to
16-bit values and uses signed comparisons (`0x469a18`..`0x469a37`) before
constructing the result by temporarily writing a NUL at the end position.
The called policy below preserves the signed branch order and safe reversed
ranges. Effective negative pointer offsets remain refused; Rust character
slicing or an unconditional unsigned clamp does not describe this helper.

`StringHash` registration at `0x49b58b` selects `0x4ac5a0`. Null/empty returns
zero; otherwise it calls `0x433890`, without the trigger-table lookup used by
length/case. The helper uppercases ASCII a..z, replaces slash with backslash,
and admits at most `0x3ff` bytes into its stack buffer before calling
`0x4347f0` with zero as its third argument. Non-ASCII bytes pass through
this preprocessing.
The complete hash now runs through the real VM builtin and a selected-image
`ScriptHashPolicy`, installed by `campaign_client::read` and map startup.
The readonly Host hook refuses nonempty input without that policy. Initial null,
empty and NUL-first input return zero before policy demand. StringHash never
performs WTS, dictionary or tag lookup; raw trigger tokens remain raw.

The policy reads both raw/native wrappers, normalizer, hash core and tail jump
table from the complete fingerprinted image. Capacity, prefix cap, case range,
case delta, slash mapping, initial lane word, seed and all nine mix directions
and shift counts come from those windows. The parser checks call/data joins,
source ranges, byte-packing tail fallthrough, both mix copies and their register
roles. Unsupported source shapes remain errors. Three wrapping 32-bit lanes
consume little-endian words, add the normalized prefix length, pack the tail
with the third lane's low byte reserved, then run the final mix. The signed
integer result is the third lane. This is byte preprocessing, including prefixes
that end inside UTF-8; the byte policy does not decode or replace those bytes.

Borrowed/owned localization, recording, selected-map neutral wrappers, map
startup, lent AI and preload hosts forward the readonly policy. No AI native
whitelist expansion or preload cross-program callback relaxation is involved.
Missing policy and an unlent world remain explicit errors; pure builtins retain
the existing recording behavior without duplicate host-native events.

### Original-client byte-string observation

Bounded static REA evidence is
`ev_7a1f9f97eac380a4b5ac960dba5177c4f3c9f27340e8bffaa0c339d3eb290ec2`.
A private copy of the CRC-fixed diagnostic recipe retains Booty Bay main/terrain,
changes only script/attributes CRC, and resolves against selected common/Blizzard
scripts. One private offscreen Xvfb/Pulse-null Wine container captured 83 cases
plus BEGIN/END and stopped; Docker exited/PID-zero and unchanged raw trace hash
were verified before releasing the native slot. Raw trace SHA-256:
`5e565636ab170a9c70c164a05ab91e9e60e46378549dc5eb7638f10a95177cce`.
Original image, map, disassembly and captures remain local.

The matrix records ordered index/label, native hash/length and null/empty/equality
bits as ASCII metadata, so invalid UTF-8 results are not decoded lossily. Its 38
dedicated Hash expressions cover all tails, multiple full blocks, case/slash,
punctuation, UTF-8, raw trigger tokens and byte lengths 1022/1023/1024/1025/2048.
`string_hash_selected_image_matches_38_original_expressions_through_vm` reads the
selected image and explicit local receipt, verifies image/raw trace identities,
installs the real ObjectHost policy and compares every captured Hash expression
through the actual VM. This is controlled Wine behavior, not native Windows or
complete campaign parity.

The same native matrix qualifies byte-value migration needs. SubString on é
with range 0..1 returns byte C3; 1..2 returns A9 (each uniquely identified by its
one-byte hash). Valid empty ranges produce an empty string, while null/empty
input and an out-of-range start produce null. A positive reversed range 3..1 on
abcdef produces def. Indices narrow to 16 bits; negative effective offsets can
touch outside the allocation, so inventing unsigned clamps is unsupported.
Four large SubString cases did not record the actual producer's raw length;
their requested repeat counts must not be treated as witnessed input lengths.

StringCase's CRT entries test mutable locale state at `0x11b4454`; the zero
branch uses encoded byte ranges, while the nonzero branch uses locale wrappers.
Observed UTF-8 inputs follow the C-like ASCII path in this run. That does not
qualify all locale states. Case can cut 1022 a bytes + é after byte C3. The earlier
`Value::Str(String)` could not preserve these outputs. The byte payload migration
below removes that representation limit. The following selected policies now
implement the bounded Case/SubString paths.
No Rust Unicode or lossy UTF-8 substitution is supported.

### Hash validation (2026-11-13)

From `d73f9224`, the isolated worktree target passes the complete script/trig/play
suites with `pcx-play/test-fixtures`: 3,504 passed, zero failed, 164 explicit
ignores. All ten new ordinary Hash regressions run by name. The separately
selected original-input test passes all 38 expressions through VM/ObjectHost.
Strict native/WASM all-target/all-feature Clippy and both library builds pass
with warnings denied. Checked Rust source hashes remain unchanged across those
gates. Application startup/session integration remains a separate main-checkout
gate; these checks do not establish full campaign or rendered byte-text parity.

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

S2I/S2R, non-C locale mapping and unsafe effective substring pointer domains remain
unimplemented. Additional map-string wrapper families remain refused until their
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


## Byte-valued VM strings and checked consumers

`Value::Str` now owns `ScriptString`, whose raw constructor and byte accessors
preserve every byte, including embedded NUL and invalid UTF-8. Source literals
and identifiers remain UTF-8 and convert once at the evaluation boundary.
Globals, arrays, parameters, returns, concatenation, equality, callbacks and
parked continuations retain bytes. Null and an allocated empty string stay
distinct. Textual diagnostic formatting does not become a game-data conversion.

The native C-string boundary and one-level WTS resolution operate on bytes.
`StringLength` counts the resolved C-prefix bytes, and StringHash still consumes
raw bytes without WTS. Source WTS decoding remains checked, with absent source,
absent entry and malformed source distinguished. No global native-argument
conversion, Unicode case conversion or lossy decoding is introduced.

UI text state retains the native byte result. Dictionary lookup compares against
its admitted UTF-8 keys, preserving an unmatched raw key exactly. A found label
is capped in bytes by the selected native output bound, even when that splits a
UTF-8 sequence. Rich-text, paths, model selectors and source identifiers decode
only at their actual textual consumer and report contextual encoding errors.
Existing handle, local-recipient, admission and byte-budget checks retain their
order. Message history retains raw bytes until its checked display projection.
Rendered non-UTF-8 glyph behavior remains unqualified.

The recreation-owned trace writer retains version 1 for UTF-8 values and uses
version 2 when a value needs raw bytes; trace metadata remains checked UTF-8.
The campaign cache similarly retains existing versions for representable data
and uses version 3 for raw string payloads. Earlier versions continue to reject
invalid text. Existing cache key and embedded-NUL refusals remain unchanged.
These are private recreation formats, not original save or replay formats.
The checkpoint codec and its wire admission are coordinated separately.

### Remaining native policy qualification

Selected-image static evidence traces the CRT locale-changed flag at `0x11b4454`
from zero-filled image storage through normal startup. The callback at
`0xa94fc7` installs the initial C locale object at `0x10ea118`; its lower/upper
byte tables are reached through fields `+0x94`/`+0x98`. Their byte mappings and
case-handler output cap must be read from that image. Two sticky flag writers
were found, but indirect reachability and non-C operating-system locale mapping
are not fully qualified. No host locale or Unicode substitute is admitted.

For SubString, source assembly narrows start, end and C-prefix length to signed
16-bit operands. An out-of-range start returns null before the pointer path.
After the signed end clamp, a safe reversed positive range copies the suffix
from start, because its temporary NUL is before the copied source. Effective
negative pointer offsets remain refused; no unsigned clamp is invented. The
native byte representation supports these results; the following called-native
implementation supplies the selected policies, factory and host integration.

### Byte migration validation (2026-11-13)

The isolated worktree at control baseline `7c443b78` passes all three affected
package suites with `pcx-play/test-fixtures`: 3,565 passed, zero failed, 170
explicit ignores. Ten named new regressions execute: byte globals/arrays/concat/
registered callbacks/park-resume, raw equality/native prefixes, one-level byte
WTS demand, retained local messages and atomic budgets, dictionary byte caps,
raw unit/order names, cache/trace version admission and atomic path refusal.

The separately enabled selected-image gates execute all 38 original Hash
expressions, all 194 formatter expressions and all 216 public timer-query bit
comparisons. Strict native/WASM all-target/all-feature Clippy and explicit native
and WASM library builds pass. Application all-target/all-feature compilation also
passes. Frozen changed Rust hashes remain unchanged through these gates; local
receipts are under `/tmp/sol01-byte-validation/`. Tests use verified private
Xvfb and null audio. No original client, GPU window or playback is launched.

The first full run found a valid-string diagnostic spelling regression. Restoring
that existing debug spelling preserves its assertion; invalid bytes still have
an explicit byte diagnostic. The complete successful rerun is retained alongside
the failed run. The application message-history refusal/recovery test compiles;
its execution and other application UI/session tests remain downstream gates.
Case/SubString selected policies, non-C locales and rendered byte glyph behavior
remain open; this validation does not establish full native string parity.


## Called StringCase and SubString policies

The selected campaign client reader installs `ScriptStringPolicy` through normal
campaign inputs and ObjectHost assembly. Exact builtin signatures and two readonly
Host hooks reach that policy; both localization wrappers, selected neutral worlds,
recording, startup, lent AI and preload forward the same path. No numeric, actor,
AI routing, checkpoint-control or wire behavior changes in this slice.

The reader admits fifteen bounded source windows and checks their complete-image
fingerprint, individual identities, call joins and encodings. Case capacity and
buffer/cookie relationship come from its wrapper; the decrement/minimum/NUL copy
branch is verified. Signed case ranges and arithmetic come from the CRT fast
branches. Every byte mapping is compared with the source initial locale tables;
no host case conversion is called. The initial locale flag's PE storage and the
entry/startup/CRT callback walk through the locale installation are checked.
The profile is explicitly the initial C context. Later effective locale changes
or NLS-provider behavior are not inferred from that initial state.

Case performs one native WTS lookup before conversion and checks null/empty at
its observed points. Copy and conversion may split UTF-8. Interning stops at the
first output NUL; an allocated empty result stays distinct from null. SubString
performs no WTS lookup, narrows source length/start/end to the admitted x86 signed
word operands, takes its early-null branch before pointer access and then clamps
end with the signed comparison. Safe reversed ranges copy the tail. An unsafe
effective start/end is an error naming the narrowed offsets and length, without
invented clamping or memory access. Input storage remains unchanged.

The retained private Wine matrix supplies thirty-nine bounded Case/SubString
observations. Replay goes through the actual builtins and selected ObjectHost
policy, checking raw return null/empty, native hash, native length and expected
comparison results. Split UTF-8 bytes are also asserted exactly. Four large
SubString producers remain excluded because their actual raw lengths were not
witnessed. A returned trigger token's fresh native length demand is tested as
such, not used as its raw byte length. Source map identity, image/raw trace digest,
frame nonce/order/count and result rows are cross-checked with the local receipt.
No source map, trace, image or generated original content is committed.

Nonstock tests vary case operands, capacity and table data, including output NUL
and allocated empty results. VM tests exercise missing versus malformed WTS,
missing entries, raw SubString tokens, signatures, absent policy, callbacks,
parking, byte concatenation and recovery after real path-consumer refusal.
This is a bounded called-native slice, not general locale, rendered glyph or
campaign-completion parity.

### Case/SubString validation (2026-11-13)

From byte baseline `d36f2312`, the isolated worktree passes all three affected
package suites: 3,576 passed, zero failed, 171 explicit ignores. All eleven new
ordinary regressions execute, including the core signature/default-policy test
and the real byte-result callback/park/path-refusal test. The separately enabled
Case/SubString gate runs eleven focused tests and compares all thirty-nine
bounded original observations through VM/ObjectHost. The four unwitnessed large
producer lengths are excluded explicitly.

The preserved Hash38, formatter194 and timer216 selected-image gates each run
and pass. Strict native/WASM all-target/all-feature Clippy, both explicit-target
library builds and application all-target/all-feature compilation pass. Frozen
changed Rust hashes remain unchanged across every gate. Local commands, results
and source witnesses are under `/tmp/sol01-case-validation/`; no original assets
or generated original copies are committed. The checks use verified private
Xvfb and null audio, without launching an original client or GPU window.

This slice has no production application UI algorithm changes; its only app
fixture addition supplies absent policy explicitly. Downstream combined app
execution and checkpoint source-ledger admission remain separate integration
gates. Non-C effective locales, unsafe pointer domains, unqualified producer
lengths, S2I/S2R and rendered byte glyph behavior remain open.
