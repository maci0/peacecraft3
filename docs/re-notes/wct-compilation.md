# Bounded WCT custom-text compilation

`pcx_play::custom_scripts::CustomScripts::compile` compiles only authored
non-empty WCT blocks. It validates WCT/WTG association through
`pcx_trig::wct_wtg_violations`, preserves block order and trigger association,
and returns one resolved `pcx_script::Program` per custom trigger.

Preludes (`common.j`, `Blizzard.j`, optional `war3map.j`, optional local WTS) are
supplied by the caller. No JASS prelude text is generated and no WTG ECAs are
lowered. Input budgets are caller policy, not game limits.

Source diagnostics render against the stored WCT block source using the
trigger-index label `war3map.wct:trigger[index]:<trigger>`. Typed errors cover
identity, budget, association, UTF-8, parse and resolution failures.