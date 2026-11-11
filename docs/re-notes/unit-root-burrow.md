# Root, unroot and burrow immediate orders

Reviewed exe `Warcraft III.exe` 1.29.2.9231 (SHA-256 `3f2ed012...9570eed`, ImageBase `0x400000`).
Addresses are virtual. "Observed" is read from the assembly or shipped data; "inferred" is a
reading of observed code that no run has checked. Unit and ability records are native objects:
`[unit+0x164]` is the unit state word, `[ability+0x20]` the ability flag word, `[ability+0x30]`
the owning unit.

## Immediate-order result (observed)

- `IssueImmediateOrder` is `0x49a610`: it maps the string to an order id (`0x469010`,
  `0x477030`, `0x4770c0`; no id returns false) and continues in `IssueImmediateOrderById`
  `0x49a670`.
- `0x49a670`: a null unit returns false. It calls `0x6a16b0(unit, order, owner, 0)`; a result
  other than `0` or `0xdc` returns false. Otherwise it builds the command (`0x698910`), issues it
  with `0x690590(unit, cmd, 1, 1)` and returns true.
- `0x6a16b0` returns `0` for order `0xd0005`, tests unit bit `0x100` for `0xd0004`, and for every
  other order walks the unit's ability list (`[unit+0x3e8]`). It starts at error `0xdd` and keeps the
  smallest error over the abilities: ability virtual `+0xa0` must equal the fourth argument, the
  order check is ability virtual `+0x264(order, owner, 3)`, counters at ability `+0x40` above zero
  give `0xdd`, counters at `+0x3c` or `+0x44` above zero give `0xb9`.
- Result: a unit with no ability that handles the order returns `0xdd`, so the native returns
  false and changes nothing. Orders `0xd00c5`/`0xd00c6` (`root`/`unroot`) and `0xd0235`/`0xd0236`
  (`burrow`/`unburrow`) occur in no class getter other than the two classes below.

## Root class `CAbilityRoot` (code `Aroo`)

Ability aliases `Aro1` (Ancients) and `Aro2` (Ancient Protector) share the class; the profile
section is `[Aroo]` (`Order=root`, `Unorder=unroot`).

Observed:

- Class: name getter `0xb8d9d0`, vtable `0xf36050`, code getter `0xb8d9e0` (`Aroo`). Order getters
  `0xb8e1e0` (virtual `+0x310`, `0xd00c5` `root`) and `0xb8e1d0` (`+0x320`, `0xd00c6` `unroot`).
  The registry pushes names and ids at `0xb120f6` and `0xb1210c`.
- Level fields read by the attach routine `0xb8d9f0`: `DataA` to `+0x84` (rooted attack mask),
  `DataB` to `+0x88` (uprooted attack mask), `Dur` to `+0x90`, `HeroDur` to `+0x98`; `DataC`
  nonzero sets ability bit `0x100`. Shipped rows: `Aro1` `DataA=1`, `DataB=2`; `Aro2` `DataA=2`,
  `DataB=1`; both `Dur=HeroDur=2.5`, `Cost=0`, `Cool=0`.
- Initial state: attach sets unit bit `0x80` (`0xb8db63`) and ability bit `0x80`, and `0xb8f470`
  applies the rooted mask and sets unit bit `0x8000000` (`0xb8f537`). Bit `0x8000000` means rooted.
- Order check `0xb8dc80` (virtual `+0x264`): base check `0xcb6180` calls virtual `+0x30c`
  `0xb8de60`. That function returns `0xdd` unless the order is `0xd00c6`, then requires ability bit
  `0x80` set (`0xb994b0`) and unit bit `0x8000000` set. For order `0xd00c5` it returns `0xdd`, so
  `root` as an immediate order is always false (the point-order check is `0xb8dec0`, virtual
  `+0x308`). The final step tests that the issuing player shares control with the unit owner
  (`0x47f9f0`); player initialisation sets each player's shared control to itself
  (`ai-construction.md`).
- `unroot` accepted: `0xb99b50` clears ability bit `0x80` and calls virtual `+0x34c` (`0xb8e210`),
  which queues internal orders `0xd0162`, `0xd0148`, `0xd0190`, `0xd014c`, `0xd014e`, `0xd0178`.
  Order `0xd0190` runs `0xb8e9f0`: it clears unit bit `0x8000000` (`0xb8ea78`), applies the
  uprooted attack mask with `0xb8ed70(0)` (mask `~DataB`), removes the abilities `0xb8ec60`
  names, and posts event `0xd01e5` after `HeroDur` (`0xb8eaeb`), which `0xb8ee80` completes.
  Rooting is `0xb8e8c0`: sets the bit and posts `0xd01e4` after `Dur`.
- Attack masks `0xb8ed70`: two attack slots; the slots in the mask go to `0xb1fcb0` (sets bit
  `0x80000 << slot` in the weapon controller word) and the others to `0xb1fc20` (clears it).
- `0xb8ed70(flag)` (observed): the target is the unit's attack ability `[owner+0x3f4]` and a null
  pointer returns at once (`0xb8ed8f`). The mask is `[this+0x84]` (`DataA`) for `flag=1`, and
  `~[this+0x88]` (`~DataB`) for `flag=0`. A slot whose mask bit equals `flag` goes to `0xb1fcb0`,
  every other slot to `0xb1fc20`, so rooting enables the `DataA` slots and disables the rest, and
  unrooting enables the `DataB` slots and disables the rest. `0xb1fcb0`/`0xb1fc20` also refresh the
  command card (`0x46e070`).
- Creation (observed): `0xb8d9f0` (class vtable `+0x80`, called by the add-by-alias routine
  `0x6d8e77`) runs `0xb8f470`, which calls `0xb8ed70(1)` and then sets unit bit `0x8000000`.
  `0x6d4560` (add ability to unit) caches the ability whose code getter returns `Aatk` into
  `[unit+0x3f4]` (`0x6d4696`). The unit initializer `0x66efc0` creates the default `Aatk` ability
  through factory `0x683420` and adds it before the type's other abilities (inferred from the
  initializer's order; the authored ability list consumer was not located). With the attack
  ability present, a created Ancient starts with the `DataA` slots only, whatever `weapsOn`
  enabled; the `weapsOn` mask reaches the weapon record at `0x67c7de`.
- If unit bit `0x100000`, `0x200000` or `[unit+0x15c] > 0`, `0xb99b50` takes a deferred path
  (`0xb99b9f`, queues the order) that is not decoded.

Inferred: the slot bit set by `0xb1fcb0` is "enabled". Evidence: `Aro2` rooted mask is slot 2, and
the shipped Ancient Protector (`etrp`) has its ranged attack 2 (`rangeN2=700`) as the rooted
attack. The queued internal orders run synchronously for an idle unit.

Not decoded: what `0xd0148`, `0xd014c`, `0xd014e`, `0xd0178` do, the movement enabled by
uprooting (unit virtuals `+0x150`/`+0x154`, `0x4c5770`, `0x69b800`), the removed abilities,
the point order `root` (`0xb8f000`, `0xb8f590`), the deferred path, and event delivery.

## Burrow class `CAbilityBurrow` (code `Abur`)

Observed:

- Class: name getter `0xc51820`, vtable `0xf9a3f4`, code getter `0xc51830`, `burrow` `0xd0235`
  (`0xc51840`), `unburrow` `0xd0236` (`0xc51850`). Registry pushes at `0xb12305`/`0xb12317`.
- Its vtable equals `CAbilityRavenForm` (`0xf4a000`) except the class getters, the dispatcher
  (`+0xc`: `0xcb0050`, other events fall to the Raven dispatcher `0xbadd40`), serialization
  (`+0x38`, `+0x3c`) and `+0x410`, `+0x414`, `+0x418`, `+0x424`, `+0x438`, `+0xc4`, `+0xc8`,
  `+0xf4`. Burrow's `+0x410` (`0xc518f0`) sets unit word bit `[unit+0x20] |= 0x4000000`, `+0x418`
  (`0xc51950`) clears it. Event `0xd01bf` runs `0xcb02f0`, gated on virtual `+0x3e0` (autocast).
- The shared initializer `0xbacc00` maps `DataB` to ability bits: `1` to `0x200000`
  (uninterruptible), `2` to `0x400000`, `4` to `0x800000` (immediate form change), `8` to
  `0x2000000`, `0x10` to `0x4000000`. The existing Raven rules read bits `1` and `4`. `Abur`
  `DataB=1`: uninterruptible, no immediate form change.
- Level fields (accessors at `0x46eba0` `Cast`, `0x46f3a0` `Dur`, `0x46f500` `HeroDur`, `0x46ed70`
  `DataA`, `0x46ee20` `DataB`, `0x46eed0` `DataC`, `0x46ef80` `DataD`): `Abur` has `DataA=ucry`
  (normal form), `UnitID=ucrm` (burrowed form), `Cast=0`, `Dur=1.45`, `DataC`/`DataD` empty.
- Order check `0xbad490` to `0xbad5e0` to `0xb4adf0`: the on order needs ability bit `0x80` clear,
  the off order needs it set (`0xb994b0`). The initializer sets bit `0x80` only for a unit whose type
  equals `UnitID` (`0xbacd75`). `u000` of UndeadX04 is a custom unit derived from `ucry` (map
  object data), so it is neither form: it is treated as unburrowed, and `bad740` begins the
  change to the alternate form (`0xbafc30`).
- Form swap `0xbaed20(ability, new, old)`: calls `0x6805e0` (type change) and rescales life and
  mana with `0x6be8c0`/`0x680350` from the old and new maxima (inferred: the ratio carries).
- Swap timing: `0xbafd30` (stage 1, reached by `0xbafc30` through `0xbaec20`) calls virtual `+0x400`
  at once only when the immediate bit `0x800000` is set (`0xbafd60`), applies `0xbaf580` (the
  `0xbaed20` call when state bits `0x8000`/`0x10000` are set), and posts event `0xd01dc` after
  `Cast` (`0xbafdc9`), or calls stage 2 `0xbaf2b0` at once. Stage 2 calls `+0x400` only without the
  immediate bit (`0xbaf35b`). Event `0xd01c4` (`0xbaf000`) also applies `0xbaf580`.

Not decoded: who posts `0xd01c4`, hence the instant of the form swap and the use of `Dur` for a
form with the immediate bit clear (Burrow); the effect of unit bit `0x4000000`; the
`DataC`/`DataD` defaults for empty cells.

## Recreation

- `unit_root_rules.rs` reads the class from the selected image (name, getter, vtable, order ids, the
  rooted state bit shared by both validity functions). `unit_morph_rules.rs` reads `CAbilityBurrow`
  with `CAbilityRavenForm`.
- `unit_root_host.rs`: `root` returns false; `unroot` returns false without a handling ability,
  without availability, or when already uprooted; otherwise it records the uprooted state and applies
  the `DataB` mask to the single modelled weapon (attack 1; attack 2 is not modelled). Paused,
  dead or constructing units, nonzero cost or cooldown, and registered issued-order listeners
  refuse the call before any change. Movement orders on an uprooted unit refuse
  (`prepare_record_steer`).
- Creation applies the rooted `DataA` mask over `weapsOn` (`apply_creation_root_mask`,
  `unit_root_host.rs`): `Aro2` (`DataA=2`) on `etrp` leaves attack 2 only, `Aro1` leaves attack 1.
  The combat model holds one weapon: `weapon_for_mask` (`combat_host.rs`) takes attack 1 when its
  bit is set, else attack 2, else none. A mask with both bits keeps attack 1 (both attacks at once
  are not modelled; this is the `weapsOn=3` behaviour of every other unit). `read_weapon` reads
  the splash columns of attack 2 as well (`Farea2`, `Harea2`, `Qarea2`, `Hfact2`, `Qfact2`,
  `splashTargs2`): the shipped `etrp` row authors attack 2 as `msplash` with `rangeN2=700`,
  `cool2=2`, `dice2=1`, `sides2=10`, `dmgplus2=44` and its own splash, which the model dropped
  because it read splash for attack 1 only. The only other shipped row with a single-attack-2
  `weapsOn` is `echm` (`weapsOn=2`), which previously had no weapon.
- `burrow` on a unit with no `Abur` returns false (`morph_inactive`). On `Abur` the morph path
  refuses at `Spec::read` ("morph flags 0x1 of class Abur ..."): the swap time for a form without
  the immediate bit is not decoded.
