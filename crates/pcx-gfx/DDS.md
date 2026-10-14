# Explicit legacy DDS BC1/BC3 import

`dds::decode(bytes, source, limits)` imports caller-supplied legacy DDS bytes into
an existing `LoadedTexture` plus a retained `Profile`. It decodes the top mip to
RGBA8; it validates every authored mip and returns each source range. No I/O,
TEXS suffix rewriting, namespace priority, material binding, sampler selection,
color-space conversion or rendering behavior is added.

The previous `load_image(bytes, source)` signature remains unchanged. DDS magic
now classifies as `AssetKind::Dds`; that unbudgeted entry point returns a typed
`Error::Dds` refusal directing callers to `dds::decode`. There is no asset-size
budget hidden in a default. BLP/TGA behavior is unchanged.

## Bounded profile and error contract

- Magic `DDS `, 124-byte legacy header, 32-byte pixel-format record; FourCC
  `DXT1` (BC1) or `DXT5` (BC3). Mandatory texture/FourCC/extent flags must exist.
- Single 2D surface with positive top extents divisible by four, matching the
  existing `image` DDS reader's top-level width restriction. Depth and caps2
  must be zero. DX10, arrays, cube/volume layouts, other FourCCs (including
  ATI2/BC5 and DXT3), unknown flags/control words and nonzero pixel masks or
  RGB-bit-count/swizzle fields refuse explicitly.
- A declared mip-count flag requires a positive count no greater than the
  extent-derived chain. Multiple levels require complex/mipmap caps. An
  unflagged count of zero or one describes the base surface; the raw count is
  retained. Partial authored chains are allowed; no missing levels are generated.
- Each level occupies `ceil(width/4) * ceil(height/4) * block_size` bytes, with
  at least one block per nonzero dimension. Block size is eight for BC1 and
  sixteen for BC3. Every range is checked before reading, arithmetic is checked,
  the final range must end exactly at file length, and a flagged linear size
  must equal the computed top span. A nonzero unflagged pitch refuses.
- Every BC1 block in every mip is checked. Endpoint0 <= endpoint1 with any
  selector3 lane refuses as punch-through alpha, including equal endpoints and
  later mips. The existing RGB decoder would otherwise discard that alpha.
  Padding lanes are also checked; no opaque repair or padding interpretation
  relaxes this bounded policy.

Header/pixel flags, caps, the authored count and eleven reserved words are retained
in `Profile`. The non-standard NVTT `0x40000000` marker is accepted only with its
explicit `NVTT` reserved-word signature; it is preserved raw and never used to
choose gamma, a normal interpretation, opacity, shader or alpha policy. Presence
flags are not channel semantics. Unknown pixel/profile bits remain a typed refusal.

`dds::Error` contains source name, byte offset and a typed `Failure`: malformed,
unsupported, caller limits, arithmetic overflow (malformed), or decoder limits.
Decoder failures retain source context. These are checked import policies, not
claims about the original client's malformed-input rejection.

## Caller budgets and official decoder reuse

`Limits` has no default. Callers supply `max_encoded_bytes`, `max_rgba_bytes`
and `max_working_bytes`. Before invoking `image`, the importer checks input length,
checked width × height × four and addressability. `Layout::array::<u8>` also
checks the RGBA vector's platform allocation limit before decoding; fitting
`usize` alone is insufficient for the later RGB-to-RGBA allocation. Working-buffer
checks cover logical decoder pixel bytes plus an encoded block row, or simultaneous RGB and
RGBA buffers during BC1 conversion, whichever is larger. They exclude already
borrowed input, bounded profile metadata and allocator overhead; they are not a
process-memory/RSS guarantee. ImageReader also receives strict width/height
limits and its allocation budget. BC3 conversion consumes the RGBA buffer;
BC1 conversion requires both RGB and RGBA payload buffers.

The existing locked `image 0.25.10` package supplies
`ImageReader::with_format(Cursor::new(bytes), ImageFormat::Dds)`, `decode` and
`into_rgba8`. No DDS decompressor is duplicated and no new package is introduced.
`Profile.mips` retains compressed source ranges; lower mips are checked, not
converted to new pixel buffers. Native gamma, sampler behavior, channel roles,
BC5/normal-map decoding and compressed GPU images remain outside this API.

Format sources: Microsoft's [DDS layout and profile guide](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dx-graphics-dds-pguide)
and [DDS_HEADER](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header).
The [DirectXTex DDS reader](https://github.com/microsoft/DirectXTex/blob/main/DirectXTex/DirectXTexDDS.cpp)
identifies NVTT's non-standard high flags using `reserved1[9] == 'NVTT'`; only
that profile identification is reused, not its color-space conversion.
The actual decoder contract is the locally locked `image-0.25.10/src/codecs/dds.rs`
and `dxt.rs`: DXT1 outputs RGB8, DXT5 outputs RGBA8, and ATI2 is unsupported.

## Explicit original-input regression gate

The ignored test `named_original_footman_dds_provenance_decode_and_ati2_refusal`
requires `PEACECRAFT3_REFORGED_DIR`. It resolves these fully qualified configured
members, checks their EKey/CKey identities, reads content only in memory, compares
output dimensions with the checked source header, verifies all mip spans, and
checks keyed decoded-pixel hashes. Build/version and namespace-link provenance
remain supplied by CASC; this test does not implement any binding into TEXS.

All three named paths have the explicit prefix
`war3.w3mod:_hd.w3mod:units/human/footman/`:

| Member | EKey | CKey / decoded file MD5 | Expected branch |
|---|---|---|---|
| `human_footman_main_diffuse.dds` | `dad62eed624c84a8fcd4335893270f81` | `25a4a52563136827ff89f3ebc966123a` | BC1 |
| `human_footman_main_orm.dds` | `f9c63003c3c5c403e13452c45d517c81` | `8a1008e9e79cbf2043459daf1b1263b6` | BC3 |
| `human_footman_main_normal.dds` | `a94fde4333e1fd247c30b65c3db6b4aa` | `dba0c21180bef10bda12e16e709aa266` | typed ATI2 refusal |

These identities are regression fixture metadata, not production defaults or
embedded assets. Original files and generated pixels are never saved by the test.

### Verified local result (2026-10-14)

Build `3.0.0.24268`: diffuse BC1 (1,398,248 bytes) and ORM BC3
(2,796,368 bytes) each validated twelve authored mip spans and decoded a
1,024 × 2,048 top mip into 8,388,608 RGBA bytes. Pixel MD5s were respectively
`67ab3c72a44d45aab7731a822e3ae16c` and
`fb593afff3d175c0bbeddd9f4b7dd4a5`. Main-normal (2,796,368 bytes) produced
`Failure::Unsupported` at FourCC file offset `0x54` for ATI2; no decode was
attempted. The keyed test checks all three file identities listed above.

Exact verification commands (private target; frozen shared lock):

```sh
CARGO_TARGET_DIR=/tmp/pcx-gfx-dds-target cargo test -p pcx-gfx --locked --test dds
PEACECRAFT3_REFORGED_DIR='/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III' CARGO_TARGET_DIR=/tmp/pcx-gfx-dds-target cargo test -p pcx-gfx --locked --test dds named_original_footman_dds_provenance_decode_and_ati2_refusal -- --ignored --nocapture
CARGO_TARGET_DIR=/tmp/pcx-gfx-dds-target cargo clippy -p pcx-gfx --locked --lib --test dds --no-deps -- -D warnings
CARGO_TARGET_DIR=/tmp/pcx-gfx-dds-target cargo build -p pcx-gfx --locked --lib
CARGO_TARGET_DIR=/tmp/pcx-gfx-dds-target cargo check -p pcx-gfx --locked --lib --target wasm32-unknown-unknown
```

Final synthetic and original gates pass; scoped Clippy, native build and WASM
check exit zero:

```text
synthetic: 12 passed, 0 failed, 1 original fixture ignored
original fixture: 1 passed, 0 failed
```

The synthetic gate includes BC1 colour/three-colour mode, two distinct BC3
alpha endpoint orders, rectangular lower mips, source context, bad framing,
wrong length, arithmetic and vector-layout overflow, exact budgets, unknown
layouts/semantics, retained NVTT metadata and punch-through refusal in both top and
later mips. Compilation and
Clippy deny warnings. A separate 32-bit WebAssembly source-leaf run exercises the
same `src/dds.rs` implementation with oversized vector-layout, BC1 three-colour
and BC3 fractional-alpha fixtures; all three pass. It does not run the application
or render textures. Application browser execution and rendering remain untested.
