# The 32-bit WebAssembly test gate

Process note, not game behavior. No shipped-byte, native-code or runtime
evidence applies. It records how the 32-bit WebAssembly checks are actually
run, because the obvious command does not run them.

## The trap

`cargo test --target wasm32-unknown-unknown` does not execute the tests.

A `wasm32-unknown-unknown` binary exports `main`, not the WASI command
`_start`, so `wasmtime run` only instantiates the module and exits. The
target's standard library has no process to exit and no console to write to,
so the test harness's result has nowhere to go: a deliberately failing test
still exits 0, with no output. A run of that command is not evidence.

```text
$ cargo test --release --target wasm32-unknown-unknown -p pcx-sim --test determinism
    Finished `release` profile [optimized] debuginfo] target(s)
     Running tests/determinism.rs (target/wasm32-unknown-unknown/release/deps/determinism-....wasm)
$ echo $?
0
```

With a deliberate `assert_eq!(expected.tick, 999, ...)` in that test, the same
command still exits 0 and prints nothing.

## The procedure that works

`wasm32-wasip1` is the same 32-bit pointer width and has a real console and a
real process exit, so the harness's result is visible and its exit code is
the test result:

```sh
CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir ." \
  cargo test --release --target wasm32-wasip1 -p pcx-play --test trace_capacity
```

The same deliberately failing test exits 134 with the failing test named.

Limits of this path: a test that spawns a process cannot run there. The
`session_server`, `playrun`, `playgen` and `pcx-diff` process tests and the
`playgen` CLI test are native-only for that reason, and a test that traps
takes the whole binary with it, so the process-spawning tests are skipped
rather than left to fail.

## What was verified this way

The 32-bit cases the documentation claims, run as actual 32-bit WebAssembly:

- `crates/pcx-play/tests/trace_capacity.rs` — three cases, including
  `tick_count_larger_than_usize_is_not_truncated`, which only exists on a
  32-bit target, and `unrepresentable_trace_layout_is_a_typed_error_before_allocation`.
- `crates/pcx-play/tests/generator.rs` — eight cases, including the complete
  signed-coordinate domain, the exact output/work/cardinality budgets (the
  oversized tick layout, the exhausted unit-ID capacity and the output-byte
  refusal), and 100 byte-identical fresh runs of one generated scenario and of
  the checked-in seed corpus.

These are platform-safety checks on the tested boundaries. They are not
browser or native-game execution, and they do not prove Warcraft III parity.

## The guard

`tools/test_wasm_gate.py` builds a fixture with a failing test and requires
the procedure above to report it, so a silent gate cannot be mistaken for a
passing one. It skips when `wasmtime` or the `wasm32-wasip1` target is absent.
