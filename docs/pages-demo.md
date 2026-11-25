# Asset-free GitHub Pages demo

Demo: <https://maci0.github.io/peacecraft3-demo/>

The separate public `maci0/peacecraft3-demo` repository contains only the static
page, CSS, browser modules, generated WASM and the installer's GPL decoder
component with its corresponding source. The application source is in
`maci0/peacecraft3`. Game assets and extracted fixtures stay local. The browser
reads selected files locally and keeps its asset store for the page session.
The demo has no upload endpoint.

The page is a partial menu demo. It supports ISO9660 primary and Joliet discovery,
and the Inno/FreeArc profile verified against a locally supplied Complete Edition
installer. Other installer profiles and UDF images remain unsupported.
Use **Already have extracted archives?** to select loose MPQs.

Joliet uses its own big-endian UCS-2 directory records, not primary-namespace
filename substitution. A single recognized Joliet descriptor is selected when
present and recorded in provenance. Unknown or multiple supplementary
namespaces, invalid characters, conflicting volume sizes and malformed extents
fail without falling back to primary. This follows
[ECMA-119 Annex C](https://www.ecma-international.org/wp-content/uploads/ECMA-119_4th_edition_june_2019.pdf).

The Complete Edition ISO discovery check recorded at `fd4a6e0d` on October 14,
2026, found ten files with 15 bounded reads (8,855 bytes total, with a 2,048-byte
largest read). Six files contained FreeArc signatures. The local decoder
reads the archive and password bindings and validates encrypted control
directories. A dedicated worker streams the selected REP/4x4/Tornado block.
It runs no installer or DLL. Every member CRC in the decoded block is checked;
only selected members are written to temporary browser storage. MPQ-named files
must also pass their actual payload header check: nine movie members contain
AVI data and cannot be staged as archives.

The demo preselects a unique `war3.mpq` within the import size limit.
Otherwise, select an archive manually. You can change the selection before import.

Imports expose stored base files before language patches, whose format and
application remain unverified. Installer release labels do not establish
archive identity. See the [measured ISO evidence](re-notes/original-iso-installers.md).

The [decoder](../tools/freearc-codec/README.md) accepts a bare `exe` stage
between REP and 4x4/Tornado for eligible blocks. Unsupported codec chains
return a typed error.

The [client member selector](../web/iso/engine-member.js) uses a unique,
nonempty `game.dll` within budget. An ambiguous, empty or oversized
`game.dll` leaves the MPQ selection intact and omits the optional client
image. Only an absent `game.dll` permits fallback to a unique
`Warcraft III.exe`.

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
or complete expansion gameplay support. Base skins can name their scene
through an unversioned `GlueSpriteLayerBackground` binding.

The local-only page does not preload served artwork, audio or startup movies.
Music and click audio can use imported local Blobs; unsupported/missing audio
is reported. No converted movie is shipped with the page.

## Advanced map data

The collapsed **Advanced map configuration** panel accepts a local metadata JSON
file and an explicit byte limit. Oversized files are refused before reading;
invalid UTF-8 and Rust schema/source errors are reported without substituting
defaults. The archive inspector displays the selected archive ordinal, name,
size in bytes and committed revision.

The JSON must supply the source bindings, identities, field selection and budgets
described in [browser map configuration](re-notes/browser-map-configuration.md).
Successful loading publishes parsed map data through the existing application
consumer. It does not run gameplay or enable additional menu editions. Controls
are disabled while an archive import or configuration request is pending.

## Build and package

Install the `wasm32-unknown-unknown` Rust target, the `wasm-bindgen` CLI
version matching `Cargo.lock`, and `wasm-opt`. See the
[codec build prerequisites](../tools/freearc-codec/README.md#build-and-test).

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

The browser gate uses Chromium at `/usr/bin/chromium`. Set `PCX_PLAYWRIGHT_MODULE` to the
installed Playwright Core entry point if it differs from
`.scratch/browser-check/node_modules/playwright-core/index.mjs`. Verify
offscreen display and null audio before running the gate.

With the checked package served at the default loopback URL
`http://127.0.0.1:8944/peacecraft3-demo/`, run:

```sh
node tests/browser-iso/demo-runtime.mjs
```

Set `PCX_DEMO_ORIGIN` for a different package URL. This gate checks empty
startup, mobile overflow, synthetic base-only ISO capability and refusal,
clearing assets, and a delayed direct-MPQ read
with competing selection/clear events. An optional `PCX_DEMO_PREPARED` points to
an explicit local original-menu fixture for the real file-input/render gate.
Captures and proof remain under `.scratch/pages-demo-root/`.

`node tests/browser-iso/map-configuration.mjs` checks the configuration controller's
read ceiling, fatal UTF-8, request ownership, archive metadata and import/read
interleaving. `PCX_REAL_WASM_JS` adds an explicit Cargo-built test-WASM App gate;
that Node/fake-DOM check is separate from a production browser frame.

`PCX_DEMO_INSTALLER_ISO` adds the explicit local Joliet/FreeArc directory gate.
`PCX_DEMO_INSTALLER_MEMBER` selects an exact caller-supplied directory member for
the streaming extraction/import gate; `PCX_DEMO_INSTALLER_EDITION` checks its
expected hydrated capability. `PCX_DEMO_INSTALLER_CANCEL=1` tests cancellation
before commit, and `PCX_DEMO_INSTALLER_REFUSED_MEMBER` selects a payload whose
header must be refused. These inputs are private local files, never fixtures
included in the public package.
`PCX_DEMO_CAPTURE_DOWNLOAD=1` captures an in-flight download against a throttled
local test server. The progress and Joliet captures are recorded in
`.scratch/joliet-progress-root/`.

## 0.19.0 publication

The 18 November 2026 deployment used default-feature production WASM built
with Rust 1.100.0, matching wasm-bindgen bindings and size optimization. See
[release validation](releases/0.19.0.md#release-validation) for the local and
hosted synthetic browser results and file verification.

## Earlier verification runs

The October runs reported no page errors or sockets. Request counts cover code,
WASM and local Blobs. The synthetic configuration, original-menu and installer
runs also explicitly checked that no uploads occurred.

| Date | Scope | GETs | Local evidence |
| --- | --- | ---: | --- |
| 15 Oct 2026 | Original-menu import with configuration disabled during import | 18 | `.scratch/browser-configuration-ui-root-review/demo-runtime/` |
| 15 Oct 2026 | Local synthetic configuration through the main app | 14 | `.scratch/browser-configuration-ui-root-review/functional-main-app-policy-baseline/` |
| 15 Oct 2026 | Hosted synthetic configuration, with all 22 package files matching | 14 | `.scratch/browser-configuration-ui-root-review/live-functional-main-app/` |
| 14 Oct 2026 | Local 115-member original-menu fixture | 15 | Recorded at `8d1ef9f1` |
| 14 Oct 2026 | Installer stored-base archive import | 33 | `.scratch/freearc-browser-root/browser-4-stable-wasm/` |

The 15 October captures covered default and expanded error/focus states at
desktop, mobile and 600-CSS-pixel reflow widths without horizontal overflow.
The 600-pixel case used a device pixel ratio of 2. Native browser zoom was outside this check.
The synthetic configuration gates covered eight groups, including archive metadata,
size refusal before the read, UTF-8 and schema errors, admission, stale-source
refusal, cleanup and base-only menu restrictions.

The 115-member fixture verified base-only TFT refusal, import ownership, scene
refresh and six authored label logs. It produced desktop and mobile captures
for local review. The separate synthetic model check verified base-only edition
restrictions. Base-game rendering was outside that test’s scope.

The installer run rendered the RoC scene and all six labels. TFT stayed
unavailable. Cancellation preserved the previous revision, temporary decoded
files were removed, and an AVI member named `.mpq` was refused before staging.
The result applies to the stored base archive. Language patches, other retail
discs, native archive priority and complete menu fidelity remain unverified.
