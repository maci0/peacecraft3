# Player health handicap

Human02's easy-difficulty branch calls `SetPlayerHandicapBJ` for its enemy
players. The selected `Blizzard.j` supplies the easy percentage and converts
percentages to the factor accepted by `SetPlayerHandicap`. The current
recreation implements the separate XP handicap interface; health handicap
remains an open gameplay consumer.

## Original-client observations (2026-10-23)

Two Human02-derived diagnostic maps ran on the reviewed 1.29.2.9231 executable,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
with explicit `-classic` and a direct `VersionGet` readback of RoC. Authored
triggers and the ordinary creation entrypoints were omitted; the probe used
the source Blademaster creation/setup and selected ordinary-unit creation
inputs, matched other-owner controls, and an empty derived destructable file.
Map information, unit edits and terrain were retained. These are controlled
diagnostics rather than unchanged-chapter acceptance.

Both runs collected all 28 bounded, two-part markers. Wounded life was set and
recorded immediately before the handicap call in the same callback. The easy
factor read back as 0.6. Current/maximum life observations:

| Unit | First run, before → after | Half-life run, before → after |
| --- | --- | --- |
| Source level-2 `Obla` | 150/600 → 1/360 | 300/600 → 80/360 |
| `hfoo` | 105/420 → 63/252 | 210/420 → 126/252 |
| Other-owner hero and ordinary controls | 600/600 and 420/420 unchanged | 600/600 and 420/420 unchanged |

The hero's mana stayed 270/270 at those checkpoints. A fresh same-owner
footman, created after the setter, started at 252/252. Repeating the factor
left the sampled maxima unchanged. Ownership transfers into or out of the
handicapped player also left the sampled existing maxima unchanged. Restoring
the factor to 1 returned the target hero and footman maxima to 600 and 420.
Later current-life comparisons include regeneration between callbacks; they
do not isolate the current-life effect of repeat or ownership calls.

An isolated dead peasant retained its dead status while a positive current-life
write and the handicap change produced 55/220 → 33/132. Separate bound stimuli
show why the player's stored factor and unit-pool effects must be distinguished:

| Requested factor | Getter | Immediate peasant life/max | Delayed life/max |
| --- | --- | --- | --- |
| -0.5 | 0.01 | 0/-110, dead | -110/-110, dead |
| 0 | 0.01 | 0/0, dead | 0/0, dead |
| 1.5 | 1.5 | 330/330, alive | 330/330, alive |

These observations do not establish the upper clamp boundary or behavior of
repeated out-of-range requests. The integral ordinary-unit examples do not
distinguish raw from rounded ratio arithmetic.

## Native-code evidence and composition inference

The selected registration maps the setter to `0x4a95a0` and getter to
`0x494590`. They share the player lookup; the getter reads the factor field
that the setter updates. The setter compares the requested and stored factors,
visits units through callback `0x489f70` → `0x67af40`, and then clamps and stores
the player factor. Applying only the stored clamp to unit pools would therefore
lose observable behavior above.

The unit path reads a unit-data value, applies numeric operations, updates a
pool through `0x6be8c0` with mode 1, and optionally updates a hero component
through `0xb3fd00`. The selected default `Obla` record has base HP 100, initial
strength 18 and strength growth 2. Its source level-2 setup and the selected
strength-HP coefficient of 25 give a decomposition of 100 base HP plus 500
strength HP, matching the observed 600 maximum.

An ordered base-pool ratio update followed by a strength-pool delta explains
both hero samples: changing the base term first gives an intermediate maximum
of 560. At half life the intermediate current is 280; applying the strength
term's -200 delta gives 80. At quarter life the intermediate current is 140;
the same delta reaches the observed living floor of 1. This is a composition
inference supported by source rows, the native call sequence and two runtime
samples. It does not qualify every term, rounding step, held-item combination
or native internal representation. A single whole-pool multiplier fails the
observed hero cases.

## Receipts and limits

The private campaign worktree retains `.scratch/player-health-handicap-probe-r5/`
and `-r6/`, including maps, scripts, source hashes, packed-member checks,
protocols, raw markers and results. Each collector has six passing synthetic
checks, including missing/truncated output, factor-floor discrimination,
raw-versus-rounded pool ratios, timer-step coverage and fresh-creation order.

| Run | Diagnostic map SHA-256 | Script SHA-256 |
| --- | --- | --- |
| `hch-1023e` | `61802cd175e52ec9459913ede2073556b9593ba96d3b742a21d5c025be1139d7` | `ff0f228ca14a76a980b72b87f7d2a4684ecfc52c386055df161e35ec331dec2a` |
| `hch-1023f` | `5a14da4371d89f06d674cc84d5a1bb1d044fa087cdb20f1ea0286f90b8464321` | `90f1562bb1c291ace71106ae11fe084963fd279efa8ad45aefcde3d04d8ddd87` |

Earlier offline drafts remain preserved. Run `hch-1023c` showed a load-error
dialog and yielded no markers; renaming the probe's colliding global/function
identifier allowed the next map to load. Run `hch-1023d` then ended before the
setter because of a diagnostic timer-step gap. Neither supplies health-handicap
results. Parse/resolve success alone did not establish native script acceptance.

The reference clients are stopped. No health-handicap production implementation,
new rendered comparison or native Windows run was completed here. Alternate
hero attributes, non-integral pool rounding, held items, revival and the initial
factor's production admission remain open.
