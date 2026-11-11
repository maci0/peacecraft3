# Sentinel (`Aesn`, night elf huntress)

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Static disassembly only.

Status: identified, not implemented. `IssueTargetOrder` with `"sentinel"` refuses with a message
naming the class (`combat_host.rs`). Shipped use: RoC `NightElf03` `Trig_Setup_Sentinel_Actions`
orders three hidden `esen` huntresses (`ShowUnitHide`) to `sentinel` three trees through
`IssueTargetDestructableOrder` (`war3map.j:1236`, `1241`, `1246`). The return value is ignored by
the script.

## Class

- `CAbilitySentinel`: name string `0xf986a8`, getter `0xc4d1c0`, name slot `0xf98144`, vtable
  `0xf980ec`. Class code getter `+0x1c` (`0xc4d220`) returns `Aesn`.
- `+0x310` (`0xc4d270`) returns order id `0xd00d6`; `+0x314` (`0xc4d280`) returns flags `0x4108004`.
  `+0x21c` (`0xc4d2b0`) is true for order id `0xd00d6` only. The order registry's id for the string
  `sentinel` was not compared with `0xd00d6`.
- Companion classes: `CMissileSentinel` (allocator `0xc4d680`, name getter `0xc4d770`) and
  `CEffectSentinel` (allocator `0xc4d780`, code getter `0xc4d820` returns `Mesn`, name getter
  `0xc4d830`), by name the owl missile and the effect. A routine at `0xc4d8f0` compares an ability class code with
  `Xesn` (`0xc4d99f`), which matches the `EfctID1` `XEsn` of the row.
- Slot `+0x428` (`0xc4d350`) decrements `[this+0x128]` unless flag `0x80000` is set, increments
  `[this+0x40]`, sets `0x80` in its argument's flags (`[arg+0x20]`), builds a record whose float
  comes from the global at `0xe9ba10` and calls `0xc4e880`. Its role (the cast start, and what
  `[this+0x128]` counts) is not decoded.
- `0xc4d2f0` (`+0x1c4`) and the save and load routines (`0xc4d4e0`..`0xc4d610`, `0xc4da90`..
  `0xc4dc90`) were read only far enough to see they serialize the missile and effect state.

## Data (`AbilityData.slk`, alias `Aesn`, one level)

`Cast1=0`, `Cool1=120`, `Cost1=0`, `Dur1=0`, `HeroDur1=0`, `Rng1=800`, `Area1=-`, `DataA1=100`,
`DataB1=900`, `DataC1=275`, `DataD1=1`, `targs1=tree,vuln,invu`, `EfctID1=XEsn`. Authored art
(`NightElfAbilityFunc.txt`): `MissileSpeed=1500`, `Missileart=Units\NightElf\Owl\Owl.mdl`,
`Order=sentinel`; `XEsn` has `Targetart` the owl model attached `overhead`. What `DataA..DataD`
mean for this class was not decoded.

## Not decoded (stays refused)

- Target validation for the `tree,vuln,invu` target type through the order validation chain, and so
  the boolean the order returns.
- What happens at cast time: the owl missile flight, the effect it leaves on the tree, the vision it
  grants, its duration and the charge count (`DataD`?), and removal when the tree dies.
- Whether a hidden unit (`ShowUnitHide`) walks into range `800` and casts the same way as a
  visible one.
