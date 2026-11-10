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

## Word expander decoded; texcolordepth link still missing (2026-11-05)

Bounded REA trace ONLY of expander `0x7dab00` word path
`0x7daba0..0x7dabd9`, caller upload `0x7daa24` and internal-format
table `0xebc5f4`, in the qualified image
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
(PE32, ImageBase `0x400000`). No client executed, no original bytes
committed, bounded reads only.

### Expander format recovered from instructions (not assumed RGB565)

Per `movzwl` word `w` at `0x7daba0`, the shifts/masks are
`shr 4` + `and 0xf0`, `shr 8`, `shl 4` on the low byte, `shl 8`
stages, `and 0xfffffff0` on the high byte — no `0x07e0`/`0xf800`
565 masks and no 5/6/5 splits anywhere in the loop. The stored
little-endian dword bytes `[b0,b1,b2,b3]` (uploaded as GL format
`0x1908`, so `R=b0, G=b1, B=b2, A=b3`) are:

- `R = ((w >> 8) & 0x0f) << 4`
- `G = ((w >> 4) & 0x0f) << 4`
- `B = (w & 0x0f) << 4`
- `A = ((w >> 12) & 0x0f) << 4`

i.e. nibbles `A R G B` from high to low (`A` bits 12-15, `R` 8-11,
`G` 4-7, `B` 0-3; little-endian word low byte holds `G|B`, high
byte holds `A|R`). Alpha has no branch: `A` nibble `0` expands to
`0`, never forced opaque. Expansion is truncation `nibble << 4`
with low nibble zero (`0x0f -> 0xf0`, not `0xff`): no
bit-replication and no rounding. This is an ARGB4444 word layout
as named by the masks/shifts. The other expander path
(`0x7dab40`, arg zero) is a BGRA byte swizzle:
`out = [src3, src0, src1, src2]` per 4-byte pixel.

### Caller and table

`glTexImage2D` at `0x7daa24`: target `0xde1`, level from loop,
internal format from table `0xebc5f4` indexed by format enum at
descriptor+`0x1c`, format `0x1908`, type `0x1401`. Table bytes:
`{0: 0x8058, 1: 0x8056, 2: 0x8057, 3: 0x8050}`. The `0x8058`
placeholder at `0x7da550` (`glTexImage2D` internal `0x8058`,
format `0x80e1`, type `0x1401`, pixels NULL) is a storage alloc,
not the data upload. The data upload is RGBA/`0x1908`/`0x1401`
after expansion, not a 16-bit GL store.

### Gx+e8 relationship (does not select the expander)

Base setter `0x7d3280` stores its arg to device+`0xe8`; getter
`0x7d32a0` returns it. Dispatcher at `0x42abeb`: depth `16` calls
setter `0`, depth `32` calls setter `1` (each only when the getter
does not already hold that value), so `+0xe8 == 0` means 16-bit
depth selected and `+0xe8 == 1` means 32-bit. In the upload worker
at `0x7da762`, `ebp-0x2c = (+0xe8 != 0 && dword[device+0xb0] > 0x10)
? 1 : 0`; at `0x7da9ee` that flag only decides whether the table
internal format is forced to `0x1908` (when nonzero and the
expander arg is zero). It never selects the word expander.

Expander selection is independent: at `0x7da9a8` the caller tests
flag bit `0x20` at descriptor+`0x20` and passes
`flag ? descriptor+0x1c : 0` as the expander arg. That flag is
stored at texture creation (`0x7d2bf0`: `+0x20 = arg4`, via
`0x7d4590` / `0x7d11f0` from the BLP/TGA call sites at
`0x970458`/`0x970646`, whose flags arg is the loader caller's
`0xc(%ebp)`). No traced byte sets flag `0x20` from
`texcolordepth` or from Gx+`0xe8`; the only Gx+`0xe8` readers in
the upload path are `0x7da762` and `0x7df333`.

### Exact missing link

The bounded trace does not establish that any BLP/TGA texture
reaches the upload with flag `0x20` set, nor that
`texcolordepth == 16` causes it. The packed-word expander alone
could serve another 16-bit asset input. No byte in the traced
region quantizes RGBA8 down to 16-bit words (only the 16-to-32
expand appears), so a texcolordepth quantize/expand RGBA8 path
would still be invented, including the RGBA8-to-nibble inverse.
Therefore no conversion is implemented: `one_mip` and
`derive_texture_quality` keep refusing 2-byte depth fail-closed,
and `Refusal::Depth16Unresolved` still retains the previous
texture. The unclosed callsite is now the caller of
`0x970280`/`0x9704b0` that supplies the flags word reaching
descriptor+`0x20`; until a byte there ties flag `0x20` to the
16-bit depth selection, the 4444 expansion stays a recovered
expander format, not a texcolordepth policy.

## Next precise function

Caller of `0x970280`/`0x9704b0` supplying the flags word stored
to descriptor+`0x20` at `0x7d2bf0`. Until that caller is traced,
the expander-vs-`texcolordepth` link stays open and 16-bit stays
unsupported.

## Flag 0x20 clearing verified at all sampled callers (2026-11-05)

Bounded trace of ALL direct callers of `0x9721b0` (11 sites) and of
`0x7d11f0` (10 sites) in the qualified image, plus the worker
`0x7da740..0x7daaf1` decision bytes. No client executed, no original
bytes committed.

- Dispatcher `0x9721b0`: `0x970280` (BLP) `flags = dispatcher +0xc`;
  `0x9704b0` (TGA) same position. Inside `0x970280` the `0x7d11f0`
  call pushes `[ebp+0xc]` as flags, `eax` (BLP switch) as format.
  The BLP switch at `0x970412`: `word-2` on header field at
  struct+`0x1c`, jump table `0x970498 = [0x97043f,0x97042a,0x970431,
  0x970438]` mapping `{2:0,3:2,4:1,5:3}`, else 0. Format enum is
  asset-derived (BLP header `picture_type` word), never `texcolordepth`.
  `0x9704b0` (TGA) pushes constant format 0.
- Every sampled flags construction clears bit `0x20`:
  `0x71b7fa` `and 0xffffffce; or 0xe`;
  `0x87ab23`/`0x87abbf`/`0x87ac06` `and 0xffffffd1; or 0x11` plus
  synthetic `12*bit`;
  `0x87cbc3`/`0x87cc4b` `and 0xffffffce; or 0xe`;
  `0x887d21` `and 0xffffffd1; or 0x11`;
  `0x920d73`/`0x9246fd` `and 0xffffffc2; or 2/4/8`;
  `0x925527` `and 0xffffffc2` on `edi` plus channel-derived bits;
  `0x542e8c`/`0x7e41d0` `and 0xffffffc1; or 1`;
  `0x7e9e8a` `and 0xffffffc0`;
  `0x6378a8` pushes 0/1; `0x745250` pushes a stack word.
  The masks `0xffffffc0/c1/c2/ce/d1` all clear bit 5 (`0x20`).
  No traced caller ORs `0x20` into flags, and no traced byte ties
  flags to `texcolordepth` or Gx+`0xe8`.
- Worker `0x7da740`: `ebp-0x2c = (Gx+0xe8 != 0 && dword[Gx+0xb0] > 0x10) ? 1 : 0`
  (`0x7da762`); `ebp-0x28` set only on first upload when
  `descriptor+0x30 == 0` (`0x7da7a0`). At `0x7da9ee`, only when
  `ebp-0x28 != 0`: `internal = table[descriptor+0x1c]`; if
  `ebp-0x2c != 0 && expander_arg == 0` then `internal = 0x1908`.
  Expander arg at `0x7da9a8`: `test descriptor+0x20, 0x20` -> arg is
  `descriptor+0x1c` or 0. The expander ignores the arg value except
  for the nonzero test (`0x7dab09`), so the format enum never
  selects 4444 vs 565; only the flag does.
- GL enum meanings are format invariants (stable OpenGL constants,
  not measured asset data): table `0xebc5f4 = {0:0x8058 GL_RGB565,
  1:0x8056 GL_RGBA4, 2:0x8057 GL_RGB5_A1, 3:0x8050 GL_RGB5}`.
  Neighboring texparam tables `0xebc5e4`/`0xebc5d4` are indexed by
  the same format enum (`descriptor+0x20 & 3` at `0x7da7c0`/
  `0x7da7dd`).
- Creator `0x7d2bf0`: `descriptor+0x1c = arg3 (format)`,
  `descriptor+0x20 = arg4 (flags)` via `0x7d4590` / `0x7d11f0`
  vtable+`0xa8`. The descriptor `+0x28` decode callback at
  `0x7da8f8` returns the pixel buffer that the expander consumes;
  the callback is selected at creation, not by `texcolordepth`.
- Alpha: no traced byte derives alpha from asset alpha or from
  `texcolordepth`. The expander preserves the alpha nibble
  (`A = (w>>12)&0xf << 4`, zero stays zero); the BGRA swizzle
  preserves all four bytes. The decoder callback owns alpha
  production; the worker never branches on it.

Decision: RGBA8. No RGBA8->nibble quantize exists in the traced
bytes, word expander is unreachable for BLP/TGA (flag cleared),
and forcing ARGB4444 for texcolordepth would be inventing the
quantize. `one_mip` and `derive_texture_quality` keep refusing
2-byte depth fail-closed; `Refusal::Depth16Unresolved` retains
the previous texture. The word expander stays a pinned spec test
only (`word_expander_4444_nibble_shift_spec`), separate from
quality quantization. GLenum channel layouts in the table are
format invariants; per-texture choice comes from the source
header (BLP `picture_type` word) via `descriptor+0x1c`, which the
native path already implements - this recreation's BLP path
decodes to RGBA8 directly and does not need the table.

## Next precise function (updated)

Nothing further on the flag-`0x20` question within the traced
callers: the flag is cleared on every sampled path. The remaining
open item is whether any *non-traced* texture source sets flag
`0x20` - that source would be a different loader, not BLP/TGA,
and not `texcolordepth`. Until a byte there ties flag `0x20` to
the 16-bit depth selection, 16-bit stays unsupported.
