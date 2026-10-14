# In-game cinematic and dialog declaration ingestion

The closed slice is checked FDF ingestion and dependency binding. It replaces the
unfinished controller/default-value draft. It does not execute transmissions,
dialogs, camera or fade natives, allocate game handles, select portrait models,
synthesize skip events, interpolate colors, or decide scene/sound lifetimes.
Those require separate script/native/runtime evidence. Campaign dialogue and
cutscene gates in `PLAN.md` and `TRACKER.md` remain open.

## API and data ownership

`FdfCatalog::parse(&[FdfAsset], FdfLimits)` consumes explicitly labeled UTF-8 inputs.
The public entry point is `pcx_play::cinematics`; integration tests exercise that
module through the workspace crate rather than compiling another private copy.
Every input identifies edition, version and source separately and retains a
checksum over the exact supplied bytes. Unknown release identity must be stated
as unknown; a neighboring executable's PE version does not identify the archives.
There are no edition, frame-name, font, art, color or coordinate defaults.

The catalog retains declarations, lexical parent/child edges, direct template
edges including the authored `WITHCHILDREN` flag, property records with original
spelling, and every checked `SetPoint`. Numeric atoms retain their spelling and a
finite f64 view; this is not an assertion about engine numeric representation.
Three-component colors stay three-component. Missing height or font properties
stay absent. No text measurement, inherited-property merge or inherited-child
expansion is performed.

`frame_id`, `frames`, `sources` and `dependencies` expose immutable metadata.
`Frame::record` reads only that declaration's local record. IDs index this catalog
and are not native handles. Duplicate source labels, globally ambiguous frame
names and repeated property records are refused rather than resolved by input
order. Exact repeated anchors are refused; distinct anchors are all retained.
Forward template references may resolve against uniquely supplied declarations;
missing templates and inheritance cycles fail with source/byte context.

`relationships(FrameId, &[FrameBinding], Option<Target>)` requires caller-selected
targets for every local anchor. An authored `SetAllPoints` requires an explicit
parent/context target. It never implies a particular screen rectangle. Bindings
may name catalog frames or caller-owned contexts. The result preserves every
anchor plus the direct inheritance edge; it does not solve geometry, flatten
inheritance, choose between conflicting constraints, or promise full rectangles.
A text frame without authored height may need engine text sizing before geometry
can be resolved.

## Supported grammar and refusal boundary

Frame headers support quoted type/name and optional `INHERITS [WITHCHILDREN]`.
Headers may span lines; braces and empty frames are checked. Property records
occupy one logical line and require a trailing comma. `SetPoint` and `FrameFont`
require their comma-separated field structure. Line and block comments are
recognized outside quoted strings; unterminated comments, strings and frames
fail. Literal backslashes in paths remain literal; quoted-string escapes are
explicitly unsupported instead of interpreted through an invented escape rule.

Width/height, relevant backdrop dimensions/insets, font/color records and anchor
records receive shape/type checks. Malformed and non-finite numeric atoms fail.
Unrecognized frame properties remain raw authored records with their tokens;
there is no successful runtime interpretation attached to those records. Unknown
top-level declaration kinds fail. Includes are inventoried, not implicitly loaded.

The dependency inventory covers includes, inheritance, local anchors and the
supported font/art/frame-reference property keys. Other records remain available
raw; this is not a complete engine dependency resolver. Font and art aliases are
not silently replaced with assumed paths or assets. Native/event declaration
inventory is deferred to the existing checked script declaration APIs, rather
than another hand-maintained dispatcher or list of numeric event IDs.

Caller-supplied limits bound aggregate input bytes, aggregate tokens, frame count
and lexical frame depth. Parsing and inheritance traversal are iterative. Limits
are harness policy, not measured game rules or a total process-memory guarantee.

## Checks and local evidence

Seven synthetic tests change frame/template names, dimensions, colors, font/art
names and anchor offsets. They check multiple anchors, absent text height, raw
unknown properties, explicit context/bindings, duplicates and ambiguity, missing
and cyclic inheritance, bad numerics/record shapes and separators, unsupported
syntax, unterminated input, source identity and aggregate budgets. There are no
rendering or native-success mocks.

An ignored original-input gate requires exact local paths, expected hashes and
caller-selected asset identity/context through environment variables:

- `PEACECRAFT3_CINEMATIC_FDF`, `PEACECRAFT3_DIALOG_FDF`,
  `PEACECRAFT3_TEMPLATE_FDF`;
- matching `*_FDF_HASH` values as FNV-1a 64 hexadecimal;
- `PEACECRAFT3_FDF_EDITION`, `PEACECRAFT3_FDF_VERSION`,
  `PEACECRAFT3_FDF_SOURCE`, `PEACECRAFT3_FDF_CONTEXT`.

The correction gate read the existing local decoded inputs under
`.scratch/ingame-cinematics-dialogs/` without modifying or copying them:

| Input | Bytes | FNV-1a 64 |
| --- | ---: | --- |
| CinematicPanel FDF | 3237 | `4c094662a490dafc` |
| ScriptDialog FDF | 1287 | `659841d67f8eb58f` |
| EscMenuTemplates FDF | 16134 | `bf030d799461c670` |

These are measured identities for that local run, not production constants or
acceptance thresholds. Archive release identity was explicitly unverified; the
existing extracted paths alone do not establish archive priority or provenance
back to a particular release. The gate parsed 66 declarations after excluding
comments and retained 105 inventory entries. Every local anchor received an
explicit caller binding; SetAllPoints used a caller-owned context identity with
no supplied or invented geometry. The prior draft's different checksums and
layout/runtime claims are not evidence for this correction.

Strict native tests, scoped all-target Clippy, rustfmt check and a strict
`wasm32-unknown-unknown` all-target compile check passed in the isolated leaf
harness at `.scratch/cinematics-correction-sol/`. Its manifest uses the production
source and integration-test files with a private target directory, avoiding
shared workspace builds. The original gate also passed. Commands, identity
metadata, SHA-256 checksums and captured logs remain in that private directory;
original contents remain local and are not included in tracked fixtures.

This establishes checked declaration ingestion and binding metadata only. It
provides no rendered, native-code, native runtime, full dialog or full-cutscene
success claim.
