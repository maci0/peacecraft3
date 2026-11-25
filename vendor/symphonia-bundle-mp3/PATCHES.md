# symphonia-bundle-mp3 0.6.1 compatibility patch

Based on the published `symphonia-bundle-mp3` 0.6.1 crate (MPL-2.0), SHA256
`98ea5ffc8716bff677dfb3b01b420c7b758de901a72b8c330bf2040ab74b4add`.

Gate private `FrameHeader` stored `bitrate` and its initializers on `mp2`, its
only reader. Similarly gate stored `sample_rate_idx` and its initializers on
`mp3`, its only reader. The sample-rate tuple is still parsed unconditionally.
All bitrate and sample-rate parsing, invalid-header rejection, Layer II mode
restrictions and frame-size calculations remain unconditional for every layer.
A synthetic header regression checks all three layer frame sizes and invalid
bitrate/rate/mode cases, including mp1-, mp2- and mp3-only builds.

The separate Symphonia 0.5.5 family is unchanged. No decoder, demuxer or synthesis
algorithm is changed. The published Rust 1.85 floor is unchanged.

Published license notices and other source files are retained. Package lockfiles
and Cargo cache/package stamps are omitted; the workspace lockfile owns product
resolution. The published manifests are unchanged.

See [the workspace compatibility note](../../docs/re-notes/rust-dependency-warnings.md)
for provenance, validation scope and remaining upstream Clippy style debt.
