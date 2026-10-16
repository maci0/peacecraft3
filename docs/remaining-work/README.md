# Remaining work

[Requirements](requirements.json) maps all 101 open, partial, unknown or blocked
checkboxes in the tracker snapshot at commit `c2ce1d6` to source evidence,
acceptance gates, dependencies and the next implementation slice.
[Dependency order](dependency-order.md) puts prerequisites before consumers;
implementation priority is a separate ordering. The
[JSON schema](requirements.schema.json) describes the machine-readable records.

This is a source and evidence audit, not gameplay acceptance. Record identifiers
and line numbers refer to that pinned snapshot, whose SHA-256 is recorded in the
JSON. The accompanying tracker corrections change those lines; subsequent
implementation must update the live tracker rather than treating this snapshot
as current completion state.

The campaign critical path is source-backed startup dependencies, terrain and
camera initialization, a rendered HUD, persistent selection and orders, then
associated gameplay stats. Chapter progression, transmissions, quest state and
AI each require their own application consumers. Private prototypes and parser
tests do not close these gates.

The original-game baseline remains explicit: one trace must be byte-identical
across 100 consecutive runs and unchanged against that baseline. Local synthetic
repeats, admitted map metadata, network movement and successful asset parsing
remain narrower evidence.

Review checks: 11 reconciliation and mutation tests passed, including missing
records, missing evidence, weakened broad gates, reversed dependencies and
cycles. The JSON also passed independent Draft 2020-12 schema validation.
