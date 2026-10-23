# Maximum states in SetUnitState

The selected original `common.j` declares current life, maximum life, current
mana and maximum mana through `ConvertUnitState`. Their initializers are the
source of the enumerands; the recreation must continue to read those names
and values from the admitted source.

## Native dispatch evidence

Reviewed client SHA-256:
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The `SetUnitState` name is at file offset `0xa70c58`, VA `0xe71e58` using the
image's actual section mapping. The registration at `0x49d5a8` names it and
the next push selects handler `0x4abba0`.

That handler resolves the unit, checks the state range and calls `0x69dd90`.
The dispatch writes only the current-life case (unit virtual slot `0x124`)
and current-mana case (`0x800ff0`, pool offset `0x1c0`). The maximum-life and
maximum-mana cases return without a write. These correspond to the selected
source globals at lines 436–439. The retained disassembly windows are local
under `.scratch/campaign-continuation/set-unit-state-*.txt`.

## Controlled runtime observation

The RoC run `max-state-write-20261022-a` confirms that dispatch behavior.
`VersionGet` reports RoC; all eleven expected native markers were collected.
The diagnostic map SHA-256 is
`b4596bfc996cd6cd4d507182d35025a9bc4f06b91cd8117122eaa8a7afc4ada3`;
its script SHA-256 is
`5e4df686ee2af12ecbe42356842f23e649d8e01dad95d82c7007f22ece5a1dce`.

The level-ten Uther control has life/max life 1250 and mana/max mana 495.
Writing 100 to maximum life leaves both life values at 1250. Writing 100 to
maximum mana leaves both mana values at 495. A second Uther equips `prvt`,
raising life/max life to 1400; the same maximum-life write leaves them at
1400, and removing the item restores 1250. These are observed source-driven
values, not runtime constants.

The original client ran through Wine with the reviewed image above and
`-classic`; this does not qualify native Windows behavior or dedicated
maximum-stat natives. Local raw markers, map/script receipts and collector
result are retained under `.scratch/max-state-write-probe/` in the main
checkout. Dedicated maximum-stat natives require their own admission and
evidence.

## Recreation correction

The previous `assign_unit_vital` replaced the maximum pools for those two
state cases. That contradicted the reviewed dispatch and allowed a maximum
write to invalidate later removal of a held life bonus. The corrected path
preserves valid call acceptance and argument checking while leaving the
maximum pools unchanged. Executed-request receipts retain the requested
value without presenting it as an effective world-state change.

Synthetic tests that use these calls to construct arbitrary maximum pools
now use explicit test-only pool mutation where necessary. Native behavior
tests assert unchanged maxima and retained requests; pool-failure tests use
an admitted source configuration that reaches
the failure, or explicitly identify an internal fixture state. The admitted
large-strength precision-loss case remains a separate reason to preflight
sequential inventory cleanup.
