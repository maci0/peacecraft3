# UnitSetUsesAltIcon

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Native
code evidence only (static disassembly). No original-client run.

## Registration and handler

- Name at VA `0xe729dc`, signature `(Hunit;B)V` at `0xe71b0c`, registration row
  at `0x49df53`. Handler `0x4af800`.
- The unit comes from `0x483940`. A null or removed unit returns with no write.
- A nonzero flag ORs `0x200` into the dword at unit `+0x48c`. A zero flag clears
  that bit (`andl $0xfffffdff`).

## Readers of bit 0x200

The disassembly holds one direct `testl $0x200, 0x48c(...)` reader, at `0x65be9b`.
It sits in a minimap-style per-unit skip test with `+0x164 & 0x40000` and
`+0x168 & 1` (see `campaign-minimap.md`, where that test is listed as not
decoded). Whether it is the alt-icon presentation is unverified.

Other reads of the word at `+0x48c` in `.text` test other bits. No command-card,
portrait or selection-panel reader of bit 0x200 was found.

## Status

The state is recorded on the executed unit (`ExecutedUnit::uses_alt_icon`,
`unit_alt_icon_host.rs`). The recreation has no presentation consumer for it.
The command-card icon choice and the portrait are unchanged by this native.

## Chapters that call it

Scan of every campaign map's `war3map.j`: TFT `HumanX01`, `HumanX02`, `HumanX05`,
`UndeadX08`, `OrcX01` (and variants `_02` to `_05`), `OrcX02` (and variants `_02`
to `_10`). The script declares it as `native UnitSetUsesAltIcon takes unit
whichUnit, boolean flag returns nothing` in `Scripts\common.j`.
