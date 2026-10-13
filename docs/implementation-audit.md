# Implementation audit and parallel workstreams

Reviewed 2026-10-14 against all of `TRACKER.md`, the corresponding plan gates,
current source APIs and the CPU test baseline. Checkboxes are historical evidence,
not a reliable measure of game completeness. New implementation changes are separately
validated before integration.

## Corrections to the tracker

- The scripting crate already contains a resolver, typed values, interpreter and
  host interface. The statement that it stops at syntax is stale. Most game
  natives and original event ordering are still missing; parser/VM tests do not
  close the campaign or community-map gates.
- Map info (`pcx-map::info::MapInfo`) and terrain mesh generation already exist.
  Their unchecked rows need reconciliation. Terrain blending, cliffs, water,
  pathing semantics and the actual map render path remain incomplete.
- The deterministic movement core, authority, wire codec, rollback and local
  browser transport exist. The unchecked foundational simulation/server rows
  conflict with the multiplayer section. Combat, economy and a playable match
  remain missing.
- Original button plates, TFT background, transparent logo and logo entrance
  render in the browser. Older menu rows saying they are not rendered are stale.
  FDF-derived inner blue backdrops are implemented pending updated runtime proof; hanging sprites, state art, options/HUD and exact
  material/lighting/animation behavior remain open.
- A model's framing or a rendered smoke image does not establish complete model
  semantics. Layer decoding, material alpha, helper animation, texture animation,
  particles and original projection/render behavior need separate gates.
- Local PE resources identify the target executable as `1.29.2.9231` and the
  compatibility `Game.dll` as `1.26.0.6401`. Historical archive labels do not
  establish that every adjacent file belongs to the same release.
- Four native targets passed at `d31dc67`; downloaded artifact checksums and executable architectures were independently verified.
  Compilation, packaging and unit tests do not prove native GPU/audio behavior.
- The startup movie implementation does not close campaign cinematics, original
  dispatcher behavior or frame-perfect A/V parity.

The reviewed CPU baseline passed script 96, triggers 52, play 14 plus one CLI,
network 43 and simulation 8 tests. Original-asset fixtures remain explicit and
ignored by default. These counts identify a baseline, not a fidelity result.

## Full backlog and dependencies

| Area | Existing foundation | Missing work and acceptance gate | Workstream |
| --- | --- | --- | --- |
| M0: evidence | Local binaries, snapshots, RE driver and format notes | Native tick/RNG/event order, archive priority, live glue flags, original capture determinism; repair driver defects and preserve evidence identity | Integration and subsystem owners |
| M1: archives | MPQ tables, encryption, lookup, sector extraction, zlib/implode | Correct single-unit/legacy dispatch and malformed offsets; Huffman/ADPCM/sparse; extraction CLI; byte-identical Storm comparison | Formats |
| M1b: playtest harness | Canonical traces, diff, deterministic scheduler | Strict scenario ingestion, action/assertion coverage, runner, minimizer, 100-run gates and original baselines | Scripting/campaign/playtest |
| M1c: Reforged assets | Local storage inventory | Encoding/BLDR content reads, path-to-key resolution, layered provenance and overrides; same viewer path must load an actual Reforged model | Queued asset pipeline |
| M2: images/models/audio tables | BLP/TGA/SYLK and model framing, meshes, clips and skinning | Complete authored orientation/interpolation/bindings/layers; helpers, effects and texture animation; async loaders/cache; original material and render parity | Formats, then renderer |
| M3: maps/world | Map container, terrain corners, doodads, shadows, minimap and map info | Regions, preplaced units/items/random tables, slot blending, cliffs/water/pathing and camera rules; Human01 and community maps must load/render correctly | Formats, then world integration |
| M4: objects | Shipped editor schemas, metadata joins and map edits | Full abilities/items/upgrades/buffs/destructables, tech/availability, allied/random tables and absent extension layouts supported by evidence; stock stats must match original | Formats/data, then gameplay |
| M5: simulation | Fixed-point movement and command validation | Stats, damage, attacks/orders/pathing, abilities/buffs, costs/cooldowns/upkeep, projectiles and death/XP; original duel curves and RNG draws must match | Queued gameplay |
| M6: loop parity | Deterministic project clock and rollback | Original tick/RNG, event/sound/effect order, save/load and 100-run ten-minute matches; project-selected constants are not native parity | Gameplay and evidence |
| M7: scripting | Syntax, resolver/value/interpreter, WTG/WCT/WTS containers | Native game host, handles/casts, trigger desugaring, declared script sources, localization, timers and era tolerance; DotA/Athena/Footman Frenzy must execute without hidden refusals | Scripting/campaign/playtest |
| M8: campaign/polish | Startup movie and partial original menu | Campaign AI/cinematics, dialogue/quests/victory, HUD/minimap/command card/chat/tooltips and spatial sound; original behavior must be observed | Campaign, renderer and gameplay |
| M9: playthrough gates | Scenario/trace foundation | Every chapter's concrete outcomes, original baselines, seeded fuzzing/minimization and no unexplained divergences; no synthetic win substitutes | Scripting/campaign/playtest |
| M10: multiplayer | UDP/LAN, authority/rollback, local WebSocket movement | Actual game/lobby integration, rooms/admission/TLS/identity, original LAN/Battle.net compatibility and completed matches; transport success is insufficient | Queued networking |
| M11: browser | WASM/WebGL renderer, memory assets and movie | Local ISO discovery/import, archive-to-asset routing, async loading, actual WebGPU validation, full game runtime and exact resolution/menu fidelity | Queued browser integration |
| Desktop | Four OS/CPU build and packaging jobs | Latest-source artifacts and native GPU/audio/input/cinematic verification; signing/installers and older OS support remain separate | Integration |
| Performance | F8 frame diagnostics, strict profiling feature checks and a deterministic movement workload | F8 overlay, CPU/render traces, fixed-work allocation/instruction budgets as each subsystem lands; timing/FPS are report-only under contention | Integration/performance |
| Menu fidelity | Original font/art/scene/logo, relative layout | Parse FDFs instead of copied values; blue fill/state edges, hanging panel/entrance, live camera/tints, scene framing, options and original input/sound | Menu integration, then renderer |

## Implementation order

Keep parser/data, scripting/playtest and menu changes isolated until their
interfaces and targeted tests are stable. Integrate those results into the app
and renderer before claiming runtime behavior. Coordinate manifest and lock
updates and schedule expensive shared builds against a stable source snapshot.

The next dependent scopes are gameplay/world integration, browser-local ISO
import, multiplayer match integration, Reforged asset reads and renderer fidelity.
Completing a parser or transport slice does not complete those dependent features.

Archive correctness and authored format semantics unblock rendering and asset
import. Map/object data and script-host execution unblock actual campaigns and
gameplay. Multiplayer match integration depends on that real gameplay. Campaign
completion and community-map compatibility cannot be claimed before these
dependencies close, regardless of unit-test totals.

All work follows `AGENTS.md`: derive asset-defined data at build/runtime, keep
original assets local, preserve native behavior, report unsupported states and
keep performance gates independent of unrelated machine load.
