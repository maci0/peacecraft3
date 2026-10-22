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

### Held life and strength bonuses

A third diagnostic retained the half-life setup and equipped the source
Human02 `prvt` item on the target Blademaster before the baseline. The script
checked slot 0's item type and marked the run unqualified if the item was not
held. All 28 markers qualified on RoC. The target's baseline was 750/750,
while the matched other-owner hero remained 600/600. The same-callback
wounded and handicap snapshots were **375/750 → 155/510**. Mana stayed
270/270, and the ordinary-unit control repeated 210/420 → 126/252.

The 150-point held-life contribution remains in the new maximum: the earlier
unmodified hero reached 360, and this holder reached 510. Scaling the whole
750 maximum by 0.6 would incorrectly yield 450. The ordered composition
described below also explains current life: the base-term change gives an
intermediate maximum of 710 and current life 355, then the strength-term
delta of -200 yields 155. This supports that composition for the tested item;
it does not establish every passive ability or item removal after a handicap.

A slot-verified `ckng` variant distinguishes held strength from flat life.
Its source ability grants five points to each hero attribute. The target's
baseline was life 725/725 and mana 345/345. The same-callback snapshots were
**362.5/725 → 93/435**, with mana unchanged. All 28 markers qualified on RoC.
The held strength contribution therefore participates in the tested handicap
change, unlike `prvt`'s flat life contribution. The ordered reconstruction
predicts an intermediate maximum of 685, rounded current life of 343, then a
strength-contribution delta of -250 giving 93/435. Pickup, removal and attribute
changes after applying the handicap still need their own qualification.

## Native-code evidence and composition inference

The selected registration maps the setter to `0x4a95a0` and getter to
`0x494590`. They share the player lookup; the getter reads the factor field
that the setter updates. The setter compares the requested and stored factors,
visits units through callback `0x489f70` → `0x67af40`, and then clamps and stores
the player factor. Applying only the stored clamp to unit pools would therefore
lose observable behavior above.

The maximum operand at `0x4a9636` references global `0x112d854`. Its initializer
at `0x401350` supplies integer 100 through the same numeric conversion helper
already admitted by the XP-handicap reader. The null-player getter references
`0x112d810`, initialized from integer 1 at `0x401460`. A property-reset path
at `0x4bdf31` initializes the `+0x358` property from that same global; the
getter reads its `+0x35c` value. These are native operand relationships, not
values inferred from zero-filled on-disk globals. The complete set of callers
of the reset path has not been established. The local checked receipt is
`.scratch/player-health-handicap-native/initialization.json`.

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
| `hheld-23b` | `409c7b5025220c67854d03e38b664f3b3468ba793a68bff8546bb1561cea7b9d` | `a355ecb77923ee790fa30793993578b4e0f8691c1b355854cdc90326b75b10ac` |
| `hstr-23a` | `5ffeccea387ccc2567208a690b26e5af1580695843e41ce3fa7f6652c53ba95e` | `c6700b511681e4ba7db5c8aafd7ff1d11624c58cefffc6e9bec68616308e5646` |

The held-item run is retained in
`.scratch/player-health-handicap-held-probe-r2/`, including the inventory
check, packed-member hashes, raw markers and result. Seven synthetic checks
pass, including the inventory qualification and earlier protocol checks. Its
earlier variant was staged but never launched because its helper's non-null
item return did not prove inventory ownership.
The strength variant is retained under
`.scratch/player-health-handicap-strength-probe/`, with seven passing synthetic
checks and the same inventory qualification. Its client run is also stopped.

Earlier offline drafts remain preserved. Run `hch-1023c` showed a load-error
dialog and yielded no markers; renaming the probe's colliding global/function
identifier allowed the next map to load. Run `hch-1023d` then ended before the
setter because of a diagnostic timer-step gap. Neither supplies health-handicap
results. Parse/resolve success alone did not establish native script acceptance.

## Implementation status

`SetPlayerHandicap` and `GetPlayerHandicap` now use the source-qualified setter,
getter, initializers, reset, rounder, floor, threshold and minimum operands. The
setter applies its raw requested factor to existing owned units before storing
the clamped player factor. Base HP uses the ratio update; the strength HP term,
including held or permanent strength, follows as a direct pool delta; flat held
maximum-life bonuses stay unscaled. Life rounding is read from the selected
rounder (bias and floor routine), including the strength-item delta path, rather
than fixed at half-up. Unit creation uses the owner's stored factor, while
ownership transfer retains both the fitted factor and current maxima.

The live-owner requested factor is validated once before unit projection, so a
non-finite request refuses even when the owner has no vital records. Before
commit, owned units are regenerated read-only through the current clock and the
resulting pools are projected with the health factor. A projection that would
make a maximum nonpositive, or turn a living pool into zero, now refuses
atomically with no unit or player-row change because the death transition is not
reconstructed. This is a deliberate support boundary, not native behavior: the
observed client lets zero and negative factors drive zero or negative pools and
death. The previously accepted zero/negative fixture was replaced by atomic
refusal tests; native negative-pool/death semantics remain unimplemented.
Requests within the source threshold neither change pools nor store a new factor.

Runtime tests cover existing units, threshold repeats, fresh units, flat held
life, destructive refusal, empty-owner non-finite refusal, ownership transfer,
image-derived item-delta rounding and scaled powerup/level strength gains.
Production campaign startup binds the health windows alongside the XP windows.

Remaining open questions are the native death/revive transition for destructive
factors, alternate hero attributes, other held-item combinations, and the native
reset path's wider caller set. This work did not run a rendered comparison or a
native Windows client.
