# Sentinel (`Aesn`, night elf huntress)

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Static disassembly only; no
original client was run.

Status: the order is accepted for the case the shipped map uses; the missile flight and the
in-flight sight are not modeled (below). Shipped use: RoC `NightElf03` `Trig_Setup_Sentinel_Actions`
orders three hidden `esen` huntresses (`ShowUnitHide`) to `sentinel` three trees through
`IssueTargetDestructableOrder`, a Blizzard.j wrapper that calls `IssueTargetOrder`
(`war3map.j:1236`, `1241`, `1246`). The script ignores the return value.

## Class

- `CAbilitySentinel`: name string `0xf986a8`, getter `0xc4d1c0`, vtable `0xf980ec`. Class code
  getter `+0x1c` (`0xc4d220`) returns `Aesn`. `+0x310` (`0xc4d270`) returns order id `0xd00d6`;
  `+0x314` (`0xc4d280`) returns flags `0x4108004`. `+0x21c` (`0xc4d2b0`) is true for order id
  `0xd00d6` only. The order registry's id for the string `sentinel` was not compared with
  `0xd00d6`.
- Companion classes, each with an allocator, a code getter and a vtable:
  - `CMissileSentinel`: allocator `0xc4d680`, constructor `0xc4e1a0`, vtable `0xf98544`, code
    `Mesn`.
  - `CEffectSentinel`: allocator `0xc4d780`, constructor `0xc4e100`, vtable `0xf98614`, code getter
    `0xc4dd50` returns `Xesn`.

## Data meaning (`AbilityData.slk` row `Aesn`, one level)

`Cast1=0`, `Cool1=120`, `Cost1=0`, `Dur1=0`, `Rng1=800`, `Area1=-`, `DataA1=100`, `DataB1=900`,
`DataC1=275`, `DataD1=1`, `targs1=tree,vuln,invu`, `EfctID1=XEsn`. The shipped
`UI\WorldEditStrings.txt` names the fields (`AbilityMetaData.slk` rows `Esn1` to `Esn4`, data
indices `A` to `D`): `DataA` "In Flight Sight Radius", `DataB` "Hovering Sight Radius", `DataC`
"Hovering Height", `DataD` "Number of Owls". Authored art (`NightElfAbilityFunc.txt`):
`MissileSpeed=1500`, `Missileart=Units\NightElf\Owl\Owl.mdl`, `Order=sentinel`.

## Rules (observed in code unless marked)

- Level hook `0xc4d230` (vtable `+0x80`): `[this+0x128]` (owls left) is `DataD` read through
  `0x46ef80` (level record `+0x2c`, same record as `DataA` `+0x20`, `DataB` `+0x24`, `DataC`
  `+0x28`) and truncated by `0x9a07b0`. A value of zero sets flag `0x80000` of the ability.
- Cast slot `+0x428` (`0xc4d350`), argument the target widget: with `0x80000` clear it decrements
  `[this+0x128]`; at zero it increments `[this+0x40]` and calls `0x46e070` (not decoded). It sets
  state bit `0x80` on the target (the generic modified bit, not a vision flag), creates the
  missile `Mesn` (`0xc4e880`) and configures it (`0xc4e9e0`) with the values `DataA`, `DataB`,
  `DataC` and the constant `0x12c` (300, through `0x9a1620`; its role is not decoded).
- `0xc4e9e0` also creates the effect `Xesn` (`0xc4e780`) with the caster's owner (`+0xec` of the
  caster object) and `DataA` as its sight radius (the effect's value holder `+0x2c`, whose value
  `0xb7e710` reads at `+0x30`). The missile keeps `DataB` at `+0xa8` and `DataC` at `+0xb0`.
- Missile hit slot (`0xc4d8f0`, vtable `+0xbc`, called from `0x6e6f6c`): the target resolves by
  handle; when the target widget's dead test (`+0x13c`, `0x664bb0`: life at or below the zero
  constant) is false, the effect's radius becomes `DataB`, then `0xc4e4f0` attaches the effect to
  the target with height `DataC`, and the missile is destroyed. A dead target gets no effect.
- Attach (`0xc4e4f0`): stores the target handle at effect `+0x38`, positions the effect at the
  target through slot `+0x88` (`0xb7e710`), which creates a sight area of the effect's radius at
  that position (`0x800070`, events `0xd01b4` enter and `0xd01b5` leave handled by `0xb803a0` and
  `0xb80e40`), subscribes the effect to the target's events `0xd01e6`, `0xd01a0` and `0xd01e7`
  (`0x6c0040`, `0x6bffb0`, `0x6bffe0`), and attaches the owl model overhead by `DataC`.
- Effect end: its event slot (`0xc4de30`) destroys the effect on `0xd01a0`, `0xd01e6` and
  `0xd01e7`. The destructable life setter (`0x6cee60`, vtable `+0x124`) posts `0xd01e6` after
  every life write; the widget destructor (`0x668cb0`) posts it too. No timer ends the effect
  (`Dur1=0`). Inferred: any tree damage ends the owl, not only its death.
- Inferred, not traced to the fog stamp: the sight area reveals terrain and units like a unit's
  sight area.

## Recreation (`sentinel_host.rs`)

Accepted when the caster owns an `Aesn` ability, the target is a live, shown destructable whose
catalog `targType` lists `tree` and whose `targs1` lists `tree` and its stance (`invu` or
`vuln`), and the tree lies within `Rng1` by center distance. The cast spends one owl of `DataD`
and attaches a sight circle of radius `DataB` at the tree for the caster's owner (and players
sharing vision with it). Every destructable life write ends the tree's owls.

Left out, and why:

- Missile flight time and the in-flight sight (`DataA`): the missile movement and its launch
  point (`0x6e6650`, `0x6e5430`) are not decoded, so the owl attaches at the cast.
- Hidden casters: the order on a `ShowUnitHide` unit is accepted like any other; whether the
  original casts the same way was not traced.
- Refused with a message: a target that is not a live, shown tree (including dead trees, whose
  validation the data does not settle), a tree beyond `Rng1` (the approach), and a Sentinel with
  no owls left (flag `0x80000`).
- `Cool1` (`120`) is not applied: the cast slot starts no cooldown, and the generic cast wrapper
  that might was not decoded.
