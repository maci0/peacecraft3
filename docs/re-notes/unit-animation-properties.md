# AddUnitAnimationProperties

Image: Legacy `Warcraft III.exe` 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`. Native
code evidence only (static disassembly). No original-client run.

Status: decoded in part. The native stays refused as unsupported. The list
mechanics below are decoded, but the link between the list and the sequence
selector's slot 0 is not, so an implementation would bake in an unverified
assumption (see Open).

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
- After each token the handler makes a virtual call at `+0x8c` with the result
  of `0x692830(0, 1)`. Neither is decoded.

## List on the unit

Count at `+0x4e4`, tags at `+0x4ec + 4 * index`, capacity 6. The routine around
`0x5dd6f3..0x5dd743` zeroes the count and writes 1.0 into `+0x4ec` and `+0x4f0`.
That routine is not confirmed as the unit constructor, and the 1.0 values are not
read as tags while the count is zero.

- Add (`0x67a900`): if the tag is in slots `0..count`, return 0. If `count >= 6`,
  return 0. Otherwise append and return 1.
- Remove (`0x69b800`): if `count <= 1`, return 0. Search slots `1..count`. On a
  hit at `k`, copy the last slot into `k`, decrement the count, return 1. Slot 0
  is never removed, and removal does not keep order.

## Selection

- `0x666a20` stores its first argument into slot 0 (`+0x4ec`) without changing
  the count. It then calls `0x6a2910` with the count.
- `0x6a2910` forwards `(value, list, count)` to the sequence chooser `0x966c10`
  when the model pointer at `+0x28` is set.
- `unit_animation_rarity.rs` decodes the chooser's inputs. It scores each
  sequence by matching the requested name tokens and the property tags. The
  native list is the property-tag input.

## Open

1. Slot 0. `0x666a20` writes the requested value into slot 0 without changing the
   count, and `0x67a900` appends at `count`. The first added tag therefore sits in
   slot 0, which the next `0x666a20` write may overwrite. Whether a Set call's name
   tag occupies that slot, and what happens to a tag added first, is not decoded.
   The caller of `0x666a20` (a virtual slot, no direct call found) is not
   identified.
2. The `+0x8c` refresh after add and remove: not decoded. It may re-select the
   current clip immediately.
3. Whether explicit `SetUnitAnimation` consults the list. The recreation's
   explicit Set uses source order; its engine activity path consumes Animprops
   metadata only.
4. Token rules: quote handling in `0x434460`, empty tokens (for example leading
   or doubled spaces), and tokens of 32 or more bytes are not decoded.

## Consumers in the campaign set

Human07 (RoC) adds and removes `gold` and `lumber` on peasants
(`war3map.j:1776` onward, 7 calls). Orc03 (RoC) removes `ready` from ogre units
(4 calls). Human08 (RoC) and HumanX03Secret, NightElfX06 (TFT) also call it.
The call sites come from `AddUnitAnimationPropertiesBJ`.

## Refusal boundary

Until open item 1 is decoded, `AddUnitAnimationProperties` refuses as an
unsupported native. Implementing only the list would make a first add either
persist or vanish on the next animation set, and the campaign effect for
`lumber` and `gold` would be unverified in both directions.
