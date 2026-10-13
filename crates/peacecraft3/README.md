# peacecraft3

The launcher. Two front-ends over one mode model ([`launch`](src/launch.rs)) — a CLI
for CI and scripts, and a GUI ([`launch_gui`](src/launch_gui.rs)) rendered on the
engine it stages for — plus the wasm entry point that runs the same GUI in a
browser ([`browser`](src/browser.rs)).

```
cargo run -p peacecraft3                        # launcher window
cargo run -p peacecraft3 launch sources <path>  # classify a source
cargo run -p peacecraft3 launch plan 1.29b-tft-hd-vanilla
cargo run -p peacecraft3 launch stage 1.29b-tft-vanilla-vanilla <src> <dst>
cargo run -p peacecraft3 view <install> [model] # M2 model viewer gate
```

## Building for the browser

The module needs wasm-bindgen glue — it imports wgpu's WebGL2 and winit's DOM
functions through the wbindgen shim, so a bare `.wasm` is not instantiable. The
crate is a binary target, so `cargo build` plus `wasm-bindgen` is enough; no
`wasm-pack`, no bundler. Run these from the repository root:

```
cargo build -p peacecraft3 --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/peacecraft3.wasm

python3 -m http.server 8765            # from the repository root
# open http://127.0.0.1:8765/web/index.html
```

Both build commands were run to produce this file; the `python3 -m http.server`
line is the same server the verification below used. Serve the repository root
(not `web/`) so that `pkg/` is fetchable alongside the page.

`web/` holds the shell: `index.html` provides `#launcher-canvas`, which is what
winit attaches the window to, and `loader.js` instantiates the module and wires
the archive picker. `web/pkg/` is generated — it is gitignored.

`cargo build` produces the module for `wasm32-unknown-unknown`; `wasm-bindgen
--target web` then emits the glue and a `.wasm` with the wbindgen imports
resolved. The renderer is the WebGL2 backend that `bevy`'s default features
select on wasm32 (Chrome, Edge, recent Firefox). `bevy`'s `webgpu` feature would
select the WebGPU backend instead and is deliberately not enabled here — an
unstable API on most browsers as of this writing.

## How big the release wasm is

Measured on the release build produced by the commands above, 2026-10-12:

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

It is still very large, and **the `name` custom section is 38.7% of it** — 28 MiB
of symbol names for 106,791 functions. That section exists only to make stack
traces readable; dropping it changes no behaviour. A debug build is far worse —
975,009,181 B (929.84 MiB) — because DWARF dominates it: `.debug_str` 311 MiB,
`.debug_info` 293 MiB, `.debug_loc` 125 MiB, `.debug_line` 55 MiB,
`.debug_ranges` 33 MiB.

### Making it smaller yourself

`wasm-bindgen` removes the section without touching the build profile:

```
wasm-bindgen --target web --remove-name-section --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/peacecraft3.wasm
```

That yields `web/pkg/peacecraft3_bg.wasm` at **46,520,408 B (44.37 MiB)**, a
38.7% reduction. Verified: the stripped module boots, the launcher GUI renders
all four mode axes, the JS API answers, and `war3.mpq` opens in 96.8 ms — the
same numbers as the unstripped build. The only loss is readable stack traces.

If the server compresses responses, the win is smaller than the raw percentage
suggests, because the `name` section is highly repetitive text:

| Artifact | raw | gzip -9 | brotli -11 |
|---|---:|---:|---:|
| with the `name` section | 75,887,489 B | 12,877,415 B | 7,003,290 B |
| `--remove-name-section` | 46,520,408 B | 10,382,400 B | 6,084,443 B |

`python3 -m http.server` does not compress, so the numbers above measure the
module itself, not what a compressing host would send.

The workspace `[profile.release]` does not set `strip`, so every build keeps the
names. Changing that would edit the workspace `Cargo.toml`, which this crate does
not own; the flag above is the user's own lever.

What *still* blocks a smaller binary, and cannot be fixed by stripping:

* **`code` is 39.2 MiB and is the real floor.** Attributing the code section by
  the `name` entries across the 106,791 functions puts **`bevy_reflect` at
  7.78 MB over 49,302 functions** — by far the largest single contributor, ahead
  of `web_sys` (4.24 MB / 660), `serde_core` (2.36 MB / 6,459), `bevy_ecs`
  (2.11 MB / 3,313) and `bevy_render` (1.92 MB / 4,243). The whole Bevy engine
  is linked, monomorphised, and generic; nothing is tree-shaken at the Cargo
  level. Dropping `bevy_reflect`'s unwrap-through-`Any` paths, or Bevy's
  default features, is the only way below this, and that is a Bevy-side decision.
* **`data` is 4.78 MiB** — read-only constant tables, which is the price of the
  engine's built-in font, palette and geometry staying inside the module.
* The release profile already emits no DWARF (`b".debug_"` is absent from the
  release artifact), so there is nothing further to strip on that axis.

## What runs in a browser

Verified in headless Chromium 153 against the real 440,825,847-byte
`wc3/game/app/war3.mpq`:

| Feature | In a browser |
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

The wasm API the page can call is `add_archive(name, bytes)`,
`add_asset(path, bytes)`, `stored_assets()`, `drop_assets()` and `selftest()`,
plus the `#[wasm_bindgen(start)]` entry point that runs the launcher. `web/loader.js`
exposes them on `window.peacecraft3` for the console.

How the table above was established: the page was served with
`python3 -m http.server` from the repository root and driven over the Chrome
DevTools Protocol in headless Chromium 153, so the log contents and timings
quoted are observed values from that run, not claims from reading the source.
The mode axes, the log, and the archive summary in the screenshots are what the
`#launcher-canvas` element actually rendered.

## Why the entry point is not `main`

`main` ends in `std::process::exit`, and wasm has no process to exit. The browser
boots from `browser::start`, a `#[wasm_bindgen(start)]` function that runs the
Bevy app and returns. `start` checks for a DOM first: without one there is
nothing for winit to attach a window to, and the module still answers its asset
API rather than trapping mid-initialisation.

## Two things that only bite on wasm

* **A re-entrant `std::sync::Mutex` panics.** On `wasm32-unknown-unknown` `std`'s
  mutex is single-threaded and turns a second lock on the same thread into an
  `assert!` rather than a deadlock, so the same mistake that hangs a desktop build
  traps here. `browser::held()` documents it; hold the guard in a binding.
* **No cross-thread work.** Nothing in the browser path spawns a task, so the
  launcher runs on the main thread and a long decode blocks the page. A 440 MB
  archive takes ~90 ms cold and ~27 ms once the page is warm, which is fine; a
  real staging pass would not be.

## Caveats

* **`pcx-net`'s UDP transport is compiled out on wasm.** `transport` is behind
  `#[cfg(not(target_arch = "wasm32"))]` (`crates/pcx-net/src/lib.rs`), because a
  browser has no UDP to bind and a build that fails to link on `std::net` is worse
  than one that simply has no transport. `pcx-net` is not currently a dependency
  of this crate, so the launcher does not link it; the gate matters for whatever
  eventually adds multiplayer.
* **The renderer needs a GPU that speaks WebGL2.** A software adapter (SwiftShader
  under headless Chromium) works but logs `GPU preprocessing is not supported`,
  and disables SSAO, atmosphere, order-independent transparency and depth of field.
  On such a device the GUI still renders and the API still answers.
