# AddUnitAnimationProperties

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Native
code evidence only (static disassembly). No original-client run.

Status: list mechanics and the slot 0 question decoded, native implemented
(`Rules::edit_properties` in `unit_animation_rarity.rs`, `add_unit_animation_properties`
in `unit_host.rs`). Not reproduced: the immediate re-selection of the playing
animation (see Slot 0 and refresh), and the token cases listed under Open.

## Registration and handler

- Name at VA `0xe71f78`, signature `(Hunit;SB)V` at `0xe71f6c`, registration
  row at `0x49d662`. Handler `0x489d00`.
- The unit comes from `0x483940`. A null or removed unit returns with no work.
- The string cursor comes from `0x469010`. The loop runs while the cursor's
  first byte is nonzero.
- Each token comes from `0x434460`, a quote-aware tokenizer. Its buffer
  capacity is 32 (`push 0x20`). Its delimiter argument points at the one-byte
  string `" "` at VA `0xe6a244` (a single space).
- Each token maps through `0x6dafc0(token, 4)`: a case-insensitive lookup in the
  registry table at `0x113f344`, built from the registration list at `0x6db0e0`
  (the same registry `unit_animation_rarity.rs` decodes). Unknown tokens return
  the default `4`.
- `boolean add` nonzero calls `0x67a900` (add). Zero calls `0x69b800` (remove).
- After each token the handler calls the unit's virtual `+0x8c` as
  `(0x692830(), 0, 1)` (stack `1, 0, value`, `this` = the unit; the argument
  order is read from the pushes at `0x489d80..0x489d89`). `0x692830` returns the list's
  slot 0 when the count is nonzero, else 0. The call result (an x87 float) is dropped.
- The unit is `CUnit`: vtable `0xe9c504` (name slot `0xe9c55c`, getter `0x66edc0`
  for the string at `0xe9d998`), written by the constructor `0x6756d0` at
  `0x67572e`. Its vtable slot `+0x8c` holds `0x666a20` (data word at `0xe9c590`).

## List on the unit

Count at `+0x4e4`, tags at `+0x4ec + 4 * index`. The CUnit constructor `0x6756d0`
calls `0x6a02f0` at `0x675d37`; that routine stores count `1` at `0x6a034b`, so slot
0 is reserved from creation (and stores `1.0f` at `+0x50c`). The constructor also
writes `6` to `+0x4e8` (`0x675cc8`); add compares the count with the immediate `6`
(`0x67a92c`, `cmp esi, 6`), not with that field. The earlier `0x5dd72f` routine, which
zeroes a count at the same offset and writes `1.0f` into `+0x4ec`, belongs to a
different class (floats at those offsets) and is not the unit list.

- Add (`0x67a900`): if the tag is in slots `0..count`, return 0. If `count >= 6`,
  return 0. Otherwise append and return 1.
- Remove (`0x69b800`): if `count <= 1`, return 0. Search slots `1..count`. On a
  hit at `k`, copy the last slot into `k`, decrement the count, return 1. Slot 0
  is never removed, and removal does not keep order.

## Slot 0 and refresh (decoded)

- `0x666a20(value, arg2, flag)` stores `value` into slot 0 and leaves the count
  unchanged. When `flag` is nonzero it first calls the unit's virtual `+0x98`. It
  then calls `0x6a2910`, which forwards `(value, list, count, arg2)` to the sequence
  chooser `0x966c10` when the model pointer at `+0x28` is set. The chooser queues the
  newly chosen sequence on the model (`0x9632b0`), so this is a real re-selection.
- The native passes the current slot 0 back as `value`. The native never changes
  slot 0, and slot 0 is only changed by the animation selector writing the requested
  primary tag. The caller that sets the primary is a virtual call through the same
  slot `+0x8c`; the callers in the unit animation code were not identified.
- The count starts at 1, so the first added tag lands in slot 1 and is not lost by a
  later selector write. The earlier suspicion (first add in slot 0, overwritten by
  the next selection) is refuted: slot 0 is reserved from the constructor.
- Duplicate check: add compares the tag against slots `0..count`, including slot 0.
  Adding a tag equal to the current primary tag is therefore skipped. Remove never
  looks at slot 0.
- Slot 0 before the first selector write is not initialized by the constructor chain
  read here (`0x46d010` and `0x46d0d0` zero other fields; `0x6a02f0` does not write
  `+0x4ec`).
- `unit_animation_rarity.rs` decodes the chooser's inputs. It scores each
  sequence by matching the requested name tokens and the property tags. The
  native list is the property-tag input.

## Recreation

- `Rules::decode` reads from the image: the native's delimiter string (`0x489d48`,
  must equal the rarity input delimiter), token buffer `0x20` (`0x489d4d`), unknown
  token default `4` (`0x489d5f`, must equal the rarity fallback), capacity `6`
  (`0x67a92e`) and the reserved slot 0 count `1` (`0x6a0351`). Decode fails if any differs.
- The unit's list is `Animation::engine_properties` (the form's authored list until the
  first edit). `edit_properties` keeps one token per entry for slots 1 and up. Add skips a
  tag already listed, a tag equal to the primary tags, and any add at capacity; remove swaps
  the last entry into the hole. The explicit rarity selection and the engine selection both
  read this list.
- Slot 0 is not tracked. The tags of the current sequence's name stand in for it, so an add
  whose tag equals any tag of the playing sequence name is skipped (a superset of the real
  rule, which compares the requested primary tag only).
- Refused with a message: quoted text (the tokenizer `0x434460` is quote aware, not
  decoded), empty tokens (leading, trailing or doubled spaces), tokens of 32 or more
  bytes, non-ASCII text, a unit with no animation state.
- Not reproduced: the re-selection after each token. The new list takes effect at the
  next selection. A unit whose clip would change at once in the original keeps its
  current clip until the next selection here.

## Open

1. The primary tag written to slot 0 by the animation selector (callers of virtual
   `+0x8c` in the unit animation code), and slot 0 before the first selection.
2. The re-selection after each token (`+0x8c` with flag 1, virtual `+0x98`): its
   effect on the playing clip is not modelled.
3. Whether explicit `SetUnitAnimation` consults the list. The recreation's explicit
   Set uses source order; `SetUnitAnimationWithRarity` and engine selection consume it.
4. Token rules of `0x434460`: quotes, empty tokens, tokens of 32 or more bytes.
5. Population of the list from the unit's authored `Animprops` at creation (the
   recreation starts from the authored list).

## Consumers in the campaign set

Human07 (RoC) adds and removes `gold` and `lumber` on peasants
(`war3map.j:1776` onward, 7 calls). Orc03 (RoC) removes `ready` from ogre units
(4 calls). Human08 (RoC) and HumanX03Secret, NightElfX06 (TFT) also call it.
The call sites come from `AddUnitAnimationPropertiesBJ`.

