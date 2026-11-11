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

### Poster of event `0xd01c4` and the delayed swap (observed in code, nothing run)

- Poster: `0xbafb00`, at `0xbafc19` (`push 0xd01c4`, then the timer arm `0x7fbe90` on the member
  `[ability+0xe8]`). It is the last step of both start routines: forward `0xbafc30` (`0xbafd0f`)
  and reverse `0xbaf750` (`0xbaf7f9`, `0xbaf80e`). The Raven dispatcher `0xbadd40` maps
  `event - 0xd01a0` through a byte table (`0xbadec4`) and a jump table (`0xbadeac`); index `0x24`
  (event `0xd01c4`) reaches `0xbadd65`, which calls `0xbaf000`. Other classes push the same id
  at `0x6ba484`, `0xac3193`, `0xb1e30f`, `0xb8089c`, `0xbfdf14` and others, each handled by its own
  dispatcher; the id is generic. No other push of it lies in the Raven or Burrow code.
- Direction: `0xbad740` (vtable `+0x3c8`) calls reverse `0xbaf750` when the unit's type
  (`[unit+0x34]`) equals the alternate form `[ability+0x194]` (`UnitID`), and forward `0xbafc30` for
  every other type. `[ability+0x190]` is `DataA` (normal form). Forward sets ability bit `0x8000`,
  virtual `+0x410`, then `0xbaec20`: when the type equals the normal form and exactly one of the
  two forms has movement type 2 (`0x689490`, UnitData `movetp` air) it takes the flight stages
  (`0xbaf820` landing, `0xbafd30` take-off); otherwise virtual `+0x400` (`0xbad830`, animation tag
  `0x67a900(0xb)` and unit virtuals `+0x8c`/`+0x90`; effects not decoded). A type that is neither
  form skips the air test and takes `+0x400` directly.
- `0xbafb00`: if ability bit `0x80000` (set only for a unit with `[unit+0x164]` bit `0x10`, which
  `docs/re-notes/input-control-groups.md` and `0x6d8cd7` give as the loaded-in-a-container flag) it calls `0xbaf000` at once. Otherwise
  it counts the ability busy counter `[ability+0x44]` up (`0x46e130`; `0xb4adf0` answers `0xb9`,
  so `IssueImmediateOrder` returns false, while it is above zero) and the unit counter
  `[unit+0x3a4]` up (`0x694c10`), then arms the timer with no epsilon test. Delay, by the float
  add `0x9a01c0` (`0xbafbd6`..`0xbafc08`): `Dur` (`0x46f3a0`, level `[ability+0x50]`)
  `+ [ability+0xa0] + (DataC if ability bit 0x40000 else 0.0)`. `[ability+0xa0]` is the value of a
  base-class float member (`+0x9c`, initialised to `0.0` at `0xb2ae0e`); no Raven or Burrow routine
  writes it. Bit `0x40000` is set only at `0xbaf832` (landing stage `0xbaf820`). So a ground form
  waits exactly `Dur`; `Cast`, `HeroDur`, `DataC` and `DataD` are not read. `0xbad740` stores the
  same sum in the member at `+0x94` (`0xbad7e3`). `Abur`: `Dur=1.45`, `DataC=DataD=-`.
  Nothing reads the `morph` sequence length: the delay comes from the ability row only.
- `0xbaf000` (the `0xd01c4` handler): clears the timer, releases both counters
  (`0x46e6c0`, `0x688710`), then `0xbaf580`. `0xbaf580` with state bit `0x8000` calls the swap
  `0xbaed20(ability, alternate, normal)` only when the unit's type equals `[ability+0x190]`
  (normal) and the unit is not dead (`[unit+0x164]` bit `0x100`, unless ability bit `0x8000000`);
  with `0x10000` it swaps back only when the type equals `[ability+0x194]`. Then `0xbaf190` (or
  `0xbaeef0`) runs the completion virtuals (`+0x408`/`+0x414`, or `+0x40c`/`+0x41c`) and clears the
  state bit. Ability bit `0x80` (the "alternate" state the order check reads) is set at order
  acceptance by the base routine `0xb99a80` (`0xb99af1`) and cleared by the off order `0xb99b50`
  (`0xb99b85`), each followed by the queueing virtual (`+0x348`, `+0x34c`, through `+0x380`
  `0xb49ce0`, internal orders `0xd0148` and others). The chain from that queue to `0xbad740` was not
  traced and is read as synchronous for an idle unit, as for `unroot`; a unit with
  `[unit+0x164]` bit `0x100000` or `0x200000`, or `[unit+0x15c] > 0`, takes the deferred branch
  (`0xb99b0b`; it queues the order with `0x6994a0` when `[unit+0x3a4] > 0`), not decoded. The
  opposite order during the delay is refused by the busy counter.
- `0xbaed20`: type change `0x6805e0(unit, flags 0x522, new type)` (flags `+0xb00` when virtual
  `+0x430` is nonzero), then life and mana are rescaled by the new/old maxima ratio
  (`0x67b6c0`, `0x67b740`, `0x9a00b0`, `0x6be8c0`, `0x680350`). `0x6805e0` re-adds abilities
  (`0x6d4560`) and calls the life and mana regeneration refreshes (`0x6a0f20`, `0x6a1070`); that
  these read the new type's rates is inferred. Ability bit `0x4000000` (`DataB` bit `0x10`) adds an
  extra pair of calls (`0x4b5820`, `0x4b5860`) that the Burrow row does not use.
- Burrow's `+0x410` sets unit word `[unit+0x20] |= 0x4000000` (`0xc518fc`); `+0x418` clears it
  (`0xc5197d`) and is called only by the reverse path. The consumers of that unit bit are not decoded.

### The custom unit `u000` and `Abur`

UndeadX04's `u000` (`war3map.w3u`: base `ucry`, `unsf`, `ua1b`, `ua1d`, `ua1s` set; no `uabi`)
keeps `ucry`'s `abilList` (`Aweb,Aspa,Abur,Aiun`), so it owns the stock `Abur`, whose `DataA` is
`ucry`. Its type id `u000` is neither `DataA` nor `UnitID`, so:

- `burrow` and `unburrow` are both accepted (ability bit `0x80` toggles at acceptance) and both run
  the forward start routine (`0xbad740` tests the alternate form only), including the `Dur` delay,
  the busy counters, the `0xd01c4` event and the completion virtuals.
- `0xbaf580` skips the type change at the end of each, so `u000` never becomes `ucrm`.
- Unit bit `0x4000000` stays set after `unburrow` (`+0x418` runs only on the reverse path); its
  consumers are not decoded, so no effect of it is modelled.

UndeadX02 uses the stock `ucry` and takes the swap. `ucry` and `ucrm` differ in model, ability list
(`ucrm` lacks `Aweb`) and life regeneration (`regenType` `blight`, `regenHP` 2 against `always`, 5);
`HP` 550, `manaN`, armor and defense type are equal.

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
- `burrow` on a unit with no `Abur` returns false (`morph_inactive`).
- `Spec::read` (`unit_morph_rules.rs`) admits `DataB` equal to the uninterruptible bit alone
  (delayed change) beside the existing uninterruptible-plus-immediate value; any other value
  refuses with the flags named. The delayed change reads `Cost` and `Dur` only.
- `unit_morph_host.rs`, `Change::Delayed`: the order is accepted like an immediate form (mana
  debit, `morph` animation, issued-order and spell events at once), nothing else changes at the
  order, and the form changes at `now + Dur`. The finish and end-cast events follow the change;
  that Burrow posts them as Raven does is inferred from the shared vtable, not observed. While
  the change is pending both orders return false (the busy counter, `0xb9`) and other orders to
  the unit refuse through `require_morph_interruption`, as for the immediate form.
- The swap (`commit_delayed_swap`) uses the replacement path of Polymorph (`FormChange::Replacement`,
  `commit_replacement_form`): new definition, model animation, movement profile, weapon and
  the target type's ability list; life, mana, armor and defense type stay. Life and mana regenerate
  to the swap on the old rates, then take the new form's life rate and regeneration type. It
  is prepared at the order, so a failing form refuses before any change: the forms must both have
  non-air movement (`0xbaec20`; an air form without the immediate bit runs the flight stages, not
  decoded), equal `HP`, `manaN`, `fused`, `fmade`, `isbldg`, `def`, `defType`, `regenMana` and
  `moveHeight`, and the unit's ability list must equal its type's own (the swap replaces it).
  The ratio rescale of `0xbaed20` is the identity for equal maxima and is not implemented.
- A unit whose type is not the source form of the direction (UndeadX04's `u000`) takes the same
  phase without the swap, as `0xbaf580` does. A unit created as the alternate form is refused
  (its initial burrowed state, `0xbacd75`, is not modelled).
