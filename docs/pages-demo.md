# Asset-free GitHub Pages demo

Demo: <https://maci0.github.io/peacecraft3-demo/>

The separate public `maci0/peacecraft3-demo` repository contains only the static
page, CSS, browser modules, generated WASM and the installer's GPL decoder
component with its corresponding source. The application source repository stays
private. No game art, fonts, sound, movies, MPQs, ISOs or extracted fixtures are
published. There is no upload endpoint: selected files are read locally by the
browser and the resulting asset store lasts only for that page session.

The page is a partial menu demo. It supports ISO9660 primary and Joliet discovery,
direct MPQs, and the checked Inno/FreeArc profile in the locally supplied
Complete Edition installer. Other installer profiles and UDF images remain
explicitly unsupported. Full gameplay and native visual fidelity remain open.

Joliet uses its own big-endian UCS-2 directory records, not primary-namespace
filename substitution. A single recognized Joliet descriptor is selected when
present and recorded in provenance. Unknown or multiple supplementary
namespaces, invalid characters, conflicting volume sizes and malformed extents
fail without falling back to primary. This follows
[ECMA-119 Annex C](https://www.ecma-international.org/wp-content/uploads/ECMA-119_4th_edition_june_2019.pdf).

The supplied Complete Edition ISO now discovers ten files with 15 bounded reads
(8,855 bytes; largest read 2,048 bytes). Six contain FreeArc signatures and are
not directly readable MPQs. The local decoder statically reads the installer
script's archive/password bindings, validates encrypted control directories,
then streams the selected REP/4x4/Tornado solid block in a dedicated worker.
It runs no installer or DLL. Every member CRC in the decoded block is checked;
only selected members are written to temporary browser storage. MPQ-named files
must also pass their actual payload header check: nine movie members contain
AVI data and cannot be staged as archives.

When discovery finds one unique, budget-eligible `war3.mpq`, the demo selects it
by default for the base menu. A single discovered archive is also selected.
Multiple archives with no unique base require a choice; the demo does not guess
which copy of a conflicting member should win. The selection remains editable.
Base skins with an unversioned `GlueSpriteLayerBackground` binding also resolve
their scene from that authored value.

Imports expose the stored base files before language patches. The patch format
and application remain unverified; no installer release label establishes game
archive identity. Other FreeArc chains, including the EXE-filter block, remain
typed refusals. See
[the measured ISO evidence](re-notes/original-iso-installers.md).

Loading bars show measured WASM download bytes when an unencoded response has
a reliable length, then switch to indeterminate initialization. ISO discovery
has no known work total and remains indeterminate with a read-byte count. Archive
reading uses selected byte totals. Solid decoding reports its directory-derived
decoded-byte total; authored dependency validation is a separate
indeterminate phase. Completion, cancellation and errors are explicit, with no
timer-generated percentages.

## Edition restrictions

Menu capabilities come from the scene contracts successfully hydrated in the
selected transaction, not filenames or an assumed release. Base-only content
exposes only Reign of Chaos. The exported menu request, launcher addon choice
and in-game edition button all refuse TFT when its content is absent. Failed
imports retain the previous store and capabilities; clearing assets disables
both menus. This identifies renderer content, not an executable/archive version
or complete expansion gameplay support.

The local-only page does not preload served artwork, audio or startup movies.
Music and click audio can use imported local Blobs; unsupported/missing audio
is reported. No converted movie is shipped with the page.

## Advanced map data

The collapsed **Advanced map configuration** panel accepts a local metadata JSON
file and an explicit byte limit. Oversized files are refused before reading;
invalid UTF-8 and Rust schema/source errors are reported without substituting
defaults. The archive inspector exposes an explicitly selected ordinal and its
current committed revision, not archive contents or an inferred release.

The JSON must supply the source bindings, identities, field selection and budgets
described in [browser map configuration](re-notes/browser-map-configuration.md).
Successful loading publishes parsed map data through the existing application
consumer. It does not run gameplay or enable additional menu editions. Controls
are disabled while an archive import or configuration request is pending.

## Build and package

```sh
cargo build --locked --release -p peacecraft3 --target wasm32-unknown-unknown
wasm-bindgen --target web --remove-name-section --remove-producers-section --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/peacecraft3.wasm
wasm-opt web/pkg/peacecraft3_bg.wasm -Oz -o web/pkg/peacecraft3_bg.optimized.wasm
mv web/pkg/peacecraft3_bg.optimized.wasm web/pkg/peacecraft3_bg.wasm
tools/freearc-codec/build.sh test
mkdir -p web/pkg/freearc
cp tools/freearc-codec/build/freearc-codec.mjs \
  tools/freearc-codec/build/freearc-codec.wasm web/pkg/freearc/
python3 tools/package-pages.py --self-test
python3 tools/package-pages.py --output .scratch/pages-release
```

The package helper requires absent/empty output and uses an explicit allowlist.
It never copies `web/assets`, local fixtures, scratch data or credentials.
Generated JS snippets are included only when referenced by the bindings.
The codec's complete corresponding source archive is generated from an explicit
source list and its hash-checked upstream manifest, including its GPL-2.0 license,
build script and synthetic tests. Build outputs are not swept into that archive.
Missing files, symlinks and escaping imports reject the package. `.nojekyll`
allows the built static files to be published from the public repository's main
branch without building Rust on GitHub.

Publish only this checked output to the demo repository. Do not copy the entire
`web` directory or upload browser captures. Removing debugging names leaves
executable sections intact. The subsequent size optimization rewrites code;
run the generated-WASM browser checks against its final output before publishing.

## Verification

`tests/browser-iso/demo-runtime.mjs` runs against the packaged page under a
project-style URL prefix. It checks empty startup, mobile overflow, synthetic
base-only ISO capability/refusal, clearing assets, and a delayed direct-MPQ read
with competing selection/clear events. An optional `PCX_DEMO_PREPARED` points to
an explicit local original-menu fixture for the real file-input/render gate.
Captures and proof remain under `.scratch/pages-demo-root/`.

`node tests/browser-iso/map-configuration.mjs` checks the configuration controller's
read ceiling, fatal UTF-8, request ownership, archive metadata and import/read
interleaving. `PCX_REAL_WASM_JS` adds an explicit Cargo-built test-WASM App gate;
that Node/fake-DOM check is separate from a production browser frame.

The current packaged demo-runtime gate also passes with the supplied local
original-menu fixture and configuration controls disabled during import. Its 18
requests were code/WASM or local Blob GETs, with zero page errors or sockets.
The private proof is `.scratch/browser-configuration-ui-root-review/demo-runtime/`.
Six reviewed asset-free captures cover default and expanded error/focus states
at desktop, mobile and 600-CSS-pixel reflow widths without horizontal overflow.
The 2DPR reflow capture is not a native browser zoom test.

A separate production-page gate submits synthetic local metadata through the
running main App without test-frame exports. Eight groups pass: archive metadata,
pre-read size refusal, fatal UTF-8/schema errors, admission, stale-source refusal,
result cleanup and preserved base-only menu restrictions. It makes 14 code/WASM
GETs and no uploads, sockets or page errors. This is data admission, not gameplay
or original-map execution. The private proof is in
`.scratch/browser-configuration-ui-root-review/functional-main-app-policy-baseline/`.

The published configuration page passes the same eight synthetic main-App groups,
including stale-source refusal and the base-only TFT restriction. All 22 hosted
files match the checked package byte for byte. Its 14 requests are code/WASM GETs,
with no uploads, sockets or page errors. Public-site proof stays local in
`.scratch/browser-configuration-ui-root-review/live-functional-main-app/`.

`PCX_DEMO_INSTALLER_ISO` adds the explicit local Joliet/FreeArc directory gate.
`PCX_DEMO_INSTALLER_MEMBER` selects an exact caller-supplied directory member for
the streaming extraction/import gate; `PCX_DEMO_INSTALLER_EDITION` checks its
expected hydrated capability. `PCX_DEMO_INSTALLER_CANCEL=1` tests cancellation
before commit, and `PCX_DEMO_INSTALLER_REFUSED_MEMBER` selects a payload whose
header must be refused. These inputs are private local files, never fixtures
included in the public package.
`PCX_DEMO_CAPTURE_DOWNLOAD=1` captures an in-flight download against a throttled
local test server. The current progress/Joliet captures and network evidence are
retained separately in `.scratch/joliet-progress-root/`.

The final local gate passed with the separately retained 115-member fixture:
base-only TFT refusal, import ownership, fresh original scene and all six label
logs, and desktop/mobile captures. Fifteen requests were static code/WASM or
local Blob GETs; there were no hosted asset requests, uploads, sockets or page
errors. Original-menu captures were reviewed locally. The synthetic base model
proves policy only; it does not prove base-game rendering. A missing `MODL`
record now produces a typed error instead of terminating the WASM app.

The supplied installer gate also passed with its stored base `war3.mpq`:
the RoC scene and all six authored labels render, TFT is unavailable, cancellation
retains the previous revision and temporary decoded files are removed. The
final checked run also rejected an actual AVI member named `.mpq` before staging
and made 33 local code/Blob GETs, with zero uploads, sockets or page errors.
This proves the bounded stored-base import, not language-patched install,
every retail disc, native archive priority or complete menu fidelity. Its private
proof is retained in `.scratch/freearc-browser-root/browser-4-stable-wasm/`.
