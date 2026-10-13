# PeaceCraft3

A faithful reimplementation of **Warcraft III: Complete Edition 1.28c** in Rust on the
Bevy engine, reading the **original game assets directly from the shipped MPQ archives**.

- **[PLAN.md](PLAN.md)** — technical plan, milestones, risks, definition of faithful
- **[TRACKER.md](TRACKER.md)** — live task tracker, open unknowns, fidelity log
- **[docs/re-notes/](docs/re-notes/)** — reverse-engineering notes, cited by REA evidence id
- **[tools/rea.sh](tools/rea.sh)** — REA driver with per-target analysis snapshots

No Blizzard assets are committed. The game reads them from your own install at runtime
(`PEACECRAFT3_INSTALL`, default `wc3/game/app`).

## Status

Reverse-engineering harness is live. See [TRACKER.md](TRACKER.md) for current state.

```
wc3/game/app/game.dll    71 810 procedures / 17 207 strings   (REA + Ghidra 12.1.3)
wc3/game/app/Storm.dll    1 341 procedures /   736 strings
```

## Quick start

```sh
export PEACECRAFT3_INSTALL="$PWD/wc3/game/app"

cargo test -p pcx-mpq          # header assertions against the four real archives
tools/rea.sh doctor            # REA + Ghidra readiness
tools/rea.sh search  wc3/game/app/Storm.dll "SFile"
cargo run -p peacecraft3
```

PeaceCraft settings open first. Use **Main menu** to enter the original game
menu. Original menu rendering and audio are being verified; full gameplay and
exact visual fidelity are still in progress.

[Windows and macOS builds](docs/build-desktop.md) cover Intel/x64 and ARM64.
Browser-only ISO asset discovery, without uploading the ISO to a server, is
planned in [PLAN.md](PLAN.md).

The optional `pcx-blp` `wasm_smoke` example requires `--features local-fixtures`
and locally extracted `SelectionCircleLarge.blp`, `Footman.mdx` and `misc.tga`
under `crates/pcx-blp/tests/fixtures/`. These original assets stay untracked.
