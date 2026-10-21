# Command card

The command card is still being implemented. Learned Holy Light and Water
Elemental buttons now have a receipt-bound renderer and input consumer;
ranked tooltips are connected. Standard Move, Stop, Hold, Patrol and Attack
presentation and dispatch are connected as described below. Readiness presentation remains unfinished.

## Selected-client evidence

The selected Legacy executable has VERSIONINFO `1.29.2.9231` and SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Static reads use that complete local image, not the older compatibility DLL or
the IDA launcher decompilations. Extracted inputs and disassembly stay local in
the ignored `.scratch/command-card-source/` directory.

`UI\FrameDef\UI\ConsoleUI.fdf` in the selected base archive defines console
textures, without command-button frames. The client supplies the command grid:

- MSVC RTTI identifies `CCommandBar` through type descriptor `0x10f8c98`,
  complete-object locator `0xff1060` and vtable `0xe95c60`.
- The constructor at `0x61d130` installs that vtable at `0x61d173`.
  The gameplay UI creates it at `0x5e805d`, passing its console parent from
  field `+0x468`. That parent is loaded through the skin key pushed at
  `0x5e7ca5` and retained at `0x5e7cbd`.
- The constructor passes the grid dimensions at `0x61d1c4`/`0x61d1c6` to
  `0x88d050`; the two creation loops compare the same dimensions at
  `0x61d357`/`0x61d364`. Allocation and loop operands must agree.
- Width and height operands at `0x61d1d6`/`0x61d1e5` go to setters
  `0x873b40`/`0x873830`, which write frame width/height at `+0x58`/`+0x5c`.
- The anchor call at `0x61d20c` supplies the bottom-right point on both frames,
  with X/Y operands at `0x61d200`/`0x61d1f8`. Other anchor pairs are unsupported
  by this reader rather than treated as the same layout.
- Each grid child installs the `CCommandButton` vtable `0xe9528c` at
  `0x61d2a9`, then reads its own width and height from `0x61d2e1`/`0x61d2f0`.
- `0x88c970` stores the child at the row/column pair supplied by the loops.
  The grid's layout body at `0x88cb50` divides its extent by the row/column
  dimensions and anchors each child at its top-left, with negative Y steps.
  Its four inset fields are initialized to zero by `0x88c7a0`; the command
  constructor does not set different insets.

`pcx_play::command_layout::Rules` reads the operands from the supplied PE at
runtime, including the parent skin key and grid's default insets, retains input
identity and hash, and derives slot rectangles in logical
UI coordinates. The viewport references are the same selected-client reads as
the existing inventory reader. Instruction locations identify this reviewed
build; they are not copied geometry values. Synthetic fixtures use different
dimensions and operands.

The focused suite passes eight tests, including the explicitly enabled local
selected-client gate. Strict `pcx-play` all-target Clippy and the WASM library
check pass on the same frozen inputs. Receipts are retained locally in
`.scratch/command-card-source/layout-results.json`. These checks prove the reader
and its refusal paths, not displayed buttons.

## Ability presentation inputs

The shared `ability_profile` reader now applies scalar W3A profile modifications
through the loaded AbilityMetaData mapping. Last authored modification wins;
empty values remove the stock value. UTF-8, string wire/type, family and metadata
applicability are checked. Unknown fields, multi-slot fields and ranked profile
modifications refuse explicitly. Stock values are not substituted for those
unsupported map records.

Both live hero ability reads and startup's Holy Light model read use that reader.
This keeps a changed `Targetart` path paired with the model whose sequence data
startup admits. Holy Light input now reports profile errors instead of dropping
them. Synthetic checks cover replacement hotkeys and art paths, repeated and
empty modifications, and malformed/indexed refusal. This does not mount the
command card or implement ranked tooltip expansion.

The selected `HumanAbilityFunc.txt` records for `AHhb` and `AHwe` carry `Art`,
`Buttonpos` and `Order`. `HumanAbilityStrings.txt` carries `Hotkey` and ranked
`Tip`/`UberTip` lists. The existing object catalog preserves each assignment,
its parse errors, archive and edition/version. Rendering must consume those
records, including source token substitutions, rather than supplying icon paths,
button positions or tooltip text from code. Map profile overrides must not be
silently replaced with stock presentation.

## Remaining work

The geometry reader alone does not establish native/rendered parity. The button
checkpoint below connects the two implemented learned commands, source icon
reads and sampler admission, pointer consumption and dispatch. Tooltips,
cooldown/readiness presentation and other command types remain unfinished.
Native command ordering, disabled states and the rendered console relationship
still need runtime comparison.

## Learned button data (2026-10-21)

`ObjectHost::ability_button(unit, ability)` now projects the learned rank,
player availability, authored `Art` and metadata-mapped `Buttonpos`. Unlearned
or player-unavailable abilities return no button. This is presentation data,
not a mana, cooldown or target-readiness decision; accepted orders must still
go through the gameplay command path.

The two position components are identified through AbilityMetaData profile
slots, not field fourcc literals or copied coordinates. Each scalar map
modification replaces only its own component. A valid map override does not
require a stock coordinate to exist. Malformed types, nonzero selectors,
negative coordinates and unsupported mapping attributes refuse. Negative
coordinates may have native hiding semantics; no such rule is claimed here.
The caller must pass the position through the selected grid's `slot` bounds
check before drawing it.

Synthetic tests cover component mapping, map replacement, absent stock values,
malformed inputs and learned-rank projection. The explicit original gate
`ability_profile::tests::original_button_positions_agree_with_authored_profiles_and_client_grid`
compares both implemented Human hero spells with their supplied profile text
and checks the selected client grid. It requires `PCX_NATIVE_CHAPTER_INSTALL`
and `PCX_WAR3_EXE`; no measured coordinates are embedded in the test.

## Button rendering and dispatch (2026-10-22)

`campaign_command_card` mounts the two implemented learned abilities under the
console frame selected by the skin. The mounted HUD must match the exact
published context and binding revision. A new session removes the old root and
only its owned image assets. Geometry remains relative to the source viewport.
Duplicate authored slots refuse rather than inventing button priority.

The selected executable links the command-bar creation (`0x5e805d`) through
the child-button constructor (`0x61d24d`). The command-grid lookup at `0x5eafb6`
feeds the art setter at `0x5eafd5`, its wrapper at `0x5d3caf`, and texture
loader at `0x5d3e60`. The GameUI field written at `0x5e8074` must match the
field read at `0x5eafab`; the reader checks their relationship. Texture
addressing comes from flags at `0x8750db` and `0x875141`, interpreted through
the loader mask at `0x887ce5`. Both paths must agree. These static links qualify
the addressing input, not native filtering or rendered equivalence.

Icons use bounded, receipt-carrying reads and decoding. Successful and failed
image reads are cached per session. Stable selection, learned ranks and
player availability reuse presentation data rather than rebuilding nodes each
frame. Button presses consume world pointer input. Activation rechecks the
session, local ownership, selection, controls and current command availability.
Targeted spells arm the existing target-click path; immediate spells use the
same native and accepted-command log as their hotkeys.

Synthetic session/pointer tests and a selected-client geometry test cover
stale witnesses, pointer consumption, preflight of failed replacement and
owned-asset teardown. The image assets in that teardown test are synthetic;
it does not establish displayed original icon fidelity. The explicit Human01
input gate uses the real chapter startup, with operator setup to enable control
and select/learn Holy Light. It is component input evidence, not progression
through the authored cinematic.

Ranked tooltips and substitutions, cooldown/mana presentation, disabled and
pressed art, remaining commands, and native/rendered comparison remain open.

Private-target validation: 1,086 core tests pass (31 ignored), the selected
layout gate passes, and 639 feature-enabled library/bin app tests pass (50
ignored). The explicit Human01 input gate passes after the system-parameter
cleanup. Strict all-target Clippy with `profile-gpu,test-fixtures`, affected
WASM checks and touched-file formatting pass. Logs are retained locally under
`.scratch/command-card-source/card-validation` and `card-validation-fixed`;
the first chain's Clippy failure is preserved, with its corrected rerun in
`card-clippy-2.log`. No original asset or screenshot is committed.

## Original icon component capture (2026-10-22)

The explicit selected-install gate now reads both authored `Art` paths through
the bounded install resolver, decodes their images, and checks that repeated
reads reuse the image handle and receipt without increasing retained bytes.
A missing synthetic path keeps its first refusal in the bounded error cache.
The runtime layout provenance now uses the shared client-identity helper,
including the executable digest, rather than a descriptive version placeholder.

The native ignored component fixture renders each spell separately through
the production `paint` and image-loading functions. Both original icons were
observed at 1280×960 and 640×480, with clean process exits. Their non-background
pixel bounds shrink from 61×61 to 30×30; placement shrinks with the viewport
with raster rounding. This uses a synthetic full-viewport parent and a
synthetic rank/selection. It establishes icon decoding and component rendering,
not the mounted campaign console, pointer picking, native filtering or original
client appearance parity. No visual adjustment was made from screenshot fits.

Local receipts, four screenshots and binary hashes are in
`.scratch/command-card-source/icon-visual`; the binary was selected from the
private Cargo JSON artifact stream. Both windows were closed through their own
window IDs; no other application was controlled. The only runtime warning was
an unmapped gamepad using its default mapping. Strict all-target Clippy passes.
The fixtures, screenshots and original bytes remain local.

## Admitted console lifecycle (2026-10-22)

The explicit Human01 gate now runs the normal chapter startup, HUD source-policy
producer, admitted HUD mount and command-card projection together. Operator
selection/learning selects Arthas's Holy Light independently of authored intro
progression. The card attaches under the actual source-selected console frame,
and repeated updates preserve both its root and button entities.

A fresh startup of the same map is a different context and session. Publishing
that session with the old bindings retires the previous card and its image
assets while retaining the caller-owned console. It cannot borrow the old
bindings. Publishing its matching requests and source reader then mounts a new
card; clearing the session retires it again.

This exposed a HUD policy bug: the producer's attempt flag persisted across
all sessions. The source policy now retires on a session-witness change and is
derived again only when the session's exact admitted context and published
receipt arc match the bindings. The original gate failed at the stale-policy
assertion before this correction and passes after it. The red and green receipts
are `.scratch/command-card-source/mounted-console-red.log` and
`mounted-console-green.log`. This is actual admitted ECS mount/lifecycle evidence,
not a full campaign render, physical-picking test or original-client comparison.

## Ranked command tooltips (2026-10-22)

Learned available commands now read `Tip` and `UberTip` at their admitted rank
from the original profile assignments, retaining their CSV quoting rules. Ranked
W3A string replacements are selected through the loaded AbilityMetaData mapping;
an authored empty replacement remains empty. Unsupported mappings, unresolved
map strings and malformed references report a refusal instead of substituted
content. The explicit selected-image metadata gate also checks real repeated
Tip/UberTip mappings using synthetic rank edits.

Bounded `<rawcode,column>` expansion reads ability numeric cells or exact unit
SLK columns. A rank suffix inside a reference stays literal: the reader does not
rewrite it to the caster's rank. Referenced unit columns with map modifications,
percentage-format tokens and unresolved WTS text remain unsupported. Holy Light's
separate scalar UberTip admission still needs ranked-edit compatibility review.

Command buttons share the inventory tooltip's source fonts, colours, frame art,
preference and geometry under the admitted console parent. Hover exits and
session replacement retire that tooltip; malformed rich text releases partially
created nodes. Empty authored titles do not acquire a fabricated label.

Synthetic tests and explicit installed-original checks establish data selection
and host/UI contracts. Both spell tooltips were also hovered and captured at
1280×960 and 640×480, using real source fonts, frame art and rank-three text
under a synthetic full-viewport parent. This exposed the shared tooltip's default
Bevy image-fitting behavior: its background and vertical edges did not fill their
derived rectangles. They now use the same explicit Stretch mode as the shared
menu backdrop renderer. A synthetic regression checks that mode for every image
piece; source rectangles and UV selection are unchanged.

Corrected captures use a private X11 display and software Vulkan. Pointer state,
tooltip readiness, binary and image hashes, and clean process exits are retained
under `.scratch/command-card-source/tooltip-visual-final`. Host-desktop
attempts where no cursor event arrived remain failed receipts, not evidence of
tooltip rendering. The private display was stopped without changing host input
policy. Original-client appearance parity, cast-readiness presentation and the
remaining command kinds stay open. Local source assets and captures are not committed.

Private-target checks: 1,089 core tests (31 ignored), 64 object tests with both
explicit local install paths (4 ignored), and 589 feature-enabled binary app tests
(51 ignored) pass. The explicit original Human01 rank-expansion and admitted-console
mount/replacement gates both pass. Strict affected all-target Clippy, native
test-fixture compilation, affected WASM library/bin checks and formatting pass.
Receipts remain in `.scratch/command-card-source/tooltip-*`; earlier failed
missing-default-install census, image-fit and host-pointer attempts are retained.

## Readiness inputs (2026-10-22)

The live Holy Light and Water Elemental casts now share the payment/cooldown
snapshot used by their read-only readiness queries. It reports the source-ranked
mana cost, current optional mana pool, remaining game-clock cooldown and pause
state. Missing pools stay unavailable. This is not a target-validity, creation
capacity or full cast-acceptance result. In particular, summoning starts its
cooldown when the cast-point callback actually creates the unit, not when the
input order is accepted. No random draw, pool write or clock advance occurs in
the query.

Static selected-client evidence establishes the disabled-image construction:
the texture setter at `0x5d3e30` searches both path separators for the basename,
loads the skin key through the pointer at `0x5d3eaa`, then uses the format pointer
at `0x5d3eb8` before passing the result to texture state zero at `0x5d3edc`.
These are inputs from the selected image and skin, not a disabled-image path
constant. Local disassembly is in `.scratch/command-readiness/texture-setters.asm`.
This does not establish which low-mana, cooldown or other gameplay states select
that image, their tint, or their model animation. Button visual-state mapping
and its runtime comparison remain open; the normal icon is not recoloured from
an assumed grey or blue palette.

The private core suite passes 1,090 tests (31 ignored), including source-fixture
costs, cooldown start/expiry, pause and insufficient-mana refusals. The explicit
Human01 gate checks every rank's snapshot against the installed AbilityData cost
column and the live unit pools/pause flag. Affected core/app strict all-target
Clippy, WASM and native app tests pass. Receipts are local under
`.scratch/command-readiness/`. No new F7 rows or button states are published by
this core checkpoint. The subsequent fractional-field change replaces Holy
Light's older integer-only read (see `holy-light-ability.md`).
The selected `common.j` declares ability cooldown natives. Live remaining/end and
unit-duration bindings below cover the two implemented spell classes; the global
source query also reads other admitted rows without requiring a cast implementation.

## Script cooldown remaining/end (2026-10-22)

`BlzGetUnitAbilityCooldownRemaining` and `BlzEndUnitAbilityCooldown` now use the
same per-unit deadlines as Holy Light and Water Elemental. Their declarations
are optional at admission; when present, their signatures, unit family and source
must match the admitted unit declarations. A present ability whose cooldown
behavior is not implemented refuses explicitly. This is not general support
for every ability class or item cooldown group.

Primary registrations in the selected image bind remaining at `0x4a185e` to
`0x497190`, which resolves the unit and calls `0x6d6e00`; end at `0x4a1872`
binds `0x48fe60`, which calls `0x6d63a0`. The bodies look up the ability and
perform a type check. Failure returns zero for remaining and does nothing for
end. The remaining body calls the ability's getter at vtable offset `0x2dc`;
end calls offset `0x3b0`. Null and removed handles take the wrapper's absence
path. The recreation does not replace this lookup with the UI's learned-and-
available readiness query; disabled/dead units can still hold tracked cooldowns.
These are static primary-path reads, not a new observation of the original
client's active registrar or timer behavior. Disassembly and selected-image hash
remain local under `.scratch/ability-cooldown/`.

Synthetic scripts verify query/expiry/end/recast, independent units and spells,
summon cast-point timing, no cancellation of a pending summon, wrapper absence
paths, argument refusals and unchanged RNG/clock/pools on queries. Ending a
cooldown does not refund mana or undo an effect. The original Human01 host gate
also checks both declared bindings against actual learned ranks, but it does
not establish original-client timer parity.

Other live ability classes and item transfer/group timing remain open. The
global duration getter's level-zero path is not qualified here.

`UnitResetCooldown` now clears all currently modeled deadlines for one unit,
including the holder's item groups; configured durations are preserved. Its
source-checked admission also works without hero-skill rules. Spell/item state,
null/removed units, pending casts, wrong-handle refusals and reclaimed record
budgets are covered by host tests. See [item cooldowns](item-charges-cooldown.md)
for the selected-client traversal and the explicit item grouping/transfer limits.

### Per-unit configured durations

The selected 1.29.2.9231 image's primary unit-duration getter (`0x497150`)
and setter (`0x4aade0`) reject levels below one and subtract one before calling
`0x6d6dc0` / `0x6d9030`. The spell, Holy Bolt and Water Elemental vtables all
resolve their duration getter/setter to `0x46b400` / `0x46b590`. The getter reads
the per-instance data clone when present, otherwise the source data. The setter
clones source data on first write and changes the requested rank's duration cell;
it does not write the active timer. These are static reads from the selected
image (SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`),
not a new original-client observation or proof of runtime registrar selection.

`BlzGetUnitAbilityCooldown` and `BlzSetUnitAbilityCooldown` use one-based rank,
unit-local configuration for the two implemented spells. Defaults use the same
typed, map-overridden `Cool` cells as casts. Both cast paths read this configuration;
a duration change leaves an existing deadline intact, while a pending summon
reads the current duration at its cast point. Queries and writes do not draw RNG
or advance time. Physically resolved source ranks bound retained overrides.

The recreation-owned cached-unit format is now version two and preserves these
rank overrides as exact float bits; decoding version one remains supported.
Restore checks ranks and cells against the selected source before publication.
This does not establish the native game's cache persistence policy. Negative or
nonfinite duration writes refuse before mutation; the native setter copies raw
bits and its subsequent cast behavior for those values is unqualified. Ranks
above the authored count take a bounded no-op/zero path here: the native setter's
already-cloned out-of-range indexing is not emulated or claimed safe.

The getter's out-of-range value is zero: startup `0x401300` calls `0x9a1620`
with zero and destination `0x112d88c`; the helper's zero branch writes zero bits.
The getter copies that value on its absent-row path. The local receipt includes
the initializer and helper reads.

Duration-increment checks pass in the private target: 1,101 core tests (31
ignored), 589 app binary tests with `test-fixtures` (51 ignored), the installed
Human01 every-rank getter/setter gate, strict affected all-target Clippy and
affected WASM checks. Synthetic tests discriminate unit/rank scope, a changed
next cast, unchanged active deadlines, and a pending summon's cast-point read.
Cache tests cover exact override bits, restore, old-version decoding, truncated
records and malformed/duplicate overrides. These are recreation host and parser
checks; no new original-client timer/cache or rendered capture is claimed.

### Global source duration

`BlzGetAbilityCooldown` is now admitted from the same selected declaration source
with its integer/integer-to-real signature. Unlike the unit APIs, it reads the
ability's source/default `Cool` cell with the map's typed ranked modifications,
including a custom alias's base row. It never reads a unit's configured duration
or active deadline, and it does not require learning or a supported cast class.
Its tables still must have been admitted; this is not executable discovery or a
fallback to another install.

Static primary registration `0x4a181f` selects `0x490c60` in the selected
1.29.2.9231 image. The handler looks up the ability data with `0x6aefb0`, returns
zero for absence or an unsigned out-of-range level, subtracts one from an accepted
level and reads the duration cell. This supports one-based positive levels and
zero for negative/above-count levels, not another version's indexing convention.
For a present row, level zero reaches an unqualified preceding-row index; the
recreation refuses it explicitly. Missing identifiers return zero, whereas
malformed/unsupported source cells report their source error instead of being
converted into a fake zero. Physical cell resolution and supported typed map
selector rules are the same as the existing cast/default reader.

Synthetic checks cover unlearned/no-live-unit queries, a class without admitted
cast behavior, exact ranked map edits, a custom alias, unknown identifiers,
bad arguments/levels and malformed cells. Repeated queries leave the game clock,
RNG, mana, unit override and active cooldown unchanged. The installed Human01
gate compares the global value with the actual source row at each learned rank,
including after a unit-only duration mutation. These host/source checks are not
an original-client runtime comparison; active registrar and rendered parity stay
separate. Private receipts are under `.scratch/global-cooldown/`.

Private-target checks pass: 1,112 core tests (31 ignored), 589 app binary tests
with `test-fixtures` (51 ignored), the installed Human01 every-rank source/unit
query gate, strict affected all-target Clippy, affected native/WASM compilation,
and formatting. No new original-client runtime or rendered capture was made.

Private-target checks pass: 1,097 core tests (31 ignored), 589 feature-enabled
app binary tests (51 ignored), the explicit installed Human01 rank/binding gate,
strict affected all-target Clippy, native compilation and affected WASM library/
binary checks. No new rendered or original-client runtime capture was made for
this script-state increment.

## Standard commands (2026-10-22)

Move, Stop, Hold, Patrol and Attack read the selected `CommandFunc.txt` and
`CommandStrings.txt` through the receipt-bound UI resolver. `CmdHoldPos` is the
profile identifier for Hold. Art is a skin key, resolved through the same
selected race skin chain as the console; it is not treated as a texture path.
Button positions, hotkeys and tooltip strings remain authored data. The existing
PE-derived command grid validates the positions and scales the mounted children.
Missing, ambiguous, oversized or unsupported profile values refuse by field.
Command profiles belong to the editor reader, not `DefaultTables`' gameplay
unit/item/ability/upgrade namespace.

The command identifiers come from the admitted activity/combat rules. Capability
presentation currently uses the primary selected unit's liveness, visibility,
mobility and weapon availability. This is a recreation policy; exact native
availability for mixed selections, buildings and disabled intrinsic abilities
is not established. Stop and Hold apply immediately to eligible locally-owned
selected units. Move, Patrol and Attack arm a session-bound ground/target click;
right click cancels. The consumer rechecks session, ownership, selection, user
control and the existing HUD/dialog/input gates. Hotkeys use the authored
profile values. Standard hotkeys currently precede learned-spell hotkeys when
they collide; native collision precedence is unqualified.

Ground Move, ground Patrol, ground Attack and unit-target Attack use the existing
native order paths. The source Move tooltip describes following a unit, and the
Patrol tooltip describes imitating one. Those widget-target modes remain
unsupported and report that state instead of substituting a ground destination.
Only `Bool(true)` native results enter the player command journal. Refused and
failed orders do not become accepted replay entries; armed Holy Light and the
right-click order path share this result boundary.

Synthetic parser mutations and host dispatch checks cover the source-derived
fields and all five order identities. App tests distinguish accepted, refused
and failed journal results. The explicit installed Human01 checks mount all five
buttons under the admitted console and exercise group Hold/Stop with control
closed and reopened. These are component/host checks, not original-client or
full campaign rendering evidence. Target cursor artwork, disabled/readiness art,
follow/imitation, mixed-selection fidelity and native visual/input parity remain
open. Private validation receipts are under `.scratch/standard-commands/`.

Private-target validation passes: 1,114 core tests (31 ignored), 590 app binary
tests with `test-fixtures` (52 ignored), and three explicitly enabled Human01
component/input gates (mounted console lifecycle, group Hold/Stop control gates,
and the existing session/selection/control click check). Strict affected
all-target Clippy, core WASM and app WASM library/binary compilation, and
touched-file formatting pass on the frozen source manifest. No new original
client or rendered standard-command capture was made.
