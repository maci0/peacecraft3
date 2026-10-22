# Scripted movement speed

Human02's intro calls `SetUnitMoveSpeed` on Arthas. Other campaign scripts also
request speeds below the editor's nominal minimum; the editor field alone does
not establish the native setter's permitted range.

## Controlled observation

A Human02-derived diagnostic ran on the reviewed Legacy 1.29.2.9231 executable,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Map SHA-256:
`f87d9dd8520a6c3255ef7d8f89c63d60b650264b447c3bb485810c2dd9b016d4`.
Staged map and extracted script matched their build receipt. Original terrain,
information and unit overrides were retained; authored units and initialization
triggers were omitted. Each checkpoint explicitly reported
`VERSION_REIGN_OF_CHAOS` under the `-classic` launch.

Five fresh footmen received separate input speeds at 5.000 and a point move
order. Pathing was disabled for these boundary cases. Every unit initially
reported current/default speed 270; the default getter remained 270 after each
setter. Every move order returned true.

| Setter input | Immediate current-speed readback | Position at 13.000 | Current order at 13.000 |
| --- | --- | --- | --- |
| 80 | 80 | (-271.484, 2240.001) | none |
| 300 | 300 | (-264.670, 2079.983) | none |
| 0 | 10 | (-688.473, 1919.891) | move |
| -50 | 10 | (-688.473, 1759.891) | move |
| 1000 | 400 | (-264.879, 1599.956) | none |

The five units began at x=-768 in separate horizontal lanes and were ordered to
x=-256. Both zero/negative cases continued moving slowly; neither stopped or
reversed. These observations establish the tested input/readback pairs, not the
source or full arithmetic of the clamp. Production bounds must be derived from
admitted assets or native bytes, rather than copied from this table.

A sixth footman retained pathing. It began moving at its default speed, changed
to 80 at 6.000, then received its `GetUnitDefaultMoveSpeed` result at 7.000.
Its position was (-509.454, 2403.003) at the slow setter and
(-429.490, 2404.879) at reset. Current speed immediately returned to 270 while
the default getter stayed 270. It continued to (243.638, 2400.864), with no
current order, by 13.000. This observes moving-unit speed changes and continued
motion; it does not establish the native internal route-rebasing algorithm or
Walk animation phase.

All 57 expected files and 217 payload records arrived, including completion at
13.000. Local receipts, protocol, hashes and markers remain in ignored
`.scratch/move-speed-runtime-probe`. The Wine/software reference does not
qualify native Windows or rendered parity. Bonus composition, other movement
classes, nonfinite inputs, and exact endpoint/turn timing remain separate gates.

## Source-derived bounds

New games resolve the logical `Units/MiscGame.txt` member through the scoped
campaign reader, then let a map-local Misc profile override it. No play-side code
names an edition-specific game-data member. `MoveSpeedLimits::read` takes
`MinUnitSpeed`, `MaxUnitSpeed`, `MinBldgSpeed` and `MaxBldgSpeed` from those
admitted profiles. Missing, non-finite, negative or inverted limits refuse
admission rather than falling back to a copied numeric range.

For a definition, authored speed is `UnitBalance/spd`. Structure classification is
`UnitBalance/isbldg`, so a movable `foot`, `horse` or `fly` unit uses the unit pair
and a building uses the building pair. Movable units have their authored speed
clamped when the activity state is created. `SetUnitMoveSpeed` accepts any finite
real, including a negative input, and passes it through the same source-derived
clamp. `GetUnitMoveSpeed` returns the stored speed after that clamp;
`GetUnitDefaultMoveSpeed` returns authored `spd` without applying the setter
limits. NaN, infinity, wrong arity, wrong value or handle family, and calls on a
non-unit family are refused; null and removed units remain no-ops.

The focused fixture test uses non-stock limits (`MinUnitSpeed=37.5`,
`MaxUnitSpeed=412.5`, `MinBldgSpeed=41.25`, `MaxBldgSpeed=311.5`) and checks unit
and building negative/over-range inputs. This demonstrates that the floor comes
from the admitted profile rather than the observed stock value or a copied
constant.

## Recreation choices and open fidelity

The controlled readbacks above establish finite input/readback pairs for one foot
unit class. They do not establish creation-time clamping, class-specific native
limits, or map-profile composition. The recreation clamps authored movable speed
at creation and leaves the default getter raw; native behavior at creation and on
non-movable units is unqualified. Structure setters are applied even when the
structure has no moving order; no native structure readback covers that case.

Changing speed while moving reselects the `Walk` animation at the setter time,
using the existing explicit-selection rule that clears queued clips while keeping
the unit scheduled and its motion rebased. A host test locks that queue
interruption and continued scheduling. The diagnostic did not establish native
Walk phase, animation queue interaction or the exact route-rebasing arithmetic.

An admitted profile may set `MinUnitSpeed=0`, so the setter can store a zero
speed. Motion projection then retains the route, destination and current order
without displacement; it emits no nonfinite span and never divides by the zero
speed. A later positive setter reanchors the retained route and motion resumes.
A synthetic zero-minimum fixture locks that behavior. This is a recreation
choice: the controlled stock profile clamps zero to its minimum of 10, so no
probe observed a stored zero speed or the native zero-denominator behavior.

The controlled probe reported `VERSION_REIGN_OF_CHAOS` checks under Wine, not
native Windows or rendered parity. Nonfinite behavior, other movement classes,
bonus compositions and exact endpoint/turn timing remain separate gates.
