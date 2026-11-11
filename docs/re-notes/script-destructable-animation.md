# Script destructable animation and water base colour

## Declarations

The selected Legacy `Scripts\common.j` (`War3x.mpq`) declares:

- `SetDestructableAnimation takes destructable d, string whichAnimation returns nothing`
- `QueueDestructableAnimation takes destructable d, string whichAnimation returns nothing`
- `SetWaterBaseColor takes integer red, integer green, integer blue, integer alpha returns nothing`

`Scripts\Blizzard.j` `SetWaterBaseColorBJ` converts percentages with `PercentTo255`
and passes `100 - transparency` as alpha. The destructable gate helper
`ModifyGateBJ` and `ChangeElevatorHeight` call the two animation natives with
clip names such as `stand`, `death alternate` and `birth third`.

## Destructable animation

`ObjectHost` records each call on the destructable handle, outside the arena entry.
`SetDestructableAnimation` starts a new epoch and drops the earlier calls.
`QueueDestructableAnimation` appends one call. Both:

- refuse a name longer than 16 KiB or containing a control character;
- refuse when no game clock is admitted;
- ignore a null destructable, as the other destructable setters do;
- refuse a wrong declaration or argument type through the runtime signature table.

The queue refuses once 8 queued calls have been recorded since the last set
(`QUEUED_LIMIT`, shared with unit animation queues). That capacity is a recreation
bound; the original capacity is not established.

The renderer owns the model `Animation` of each drawn destructable. It applies the
calls it has not seen, through `Animation::select` and `Animation::queue`, before
advancing that animation. A missing clip name is ignored by those calls, as it is
for units. A call recorded before the animation's current clock starts at that
clock. A destructable first drawn after a call therefore starts the clip when it is
drawn. Start times of calls made while the destructable is off-screen are not
established.

## Destructable animation speed

Reviewed exe SHA-256 `3f2ed012...0eed`. `common.j` declares
`SetDestructableAnimationSpeed takes destructable d, real speedFactor returns nothing`.
The registration `0x49d1a8` (signature `(Hdestructable;R)V`) pushes handler `0x4a7b30`:

- `0x481470` resolves the handle to the destructable object; a null result ends the call
  (a null or removed destructable is ignored).
- Otherwise `0x6d2ca0` stores the real at `[object+0x164]` unchanged (no clamp or
  validation), reads the model pointer at `[object+0x28]` (`0x6a2900`) and calls
  `0x967830(model, speed)`, which calls the model's virtual slot `+0x28` (a null model
  reports error `0x57`). Unit `SetUnitTimeScale` ends in the same `0x967830` call
  (`script-unit-animation.md`), so the speed is the model time scale.
- The constructor `0x6cf36f` initialises `[object+0x164]` to `1.0` (`0x3f800000`). The model
  create path (`0x6d1e4c`) and the hide path (`0x6d20a2`) reapply the stored value, so the
  speed belongs to the destructable and survives hiding, showing and restarting a clip.
  `SetDestructableAnimation` (`0x4a7a60` to `0x4a7a80`) does not touch it.

Implementation: `ObjectHost` keeps the last call (factor, game time, serial) beside the
animation log, outside the per-restart epoch. The renderer applies an unseen call through
`Animation::set_time_scale` at `max(call time, animation clock)`, which keeps the visible
phase as it does for units; zero freezes and a negative factor plays backwards (the
recreation's unit semantics; the native passes the real to the model unchanged). The
native refuses a non-finite real and a missing game clock, like the sibling setters. Not
decoded: how the model virtual slot `+0x28` treats zero or negative factors; the
recreation reuses its unit time-scale behaviour for them.

## Water base colour

`SetWaterBaseColor` stores its four channels in `PresentationState::water_base_color`
as 0 to 255 values. A channel outside that range refuses and leaves the prior
value. Nothing reads the value yet. The water surface draws the frame texture with a
white base colour, and `Water.slk` colour cells stay raw
(`crates/pcx-tera/WATER_EVIDENCE.md`). No evidence establishes how a base colour
combines with the frame texture or those cells, so the rendered effect is not
implemented.

## Still refused

- `SetDoodadAnimationRect` refuses any doodad of the matched type that has a
  pathing footprint, wherever it stands. The footprint-overlap branch of native
  `751d90` is not established (see `script-doodads.md`).
- `CreateUnit` refuses the neutral building `nmoo` (Fountain of Life) in TFT
  `HumanX01`: its `UnitBalance` `spd` cell is `"-"`, the absent-value marker. Whether
  an absent speed means zero for an immobile building is not established.
  Refusal now names the column and cell.

## Probe state

`native_campaign::tests::selected_install_locked_chapter_startup_probe` on the
selected Legacy install, one chapter at a time:

| Chapter | Before | After |
| --- | --- | --- |
| RoC `Undead05` | `SetDoodadAnimationRect`: footprint-overlap selection not admitted | same |
| RoC `Undead08` | `SetDestructableAnimation` unsupported | passes activation |
| TFT `HumanX01` | `SetWaterBaseColor` unsupported | `CreateUnit` refuses `nmoo` `spd` `"-"` |
