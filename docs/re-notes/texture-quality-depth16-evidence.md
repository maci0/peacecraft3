# Texture Quality 16-bit depth: native consumer evidence

Legacy executable SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
(VERSIONINFO 1.29.2.9231), disassembled locally, no client executed.
This note records what the shipped bytes do and do not establish about the
16-bit `texcolordepth` consumer. It does not add a conversion.

## What is established

Options write routine `0x538440..0x53882f` writes `texquality`, then:

- `miplevel` (id `0x0a`) at `0x53865c`: q0 writes word `1`, others `0`.
- `texcolordepth` (id `0x0b`) at `0x538681`: q2 writes word `32`, others `16`.

Startup writer `0x428682` independently confirms (`cmp edi,2; cmove`).
Either way 16 is decimal bits, i.e. 2 bytes/pixel. The descriptor table
row for id `0x0b` is at `0xe68b90` (base `0xe68b80` + `0x0b*16`); its
name pointer is `0xe69144` (`fixedaspect`), and the string at `0xe69188`
is `texcolordepth`, with `miplevel` at `0xe6917c`. The getter pair
`0x42b590` (string) / `0x42b5f0` (word) dispatch on these ids; the word
getter converts the fallback via the same `atoi`-style helper.

## What is not established

- No pixel layout (RGB565, ARGB1555, ARGB4444, BGR565, …) is named by
  the write routine, the descriptor row, or any of the readers examined.
  16 bits = 2 bytes does not establish a colour/alpha arrangement.
- Tracing the `0x0b` readers through the option-object getters reaches
  D3D-adjacent/wrapper layers and the BLP path, but the terminal
  surface/pixel-format creation site that would name the 16-bit storage
  was not isolated in this pass. The BLP pipeline itself is unaffected:
  BLP1 ships indexed palettised and JPEG content; the native path
  presents RGBA-style pixels to the device. A 16-bit GPU store, if any,
  happens at upload, not in the BLP bytes.
- Consequently no source-qualified 16-bit quantization/expansion to
  RGBA8 can be stated without inventing channel order, bit allocation,
  alpha semantics, rounding and byte order.

## Decision

`pcx_gfx::one_mip` keeps refusing the 2-byte depth fail-closed, and
`derive_texture_quality` refuses any depth word other than 16/32.
Wiring a guessed 565/1555/4444 path behind the Texture Quality control
would be inventing a pixel format. The exact consumer function that
names the format (the D3D surface-creation call that receives the depth
word on the texture-upload path) remains the unclosed evidence; until
it is isolated, the unsupported-format consumer is recorded here rather
than implemented. The ordinal bound is no longer a crate-local constant:
`derive_texture_quality` takes a caller-supplied `domain` measured from
the actual `TextureQualityMenu` FDF `MenuItem` records by the menu layer
(which owns the FDF parser), so pcx-gfx never copies the popup count.
Synthetic tests prove a non-four domain (3 and 6) bounds the ordinal;
the original-image gate reads the real source count from the selected
install's Options FDF. The remaining menu-consumer hook is unchanged:
the UI layer must parse the popup choices, pass their length as
`domain`, and call `derive_texture_quality` + `one_mip` on selection —
no menu wiring is added here.
