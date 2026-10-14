# Asset-free GitHub Pages demo

Demo: <https://maci0.github.io/peacecraft3-demo/>

The separate public `maci0/peacecraft3-demo` repository contains only the static
page, CSS, browser modules and generated WASM. The source repository stays
private. No game art, fonts, sound, movies, MPQs, ISOs or extracted fixtures are
published. There is no upload endpoint: selected files are read locally by the
browser and the resulting asset store lasts only for that page session.

The page is a partial menu demo. It supports ISO9660 primary and Joliet directory discovery, followed by direct-MPQ
import path; installer-only and unsupported UDF images are diagnosed rather
than treated as working retail ISO imports. Full gameplay and native visual
fidelity remain open.

Joliet uses its own big-endian UCS-2 directory records, not primary-namespace
filename substitution. A single recognized Joliet descriptor is selected when
present and recorded in provenance. Unknown or multiple supplementary
namespaces, invalid characters, conflicting volume sizes and malformed extents
fail without falling back to primary. This follows
[ECMA-119 Annex C](https://www.ecma-international.org/wp-content/uploads/ECMA-119_4th_edition_june_2019.pdf).

The supplied Complete Edition ISO now discovers ten files with 15 bounded reads
(8,855 bytes; largest read 2,048 bytes). Six contain FreeArc signatures and are
reported as `UNSUPPORTED_FREEARC`; none is a directly readable MPQ. Its encrypted
installer payload remains a separate decoder blocker. See
[the measured ISO evidence](re-notes/original-iso-installers.md).

Loading bars show measured WASM download bytes when an unencoded response has
a reliable length, then switch to indeterminate initialization. ISO discovery
has no known work total and remains indeterminate with a read-byte count. Archive
reading uses selected byte totals; authored dependency validation is a separate
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

## Build and package

```sh
cargo build --locked --release -p peacecraft3 --target wasm32-unknown-unknown
wasm-bindgen --target web --remove-name-section --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/peacecraft3.wasm
python3 tools/package-pages.py --self-test
python3 tools/package-pages.py --output .scratch/pages-release
```

The package helper requires absent/empty output and uses an explicit allowlist.
It never copies `web/assets`, local fixtures, scratch data or credentials.
Generated JS snippets are included only when referenced by the bindings.
Missing files, symlinks and escaping imports reject the package. `.nojekyll`
allows the built static files to be published from the public repository's main
branch without building Rust on GitHub.

Publish only this checked output to the demo repository. Do not copy the entire
`web` directory or upload browser captures. The generated debugging name section
is removed to reduce transfer size; executable WASM sections are preserved.

## Verification

`tests/browser-iso/demo-runtime.mjs` runs against the packaged page under a
project-style URL prefix. It checks empty startup, mobile overflow, synthetic
base-only ISO capability/refusal, clearing assets, and a delayed direct-MPQ read
with competing selection/clear events. An optional `PCX_DEMO_PREPARED` points to
an explicit local original-menu fixture for the real file-input/render gate.
Captures and proof remain under `.scratch/pages-demo-root/`.

`PCX_DEMO_INSTALLER_ISO` adds the explicit local Joliet/FreeArc discovery gate;
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
