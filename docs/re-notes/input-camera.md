# Camera input: zoom, rotation, scrolling and jump keys

Slices 4, 8 and the camera part of 10 of the [input behaviour audit](input-behaviour-audit.md),
built on the [live capture](input-live-capture.md). Every executable number the camera
uses is read at runtime from the selected client image; nothing below is a source
literal except where a row says so.

## Evidence

Same tags as the audit (SB shipped file bytes, EX decompiled executable code, RO and
RO-dbg runtime observations, INF inference, UNK not established). Image: Legacy
`Warcraft III.exe`, SHA-256 `3f2ed012...0eed`, 1.29.2.9231. Code: rizin `af` and the
`pdg` decompiler on the functions named, run for this note. Nothing from the image is
copied into the repository.

### Zoom and rotation

| Fact | Evidence |
| --- | --- |
| The camera setter `0x5de2c0(duration, flag)` fills a stack frame from two XMM table copies per field plus immediates, then indexes it: distance `[ebp+level*4-0x28]`, angle of attack `[ebp+level*4-0x40]`, rotation `[ebp+index*4-0x88]` with `index = level + 6 * state`. `level` is `[this+0x20]`, `state` `[this+0x24]`; both are range-checked (`cmp edx,6`, `cmp ecx,0x12`) and fall back to 0. | EX `0x5de2c0` |
| Distance 1650, 1600, 1500, 1400, 1275, 1200; angle of attack 304, 311, 318, 325, 332, 335; rotation 90 x6, 20 x6, 160 x6. The last level differs from the `[Camera]` file (1100 and 339); the live camera settles at the executable values. | EX table words; RO-dbg ([live capture](input-live-capture.md)) |
| FarZ 5000, field of view 70 degrees, roll 0 and height 0 are immediates in the same function, not table entries. The audit found the file values equal for field of view and FarZ. | EX `0x5de4eb`, `0x5de6fd`, `0x5de826`, `0x5dea09` |
| Each field moves at `(target - current) / duration`, so every ease is linear in time. Rotation, angle of attack and field of view pass through angle helpers (`0x5dfd00`, `0x8a19b0`) that were not decoded. No table pair differs by more than 180 degrees, so a plain linear ramp gives the same path. | EX `0x5de43f`, `0x5de5fb`; helpers UNK |
| `0x5e1090(level)` clamps to 0..5 (`mov eax,5`) and, only when the level changes, calls the setter with `1.0` (`mov dword [esp],0x3f800000`). | EX `0x5e1090` |
| Page Up (VK `0x21`) requests level + 1 on every key-down message including repeats; Page Down (`0x22`) requests level - 1 when the level is not 0. | EX `0x5e12f0` |
| Wheel event `0x400500cd`: delta divided by 120 (multiply by `0x88888889`, shift 6), truncating toward zero. A negative quotient with level > 0 requests level - 1, a quotient of 1 or more requests level + 1. The size of the quotient does not matter, so a 360-unit delta is one level. | EX `0x5e12f0` |
| Insert (`0x2d`) and Delete (`0x2e`): ignored while any state is held; otherwise state := 1 or 2 and the setter runs with `0.5`. Key-up of the same key returns to 0 with `0.5`. Numpad 0 (`0x60`) key-up clears state 1 and Numpad Decimal (`0x6e`) key-up clears state 2. | EX `0x5e12f0` (`0x5e1734`, `0x5e1740`) |
| The wheel, Page and Insert/Delete cases return early on `[this+0x5c]` and on `0x5e0ba0()`; their meaning is unknown. | EX; UNK |

### Scrolling

| Fact | Evidence |
| --- | --- |
| Arrow keys (VK `0x25` to `0x28`) call `0x5e07a0` and `0x5e52f0` with the key multiplier `[this+0x4f0]`. Edge events (types 1, 2, 4, 5, 6, 8, 9, 10 are direction masks, `+0x10` is the begin flag) call the same helpers through `0x5dfd50` with the mouse multiplier `[this+0x4ec]`. Edge begin events return early when `[this+0x30]` (`mousescrolldisable`) is nonzero. | EX `0x5e12f0`, `0x5dfd50` |
| A helper builds a velocity of `multiplier * 3000.0 * sign` along yaw minus pi/2 (lateral) or yaw (forward), stored in a per-direction slot (`+0x444 + dir*4`, `+0x44c + dir*4`). Key and edge motion use the same slot, so one direction holds one velocity. | EX `0x5e07a0`, `0x5e52f0` |
| Constructor `0x5dced0` reads option 0x2a (`keyscroll`) into `+0x4f0`, 0x28 (`mousescroll`) into `+0x4ec` and 0x29 (`mousescrolldisable`) into `+0x30`. A value of 100 or less becomes `value / 100.0`; if that is within 0..1 the multiplier is that plus `0.5`. Anything else leaves the initial `1.0`. | EX `0x5dd914` to `0x5dda16`, initial stores `0x5dd743`, `0x5dd74d` |
| The three option indices resolve to the names `keyscroll`, `mousescroll`, `mousescrolldisable` in the descriptor table, with fallbacks 50, 50 and 0. | EX plus [original options defaults](original-options-defaults.md); verified at load |
| Edge scrolling: margin 0.004 of the 0.8 x 0.6 UI space per axis, strict; corners scroll both axes; no delay; speed equal to arrow keys. | RO ([live capture](input-live-capture.md)) |
| The producer of the edge events and the margin constant were not found. The only `0.004f` immediates (`0x3b83126f`) sit in unrelated constructors (selection mode, XP bar border) and a constant table at `0xe9aa5c`; searches for 0.796 and 0.596 found nothing. | EX search; UNK |
| Middle button: a press with button code 2 enters `CDragScrollMode`, which stores the camera target (`+0x6c` to `+0x74`) and the screen point (`+0x78`). The per-frame update was not located. | EX `0x63bd00`, `0x5f9ed0` |

### Jump keys

| Fact | Evidence |
| --- | --- |
| Space (VK `0x20`, key-down incl. repeats) runs `0x5e1930`: the history is an array of (x, y, z) at `[this+0xac]` with count `+0xa4`, newest index `+0xa0`, current index `+0x98` and last press `+0x9c`. A press before `last + 1.0` steps to `(current - 1 + count) % count`, otherwise to the newest entry; either way the camera jumps there and `last := now`. Disabled when `[gameui+0x298]` is set (UNK). | EX `0x5e1930` |
| Backspace (VK `8`, key-down incl. repeats) runs `0x5e1120` over a sorted object list (`+0xc0`, count `+0xbc`): at or after `last + 1.0` it takes the first valid entry, otherwise it scans from `(index + 1) % count`. A valid entry is an object of class tag `0x2b61676c` with `+0x20 == 0` and flag `0x100` clear in the entry. `last` is updated only when an entry is found. | EX `0x5e1120` |
| The comparer `0x5e4cb0` puts valid entries first, then orders by a virtual key (`+0x130`) and falls back to the id difference; what the key is was not decoded. | EX `0x5e4cb0`; key UNK |
| Both windows load the same float `1.0` (`addss xmm0,[0xe69584]`). The clock is `0x796d00`: an OS tick count times `0.001`, so wall-clock seconds, not game time. | EX `0x5e1973`, `0x5e1148`, `0x796d00` |
| F8 and backtick call one virtual method (`0x5eb420` to `[obj+0x138]` vtable `+0x70`); the method was not decoded. Live: both select an idle worker and centre on it; a second press up to 984.7 ms later advances, 1014.3 ms later restarts. | EX `0x5eb420`; RO ([live capture](input-live-capture.md)) |
| Backspace jumps to the own Town Hall, not the selection; nothing happens without an own building. Several halls and the restart window were not exercised. | RO ([live capture](input-live-capture.md)) |

## Implementation

| Part | Where |
| --- | --- |
| Executable rules: zoom, rotation and scroll tables, ease times, wheel unit, repeat window, option fallbacks; pure zoom, swing, edge and cycle rules | `crates/pcx-play/src/camera_input.rs` (`CameraInputRules::read`) |
| Host call that eases distance, angle of attack and rotation and makes them the game camera; town hall query | `crates/pcx-play/src/camera_view_host.rs` (`ease_player_camera`), `unit_type_host.rs` (`town_halls`, `unit_camera_point`) |
| Systems: wheel and keys, scroll, drag, jump keys, alert history | `crates/peacecraft3/src/campaign_camera_input.rs` |
| FPS overlay key | `crates/pcx-gfx/src/performance.rs` (`FPS_KEY`) |

`CameraInputRules::read` accepts only the reviewed image. Each operand site is an
address plus an opcode check; a changed byte refuses the whole profile (one test flips
the first byte of every site). The ease times, the top level (checked against the
table length), the wheel unit (recovered from the multiply-shift pair and checked to be
the rounded-up reciprocal), the repeat window, the scroll constants and the option
indices come from the operands. Without the rules, zoom, rotation, edge scroll and the
jump keys stay off, an error is kept in `CameraInput::last_error`, and arrow keys keep
the `[CameraRates]` speed.

Zoom and rotation set the three fields through `ease_player_camera`, which also
rewrites the game camera, so `ResetToGameCamera` returns to the player's pose and a
running script camera is replaced the way a script setup replaces it. Script-driven
state keeps precedence: all input is off while scripts disable user control, a menu,
dialog or the quest log is open (`commands_available`), except releases of Insert and
Delete, which always end their state.

## Decisions taken where the original is not established

| # | Decision | Basis |
| --- | --- | --- |
| 1 | Eases run on the game clock (`timer_game_time`), so a paused game pauses an ease. | INF; the original camera clock is UNK |
| 2 | Fine wheel steps accumulate to one notch; pixel-unit wheels count one unit per pixel; a reversal clears the residue. A whole notch behaves exactly like the original. | INF |
| 3 | Physical key codes are used. Numpad 0 and Numpad Decimal releases are the extra clears; Numpad 0 press is not Insert. | EX covers virtual keys, INF for NumLock |
| 4 | Space, Backspace and Page keys act on key repeats; F8 and backtick do not. | EX for the first group (no repeat test); handler of the second unread |
| 5 | Modifiers are ignored for these keys. | EX switch on the key only; the action hash table behind `0x7cd160` was not recovered |
| 6 | A held arrow wins over the pointer in the same direction. The client shares one slot per direction and the last writer wins. | EX slot; INF for the order |
| 7 | Edge scroll needs a focused window with the pointer inside it; HUD panels do not block it (the arrow cursor shows over the HUD and is drawn from `CameraInput::edges`, see [input cursor](input-cursor.md)). The 0.004 margin is the named constant `EDGE_MARGIN_UI`. | RO fit |
| 8 | Scroll speed is the `[CameraRates]` value times the option multiplier. The executable multiplies a literal `3000.0`, which equals the file value. | SB and EX agree |
| 9 | The scroll options are the descriptor fallbacks (50, 50, 0). No saved-preference store exists in the recreation yet, so nothing overrides them. | See [original options initialization](original-options-initialization.md) |
| 10 | Middle drag keeps the grabbed ground point under the pointer: each frame the camera target moves by the grabbed point minus the ground point now under the cursor. | INF from the stored target and screen point |
| 11 | Backspace list: own, shown, living `UNIT_TYPE_TOWNHALL` units in creation order. | RO for the one-hall case; order INF |
| 12 | The idle-worker list is `ObjectHost::idle_workers` (also behind the idle button). The F8 and backtick repeat window reuses the Space and Backspace value. | RO bounds the window between 984.7 and 1014.3 ms |
| 13 | The alert history holds 32 points. Nothing in the recreation raises alerts yet, so Space does nothing until a producer calls `AlertHistory::push`. | Capacity project policy; producers UNK |
| 14 | The near clip plane is read once for the starting zoom index and not changed per level. | Not traced |
| 15 | The FPS overlay moved from F8 to F6 (F8 is the idle-worker key). F7, the debug overlay, is unchanged. Page Up and Page Down still page the F7 panel while it is open and also zoom. | No F6 binding found in the camera and select handlers |
| 16 | Arrow keys now stop while the quest log or a script dialog is open; before, only a menu or a control lock stopped them. | Same gate as the other input |

## Not established

- Where the edge events and the 0.004 margin come from in the executable, and
  edge-scroll speed in world units at other option values (the speed follows the
  multiplier by construction here; it was not measured).
- The per-frame update of middle-button drag and its ratio.
- The camera clock, and the angle helpers `0x5dfd00` and `0x8a19b0`.
- What `[this+0x5c]`, `0x5e0ba0()` and `[gameui+0x298]` mean.
- The sort key of the Backspace list, the multi-hall order, and the exact idle test and
  repeat window of F8.
- What raises alerts, and the capacity of the history.
- NumLock handling of the Insert/Delete group; whether modifiers change any of these keys.
- Level-dependent near clip and the `[Listener]` distance and angle (audio) the setter
  also sets.
- Native Windows behaviour: the live numbers are Wine runs with synthesised input.

## Verification

- `pcx-play` unit tests in `camera_input_tests.rs`: synthetic image with every operand
  form, every site refused when a byte changes, inconsistent operands refused, option
  rows checked by name, zoom clamping and easing, wheel units, swing states and the
  `level + 6 * state` index, scroll multipliers, the measured edge boundaries at
  1280x960, 1024x768 and 1920x1080, repeat cycles. An ignored test reads the real image:
  `PCX_CAMERA_CLIENT_IMAGE=<Warcraft III.exe> cargo test -p pcx-play --lib
  original_camera_rules -- --ignored`.
- `pcx-play` host test `player_zoom_eases_the_live_fields_and_becomes_the_game_camera`.
- `peacecraft3` binary tests in `campaign_camera_input_tests.rs` drive the real systems
  on a synthetic session with `KeyboardInput`, `MouseWheel`, mouse-button messages and
  window cursor state: no host window, pointer or audio.
