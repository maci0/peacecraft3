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
