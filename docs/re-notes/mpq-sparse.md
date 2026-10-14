# MPQ sparse compression (0x20)

StormLib implements sparse compression (`MPQ_COMPRESSION_SPARSE`) as run-length
encoding specialised for zero-heavy data. This note captures the decoder framing:

- The stream begins with the original data length as a big-endian `u32`.
- Marker high bit set (`0x80..=0xff`): a non-zero run of `(marker & 0x7f) + 1`
  bytes, followed by that many literal bytes.
- Marker high bit clear (`0x00..=0x7f`): a zero run of `(marker & 0x7f) + 3`
  bytes (`0x7f` therefore means 130 bytes, the format's largest zero run).
- The sum of all run lengths must equal the declared original size; truncation,
  leftover framing, or a declared size beyond the caller's buffer is an error.

Source: StormLib `src/sparse/sparse.cpp` (`CompressSparse` / `DecompressSparse`),
Ladislav Zezula. Synthetic parity tests in `crates/pcx-mpq/tests/sparse.rs`.
The decoder lives in `crates/pcx-mpq/src/sparse.rs`; root integration into
`payload::decompress_sector` is pending.
