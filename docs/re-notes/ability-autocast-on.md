# Ordinary autocast ON orders

Reviewed executable: `Warcraft III.exe` 1.29.2.9231 (SHA-256 `3f2ed012...70eed`), addresses are
virtual addresses at image base `0x400000`. Companion to `ability-autocast-off.md`, which owns the
class discovery (RTTI-style class-name strings, getter, name slot, vtable = name slot - 0x58) and
the OFF handler.

## Class ABI slots used here

Read from the class vtable (byte offsets); `Ainf` is `CAbilityInnerFire`, vtable `0xf676bc`.

| slot | meaning | Inner Fire |
| --- | --- | --- |
| `+0x1c` | class code getter (`mov eax, imm; ret`) | `Ainf` |
| `+0x1b8` | exclusion group (`mov eax, imm; ret`, or `xor eax, eax; ret`) | 1 |
| `+0x310`, `+0x314` | ON order id, ON order flags | `0xd0063`, `0x2006c01` |
| `+0x320`, `+0x324` | OFF order id, OFF order flags | `0xd0064`, `0x2007c01` |
| `+0x328`, `+0x330` | cast order id (tail jump to `+0x350`) | `0xd0062` |
| `+0x338` | ON wrapper (`call [vtable+0x340]; ret 4`) | `0xb81080` |
| `+0x33c` | OFF wrapper (`call [vtable+0x344]; ret 4`) | `0xb2a310` |
| `+0x340`, `+0x344` | ON handler, OFF handler | `0xb99a80`, `0xb99b50` |
| `+0x348`, `+0x34c` | post-toggle hooks | `0xbdb5e0`, `0xbdb5f0` |
| `+0x434`, `+0x438` | activate, deactivate | `0xb9c590`, `0xb9c5c0` |
| `+0x450` | acquire target | `0xb9c8e0` |

## IssueImmediateOrder validation (observed in shipped code)

- The native registers at `0x49e03a` and runs `0x49a610`: order string to id, then `0x6a16b0`
  (unit validation). It scans the unit's abilities (`[unit+0x3e8]` list); each ability checks the
  order through `vtable+0x264` (`0xcb6180`), which calls `vtable+0x30c` (`0xb28850`), then
  `0xb4adf0`, then the toggle check `0xb994b0`.
- `0xb994b0` for the ON id: returns `0xdd` ("not applicable") when state bit `0x80` of `[ability+0x20]`
  is already set, else `0`. For the OFF id: `0xdd` when the bit is clear, else `0`. Exception: an
  OFF id equal to the global at `0xe9ba14` (`0xd0142`) skips the ON-side bit test.
- A result other than `0` or `0xdc` makes the native return false with no order. `0` creates the order
  (`0x698910`), issues it (`0x690590`) and the native returns true. A unit with no ability for
  the order ends the scan at `0xdd`, so the native returns false.

So ON of an already-enabled ability, and ON on a unit without the ability, return false with no
effect. ON of a disabled ability is accepted.

## ON handler `0xb99a80`

Shared by all 331 class vtables that inherit the base toggle (all abilities, `+0x340` is the same
function). Steps, from the bytes:

1. Owner `[this+0x30]` (or `0x7f2610`). If owner `[+0x164]` has `0x100000` or `0x200000`, or
   `[owner+0x15c] > 0`, the handler leaves the state alone and, unless `0x100` is set and
   `[owner+0x3a4] > 0`, reports through `0x698910` and `0x6994a0`. This is the same gate as the OFF
   handler at `0xb99b50` (dead, disabled owner).
2. If bit `0x80` of `[this+0x20]` is set: return, nothing changes.
3. If the OFF id is not `0xd0142`: read the group (`+0x1b8`). If nonzero, call `0x6d9270(owner,
   group)`, which calls the OFF handler (`+0x344`) of every ability of the unit with flag
   `0x20` clear and an equal group. Then `[this+0x20] |= 0x80`, run `0x46e070` (command-card refresh
   through `0x5f5d70`).
4. Tail jump to `vtable+0x348`.

The handler sets no return value; the order's true comes from the validation above.

Group exclusion: enabling an ability switches OFF every other ability of the same unit whose class
has the same nonzero group and is currently ON. The OFF of such a peer runs the plain OFF handler:
state bit cleared, update hook run, no order and no event. Measured over the image: 34 classes
carry a nonzero group (27 with group 1, among them `Ainf`, `Aweb`, `Arai`, `Ahea`, `Aslo`; 7 with
group 2: `ANba`, `AHfa`, `AHca`, `Afak`, `AEpa`, `AHRa`, `ANia`); the other 297 return 0.

## Update hook and activation

`+0x348` (`0xbdb5e0`) jumps to `0xbdb730`. It computes active = `[this+0x3c] <= 0`, flag `0x200`
clear and `flags & 0x82`. Active and bit `0x80000` clear: set `0x80000` and call `+0x434`
(`0xb9c590`). Inactive and `0x80000` set: clear it and call `+0x438` (`0xb9c5c0`).

`0xb9c590`: if flag `0x100000` is clear, register timer event `0xd01c2` on `this+0x128` through
`0x7fbe90` with the delay float at `0x10db05c` (`0x3ca3d70a`, 0.02 s). The event dispatcher
(`vtable+0xc`, `0xb9c5f0`) routes `0xd01c2` to `0xb9d120`.

## Automatic cast tick `0xb9d120`

Runs when the timer fires. Observed conditions, in order: ability flag `0x100000` clear; the
re-arm delay is read through `+0x470` (`0xb9cbb0`, global `0x112d7c8`); `[this+0x20] & 0x20`
clear; `0x80000` set; ready check `+0x3e0` (`0xb4a4a0`) nonzero; owner `[+0x164] & 0x1000000`
clear, `[owner+0x20] & 0x4000000` clear and `[owner+0x3a4] <= 0`; then `+0x450` (`0xb9c8e0`)
returns the target. A nonzero target sets `0x220000` and casts through `0xb4b0f0` or `0xb2b080`
according to `+0x354` bits; then the timer re-arms with `max(+0x3e8, +0x474)`.

Not decoded, so not implemented: the acquisition scan in `0xb9c8e0` (range `+0x44c`, candidate
filters `+0x3f4`, `+0x458`, `+0x45c`, `+0x460`, `+0x468`, ordering `0x9a01c0`); the ready check's
mana and cooldown terms (`0xb4a4a0`, `0xb4a530`); the cast itself and the buff each class applies.

Default-enabled abilities (`UnitAbilities.auto`: `Ainf` on `ntks`, `Aweb` on `ucry` and `nnwl`) run
the same activation when the unit is created in the original. The recreation does not arm them; that
gap predates this note and is unchanged.

## Recreation

- `autocast_rules.rs` decodes per class the ON handler shape (state bit, group call, tail hook), the
  update hook, the activation timer and its delay float, and the group of every class sharing the ON
  handler. A class whose handler, hook or activation differs (`CAbilityManaShield`,
  `CAbilityImmolation`) has no ON decode and its ON refuses before any change.
  `CAbilityBlackArrow` (`ANba`, `blackarrowon` `0xd0262`, group 2) joins the bound classes.
  `selected_original_autocast_classes_bind` checks the real image (34 grouped families).
- `autocast_host.rs`: ON of a disabled ability returns true, runs the same order interruption, events and
  state revision as OFF, restores a default-on ability to its authored state or records it as enabled,
  switches the group peers OFF without events, and arms the first tick at `now + 0.02` (the decoded
  float). Already-enabled, missing-ability, dead and paused units return false without a change.
- The tick is not implemented. When game time reaches an armed tick, `advance_unit_activity` fails
  with `automatic cast of <alias> ... target acquisition and the cast are not implemented`.
  OFF, peer switch-off and unit removal disarm it. A startup script that issues ON and returns before
  game time moves 0.02 s never reaches the tick, so the order is faithful there.

Inferred, not measured: the ON order stops the unit and emits the issued-order events exactly as the
runtime-probed OFF does (same validation, order creation and issue path); no ON runtime probe exists.
