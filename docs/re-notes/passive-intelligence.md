# Held intelligence and mana pools

## Source relationships

Human02 equips `ckng` after `prvt`. Its passive `AIx5` row has code `AIab`.
Selected `AbilityMetaData.slk` maps `Iagi` to Data column 1, `Iint` to column 2,
and `Istr` to column 3. The three amounts must be read through that metadata and
typed map overrides. `ciri` names `AIi6`, `rin1` names `AIi3`, and both use the
same intelligence column. `rwiz` instead names the mana-regeneration class
`AIrm`. Selected Misc data supplies the intelligence-to-mana and regeneration
relationships; the numbers below are witnesses, not production constants.

## Controlled original-runtime observation

The replacement script retains Human02 terrain, object data, upgrades and
destructables, but omits authored unit and trigger initialization. It creates
matched level-10 `Huth` holders and a separate `hfoo` with successfully admitted
`AInv`. Both heroes stay at full life to avoid healing-related mana expenditure.
Mana resets occur before each operation's before-marker, never during its
after-to-delay measurement interval. Time of day is fixed at noon.

- Reviewed client SHA-256:
  `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
- Map SHA-256:
  `79676598b4e35490847a10a63f0f07244d98b7338b2b41ec1e7b675facc5f62e`.
- Script SHA-256:
  `4471c7237004d6c0a5cc97fe4403a3d424e1f19d46585d62e24585b03433021f`.
- Launch uses `-classic`; typed `VersionGet` reports RoC.
- All 60 expected nine-part markers arrived. Raw files and receipts remain local
  under `.scratch/passive-intelligence-probe-r2/`. The owned run is stopped.

Immediate synchronous transitions show additive intelligence and maximum mana,
with current mana preserving its ratio and rounding to a whole point:

| Operation | Effective INT | Current / maximum mana |
|---|---:|---:|
| Before `ciri` | 33 | 123.750 / 495 |
| After `ciri` | 39 | 146 / 585 |
| Before adding `rin1` while holding `ciri` | 39 | 146.250 / 585 |
| After adding `rin1` | 42 | 158 / 630 |
| Before removing `ciri` while holding both | 42 | 157.500 / 630 |
| After removing `ciri` | 36 | 135 / 540 |
| Before `ckng` alone | 33 | 123.750 / 495 |
| After `ckng` alone | 38 | 143 / 570 |

`ckng` also raises observed strength and agility by five each. Its operation
restores full life before the after-marker, so it does not independently measure
the current-life adjustment policy. Transfers in this experiment explicitly
remove then add the item. `ciri` is accepted into the inventory-capable non-hero's
slot; that fixture does not record its mana or hero attributes, so acceptance
does not establish an intelligence effect on non-heroes.

## Intelligence and the mana-regeneration fraction

The matched control cancels common regeneration and timer quantization. Over
two-second intervals, the ordered-minus-control rates are approximately:

| Held items | Differential mana / second |
|---|---:|
| None | 0 |
| `ciri` alone, +6 INT | 0.300 |
| `rwiz` alone | 0.830 |
| `rwiz` and `ciri` | 1.280 |
| `ciri` after removing `rwiz` | 0.300 |

The additional intelligence contribution with the mask is therefore
`1.280 - 0.830 = 0.450`, versus `0.300` without it. This supports applying the
selected mask fraction to the intelligence regeneration contribution as well
as the existing base contribution. Native internal precision and rounding of
the integrated rate remain unqualified.

The initial analysis mistakenly used the re-add phase as an intelligence-only
comparison, although that phase still held `rin1` and had nine extra INT. The
valid comparison uses the first `ciri` pickup and the independent mask-removal
phase, both with six extra INT. The raw map, script and markers are unchanged.

## Death, revival and limits

At death, `ciri`, `rin1` and `ckng` remain held; effective INT stays 47 and maximum
mana stays 705. Current mana stays 180.697 through the two-second dead interval.
`ReviveHero` returns true, retains those items and attributes, restores full
life, and reads current mana 200 immediately afterward. The source of that
revival mana value is not established by this experiment and must not be copied
as a constant.

This run does not measure base-only `GetHeroInt(..., false)`, near-zero mana
rounding, edited amounts, direct held-item transfers, dead-holder item removal,
TFT, native Windows, or rendered parity. Pool changes and sequential cleanup
must be preflighted before item ownership or event writes, just as for strength
and maximum-life passives.
