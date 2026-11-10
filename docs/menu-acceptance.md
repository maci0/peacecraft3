# Menu acceptance

All menus require both complete observable behavior and pixel-perfect presentation.
A working route, parsed asset, passing widget test or recreation-to-recreation
screenshot match does not establish original-client pixel parity. Campaign
progression follows menu completion; it does not replace unfinished menu work.

## Current coverage

This inventory follows the current `Screen` and escape-menu page types and their
production action consumers. It is not a claim that the recreation exposes every
screen authored by the original. Missing screen discovery remains open.

| Area | Functional evidence or known gap | Original full-frame pixel gate |
| --- | --- | --- |
| Main | Single Player, Options, LAN, edition switching, Credits and Quit have consumers; Credits launches the edition's credits map named in `UI\MiscUI.txt`; Battle.net and realm flows remain incomplete | Open |
| Single Player | Campaign and other submenu routes exist; profile and terminal actions remain incomplete | Open |
| Campaign selection | Selected original chapters launch; complete unlock/progression behavior remains open | Open |
| Custom Campaign | Route exists; complete campaign import/selection/launch remains open | Open |
| Load Saved Game | Route exists; complete save discovery and restore remain open | Open |
| View Replay | Route exists; complete replay selection and playback remain open | Open |
| Custom Game | Route exists; bounded map discovery and admission exist in `custom_game.rs` but the screen does not list or launch maps yet | Open |
| LAN | Route exists; complete discovery/join/create workflows remain open | Open |
| Options landing and category navigation | Existing production routes and transitions; native timing/opacity acceptance remains open | Open |
| Gameplay options | Scroll controls, Enhanced Tooltips and Custom Keys have applied/persisted consumers; other controls remain incomplete | Open |
| Video options | Client-size request/confirmation exists; Texture Quality stays a read-only preview until its 16-bit storage is established; remaining quality and display behavior remains incomplete | Open |
| Sound options | Music/effects enable and volume and Unit Responses have applied/persisted consumers; remaining category/provider behavior remains incomplete | Open |
| In-game Main menu | Return, End Game, Help and Tips work (Tips: Next/Back/OK with retained selection); Options is disabled because the in-game `UI\FrameDef\UI\EscMenuOptionsPanel.fdf` panel is not rendered (its escape-menu route reached the front-end Options screen, which stays hidden during play); Save and Load remain incomplete; Pause is hidden in the single-player modal | Open |
| In-game End Game | Back, restart, quit mission and exit route have consumers | Open |
| In-game exit confirmation | Confirm/cancel have consumers; native/browser exit behaviors are distinct | Open |
| Other original dialogs, popups and screens | Enumerate from selected original inputs and runtime observations; absence from current types is unfinished scope | Open |

The selected edition/version and source identity must accompany each acceptance
record. Read authored geometry, colors, fonts, textures, durations and defaults
from that source. Unavailable source content is a reported gap, never a guessed
replacement. Keep all original assets and generated copies local.

## Required evidence for each screen

- Exercise every authored control and its resulting state through production
  input: pointer, keyboard focus, shortcuts and text editing where applicable.
- Verify normal, hover, pressed, disabled, focused, selected and expanded states
  that the source actually supports, including nested dialogs and scrolling.
- Verify Apply, Cancel, persistence, reload, refusal and the actual runtime effect
  where applicable. A stored setting without its consumer is incomplete.
- Capture the entire original and recreation frame at matching dimensions,
  edition, state and animation tick. Retain tick evidence; differing wall-clock
  timestamps alone do not synchronize animation.
- Produce side-by-side, overlay and exact pixel-difference artifacts. Record
  differing pixels and bounds over the full frame. Crops help diagnose a failure;
  they cannot substitute for the full-frame gate. Zero differing pixels is the
  pixel-perfect acceptance condition for a qualified identical state.
- Repeat applicable resolution/aspect-ratio and native/browser workflows. Keep
  unsynchronized or otherwise unqualified comparisons explicitly unverified.

Run all automated captures offscreen and muted. Existing evidence and unresolved
transition questions are linked from [menu navigation](re-notes/menu-panel-navigation.md),
[camera options](re-notes/input-camera.md),
[tooltip options](re-notes/held-inventory-presentation.md) and
[in-game menus](re-notes/campaign-escape-menu.md) and
[Help/Tips bindings](re-notes/escape-help-tips.md).
