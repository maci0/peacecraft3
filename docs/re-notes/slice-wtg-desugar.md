# WTG desugar: ForLoopVar

`ForLoopVar` is a version-4 action (`integervar`, `integer`, `integer`, `code`).
`Wtg::parse` already reads it as an ordinary action. `lower` used to refuse the
name from `emit_call`.

A statement `ForLoopVar` now writes the loop with the declared integer variable
as the index. The end value stays in the `exitwhen`. `ForLoopA` still stores
its end in `bj_forLoopAIndexEnd`; this action does not. The body is the single
wrapped action, inlined the same way as `ForLoopA`.

```
set udg_Count = 1
loop
    exitwhen udg_Count > 10
    call DoNothing()
    set udg_Count = udg_Count + 1
endloop
```

Names already produced for other actions: `udg_` variables, `Trig_<id>_Actions`,
and the inlined action call. No new function is introduced.

The index must be a declared `integer` variable. Anything else is
`LowerError::Unsupported`. A call-shaped `ForLoopVar` is still refused
(`editor-expanded ForLoopVar`), because it remains in `EDITOR_EXPANDED`.

`AndMultiple`, `OrMultiple`, `IfThenElseMultiple`, `ForLoopAMultiple`,
`ForLoopBMultiple`, `ForLoopVarMultiple`, `ForForceMultiple`, `ForGroupMultiple`,
and `CustomScriptCode` stay refused.

`docs/re-notes/wtg-lowering.md` records this action separately from the refused
editor-expanded names. No shipped campaign map contains `ForLoopVar` (probe count
0). This slice does not claim a campaign script ran.

## Provenance

- 2026-10-31: emit statement `ForLoopVar`. The index and `exitwhen` shape follows
  the editor transcript at https://xgm.guru/p/wc3/jass_conditions_and_loops
  (`set udg_i = 1` / `exitwhen udg_i > 10` / `set udg_i = udg_i + 1`), which is
  the same comparison and step `ForLoopA` already writes with
  `bj_forLoopAIndex`. Checked by `for_loop_var_indexes_the_declared_integer` in
  `crates/pcx-trig/src/lower.rs` and `crates/pcx-trig/tests/lower_wtg.rs`.
