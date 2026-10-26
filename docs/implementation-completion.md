# Implementation completion scope

Renewed goal: implement all missing formats and functionality. This expands the acceptance backlog; it does not mean the game or any format family is complete. Target and asset-provenance rules remain those in PLAN.md and AGENTS.md. External projects are inspiration only; no third-party code is copied, ported, translated or adapted.

## Acceptance levels

Track each item separately through bounded parsing, canonical runtime integration, public input/presentation, and unchanged-map acceptance. Parser round trips, synthetic tests, native-code analysis, original-client observations and rendered comparisons establish different claims. Never close a gameplay or visual item solely because its parser passes.

## Work order and gates

| Area | Existing implementation / evidence | Remaining completion gate |
|---|---|---|
| Current campaign frontier | Human01 authored victory repeated; Human02 base, combat and cinematic slices run through production paths | Finish Human02 outro and verify earned player Victory, real continuation and carryover; then remaining Human chapters/interludes, other campaigns and community maps |
| Archives and install/import | MPQ/CASC and installer/import modules exist; archive evidence and bounds retained | Inventory unsupported codec/version/patch/locale/corruption cases against current code; test actual native/browser import and mount precedence |
| Map containers and world data | Map info, terrain, placements, pathing, regions, cameras, sounds, shadows and minimap modules exist | Version/variant census plus runtime consumers for every required map member; unchanged custom-map acceptance |
| Object edits and metadata | Current ObjectFile parser accepts all seven w3u/w3t/w3a/w3b/w3d/w3h/w3q families for supported v1/v2 layouts | Verify field/metadata/overlay semantics and runtime behavior per family; older PLAN census/refusal wording is historical, not current parser capability |
| Scripts and trigger graph | JASS VM/native host and WTG/WCT/WTS parsing/lowering exist | Complete reachable native semantics, scheduling, handle lifetime, callbacks, GUI/raw-script equivalence and map-version compatibility; no successful no-op substitutes |
| Models, textures and effects | MDX/MDL, classic and selected later-model records, BLP/DDS and source-driven animation/effects paths exist | Unsupported chunks/versions, materials, attachments, emitter/ribbon/buff/art lifecycle, exact source rendering and sound comparisons |
| Simulation and public controls | Canonical units, combat, economy, construction, training, abilities and source-derived command surfaces exist in scoped slices | Complete missing abilities, items/shops, build styles, upgrades/research, pathing/vision, AI, resurrection and lifecycle edges through public controls |
| UI and audio/video | Source FDF/skin/fonts/menu/HUD, selected sound/movie paths exist | Remaining panels/controls, layouts, text overflow, input semantics, audio timing/mixing and original paired comparisons |
| Saves and replays | Campaign profile carryover and bounded original W3G container/action decoding exist | Production save/load, reconstructible recording/playback, exact input phase/ticks, checked original object mapping and qualified interoperability; container roundtrip is not playback |
| Multiplayer | Transport/session/lobby/codec components exist | Real initialized matches, deterministic economy/combat, map transfer/admission, original LAN compatibility and hosted-session security |
| Browser and packaging | Native/WASM builds and local import/launcher paths exist | Full browser gameplay/import matrix, bounded memory/async I/O, native OS packaging and actual isolated end-to-end tests |

## Inventory discipline

This is the initial acceptance map, not an exhaustive audited list of unsupported formats or natives. Reconcile PLAN.md, TRACKER.md, source refusals, ignored local-source gates and current receipts before selecting each slice. Record exact versions, source identities, public consumers and failure behavior. Keep safe unsupported states until replaced by a tested implementation; do not remove refusals simply to make a map run.

The active tracked inventory task lives in the existing local campaign Taskwarrior database. Implementation continues in bounded connected slices with coordinated file/Cargo ownership. Native/WASM strict checks, affected builds, meaningful synthetic tests and selected original-asset gates precede completion claims. Automated game/reference tests remain headless and muted. Supplied game data and generated asset copies stay local and uncommitted.

## Current evidence boundary

Human02 has not yet been verified won in the recreation. The support route reached the boss/outro boundary; a harness assertion incorrectly treated authored-hidden units as absent. The short outro confirms hidden-live identity and currently exposes hidden-world gold-harvest continuation. Original probes establish mining exit and retained hidden-depot travel; deposit-while-hidden is under measurement. Original-client Human02 victory is separate evidence and does not close recreation completion.

See [external references](re-notes/external-engine-references.md) for the pinned inspiration-only index. Existing historical tracker entries retain their original slice results; do not infer that every older frontier is still active.
