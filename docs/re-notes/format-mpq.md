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
Huffman/ADPCM and other compression chains remain explicit gaps. No original
runtime comparison or byte-for-byte Storm extraction oracle was run.
