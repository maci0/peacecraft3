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

Skipping is done by the test filter, not by the build: nothing in these test
targets is compiled out for `wasm32-wasip1`. The filter has to be named, and
without it the process-spawning test traps and takes the whole binary with it,
so the run fails even though the cases under test passed:

```sh
CARGO_TARGET_WASM32_WASIP1_RUNNER="wasmtime run --dir ." \
  cargo test --release --target wasm32-wasip1 -p pcx-play --test generator \
  -- --skip cli_emits_executable_rpl_and_refusals_do_not_emit_partial_output
```

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

A sweep of `pcx-play`'s library tests at 32-bit found one assertion that was
a 64-bit observation rather than a requirement: the town centre's payload
carries a position and a handle, and only a 64-bit target's niche layout
keeps that wider payload the same size as a position alone. Nothing depended
on the sizes, so the observation is now scoped to the targets where it holds.
Two of the library's test groups need a filesystem the 32-bit path does not
provide — the campaign data set's file-backed cases and the native coverage
audit, which reads the crate's own sources — so they stay native-only. Every
other group passes at 32-bit.

Of the forty `pcx-play` integration targets, thirty-three pass at 32-bit.
The seven that do not need a host the WebAssembly path does not provide:
`chapter_scenarios` and `headless_startup` want a filesystem, and
`pcx_diff_cli`, `playgen_cli`, `playrun_cli`, `playrun_script_cli` and the
generator's own CLI case want to spawn a process.

`pcx-sim`'s whole suite — its library, the column lookup, the coordinate
bounds, the pathing and movement admission, the selection, the world grid, the
world import and the determinism cases — passes as actual 32-bit WebAssembly.
`pcx-script`'s whole library suite passes there too, including the refusal of
a hostile script whose nesting is deeper than the parser bounds: that guard
measures the descent's stack use rather than assuming a frame size, so it holds
on a 32-bit stack as well.

These are platform-safety checks on the tested boundaries. They are not
browser or native-game execution, and they do not prove Warcraft III parity.

## The guard

`tools/test_wasm_gate.py` builds a fixture with a failing test and requires
the procedure above to report it, so a silent gate cannot be mistaken for a
passing one. It then runs the harness's own 32-bit cases through the same
procedure, so the gate covers the harness and not only itself. It skips when
`wasmtime` or the `wasm32-wasip1` target is absent.
