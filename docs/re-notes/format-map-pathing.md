# Authored map pathing raster

Fresh REA analysis of the local Legacy World Editor, SHA-256
`d198d2ca47fc29d4901f3de27ff17619824afef5af7e42b9ff17cbe03723d7b7`,
writer `0x50f9c0`, evidence
`ev_40fa104155bf56eed85e4771c73f4b951f8079bd5e0702a3f2d4a76ebe0f78e4`,
writes `MP3W`, version zero, width and height, then width×height bytes. Its nested
loop advances x within each row and then y. It maps internal flag bits to file
bits 2,4,8,32,64,128; those masks do not by themselves prove gameplay meanings.

`Pathing::parse()` requires this framing exactly, with overflow-safe dimensions,
and retains all authored cell bytes. `Map::pathing()` exposes it and preserves
MPQ extraction errors. It does not synthesize pathing from terrain or assign
walkability/buildability meanings without evidence. Missing, corrupt and unknown
version files return errors.

The narrow map suite passed 50 tests and the explicit original fixture check.
The archive fixture covers 44 maps; the separate expanded census finds WPM in
284 source maps, including loose `.w3x` files. Counts may include duplicated maps
across original archives and loose files and are not acceptance constants.
Synthetic tests cover authored dimensions, native row order, out-of-range cells,
truncation, invalid dimensions and unsupported versions.
