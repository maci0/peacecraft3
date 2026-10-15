# Source-qualified Options fallback data

The explicitly supplied Legacy executable has SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
and qualified VERSIONINFO 1.29.2.9231. This identifies the executable, not
adjacent archives. No original client, registry or display driver was executed.

The complete string getter at `0x42b590..0x42b5eb` selects a descriptor's
fallback pointer when its internal lookup returns null. The separate word
getter at `0x42b5f0..0x42b65d` selects the same field and delegates conversion.
Their shared positive index upper bound and stride come from instructions.
Negative indexes are not checked by that native signed upper-bound branch;
the public importer accepts unsigned indexes and checks their bound.

This is actual fallback-field use, rather than merely nearby numeric strings.
It does not establish which saved-state failures yield a null lookup, effective
first-run state, conversion of malformed values, or the final capability-adjusted
display choice. See [display enumeration](original-display-modes.md).

## Implemented import boundary

`pcx_play::options_defaults::Defaults::read` takes complete local image bytes,
an independently reviewed `Binding`, and explicit `RuleInputBudget`. It reuses
the existing PE32 reader and SHA-checked `SourceWindow` inputs. It does not
discover an executable, scan unrelated code or contain target addresses, a
target fingerprint allowlist, a preset list or copied preference values.

The supported complete string-getter shape validates its signed branch,
lookup-null branch, fallback load and merged output path. Index bound, stride
and field displacement are read from its operands. Selected descriptor indexes,
spans, common table base, field alignment, direct name/fallback pointers and
NUL framing must agree. Empty selections, duplicate indexes/names, bad identity,
uninitialized virtual padding, null fallback pointers and mismatched windows
refuse contextually. A null pointer is distinct from a present empty string.

Selection source bytes are bounded before allocation; metadata count is also
bounded by the supported getter's positive signed-byte limit. The result borrows
descriptor bytes, names and raw fallback bytes. No numeric conversion, group
meaning, preference priority or default-to-widget mapping is invented. Opaque
descriptor words remain available for later qualified interpretation. Entry
order is caller selection order; missing keys remain absent. The reported
ordinal upper bound is not the number of admitted records.

The first root gate imported 32 previously inspected records. A subsequent
bounded census read exactly the instruction-derived descriptor span and only
its directly referenced strings. The root importer then admitted all 80 rows
using independently reviewed fingerprints, preserving 12 empty fallback strings.
There were no null pointers or duplicate names in this selected image. Those
are observed source facts, not implementation limits or guarantees for another
image. No generated profile, original payload or original default table is
distributed with the repository.

## Runtime discovery

`Defaults::discover` now removes the need for a manually assembled binding when
the supplied image has the supported complete getter/descriptor profile. It
scans only initialized readable executable sections, checks the entire getter
shape, and derives each candidate's count, stride and fallback displacement.
The supported descriptor profile places the fallback pointer in its last word;
this is an explicit format interpretation, not a copied table address or value.
All rows and their directly referenced strings must validate. Exactly one valid
candidate is required; even two getters addressing the same table are ambiguous.
An invalid decoy does not establish address priority. Limit exhaustion is fatal,
including after an earlier valid candidate.

`DiscoveryBudget` separately bounds complete-image bytes, selected source bytes,
per-string bytes including NUL, complete-shape candidate count and logical scan
work. Logical work charges full scanned code ranges, candidate table sizes and
inspected string bytes, including repeated pointers. It is not a whole-CPU or
whole-heap budget; the image/section/count bounds also constrain hashing and
metadata. The existing PE mapping checks still exclude headers, overlapping
sections, uninitialized flags, virtual padding and cross-section windows. The
existing manually reviewed partial-selection API remains available.

Nine discovery cases test moved code/data/raw offsets, changed strides/counts,
collisions, invalid decoys in both orders, late budget exhaustion, non-executable
and BSS ranges, truncation, malformed and empty/raw strings and exact limit edges.
Two additional mapping regressions preserve the shared PE reader's boundaries.
The full native package suite and strict native/WASM checks pass; an actual
32-bit WASM leaf executes six discovery groups with mask 63 and no imports.
The local original-image gate discovers all 80 records and preserves 12 empty
fallbacks without supplying addresses or a profile. Receipt:
`.scratch/options-discovery-root-20261016/`.

## Verification and remaining work

Eight synthetic tests cover relocation, changed bounds/values, partial selection,
control-flow mutations, source-byte budget boundaries, bad identity, BSS,
duplicate/stale references, exact string framing and raw non-numeric values.
The affected full native package suite, strict native/WASM Clippy and WASM build
pass. An actual 32-bit WASM source-leaf executes four groups with mask 15 and
no imports. The explicitly selected original-image gates admit the partial
32-record selection and subsequent complete 80-record selection.

Evidence is retained locally in `.scratch/options-defaults-root-20261016/`;
native semantic receipts are in
`.scratch/legacy-options-folded-default-field-evidence/` and the independently
reviewed plan in `.scratch/legacy-options-default-profile-plan-20261016/`.
The bounded complete-table census is retained separately in
`.scratch/legacy-options-complete-descriptor-census-20261016/`.
Earlier failed fixture assertions and compile attempts remain failures in those
logs; they are not rewritten as successful checks.

This importer is a data prerequisite. The menu does not yet consume discovered
defaults automatically. Saved/default precedence, source units and actual setting
owners must be connected before claiming faithful Options defaults. The browser
cannot enumerate exclusive OS modes; native exclusive mode requests need live
capability checks and an observed application result. Unsupported settings remain
disabled rather than being persisted without an effect.
