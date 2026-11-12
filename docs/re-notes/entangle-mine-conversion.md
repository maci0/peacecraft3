# Entangle gold mine conversion

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Static disassembly only; no
original-client run. Builds on the order class in [input-smart-order.md](input-smart-order.md).

## Classes and data

- `CAbilityEntangle` (`Aent`, name `0xf37f60`), `CAbilityEntangledGoldMine` (`Aegm`, `0xf37f74`) and
  `CAbilityEntangleCargo` (`Aenc`, `0xf37f90`). Class getters `0xb94d80`, `0xb94da0`, `0xb94d90`.
- Shipped `Units\AbilityData.slk`: `Aent` has `Cast1` 3, `Cool1` 0, `Cost1` 0, `Rng1` 500 and
  `UnitID1` `egol`. `egol` lists `Abds,Aenc,Slo2,Adri,Aegm`; the Tree of Life types list `Aent`.
  `Aegm` `DataA1` 10 and `Aenc` `DataA1` 5 are not consumed here.
- The harvest resolver `0xb62440` maps `ngol` to ok, `egol` to `0xb946b0` (wisp only, `ewsp`) and `ugol`
  to `0xb27440` (observed).

## Conversion (observed)

Order `0xd00b3` (`entangle`) and `0xd00b4` (`entangleinstant`) reach `0xb95030(event, instant)`.
`0xb95100` checks that the target carries `Agmb` (`0xb94700`), plans the approach (`0xb94b70`) and
queues event `0xd018b + instant`. The ability handler `0xb93ca0` routes `0xd018b`/`0xd018c` to
`0xb95790`, which revalidates and calls `0xb94a70`, which calls `0xb947a0(target, instant)`:

1. The target mine is hidden (unit slot `+0x84`); undone by slot `+0x80` if creation fails.
2. A unit is created through `0x687ca0`, the `CreateUnit` factory, with owner = the caster's owner
   (slot `+0xec`), type = the ability level's `UnitID` (`0x46fb50`), position = the mine's position,
   facing = the `Misc/BuildingAngle` value reduced by `0x46dea0` (the quarter-turn snap, 270 in the
   shipped data), flags `0x502` for the instant form and `0x20101` for the timed form.
3. The new unit's `Aegm` (`0xb27980`) finds the mine's `Agld` (`0x41676c64`), releases the workers
   inside the mine (`0xb63060`), links the mine to the new unit (`0xb28160`), copies the mine's amount
   (`+0x70`) and records the mine handle (`+0x88`).
4. The instant form calls unit slot `+0x8c(4,0,1)` on the new unit, the animation restart the harvest
   code uses after a tag change (`0xb791a3`); the timed form starts a timed object (`0xb741f0`).
5. `0xb94a70` stores the new unit's handle in the ability (`+0x78`) and registers for its events
   (`0xb949a0` is the reverse: unhide the mine, remove the entangled mine).

## Implemented

`entangle_host.rs`: `entangleinstant` on a caster listing an `Aent` class ability and a live plain mine
(lists `Agld`). Hides the mine, creates the `UnitID1` unit for the caster's owner at the mine position
facing the `BuildingAngle` quarter, gives it the mine's remaining gold and the mine link (the
haunted-mine link: the mine returns, with the remaining gold, when the entangled mine dies). Returns
true. No `Aent` ability, a missing target or a non-resource target returns false.

## Open, refused

- The timed `entangle` (`Cast1` stage).
- The approach and placement test (`0xb94b70`, `0xb945e0`, `0xb955d0`, `0x7ff460`). The repo reads the
  in-range test as the gap between collision circles not exceeding `Rng1`; an out-of-range target
  refuses. This is a recreation reading.
- Non-zero `Cost`/`Cool`; targets that are resource units without `Agld` (entangled, haunted mines):
  the `Agmb` slot `0x2fc` answer for them is not decoded.
- Releasing workers inside the mine, selection transfer to the new unit (`0x4efc40`), the owner-change
  event sent to the mine (`0x6befa0`).
- Harvest of the entangled mine by wisps (`0xb946b0`): not started.

## Cargo `load` (RoC NightElf02)

`IssueTargetOrder(burrow, "load", peon)`: `load` is `0xd004e`, `unload` `0xd004f`. The only getter
returning `0xd004e` is `0xb4b840` of the cargo-load class (`Aloa`, class getter `0xb4b780`). The order
dispatch `0x49b060` calls `0x6a19a0`, which returns `0xdd` when no ability answers the order; the
caller then issues nothing and returns false. The Orc Burrow lists `Abds,Aspi,Abun,Abtl,Astd,Arbr`
(`Abun` is `CAbilityBunker`, getters `0xc17b20`, battle stations `0xd0083`, stand down `0xd0091`), none with
`Order=load`, so the call returns false. Inferred by exhaustion of order-id constants, not by a
runtime check. A unit that does list a `load` ability refuses (the load is not decoded).

## Admission and placement follow-up (2026-11-12)

Static evidence, same SHA-256 as above; no runtime comparison. Both order forms
require an answering ability before their class handler can run (`0x49b060` ->
`0x6a19a0`, documented in the cargo section). This implies that a timed order
without an answering ability should return false before reaching the casting
stage. This admission correction is not implemented here; the existing timed
casting refusal remains. No new behavior or tests are shipped in this follow-up.

`0xb94b70` first tests the caster's current coordinates through `0xb945e0`.
On failure it calls `0xb94de0`, searches through `0x6be040` with callback
`0xb955d0`, then retests through `0xb945e0`. The callback calls `0x7ff460`,
`0x67ecd0` (accepting zero or `0x45`), `0x7ff460` again, and `0x6c0f80` before
publishing accepted coordinates at context offsets `+0x1c/+0x20`. This establishes
a placement predicate/search chain, not the existing collision-circle range
interpretation. The transforms, predicate semantics and approach remain open;
no new range or placement rule is admitted by this change.

For Black Arrow, `0xcaf240` calls `0xcaf350` before `0x687ca0`. The helper reads
the source position and orientation, calls `0x9969a0` and `0x679d00`, and adds the
first two resulting coordinates to the source position. Interpreting those calls
as rotation of the supplied displacement is an inference pending their decode.
The facing branch at `0xcaf2c6` selects the source transform when argument
`+0x18` is nonzero; otherwise it calls `0x46dea0` and `0x99ffe0` with operand
`0x10db0f8`. Creation flags are argument `+0x1c` OR 2. Caller ownership, facing
operands, factory position search and `BNdm` lifetime remain unresolved here;
summon death retains its existing refusal. No original bytes are included.
