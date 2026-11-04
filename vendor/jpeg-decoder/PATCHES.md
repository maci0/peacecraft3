Based on image-rs/jpeg-decoder 0.3.2 (MIT OR Apache-2.0).

The full 8×8 integer IDCT uses 13 fractional bits and signed rounding, matching
the standard integer transform in libjpeg's jidctint.c. Reduced-size transforms
retain upstream behavior. PeaceCraft enables `platform_independent` so architecture
specific 12-bit transforms cannot change decoded pixels. Quantization tables and
coefficients continue to come from each JPEG stream.

The selected legacy executable contains the same 13-bit transform constants;
see docs/re-notes/campaign-minimap.md in the repository root for evidence and
local original-input validation. No game data is included here.

Obsolete `asmjs` target conditions were removed so current Rust check-cfg stays warning-free.
The saturated-block wrapping regression has the 13-bit transform result.
The thread-worker module is compiled only on its supported non-WASM targets.
File-based documentation examples compile without requiring upstream image fixtures.
