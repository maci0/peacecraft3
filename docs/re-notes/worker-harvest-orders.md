# Nearby lumber order

Human02 initialization orders `udg_Lumber01` and `udg_Lumber02` to
`autoharvestlumber`; these refer to the authored player-nine peasants
`gg_unit_hpea_0045` and `gg_unit_hpea_0044`. The shipped TriggerData label is
`Harvest Nearby Lumber`. That label establishes an immediate worker order;
it does not establish an ability autocast toggle or a harvesting algorithm.

A controlled Human02-derived map ran on the reviewed Legacy 1.29.2.9231
executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Map SHA-256:
`4080d1e37859378f5ca25c20385f9a19d4b23faa3ad6d18d253b3014b78bab41`.
The staged map and script matched the build receipt. Terrain and map overrides
were retained, standard initialization ran, and authored units and initialization
triggers were omitted. The probe created those two peasants from their authored
creation statements; only the first received the diagnostic order.

All 16 records arrived. `VersionGet` reported `VERSION_REIGN_OF_CHAOS` under
the explicit `-classic` launch. Observations:

| Checkpoint | Ordered peasant | Unordered peasant |
| --- | --- | --- |
| 5.000 before order | order 0; position (-3538.000, -4775.750) | order 0; position (-3862.250, -4799.250) |
| 5.000 first and repeated order | both returned true; order 852018 | no order issued |
| 6.000 | order 852018; position (-3537.386, -4798.449) | order 0; unchanged position |
| 10.000 | order 852018; same position as 6.000 | order 0; unchanged position |

Player-nine gold and lumber were both zero before the order and at 10.000.
The numeric current order above is an observed diagnostic value, not a constant
to copy into dispatch; production order identifiers come from the admitted
client registry. The two peasants use different authored positions, so they are
not a matched test of local tree accessibility.

This qualifies order acceptance and changed order/motion, not tree selection,
work animation, damage, collection, carrying capacity, return behavior or a full
resource cycle. An inert accepted handler would lose observable behavior.
The existing unsupported boundary remains until that behavior is modeled.
Receipts, markers and launch arguments stay local in ignored
`.scratch/auto-harvest-probe`. The Wine/software reference does not qualify
native Windows or rendered parity.

## Controlled tree and deposit cycle

A separate diagnostic used the same reviewed executable with `-classic` and
reported `VERSION_REIGN_OF_CHAOS`. Map SHA-256:
`1304832fd9ad8260511a1ccd991c55ecf483530cfcdc182c580635e2ea10c486`.
Its original Human02 member set was retained except for the replacement script.
Before measurement, the script removed existing destructables in the test
region, created an owned town hall from the authored creation statement, and
created a worker/tree pair plus a separate-owner unordered control pair. It
configured a two-way passive alliance between the test owners and reset both
lumber banks to zero. These are diagnostic stimuli, not unchanged-campaign
acceptance.

The controlled tree was requested at (-3600, -4800) and read back at
(-3584, -4800), with maximum/current life 50. The ordered worker began at
(-3600, -4700); the town hall read back at (-3776, -4032), owned by player nine.
The order returned true. Sampling used the actual game clock, with a requested
0.1-second timer period and a bound of 1200 samples.

All 155 samples arrived through the completion checkpoint at 20.500:

| First observed checkpoint | Ordered worker/tree/bank |
| --- | --- |
| 5.000 | tree life 50, order 852018, lumber zero |
| 5.500 | tree life 49, worker still at its starting point |
| 15.438 | tree life 40 after ten one-point decrements |
| 16.000 | order changed to 852017; tree life 40, lumber zero |
| 18.500 | lumber became 10 while tree life remained 40; order was 852018 again; worker at (-3661.882, -4254.364) |

The control worker retained order zero and its initial position; its tree
remained at life 50 and its owner's lumber stayed zero throughout. This observes
a complete first harvest/deposit cycle while the measured tree remains alive,
contradicting a kill-required reward model. The measurements do not directly
expose a carried-lumber counter or prove arbitrary upgrade/partial-load rules.
The order numbers are diagnostic values; production resolves their identities
through the selected client registry.

The shipped ability metadata supplies independent field meanings: `Har1` maps
`Ahar.DataA` to `Damage to Tree`, `Har2` maps `DataB` to `Lumber Capacity`, and
`Har3` maps `DataC` to `Gold Capacity`. These labels alone do not specify transfer
timing. Exact work animation phase, nearest-tree/depot selection, interaction
distance and the intervals between sampled checkpoints remain separate fidelity
questions. Local source hashes, all records and the recovered launch receipt
are in ignored `.scratch/harvest-cycle-probe-v2`. The launch wrapper timed out
while its existing container continued initialization; no duplicate was started.

## Model presentation evidence

The selected `UnitUI.slk` binds `hpea.file` to
`units\human\Peasant\Peasant`. The corresponding binary model has SHA-256
`34fd116662fd076e538bd9c8ea65895ed72a0e593536b84d3994012575842b90`.
An offline read through `pcx-mdl` retains its exact sequence intervals, loop
flags and EVTS tracks in ignored `.scratch/mdx-peasant-probe/peasant.json`.
The parser already exposes `Mdlx::event_nodes`; no new binary parser is needed.

The model contains distinct ordinary, Gold and Lumber variants of Attack,
Walk and Stand Work. Its `SNDXAREP` event occurs within the three Stand Work
intervals. This establishes authored event placement, not the worker order
that selects a sequence. In particular, the presence of a Lumber suffix does
not establish that an empty worker moving toward a tree uses that variant.
Order-to-sequence selection, carry-state transitions and sound playback remain
unqualified; the measured tree damage/deposit cycle does not qualify those
presentation details.
