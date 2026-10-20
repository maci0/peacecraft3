# MPQ payload corrections

## Evidence and scope

Fresh REA Ghidra analysis of local `wc3/game/app/Storm.dll`, SHA-256
`eecc42b05a91413dee10588a654c6cca05836e7513af6c7c568a56124bf97d0a`,
function `0x15017dc0`, evidence
`ev_23e85ced6d86f6a0b9d5b3b58d512d833423891ea1dc872f4f6079563db43aac`:

- The sector table is selected by compression flag bits (`flags & 0xff00`),
  independently of encryption. Stored encrypted files use fixed-size sectors.
- Descending sector offsets return failure.
- A sector smaller than its expected output uses separate dispatch:
  `0x100` calls the raw implode reader at `0x15017d30`; `0x200` calls the
  mask-prefixed composition reader at `0x1500dd30`.
- Each decoder receives the expected output size for this sector, including the
  final partial sector.

This is native-code evidence for that DLL. The mixed compatibility tree does not
establish the edition of its neighboring archives, nor prove target 1.29 runtime
behavior. The retained snapshot failed profile-integrity validation; the evidence
above comes from a fresh import of the original local DLL.

## Implemented behavior

`pcx-mpq/src/payload.rs` fixes the always-verbatim SINGLE_UNIT predicate,
selects the raw IMPLODE stream without dropping its first byte, rejects offsets
outside packed data or inside its table, preserves encrypted stored sectors,
and requires exact output sizes. Zlib must reach stream end; overflow, truncation
and unsupported composed masks cannot silently return partial bytes.

Six synthetic regressions cover compressed single-unit zlib, raw IMPLODE in both
single-unit and sector blocks, malformed offsets, short output, unsupported
compression composition and a sector overrunning its own output space.

## Verification

`cargo test -p pcx-mpq -- --include-ignored --nocapture` with
`PEACECRAFT3_FORMAT_MPQ` explicitly pointing at the local Legacy `War3.mpq`:
28 unit tests and the explicit fixture test passed. The fixture extracted 6,463
supported files and reported 2,705 unsupported compression cases, with no other
errors. These are measured run results, not parser constants or acceptance floors.
The archive contained no compressed SINGLE_UNIT or IMPLODE-flag entries;
synthetic fixtures establish those branches' regression coverage.

The fixture requires its environment variable and fails if the archive is missing.
It imports and extracts locally without writing archive contents. Unsupported
At this checkpoint, Huffman/ADPCM remained explicit gaps and no original
runtime comparison had run. The following 2026-10-14 section supersedes that
codec/oracle status.

## 2026-10-14: globs, recovered codecs and the original extraction oracle

### Source identity and evidence

All native derivations below refer to the user's local `wc3/game/app/Storm.dll`,
SHA-256 `eecc42b05a91413dee10588a654c6cca05836e7513af6c7c568a56124bf97d0a`.
This is the compatibility-tree DLL; its neighboring archives do not establish a
1.29b binary identity. The local 1.29b Legacy install has no separate Storm.dll.
The 1.29b WAV comparison below is **parity against the compat-tree Storm.dll
(sha256 above), not the 1.29b target binary**. Probing the statically linked
codec in the target's `Warcraft III.exe` remains separate work.

`FUN_1500dd30` dispatcher evidence
`ev_7a8096c7364851125adccb976b2935d871216f7f449ab2145d69277e5f2ef313`
visits the handler table backwards: implode, zlib, Huffman, stereo ADPCM, mono
ADPCM. Table bytes at VA `0x15042c64`:
`ev_7be596f3cf153d1fe4da9a3529555c088e88377b6c47cf10ab20d521226f82e0`;
file offset `0x42c64`:
`ev_77fa0f00d141382d9cbb29dd26fda3c4bbd74809570651781f48eea15668df88`.
The mono/stereo pointers are `0x1500d960` / `0x1500d9f0`; their instruction
bytes call `FUN_1500d240` with channel counts one/two. Missing Ghidra procedure
entries for those wrappers were a discovery limitation, not invalid pointers.
This corrects the earlier table-as-string and RVA-offset interpretation in
`000-mpq.md`; that historical note was not rewritten.

ADPCM evidence:

- Shared decoder `0x1500d240`:
  `ev_5abf352abd1291d6df0c852c62634902616f2b08066b7ae35baf05e0173c3c6a`,
  corroborated by
  `ev_5884eec95cd29ec921db15f15b27b0cab0bf6e7ccd8256db30061e5852272a21`.
- 89 LE step dwords at `0x15042ad8`:
  `ev_44af6eaa20915be4d5dad55689e927b483599dab5bbe95f28653b900f5fa3b3e`.
- 32 signed LE index adjustments at `0x15042a58`:
  `ev_105707040ec97e135125729304f99e50526576997387258cf586e90e020f5891`.

The protocol coefficients were transcribed from those reads, including the
literal index schedule with every even entry equal to -1; no remembered table
was substituted. Initial index is 44 per channel, clamped to 0..88. The stream
supplies its shift count and signed initial samples; x86 shift masking is
preserved. Stereo advances channels before each opcode; `0x81`/`0x82` change
the index without producing a sample and undo that advance. `0x80` repeats the
sample and decreases the index. Other high-bit opcodes produce no sample and
retain the advance. Output saturates to signed 16-bit range. Truncated input or
output overflow returns `PayloadError::Adpcm`.

Huffman evidence:

| Function | Evidence |
| --- | --- |
| `0x1500eb80`, wrapper | `ev_bb13a85cd5ca6f3dea0e2f742aa2154ad3d74b2e3d96ecec80e73fbd56b59dde` |
| `0x1500e5c0`, context init | `ev_8083ae644244dee24b4a9cc51e966eaa9af3d051e108e642c852205afcddb8c3` |
| `0x1500e870`, decode/output loop | `ev_abbab2a0f6e2bf243a22a98a8420a7076b1b67f9abcc64c54e9188d37b9d6b6f` |
| `0x1500df20`, symbol/bit decode | `ev_a20ac3cd396ac18eea3cbe1ce7d8d4bf72c0cd92b1dfb1ad02be900fbf084b44` |
| `0x1500e0c0`, weighted tree init | `ev_cc760b981fdf43a6c1f2e6acafc26d19070242bd70b22e22e3b96c0d1c3efa9a` |
| `0x1500e630`, escape insertion | `ev_5637274e38c3400bc32fc39544469db676d9a18d8cb24e293fb54c44e49ca8ac` |
| `0x1500e4c0`, ancestor weights/list exchanges | `ev_5eb86f057bb7dd8a91e9f56936990aeea6ba29f636c53f952c83ef8e30f9a391` |
| `0x1500e7d0`, cleanup/node capacity | `ev_0a6beb6b5b8c33c6063541b71986e79310c2651330a1be41dacc147cc366b2d4` |

Fresh CLI observations corroborated init/decode/insert/symbol/wrapper:
`ev_292db226f26d0d61273e658161fc58b82286d901e5fa6a72b61451c4b4752e27`,
`ev_e5f707b320d744bc09fd2486c006d2412d4907025434bbebd0f221bbcf1bf141`,
`ev_901f81cbc0bc1aa21401b95a7566eeff5d79651739051bb348633a1e17cac981`,
`ev_87d3041160dc04e01e738b96f4d87d75494b63328cca7057f5d2ab5023117552`,
`ev_fa58f66d28abb8e3411cea06a9f18c6c37a50443076e98c4c25fd7c0313c0cdb`.
The Rust tree uses safe indices and a bounded LSB bit reader; the native
seven-bit decode cache is an optional optimization and is not reproduced.
Symbol `0x100` terminates and `0x101` inserts a following literal. Adaptive
updates and the output-capacity stopping schedule follow `0x1500e870`.

### Runtime weights, provenance and refusal paths

The init consumes 256 byte weights, with input row selector `u8` and stride
`0x102`; the two control symbols receive protocol weight one without table
reads. REA `read_bytes` at `0x15042140` returned all 66,048 requested bytes:
`ev_8afb5793db702c8c927856fbc5d7548c9efcd17189710b43e520fdc6c2e358b4`.
That is an addressable span, not proof of 256 separately authored profiles.
PE section mapping derives 40,640 source-file bytes followed by 25,408 bytes
of `.data` loader zero fill. Reconstructing those bytes from the local PE
matched the REA memory read exactly. The first table byte maps to file offset
`0x42140` in this source. Loader zero fill follows the PE section's authored
VirtualSize/SizeOfRawData relationship, as defined by the
[PE section format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format#section-table-section-headers).
No Huffman weights or generated copies are shipped in the repository.

`HuffmanTable::from_pe` derives the unique IMUL EAX immediate and following LEA
ECX displacement from the supplied executable section bytes, then maps through
its PE section headers. This is an input-derived heuristic verified against
this shipped source, not a general disassembler or proof for another edition.
Missing/ambiguous consumers, truncated PE fields, overlapping section mappings
or unmapped requested bytes produce `HuffmanError::Source` with source context.
The documented fallback on a scan miss is to inspect that input's
`FUN_1500e0c0` operands, run REA `address-to-file-offset <dll> <table-VA>`, and
read the relevant row through that PE mapping. A guessed pointer is never
substituted automatically.

Public additions:

- `HuffmanTable::open` / `from_pe` import local bytes and retain source path,
  observed table VA, row stride and mapped byte counts.
- `MpqArchive::extract` lazily imports a case-insensitively named co-located
  `Storm.dll` on the first Huffman request. Missing DLL retains the typed
  `UnsupportedCompression` refusal. A foreign/modified unsupported DLL produces
  the contextual typed Huffman source error. A missing-source search is cached.
- `MpqArchive::set_huffman_table(bytes, source) -> Result<(), HuffmanError>`
  provides explicit local imports, including browser/memory archives. Wrong-size
  or unsupported bytes fail before replacing an existing table.
- `decompress_sector_with_huffman` and `decode_block_with_huffman` accept an
  optional imported table. Existing entry points retain their signatures and
  refuse Huffman when no table is supplied.
- `PayloadError::Adpcm`, `PayloadError::Huffman`, and public `HuffmanError`
  distinguish stream corruption from unavailable/unsupported source input.

Memory archives do not silently read host files. Browser callers must import
local DLL bytes explicitly; no remote fetch or baked-in weights are available.
The Legacy install's absent DLL therefore needs that explicit import. This
heuristic's admission and codec behavior remain unverified for other DLLs.
`DEFAULT_PRIORITY` and all archive priority decisions were left unchanged.

### Why the old Wine extractor failed

The exact failing member was
`UI\Widgets\Console\Human\infocard-heroattributes-agi.blp` in local `war3.mpq`.
`FUN_15019b60` returned error 2 before decompression:
`ev_e527eb6922a83572be365705b25878a5bb65ad5a9cf88d76bdfdddcd8513c25a`.
The resolver `0x15018e20`:
`ev_9b956d710feb14f580b370d97864239012b0940f3c7e03ad5c8361e8121c8c3b`;
crypt initializer `0x15017650`:
`ev_eda7ae6d3e849e7e99067bcc2fba8d29c001b4169edcfe73871d53966a258712`.

The harness used `LoadLibraryA`. This DLL's process-attach routine at
`0x15035910` (`ev_2cce087d670eb60359f2d8c2d97e5f8d27b32f230f9b87d6dbfb6ba6d278dc8b`)
calls `0x15017bf0` when the reserved argument is null; the latter
sets EAX to `0x10000100` before jumping to the crypt initializer. The archive
APIs' lazy path instead supplies `0x100001`. Dynamic loading initialized the
non-null table first, preventing that lazy path. The loaded module was verified
as the local DLL above at base `0x15000000`; all 1,280 live crypt words differed
(first `0x7b6394af` versus `0x55c636e2`), and scopes 0..3 all failed the same
member. This was a harness loading-mode artifact, not a name/case, compression,
or Rust reader defect.

The native comparison harness now forces a normal static PE import of ordinal
266, using `tests/native/storm.def`, and obtains the module with
`GetModuleHandleA`. Static attachment leaves the lazy initializer available.
All 1,280 live words then matched, the identical member extracted 12,102 bytes,
and `cmp` matched the Rust output. No DLL patch or live-table replacement was
used for these comparisons. An earlier temporary table-replacement experiment
was diagnostic only and is excluded from every oracle result.

At the time of these probes, `tools/rea.sh` shifted away the target before
computing its snapshot path. Direct CLI probes used
`GHIDRA_INSTALL_DIR=$PWD/.tools/ghidra` and the original DLL. The driver has
since been repaired separately: cold and cached routing checks cover the target,
snapshot identity and forwarded options without launching native analysis.

### Measured verification and coverage

`CARGO_TARGET_DIR=/tmp/pcx-mpq-target` kept all build artifacts isolated.
Default tests cover synthetic parser/matcher/codec fixtures; original-input
and Wine oracle tests remain explicitly `#[ignore]`d.

- ADPCM oracle: 16 synthetic sectors, mono/stereo at shifts
  0,1,2,5,31,32,33,255, all opcode bytes plus clamp sequences: all bytes matched
  original export #552. The ignored `local_adpcm` test passed.
- Original `FootmanWhat1.wav`: 23,350 bytes from six sectors matched #552 in
  both the compatibility archive and 1.29b Legacy `War3.mpq`. The Legacy run
  explicitly imported the compat-tree DLL; both ignored `local_huffman` tests
  passed. This does not claim target-native 1.29b parity.
- Compatibility `war3.mpq` payload sweep: `supported=10679 unsupported=0
  compressed_single=0 legacy_imploded=0`.
- `War3Patch.mpq` has no `(listfile)`. A union of 17,137 names from the local
  war3/War3x/War3xlocal listfiles yielded `supported=227 unsupported=0
  compressed_single=0 legacy_imploded=0`. This is candidate-name coverage;
  unlisted patch blocks remain an explicit enumeration limitation.
- Native direct byte comparison, `war3.mpq`: `payload_files=10679
  identical=10679 mismatched=0 refused_files=0 refused_codec_sectors=0
  refused_kind_sectors=0`; original archive API: `archive_files_opened=10679
  identical=10679 mismatched=0 refused_open=0 refused_read=0`.
- Native direct byte comparison, patch candidates: `payload_files=227
  identical=227 mismatched=0 refused_files=0 refused_codec_sectors=0
  refused_kind_sectors=0`; original archive API: `archive_files_opened=227
  identical=227 mismatched=0 refused_open=0 refused_read=0`.

The ignored `local_storm` test checks actual bytes, not digest equality, and
requires both native file comparison and independent #552 sector comparison
counts to agree. `export_storm_comparison` writes the local-only PXC1 stream;
its original contents must stay outside the repository. Native build recipe:

```sh
i686-w64-mingw32-dlltool --def crates/pcx-mpq/tests/native/storm.def --output-lib /tmp/pcx-mpq-oracle/libstorm.a
i686-w64-mingw32-gcc -O2 -Wall -Wextra -Werror -o /tmp/pcx-mpq-oracle/storm_compare.exe crates/pcx-mpq/tests/native/storm_compare.c /tmp/pcx-mpq-oracle/libstorm.a
```

Place the user's own DLL/runtime dependencies beside that executable locally.
Generate the stream with
`cargo run -p pcx-mpq --example export_storm_comparison --locked -- <archive>
<candidate-names-or-dash> <local-output>`. `local_storm` requires
`PEACECRAFT3_STORM_COMPARATOR`, `PEACECRAFT3_STORM_COMPARE_ARCHIVE` and
`PEACECRAFT3_STORM_COMPARE_STREAM`; archive/stream operands use Wine paths.
`local_huffman` requires `PEACECRAFT3_FORMAT_MPQ`,
`PEACECRAFT3_STORM_AUDIO_MEMBER`, `PEACECRAFT3_STORM_AUDIO_OUTPUT`, and optionally
`PEACECRAFT3_STORM_DLL` for an explicit local import. `local_payload` optionally
uses `PEACECRAFT3_FORMAT_MPQ_NAMES` to declare its candidate inventory.

`mpqxtract` list/show/get support quoted ASCII case-insensitive globs: `*` and
`?` stay in a segment; a whole `**` segment spans zero or more segments; both
slash spellings agree. Glob `get` writes relative archive paths below an output
directory and rejects traversal/symlink destinations; literal `get` preserves
its existing single-output-file behavior. Listfile absence/malformed UTF-8 and
show/get no-match results are reported, not silently synthesized.

Live demonstrations: root `*.blp` selected zero entries; recursive `**/*.BLP`
selected 3,935; `UI/Widgets/Console/Human/*.BLP` selected 58; the
`infocard-heroattributes-???.blp` pattern showed and extracted three 12,102-byte
files, with slash/case normalization. Duplicate archives selected three rather
than six copies. A missing wildcard path returned exit 1 with `not found`.

Sparse (`0x20`), bzip2 and LZMA remain typed refusals. The evidenced Storm
`0x1500dd30` rejects those mask bits rather than providing a sparse decoder;
implementing one would need a different identified input and its exact dispatch
and run encoding. No archive priority or target-native runtime claim follows
from the codec/parsing tests.

Final strict commands and observed summaries:

```text
CARGO_TARGET_DIR=/tmp/pcx-mpq-target cargo test -p pcx-mpq --locked
  test result: ok. 38 passed; 0 failed; 0 ignored (library)
  test result: ok. 13 passed; 0 failed; 0 ignored (CLI)
  four explicit local oracle/fixture tests ignored by default
CARGO_TARGET_DIR=/tmp/pcx-mpq-target cargo build -p pcx-mpq --locked
  Finished `dev` profile [optimized + debuginfo]
CARGO_TARGET_DIR=/tmp/pcx-mpq-target cargo build -p pcx-mpq --locked --target wasm32-unknown-unknown
  Finished `dev` profile [optimized + debuginfo]
CARGO_TARGET_DIR=/tmp/pcx-mpq-target cargo clippy -p pcx-mpq --locked --all-targets -- -D warnings
  Finished `dev` profile [optimized + debuginfo]
```

All exited zero with no warnings. Existing strict-Clippy issues within this
crate were resolved with fixed-array chunks, temporary reader lifetimes and
let chains; no lint suppression was added. No dependency or manifest change
was required. No git mutation was performed and original assets, runtime
imports and comparison streams stayed local and uncommitted.

## 2026-10-20: Legacy codec source and WAV verification

### Legacy Huffman weights

`Warcraft III.exe` (Legacy 1.29.2, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`) holds the
Huffman weight table in `.rdata` at VA `0xe6ada0`. Its consumer has the Storm shape:
`IMUL EAX,EAX,258` at VA `0x443537`, then `LEA EAX,[EAX+disp32]` whose displacement at
`0x44353f` is the table address. `HuffmanTable::from_pe` reads 66,048 file-backed
bytes with no zero fill. The compatibility `Storm.dll` table has 25,408 zero-filled
bytes.

`MpqArchive::extract` takes `Storm.dll` from the archive directory and falls back to
`Warcraft III.exe`. `native_campaign` imports the same executable explicitly.

### Native Storm oracle (primary evidence)

The compatibility-tree `Storm.dll` (SHA-256
`eecc42b05a91413dee10588a654c6cca05836e7513af6c7c568a56124bf97d0a`) was run under
Wine through `tests/native/storm_compare.c`. It opened the Legacy `War3.mpq` and read
each `.wav` member through Storm's archive API. The sector decoder, ordinal 552, was
called on the packed sectors. Native results against the Rust output:

- All 2,716 `.wav` members: payload sectors and archive-open bytes identical, 0 mismatches,
  0 refusals. Reproduced 2026-10-20 with retained receipts under
  `.scratch/native-oracle-final/` (commands, hashes, logs, digests, negative controls).
  Status: reproduced, not yet qualified by root. The oracle is compat `Storm.dll` under
  Wine, not the Legacy 1.29 runtime or the MILES PCM path.
- A flipped byte in one expected sector is reported as a mismatch (negative control).
- Native archive-open bytes are dumped per member and compared by
  `tests/local_huffman.rs::original_wav_members_match_storm_archive_open_bytes`
  (ignored; needs `PEACECRAFT3_FORMAT_MPQ` and `PEACECRAFT3_STORM_NATIVE_DIR`).

So the Huffman and ADPCM stages agree with Storm's own decoder, and the Legacy exe
table decodes every WAV member to the same bytes as the compatibility table. The
Storm DLL run is a Wine-hosted native execution, not Legacy runtime proof.

### Attributes CRC is not a decoder oracle for ADPCM members

`(attributes)` CRC32 matches the decoded bytes for all 6,463 non-WAV members that
decode. For the 2,716 `.wav` members, a direct sector-mask scan shows the partition is
exact: all 11 matches have only zlib (`0x02`) sectors, and all 2,705 mismatches contain
an ADPCM mask (`0x40` or `0x80`). Native Storm agrees with our output on every member.

The CRC-to-ADPCM correlation is consistent with the CRC describing the original 16-bit
PCM before lossy ADPCM encoding. That is an inference: no MPQ format source was checked
for attributes semantics, and no pre-encoding original was available. Decoder fidelity
rests on the native oracle. The CRC does not gate WAV decoding.

Measurement tool: `cargo run -p pcx-mpq --example attributes_crc -- <archive> <suffix>`.

### Remaining gates

- Sample-level fidelity is established against the Storm archive-open bytes. Actual
  playback and the MILES PCM path are not verified.
- The native gate needs a local dump: build `storm_compare.exe` with
  `i686-w64-mingw32-gcc` and `storm.def`, then run it under Wine with the compatibility
  `Storm.dll` beside it and a `PXC1` stream from `export_storm_comparison`.

### ADPCM tables come from the codec source

The 89 step values and the 32 index adjustments are no longer compiled into
`pcx-mpq`. `HuffmanTable::from_pe` locates them in the codec source's file-backed
section bytes. The step run must be strictly increasing, start above zero and end at
`0x7fff`. The index run is the 32 dwords directly before it, each in `-1..=8`.

Exactly one step run must match. Zero matches give `adpcm: None`, so ADPCM sectors
refuse with `UnsupportedCompression`. Two matches are a source error.

Observed locations, both unique in their sources:

| Source | Steps VA | Index VA |
| --- | --- | --- |
| compat `Storm.dll` (SHA-256 `eecc42b0…`) | `0x15042ad8` | `0x15042a58` |
| Legacy `Warcraft III.exe` (SHA-256 `3f2ed012…`) | `0xe6ac38` | `0xe6abb8` |

Both sources hold the same bytes, which shows the two binaries agree. It does not
establish that the values are an independent format invariant. The 89-entry step
sequence resembles the public IMA ADPCM step table. The index adjustments have
32 entries with every even entry -1. This was not checked against a primary IMA reference
here, so neither point is treated as a constant.

Decoder fidelity to the compat Storm oracle is reproduced for all 2,716 WAV members
(status above).
The initial index of 44 per channel is still a literal in `payload.rs`. It is not
covered by the source-derived table path.

### Codec source flow in the browser and bounded readers

- `peacecraft3::browser::commit_archive_import` parses the staged client image with
  `HuffmanTable::from_pe` and gives the table to every archive in the transaction.
  An absent or unparsable image is a commit note. The archives still commit, and only
  members that need the codec refuse.
- `MpqArchive::set_codec_table` and `codec_table` expose the imported table. Bounded
  readers (`peacecraft3::browser_read::read_bounded`, `pcx-assets` bounded reads) use a
  per-call table when given and otherwise the archive's imported table. The clone per
  read is about 64 KiB of weights.
- No executable is discovered in the browser. `stored_asset_bounded` still refuses a
  per-call table through the `"no-huffman"` policy.
- Pure mono ADPCM sectors (`0x40`) occur in shipped Legacy WAVs: 9 sectors in 9 members
  (first-byte scan, verbatim sectors excluded). They are in the native oracle set.
  The panel rule (`panel_codec_mask`) treats `0x40`/`0x80` like `0x41`/`0x81` as
  single-source codec capability: without a codec source the sound is omitted and
  reported as a gap, not decoded.

### Test fixtures

Unit tests use synthetic run pairs generated by formula: steps `7 + 367*i` with the
final value `0x7fff`, and index adjustments alternating `-1` and `i % 9`. Expected
decoder samples come from an independent script. No observed production table value
appears in test code.

Still literal: the initial ADPCM index of 44 per channel. It has no independent format
citation and no source operand is derived for it yet. The native oracle reproduces it
for the corpus, which is not a source-derived proof. The ADPCM path is therefore only
partly source-derived.
