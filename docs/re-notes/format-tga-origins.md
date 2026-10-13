# TGA raster origins

The decoder now honors descriptor bit 4 for right-origin rows, together with its
existing descriptor-bit-5 vertical conversion. Both raw and RLE streams first
produce stored-order pixels, then convert to top-left RGBA. Dimensions and origin
flags come from the input header; no image dimensions or fitted offsets are constants.

The independent format invariant is Truevision's original TGA 2.0 specification,
Image Descriptor, Table 2 (page 9): bits 5/4 select bottom-left, bottom-right,
top-left or top-right. Source:
[original specification](https://raw.githubusercontent.com/DEAKSoftware/Truevision-TGA/master/truevision-tga.pdf).
This is external primary specification evidence, not Warcraft native-code evidence.

Two synthetic regressions cover every origin with asymmetric pixels, both raw
and a literal RLE packet crossing a row boundary, and a zero-width right-origin
raster. The normal crate suite passed 15 tests; its explicitly ignored original
Legacy asset sweep also passed. The original assets do not exercise right-origin
rows, so their sweep alone cannot establish that branch. No dependencies or caller
interfaces changed; other currently unsupported TGA families remain explicit errors.
