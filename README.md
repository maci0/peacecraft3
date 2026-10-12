# PeaceCraft3

A faithful reimplementation of **Warcraft III: Complete Edition 1.28c** in Rust on the
Bevy engine, reading the **original game assets directly from the shipped MPQ archives**.

- **[PLAN.md](PLAN.md)** — technical plan, milestones, risks, definition of faithful
- **[TRACKER.md](TRACKER.md)** — live task tracker, open unknowns, fidelity log
- **[docs/re-notes/](docs/re-notes/)** — reverse-engineering notes, cited by REA evidence id
- **[tools/rea.sh](tools/rea.sh)** — REA driver with per-target analysis snapshots

No Blizzard assets are committed. The game reads them from your own install at runtime
(`PCX_WC3_INSTALL`, default `wc3/game/app`).

## Status

Reverse-engineering harness is live. See [TRACKER.md](TRACKER.md) for current state.

```
wc3/game/app/game.dll    71 810 procedures / 17 207 strings   (REA + Ghidra 12.1.3)
wc3/game/app/Storm.dll    1 341 procedures /   736 strings
```

## Quick start

```sh
export PCX_WC3_INSTALL="$PWD/wc3/game/app"

cargo test -p pcx-mpq          # header assertions against the four real archives
tools/rea.sh doctor            # REA + Ghidra readiness
tools/rea.sh search  wc3/game/app/Storm.dll "SFile"
cargo run -p peacecraft3
```