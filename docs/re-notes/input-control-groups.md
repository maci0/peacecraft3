# Input state and control groups

Evidence and decisions for the modifier/gate foundation and the control groups
(slices 2 and 3 of the [input behaviour audit](input-behaviour-audit.md)).

## Method and identity

- Executable: the selected Legacy `Warcraft III.exe`, SHA-256
  `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
  (same image as the audit).
- Data: `Units\CommandStrings.txt` (`[Errors]`) and
  `UI\FrameDef\UI\EscMenuOptionsPanel.fdf`, read through the archive chain
  `War3Local`, `War3xLocal`, `War3x`, `War3`.
- Code: rizin `aa; aap; aac` with the `pdg` decompiler and `pd` listings. Names are
  rizin's `fcn.<address>`; roles are assigned here. Decompiler output stayed in the
  ignored `.scratch/`. Nothing from the client is copied into the repository except
  addresses and short facts.
- Static analysis only. No original client was run, so no row is a runtime
  observation. The [live capture](input-live-capture.md) (runtime observations under
  Wine, run3) settles the rows marked RO below.

Evidence tags as in the audit: SB shipped data, SS string in the executable, EX
executable code read here, DOC widely documented behaviour not verified here, INF
inference, UNK not established.

## Control groups (EX unless marked)

The key handler is `fcn.005fc700` (a `CSelectMode` event; `this+0xc` and `this+0x10`
hold the double-press memory). The event carries the virtual key at `+0x10` and a
modifier mask at `+0x14` (bit 1 Ctrl, bit 2 Alt).

| # | Fact | Evidence |
| --- | --- | --- |
| G1 | Virtual keys `0x30`..`0x39` (top-row digits) reach the group code through the key switch at `0x5fc77a`; the case bodies store the group: `1`..`9` give 0..8, `0` gives 9. Keypad digits are not cases of this switch. | EX `0x5fc75f`, `0x5fc768`, `0x5fc773`, `0x5fc781`..`0x5fc7d2` |
| G2 | Ctrl (mask bit 1) selects the define branch before Shift is read. Ctrl+Shift is therefore define. Alt is not read on the digit path. | EX `0x5fc752`..`0x5fc765`, `0x5fc944` |
| G3 | Without Ctrl, Shift (read through `fcn.0083fea0`, either Shift key) selects the extend branch; without Shift the digit selects the group. | EX `0x5fc944`, `0x5fcbe7` |
| G4 | Define: the current selection is read (`fcn.004f5bf0`). Each unit that is non-null, valid and passes the ownership test (`vtable+0xec` with the local player, then the player-set bit test `fcn.0047f9f0`) is kept; a unit that fails the test sets a refused flag. A kept list replaces the group: `fcn.004f4fb0` rewrites the local mirror (stops after 12 entries) and `fcn.006cc8e0` sends the define net command (command type `0xa0017`, `min(count, 12)` unit handles). | EX `0x5fc87b`..`0x5fc93c`, `0x4f4fb0`, `0x6cc8e0` |
| G5 | Define refusals: selection list empty gives error code `5 + (fcn.004f21f0 != 0)`, where `fcn.004f21f0` reads a separate non-unit object slot (`+0x1e4`) of the selection object, so 5 or 6. Kept list empty gives `5 - refused`, so 4 when a unit failed the ownership test and 5 otherwise. | EX `0x5fc88c`..`0x5fc89a`, `0x5fc911`..`0x5fc91c` |
| G6 | Extend (Shift): the same empty-selection refusal (5 or 6) comes first, even when the group has members. The merged list is the group's members that pass the dead/ownership checks, then the selection's units that pass the selectable/ownership checks, without duplicates (pointer compare). Refused flag and empty-merge code are `5 - refused` as in define. A non-empty merge is written through the same two calls as define, so the 12 cap cuts the tail: group members first. | EX `0x5fc944`..`0x5fcbc0` |
| G7 | Select (plain digit): `fcn.00600530(group)` returns 0 with no message when the group is empty or holds no valid unit; it then leaves the selection and the double-press memory alone. Valid units replace the selection (net select mode 3), at most 12. Members that fail the checks are not selected and are passed to `fcn.004f4860`. Shift-add mode exists in this helper (cap `12 - selection count`) but the digit handler never reaches it with Shift held. | EX `0x600530`, `0x6cbf40`, `0x5fcbe7`..`0x5fcbf5` |
| G8 | Double press: after a successful select, `fcn.00796d00` reads the time (`GetTickCount` times a float `0.001`, as `f32` seconds, wall clock). If the digit differs from `this+0xc`, or `this+0x10 + window <= now` (SSE `comiss`, so an unordered compare also updates), the digit and time are stored. Otherwise the camera is centred on the selection object's primary unit (`+0x1e0`, via `fcn.004f2070` and `fcn.005eae20`) and `this+0xc` is set to `-1`. | EX `0x5fcbfb`..`0x5fcc7e`, `0x796d00` |
| G9 | The window is the `f32` at `0xe6957c`, the operand of the `addss` at `0x5fcc40`: `0.5`. | EX `0x5fcc40`; value read from the image |
| G10 | Error codes 4, 5, 6 map to the `[Errors]` keys `Invalidcontrolgroup`, `Emptycontrolgroup`, `Nonunitincontrolgroup` through the name switch of `fcn.005f0840`. `fcn.005ecc20` posts the name's string. Codes 4 to 6 have no named sound in its sound switch and end in `fcn.005d8a20`, whose effect was not read. | EX `0x5f0840`, `0x5ecc20` |
| G11 | The three strings exist in `[Errors]` of `Units\CommandStrings.txt`, one value each. | SB |
| G12 | Preconditions in the handler: the game UI object's `+0x1bc` must be non-zero, and `fcn.005ad790` (`+0x898 == 1` on a UI object) must be zero. What these two states are is not established. The local player's selection object (`player+0x34`) must exist or the handler reports `0x57` and returns. | EX `0x5fc732`, `0x5fc7d9`, `0x5fc7f4`..`0x5fc80e` |
| G13 | Units that are loaded into another object (flag bit at `+0x164`) select their container in `fcn.00600530`. | EX `0x600530`, role INF |

### Decisions and inferences

| # | Row | Status |
| --- | --- | --- |
| D1 | Shift+digit adds the selection to the group (G6); the current selection does not change. The branch builds the union and redefines the group, and the live capture saw the group gain the selection. Isolated in `campaign_control_groups::shift_digit`. | EX; RO |
| D2 | Ctrl+Shift+digit defines (G2); the live capture saw it replace a non-empty group. | EX; RO |
| D3 | The primary unit of a selection (G8) is the first selected unit of the recreation's selection. The client's field name and update rule are not read. | INF |
| D4 | The wall clock is `Time<Real>` seconds as `f32`; the compare is done in `f32` as the client does. Key auto-repeat is not distinguished: Bevy delivers one press, and the client's treatment of repeated key messages in this handler is not read. | INF |
| D5 | Dead or removed units leave the group when it is selected, extended or defined (a unit that fails the validity checks of G7 is not selected). Whether the client removes them from its stored group at that moment or elsewhere is not established; the effect on what a digit selects is the same. | INF |
| D6 | A hidden (not shown) unit the player controls is skipped by a select and kept in the group. | INF |
| D7 | The refusal text is posted to the error line (`campaign_error_line::ErrorLine`) and drawn above the console with the client's colour and timers ([chat and overlays](input-chat-overlays.md), section 1). Only a newer post or the timers remove it; an accepted key does not. | EX `0x5ecc20`, `0x5ed6c0` |
| D8 | The camera is centred with the same instant, bounds-clamped scroll the minimap uses. The client's pan call is `fcn.005e3e20`; its easing was not read. | UNK |
| D9 | Groups are kept per local player number and are dropped with the script session. Saving across missions was not examined. | recreation |
| D10 | Not modelled: G12 preconditions, G13 containers, transports, and the net-command synchronisation. | UNK |
| D11 | A group selection is logged as the same `Select` command a click logs. Defining a group is local interface state and is not logged. | recreation |

## Input state

Evidence for the gate and modifier reads is in the audit (pipeline table and rows
A3, A4, B1, C1, C5). Decisions:

- `peacecraft3::game_input::GameInput` holds Shift, Ctrl and Alt as left/right
  pairs refreshed in `PreUpdate` from `ButtonInput<KeyCode>`. The client reads a
  pair as one modifier (`fcn.0083fea0`, `fcn.0083fdc0`, `fcn.0083fca0`).
- The gate is the set of reasons input is closed: no view, no session, escape menu,
  quest log, script dialog, scripts disabled control, local player left. Input is
  open when no reason remains. The camera and session reasons are set by
  `campaign_camera::session_closers` and the interface reasons by
  `campaign_game_input::sync_gate`. The interface reasons are set once before the
  camera chain, from the previous frame, and again after the HUD and dialog sets
  for click and hotkey handling.
- Click selection and drag-box gestures, command-card hotkeys, arrow-key scroll, control groups and the
  Ctrl+Shift+F9 pacing chord read `GameInput`. `commands_available` (used by the
  command card and minimap buttons) builds the same gate from the same function.
- Not on the gate: Escape. The client raises the end-cinematic event on every
  Escape, including while scripts disabled control, so it keeps its own checks.
- Arrow-key scrolling is now closed by the quest log and script dialogs as well as
  the menu and disabled control. The client's camera lock fields (audit C5) are
  unidentified, so this is a recreation choice.
- The command card now ignores a hotkey pressed with Ctrl or Alt held, as the
  client's dispatch does (audit B1, `0x5fcd0a`).
- Option table: `mousescroll`, `keyscroll` and `mousescrolldisable` fallbacks are
  decoded from the client's preference table by `pcx_play::options_defaults` and
  resolved once per session into `GameInput::scroll_options`, with the client
  identity recorded. Saved values override them through
  `ScrollOptions::with_saved`. **No saved value flows today:** the settings store
  and `menu_settings` bind only the audio controls (`validate_settings` rejects
  other names), so the Gameplay sliders (`MouseScrollSlider`, `KeyScrollSlider`,
  `MouseScrollDisable` in `EscMenuOptionsPanel.fdf`, range 0 to 100) do not exist
  in the recreation. The reader of the options panel's applied values is in place
  and returns nothing until those controls are bound. The camera does not apply the
  values; that and edge scroll are later slices.

## Implementation

| Part | Where |
| --- | --- |
| Client constants (digit binding, cap, window, error keys) | `crates/pcx-play/src/control_group_rules.rs` |
| Group state (define, extend, recall, press timer, typed refusals) | `crates/pcx-play/src/control_groups.rs` |
| Input state resource, modifiers, gate, option values | `crates/peacecraft3/src/game_input.rs` |
| Key system, per-player state, `ControlGroupSelected` and `ControlGroupCentred` messages | `crates/peacecraft3/src/campaign_control_groups.rs` |
| Interface gate, option and string loading, command-log record | `crates/peacecraft3/src/campaign_game_input.rs` |

The rules reader checks the image hash, then decodes operands and the tables they
index at the reviewed addresses; a mismatch refuses with the reason and nothing is
substituted. The refusal texts are read from `[Errors]` by the key names the image
selects.

## Tests

- `cargo test -p pcx-play --lib control_group`: decoder against a synthetic address
  space (other digit binding, cap, window, key names; disagreeing sites, repeated
  groups and unusable windows refuse) and the state machine against a fake roster.
- `cargo test -p peacecraft3 --features test-fixtures --test map_loading control_group`:
  the key system on a synthetic published session with injected `ButtonInput` keys
  and a synthetic rules binding that differs from the keyboard order.
- `cargo test -p peacecraft3 --features test-fixtures --bin peacecraft3 campaign_game_input`
  and `--lib game_input`: refusal-text lookup, saved-value conversion, modifier
  and gate behaviour.
- Against the real client (explicit requirement, ignored by default):
  `PCX_WC3_INSTALL=<install> cargo test -p pcx-play --lib reviewed_client_yields -- --ignored`.
  It decoded the binding `[9, 0, 1, 2, 3, 4, 5, 6, 7, 8]` for digits `0`..`9`, cap 12,
  window 0.5 and the three key names.

All tests are headless: keys are written into `ButtonInput`, no window, pointer or
audio device is opened.

## Not established

- Live observations of the empty-selection and foreign-unit refusals (G5, G6).
- The meaning of the G12 preconditions and of the selection object's `+0x1e0` and
  `+0x1e4` slots beyond their use here.
- Whether the client prunes dead members from the stored group on select (D5).
- Camera pan easing for the double press (D8) and key-repeat handling (D4).
- Behaviour with hidden units (D6) and with units inside transports (G13).
- Where saved Gameplay option values will be stored.
