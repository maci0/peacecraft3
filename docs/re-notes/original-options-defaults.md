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

## Native audio consumer

The native menu now prepares the four audio fallbacks from the selected client
and selected Sound Options/template FDFs before publishing its first music
player. Complete project preferences win; missing or fully read invalid project
preferences can use the checked source fallbacks. An unreadable preference file
is a separate state and cannot be treated as absence. Source initialization
changes the active audio and matching applied/draft controls together without
saving preferences. Cancel returns to that baseline. Source integers remain in
authored units, including valid values off the interactive step; no widget
initial, copied percentage divisor or snapping replaces them. Saved normalized
gains remain authoritative when the source range changes, and an unchanged
control does not round-trip a gain through slider units on Apply.

Preparation is source-keyed, so revisiting a tab does not reset edits. Failed
replacement retains old media, gains and drafts; click and SNDX creation require
the current selected-source decision, rather than a global readiness flag.
Unsupported client-header architecture and an absent supported getter are
capability states, distinct from malformed framing or ambiguous table discovery.
MPQ-only sources retain explicitly unqualified neutral multipliers. No neighboring
executable or compatibility DLL supplies defaults. Screen members and includes
now check packed/decoded sizes against their remaining aggregate allowance
before extraction; this is not a bound on archive-open tables or decoder RSS.

Native menu music comes from the selected skin variant. A source list consisting
only of repetitions of the same member uses the existing loop; distinct-entry
playlists refuse instead of selecting an arbitrary entry. Click volume uses the
selected script's admitted sound scale. An unavailable initial click is reported
without silencing admitted music; a replacement may not discard a working click.
The qualified Legacy click currently refuses compression mask `0x41`. The existing
Huffman table recognizer also refuses this selected client, so no table or decoded
click is invented or borrowed from another installation.

Synthetic tests cover conversion, saved precedence, source rebasing, dirty-state
rollback, unsupported-header distinction and gain-bit preservation. Explicit
local original gates check the selected complete image identity, source/FDF
conversion and the music player's preplayback volume/pause components. These
checks do not establish audible playback, original gain response or native mixer
parity. Evidence and earlier failed gates remain in
`.scratch/options-audio-root-20261016/`.

The final native app suite passes 240 tests with 11 explicit local-input ignores;
the two selected-original default gates pass separately. Strict native/WASM
checks and complete app builds pass. Seven discovery/header groups also execute
in actual 32-bit WASM (mask 127, no imports). The complete desktop app starts with
an empty private configuration and admits the source fallback once during the
bounded smoke run, using a private null audio route. An earlier Xvfb attempt
cannot present through this Vulkan driver's surface and remains a failed gate.
Pending player settings and live sinks both receive volume/pause updates; a
schedule regression checks the distinct launcher-input and menu-action phases.
This startup check is not an audible or visual fidelity assessment.

Browser client transport and source-generation audio admission remain open;
this native consumer does not make the MPQ-only browser demo source-qualified.
The browser cannot enumerate exclusive OS modes; native exclusive mode requests
still need live capability checks and an observed application result. Remaining
gameplay, graphics and device settings need their actual owners before activation.
Unsupported settings stay disabled rather than being persisted without an effect.

## Resolution and setting fidelity boundary

The checked original device enumeration feeds a provider that also adds
source-table and derived candidates: see
[the display-mode contract](original-display-modes.md). The complete path is
neither device-only nor a universal fixed preset list. It retains depth and
refresh as well as dimensions. A stored descriptor fallback is
not proof of the final first-run choice after device capabilities and saved
preferences have been considered. The current portable Resolution control still
applies a window/canvas render size; it does not claim a confirmed OS display
mode or silently infer missing depth/refresh intent from existing preferences.

All admitted descriptor fallback strings remain available through the checked
importer. This is data availability, not implementation of every option. Source
Gameplay/Video/Sound controls are retained, but settings without a consuming
gameplay, renderer or device owner stay explicitly unsupported. No measured
option value, familiar display preset or invented dropdown content replaces
missing behavior. Original Options panel animations now have a separate admitted
owner; this presentation change does not activate unsupported settings.

The subsequent [Options initialization inspection](original-options-initialization.md)
now connects every retained control to its preference reads, including the
seven quality-popup indexes and the compound resolution query. It resolves
source-key correspondence without pretending unimplemented settings already
affect gameplay or rendering. Source fallbacks, saved state and the final
device-adjusted setting remain separate.
