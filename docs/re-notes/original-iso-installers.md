# Original ISO installer payload

The locally supplied image has valid ISO9660/Joliet directories containing an
ElAmigos Inno Setup installer and six encrypted FreeArc archives. The browser
now discovers its files locally, but cannot yet unpack its installer payload.
Checked static script parsing supplies the archive password arguments; both the
base and English archive control directories decrypt, LZMA-decode and match
their stored CRCs. The base MPQs still require solid-block codecs, and the
language patches' format is unverified. This does not close PLAN M11.

No installer, game, Wine process, plugin or codec executable ran. Temporary
`SETUP.EXE` copies were deleted after bounded inspection. Original configuration,
compiled script, password values and decrypted blocks stay in ignored private
scratch directories. None is shipped with the demo or committed here.

## Source

| Field | Value | Evidence |
|---|---|---|
| Size | 3,747,354,624 bytes | `stat` |
| SHA-256 | `e7d4d2a42555bbfc142c58d7b0fe224f5959eb0e48543548a3a6a3b35c22f5ce` | `sha256sum` |
| Filesystem | ISO 9660 PVD (sector 16) + Joliet SVD (sector 17, escape `%/E`) + terminator (sector 18) | shipped bytes |
| Volume | `WARCRAFT3`, 1,829,763 × 2048 = source size; application `ULTRAISO V9.5` | PVD |
| UDF observations | No recognition descriptors in the checked post-terminator window; sector 256 is not an anchor (tag 2). No UDF decoder or image-wide absence proof. | bounded shipped-byte reads |

Neither the volume label nor the installer title establishes a Warcraft III
release. The installer metadata's `1.27b` label does not establish its archives'
release identity.

## Current browser discovery

At source `fd4a6e0`, `discoverIso` selects the single recognized Joliet namespace,
decodes its big-endian UCS-2 identifiers and follows its own directory extents.
The supplied image yields ten files and zero direct MPQ candidates in 15 reads,
8,855 bytes total, largest read 2,048 bytes. The six archive signatures produce
`UNSUPPORTED_FREEARC`; the installer produces `UNSUPPORTED_PE_PAYLOAD`.
Earlier `8d1ef9f` rejected the supplementary descriptor at byte 34816. That was
parser policy, not a malformed image, and is fixed. Runtime File-input discovery
also passes in the packaged demo without uploading image or archive bytes.

The Joliet root's long names match the compiled installer's references:
`elamigos-1.bin` is primary `/ELAMIGOS.BIN`; the Spanish, Polish, Italian,
German and English archives correspond to primary `/ELAMI~01.BIN` through
`/ELAMI~05.BIN`, respectively. Matching is by authored extent, not filename
priority. The volume label and filenames do not identify a game release.

## Installer configuration and compiled script

`SETUP.EXE` is 2,358,845 bytes, SHA-256
`611208d0c86a6b0f98f8e2a56c795c53dec6ad9695558721063ebfdb64c2f66f`.
`innoextract 1.9 --list-sizes --show-password` identifies Inno Setup 5.5.0
(unicode), titled `WarCraft III Complete Edition MULTi6 - ElAmigos`. It embeds
codec DLLs/configuration and encrypted `batch.bat`/`xdelta3.exe`, not MPQs.
The separate installer password remains distinct from the archive arguments.

Selective `innoextract -e -I tmp/arc.ini -I tmp/cls.ini` extracted only 352 and
65 bytes. The configuration declares `srep`, `precomp`, `msc`, `pzlib` and memory
settings, but contains no password. Declarations do not prove which methods an
archive uses; decoded directory records establish those below.

Setup-0 starts at member offset 1,639,298. Framing derived from innoextract 1.9
validates the block header CRC and all 39 chunk CRCs. LZMA1 yields 370,709
header bytes, SHA-256
`957667c9a6d4a5a6806981211c5fc443e59c835411af5a1f491873062c7e5c04`.
Its 31st length-prefixed string is a 21,537-byte `IFPS` script, SHA-256
`98290d9010dcccb31f76023663f4e62b1dc3202cdbee420aa8139e5dc1fd8372`.
The header's `app_version` says `1.27b`, which identifies installer metadata
rather than the archived files' release.

A checked static parser against Pascal Script `TPSExec.LoadData`/`RunScript`
reads build 23, 61 types, 134 procedures (118 imports), 22 globals. Reverse
argument order is cross-checked at a `FindFirst` call. `CURSTEPCHANGED` calls
`ISArcExtract` twice with ten arguments:

| Call offset | Archive | Argument seven | Configuration | Destination |
|---|---|---|---|---|
| 1856 | `ExpandConstant('{src}\elamigos-1.bin')` | 32-character literal at procedure offset 1590 | `{tmp}\arc.ini` | `{app}\` |
| 2831 | Each `elamigos-*.bin` name containing `ActiveLanguage`, excluding directories | 32-character literal at procedure offset 2546 | `{tmp}\arc.ini` | `{app}\` |

Twelve derivations, only these two authored arguments against the six archive
footers, show that the first matches only the base archive's two-byte check and
the second matches only the five language archives. No other passwords were
tried. The script does not reference `batch.bat` or `xdelta3.exe`; installer run
entries and any patch invocation remain uninspected.

## Source-compatible control-block decoding

Pinned FreeArc 0.67 `GenerateDecryption` uses PBKDF2-HMAC-SHA512 with the password,
footer salt and iteration count. AES-256 consumes the first 32 derived bytes;
the next two are compared with the footer check. Salt/check decoding is ordinary
hex. The resulting key is hex-encoded again before the cipher method decodes it.

`ENCRYPTION_METHOD::decompress` uses `buggy_decode16` for both key and IV unless
`:f` is authored; these methods have no `:f`. Its letter nibbles map to 0 through
5 instead of 10 through 15. Five discriminating synthetic cases pass. The first
probe omitted this branch and failed; its inference that the shipped DLL must
differ from upstream is withdrawn.

CTR uses a little-endian counter. The optimized default-rounds path stores
IV minus one then pre-increments, so its first keystream block uses the IV.
WebCrypto's built-in AES-CTR counter convention cannot be used unchanged.
The cipher chain is undone right to left before LZMA. Corrected control blocks
match decoded lengths and stored CRC-32s; this establishes stored-format/source
compatibility, not execution or equivalence of the shipped `unarc.dll`.

## English language archive

Only footer/directory spans of `/ELAMI~05.BIN` (`elamigos-english.bin`) were
read. The footer descriptor at member offset 426,163,183 validates its CRC.
The 177-byte encrypted footer at 426,163,006 yields exactly 186 bytes with
matching CRC. Its directory yields 3,307 to 7,523 bytes, also CRC-checked.
Two bytes follow the footer comment field and one follows the directory fields;
their complete semantics remain unknown.

There are 187 entries: six directories, 181 files, 426,635,345 file bytes.
Every method chain ends in AES-256-CTR with record-derived parameters:

| Block | Chain before AES | Entries | Original bytes |
|---:|---|---:|---:|
| 0 | `storing` | 6 directories | 0 |
| 1 | `grzip:179kb:m4:l32:h15` | 4 files | 180,580 |
| 2 | `rep:96mb:256+4x4:tor:3:8mb` | 176 files | 426,450,097 |
| 3 | `rep:6kb:256+exe+4x4:tor:6kb:h32kb` | 1 file | 4,668 |

All files have `.patch` names: 116 `.w3x`, 45 `.w3m`, 12 `.mpq`, four `.txt`,
three `.exe`, one `.w3n` targets. No MPQ itself occurs here. None of the declared
srep/precomp/msc/pzlib methods occurs in this directory. The patch bytes and
format were not decoded; shipping `xdelta3.exe` does not prove VCDIFF.

## Base archive directory

The source-corrected reader (key/IV `buggy_decode16`) under
`.scratch/iso-installer-base-directory/` decoded only the footer and directory
of `/ELAMIGOS.BIN` (`elamigos-1.bin`, member offset 53248, size
1,176,066,217) with the literal bound to it (proc offset 1590). No data block
was read.

- Footer descriptor at member offset 1,176,066,050: CRC passes, type 4, same
  method as the English footer, 177 → 186 bytes, password check value and
  stored CRC-32 match. It lists a stored 8-byte header block and one directory
  block (5,794 → 12,516 bytes). Two bytes follow the comment field; their
  meaning is unknown.
- Directory: decrypts and LZMA-decodes to exactly 12,516 bytes and its stored
  CRC-32 matches. One byte follows the last field `DIRECTORY_BLOCK` reads; its
  meaning is unknown.

405 entries (21 directories, 384 files, 1,290,952,195 bytes) in four data
blocks, each chain ending in `aes-256/ctr:n1000:r0` with its own IV, salt and
check value:

| Block | Chain before AES | Files | Original bytes |
|---:|---|---:|---:|
| 0 | `storing` | 21 directories | 0 |
| 1 | `grzip:712kb:m4:l32:h15` | 103 | 720,705 |
| 2 | `rep:96mb:256+4x4:tor:3:8mb` | 182 | 1,257,636,361 |
| 3 | `rep:32mb:256+exe+4x4:tor:16mb` | 99 | 32,595,129 |

All 13 `.mpq` members are in block 2 (1,211,841,261 bytes):

| Path | Size |
|---|---:|
| `war3.mpq` | 447,331,787 |
| `War3Patch.mpq` | 125,201,961 |
| `War3x.mpq` | 402,990,907 |
| `War3xlocal.mpq` | 46,073,074 |
| `Movies\HumanEd.mpq` | 20,393,390 |
| `Movies\HumanOp.mpq` | 26,366,936 |
| `Movies\IntroX.mpq` | 19,385,684 |
| `Movies\NightElfEd.mpq` | 19,138,382 |
| `Movies\OrcEd.mpq` | 28,432,146 |
| `Movies\OutroX.mpq` | 23,743,732 |
| `Movies\TutorialIn.mpq` | 16,447,210 |
| `Movies\TutorialOp.mpq` | 21,283,740 |
| `Movies\UndeadEd.mpq` | 15,052,312 |

These are directory names and sizes, not verified MPQ headers, contents,
editions or archive priority. Every English `<path>.patch` name has a base
member of the same path; `Movies\UndeadEd.mpq` has no English patch. That
base-plus-patch relationship is a name match only, and the patch format is
unread.

Browser import consequence: the MPQs sit in a FreeArc data block using
`rep` + `4x4:tor` under AES-256-CTR, and the language archive supplies
same-named `.patch` files. A local import would need FreeArc framing,
PBKDF2-HMAC-SHA512, AES-256-CTR with the 0.67 hex policy, LZMA, REP and
Tornado decoders, and, if the patches must be applied, a patch decoder whose
format is unestablished.

Remaining open targets: decode one data block to
verify an MPQ header (base block 2) and identify the patch format (English
block 2).

## Reproduction and retained evidence

Primary sources: [FreeArc 0.67](https://github.com/mrkaban/FreeArc-0.67/tree/537e4ff921e55da60b27bdbae0bc757010be7e53),
[Pascal Script](https://github.com/remobjects/pascalscript/tree/f94aa665559b85f35c9da96c7d3876a1fe7ad8e4),
[innoextract 1.9](https://github.com/dscharrer/innoextract/tree/1.9).
Relevant code is `Unarc/ArcStructure.h`, `Unarc/ArcProcess.h`,
`Compression/_Encryption/C_Encryption.cpp`, `Compression/Common.h`,
`TPSExec.LoadData` and innoextract's `stream/block.cpp`/`setup/header.cpp`.

Private evidence lives under `.scratch/iso-installer-{evidence,config,script,
directory,hex-policy,base-directory}/`; later directories retain scripts,
command output and `artifacts.sha256`. The failed first attempt is retained
separately. Independent review rechecked the source ISO digest, both corrected
footer/directory lengths and CRCs, entry totals and the 13 MPQ-named base records.
No main data block, MPQ header or patch was decoded. Browser tests prove discovery
and actionable refusal, not installed-game extraction or native fidelity.
