# HUD tooltip text

`rich_text.rs` parses bounded caller text for `tooltip::hud_markup_adapter` and
for the installed tooltip consumer. Menu access-key parsing is unchanged.
The HUD adapter receives text only after the HUD planner's input checks.
The tooltip consumer checks its caller's aggregate input limit before parsing
or cloning the planned style and text.

The import profile interprets `|c` / `|C` followed by eight hexadecimal digits
as encoded ARGB, `|r` / `|R` as return to the supplied base colour, and `|n` /
`|N` as a newline. Closed `^^…^^` tags remain literal and produce notes;
substitution is unsupported. Unknown bar forms, including `||`, and incomplete
codes refuse the string atomically. These are checked import restrictions,
not proof that the original client rejects the same strings.

`TooltipSupply` names one field and one `TooltipRecord`. Its source label,
member, frame, receipt path and binding revision must match exactly one living
`HudSourceAttachment`. `TooltipOwner` is only a marker; it does not copy that
attachment or carry text. Selection uses the existing `Hovered` component or
visible `InputFocus`. Click actions do not select a tooltip.

The caller supplies an existing font handle, the `PlannedText` resolved by the
HUD planner, an explicit relative box, and the input limit. The shared HUD
renderer preserves the planned face, size, base colour, justification, wrapping,
padding and shadow. Tooltip colour runs replace only the planned label runs.
Missing fonts, invalid sizes, non-finite style values, negative extents, missing
placement and invalid markup hide the view and record a contextual fault.
There is no fallback font, rectangle or field precedence.

The view belongs to the matched frame's mounted HUD root, so percentages and
camera targeting use that surface. Refresh runs before UI preparation. Replacing
or retiring the HUD removes its tooltip tree. A changed owner or supplied
resource rebuilds the runs; unchanged accepted or refused text is not parsed
again. A retired font hides an otherwise cached view.

Tests cover colour/reset/newline/Unicode parsing, unchanged menu hotkeys, the
accepted HUD mount, hover and keyboard selection, source replacement, invalid
styles and limits, font retirement, shared text styling and stable span entities.
The separate Vulkan readback uses synthetic FDF and the test font. It checks
that differently coloured lines reach the owner's UI render target; it is not
an original-client tooltip comparison.

No application producer supplies `TooltipSupply`, chooses between `Tip` and
`Ubertip`, or attaches real HUD picking/focus inputs yet. Installed HUD frames
remain non-pickable. Tooltip chrome, native placement, wrapping/rasterization
parity, code substitution and automatic selected-unit field publication remain
open. An installed consumer alone does not display a gameplay tooltip.

An explicit local source gate also parses 62 tip-named values from a receipted
GlobalStrings member on native and WASI. That receipt does not establish an
edition or engine version. The source strings and hashes stay in private local
evidence; no game assets are included in the repository. This gate establishes
syntax coverage, not field precedence, substitution or rendered parity.
