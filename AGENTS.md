# PeaceCraft III agent instructions

## Derive data from the original assets

Never hard-code a value that can be read or derived from the supplied asset files,
whether the derivation can happen at compile time or at runtime. Parse the source
assets at runtime, or generate the necessary data from those assets during the
build. Copying a measured asset value into a constant is still hard-coding it.

This includes UI frame definitions and anchor chains, dimensions, insets, colors,
fonts, text, texture paths and sampler flags, edition mappings, animation tracks,
sequence durations, material properties, sound settings and game rules.

Keep the provenance and edition/version of the input explicit. Preserve authored
relationships and behavior when scaling or translating data into Bevy components.
Do not replace authored data with screenshot fits or assumed defaults. If an asset
is missing, malformed or unsupported, report that state instead of inventing its
contents. Test parsers and derivations with synthetic fixtures; keep tests against
locally supplied original assets explicit about their requirements.

Existing hard-coded asset data is debt, not precedent. When touching it, move the
relevant values to the asset-derived path. Format identifiers and independently
established format invariants may be constants; they must not conceal values
that belong to an asset record.

Original game assets and generated copies of their contents must not be committed
or uploaded. Runtime imports, including browser ISO imports, stay local.

## Collaboration and fidelity

PeaceCraft III is a recreation, not a full decompilation or recompilation.
The target is 100% fidelity to the source material's appearance, sound, feel and
observable behavior. Internal architecture, algorithms, data structures and
engine conventions may differ when they preserve that experience. Do not require
proof of identical native internals before implementing a faithful recreation;
label implementation choices and remaining fidelity questions honestly. This
does not relax the asset-derived data rule or authorize invented game content.

Read `PLAN.md`, `TRACKER.md` and relevant `docs/re-notes/` before changing original
game behavior. Distinguish shipped-byte evidence, native-code evidence, runtime
observations and unverified inference. Parser tests do not prove rendered or
native behavior.

Coordinate file ownership with other agents before editing shared files. Keep UI
layout relative so it scales across resolutions. Follow Bevy 0.20 APIs and preserve
the original game's behavior. Performance regression gates must use deterministic
work measurements rather than elapsed-time or FPS thresholds affected by unrelated
machine load; report live timing and FPS separately.

Do not add attribution to generated tools or agents in source, documentation,
commit messages or pull requests.

## Test isolation

Run all automated game, menu, browser and original-client tests fully headless
and muted. Use an isolated offscreen display and a null audio sink; never open
test windows on the user's desktop, steal focus, move their pointer or play
audio through their speakers. Verify display and audio isolation before launch.
Visible or audible testing requires an explicit user request for that run.

## CLI contracts

Follow [The CLI Spec](https://clispec.dev/) when creating or changing command-line
interfaces. Provide explicit structured output and errors, discoverable offline
schema introspection, accurate exit codes, and separate stdout data from stderr
diagnostics/progress. Non-interactive calls must complete or refuse safely.
Declare real retry effects and bound collection/stream output appropriately.
Preserve existing flags, output formats, exit semantics and binary/artifact
streams; use `--format` when `--output` already names a destination. Do not add
inert flags or new confirmation gates merely to satisfy a checklist.

Read the version policy before implementing schemas: v0.3 is currently a
candidate. Validate the declared version against checked, pinned published schema
bytes, with process-level tests for parsing, stream separation, failure, retries
and output limits. Do not claim conformance before those checks pass. Existing
CLI gaps are tracked debt, not permission to copy incompatible behavior.

## Rust and build policy

Use the current stable Rust release and Rust 2024 edition. Keep direct dependencies
at their latest stable releases and refresh compatible transitive dependencies in
the lockfiles; verify API changes with the affected targets and features.

Compiler warnings are build errors. Fix the underlying issue, keep test-only
helpers out of production builds, and avoid blanket lint suppression. Preserve
useful format and runtime APIs rather than deleting needed behavior to silence a
diagnostic. Use stable standard-library APIs when they improve ownership, error
handling or clarity; keep unsafe operations narrowly scoped with explicit safety
contracts. Validate asset sizes and offsets before allocation or indexing, and
propagate errors with asset/source context.

Evaluate newly stabilized Rust features against the actual code. Apply relevant
improvements without introducing unnecessary unsafe code or changing original
game semantics. Run targeted tests, strict compile checks and affected native/WASM
builds before recording a change as complete.
