# war3map.wtg version 7 (Frozen Throne) checked reader

Module `pcx_trig::wtg_v7` (source `crates/pcx-trig/src/wtg_v7.rs`) reads
the classic Frozen Throne trigger-graph format (version word `7`), which
differs from the crate's version-4 `Wtg` reader.

Record additions relative to version 4:
- category: `is_comment`, `is_expandable`, `parent_id`
- variable: `array_size` word after `is_array`
- trigger: `is_comment`, `id`, `parent_id`
- ECA: optional `group` word on child ECAs, trailing `child_branch` word,
  nested `child_ecas`
- parameters/subrecords reuse the same four parameter kinds and three sub
  kinds as version 4, including catalog arity lookup

Unknown discriminants, missing catalog arities, non-unit sub flags and
trailing bytes are errors. Synthetic-fixture tests live in
`crates/pcx-trig/tests/wtg_v7.rs`; no shipped version-7 fixture was
available in this checkout, so no shipped-byte parity claim is made.