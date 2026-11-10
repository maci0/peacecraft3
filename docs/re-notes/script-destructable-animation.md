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
