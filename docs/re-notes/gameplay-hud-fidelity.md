# Gameplay HUD viewport and stat repairs

The selected-unit capture exposed three independent defects: terrain behind
empty console areas, an entirely missing stat panel, and Armor drawn over the
hero attributes. The command icons themselves already used the source grid.

## World frame

The selected client constructs `CWorldFrameWar3` through the GameUI child stored
by the call at `0x5e77d2`. Its two normal-gameplay anchors at `0x5e7b48` through
`0x5e7ba0` address that same child. They bound the world between the upper and
lower console areas. The previous consumer rendered the scene across the full
display, exposing terrain through the console textures' transparent regions.

`command_layout::Rules` now checks that constructor/child/anchor relationship
and decodes the offsets from the selected image. No measured screenshot bounds
are embedded in the renderer. Synthetic tests vary both anchors and reject a
different child; the explicit selected-image test passes.

The existing internal-resolution consumer applies these bounds in display
coordinates and scales the world viewport for supersampling. A separate empty
camera initializes the output to black; scene and sky passes preserve that
initialization. A viewport alone is insufficient because a render-pass clear
can clear the entire target outside its draw viewport. HUD composition remains
at display resolution. World pointer input rejects positions outside the camera
viewport instead of issuing orders through an empty command slot or panel.

These are normal gameplay bounds. The existing cinematic interface suppression
and its presentation remain separate; this change does not establish cinematic
viewport parity.

## Unit panel

The panel previously required every label in `InfoPanelStrings.fdf`.
`COLON_ARMOR` is supplied by `GlobalStrings.fdf` in the selected install, so its
absence aborted the entire panel. Both receipt-carrying source files now
participate in lookup, with the panel-specific record taking precedence. A
missing label in both still reports an error. The synthetic regression covers
fallback, precedence and refusal.

The initial native constructor anchors Armor and the hero block in the same
column. That initial configuration is not the displayed layout observed in the
original client. The recreation stacks Armor below Damage using the authored
damage-frame height, leaving the hero block in the adjacent column. No pixel
offset or assumed row size replaces that asset input. This is an observable
layout reconstruction, not a claim about identical native layout internals.

## Armor formatting and command states

The armor formatter at `0x654af0` rounds the base value to a signed integer
before the integer-format call at `0x654d24`. Its bonus remains a separate
floating-point formatter. The recreation now preserves that split. Synthetic
rounding tests vary positive and negative inputs; the selected Prologue02 hero
now displays Armor 4 instead of 3.7, matching the original reference.

Blocked research icons use the selected client's disabled-texture basename
format and the skin's `CommandButtonDisabledArtPath`. The format and key are
read from the texture setter, rather than constructing an assumed texture name.
Hero level and available skill points participate in command-card refresh.

Opening the real research page exposed a separate text-parser refusal:
`<AOeq,DataC1,%>`. A tooltip failure prevented the complete page from loading.
The shared tooltip expander now accepts the percent modifier for exact integral
percentages, with bounded decimal arithmetic. Fractional percentages still
report an unsupported formatting policy until native rounding is qualified.
Synthetic cases cover signs, trailing zeros, malformed input and overflow.

## Research rank badges

The command-button number overlay is constructed at `0x62c4c3`. Its checked
bottom-right anchor pair, dimensions, offsets and centered font child now come
from that constructor. The overlay art and font keys are read from the selected
image; the font height is read from the corresponding `MiscUI.txt` section.
The default region color is also decoded. Nothing is fitted to screenshot
coordinates. Synthetic fixtures vary these inputs and reject mismatched anchors
and nonpositive dimensions.

`ResearchIcon` exposes the same upcoming rank already used by the research
title. The UI displays it in the source overlay. Disabled skills keep their
rank labels; overlay children do not intercept button input. Images and the
owned font retire with their session. A selected-image regression checks failed
replacement atomicity and preserves unrelated font/image assets.

The fresh offscreen research capture shows the expected 2, 2, 1, 1 labels for
the selected Prologue02 hero. The GPU test compares every rendered number with
the canonical research presentation, rather than embedding those values as an
assertion. This establishes that selected state, not identical font rasterization
or every other hero/edition.

## Active standard commands

The selected client loads `CommandButtonActiveHighlight` into an all-points
texture child at `0x62c07f`–`0x62c0c8`. Its blend operand is checked against the
FDF parser's `ADD` mapping at `0x85b6b4`–`0x85b6c6`. The existing additive UI
material consumes that source texture; no green tint or fitted border geometry
is supplied by the recreation.

The command card refreshes from the admitted current order and canonical Hold
state. Idle units highlight Stop; held units highlight Hold; other standard
commands match their admitted identifiers. This is a presentation mapping onto
the recreation's canonical order state. The original idle Stop reference is
observed; it does not establish every native order transition. Synthetic cases
use non-stock order identifiers. Unavailable order state leaves the highlight
absent. Owned overlay materials retire with the source session, preserving
unrelated materials and textures. The fresh selected-unit capture shows the
green Stop border, and the offscreen test verifies that it is the only active
standard-command overlay in this idle fixture.

## Hero class line

The class label now follows `SimpleClassValue` in the source FDF: its stats
panel follows the detail frame, and its top anchor follows the name's bottom.
Font, color, shadow and offsets come from that record and its inherited value
style. `INFOPANEL_LEVEL_CLASS` supplies the localized level/class format. The
level is canonical hero state; the class is the effective unit `Name` profile,
including map-string resolution. The title continues to use the proper name.
Nonheroes have no class line, and selection/session retirement clears it with
the existing panel tree.

Synthetic tests vary the format, anchors and font settings, and reject a wrong
anchor target, unsupported conversions and unsupported justification. A fresh
muted offscreen capture shows the selected hero's class line. The explicit
Human01 selection test also verifies that the class line is present for the
hero, absent after selecting a nonhero, and retired with the session. Its test
app now initializes the font/image assets required by the existing panel system.
Strict native/WASM checks and both builds pass for this increment. The class
label does not establish font-rasterization parity; XP presentation follows below.

## XP-bar source range and presentation

The reviewed client's updater at `0x65b90d` reads current XP and the threshold
for the current hero level; above the initial level it subtracts the preceding
threshold before dividing. Its threshold helper (`0x691ae0` → `0xb3ead0`)
clamps the requested level to the source maximum, then reads that level's next
threshold. The UI therefore needs one endpoint beyond the thresholds used to
admit gameplay level-ups. This is native-code evidence, not rendered verification.

The shared source-table reader now retains that extra presentation endpoint
separately. Its gameplay threshold vector and level cap are unchanged. If the
extra recurrence overflows, valid gameplay rules remain admitted and only that
presentation read reports the error. `ObjectHost::hero_experience_range` exposes
current XP, floor and ceiling from canonical state; absent legacy-cache XP is
reported rather than fabricated. Synthetic tests cover nonstock recurrence,
initial/middle/capped ranges, overflow isolation and an equal-threshold endpoint.
The host test follows real XP changes and suspension and rejects unadmitted XP.

The status-bar constructor resolves the console fill and border through skin
keys. The XP style is selected from the updater dispatch table and checked
against the constructor dispatch. Tint, background, inset, normalized range and
non-increasing-threshold fallback come from validated image instructions/data.
The texture updater crops both texture coordinates and geometry; the recreation
now does both rather than stretching a full texture into the filled region.

The mounted bar uses its FDF title-relative anchor and height. A bounded call
search identified the XP-specific frame factory: `0x61e482` constructs the same
XP status-bar style, and `0x61e497` supplies its width to `0x873b40`. That width
is read from the image, with explicit FDF width taking precedence. The early
parent-width fallback was removed; no screenshot-fit extent is stored.

The first capture also exposed a rendering mistake: Bevy 0.20 `ImageNode::Auto`
fits the source aspect inside the node, making the border narrower than its
assigned frame. The panel image helper and XP fill now use `Stretch`, matching
the source texture's authored frame. The class text remains above the bar in
the authored child order. Owned fill/border images retire with the panel's
source session.

Nonstock synthetic tests cover art/style binding, tint and inset reads, partial
and clamped fill, equal thresholds, invalid initial thresholds, malformed
instructions, FDF anchor/height, explicit width and constructor width. The
real Human01 selection/retirement test, reviewed-image gates, headless GPU
resolution test, 2,046 core tests, strict native/WASM checks and both application
builds pass. Eighty core tests remain explicit ignores. Receipts are local under
`gates-xp-bar-width/`. Earlier fixture/address/lint failures, the parent-width
intermediate and the narrow `Auto` capture are preserved; they are not final
acceptance evidence.

A fresh muted offscreen HUD/research run passes and shows the constructor-sized
bar in `captures-xp-width/selected-unit.png`. A limited diagnostic samples 1,232
static border pixels against the retained same-resolution original HUD; only
297 match exactly, with a maximum channel difference of 189. This is recorded
in `xp-border-comparison.json`, not an acceptance pass or a runtime layout input.
Bar placement/width are visually aligned, but edge/color rendering remains open.
The final anchor follow-up also carries the authored title X offset; that offset
is zero in the captured source frame, so this correction affects nonzero-offset
layouts rather than this capture. Focused panel tests, strict native/WASM checks
and both builds are repeated under `gates-xp-bar-anchor/`.


## Command images fill their authored frames

The command grid and research rank badges now use `ImageNode::Stretch`, keeping
texture aspect from shrinking art inside the source-defined rectangles. Texture
bytes, skin paths, sampler flags, positions, dimensions and input/readiness
behavior are unchanged. Most stock command images are square, so this closes
the frame-fitting contract rather than claiming a large change in every stock
capture.

The actual selected-chapter card-mount fixture reproduces `Auto` before the
correction and passes with `Stretch`. Its ownership check now runs card-only
retirement before a full app update: the card must preserve the console, then
the HUD owner must retire that console on session replacement. The old fixture
incorrectly expected the console to survive both operations together. Coherent
publication, replacement and asset-retirement checks remain in place.

The headless research-menu test also checks mounted icon and badge image modes,
alongside the existing canonical rank/disabled-state checks. Focused card tests,
selected mounting/retirement, independent asset preflight, strict native/WASM
checks and both builds pass in `gates-card-image-final/`. The preflight-only
control is not the mounting regression; both are retained separately.
A fresh muted HUD/research renderer run passes the image-mode, rank, disabled-state
and source-input checks, with unchanged source hashes. Its reviewed research
capture is retained in `captures-card-image/skill-menu.png`. This does not close
font, working-color or whole-frame pixel-parity gaps.

## Gameplay working channels and portrait correction

A fresh isolated Wine/Xvfb run traces the unchanged Prologue02 briefing,
cinematic, public Escape skip, ordinary selected-unit HUD and research menu,
then stops the owned process. The checked stream contains 2,212,291 lines and
203,590,193 raw bytes, with EOF and raw hash verified. All 96,458 render-state
setters and 40,489 sampler-state setters parse; none selects sRGB sampling or
writes. There is no pixel-shader binding or gamma-ramp call. The 1,511 presents
use the ordinary device entrypoint, without presentation flags. Captures were
visually reviewed, and the private container is stopped.

[D3D9 documents default-disabled sRGB sampling and writes](https://learn.microsoft.com/en-us/windows/win32/direct3d9/gamma).
Together with the complete observed interval, this supports numeric working RGB
for this gameplay HUD interval. This is an inference from API evidence and
defaults, not a direct state query or native Windows hardware proof. Review
pauses are experiment conditions; they do not establish simulation timing or
matched animation phases. Trace, captures, hashes, source/map identity and
isolation receipts stay local in `hud-trace/qualification.json` and its private
reference directory.

The audit found a narrower input/output mismatch in the portrait path. It
already uses `mainmenu_scene`'s numeric textures, geoset colors and lighting,
but its camera omitted `MenuColourCamera`. The camera now uses the existing
numeric composition path. Source texture bytes, lighting values, camera pose,
frame dimensions and material relationships are unchanged. The portrait target
is then sampled by the ordinary linear HUD path, preserving its displayed
numeric colors.

The actual selected-chapter portrait fixture fails without the marker and
passes with it; it also checks the required composition space and existing
animation/asset retirement. The existing synthetic geometry/particle/UI GPU
oracle passes its raw-versus-linear comparisons. Focused portrait tests, strict
native/WASM checks and both builds pass under `gates-portrait-transfer/`.
A fresh muted production HUD/research capture also passes, with unchanged source
hashes throughout. `captures-portrait-colour/selected-unit.png` shows the corrected
portrait transfer; its pose/clock is not matched to the original reference, so
this is visual verification of the integrated correction, not pixel equality.
The capture receipt retains image and source hashes.

This does not convert the rest of the HUD to numeric composition. Its image,
text, tooltip, filter and rendered-image inputs must be handled consistently
before changing that camera. The XP-border edge/color discrepancy remains
open; neither the API trace nor the synthetic oracle proves whole-HUD pixel
parity.

## Combat-icon selection audit

The missing Armor icon cannot be repaired by choosing an icon from the unit's
owner or appending a neutral suffix. In the reviewed image, `0x654c8d` calls
`0x694230` with the unit type and the armor-upgrade identifier returned by
`0x67aff0`. The predicate searches the type record's `+0x234` vector. It does
not test three independent classification fields; the earlier code comment was
incorrect.

The vector's source is now identified. Field registration at `0x6b432f` binds
`upgrades` to descriptor `+0x380`; accessor `0x6b9290` reads it, and the type
loader calls that accessor at `0x67cf78`. The loop at `0x67d326` tokenizes the
list, resolves each upgrade, appends its identifier at `0x67d3ba`, and uses the
classification lookup at `0x6dd300` to retain the corresponding class-specific
identifier. The armor case stores it at `+0x24c` (`0x67d3cd`), which is precisely
the field returned by `0x67aff0`. The class lookup and jump table are shipped-byte
evidence. The presentation reader now carries those relationships into the
recreation without depending on the current research level.

When the predicate succeeds, `0x654cd2` selects the first armor-art pointer
table; otherwise `0x654ce5` selects the second. Both tables must be decoded from
the selected image: some entries are shared, so string suffix construction
would select nonexistent keys. The existing combat snapshot already exposes
the defense type. Its unit catalog deliberately refuses upgrade definitions,
however, so that snapshot alone is insufficient to choose the variant. The
existing upgrade-admission path is the appropriate place to retain effective
upgrade classes, including map overrides, rather than adding another stock-only
SLK reader to the UI.

The local `armor-icon-audit/receipt.json` records the reviewed image hash,
checked instructions and decoded tables; `native.log` preserves the bounded
disassembly. These static checks passed. The panel now decodes both armor-key tables and
the armor-class label from the reviewed image, resolves their skin art through
the selected race sections, and mounts the selected image in the existing FDF
icon frame. Owned images retire with the session. No owner-based classification,
constructed texture suffix, measured frame size or fitted offset is used.

Synthetic tests vary table contents and class labels, cover stock/custom/map
overridden upgrade classes, reject malformed selectors, and preserve valid
counter data when presentation refuses. The actual selected-unit fixture checks
the mounted image and its Stretch mode, then verifies owned-image retirement.
The core suite passes 2,048 tests with 80 explicit ignores; source-image tests,
strict native/WASM checks and both production builds pass. The fresh offscreen
HUD/research capture also passes with frozen source hashes.
`captures-armor-icons/selected-unit.png` (SHA-256
`c247237f43528e1ae4005ebd2e2267e637b729a03b32040db3646a54856ba10e`)
was visually inspected: the armor image is present beside its readout. This is
integrated visual verification, not exact native pixel equality. The damage-icon follow-up is described below.

## Damage-icon binding

The damage icon is now mounted through the same source-table, FDF-frame and
owned-image path as Armor. The native damage readout loads its attack-type
index from the weapon component at `0x6551e0`. The source parser at `0x6ac9e0`
does not use the ordering of the recreation's combat multiplier lists: it has a
zero-return spell branch, explicit middle-case results and a final hero branch.
The UI reader decodes those token strings and result indices from the reviewed
image, verifies a complete index domain, and uses them to select from the two
art tables referenced at `0x655387` and `0x65539a`. Neither a copied enum order
nor a constructed texture suffix is used.

The three upgrade getters called at `0x6552b7`, `0x6552c5` and `0x6552d3`
read different class-selected fields from the unit-type record. Their storage
cases in the source upgrade-class dispatch determine which admitted classes
select the first icon table. The same checked dispatch and classification table
now serve both Armor and Damage. Research levels do not change this artwork
choice. The panel omits the damage icon with the existing absent-weapon readout;
unknown attack types still refuse instead of receiving invented art.

Synthetic fixtures permute attack indices and class labels, exercise all three
qualifying damage classes, keep armor-only upgrades in the other damage-art
variant, and reject a duplicate index or mismatched getter. The actual panel
fixture checks both mounted images, Stretch modes and owned-image retirement.
The 2,050-test core suite passes with 80 explicit ignores; selected-source,
panel, input, GPU-resolution, strict native/WASM and both build checks pass.
The fresh headless/muted HUD and research capture also passes with frozen source
hashes. `captures-damage-icons/selected-unit.png` (SHA-256
`111d40771029023a98a594048adb2273931fbd9c6f306a2330f2efd0d709700b`)
was inspected: both combat icons are visible beside their respective readouts,
without clipping in this capture. The bounded static audit is retained under
`damage-icon-audit/`; this and the rendered check do not establish full HUD
pixel equality.

## Status label

The unit detail's missing status heading is native-constructed rather than a
stat-row FDF child. The call at `0x632cfa` creates its buff bar with the label
flag enabled and retains that child at `+0x170`. The anchor at `0x632fee` through
`0x63300d` attaches this same bar's bottom-right to the unit detail's
bottom-right. Its constructor supplies the bar dimensions. Inside that bar,
`0x61bbac` through `0x61bbb1` attach the label's right edge to the bar's left
edge. The recreation now preserves this chain using the admitted information
frame; no screenshot position is embedded.

The localized label key, skin font key, font height, foreground, shadow and
shadow offset are decoded from the constructor at `0x61bb4e` through
`0x61bc23`. The label uses the existing rich-text renderer and source-owned
font lifetime. Missing strings or unreadable fonts report an error. The
bounded native audit is retained under `status-label-audit/`.

Synthetic tests vary the bar dimensions, both anchor offsets, font and colors,
and reject an invalid parent/anchor or an information frame too narrow for the
binding. The actual selected-unit fixture requires one mounted localized label
and verifies its font retirement. The 2,050-test core suite passes with 80
explicit ignores, together with source-image, panel, input, GPU-resolution,
strict native/WASM and both production-build checks. The fresh headless/muted
HUD/research capture passes with frozen source hashes; its selected-unit image
was visually inspected alongside the preserved original reference. The missing
heading is now present in the lower-panel area. This verifies the integrated
label, not native glyph rasterization, long-string truncation or full pixel
parity. Buff-effect icons and their lifecycle remain separate unfinished work.

## Upper-menu pressed states

The upper Quests and Menu controls previously drew only their normal texture.
`UpperButtonBar.fdf` supplies separate `NormalTexture` and `PushedTexture`
references, atlas coordinates and `ButtonPushedTextOffset`. The renderer now
reads those records for each control, switches the existing image's source
rectangle while the enabled button is pressed, and displaces its label by the
authored offset. Release restores the normal rectangle and label position.
Both states stretch into the authored button frame. No texture rows, offsets
or dimensions are copied into production constants.

The narrow reader reports missing/malformed records, a different state atlas,
invalid UV ranges and nonfinite text-offset ratios. It does not assign a
substitute pressed image. The existing quest/menu actions, shortcuts and modal
input handling remain unchanged. The prior command-card disabled-art rule was
not generalized: the preserved original build-menu screenshot is a visual
observation, not qualification of every affordability or prerequisite state.

Synthetic FDF tests vary both texture regions and text-offset axes, exercise
press/disable/release, and reject malformed inputs and arithmetic overflow.
All six upper-control tests pass, including the existing modal/shortcut input
regressions. Command-card ownership tests, strict native/WASM checks and both
builds pass. The fresh muted, headless HUD/research test explicitly holds the
Quests button's visual Pressed state, captures it, checks that the quest log was
not activated, and requires normal-art restoration after release. Its source
hashes remain frozen. `captures-upper-pressed/upper-button-pressed.png` was
visually inspected. This is a rendering-state diagnostic, not an original-client
held-pointer comparison. Disabled-state presentation and the missing upper controls remain open.

## Upper-menu hover states

The upper controls now read each button's `UseHighlight` texture and
`HighlightText` style/key from the admitted FDF. The highlight must declare the
supported `ADD` mode and use the admitted upper-menu atlas; missing records,
unsupported blending and invalid regions are reported. The overlay samples its
authored atlas region in the existing additive UI shader, without extracting a
new texture or fitting a screenshot. Existing full-texture additive callers use
an identity UV transform. Session retirement removes the owned highlight
materials alongside their atlas.

Hover shows the authored overlay and highlighted text instead of normal text;
exit or disabling restores the normal presentation. Both text layers retain the
authored pressed offset. These visual states do not activate menu or quest
commands. Font loading includes the referenced highlight styles rather than
assuming that they use the normal font.

Synthetic tests cover nonstock highlight UVs, unsupported blend refusal, hover
entry/exit and disabling. All seven upper-control tests, the additive working-
channel GPU readback, command-card ownership checks, strict native/WASM checks,
both production builds and formatting/diff checks pass. Receipts are in
`gates-upper-hover-final/`. This qualifies the implemented source-driven states,
not original-client pixel parity or disabled-state fidelity.

The fresh headless/muted HUD/research capture also passes with frozen source
hashes (`captures-upper-hover-receipt.json`). Its visual-only hover diagnostic
keeps the quest log closed, and the existing pressed-state check still requires
normal-art restoration. `captures-upper-hover/upper-button-hover.png` was
inspected: Quests uses the highlighted text and additive atlas border while
Menu remains normal. This is not a matched original-client pointer comparison.

## Upper-menu disabled presentation

The same upper-button reader now requires `DisabledTexture` and the authored
`DisabledText` style/key. Disabled presentation takes precedence over pressing
and hovering: it selects the disabled atlas region, hides the additive hover
layer, clears the pressed label offset and shows only the disabled text layer.
Re-enabling restores the state selected by the live pointer/press components.
All three text states share one creation path, and their referenced font styles
are preflighted explicitly. This change does not introduce a gameplay rule for
when controls become disabled.

Synthetic FDF regions deliberately differ from the stock atlas. State tests
cover disabled-plus-pressed and disabled-plus-hovered cases, offset reset and
normal restoration. Source-derived colors, text, fonts and UVs remain in the
asset path. The disabled rendering diagnostic does not click or open the quest
log; its temporary interaction state is restricted to the explicit GPU test.

All seven upper-control tests, additive color GPU readback, command-card
ownership checks, strict native/WASM checks, both production builds and
formatting/diff checks pass (`gates-upper-disabled/`). The fresh muted headless
HUD/research capture passes with all tracked Rust source hashes unchanged
(`captures-upper-disabled-receipt.json`). The disabled capture was inspected:
Quests shows the authored disabled row/text and Menu remains normal. No matched
original-client disabled-condition or whole-HUD pixel-parity claim is made.

## Quest-label shadow projection

The shared upper-control/quest-log label builder read `TextStyle` but omitted its
shadow color and offset. Quest styles inherit nonzero shadow records from the
admitted `EscMenuTemplates.fdf`. The builder now uses the existing `TextShadow`
and `FrameShadow` path already used by other HUD text. Its offset scales with
the default UI camera's logical viewport and preserves the source Y-up to UI
Y-down conversion. No shadow dimensions or colors are copied into constants.
An omitted transparent shadow remains transparent under the existing style
reader's contract.

A synthetic inherited style varies shadow color/alpha and both offset axes,
while preserving foreground and inline color runs. The headless HUD diagnostic
now opens and closes the real quest log through the existing button-action
path, in addition to its visual-only upper-button state captures. This does not
alter quest state or assert original font-rasterization parity.

## Quest backdrop frame fitting

The first explicit quest-log capture exposed broken vertical borders: the
quest-specific painter used `ImageNode::Auto` for cropped border pieces. Bevy
fit the square atlas slices inside the tall assigned edge rectangles, leaving
gaps. The existing shared menu painter already uses `Stretch` for those pieces.
The quest painter now follows that same contract for non-horizontal edges and
non-tiled backgrounds. Authored corner sizes, UVs, frame geometry, tiled fill
and rotated horizontal-edge materials are unchanged.

A non-square synthetic dialog fixture exercises all eight edge pieces and an
untiled background. It requires the seven ordinary image nodes to fill their
rectangles and retains two horizontal-edge materials. The earlier
`captures-quest-shadow/quest-log.png` is preserved as the broken-border visual
reproduction, despite that capture test completing successfully.

All nine upper-control tests, the additive working-channel readback, selected
command-card ownership and asset tests, strict native/WASM checks, both builds
and formatting/diff checks pass (`gates-quest-backdrop/`). The fresh headless,
muted HUD/research capture opens and closes the real quest log and passes with
frozen Rust source hashes (`captures-quest-backdrop-receipt.json`). Its quest
image was inspected: the vertical borders are continuous and source text
shadows are present. That capture still lacked Done-button background art; the follow-up below
addresses it. Native edge filtering/repetition and font/pixel matching are not
established by the border check.

## Quest Done-button artwork

The quest painter previously created only a clickable rectangle and a Done
label. It now reads `QuestAcceptButton` with the existing glue-button parser,
preflights its normal/pushed/disabled backdrops and hover image through the same
receipt-bound skin path as the dialog, and paints them with the existing quest
backdrop renderer. Tile sizes, border slices, insets and textures remain
source-derived. The additive hover material is created once per source session
and reused across quest-log rebuilds; it and the added images retire with the
existing source-owned assets.

`ButtonText` now selects the child text frame and global-string key. The
button's pressed offset is translated into its own relative label frame. Its
normal, hover and disabled text colors come from that child's resolved style;
explicit inline color runs retain their authored colors. The Close action and
modal input handling are unchanged. This projection uses the same disabled-over-
pressed precedence as the existing glue-button renderer; it does not qualify
all native control modes or disabled-pushed template semantics.

A nonstock state regression checks art visibility, both offset axes, inherited
text colors, explicit color preservation and release restoration. Earlier
fixture failures are preserved: ECS query order did not represent child order,
and the follow-up needed the Bevy 0.20 owned-Entity iterator form. Neither was a
production rendering failure.

The first actual state capture reached the rendered button but held its entity
across an authored quest-discovery update. The dialog correctly rebuilt, so the
fixture failed on the retired entity. Its visual diagnostic now follows the
current named button every frame and obtains the current entity for Close. The
failed capture and receipt remain local; this correction does not pause or
change quest execution.

Final checks pass in `gates-quest-done-current/`: ten upper-control tests,
additive working-channel GPU readback, command-card ownership and asset checks,
strict native/WASM checks, both production builds and formatting/diff checks.
The fresh headless/muted HUD/research capture passes with frozen Rust source
hashes (`captures-quest-done-current-receipt.json`). Normal, hovered and pressed
Done captures were inspected; the current Done button closes the real dialog.
This verifies the authored-state projection and action path, not a matched
original-client pointer capture or whole-HUD pixel equality.

## Quest details backdrop

The authored `QuestDisplayBackdrop` has a separate tiled edit-box background
and eight-piece border, but the quest painter previously created only its
scrolling content node. That source backdrop is now preflighted with the dialog
and button artwork, retained under the same path/sampler cache, and painted by
the existing backdrop helper in the source-defined details rectangle. Its
images and materials use the existing session/rebuild ownership paths.

The actual HUD fixture requires the details pane's ordinary border images and
tiled-background material to be mounted. The existing flat item/status flow
remains a documented shortcut; this change does not invent native item-list or
scrollbar geometry to replace it.

The first pane-art capture made a second omission visible: the details title
was in that scrolling flow, overlapping the pane border. It now has its own
source-FDF rectangle under the dialog root, honoring the authored anchor above
the pane. Its text height is measured from the admitted font-height record by
the same existing path as the main/optional headings; native glyph metrics and
font rasterization remain separate fidelity questions. The title is no longer
clipped by the scrolling content container.

Final focused/integration checks, strict native/WASM checks, both builds and
formatting/diff checks pass (`gates-quest-details-anchor/`). The fresh muted
headless HUD/research capture passes with frozen Rust source hashes
(`captures-quest-details-anchor-receipt.json`). The populated quest view was
inspected: the details artwork is present and its title clears the upper border.
The status/item flow still starts at that border and needs its own source-bound
layout; scrollbar behavior, edge filtering and native glyph/pixel parity remain
open. The earlier unanchored capture is preserved as the visual reproduction.

## Quest item and description frames

Quest status/items now mount in the authored `QuestItemListContainer`; the
quest description mounts separately in `QuestDisplay`. Both use their FDF
anchor chains instead of sharing the backdrop's flat flow. Item labels use
`QuestItemListItemTitle` and its admitted font/style. The description reads
`TextAreaInset`, `TextAreaLineHeight` and `TextAreaLineGap`; the line metrics are
converted to a ratio of the source font height so display scaling remains
relative. Missing, nonfinite, negative and degenerate metrics are refused.

Clipped quest containers use the installed Bevy scroll-area widget. A narrow
quest-only router forwards child-targeted pointer-scroll events to the nearest
marked area, preserving the input delta; direct area events are untouched.
Synthetic tests check both targets, small deltas, clamping and the absence of
double scrolling. The first isolated child-target fixture and the observer-
initialization attempt are preserved as failures; the explicit routing case
passes. The actual HUD fixture requires scroll-area components on the mounted
item and description frames.

This is source-frame projection with a reconstruction input mapping, not a
qualification of native wheel step size or scrollbar visuals. Native row
factory overrides, completion styling, status-row binding, scrollbar controls
and text-area line-count behavior remain open. In particular, constructor
references to completed/failed strings alone do not qualify removal of the
existing status row, so this change retains it rather than inferring behavior.

## Verification and open scope

Local evidence is retained under `.scratch/ui-fidelity-20261027/`. Fresh muted,
offscreen Vulkan captures show the stat panel, black console gaps and separate
Armor/hero text. The broader capture test subsequently refuses its existing
visible-ground-item precondition in this chapter; it is not a full GPU suite
pass. A separate explicit `mounted_campaign_hud_and_research_states` test reuses
the same menu/control/selection path, checks the panel and command-card errors,
opens the actual research menu, compares every rendered disabled state with
canonical learnability, checks every displayed rank, requires the idle selected
hero's Stop button alone to own the additive highlight, and returns through the
source cancel button. That test passes on a fresh isolated profile. The broader test retains its item check.

Source-layout, panel and resolution tests pass, along with the 2,050-test core
suite, strict native/WASM checks and both production builds. Eighty core tests
remain explicitly ignored. The UI task is still open: hero
experience-bar pixel matching, time-of-day and minimap controls, buff-effect icons, other command visual
states and a matched original-client pixel comparison remain incomplete.
