# jpeg-decoder 0.3.2 patch

Based on image-rs/jpeg-decoder 0.3.2 (MIT OR Apache-2.0).

The full 8×8 integer IDCT uses 13 fractional bits and signed rounding, matching
the standard integer transform in libjpeg's jidctint.c. Reduced-size transforms
retain upstream behavior. PeaceCraft enables `platform_independent` so architecture
specific 12-bit transforms cannot change decoded pixels. Quantization tables and
coefficients continue to come from each JPEG stream.

The [minimap JPEG evidence](../../docs/re-notes/campaign-minimap.md#live-sight-endpoints-and-jpeg-precision-2026-11-04)
records the 13-bit constants and original-input check for the selected Legacy
executable. The vendored crate contains no game data.

Obsolete `asmjs` target conditions were removed so current Rust check-cfg stays warning-free.
`test_dequantize_and_idct_block_8x8_saturated` checks the 13-bit result.
The thread-worker module is compiled only on its supported non-WASM targets.
File-based documentation examples compile without requiring upstream image fixtures.
