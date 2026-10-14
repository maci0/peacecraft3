# pcx-assets

A source-resolution library for PLAN §3.2/§3.6/§3.9. It implements
the filesystem override layer, explicit archive priority, per-path provenance
and generated `override/index.json`. Concrete adapters use the checked MPQ and
CASC readers. It does not select installs or native priorities, stage copies,
apply gameplay fixes, or certify original behavior.

## Modes

`GameMode` parses and formats exactly
`<build>-<addon>-<assets>-<behaviour>`:

| Axis | Accepted components |
|---|---|
| build | `1.29b`, `1.28c` |
| addon | `tft`, `noaddon` |
| assets | `vanilla`, `hd`, `reforged` |
| behaviour | `vanilla`, `reforged`, `fixes` |

Default: `1.29b-tft-vanilla-vanilla`. Components are positional and independent.
Changing art never changes behavior, archive order, or gameplay data. Legacy
combined aliases from PLAN §3.9 are described by the separate §3.6 axes rather
than accepted as ambiguous mode strings. `is_fidelity_baseline()` identifies
vanilla art/behavior policy for the selected build/addon, not proven parity.

## Sources and resolution

`OverrideDirectory::open(root, id, version)` recursively mounts an existing real
directory. `version: None` records unknown/unversioned input explicitly; no
edition is inferred from a folder or requested game mode. Canonical directory
origin, actual filename and observed size are retained.

Project path identity uses `/` separators and ASCII lowercase. Both `/` and `\`
are accepted in lookup/archive member names. Non-ASCII characters remain intact;
no native CASC Unicode/case rule is assumed. Absolute paths, drive/stream colons,
control characters, empty components, `.` and `..` are rejected. Filesystem
case aliases and archive members colliding after normalization are refused.
Original archive names are passed to the adapter verbatim.

An `ArchiveLayer` supplies a unique numeric `priority` and a boxed
`ContentSource`. Higher numbers win; duplicate priorities and duplicate layer
IDs are errors. The override directory wins before every archive. There is no
built-in archive-name order or art-mode priority. The caller supplies the ordering
and chooses the relevant art/gameplay sources; this policy is not native evidence.

`ContentSource` has four methods:

* `info()` supplies layer ID, MPQ/CASC kind, source identity and detected version
  or explicit unknown version. These identify the input, not the requested mode.
* `paths()` supplies the mounted logical inventory, which can be an explicitly
  selected subset. Unavailable inventories must fail rather than claim an empty
  archive or full coverage.
* `read(member)` supplies decoded bytes, source-declared length and exact source
  locator. Adapters must validate sizes/offsets before allocating or reading.
  Missing, malformed, ambiguous and truncated input can return `SourceError`.

* `read_bounded(member, decoded_byte_limit)` is an additive opt-in contract. It
  refuses known oversized content before payload allocation/read/decode. Its
  default returns `SourceError::BudgetUnsupported` without calling `read`. An
  implementing adapter must enforce the ceiling through its payload reader;
  returning an oversized allocation for the resolver to reject is insufficient.

The resolver validates lengths and locators. A listed winning member that is
missing or fails to decode aborts resolution; it never falls back to different
contents from a lower layer. Empty, correctly sized content is a valid asset.
Errors retain the affected layer, path/location and underlying source failure.

The override inventory is a mount-time snapshot. Existing files are read on
demand, with sizes revalidated against that snapshot; remount after adding,
removing or resizing files. No decoded-asset cache is maintained. Symlinks and
special files below the root are refused, and ancestors are checked again before
reads. This is ordinary local filesystem access, not a transactional snapshot
or isolation from concurrent hostile filesystem changes.

## Index and usage

`Resolver::resolve` returns `ResolvedAsset { path, bytes, provenance }`.
Provenance contains the winning `SourceInfo`, caller archive priority (null for
override), exact locator and byte length. `layer_info` refuses unknown IDs;
`paths` iterates the sorted union of all inventories.

`Resolver::resolve_bounded(path, decoded_byte_limit)` uses the same winning
source and receipts as `resolve`. The override reader revalidates mounted and
current file sizes and refuses an oversized file before its payload reserve or
read. It retains the existing one-byte growth probe, so this is a decoded-content
ceiling, not an exact allocator or process-memory ceiling. Mount inventory,
metadata checks and error allocations are outside this per-read contract.
Legacy-constructed MPQ adapters inherit the typed unsupported-budget response;
their existing `read` methods and mount-time decoder limits remain available.
There is no fallback to another layer or to unbounded `read`. Existing `resolve`,
`index` and `write_index` keep their existing behavior.

`Resolver::index` resolves every inventoried path and returns `AssetIndex`.
`AssetIndex::to_json` encodes a deterministic schema-version-one JSON object with
the complete mode string and a sorted path-to-winning-provenance map. It contains
no asset payloads. Unknown versions remain null, not guessed build identities.

`Resolver::write_index` generates the override root's `index.json`. It validates
all winners first, writes a complete temporary file, and renames it into place.
A failed resolution leaves the previous index unchanged. Root `index.json` and
`.pcx-assets-index-*` files are reserved generated metadata, excluded from override
assets; case aliases of `index.json`, directories and symlinks there are refused.
Without an override directory, `index`/`to_json` work and `write_index` reports
the missing output layer.

```no_run
use pcx_assets::{GameMode, OverrideDirectory, Resolver};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mode: GameMode = "1.29b-tft-hd-vanilla".parse()?;
    let overrides = OverrideDirectory::open("override", "user", None)?;
    let resolver = Resolver::new(mode, Some(overrides), vec![])?;
    let asset = resolver.resolve("textures/example.blp")?;
    println!("{}: {}", asset.path, asset.provenance.source.id);
    resolver.write_index()?;
    Ok(())
}
```

Generated indexes, local override trees and original payloads remain local.
Default tests use synthetic fixtures. The tests cover all 36
mode combinations, default/fidelity policy, real override directories, archive
priority, provenance/JSON escaping, deterministic output and typed failure paths.
They prove library policy and filesystem behavior, not native archive order,
CASC/MPQ native behavior or the Reforged Footman rendering gate. Two explicitly
ignored original tests additionally check a named MPQ member and a named TVFS
asset through the real adapters, comparing content and retained source identity.

## Checked archive adapters

`NamedMember { asset, member }` maps a safe project-layer identity to an exact
MPQ member or stored TVFS namespace. The mapping is caller policy. CASC colon
mounts belong to `member`, never to the logical filesystem path. Duplicate
logical names after normalization are refused. This does not infer shorthand,
module, locale, HD, archive order or complete storage inventory.

`MpqSource::new(archive, id, version, members, decoded_byte_limit)` consumes an
existing checked archive, validates each declared member and its decoded size
before extraction, and refuses multiple locale/platform candidates. `None`
records an unknown version; adjacency to an executable does not identify the
archive. The existing reader is serialized behind a mutex. Missing codecs remain
errors; codec tables are supplied to the archive before mounting when required.

`MpqSource::new_bounded(archive, id, version, members, ExtractionBudget,
optional_huffman_table)` supplies explicit packed/decoded policy and owns the
optional caller-provided Huffman table. Its `read_bounded` clamps the decoded
ceiling to the caller read limit and uses `MpqArchive::extract_bounded` before any
packed payload read/clone or decode. Extraction errors remain typed under
`SourceError::MpqExtraction` with archive and exact member context. The legacy
constructor has no packed policy, so its bounded reads remain explicitly
unsupported. Its existing unbounded `read` behavior is unchanged.

`MpqArchive::extract_bounded(member, budget, optional_table)` accepts dynamic
member strings and uses the existing crypt/sector/payload implementation. Before
packed reads it validates version-zero header/table consistency, sector metadata,
strict selection, packed/decoded allocation layouts, byte ceilings and declared
archive/backing spans. Identical locale/platform/block hash entries are allowed;
distinct candidates refuse rather than guessing a locale priority. Missing input,
malformed metadata, budget/layout failures and decoder errors retain their typed
causes and caller member/source context. File-backed reads recheck backing length;
this is not a transactional filesystem snapshot.

Only the supplied Huffman argument is used: the bounded API neither searches for
executables nor falls back to a previously installed archive table. `None` refuses
Huffman through the existing decoder. Importing/parsing that optional table and
archive mount/table allocation are outside this per-member policy. Packed and
decoded limits do not bound simultaneous decoder buffers or total process RSS.
The decoder retains its existing PKWARE support for binary literals and dictionary
widths 4 through 6. Other dictionaries/literal modes, unavailable codecs and
unsupported compression chains remain typed refusals; no native codec parity is
claimed. Existing `extract` and its legacy discovery behavior remain unchanged.

`CascSource::new(storage, tvfs_resolver, id, members)` checks matching build
configuration and resolves every selected exact namespace. Its version comes
from `.build.info`. Reads verify EKey/chunk checksums, decoded size and CKey
through `Storage::read_path`; unsupported layouts/codecs remain errors.
`CascSource::read_bounded(asset, decoded_byte_limit)` reuses that verified path.
It checks resolved TVFS/ENCODING decoded size against both the caller ceiling and
mounted decoded policy before encoded payload reads. The existing BLTE decoder
receives the narrowed ceiling (also no larger than the resolved declared size),
retaining mounted encoded limits and all EKey/chunk/CKey/length checks. Typed CASC
failures remain under `SourceError::CascRead`; legacy `read` remains compatible.
This is a decoded payload ceiling, not a bound on mount, encoded, codec workspace,
parsed metadata or total process allocations. Exact selected receipts are unchanged.

Public synthetic mounted-source regressions cover exact N/Z reads, missing-file
metadata preflight, no lower-source fallback, checksum failures and forged small
decoded sizes. The explicitly supplied original TVFS member gate compares bounded
and legacy bytes/receipts, and checks a one-byte-smaller caller ceiling. These are
native file-backed checks; WASM compile checks do not establish browser CASC I/O.

`identity(asset)` exposes the complete configuration/encoding/manifest chain.
The returned locator also records keys, manifest links/spans and archive offset.

Both adapters expose only their declared logical inventory. An inventoried winner
that fails decoding stops resolution; a lower-priority member is not substituted.
The four adapter regressions cover actual synthetic MPQ table decoding, exact
locator/size retention, duplicate/missing mappings, decoded budgets, locale
ambiguity and refusal to fall through an unsupported winning member.

The gated originals passed for `UI\war3skins.txt` (44,153 bytes, explicitly
unknown archive version) and the stored Footman namespace (105,602 bytes,
build 3.0.0.24268). These are selected-reader checks, not full-inventory,
application-mode wiring, native lookup or rendered fidelity proof.

## Build and consumer integration

The crate shares the workspace's Rust 2024, minimum Rust 1.99, lockfile and
warning-denial policy. Direct dependencies were verified against the primary
crates.io registry: serde 1.0.229, serde_json 1.0.154 and thiserror 2.0.21.

```sh
CARGO_TARGET_DIR=/tmp/pcx-assets-target cargo test --locked -p pcx-assets
CARGO_TARGET_DIR=/tmp/pcx-assets-target cargo clippy --locked -p pcx-assets --all-targets -- -D warnings
```

Consumers can add under `[dependencies]`:

```toml
pcx-assets.workspace = true
```

The concrete adapters are available to consumers. Application asset-mode wiring
and native archive/module/locale precedence remain unresolved.
