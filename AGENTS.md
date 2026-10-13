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
