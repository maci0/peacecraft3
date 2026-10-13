# TriggerData binding inputs

`pcx_trig::catalog::Catalog` parses one explicitly identified edition/version/source's
actual `UI\TriggerData.txt`. It retains editor definitions, authored argument fields,
call return types and explicit `ScriptName` aliases. Its arity table feeds the existing
WTG decoder; there is no duplicated WTG reader or copied stock-function mapping.

This is a binding input, not WTG execution. It does not generate trigger function names,
interpret GUI conditions/actions, register events, merge WCT code or assert quest success.
Those behaviors still require a translator and native evidence.

## Shipped-byte observation

The selected local Legacy `War3.mpq:UI\TriggerData.txt` decoded to 320608 bytes with
FNV-1a 64 `dda78b684d5488cf`. These describe the observation; code derives all records
from the supplied bytes and does not use those values as constants. The executable's
`1.29.2.9231` identity does not establish this archive's release identity.

The explicit ignored original-source test parsed 1136 definitions and 15 authored
aliases. Its derived arity table equals the existing `TriggerArity` parser's table.
That equality proves agreement on editor container parameter counts, not JASS/native
runtime behavior.

The supplied table also contains issues the old arity-only parser skipped:

- Conflicting repeated editor-default metadata, observed at line 2361 for
  `_TriggerRegisterUnitInRangeSimple_Defaults`. Records are preserved in source order.
  `metadata_records` exposes every line/value; `metadata` refuses an ambiguous lookup.
  No first-definition/last-definition engine precedence is assumed.
- One malformed metadata line, observed at line 5959, lacking `=`. It remains in
  `unsupported_metadata`; no corrected limit/default is invented.
  `require_complete_metadata` fails with its source line when a caller requires the
  entire metadata set to be supported.

Explicit alias conflicts prevent catalog binding. Non-call return types are absent
in this table's definition shape and remain absent rather than becoming guessed JASS
signatures. Unknown metadata is preserved. Source files were extracted to a temporary
local directory and deleted afterward; checked-in fixtures are synthetic.

## Verification

Synthetic tests check authored aliases and fields, conflicts, missing identity,
malformed records and ambiguous defaults. The ignored original test requires explicit
`PEACECRAFT3_TRIGGER_FILE`, `PEACECRAFT3_TRIGGER_EDITION` and
`PEACECRAFT3_TRIGGER_VERSION`; missing inputs fail when the test is requested.
WTG/WCT/WTS front ends remain separate from runtime execution. Athena has not passed.
