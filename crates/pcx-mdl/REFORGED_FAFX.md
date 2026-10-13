# Version-1800 raw FAFX records

This accessor establishes stored framing only. It does not interpret text,
resolve actor files, run FaceFX, select a head anchor or enable rendering.

## Evidence and the absent count prefix

The primary implementation is WhiteoutLib revision
`92887f9156cdaca8966b3b58e2625628aa0c2c30`:
[parseFAFX](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/mdx/parser.cpp#L1550)
derives record count from payload size divided by 340;
[writeFFX](https://github.com/FernandoS27/WhiteoutLib/blob/92887f9156cdaca8966b3b58e2625628aa0c2c30/src/whiteout/models/mdx/writer.cpp#L899)
writes consecutive 80-byte name and 260-byte path fields. There is **no embedded
count word**. FAFX payload framing is `count * 340`, not `4 + count * 340`.
The adjacent BPOS format has a four-byte count, followed by 48-byte records;
that prefix must not be transplanted into FAFX.

The checked configured-TVFS HD fixture is build `3.0.0.24268`, path
`war3.w3mod:_hd.w3mod:units/human/footman/footman.mdx`, MDX version 1800:

- CKey `6fcd268161babbd73f4601060be39df5`.
- EKey `55ddc4ab832e5eadc526d91d40cdd9dd`.
- FAFX tag at `0x4367c2`; payload `[0x4367ca,0x43691e)` is exactly 340 bytes.
- One record: name `[0x4367ca,0x43681a)`, path `[0x43681a,0x43691e)`.

Those identities and offsets describe the gated original fixture; they are not
decoder constants. Original bytes are read only into local memory.

## Additive API and checked policy

`Mdlx::fafx_stream_v1800(bytes)` returns
`Result<Option<FafxStream1800<'_>>, Error>`. Call it with the same bytes used
by `Mdlx::parse_stored_v1800`. Version 1800 is the only enabled version.
Missing FAFX is `None`; an explicitly present empty payload is a stream with
zero records. No default record or text is supplied.

The stream exposes version, absolute tag/payload offsets, `data()`,
`record_count()` and `get(index)`. A record contains its absolute source
offset, a borrowed `&[u8;80]` name and a borrowed `&[u8;260]` path. The path
starts at record offset + 80. NULs, trailing bytes, non-UTF8 and unterminated
fields are preserved verbatim. No allocation, decoding or re-encoding occurs.

The local **checked import policy** permits at most one FAFX chunk. Matching,
conflicting and empty duplicates return `UnsupportedVersionedChunk` with
`chunk: "duplicate FAFX"`. Non-340 remainders return `RaggedRecords` with
stride 340; payload bounds/offset overflow return `Truncated`. Record width
multiplication and range arithmetic are checked before slicing. Out-of-range
record access, including overflowing indexes, returns `None`.

This policy is not native malformed-input rejection parity. No existing error
variant or accessor is changed. Strict `Mdlx::parse` still refuses FAFX, and
the strict HD mesh path still refuses TANG/SKIN. Joint mapping, BPOS and weight
conversion are separate work and are untouched.

## Tests and residual unknowns

Synthetic tests cover two exactly tiled records and borrowed field offsets,
all raw bytes including non-UTF8/NUL/trailing/unterminated fields, missing versus
empty chunks, short/long records and remainders, an erroneous four-byte count
prefix, duplicates, unsupported versions, truncated inputs and offset/index
overflow. The exact keyed HD-Footman gate compares every FAFX byte and pointer
against its input slice and retains the strict parse/mesh refusals.

Native FaceFX behavior, text encoding, actor-path resolution, animation and
render fidelity are unproved and not implemented. No source game bytes are
embedded in the tests or this note.

## Validation

All commands used the private target directory and the unchanged lockfile:

```sh
CARGO_TARGET_DIR=/tmp/pcx-mdl-fafx-target cargo test -p pcx-mdl --locked
CARGO_TARGET_DIR=/tmp/pcx-mdl-fafx-target cargo test -p pcx-mdl --locked -- --ignored --nocapture
CARGO_TARGET_DIR=/tmp/pcx-mdl-fafx-target cargo clippy -p pcx-mdl --locked --all-targets --no-deps -- -D warnings
```

The narrow run passed 55 library tests and 24 synthetic integration tests
(eight new FAFX tests), with zero failures. Eight original-fixture tests are
ignored by default. The ignored run passed all eight with explicit configured
SD/HD paths and the compatibility install; a second run passed all eight with
the legacy install. Classic corpus lines were `mdx=3234 parsed=3234 failed=0`
and `mdx=3249 parsed=3249 failed=0` respectively. The keyed HD gate printed
`FAFX_records=1 payload=[0x4367ca,0x43691e) bytes=340 name_bytes=80 path_bytes=260`.
Clippy exited 0: `Finished dev profile [optimized + debuginfo] target(s) in 3.50s`.
Formatting and `git diff --check` were clean.

Full logs are local at `/tmp/pcx-mdl-fafx-target/{tests,originals,originals-legacy,clippy}.log`.
The ignored runs explicitly set `PEACECRAFT3_INSTALL` and
`PEACECRAFT3_FORMAT_INSTALL` to the chosen MPQ install,
`PEACECRAFT3_LEGACY_DIR` to the installed legacy tree,
`PEACECRAFT3_REFORGED_DIR` to the supplied CASC install,
`PEACECRAFT3_REFORGED_ASSET` to the configured SD Footman path and
`PEACECRAFT3_REFORGED_HD_ASSET` to the configured HD Footman path recorded above.
