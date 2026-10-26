<p align="center">
  <img src="docs/branding/peacecraft3-logo.png" alt="PeaceCraft III" width="100%" />
</p>

<p align="center">
  <a href="https://github.com/maci0/peacecraft3/actions/workflows/desktop-builds.yml"><img src="https://github.com/maci0/peacecraft3/actions/workflows/desktop-builds.yml/badge.svg" alt="Desktop builds" /></a>
</p>

# PeaceCraft III

A faithful reimplementation of **Warcraft III: The Frozen Throne 1.29b** in Rust on the
Bevy engine, reading the **original game assets directly from the shipped MPQ archives**.

- **[PLAN.md](PLAN.md)** — technical plan, milestones, risks, definition of faithful
- **[TRACKER.md](TRACKER.md)** — live task tracker, open unknowns, fidelity log
- **[docs/re-notes/](docs/re-notes/)** — reverse-engineering notes, cited by REA evidence id
- **[tools/rea.sh](tools/rea.sh)** — REA driver with per-target analysis snapshots

No Blizzard assets are committed. The game reads them from your own install at runtime
(`PEACECRAFT3_INSTALL`, default `wc3/game/app`).

## Screenshots

Captured from PeaceCraft III's Bevy renderer on 26 October 2026 using locally
supplied game assets. Rendering and gameplay remain in progress.

**Main menu**

![PeaceCraft III rendering the Reign of Chaos main menu](docs/screenshots/main-menu.png)

**Campaign and chapter selection**

![PeaceCraft III campaign and chapter selection](docs/screenshots/chapter-selection.png)

**Campaign world and command card**

![PeaceCraft III rendering the Prologue02 campaign world and command card](docs/screenshots/campaign.png)

[Capture details and known rendering limits](docs/screenshots/README.md).

## Status

Human01 and Human02 have reached authored Victory in local scripted gameplay runs.
The remaining campaign, interlude continuation and complete rendering fidelity are
still in progress. See [TRACKER.md](TRACKER.md) for the current scope.

```
wc3/game/app/game.dll    71 810 procedures / 17 207 strings   (REA + Ghidra 12.1.3)
wc3/game/app/Storm.dll    1 341 procedures /   736 strings
```

## Quick start

```sh
export PEACECRAFT3_INSTALL="/path/to/Warcraft III (Legacy)"

cargo test -p pcx-mpq          # header assertions against the four real archives
tools/rea.sh doctor            # REA + Ghidra readiness
tools/rea.sh search  wc3/game/app/Storm.dll "SFile"
cargo run -p peacecraft3
```

PeaceCraft settings open first. Use **Main menu** to enter the original game
menu after the skippable original startup cinematic. The menu uses shipped
font/art data and traced original layout rules; full gameplay and exact
visual fidelity are still in progress. Browser fullscreen is available.
The target is the installed classic 1.29b release. Its executable reports
`1.29.2.9231`; the earlier `9232` label was not verified. The extracted
compatibility tree contains a `1.26.0.6401` Game.dll, so its old “1.28c” label
is not reliable evidence of every archive’s version. See the
[original-runtime evidence](docs/re-notes/glue-coordinate-runtime.md).

[Windows and macOS builds](docs/build-desktop.md) cover Intel/x64 and ARM64.
The [native build workflow](https://github.com/maci0/peacecraft3/actions/workflows/desktop-builds.yml)
compiles, tests and packages each target separately.
The [browser demo](https://maci0.github.io/peacecraft3-demo/) contains no original
game assets. Import your own local ISO; base-only content does not enable TFT.
See [supported imports and packaging](docs/pages-demo.md). This is a partial menu
demo, with installer-only images and full gameplay still unsupported.

The optional `pcx-blp` `wasm_smoke` example requires `--features local-fixtures`
and locally extracted `SelectionCircleLarge.blp`, `Footman.mdx` and `misc.tga`
under `crates/pcx-blp/tests/fixtures/`. These original assets stay untracked.
