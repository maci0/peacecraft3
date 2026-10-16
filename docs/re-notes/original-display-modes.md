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
capability; a browser cannot claim this OS-exclusive enumeration contract. A
browser popup list is one logical canvas bound from the screen observation,
not per-index native capacity.

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

### Complete local caller

A later, separately bounded read closes `0x42e630..0x42e6e0`: 176 bytes and
60 instructions, with independent decoders agreeing on all reached instructions.
Its call at `0x42e6d3` forwards the first incoming argument directly as the
provider selector and returns the target's EAX unchanged. This is a dynamic
input, not a locally selected startup backend. Another argument affects query
fields through a conditional move; its enum meaning is not established.

The wrapper reads `reswidth`, `resheight`, `colordepth`, `refreshrate` and
`adapter` through the already-qualified word getter. Same-image retained
descriptor metadata supplies those names. Width, height, depth and refresh
enter the query; the adapter output local is not read again in this body.
Getter return statuses are not tested. This refresh-bearing wrapper is distinct
from the previously checked ResolutionMenu selection query, which supplies zero
refresh. Literal additional dimension operands are query construction data,
not proof of a selected default resolution or a preset list.

Only the authorized 512-byte code window was read; no delegate, new data or
outside caller was followed. Static source/control flow still does not identify
startup invocation, saved-versus-fallback inputs or the host-adjusted result.
The three synthetic analyzer groups and retained-metadata root replay pass.
Evidence: `.scratch/options-mode-caller-body-20261016/` and
`.scratch/options-caller-root-review-20261016/`. No runtime mode policy or
hard-coded default was introduced.

## Project startup window

The investigation above stands. PeaceCraft still creates the native window
from the 880 by 860 seed in `launch_gui::window_plugin`. After settings have
loaded, and only once the host monitor is known, `startup_window` may request
a windowed client of nine tenths of that monitor's physical width and height,
centered on the current monitor.

That fraction is a PeaceCraft presentation policy. Asset-derived Warcraft III
defaults stay unselected: there is no preset list and no exclusive mode
change. An explicit saved resolution keeps the seed until the existing resolution
transaction restores a size the display can show. A saved size the display
cannot show keeps the current window and its existing notice. A preference
file with audio settings and no resolution allows the monitor fit. An unreadable
preference file has no saved size, so the monitor fit still runs. The browser
build does not resize the canvas bitmap here. Its page gives the stage the
viewport, and the canvas follows that box. Focusing the canvas reveals that
stage without resetting its internal pan. The stage origin is aligned to the
current device-pixel grid from DOM geometry; fractional surrounding page
layout must not clip a viewport-sized canvas. Render dimensions and authored
menu coordinates are unchanged by this page-layout correction.

## Production selector

When the selected client admits a complete selector-2 popup caller, Options uses
its source pair families and captured full-monitor bounds as described below.
Otherwise, with one enumeration-family profile and primary-monitor video modes,
Options lists those grouped enumeration rows.
Admission depths, minimum width, enumerated kind and flag, the refresh center,
the score operands and both label formats are read from that image. Host width,
height, observed depth and refresh are the records. Linux and other hosts that
are not Windows or macOS leave bit depth unqualified, so those labels use the
size-only format. Windows and macOS pass a nonzero reported bit depth. Refresh
still groups on every native host, including Linux. Offer order is first-seen
provider order after grouping. Winit's video-mode order is not
EnumDisplaySettings order.

The absent FixedAspectRatio checkbox supplies flag 0. If the discovered
enumeration flag is not 0, the qualified list is empty. The adapter-owner
compare and the active-kind compare stay off because those inputs are
unresolved and applying them would hide rows. Work-area families remain withheld.
An empty qualified result is not filled from a preset list or from the current
window.

Apply still changes the window or browser render extent. It does not request
an exclusive physical mode. A browser popup row is a logical canvas size for
the physical screen observation: render extent only, with no monitor, depth,
or refresh. Hardware identity and the source current-kind policy stay out of
that row. A missing image, an unsupported image, or a missing screen bound
keeps measured render sizes. An admitted browser popup list that is empty
stays empty. A qualified profile whose primary-monitor rows are all rejected
stays empty.
Version-1 preferences store the render size only. Same-size depth and refresh
remain a session selection until cancel, reopen, a draft size change, or
replacement of that row.

Synthetic image tests check admission, order, grouping, depth and refresh
filtering, missing input and score ties. They do not establish rendered Options
parity or a physical mode switch.

### Indexed capacity investigation

Further bounded offline reads of the same qualified Legacy client identify
`7d8b70` as a count produced through `EnumDisplayMonitors`. The indexed width
and height helpers select an enumerated monitor handle. Their common rectangle
helper chooses `MONITORINFO.rcMonitor` or `rcWork`; `7d7d80` then subtracts
the frame extent returned by `AdjustWindowRectEx` for source-selected styles.
The selector-3 table paths use the work rectangle, while selector-2 paths use
the full monitor rectangle. Those style operands must still be derived from
the supplied client, rather than copied into a preset policy.

A single primary-monitor physical extent therefore does not establish every
candidate family's bound. The current production enumeration path does not
yet supply work-area and source-style frame capabilities or active-kind and
adapter-owner selection. Those remain integration work. Repeated label sites
with identical formats and limits now share one effective profile; conflicting
sites still refuse. Ten synthetic tests and the explicit selected Legacy image
admission gate pass. These checks do not prove equal rendered choice lists.

Evidence: `.scratch/options-display-capacities-20261017/` and
`.scratch/options-display-selector-root-20261017/`. The six code windows total
3 KiB; independent instruction decoders agree on their reached boundaries.
No game, display mode, window or running original process was changed by these
reads. Allocation failure and monitor-order correspondence remain unqualified.

A separate conditional check reads the second pair table from that unchanged
image. The caller saves the selector-2 full-monitor result and copies those
dimensions unchanged into both dimension pairs of its final row. The earlier
aspect-transform call belongs to another path; finding that call elsewhere in
the same function does not bind it to this row. A 4:3 witness cannot distinguish
the two interpretations.

With the original list capture's framebuffer supplied as the full-monitor bound,
the popup flag-zero table rows followed by that full bound match all five visible
rows in order. This checks consistency with that observation; it does not
establish the original active kind, adapter owner, absence of hidden rows, or
equal choices on another monitor. No table payload was saved by the check.
The corrected receipt is
`.scratch/options-display-visible-witness-20261017/full-bound-receipt.json`;
the earlier aspect-fit receipt is retained as superseded evidence.

### Selector-2 caller integration

The checked profile now links the frameless bounds helper to one capture call
followed by two table loops. It reads table extents, pairs, kind and flag from
that selected image. Table records must explicitly write zero depth and refresh
before those fields are represented as absent. Both equivalent x86 frame and
captured-width MOV encodings are supported; the supplied Legacy gate exposed an
encoding the original synthetic fixture missed.

Each current monitor supplies its own full physical bound. Pair families keep
caller order, then the validated final record adds that raw bound. The unrelated
aspect transform is not applied. Equal dimension/depth/monitor/kind/flag keys
group at their first occurrence, so a full bound that already exists in a table
does not add a duplicate choice. Different monitors and flags stay distinct.
Table offsets are session row identities, not exclusive OS mode IDs.

The app retains the selected monitor Entity and refreshes its pass ordinal after
query reordering. If that Entity disappears, it preserves the draft and stale
witness; Apply refuses until an explicit valid selection replaces it. An
equal-sized remaining monitor cannot silently take the old choice. Neither the
Entity nor its ordinal is persisted in version-1 render-size preferences.

Discovery caps candidates, calls, groups, inspected tables, code passes and
backward searches before growth or reads. Host inputs and emitted rows have
separate bounded counts. Bad complete table operands refuse instead of becoming
a missing profile. The parsed profile is shared from the application cache;
its pair tables are not cloned every frame.

Nineteen synthetic display tests pass natively and execute under Wasmtime. The
explicit supplied-Legacy popup-profile gate, 347 application tests, strict native
and browser-WASM Clippy, warning-denied profile documentation and formatting
checks pass. The duplicate-row regression
first fails with six rows instead of two. Evidence, including earlier compilation
and admission failures, is retained in
`.scratch/options-display-popup-root-20261017/`.

This is a source-qualified frameless subset. Selector-3 work-area/frame
capacities, original active-kind and adapter-owner policy, monitor-order parity,
exclusive switching and equal rendered original choice lists remain open.
Browser builds list logical canvas rows when the committed image admits this
popup profile and the physical screen bound is positive. A missing or
unsupported image keeps measured render extents. The retained installer-engine admission did not find this profile. The four fresh browser menu captures in
`.scratch/menu-depth-root-20261017/` precede this selector integration and do
not establish its rendered acceptance.

### Browser import and conditional visible-list gate

The existing file picker now commits an optional bounded client image with its
archives. A successful hydrate publishes the image on the same revision; cancel
or failed hydrate retains the previous image and assets. Replacing the archives
without an image clears the previous profile input. The image stays local and
outside the asset store. The retained installer-engine gate did not admit a profile; this runtime gate
uses an explicitly supplied Legacy image that admits the frameless profile.

With that image and the retained original screen bound, the actual browser
dropdown offers the same five visible rows in the same order. This is a
conditional runtime comparison against one retained Wine capture, not an
exhaustive device census or proof of original active-kind/adapter policy.
The reviewed current screenshot retains Options panel, typography and popup
artwork differences. No physical mode switch was attempted.

The generated-WASM transaction passes eight checks; the focused dropdown run
records no page errors, uploads or external requests. The screenshot's state
is stable across capture. Forty importer/settings JavaScript tests and 353
application tests pass; strict native and browser checks pass. Evidence is
`.scratch/browser-display-rows-root-20261017/`. The browser artifact is private
and contains no original assets. Full original list and rendered parity remain
open.
