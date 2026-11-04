# BLP JPEG color channel order

BLP1 JPEG color components use BGR channel order. The decoder previously
returned the first component as red. That exchanged red and blue throughout
JPEG-backed art: the TFT logo had cyan letters and a warm frame.

`pcx_blp::decode_jpeg` now reorders color components into its RGBA output.
Four-component streams decode as inverted BGRA, retaining the independent fourth
component even when the header declares zero alpha bits. The selected client
uses that component for affected art; the header alone does not discard it.
Three-component color JPEGs reorder likewise; grayscale and paletted BLPs are
unchanged. No image names, selected pixels, replacement colors or measured
artwork values control this conversion.

The primary implementation comparison is
[Pillow's BLP JPEG decoder](https://github.com/python-pillow/Pillow/blob/main/src/PIL/BlpImagePlugin.py),
which explicitly requests raw BGR output after JPEG conversion. This is format
corroboration, not proof of a particular native game's internal decoder.

A fresh caller of the fixed public `pcx_blp::decode` was compared against the
independent decoder on two explicitly retained local logo textures. On fully
opaque pixels, TFT covered 183,561 RGB components and RoC covered 209,229; the
maximum difference was one intensity level on both, consistent with differing
JPEG IDCT rounding. Both alpha planes are byte-identical to the prior decoder.
Private source/artifact hashes, exact commands, comparison data and diagnostic
images are in `.scratch/menu-material-fidelity-review/fixed-caller/`. Original
textures and generated images remain local and are not part of this change.

Synthetic component tests distinguish red and blue, alpha and black-ink paths,
and grayscale. The local original-logo test derives dimensions from the header
and checks its transparent/opaque/partial-alpha classes rather than embedding
measured dimensions, pixel coordinates or the former incorrect blue-color
expectation. Texture decoding evidence alone does not certify whole-menu
rendering, lighting or animation fidelity. SKY2 is paletted, so this JPEG fix
does not address its separate projection and scene-state discrepancies.

The later 13-bit integer transform correction and exact native minimap base-image
comparison supersede the one-level JPEG rounding residual described above. See
[current minimap evidence](campaign-minimap.md#live-sight-endpoints-and-jpeg-precision-2026-11-04).
