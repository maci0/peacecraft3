# Ordered WCT custom-text compilation

`pcx_play::custom_scripts::CustomScripts::compile(identity, inputs, budget)`
compiles the non-empty bodies in a parsed WCT/WTG pair as one JASS environment.
The `custom_scripts` module is exported by `pcx-play`; tests use that public API.
The adapter adds no generated GUI code and does not run a trigger automatically.

## Inputs and source relationships

`CustomScriptInputs` requires caller-supplied `common.j` and `Blizzard.j` bytes,
explicit WCT/WTG labels, parsed containers, optional authored declaration context,
and optional local WTS bytes. Labels must be distinct and nonempty. Prelude
labels cannot use the VM's reserved main-source label, `the map script`, because
that would make runtime source attribution ambiguous. This is local diagnostic
policy. The asset identity must specify edition, version and source. The caller owns archive
selection and release claims. No edition union or native archive priority is
inferred.

The checked profile is the measured zero-header WCT container and the existing
version-4 WTG parser. The WCT/WTG slot count, custom-text flags and absence of ECAs
on custom triggers must agree. Empty GUI slots retain their positions, so later
custom blocks keep their original trigger indices. Every non-empty body must
contain one final NUL and no embedded NUL. The terminal NUL remains in the stored
block fingerprint; it is not passed to the JASS parser. Trigger names stay raw
bytes, including names that are not UTF-8.

Optional authored context precedes the custom bodies. Each source is parsed
individually; then bodies are joined in WCT slot order with newline separators
and resolved together against the supplied preludes. Functions can call bodies
in later slots, and one VM owns the shared globals. Stored text is preserved
apart from the final framing NUL; source ranges map compilation/runtime errors
back to the actual block and local line. `resolve_with_diagnostics` retains the
declaration source for prelude type/library graph and signature-conflict errors;
the adapter renders them against that prelude rather than a WCT byte range.
The existing resolver's acceptance and `Error` behavior are unchanged. Runtime
prelude errors also retain their own source.
WTS lookup uses the existing `LocalizedHost`, preserving authored control codes
and refusing malformed or duplicate table entries.

The join order is explicit import policy from the stored relationship, not native
compiler load-order or initializer-order proof. The adapter does not invent GUI
globals: callers must supply actual context for missing symbols. A full generated
`war3map.j` that already contains the custom bodies cannot silently overwrite them.
Duplicate globals, function/native names and top-level types in the combined input
are refused. This collision check is local policy, not native rejection parity or
vJass preprocessing.

## Input limits

Caller limits cover WCT bytes, WTG bytes, slot count, each stored body, total
prelude bytes, optional context, optional WTS and aggregate original input bytes.
Checked arithmetic and all byte ceilings run before parsing source or allocating
the combined string. The string's size must fit platform allocation layout and
32-bit JASS spans. Its allocation and block-metadata reservation are fallible.
The already parsed WCT/WTG allocations belong to their caller. These limits do
not claim a bound on AST, resolver or VM memory.

Failures retain typed identity, budget, overflow, platform, allocation, framing,
association, UTF-8, source-stage and WTS context. Missing functions or game natives
do not become synthetic definitions or successful no-ops.

## Verification and remaining work

Active synthetic tests exercise cross-block calls and shared state, exact caller
context, later-block compile/runtime diagnostics, local WTS resolution, missing
and malformed WTS, duplicate declarations, exact/over-budget boundaries before
parse, empty GUI slots, invalid framing, unsupported header words and raw names.
An independently compiled copy of the retained draft reproduces the original
cross-block failure: its first body refuses an authored call to a later body.
The corrected public API runs the two bodies in one VM with shared state.

An explicit ignored original gate reads a caller-selected map and preludes from a
local archive set. Its context consists of authored spans from the compiled map,
excluding functions independently supplied by WCT. No GUI code is synthesized.
For supplied `Maps\Campaign\Human06.w3m` in `wc3/game/app`, the gate compiled 97
slots, three non-empty custom blocks and six custom functions into an environment
of 1,514 functions. This is source compilation, not campaign execution; no original
bytes are written or committed. The archive release remains unverified.

WTG desugaring, native generated-code naming, original source selection, native
side-effect ordering, game Host coverage, campaign execution and the Athena gate
remain open. Compilation and synthetic VM tests do not close those requirements.
