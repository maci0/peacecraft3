# Implementation completion scope

This guide defines the remaining acceptance work. See the [project plan](../PLAN.md)
and [asset provenance rules](../AGENTS.md). The reference-engine implementations
linked here inform design and comparison. Licensed dependencies are separately
identified in the repository.

## Acceptance levels

Track each item separately through bounded parsing, canonical runtime integration, public input/presentation, and unchanged-map acceptance. Parser round trips, synthetic tests, native-code analysis, original-client observations and rendered comparisons establish different claims. Never close a gameplay or visual item solely because its parser passes.

## Work order and gates

| Area | Existing implementation / evidence | Remaining completion gate |
|---|---|---|
| Recorded campaign milestones | Human01 authored victory repeated; Human02 earned base, boss defeat and player Victory passed at tick 72769 / simulation time 1137.015625 with six surviving footmen | Later runs carried the earned Human02 state through the interlude into Human03. Complete the remaining campaign and community-map gates. See the [tracker](../TRACKER.md) and [Human campaign handoff](handoff-human06.md) |
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

## Earlier Human02 checkpoint

Before the earned victory run, the support route reached the boss and outro.
A harness assertion had treated hidden units as absent. The short outro
confirmed that hidden units remained live and left hidden-world gold-harvest
continuation open. Original-client probes separately observed mine exit and
travel to a hidden depot. Deposit while hidden was still under measurement.

See [external references](re-notes/external-engine-references.md) for the pinned inspiration-only index. Existing historical tracker entries retain their original slice results; do not infer that every older frontier is still active.

## Source-audited next format gaps (2026-10-26)

Campaign continuation must distinguish mission and interlude startup. The current `campaign_human_route::continue_campaign` helper assumes a newly restored Arthas and a score continuation, which is specific to Human01→Human02. Its corrected receipt-member assertion still needs a complete earned-route transition run; the shorter diagnostic establishes a different scope. The supplied Human02Interlude script has no cache restore/store calls; its victory action at lines 1689–1690 selects Human03 with both victory dialog/scores flags false. Reuse the production transition lifecycle, but assert hero restoration and score controls only where the authored chapter requests them. This source audit does not verify interlude execution or Human03 carryover.

The carryover acceptance must also cover multiple heroes. Human03's `Trig_Load_Heroes_Actions` restores Arthas from the Human03 cache entry and otherwise creates a fallback hero; an earned-chain check must distinguish those paths. Its next-level preparation stores both Arthas and Jaina for Human04, whose loader restores each separately. A successful Arthas-only comparison cannot establish that later transition's complete carryover. Derive hero entries from the authored cache calls and compare every restored persistent snapshot; do not count fallback creation as a restored campaign state.

These are concrete remaining surfaces identified in the current source, not an exhaustive inventory. An explicit unsupported diagnostic is a safe boundary, not evidence that every input reaching it should be accepted.

| Surface | Current source evidence | Next implementation acceptance |
|---|---|---|
| Editor-generated control flow | `pcx-trig/src/lower.rs` has an `EDITOR_EXPANDED` refusal set for multiple-action Boolean/loop/group/force/custom-script constructs; initially-off custom text and custom-text run-on-init are separately refused | Lower each construct from selected TriggerData/BJ/editor output, preserve callbacks and variable scoping, compare generated semantics and unchanged GUI maps |
| W3G actions and startup | `pcx-net/src/w3g_action.rs` admits the qualified game-version29/build6060 layout and refuses other actions at their exact byte; object pairs remain unresolved file identities | Qualify additional action/version layouts with locally owned captures; map slots and object IDs explicitly; replay through reconstructed production startup and exact command phases, not the separate simulation harness |
| Archive compression combinations | `pcx-mpq/src/payload.rs` has bounded codec-mask checks and explicit LZMA/BZIP2/other-chain refusals; sparse and sparse+zlib have dedicated paths | Audit each source-qualified codec/chain with synthetic bounded data, exact output checks and malformed-input refusal before broadening acceptance; preserve imported-table provenance |
| Object-family runtime semantics | `pcx-objects/src/lib.rs` currently accepts all seven named object extensions for v1/v2, with indexed records selected by family/version | Audit map overrides, metadata interpretation and canonical consumers per family; do not add a second parser merely because an older census said a family was unmeasured |

The trigger-lowering module's introductory comment is less current than its implementation: custom text already has a path, with specific restrictions. Any change must inspect actual branches and callers rather than treating the whole feature as absent. Similarly, archive rejection variants include malformed-input protections, not only missing codecs.

The campaign test remains the immediate runtime priority. These independent format gaps can be selected at a coordinated source/build boundary; no extra implementation or Cargo job is started while the full mission inputs are frozen.
