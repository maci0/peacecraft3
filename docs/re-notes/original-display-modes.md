# Original display-mode enumeration boundary

Static target: explicitly supplied Legacy Warcraft III.exe, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
qualified executable version 1.29.2.9231. This does not authenticate the release
of adjacent archives. No game or hardware enumeration was executed.

## Observed native data path

One file-backed code region, `0x7d5000..0x7d5800`, SHA-256
`cd6e6319d62ebefbed7a0a42759b518b0f1c325c5c959f470ccff99e49683eb0`,
contains an aligned complete mode-enumeration routine at `0x7d55d0..0x7d577f`.
Selected PE imports establish USER32.dll EnumDisplayDevicesA at IAT `0xe66788`
and EnumDisplaySettingsA at `0xe66784`.

The outer loop visits device indexes starting at zero until API failure. It
admits devices whose StateFlags have both attached-to-desktop and primary-device
bits. The inner loop similarly visits mode indexes starting at zero until
failure. Returned DEVMODEA fields are filtered by two color-depth alternatives
and an unsigned width minimum. The observed operands are 16/32 bits and 640
pixels, respectively: these are source diagnostics, **not production constants
or a resolution preset list**. This local body has no explicit height, refresh
or aspect filter; unread storage helpers and later consumers remain outside
that bounded negative.

Each offered record preserves device index, mode index, width/height, color
depth and refresh frequency. Width/height occur twice; their distinct meanings
are unresolved. Native record size is 40 bytes. No user-facing label is built
here, and final container order/deduplication is not proved.

The API index contract and output field names agree with
[Microsoft EnumDisplaySettingsA](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaysettingsa)
and [DEVMODEA](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-devmodea).
Installed public Windows declarations independently qualify the x86 structure
layout. Documentation does not establish this client's UI or first-run policy.

## What this changes about the implementation plan

The current application reduces device modes to `(width,height)` pairs. That
loses fields the original routine retains. Future mode choices must preserve
stable device/mode identity, depth and refresh, and distinguish a window render
size from an exclusive display mode. Browser render sizes are a separate host
capability; a browser cannot claim this OS-exclusive enumeration contract.

Do not paste observed filter/default values into code. A checked profile may
read them from a caller-supplied, qualified executable, or another independently
qualified source must supply equivalent data. A fixed familiar list of classic
resolutions would not reproduce this device-derived path.

At this enumeration-only boundary, the connection to ResolutionMenu, formatting
and selection was unresolved. The subsequent Options inspection below closes
the local UI path; the conditional bridge and source-backed augmentation are
established in the later provider inspection below. Effective first-run inputs
remain open. A null-input reset alone establishes no default choice.
The wider Options implementation remains open; no menu setting was enabled by
this evidence-only slice.

## Verification and retained evidence

Local receipt: `.scratch/legacy-display-mode-contract-20261016/HANDOFF.md`,
SHA-256 `38b5a13cda7d0348d6528449e3ff82e6d966d319aac4bb0b849c923e30dd0d12`.
Non-call control flow for both contained routines has no gaps or out-of-region
edges. Exact target identity is unchanged before/after. Public-layout checks
and the selected-import check pass; no source/dependency/build changed.

The initial inspection command ended with a Python module-shadowing error
after recording the region and hashes. That failed command is preserved as a
failure. Later checks consume retained instruction metadata; no body reread was
used to claim a successful original inspection. No raw code, game asset,
credentials or original preference contents are included in the repository.

## ResolutionMenu population and preference matching

The subsequent qualified Options routine at `0x531260` reads reswidth,
resheight, colordepth, adapter and fixedaspect through the known preference
getter. It calls `0x536500` to populate ResolutionMenu, then `0x539360` to
select a row. The getter can return saved state or a lookup-null fallback;
observing these calls does not establish which values apply on a first run.

Population consumes a count and 44-byte record array from `0x7d0c40`, whose
producer was unread at this first inspection boundary. The later inspection
below connects it conditionally to the 40-byte OS enumeration records.
It filters against an owner adapter field, a current-kind result and
an optional fixed-aspect-related checkbox property. The optional checkbox is
absent from the selected retained FDF; native code supplies false when absent,
rather than creating a replacement widget. The owner adapter's initialization,
provider record construction and current-kind meaning remain separate inputs.

The local loop offers eligible modes in provider order without sorting or
deduplicating them. It records each row's native mode index in a parallel map.
The source label format receives decimal width and height, adding color depth
when that record field is nonzero. Refresh remains in the record but is not
included in this label. Equal displayed labels can therefore represent distinct
mode identities; collapsing choices by width/height loses source distinctions.
The insert helper's effects on final popup order remain uninspected.

Selection constructs a query using those preference inputs, the current kind
and a zero refresh field. It scores every provider record, replacing the winner
only for a strictly smaller unsigned score, then finds that native index in
the visible-row map. Ties keep the first provider record. An unmapped winner
leaves the initial row-zero request and does not update stored native selection
in this routine. An empty list is not evidence that row zero is a valid mode.

The complete scorer at `0x7cf690` prioritizes adapter, kind and flag mismatches,
then quantized width/height distance and depth/conditional refresh differences.
Its adapter resolver remains unread. Because this UI query sets refresh to zero,
it does not prefer the descriptor's refreshrate fallback. Scoring operands,
wildcards, count-dependent font style and filter operands remain source metadata;
they must be derived from the selected input if used by a production profile.
No diagnostic values or familiar resolution array have been copied into code.

Four bounded bodies total 16,384 inspected bytes and 408 reachable instructions,
with independent Capstone/objdump comparisons. Seventeen synthetic checks cover
filtering, ties, refresh-zero behavior, scoring, duplicate labels, row mapping
and empty/unmapped cases. Root replay passes separately. The checked model
refuses excessive arithmetic rather than promising native overflow behavior.
Evidence is retained in `.scratch/options-resolution-selection-20261016/`;
root review is in `.scratch/options-resolution-selection-root-20261016/`.

This resolves local population, formatting and matching, not a universal first-run
resolution or exact physical mode list. The subsequent provider inspection
closes several of that inspection's open targets. Actual device switching,
acknowledgement and rollback remain unimplemented. Browser/window render sizes
retain their existing separate meaning; no unsupported exclusive choice has
been enabled by this inspection.

## Conditional provider bridge and source additions

The getter at `0x7d0c40` returns a global UI collection. Its producer at
`0x7cf320` converts 40-byte source records into 44-byte UI records, preserving
the input-array index in the additional word. The caller `0x7d0af0` forwards
the exact collection pointer returned by `0x7d01c0` into that producer. On a
cache miss with source selector zero, `0x7d01c0` passes that same collection to
the already checked Windows enumerator. Selector one uses another, unread
enumerator. The actual startup selector is not established.

The complete provider is therefore more than a list of device modes. Before
returning the collection, it augments the enumerated records using two
source-resident dimension-pair tables, a source floating-point aspect predicate,
a dimension transformation, and per-index dimension queries. Both tables have
nine pairs in this selected image. Their contents and the predicate operands
were read in memory and validated, without publishing a resolution list or
copying those values into production. The transformation fits a rectangle to
source-derived integer aspect units; it does not use a screenshot fit.

The conversion groups records by a formatted key containing dimensions, depth,
adapter, kind and flag. Refresh and original mode index are absent from that
key. A later record replaces an existing group's contents only if its refresh
is strictly closer to a source-derived operand. Equal distances retain the
earlier record. Under the explicitly assumed ordinary successful clear,
allocation and exact-key lookup behavior, groups keep their first occurrence
order. The storage helpers' exceptional paths remain unread. This grouping is
different from collapsing all equal width/height pairs.

The current-kind wrapper is also closed: a null active object produces zero;
otherwise its delegate returns the object's field at `+0xac`. The field's
writer and enum meanings remain unproved. Do not label source kind tags as
exclusive, borderless or windowed merely from their numeric values. The
per-index count/dimension provider and the Options owner's adapter origin also
remain separate inputs. A checked production profile must derive the relevant
tables, operands and relationships from the selected client and combine them
with actual host capabilities. Neither the raw device list alone nor a familiar
fixed preset list reproduces this source path.

Independent root replays pass 12 converter checks, six conditional-bridge
checks and 32 augmentation checks. The last inspection independently compares
44 reachable instructions; its four direct data ranges total 152 bytes and
are retained only as identities/counts/validation metadata. Evidence is in
`.scratch/options-display-provider-20261016/`,
`.scratch/options-display-provider-caller-20261016/` and
`.scratch/options-display-augmentation-20261016/`, with separate root reviews.
These checks do not enumerate a live original display list, select a first-run
mode or apply a display change.

## Startup-selector boundary

A subsequent single direct-call instruction scan for `0x7d0af0` finds candidates
at `0x42e6d3`, `0x531e68` and `0x7d1156`. Independent objdump checks agree on
those local CALL instructions. Their retained contexts pass a caller argument
or a result of an unread call; none establishes a literal startup selector.
Nearest-prologue candidates do not prove complete function boundaries or
startup execution. The multiple-candidate stop was observed, with no caller
body or delegate expansion. The exact Legacy image hash is unchanged.

Receipt: `.scratch/options-mode-selector-caller-20261016/HANDOFF.md` and
`xref.json`. This bounded result does not select an original default or activate
a production profile. Source fallbacks, saved state, host capabilities and the
final effective first-run mode remain distinct inputs.
