# MPQ sparse compression (0x20)

StormLib implements sparse compression (`MPQ_COMPRESSION_SPARSE`) as run-length
encoding specialised for zero-heavy data. This note captures the decoder framing:

- The stream begins with the original data length as a big-endian `u32`.
- Marker high bit set (`0x80..=0xff`): a non-zero run of `(marker & 0x7f) + 1`
  bytes, followed by that many literal bytes.
- Marker high bit clear (`0x00..=0x7f`): a zero run of `(marker & 0x7f) + 3`
  bytes (`0x7f` therefore means 130 bytes, the format's largest zero run).
- The final zero run is capped to the remaining declared output. The primary
  compressor emits `0x7f` for its final short all-zero tail, so rejecting that
  terminal form would refuse valid compressor output.
- Complete output and consumption, truncated literals and the caller's buffer
  bounds are checked import policy. This is stricter than the reference's
  treatment of some malformed streams; hand-written synthetic tests alone are
  not a reference differential or native parity proof.

Source: StormLib `src/sparse/sparse.cpp` (`CompressSparse` / `DecompressSparse`)
at revision `116135c27e8c0dcf8ea0a0c549a6d737ce10fc73`,
[primary implementation](https://github.com/ladislav-zezula/StormLib/blob/116135c27e8c0dcf8ea0a0c549a6d737ce10fc73/src/sparse/sparse.cpp).
The private differential compiles that exact implementation and compares 171
synthetic compressor roundtrips plus the short terminal-zero case against the
Rust reader: all 172 match. No original assets or native Storm.dll were used.
The 13 malformed/boundary tests live in `crates/pcx-mpq/tests/sparse.rs`.
The decoder is exposed as `pcx_mpq::sparse::decompress`. Archive integration into
`payload::decompress_sector` and original-member gates remain pending.
