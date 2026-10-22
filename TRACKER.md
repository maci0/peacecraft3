# PeaceCraft3 — Tracker

Legend: `[ ]` todo · `[~]` in progress · `[x]` done · `[!]` blocked · `[?]` open unknown

- [x] Bind headless session admission to actual selected catalog/Misc contents.
  The readiness receipt keys every admitted catalog table/profile and the three
  stock Misc tables (`Units/MiscData.txt`, `Units/MiscGame.txt`,
  `UI/MiscData.txt`) by normalized member path and SHA-256 (or explicit absence),
  so relocated equal archives share identity and changed bytes at one path change
  it or refuse. Executable, map, scripts, seed, mode, roster and explicit profile
  remain distinct inputs. Core suite, strict Clippy, WASM lib, selected Human01
  identity/startup gates pass. Other selected source reads (sound/label banks,
  UI localization) are outside this catalog/Misc receipt; native lockstep remains
  a separate open gate.

- [~] Global `BlzGetAbilityCooldown` reads admitted source/map-edited durations,
  independently of learned units and live overrides. Core/app, selected-install,
  strict Clippy and affected WASM checks pass. Native level-zero indexing remains
  unqualified and refuses explicitly.

- [~] Per-unit cooldown duration get/set is connected to the two implemented
  spell classes, preserving active deadlines and recreation unit-cache state.
  Selected-client wrappers use one-based levels. Core/app suites, selected
  Human01 rank checks, strict Clippy and affected WASM checks pass; other classes
  and native timing/cache parity remain open.
- [~] `UnitResetCooldown` resets currently modeled spell and unit/item-group
  deadlines without refunds or duration changes; independent holder state and
  removal cleanup pass core/app, selected Human01, Clippy and WASM checks. Native item group
  propagation/transfer timing and other ability classes remain open.

### 2026-10-22: Learned command buttons

- [~] Standard Move, Stop, Hold, Patrol and Attack now share the receipt-bound
  command grid and player-order path. Profiles supply skin keys, positions,
  hotkeys and tooltips; group commands filter current local ownership. Only
  accepted native orders enter the journal. Unit-target Move and friendly-unit
  right-click now retain follow intent using admitted Misc ranges. Patrol imitation,
  readiness/cursor art, exact mixed-selection policy and native rendering/input
  comparison remain open. Validation receipts: `.scratch/standard-commands/`.
- [~] Holy Light and Water Elemental have a receipt-bound command-card renderer
  and click consumer. Geometry and texture addressing are derived from the
  selected client; icons come from the learned ability's source profile. Session
  replacement releases owned assets, and button input rechecks current selection
  and control. Synthetic pointer/session tests, selected-client replacement and
  teardown, and the explicit Human01 input gate pass. Rendered comparison,
  tooltips, readiness/disabled art and remaining command types stay open.
- [x] Selected-original command icons decode through bounded source reads and
  reuse their cached handles/receipts. Native component captures of each spell
  at 1280×960 and 640×480 show proportional placement and scaling. This is a
  synthetic parent/selection fixture; full mounted-console and original-client
  rendering comparison remain open.
- [x] An explicit unchanged-Human01 gate mounts the command card beneath its
  actual admitted console, preserves button entities across stable updates,
  rejects a lookalike session with old bindings and remounts after coherent
  publication. The gate exposed and now covers HUD source-policy invalidation
  on session replacement. Full campaign rendering and native comparison remain
  separate requirements.

### 2026-10-21: Ability profile overrides

- [x] Live hero ability profiles and startup's Holy Light model read apply the
  same scalar map modifications through AbilityMetaData. Empty overrides do not
  fall back to stock; malformed, unknown, ranked and indexed forms refuse.
  Holy Light input reports lookup failures. Core tests pass (1,082/0 failed/30
  ignored), the selected command-layout gate passes, original Human01 chapter
  startup still publishes, and app input tests, strict Clippy and WASM pass.
- [~] The source-derived command grid is preserved; ranked tooltip expansion
  remains unfinished.
  Learned button presentation now derives Art/rank and independent Buttonpos
  components, including scalar map replacements. Synthetic tests and the
  explicit two-spell selected-install/grid comparison pass. The later button
  checkpoint adds mounting and sampler admission; tooltips and rendered behavior
  remain unfinished.

### 2026-10-21: Native chapter activation

- [~] Selected-install chapter activation now has an explicit ignored application
  gate: actual campaign index/button activation, configuration, admission, normal
  input preparation and script startup publish one context and 152 Human01 units
  on the selected Legacy RoC input with seed 1. Strict app Clippy passes. This is
  not rendered pointer delivery or mission progression; both remain unverified.

### 2026-10-21: F7/F8 diagnostics

- [x] F8 adds bounded logarithmic-age timing history beside FPS and milliseconds:
  192 peak buckets, up to 2,016 frames, full-resolution live edge and per-frame redraw.
  Compression checks: 62 graphics tests and 466 app tests pass (41 ignored), strict
  Clippy and WASM checks pass; native wide/narrow component captures inspected.
  F7 independently shows display/backend, mesh-asset vertices, GPU allocator
  and available whole-GPU VRAM counters, plus campaign tick/speed/seed/selection
  and error excerpts. Panels share a click-through camera and stop sampling
  while hidden. Native component renders pass at wide/narrow sizes. Unsupported memory
  counters report unavailable; full campaign/browser rendering stays separate.

### 2026-10-21: CPU hotpath evidence

- [~] Native launcher GPU diagnostics and a RADV capture are verified on the
  RX 7900 XTX; RGP opens and displays the capture. The Xvfb application attempt
  is invalid (no DRI3). Chapter execution was unobserved, so rendered campaign
  GPU coverage remains open. See [GPU evidence](docs/performance/gpu-launcher.md).
- [x] Huffman bit traversal reuses a bounded byte reservoir. Synthetic read and
  truncation comparisons, 76 codec tests, strict Clippy and WASM pass; 2,716
  retained Storm WAV dumps remain byte-identical. Human03 keeps its unit/RNG
  projection; hardware instruction reductions remain report-only.
- [x] Optional core spans separate startup, asset decoding, scripting, motion,
  combat and animation work. Bounded trace summaries subtract child spans.
  Real Human03 stacks captured 1,897 samples with zero lost samples; five runs
  preserve unit projections and source RNG state. Core tests (1,430), strict
  app Clippy and affected WASM checks pass.
- [~] Complete rendered campaign coverage remains open. Four hidden-window
  capture attempts are invalid; none supplies frame or GPU evidence.
  See [CPU hotpaths](docs/performance/cpu-hotpaths.md).
- [x] Derive catalog collision geometry once per created unit, preserving deferred
  validation errors. Human03 metadata walks fall from 295,290 to 148 with the
  same unit/RNG projection; a repeated-scan mutant fails the work-count gate.
  Core tests, actor tests, strict application Clippy and WASM checks pass.
- [x] Combat acquisition scans a per-step unit list and computes candidate
  distance once, preserving handle order, initial attacker eligibility and live
  target checks. New tie/death regressions and 1,073 core tests pass; Human03
  keeps its unit/RNG projection and native-call name sequence. Instruction
  counters are report-only; full rendered coverage stays open.
- [x] `sudo perf` attaches to the normal-user Wayland viewer (4,494 samples).
  `bpftrace` DWARF CPU flame graphs capture 1,171 unchanged-map samples, split
  into startup/simulation/other; 24 repetitions retain the unit/RNG projection.
  Unresolved runtime addresses, external helper warnings and the sparse viewer
  BPF sample are recorded; full rendered campaign/GPU coverage remains open.

### 2026-10-21: Five measured optimization rounds

- [x] Sorted simulation commands allocate zero times over the unchanged fixed
  workload. Asset paths allocate once; skeleton traversal scratch is reused.
- [x] Disconnected blocked-goal pockets are proven unreachable within a charged
  bound; the synthetic route drops from 4,647 to 648 work with the same endpoint.
- [x] Identical skinned positions emit no mesh modification event. Native Vulkan
  before/after captures retain draw counts; timings remain report-only.
- [x] Ctrl+Shift+F9 selects 1×/2×/4×/8× campaign pacing with unchanged fixed ticks.
  Timer/RNG gates and the complete 131-unit Human03 snapshot agree across rates.
  Source-bound RNG state and 64 subsequent draws agree too.
- [x] Affected core/app tests, strict Clippy, WASM and the CPU-busy allocation/state
  gate pass. Whole-game original-client lockstep and full rendered campaign
  performance remain unqualified. See [measurements](docs/performance/five-rounds.md).

### 2026-10-21: Water Elemental in progress

- [x] Source-ranked summoning, cast point, mana/cooldown, unit factory, timed death,
  summon responses and primary-unit hotkey are wired in the private candidate.
  Nine focused synthetic tests pass, including map overrides and parsed model
  animation completion. The play suite passes 1,190 tests (47 ignored); strict
  native app/play Clippy, app WASM, six app input tests and four selected-install gates pass.
- [x] The Area placement follow-up derives the forward distance per rank. The
  selected native loader/getter/effect chain establishes the Area relationship;
  synthetic cardinal and rank tests pass. Suppressing the offset fails the new
  regression. All affected checks above were repeated on the follow-up.
- [ ] Native collision adjustment, event ordering, animation blending and
  lifetime pause semantics remain unqualified. The command-panel button is not drawn.
  Buff/status-bar/dispel integration also remains open, tracked under
  `96c729e7-8dac-4f49-b87d-dc3dd4e1ca3a`.
- [~] Unchanged Human03 creates a source-loaded `hwat` at tick 999. With Area
  placement and no injected player movement, the longer diagnostic reaches
  movement work exhaustion for unit 780 at tick 2570 before the profiling fix.
  The bounded disconnected-pocket proof now allows the same diagnostic to reach
  tick 4096 / 64 game seconds without injected player orders. The v1 diagnostic stopped
  earlier but also injected a movement order, so this is not a controlled speedup
  comparison. This remains partial startup observation, not campaign acceptance.
  See [summoning](docs/re-notes/water-elemental.md).

Last updated: 2026-10-15

Historical menu verification (2026-10-14; current increment below): WASM release build succeeds; the app
has 99 passing tests and five explicitly ignored local-fixture tests. PeaceCraft
settings precede the skippable original cinematic and original menu. Edition is
RoC on the left and TFT on the right, with TFT selected by default; 1.28 is absent
from settings. Original font, logo alpha and parsed 500 ms logo entrance are
integrated. Relative layout follows native 0.8 × 0.6 frame conversion and its
widescreen branch; the 3D background fills the window independently.

Browser checks exercise resizing, click/music playback and real cinematic
playback. UI-only resize/click cases pass at 320×240, 640×480, 1280×1024,
1280×720, 1920×1080, 2560×1080, 3840×2160 and 430×932. Browser-native 2×
DPI checks pass at 1280×720 and 430×932; DPR emulation alone reported an
inconsistent physical pixel box and is not used as the DPI oracle. The TFT scene selects its original 18 geosets and 49 textures and
honours authored wrapping. The normal blue inner fill now uses decoded original
texels and FDF-derived bounds; browser captures confirm the fill extent before
and after resize. The largest edge discrepancy against continuous coordinates
is 1.133 pixels, so exact pixel alignment is not certified.
Full fidelity remains open: the grey hanging panel and inner button edges are
not rendered, and lighting, particles,
texture animation, material blending and original options/gameplay remain
unfinished. Parser and screenshot success do not establish native behavior.

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

## Current recreation work (2026-10-16)

### Human01 runtime frontier (2026-10-20)

- [~] Unchanged Human01 publishes through the normal application path and runs
      1800 game seconds under `CampaignRuntime` without a script refusal (no
      player input yet). Added script threads with `TriggerSleepAction`, event
      delivery (regions, deaths, attacks), combat, time of day, live camera ramps,
      client software `SquareRoot`/`Atan2`, selection, effects, vitals and about
      thirty further natives. Application consumers (camera, input, HUD text,
      effects, audio) remain open; a synthetic software-math test is missing.
      See `docs/re-notes/campaign-human01-runtime.md`.

### Campaign and gameplay implementation queue

Each slice below needs a reviewed implementation, real tests and application
integration. A completed prototype does not close its broader milestone.

| Slice | Concrete next acceptance gate | Depends on |
| --- | --- | --- |
| Campaign startup | Selected original chapter executes `config`/`main` through the first live objective; unsupported calls retain source diagnostics | Map admission, composed gameplay Host |
| In-game HUD | Authored FDF relationships reach viewport-relative HUD components with checked resource bindings | Admitted map and selected UI assets |
| Terrain and camera | Selected map terrain, pathing and authored camera initialize one coherent scene | Map admission, terrain/material resources |
| Selection and orders | Selected owned units send supported orders to the existing simulation without losing command batches | World/entity identity and gameplay input |
| Transmissions and dialogs | Script calls create bounded visible state and correctly route skip/dialog events | Gameplay Host, source-derived UI |
| Quests and victory | Quest/quest-item handles preserve state and drive explicit objective/victory events | Gameplay Host and ordered callbacks |
| Campaign progression | Authored chapter identity and completion select the correct next chapter without enabling TFT on base-game inputs | Campaign metadata and outcome events |
| Combat data consumer | Required authored stat fields reach an existing entity consumer with explicit conversions/refusals | Object catalog, world and orders |
| AI startup | Selected campaign AI entrypoint executes one supported behavior with the next missing interface identified | Script ownership, world and gameplay Host |
| Remaining-work reconciliation | Every open/partial checkbox maps to a concrete acceptance gate and dependency, with stale claims corrected | Current source and retained verification |


- [x] Shared BLP JPEG color conversion now respects BGR component order. Both
      retained logo alpha planes are unchanged; fixed public-decoder colors
      agree with an independent decoder within one intensity level on opaque
      pixels. Synthetic tests and strict native/WASM Clippy pass. This corrects
      cyan lettering without an artwork-specific tint. See
      `docs/re-notes/blp-jpeg-channel-order.md`. The corrected WASM is deployed
      in the asset-free public demo at `1380001`.
- [x] Local original-game reference harness: private Docker/Wine/Xvfb runs,
      hashed installation copies, source/settings/window receipts, offline CLI
      schema and equal-extent comparison reports. Ten strict harness tests pass.
      Five 96-DPI runs cover 1024×768, 1280×960, 1920×1080, 2560×1440 and
      3840×2160; all 15 reviewed Main/Options/Video captures match their PNG,
      display and Warcraft client extents. Originals and captures stay local.
      This is a Wine menu reference; movies, synchronized recreation comparison
      and complete pixel fidelity remain open. See
      `docs/re-notes/original-game-vm.md`.
- [~] Paired original/browser UI replay sends one checked physical-input journal
      to both isolated targets, preserves unchanged checkpoint PNGs and reports
      exact RGB differences. Twenty-one strict harness tests and an actual
      browser adapter check pass. Four checkpoints complete at 1024×768,
      1280×960, 1920×1080 and 2560×1440; the historical 4K browser timeout
      is retained as failed. The current three-application matrix below captures
      4K but exposes a navigation failure. Before/after snapshots expose asynchronous
      UI changes. RNG/clock synchronization and pixel fidelity remain open;
      non-null seeded comparison refuses rather than inventing a seed hook.
      See `docs/re-notes/original-game-vm.md`.
- [~] Original Options and adjacent FDF screens: source-derived controls/art,
      draft/Cancel/Apply, runtime display choices, local preferences and user
      audio gains are implemented. Bottom-left cog separates PeaceCraft settings.
      Submenu navigation is started; unsupported game settings and gameplay
      actions remain explicit. Browser checks cover apply/cancel/persistence,
      resolution reload/resize, readable narrow-window Actual size, Single Player,
      Campaign and LAN. Full run 12 retains a final cog-caption OCR failure;
      separate visual review and real cog click-through pass. Strict native/WASM
      Clippy and 2 library + 170 app + 74 integration tests pass. See
      `docs/re-notes/original-options-workflow.md` for exact build/test scopes;
      complete original-client appearance and gameplay remain open.
- [~] Original Options defaults: checked source-profile import admits all 80
      reviewed Legacy preference descriptors, preserving raw fallback strings,
      including empty values. No measured defaults or display presets are copied
      into production constants. Available device modes retain depth/refresh in
      the original enumeration evidence; current menu choices still represent
      render sizes. Native music/effects defaults now feed the active gains and
      matching controls before playback, with saved-preference precedence and
      failed audio-candidate rollback. Browser client/default transport, other
      setting owners and confirmed exclusive-mode application remain open. See
      `docs/re-notes/original-options-defaults.md`.
      Subsequent native population checks connect every retained control to its
      preference reads and recover ResolutionMenu labels, filtering and matching.
      Fifteen checked label-adapter and 17 selection-model cases pass; prototypes
      remain unapplied. The conditional provider/OS bridge is now checked,
      including source-table/derived additions, stable grouping and refresh
      replacement. Root replays pass 12 provider, six bridge and 32 augmentation
      checks; runtime kind/adapter inputs and effective first-run selection
      remain open. Texture quality's distinct mip/depth preference writes are
      checked, but their renderer consumer and live replacement remain open.
      Selected BLP mip decoding now refuses invalid/absent levels before image
      allocation while retaining raw absent-slot inspection. This decoder guard
      does not enable Texture Quality or establish its color-depth behavior.
      See `docs/re-notes/original-options-initialization.md` and
      `docs/re-notes/original-display-modes.md`.
      Shared popup parsing now refuses malformed items without shifting indexes;
      259 app tests, both selected-original popup gates, strict native/WASM checks
      and the actual release-WASM runtime checks pass. This does not enable
      unimplemented quality or gameplay controls.
      Disabled settings now have source-qualified read-only previews: the
      explicit selected-original client/FDF gate checks all 25 bindings, with
      23 admitted projections and two empty dynamic-menu refusals. Unavailable
      slider state is cleared, while supported audio/resolution owners retain
      Apply/Cancel and saved-preference behavior. Main-only cog scope includes
      pointer, keyboard focus and accessibility. Current native app suite:
      409 passes/15 explicit ignores; strict native/WASM app checks and release
      build pass. Fresh local browser checks 32 disabled input cases across
      all three tabs before/after saving, active audio at both viewport origins,
      fresh Main/Single Player/Main and restored cog click-through. Twelve
      reviewed captures, 15 local code GETs, zero uploads/sockets/page errors.
      Browser archive-only sources have no admitted client fallback input;
      previews remain unavailable there. Other setting effects and complete
      original first-run/mode fidelity remain open. See the fallback note above.
      Portable resolution Apply now waits for the matching Winit client report
      before saving, preserves the prior preferences/audio on save refusal and
      confirms restoration before reporting it. Cancel and a retryable dirty
      draft are retained; a post-save audio warning is a committed result.
      First observation also replaces the startup-size seed without losing an
      edited draft. The real-window gate, 270 app tests, strict native/WASM checks
      and five reviewed release-browser captures pass. The browser exercises first
      and later failed saves, parent-following restoration and a post-save audio
      warning, with zero uploads, sockets or page errors. Physical exclusive mode,
      source-qualified first-run choices
      and remaining renderer/gameplay setting consumers stay open. See
      `docs/re-notes/original-options-resolution-apply.md` for checks and scope.
      Centered-viewport slider presses now use the UI camera's local coordinate
      space, explicit Options tab choices survive entry discovery, and generated
      decorative text spans no longer lose mouse activation when pressed labels
      move. Seven widget regressions and a production-text/picking regression
      pass; removing the span fix reproduces zero clicks/actions. The complete
      suite passes 282 app tests with 13 explicit ignores, four library and 84
      integration tests, with strict native/WASM checks. A fresh local packaged
      browser passes both source sliders at two viewport layouts, mouse Apply
      and Cancel preservation, with four reviewed captures and zero uploads,
      sockets or page errors. Disabling a pressed button now clears its temporary
      pressed state without activation, including deferred press insertion;
      re-enable permits a fresh click. Native/WASM checks cover that additional
      lifecycle fix. Event-local UI/sprite/window hit resolution now preserves
      fresh same-frame clicks and A-click/final-pointer-B targeting. Nineteen
      new app pipeline cases and a trusted browser batch check pass, alongside
      the source sliders, mouse Apply and Cancel. The complete suite passes
      301 app cases with 14 explicit ignores, four library and 84 integration
      cases; strict native/WASM/library/docs checks and release app build pass.
      The final 24-file browser package has five reviewed captures, 15 local
      code GETs and zero uploads, sockets or page errors. The visible cog overlap
      and disabled-setting source previews remain separate work. The
      earlier larger-view caption-disappearance claim is withdrawn: exact
      runtime-bounded image regions are unchanged between entry and Sound in
      both the old and fresh large-view runs. No rendering repair is claimed. See
      `docs/re-notes/options-slider-input.md`.
- [x] Corrected menu additive geometry to the retained original flare draw's
      source-alpha/One RGB equation (2026-10-19). The earlier ONE/ONE fidelity
      claim is withdrawn: that synthetic test proved a chosen equation, not
      the original. Removing the override makes zero texture/material alpha
      contribute no additive light, without changing authored opacity. The
      causal GPU regression fails before the fix and passes after it; all eight
      synthetic hardware render-graph cases pass. See
      `docs/re-notes/menu-additive-blending.md`.
- [x] RoC menu root-node model-space particles, STEP bursts and animated emission
      rates now retain the complete batch. Cross-pass priority checks no longer
      reject the source scene; opaque geometry precedes translucent sorting.
      Fifty native/actual-WASI CPU/quad cases and an explicit local two-loop
      RoC gate pass. Native edition-switch journals render through 4K. Original
      RNG/phase, fresh browser visuals and full parity remain open. See
      `docs/re-notes/menu-particles.md`.
- [x] Menu node transforms respect the selected source sequence. Other-sequence
      keys previously moved the RoC gradient plane into the sky, creating a flat
      cutoff. Fresh original WC3/Wine and corrected native captures at 1024×768
      and 3840×2160 confirm the skyline correction. All 116 model, 171 graphics
      and 411 menu binary tests, two actual WASI pose cases, strict native/browser
      checks and both builds pass. Exact three-target pixels remain open. See
      `docs/re-notes/menu-node-sequences.md` and local evidence
      `.scratch/menu-horizon-root-20261019/`.
- [x] Background full-billboard posing consumes source flags, hierarchy, pivots,
      tracks and the selected source camera (2026-10-19). Nested camera-facing
      joints cancel the parent turn once; skinning and lighting share the pose.
      Six synthetic regressions, 53 renderer tests, 57 decoder tests and both
      explicit 101-pose source gates pass. Unsupported transforms in affected
      families refuse; axis locks outside them remain on the existing path.
      Native matrix identity and particle-phase parity are not proved. See
      `docs/re-notes/menu-billboards.md`.
- [x] Non-line PRE2 emission now uses a circular cone from the selected client's
      separate latitude and full-circle azimuth paths (2026-10-19). Two bounded
      tilts previously let particles leave the authored cone. The regression
      fails before and passes after; 156 graphics tests, 654 application tests,
      21 actual WASI particle tests, two selected-TFT CPU/quad gates and strict
      native/WASM checks/builds pass. Fresh native/browser journals through 4K
      use calibrated native monitor modes; all sixty RGB comparisons against
      retained WC3/Wine frames still differ. Original PRNG/phase, quality scaling
      and complete matrix/trajectory equivalence remain open. See
      `docs/re-notes/menu-particles.md` and local evidence
      `.scratch/menu-particle-policy-root-20261019/`.
- [x] PRE2 gravity now follows the selected client's constant-acceleration
      position equation before updating velocity (2026-10-19). The causal
      trajectory regression fails before the correction and passes at two
      integration tick sizes afterward. All 157 graphics, 654 application and
      22 actual WASI particle tests, two selected-TFT CPU/quad gates and strict
      native/WASM checks/builds pass. Native 4K navigation passes; scheduled-entry
      WASM verifies rendering only. See `docs/re-notes/menu-particles.md` and
      `.scratch/menu-particle-gravity-root-20261019/`.
- [x] PRE2 tail width now normalizes in the camera plane, preserving width on
      oblique tails and tapering subunit projections (2026-10-19). Forty-one
      selected-client instruction checks reject a depth-denominator mutation;
      the regression fails before and passes after. All 158 graphics, 654 app
      and 37 actual WASI CPU/quad tests pass, alongside strict native/WASM
      checks/builds and both selected-TFT profile gates. Five native and five
      trusted browser journals pass through 4K. Forty new captures are reviewed
      with twenty retained originals; all sixty RGB comparisons still differ.
      Original RNG/initial ages, source settings, native Windows and complete
      rendered/simulation parity remain open. See `docs/re-notes/menu-particles.md`
      and `.scratch/menu-plane-transform-root-20261019/verification-final-1.json`.
- [~] The fresh trusted-click browser 4K entry attempt timed out before its
      journal began. An isolated scheduled-DOM-entry diagnostic completes the
      four views, but does not replace that failed workflow or establish its
      cause. Keep entry acknowledgment/load diagnosis and exact pixel/RNG parity
      open; no blind replay or raised timeout is recorded as a fix.
- [x] Browser refusal diagnostics retain bounded tails, omitted counts and the
      failing stage (2026-10-19). Large retained logs previously suppressed the
      structured failure response. The process regression fails before and passes
      after; all 51 Python harness and five browser-adapter tests pass. The same
      sealed application completes trusted 4K navigation in isolation, alongside
      native rendering, and with the tracked diagnostic worker. Sixteen fresh
      checkpoints are reviewed; all twelve three-target RGB comparisons still
      differ. The earlier timeout and a missed native operator-review deadline
      remain preserved. No entry-cause, exact-output or deterministic-parity claim.
      See `docs/re-notes/menu-particles.md` and local evidence
      `.scratch/browser-entry-ack-root-20261019/`.
- [~] The earlier composed alpha/billboard candidate passes 651 application tests,
      strict native/WASM checks/builds, five hardware source-background extents
      and ten fresh native/browser navigation runs through 3840×2160. Forty new
      captures are reviewed against twenty retained WC3/Wine captures; all
      sixty exact RGB comparisons still differ. No version pixels are excluded.
      Glow/rays, panel appearance, source settings, glyphs, cutout/culling,
      native Windows and deterministic original/native/WASM lockstep remain
      open. Public site unchanged. Local receipts and report:
      `.scratch/menu-alpha-root-20261019/audit-final-2.json`.
- [~] Local imports follow skin-selected hanging-panel model/TEXS edges.
      Known unsupported panel audio codecs now preserve admitted graphics and
      other playable sounds. Qualified Legacy panel spawn and a six-capture local
      original-fixture Main/Options/Single Player/return browser run pass, with
      no uploads, sockets or page errors. Main/Options and Main/Single Player
      now use separately admitted source phase clocks and settle in the current
      six-capture browser run. Unavailable Huffman audio and full menu/sky
      fidelity remain open. See
      `docs/re-notes/menu-panel-navigation.md`.
- [x] Background camera omits the extra Bevy filmic/deband transform. Synthetic
      non-HDR draws pass 416 source-color checks on each of software and Radeon
      Vulkan; old-policy controls detect changed RGB. Native/WASM checks and six
      reviewed local original-fixture browser captures pass. The fog veil remains
      visible and native gamma/framebuffer parity is open. See
      `docs/re-notes/menu-display-transfer.md`.

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
- [x] `pcx-mpq`: ADPCM mono/stereo and Huffman decode, derived from the supplied
      compatibility Storm.dll; synthetic sectors and original WAV bytes match
      its native decoder. Huffman weights require a caller-supplied local PE
      source; Legacy has no co-located DLL, and browser import wiring remains open.
      This is not target-native 1.29b codec parity; see `format-mpq.md`.
- [x] `pcx-mpq`: bounded standalone sparse (`0x20`) and sparse-plus-ZLIB (`0x22`)
      archive admission. Six encrypted/public consumer groups pass native and
      actual WASM; intermediate arithmetic/layout bounds are checked before
      allocation. A selected-primary differential records 29 matching roundtrips
      and two shared refusals across 31 synthetic vectors; no native parity claim.
- [ ] `pcx-mpq`: other compression chains and original sparse-member acceptance
      The standalone sparse reader is exposed and passes 13 checked public-API
      tests, a 172-case synthetic primary-source differential, strict Clippy
      and a warning-denied WASM check. Other sparse combinations, BZIP2/LZMA and
      target-native codec acceptance remain open; see `mpq-sparse.md`.
- [~] `pcx-mpq`: archive priority resolution (`DEFAULT_PRIORITY` exists but is still assumed — see M0 open)
- [x] `mpqxtract` list/show/get with glob patterns and bounded output paths
- [x] Test: `(listfile)` decodes end to end — 529 298 bytes of imploded sectors → real paths
- [x] Test: known-answer block entry for `war3.mpq` block 0
- [x] Test: live hash entries cover every existing block, in all 4 archives
- [x] Golden test: byte-identical extraction vs supplied Storm under Wine;
      10,679 compatibility archive files and 227 patch candidates match.
      Patch coverage is candidate-based because it has no `(listfile)`;
      unlisted blocks and target-native 1.29b parity remain open.
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
- [x] `pcx-play`: checked `.rpl` ingestion, source-line errors, explicit expected
       unit/timer/RNG state and 100 independent canonical runs. Map/world and
       native-gameplay integration remain open.
- [~] `pcx-play`: declaration-ordered actions enter existing simulation steps;
       move/select/stop/nop and harness RNG are supported. Attack/build/cast/train
       and native event-order parity remain open.
- [x] `pcx-diff`: bounded seed minimiser with fresh paired RPL executions,
       independent inspection/oracle ceilings, canonical trace/identity refusal
       and checked-corpus minimum. Library and CLI regressions, strict Clippy and
       native/WASM builds pass. Native baseline capture and Tier C corpus/fidelity
       gates remain open; see `docs/playtest-minimization.md`.
- [x] `playrun` CLI and `tools/playrun.sh`: assertions, repeats, explicit work
       limit and optional baseline comparison. Supplied baselines are not
       automatically original-runtime evidence.
- [x] Trace capacity is checked before allocation, including platform vector
       layout and fallible reservation. Three native and three actual 32-bit
       release-WASM regressions pass; invalid runs preserve existing CLI output.
- [ ] One scenario per Rosetta campaign chapter; assert specific outcomes, not "no crash"
- [~] Gate: one trace byte-identical across 100 consecutive runs, and `unchanged` vs baseline. Synthetic harness and RPL self-repeats do not meet that original-game baseline.
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
- [~] `pcx-casc`: local EKey spans, BLTE N/Z chunks and ENCODING CKey joins
      yield verified bytes. The original-install gate checks 74,500 index entries
      and 112 configured manifest aliases; unsupported codecs/ESpec remain explicit.
- [x] `pcx-casc`: resolve explicitly stored TVFS paths through configured mounts
      to full EKey/CKey identities. Verified named Footman content and two manifest
      links; no shorthand, module/HD/locale precedence or native lookup claim.
- [~] `pcx-assets`: caller-ordered source layers and real `override/` resolution
      retain provenance and produce deterministic indexes. Concrete MPQ/CASC
      adapters read explicitly mapped members and retain their identities;
      application mode selection and exact-target archive priority remain open.
- [~] Asset modes: orthogonal build/addon/asset/behaviour axes exist (`vanilla`/`hd`/`reforged` assets and `fixes` behaviour). The literal set `vanilla` / `reforged` / `reforged-hd` / `reforged-fixes` is not that grammar, and application selection remains open.
- [x] Library `override/` drop-in tree + generated `index.json`; app wiring pending
- [~] Gate: Reforged Footman geometry loads through the same `pcx-gfx` path as vanilla.
      The checked version-1800 LoD trailer yields five geosets and 582 vertices
      from the verified 105,602-byte CASC asset. Bevy mesh construction passes;
      explicit stored material parsing also passes. Shader semantics, TANG/SKIN
      and actual rendering remain unsupported/unverified.
      Stored-only v1800 TANG/SKIN readers now retain the HD Footman's 40,142
      records and wide joint/weight words; strict mesh import still refuses
      weighted HD. Borrowed BPOS rows and an explicit primary-convention
      SKIN → MATS → BONE-object-ID/BPOS join now pass ten synthetic tests and
      keyed Altar/Footman fixtures. Every integer lane remains unchanged;
      native joint selection, pose interpretation, conversion and GPU behavior remain open.
      Raw FAFX records now expose borrowed 80-byte names and 260-byte paths with
      exact 340-byte tiling and source offsets. Eight synthetic tests, the keyed
      HD Footman fixture and WASM check pass; FaceFX behavior is not implemented.
- [x] `pcx-gfx`: explicit budgeted legacy DDS BC1/BC3 top-mip CPU decoding reuses
      the existing `image` package, checks all authored mip spans and allocation
      layouts, and retains profile metadata. Twelve synthetic tests, keyed original
      Footman diffuse/ORM pixel hashes, ATI2 refusal and native/WASM checks pass;
      three source-leaf 32-bit WASM fixtures also pass. No automatic TEXS suffix or
      namespace binding is added. CPU BC5, punch-through BC1, other profiles,
      lower-mip pixel decoding, application import and rendering remain
      unsupported/unverified.
- [x] `pcx-gfx`: explicit budgeted legacy ATI2/BC5 compressed import checks the
      actual enabled `RenderDevice` BC feature and extent limits. The complete
      raw header, checked mip spans and compressed payload accompany the Bevy
      `Image`. Five synthetic cases and the keyed HD Footman normal pass; actual
      Vulkan devices on the Radeon RX 7900 XTX verify import with BC enabled and
      typed refusal with BC disabled despite adapter support. Native/WASM checks
      and scoped strict Clippy pass. A separate keyed headless GPU upload and
      copy/readback check preserves all twelve compressed mips exactly on the
      enabled host device. GPU sampling/drawing, application upload integration,
      browser execution, channel/material/gamma semantics and automatic texture
      binding remain open.
- [ ] `docs/reforged-fixes.md` fix ledger; rows default to `defer`

## M2 — Asset decoders + first Bevy render — **render smoke passed; full material fidelity open**

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
- [x] `pcx-mdl`: malformed `BONE` records return typed errors before reading
       node fields: declared node size must be at least 96 bytes, with a separate
       8-byte trailer; record and track spans use checked arithmetic. Four
       regressions cover every shorter size, the valid minimum, and record/track
       overflow. The same tests pass in a 32-bit WebAssembly module; the previous
       module trapped on all three malformed cases. Both local classic model
       corpora still parse (3,234 and 3,249 models). This is checked import policy,
       not a claim about the original renderer's malformed-input behavior.
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
- [x] `pcx-gfx` viewer: `peacecraft3 view [install] <model>` displays the explicitly
       selected model, with camera/light framed from its extents. Optional
       `--tile FOURCC` selects one exact table row; errors stop rather than choose
       another row or draw a fallback color. This is a model-bound texture preview,
       not a W3E terrain renderer; see `docs/re-notes/viewer-terrain-selection.md`.
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

- Caller-selected SYLK coordinate limits now reject oversized C/F cursors before
  cell insertion or extent-driven postprocessing. Repeated K writes expose ambiguity
  while retaining the existing first value. Seven synthetic native and actual
  32-bit WASM cases pass; this is checked import policy, not native precedence or
  a whole-process memory bound. See `docs/re-notes/sylk-admission-limits.md`.

**Display boundary:** offscreen geometry/animation checks and the explicitly selected
model-bound texture preview are partial display evidence. They do not close W3E
terrain rendering, asset-defined terrain materials or native rendering fidelity.

### Known issues

- JPEG mips match an independent decoder to within ±1 per channel on 1.9% of
  bytes (15,111 / 786,432 on `AshenvaleExpansion-TopLeft.blp`); this is IDCT
  rounding between `jpeg-decoder` and libjpeg, not a format mismatch. Chasing
  bit-exactness would mean porting libjpeg's IDCT, so it is documented rather
  than fixed.
- Rust 1.99 workspace Clippy passes native all-target/all-feature checks with
  `--no-deps -D warnings`; affected SYLK/terrain/object/asset/application targets
  also pass on WASM. External dependency lints and other platform CI are not
  covered by those checks.
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
- [x] Custom launcher configuration layout (2026-10-18): project-owned controls replace stretched imported plates; a bundled sans font, stable selection borders, readable disabled labels, wrapping source paths and keyboard focus/scrolling are integrated. Original landscape artwork decorates only the custom launcher, with no extracted game art. Actual native and WASM launcher captures at 880×860, 1280×960 and 640×800, native main-menu entry, 48 launcher/navigation tests (one explicit asset test ignored), strict native/WASM builds and Clippy pass. Evidence: `.scratch/launcher-ui-root-20261018/verification.json`. This does not close original-menu, gameplay or rendered parity gates; no site deployment is claimed.
- [x] Native launcher monitor calibration (2026-10-19): the observed headless
      overflow came from mismatched framebuffer/RandR output bounds. The
      unchanged project fitter passes six natural-size native runs from 640×800
      through 3840×2160. The comparison boundary now refuses inconsistent
      monitor inputs before focus/input. Forty-eight Python tests, five startup
      unit tests, seven actual Winit cases, three real positive adapter checks
      and a real mismatch refusal pass; the rebuilt runtime image passes six
      display probes without launching a game. Saved-seed verification handles
      reported DPI scale. Desktop work areas/decorations and native Windows
      remain unverified. See `docs/re-notes/launcher-monitor-calibration.md`.
- [x] Launcher keyboard return now reveals the complete heading with the Main
      menu control (2026-10-19). Other controls retain nearest-edge scrolling;
      pointer and unchanged-focus scrolling remain undisturbed. Three synthetic
      regressions cover both scale factors and controls without a Pick action.
      The old path reproduces the clipping in a unit assertion and actual
      native/WASM captures. All 654 application tests, strict native/WASM
      checks/builds and twelve actual configuration runs at six desktop sizes
      through 4K pass; 36 new captures were reviewed. The browser runs use a
      code-only component host. See `docs/re-notes/launcher-keyboard-focus.md`.
- [x] Three-application comparison instrumentation (2026-10-18): one input journal drives actual original WC3/Wine, native PC3 and browser WASM PC3, with checked native process/window/build/content identities and matching native/WASM source fingerprints. Four checkpoints retain twelve unscaled 1280×960 captures and all three pairwise RGB comparisons; the run completed with differences (exit 1), not a runtime refusal. Forty-four Python checks and two browser-adapter checks pass. Evidence: `.scratch/three-target-reference-root-20261018/verification-final-4.json`.
- [~] Exact rendered-output acceptance: every decoded RGB pixel must match across all three targets, except the bottom-right version string only. No tolerance, normalization or broad corner crop is allowed. Its precise text footprint is not qualified, so no pixels are currently excluded. All four retained checkpoints fail the strict comparison; animation clocks/RNG remain uncontrolled and native frame state is unobserved. The local source closure was corrected to include the model-derived left-panel sound dependencies; fresh native/WASM captures restore the Video housing without inventing audio. Lighting, glyphs, slider thumb, settings admission and animation alignment still differ. This does not close deterministic gameplay, audio or original/native/browser visual parity.

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
- [x] Original grey outer button plate, TFT scene and transparent TFT logo render
      from locally served original assets. Parsed authored logo Birth is integrated.
- [~] Blue inner fill geometry is now parsed from FDF; browser integration proof
      and authored hover/pushed/disabled state rendering remain open.
- [~] Labels use centered relative text layout and native frame normalization;
      full FDF-driven style/text/anchor migration and fidelity remain open.
- [~] Source-derived hanging-panel Main/Options/Single Player phase animation
      is integrated and locally browser-checked. Native composited tint/render
      parity and the full-screen opening remain open.

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
- [~] `pcx-map`: versioned `war3map.w3r` bounds parse. Circle and polygon regions, and runtime region handles, remain open.
- [~] `pcx-map`: `war3map.w3i` MapInfo parser exists; complete slot/object semantics and runtime integration remain open
- [~] `pcx-map`: `war3mapUnits.doo` placements, item-set groups and random-selection payloads parse. Spawning them into the simulation remains open.
- [~] Admitted ground now resolves the selected Terrain.slk/BLPs and draws isolated
      variation-array cells with alpha composition under a pinned reconstruction
      profile. Actual Vulkan edge/minification and failed-publication checks pass;
      original appearance and coherent gameplay-world integration remain open.
      See `docs/re-notes/admitted-ground-rendering.md`.
- [~] `pcx-tera`: caller-selected cliff wall geometry and source-selected water
      surface geometry are implemented and tested. Water levels use the loaded
      `Water.slk` row and W3E words under the pinned recreation profile.
      Original cliff placement/threshold and water rendering remain open.
      Ground filtering/mips have a synthetic GPU gate; original mip/filter
      equality remains open.
      `WaterData` preserves source records and reports unproven level/geometry
      explicitly; see `crates/pcx-tera/WATER_EVIDENCE.md`.
- [ ] Camera: bounds, pitch, pan speed, zoom steps — values from RE
- [ ] **Gate:** Human campaign mission 1 loads and pans identically
- [ ] **Gate:** a community map's full container parses — container, `w3e`, `doo`, `shd`,
       minimap measured; `w3i` and region bounds parse, while slot semantics, `wpmp`, non-rect regions and full-container proof remain

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
- [x] Asset-derived stock tables: all seven families retain SLK/profile cells,
      source fingerprints, aliases, level columns and metadata. Ambiguous,
      missing and unsupported joins are reported. This is supplied-asset data,
      not proof of engine defaults, tech-tree behavior or gameplay parity.
- [ ] `pcx-objects`: `war3map.wpmp` allied shared data
- [~] Unit table: 4 races + neutral, buildings, heroes. Stock SLK cells exist; race, building and hero gameplay rules remain open.
- [~] Ability table + ability levels/objects. AbilityData cells exist; level objects and runtime abilities remain open.
- [~] Item table, upgrade table, buff table, destructable table. Stock family cells exist; gameplay tables remain open.
- [~] Tech tree, availability, random item/unit tables
      Caller-selected field catalog is checked separately: unknown metadata
      identifiers return an error; raw joins retain provenance and refusals.
      Synthetic tests and an explicit local archive gate cover collection,
      not native prerequisite evaluation or availability.
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

- [x] `pcx-sim`: deterministic project fixed-tick movement harness (headless, Bevy-free); original tick/RNG parity remains M6
- [x] Movement/selection distances cover the full signed coordinate range without
      subtraction/absolute-value overflow. Four native and four actual 32-bit
      release-WASM regressions pass; existing simulation/network/play traces
      remain green. Original movement/pathing semantics remain separate.
- [~] Entity store and deterministic movement iteration exist; original game handles and full gameplay store remain open
- [ ] Stats model: HP/mana/attack/armour/resistances/regen
- [ ] Damage pipeline: attack types, mitigation, crit, dodge, block, invuln
- [ ] Combat abilities, spells, channeling, stacking buffs/debuffs
- [~] Orders: select, move and stop reach the movement sim. Attack, attack-move, hold, patrol, follow and queueing remain open.
- [~] Pathing: `war3map.wpm` flag rasters parse. Walkability consumption, local avoidance and formation offsets remain open.
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

#### `pcx-script` — raw-JASS syntax, resolution and interpreter foundation

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
- [x] Interpreter prohibition retired. `pcx-script` resolves and evaluates, and `pcx-play` hosts caller-owned natives, including `TRIGSTR_` lookup. Native coverage, ordering and the map gates below stay open.

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

- [~] JASS resolver, typed values, interpreter and caller-owned Host exist. Native handle behavior/casts and the game Host remain incomplete.
- [ ] `WTG` trigger graph → JASS desugar with the original's exact semantics (incl.
       generated-code naming so `GetTriggerName` returns the right string)
- [~] `WCT` custom-text compilation: the public bounded adapter joins checked
       zero-header WCT/v4 WTG bodies with caller-supplied context and preludes,
       preserves cross-block state, source diagnostics and WTS lookup. Synthetic
       VM tests and the identified local Human06 compilation gate pass. Native
       source/load/initializer ordering, GUI lowering and campaign execution are
       still open; see `docs/re-notes/wct-compilation.md`.
       Complete WCT framing is now checked before materialization; malformed
       allocation probes and eight actual WASM parser groups pass. The explicit
       local NightElf06 authored-context compile gate passes; chapter execution
       remains open. See `docs/re-notes/wct-framing-preflight.md`.
- [~] Native function table (exact signatures, return conventions and **side-effect
       ordering** from `game.dll`). Caller-owned hosts implement a bounded subset; the full table remains open.
- [~] `BJ` / `Blz` library surface, including the `Blz` extensions. A few neutral-player `GetBJ*` adapters exist; the library remains open.
- [~] Timers and the original's timer/event ordering — the single thing DotA depends on
       most. `CreateTimer` handle admission exists; original event ordering does not.
- [~] `war3map.wts` string handles + localization lookups (the container is decoded;
       evaluator `TRIGSTR_` lookup exists). Broader localization and handle semantics remain open.
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
- [x] Cinematic/dialog FDF declaration ingestion preserves source identity,
      local records, template edges and every authored anchor with explicit
      caller bindings and limits. Seven synthetic tests, explicit local FDF gate,
      public crate integration and native/WASM checks pass. Native playback,
      dialog handles/events and rendered layouts remain open; see
      `docs/re-notes/ingame-cinematics-dialogs.md`.
- [ ] Netcode surface (loopback first)
- [ ] UI parity (HUD, minimap, command card, tooltips, chat)
      The HUD derivation now refuses broken template chains, missing
      labels, ambiguous/non-finite layout records and unsupported child/anchor
      inheritance instead of silently producing rectangles. Application tests
      and a seven-case actual WASM source-leaf gate pass. A caller-qualified HUD
      renderer is installed, with exact admitted-context/receipt checks and
      atomic resource replacement. The campaign path now produces the ConsoleUI
      policy (`campaign_hud.rs`); resource bar, buttons, minimap, portrait,
      rendered/native parity and command dispatch remain open. See `hud-derivation.md`.
      Shared FDF parsing now preserves nested ConsoleUI texture records without
      closing their frame or leaking properties. Both explicitly selected local
      ConsoleUI members parse; 13 actual WASI parser cases pass. Selected ConsoleUI
      inputs mount, and a synthetic Vulkan check covers texture ranges and alpha.
      Native texture sampling/composition remain open; see
      `docs/re-notes/hud-texture-blocks.md`.
      A caller-qualified tooltip text consumer now reuses the HUD's planned
      font, justification, wrapping and shadow, with bounded markup parsing and
      hover/keyboard focus on the same mounted surface. Synthetic application,
      Vulkan and parser checks do not establish native tooltip parity. Runtime
      field/picking publication and tooltip chrome remain open; see
      `docs/re-notes/hud-tooltips.md`.
- [ ] Audio parity (`.slk` sound-list channels, 3D positioning)
- [~] Deterministic movement work/allocation/state gate exists; extend fixed-work gates to gameplay/rendering as implemented. Report 4v4 1080p FPS observationally, never as a load-dependent pass/fail test.

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
      Explicit quest/quest-item state now shares the gameplay handle arena;
      creation, setters/queries, destruction, budgets and stale/cross-kind
      refusals execute in native and WASI tests. Initial flags follow the pinned
      original runtime probe, not guessed false values. A read-only view now
      exposes live text, flags and owned items from the same session; real
      owned-VM tests cover changes and destruction interleaved with timers.
      Campaign objectives and
      the rendered quest window remain open; see `docs/re-notes/script-quests.md`.
- [ ] Win condition triggers; end-of-movie / `endGame` entered
- [ ] Custom-map smoke set: all shipped `Maps/*.w3m` load and run their init script

### Tier C — differential fuzzing
- [~] Seeded movement-v1 generator and `playgen` emit checked synthetic RPLs
      from explicit domains/limits. Unit-type/combat compositions, terrain and
      native RNG generation remain open; see `docs/playtest-generation.md`.
- [~] Three synthetic movement seeds are checked in, regenerated byte-for-byte
      and independently repeated 100 times. Native differential corpus remains open.
- [ ] Divergences auto-filed into *Open unknowns* with minimal reproducing seed
- [ ] Gate: zero unexplained divergences over the corpus

## Open unknowns

| id | question | settle by | status |
|---|---|---|---|
| RE-Q1 | MPQ header layout | resolved — §2.3 item 1, `ev_faddef74…` | **closed** |
| RE-Q2 | Filename hash + seed policy | resolved — §2.3 item 3, `ev_ddc2808a…` | **closed** |
| RE-Q3 | Table cipher + file keys | resolved — §2.3 items 4–5, `ev_703ee25f…` | **closed** |
| RE-Q4 | Compressors | resolved — implode + zlib working | **closed** |
| RE-Q5 | Sim tick rate / frame relationship | Exact Legacy scheduler/timer chain traced; QPC calibration, gameplay subscribers and render coupling unresolved. See `docs/re-notes/legacy-simulation-clock.md`. | open |
| RE-Q6 | RNG generator + call order | Explicit seeded Host/factory behavior matches selected-code references on native/WASI and retained original 80 integer/16 formatted real observations. Twelve raw Human01 loading-time real results across three fresh unmodified Wine runs match direct Host history replay. Captured race inputs and ordered fresh wander units qualify one primary prefix. The owned timed-trigger consumer removes the registration boundary; the separate factory four-result gate still fails after one exact result at unsupported `TriggerRegisterUnitInRange` on native/WASI. Its private warmup adapter is not production engine initialization. Engine/secondary initialization, consumer integration and broader real-bit parity remain open. See `docs/re-notes/original-rng-control.md`. | open |
| RE-Q7 | Header words `0x20`/`0x24` | REA: `Storm.dll` open path; nothing depends on them | open |
| RE-Q8 | Storm's live table at `+0x4C2E4` ≠ fixed table | REA: `FUN_15017650`; archives use the fixed table | open, not blocking |
| RE-Q9 | Archive priority order | Selected 1.29b `Warcraft III.exe` archive-open/list-search order established for browser reads; `DEFAULT_PRIORITY` and other builds remain assumed | partial |
| RE-Q10 | ADPCM decompression (blocks 2,685 `.wav`) | Synthetic sector agreement is verified by the M1 ADPCM gate. Target-native 1.29b parity and browser wiring remain open. | partial; not the sole M1 gap |
| RE-Q11 | Determinism of the original under Wine | REA `capture-process`; gates all of Tier A/B | open, blocks M1b |
| RE-Q12 | `blizzard.ax` purpose | REA / string scan | open |
| RE-Q14 | `game.dll` leaks its full build tree + class names | **closed** — `docs/re-notes/001-classic-symbols.md`; 254 paths, 25 subsystems, artifacts in `rea/` | **closed** |
| RE-Q15 | Reforged PDBs + debug client console (2.0.1 leak) | recorded in §2.5; user-side acquisition only, nothing enters the repo | documented |
| RE-Q17 | Reforged path → EKey resolution | Checked configured TVFS mounts resolve the stored Footman path with complete manifest/EKey/CKey evidence; shorthand and client precedence remain unproved | explicit namespace implemented; native lookup open |
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
- [x] Movement lockstep server owns `pcx_sim::World` (`pcx-net::Server`). Gameplay-state ownership remains the open simulation store.
- [x] Project UDP transport and discovery request/response implementation.
      Fresh ephemeral loopback covers host response, source binding and resumed
      game polling; actual broadcast routing and application lobby wiring remain
      unverified. See `docs/re-notes/lan-discovery-collection.md`.
- [x] `LanSession`: lobby states, ordered inbox, tick gated on input arrival
- [ ] Gate: a real 1.29b LAN client joins a game we host
- [ ] Classic Battle.net client through an emulator realm
- [x] NET-C01: Pinned local PvPGN Docker realm with tracking disabled and a real
      bounded BNCS response probe; source/image/config identities retained.
      Root verified 12 bounds tests and live valid-init/invalid-init responses.
      This is a reference server, not original-client login or gameplay.
- [~] NET-C02: BNCS/W3GS header framing and a bounded native TCP caller pass
      fragmented/malformed/count/size tests and the owned PvPGN init/auth-info
      probe. Typed challenge parsing checks the selected versioncheck profile;
      W3GS reject-join decoding always returns a refusal, including raw unknown
      reasons. Other message bodies and the project codec remain separate.
      Original-client and lobby/gameplay checks remain open.
- [ ] NET-C03: PeaceCraft discovers and joins an original Legacy 1.29b LAN host
      over its original UDP discovery / TCP game path.
      A bounded production unicast query now reads a live original-host
      GAMEINFO using its observed product/version, preserving the map path,
      checksum, port and opaque stat extension. Synthetic native/WASI and
      loopback checks pass. A bounded Rust probe now uses the integrated join
      encoder and map validator/reply to reach the original lobby, receive map
      status 100 and answer pings. The later run generates its 1.29 IPv4 address
      tail from a route-selected IP and its own bound listener port, with no
      REQJOIN capture input. The native `tcp::Connection` now carries its
      decoder, queued/partial frames and cumulative bounds across join, map
      and ping responses; the original-host probe uses that production caller.
      Broadcast collection, launcher/lobby wiring and peer-to-peer reachability
      remain open;
      see `docs/re-notes/classic-network-interoperability.md`.
- [ ] NET-C04: Original unmodified Legacy 1.29b client discovers and joins a
      PeaceCraft host; record exact executable identity and both directions.
- [ ] NET-C05: Original-compatible lobby slots, map fingerprints, readiness and
      start/seed exchange; mismatched local edition/build/map refuses before play.
      Source-profile slot-info/join body decoding retains raw slot fields and
      seed bytes with exact size/count checks. It is not readiness, an original
      peer exchange or proof of matching RNG streams.
      Source-profile MAPCHECK comparison now validates the selected complete
      file's size/CRC-32 and the applicable MPQ/script XORO/SHA-1 stream, with
      caller byte budgets and borrowed provenance. Synthetic native/WASI and
      pinned C++ checksum vectors pass. Map-ready publication, original peer
      exchange and full NET-C05 acceptance remain open; see
      `docs/re-notes/classic-network-interoperability.md`.
      An original-to-original Wine lobby baseline now retains a complete
      leave/rejoin handshake. Integrated readers accept its frames/slots and
      verify the original MAPCHECK against the supplied local map/scripts;
      modified/absent maps refuse. Slot download status reaches 100 without
      map transfer. Lobby seed words change between updates; no game-start or
      RNG agreement follows. This is not a PeaceCraft peer exchange.
      A successful `MapMatch` now constructs a MAPSIZE "have map" reply from
      its verified size and an explicit caller prefix; it equals the captured
      original reply offline. Five map checks pass under WASI. Sending that
      reply from the live Rust probe produces map status 100 on the original
      host. A mutated local map refuses before the probe sends MAPSIZE.
      Publishing revision-bound application readiness remains unimplemented.
- [ ] NET-C06: Ordered actions and checksum/RNG agreement, followed by a ten-minute
      mixed-client match without divergence; lobby-only success is insufficient.
      Bounded source-profile action-body readers now preserve player ids,
      payload order and raw checksum bytes. Checksum verification, game-order
      decoding, simulation agreement and original-client exchange remain open.
- [ ] NET-C07: Classic realm version checks, authentication and negative cases,
      channel/chat, game advertisement/list/join; unmodified and loader profiles
      have separate results, including the unresolved 1.29.2 authentication path.
- [ ] NET-C08: Disconnect, timeout, lag and cancellation; prove any optional
      reconnect extension separately from original-client support.
      The native continuous TCP caller shuts down on transport/framing/budget
      failure, refuses reuse, checks partial EOF and preserves one absolute
      deadline across calls. Loopback tests cover peer closure and multi-step
      partial frames. A shutdown-only controller now aborts an established
      connection with an explicit Cancelled result; loopback tests cover stale
      controller isolation, and a live original-host probe cancels after map
      verification and ping/pong. Application worker/publication wiring, graceful
      leave, lag and reconnect remain open.
- [ ] NET-C09: Reproducible original/PeaceCraft network comparison harness using
      local assets, the same admitted map and captured inputs/seed; no proprietary
      asset transfers, credentials or key material in committed evidence.
      Reference sources and dependencies: `docs/re-notes/classic-network-interoperability.md`.
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
- [x] Project protocol: **server holds state, client only proposes** — the rollback
      base is always a state the server sent; replay only re-applies the client's *intent*

## WASM build

- [x] Original startup movie playback with Escape/visible skip/end/error paths, soundtrack ownership, browser mute setting and native decoder cleanup. Native cinematic playback currently needs an installed FFmpeg; original startup dispatch and frame-perfect A/V parity remain unverified.
- [x] Parsed TFT logo Birth curve and native default sprite-origin placement; shipped-model fixture and browser render verified. The hanging panel entrance remains open.
- [x] Settings expose 1.29b only; Edition is RoC on the left, TFT on the right, TFT selected by default.
- [~] Complete original resolution parity: full-window background and conditional authored UI branch are traced and implemented; portrait containment is an extension, live expansion flags remain unmeasured.

- [x] Two-browser local WebSocket command/snapshot path for existing movement rules, including same-tick merging, identity rejection and disconnect/reconnect/resync.
      Selection now survives snapshots and bounded client replay. The browser
      decoder accepts the checked selection extension; the live two-browser gate
      covers deselection, ignored move, reselection and reconnect. Disconnect
      stops unselected owned units too. Native and actual WASI regressions pass;
      original gameplay/lobby integration remains open.
- [ ] Actual gameplay/lobby integration and public multiplayer service; see `docs/browser-online.md` for the current limits.

- [~] Browser-only ISO import: local File/Blob reads, filesystem discovery,
      validated Warcraft III archive selection and in-memory asset loading.
      **No server upload or remote extraction.** Track unsupported repacked
      installers explicitly; bounded memory, progress/cancel, and a network
      capture proving content stays local are acceptance checks (PLAN M11).
- [x] Asset-free [GitHub Pages menu demo](https://maci0.github.io/peacecraft3-demo/):
      only HTML/CSS/JS and WASM are published from a separate public repository.
      Local file-input import and base-only TFT refusal pass in the packaged app;
      competing import/clear events are guarded. Live HTTPS startup has no game
      assets or enabled menu before import. Retail installer ISO and full gameplay
      acceptance remain separate open gates; see `docs/pages-demo.md`.
- [~] Original menu feel/sound: animated background, correct button states and
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
- [~] Explicit browser map configuration: mandatory caller metadata now reaches
      the existing bounded binder/App consumer through owned WASM inspection,
      submission and completion handles. The collapsed local JSON/explicit-limit
      form and metadata inspector pass production-page main-App admission,
      stale-source refusal, request cleanup and base-only TFT restriction checks.
      Cargo-built WASM/Node gates also cover whole-state rollback. The asset-free
      public demo matches the checked 22-file package and passes the same eight
      main-App browser groups. Original-map execution remains open;
      see `docs/re-notes/browser-map-configuration.md`.
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
- [x] Current Pages artifact uses `wasm-bindgen --remove-name-section`:
      61,332,742 bytes, no debugging name section, with all executable sections
      compared equal to the same build's unstripped output. Prior artifact sizes
      above are historical. Native performance/full-frame FPS are not implied.
- [x] Three browser limitations documented rather than fixed: **staging** (no
      destination to copy into → every source reported rejected), **install-level
      version detection** (one archive can be opened and counted, but not
      identified as 1.28c or 1.29b), **rendering without a canvas**.
- [x] `crates/peacecraft3/README.md` provides the browser build recipe and earlier
      artifact-size measurements. Current artifact measurements remain a separate gate.

## Desktop builds

- [x] Four-target native CI matrix and packaging helper added; executable
      header validation covers both Windows PE and macOS Mach-O architectures.
- [x] Windows x86-64 compiled, tested and artifact verified.
- [x] Windows ARM64 compiled, tested and artifact verified.
- [x] macOS Intel x86-64 compiled, tested and artifact verified.
- [x] macOS Apple Silicon ARM64 compiled, tested and artifact verified.
- [ ] Menu GPU rendering and audio verified on both desktop operating systems.
      Run `38439725394` at `d31dc67` succeeded on all four targets.
      Downloaded artifact checksums and PE/Mach-O architectures were independently
      verified, including executable permissions on macOS. These artifacts include
      movie/layout changes but predate the new FDF/FPS/parser integration.
      Commands and limitations are recorded in `docs/build-desktop.md`.

## Integration audit and performance

- [x] Avoidable Rust `unsafe` removed from the retained Naga snapshot and perf
      symbol export. Workspace/tool lints deny it; Naga forbids it. Only the two
      documented `GlobalAlloc` implementations remain for allocation gates.
      Strict native all-target/all-feature checks, native/WASM application builds,
      742 workspace tests (53 explicitly ignored) and 115 standalone Naga tests
      in debug/release pass. Naga minimal/no_std and all-feature WASM checks pass;
      allocation counts and simulation digest are unchanged. The existing host
      Callgrind loader failure remains documented in `docs/performance-testing.md`.
- [x] Full milestone/source audit: `docs/implementation-audit.md` records the
      remaining format, world, gameplay, scripting, campaign, multiplayer and
      browser dependencies. Historical sweep counts are not completion gates.
- [x] `AGENTS.md` requires deriving authored data from original assets at build
      time or runtime; measured constants are still hard-coding.
- [x] F8 top-right full-window FPS/frame-time overlay: strict toggle test, profiling
      feature checks and rendered wide/portrait browser placement/resize proof pass.
- [x] Local profiling helper and doctor: self-process `perf` capture succeeds;
      ROCm HIP/HSA tools and RADV graphics routes are documented separately.
      RADV hardware capture passed (9 MB RGP); viewer analysis remains unverified.
- [x] Movement performance gate measures deterministic allocations, bytes and
      state digest under concurrent CPU load. Timing is report-only.
- [x] Native application Clippy passes all targets/features with `--no-deps`
      and warnings denied. Related Bevy parameters/query tuples are grouped;
      mechanical Rust 2024 fixes preserve fields, filters and runtime behavior.
      The 106-test app suite, explicit original menu/movie gates and WASM compiler
      check pass. Dependency Clippy debt is outside this application-only gate.
- [x] Borrowed menu-track sampling removes the per-track bone/key clones.
      All interpolation kinds and clock/duplicate behavior have regression
      checks. An isolated allocation gate reports zero sampling allocations;
      original RoC/TFT BONE-only measurements preserve exact pose-bit digests
      while removing 12,064/11,504 allocations across eight sampled frames.
      See `docs/performance/menu-pose-sampling.md`; full-frame work and FPS are
      outside this measurement.
- [x] Normal blue inner fill uses original decoded texels and relative,
      FDF-derived grid bounds. Actual browser import/resize captures show the
      expected fill extents. The strict one-pixel edge diagnostic fails on two
      edges (maximum 1.133 pixels); inner edges/state art remain unfinished.
- [!] Original script execution remains incomplete. Selected-map adapters now
      pass the earlier `GetPlayerNeutralPassive` initialization boundary; original
      campaign completion and native behavior are not established.
      Opt-in selected-map adapters derive passive/aggressive and first-family
      `GetBJMaxPlayers`/`GetBJPlayerNeutralVictim`/`GetBJPlayerNeutralExtra`/
      `GetBJMaxPlayerSlots` rules from independently approved local executable
      profiles and the admitted W3I. The same Human01 Program/localization gate
      composes 43 source-selected symbolic contracts, retains 432 typed
      identities. Explicit shared-arena timer/group admission now completes
      all Human01 globals (four timers and three groups) and enters authored
      `config`. Executed config now retains five metadata fields and five start
      locations. Source-bounded shared-arena players retain five player
      identities and 35 authored field assignments. Bounded authored priority
      relationships now let Human01 `config` complete; the same VM enters `main`
      and uses source-bound camera margins to retain authored camera bounds before
      selecting explicitly bound day/night source models, then reaching
      a source-qualified immutable startup time-of-day snapshot, then requesting
      both source-backed ambient sounds. An explicit bounded start queue retains
      the authored night sound request. Source-bounded map-music selection now
      retains the executed name/random/index. Exact direct-file PCM admission now
      creates the first authored voice object. Selected source-label parameters
      now bind to that same sound identity while preserving its PCM/creation inputs,
      and explicit executed duration state now retains the authored integer on
      that same identity. Bounded MPEG admission now retains original encoded
      resources alongside PCM: the pinned Human01 VM admits all 58 direct file
      bindings and 51 distinct parameter labels, then executes all 58 creations,
      label assignments and duration assignments. Shared-arena rectangles now
      retain executed region bounds with exact coordinate-bit checks;
      direct-file and label sound identities now retain explicit numeric
      properties without inferring mixer defaults. Executed 3D sound position
      matches the script's coordinate bits. Bounded register/unregister
      requests now preserve their sound identities,
      dimensions and position snapshots. The same gate reaches 142 rectangles
      and seven stacked registrations. Shared-arena camera setups now retain
      18 identities, 126 executed field assignments and 18 destinations with
      exact value/duration-bit checks. Its next refusal is
      `SetPlayerTechMaxAllowed` in that camera gate. The subsequent player-state
      gate retained 36 executed technology maxima on their source players.
      With researched-level storage admitted, the same run retains 65 maxima
      and one researched-level record, matching the executed signed values and
      player identities. Creation-state support then retains four destructable
      records with exact executed arguments on the same arena. Its next refusal
      is `CreateDeadDestructableZ` at map span `58398..58471`. Admitting the
      declared create variants then retains five creation records, including
      the executed dead form and its supplied `z`; the next refusal is
      `CreateUnit` at map span `69421..69469`. Source-bound unit creation now
      retains 53 executed unit records against the same selected catalog and one
      explicit acquire-range assignment. Shared-arena trigger admission then
      retains one trigger and one unit-event registration with their exact
      captured identities and symbolic enumerand. Admitting authored action
      registration then reaches 153 source-bound units, three triggers, three
      unit events and three action records. One executed `SetHeroLevel` request
      now retains its signed level and eye-candy flag on that same unit;
      experience, derived stats and effects remain unimplemented. One executed
      `SetUnitState` assignment now keeps its symbolic domain handle and exact
      finite real, with caller-bounded pair storage. This reaches 154 unit
      records; resource spending and combat remain unimplemented. The next
      refusal is `IssueImmediateOrder` at map span `67827..67884`; callback
      execution and event delivery remain open. See `docs/re-notes/script-trigger-admission.md`.
      A source-associated original-client order control now repeats at actual
      3840×2160 and 1280×960 under isolated Wine: on/off transitions succeed,
      repeats and an invalid order fail, and a sibling unit stays independent.
      Both preload outputs match byte for byte. This instrumented probe does
      not implement the order native or advance that VM frontier; see
      `docs/re-notes/script-immediate-orders.md`.
      The source-composed native Rust gate has now been recovered against current
      libraries after loss of the earlier advanced harness directory. Its
      source-derived 58-file bank (23 PCM/35 MPEG), 51 parameter labels and shared
      Host reach the same `IssueImmediateOrder` span after globals/config and
      154 unit creations. Six collector tests and 324 play-library tests pass
      (five library ignores). The source/artifact and external-input hashes stay
      unchanged through the qualified run. This restores the acceptance gate;
      application Host construction, chapter execution and rendering stay open.
      Explicit Defend activation admission now reads the selected ability row,
      exact alias/code, on/off strings and single-level upgrade requirement. Its
      per-unit bit returns the observed on/off transition results without a
      fabricated resource default. Unmatched order strings stay unsupported.
      The unchanged Human01 passes both fresh off requests and reaches
      `SetPlayerAlliance` in `Blizzard.j` at `224476..224550`, retaining 155 units on
      the same Host/context. Combat modifiers, animation and the application
      startup caller remain open; see `script-immediate-orders.md`.
      Explicit directed alliance admission subsequently retains 26 tuples from
      the same map's executed calls, preserving false and separate directions.
      The next refusal is `SetPlayerState` in `Blizzard.j` at `225072..225136`, with
      843 shared identities and 155 units. Getter/default policy and alliance
      gameplay remain open; see `script-player-setup.md`.
      Setter-only player-state request admission then retains the exact executed
      bounty-setting integer and advances to `Filter` in `Blizzard.j` at
      `436855..436900` (common declaration `71869..71927`). All previous state
      checks still pass. Effective balances/defaults, `GetPlayerState`, bounty
      effects and the application consumer remain open.
      Filter admission then creates seven callback wrappers on that same arena,
      with loaded ids, kinds and declaration provenance. The next refusal is
      `CreateForce` in `Blizzard.j` at `437580..437593` (common declaration
      `65813..65872`), retaining 850 identities and 155 units. Predicate execution,
      candidate context, destruction and repeated-code identity remain open;
      see `script-filter-wrappers.md`.
      Explicit shared-arena force admission then creates 17 force identities and
      retains 16 player memberships, matching the initialized source slot bound
      and each singleton's actual player row. The final all-players force stays
      unpopulated. The same gate retains 868 identities and reaches
      `ForceEnumPlayers` in `Blizzard.j` at `437759..437808`, selected common
      `66202..66289`. Enumeration candidates, callback contexts, destruction and
      native duplicate-input behavior remain open. See
      `docs/re-notes/script-force-memberships.md`.
      Two isolated original-client Wine controls now distinguish enumeration
      from the full admitted slot domain: the original configuration includes
      its active user; an explicit computer configuration adds that computer.
      False filters empty populated destinations and selective filters keep
      predicate acceptance separate from visits. Playing creep neutral slots
      remain excluded. Controller/state causality and dynamic participation are
      not fully qualified; enumeration stays unsupported pending a current
      participation input. The 100-run baseline gate remains open.
      Null-filter enumeration now accepts an explicit caller-qualified startup
      participant set on the same Host/context, with no controller or slot default.
      Replacements check final aggregate capacity before mutation; non-null filters
      refuse. Controller writes and explicit external invalidation retire the
      input. Loading the preserved original startup set in the unchanged Human01
      gate yields one all-force member, 868 identities and the next refusal
      `GetGameSpeed` in `Blizzard.j` at `437880..437894` (common `59055..59114`).
      A fresh original-client settings probe now qualifies explicit speed/fog/mask
      input for that context. The unchanged gate passes all three getters and
      stores their values in the Blizzard globals; the next refusal is
      `GetPlayerSlotState` in Blizzard at `438504..438537` (common `61390..61470`).
      Independent fog/mask setters retain boolean state. Rate-changing
      `SetGameSpeed` requests remain unsupported: Fast/Fastest requested during
      the probe read back Normal. Already-current requests are supported by the
      later current-speed continuation below.
      No startup defaults, speed rates or rendered fog behavior are inferred;
      see `docs/re-notes/script-startup-settings.md`.
      Explicit startup slot-state input now passes `GetPlayerSlotState` on that
      unchanged context. Only the five existing post-config rows receive input;
      omitted/later rows remain unset, and controller writes retire the snapshot.
      The next missing input is player 3's unset controller, reached through the
      existing `GetPlayerController` getter in Blizzard at `438445..438479`
      (common `61313..61388`). No controller/occupancy defaults or completed
      single-player check are inferred. See `docs/re-notes/script-player-slots.md`.
      A separate explicit current-controller snapshot now supplies all captured
      source slot numbers without minting players or overwriting executed setup.
      The unchanged source completes the single-player loop (`bj_isSinglePlayer`
      true) and next refuses `CreateSoundFromLabel` for Rescue in Blizzard at
      `438917..438982` (common `155708..155881`). Current input omissions and
      invalidation refuse instead of falling back to assigned setup. This is not
      a live lobby provider, completed sound initialization or playable chapter;
      see `docs/re-notes/script-player-controllers.md`.
      Explicit label-constructor banks now qualify all 11 literal labels from the
      unchanged Program against two selected sound tables and their PCM members.
      The same arena retains 11 executed creations across nine labels, with exact
      booleans, signed fades and source-bound resources. It now contains 880
      identities and next refuses `TriggerRegisterTimerExpireEvent` in Blizzard
      at `143744..143837` (common `72705..72793`). Four new regressions and all
      357 native/actual WASI play tests pass; strict native/browser Clippy passes.
      This is resource/creation state, not playback, timer scheduling or a chapter
      completion. See `docs/re-notes/script-label-sounds.md`.
      Timer-expiry subscriptions now separately admit the loaded trigger/timer
      contract, require both actual identities and retain their association on
      the same bounded arena. The unchanged gate passes one registration and its
      following action, reaching 882 identities before refusing `VersionGet` in
      Blizzard at `440139..440151` (common `116044..116091`). Four new regressions
      and all 361 native/actual WASI play tests pass; strict native/browser Clippy
      passes. Timer start, expiry delivery, callback execution and native lifetime
      parity remain open. See `docs/re-notes/script-trigger-admission.md`.
      Explicit runtime version input now resolves the preserved original-client
      observation through the selected `ConvertVersion` domain and exact map
      context. `VersionGet` returns that identity without allocating; absent,
      invalidated, foreign or out-of-domain input refuses. The unchanged gate
      reaches 888 identities and six triggers, then refuses
      `TriggerRegisterGameStateEvent` in Blizzard at `435358..435454`
      (common `72797..72927`). All 364 native/actual WASI play tests and strict
      native/browser Clippy pass. No edition/default or archive precedence is
      inferred; see `docs/re-notes/script-startup-settings.md`.
      Game-state subscriptions now validate selected integer/float state and
      limitop domains and retain the exact trigger/state/comparison identities
      and finite real threshold bits on the same arena. The unchanged gate passes
      six registrations, reaching 901 identities and nine triggers before
      `GetCameraBoundMinX` in Blizzard at `440924..440944`
      (common `154188..154249`). Three new regressions and all 367 native/actual
      WASI play tests pass; strict native/browser Clippy passes. No comparator,
      running clock, initial notification or callback delivery is inferred. See
      `docs/re-notes/script-trigger-admission.md`.
      Camera-bound getters now separately admit exact source-matching contracts
      and read coordinate extrema of all four executed corners in the qualified
      unrotated startup profile. A corrected original Wine probe verifies
      reordered/irregular corners and restored authored bounds. The unchanged VM
      matches eight reads and both BJ rectangles, reaching 903 identities and
      144 rectangles before `GetPlayerTechResearched` in Blizzard at
      `441600..441652` (common `113417..113526`, boolean return). All 370
      native/actual WASI play tests and strict native/browser Clippy pass.
      Rotated bounds, missing research state and rendered-camera parity remain
      open; see `docs/re-notes/script-camera-input.md`.
      Explicit current boolean research observations now serve full source
      player/tech/specificonly query keys without interpreting retained levels.
      A readonly original Wine probe records 24 identical before/after answers;
      the same VM refreshes after its early setter, consumes all 24 queries and
      completes `InitSummonableCaps`. Successful research setters retire that
      player's current answers; failed writes and invalid admissions preserve
      prior input. Cumulative tech-max calls reach 101; the 903 identities and
      prior shared-arena checks remain intact. Next refusal:
      `SetAllItemTypeSlots`, Blizzard `446992..447041`, common `109983..110054`.
      All 373 native/actual WASI play tests and strict native/browser Clippy pass.
      Counts, missing research defaults, live provider, item-stock behavior and
      playable chapter remain open; see `docs/re-notes/script-player-setup.md`.
      Explicit stock slot configuration now retains the executed integers from
      `SetAllItemTypeSlots` and `SetAllUnitTypeSlots`, plus bounded per-unit
      `SetItemTypeSlots` and `SetUnitTypeSlots` records on the same Host.
      Unset stays distinct from an explicit zero. The BJ slot constants are
      not copied into the host, and a per-unit record does not inherit the
      global integer. Add/remove stock, restock, categories and shop effects
      stay refused. The same Human01 gate now retains both global assignments
      and refuses `TimerStart` in Blizzard.j (`447183..447286`). All 376 native
      and actual WASI play tests, strict native/browser Clippy, 78 map-loading
      and four app-library tests pass. Native slot-capacity semantics and a
      playable chapter remain open; see `docs/re-notes/script-stock-slots.md`.
      Live participation, application provider and general eligibility remain open.
      No world objects or rendering are claimed. These values do not yet enforce
      construction, training or
      upgrade effects. Applied camera motion remains open. See
      `docs/re-notes/script-camera-setups.md`.
      This probe still uses the explicitly identified compatibility object catalog;
      launcher bank preparation remains open.
      The selected CampaignStrings reader now derives ordered chapter/map
      records through the shared profile parser, retaining blank slots and
      uninterpreted extra fields. Synthetic native/WASI checks and explicit
      base/expansion member gates pass. Unlock/progress rules, chapter UI and
      the map execution caller remain open; see
      `docs/re-notes/campaign-index.md`.
      Raw table flags remain separate from effective settings.
      Explicit parameter collections now retain each assigned table/row/source,
      reject cross-table label ambiguity and preserve a separate dialogue schema
      without fabricating the absent full-profile fields. A bounded PCM decoder
      view preserves source receipts and exact samples while placing fmt/data
      chunks before opaque ancillary chunks for the installed WAV decoder.
      The snapshot does not advance time or replace a running game clock, and
      queued music selection is not yet consumed by a playback backend.
      A bounded native script-sound consumer now handles the explicitly supported
      non-spatial profile; see `docs/re-notes/script-audio-playback.md`. All 305 play tests
      pass with five explicit ignores; strict native/WASM Clippy, WASM generation
      and actual 32-bit lifecycle/config/player/priority/camera/model/clock/sound checks pass. These are bounded Host/data
      checks, not campaign or native gameplay proof.
      Launcher profile loading/default Host wiring and other player natives
      remain open; see `selected-map-neutral-host.md`, `playercolor-host.md` and
      `script-timer-arena.md`/`script-group-arena.md`/`script-map-config.md`/
      `script-player-setup.md`/`script-start-priorities.md`/`script-camera-input.md`/
      `script-day-night-models.md`/`script-label-sounds.md`/`script-startup-clock.md`/
      `script-map-music-selection.md`/`script-file-sounds.md`/`script-sound-parameters.md`/
      `script-sound-parameter-banks.md`/`script-pcm-decoder.md`/
      `script-mp3-sounds.md`/`script-rectangles.md`/`script-sound-properties.md`/
      `script-stacked-sounds.md`.

### Current implemented slices (2026-10-14)


- Menu PRE2 now has bounded source-driven persistent simulation, image admission,
  geometry/particle ordering and a particle-only additive black-fog material.
  The affected native suite passes 446 tests with 18 explicit local-input/device
  ignores, strict native/WASM Clippy, actual release-WASM rollback checks and
  the separately executed paired original ECS gate. A corrected local-repack
  browser workflow passes four reviewed Main/Video Options/Main captures with
  every parsed emitter admitted and no uploads/sockets/page errors. Snow draws
  without the first integration's bright fog rectangles. This does not establish
  native distribution, clocks, density, phase or full material parity; gray haze
  remains open. Empty meshes now stay CPU-owned until populated; actual Bevy
  extraction/lifecycle regressions pass and a fresh browser run reports zero
  empty-mesh allocator warnings. The split opening gate's source is still
  unidentified. See `docs/re-notes/menu-particles.md`.
- CLI Spec guidance is recorded in `AGENTS.md`. A source-only inventory covers
  all 14 workspace binaries. Unknown application commands now return usage on
  stderr and exit 2 instead of launching the GUI; no-argument GUI and existing
  launch/view routes remain. Sixteen genuine subprocess cases, affected tests
  and strict native/WASM checks pass. Structured output/offline schemas across
  the CLI set remain open; the v0.3 candidate pin is unverified, so no full
  conformance is claimed.
- Menu fog is read from the required local `UI\MiscData.txt` edition column and
  attached to the original background camera. Material Unfogged flags remain
  independent of Unshaded. Missing/malformed fog rejects the import candidate
  before replacing its previous assets. Additive mode 3 suppresses fog color as a
  trace-corroborated recreation convention, preserving authored alpha. The fog
  application gate passes 272 tests with
  eight explicit ignores, strict native/WASM Clippy and the paired original-menu
  camera gate. Adding classic Transparent cutout raises the combined native
  count to 273; strict native/WASM gates remain clean. The earlier release WASM passes 49 in-process import-worker checks and
  explicit fog rollback, clear and RoC/TFT cases without a server/network.
  A local original-fixture browser Main/Video Options/Main run passes, with no
  uploads, sockets or page errors; its captures identified haze before the
  additive correction. A fresh corrected release separately passes that same
  four-capture workflow and actual WASM checks with zero uploads/sockets/errors.
  All captures were reviewed; haze and native visual parity remain open.
  Publication is tracked separately; see `docs/re-notes/menu-authored-fog.md`.
- Classic material Transparent mode now uses alpha-test coverage instead of
  soft blending, following the pinned primary renderer's cutoff and composed
  opacity convention. Actual ECS material construction preserves source alpha;
  PRE2 is unaffected. Native fragment/depth parity remains open; see
  `docs/re-notes/menu-alpha-test.md`.
- The authored menu background camera now derives its normalized angle and
  logical aspect from CAMS plus the established glue coordinate convention.
  Source clips/pose are checked before scene writes; resize retains the logical
  frustum. The affected application passes 266 tests with eight explicit ignores,
  original RoC/TFT import and material-binding gates, strict native/WASM checks
  and a release WASM build. The code-only package is local and unpublished;
  rendered comparison and native presentation scaling remain open. See
  `menu-camera-projection.md`.
- Strict workspace check across all targets/features passed; direct registry
  dependencies are at latest stable releases and compatible transitives refreshed.
  Rust stable is 1.99.0; native builders install the stable channel and deny warnings.
- Refreshed CPU integration: 417 passing tests, 24 explicitly ignored original
  fixtures. Original format checks are separate: later MPQ Huffman/ADPCM checks
  match the supplied compatibility Storm oracle, while single-unit/IMPLODE
  branches still have synthetic coverage only.
- MDX checked layers retain authored layer counts, corrected alpha, filter/shading
  fields, alpha/texture tracks and PRE2 data. Full renderer wiring remains open.
  Graphics model import now rejects non-finite header extents with source/axis
  context while preserving raw parser words and finite flat/inverted records.
  The local header census reads all 3,234 compatibility and 3,249 Legacy models;
  four in each have inverted bounds and remain accepted. Footprint-specific
  ordering stays with its consumer; see `viewer-terrain-selection.md`.
  The menu now binds GEOA opacity by geoset ID and samples first-layer KMTA and
  KGAO/static alpha using selected sequence/global clocks. Sparse-ID, independent
  clock and actual ECS material/visibility regressions pass; both original menu
  imports and browser runtime pass. First-layer KMTF texture selection now binds
  authored TEXS IDs at spawn and advance on independent global clocks, retaining
  per-record sampler policy and avoiding redundant material changes. Native ECS
  regressions, both original background imports and six source-qualified retained
  model bindings pass; strict native/WASM checks pass. Release WASM and a fresh
  original-repack browser Main/Video Options/Main run pass; reviewed captures
  show water and the corrected logo. The asset-free demo at `6e2c4de` matches all
  23 package hashes and passes live startup. This is not native visual parity:
  multilayer drawing, sky projection, the uncovered black region and native
  clock/blend equivalence remain open. GEOA static tint and KGAC animation now
  reach background/panel materials through a pinned Warsmash source profile:
  static file order passes through, animated BGR swaps into RGB, and alpha is
  preserved. Six colour and five duplicate-time plan cases plus seven runtime
  cases pass natively and under WASI; an ECS update/atomic-refusal regression,
  both supplied menu imports and strict native/browser-WASM builds pass. Native
  flag/colour-space semantics and rendered colour parity remain open. A later
  explicit synthetic Bevy render-graph gate passes 108 RGB checks on each of
  software and Radeon Vulkan, including asymmetric KGAC endpoints and an untinted
  control. This verifies transport through the actual caller/material, not paired
  original-game parity. See `menu-opacity.md`.
  First-layer TXAN translation now uses parser-derived UV-set metadata and an
  absolute UV transform on independent global clocks, including panel phases.
  The full affected native run passes 460 tests (184 application), with 24
  explicitly ignored inputs; required original model/import/binding gates and
  strict native/WASM checks pass. Release/rendered acceptance is pending.
  Rotation/scale, multiple UV sets, native clock/UV orientation, sky projection
  and full menu fidelity remain open. See `menu-texture-animation.md`.
- World import retains original placements/ownership and metadata values.
  Human01 resolves 155 units while retaining six named unsupported `sloc` records;
  Human03 resolves two items. This is data import, not playable campaign proof.
- ISO9660 primary/Joliet discovery: synthetic namespace, bounds and refusal
  tests pass with network APIs trapped. The supplied installer ISO discovers
  ten files via its actual Joliet directory. The bounded Inno/FreeArc browser
  decoder derives bindings from the local installer script and validates control
  directories, then streams REP/4x4/Tornado with all member CRCs checked.
  The supplied stored base MPQ imports and renders its RoC menu in Chromium;
  cancellation retains the prior revision and temporary files are removed.
  MPQ-named AVI payloads cannot be staged. Language-patch decoding/application,
  other installer/codec profiles and full installed-game equivalence remain open.
  See `docs/re-notes/original-iso-installers.md`. WASM download and local
  import progress use measured work or explicit indeterminate phases.
  Transactional in-browser MPQ registration, runtime asset hydration, local audio
  and cache refresh pass actual generated-WASM/browser verification.
  Unsupported namespaces/installer payloads and conflicting archives fail closed.

- Current release WASM, native/WASM builds, rustdoc and strict Clippy pass;
  generated glue uses matching wasm-bindgen 0.2.129. Independent MainMenu/Options
  and MainMenu/SinglePlayer panel admission uses source clocks and target-control
  handoff. Four library + 257 app tests (11 explicit ignores) and 83 integration
  tests pass. Actual Chromium/WASM executes 49 existing + 103 panel checks,
  including 12 required rollback cases and four combined-failure orderings.
  The unchanged local original-repack fixture passes six reviewed captures with
  settled Main/Options/Main and Main/SinglePlayer/Main navigation, 15 local code
  GETs and zero uploads/sockets/page errors. Known unavailable Huffman audio no
  longer hides checked graphics. Full-screen opening, sky/native rendering,
  audio parity and remaining Options/gameplay behavior stay open. See
  `docs/re-notes/menu-panel-navigation.md` for exact checks and limitations.
- Local ISO file-input discovery and 26 actual WASM archive assertions pass;
  Chromium audio replacement/reset passes with zero uploads/unexpected requests.
  Running-app File-input import refreshes six authored labels and the scene;
  click audio, resize and unequal-member rollback pass with zero uploads/sockets.
  Bounded stored-base import from the supplied installer ISO passes separately;
  patched installer/retail-disc acceptance and full rendering parity remain open.

- Menu font files derive from each entry's FDF face and the supplied skin table,
  with no production face-to-file constant. New browser verification passes
  12 Node tests and 26 actual WASM assertions, including duplicate-face refusal
  and atomic rejection of unequal files mapping to the same renderer alias;
  running-app original font import/alias bytes, cache, audio, resize and rollback
  pass. Per-entry handles now apply to the label root and every authored run;
  a two-face ECS regression verifies unrelated launcher/F8 text is untouched.
  Six browser F8 visibility checks pass before/after import and after resize.
  Skipping the movie with Escape or its button returns focus to the visible
  canvas; direct F8 works without an extra click in both browser checks.
  Shadow offset, FIXEDSIZE and accelerator dispatch remain unresolved.

- Checked CASC readers and asset-source layering belong to the strict workspace.
  Seven CASC synthetic tests, one explicitly supplied Reforged install sweep,
  19 asset-layer tests and one documentation test pass. Archive adapters and full
  Reforged rendering remain open.
  TVFS adds checked explicit namespace traversal: 12 synthetic and two original
  tests pass; 110 manifests contain 175,018 entries and retain one non-UTF8 path.
  The named Footman verifies to 105,602 MDLX bytes. Version-1800 geometry now
  decodes into five geosets and 582 Bevy mesh vertices. Authored LoD metadata is
  preserved; no LoD selection or newer material/rendering behavior is inferred.
  The explicit version-1800 material reader tiles the supplied Footman's 404-byte
  MTLS payload into three materials/four layers, preserving four subtextures and
  seven KMTA keys. Three discriminating synthetic tests and the original parse
  gate pass. Classic accessors retain their newer-version refusal; no shader,
  interpolation evaluation or rendering parity follows from stored framing.
  Four asset-adapter regressions and two explicitly supplied original members
  pass through source layering. MPQ decoded budgets and locale/platform ambiguity
  are checked before reads; CASC locators retain config/encoding/manifest keys
  and source spans. Unsupported winning content does not fall through.

- Rust 1.99 format cleanup preserves classic framing and output while adopting
  typed array chunks, let chains and `is_multiple_of`. Scoped strict Clippy covers
  MDL/BLP/SYLK libraries, tests and CLIs without dependency lint expansion;
  graphics-library and map all-target scoped Clippy also pass. Map cleanup keeps
  the stronger original version guards; the complete 284-source original fixture
  passes after correction, as does the WASM check.
  Object-data all-target scoped Clippy also passes. Malformed stock-value errors
  own their complete untrimmed text as `Box<str>`; their return signatures and
  diagnostic fields are retained. The existing diagnostic regression, 43 crate
  tests, explicit both-install default-table sweep and WASM check pass.
  Native application Clippy now passes all targets/features with `--no-deps`
  and `-D warnings`; 106 app tests pass, five original fixtures remain explicitly
  ignored by default, and the WASM check passes. Dependency lint debt is separate.

- [!] Latest checked four-target CI at `db9e387` (run `38496520451`) did not start:
      GitHub reports failed account payments or an insufficient spending limit.
      Local strict/native/WASM checks are distinct from this runner blocker;
      verified Windows/macOS artifacts remain at `d31dc67`. No repeated retries.

- [x] Viewer skinning now binds each mesh to its model-instance entity and authored
      GEOS ordinal. Equal vertex counts no longer select another geoset's rest
      positions/bindings. A synthetic ECS regression also preserves unrelated
      model instances; strict package tests and WASM compile check pass.

## Remaining-work dependency map

The current menu baseline preserves source-qualified enumeration rows, labels,
refresh grouping and session selection without a preset resolution list. A
native first-run window fits nine tenths of its reported monitor; explicit saved
resolution intent retains the existing restoration owner. This fraction is a
project presentation policy, not an original game default. Audio-only Apply can
save at an actual reported client size absent from the offered list.

This integration passes 340 application tests, three explicit actual-window
gates, strict native/browser-WASM Clippy, ten display-profile tests executed by
Wasmtime, and warning-denied display documentation. Source-table popup families,
active original mode policy, exclusive switching and equal rendered Options
choices remain open. The current installer engine does not admit the supported
display profile; its successful local extraction does not grant that capability.
Wide and compact main-menu captures pass geometry checks, but the fresh Options
sequence remains incomplete. No new browser package is published by this
checkpoint. Evidence is retained in `.scratch/menu-ui-native-root-20261017/`,
`.scratch/menu-ui-cross-root-20261017/` and
`.scratch/browser-display-profile-root-20261017/`.

The authored depth bits now reach the background and panel material pipeline.
Disabled depth testing also suppresses writes, matching the pinned viewer/OpenGL
profile. All 108 RGB overlap checks and the existing geoset-colour GPU readback
pass on Vulkan; 346 application tests, strict native/browser-WASM Clippy and
warning-denied application documentation pass.
Evidence is in `.scratch/menu-depth-root-20261017/verification-1/`. The matching
private browser package has four reviewed Main/Options captures. Its fifth
resolution-list capture misses the bounded collection deadline, so the complete
interaction gate remains open. Original depth execution, the missing Video
panel artwork, popup text placement and other visual differences remain open.

The selector-2 caller now derives frameless rows from source tables and each
monitor's full bound, with first-occurrence grouping and stable monitor identity.
Losing a selected monitor retains the draft and refuses Apply. Nineteen native
and actual WASI display tests, the supplied Legacy profile gate, 347 application
tests and strict native/browser-WASM checks pass. Evidence is in
`.scratch/options-display-popup-root-20261017/`. Work-area families, original
active policy, exclusive switching and equal rendered choice lists remain open;
browser client-image import and row wiring are now integrated; the conditional
five-row browser gate below passes.

The [101-entry requirements map](docs/remaining-work/README.md) reconciles the
open and partial gates against source evidence and lists their prerequisites.
It is pinned to the pre-correction tracker snapshot; broad gameplay and original
comparison gates remain open.

Browser display input now shares the native image admission parser. The existing
picker accepts one optional local client image with its MPQs; cancel and failed
hydrate keep the previous image/assets/revision, while an archive-only replacement
clears the image. Eight actual generated-WASM transaction checks and a focused
Video Options dropdown run pass. At the retained original screen bound, its five
visible rows match the original witness in order. The screenshot was reviewed
and its capture state is stable. Forty JS tests, 353 app tests, strict native
and browser Clippy and the actual browser build pass. Evidence is retained in
`.scratch/browser-display-rows-root-20261017/`. No original assets were packaged
and nothing was published. Full original mode policy, physical switching,
Options panel/artwork/typography and unsupported settings remain open.


The resolution dropdown now renders the selected MENU's backdrop, border and
text highlight through the existing authored-art path. A fixed clipped plate
surrounds the row scroller. Focus scrolling waits for first layout, starts at the
selected witness, and physical trackpad deltas are converted to logical pixels.
Four popup/system regressions and 361 application tests pass (18 explicit gates
ignored), with strict native/browser Clippy, browser build and formatting. The
fresh local browser run passes source-colour hover/keyboard checks, selection
after reopening against the current rows, Escape and Cancel back to Main; all
three reviewed captures are stable. Evidence is in
`.scratch/resolution-dropdown-art-20261017/root-2/`. The earlier rejected
collector result remains retained. No original assets were packaged or
published. Complete source mode policy, physical switching, font rasterization
and full native Options parity remain open.

A focused dropdown row now stays visible across viewport-size and density
changes; unchanged geometry preserves manual wheel scrolling. The discriminating
persistent-system regression and all 362 app tests pass (18 explicit gates
ignored), with strict native/browser Clippy, browser build and formatting.
Evidence: `.scratch/resolution-focus-resize-20261017/`. Geometry is synthetic;
fresh rendered resize and native behavior remain unmeasured. No font, mode-policy
or physical display switching claim is added.

The category sound-slice regression now calls production panel `advance` and the
real return request, checking spawned players/gains for left category, continued
right Stand and shared Options Death. All 363 app tests and strict native/browser
Clippy pass, with formatting. Evidence:
`.scratch/category-pop-advance-20261017/root-verification/`. Test-only change;
no audible original comparison or full Bevy WASI claim.

A fresh same-source font A/B comparison rejected uniform physical-pixel
truncation: some label dimensions improve, but other reference-matching widths
regress and the heavier fringe remains. Both captures are stable and local;
the experimental code was removed and the prior Rust sources restored exactly.
The candidate's 365 passing tests do not close typography; the restored product
retains its verified 363-test baseline. Evidence:
`.scratch/menu-font-experiment-20261017/decision.json`. Font rasterization,
spacing, archive selection and native visual parity stay open. No publication.

The reference browser capture now preserves emulated screen bounds. A real
scrolled-canvas regression fails on the old clipped CDP path (200x150 becomes
800x600), then passes with the viewport-relative Playwright crop. Exact green
pixels, document-offset receipts, physical inputs, change detection and existing
file refusal still pass. Evidence: `.scratch/browser-capture-screen-20261017/`.
This corrects the comparison harness; it does not establish game or UI parity.

Menu label line boxes now keep paragraph leading out of measured FDF anchor
heights, on both roots and markup spans. A fresh same-source 1280x960 browser
capture reduces the nine left Video label offsets from 6–39 pixels to at most
one pixel against the Wine reference mask, while the static right-button ink
masks remain unchanged. Main and Options captures are stable; the five-row
conditional source list and centered value remain. All 363 app tests, strict
native/browser checks, browser build and formatting pass. Evidence:
`.scratch/options-text-anchors-20261017/`. One-font-size line boxes are a
recreation layout convention; original font metrics, multiline spacing,
rasterization, shading and full native Options parity remain open. No publication.

Owned map configuration now retains its selected resolver in `SelectedMapSources`
with the exact published receipt arc, so gameplay asset preparation can keep the
same reader after the request retires. Failed replacement preserves the reader;
copied or replaced receipts cannot select it. Reloading the same bindings keeps
that association. Actual committed-archive generation changes still refuse reads.
All 78 map-loading tests, four library tests, strict native/browser checks and
browser build pass. Generated 32-bit WebAssembly executes seven JSON and six
consumer groups, including these ownership cases. Evidence:
`.scratch/map-runtime-source-owner-20261017/`. This supplies owned readers through
the existing configuration path; it does not construct a gameplay Host, execute
chapters, freeze external sources or complete terrain/HUD integration.


The admitted-map startup bridge now uses one reusable source-bound Host factory,
with explicit caller-owned rules, banks, budgets, catalog and runtime observations.
Successful delivery runs globals, map-script `config`, post-config admission and
`main` before publishing the candidate session. Factory, post-config and native
refusals preserve the old session. Revision, identity and exact receipt checks
prevent stale outcomes from consuming fresh inputs; file-drop labels remain valid.
Four new real-App cases pass, bringing map-loading coverage to 82 tests. All 376
native and actual WASI play tests, 363 app tests, four app-library tests, strict
native/browser Clippy, docs and formatting pass. The same qualified Human01 input
now uses this production factory and post-config path, retains one arena and still
refuses `TimerStart` at Blizzard.j `447183..447286`. Evidence:
`.scratch/campaign-startup-host-root-20261017/`. Automatic launcher input preparation,
native reader publication, timer scheduling, world/HUD integration and playable
chapters remain open. No new rendered or native gameplay comparison is claimed.


The existing script VM now retains expression continuations for an opt-in Host
native, including partial arguments, operands, indexes, locals, assignments,
returns and conditions. Resume supplies that native's result without repeating
earlier effects. Short-circuit behavior, source diagnostics, cumulative steps and
depth survive. Body snapshots and unfinished argument tails are shared across
returns, with deterministic copy-work regressions and a bounded native-stack
check for a wide call. Localized, neutral and trace wrappers forward the hook.
The synchronous application session still refuses a suspended candidate and
keeps the previous session and its callback tickets.
All 112 native and 111 actual WASI script unit tests, 14 script integration tests
on each target, 378 play unit tests and 65 selected play integration tests on each
target pass. All 83 map-loading, 363 app and four app-library tests, strict
native/browser Clippy, browser build, warning-denied docs and formatting pass.
The qualified Human01 production-factory replay still completes globals/config,
retains 904 identities, 144 rectangles and 155 units and refuses `TimerStart` at
Blizzard.j `447183..447286`. Evidence:
`.scratch/vm-continuation-root-20261017/`; see
`docs/re-notes/script-continuations.md`. Current ObjectHost timer admission,
expiry order, cooperative scheduler, native timing and playable chapters remain
open. No fresh gameplay or rendered comparison is claimed.


Published script-sound starts now have a bounded native Bevy consumer on the same
`MapScriptSession<MapStartupHost>`. Distinct sound handles can share their
encoded source. A failed replacement keeps the current voices and queue;
successful replacement or clear retires that session's voices. Finished
one-shots release their source and can start again. Active-handle repeats are
reported as unsupported until restart-versus-mix behavior is qualified. The
existing effects gain and mute control applies before sink creation. Unsupported
spatial, EAX, fade, duration, channel, volume, pitch and parameter profiles remain
explicit refusals. Browser requests remain queued without a decoder backend.
All 364 app tests, 19 library tests, 85 map-loading and 92 factory/audio integration
tests pass, alongside strict native/browser Clippy, the full browser build,
warning-denied docs and formatting. Installed-decoder tests consume synthetic
PCM and MP3 samples without a device. Evidence:
`.scratch/script-audio-root-20261017/`; see
`docs/re-notes/script-audio-playback.md`. Audio device output, native timing and
mixing, automatic qualified Host inputs, timer scheduling and full campaign
playback remain open. No fresh rendered or audible comparison is claimed.

The script-sound consumer now derives its default staging/cache byte ceilings
from the published Host's already admitted immutable sound banks, including
MIDI labels, creation labels, PCM files and MP3 files. No separate limit resource
or hard-coded default is required on the installed path. An explicit caller
quota remains an override. Inventory reads do not clone sound bytes; repeated
handles cannot enlarge the ceiling. A production-factory regression now omits
the limit resource and verifies two sound handles share one source without
replay. All 379 play tests pass on native and actual WASI. The 364 app, 19 library,
85 map-loading and 92 factory/audio tests, strict native/browser Clippy, full
browser build, warning-denied docs and formatting also pass. Evidence:
`.scratch/script-audio-root-20261017/bank-ceiling-1/`. These are encoded-copy
ceilings, not a total backend decoder/voice memory bound. Native mixing, device
output, automatic qualified Host input preparation and full campaign playback
remain open.


The deterministic point-movement harness can now clamp its existing step against
an explicitly caller-admitted WPM raster. Grid scale is derived from both input
dimensions; origin, tile extent and blocking mask remain explicit caller inputs.
Snapshots share one immutable profile/raster. Existing wire payloads stay the
same, and client reconciliation restores the selected profile before subsequent
steps instead of silently reverting to unadmitted movement. Different explicit
profiles refuse without changing client state. Shared-identity comparisons avoid
raster scans on the usual path, and admission makes one bounded raster copy.
All 44 native simulation and 142 native network tests pass. Fourteen movement
and four reconciliation cases also execute successfully on actual WASI, alongside
379 play tests on native and actual WASI. All 364 app, 19 library, 85 map-loading
and 92 audio/factory tests, strict native/browser Clippy, full browser build,
warning-denied docs and formatting pass. Evidence:
`.scratch/pathing-root-20261017/`; see
`docs/re-notes/admitted-pathing-movement.md`. Original blocking bits, world-axis
mapping, unit footprints/classes, sliding/path search, dynamic pathing and the
application unit world/session clock remain open. This is a tested point-movement
policy, not original-game movement or playable-campaign acceptance.


A fresh local browser package of accepted source `dfd6c02` now passes the bounded
1280×960 Main → blank Options landing → Video → open resolution-list check.
The matching generated WebAssembly and allowlisted static package were rebuilt;
reviewed captures show the left Video plate, both right housings and chains, and
the blue dropdown. The closed resolution value has zero vertical-center error.
The browser reports no errors or request overflow; original MPQ/client inputs
remain local and are absent from the package. Evidence:
`.scratch/browser-refresh-root-20261017/acceptance.json`. This check does not
exercise HUD/tooltips or establish native appearance, physical mode switching,
or resolution offerings beyond this supplied image and screen bound. The public
site was not changed, and the earlier 4K timeout remains open.

The selected Legacy `TimerStart` functional native path is now statically linked
to the game-time queue: its zero selector is forwarded through the timer vtable
to `AGILE_DATA+0x44`. The callback store and timer-message dispatch into the
script executor are also traced. Fourteen checked raw-range digests and operand
checks bind these findings to the selected executable. See
`docs/re-notes/legacy-timer-start.md`. This closes an unread clock-selector
question, not RE-Q5 or timer execution. At that static-evidence step,
`ObjectHost` still refused `TimerStart`;
callback ordering/mutation, scheduler integration, native timing and playable
campaigns remain open. No original runtime or new rendering check was run.

A subsequent isolated Legacy/Wine timer probe completes with sixteen records.
It observes handler replacement/clearing, armed-timer destruction, periodic
self-restart and destruction, expired-timer identity, and a non-yielding expiry
action before the direct handler. Source/staging/log/capture receipts and three
rejected log mutations pass; the owned probe container is stopped and preserved.
See the bounded runtime section of `docs/re-notes/legacy-timer-start.md` and
`.scratch/timer-runtime-root-20261017/attempt-1/`. This is one instrumented Human01
run, not the unmodified-map baseline gate, a native Windows check, universal timer
ordering or a recreation timer implementation. Pause, edge delays, yielding
actions, repeated-run stability and scheduler/world integration remain open.


### 2026-10-17 — explicit positive-timeout timer delivery

- [~] Timer runtime: caller-supplied game time now admits positive finite starts
  on the existing ObjectHost arena. The existing published map VM delivers
  bounded callbacks, retaining restart/destroy effects and clearing error context.
  Factory input is opt-in; no default app clock or second VM is introduced.
- [ ] Timer fidelity remains open: expiry-trigger execution, yielding callbacks,
  pause/speed integration, edge delays, native rounding/tie order and full map
  execution. The 100-consecutive unmodified-original baseline gate is unchanged.
  See [the timer evidence and implementation limits](docs/re-notes/legacy-timer-start.md).
- [~] A fresh bounded timed-trigger reference qualifies condition/action order,
  callback context, disabled periodic phase and self-destruction's active action
  tail. Both registered conditions execute even when the first returns false;
  automatic timed triggers observe a null expired timer. The 22-record probe and
  seven refusal mutations pass; production event delivery is still unsupported.
  See [the reference scope](docs/re-notes/legacy-timer-start.md#timed-trigger-lifecycle-reference-2026-10-18).


### 2026-10-17 — retained player-unit event associations

- [~] The production startup factory can opt into bounded player-unit event
  registrations on its existing ObjectHost arena. Records retain the executed
  trigger, player, source event and optional real Filter identity; predicates
  and actions are not evaluated by registration.
- [~] Unchanged Human01 passes that registration and two positive timer starts
  under an explicit verifier clock origin. Globals and `config` finish; `main`
  next refuses `Condition`. The retained compatibility catalog and startup
  observations remain explicit conditions of this check.
- [ ] Condition/event execution, the application world and clock, rendered
  gameplay, chapter completion and the 100-consecutive original baseline gate
  remain open. See [the registration scope](docs/re-notes/script-trigger-admission.md#player-unit-event-subscriptions).


### 2026-10-17 — retained boolean wrappers and trigger conditions

- [~] Source-bound `Condition` wrappers and `TriggerAddCondition` associations now
  use the existing ObjectHost arena and independent caller budgets. Player-unit
  registrations accept either supported real boolean wrapper. None of these
  operations evaluates a predicate or invents initial trigger-enabled state.
- [~] Unchanged Human01 passes two wrappers and two condition associations through
  the production factory under the retained verifier inputs. Globals and `config`
  finish; `main` next refuses `DisableTrigger` at map span `127485..127527`.
- [ ] Trigger state/evaluation, event delivery, the application clock/world,
  rendered gameplay and the 100-consecutive original baseline remain open.
  See [the supported condition slice](docs/re-notes/script-trigger-admission.md#condition-wrappers-and-trigger-associations).

### 2026-10-17 — explicit trigger enabled state

- [~] Loaded enable/disable/query contracts use the existing trigger admission
  and arena. Only executed assignments establish a value; creation leaves the
  initial state unknown. State changes allocate nothing and run no callbacks.
- [~] Unchanged Human01 passes one `DisableTrigger` through the production factory.
  Globals and `config` finish; `main` next refuses `TriggerRegisterPlayerEvent`
  at Blizzard span `80853..80926`.
- [ ] Initial native enabled state, event delivery, the application world/clock,
  rendered gameplay and the 100-consecutive original baseline remain open.
  See [the explicit-state scope](docs/re-notes/script-trigger-admission.md#explicit-trigger-enabled-state).

### 2026-10-17 — retained player-event subscriptions

- [~] Source-bound `TriggerRegisterPlayerEvent` records now use the existing
  trigger/player/enum arena and an independent caller budget. Registration
  preserves explicit disabled state and runs no action.
- [~] Unchanged Human01 passes one player-event registration through the production
  factory. Globals and `config` finish; `main` next refuses `CreateRegion` at
  Blizzard span `81939..81953`.
- [ ] Event context/delivery, native duplicate-registration identity, the
  application world/clock, rendered gameplay and the 100-consecutive original
  baseline remain open. See [the subscription scope](docs/re-notes/script-trigger-admission.md#player-event-subscriptions).

### 2026-10-17 — retained region requests and entry/leave subscriptions

- [~] Source-bound region creation, rectangle-addition requests and entry/leave
  subscriptions use the existing arena and independent caller limits. Call-time
  bounds and real filter/condition identities are retained; callbacks stay idle.
- [~] Unchanged Human01 passes six regions, additions and entry subscriptions
  through the production factory. Globals and `config` finish; `main` next
  refuses `TriggerRegisterDeathEvent` at map span `184271..184338`.
- [ ] Native cell geometry, membership, event delivery, the application world/clock,
  rendered gameplay, chapter completion and the 100-consecutive original baseline
  remain open. See [the supported region slice](docs/re-notes/script-regions.md).

### 2026-10-17 — retained widget death subscriptions

- [~] Source-bound `TriggerRegisterDeathEvent` records use the existing arena for
  actual units and destructables with a separate caller limit. Registration
  preserves disabled state and runs no callback. Item widgets remain unsupported.
- [~] Unchanged Human01 passes three destructable death subscriptions through the
  production factory. Globals and `config` finish; `main` next refuses
  `GetRandomReal` at map span `186558..186584`.
- [ ] Death detection/context/delivery, native random-stream fidelity, the
  application world/clock, rendered gameplay and the 100-consecutive original
  baseline remain open. See [the subscription scope](docs/re-notes/script-trigger-admission.md#widget-death-subscriptions).

### 2026-10-17 — selected primary random-generator evidence

- [~] Static registration/handler tracing on the selected executable identifies
  the shared primary draw helper and a separate secondary-state seed path.
  Bounded offline selected-instruction emulation matches all 80 retained integer
  observations from the two original probe runs, including post-real draws.
- [~] The retained unchanged Human01 prefix has no executed JASS random call or
  seed setter before its first `GetRandomReal` attempt. Engine-internal prior
  consumption and original initial state are not observed by that trace.
- [ ] Production `GetRandomReal` remains unsupported. Real arithmetic,
  secondary-state effects, initial-state admission, paired recreation execution
  and the 100-consecutive original baseline remain open. See
  [the evidence boundaries](docs/re-notes/original-rng-control.md#selected-primary-generator-static-and-retained-output-checks).

### 2026-10-17 — selected random-real arithmetic evidence

- [~] Offline selected-wrapper execution matches 16 retained formatted real
  observations and preserves the 80 integer checkpoints. Original real bits
  remain unobserved. Six interpreter self-checks and eleven synthetic
  edge/refusal checks pass.
- [~] At bounds derived from the Human01 receipt, ordinary `f32` multiply/add
  differs from the selected wrapper for 32 of 64 synthetic seeds. The selected
  arithmetic must be preserved rather than assumed equivalent.
- [ ] Original initial state, prior engine consumption and secondary streams
  remain open; production `GetRandomReal` stays unsupported. Paired execution,
  native bit-exact real output and the 100-consecutive baseline are not certified.
  See [the real-wrapper scope](docs/re-notes/original-rng-control.md#selected-real-wrapper-arithmetic-follow-up).


### 2026-10-18 — receipt-bound native source readers

Resolver-backed native requests and file drops now publish the same reader with
its exact receipt arc. Refused candidates preserve the previous publication;
copied receipts cannot select a reader. `bind_map` does not retain a resolver,
so automatic native input preparation remains open for that path. Browser inbox
first publication and replacement retain their atomic commit and retirement rules.

Validation passes 95 map-loading, 19 library and 394 application tests, plus 13
configuration checks executed from actual Cargo WebAssembly. Strict native/browser
Clippy, browser build, docs and formatting pass. The 21 explicitly ignored app
tests, native visual/gameplay comparison, world/HUD integration, chapter completion
and the 100-consecutive unmodified-original baseline remain open. Evidence:
`.scratch/native-resolver-root-20261018/review-3/acceptance.json`.


### 2026-10-18 — executed unit identity getters

`GetUnitTypeId` and `GetOwningPlayer` now read admitted `CreateUnit` records on
the existing arena. Loaded declarations must match the source and signature;
constant and non-constant declarations are supported. Omitted declarations remain
unsupported. Getters neither allocate identities nor query the catalog again.

Validation passes 29 unit-creation tests, 423 native and 423 actual WASI play
tests (six explicit ignores per target), and 95 map-loading tests. Strict
native/browser Clippy, browser application build, docs and formatting pass. The
original Human01 factory returns the executed rawcode and original owner handle
for all 155 created units. Globals and `config` complete; `main` still stops at
`GetRandomReal`. The retained compatibility catalog and explicit observations do
not establish native Legacy archive priority. Owner changes, world association,
chapter completion, native/render parity and the 100-consecutive unmodified-original
baseline remain open. Evidence:
`.scratch/unit-identities-root-20261018/review-1/acceptance.json`.


### 2026-10-18 — published-session timer lifetime regression

A new production-factory/session regression destroys the expired timer and starts
another in the same callback. The new timer retains its own elapsed/remaining
values; its later null-handler expiry consumes one delivery without rerunning the
callback. Production timer and session code are unchanged.

All six timer-session cases pass natively and execute from the actual Cargo
WebAssembly integration artifact under Node. All 96 native map-loading tests,
strict native/browser Clippy and formatting pass. No original-client runtime,
rendering or timing comparison was repeated; automatic application clock, yielding
callbacks, matching expiry-trigger delivery and the 100-consecutive original
baseline remain open. Evidence:
`.scratch/timer-lifecycle-root-20261018/review-1/acceptance.json`.


### 2026-10-18 — archive revision and reader retirement coverage

A committed-generation fixture now swaps payloads with identical archive metadata
and opened tables. The old reader refuses; a new publication selects the new
reader and retires the old receipt association. Resolver-source destruction also
reenters the actual inbox successfully. Production behavior is unchanged.

Validation passes 97 native map-loading and 19 library tests, 14 configuration
groups from actual Cargo WebAssembly, and the application's real import
begin/refused-commit/cancel test from application WebAssembly. Strict
native/browser Clippy and formatting pass. Full successful UI hydration, browser
interaction, campaign/HUD/world integration, native/render parity and the
100-consecutive unmodified-original baseline remain open. Evidence:
`.scratch/archive-reader-root-20261018/review-1/acceptance.json`.

### 2026-10-18 — terrain publication after budget correction

Terrain publication now retries the same admitted map when its caller corrects
an insufficient read or texture budget. Unchanged inputs keep their draw; a
temporarily missing budget preserves the mounted entity and can recover without
another map delivery. Asset-derived terrain values and rendering are unchanged.

Validation passes 98 native map-loading and 19 library tests, three terrain
publication cases executed from actual Cargo WebAssembly, strict native/browser
Clippy and formatting. Automatic launcher budgets, gameplay world/camera wiring,
native/render parity and the 100-consecutive original baseline remain open.
Evidence: `.scratch/terrain-budget-root-20261018/review-1/acceptance.json`.

### 2026-10-18 — explicit recovery from campaign startup refusal

Fresh caller host inputs can retry a failed factory, post-config or execution
candidate on the same admitted map. Stale revisions preserve those inputs and
the previous session. Ordinary updates retain the failure without replay, and
inputs alone do not restart a successful session. Successful recovery retires
the previous callback tickets.

Validation passes 99 native map-loading tests, five production-startup cases
executed from actual Cargo WebAssembly, strict native/browser Clippy and
formatting. No original-client runtime or rendered comparison was repeated;
automatic host-input production, chapter completion and the 100-consecutive
original baseline remain open. Evidence:
`.scratch/startup-retry-root-20261018/review-1/acceptance.json`.

### 2026-10-18 — current-source private browser smoke

A private browser package rebuilt from `d6c837e` passes a bounded 1280×960 run
with locally imported client/MPQ inputs. Reviewed captures cover Main, the blank
Options landing, Video and the five-row resolution list. The closed resolution
value has zero vertical-center error. All 17 requests stay local, with no page
or console errors; four WebGL readback-stall warnings remain in the evidence.

The 26-file package includes code and decoder files only. Original assets remain
local and are neither packaged nor uploaded. No product code changed. This is
not a performance, native-parity, full accessibility or gameplay/HUD acceptance.
The public site and unresolved 4K capture remain unchanged, as does the
100-consecutive original baseline gate. Evidence:
`.scratch/browser-refresh-root-20261018/review-1/acceptance.json`.

### 2026-10-18 — browser configuration form admission

The real browser form now has a verified successful path through local file
selection, archive inspection, metadata decoding, the production WASM inbox and
the App map-admission consumer. Five submissions cover an insufficient metadata
byte limit, first admission, invalid UTF-8, JSON syntax failure and a second
admission. Controls recover after each refusal; successful requests receive
distinct IDs while the committed archive revision stays unchanged.

The admitted map, common/BJ scripts and catalog are existing wholly synthetic
fixtures. A pre-existing local menu closure supplies import hydration. All 17
requests stay local, with no page or console errors. Two earlier harness setup
failures are preserved separately; no product code changed. This does not prove
original-map startup, gameplay/world/HUD behavior or full hydration across source
sets. Public-site freshness, 4K and the original baseline remain open. Evidence:
`.scratch/browser-map-form-root-20261018/review-3/acceptance.json`.


### 2026-10-18 — bounded quests in the production startup factory

The caller can opt into the existing quest session with
`MapStartupBudgets::quests`. Quest, item and timer identities share the factory
Host's existing arena; globals, `config`, `main` and later callbacks use the same
published VM. Omitting the budget leaves quest natives unsupported. Exhausted
quest/item/text/event budgets and missing declarations refuse replacement while
preserving the active quest state, timers and callback. Successful replacement
retires the previous callback ticket; raw handle tokens remain session-local.

Two delivered-map cases fail before the factory wiring and pass afterward,
including execution from the actual browser-target Cargo WASM artifact. All 101
native map-loading tests, 19 app-library tests, 423 native and 423 actual WASI play
tests pass; each play run retains six explicit local-original-input ignores.
Strict native/browser Clippy, affected native/WASM compilation and formatting
pass. Evidence: `.scratch/quest-factory-root-20261018/review-1/acceptance.json`.

The existing constructor flags retain their documented Legacy runtime scope;
this change assigns no new defaults and verifies no additional executable. No
original runtime or rendered comparison was repeated. Automatic Host-input
preparation, quest-window rendering, campaign completion and the original-map
acceptance gate remain open.


### 2026-10-18 — chronological client rollback history

The existing movement Client now records the authoritative correction base
before replay, and recording an earlier history tick discards the abandoned
newer tail. Previously replay appended duplicate, out-of-order frames, evicting
still-replayable history and leaving old worlds available to later corrections.
The repair uses the existing history ring and its retained RNG frame.

A ring regression and an actual server/wire-codec/Client regression both fail
before the fix. The latter checks two successive corrections under four- and
eight-frame limits, including every retained world, selection, targets, exact
replay counts and capacity. All 144 native and 113 actual WASI network tests pass,
as do strict native/browser all-target/all-feature Clippy, the browser-target
build and formatting. Evidence:
`.scratch/reconcile-history-root-20261018/review-1/acceptance.json`.

This repairs movement-harness reconciliation. It does not supply the pending
selected-unit application caller or restore map VM, combat, economy or other
omitted authoritative state. Original movement/protocol fidelity, rendered/HUD
parity, full campaign completion and the 100-consecutive original baseline gate
remain open. No original runtime or browser visual run was repeated.


### 2026-10-18 — full-range snapshot divergence arithmetic

A valid decoded snapshot with opposite signed coordinate extremes previously
panicked in Client correction while computing its positional divergence. The
metric now uses full-range standard-library absolute differences, widens axis
totals to `u64` before addition and saturates the aggregate report at `u64::MAX`.
The existing snapshot format, coordinate domain and correction rules are kept.

The real decode-then-Client regression fails before the repair and passes across
four extreme-coordinate pairs, two units and reversed record order. All 145
native and 114 actual WASI network tests, strict native/browser Clippy with all
targets/features, the browser-target build and formatting pass. Evidence:
`.scratch/reconcile-coordinate-root-20261018/review-1/acceptance.json`. This is a
movement-harness diagnostic/correction repair, not original map extent, movement,
protocol, gameplay or rendered parity evidence. Original runtime/browser visual
runs and the 100-consecutive original baseline gate remain open.


### 2026-10-18 — bounded catalog-backed script item creation

`MapStartupBudgets::items` now opts the production factory into `CreateItem`.
The call resolves the selected catalog's item definition and retains its
metadata/modifications, explicit map Source and executed coordinates on the same
handle arena as timers and units. Omitting the budget preserves unsupported
behavior. No item owner, life, inventory or world/render defaults are inferred.
Catalog/family and record/handle/field/modification refusals allocate no item;
failed startup candidates preserve the previous published session.

Five core cases and two delivered-map App cases pass. The latter parse synthetic
ItemData and metadata tables through the real selected resolver/binding path,
check the retained item cell, interleave timer handles, exercise refusals and
retire the old session on valid replacement. Both cases also execute from the
actual browser-target Cargo WASM artifact. All 103 native map-loading tests,
19 app-library tests and 428 native plus 428 actual WASI play tests pass; each
play run retains six explicit local-original-input ignores. Strict native/browser
Clippy, affected native/WASM compilation and formatting pass. Evidence:
`.scratch/item-creation-root-20261018/review-1/acceptance.json`; scope:
`docs/re-notes/script-items.md`.

This establishes admission and executed item records. Other item natives,
inventory, death events, world publication, rendered items, original behavior
comparison, playable campaigns and the 100-consecutive original baseline gate
remain open. No original runtime or browser visual run was repeated; the public
site package was not rebuilt or changed.


### 2026-10-18 — shared browser script-sound playback

Browser `StartSound` requests now use the existing native playback consumer,
source cache, quotas and published-session lifetime. Both targets enable the
existing Bevy WAV/MP3 features. The existing effects gain/mute system and its
effective-gain helpers now also run on browser script voices after settings and
playback updates. Native menu loading and the browser menu audio path retain
their existing behavior; no new decoder or dependency was added.

The actual before-change browser WASM session gate loads, then fails because no
voices are published. All 21 explicit WASM cases pass afterward: 11
decoder/lifecycle cases, nine delivered-session cases and one registered gain
case. All 19 native app-library tests, 109 native session tests and six native
audio-control tests pass. One explicit original-install audio test remains
ignored, and 408 other app tests were filtered out. Strict native/browser
all-target Clippy, production browser compilation and formatting pass with
unchanged source hashes during validation and an unchanged lockfile. Evidence:
`.scratch/browser-script-audio-root-20261018/review-3/acceptance.json`;
scope: `docs/re-notes/script-audio-playback.md`.

Actual browser `AudioPlugin` output is unverified: loopback binding is denied,
Chromium exits after a socket-permission error, and the connected browser
inventory is empty. This acceptance remains tracked separately. No original
runtime or browser visual run was repeated, and the public site package was not
rebuilt. Original sound levels, mixing/timing, unsupported sound profiles,
playable campaigns, rendered parity and the 100-consecutive original baseline
gate remain open.


### 2026-10-18 — browser script-audio gesture unlock

After the environment permissions changed, the shared playback change was
published as `16adf4d` and confirmed on origin. A caller-faithful Chromium check
then found that Bevy's audio context, created before interaction, stayed
suspended after a click. The existing menu gesture handler now resumes live
Web Audio contexts before its HTML-media guards and releases closed contexts.
This changes no native playback, asset values or sound gain/mute policy.

The actual before-change browser artifact/module stays suspended after a trusted
click with two queued-to-player voices that cannot finish. With the fix, fresh
PCM and MP3 documents begin unactivated with suspended contexts, then produce
nonzero Web Audio output after a trusted click. The one-shot retires naturally,
the loop keeps playing, and session clear leaves no voice or source. Passive
pre-click reads explicitly avoid the automation driver's user-gesture behavior.
Nine published-session cases execute in each document. Eleven JavaScript audio
tests, 109 native session tests, strict native/browser Clippy and formatting pass
with stable source hashes. Evidence:
`.scratch/browser-script-audio-root-20261018/device-6-after/acceptance.json`.

Physical audibility, original mixing/timing and sound levels, unsupported sound
profiles, playable campaigns, rendered parity and the 100-consecutive original
baseline gate remain open. No original runtime or browser visual comparison was
repeated, and the public site package was not rebuilt.


### 2026-10-18 — source-bound explicit script random state

- [~] The production object host and startup factory accept an optional reviewed
  local-image policy and bounded secondary storage. Exact declarations, seed
  packing, draw lanes/table, real parameters and direct-call relationships are
  checked. No initial seed is supplied; ordinary unseeded draws refuse.
- [~] Native and actual WASI each pass 443 unit cases and four explicit local
  RNG gates: 1,815 selected-code arithmetic cases, profile refusal mutants,
  2,842 host checkpoints, and the retained 80 integer/16 formatted original real
  observations. Native map loading passes 103 cases. Actual Chromium runs four
  factory publication/replacement cases from Cargo WebAssembly. Strict
  native/browser Clippy and formatting pass.
- [ ] Human01's initial engine state, prior/secondary engine consumers,
  exceptional native reals, original real bits, ordered paired gameplay and the
  100-consecutive original baseline remain open. The public site was not rebuilt.
  See [the supported scope](docs/re-notes/original-rng-control.md#explicit-seeded-production-host-2026-10-18).

### 2026-10-18 — owned timed-trigger delivery

- [~] Explicit trigger runtime input now admits timed-event registration,
  destruction and non-yielding condition/action delivery on the existing arena,
  clock and published VM. Event identity derives from the selected constant;
  the initial enabled state is an explicit caller observation. Omission preserves
  unsupported behavior. Disabled periodic phase, active action tails and context
  cleanup follow the qualified reference cases.
- [~] Native and actual WASI pass 446 Host tests; native application-library and
  map-loading suites pass 19 and 107 cases. Actual Cargo browser WASM executes
  four new timed-trigger and six existing timer cases. Strict native/browser
  Clippy and formatting pass. The unchanged Human01 factory passes registration
  and reaches `TriggerRegisterUnitInRange`; the four-result RNG gate remains
  failed after one exact result on native/WASI.
- [ ] Unit/world range-event delivery, engine RNG initialization, yielding and
  callback-list mutation semantics, native timing precision, application clock,
  rendered gameplay and the 100-consecutive baseline remain open. See
  [the scope and evidence](docs/re-notes/legacy-timer-start.md#owned-timed-trigger-consumer-2026-10-18).

### 2026-10-18 — live unit placement and range-event reference

- [x] Declared `BlzGetUnitCollisionSize` reads the existing unit's retained
  catalog geometry through checked metadata, without copied collision values or
  another catalog lookup. Missing, ambiguous or unusable geometry refuses.
  Selected class relations and the parsed footman row agree with the retained
  runtime query; unusual override clamping and runtime mutation remain open.
- [~] One fresh instrumented reference completes 51 records, with eight mutated
  logs refused. It distinguishes range notifications from `IsUnitInRange`,
  observes deferred entry and same-callback movement coalescing, and records
  filter/condition/action context plus filter work after trigger destruction.
- [~] A separate 19-request initialization probe retains 57 creation/setter
  records and refuses eight mutated logs. Creation agrees with `SetUnitPosition`
  in this sample, while axis setters preserve requested coordinates. Bounds and
  114 cliff-level queries join to the parsed terrain. A limited terrain/wall
  replay initially matches only 14 requests. One accepted
  creation is reported blocked by the terrain predicate, so that predicate
  cannot become the placement test. The original container is stopped; the
  capture shows its loading screen, not recreation rendering.
- [~] Source movement-token decoding now supplies the selected creation masks.
  Finite-search failure behavior plus additional source object/building
  footprints explain the five replay misses. A fresh run repeats the same 19
  coordinates against saved predictions: 19 creation positions, 114 WPM
  predicate queries and 114 cliff queries match. The explicit source building
  binding is verified and 16 mutated logs refuse. This is repeated-coordinate
  evidence; automatic placed-object activation, asymmetric/rotated footprints,
  complete occupancy/lifetime and platform overlays remain unqualified. The
  private replay is not a production world or placement default.
- [x] Optional `IsTerrainPathable` uses the existing admitted map's WPM cells
  with masks and scale decoded from fingerprinted executable windows. Loaded
  signature/conversion source, symbolic handle family, arithmetic and raster
  bounds are checked; out-of-raster queries explicitly refuse. The actual
  startup host matches all 114 retained reference queries in native and WASI
  runs. All 454 library cases pass on both targets (six explicit ignores), plus
  107 native map-loading cases (one ignore), strict native/browser Clippy and
  affected library builds. This admits no mutation, dynamic occupancy or live
  placement. The full-map four-result RNG gate remains failed at one result.
- [~] A fresh source-predicted census verifies 1,422 automatic destructables
  retain their identities and state, then five explicit source globals create
  five new handles. A separate source-tagged unit stream records all 155 map
  constructor calls across 53 rawcodes. One hundred getter poses differ from
  post-native argument snapshots; 146 fit quarter-unit flooring and nine move
  further. A subsequent 32-case probe isolates 26 compiled-literal differences
  before native creation, using scaled integer checks. Real equality accepts
  those differing pairs while ordering distinguishes them. Exact conversion
  and equality policies remain open. Two incomplete unit traces remain rejected;
  accepted datasets refuse eight mutants each and all containers are stopped.
  This changes the required world inputs, not production placement behavior.
- [~] Selected numeric execution now reads reciprocal data and an equality
  threshold from the fingerprinted image. The owned startup VM matches 32 exact
  literal controls, 465 formatted inputs and 72 comparison results on native
  and actual WASI; all 155 original constructor argument sets also agree.
  Real equality and inequality retain their distinct native behavior. The
  full-map startup RNG gate still fails at one of four results; live placement,
  declared-real coercions, other numeric natives and non-normal values remain open.
- [~] The reference harness now generates one explicit seeded serial JASS RNG
  fixture for both targets. Two fresh original runs, two native owned VMs and
  two actual WASI VMs agree on all 60 ordered records, including exact real bits.
  Source/probe fingerprints, seed acknowledgments, call order, arguments and
  repeated outputs are checked; a local side-by-side report identifies the first
  divergence. This does not synchronize world ticks, other engine consumers,
  animation or rendering. Visual seeded replay, unchanged-map startup and the
  100-consecutive original baseline remain open.
- [~] DST now has a selected-RNG timer scheduling gate: exact callback/RNG states
  must survive different polling/catch-up partitions, delivery budgets and bounded
  CPU work. Logical time drives the production VM/timer queue; no FPS or elapsed
  threshold decides success. Original timer ordering, the application clock,
  all engine consumers and full gameplay/render lockstep remain open.
- [ ] Queried initial positions do not preserve the requested creation geometry.
  Accepted live placement must precede a world-coordinate consumer. Parsed
  placement records and movement-harness ids do not establish the script unit's
  world identity. No placeholder join or range registration is added; the
  unchanged-map four-result RNG gate remains failed after one exact result.
  See [the evidence and limits](docs/re-notes/script-unit-world.md).

- [x] Five-resolution three-application diagnostic (2026-10-18): original WC3/Wine, actual native PC3 and actual WASM PC3 each receive the same eleven-event journal at 1024×768, 1280×960, 1920×1080, 2560×1440 and 3840×2160. Sixty raw PNGs retain exact extents and sixty pairwise RGB comparisons. Capture and pointer-delivery checks are corrected; 46 Python and four Node checks pass. Evidence: `.scratch/menu-resolution-transparency-root-20261018/verification-final-1.json`; local report: `comparison-1/index.html`. This completes the diagnostic scope, not parity.
- [~] Resolution/transparency fidelity: PC3 panels remain paler; fog, effects, missing settings defaults and notices differ. The baseline 4K WASM Video checkpoint remained on Options landing because input arrived during an active transition; the panel-clock correction below passes that navigation check. Other baseline WASM captures also include moving panels. No alpha constants were changed, no pixels excluded, no simulation/animation lockstep or load-independence claim, and no public-site rebuild. See `docs/re-notes/original-game-vm.md`.

- [x] Menu panel elapsed-time correction (2026-10-18): a deterministic production-system regression reproduces virtual-clock frame loss and missing sound events. Panels now use uncapped presentation time and advance before new navigation requests, after source invalidation. Frame-partition and virtual-pause checks, full application tests (645 pass, 23 explicit ignores) and strict native/WASM Clippy pass. Fresh native and WASM 3840×2160 runs complete the same eleven-event journal: Main, Options landing, Video and return are visually reviewed, and all four WASM after-capture states are settled. Twelve pairwise pixel comparisons still differ. Simulation, background/particle clocks, RNG and transparency remain separate gates. Evidence: `.scratch/menu-panel-clock-root-20261018/`.

- [x] Native harness pointer-query boundary correction (2026-10-18): the read-only position query is now admitted by the real display-command filter, and a failed move invalidates the stored point before any later button press. The regression failed before correction; 47 Python harness tests and four Node tests pass afterward. A fresh native 4K journal exercises the actual filter successfully. The earlier pre-journal refusal remains in private evidence.

- [x] Current-build five-resolution navigation follow-up (2026-10-18): the corrected native/WASM application builds complete Main → Options landing → Video → Main at 1024×768, 1280×960, 1920×1080 and 2560×1440 as well as the verified 3840×2160 follow-up. Thirty-two fresh captures, eight retained current-build 4K captures and twenty retained original WC3/Wine references produce sixty exact-size captures and sixty differing pairwise comparisons. All twenty browser after-capture states are settled; snapshots still change during capture, so animation lockstep is unproved. Source/build/hash/geometry checks, 47 Python and four Node harness checks pass; all fresh runtimes are stopped. No alpha adjustment, pixel exclusion, public deployment or full fidelity claim. Evidence: `.scratch/menu-resolution-current-root-20261018/verification-final-1.json`; report: `comparison-1/index.html`. Transparency, fog/effects, unavailable settings, notices, native Windows and gameplay determinism remain open.


- [x] Menu working-channel correction (2026-10-18): a complete fresh original
  Wine/D3D9 API interval supports numeric sampling/blending. Geometry, PRE2,
  source colours, fog, ordinary UI and custom UI materials now share that path,
  with the final display conversion applied once. Authored alpha and texture
  bytes are unchanged. Actual WebGL caught a missing alternate-view capability;
  the guarded vendored Bevy descriptor retains the same pixel format. Renderer
  errors logged through the Rust console now fail the comparison harness.
  Validation: 647 application tests, 24 explicit ignores, strict native/WASM
  Clippy/builds/fmt, five GPU checks on Radeon and llvmpipe, 47 Python and five
  Node/browser checks. Evidence and limits: `docs/re-notes/menu-working-colour.md`.
- [x] Corrected-composition five-resolution diagnostic: forty fresh native/WASM
  captures and twenty retained original WC3/Wine references cover Main, blank
  Options landing, Video and return at 1024×768, 1280×960, 1920×1080, 2560×1440
  and 3840×2160. All twenty browser after-capture states are settled, without
  renderer errors; all owned processes are stopped. The native visual run uses
  an isolated null audio output. Report and verification:
  `.scratch/menu-composition-root-20261018/comparison-2/index.html` and
  `verification-final-2.json`.
- [~] Exact menu fidelity remains open: all sixty pairwise full-frame RGB
  comparisons still differ. No pixels are excluded or rescaled; the permitted
  version-string footprint is still unqualified. Fog, lighting, particles,
  settings defaults/notices, native Windows rendering and deterministic
  simulation/animation lockstep remain acceptance gates. No public-site update
  or 100-consecutive original-baseline closure is claimed.

- [x] Menu fog depth correction (2026-10-18): geometry and PRE2 now
  share camera-depth linear fog, preserving source settings and the existing
  particle-only black-fog convention. Retained same-executable evidence supports
  this cross-run reconstruction. Six GPU tests each on Radeon/llvmpipe, 647 ordinary tests,
  strict native/WASM Clippy, both builds, formatting and local browser packaging
  pass. After session access was restored, the same frozen candidate completed
  forty fresh native/browser captures at five resolutions through 4K, with
  settled navigation and no browser render errors. Initial socket refusals are
  retained. See `docs/re-notes/menu-authored-fog.md` and
  `.scratch/menu-fog-depth-root-20261018/verification-final-2.json`.
- [~] Exact visual parity still fails all sixty RGB comparisons against retained
  original captures and between PC3 targets. No pixels are excluded or rescaled.
  Lighting/effects, source settings, original/native/WASM simulation and
  animation lockstep remain open; public site and original-baseline gate unchanged.

- [x] Source-bound menu lighting implementation (2026-10-19): replaces lumen
  multipliers and the white ambient floor with source-parameter Gouraud vertex
  lighting before texture modulation. The reviewed client supplies attenuation,
  range, byte scale and light-slot limit on both native and WASM. Missing images
  report a notice while independent panels remain usable. ISO client-image
  selection recognizes file-version suffixes without discarding provenance or
  choosing between ambiguous versions. See `docs/re-notes/menu-source-lighting.md`
  and private terminal receipts in `.scratch/menu-lighting-root-20261018/`.
  The frozen candidate passes 653 ordinary application tests, strict native/WASM
  checks, actual WASI parser/parameter execution, eleven ISO selection cases,
  missing-image panel recovery and five offscreen source backgrounds through 4K.
- [x] Source lighting and contextual menu guidance verification (2026-10-19):
  after access restoration, seven hardware GPU cases, 654 application tests,
  strict native/WASM checks, both builds and local browser packaging pass.
  Passive display/preview guidance no longer covers idle Main or Options views;
  errors retain priority and live resolution hover/unhover still shows help.
  Forty fresh native/browser journal captures at five resolutions through 4K
  are compared with twenty retained original WC3/Wine captures. Early invalid
  or partial preparation attempts are retained, excluded from accepted captures.
  See `.scratch/menu-notice-scope-root-20261019/verification-final-2.json`.
- [~] Exact menu appearance remains open: all sixty full-frame RGB comparisons
  differ. No pixels are excluded or rescaled. Light rays/effects, supported
  settings and initialization, cursor/font details, native Windows and original/
  native/WASM deterministic lockstep still need work. The public site and the
  100-consecutive original-baseline gate are unchanged.
- [x] Source-bound particle appearance calculation (2026-10-19): the qualified
  client supplies biased phase, RGB endpoint scale and byte-packing operands to
  live native/WASM menu preparation. Color/opacity, size and inclusive/reversed
  atlas intervals share the reviewed phase; tail intervals share head repeats.
  Model and client-appearance provenance remain separate, preserving texture
  ownership. Synthetic/live binding regressions, 162 graphics, 575 playback and
  655 application tests, strict native/WASM checks, both builds, 41 actual WASI
  module tests and explicit selected-source gates pass. Bounded instruction
  interpretation matches 462 color/size and 455 complete atlas samples.
  See `docs/re-notes/menu-particles.md` and private evidence under
  `.scratch/menu-particle-fragment-root-20261019/`.
- [~] Full particle/menu parity remains open: ten fresh native/browser journals
  pass at five extents through 4K, but all sixty unscaled RGB comparisons with
  retained WC3/Wine frames differ. Rays/density, background/contrast, settings,
  cursor/focus, original RNG/ages/phase and native Windows need further work.
  No fresh original runtime or public deployment; the deterministic three-target,
  campaign/custom-map and 100-consecutive original-baseline gates remain open.
- [~] Panel composition qualification (2026-10-19): sixty bounded named reads
  find all twelve TFT panel model/texture inputs byte-identical to present
  candidates in the four selected Legacy MPQs. All 136 panel geosets have
  single-matrix groups. Thirty-one native instruction checks and six bounded
  helper cases establish a backend branch that combines alpha testing and
  blending, unlike the current Mask convention. The parsed-layer/instance-to-
  backend selector join was still incomplete at that stage; compatible draw counts are not
  unique material attribution. No production opacity/render change or new
  runtime acceptance is claimed. See `docs/re-notes/menu-alpha-test.md` and
  `.scratch/menu-panel-input-root-20261019/`.

- [x] Default native menu cutout binding (2026-10-19): the MDX filter-to-state
  packet join is qualified with 55 instruction checks and seven bounded cases.
  The supplied client supplies the byte cutoff; background and panel geometry
  preserve surviving source alpha for RGB blending while retaining cutout depth
  writes. The previous opaque-survivor GPU regression fails before and passes
  after the fix. 162 graphics, 576 playback and 657 application tests, strict
  native/WASM checks/builds, five actual WASI and nine synthetic GPU tests pass,
  along with explicit selected-source and offscreen background gates. This
  closes the previous default-selector gap; arbitrary instance overrides remain
  separate. See `docs/re-notes/menu-alpha-test.md` and private receipts under
  `.scratch/menu-native-cutout-root-20261019/`.
- [~] Full menu opacity/output parity remains open: five calibrated native and
  five trusted browser navigation journals pass through 3840×2160, but all sixty
  full-frame RGB comparisons against twenty retained WC3/Wine captures differ.
  The Video left panel remains darker; particle rays/phase, settings, focus and
  native Windows still differ or remain unverified. No version-pixel mask,
  fresh original runtime, public deployment or deterministic three-target
  simulation acceptance is claimed. The 100-consecutive original-baseline,
  campaign/custom-map and overall fidelity gates remain open.


### 2026-10-19 — campaign application clock and initialization callbacks

- [~] The launcher drives a published campaign VM with fixed logical steps and
  retained catch-up work. Loading time is excluded; pause, replacement and callback
  failure have explicit behavior. This scheduling policy is not the original tick
  or game-speed proof.
- [~] Direct initialization triggers call their retained script callbacks in the
  owning VM. Nested contexts and error cleanup preserve prior effects. The bounded
  range-registration candidate is reviewed and integrated; it does not deliver
  world range events or change placement.
- [~] The local Human01 candidate now enters its initialization action and refuses
  `SetCampaignMenuRace`, after globals/config and 155 executed unit creations. Its
  explicit recreation seed and labelled compatibility catalog do not establish an
  original startup match. Native and Cargo WebAssembly checks are retained in
  `.scratch/campaign-runtime-root-20261019/`.
- [ ] Automatic startup inputs, live scene/unit/control wiring, remaining mission
  natives, original seed/call order, complete campaign playability and the
  100-consecutive original baseline gate remain open. No menu visual comparison,
  original runtime or public site deployment was repeated. See
  [campaign runtime](docs/re-notes/campaign-runtime.md).

### 2026-10-19 — executed campaign camera connection

- [~] The launcher consumes instant camera-setup applications from the published
  VM, using its actual initialized field handles, selected-source near clipping
  and matching admitted terrain. Camera/ground render separately from menu
  geometry; view suspension preserves explicit simulation pause. Timed camera
  applications remain unsupported.
- [ ] First Human campaign playability remains open. Automatic startup inputs,
  remaining initialization natives, actor rendering/control and original/native/
  WASM gameplay projection and simulation comparisons are still required.
  No original runtime, fresh gameplay capture, public deployment or
  100-consecutive original-baseline acceptance is claimed by this change.

- [~] Camera increment checks pass: selected local config, native suites, strict
  native/WASM checks and builds, 17 actual Cargo WASM integration cases and nine
  targeted WASI cases. A bounded native application startup reaches its Vulkan
  window without a scheduling panic; this is not rendered campaign acceptance.
  Receipts are local in `.scratch/campaign-camera-root-20261019/`.
### 2026-10-19 — campaign local-player camera branch

- [x] Explicit post-config local slot reaches the production Host and published VM;
  `GetLocalPlayer` aliases the ordinary lazy `Player` identity. No slot/controller
  or participation default is guessed.
- [x] Exact signature, source, domain and context checks preserve opt-in behavior.
  Tests cover both sides of the application camera branch and session isolation.
- [x] Native checks pass: 473 Host-library, 20 application-library and 115
  map/session tests; explicit original-input ignores remain. Strict native/WASM
  lint and builds, 23 actual Cargo WASM cases and five targeted WASI cases pass.
  Receipts are local in `.scratch/campaign-local-player-root-20261019/`.
- [ ] First Human campaign remains unfinished: automatic Host inputs, the last
  observed `SetCampaignMenuRace` refusal and later mission natives, actors and
  controls. Original/native/WASM gameplay and visual parity remain open.

### 2026-10-19 — automatic campaign lighting inputs

- [x] Campaign startup attaches its exact binding reader to the Host; executed
  day/night paths load and validate both source models before changing selection.
  Existing bindings are reused; new source work is bounded per session. Failed
  replacement keeps the prior VM, lighting and source reader.
- [x] Native suites, strict native/WASM lint/builds, 27 actual Cargo WASM cases
  and ten targeted WASI cases pass. The actual Human01 map arguments load both
  selected original lighting members with matching retained hashes. No mission
  or original game was run. Evidence: `.scratch/campaign-lighting-root-20261019/`.
- [ ] Other automatic startup inputs, `SetCampaignMenuRace` and later mission
  natives, world lighting, actors/controls and complete first-chapter playability
  remain open. This is source preparation, not rendered or original-game parity.

### 2026-10-19 — automatic launcher ground budgets

- [x] Launcher supplies fixed application read/texture ceilings after map
  delivery, using the existing exact-reader terrain publisher. Explicit supply
  callers remain supported. Replacement failures keep the prior mounted source.
- [x] Five ground-delivery cases pass on native and actual Cargo WASM. The full
  native application suites pass (550 tests, 30 explicit skips), plus strict
  native/WASM lint/builds and formatting. Seventeen targeted Cargo WASM cases
  pass. Evidence: `.scratch/campaign-ground-budget-root-20261019/`.
- [ ] First-chapter Host bootstrap, mission natives, lighting, actors/controls
  and three-target gameplay fidelity remain open. No original game or new
  rendered campaign comparison was run for this caller change.

### 2026-10-19 — executed campaign sound-file preparation

- [x] Load direct PCM/MP3 files from executed constructor arguments through the
  retained session reader. Append under cumulative input bounds, preserving old
  handles and resource buffers; refuse failed/wrong bindings before mutation.
- [x] Native Host/application suites, 21 actual Cargo WASM cases, 25 targeted
  WASI cases, strict native/WASM lint/builds and formatting pass. All 58 selected
  Human01 sound inputs (23 PCM/35 MP3) load through the production source adapter
  with matching retained byte fingerprints. Evidence:
  `.scratch/campaign-sound-source-root-20261019/`.
- [ ] Other Host inputs, mission initialization, world lighting, actors/controls
  and full playability remain open. Source-input qualification is separate from
  chapter execution, audible behavior and original-game parity.

### 2026-10-19 — executed sound parameter source rows

- [x] Load requested source parameter labels through the session reader, preserve
  earlier bank/definition keys, and reuse table/index state under explicit bounds.
- [x] Native suites, 25 actual Cargo WASM cases, 19 targeted WASI cases, strict
  native/WASM lint/builds and formatting pass. All 51 distinct Human01 parameter
  labels qualify across five selected table byte inputs; reference-only animation
  rows do not create false ownership. Evidence:
  `.scratch/campaign-parameter-source-root-20261019/verification-final-2.json`.
- [ ] Constructor-label banks, other Host bootstrap inputs, mission natives,
  world lighting and live gameplay remain unfinished. This does not close chapter
  or audible/native parity gates.

### 2026-10-19 — executed label-constructor source preparation

- [x] Prepare PCM definitions/files from executed `CreateSoundFromLabel` and
  `CreateMIDISound` arguments through the retained reader. Preserve prior object
  indices and buffers, validate cumulative inputs before append, and share the
  successful/failed table index with parameter assignments.
- [x] Native suites, 28 actual Cargo WASM cases, 22 targeted WASI cases, strict
  native/WASM lint/builds and formatting pass. All 13 selected label resources
  qualify, including the large ambient loops, with matching retained file hashes.
  Evidence: `.scratch/campaign-label-source-root-20261019/verification-final-6.json`.
- [ ] Other Host bootstrap inputs, mission natives, world lighting and live
  actors/controls still block a playable first chapter. MIDI/DLS decoding,
  multi-file variant selection and original audible/native behavior remain open.

### 2026-10-19 — selected neutral-rule discovery

- [x] Add an opt-in source-image constructor for the startup factory's six
  first-family neutral rules. Derive encoded operands and identities, bound scan
  work/candidates and refuse missing/ambiguous patterns. Retain approved imports.
- [x] Native suites, strict native/WASM lint/builds, 28 existing application WASM
  regression cases and two new discovery cases in actual WASI pass. The actual
  Human01 context matches all six reviewed results; the discovered-rule factory
  completes globals/config and retains 155 creation calls before the unchanged
  `SetCampaignMenuRace` refusal. Evidence:
  `.scratch/campaign-bootstrap-root-20261019/verification-final-0.json`.
- [ ] Remaining Host inputs and application preparation, active native family/
  cache behavior, later mission natives, live actors/controls and chapter fidelity
  remain open. The first campaign is still not playable.

### 2026-10-19 — legacy campaign-menu compatibility

- [x] Qualify the selected image's old campaign-menu setter against `VersionGet`:
  the Frozen Throne branch returns before race selection. Follow that branch on
  the existing Host only with an explicit source-qualified version input; derive
  the constant from the loaded common source. Keep Reign of Chaos and Ex paths
  unsupported, with signature/source/argument and version-invalidation checks.
- [x] All 489 native Host cases, six actual WASI version cases, strict native/WASM
  lint and both application builds pass. Unchanged Human01 scripts pass the call
  on the production factory and next refuse `InitGameCache("Campaigns.w3v")`.
  Evidence: `.scratch/campaign-menu-state-root-20261019/`.
- [ ] Automatic remaining Host inputs, cache persistence, later mission natives,
  accepted live units/controls and three-target campaign fidelity remain open.
  No new original-game run or playable-chapter claim.

### 2026-10-19 — persistent primitive campaign caches

- [x] Add bounded primitive game caches to the existing Host arena: independent
  handles share filename data, typed entries remain distinct, and save/reload/
  flush preserve explicit storage and identity boundaries. Validate signatures,
  source ownership, profile records and resource limits. Preserve exact real
  bits and prior saves on failure. Keep unknown storage formats explicit.
- [x] Attach the profile to the startup Host with native atomic file replacement
  and a bounded browser local-storage adapter; include that adapter in the
  loader and code-only package manifest. Script names are opaque profile keys.
- [x] All 496 native Host, 21 application library and 132 map-integration cases,
  seven actual WASI cache cases, four actual Cargo WASM storage-binding checks,
  the JavaScript storage test, strict native/WASM lint/builds and formatting pass.
  Unchanged Human01 retains the returned cache handle and advances to
  `EnableUserUI(false)`. Evidence: `.scratch/campaign-cache-root-20261019/`.
- [ ] Original W3V import, hero/unit serialization and synchronization, native
  limits/mode/locale details, real browser persistence, other startup inputs,
  cinematics and live units/controls remain open. The first chapter is still
  not playable; the original game was not rerun.

### 2026-10-21 — modeled campaign unit caches

- [x] Store/restore/query/flush unit values in the existing profile. Preserve
  modeled hero progress, permanent strength, pools, sparse item slots, charge
  overrides and explicit item flags. Validate source/client identity, current
  ability rules and creation policies before publishing a restored unit.
- [x] Read old primitive profiles; use version 2 only when unit values exist.
  Full pcx-play tests: 1,151 passed, 44 ignored with required local fixtures.
  Strict all-target clippy, WASM library and app test-fixture compile checks pass.
- [ ] Full native hero payload/XP, original W3V import and native restore parity.
  Fresh-profile Human03 passes RestoreUnit and reaches the later SetHeroLevel
  hero-event-delivery refusal. This is not campaign acceptance. See
  [game caches](docs/re-notes/script-game-caches.md#unit-cache-extension-2026-10-21).

### 2026-10-19 — executed cinematic filter and native real arguments

- [x] Retain the ten executed UI/filter calls on the existing Host. Preserve
  explicit fields, low-byte colors, sorted UVs and logical-clock interpolation;
  qualify supported modes from the loaded common source. Keep unsupported inputs
  explicit. Gate activations/picking separately from HUD visibility and time.
- [x] Render through the published campaign's own UI camera, retained source
  reader and bounded image decoder. Support the supplied same-stem BLP for a
  missing TGA reference, preserve successful images on failure, and clear assets
  on successful session replacement. Keep real source errors visible.
- [x] Convert integer arguments at declared native real parameters through the
  shared I2R policy before suspend/callback/native dispatch. Preserve the current
  VM callbacks; real storage and user-function/return coercion remain open.
- [x] Native suites, strict native/WASM lint/builds, nine targeted actual WASI
  cases, 26 actual Cargo WASM cases and synthetic Vulkan readback through 4K pass.
  Unchanged Human01 completes its fade setup. A private explicit selection of the
  single supplied user-controller row reaches `IsPlayerInForce`; no production
  slot default or original-engine local-player observation is inferred.
- [ ] The first chapter is still not playable. Automatic Host preparation,
  remaining mission natives, actors/controls, native sampling/composition parity,
  real browser rendering and original/native/WASM campaign comparison remain open.
  Details: `docs/re-notes/campaign-presentation.md`; receipts:
  `.scratch/campaign-presentation-root-20261019/`. The previously verified
  implementation is now published; chapter playability remains open.


### 2026-10-19 — force membership predicate

- [x] Add optional source-qualified constant `IsPlayerInForce` to the existing
  force admission. Read explicit shared-arena membership without allocation or
  player/force adoption; preserve signature, source and identity validation.
- [x] Native, actual WASI and actual Cargo WASM checks cover aliases, membership
  changes, capacity, invalid identities, omitted/foreign declarations and failed
  session replacement. Strict native/WASM lint/builds pass. Evidence:
  `.scratch/force-query-root-20261019/`.
- [x] Unchanged Human01 passes the force query with its private explicit local
  client selection and next refuses `ClearTextMessages`. No production local
  slot default, original local-player identity or original runtime result is
  inferred from this probe.
- [ ] Text message consumers, further cinematic/mission natives, normal Host
  preparation and live actors/controls still block the first playable chapter.
  Original null-force/null-player behavior and destruction edges remain open.

### 2026-10-19 — executed campaign text messages

- [~] Production startup admits optional timed local text and clearing on the
  published Host. The launcher consumes the same state through the campaign UI
  camera, source-derived message font/height and existing rich-text parser.
  Visibility uses logical game time; clearing releases the actual retained text.
  Refused declarations preserve the previous published session.
- [ ] Native frame margins, wrapping/fading, automatic-duration text, sender
  prefixes and original rendered comparison remain open. This does not make
  Human01 playable or close the original/native/WASM parity gate. See
  [text messages](docs/re-notes/script-text-messages.md).


### 2026-10-19 — immediate campaign interface and control

- [~] Executed zero-duration `ShowInterface` hides/restores ordinary HUD roots and
  changes the same input-control flag as `EnableUserControl`. The separate UI
  flag, cinematic text/filter layers, child source states and logical clock are
  preserved. Newly mounted roots observe the request before their first layout;
  rejected replacements retain the existing session's state.
- [ ] Positive interface fades, other cinematic surfaces/viewport transitions and
  original rendering comparison remain open. The unchanged private Human01 prefix
  reaches `EnableOcclusion`; this is not a playable first chapter. See
  [campaign presentation](docs/re-notes/campaign-presentation.md#immediate-interface-and-control-follow-up).

### 2026-10-19 — renderer-qualified occlusion disable

- [~] The ground renderer explicitly declares that it has no gameplay
  occluder-fade pass. Only that capability admits `EnableOcclusion(false)` in
  campaign startup; absent capability, incompatible declarations and enabling
  remain unsupported. Rejected candidates preserve the previous session.
- [ ] Active eligibility, live model association, source-driven fades/markers,
  building rules and original rendering comparison remain open. The broader
  occlusion task remains active, and first-chapter playability is unfinished.
  See [script occlusion](docs/re-notes/script-occlusion.md).


### 2026-10-19 — current-speed campaign request

- [x] Source-qualified `SetGameSpeed` accepts an already-current identity without
  changing settings, arena or logical time. Optional declaration validation and
  invalid/different-speed refusal preserve the prior state and published session.
- [ ] Different-speed requests, source-qualified pacing, original/native/WASM clock
  parity, normal first-chapter bootstrap and playable world actors remain open.
  See [startup settings](docs/re-notes/script-startup-settings.md).


### 2026-10-19 — executed campaign speed lock

- [x] `SetMapFlag`/`IsMapFlagSet` derive `MAP_LOCK_SPEED` from the selected script.
  Explicit lock writes now control `SetGameSpeed`: locked valid requests preserve
  the current speed, and unlocking restores different-rate refusal. Missing state,
  other flags, malformed inputs and incompatible sources remain unsupported.
- [ ] Other map flags, initial flag admission, actual speed changes, source-qualified
  pacing, playable first chapter and original/native/WASM parity remain open.
  See [startup settings](docs/re-notes/script-startup-settings.md).


### 2026-10-19 — campaign terrain fog boundary

- [~] Executed world-boundary switches now reach a source-derived terrain mask and
  profile color in the ground material, including explicit removal. The pass
  requires matching published sources and explicitly disabled fog/mask state.
  Full fog of war, actor boundary colors, initial state, native sampler/framebuffer
  equivalence and first-chapter playability remain open. See
  [world fog boundary](docs/re-notes/script-world-fog-boundary.md).


### 2026-10-19 — source-qualified creation-position cancellation

- [x] Two bounded original-game initialization requests now join constructor data,
  ordered position updates and public getter words. The selected large initial
  coordinate explains the observed fractional loss through real-arithmetic
  cancellation. A synthetic regression preserves that loss; production native
  and WASI arithmetic match all captured stored/reconstructed axis words.
- [~] The creation-position and null-filter rectangle slice now uses the same
  executed handles, as recorded below. Live updates, filtered enumeration and
  ordinary campaign world assembly remain open. No measured coordinate constant
  or quarter-floor shortcut is added. Human01 is not playable. See
  [creation-position cancellation](docs/re-notes/script-unit-world.md#creation-position-cancellation-2026-10-19).


### 2026-10-19 — full creation-stream placement candidate

- [~] A source-driven cell-occupancy candidate reproduces all 155 retained
  original creation positions and all 21 separate control requests. Omitting
  actor occupancy or substituting circular overlap misses six creations. All
  53 executed catalog types and six building footprint textures are retained
  with explicit provenance; no original contents are published.
- [x] Correct the blocked source caller to `GetUnitsInRectMatching` for the
  authored starting-forces rectangle. The corrected private startup verifier
  supplies and forwards the selected numeric policy and matches all 155 exact
  coordinate pairs; the former host-arithmetic run matched only 80.
- [~] Creation-time accepted positions and occupancy now use the actual shared
  handles through explicit admission. Filter/order equivalence, geometry boundary
  cases, ordinary world assembly and later mutation remain open. Human01 is not playable. See
  [full creation-stream replay](docs/re-notes/script-unit-world.md#full-creation-stream-and-rectangle-caller-2026-10-19).


### 2026-10-19 — accepted creation positions and startup rectangles

- [x] An explicit initialization cache now stamps successful births and stores
  accepted coordinates on the existing unit handle. Source windows supply
  geometry operands; failed placement/allocation leaves occupancy unchanged.
  The actual native Host matches all 155 retained original creation positions.
- [x] Four null-filter rectangle helpers now complete, including their
  source-qualified null boolean-expression cleanup. The starting group retains
  the authored Arthas identity. Native 529 tests, actual WASI twelve tests,
  strict native/browser lint and both application library builds passed.
- [x] Filtered rectangle predicates now run on the actual owned VM, as recorded
  below. `GetUnitsInRectOfPlayer` completes; group iteration is the next refusal.
  Creation-only occupancy, static-cache assembly, movement/removal/death,
  native ordering/boundary/failure behavior and rendered parity remain open.
  Human01 is not playable; the broader original baseline gate is unchanged.
  See [creation placement](docs/re-notes/script-unit-world.md#creation-placement-on-executed-identities-2026-10-19).


### 2026-10-19 — rectangle predicates on the owning VM

- [x] Loaded filter/condition predicates now share the existing VM and unit
  handles. One continuation stack routes nested rectangle and trigger calls;
  `GetFilterUnit` restores parent context and returns null after unwinding.
- [x] Source code qualifies destination clearing and insertion after a true
  predicate. Human01 selects five of seven candidates; membership checks cover
  all 155 handles and their original creation positions still match.
- [x] Native 535 tests, six application trigger integration tests, actual WASI
  21 tests, strict native/browser lint and both library builds pass.
- [x] `ForGroup` in `GroupAddGroup` now completes as recorded below. Candidate ordering/edges,
  mutable traversal, ordinary cache assembly, live world updates and rendered
  parity remain open. Human01 is not playable and the original baseline gate
  remains unchanged. See [rectangle predicates](docs/re-notes/script-unit-world.md#filtered-rectangle-predicates-2026-10-19).


### 2026-10-19 — retained group actions on the same VM

- [x] `ForGroup` and `GetEnumUnit` use the existing callback stack and actual unit
  handles. Native code qualifies a retained member snapshot; nested group,
  rectangle and trigger contexts restore independently, including on error.
- [x] The unchanged Human01 completes two group iterations/eight enum reads;
  all 155 creation inputs and accepted positions retain the original comparison.
- [x] Native 542 tests, six application trigger tests, actual WASI 28 tests,
  strict native/browser lint and both application library builds pass.
- [x] Explicit `SetUnitInvulnerable` now completes as recorded below. Ordinary Host/cache assembly, live
  world updates, unit lifecycle/combat, native ordering and rendered/4K parity
  remain open. Human01 is not playable; the original baseline gate is unchanged.
  See [group actions](docs/re-notes/script-unit-world.md#group-actions-on-the-owning-vm-2026-10-19).


### 2026-10-19 — explicit unit protection and its script query

- [x] Executed `SetUnitInvulnerable` changes the same unit record consumed by
  the loaded `BlzIsUnitInvulnerable` query. Null behavior follows selected native
  wrappers; untouched initial protection stays unknown rather than defaulting.
- [x] Human01 completes three protection assignments and queries on three
  actual handles; all 155 original creation inputs/positions still match.
  Native 547 tests, application trigger six tests, actual WASI 34 tests, strict
  native/browser lint and both application library builds pass.
- [x] `SetPlayerHandicapXP` now completes as recorded below. Initial/other ability effects, ability
  list/level changes, damage/combat, ordinary Host preparation, playable actors
  and original/native/WASM rendered/4K parity remain open. Human01 is not
  playable and the original baseline gate is unchanged. See
  [explicit unit protection](docs/re-notes/script-unit-world.md#explicit-unit-protection-2026-10-19).


### 2026-10-19 — source-bound player XP factor

- [x] The startup factory accepts an explicit source policy for the XP setter
  and query. Limits/null results come from fingerprinted image data and checked
  real initializers using the same selected numeric model; no measured bounds
  or initial valid-player factor is a production default.
- [x] Human01 completes the assignment and query on one actual player row.
  All 155 creation-input/position comparisons remain exact. Native 557 tests,
  six application integration tests, 188 distinct actual WASI tests, strict
  native/browser lint and both application library builds pass.
- [x] The `SetUnitOwner` initialization caller now completes as recorded below.
  Initial player factors, experience awards,
  ownership/colour/event effects, ordinary Host preparation, live actors and
  original/native/WASM deterministic/rendered/4K parity remain open. The first
  campaign is not playable; the original baseline gate is unchanged. See
  [experience handicap](docs/re-notes/script-player-setup.md#source-bound-experience-handicap-2026-10-19).


- [x] Edition-switch logo lifetime (2026-10-19): switching source/edition retires
  the old logo and re-reads the authored skin/model/texture/anchor/Birth path.
  411 native tests, one explicit local Legacy TFT/RoC/TFT logo test, strict
  native/browser checks and both builds pass. Three native journal captures
  verify the switch. At this checkpoint the RoC model-space burst emitter
  refused its batch; the later particle/horizon corrections above address it.
  This logo-only evidence predates the fresh original/native 4K horizon check.
  Full background/particle and browser fidelity remain open. Evidence:
  `.scratch/menu-edition-root-20261019/verification-1.json`.


### 2026-10-19 — same-arena initialization ownership

- [x] Loaded `SetUnitOwner` updates the existing unit's current player identity.
  Creation player number, catalog/placement and aliases are preserved. Owner
  queries and Defend research use the new owner; null and unchanged-owner calls
  have no effect. False preserves color; true requires a known assigned player
  color. Initial class color is not guessed.
- [x] Human01 completes its actual `hhdl` transfer. All 155 original creation
  inputs/positions and prior group, protection and XP records still match.
  Native 564 library tests, six app trigger tests, seven actual WASI ownership
  tests, strict native/browser lint and both application library builds pass.
- [~] Ownership-event delivery, initial/rendered color, control/food/vision/AI
  effects and live world association remain open. Matching or unqualified live
  listeners refuse before mutation; disabled/unrelated listeners are covered.
- [ ] Next caller: `SelectHeroSkill` at `109856..109898`. Ability levels/effects,
  ordinary Host preparation, playable actors/controls and full deterministic
  original/native/WASM rendered/4K parity remain open. Human01 is not playable;
  the original baseline gate is unchanged. See
  [initialization ownership](docs/re-notes/script-unit-world.md#initialization-ownership-transfers-2026-10-19).


### 2026-10-19 — preserve image filtering when applying texture wrap

- [x] The shared TEXS sampler now changes addressing without accidentally
  replacing the application's linear filtering with nearest-neighbour defaults.
  Source wrap flags, image pixels, alpha and geometry remain unchanged.
- [x] 53 graphics and 411 menu tests, strict native/WASM checks and both full
  builds pass. Fresh native TFT/RoC/TFT captures at 1024×768 and 3840×2160 show
  smoother repeating textures and unchanged RoC geometry.
- [ ] The fresh browser adapter times out before menu readiness at 1024×768;
  it is stopped without retry or subsequent 4K attempt. Rendered WASM, rain,
  transparency and full original/native/WASM equality remain open. The public
  site is unchanged. See [texture filtering](docs/re-notes/menu-texture-filtering.md).

### 2026-10-20 — retain authored menu mipmaps

- [x] Bounded model-image loading and geometry/PRE2 uploads now retain the source
  BLP mip chain, including lower-level RGB and alpha. Complete-chain budgets and
  duplicate identity checks apply before scene admission; base-only terrain-atlas
  consumers retain their existing contract.
- [x] 55 graphics-library, 17 particle-render integration and 412 menu tests,
  synthetic Vulkan mip readback, strict native/WASM checks and both full builds
  pass. All 257 levels in the 33 selected RoC textures match decoder/upload bytes.
  Fifteen native edition-switch captures pass at five extents through 4K; fresh
  isolated software-WebGL RoC captures pass at 1024×768 and 4K. Geometry is unchanged.
- [ ] Original mip selection/LOD bias, hardware-browser startup, particle state,
  full transparency and exact three-target pixels remain open. The six raw RGB
  comparisons still differ. The first campaign remains unplayable and the original
  baseline gate is unchanged. Public hosting was not updated. See
  [texture mipmaps](docs/re-notes/menu-texture-mipmaps.md).

### 2026-10-20 — source-backed hero learning state

- [x] The campaign factory admits source-derived hero counters, learned ranks,
  authored level requirements/caps and scalar custom-ability overrides on the
  existing unit. `SelectHeroSkill` spends a point; the ability/hero queries read
  that state. Implicit components, source identity, budgets and live-listener
  qualification are explicit; no measured game-rule values are copied into defaults.
- [x] Unchanged Human01 learns `AHhb` on its existing `Hart`: rank one, level one,
  zero unspent points. All 155 original creation inputs and accepted positions
  still match exactly. Earlier group/protection/XP/ownership results are retained,
  and two more source ownership calls execute. Native 577 playback tests, one
  profile test, six app trigger tests, seventeen actual WASI hero cases, strict
  native/browser checks and both full builds pass.
- [ ] Next refusal is `SetUnitAnimation("stand defend")` at `110016..110069`.
  General spell/availability/dependency effects, XP/stat gains and hero-event
  delivery remain open, as do ordinary Host inputs, live actors/controls and the
  full campaign and deterministic three-target gates. Hero task remains active;
  first Human01 is not playable and the original baseline gate is unchanged.
  See [hero skill state](docs/re-notes/script-hero-skills.md).

### 2026-10-21 — source-bound hero events

Level/skill setters now deliver source-bound
unit/current-owner player events with captured learned ranks, updated callback
state, filters and response cleanup. Private package tests: 1,155 passed,
0 failed, 44 ignored; strict clippy, WASM library and app all-target fixture
checks pass. Fresh-profile unchanged Human03 next refuses `UnitAddSleep` through
Blizzard.j:3524. Native synchronous ordering, XP-driven events and campaign
acceptance remain open. See [hero events](docs/re-notes/script-hero-skills.md#hero-events-2026-10-21).

### 2026-10-21 — explicit headless campaign profiles

- [x] Require initial PC3 profile bytes or explicit absence in headless startup,
  validate supplied bytes, and include their identity in admission. Keep saves
  session-local with export for later maps; the app retains local persistence.
- [x] Synthetic save/reload isolation and bounds tests pass. The selected Human01
  startup gate compares fresh, identical and different saved profiles, observes
  the stored values and verifies that saving leaves other sessions unchanged.
- [ ] Original W3V compatibility, full unit payload fidelity and multiplayer
  campaign play remain open. See [profile admission](docs/re-notes/script-game-caches.md#headless-profile-admission-2026-10-21).

### 2026-10-21 — natural unit sleep

Four natural-sleep natives now use the
existing unit record and activity loop. Source permission, map overrides,
night/day transitions, acquisition suppression, damage retaliation, accepted
movement wake, lifecycle and bounded work have synthetic coverage. Full playback
package: 1,162 passed / 0 failed / 45 ignored; strict clippy and WASM pass.
The app all-target fixture check and both selected Human01 headless gates pass.
Human03 next refuses `IssuePointOrderLoc` at Blizzard.j:7800, after creep setup.
Permanent/spell sleep, native wake timing/proximity and presentation remain
open. See [natural sleep](docs/re-notes/unit-natural-sleep.md).

### 2026-10-21 — source-bound unit patrols

- [x] Point patrols retain both endpoints, reverse with unused travel time and
  resume through automatic combat. Existing route budgets, accepted spans,
  explicit replacement, pause, guard and natural-sleep behavior are covered.
- [x] Private-target package: 1,170 passed / 0 failed / 46 ignored. Native app
  all-target fixture and WASM library checks pass. Selected Human03 initializes
  three scripted patrols; the Human01 startup/order/profile gates also pass.
- [ ] Human03's first clock tick still refuses blight regeneration without
  terrain state. Native timing/turning, queued patrols and command-panel input
  remain open. See [unit patrol](docs/re-notes/unit-patrol.md).

### 2026-10-21 — initial terrain blight

- [x] Read W3E blight flags independently of ground texture names. Derive world
  spacing from the admitted placement normalization and map grid ratio.
- [x] Integrate accepted straight motion spans at exact cell crossings, with
  deterministic work and staged pool writes. Synthetic coverage includes
  stationary/moving regeneration, chunk composition, diagonal boundaries,
  atomic refusal, clock rebases and relocation boundaries.
- [~] Human03 advances beyond the first tick to a movement-budget refusal at
  tick 155. No campaign acceptance or native/rendered eligibility claim.
- [ ] Player-owned script painting, overlap/removal, terrain presentation and
  native regeneration timing remain open. See [terrain blight](docs/re-notes/terrain-blight.md).

### 2026-10-21 — bounded search for blocked movement goals

- [x] Stop forward A* at a proven nearest clear-cell distance rather than
  continuing to consume the shared movement budget. Caller limits are retained.
  Cardinal, diagonal and larger blocked areas have deterministic work and
  route-clearance coverage; disabling termination fails the regression.
- [x] Selected Human03 passes the 256-tick startup/order/control gate.
- [ ] The longer headless run stops at tick 257 on `SetUnitLookAt`, so campaign
  acceptance and rendered/native path fidelity remain open. See
  [collision replanning](docs/re-notes/unit-collision-replanning.md#blocked-destination-search-termination-2026-10-21).

### 2026-10-21 — scripted bone look-at

- [x] Retain a source-model bone, live target and script offsets; reset preserves
  the sampled animation and removing a target clears its look-at references.
- [x] Campaign actors resolve the moving target in world space and aim the
  selected bone and descendants before mesh skinning and lighting.
- [x] Unchanged Human03 executes its Arthas/Captain look-at at tick 257.
- [ ] The longer run next refuses `IssueImmediateOrder` at tick 980. Native
  angular limits, blending, target-height semantics and rendered comparison
  remain unqualified; see [unit look-at](docs/re-notes/unit-look-at.md).

### 2026-10-20 — source-derived edition-switch icon states

- [x] The control builder now reads both icon families and their three state
  bindings from the supplied client. RoC advertises TFT and TFT advertises RoC;
  FDF layout, borders and dispatch are retained. Browser import follows the
  same source paths before committing its candidate archive store.
- [x] Native 579 host-library and 413 menu tests, the explicit original-client
  reader test, strict native/browser lint and both full builds pass. Actual
  production WASM imports six source images; three missing-image and one
  changed-client candidates preserve the previous committed state on refusal.
- [ ] Fresh window/browser visual checks are refused by local-socket restrictions.
  Six software-Vulkan offscreen TFT/RoC/TFT captures confirm icon changes at
  1024×768 and 4K, but the 4K journal loses labels after switching editions.
  That observation, window/browser rendering, tooltips, background/particle fidelity, public deployment and full
  three-target equality remain open. The first campaign remains unplayable.
  See [edition button](docs/re-notes/menu-edition-button.md).

### 2026-10-20 — first-mission source actors and animation in progress

- [x] Unchanged Human01 executes its five authored animation calls on the same
  unit handles; all 155 creations retain source models. Their position-bit
  records match the prior original-qualified replay. The next refusal is
  `IssueImmediateOrder("holdposition")`.
- [x] An isolated CPU ECS audit accepts all 155 actual actors through the
  production spawn/update functions, including every material layer: 694 draws,
  84,818 layer vertices and 109 actors with changing poses. Palette prefixes and
  count come from the selected skin; enum string overrides are preserved.
- [x] Native host 583, application 418 and map-integration 148 tests pass, along
  with four actual WASI cases, strict native/browser checks and both full builds.
- [ ] Runtime startup state, active orders, source unit lighting, opaque/cutout
  pass ordering and GPU acceptance remain open. The factory loader and actor
  consumer are incomplete first-mission work. Human01 is not playable; the
  original baseline and three-target deterministic/rendered gates are unchanged.
  See [unit animation](docs/re-notes/script-unit-animation.md).

### 2026-10-20 — map player roles for campaign startup

- [x] Corrected the W3I player-role helpers: player number, controller, race and
  fixed-start flag are distinct words. Sparse player numbers and canonical
  bytes are preserved; legacy public fields remain compatible.
- [x] Synthetic version-18/version-25 cases distinguish a user in slot 8 from a
  neutral player in slot 1. Native 96 map tests, four actual WASI cases, strict
  native/browser checks and both application builds pass. Native and WASI
  source comparisons join all five Human01 player records and exact start
  positions to its authored configuration with identical results.
- [ ] Host source preparation now has a producer; runtime startup state remains
  incomplete. Authored roles do not prove
  local-client identity or occupied lobby slots. Live orders/world and full
  first-mission, original/native/WASM determinism and rendered fidelity remain
  open. See [map player roles](docs/re-notes/map-player-roles.md).

### 2026-10-20 — campaign source-input producer

- [x] Normal native/browser startup prepares source inputs from the admitted
  resolver and fingerprinted client, with bounded source reads and no copied
  game-rule defaults. Failed preparation preserves the previous session and
  matching corrected inputs recover.
- [x] The supplied Human01 audit constructs the production factory and completes
  globals/config with 43 source conversion contracts. The selected client reader
  agrees bit for bit between native and WASI execution, including 128 explicit
  seed integer/real draw pairs. Native 584 host, 171 library/map and 418 application
  tests pass, with strict native/browser checks and both full builds.
- [ ] Runtime startup observations, initial engine seed, live placement/world,
  orders/combat, initialization scheduling and campaign-selection delivery are
  unfinished. No real-asset `main`, gameplay, new original-client runtime or
  rendered-parity check is claimed. Human01 remains unplayable and the original
  baseline gate is unchanged. See
  [source preparation](docs/re-notes/campaign-source-preparation.md).

### 2026-10-20 — post-config slot input before lazy player creation

- [x] The production startup factory retains explicitly supplied slot numbers
  before their `Player` handles exist. Missing states stay unset, rejected input
  preserves prior state, and controller writes invalidate future entries too.
  No early player handles or occupancy defaults are introduced.
- [x] The source-prepared Human01 Host matches all sixteen retained original
  census entries: five existing players, eleven later creations and zero handles
  allocated during admission. All 586 Host and 172 library/map tests, five actual
  WASI slot tests, strict native/browser checks and both full builds pass.
- [ ] This comparison does not supply ordinary new-game runtime state, execute
  real-asset `main` or make the first mission playable. The active bootstrap and
  world/order work remains open. See [slot input](docs/re-notes/script-player-slots.md).

### 2026-10-20 — normal fresh-game state and pre-global RNG seed

- [x] Normal startup derives fresh speed/fog values from the selected client,
  resolves version and enum identities through loaded common source, and builds
  the selected single-player roster after config without census defaults.
  An explicit game seed initializes the source-bound RNG before globals;
  platform entropy supplies normal launches and a recorded override supports tests.
- [x] Derived Human01 controller/slot/settings/version state matches the retained
  original observations. Native/WASI fresh-settings readers agree. All 587 Host,
  23 library, 151 map/application and 419 binary tests, strict native/browser
  checks and both full builds pass. Actual application startup with the supplied
  Legacy archive inventory loads ambient audio sources and reaches the next
  unsupported `GetPlayerTechResearched` call in `InitSummonableCaps`.
- [ ] Fresh research, implicit original engine seed policy, live world/orders,
  startup scheduling and campaign-selection delivery remain open. The real
  candidate does not publish and Human01 remains unplayable. No new original
  runtime, audible/GPU verification or full original/native/WASM parity is claimed.
  See [source preparation](docs/re-notes/campaign-source-preparation.md).

### 2026-10-20 — fresh source-bound upgrade counters

- [x] Normal new games admit upgrade counters before globals on the existing Host.
  Caps follow stock tables, metadata and map edits. Raw setter requests remain
  separate from applied levels, and Defend consumes the applied result. Invalid
  input and invalidation preserve explicit refusals; observation mode stays separate.
- [x] All 592 Host, 23 library, 151 map/application and 419 binary tests pass,
  along with eight actual WASI tests, strict native/browser checks, both builds
  and formatting. Unchanged Human01 now passes `InitSummonableCaps` through the
  application and reaches unsupported `TriggerRegisterTimerEvent` at BJ1660.
- [ ] General research effects/relationships and original initialization parity
  remain open. Trigger runtime supply, startup scheduling, normal world/orders
  and campaign selection still prevent playability. The failed candidate remains
  unpublished; no new original runtime or rendered/audio parity is claimed.
  See [upgrade counters](docs/re-notes/campaign-source-preparation.md#fresh-upgrade-counters-2026-10-20).

### 2026-10-20 — normal timed-trigger runtime supply

- [x] Fresh preparation supplies the existing arena/clock/session trigger runtime.
  The fresh enabled policy uses the decoded enable operand, supported by retained
  original behavior. The event identity is decoded and joined to its unique loaded
  common-source conversion constant. No captured ordinal/default or alternate VM.
- [x] All 593 Host, 23 library, 151 map/application and 419 binary tests pass,
  with six actual WASI tests, strict native/browser checks and both full builds.
  A zero-case WASI selector is excluded; its corrected run executes all four
  trigger-runtime cases. Unchanged Human01 application startup advances to
  `TriggerRegisterUnitInRange` at BJ1814 and remains unpublished.
- [ ] Range transitions need actual positions, membership changes, predicates and
  delivery through the owned VM. The registration-only API remains disabled in
  this producer. Main scheduling, normal world/orders, campaign selection and
  original/native/WASM rendered gameplay parity still prevent playability.
  See [fresh trigger input](docs/re-notes/campaign-source-preparation.md#fresh-timed-trigger-input-2026-10-20).

### 2026-10-20 — normal creation-placement supply

- [x] Normal startup assembles the existing placement cache from its admitted map,
  source metadata/edits, pathing textures and executed stationary records before
  the first unit birth. The same arena owns accepted positions; no observed
  positions or placement-id aliases enter the producer.
- [x] All 594 Host, 26 library, 151 map/application and 419 binary tests pass,
  with eight actual WASI core/parser cases, strict native/browser checks and both
  full builds. An instrumented creation-cut diagnostic through the normal loader
  matches all 155 requests and accepted position bits to retained original results.
- [ ] Unchanged Human01 still refuses range registration at BJ1814 and publishes
  no session. Actual membership/filter/action delivery, startup scheduling, live
  world/orders and campaign delivery remain open. Static raster limits and the
  missing original/native/WASM rendered comparison are explicit; this is not a
  playable mission. See [placement supply](docs/re-notes/campaign-source-preparation.md#normal-creation-placement-supply-2026-10-20).

### 2026-10-20 — deferred unit-range delivery

- [x] Normal new games admit range registration and a real accepted-position
  consumer. Logical ticks deliver entry filters, all registered conditions and
  actions on the owned VM. Context, stale arena/poll tickets, work refusal,
  disabled/destroyed owners and failure cleanup are covered. Startup/loading
  itself does not fire entries; no copied position, ordinal or alternate arena.
- [x] All 604 Host, 26 library, 154 map/application and 419 binary tests pass,
  plus fourteen actual WASI cases and eighteen application cases on the actual
  browser-target WASM module under Node. Strict native/browser checks and both
  builds pass. The 155 original placement comparisons remain exact.
- [ ] Native range cadence/equality/eligibility/order and full lifecycle remain
  open, as do live movement/death, activity/AI/combat, startup suspension and
  ordinary campaign delivery. Unchanged Human01 now refuses `IssueImmediateOrder`
  at BJ7817 during initialization (`holdposition`); no session is published and
  the mission is not playable. No new original run or rendered/audio parity claim.
  See [range delivery](docs/re-notes/script-unit-world.md#deferred-range-consumer-2026-10-20).

### 2026-10-20 — selected order-name registry

- [x] Fresh-game startup supplies checked `OrderId`/`OrderId2String` lookups
  from both reviewed client registration paths, including the preceding
  bootstrap entry. All names/IDs come from the selected image; the identical
  repeated registration, ASCII case comparison and null/missing results are
  covered without an embedded order table.
- [x] All 609 Host, 26 library, 154 map and 419 application tests pass, with
  five actual WASI registry tests, eighteen browser-target WASM application
  cases, strict native/browser checks and both full builds. The actual startup
  Host matches 349 source registrations and four original-runtime lookup
  controls. A bounded original probe retains 23 records; its earlier loading
  screen failure is preserved and both private containers are stopped.
- [ ] Hold Position activity, acquisition/movement and coherent live collision
  occupancy remain open. Repeated Hold requests are accepted by the original,
  while its current-order query remains zero even during the observed autonomous
  motion after Stop. Last-request storage would be wrong. Unchanged Human01
  still refuses Hold Position at BJ7817 and publishes no playable session.
  Rendering and original/native/WASM gameplay parity remain unverified. See
  [order behavior](docs/re-notes/script-immediate-orders.md#selected-client-lookup-and-activity-order-frontier-2026-10-20).

### 2026-10-20 — explicit unit activity and live movement

- [x] Fresh startup supplies source-decoded Hold/Stop/Move identifiers on the
  existing unit arena. Explicit movement consumes metadata-associated movement
  class/speed and the live map collision cache; Hold/Stop cancel that movement.
  Location orders copy a destination from the same arena. Motion, queries,
  range delivery and actor transforms share the accepted unit record.
- [x] Timer callbacks observe motion at their logical deadline. Repeated projection
  is idempotent, and fixed-tick tests preserve results across frame chunks and
  loading delays. Nonmoving source rows need no numeric speed to be created;
  querying a marker still refuses without an invented value.
- [x] All 1,378 affected native tests and twenty actual WASI tests pass, with
  nineteen browser-target WASM application cases under Node, strict native/browser
  checks and both full builds. All 155 original creation requests and accepted
  position pairs remain exact. An additional bounded original Wine probe retains
  fourteen records; nominal speed and explicit order-state controls agree through
  the normal source-backed Host. The original runtime is stopped.
- [ ] Original steering, acceleration, Walk playback rate, path choice and local
  avoidance are not matched. Autonomous acquisition, combat/death, visibility,
  startup suspension and player controls remain open. Unchanged Human01 now
  passes its Hold calls and refuses `ShowUnit` at BJ3412, publishing no session.
  The mission remains unplayable; no rendered original/native/WASM parity claim.
  See [activity behavior](docs/re-notes/script-immediate-orders.md#explicit-movement-and-activity-consumer-2026-10-20-candidate).

### 2026-10-20 — unit visibility on the owned world

- [x] Checked optional `ShowUnit`/`IsUnitHidden` contracts mutate the canonical unit.
  Hidden units retain orders and existing groups, continue movement, release their
  collision occupancy and leave new spatial enumerations. Showing restores valid
  placement and rebases motion if relocated. Actor parents follow the same state.
- [x] Three bounded original Wine probes retain 59 records. A wider rectangle
  corrects an inconclusive narrow control, and a separate experiment establishes
  that hidden anchors still deliver range entries for visible candidates. All
  original runtimes are stopped. No original/generated assets are published.
- [x] All 1,385 affected native tests, 25 actual WASI cases and twenty application
  cases on the actual browser-target WASM module pass, with strict checks and
  both full builds. The 155 original creation requests/positions remain exact;
  the source-backed unit passes visibility, spatial, collision and motion controls.
- [ ] Related attachments, passengers, selection/death interactions, all hidden
  pathing/reveal cases, small-rectangle native raster behavior and full rendered
  parity remain open. Unchanged Human01 passes its visibility calls and now refuses
  `AttachSoundToUnit` at BJ2226. It publishes no session and remains unplayable.
  See [executed visibility](docs/re-notes/script-unit-world.md#executed-visibility-and-live-world-membership-2026-10-20).


### 2026-10-20 — cinematic skip and selection circles

- [x] Escape queues the selected local-player end-cinematic registrations on the
  owned VM. The launcher suspension key follows the selected highlighted
  `GlobalStrings.fdf` `KEY_MENU`, with a retained byte receipt and explicit
  missing/unsupported-state diagnostics.
- [x] Zero one-shot cinematic timers expire at the current scheduler boundary;
  invalid/zero-periodic starts retain the old run. Scripted unit relocation uses
  the canonical record and prepares collision occupancy atomically. These timing,
  placement and motion-retention rules remain stated recreation choices.
- [x] Source selection bands/scales/colors supply ground circles, sharing the
  local owner's directional alliance rule. Settings/materials retire with their
  session; mismatched map bindings cannot supply replacement textures.
- [x] All 1,411 affected native tests and 643 actual WASI cases pass, plus 32
  exported integration cases on the actual browser-target WASM module under
  Node. Strict native/browser Clippy, formatting and both full builds pass.
  The stable-source Human01 driver publishes normally, queues four local
  registrations at 20 seconds, and advances 19,200 ticks to 300 seconds without
  a runtime error. Its explicit compatibility catalog and separate diagnostic
  main refusal remain recorded; this is not a chapter-completion result.
- [x] A fresh native Xvfb/Vulkan software-rendered launch enters Human01, and
  Escape replaces the opening cinematic with the overhead mission view/HUD.
- [ ] Native zero-delay/teleport equivalence, selection-circle slope/projection
  parity, complete HUD content, missing sound placement/fades, mission completion
  and synchronized original/native/browser comparison up to 4K remain open.
  Receipts and captures stay local under `.scratch/campaign-controls-20261020/`
  and `.scratch/campaign-time-of-day-root-20261020/controls-confirmed-20261020/`.


### 2026-10-20 — campaign transmission, quests and attack responses

- [x] Executed cinematic scenes project source-authored speech/speaker frames,
  portrait backing and supported border pieces. FDF, skin, font and image
  identities remain local and session-owned; unsupported unequal-inset background
  layers are reported. Portrait models and player-colour interpretation remain
  open. See [transmission panel](docs/re-notes/campaign-transmission-panel.md).
- [x] Source upper Quests/Menu controls and QuestDialog project discovered,
  enabled live quests and item state. Completed pointer/keyboard activations use
  the shared event queue, including fast press/release; modal Escape/pointer
  consumption precedes world input. Resource balances are not fabricated.
  See [upper controls](docs/re-notes/campaign-upper-controls.md).
- [x] Player-unit attacked registrations and point attack-move orders deliver
  Human01's authored bandit response through the existing combat/activity paths.
  Destination retention, acquisition and resumed travel are stated recreation
  policies. See [attack responses](docs/re-notes/script-attack-move.md).
- [x] Cursor rays can enter admitted terrain from outside its bounds; invalid
  samples no longer abort every such click. Menu-key strings use their own bounded
  read policy rather than the smaller camera-configuration budget.
- [~] Non-spatial supported sound rows retain source volume/pitch/duration and
  executable-derived fade timing. Actual Human01 voices still refuse unsupported
  channel/list preemption flags with exact source-row diagnostics.
  See [dialogue audio](docs/re-notes/campaign-dialogue-audio.md).
- [x] Native click selection and right-click travel visibly render the green
  circle after moving a footman onto open ground. Initial model/terrain overlap
  concealed it; the flat-plane slope limitation remains explicit.
- [ ] Script-created button dialogs have no executed Host state or button-event
  consumer. Full cinematic/quest styling, selection-circle rendered parity,
  original/native/browser comparison up to 4K and chapter completion remain open.

- [x] Final stable-source validation: 1,433 affected native cases, two explicit
  local original-FDF checks, 653 actual WASI cases and 47 exported cases on actual
  browser-target WASM under Node pass. Strict native/browser all-target Clippy,
  formatting and full native/browser builds pass. Node execution does not prove
  browser rendering.
- [x] Human01's late cinematic skip reproduced an order against an existing
  removed-unit tombstone. Supported activity orders now return false without
  revival; wrong-family and unknown handles still error. This return behavior is
  labeled a recreation policy. The stable-source production-factory driver runs
  both skip-at-20 and skip-at-48 paths for 19,200 ticks to 300 seconds without a
  runtime error. Its explicit compatibility stock catalog and independent
  diagnostic-main refusal remain recorded; this is not chapter completion.
  Verification stays local under `.scratch/campaign-integration-20261020-v8/`
  and `.scratch/campaign-time-of-day-root-20261020/integrated-v8-skip48-20261020/`.

### 2026-10-20 — script dialogs, resource projection and voice policy

- [x] Session-owned JASS dialog/button lifetimes, local display, callbacks,
  response context and quit requests execute through the Host. The application
  renders source-bound normal/pushed controls and suppresses world input during
  modal interaction. Stale session witnesses discard pending clicks and hotkeys.
  Multiline caption geometry, rendered comparison and score presentation remain
  open. See [script dialogs](docs/re-notes/script-dialog-runtime.md).
- [x] The resource bar reads its FDF/skin/font/icon data and projects admitted
  effective gold, lumber and food counters. Authored quest hotkeys use the shared
  activation path. Upkeep remains explicitly unknown.
- [x] Nonspatial voice admission implements selected-client channel/list priority
  policy and source defaults. The selected original opening voice publishes and
  decodes nonzero samples; physical output and original/browser parity remain open.
- [x] Selection circles conform to rendered terrain triangles, retaining source
  size/tint/UVs and bounded mesh work. Initial-position rendered proof is pending.
- [x] UnitWeapons identity derives from unique canonical row joins when the
  expected header is absent; both Editor and DefaultTables now reach the source
  weapon-enable cells. Original field availability is checked, combat progression
  remains a separate gate.
- [x] Frozen-source v13 checks: 1,527 affected native cases, seven explicit local
  source gates, 734 actual WASI cases, strict native/browser all-target Clippy,
  formatting and native/browser builds pass. Fifty browser-target WASM cases execute
  under Node; this is not browser rendering. Local evidence stays under
  `.scratch/campaign-gameplay-20261020-v13/`.
- [x] The production-factory replay exposed a missing food-ceiling source and
  authored numeric markers. Admission now reads `FoodCeiling` overrides or the
  selected-client edition table, and food projection uses the selected integer
  reader's marker semantics. Original profile and resolved-unit query gates cover
  both producers. Normal Human01 startup and all five resource queries now pass.

### 2026-10-20 — actual modal input and objective-route checkpoint

- [x] Script captions use actual Bevy shaped text height, including wrapping,
  explicit line breaks and scale changes. Formatting-only assigned captions retain
  a present zero extent. Hidden measurement candidates block world input until
  ready; stale session/root/revision probes cannot publish geometry.
- [x] A native full-application fixture visibly renders the source dialog panel.
  Primary-pointer Continue closes it and primary-pointer Quit clears the owned
  session. The campaign control barrier now sits below modal controls while the
  cinematic filter retains its prior visual ordering. Real UI picking tests also
  verify ordinary HUD suppression. This fixture's text is synthetic, not an
  authored Human01 dialog or original-game visual comparison.
- [x] Hero weapon damage, armor and attack timing consume source coefficients and
  current authored attribute growth. Explicit invulnerability suppresses weapon
  targeting/damage, including in-flight impacts; direct script kills and life
  setters remain independent. Initial protection, hero vitals after level changes
  and scaled attack animation remain open.
- [x] Source-qualified fresh cheat getters preserve normal defeat. The native
  no-input Human01 path now reaches the subsequent unsupported `RemovePlayer`
  declaration instead of stopping at `IsNoDefeatCheat`. Cheat mutation remains
  explicitly unsupported.
- [x] `MoveRectToLoc` copies admitted location coordinates into the shared
  extent-preserving rectangle movement path. Eight rectangle checks pass; the
  objective replay independently advances to three of seven authored route
  rectangles before refusing `UnitHasItem`. This is neither three completed
  objectives nor chapter completion.
- [x] Native isolated-sink capture contains the opening voice, matching the
  selected original MP3 at approximately 12.043 seconds (normalized waveform
  correlation 0.481 in the mixed output). A separate test profile enabled gain;
  the actual saved music/effects gains remain zero and their file hash is
  unchanged. The dedicated sink and owned process were removed afterward.
  This verifies output routing, not physical speakers or original-game parity.
- [x] Menu music now pauses during campaign or intro ownership and resumes on
  return. The lifecycle regression and selected-original preplayback gate pass.
  Full mission/campaign progression and original/native/browser comparisons remain open.

- [x] Final v15 frozen-source checks pass: 1,556 affected native cases, ten explicit
  original-source gates, 758 actual WASI cases, 51 cases on browser-target WASM
  under Node, strict native/browser all-target Clippy, formatting and full native/
  browser builds. Native automatic Escape followed by F9 visibly opens the quest
  log and shows live resource values. Initial selection-circle pixels remain
  unproved; a later manual click missed the unit, so its screenshot is not a
  successful circle gate. Local records stay under
  `.scratch/campaign-gameplay-20261020-v15/` and
  `.scratch/campaign-objective-20261020/normal-gui-v15/`.


### 2026-10-20 — inventory, result transitions and live presentation

- [x] Inventory capacity/permissions derive from selected ability metadata, W3A
  overlays and profiles. Canonical ground/held/removed item identities, slot and
  ownership queries, routed pickup, response events and death/removal policies
  execute in the shared Host. Powerup effects and unsupported item actions refuse.
- [x] Ground items render their resolved model, scale and colors through the
  existing model/material renderer. Pointer targeting intersects source model
  bounds and issues a real pickup order. A private native fixture visibly renders
  a synthetic ledger, right-clicks its projected model bounds, and observes the
  same item held in Arthas's slot with its ground actor retired. This is a
  component/input gate, not authored ledger quest completion. Inventory icons,
  item use/passive effects and drag/drop/give controls remain open.
- [x] Source-qualified `RemovePlayer` preserves the VM and player/controller
  identity, retains sticky results and queues typed result events. The unchanged
  no-input Human01 defeat path passes removal and now stops at
  `GetLocalizedString` in the defeat helper. Localization, hotkeys and simulation
  pause still block the authored defeat dialog.
- [x] Hero level changes update maximum/current life and mana by the source
  attribute contribution delta, preserving missing resources and explicit maxima.
  Unqualified living/dead boundary transitions refuse atomically.
- [x] Upkeep labels consume effective food, selected-edition thresholds and
  selected localized strings, including custom-rank fallback and inline colors.
  A normal native Human01 view visibly displays No Upkeep. Observer presentation
  and economic tax behavior are separate remaining requirements.
- [x] Selection projection reads the native image height offset and strict texture
  band boundary from the selected executable. A normal native automatic Escape
  capture shows the initial selected hero's green rim around its blue aura;
  no manual selection workaround or inferred size doubling is used. Original-game
  and browser rendered comparison remain open.
- [x] Frozen-source v17 gates: 1,585 affected native cases, 15 explicit local
  original-source gates, 781 actual WASI cases, 51 browser-target WASM cases under
  Node, strict native/browser all-target Clippy, formatting and full native/browser
  builds pass. Evidence stays local in `.scratch/campaign-gameplay-20261020-v17/`
  and `.scratch/campaign-objective-20261020/`.
- [ ] The unchanged production-factory route advances through four of seven
  authored rectangles before `RemoveGuardPosition` refuses at 112.2256 seconds.
  These are route points, not completed objectives. AI guard management, authored
  ledger pickup/return, chapter victory, campaign transitions, networking and the
  required unmodified community maps remain unfinished.


### 2026-10-20 — localized defeat dialogs and simulation pause

2026-10-22 correction: selected-client startup passive alliances for
neutral/rescuable controllers are now read from executable branch operands and
applied before `main`, separately from explicit script writes. The unchanged
Human01 no-input regression reaches 60 game seconds twice with Arthas alive;
the old unintended Defeat pause is removed. Core/package 1,247 and app 590 tests
pass, plus explicit source/replay gates, strict Clippy and WASM checks. General
team/force alliances, current-team getters and chapter completion remain open;
see [opening alliance evidence](docs/re-notes/hold-position.md).

- [x] Selected-source localization and hotkey queries retain file provenance and
  native operand qualification. Executed `PauseGame` lets the current script
  finish, freezes later simulation delivery, and permits queued dialog callbacks
  to resume without accumulating paused wall time.
- [x] Unit guard participation tracks source-qualified computer ownership,
  authored home positions and selected combat chase/return settings. Special
  native enrollment flags and complete computer AI remain open.
- [x] Historical VM boundary: the unchanged no-input Human01 script reached its
  localized Defeat dialog without a runtime error. The 2026-10-22 original-client
  alliance probe shows that missing startup alliances induced this defeat;
  it is not valid opening gameplay. The checkpoint verified VM/dialog delivery,
  not rendered parity or working restart/load/quit actions.
- [ ] Held inventory presentation reads selected image geometry and source skin,
  item art and text. Native rendered behavior remains unverified. Applying the
  decoded cover UV crop, asset-load rollback and effective verbose-tooltip
  preference handling remain open; item use/drag/drop explicitly refuse.
- [ ] The stateful authored ledger route reaches quest discovery and camp combat.
  Its current frontier is in the 2026-10-20 checkpoint entry below. Authored
  pickup/return and the strength reward remain unverified; chapter completion and
  campaign transitions remain open.
- [x] Checkpoint validation: 733 shared-host tests and 812 application tests
  pass, including all four pause runtime cases. Selected original localization,
  pause declaration, guard operand and inventory layout gates pass. Strict native
  and browser all-target Clippy pass. Rendered inventory/dialog comparison and
  browser runtime execution for this checkpoint remain unverified.

### 2026-10-20: checkpoint: group orders, game exit, score layout, items, armor, explosion flag

Supersedes the earlier 2026-10-20 entries for this checkpoint. Their claims of
unwired score input, click handling, and FDF or header restrictions are out of date.

- [x] Group orders: the six `Group*Order*` natives admit through
  `ObjectHost::with_group_orders` after groups, unit activity and the order registry.
  `group_order_rules::Rules` reads the point member cap from the bounded enumeration
  pushes (12 on the supplied image) and checks the immediate `-1` sentinel. Duplicate
  admission refuses. Evidence: `cargo test -p pcx-play group_` (exit 0),
  `PCX_GROUP_ORDER_IMAGE=<supplied exe> ... supplied_group_order_image -- --ignored`
  (exit 0). See [group orders](docs/re-notes/script-group-orders.md).
- [x] Game exit: `EndGame` records a bounded request and the calling thread continues.
  The runtime takes requests after `advance_campaign`, inside `CampaignExitSet`. The score
  consumer runs after that set. Evidence: `cargo test -p pcx-play game_exit` (exit 0),
  `cargo test -p peacecraft3` (exit 0, 849 passed, 0 failed). The quit-button fixture
  exercises the request path.
- [x] Score request handoff: a score request keeps the exiting session context. The
  selected `ScoreScreen.fdf`, its includes, and `GlobalStrings.fdf` load through that
  session's resolver, once per context. A context that fails to load keeps no layout.
  Evidence: same-update and missing-UI handoff tests in `script_game_exit_cases.rs`.
- [x] Score layout and view: the FDF parser accepts `Frame"TEXT"` (compact) and
  `Frame "TEXT"` (spaced), and refuses unterminated quotes. Labels are scoped to the
  score root. OK dismisses the request; Save Replay is refused and keeps it. Evidence:
  parser and button tests pass in the peacecraft3 suite (exit 0); original
  `PCX_SCORE_UI_DIR` gate (exit 0).
- [~] Score counters and replay: `GetPlayerScore` is declared by the selected `common.j`
  but no native implements it, so rows stay refused. Save Replay has no replay writer.
  No values are zero-filled.
- [ ] Score skin and model art: not implemented. The glue texture loader reads from the
  install directory, not the selected resolver, and `MODEL` frames are not drawn.
- [x] Items: `SetItemDropID` is retained setter state for the drop origin. It is not a
  drop-origin query, and no drop behavior is implemented. `GetItemLevel` reads `ilev`.
  `GetItemType` maps `icla` through the class compare chain of the selected executable
  (`PCX_WAR3_EXE` gate, exit 0, version 1.29.2.9231). Evidence: `cargo test -p pcx-play
  item_drop` and `item_query`, and the peacecraft3 suite, all exit 0.
- [x] Passive armor: held armor adds to the target's armor in `roll_damage`, once. Hero
  armor derives from agility only. Evidence: `held_armor_changes_real_damage_and_its_drop_restores_it`
  (exit 0). Overflow of the held armor sum is guarded but not tested.
- [~] Explosion flag: `SetUnitExploded` admits against the selected `common.j`
  declaration, anchored to the admitted `CreateUnit` source. A foreign-source declaration
  refuses. The flag survives `KillUnit`. The selected `nC25` model has ordinary `Death`
  but no `Death Explode` sequence. The unimplemented parts are the native explosion effect
  and its corpse consumer. Evidence: `cargo test -p pcx-play unit_explosion` (exit 0).
- [~] Stock and quest route: `UpdateStockAvailability` is a BJ function on the verified
  ledger path. Stock-management branches outside that path are unverified. The ledger
  route 3 reached quest pickup at 222 s and return at 250 s, quest complete at 251 s.
  The strength reward was not observed. Chapter victory is unverified and not reached. Restart and load are unsupported.
- [ ] Rendered parity for score, explosion, and inventory presentation is unverified.

### 2026-10-22: Learned ability tooltips

- [~] Human01 continuation: a selected-install production gate verifies Escape's
  authored cinematic cancellation and restored control/interface. The private
  ordinary-player route observes ledger completion and the strength reward but
  still loses the hero before chapter victory. Kill-based hero XP is absent;
  two original-client probes qualify initial XP, early thresholds, positive-only
  SetHeroXP and suspension, including Human01's authored level-2 cap. XP awards,
  save compatibility, victory and next-map progression remain open. See
  [experience checkpoint](docs/re-notes/hero-experience.md).

- [~] Holy Light and Water Elemental command buttons share the source-derived
  inventory tooltip style. Ranked profile strings and supported numeric references
  expand from admitted data; ranked string edits preserve explicit empty values.
  Native component hover checks pass for both spells at two resolutions on a
  private X11/software-Vulkan display. They exposed and fixed background/edge
  aspect fitting inside the source-derived frame rectangles. Core/object/app tests,
  strict Clippy, WASM and original Human01 rank/mount lifecycle gates pass; see
  `docs/re-notes/command-card.md`. Original-client visual parity, remaining command
  kinds, cast-readiness presentation and unsupported custom tooltip forms stay open.

### 2026-10-22: Spell readiness state

- [~] Holy Light and Water Elemental expose live source-ranked mana/cooldown/pause
  snapshots and share the same calculation with their cast checks. Core tests and
  every-rank installed-data checks pass, with strict native/WASM checks. Native
  disabled-texture construction inputs are traced, but readiness visuals are not
  assigned an assumed palette/state mapping. Fractional Holy Light fields and the
  selected script's cooldown-query/reset natives remain follow-ups.

### 2026-10-22: Fractional Holy Light fields

- [~] Holy Light now reads ranked cooldown, range and healing as real values,
  while preserving signed-integer mana-cost bounds. The shared ranked target-list
  read accepts supported numeric map edits instead of blanket refusal. Synthetic
  casts verify fractional range/healing/cooldown and per-rank edits; invalid values
  refuse before mana or healing writes. Core 1,093 and app 589 tests pass, along
  with the installed-data gate, strict Clippy and affected native/WASM checks.
  Original-client fractional-cast parity and readiness visuals remain unqualified.

### 2026-10-22: Script cooldown remaining/end

- [~] Source-declared remaining/end cooldown natives now share the live Holy Light
  and Water Elemental deadlines. Compiled synthetic scripts cover recasting after
  reset, per-unit/per-ability isolation, summon cast-point timing, absence paths,
  refusal before mutation, and unchanged query RNG/clock/pools. Core 1,097 and app
  589 tests pass, plus the selected-install binding gate, strict Clippy and WASM.
  Other ability classes, item groups, duration getters/setter and whole-unit reset
  remain unfinished; native timing parity is not claimed.

### 2026-10-22: Change-level request target

- [~] `ChangeLevel` now admits against the selected `common.j`, validates the
  `string, boolean` signature and same source as `CreateTimer`, and queues a
  request-local verbatim target with its score flag. `EndGame` queues no target.
  The existing `SetNextLevel` ambient request remains separate and is not inherited
  by either request. Bad arity/type, NUL, oversized targets and a full queue refuse
  without changing earlier requests. Evidence: `cargo test -p pcx-play --lib`
  (1,180 passed, 34 ignored), strict `cargo clippy -p pcx-play --all-targets --
  -D warnings`, WASM `cargo check -p pcx-play --lib`, and the ignored
  selected-`common.j` gate with `PEACECRAFT3_COMMON_J`, all exit 0. The app-owned
  map transition, save handoff and score presentation remain open; see
  [game exit](docs/re-notes/script-game-exit.md).
