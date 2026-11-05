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

## Bounded terminal-upload trace (2026-11-05)

Scope was a bounded REA trace ONLY of the terminal upload format
selection following `0x42b590`/`0x42b5f0` readers and `0x9721b0`
dispatch `0x9704b0`/`0x970280`, without repeating the options-writer
proof. Result: the terminal format still does not qualify. No
quantize/expand conversion is added; this section records the new
precise callsite state instead.

Exact callsite evidence (bytes read locally, no client executed):

- The Legacy 1.29.2.9231 image `3f2ed012…b959d59a` is not present on
  this machine. The only PE32 images under `peacecraft3` are the
  1.26.0 set (`game.dll` `6d21bd9a…`, `war3.exe` `28173101…`, etc.).
  No file hashes to `3f2ed012…`; the Legacy addresses therefore
  cannot be decompiled here without downloading or inventing an
  image, both out of scope.
- Read as RVAs in the local 1.26 `game.dll` (imagebase `0x6f000000`,
  `.text` `0x1000–0x86c60a`, `.rdata` `0x86d000–0xa4e1c4`), the Legacy
  addresses are not the claimed functions: `0x42b590` holds
  `5e5b83c430c20c00…` (`pop esi; pop ebx; add esp,0x30; or al,0xc0`),
  `0x42b5f0` holds `0000002bc8894c24…`, `0x538440` holds
  `0000005ec20400…`, and `0x9721b0`/`0x9704b0`/`0x970280` lie in
  `.rdata` and hold data/strings — e.g. `0x9721b0` holds
  `000000002e5c426174746c654e657443…` (`.\\BattleNetC…`). They are
  not a texture-creation dispatch in this image.
- In the local 1.26 `game.dll` the descriptor table sits at file
  offset `0x875588` (16-byte rows), with `texquality` at `0x875628`,
  `miplevel` at `0x875638` and `texcolordepth` at `0x875648`
  (fallback `"16"`, matching the 1.29 note's depth value but at a
  different base than `0xe68b80`). No direct code xref to the
  `texcolordepth` row exists in `.text`; the only immediate
  reference to the table base is at `0x29b9`
  (`fld [0x6f875588]`), and the `push 0x0b; call` readers at e.g.
  `0x1510–0x1526` go through an import thunk at `0x6eb5a0`
  (`jmp [0x6f86d570]`), identified as `Storm.dll` ordinal `0x12c`
  (300). `Storm.dll`'s export table has no name for ordinal 300,
  so the read is ordinal-only and cannot be followed to a D3D
  upload call within the bounded scope.
- No `D3DCreateTexture`/`CreateSurface` format argument
  constant/table and no alpha-dependent branch were isolated in
  either image. The 16-bit texcolordepth word still establishes
  only bits-per-pixel (16 = 2 bytes), not a pixel layout.

Implication: the unclosed callsite is now precise — the Legacy
`3f2ed012` image is required and absent; on the locally available
1.26 image the `texcolordepth` read is via `Storm.dll` ordinal 300
with no onward upload trace. Guessing RGB565/1555/4444 remains
unsupported. `one_mip` and `derive_texture_quality` keep refusing
2-byte depth fail-closed.
