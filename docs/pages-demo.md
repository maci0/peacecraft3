# Asset-free GitHub Pages demo

Demo: <https://maci0.github.io/peacecraft3-demo/>

The separate public `maci0/peacecraft3-demo` repository contains only the static
page, CSS, browser modules and generated WASM. The source repository stays
private. No game art, fonts, sound, movies, MPQs, ISOs or extracted fixtures are
published. There is no upload endpoint: selected files are read locally by the
browser and the resulting asset store lasts only for that page session.

The page is a partial menu demo. It supports the current ISO9660/direct-MPQ
import path; installer-only and unsupported UDF images are diagnosed rather
than treated as working retail ISO imports. Full gameplay and native visual
fidelity remain open.

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

The final local gate passed with the separately retained 115-member fixture:
base-only TFT refusal, import ownership, fresh original scene and all six label
logs, and desktop/mobile captures. Fifteen requests were static code/WASM or
local Blob GETs; there were no hosted asset requests, uploads, sockets or page
errors. Original-menu captures were reviewed locally. The synthetic base model
proves policy only; it does not prove base-game rendering. A missing `MODL`
record now produces a typed error instead of terminating the WASM app.
