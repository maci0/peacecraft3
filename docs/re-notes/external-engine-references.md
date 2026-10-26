# External engine references

Reviewed: 2026-10-26. This is a reference index, not a dependency or code-import plan.

## Use policy

Do not copy, port, translate, or adapt these projects’ code into PeaceCraft. Use them to discover concepts, missing behavior, file-format edge cases, and test scenarios. Implement independently in the existing Rust/Bevy architecture. Validate proposed behavior against the supplied assets and original runtime/native evidence; another recreation is not proof of Warcraft III fidelity.

Game data remains local. Do not import external repositories’ bundled game assets or generated copies. No external project was built or run during this review. Feature descriptions below reflect repository documentation and inspected source organization, not verified gameplay or rendering parity.

## Priority references

| Project | Type | Useful questions for PeaceCraft |
|---|---|---|
| [Warsmash](https://github.com/Retera/WarsmashModEngine) | Java/LibGDX Warcraft III emulation engine | JASS native coverage, ability and unit lifecycles, construction, movement, campaign flow, animation selection. |
| [OpenRealm](https://github.com/corepunch/open-realm) | C engine; WC3 primary, WoW/SC2 additional targets | JASS/FDF implementation boundaries, campaign transitions, runtime/renderer separation, native and browser targets. Likely candidate for the remembered multi-game engine. |
| [OpenWarcraft3 (game-a11y)](https://github.com/game-a11y/OpenWarcraft3) | SDL2/C WC3 implementation; fork of OpenRealm | Earlier `src/` architecture, JASS API dispatch, routing, FDF, MDX and fog-of-war leads. Do not count shared ancestry as independent evidence. |
| [WhiteoutFlakes](https://github.com/FernandoS27/WhiteoutFlakes) | Multi-game rendering library | WC3 model adapters, animation, materials, particles, ribbons, render-profile separation. Handles WC3 and WoW formats, among others. |
| [mdx-m3-viewer](https://github.com/flowtsohg/mdx-m3-viewer) | TypeScript/WebGL viewer and parsers | MDX hierarchy, interpolation, PRE2/ribbons, map parsing, visual-regression scenarios. README says no longer actively maintained. Not Three.js. |
| [war3-model](https://github.com/4eb0da/war3-model) | TypeScript WC3 converter/renderer | MDL/MDX parsing and particle/ribbon rendering edge cases. Not a complete gameplay engine. |
| [Wowser Scene](https://github.com/wowserhq/scene) | Three.js WoW rendering library | Actual Three.js model-animation and terrain-material abstractions; uses @wowserhq/format and WebGL 2. |
| [Wowser](https://github.com/wowserhq/wowser) | Browser WoW client proof of concept | Asset loading, background work, terrain/world streaming, transport and rendering boundaries. Separate from the newer scene package. |
| [WoWee](https://github.com/Kelsidavis/WoWee) | Native C++ WoW replacement client with Vulkan renderer | Asset discovery/import provenance, animation/particles, Lua UI integration, client subsystem organization. WoW gameplay is not WC3 behavior evidence. |
| [WebWowViewerCpp](https://github.com/Deamon87/WebWowViewerCpp) | C++ WoW model/map viewer, native and WebGL frontends | Animation, visibility, particles/ribbons, native/browser rendering comparisons. No root license was detected in this review. |

## Pinned source entry points

Links below name the reviewed commit, so later upstream changes do not silently change the reference. The JSON companion records exact revisions and SHA-256 values of locally inspected source files.

### 4eb0da/war3-model

Reviewed commit: [`52d7f421a550`](https://github.com/4eb0da/war3-model/tree/52d7f421a5505a0434d764294057f2009ce3987b).

PRE2/ribbon edge cases and parser/renderer test cases.

Inspected source entry points:
- [renderer/particles.ts](https://github.com/4eb0da/war3-model/blob/52d7f421a5505a0434d764294057f2009ce3987b/renderer/particles.ts)
- [renderer/ribbons.ts](https://github.com/4eb0da/war3-model/blob/52d7f421a5505a0434d764294057f2009ce3987b/renderer/ribbons.ts)

### Deamon87/WebWowViewerCpp

Reviewed commit: [`1a8cccbeffc4`](https://github.com/Deamon87/WebWowViewerCpp/tree/1a8cccbeffc46231c6497e6b3f5bfbf3507d8071).

Animation, visibility, particles, ribbons and browser/native rendering comparison; no root license detected.

Repository README, license metadata, and source tree reviewed; no subsystem implementation verified.

### FernandoS27/WhiteoutFlakes

Reviewed commit: [`d2756ce11e0b`](https://github.com/FernandoS27/WhiteoutFlakes/tree/d2756ce11e0bbb5dac43b97ea5cf1627d53a402b).

WC3 MDX animation/material/particle/ribbon adapters and format-to-render profile boundaries.

Inspected source entry points:
- [src/io/mdx_model_adapter.cpp](https://github.com/FernandoS27/WhiteoutFlakes/blob/d2756ce11e0bbb5dac43b97ea5cf1627d53a402b/src/io/mdx_model_adapter.cpp)
- [src/renderer/core/particle_dialect.h](https://github.com/FernandoS27/WhiteoutFlakes/blob/d2756ce11e0bbb5dac43b97ea5cf1627d53a402b/src/renderer/core/particle_dialect.h)
- [src/renderer/particle/base/particle_adapters_wc3_m2.cpp](https://github.com/FernandoS27/WhiteoutFlakes/blob/d2756ce11e0bbb5dac43b97ea5cf1627d53a402b/src/renderer/particle/base/particle_adapters_wc3_m2.cpp)

### Kelsidavis/WoWee

Reviewed commit: [`9cf6238fd4ef`](https://github.com/Kelsidavis/WoWee/tree/9cf6238fd4efb71a696e2b695bcdfe22c33f6076).

Asset import/provenance, M2 particles, Lua/UI integration; WoW mechanics are not WC3 evidence.

Inspected source entry points:
- [src/addons/lua_engine.cpp](https://github.com/Kelsidavis/WoWee/blob/9cf6238fd4efb71a696e2b695bcdfe22c33f6076/src/addons/lua_engine.cpp)
- [src/pipeline/asset_manager.cpp](https://github.com/Kelsidavis/WoWee/blob/9cf6238fd4efb71a696e2b695bcdfe22c33f6076/src/pipeline/asset_manager.cpp)
- [src/rendering/m2_renderer_particles.cpp](https://github.com/Kelsidavis/WoWee/blob/9cf6238fd4efb71a696e2b695bcdfe22c33f6076/src/rendering/m2_renderer_particles.cpp)

### Retera/WarsmashModEngine

Reviewed commit: [`f9e0aeed4be3`](https://github.com/Retera/WarsmashModEngine/tree/f9e0aeed4be372d6016519d0e97b384aa873f374).

Gameplay/JASS/ability lifecycle, sequence selection, movement, construction and campaign hypotheses.

Inspected source entry points:
- [core/src/com/etheller/warsmash/viewer5/handlers/w3x/SequenceUtils.java](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/w3x/SequenceUtils.java)

### corepunch/open-realm

Reviewed commit: [`2cea7464ad1f`](https://github.com/corepunch/open-realm/tree/2cea7464ad1f3bb466dafcdc98b04964981ac1c3).

JASS lifecycle, FDF, campaign flow, MDX rendering; inspect games/warcraft-3 and shared format boundaries.

Source-tree leads (located, not yet behavior-reviewed):
- [games/warcraft-3/game/skills/s_mirror_image.c](https://github.com/corepunch/open-realm/tree/2cea7464ad1f3bb466dafcdc98b04964981ac1c3/games/warcraft-3/game/skills/s_mirror_image.c)
- [games/warcraft-3/jass](https://github.com/corepunch/open-realm/tree/2cea7464ad1f3bb466dafcdc98b04964981ac1c3/games/warcraft-3/jass)
- [games/warcraft-3/renderer/mdx/r_mdx_anim.c](https://github.com/corepunch/open-realm/tree/2cea7464ad1f3bb466dafcdc98b04964981ac1c3/games/warcraft-3/renderer/mdx/r_mdx_anim.c)
- [docs/games/warcraft-3/unit-animation-properties.md](https://github.com/corepunch/open-realm/tree/2cea7464ad1f3bb466dafcdc98b04964981ac1c3/docs/games/warcraft-3/unit-animation-properties.md)

### flowtsohg/mdx-m3-viewer

Reviewed commit: [`2ff0bc00c636`](https://github.com/flowtsohg/mdx-m3-viewer/tree/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc).

MDX particles/ribbons, hierarchy, sequence interpolation, map parsing and visual regression ideas.

Inspected source entry points:
- [src/viewer/handlers/mdx/particle2.ts](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/viewer/handlers/mdx/particle2.ts)
- [src/viewer/handlers/mdx/ribbon.ts](https://github.com/flowtsohg/mdx-m3-viewer/blob/2ff0bc00c6363f425016e23d88c0fb2929d3b3cc/src/viewer/handlers/mdx/ribbon.ts)

### wowserhq/scene

Reviewed commit: [`cbe1211ab658`](https://github.com/wowserhq/scene/tree/cbe1211ab658965667bd4bcd61561b11a944028e).

ModelAnimation and terrain material classes; useful browser rendering concepts, not WC3 gameplay.

Inspected source entry points:
- [src/lib/map/terrain/TerrainMaterial.ts](https://github.com/wowserhq/scene/blob/cbe1211ab658965667bd4bcd61561b11a944028e/src/lib/map/terrain/TerrainMaterial.ts)
- [src/lib/model/ModelAnimation.ts](https://github.com/wowserhq/scene/blob/cbe1211ab658965667bd4bcd61561b11a944028e/src/lib/model/ModelAnimation.ts)

### wowserhq/wowser

Reviewed commit: [`5fcd3e607db7`](https://github.com/wowserhq/wowser/tree/5fcd3e607db7551a74caf6d7ffa4dd785a264dc8).

Asset transport, workers, world/terrain loading and network/client separation.

Repository README, license metadata, and source tree reviewed; no subsystem implementation verified.

### game-a11y/OpenWarcraft3

Reviewed commit: [`a632312e93f8`](https://github.com/game-a11y/OpenWarcraft3/tree/a632312e93f82abc42e484b49fadc40acdf23474). GitHub identifies this repository as a fork of [corepunch/open-realm](https://github.com/corepunch/open-realm). Its README describes SDL2/Linux/macOS, original v1.0 assets and ongoing v1.29b support. These are project claims, not tested compatibility.

Source-tree leads (located, not behavior-reviewed):

- [JASS API unit handlers](https://github.com/game-a11y/OpenWarcraft3/blob/a632312e93f82abc42e484b49fadc40acdf23474/src/game/api/api_unit.h)
- [Routing](https://github.com/game-a11y/OpenWarcraft3/blob/a632312e93f82abc42e484b49fadc40acdf23474/src/common/routing.c)
- [Game pathing](https://github.com/game-a11y/OpenWarcraft3/blob/a632312e93f82abc42e484b49fadc40acdf23474/src/game/g_pathing.c)
- [MDX renderer](https://github.com/game-a11y/OpenWarcraft3/tree/a632312e93f82abc42e484b49fadc40acdf23474/src/renderer/mdx)
- [FDF UI](https://github.com/game-a11y/OpenWarcraft3/tree/a632312e93f82abc42e484b49fadc40acdf23474/src/game/ui)

No root LICENSE was found at this revision; do not assume the newer upstream license applies to every fork file. No accessibility-specific feature was verified from the organization name. The README's asset-download target was not run; reference work uses only the supplied local assets. Follow the shared no-code-reuse policy above.

## Archive-format references

[StormLib](https://github.com/ladislav-zezula/StormLib) and [libmpq](https://github.com/mbroemme/libmpq) are directly relevant to `crates/pcx-mpq`. Use them to identify MPQ edge cases and design independent synthetic fixtures: malformed header/table bounds, encrypted sectors, multi-codec pipelines, patch application, locale/name lookup, checksums and truncated input. Compare archive membership and extracted-byte hashes when an independently built reference tool is separately authorized. An extractor agreeing with another extractor does not prove original-game mount precedence or engine behavior.

The no-code-reuse rule includes codec implementations, lookup tables and test fixture contents. Do not import upstream archive fixtures without checking their provenance; generate our own synthetic cases and use supplied original archives locally. No new dependency, build or differential run was added here.

StormLib root license is MIT. libmpq metadata identifies LGPL-2.1, with both COPYING and COPYING.LESSER present; file/dependency terms still need separate review if reuse is ever proposed.

### ladislav-zezula/StormLib

Reviewed commit: [`116135c27e8c`](https://github.com/ladislav-zezula/StormLib/tree/116135c27e8c0dcf8ea0a0c549a6d737ce10fc73). Archive headers/tables, sector reads, encryption, chained compression, patch archives, locale lookup and integrity test scenarios.

Source-tree leads (located, not behavior-reviewed):

- [src/SFileOpenArchive.cpp](https://github.com/ladislav-zezula/StormLib/blob/116135c27e8c0dcf8ea0a0c549a6d737ce10fc73/src/SFileOpenArchive.cpp)
- [src/SFileReadFile.cpp](https://github.com/ladislav-zezula/StormLib/blob/116135c27e8c0dcf8ea0a0c549a6d737ce10fc73/src/SFileReadFile.cpp)
- [src/SCompression.cpp](https://github.com/ladislav-zezula/StormLib/blob/116135c27e8c0dcf8ea0a0c549a6d737ce10fc73/src/SCompression.cpp)
- [src/SFilePatchArchives.cpp](https://github.com/ladislav-zezula/StormLib/blob/116135c27e8c0dcf8ea0a0c549a6d737ce10fc73/src/SFilePatchArchives.cpp)
- [test/StormTest.cpp](https://github.com/ladislav-zezula/StormLib/blob/116135c27e8c0dcf8ea0a0c549a6d737ce10fc73/test/StormTest.cpp)

### mbroemme/libmpq

Reviewed commit: [`142f5ba26b23`](https://github.com/mbroemme/libmpq/tree/142f5ba26b2387bcc3fb5a3bd7ef928d3d776374). Independent archive API/format comparison, malformed-input cases, codec/crypto tests, patch readers and bounded/fuzz scenarios.

Source-tree leads (located, not behavior-reviewed):

- [src/mpq-compression.c](https://github.com/mbroemme/libmpq/blob/142f5ba26b2387bcc3fb5a3bd7ef928d3d776374/src/mpq-compression.c)
- [src/mpq-patch-reader.c](https://github.com/mbroemme/libmpq/blob/142f5ba26b2387bcc3fb5a3bd7ef928d3d776374/src/mpq-patch-reader.c)
- [tests/test-mpq-malformed.c](https://github.com/mbroemme/libmpq/blob/142f5ba26b2387bcc3fb5a3bd7ef928d3d776374/tests/test-mpq-malformed.c)
- [tests/test-mpq-crypto.c](https://github.com/mbroemme/libmpq/blob/142f5ba26b2387bcc3fb5a3bd7ef928d3d776374/tests/test-mpq-crypto.c)
- [fuzz/fuzz-encrypted-archive.c](https://github.com/mbroemme/libmpq/blob/142f5ba26b2387bcc3fb5a3bd7ef928d3d776374/fuzz/fuzz-encrypted-archive.c)

## Nirvana: a different category

[Nirvana’s publisher release](https://www.moddb.com/downloads/warcraft-iii-nirvana-release-10) describes `Nirvana.dll` as modified from `Game.dll`, with a RenderCraft graphics plugin. Treat it as an original-engine binary modification, not an independent replacement engine. A public source repository and its implementation details were not verified. Do not infer a particular assembly workflow merely from the modified-DLL description.

## Licensing and provenance

The no-code-reuse rule applies regardless of license. Metadata at the reviewed revisions:

- Warsmash: AGPL-3.0; its README also discusses differently licensed incorporated components.
- WoWee: MIT text with an additional commercial-game restriction; music/audio have separate reserved terms. It is not plain MIT.
- WhiteoutFlakes: BSD-3-Clause root license; additional notices and dependencies require their own review if reuse is ever separately proposed.
- OpenRealm, Wowser, Wowser Scene, mdx-m3-viewer, and war3-model: root metadata identifies MIT.
- WebWowViewerCpp: no root license detected; do not assume public visibility grants reuse rights.

No third-party implementation was incorporated. Local public-source snapshots and metadata are under ignored `.scratch/external-engine-reference-20261026/`; no original game assets were fetched.

## Applying references to current gaps

1. Start with a specific gap and inspect the corresponding subsystem, rather than importing an engine or framework.
2. Record an external implementation as a hypothesis, including repository revision and source location.
3. Turn the hypothesis into synthetic parser/lifecycle/render tests and a bounded original-client check where behavior is uncertain.
4. Derive authored values from PeaceCraft’s selected assets and preserve edition/version provenance.
5. Implement with existing canonical stores, scheduling, event delivery, and rendering paths. Keep unverified native/render differences explicit.

For current campaign work, Warsmash and OpenRealm are the first places to investigate lifecycle edge cases. For remaining MDX effects and material gaps, WhiteoutFlakes, mdx-m3-viewer, and war3-model are the stronger starting points. Use WoW projects for cross-cutting rendering and client architecture ideas, not Warcraft III rules.
