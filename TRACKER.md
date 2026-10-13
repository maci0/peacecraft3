# PeaceCraft3 — Tracker

Legend: `[ ]` todo · `[~]` in progress · `[x]` done · `[!]` blocked · `[?]` open unknown

Last updated: 2026-10-13

Current menu verification (2026-10-13): WASM release build succeeds; selected
decoder, graphics and app tests pass (131 passed, 6 explicitly ignored).
PeaceCraft settings precede the menu, buttons use relative flow and rem/viewport
sizing, and hidden scenes stop rendering and advancing. Bone pivots, parent
rotations, sparse node IDs and fractional global-sequence timing have regression
tests. Original JPEG logo alpha is decoded; no-alpha decoding is preserved.
Desktop and narrow browser checks verify menu entry, clean screen clearing,
resize hit-testing and original click/music playback without JavaScript errors.
Those checks isolate UI from the unfinished 3D scene.
Full browser scene captures now show coherent Orc camp geometry; the previous
large red planes no longer reproduce. Full fidelity remains open: exact TFT
scene/logo selection, grey sky, helper inheritance, particles, texture animation
and blend/lighting behavior still need evidence.

Earlier format-sweep baseline: **15 crates / 336 tests passing, 0 failing**, `cargo fmt`
clean — expect the number to move, because `pcx-trig` is still being edited
concurrently. Engine is **Bevy 0.20** (upgraded from 0.17 this wave; the four API
migrations that forced are in the fidelity log). The coordinator's last measurement
before the wave was **314 tests, 0 failures** on 1.28c and the same suite on 1.29b,
with every `#[ignore]`d asset sweep green on both builds — that still holds; the count
grew with the four new crates.

The full-asset sweeps are `#[ignore]`d because they walk the whole shipped set; run one
with `cargo test -p <crate> -- --ignored --nocapture`. Nine crates carry twelve such
sweeps (`pcx-blp`, `pcx-map`, `pcx-mdl` ×3, `pcx-objects`, `pcx-play`, `pcx-script`,
`pcx-slq`, `pcx-tga`, `pcx-trig` ×2). The sweeps discover installs rather than being
told one: `PEACECRAFT3_INSTALL` (default `wc3/game/app`, the 1.28c tree) and
`PEACECRAFT3_LEGACY_DIR` (default the Battle.net Legacy install, 1.29b). Both present →
one run covers the matrix; only `PEACECRAFT3_LEGACY_DIR` set → 1.29b alone.
**Every sweep is `failed=0` on both 1.28c and 1.29b** — MDX, BLP, SYLK, TGA, the map
container family (`doo`/`shd`/`mmp`/minimap image), `pcx-objects`, `pcx-script` and
`pcx-trig`. TGA's 1.29b run still pins its own measured footer split — see *Known
issues*.

---

## M0 — RE harness and evidence base

- [x] Locate and extract WC3 Complete Edition install (Inno Setup → `wc3/game/`)
- [x] Inventory shipped binaries and archives
- [x] Install/wire REA (`rea-agents@6.3.0`) into opencode
- [x] Fix Ghidra provider (Flatpak tree is unsymlinkable → `.tools/ghidra` symlink farm)
- [x] Verify REA end-to-end on `game.dll` (71 810 procedures, 17 207 strings)
- [x] Verify REA end-to-end on `Storm.dll` (1 341 procedures, 736 strings)
- [x] Pin `GHIDRA_INSTALL_DIR` + startup deadline in `~/.config/opencode/opencode.jsonc`
- [x] `tools/rea.sh` driver with per-target analysis snapshots
- [x] Workspace scaffold + crate skeletons (`cargo build --workspace` green, Bevy **0.20**)
- [x] `pcx-mpq`: raw header parse + assertions against all four real archives
- [x] `docs/re-notes/000-mpq.md` written with the measured header table
- [x] Negative result recorded: brute-force (position, seed) sweep with the published
      `CryptTable` keystream yields no region markers → RE required, not guesswork
- [x] Answer RE-Q1: exact MPQ header layout — confirmed, §2.3 item 1 (`ev_faddef74…`)
- [x] Answer RE-Q2: filename hash + seed policy — 4 slices, §2.3 item 3 (`ev_ddc2808a…`)
- [x] Answer RE-Q3: table cipher + file-key derivation — §2.3 items 4 and 5 (`ev_703ee25f…`)
- [x] Answer RE-Q4: compressors — PKWARE implode dominant, zlib present; imploder working
- [x] Record answers in `docs/re-notes/000-mpq.md` with `ev_…` citations
- [?] Header words `0x20`/`0x24` still unattributed; nothing depends on them yet
- [x] Harvest classic symbol map from `game.dll` build-tree strings → `docs/re-notes/001-classic-symbols.md`
- [x] Record Reforged CodeView/PDB references from the local install → same note
- [?] Storm's live table at `+0x4C2E4` differs from the fixed table the archives use
      (all 1280 words). `FUN_15017650` builds it lazily. Recorded, not blocking.

## M1 — MPQ layer (first shippable milestone) — **done**

- [x] `pcx-mpq`: header parse per RE findings
- [x] `pcx-mpq`: hash table build + decrypt
- [x] `pcx-mpq`: block table parse + decrypt
- [x] `pcx-mpq`: name lookup with Storm's probe order, case-insensitive
- [x] `pcx-mpq`: sector read + file-key derivation (basename, slice `0x300`, `FIX_KEY`)
- [x] `pcx-mpq`: decompression — PKWARE implode + zlib working
- [ ] `pcx-mpq`: decompression — ADPCM (`0x40`/`0x80`). Blocks all 2,685 `.wav` in `war3.mpq`;
      the independent reference fails identically, so it is a format gap, not a reader bug.
      Schedule + step table must come from Storm's decompressor before it ships — not guessed.
- [ ] `pcx-mpq`: decompression — sparse, Huffman (explicit `Unsupported`, not guessed)
- [x] `pcx-mpq`: archive priority resolution (`DEFAULT_PRIORITY`, still assumed — see M0 open)
- [ ] `extract` subcommand, glob patterns
- [x] Test: `(listfile)` decodes end to end — 529 298 bytes of imploded sectors → real paths
- [x] Test: known-answer block entry for `war3.mpq` block 0
- [x] Test: live hash entries cover every existing block, in all 4 archives
- [ ] Golden test: byte-identical extraction vs Storm under Wine
- [x] **Gate:** list + extract works on all 4 archives

## M1b — Play-test harness skeleton (new)

`pcx-play` — 15 lib tests plus a `pcx-diff` CLI integration test, Bevy-free. The trace
is canonical text (no serde: fixed field order and sorted rows are what makes it
byte-stable), and its decoder refuses anything that is not already canonical so
`encode(decode(b)) == b`.

- [x] `pcx-play`: canonical trace format — header carries scenario, map, asset-mode
       (`vanilla`/`hd`/`reforged`), seed, the active fix-ledger rows
       (`adopt|reject|defer <id>`) and tick count, so a `vanilla` run can never hash or
       read like a `reforged` one
- [x] `pcx-play`: per-tick rows — unit state, RNG draws, timer events, sorted
- [x] Determinism proven: byte-identical `encode()` plus equal FNV-1a 64 across two
       independent runs, and the same at 10,000 ticks (`#[ignore]`d, green)
- [x] `pcx-diff` binary: first divergence as one line — `unchanged` (exit 0) or
       `diverge tick=<n|header> field=<path> left=<v> right=<v>` (exit 1). Compares
       header identity first, then ticks in order, then the record tail
- [ ] `pcx-play`: scenario script parser (`.rpl`) — the format is sketched
       (`SpawnSpec`/`ScriptedAction`), real ingestion is M3+
- [ ] `pcx-play`: deterministic action scheduler wired to real sim steps
       (select/move/attack/build/cast/train/wait)
- [ ] `pcx-diff`: seed minimiser for failing Tier C scenarios
- [ ] `tools/playrun.rs` CLI
- [ ] One scenario per Rosetta campaign chapter; assert specific outcomes, not "no crash"
- [ ] Gate: one trace byte-identical across 100 consecutive runs, and `unchanged` vs baseline
- [ ] M5/M6 trace columns: hp/mana/xp/cooldown, and RNG draws taken *inside*
       `pcx_sim::step` rather than beside it

## M1c — Reforged asset pipeline (new)

Local source: `/home/maci/Faugus/battle-net/` — Reforged **3.0.0.24268**, CASC storage
(`Data/data/*.idx` + `*.0NN`), not MPQ. See `PLAN.md` §3.7.

- [x] Reforged CASC store **opens** against the local install
      (195 configured archives, 33 on disk, 16 indices, **74 500 entries, 34.96 GB**)
- [x] Recover the build config from `Data/config/3a/9d/…`: build-name `3.0.0.24268-retail`,
      build-uid `w3`, comment `RETAIL - RC 3.5 LAUNCH BUILD`, `root`, `encoding`, `vfs-root`
      EKeys, 110 VFS entries
- [ ] `pcx-casc`: BLDR/encoding chain so an EKey actually yields bytes
      *(measured: reading the first enumerated EKey returns 0 bytes — decoding is not wired)*
- [ ] `pcx-casc`: resolve a content **path** to an EKey (WC3 FDID/VFS mapping, not yet solved)
- [ ] `pcx-assets`: layering vanilla MPQ | Reforged CASC | `override/`, with provenance
- [ ] Asset modes: `vanilla` (default) / `reforged` / `reforged-hd` / `reforged-fixes`
- [ ] `override/` drop-in tree + generated `index.json`
- [ ] Gate: Reforged Footman loads through the same `pcx-gfx` path as vanilla
- [ ] `docs/reforged-fixes.md` fix ledger; rows default to `defer`

## M2 — Asset decoders + first Bevy render — **rendering/material gate closed**

- [x] `pcx-blp`: BLP1 header, 16-entry mip offset/size tables, palette
- [x] `pcx-blp`: BLP1 direct/palettised mips — planar index + planar alpha
- [x] `pcx-blp`: BLP1 JPEG mips — shared header at `0x9C` + per-mip scan data
- [x] `pcx-blp`: inverted-CMYK → RGB, verified against an independent decoder
- [x] `pcx-blp`: sweep gate — **6,273 / 6,273** shipped textures decode
- [x] `pcx-blp`: `blpx` inspector (`info` / `dump` / `pixels` / `hash` / `export`)
- [x] `pcx-blp`: BLP0 (external `.b00`–`.b15` mips) — **does not exist in either build**;
      `Header::parse` now *refuses* it with `Blp0ExternalMips` instead of misreading it as
      BLP1 and handing callers a garbage mip table. Zero shipped instances.
- [x] `pcx-blp`: BLP2 (BGRA / DXT) — **does not exist in either build**; refused with
      `Blp2Unmeasured { format }`. Zero shipped instances.
- [x] `pcx-blp`: census proves coverage is complete — every **existing block** of every
      archive was walked on both builds (not just the union of the listfiles), and every
      `content` byte in every shipped BLP is `0` (JPEG) or `1` (direct). Nothing remains
      open. The refusals are now a visible `blpx info` census row rather than a silent gap.
- [x] `pcx-tga`: RLE + uncompressed, 16/24/32 bpp — **383 / 383** on 1.28c,
       **435 / 435** on 1.29b, 0 unread, 0 failed on both
- [x] `pcx-mdl`: `MDLX` container walk — **3,234 / 3,234** shipped models, all v800
- [x] `pcx-mdl`: `MODL` name + model extents
- [x] `pcx-mdl`: `SEQS` animation timeline (132-byte records)
- [x] `pcx-mdl`: `TEXS` texture list (268-byte records)
- [x] `pcx-mdl`: 20 chunk tags; unknown tags are a hard error, not a skip
- [x] `pcx-mdl`: `BONE` skeleton — variable-length records, resolved by the
       "size excludes an 8-byte trailer" relation rather than a fixed stride;
       **decoded** — 35,257 records on 1.29b, 35,097 on 1.28c, walk lands exactly
       on every one
- [x] `pcx-mdl`: `GEOS` geoset envelope — self-inclusive sizes, exact walk
- [x] `pcx-mdl`: `GEOS` sub-blocks — VRTX/NRMS f32x3, PVTX u16, UVBS f32x2, GNDX
- [x] `pcx-mdl`: `GEOS` skinning — `MTGC`/`MATS`/`GNDX`, all three invariants verified
- [x] `pcx-mdl`: `GEOS` trailer — `materialId` at `MATS + count*4`, plus
       selectionGroup / unselectable / boundsRadius / extents / seqBounds, behind a
       **44-byte** fixed part with the `seqBounds` array at `+0x2C`. Decoded **and
       applied**: `to_bevy` binds `paths[material_id]`.
       Four assertions hold with zero failures — trailer lands on the next
       sub-tag for all 17,442 geosets of both builds, `matrix_span_u32s ==
       MATS.count`, `material_id < material count` unconditionally, and
       `unselectable <= 4`. `seqBounds` count ∈ {0, sequence count}: 5,971/2,701
       on 1.28c, 6,084/2,686 on 1.29b, nothing in between
- [x] `pcx-mdl`: `MTLS` layer bodies — `textureId` at LAYS+20, declared length at LAYS+8
- [x] `pcx-mdl`: `TXAN` texture animation — 279 records, `len == 4 + Σ tracks`
- [x] `pcx-mdl`: `RIBB` ribbons — 715 records, node + 52-byte body + tracks
- [x] `pcx-mdl`: `PREM` particles — 128 records, node + 284-byte body + `KPEV`
- [x] `pcx-mdl`: `CAMS` cameras — 1,300 records, **no node header**,
       `size == 120 + Σ tracks`, `far > near` in 1,300/1,300
- [x] `pcx-mdl`: `LITE` lights — 500 records, node + 44-byte body + tracks,
       `kind ∈ {0,1,2}`
- [x] `pcx-mdl`: `PRE2` **framing** — 3,572 records on 1.28c, 3,628 on 1.29b,
       0 failures. Three relations pin it, and all three hold on 7,200/7,200:
       `node_size == 96 + Σ node tracks`; `total == 4 + node_size + 171 + Σ
       sub-chunks`; and the sub-chunk chain is sorted `KP2S < KP2L < KP2E <
       KP2V < KP2N < KP2W`. All six `KP2*` tags decode fully as scalar key
       tracks. The 171-byte prelude is **measured in width and left unnamed** —
       kept as raw bytes rather than guessed into named floats. 22 records open
       on a negative frame, the same real phenomenon as camera tracks.
- [x] `pcx-mdl`: `CLID` **framing** — 846 records on 1.29b, 832 on 1.28c, 0
       failures. `header_size == 96 + Σ node-track bytes`, **not** a constant 96:
       the constant-96 rule holds for only 94% of records and 96 of them carry a
       `KGTR` track inside the header. Kind word is only `0` or `2` and anything
       else is **refused**, never defaulted. Floats stay raw and uncaptioned:
       box-as-AABB and sphere-as-centre+radius are supported but unproven, and a
       wrong collision shape lets units walk through walls.
- [x] **No MDX chunk is left with undecoded framing.** `PRE2` and `CLID` were the
       last two; what they refuse is *semantics* — `PRE2`'s prelude and `CLID`'s
       float meanings — not framing.
- [x] `pcx-gfx`: Bevy loaders — `load_image`/`load_model`, magic-based format detect
- [x] `pcx-gfx`: `to_bevy` — Footman becomes 5 Bevy meshes; skin tables intact
- [x] `pcx-gfx` viewer: `peacecraft3 view` spawns the Footman (5 geosets, camera
       and light framed from the model's own extents)
- [x] **The render is verified by a pixel test, not by a human.** `pcx-gfx`
       `capture` example renders the Footman offscreen (no window, render target
       image, posed at 7,000 ms) and `analyze` asserts on the readback:
       45,759-byte PNG, 100% opaque coverage, **18.7% lit**, **1,353 distinct
       5-bit colours**, row profile is a standing figure. The old "a human must
       eyeball the render" item is closed.
- [x] **The material fix landed and the capture proves it.** Binding by
       `materialId` moved the capture from 49,953 to 45,759 bytes and from 1,588
       to 1,353 distinct colours, with **11,704 pixels (11.4%)** changed in the
       lower body only — silhouette and camera identical. The blood geoset now
       draws `gutz.blp` and the empty-material geoset is flat grey instead of
       wearing the blood texture.
- [x] **M2 rendering/material gate: CLOSED.** Skinning, material binding and the
       terrain tile are in, the offscreen capture is asserted by a pixel test, and
       `to_bevy` no longer cycles by geoset index. Nothing
       material-binding-related remains.
- [x] `pcx-mdl`: full mesh sweep — 8,672 meshes (1.28c) / 8,770 (1.29b),
       968,613 / 978,194 vertices, 0 failures
- [x] `pcx-mdl`: `MTLS` material / texture-layer binding
- [x] `pcx-mdl`: GEOS skinning matrices → Bevy `Mesh`
- [x] **Bevy upgraded 0.17 → 0.20** in `pcx-gfx` and `peacecraft3`. Four API
       migrations forced, all of them silent-behaviour traps rather than compile
       errors — see the fidelity log for the `FontSize` / `BorderRadius` /
       `Button` / `Pressed` / wasm asset-source details. The offscreen capture
       re-measured at **46,006 bytes, 18.7% lit** after the upgrade — silhouette
       head→torso→legs plus the terrain tile intact, so the migration is verified
       by pixels rather than by it compiling.
- [x] `pcx-gfx` viewer: `AnimationClip` playback. One clip per `SEQS` sequence,
       built from the model's own `KGTR`/`KGRT`/`KGSC` keys, played through an
       `AnimationGraph` on a fixed tick. The test samples every animated bone of
       the Footman **and** the Knight at instants *between* the bone's own keys,
       so it pins the **Hermite interpolation** rather than the stored key values —
       and it is proven non-vacuous: returning `Quat::IDENTITY` or `Vec3::ZERO`
       from a curve fails it. Past a sequence's end the clip holds that
       sequence's last pose instead of sliding into the next sequence's keys, and
       the test asserts some sequence exists where that is observable.
- [x] `pcx-slq`: `.slk` / SYLK tables — **not** a Miles container, see below
- [ ] `pcx-gfx`: Bevy loader plugins (asset cache, async decode)
- [ ] `pcx-gfx` `view`: no terrain tile (the capture draws one; the viewer does not)

#### `pcx-slq` — the premise was wrong, and the crate name is now a misnomer

There is no binary Miles "sound list" container and no `.slq` file in either archive.

- **The sound tables are `UI\SoundInfo\*.slk`, plain ASCII SYLK** (Spreadsheet Link Key),
  the same dialect `Units\UnitData.slk` and `Units\ItemData.slk` use.
- **59 `.slk` files ship on 1.28c and 65 on 1.29b**, 12 of them sound tables (13 on
  1.29b, which adds `AmbientMusic.slk`). Sweep: **59/59** and **65/65** decoded,
  **0 failed**, 768,067 / 983,763 cells, 10,373,503 / 12,971,315 bytes.
- Corroborated independently of the bytes by vanilla 1.28c `game.dll`: the literal
  `ID;PWXL;N;E` at `.rdata+0x95494e` (VA `0x6f95494e`, base `0x6f000000`), followed by
  `\Database.cpp`, `DATABASE`, `DBFIELD`, `DBENTRY`, with the twelve `UI\SoundInfo\`
  paths listed verbatim at `.rdata+0x937064`.
- **The non-obvious part is the cursor rule, not the format.** `X`/`Y` are sticky state
  shared by `C` *and* `F` records; a record omitting a tag reuses the last value from
  either letter. `DialogSounds.slk` carries no `Y` on any of its 14,634 `C` records, so
  reading `C` alone collapses it to 2,231/2,439 rows, double-writes 1,249 cells and
  invents 202 gap rows. Reading the cursor off both letters leaves zero duplicates and
  zero gaps across all 124 shipped files.

Data findings worth keeping: the only cross-build cell difference in the entire sound
family is **one** cell — `UnitAckSounds.slk` row 244, where 1.28c lists 5 files ending in
a bare `FrostwyrmPissed5` with no extension and 1.29b lists 4 and dropped it, though
`FrostwyrmPissed5.wav` ships in both archives. 1.29b also strips all Excel presentation:
zero `P`/`F`/`O` records and zero `E` formulas across all 65 files, against 462 / 2,868 /
3 / 2,430 on 1.28c.

**Gate:** met for the renderer; met for animation and the interactive viewer's terrain
tile in the offscreen path only — and animation now reaches the viewer through a real
`AnimationClip` on Bevy 0.20 rather than a posed skeleton. The open items are the
viewer's terrain tile and the asset-cache plugins — neither is a format gap.

### Known issues

- JPEG mips match an independent decoder to within ±1 per channel on 1.9% of
  bytes (15,111 / 786,432 on `AshenvaleExpansion-TopLeft.blp`); this is IDCT
  rounding between `jpeg-decoder` and libjpeg, not a format mismatch. Chasing
  bit-exactness would mean porting libjpeg's IDCT, so it is documented rather
  than fixed.
- `cargo clippy` cannot run in this environment: the installed clippy bundles
  rustc 1.98 while the workspace targets 1.99, so every crate is rejected.
  `cargo test` and `cargo fmt` both work.
- `nodes()` must not be pointed at `CAMS` or `LITE`. Both are read through their
  own readers, and the shared one is *kept* as a pinned misread rather than
  deleted: 30 of 656 camera records on 1.29b yield a `parent_id` in `0..100000`
  that is indistinguishable from a real node index, and 0 of 250 light records
  get their name or ids right. The traps are documented in
  `docs/re-notes/004-mdx.md`.
- The offscreen capture is 320×320 with the camera framed from hard-coded
  numbers in the example. It is a smoke test for "something drew and it is the
  right shape", not a parity check against the original — the parity tier still
  needs the differential renderer described in §3.3 item 3.
- `pcx-tga`'s footer split is per install, not per format: `(381, 2)` on 1.28c
  and `(433, 2)` on 1.29b. The sweep now pins the pair for whichever install
  `PEACECRAFT3_LEGACY_DIR` selects, so both builds assert their own measured
  split instead of one build silently failing a constant written for the other.
  Both pass, 435/435 and 383/383, 0 unread and 0 failed.

## Browser / wasm render path (verified)

- [x] wasm32 release build of `peacecraft3` compiles
- [x] Wasm runs in a browser on **WebGL2** (ANGLE/Vulkan/SwiftShader). Proof:
      `chromium --headless` loads `http://127.0.0.1:8931/index.html`, and the
      Bevy adapter log reports `AdapterInfo { backend: Gl,
      driver_info: "WebGL 2.0 (OpenGL ES 3.0 Chromium)" }` with **0 page
      errors**. Driven by a Playwright/CDP session
      (`/tmp/opencode/menu-check.mjs`) rather than a `--virtual-time-budget`
      screenshot, because a continuously rendering Bevy window never settles
      under a virtual clock.
- [x] **WebGPU is not available on this host**, only in the API surface:
      `navigator.gpu` exists but `requestAdapter()` returns null in both
      Chromium 153 and Chrome 155, with or without `--enable-features=WebGPU`.
      wgpu therefore selects the WebGL2 backend, and the app runs on it.
- [x] The launcher canvas is created inside `#launcher-canvas` at 880×860.

### Original main menu (M7 entry screen) — WC3-styled, interactive

- [x] `crates/peacecraft3/src/mainmenu.rs` — the menu's geometry, labels and
      art paths measured from `UI\FrameDef\Glue\MainMenu.fdf` and
      `UI\FrameDef\Glue\StandardTemplates.fdf`. 16 unit tests.
- [x] Menu draws over the staging panel with the original's own labels in the
      original's gold (`StandardButtonTextTemplate`'s `FontColor 0.99 0.827
      0.0705`) at `FrameFont "MasterFont", 0.013` of the window height.
- [x] **A click reaches Rust and is acted on.** Clicking *Options* despawns the
      menu layer and reveals the staging panel. Before/after screenshots
      (`/tmp/opencode/menu-before.png` vs `menu-after.png`) show all six entries
      gone, which can only be produced by a click landing on a menu frame.
- [ ] **The button's `GlueScreen-Button1-Border.blp` art is NOT shown in the
      browser.** `load_glue_art` reads a desktop install path, and a browser
      cannot see those paths, so the frames render without their 9-slice art.
      The upload path in `web/loader.js` (`add_archive`) opens memory archives
      but the glue art is never routed through it. Closing this means either
      serving the archives over HTTP from the page, or wiring
      `add_archive`/`add_asset` into `load_glue_art`.
- [ ] **The 3D backdrop (`MainMenu3d.mdx`) and logo (`WarCraftIIILogo.mdx`) are
      decoded and their bytes carried in `GlueArt`, but not rendered.** They are
      recorded as booleans only. Honest status: unfinished.
- [ ] Labels are drawn centred in the frame, per `StandardButtonTextTemplate`'s
      `JUSTIFYCENTER`/`JUSTIFYMIDDLE`. The frame itself is right-anchored at
      `1 - 0.015`, which on an 880-wide canvas leaves a 13 px right margin.

## M3 — World: maps, terrain, camera, doodads

**Map compatibility is now a hard requirement** (see `PLAN.md` §3.11). Loading the
campaign is not the acceptance bar; DotA, Athena and Footman Frenzy running unmodified is.
The list below is the measured part; the rest of the container surface is tracked in
`PLAN.md` §3.11 and is **unverified** until each lands with its framing asserted.

- [x] `pcx-map`: W3M/W3N container — **`HM3W` + `0x200` header + embedded MPQ**, no chunk
      table. `archive_size == file_len - 0x200` on 133/133; `NGIS`+256B trailer on 89/133,
      absent on 44, any other length refused. 89 maps on 1.28c (88 `.w3m` + 1 `.w3n`),
      44 on 1.29b.
- [x] `pcx-map`: terrain `war3map.w3e` inside the embedded archive — framing
      `17 + 4N + 4 + 4M + 16 + 7·(w+1)·(h+1)` holds on **132/132**, zero remainder.
      Sizes 64×64 … 224×160, 127 distinct slot fourccs.
- [x] **Height ramp: refused, and it does not exist.** The corner table consumes the file
      *exactly*, leaving zero bytes for an interior grid. `corner_size == 1`, heights are
      raw **14-bit indices** handed out as `height_index` with no world-unit claim; shipped
      indices span only 5,477..14,336 of 16,384 so the data cannot supply the mapping
      either. Terrain is per-corner, not a sampled heightfield — the older "tilepoints +
      interior heightmap" framing in `PLAN.md` was wrong and is corrected.
- [x] `pcx-map`: `war3map.doo` doodad instances — `W3do` + version 7 (8+ **refused**,
       its records are wider by item-drop fields no shipped file proves the offset of),
       subversion 9 on all 132, `16 + 42·n + 8 + 16·m == file_len` on 132/132. Instance
       record is a fixed 42 bytes: type fourcc, variation, position (3×f32), rotation,
       scale (3×f32), flags byte, life byte, creation number. Terrain doodads are 16
       bytes. Creation numbers are unique but **not** dense and not bounded by `n`
       (`Undead03`: 3,941 instances, ids up to 4,146) — editor identities, not indices
- [x] `pcx-map`: `war3map.shd` shadow map — `(4·w) × (4·h)` one-byte mask, **no header**;
       `file_len == 16·w·h` against the *terrain's* tile dimensions on 132/132, because
       the file does not carry its own grid. Values are exactly `0x00`/`0xFF`; a mid-value
       is handed back unchanged rather than refused, since only the writer's output is
       measured
- [x] `pcx-map`: minimap — the image is `war3mapMap.blp` (**BLP1 JPEG, 256×256, one mip**,
       132/132 framed through `pcx-blp`, not re-decoded here) and the markers are
       `war3map.mmp` (`8 + 16·n`, kind/`x`/`y`/`0xAARRGGBB`). 132/132 on both builds
- [x] **`war3map.map` / `war3map.ter` do not exist** — 0 of 133 shipped maps, 0 of 200
       loose community maps, no `(listfile)`, and in neither build's name tables
       (`game.dll` names `war3mapMap.blp`/`.tga`; the Reforged PDBs name
       `war3map.mmp`/`war3mapMap.blp`/`.shd`/`.doo`). A map really carrying one is
       **refused** (`UnprovenContent`) — there is no layout to read it with
- [ ] `pcx-map`: regions (rects / circles / polygons)
- [ ] `pcx-map`: `war3map.w3i` map info — player slots, flags, declared object tables
- [ ] `pcx-map`: preplaced units, item sets, random tables
- [ ] `pcx-tera`: terrain mesh generation from per-corner indices + slot texture blending
- [ ] `pcx-tera`: cliff walls, water rendering
- [ ] Camera: bounds, pitch, pan speed, zoom steps — values from RE
- [ ] **Gate:** Human campaign mission 1 loads and pans identically
- [ ] **Gate:** a community map's full container parses — container, `w3e`, `doo`, `shd`,
       minimap and `w3i` now all measured; `w3i`, `wpmp` and regions are what is left

## M4 — Object data

Object *edits* shipped inside a map are now in scope, not optional: DotA and Athena both
ship custom objects, and a map whose rawcodes do not resolve is unplayable.

- [x] `pcx-objects`: **the `W3?` container is measured, and only two of the seven
       extensions ship.** Census over every map's embedded archive on both builds:
       **47 `.w3u` + 1 `.w3t` on 1.28c, 44 `.w3u` on 1.29b — 92 files, 2,179 object
       records, 5,975 modification entries.** Version word `1` in 91 files, `2` in one
       (1.29b `Maps\Campaign\Human06.w3m`), same layout byte for byte
- [x] `pcx-objects`: layout and framing — `u32 version` + base table + custom table,
       each `[original fourcc, modified fourcc, mod count, entries]`; each modification
       is `field fourcc | type word (0 int / 1 real / 2 unreal / 3 string) | value | u32
       end word`. `file_len == 4 + 4 + Σ_base(12 + Σ_mods) + 4 + Σ_custom(12 + Σ_mods)`
       on **92/92**, plus round-trip `canonical_bytes() == raw` on the same 92
- [x] `pcx-objects`: two more relations on 2,179/2,179 records — every first-table
       record has `modified id == 0`, every second-table record has `modified id != 0`
- [x] `pcx-objects`: strings are **NUL-terminated, not length-prefixed** — 1,937 string
       values (315 empty, all valid UTF-8), each ending in one NUL with the end word
       immediately after. The DSP-style leading length word the folklore describes is
       **refuted by the bytes**
- [x] `pcx-objects`: **`w3a`/`w3b`/`w3d`/`w3h`/`w3q` ship in no map on either build —
       proven absent, and refused** (`Error::UnmeasuredExtension`) rather than run
       through a w3u-shaped walk. Nothing measures whether ability records carry the
       level/variation columns the community notes ascribe to them
- [?] `pcx-objects` refused semantics, recorded not guessed: per-fourcc int width and
       signedness (a four-byte slot holding small values is indistinguishable from a
       `u8` field, so `Value::Int` hands out the raw word); what `real` and `unreal` do
       differently; and the **end word** — 0 in 5,690 of 5,975 entries, 285 exceptions:
       106 repeat their record's own original id (`DemoCampaign.w3n`'s
       `war3campaign.w3u`) and **179 sit next to an `umdl`/`uico` art path carrying what
       reads like the owning unit's rawcode** (`"hcth"` beside
       `units\human\TheCaptain\TheCaptain`). Recorded as a hypothesis, not a proof, so
       the word is handed out as bytes
- [x] `pcx-objects`: shipped object editor data ingest — `Units\`: 15 `.slk` + 48
      `.txt` on 1.28c, 17 + 49 on 1.29b; `Doodads\`: 2 + 0 on both. The union
      listfile holds 59 `.slk` (1.28c) / 65 (1.29b). **No `Abilities\*.slk` and no
      `Misc\` directory exist** — the remembered names are wrong; the ability data
      is `Units\AbilityData.slk`, the misc constants `Units\/`UI\MiscData.txt`, the
      UI table is spelled `Units\unitUI.slk`, and the unit localisation tables are
      `.txt`, not `.fdf`. `Melee_V0`/`Custom_V0`/`Custom_V1` are the World Editor's
      working copies and are deliberately not decoded
- [x] `pcx-objects`: **the union listfile under-counts** — 1.28c's
      `Units\AbilityBuffData.slk` / `AbilityBuffMetaData.slk` live in
      `War3Patch.mpq`, whose `(listfile)` extract fails and whose files no listfile
      names, yet they decode (238 and 28 rows). `Editor::absent()` is empty on both
      builds; only `MpqSet::locate` finds them
- [x] `pcx-objects`: **the fourcc → column join is measured, not folklore.**
      `Units\UnitMetaData.slk` (267 rows) is the editor's own machine-readable
      schema; **267/267 rows resolve** to a row-1 column of the shipped table
      (`UnitBalance` 51, `UnitUI` 43, `UnitData` 24, `ItemData` 30, `UnitWeapons` 59,
      `UnitAbilities` 3) or to a `.txt` key (57 `Profile` rows, 49 shipped and 8
      refused). **0 duplicate fourccs.** Eight metadata tables hold **1 443 field
      rows and share none**
- [x] `pcx-objects`: **the editor's declared `type` predicts the wire word** of
      **3 393 of 3 401** 1.28c modifications; the single exception is `uhot`
      (declared `char`, shipped on the string wire 8 times) — applied as text, the
      `char` read refused. 836 unit rawcodes, unique in every one of the five unit
      tables
- [x] `pcx-objects`: **raw codes must be matched case-insensitively** —
      `Units\UnitBalance.slk` row 664 spells `Nman` while
      `Maps\Campaign\Orc04Interlude.w3m`'s `war3map.w3u` writes `nman`; 91
      mixed-case raw codes in that column alone. A case-sensitive join drops records
      the map really declares
- [ ] `pcx-objects`: `game.dll`-derived default tables
- [ ] `pcx-objects`: `war3map.wpmp` allied shared data
- [ ] Unit table: 4 races + neutral, buildings, heroes
- [ ] Ability table + ability levels/objects
- [ ] Item table, upgrade table, buff table, destructable table
- [ ] Tech tree, availability, random item/unit tables
- [ ] **Gate:** stock unit stats diff-clean against in-game editor
- [x] **Gate:** a map with custom objects resolves every rawcode it declares —
      1.28c **888 first-table records (887 resolved) + 282 custom (282 new, 0
      collisions)**, 1.29b **757 (756) + 252 (252)**, 0 unknown fourccs, 0 field
      mapping failures. **One rawcode resolves to nothing on both builds**: `Nwar`
      in `Maps\Campaign\Orc02Interlude.w3m`'s `war3map.w3u`, reported by name in
      `Refusal`/`Unresolved` rather than dropped — no shipped `.slk` or `.txt` on
      either install carries it, at any case. The only field-level mismatch is
      `uhot`, a declared `char` on the string wire (8 shipped entries)

## M5 — Simulation core

- [ ] `pcx-sim`: deterministic fixed-tick harness (headless, Bevy-free)
- [ ] Entity store, handles, deterministic iteration order
- [ ] Stats model: HP/mana/attack/armour/resistances/regen
- [ ] Damage pipeline: attack types, mitigation, crit, dodge, block, invuln
- [ ] Combat abilities, spells, channeling, stacking buffs/debuffs
- [ ] Orders: move/attack/attack-move/hold/patrol/follow/stop, queueing
- [ ] Pathing: walkability grid, local avoidance, formation offsets
- [ ] Cooldowns, cast times, mana cost, upkeep
- [ ] Projectiles, missiles, tracking
- [ ] Death, respawn, corpse decay, XP/levels
- [ ] **Gate:** 1v1 duel HP curves identical to original

## M6 — Game-loop parity

- [ ] RE-Q5: exact sim tick rate and frame relationship
- [ ] RE-Q6: RNG generator + call-site ordering
- [ ] Event ordering across systems
- [ ] Effect / sound scheduling parity
- [ ] `.w3s` save/load round-trip
- [ ] **Gate:** 10-minute scripted match bit-reproducible over 100 runs

## M7 — Scripting (JASS + triggers)

**Two code paths, one engine, and era tolerance.** Community maps reach the VM two
different ways and both must work: raw JASS (`war3map.j`, DotA's path) and the editor's
trigger graph (`WTG`/`WCT`, Athena's path). The community also spans pre-1.20 and
post-1.24 APIs in active use simultaneously.

#### `pcx-script` — the raw-JASS front end (done, and it stops at syntax)

- [x] JASS lexer + parser — **46 passing tests** plus the `#[ignore]`d corpus sweep, and
       it parses **1,326 real scripts with round-trip proven on every one**. Corpus:
       462 install scripts (`Scripts\*.j`/`*.ai` out of both installs' MPQs,
       `common.j`/`Blizzard.j`/`Cheats.j` among them), 191 `war3map.j` from the shipped
       `.w3m` maps of both installs, 676 from the community `.w3x` maps in those Maps
       trees, and 3 from `DemoCampaign.w3n`. **24,085,861 tokens, 335,112 functions,
       2,288,966 statements.** 3 of 1,329 files are not UTF-8; they are named by path
       and their lossy-decoded form still parses
- [x] Round-trip proof: re-lex to identical token kinds, re-parse to identical canonical
       bytes, same FNV-1a 64 hash, idempotent print — on every real script, not just
       hand-written cases
- [x] **Spans on every node**, and `SourceMap` maps byte offsets to the map's own
       1-based line/column. A missing native or unresolved rawcode can therefore be
       reported against the map's own identifier *and* position
- [x] Two real bugs fixed here: **bare-CR line endings** (a shipped protected map's
       `war3map.j` is CR-only and the game accepts it, so `\r` ends a statement like
       `\n` and `\r\n` is one break), and **`SourceMap` reporting every position as
       line 1**
- [x] Deliberate refusals: vJass `method`/`cast`/`delegate` and Zinc (`.j` library and
       scope *containers* parse, their bodies are vanilla), `.` member access and `..`,
       bare `!`, comma-separated local lists, `constant … array`, chained `a[i][j]`
- [ ] **No interpreter by design** — no evaluation, no name/type resolution, no native
       dispatch, no `^^` code-tag substitution. A half-VM on a lossy tree is worse than
       no VM, because the loss is invisible and the result is *plausible but wrong*

#### `pcx-trig` — the trigger-graph containers

- [~] **Framing decoded and sweeping green; the `wts` expectation tests were still
       being finalised during this wave and are now green.** ~2,668 lines of decoder
       plus the `wts` module, every test passing as of this write (`pcx-trig` is being
       edited concurrently, so re-run before trusting the count). While this wave was
       being written up, six exact-value expectations in the `wts` module were failing —
       a `2632 vs 2631` record count, a BOM-present assertion, a `23 vs 21`
       trailing-byte count, a body that ends with a blank line, and two refusals counted
       as failures. **A concurrent agent fixed them mid-wave**; the fix also picked up
       `DemoCampaign.w3n`'s `war3campaign.wts`, which is what the record-count delta
       was. Verified here after the fix: `cargo test -p pcx-trig` green, both sweeps
       `failed=0`
- [~] One community file is **refused as unprovable** by the `wts` walk:
       `Maps\Siege\X Hero Siege 3.08.w3x` opens `3F 53 54 52 49 4E 47` — `?STRING 1`,
       a stray byte ahead of the first header. It is in no other file, it is not a
       documented construct, and guessing what it means would be a plausible-looking
       lie (`Unsupported::NotAHeader`). The sweep reports it as a typed refusal, not a
       framing failure
- [x] Census on both builds: **1.28c 265 files (89 wts / 88 wtg / 88 wct)** with 5,171
       string entries, 3,207 variables, 4,689 triggers (7 custom), 57,289 ECAs,
       172,287 parameters, 4,689 wct blocks; **1.29b 132 files (44 each)** with 4,431
       entries, 3,113 variables, 4,442 triggers (7 custom), 55,036 ECAs, 165,922
       parameters, 4,442 wct blocks
- [x] `war3map.wts` — `STRING <id>` header (**space, not a colon**; `STRING:<n>` is
       refuted by 511,490 records across 913 files), brace-delimited raw bodies, a BOM
       on all 133 shipped files (11 of the 913 loose community files have none, every
       DotA variant among them), no count, no index, no version word and no trailing
       empty record. Bodies are raw bytes — a decoder that rewrote `|n` or stripped
       `^^` would corrupt every string in the map
- [x] `war3map.wts` ids are **unique within a file** (511,490 records, 0 duplicates)
       but **not monotonic**: 6 files restart their id run after 999, the classic
       editor's three-digit wrap, visible on `TowerWars` and `Counter-Strike`. Unique
       is what an id-keyed lookup needs, so that is what is asserted
- [x] `war3map.wct` — `u32 unknown0` + `u32 N` + `N × {u32 len; len bytes}`; 14 non-empty
       texts are NUL-terminated JASS with the NUL **inside** `L`, the 9,117 empty blocks
       are `L == 0` with no NUL
- [x] `war3map.wtg` — `WTG!` + **version 4 only** (version 7 is Reforged-era with child
       ECAs, `is_comment` and array-size words, and is **refused**, not walked with v4
       rules), then categories, variables, triggers, ECAs
- [x] **Parameter arity comes from `UI\TriggerData.txt`, never from the file** —
       `TriggerArity::from_trigger_data` builds the table (4 groups, 1,045 names on
       1.28c, 1,136 on 1.29b, 0 arity conflicts) and a name outside it is
       `Error::UnknownFunction`, not a zero-arity assumption
- [x] Parameter types 0/1/2/3 proven on 132/132; `-1` and any other discriminant are
       refused. Function-with-array and any non-zero function trailer are refused
       (`UnsupportedFunctionArray`) — array indexing is proven only for
       preset/variable/string parameters (1,431 index parameters)
- [x] Framing invariants hold **133/133 (wts), 132/132 (wct), 132/132 (wtg)**, plus the
       cross-file relations `wct.blocks == wtg.triggers`, per-trigger `is_custom != 0`
       exactly when the WCT text is non-empty, and zero ECAs on every custom trigger

#### What M7 still is

The two front ends exist and are measured; what remains is the engine. Nothing below is
closed by the work above:

- [ ] JASS VM: value model, handles, handle casting/wrapping
- [ ] `WTG` trigger graph → JASS desugar with the original's exact semantics (incl.
       generated-code naming so `GetTriggerName` returns the right string)
- [ ] `WCT` custom-text script compilation
- [ ] Native function table (exact signatures, return conventions and **side-effect
       ordering** from `game.dll`)
- [ ] `BJ` / `Blz` library surface, including the `Blz` extensions
- [ ] Timers and the original's timer/event ordering — the single thing DotA depends on
       most
- [ ] `war3map.wts` string handles + localization lookups (the *container* is decoded;
       the VM-side handle resolution is not)
- [ ] `war3map.ini` — script source selection, safe mode, forced flags
- [ ] Pre-1.20 **and** post-1.24 native shapes coexisting; a missing native or
       unresolved rawcode must fail loudly with the map's own identifier, never
       silently return zero
- [ ] **Gate:** **DotA** joins, creates units, runs its timers and completes a match with
       no script error; observable behaviour matches the original under
       `compare-process_captures`
- [ ] **Gate:** **Athena** runs — proves the `WTG` path, not just the script path
- [ ] **Gate:** **Footman Frenzy** runs — proves pre-1.20 API tolerance
- [ ] **Gate:** all stock maps load; Human campaign completes

A scripting layer that satisfies none of the three named maps has not met this
milestone, regardless of unit-test count.

## M8 — Full campaign + polish

- [ ] Campaign AI behaviour parity
- [ ] Cinematic playback
- [ ] Netcode surface (loopback first)
- [ ] UI parity (HUD, minimap, command card, tooltips, chat)
- [ ] Audio parity (`.slk` sound-list channels, 3D positioning)
- [ ] Performance pass (target 60 fps on 4v4 1080p)

---

## M9 — Campaign playthrough gate (Tier A + B)

See `PLAN.md` §3.5 for the harness design. Each chapter asserts specific outcomes.

### Tier A — scripted scenarios
- [ ] `tests/play/roc/` — 5 chapters (Reign of Chaos) + 3 (Frozen Throne)
- [ ] `tests/play/orc/`, `undead/`, `nightelf/` — chapters + 3/1/1
- [ ] `tests/play/demo/DemoCampaign.rpl`
- [ ] Gate: every scenario's expectations met, traces byte-identical across 100 runs
- [ ] Baselines captured from the original under Wine, each pinned to an `ev_…` id

### Tier B — completion
- [ ] Intro dialogue gates fire in order, per chapter
- [ ] All quest objectives satisfied and tracked
- [ ] Win condition triggers; end-of-movie / `endGame` entered
- [ ] Custom-map smoke set: all shipped `Maps/*.w3m` load and run their init script

### Tier C — differential fuzzing
- [ ] Seeded scenario generator (unit comps, terrain, RNG)
- [ ] Seed corpus checked in and reproducible
- [ ] Divergences auto-filed into *Open unknowns* with minimal reproducing seed
- [ ] Gate: zero unexplained divergences over the corpus

## Open unknowns

| id | question | settle by | status |
|---|---|---|---|
| RE-Q1 | MPQ header layout | resolved — §2.3 item 1, `ev_faddef74…` | **closed** |
| RE-Q2 | Filename hash + seed policy | resolved — §2.3 item 3, `ev_ddc2808a…` | **closed** |
| RE-Q3 | Table cipher + file keys | resolved — §2.3 items 4–5, `ev_703ee25f…` | **closed** |
| RE-Q4 | Compressors | resolved — implode + zlib working | **closed** |
| RE-Q5 | Sim tick rate / frame relationship | REA: `game.dll` main loop | open |
| RE-Q6 | RNG generator + call order | REA: `game.dll` `FRand` | open |
| RE-Q7 | Header words `0x20`/`0x24` | REA: `Storm.dll` open path; nothing depends on them | open |
| RE-Q8 | Storm's live table at `+0x4C2E4` ≠ fixed table | REA: `FUN_15017650`; archives use the fixed table | open, not blocking |
| RE-Q9 | Archive priority order | REA: `game.dll` archive-open sequence; `DEFAULT_PRIORITY` assumed | open |
| RE-Q10 | ADPCM decompression (blocks 2,685 `.wav`) | REA: Storm.dll ADPCM decompressor; derive the step table + index schedule from it, do not port from memory | open, the only gap left in M1 |
| RE-Q11 | Determinism of the original under Wine | REA `capture-process`; gates all of Tier A/B | open, blocks M1b |
| RE-Q12 | `blizzard.ax` purpose | REA / string scan | open |
| RE-Q14 | `game.dll` leaks its full build tree + class names | **closed** — `docs/re-notes/001-classic-symbols.md`; 254 paths, 25 subsystems, artifacts in `rea/` | **closed** |
| RE-Q15 | Reforged PDBs + debug client console (2.0.1 leak) | recorded in §2.5; user-side acquisition only, nothing enters the repo | documented |
| RE-Q17 | Reforged path → EKey resolution | `casc-storage` is FDID-oriented; WC3 needs the VFS/FDID mapping — solve before wiring `pcx-assets` | open, gates M1c |
| RE-Q16 | `war3_loader.dll` is Aegis-obfuscated (no CodeView symbol ref) | REA on the local 3.0.0.24268 build; deobfuscate before deep analysis | open, gates all Reforged code RE |
| RE-Q13 | Name normalisation before hashing | **closed** — uppercased ASCII, `\`/`/` unified; pinned by `hashing_is_case_insensitive` and `separators_are_normalised` | |

## Fidelity log

Records divergences discovered during differential testing against the original.
Format: `date — subsystem — symptom — root cause — status`

| date | subsystem | symptom | root cause | status |
|---|---|---|---|---|
| 2026-10-12 | Map format | the map container was assumed to be `HM3W` + a chunk table with a `TERRAIN` chunk | it is `HM3W` + `0x200` header + an **embedded MPQ**; terrain is `war3map.w3e` inside. `game.dll`'s writer stores the literal then opens an MPQ and never returns to a chunk table | **fixed** — measured 133/133; `PLAN.md` §3.11 now states the real surface |
| 2026-10-12 | Terrain | the height ramp was assumed to exist, mapping interior heightmap bytes through corner heights | there is **no interior heightmap**: the corner table consumes the file exactly, `corner_size == 1`, and heights are raw 14-bit indices whose shipped range cannot supply a mapping | **fixed** — ramp refused rather than guessed; `height_index` exposed with no world-unit claim |
| 2026-10-12 | Scope | "the campaign loads" was being treated as the fidelity bar | it is not: the community's own maps are the bar, and they need the full container plus both scripting paths | **fixed** — DotA / Athena / Footman Frenzy added as gates in `PLAN.md` §3.11, M4, M7 and §6 |
| 2026-10-12 | BLP | the TGA-style lesson recurred: `pcx-tga` pinned a footer split measured on 1.28c only | a build-specific measurement was asserted as a constant | **fixed** — pinned per install; both builds pass |
| 2026-10-11 | MPQ | Storm's live hash table at `+0x4C2E4` differed from the table the archives use; decrypting with it produced garbage tables | `FUN_15017650` builds the table lazily and the process copy is not the one the data was encrypted with | recorded, not blocking (RE-Q8) |
| 2026-10-11 | MPQ | First 2³² key sweep "proved" the archives unreadable | the sweep used the runtime table, not the fixed table; the model was wrong, not the data | superseded by the closed RE-Q1..Q4 |
| 2026-10-12 | MDX / render | 521 models and 3,121 geosets were drawn with the wrong texture; the Footman's blood geoset took the empty-texture material and a body piece took `gutz.blp` | `to_bevy` paired geosets to materials by index cycling because `materialId` was thought not to be on disk. It is: the first `u32` after the `MATS` body, swallowed by the scan-to-next-known-tag reader that decoded `MATS` | **fixed** — measured on 8,770/8,770 geosets of 3,249 models, then applied: `to_bevy` binds `paths[material_id]`, and the capture changed by 11,704 px (11.4%) in the lower body only |
| 2026-10-12 | MDX | `CAMS` and `LITE` decoded without error through the shared `nodes()` reader, with plausible-looking ids | a camera has no node header at all (its eye position sits where the ids go) and a light opens with two size words. 30 of 656 camera records produced a `parent_id` indistinguishable from a node index | both moved to their own readers, and the shared misread is pinned by a test so it cannot come back |
| 2026-10-12 | Sound | the sound lists were assumed to be a binary Miles "sound list" container (crate `pcx-slq`) | there is no such container and no `.slq` file in either archive. `UI\SoundInfo\*.slk` is plain ASCII SYLK, the dialect `Units\UnitData.slk` already used — confirmed by `ID;PWXL;N;E` at `.rdata+0x95494e` in vanilla `game.dll` | **closed** — 59/59 and 65/65 decode; `X`/`Y` are sticky state shared by `C` and `F`, which is what `DialogSounds.slk`'s 14,634 `Y`-less `C` records prove |
| 2026-10-12 | MDX | `PRE2` and `CLID` recorded as "not decoded" because their words had no agreed meaning | framing and semantics were conflated: both chunks' byte arithmetic is decidable without knowing what the fields mean | **closed as framing** — 7,200/7,200 `PRE2` and 1,678/1,678 `CLID` records; the 171-byte prelude and the collision floats stay refused on purpose |
| 2026-10-12 | TGA | the 1.29b TGA sweep failed a footer-split assertion | `pcx-tga` pinned `(381, 2)`, the 1.28c measurement; 1.29b gives `(433, 2)`. 435/435 decode with 0 failed | **fixed** — split is per install, now pinned per build; both builds pass |
| 2026-10-13 | Object data (`w3u`/`w3t`) | the community format notes describe object-edit string values as **length-prefixed**, the way a DSP or WTS-style record would be | the bytes refute it: all **1,937** string values are NUL-terminated with the end word immediately after the NUL, and the file is consumed byte-exactly on **92/92** files. A leading length word would leave bytes over on every file | **fixed** — NUL-terminated, no length prefix; framing is now an arithmetic identity |
| 2026-10-13 | Object data | five of the seven `W3?` extensions (`w3a`/`w3b`/`w3d`/`w3h`/`w3q`) were listed as "one container family, seven uses" and treated as a decode waiting to happen | they **ship in no map on either build** — 0 files, proven by a census over every map's embedded archive, not assumed from the extension list. Ability records may or may not carry the level/variation columns the notes ascribe to them; no shipped byte says | **fixed** — `parse` returns `Error::UnmeasuredExtension` instead of running a w3u-shaped walk; M4's gate no longer claims all seven |
| 2026-10-13 | Trigger strings | the `war3map.wts` header was written `STRING:<id>` with a colon and the body read as a quoted string terminated by a blank line | **`wts` uses `STRING <id>` — a space** — in 511,490/511,490 records across 913 files, and the body is delimited by brace lines and is arbitrary bytes (quotes, braces and `CR LF` inside bodies are payload, not escapes). The `pcx-objects` container is a different family entirely and shares none of this | **fixed** — space header, brace-delimited raw bodies, BOM recorded, ids unique but not monotonic across the classic editor's three-digit wrap |
| 2026-10-13 | JASS | a real map's script was lexed as whitespace-only, and every diagnostic said line 1 | two genuine bugs: **bare-CR line endings** were not treated as line breaks (`Maul/Anime Maul Beta v0.952(Protect).w3x`'s `war3map.j` is CR-only and the game accepts it), and **`SourceMap` built no line-start table**, so `line_col` returned `(1, c)` for every offset | **fixed** — `\n`, bare `\r` and `\r\n` each end exactly one line, and `line_col` binary-searches a prefix table. Both are pinned by tests, and the CR case is also a round-trip case on 1,326 real scripts |
| 2026-10-13 | Bevy 0.20 migration | the UI compiled against 0.17 and silently lost behaviour on 0.20 | four API changes, none of them a compile error in the shape we had: `BorderRadius` stopped being a standalone Component and became a `Node` field; `bevy::ui::Button` became a deprecated alias for a crate-private unit struct that cannot be spawned; click handling moved from polling `&Interaction` to querying `(With<Button>, With<Pressed>)` while keeping while-held semantics and the `InteractionDisabled` + `Pick::Download` double-guard; and on wasm `AssetSourceBuilder::default().with_reader(..)` became `AssetSourceBuilder::new(reader)` | **fixed** — and verified by pixels, not by compilation: the offscreen capture is 46,006 bytes / 18.7% lit / 100% opaque / 1,339 distinct colours, silhouette head→torso→legs plus the terrain tile intact |
| 2026-10-13 | Map container | the tracker and plan both named the minimap pair `war3map.map` + `war3map.ter` | neither name exists — 0 of 133 shipped maps, 0 of 200 loose community maps, no `(listfile)`, in neither build's name tables. What the tracker describes by function ships as `war3mapMap.blp` (BLP1 JPEG, 256×256) and `war3map.mmp` | **fixed** — measured names decoded on 132/132 both builds; a map really carrying the literal names is refused rather than parsed against an invented layout |
| 2026-10-13 | Object data | 285 of 5,975 `w3t`/`w3u` modification entries carry a non-zero **end word** | 0 in 5,690; the 285 exceptions are 106 that repeat their record's own original id (`DemoCampaign.w3n`'s `war3campaign.w3u`) and **179 that sit next to an `umdl`/`uico` art path with what reads like the owning unit's rawcode** — `"hcth"` beside `units\human\TheCaptain\TheCaptain` | **recorded as a hypothesis, not a proof** — the word is handed out as bytes. Two hand-checked samples do not prove an art-owner relation |
## Multiplayer

- [x] `pcx-sim`: deterministic fixed-tick sim, fixed-point, no Bevy — 8 tests
- [x] `pcx-net`: transport trait, wire codec, authoritative lockstep server,
      client prediction + reconciliation — 26 tests
- [x] Enforced by test: input horizon, no rewind, no cross-player commands,
      missing tick stalls rather than diverging, server/world round trips
- [ ] Authoritative lockstep server owning the world
- [x] UDP transport + LAN discovery (broadcast request/response, host advert)
- [x] `LanSession`: lobby states, ordered inbox, tick gated on input arrival
- [ ] Gate: a real 1.29b LAN client joins a game we host
- [ ] Classic Battle.net client through an emulator realm
- [x] Own low-latency protocol (lockstep + prediction/**rollback**)
      — **rollback is implemented**: `Client` keeps a bounded `TickHistory` ring of
      `(state-before-tick, Rng)` plus its own unacknowledged inputs and the server's
      acknowledged tick. On a correction for tick T the **server's** world at T becomes
      the base and the client's inputs `T..head` are replayed through `pcx_sim::step`, so
      the result is "what the server will compute", never "what the client asserts".
      Window and re-simulation budget are both 30 ticks (750 ms at 25 ms); exceeding them
      falls back to the old snap and says which of `OutsideWindow` / `ResimBudget` /
      `Unconfirmed` / `Stale` caused it. A stale correction behind accepted authority is
      ignored outright, so a reordered packet cannot rewind the client. Proven by a
      no-op-correction test that must reproduce the undisturbed prediction exactly, plus
      a lost-input divergence test that ends equal to a genuine authoritative server run.
      Frames on the replayed path are overwritten and everything past the new head is
      truncated, so a later rollback cannot resurrect an abandoned trajectory.
- [x] Rule enforced everywhere: **server holds state, client only proposes** — the rollback
      base is always a state the server sent; replay only re-applies the client's *intent*

## WASM build

- [ ] Browser-only ISO import: local File/Blob reads, filesystem discovery,
      validated Warcraft III archive selection and in-memory asset loading.
      **No server upload or remote extraction.** Track unsupported repacked
      installers explicitly; bounded memory, progress/cancel, and a network
      capture proving content stays local are acceptance checks (PLAN M11).
- [ ] Original menu feel/sound: animated background, correct button states and
      original music/clicks, verified in the running browser. Local audio files
      are extracted and playback wired; complete visual fidelity remains open.
- [x] `pcx-sim` + `pcx-net` build for `wasm32-unknown-unknown` (transport cfg'd out)
- [x] **Full workspace builds** for `wasm32-unknown-unknown` — both decoders,
      every binary, the launcher. `cargo build --target wasm32-unknown-unknown
      --workspace` links clean.
- [x] `pcx-mpq` reader works from an in-memory buffer (`from_bytes`, same parser)
- [x] Browser entry point: `crates/peacecraft3/src/browser.rs` provides
      `#[wasm_bindgen(start)] fn start()`, which runs the Bevy app and *returns*
      (a browser has no process to `exit`). `wasm-bindgen` is a wasm32-only dep.
- [x] In-browser asset path: Bevy's default wasm asset source (HTTP reader rooted
      at `assets/`) replaced by Bevy's own `MemoryAssetReader`; archives read via
      `MpqArchive::from_bytes`.
- [x] JS side: `web/index.html` + `web/loader.js` + `web/pkg/`. No trunk, no
      wasm-pack, no bundler. Build is `cargo build --target wasm32-unknown-unknown
      --release -p peacecraft3` then `wasm-bindgen --target web --out-dir web/pkg`.
- [x] **Launcher runs in Chromium with WebGL**, verified at desktop and narrow
      sizes: original art/font loading, Options/Main menu navigation and settings
      scrolling pass. Actual music and canvas-click playback also pass. Full
      game playback and exact menu visual fidelity are still open.
- [ ] `pcx-gfx` cache is async — no synchronous file I/O on a shared path
      *(the only §3.8 constraint still unimplemented rather than impossible)*
- [x] **Dev artifact measured: 975,009,181 bytes, 913,000,035 (93.6%) of it DWARF
      + `name`**, only 51,609,943 `code` + 8,629,367 `data`.
- [x] **Debuginfo-stripped artifact measured: 119,794,662 bytes**
      (`web/pkg/peacecraft3_bg.wasm`), zero DWARF. Still 62,950,809 of `name`
      section (symbol names, not code); `code` 47,289,068, `data` 8,629,367.
      Same lesson: measure the release path, never the dev artifact.
- [ ] `wasm-strip` / `-C strip=symbols` on that 63 MB `name` section — unmeasured.
- [x] Three browser limitations documented rather than fixed: **staging** (no
      destination to copy into → every source reported rejected), **install-level
      version detection** (one archive can be opened and counted, but not
      identified as 1.28c or 1.29b), **rendering without a canvas**.
- [ ] `crates/peacecraft3/README.md` — `web/loader.js` points at it for the build
      recipe and it does not exist yet.

## Desktop builds

- [x] Four-target native CI matrix and packaging helper added; executable
      header validation covers both Windows PE and macOS Mach-O architectures.
- [ ] Windows x86-64 compiled, tested and artifact verified.
- [ ] Windows ARM64 compiled, tested and artifact verified.
- [ ] macOS Intel x86-64 compiled, tested and artifact verified.
- [ ] macOS Apple Silicon ARM64 compiled, tested and artifact verified.
- [ ] Menu GPU rendering and audio verified on both desktop operating systems.
      First run `38409132391` compiled Windows ARM64 and macOS ARM64, but their
      tests failed because a local-install test assumed developer folders.
      That test is now explicitly ignored; corrected artifacts remain pending.
      Commands and limitations are recorded in `docs/build-desktop.md`.
