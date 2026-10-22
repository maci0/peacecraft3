# Held maximum-life bonus

## Source admission

Human02 equips Uther with `prvt` after `rwiz`. The selected item row names
`AIlf`; that ability's code is `AIml`, with one level and integer `DataA1`.
`AbilityMetaData.slk` row `Ilif` maps the amount to Data column 1. The item text
references that same ability field. The reviewed registration window at
`0xbc9f40` registers `AIml` against `APbo` through `0x7f6360`.

These relationships must be parsed from the selected tables, metadata, image
and typed map overrides. The stock amount observed below is a witness, not a
production constant. Intelligence bonuses remain a separate unsupported class
of effect within the otherwise admitted `AIab` attribute ability.

## Original-runtime observation

The local controlled map keeps Human02 terrain and object data and replaces
script execution with matched level-10 `Huth` units and an `hfoo`. Authored unit
and trigger function definitions remain in the script, but their initialization
calls are omitted. The non-hero receives the source-confirmed `AInv` capability;
both its successful admission and actual held slots are recorded.

- Client SHA-256: `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
- Map SHA-256: `036ce7c4c5ac7839a83811588a299414ce88f7f3e838befe881fd9a2de6dab99`.
- Script SHA-256: `4301ae20ba689bdebbc09a91904bf1c30f487cb5d255b6c6fef525f6ebd5d2a3`.
- Launch uses the verified `-classic` selector; typed `VersionGet` reads RoC.
- All 51 expected markers arrived. Receipts and raw markers remain local under
  `.scratch/passive-max-life-probe/`; the owned reference run is stopped.

Each held item adds 150 maximum life in this source generation, including on the
non-hero with inventory. Two items stack. Removal reverses the maximum change.
Immediate current life follows the old current/maximum ratio and rounds to a
whole hit point, rather than preserving the absolute missing amount:

| Holder/action | Before current/max | After current/max |
|---|---:|---:|
| Hero pickup | 315.050 / 1250 | 353 / 1400 |
| Hero second pickup | 355.550 / 1400 | 394 / 1550 |
| Hero removal | 396.550 / 1550 | 358 / 1400 |
| Non-hero pickup | 106.250 / 420 | 144 / 570 |
| Non-hero removal | 144.250 / 570 | 106 / 420 |

Before/after records surround synchronous calls; the matched control's life and
maximum do not change inside those transitions. Between samples there are other
health changes, so this experiment does not isolate regeneration or healing.
Transfers explicitly remove from one holder and add to the other; direct transfer
of an already-held item is not established by this experiment.

## Native arithmetic and remaining boundary

The existing strength path reaches `0x6be8c0` mode 1. At `0x6bea8e` it multiplies
the retained ratio by the new maximum, then calls `0x9a1d70`. That helper adds the
float at `0x10db0a8` (`00 00 00 3f`, one half) through `0x9a01c0`, then calls the
floor helper `0x9a1520`. For positive values this supports nearest rounding with
half upward, consistent with the runtime samples. It also rounds the new maximum
before selecting the mode. The current strength recreation's unrounded ratio
therefore needs attention when sharing pool adjustment with this effect.

No death, revival, near-zero-life clamp, edited amount, native Windows or rendered
parity is qualified here. Implementation must preflight every pool adjustment in
the actual removal order before detaching an item or queuing its drop event.
