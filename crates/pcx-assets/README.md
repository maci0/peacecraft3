# pcx-assets

A source-resolution library for PLAN §3.2/§3.6/§3.9. It implements
the filesystem override layer, explicit archive priority, per-path provenance
and generated `override/index.json`. It does not decode MPQ or CASC, detect
installs, stage copies, apply gameplay fixes, or certify original behavior.

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
built-in archive-name order or art-mode priority. Root must supply evidence-backed
ordering and choose the relevant art/gameplay sources.

`ContentSource` has three methods:

* `info()` supplies layer ID, MPQ/CASC kind, source identity and detected version
  or explicit unknown version. These identify the input, not the requested mode.
* `paths()` supplies the complete original-member inventory. Unavailable or
  unsupported inventories must fail rather than claim an empty archive.
* `read(member)` supplies decoded bytes, source-declared length and exact source
  locator. Adapters must validate sizes/offsets before allocating or reading.
  Missing, malformed, ambiguous and truncated input can return `SourceError`.

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
Only synthetic fixtures are used by this crate's tests. The tests cover all 36
mode combinations, default/fidelity policy, real override directories, archive
priority, provenance/JSON escaping, deterministic output and typed failure paths.
They prove library policy and filesystem behavior, not native archive order,
CASC/MPQ decoding or the Reforged Footman rendering gate.

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

The archive adapters remain unwired. Native archive priority and CASC path
resolution remain unresolved by this library.
