# Local CASC reader

This Rust 2024 / Rust 1.99 crate belongs to the shared workspace and uses its
lockfile and warning-denial policy. No network access, install mutation or original asset
contents are part of the reader. All source fixtures are synthetic.

## Provenance and measured scope

Measured on 2026-10-14 against
`/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III`.
The active `.build.info` row identifies product `w3`, version `3.0.0.24268`.
The build configuration's MD5 matches the build key read from that row.
Runtime `Evidence` records retain the install, version, product, build key,
member/span, length and MD5. A hash identifies bytes, not an independently
verified release or a security boundary. Parser entry points require explicit
provenance; they do not infer a version from a path.

Reconfirmed: `.build.info`, `_retail_/`, 16 `.idx` files and 33 data parts.
Data parts total 34,960,815,844 bytes. The ignored fixture validates every
index entry against the corresponding part's actual size. It does not hash or
decode the entire 34.96 GB store.

## Closed index feature

All 16 files are 131,072 bytes. Their contiguous V2 framing is:

| Offset | Role |
|---|---|
| 0 | little-endian header length and Jenkins checksum, 8 bytes |
| 8 | header payload, declared length 16 bytes |
| 24 | 8 alignment bytes, all zero |
| 32 | little-endian table length and Jenkins checksum, 8 bytes |
| 40 | table entries; total length read from the preceding guard |
| table end | zero tail through the physical end of the file |

The observed headers declare revision 7, bucket matching the filename, flags 0,
key/offset/size widths 9/5/4, and 30 low offset bits. The maximum-offset word
is retained as `opaque_max_offset`; its operational semantics are refused.
Widths and counts are parsed from each header, not copied from this measurement.
Entry offsets are big-endian; encoded sizes are **little-endian**. Segment and
in-part offset are derived from the authored split-bit count.

The header uses Jenkins lookup3 with initial seeds zero. The entry checksum
chains both hashlittle2 outputs across individual records, initially zero/zero.
All 16 header and 16 table checksums verify. The tables contain 74,500 sorted,
unique key prefixes, zero duplicates and zero out-of-bounds spans. Those nine-byte
prefixes are not treated as complete 16-byte EKeys.

| File | Entries | SHA-256 of entire file |
|---|---:|---|
| `0000000007.idx` | 4,686 | `597fd182cebe130127c2ea0bb3f116fc13753c3567b3f4b939daf294647f02d3` |
| `0100000007.idx` | 4,667 | `7bfc76d6a8e12ceacadcac06638f8b51ec1105bf8830429c0b9ed0c62ad0754a` |
| `0200000007.idx` | 4,663 | `2c15e533ec9385c8a1be5de91b0e708546dc7864f16575a0fd1a4684324a08c1` |
| `0300000007.idx` | 4,652 | `92c5e057ba94c2290421371046f3bf038e6a7f7b9af1e650c5dc65750340e18e` |
| `0400000007.idx` | 4,666 | `b8f16f7c6f9ad92d4c44c59720872a08dd8a2e49d1ff4c9a3a0324605a2aac0b` |
| `0500000007.idx` | 4,582 | `19af2305a381d8b33c0738aea006584a87cb7a1c6ad0e2db6b76f6dc7b9e7d03` |
| `0600000007.idx` | 4,716 | `cc36080732ec1dfbded6ee75cc3ce8b4abf13853d8719a989c2f7f6364ddaf40` |
| `0700000008.idx` | 4,618 | `7ed90076e41469e3c6c241cf24bbae581605058db2d20f988890364eb412c4a6` |
| `0800000008.idx` | 4,531 | `99cb83479daebf77398202ce567968a670e39f8a114be2f26b923e88f0d965c5` |
| `0900000007.idx` | 4,680 | `1c250cf1925f799d7a873cd23d476eae1d33c48fe1ca8b16dedf3b561d500c15` |
| `0a00000007.idx` | 4,731 | `1d68cca8121694ccceae5fd1b7d093301052bfeb6ad020a5862b43e6c3e0c5ab` |
| `0b00000007.idx` | 4,641 | `5bb5de55815f51334ac12179f20cee1c84b6e6673eb4f59b604c20e877bb6df2` |
| `0c00000007.idx` | 4,613 | `d79757140cb7612e24e6b73e3fb5693e0ea92b6bc857021e74a164e44f7702ed` |
| `0d00000007.idx` | 4,633 | `91b0c308849352cbe4158aa0d9e8ce1387b42a135d07d003f81b6c4e211d070e` |
| `0e00000007.idx` | 4,708 | `ac969ea406a197b194792d69b443a93c66b3582a19bd7d4d46b59e5eb3be0e75` |
| `0f00000007.idx` | 4,713 | `6f02c1758de1a77f1f931bd3098dc74a28c8912413ca1f485e30b9b9054a5426` |

The `.build.info` size is 801 bytes; SHA-256:
`057d9685bba40d7e4b6cc3f109850ab73d78dad0328699c374c8f4660ee5049a`.
Index files concatenated in filename order have SHA-256:
`f21a5353ce8d26b59007e5d6f375937fddac6b2d3209de301929c456f4622ff5`.
These hashes and aggregate observations are not asset contents.

## Closed byte-chain features and their limits

`Storage::open` reads the active build identity and hash-verified local config,
selects the newest authored index generation per bucket, parses all selected
indexes and checks every local span's bounds. No CDN fallback is attempted.

`read_ekey` locates candidates by their authored key-prefix length and reads
exactly the index-declared span. It preserves the local 30-byte envelope and
ten opaque control bytes. Identity is verified with the full BLTE-header MD5
(or complete BLTE-stream MD5 for a zero-sized header), never with a guessed
completion of a short index key. Envelope key claims can be unverified without
invalidating an independently verified BLTE payload. The supplied content root
has such a partial envelope claim; `envelope_key_semantics` reports it explicitly.
Envelope size disagreement is refused rather than assigned alternate semantics.

BLTE supports unencrypted `N` and `Z`, scalar and chunk-table framing. Declared
sizes, header/key identity, every chunk MD5, zlib end-of-stream, exact consumption
and caller memory limits are checked. Unknown signatures/tags, encryption and
recursive frames return contextual `Unsupported` with the next probe. The plan's
BLDR label is not assumed to be an alias for the measured BLTE signature.

The configured ENCODING manifest decodes to 8,846,470 bytes. EN v1 parsing checks
all 1,294 CKey-page and 849 EKey-page directory hashes and first-key relations.
It resolves 138,362 CKey records, retaining their authored EKeys and decoded sizes.
The 91-byte ESpec area, all hash-verified EKey pages, and 53-byte final tail remain
opaque. `interpret_specs` refuses executable codec/spec semantics.

`read_content` follows those CKey records to local EKeys, then verifies decoded
length and the complete CKey MD5. Provenance from a different install/build is
rejected. The corpus exercises configured encoding/VFS manifests and the content
root, not all 138,362 representations or all 74,500 indexed payloads.
The 112 configured aliases resolve to 111 unique EKeys, totaling 21,166,659
reported decoded bytes (including the duplicated VFS-root alias). Those aliases
contain 5 N chunks and 146 Z chunks, all verified. The separately read content
root decodes to 21,145,250 bytes with 81 verified Z chunks; its `War3` signature
is identified, but its path records are not interpreted.

## Open gates and next probes

- **Path to EKey is unsupported.** A real VFS root is decoded and identified
  before `path_to_ekey` reports the refusal. Next: establish this build's TVFS
  path-table, span/VFS-table and nested manifest joins, then verify a named path
  against the original client. No FDID/path mapping is invented.
- **Full BLDR/encoding chain is not established for arbitrary store contents.**
  Supported N/Z and CKey joins are a verified subset. Next: census actual codec
  tags and inspect each unsupported mode/key carrier before extending decoding.
- **Local span controls, partial envelope-key writing and the index maximum word
  remain unknown.** Their bytes are retained and semantic access is refused.
  Next: trace their consumers/writers in this build's original storage reader.
- ESpec/EKey-page interpretation, encrypted/nested codecs, paged/older index
  revisions, remote-only content, loose overrides and native behavior remain open.
- Native REA evidence was unavailable in this session. Static format source and
  shipped-byte integrity checks do not prove original-client execution behavior.

## Primary format and dependency references

Format references are pinned to CascLib commit
`38a34665624b8775bb875274b36191b21c38d97b` and corroborated against the local bytes:

- [Index guards, framing and actual little-endian local size reader](https://github.com/ladislav-zezula/CascLib/blob/38a34665624b8775bb875274b36191b21c38d97b/src/CascIndexFiles.cpp)
- [Jenkins lookup3 algorithm](https://github.com/ladislav-zezula/CascLib/blob/38a34665624b8775bb875274b36191b21c38d97b/src/jenkins/lookup3.c)
- [BLTE framing and local-envelope caveats](https://github.com/ladislav-zezula/CascLib/blob/38a34665624b8775bb875274b36191b21c38d97b/src/CascReadFile.cpp)
- [ENCODING structures](https://github.com/ladislav-zezula/CascLib/blob/38a34665624b8775bb875274b36191b21c38d97b/src/CascStructs.h)
- [ENCODING CKey page walk](https://github.com/ladislav-zezula/CascLib/blob/38a34665624b8775bb875274b36191b21c38d97b/src/CascOpenStorage.cpp)

Current stable versions were verified against the crates.io primary registry on
2026-10-14: [flate2 1.1.10](https://crates.io/api/v1/crates/flate2),
[md5 0.8.1](https://crates.io/api/v1/crates/md5), and
[thiserror 2.0.21](https://crates.io/api/v1/crates/thiserror).
The crate owns its lockfile; root manifests/lockfile remain untouched.

## Reproduce

```sh
CARGO_TARGET_DIR=/tmp/pcx-casc-target cargo test --locked -p pcx-casc
PEACECRAFT3_REFORGED_DIR='/home/maci/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III' \
CARGO_TARGET_DIR=/tmp/pcx-casc-target cargo test --locked -p pcx-casc -- --ignored --nocapture
```

The ignored original-fixture test fails without its explicitly supplied install.
It never succeeds by skipping missing source assets. Console measurements stay
local; no decoded original bytes are written.
