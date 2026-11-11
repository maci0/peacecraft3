# Unit movement speed

Reviewed executable: Warcraft III 1.29.2.9231, SHA-256 `3f2ed012…`, ImageBase `0x400000`. Everything
below is read from the image and the shipped tables unless marked inferred or open. No runtime capture.

## Where the speed lives

The movement object of a unit sits at `unit+0x3f8` (constructor `0xb66360`, vtable `0xf24fc8`).
Fields (observed):

| Field | Meaning | Evidence |
|---|---|---|
| `[+0x74]` (tracked float at `+0x70`) | base speed | `SetUnitMoveSpeed` handler `0x4ab880` -> `0x69cf90` -> `0xb69520` stores its argument here and recomputes (`0xb6ab70`) |
| `[+0x7c]` (tracked float at `+0x78`) | speed multiplier, neutral 1.0 | init sets it to `0x112d810` (1.0); `0xb67720` adds a signed delta |
| `[+0x80]` | movement-disable count | the getter `0xb68160` returns 0.0 while it is nonzero |
| `[+0x84]` | number of decreases holding the multiplier below 1 | `0xb67720` decrements it, `0xb685d0` increments it (first increment calls into the unit at `+0x26c`) |

`GetUnitMoveSpeed` (registration `0x49d380`, handler `0x4978b0`) resolves the unit with `0x483940`,
reads the movement object through `0x691a50` (0.0 without one) and returns `0xb68160`, which returns
0.0 for a nonzero `[+0x80]` and otherwise `0xb681b0`.

## The formula (`0xb681b0`)

```
speed = limit((max(bonuses) + base) * multiplier)
```

- `bonuses`: `0x6d91f0` walks the ability list at `unit+0x3e8`, calls virtual `+0x184` on each and
  keeps the largest, starting from 0.0. Only `AIms` answers nonzero (`passive-held-items.md`).
- The add is `0x9a01c0`, the multiply `0x99ffe0`.
- `limit` raises the result to a lower bound `L` when `L` is greater, then lowers it to an upper
  bound `U` when `U` is smaller. With `L > U` the maximum wins.

### The bounds

- Building test: `0x696b50(0)` is true when the unit flag word `[unit+0x164]` has `0x10000` (the
  `IsUnitType` structure case at `0x49a251` calls the same routine). It selects the building pair
  of globals instead of the unit pair. `IsUnitType` (handler `0x49a210`, jump table `0x49a36c`) calls
  it with 0 for case 2, `UNIT_TYPE_STRUCTURE`.
- The unit type record (table `0x113e8d8`, lookup `0x6993a0`) holds `+0x1d8` (read by `0x6975b0`) and
  `+0x1dc` (`0x697410`); both read 0.0 for a missing record. They are the unit-type `minSpd` and
  `maxSpd` columns of `UnitBalance.slk` (inferred: the columns follow `spd` directly, and `spd` is
  `+0x1d4`, read by `0x67b780`). The record used is the unit's own type id (`[unit+0x34]`), or, while
  the unit carries buff `Bply` (id returned by `0xb899c0`), the type id stored in that buff at
  `+0x118` (written by `0xb88b1e` from the polymorph form argument).
- Lower bound `L`: a type `minSpd` of 0 means the `Misc` minimum of the unit class; otherwise
  `L = min(max(minSpd, 1), 522)`.
- Upper bound `U`: a type `maxSpd` of 0 means the `Misc` maximum of the unit class; otherwise
  `U = min(max(maxSpd, 1), 522)`.
- `Misc` values: the init routine `0x695dd0` reads `MinUnitSpeed`, `MaxUnitSpeed`, `MinBldgSpeed`,
  `MaxBldgSpeed` (strings `0xe9da54`, `0xe9da64`, `0xe9da74`, `0xe9da84`) into `0x113e5d4`,
  `0x113e520`, `0x113e75c`, `0x113e420`. It then clamps each minimum to at least 1.0 and each maximum
  to at most 522.0 (the static initializers `0x408490`, `0x408500`, `0x4084a0`, `0x4084e0` convert the
  integers 1 and 522), and raises a maximum to its minimum when the minimum is larger.
- Shipped values (`MinUnitSpeed` / `MaxUnitSpeed` / `MinBldgSpeed` / `MaxBldgSpeed`): root
  `Units\MiscGame.txt` (`War3.mpq`, `War3x.mpq`) 150 / 400 / 25 / 400; `Melee_V0` and `Custom_V0`
  min 10 and max 400 for units (`Custom_V1`: 150 / 400); a map's own `Misc` profile replaces them
  (the Human02 observation in `unit-move-speed.md` read a minimum of 10). `UnitBalance.slk`
  (`War3.mpq`, `War3x.mpq`) has `minSpd`/`maxSpd` 0 for most rows; 19 rows have `minSpd` 100, one 75,
  one 200, and three rows have `maxSpd` 522.

### Multiplier updates (`0xb67720`)

The argument is a signed delta. A zero delta does nothing. Otherwise the sum `[+0x7c] + delta`
replaces the multiplier, and a sum within `0.001` of 1.0 (`0x10db054`, bits `0x3a83126f`) is stored
as exactly 1.0. Buffs call it with the negated decrease when they apply and the decrease itself when
they end. Several buffs add, so the multiplier can reach zero or below; the limit then yields `L`.

### Who writes the multiplier

- Slow: ability `Aslo` (window `0xbf8270`) and buff `Bslo` (window `0xbf8290`, base `BSPD` from
  `0x6f4c00`). The ability handler (`0xbf7dbd`..`0xbf7e05`) reads `DataA` (`0x46ed70`) and `DataB`
  (`0x46ee20`) and negates both for the buff.
- Frost: buff `Bfro`, also a `BSPD` subclass, negates `FrostMoveSpeedDecrease` and
  `FrostAttackSpeedDecrease` (`passive-held-items.md`). The `BSPD` apply `0x6f0630` adds the
  movement value through `0xb67720` and the attack value to the attack object (`unit+0x3f4`).
- Unholy aura buff `BUau` (`0xc65560`): adds the value at buff `+0xe8` (the `DataA` speed increase)
  through `0xb67720`; open part below.

## `GetUnitDefaultMoveSpeed`

Registration `0x49d394`, handler `0x497490`: the type `spd` (`0x67b780`, no limit applied; 0.0 for a
missing record) and, for a unit whose first rawcode byte is uppercase and whose flag
`[unit+0x164] & 0x40000000` is clear, plus a hero term from `0x691a30` -> `0xb3ea50`, which reads
`Misc/AgiMoveBonus` (string `0xf1eb90`) and scales it by a value of the hero object. Stock
`MiscGame.txt` has `AgiMoveBonus=0`.

## Recreation

`crates/pcx-play/src/unit_speed.rs` holds `MoveSpeed` (base, per-source bonus list, additive
multiplier, bounds); `UnitActivity` keeps one per unit and `UnitActivity::speed()` is the formula.

- Creation: base is the type `spd` (absent for a unit that cannot move); bounds come from
  `SpeedBounds::resolve` with the type's `minSpd`/`maxSpd` (a missing column or a `-`, `_` or empty
  cell reads 0) and the `Misc` pair of the unit's class.
- `SetUnitMoveSpeed` writes the base. Slow and frost add and remove their movement decrease through
  `shift_unit_speed`. Held `AIms` items set and remove their bonus per item
  (`set_unit_speed_bonus`). Every change re-anchors the route and the walk animation
  (`update_unit_speed`). A dead unit only has its state updated.
- `GetUnitMoveSpeed` returns the formula result. `GetUnitDefaultMoveSpeed` returns the authored
  `spd` and refuses for a hero while `Misc/AgiMoveBonus` is nonzero or was not read.
- Refusals lifted because the single clamped speed is gone: reading or setting the speed under a
  slow or frost, a slow on a frosted unit, a frost hit on a slowed unit. The two buffs add on one
  multiplier. Their attack-speed decreases already added.
- A form change (`replace_form_profile`) takes the new type's base and bounds and keeps the
  multiplier and the bonuses (inferred: they belong to buffs and items; the type-change writes to
  the movement object are not decoded).

Tests: `unit_speed.rs` (arithmetic), `unit_activity_tests.rs`
(`unit_type_speed_bounds_replace_the_misc_bounds_unless_zero`, the agility term),
`unit_curse_tests.rs` and `inventory_frost_host_tests.rs` (natives under a slow or frost, frost with
a second decrease), `inventory_speed_host_tests.rs` (boots), `inventory_orb_tests.rs` (rows).

## Open

- `Misc` clamp: the client clamps `Misc` limits into 1..522 at load; the recreation keeps the values
  as the profiles state them, because three existing tests use `MinUnitSpeed=0` to reach a zero
  speed. Shipped values are inside the range.
- Two further limit rules in `0xb681b0` are not modelled, because the unit state they test is not
  decoded. (a) When `[unit+0x3a0] == 5`, the unit flag `[unit+0x20] & 0x8000` is set and the unit has
  an `Ahar` ability (`0xb79d80`) without its flag `0x80`, the lower bound is capped at 100
  (`0x11b5380`). (b) For a non-building with unit flag `[unit+0x20] & 0x8000000`, the upper bound is
  522 (`0x11b5384`) instead of `U`. The recreation sets neither state on any unit.
- The tracked-float store behind `[+0x70]` and `[+0x78]` (virtual slot 0, called with the value and 1)
  is read as a plain store; any rounding or notification it does is not decoded.
- Polymorph: which writes the type change makes to the movement object. Slow, frost and a speed
  bonus on a polymorphed unit, and polymorph on a slowed or frosted unit, stay refused.
- `Misc/MoveSpeedBonusesStack` nonzero (`Custom_V0`, `Custom_V1`, hence the campaign chapters): a
  held `AIms` refuses. The stacking path and what is known of it are in `passive-held-items.md`.
- `[+0x80]` (movement disable) is not modelled; nothing in the recreation sets it.
- Hero term of `GetUnitDefaultMoveSpeed` for a nonzero `AgiMoveBonus`.
