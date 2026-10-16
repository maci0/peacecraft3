# Original seeded RNG probe

Two fresh isolated Wine runs loaded the same private, intentionally modified
map and emitted identical measured results. This establishes a working seed
experiment and output channel; it does not establish recreation RNG parity.

The selected executable is `Warcraft III.exe`, VERSIONINFO `1.29.2.9231`,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Each run uses its own copied installation, Wine prefix, profile and framebuffer
at 1280×960 and 96 DPI. Wine/software Mesa is not native Windows ground truth.
The selected source map's release is not inferred from the adjacent executable.

## Experiment and observations

The local map preparation preserves every retained original member except its
script. Four reversible script edits add self-authored probe functions and call
them before the original `main` setup. The fresh unsigned archive omits optional
attributes/signature metadata rather than presenting stale checksums as valid.
All 15 retained members were independently re-extracted; the original map and
installation remain unchanged. Script resolution and five synthetic preparation
tests pass. This modified experiment is not unmodified-map acceptance.

Each measured block calls the declared `SetRandomSeed`, captures eight ordered
`GetRandomInt` results, two `GetRandomReal` results, then two more integers.
Caller-selected seeds and bounds are synthetic experiment inputs, not defaults.
Four blocks use seed A, A, B, A. Results are stored before formatting/export.

Both actual runs show:

- All ten integer results match across the three A blocks.
- The B block differs from A; returning to A restores its measured sequence.
- The four records match across both fresh runs, including formatted real text.
- The generated output file is byte-identical: 759 bytes, SHA-256
  `633a85c6c282e14f99ae29f163e893fd52fd9cbd3c6ede67f5f775ebe7eba17e`.
- The records also appear visibly in the running original game. Captures were
  inspected; no generated preload script was executed to read the results.

The output was absent before each launch. Each run produced exactly one matching
file under its private Documents/Warcraft III/CustomMapData directory. Original
UI navigation, launch/source/map digests, generated records and captures remain
local under `.scratch/original-game-vm/rng-probe-{1,2}/`. The preparation and
cross-run receipts are in `.scratch/original-rng-map-probe-20261016/`.
No original map, script, generated value sequence or capture is distributed.

## What remains open

`R2S` provides formatted real observations, not bit-exact floating-point results.
This experiment neither identifies the generator nor observes all consuming
subsystems. It controls one serial block after entering `main`, not every random
choice made during configuration or loading. Equal probe output does not prove
equal world state, draw order, simulation clocks or animation phase.

The recreation's JASS gameplay Host still lacks these random natives. Its
synthetic simulation and menu-particle RNGs are separate implementation choices.
The paired UI replay therefore continues to reject a non-null seed instead of
claiming a launcher flag or a shared applied seed. The next step is an evidenced
recreation RNG contract and an actual paired map execution checkpoint. RE-Q6 and
RE-Q11 remain open; two fresh runs do not meet the 100-run baseline gate.
