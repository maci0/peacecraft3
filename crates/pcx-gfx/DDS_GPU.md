# Checked compressed ATI2 import

`dds_gpu::inspect(bytes, source, limits)` validates caller-supplied legacy ATI2
storage. `dds_gpu::import(bytes, source, &RenderDevice, limits, sampler,
asset_usage)` additionally checks the **enabled** device BC feature and its
`max_texture_dimension_2d` before calling Bevy's DDS reader. The result contains
a compressed `Image`, the source label, every checked mip span, and the complete
128-byte source header verbatim. Span offsets refer to the original file; the
Image payload begins at source offset 128. This does not produce RGBA pixels.

Only a single legacy 2D ATI2/BC5_UNORM image is accepted: no DX10 envelope,
arrays, cubes, volumes, signed BC5, other FourCCs, unknown storage flags or
control fields. Top dimensions must be positive multiples of four, preventing
Bevy's block-size rounding from changing the authored extent. Smaller trailing
mips retain their actual dimensions and full 16-byte block storage. The complete
authored chain must consume the file exactly; top linear size, flags, mip count
and caps must agree. This strict local import profile does not claim universal
DDS compatibility or native malformed-file parity.

The common envelope, single-2D controls and mip-span checks are shared with
`dds`. Its CPU BC1/BC3 public API and ATI2 refusal remain unchanged. No extension,
namespace, TEXS, material or shader API is added. The compressed importer has no
channel remapping, blue reconstruction, gamma inference or rendering claim.
Sampler and asset residency are explicit caller choices. BC5 has no sRGB storage
variant; the `false` passed to Bevy's generic reader selects no native colour
interpretation.

## Header evidence

The [DDS header specification](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header)
establishes header size, dimensions, mip count, flags and surface caps.
The [pixel-format specification](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-pixelformat)
establishes the FourCC discriminator; RGB bit count and masks are not storage
selectors for this FourCC profile. Microsoft's
[DDS programming guide](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dx-graphics-dds-pguide)
documents ATI2 as BC5_UNORM and its 16-byte blocks. Published identifiers and
flag masks are format invariants, not measured fixture defaults.

Microsoft's [DirectXTex DDS reader](https://github.com/microsoft/DirectXTex/blob/main/DirectXTex/DirectXTexDDS.cpp)
documents the two high pixel-format hint bits (`0xc0000000`) and ignores other
bits when matching FourCC storage. This importer permits those two non-storage
bits without interpreting them or requiring a vendor signature, and refuses
other pixel-format flags. All reserved/private words, RGB fields and signatures
remain opaque in the returned raw header. In particular `A2XY` and `NVTT` are
**data**, never accepted-value whitelists or channel/gamma instructions. Unknown
header/caps/control combinations that could alter layout produce typed refusal.

## Allocation and device boundary

There are no library budget defaults. The caller supplies encoded, compressed
payload and working-buffer ceilings. Before Bevy runs, checked arithmetic and
`Layout::array` validate the theoretical top block span, payload, reader capacity,
mip-vector layout and retained source label. Working accounting reserves
`2 * payload_bytes + 32`, up to `u32::BITS` mip records, and the source label.
It excludes the borrowed input, caller-owned sampler descriptors, stack data and
allocator overhead; it is not an RSS or GPU-memory limit.

Bevy Image 0.20.0's `dds_buffer_to_image` delegates to ddsfile 0.6.0 `Dds::read`,
which reads the remaining Cursor into one Vec and moves it into Image for BC5.
There is no BC5 transcode allocation. Rust's
[Cursor reader implementation](https://github.com/rust-lang/rust/blob/master/library/std/src/io/cursor.rs)
reserves the remaining slice and extends that Vec. The conservative reader
capacity allowance is checked again on the returned vector. Storage format,
dimensions, layer count, mip count and payload equality are also checked; reader
drift becomes a typed reader refusal. Dependency/Rust upgrades must preserve or
revisit this allocation assumption.

Missing **enabled** `TEXTURE_COMPRESSION_BC` is a typed `Capability` refusal,
even if the adapter advertises BC. Extent excess is a typed `Extent` refusal;
malformed/unsupported/limit/layout errors preserve source and offset through
`dds::Error`. The only public import path obtains features and limits directly
from `RenderDevice`. Private synthetic-capability tests are explicitly test
policy. Actual-device tests initialize only a headless device, without building
or launching the application and without drawing or uploading a texture.

## Explicit local fixture

The ignored test requires `PEACECRAFT3_REFORGED_DIR` and
`PEACECRAFT3_DDS_DEVICE_TEST=1`. It resolves only
`war3.w3mod:_hd.w3mod:units/human/footman/human_footman_main_normal.dds`
through configured TVFS links and verifies EKey
`a94fde4333e1fd247c30b65c3db6b4aa` and CKey/file MD5
`dba0c21180bef10bda12e16e709aa266`. Fixture measurements are assertions in this
explicit original test, not production accepted values: 2,796,368 bytes,
1024×2048, 12 mips; compressed payload MD5
`f6b3843fada5c1569ec0bd7cb118057f`. The complete returned header and compressed
payload must equal the caller's in-memory source. No original bytes are saved.

The device test records adapter information, adapter-supported features and
actual enabled-device features. Headless Vulkan, BC feature requests, nearest
sampling and MAIN_WORLD residency are **test policy**. A second actual device
deliberately omits BC to verify refusal on an adapter that advertises support.
This establishes host device feature/import behavior only. Native binding,
channel meanings, shader/gamma policy, GPU upload and rendered parity remain
unverified and outside this API.

Observed local device run (2026-10-14): Reforged build `3.0.0.24268`,
AMD Radeon RX 7900 XTX (RADV NAVI31), Vulkan, driver `radv`,
`Mesa 26.2.4-arch3.1`. The first device enabled only
`TEXTURE_COMPRESSION_BC`; the second enabled no optional features despite the
adapter advertising BC. The keyed asset imported as `Bc5RgUnorm`, 1024×2048,
12 mips, 2,796,240 compressed payload bytes, with both full header and payload
equal to source. The second device returned `Error::Capability`. These are
observed host device/import results under explicit test policy, not native proof.

## Separate host upload/readback check

The keyed texture also passed a headless Vulkan copy/readback probe on the same
BC-enabled device. It passed the imported descriptor, `LayerMajor` data order
and compressed payload to `RenderDevice::create_texture_with_data`, the API
used by Bevy's fresh-image upload path. `COPY_SRC` was explicitly requested as
readback test policy; the returned image descriptor already included it.

All twelve mips, including the logical 1×1 tail, returned byte-for-byte equal
to their checked source spans: 2,796,240 compressed bytes through a 2,801,920-byte
mapped staging buffer. Transfer padding and sub-block physical extents were
derived from format/device APIs. The final mip source/readback MD5 was
`0adc491d84c8738ae7706e6b8f8fc07f`. A reviewed repeat run agreed; source, pinned
library identities and both logs remain local in
`.scratch/reforged-ati2-gpu-boundary/`.

This closes copy/readback for that named asset and enabled host device only.
No application asset-loading pipeline, GPU sampling/decoding, shader, draw,
browser device or native material/channel/gamma interpretation was exercised.
No production upload integration was added by the probe.
