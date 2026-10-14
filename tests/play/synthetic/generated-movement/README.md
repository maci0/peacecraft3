# Movement-v1 seed corpus

These are generated synthetic inputs, not original game data or native captures.
The three seeds are 0, 1 and 18446744073709551615. The recipe for each is:

```text
playgen SEED 12 8 24 -2147483648 2147483647 -100000 100000 132 16384 0 1
```

`crates/pcx-play/tests/generator.rs` checks byte-identical regeneration and 100
fresh executions per file. It preserves declared assertions and trace identity.
See `docs/playtest-generation.md` for limits and the native-fidelity boundary.
