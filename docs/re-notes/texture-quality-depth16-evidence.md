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

## Provenance correction: qualified image present (2026-11-05)

The preceding section's "not present on this machine" claim is wrong for
this workspace. The exact qualified ordinary asset path was re-checked
with normal file tools (no broad search, no alternate-version fallback):

- Path: `/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)/Warcraft III.exe`
- Size: 14858960 bytes; SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
- PE32, ImageBase `0x400000`, entry RVA `0x6574bc`, sections
  `.text` RVA `0x1000` size `0xa64941`, `.rdata` RVA `0xa66000`, etc.

The earlier 1.26 `game.dll` trace above is kept as separate cross-version
evidence only. The 1.26 addresses do not qualify the 1.29 policy.
All addresses in this correction are VAs in the qualified image
(RVA = VA - `0x400000`), verified against rizin functions, e.g.
`fcn.0042b590`, `fcn.009721b0`.

Descriptor correction: the descriptor table actually starts at VA
`0xe68bd0` (row 0 = `reswidth`, fallback `800`), not `0xe68b80`. Rows
16 bytes apart: `reswidth` id 0 at `0xe68bd0`, `resheight` 1, `colordepth`
2, `adapter` 3, `refreshrate` 4, `gamma` 5, `fixedaspect` 6,
`modeldetail` 7, `animquality` 8, `texquality` 9, `miplevel` 10
(`0x0a`), `texcolordepth` 11 (`0x0b`) at `0xe68c80`. Its name pointer is
VA `0xe69188` (`texcolordepth`), fallback at `0xe68c80+12` = `16`.
`fixedaspect` string is at `0xe69144`, `miplevel` at `0xe6917c`, matching
those VAs. The earlier "row at `0xe68b90`" was an off-by-`0x50`
misalignment; the string/fallback values are unchanged. Fallback `16`
is decimal (16 bits), not a pixel layout.

## Bounded actual upload format trace from qualified image only

No guessed RGB565/alpha assignment; trace below names only constants
and calls read from the qualified bytes. No client executed, no original
bytes committed.

- Writers (qualified image): options writer at VA `0x538681` pushes
  `0x0b` then calls setter `0x42ca10` with word `16` (32 only when the
  preceding selector equals 2). Startup writer at VA `0x428682`:
  `mov ecx,32; cmp edi,2; mov eax,16; cmove eax,ecx; push eax; push 0x0b;
  call 0x42ca10`. Both write depth only.
- Live consumer: `0x42aab0` is the apply-reader. It bounds id `< 0x50`,
  reads `descriptor[id].kind` at `0xe68bd8+id*16`, calls string getter
  `0x42b590` for kind 1 and word getter `0x42b5f0` for kind 0, then
  calls dispatcher `0x42ab70`. For id `0x0b`, kind is 0, so
  `0x42b5f0` returns the stored word (16 or 32).
- Dispatcher `0x42ab70` maps id via `byte[id-5+0x42b168]`; id `0x0b`
  maps to jump-table entry 4 at `0x42b0f4`, target VA `0x42abeb`. There:
  depth must be `16` or `32`, else return 0. It calls Gx getter
  `0x7d0ad0` (returns device+`0xe8`), then calls
  Gx setter thunk `0x7d0a70` with 0 when depth is 16 (only if getter
  nonzero) or with 1 when depth is 32 (only if getter zero). `0x7d0a70`
  is `GxDevice vtable+0x20` with that 0/1 arg. Base setter
  `0x7d3280` stores the arg to device+`0xe8`; the OpenGL and D3D
  overrides (`0x7d5320`, `0x7db5a0`) call the base then the
  device-specific re-upload walk (`0x7da6e0` / `0x7df200`), which walks
  the texture list and calls `0x7d47b0`. So `texcolordepth` sets a
  0/1 device flag at +`0xe8`; it is not itself a GL enum.
- Texture creation path (`0x9721b0` -> `0x9704b0`/`0x970280`) does not
  read `texcolordepth`. BLP path at `0x970458` and TGA path at
  `0x970646` call `GxDevice vtable+0xa8` (`0x7d11f0` thunk). The format
  argument there comes from the BLP type switch at `0x970412`
  (`word-2` on the BLP header field at struct+`0x1c`, enum {0,1,2,3})
  or is constant 0 (TGA), never from id `0x0b`. `0x7d11f0` forwards
  width/height/format to vtable+`0xa8`, which for both devices calls
  base creator `0x7d4590` / `0x7d2bf0`; `0x7d2bf0` stores format to
  texture-descriptor+`0x1c` unchanged.
- Actual GL upload (imports are OpenGL32 only; no D3D import exists in
  the qualified image): OpenGL texture upload at VA `0x7da8xx`
  (virtual_0) decodes via descriptor+`0x28` callback, expands with
  `0x7dab00`, and calls `glTexImage2D` at `0x7daa24` with
  target `0xde1`, level from loop, format `0x1908`, type `0x1401`,
  or `glTexSubImage2D` at `0x7daa80` with the same format/type.
  The internal-format table at `0xebc5f4` is indexed by the BLP
  format enum: {0:`0x8058`, 1:`0x8056`, 2:`0x8057`, 3:`0x8050`}.
  `0x7dab00` has two paths: arg nonzero = 16-bit word-to-dword
  expansion (shifts/masks at `0x7daba0..0x7dabd9`), arg zero = BGRA
  byte swizzle. The caller passes nonzero only when texture flag
  bit `0x20` is set, with the format enum as arg. A placeholder
  allocation at `0x7da550` calls `glTexImage2D` with internalFormat
  `0x8058`, format `0x80e1`, type `0x1401`, pixels NULL — i.e. a
  RGB565-sized storage alloc, not the data upload.
- What this still does not qualify: the table entries were read as
  raw constants; no byte in the qualified image names the 16-bit
  channel order, alpha semantics, or rounding for the `0x7dab00`
  expansion, and the transient `0x8058` alloc does not define the
  expanded RGBA layout. The GL data upload is RGBA/`0x1908`/
  `0x1401` after expansion, not a 16-bit GL store. Therefore the
  16-bit consumer remains unsupported: no RGB565/1555/4444 path is
  implemented, and `one_mip` / `derive_texture_quality` keep
  refusing 2-byte depth fail-closed.

## Next precise function

OpenGL upload worker at VA `0x7da8xx` -> expander `0x7dab00`
(word-to-dword path at `0x7daba0`) -> `glTexImage2D` at `0x7daa24`.
That triplet is the precise place a qualified 16-bit layout would
have to be named; until a byte names channel order/alpha/rounding
there, 16-bit stays unsupported.
