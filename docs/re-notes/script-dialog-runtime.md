# Executed JASS script dialogs

The opt-in `ObjectHost::with_dialogs` admits the selected declarations for
`DialogCreate`, `DialogDestroy`, `DialogClear`, `DialogSetMessage`,
`DialogAddButton`, `DialogAddQuitButton`, `DialogDisplay`, both dialog-event
registration natives and click getters. Missing declarations remain unsupported;
present signatures, handle ancestry and source identity are checked. Caller
budgets bound live dialogs/buttons, aggregate text, registrations and pending quit
requests. These are application resource policy, not original game limits.

Dialogs/buttons use the existing session-owned arena. Destruction invalidates the
dialog and its buttons; clearing invalidates its old buttons and unassigns its
caption. Empty assigned text remains distinct from absent text. Failed arguments
and capacity checks preserve the published records. Display requires an admitted
local player, ignores another valid player's request and records explicit local
show/hide calls. Clicking requires that player's displayed live dialog/button.
The click hides that local dialog, queues matching dialog/button registrations in
arena order, and supplies `GetClickedDialog`, `GetClickedButton`,
`GetTriggerPlayer` and the selected `dialogevent` to the existing event-delivery
thread. Existing local-player and combat player-unit event producers also retain
their explicitly registered player for `GetTriggerPlayer`; ordinary events with
no established player context remain unsupported rather than inventing a player.
The local-player producer retains its typed `playerevent` as the event response,
not the event-registration handle. Its timer/trigger context cleanup owns response lifetime. Registration
IDs come from selected `EVENT_DIALOG_CLICK` / `EVENT_DIALOG_BUTTON_CLICK`
conversion initializers, never copied numeric constants.

A quit button retains the executed `doScoreScreen` flag. Activation emits a typed
application quit request; the gameplay Host does not invent a result screen or
choose a campaign outcome. Clearing/destroying a dialog leaves historical event
identities in the bounded arena, but stale buttons cannot be clicked. Callback
queueing at the next script boundary and the most recently shown modal being on
top are recreation scheduling choices, not proven native nesting internals.

## UI consumer

`campaign_script_dialogs` resolves `ScriptDialog.fdf`, `EscMenuTemplates.fdf`,
`war3skins.txt`, fonts and image files through the exact published binding reader.
It retains receipts, validates font faces and bounds source/decode allocations.
The panel's minimum dimensions, title anchor/font/style, button dimensions,
background insets/tile sizes, border art and pressed-label offsets come from
those files. Image identities include tiling mode. A refused candidate leaves an
explicit consumer error; a partially spawned tree and its materials are removed.
Session replacement releases its retained images/fonts/materials and controls.

The modal centers the source-width panel and uses `dialog_layout::Rules` for
native dynamic height, rounding, title position, first-button position and row
step. The rules qualify the exact selected executable image using the existing
reviewed image digest, decode checked instruction/data windows and retain source
identity plus image SHA-256. Numeric UI operands are read from that client, never
copied into production constants. Synthetic fixtures mutate the source operands,
including independent height/row gaps and minimum sizes. Centering remains a
recreation context choice. Caption height comes from Bevy's actual shaped rich-text
layout at the source panel width minus its authored background insets. The
physical extent is converted through the caption's actual UI target viewport,
then fed into the decoded native layout relations. An initial hidden candidate
allows layout without showing an estimated one-line panel. Gameplay suppression
remains active while measurement is pending; dialog activation waits for the
measured panel. Nonempty captions containing only parsed colour/reset markup
remain present captions with zero text extent: they use the decoded caption
relations immediately rather than waiting for glyph layout. Empty/absent raw
captions retain the absent-caption relations. Negative/nonfinite extents still
refuse. Session witness, root identity and dialog revision reject stale
probes. The measurement cache includes effective content width, viewport and
scale, so text changes and resized targets reflow without rebuilding unchanged
geometry. This recreation shaping pipeline is not evidence of identical native
font rasterization. Larger-than-viewport dialogs, hover artwork, async dialogs
and full native game-over behavior remain open. Source normal/pushed artwork and authored
pressed text offsets are consumed. Caption/button content and hotkey codes come
only from executed script state.

The application validates quit-request session witnesses and local-player identity
before leaving the current session. A requested score screen remains an explicit
unsupported presentation, not a fabricated scoreboard.

The consumer uses checked widget activation/primary release events and logical
character hotkeys (plus Enter/Escape), preserves source hotkey codes and keeps
same-frame gameplay suppression after a modal closes. It emits typed quit
requests for the application. Modal controls remain separate from gameplay user
control, allowing scripted prompts while world input is disabled. The modal's
composition layer sits above the gameplay-control picking barrier and below the
existing cinematic filter layer. These are application ordering choices, not
source asset values. The control barrier still intercepts ordinary HUD input;
its former maximum layer intercepted modal mouse events as well. A real
`UiPickingPlugin` regression exercises cursor movement, primary down/up,
pressed-state lifetime and completed modal clicks with that barrier present,
while checking ordinary HUD input stays blocked.

## Evidence and verification

The selected target executable is
`Warcraft III.exe`, PE VERSIONINFO `1.29.2.9231`, private SHA-256 beginning
`3f2ed012`. Its real native registrations bind `DialogClear` to `0x48f7e0`,
`DialogDisplay` to `0x48f8c0`, `DialogSetMessage` to `0x48f940`, and the normal/quit
button native signatures agree with the selected declarations. `DialogDisplay`
compares the resolved player with the local client before its UI call.
`DialogClear` reaches `0x50e580`, clears UI buttons and the caption through
`0x62bb40`, then clears script button identities. The real `GetTriggerPlayer` registration reaches `0x496e00`; its context getter
at `0x4ebc20` returns zero on the empty-stack branch at `0x4ebc59`, supporting
null outside an active event. These are bounded static native observations, not
an original-game runtime test.

Dynamic native sizing/placement uses `0x649b60` and `0x629150`; their operand
records/literals are separate from the supplied FDF records. The dynamic layout
reader follows those source operands and arithmetic, including the `fprem`
rounding path, while button/font extents remain source-control/text inputs.
The image qualifier prevents applying these reviewed instruction bindings to an
unrecognized release. Browser inputs use the same selected-client image admission
path as other client rules; no original bytes are bundled with the renderer. The initial REA request refused with a snapshot integrity mismatch; the
bounded fallback uses the selected local PE sections and Capstone. Source identity,
registration reads and disassembly windows stay under
`.scratch/script-dialog-host/`; no original bytes or generated extracts are tracked.

Synthetic tests cover actual VM execution, local filtering, lifetime and stale
handles, text/button budgets, preserved refused updates, quit flags, both
registration kinds and event-response cleanup with deliberately changed symbolic
IDs. UI checks cover source-derived layout mutations, hotkeys and same-frame
input suppression and stale session witnesses. A focused shaping check uses
Bevy's real text pipeline with synthetic content to exercise word wrapping,
explicit line breaks, rich-text spans and doubled target scale; unchanged
measurements converge. A separate ignored gate requires explicit local original
FDFs.
These do not prove rendered or native parity.

A local full-application native fixture invoked the real Host/consumer with
synthetic caption/button text during Human01. The source panel became visible
after actual text layout, primary-pointer Continue hid it, and primary-pointer
Quit cleared the owned session. Earlier keyboard success with mouse failure
identified the campaign control barrier above modal controls; the revised layer
relation fixes that path while preserving the filter's visual position. Captures
and logs remain under `.scratch/campaign-objective-20261020/gui-v15/`. This proves
local visible/pointer behavior for the fixture, not original-game visual parity
or an authored Human01 prompt.
