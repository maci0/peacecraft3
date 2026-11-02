# Parser nesting bounds

A map ships its own `war3map.j`, and a community map can ship one whose
nesting the parser cannot bound. The parser is a recursive descent, so unbounded
nesting is a Rust stack overflow: the process dies, not the map. The parser now
refuses such a script with an ordinary parse error.

## The two bounds

`pcx_script::parser::MAX_PARSE_DEPTH` (64) bounds the *shape* of a script: how
many declarations, statements, expressions and unary chains may nest inside
each other. It also keeps the tree the parser builds small, which bounds the
drop of that tree — a recursive drop of the same depth.

`pcx_script::parser::MAX_PARSE_STACK_BYTES` (512 KiB) bounds the *descent*: how
much of the caller's stack one parse may spend. The parser measures this
directly, by comparing the stack address of the parse's own frame with the
lowest address the descent reached (`Parser::enter_nesting` takes the address
of a local and keeps the lowest one it sees). The address comparison is
direction-agnostic (`usize::abs_diff`), so it holds on a platform whose stack
grows up.

The two are complementary. The depth limit is a guess about how many levels
fit; the byte budget is a measurement, so it holds whatever shape the input
takes. Raising either one alone does not remove the other's protection.

## Why these numbers

The engine's own limits are not measured, so both are harness guards of the
same kind as `pcx_script::eval::MAX_CALL_DEPTH`, not game rules.

The budget is a quarter of the smallest stack a parse runs on here: Rust's
2 MiB default for a spawned thread, which is also what a test-harness thread
gets. The production paths give their script workers 32 MiB
(`pcx_play/src/map_startup_host.rs`, `peacecraft3/src/campaign_workers.rs`).

What a level costs was measured in an unoptimized build, per level of the
shape:

| Shape | Bytes a level costs |
| --- | --- |
| nested statement block (`if … endif`) | 15 024 |
| parenthesised group | 8 484 |
| unary chain (`- - -`) | 1 914 |

The block is the dearest shape, and it is refused at the budget after roughly
34 levels; the other two shapes reach the depth limit first. The measurement is
the `parse_stack_use` helper, which the crate's own tests read.

## Real scripts

The shipped map scripts measured on this machine nest 15 levels deep
(`human07.j`) and 8 (`human06-interlude.j`), and spend 54 768 and 28 896 bytes
of stack. The depth limit is more than four times the deepest of them, and the
budget about ten times. The ignored test
`the_local_real_scripts_stay_inside_both_bounds` repeats the measurement over
a directory of real scripts named by `PCX_SCRIPT_CORPUS_DIR`.

## What this is not

- The engine's own limits are not established. A script the engine accepts and
  this parser refuses is a refusal of this harness, not a statement about the
  original.
- The measurements are from an unoptimized build on this machine. An optimized
  build spends less stack per level, so the budget binds later there; the
  budget is a ceiling, not a target.
- The bounds cover the parser only. The VM's own recursion is bounded by
  `eval::MAX_CALL_DEPTH`, and the trigger lowering has its own nesting limit.
- No campaign script was run for this slice, and no original was observed.

## Provenance

- 2026-11-02: the depth guard was added without a stack bound, and the
  refusal it produced still overflowed a default-sized test thread at 512
  levels. The depth limit is now 64 and the stack budget was added, so the
  refusal is a typed error on any stack the parser runs on. Measured with the
  local shipped map scripts `human07.j` and `human06-interlude.j` and with the
  synthetic shapes above; no original executable was involved.
