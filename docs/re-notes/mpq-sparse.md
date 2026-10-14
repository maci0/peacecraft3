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
The decoder is exposed as `pcx_mpq::sparse::decompress`.

## Archive admission

`payload::decompress_sector` accepts exact standalone sparse (`0x20`) and
sparse-plus-ZLIB (`0x22`) masks through the existing checked reader. For the
combined mask, ZLIB runs first and sparse runs second. Both choices follow the
generic table and explicit dispatch in the pinned
[primary compression dispatcher](https://github.com/ladislav-zezula/StormLib/blob/116135c27e8c0dcf8ea0a0c549a6d737ce10fc73/src/SCompression.cpp).
Other sparse combinations remain unsupported. Archive/profile selection,
Huffman's explicit table requirement and stored-verbatim handling are unchanged.

The combined path cannot assume its intermediate fits the final output. For
accepted sparse framing producing D bytes, literal bytes and their markers use
at most 2D bytes, and zero markers produce at least one remaining output byte.
Adding the header gives an encoded ceiling of `2D + 4`. Thus a final capacity N
permits a fixed intermediate reservation of `2N + 4`, checked for arithmetic
overflow and `Layout::array::<u8>` validity before allocation. ZLIB must finish
and consume its complete packed body within that slice. Sparse then validates
its own header and framing against the final slice.

`PayloadError::Sparse` preserves the checked sparse cause;
`SparseStageCapacity` reports an impossible intermediate layout. Existing
archive/member wrappers retain the source context. Archive reconstruction still
requires each sector and the final file to produce their exact declared sizes.
A codec error does not promise that a caller-provided output slice is unchanged;
failed extracted candidates are discarded. These stage bounds do not cover
total RSS, caller buffers, archive copies or ZLIB workspace.

Six synthetic public-API groups in `tests/sparse_zlib.rs` exercise encrypted
single-unit and sector-table archives, mixed two-sector masks, both checked and
owned bounded readers, oversized/truncated/trailing framing, packed and decoded
budget refusals, the tight intermediate ceiling, unchanged unsupported masks,
and raw bytes that resemble compression masks. All six execute on actual
wasm32 against Cargo-built libraries. A separate exact-source helper gate checks
32-bit arithmetic/layout failures without allocating giant buffers. An isolated
native probe observes one 132-byte stage allocation and no reallocation for
both 132-byte valid and 133-byte refused inflated streams with final capacity
64; separate ZLIB workspace allocation remains visible.

A fresh selected-source differential compiles the pinned dispatcher, ZLIB and
sparse bodies with a table restricted to the tested methods. Across 31 synthetic
vectors, 29 primary roundtrips match the Cargo-built Rust reader. Two primary
encoder emissions are refused by both readers and are retained separately,
rather than counted as successful roundtrips. The combined reference uses
`max(2N + 4, packed_length + 1)` because its dispatcher checks packed length
before decoding; the Rust final slice remains N. Seven compact standalone
vectors use N for both readers. This is a selected-source comparison, not a full
StormLib package or native Storm.dll execution. Captured commands, artifacts and
results are in `.scratch/sparse-chain-root-review/`.

The actual Cargo-built application test WASM also passes its existing fourteen
JS export groups and three archive-factory/map-admission groups. Additional
export checks register synthetic archives for both masks and verify exact bytes,
source receipts, six one-under budget refusals and malformed sparse context.
This executes the real browser storage exports in Node, without a DOM or draw;
it is not a hosted-page or original-ISO sparse-member acceptance result.

Original sparse-member acceptance, other compression combinations and target
native codec interoperability remain unverified. No original bytes were used by
these new gates or committed.
