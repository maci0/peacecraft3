# peacecraft3

This crate runs the native application, including its launcher, campaign and
HUD, staging CLI and model viewer. It also provides the browser entry point
for the launcher and local menu demo.

Run the applicable command from the repository root. Replace the example
paths with your local paths.

```sh
cargo run --locked -p peacecraft3
cargo run --locked -p peacecraft3 -- launch sources "/path/to/install"
cargo run --locked -p peacecraft3 -- launch plan 1.29b-tft-hd-vanilla
cargo run --locked -p peacecraft3 -- launch stage 1.29b-tft-vanilla-vanilla "/path/to/install" "/path/to/destination"
cargo run --locked -p peacecraft3 -- view "/path/to/install" "path/to/model.mdx"
```

With no arguments, the native executable opens the launcher. See the
[desktop build guide](../../docs/build-desktop.md) for packaged executables and
the [root README](../../README.md) for local asset selection.

## Browser builds

Follow the [Pages demo build guide](../../docs/pages-demo.md#build-and-package) to package and
check the public demo. Publish the allowlisted package output. The development
page is `web/index.html`. The public package uses `web/demo.html` as its
`index.html`.

For the local launcher with direct MPQ import, build the module and serve
only the web directory on loopback. Use the `wasm-bindgen` CLI version listed
in `Cargo.lock`. For installer import, also build the codec as shown in the
Pages guide.

```sh
cargo build --locked --release -p peacecraft3 --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg \
  target/wasm32-unknown-unknown/release/peacecraft3.wasm
python3 -m http.server 8765 --bind 127.0.0.1 --directory web
```

Open `http://127.0.0.1:8765/`.

The workspace enables WebGL2. In a browser window, the
[entry point](src/browser.rs) starts Bevy on the configured
`#launcher-canvas`. A worker has no window and runs only the asset API. The browser
uses an in-memory asset reader for user-selected data. The separate ISO import
path runs the installer decoder in a worker. Native directory staging stays on
desktop.

<details>
<summary>October 2026 launcher measurements</summary>

## WASM size measurements

Measured on 12 October 2026 with the earlier development launcher build.
The current Pages package uses the separate release recipe.

| Artifact | Size |
|---|---|
| `target/wasm32-unknown-unknown/release/peacecraft3.wasm` | 74,235,106 B (70.80 MiB) |
| `web/pkg/peacecraft3_bg.wasm` after wasm-bindgen | 75,887,489 B (72.37 MiB) |

Section breakdown of `web/pkg/peacecraft3_bg.wasm` (75,887,489 B total):

| Section | Bytes | MiB | Share |
|---|---:|---:|---:|
| `code` | 41,117,163 | 39.21 | 54.18% |
| `name` (custom) | 29,367,076 | 28.01 | 38.70% |
| `data` | 5,008,639 | 4.78 | 6.60% |
| `elem` | 252,748 | 0.24 | 0.33% |
| `function` | 107,829 | 0.10 | 0.14% |
| `import` | 29,158 | 0.03 | 0.04% |
| `type` | 2,423 | 0.00 | 0.01% |
| `export` | 2,070 | 0.00 | 0.00% |
| `target_features`, `producers` (custom) | 289 | 0.00 | 0.00% |

The name section held 28.01 MiB of symbols for 106,791 functions. Removing it
reduced the measured module to 46,520,408 bytes. The stripped module still
started, drew the four mode axes, and opened the local MPQ in 96.8 ms. The
development artifact was 975,009,181 bytes, dominated by DWARF and names.

If the server compresses responses, the win is smaller than the raw percentage
suggests, because the `name` section is highly repetitive text:

| Artifact | raw | gzip -9 | brotli -11 |
|---|---:|---:|---:|
| with the `name` section | 75,887,489 B | 12,877,415 B | 7,003,290 B |
| `--remove-name-section` | 46,520,408 B | 10,382,400 B | 6,084,443 B |

`python3 -m http.server` does not compress, so the numbers above measure the
module itself, not what a compressing host would send.

The same profile attributed 7.78 MB across 49,302 functions to
`bevy_reflect`. The next largest measured contributions were `web_sys`
(4.24 MB, 660 functions), `serde_core` (2.36 MB, 6,459), `bevy_ecs`
(2.11 MB, 3,313) and `bevy_render` (1.92 MB, 4,243).

## Earlier launcher browser check

The October prototype check used headless Chromium 153 and the local 440,825,847-byte
`wc3/game/app/war3.mpq`:

| Feature | Prototype observation |
|---|---|
| The launcher GUI, all four mode axes, the log | works — rendered on WebGL2 via the `#launcher-canvas` element |
| Mode summary, `1.29b-tft-vanilla-vanilla` | works — shown as `staged as 1.29b-tft-vanilla-vanilla` |
| Opening an MPQ from memory (`MpqArchive::from_bytes`) | works — a 440 MB `war3.mpq` opened in 26–97 ms and reported 10,681 live blocks of 10,760 with `(listfile)` present; a 27 MB `War3Patch.mpq` reported 576 of 576 and no `(listfile)`, which is what its tables say |
| Assets handed over from JS to Bevy's default asset source | works — `add_asset` stores them, `selftest` confirms the reader still serves them |
| Bytes that are not an MPQ | reported as an error and not kept |
| A note pushed from JS into the launcher's own log | works — `add_archive` on a non-MPQ writes `✕ not-an-mpq.bin would not open as an MPQ: …` into the GUI's LOG fieldset inside the canvas |
| Staging (`launch::stage`) | **does not work** — there is no filesystem to copy into. The launcher reports every source as rejected: `✕ source rejected: wc3/game/app does not exist`. The page's own `web/index.html` states this limitation, and the message is `launch_gui::do_stage`'s `✕ source rejected: {e}` over `Error::NotFound`, which is the shape the CLI prints for a path that is not there |
| Install version detection (`Install::open`) | **does not work** — `Install::open` fingerprints a *set* of archives in a directory, and there is no directory. One archive can be opened and counted, not identified as 1.28c or 1.29b |
| Asset paths under `assets/` over HTTP | **does not apply** — Bevy's platform default is replaced by the in-memory source, since an HTTP reader rooted at `assets/` cannot see bytes the page fetched |
| `pcx_gfx`'s viewer and cache | **does not work** — the cache is still synchronous, per PLAN.md §3.8 |
| Audio | initialises an `AudioContext` and warns that it needs a user gesture; nothing plays |

</details>
