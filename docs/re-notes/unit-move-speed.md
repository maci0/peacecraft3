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
