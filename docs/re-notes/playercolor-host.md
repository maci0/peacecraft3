# Source-derived player-color identities

`pcx_play::playercolor_host::ColorHost` is an opt-in concrete, session-owned Host.
It implements `ConvertPlayerColor` as symbolic typed identity, not RGB conversion
or native pointer emulation. The supplied compiled Program must declare the exact
integer-to-playercolor signature and handle ancestry. Constant conversion globals
in that native's declaring source supply the admitted enumerands. No color count,
numeric enumerand, palette, RGB value or engine handle ID is embedded in the code.

Supported initializers use the declared conversion with an integer literal,
parentheses or unary numeric operators under existing VM integer semantics.
Unsupported shapes, incompatible signatures, a script override, missing domain,
bad arguments and out-of-domain inputs report contextual errors. This supported
domain is a local import policy, not a claim that native conversion rejects all
other integers. Map-source globals cannot enlarge the common source's domain.

One bounded arena interns equal colors and assigns distinct typed identities to
different colors and interleaved object families. Identity allocation is a
recreation choice. Capacity failure preserves existing identities, which remain
usable at capacity. The identity-only reservation API provides no object behavior.
Future handle-producing operations in this Host must use the same arena; an
arbitrary delegate that independently mints handles cannot safely be added.
Tokens are scoped to one Host/VM session and are not cross-session identifiers.

`MapScripts::shared_program` shares the exact existing Arc with the Host instead
of compiling or cloning a second Program. The existing selected-map scalar
neutral adapters can wrap this Host without allocating competing handles.
`MapScripts::owned_vm` still owns localization and diagnostics. The default
launcher Host is unchanged; automatic installation remains separate work.

## Verification

Nine synthetic tests cover domain derivation, interning, family separation,
capacity, source context, invalid declarations/initializers, map-domain injection,
actual compiled global initialization, sticky failure and other-native refusal.
The affected crate passes 163 native tests with five explicit original-input
ignores, strict all-target/all-feature native and WASM Clippy, and WASM library
code generation. The existing ownership test also checks shared Program identity.

The separate local original gate uses retained Human01, base common and Blizzard
inputs identified in [the neutral adapter note](selected-map-neutral-host.md).
The unchanged common SHA-256 is
`3c21e998bf80d1e04718bd3f2ee7ef2bf1b83359e5b303a56cf59cd61a2afdcd`.
The same admitted map and compiled Program handle both independently approved
neutral getters and all 24 source-declared color constants, then refuse
`ConvertRace` at common line 204. The count is an observed diagnostic, not a
production constant. Exact commands, actual output, artifact hashes and source
receipt are local in `.scratch/playercolor-host-root-integration/`.

The gate uses an explicitly identified compatibility object catalog, with
unresolved records. It does not establish complete target-catalog admission,
campaign execution, rendered color interpretation or native exceptional behavior.
Original sources and executable-rule profiles remain local and unpublished.
