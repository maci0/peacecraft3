# Invisibility and detection

Static observations from Warcraft III 1.29.2.9231, executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
No original-client runtime observation is claimed.

## Detection counters

`0x694750` increments two independent counters selected by the low two bits
of its type argument. The first increment sets the corresponding player's
mask bit. `0x688290` decrements them and clears the bit on the last removal;
an already nonpositive selected counter returns without decrementing it.
Its updates are sequential: a failed second decrement does not undo the first.
`0x6947c0` and `0x688310` wrap these operations on a unit's detection object.
`0x691040` returns the union of the masks selected by its type argument,
or zero without a detection object. These observations support overlapping
detectors rather than a boolean owned by the last detector to enter or leave.
See the `Adet` section of [passive held items](passive-held-items.md) for
the range registration, owner and asset field provenance.

## Per-player invisible query

The registration at `0x49dba6` names `IsUnitInvisible` and binds `0x499f50`.
The handler returns false for a missing unit or player. Otherwise it passes
the player's byte index and mode 4 to virtual slot `+0xfc` and negates the
result. CUnit's slot is `0x66d280`.

`0x66d280` calls `0x475150` and additionally accepts a player bit present
in the union of unit fields `+0x250` and `+0x254`. Those fields' writers
remain to be decoded. `0x475150` obtains the player's mask at `+0x3a0`.
Unless mode bit 1 is set it calls `0x4758d0`; unless mode bit 0 is set it
also checks positional fog. Thus the mode-4 invisible query still traverses
the positional fog path; replacing it with a raw invisible flag is unsupported.

`0x4758d0` accepts units without `[unit+0x164] & 0x1000000`.
For a flagged unit it accepts the owner bit in the supplied player mask,
or a mask intersection with `0x691040(unit, 1)`; otherwise it rejects.
The query therefore uses detection type bit 0, independently of type bit 1.
The player-mask expansion and fog semantics must be retained by any consumer.

## Ability entry points and open work

`CAbilityInvisibility` name getter `0xbf25e0`, code getter `0xbf25f0`
identifies `Aivs`. Its effect routine `0xbf2610` looks up `Binv` on the
target and either refreshes its virtual `+0x358` or allocates through
`0xbf3560`. This is a buff path, not an immediate permanent unit boolean.
`CAbilityShadowMeld` name getter `0xc43e40`, code getter `0xc43e50`
identifies `Ashm`; its level loader starts at `0xc43e90`.

`Apiv` code getter is `0xbf2810`. Its attach routine `0xbf2840` passes the
level's real field through `0x46f3a0` to unit routine `0x67a3c0`, then reads
the level's first data field through `0x46ed70`. If that field truncates to
zero, it sets movement-object flag `0x10000000` and remembers ownership of
that change in ability flag `0x80`. Detach `0xbf28b0` calls `0x69b3d0`
with the real at `0x112d88c` and clears the movement flag only when its
remembered flag is present. This supplies a timer-related unit path to trace;
it does not establish immediate invisibility on attach.

Still unverified: the `Apiv` unit timer routines, buff fade and break events, Shadow Meld's time
conditions, the two extra visibility masks, detector-holder disable flags,
and the attack acquisition and explicit targeting call modes.

This is a decode-only change. No counter implementation, ability admission,
targeting change or gem consumer is included. Existing unsupported paths remain
unchanged; the missing evidence above must be resolved before admitting them.
