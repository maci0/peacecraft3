# Parley 0.11.1 patch

Based on the published `parley` 0.11.1 crate (Apache-2.0 OR MIT), SHA256
`22d2ff88bd3f7d68d1d9b09c7e6209f9a8e8c05088295140a2bcf2e9b17038c5`.
License notices are retained; the package lockfile and Cargo cache stamps are
omitted because the workspace lockfile owns resolution.

The private bidi mask maps the 23 named `BidiClass` values to disjoint local bits
instead of calling `to_icu4c_value`, deprecated by ICU 2.3. All consumers only
test membership in masks assembled by the same helper; no ICU ordinal is part
of their contract. Unknown future classes produce zero. A regression checks
single-bit uniqueness and bidi, isolate, explicit, RTL, X9 and reset membership.

Bevy Text 0.20 requires Parley 0.11; newer Parley minor versions are outside that
dependency range. No dependency downgrade or warning suppression is used.

The published crate's builder tests reference the omitted `parley_dev` package.
The mask regression is therefore executed through a private standalone harness
that directly includes this exact `src/bidi.rs`, with its hash pinned in the
receipt. The same harness checks mixed LTR/RTL and directional-isolate levels.
Strict native and `wasm32-unknown-unknown` library checks use the workspace's
resolved dependency graph.
