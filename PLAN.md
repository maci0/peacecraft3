# PeaceCraft3 — Faithful Warcraft III Reimplementation

Target: **Warcraft III: The Frozen Throne 1.29b** — the last stable legacy release —
reimplemented in Rust on the Bevy engine using the *original* game assets read directly
from the shipped MPQ archives.

Earlier classic builds are not a goal. 1.28c happens to still work through the same
code, and its install is still present, so it costs nothing to leave working; it is no
longer a requirement and nothing about it is allowed to shape a decision.

This is a recreation, not a full decompilation or recompilation. The target is
100% fidelity to the source material's appearance, sound, feel and observable
behavior; internal architecture and conventions may differ. Read content and
settings from the supplied assets rather than copying measured values into code.
Support fidelity claims with the appropriate shipped-byte, native-code or runtime
evidence (see [§2](#2-reverse-engineering-with-rea)); distinguish implementation
choices from proven original behavior, and keep unresolved differences visible.

### Non-negotiable requirement: original maps must work, unmodified

**Loading the campaign is not the acceptance bar. The original community's maps must run.**

A Warcraft III reimplementation that renders terrain but cannot run triggers is a
renderer, not Warcraft III. The real test is maps the community actually plays and
ships, unchanged, with their own scripting:

| Map | Why it is the bar |
|---|---|
| **DotA** (Defense of the Ancients, and its DotA Allstars / 6.x descendants) | The canonical case. Multi-player, enormous JASS codebase, no map-maker involvement in the code, heavy reliance on timer ordering, unit-array manipulation, handle semantics and native timing. If DotA runs correctly the scripting layer is real; if it does not, we have a toy. |
| **Athena** (Athenian Bloodlines) | The canonical single-player custom campaign: authored in the editor's trigger system (not raw JASS), which is a *different* code path from DotA and exercises the `WTG` trigger graph rather than the script file. |
| **Footman Frenzy** and other classic-era maps | Pre-1.20 era APIs — pre-`Blz` natives, older `BJ` library shape, earlier object-rawcode conventions. A modern-only scripting layer silently fails here. |
| **JASS`-first` community maps** | Maps that ship a hand-written `war3map.j` and no trigger graph at all. |
| Editor-authored maps with custom objects | `W3U`/`W3T`/`W3A`/`W3B`/`W3D`/`W3H`/`W3Q` object edits, `WTS` localized strings, `WPMP` allied shared data. |

This forces three things that are otherwise optional and must therefore be treated as
core scope, not stretch goals:

1. **The whole map container, not just terrain.** Every file a shipped custom map
   contains, parsed — see [§3.11](#311-map-compatibility-the-real-acceptance-bar).
2. **The scripting engine, both code paths.** Raw JASS **and** the editor's trigger
   graph (`WTG`), which desugars to JASS and must produce the *same* semantics, plus
   the `BJ`/`Blz` library surface, natives, timers, and event ordering.
3. **Version tolerance across the community's era.** The community spans pre-1.20 and
   post-1.24 map APIs in active use simultaneously.

**Honest scoping.** This is the largest single piece of work in the project and it is not
optional. It is stated here so that earlier milestones are not mistaken for a finished
product: a working `pcx-map` terrain loader plus a render loop is roughly the first
third of what "Warcraft III" means. The milestone order below is unchanged, but M4/M5/M7
carry this requirement, and [§6](#6-definition-of-faithful) is gated on it.

### Supported versions

The installed classic 1.29b release is the target. Other local artifacts are compatibility evidence, with their identities recorded separately:

| Version | Role | Status |
|---|---|---|
| **1.29b** | **The target.** Battle.net product `w3_legacy`; installed executable VERSIONINFO is `1.29.2.9231`. Earlier `9232` label withdrawn. | installed; executable identity measured |
| Extracted compatibility tree | Not a target. `wc3/game/app/game.dll` reports `1.26.0.6401`; the historical “1.28c” label does not establish its archives’ versions. | mixed-tree identity unresolved |

Archive opening and parser checks in §5.1 do not prove end-to-end game fidelity. PE VERSIONINFO identifies the executable files; it does not prove that every adjacent MPQ or movie belongs to the same release. See [the runtime evidence](docs/re-notes/glue-coordinate-runtime.md). Local sources:

| Source | Version | Location |
|---|---|---|
| Battle.net legacy | **1.29b** | `~/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III (Legacy)` |
| Extracted install | Game.dll `1.26.0.6401`; archive identity unresolved | `wc3/game/` |
| Inno installer | 1.27b | `wc3/warcraft-3-expansion/…/War3TFT_127b_English.exe` |
| ISO (ElAmigos installer repack, measured payload) | Game release unresolved; filename does not establish 1.27b or GOG provenance | `wc3/WarCraft III Complete Edition.iso` |
| Reforged | 2.0.1 | `~/Faugus/battle-net/drive_c/Program Files (x86)/Warcraft III` |

---

## 1. Current state (verified)

| Item | Status |
|---|---|
| Install source | `wc3/warcraft-3-expansion/Warcraft_III_-_Complete_Edition_Repack/` (Inno Setup, 3.7 GB) |
| Extracted tree | `wc3/game/` (1.4 GB) — `innoextract` used |
| Main binary | `wc3/game/app/game.dll` — PE32 x86, 12,042,240 B, sha256 `6d21bd9a…f4dec53c` |
| Asset library | `wc3/game/app/Storm.dll` — PE32 x86, 339,968 B, sha256 `eecc42b0…1bf97d0a` |
| Launcher | `wc3/game/app/war3.exe`, `Warcraft III.exe`, `Frozen Throne.exe` |
| Archives | `war3.mpq` (441 MB), `War3Patch.mpq` (28 MB), `War3x.mpq` (347 MB), `War3xlocal.mpq` (45 MB), 9 movie `.mpq`s |
| Runtime deps | `ijl15.dll` (Intel JPEG), `Mss32.dll` (Miles Sound System), `msvcr80/msvcp80/msvcm80` |
| Maps | `wc3/game/app/Maps/*.w3m` (~60), `Campaigns/DemoCampaign.w3n` |
| REA | `rea-agents@6.3.0` wired into opencode, provider **Ghidra 12.1.3** (flatpak, symlink-farm at `.tools/ghidra`) |
| Version detection | `pcx_mpq::Install` fingerprints archive content; the on-disk `game.dll` reports 1.26, not 1.28c — see `docs/re-notes/004-mdx.md` |
| REA verified | `rea inspect` on `game.dll` → **71,810 procedures / 17,207 strings**; on `Storm.dll` → **1,341 procedures / 736 strings** |
| Engine | **Bevy 0.20** (`pcx-gfx`, `peacecraft3`) — upgraded from 0.17; the four API migrations that forced are in the fidelity log |
| Toolchain | rustc 1.99.0, 32 cores, 125 GB RAM, ~125 GB free disk |

### RE environment note (already solved — do not redo)

The Flatpak Ghidra ships `support/launch.properties` as a symlink to the in-sandbox
`/var/config/ghidra.properties`, and `analyzeHeadless` resolves `readlink -f "$0"`, so
pointing `GHIDRA_INSTALL_DIR` at the Flatpak fails. `.tools/ghidra` is a symlink farm of
the Flatpak tree with `support/` scripts copied as **real files** and `support/launch.properties`
restored from `support/launch.properties.orig`. `tools/rea.sh` exports the right env.

---

## 2. Reverse engineering with REA

### 2.1 Driving REA

All REA invocation goes through `tools/rea.sh`, which pins `GHIDRA_INSTALL_DIR`, raises the
Ghidra startup deadline (this binary takes ~4 min per cold analysis) and keeps a
**persistent analysis snapshot** per target so repeat queries are seconds, not minutes.

```
tools/rea.sh inspect  <path>              # binary overview: segments, proc/str counts
tools/rea.sh search   <path> <pattern>    # string or procedure search
tools/rea.sh function <path> <symbol>     # full dossier: pseudocode, asm, xrefs, strings
tools/rea.sh trace    <path> <query>      # literal feature through strings + xrefs
tools/rea.sh xrefs    <path> <address>
tools/rea.sh snapshot <path>              # save analysis snapshot
tools/rea.sh evidence <op> <path> …       # arbitrary MCP op, JSON to rea/<name>.json
```

Evidence bundles land in `rea/` (one JSON per observation, digest-addressed). The tracker
references `ev_…` ids so every implementation decision is traceable to a code address.

### 2.2 Targets, in priority order

| Target | Why |
|---|---|
| `Storm.dll` | MPQ container: header layout, hash table algorithm, block table, file decryption, the five compressors. Everything asset-related depends on it. |
| `game.dll` | Game engine: object data parsing, sim tick, combat math, RNG, pathing, JASS VM, render state. 71 k procedures — the bulk of the work. |
| `war3.exe` | Bootstrap, Storm.dll loader, archive priority list. Small, quick win. |
| `Mss32.dll` | Audio; only if we go beyond re-synthesising sounds. |

### 2.3 Resolved RE questions (MPQ layer — closed)

Everything in this subsection is **solved and enforced by tests** in `crates/pcx-mpq`
(`cargo test -p pcx-mpq`, 18 passing). The earlier "this is not the published MPQ layout"
reading was wrong: the shipped headers are the classic layout, misread because the first
brute-force sweep used the wrong keystream table.

1. **Header layout.** Confirmed against all four archives and against Storm.dll
   `FUN_1501CFC0` (`ev_faddef74215f57387b18fbeb347d85a10681eddb1c5e7565647acd1dc08b0048`):

   | offset | size | field |
   |---|---|---|
   | `0x00` | 4 | magic `MPQ\x1a` |
   | `0x04` | 4 | header size (`0x20`) |
   | `0x08` | 4 | archive size |
   | `0x0C` | 2 | format version (`0`) |
   | `0x0E` | 1 | sector size shift; sector size = `0x200 << shift` (all four use `3` → 4096) |
   | `0x10` | 4 | hash table position |
   | `0x14` | 4 | block table position |
   | `0x18` | 4 | hash table entry count |
   | `0x1C` | 4 | block table entry count |
   | `0x20` / `0x24` | 4 | present, semantics still unattributed |

   The `0x00030000` word at `0x0C` is *two* fields: version `0` and shift `3`.
   Two invariants hold in all four archives and are asserted in tests:
   `block_pos == hash_pos + hash_entries*16` and `block_pos + block_entries*16 == archive_size`.

2. **Encryption table.** Fixed 1280 dwords (5 slices of 256), built from `seed = 0x00100001`
   with `seed = (seed*125 + 3) % 0x2AAAAB`. Measured caveat recorded in
   `docs/re-notes/000-mpq.md`: the table read out of a live Storm.dll process at
   `+0x4C2E4` differs from this fixed table in all 1280 words, and Storm's
   `FUN_15017650` builds it lazily (`if (DAT_1504c2e4 == 0)`). The **archives use the fixed
   table**; that is what the data is encrypted with.

3. **Filename hash** (`FUN_150175E0`, `ev_ddc2808ad6bdeac104d96f2c1c2520a975ac38f0d36e884cb777f7b1ffcc4324`).
   `seed1 = 0x7FED7FED`, `seed2 = 0xEEEEEEEE`, uppercase ASCII, slice selected by seed:
   `0` → hash-table start slot, `1` → `name1`, `2` → `name2`, `3` → table/file keys.
   Table keys: `hash("(hash table)", 0x300) = 0xC3AF3770`, `hash("(block table)", 0x300) = 0xEC83B3A3`.

4. **Table cipher** (`FUN_15017570`, `ev_703ee25fa60119017f501186a4e3477602692be9755a29492ee9964202b5d16e`).
   `seed2 += TABLE[0x400 + (seed1 & 0xFF)]`; `value = word ^ (seed1 + seed2)`;
   `seed1 = ((~seed1) << 21) + 0x11111111 | (seed1 >> 11)`;
   `seed2 = value + seed2 + (seed2 << 5) + 3`.
   **It is not self-inverse** — `seed2` folds in the *decrypted* word, so two passes do not
   cancel. There is a pinned test asserting that negative, against the tempting "simplify it
   into an involution" refactor.

5. **File keys.** Hash of the **basename only** (after the last `/` or `\`), slice `0x300`.
   With `FIX_KEY`: `(key + block.file_pos) ^ block.file_size`. Sector offset table decrypts
   with `file_key - 1`, sector `i` with `file_key + i`, single-unit files with `file_key`.
   Verified numerically on `war3.mpq`'s `(listfile)`: basename hash `0x2D2F0A94` →
   file key `0x47619FC7` → offset-table key `0x47619FC6`, independently solved from the
   on-disk table by a 256-candidate key search that landed on the same value.

6. **Compression.** PKWare implode (`0x08`) dominates — all 130 sectors of `(listfile)` are
   implode. zlib (`0x02`) is also present. `0x12` is an **exact** LZMA mask, not a bit set
   (testing it bitwise also swallows plain zlib). Sparse/ADPCM/Huffman are unimplemented and
   return an explicit `Unsupported`, never a wrong answer.

### 2.5 Symbol sources — what debug information exists

Researched 2026-10-11. Short answer: **classic 1.28c never shipped a PDB, but it leaks a
near-complete symbol map anyway; Reforged *did* ship real PDBs, by accident, in 2.0.1.**

#### Classic 1.28c — no PDB, but the build tree is in the binary

No debug build of classic WC3 has ever been found. What *is* recoverable is that `game.dll`
carries its own compile-time breadcrumbs: assertion macros and logging calls embed absolute
source paths from Blizzard's build machine,

```
e:\drive1\temp\buildwar3x\war3\source\unit\CAbilityCriticalStrike.cpp
e:\drive1\temp\buildwar3x\war3\source\unit\CUnitDatabase.cpp
e:\drive1\temp\buildwar3x\engine\source\jass2\Compile.h
e:\drive1\temp\buildwar3x\engine\source\agile\CAgent.h
e:\drive1\temp\buildwar3x\engine\source\base\CDataStore.h
```

and with them the **entire original source tree**, real **class names** (`CUnit`,
`CCustomData`, `CMissile`, `CFogMaskTable`, `CQuestDialog`, `CScriptDialog`, …) and the
**engine layering**: `engine\source\agile` (the component/agent system), `engine\source\jass2`
(the JASS VM), `engine\source\base` (`CDataStore`), `engine\source\anim`, versus
`war3\source\{unit,ui,glue,world,game}`. Profiling artefacts leak too (`calldump.log`,
`WEUTiming.txt`, `TerrainMemory.txt`).

**Action:** *(done — `docs/re-notes/001-classic-symbols.md`)* one string scan recovered **254
source paths across 25 subsystems** plus 36 type names, artifacts in `rea/game-source-paths.txt`
and `rea/game-class-names.txt`. The layering is `engine/{base,agile,jass2,frame,anim}` →
`war3/{unit,game,world}` → `war3/{UI,Glue,WorldEdit}`, which maps cleanly onto §3.1. Use them
to name procedures and types in REA and to know which subsystem a function belongs to before
decompiling it.

#### Reforged 2.0.1 — real PDBs, leaked

Blizzard shipped **Warcraft III: Reforged 2.0.1 (build 22474, 2024-12-17)** with debug
artefacts attached, pulled within a day. Contents included:

- `Warcraft III.pdb`, `Preview.pdb`, `Aegis_Gen.pdb` — full debug symbols for the game, the
  model previewer, and the obfuscator;
- a **debug game client with an interactive command console** for driving the game world;
- a **Debug World Editor** with extra tooling;
- `Preview.exe` (+ `Previewac/ae/d.exe`) — the Reforged build-pipeline model/PKB previewer,
  with an archive viewer and model validation;
- Maya export plugins for Reforged models;
- `Aegis_Gen`, their obfuscator (anti-attach, mathfuscation, import spoofing, …), leaving
  magic `0x1560E0A0` at PE offset `0x1C`;
- `DevOnly` maps containing original SD+ Footman/Grunt models.

Blizzard removed it from distribution; the files remain widely mirrored. **We do not
redistribute them, and nothing from this leak enters the repo** — it is recorded here as a
research finding and as a pointer for the user to obtain legitimately if they choose.

**Two consequences for this project:**
0. The user's own 3.0.0.24268 install shows `war3_loader.dll` and `worldedit_loader.dll` are
   **Aegis-obfuscated in every retail build** — their CodeView records are replaced with the
   `BN01` marker, so the loader has no usable PDB reference even in the 2.0.1 leak. Any Reforged
   code RE therefore starts with deobfuscation (RE-Q16). `Warcraft III.exe`, `World Editor.exe`,
   `ClientSdk.dll` and `BlizzardError.exe` are *not* obfuscated and are analysable as-is.
1. The Reforged PDB turns the modern build from guesswork into a symbol lookup, and the
   leaked debug client's command console is a candidate injection surface for the Tier A/B
   scripted-playthrough harness (RE-Q11) if the classic build cannot be driven directly.
2. `Preview.exe` validates Reforged models/PKB and can report errors, which makes it an
   *oracle* for our MDL/PKB decoders — feed it a file, compare its complaint to ours.

### 2.6 Open RE questions (next)

1. **Sim tick.** Fixed step and its relationship to render frames; where the real-time
   budget lives.
2. **RNG.** The `FRand`/`GetRandom` generator and every call site that consumes it — this
   is the single hardest thing to match exactly and gates "100% faithful".
3. **Header words `0x20`/`0x24`.** Unattributed; no reader currently depends on them.
4. **Archive priority.** `DEFAULT_PRIORITY` is currently assumed, not derived from
   `game.dll`'s archive-open order.

### 2.7 Multi-version support

One target, but the code must not hard-code build numbers, because the archive set,
spelling and even which binaries exist all differ between the classic builds. The rules:

- **No version-specific constants in a decoder.** A magic number, a record stride, a
  palette layout — anything that could differ between 1.28c and 1.29b is either proved
  to be identical in both or marked unverified against 1.29b.
- **Version is detected, not passed in.** `pcx_mpq::Install::detect` reads the version
  from the shipped binaries (`game.dll`'s `VS_FIXEDFILEINFO`, and the archive set)
  rather than trusting a caller or a directory name.
- **Every sweep is parameterised by install.** Tests discover every install present
  and run the full assertion set against each. A decoder that only passes on one
  build has not passed.
- **Differences are evidence, not bugs.** If 1.29b disagrees with 1.28c on some count,
  that is a finding to be recorded and explained — the two builds are not expected to
  be byte-identical. They are expected to be *format*-identical.

What this means in practice: the existing sweeps already cover 6,273 BLPs, 3,234 MDXs
and 383 TGAs on 1.28c alone, and the newer decoders sweep the same way across both
installs — 133 map containers (132 of them with terrain, doodads, shadows, minimap
image and markers), 92 object-edit files, 1,326 real JASS scripts, 397 trigger-container
files and 913 `wts` tables. Those same sweeps, pointed at a 1.29b install, are the 1.29b
gate. That is why the install abstraction matters more than any single decoder.

Sweeps discover the installs rather than being told one: `PEACECRAFT3_INSTALL` (default
`wc3/game/app`, the 1.28c tree) and `PEACECRAFT3_LEGACY_DIR` (default the Battle.net
Legacy install, 1.29b). When both are present one run covers the matrix; setting only
`PEACECRAFT3_LEGACY_DIR` sweeps 1.29b alone. Run any of them with
`cargo test -p <crate> -- --ignored --nocapture`. The 1.28c cross-check still runs
whenever that tree is present, because it is free and it caught a real bug — but a
1.28c failure is a bug to fix, not a gate to block on.

---

## 3. Architecture

### 3.1 Crate layout

```
crates/
  pcx-mpq        MPQ reader: header, hash/block tables, crypt, decompress, archive-priority lookup
  pcx-casc       Reforged CASC reader: .idx/.data parts, BLDR encoding, content lookup
  pcx-assets     Asset-source layering: vanilla MPQ | Reforged CASC | user override, with provenance
  pcx-blp        BLP0/BLP1/BLP2 → RGBA8 (+ mip chain), palette handling
  pcx-tga        TGA reader (terrain tilesets, world editor output)
  pcx-mdl        WC3 MDL/MDX model parser → scene graph, skin, animations, sequences
  pcx-slq        WC3 `.slk` tables → sound lists (SYLK, not Miles; see §3.1a)
  pcx-objects    `war3map.w3u`/`.w3t` object-edit container — framing on 92/92, the other five
                 extensions refused as unmeasured
  pcx-script     Raw JASS front end: lexer + parser + exact tree with spans. **No VM** — stops
                 at syntax on purpose
  pcx-trig       `war3map.wts`/`.wtg`/`.wct` — table strings, the editor's trigger graph, custom text
  pcx-map        W3M/W3N: tileset, terrain, minimap, doodads, regions, units, triggers
  pcx-tera       Terrain mesh generation, tileset blending, cliff/water logic
  pcx-net        Transport traits, wire codec, authoritative lockstep server, client prediction
  pcx-sim        Deterministic fixed-tick simulation (headless, no bevy dep)
  pcx-play       Play-test harness: `.rpl` scenarios, deterministic runner, canonical trace, `pcx-diff`
  pcx-gfx        Bevy **0.20** asset plugins: BLP→Texture, MDL→Mesh+AnimationClip, .slk→Audio
  peacecraft3    The app: window, camera, UI, wiring
```

`pcx-sim` has **zero** Bevy dependency and is driven by an integer tick counter. Everything
in it must be `no_std`-friendly in spirit: no floats where the original used fixed point,
no `HashMap` iteration where order is observable, no wall-clock reads. `pcx-play` and
`pcx-script` sit on the same side of that line: the harness is Bevy-free so its traces are
reproducible in CI, and the JASS front end is Bevy-free because it has to run inside the
same deterministic process as the sim.

### 3.1a `pcx-slq` — the premise was wrong, and the crate name is now a misnomer

The crate is called `pcx-slq` because that is what the format was *believed* to be. It is
not. There is no binary Miles "sound list" container, and no file in either archive has the
extension `.slq`:

- **The sound tables are `UI\SoundInfo\*.slk`, plain ASCII SYLK** (Spreadsheet Link Key),
  the same dialect `Units\UnitData.slk` and `Units\ItemData.slk` use.
- **59 `.slk` files ship on 1.28c and 65 on 1.29b**, of which 12 are sound tables (13 on
  1.29b, which adds `AmbientMusic.slk`). Sweep: **59/59** and **65/65** decoded, 0 failed,
  768,067 / 983,763 cells, 10,373,503 / 12,971,315 bytes.

Corroborated independently of the bytes by vanilla 1.28c `game.dll`: the NUL-terminated
literal `ID;PWXL;N;E` sits at `.rdata+0x95494e` (VA `0x6f95494e`, image base
`0x6f000000`), immediately followed by `\Database.cpp`, `DATABASE`, `DBFIELD` and
`DBENTRY` — the engine compares the first eleven bytes and dispatches on single-letter
records. The twelve `UI\SoundInfo\` paths are listed verbatim at `.rdata+0x937064`.

**The non-obvious part is the cursor rule, not the format.** `X` and `Y` are *sticky
state shared by `C` and `F` records*: a record that omits a tag reuses the last value
seen on **either** letter. `UI\SoundInfo\DialogSounds.slk` carries no `Y` on any of its
14,634 `C` records — its rows are advanced entirely by interleaved `F` formatting records.
Reading `X`/`Y` off the `C` record alone collapses that file to 2,231 of its 2,439 rows,
double-writes 1,249 cells and invents 202 rows that exist only as gaps. Reading the cursor
off both letters leaves zero duplicates and zero gaps in all 124 shipped files.

So: if you are looking for the sound lists, look for `.slk` under `UI\SoundInfo\`. Do not
go looking for Miles `.slq` containers again — there are none, and the crate name is a
historical artefact rather than a description.

### 3.2 Asset strategy — *mount, don't convert*

The four archives stay on disk and are opened read-only. Assets are decoded at load time
into Bevy resources and cached. Consequences we accept deliberately:

- No 3.7 GB of converted output to manage, and the toolchain has only ~125 GB free.
- Adding a new MPQ (user mods) works with zero conversion.
- Decoders must be fast and streaming; we budget a background-decode thread pool and a
  per-archive LRU on decoded textures/meshes.

Priority (Blizzard) when the same name exists in more than one archive is
`War3Patch.mpq` > `War3x.mpq` > `War3xlocal.mpq` > `war3.mpq` — **verify from `game.dll`'s
archive-open sequence, do not assume**.

### 3.3 Fidelity mechanisms

1. **Format decoders are tested against the original bytes.** Every decoder has a golden test
   built from a real file pulled out of a real MPQ, plus round-trip invariants.
2. **Differential process capture.** The original runs under Wine; REA `capture-process`
   records its terminal/interaction/filesystem behaviour; REA `compare-process-captures`
   diffs it against a capture of our binary driven by the same script. This is the loop that
   turns "looks right" into "is right".
3. **Visual diffing.** Headless capture of both renderers, `compare-web-screenshots`-style
   pixel metrics, to catch camera/lighting/terrain drift.
4. **Residual unknowns are tracked, not hidden.** Anything REA cannot settle goes into
   `TRACKER.md` under *Open unknowns* with the probe that would settle it.

### 3.4 Bevy mapping

| WC3 concept | Bevy |
|---|---|
| terrain | chunked meshes, per-16×16 tileset tiles, splat-blended |
| unit/building | hierarchical `Scene` per model, animation driven by `AnimationGraph` |
| projectiles / effects | pooled entities, `MaterialMeshBundle` + particle shader |
| UI (glue, HUD, minimap) | 2D scene in its own camera + a viewport for the minimap |
| sound | `AudioSink`, decoded from `.slk`/WAV via `rodio` |
| selection / drag box | a 2D overlay pass |

Bevy is used as an *engine*, not as a gameplay framework: the sim never reads Bevy state.

---

### 3.5 Programmatic play-testing (the real acceptance test)

Unit tests prove a decoder reads a byte correctly. They do **not** prove the game is
faithful. The acceptance test for this project is: **the original campaigns, played
start-to-finish by a script, with no human input, producing a recorded trace that our build
reproduces exactly.**

Three tiers, each a superset of the last:

#### Tier A — Scripted map playthrough (headless, from M3 onward)

A `.w3m`/`.w3n` map is loaded into `pcx-sim` and driven by a deterministic script of player
actions: `select`, `move`, `attack`, `build`, `cast`, `train`, `research`, `wait`. The sim
records a **trace** per run:

```
tick, unit_id, event, (x,y,target,ability), hp, mana, xp, cooldown, rng_draw, state_hash
```

The trace is the comparison artifact. Same script against the original (under Wine, via
REA `capture-process` + REA `observe-native-calls` on the decisive routines) and against
`peacecraft3` must produce identical traces. REA `compare-process-captures` and
`rea_verify_reconstruction` turn that into a pass/fail with cited evidence, so a divergence
names the routine that diverged instead of producing a diff nobody reads.

Rules:
- **Fixed tick, no wall clock, no float accumulation** — a trace must be byte-identical
  across 100 consecutive runs before it is admitted as a baseline.
- The schema above is the *target*. What `pcx-play` carries today (§3.10) is the
  header identity fields plus per-tick unit state, RNG draws and timer events: the
  `hp`/`mana`/`xp`/`cooldown` columns and the RNG draws taken *inside*
  `pcx_sim::step` are M5/M6 work, so a current trace is a proof of the harness and
  not yet a proof of combat parity.
- Scripts live in `tests/play/<campaign>/<chapter>/<scenario>.rpl` as a small declarative
  format, versioned, so a failing scenario is reproducible from the repo alone.
- Every scenario asserts a *specific* expectation (win state, surviving units, mission
  score, quest completion), not merely "did not crash".

#### Tier B — Campaign completion (from M7 onward)

The scripted harness is pointed at the stock campaigns and run to completion:

| Source | Selection | Gate |
|---|---|---|
| Stock RoC and TFT campaigns, all races | Authored campaign and mission entries from the selected installation | Every chapter's win condition asserted; each final chapter completes |
| Demo campaign (`DemoCampaign.w3n`) | Chapters read from the supplied container | Loads, all triggers fire, completes |
| Custom-map smoke set | Enumerated supplied map members | Loads and runs each init script without error |

Campaign counts, chapter map paths and ordering come from supplied source data,
not a fixed table in the engine or this acceptance plan. The existing
`pcx-objects::editor::campaign_index` reader retains authored index slots and
mission rows. It does not establish unlocks, completion or a next-chapter rule.

A chapter passing means: the intro dialogue gates fire in order, all quest objectives are
satisfied, the win condition triggers, and the end-of-movie/`endGame` sequence is entered.
This is the first milestone where AI behaviour is in scope, because a campaign cannot be
beaten without the opponent acting.

#### Tier C — Differential fuzzing (from M6 onward)

Generated scenarios (random unit comps, random terrain, seeded RNG) run against both
implementations, and the traces are compared. Divergences are auto-filed into
`TRACKER.md` with the minimal reproducing seed. This is how RNG parity and combat-ordering
parity actually get closed out, rather than argued.

**Definition of done for the project as a whole:** `cargo test --workspace --features play`
runs Tier A green against every shipped campaign, and Tier C reports zero unexplained
divergences over a fixed seed corpus.

### 3.6 Launcher and game modes

A GUI launcher (`peacecraft3`; `peacecraft3 launch <cmd>` keeps a CLI for CI) stages
assets from a source and picks a game mode. Sources are auto-detected:

| Source | Recognised by | Status |
|---|---|---|
| A directory | holds WC3 MPQs directly | works |
| A Battle.net folder | holds `Warcraft III*` product directories | works |
| A retail ISO / installer | by extension | not working — see below |

A **game mode** is four independent axes, because they are genuinely orthogonal:

```
mode  ::= <build>-<addon>-<assets>-<behaviour>
build     1.29b | 1.28c
addon     tft (The Frozen Throne) | noaddon (Reign of Chaos only)
assets    vanilla | hd (classic art + Reforged HD textures) | reforged
behaviour vanilla | reforged | fixes
```

`assets` and `behaviour` are separate on purpose: Reforged's *art* is a presentation
choice and its *gameplay* changes are not. `1.29b-tft-hd-vanilla` and
`1.29b-tft-reforged-reforged` must both be expressible and never confused — `hd`
never implies Reforged behaviour, and a test pins that.

Staging **copies** the archives and then **verifies** by opening the staged tree and
counting what it exposes, so a mode that silently landed the wrong build fails loudly.
`1.28c-noaddon` stages three archives and skips `War3xlocal.mpq`, because Reign of
Chaos does not ship it.

Two honest limits, reported rather than papered over:

- **Reforged is CASC, not MPQs**, so `assets: reforged` cannot stage by copying. The
  launcher says so and names RE-Q17 instead of failing with "no archives found".
- **Installer ISO extraction remains incomplete.** The measured
  `Complete Edition.iso` has valid ISO9660/Joliet directories and six FreeArc
  payloads with encrypted file lists. Its embedded Inno Setup executable can be
  listed with `innoextract`, but contains no MPQ. Archive password arguments
  now derive from the compiled script, and control directories validate
  offline. Browser REP/Tornado decoding and the language-patch format remain
  unresolved; directory discovery is not installed-game extraction.
  See [the original ISO evidence](docs/re-notes/original-iso-installers.md).

The launcher also carries a greyed-out **Check for updates** control. It is present
and inert on purpose, so the shape of the UI is settled before the downloader exists
rather than being redesigned around it afterwards.

### 3.7 Multiplayer

Support three distinct network paths:

| Path | Compatibility target | State and transport |
|---|---|---|
| Classic LAN / TCP/IP | Original Legacy 1.29b client, in both host directions | W3GS discovery, lobby and ordered game actions; clients simulate the same game |
| Classic Battle.net | Classic BNCS community realm plus its W3GS game hosts | Realm authentication, channels and game advertisements, then the game connection |
| Project multiplayer | Native/browser PeaceCraft peers | Existing authoritative movement world, binary commands and prediction/rollback |

Original-client interoperability is a product requirement. The project UDP
lobby and browser WebSocket session do not implement BNCS or W3GS. A classic
realm does not replace the simulation running on each game client with our
movement snapshots. Hosts must validate admission and inputs, while classic
turn ordering, checksums, random state and game data must agree between clients.
Our authoritative snapshot correction remains the project protocol's design.

Use public implementations as protocol references: PvPGN-PRO for BNCS,
BNCSUtil for authentication/version checking, and GHost++ for W3GS hosting.
Eurobattle.net also publishes its realm fork. Their existence supersedes the
previous claim that the classic protocol cannot be investigated locally.
Version support is explicit: upstream PvPGN lists through 1.28.5; its open
1.29.2 proposal requires client changes. Neither establishes compatibility
with the unmodified supplied 1.29.2.9231 executable.
See [the reference matrix and staged gates](docs/re-notes/classic-network-interoperability.md)
and the [asset-derived gateway list](docs/re-notes/005-battle-net-gateways.md).

#### Acceptance order

1. Pin source/build identities and run an isolated local PvPGN Docker realm.
   Check real bounded protocol responses separately from container readiness.
2. Implement bounded BNCS/W3GS framing and malformed/fragmented packet tests.
   Keep product/version, endpoint and local asset identities explicit.
3. Discover and join an original LAN host; allow an original client to discover
   and join a PeaceCraft host. Prove both directions on the exact client build.
4. Complete slot, map identity, readiness and game-start negotiation with maps
   already installed locally. Refuse mismatched edition/build/map before start;
   proprietary asset transfer is not part of the test harness.
5. Execute ordered actions with matching seed and simulation/checksum results,
   then a ten-minute mixed-client match without divergence. A shared requested
   seed alone does not prove equal random streams or game behavior.
6. Connect PeaceCraft to the local classic realm: version/authentication failures,
   successful login, channels/chat, game advertisement, listing and joining.
   Distinguish native authentication from any separately tested community-loader
   profile. Local realm success precedes public community-realm validation.
7. Exercise disconnect, timeout, lag and cancellation with bounded work. Rejoin
   extensions such as GProxy require their own negotiated profile and tests.

Gameplay, script/timer execution, object rules and native RNG fidelity remain
prerequisites for full crossplay even after a successful lobby join. Preserve
those open gates in TRACKER.md; network framing alone cannot close them.

---

### 3.8 WASM build

The game must run in a browser. This is a real target with real consequences for how
the code is written *now*, not a later port:

- **No synchronous file I/O anywhere in a shared path.** Reading an MPQ through
  `std::fs` works on desktop and cannot work on WASM. Asset loading already goes
  through [`crate::pcx-gfx`]'s cache, so making that cache async is the fix — but it
  means the cache cannot grow a synchronous `extract` call.
- **No thread-local or OS-dependent state in the simulation.** `pcx-sim` is already
  Bevy-free and deterministic; keeping it that way is what makes it WASM-able.
- **Assets are fetched, not mounted.** In a browser the MPQs arrive as bytes over
  HTTP, so `pcx-mpq` needs a reader that works from an in-memory buffer rather than
  from `BufReader<File>`. That is already how `pe_file_version` reads a file, so the
  shape exists.

The launcher is the prototype for this: it already renders through Bevy, which
targets WASM, and it stages assets without touching the game's own asset path.

**Measured, not assumed.** `cargo build --target wasm32-unknown-unknown --workspace`
links the whole workspace — `pcx-sim`, `pcx-net`, both decoders, every binary and
the launcher. The transport is `cfg`'d out because a browser has no UDP, and the
sim, codec and lockstep logic stay pure Rust.

The artifact is large, and the size is worth recording because it is not what it
looks like. Two builds of the same program, measured by section:

| section | dev build | debuginfo-stripped |
|---|---|---|
| `.debug_str` | 325,977,178 | — |
| `.debug_info` | 306,399,018 | — |
| `.debug_loc` | 131,152,931 | — |
| `.debug_line` | 57,914,095 | — |
| `.debug_ranges` | 34,985,238 | — |
| `.debug_abbrev` | 2,422,704 | — |
| `name` | 53,264,444 | **62,950,809** |
| `code` | 51,609,943 | 47,289,068 |
| `data` | 8,629,367 | 8,629,367 |
| **total** | **975,009,181** | **119,794,662** |

DWARF plus the `name` section is **913 MB of 975 MB — 93.6%** of the dev build.
The actual WebAssembly bytecode is ~48–52 MB and the data segment 8.6 MB, in a
`dev` profile that is `opt-level = 1` plus debuginfo.

So the number is a build-configuration fact, not a signal that the engine is too
heavy for the browser. The second column is the artifact `wasm-bindgen` was
actually run on — it lives at `web/pkg/peacecraft3_bg.wasm` and carries no DWARF
at all. What it still carries is a 63 MB `name` custom section, which is symbol
names rather than code and which `wasm-strip` / `-C strip=symbols` would remove.
The dev artifact is the one to distrust, and it is the one a fresh
`cargo build` produces.

**The gate below cannot be signed off from a dev artifact.** Measure the release
path — it is a different number by a factor of eight.

### The browser entry point, and what it cannot do

A browser has no `std::env::args` and no process to `exit`, so wasm does not boot
through `main`. `crates/peacecraft3/src/browser.rs` supplies the entry point — a
`#[wasm_bindgen(start)] fn start()` that runs the Bevy app and *returns*, leaving
the tab and its event loop alive. It also has to fix the asset path: Bevy's
platform default on wasm is an HTTP reader rooted at `assets/`, which cannot see
the bytes the launcher is handed, so the browser build replaces the **default**
source with Bevy's own `MemoryAssetReader` holding exactly what JavaScript handed
over, and the archives are read by `MpqArchive::from_bytes` — the same parser the
desktop uses, from a buffer instead of a `BufReader<File>`.

The JS side is three plain static files — `web/index.html`, `web/loader.js` and
`web/pkg/` — with no trunk, no wasm-pack and no bundler step. The only build is

```
cargo build --target wasm32-unknown-unknown --release -p peacecraft3
wasm-bindgen --target web --out-dir web/pkg \
    target/wasm32-unknown-unknown/release/peacecraft3.wasm
```

and `loader.js` offers the page `add_archive`, `add_asset`, `drop_assets`,
`stored_assets`, `selftest` and `start`.

Three things do **not** work in a browser, and they are structural rather than
unimplemented. They are recorded here rather than left to be discovered:

- **Staging does not.** `launch::stage` copies files into a destination and there
  is no destination, so every source is reported as rejected. The launcher says
  so rather than pretending to copy.
- **Install-level version detection does not.** `Install::open` fingerprints a
  *set* of archives in a directory. A single archive can be opened and counted
  here, but not identified as 1.28c or 1.29b.
- **Rendering needs a canvas.** With no DOM there is nothing for winit to attach
  a window to, so the GUI only starts when a canvas element is present.

`pcx-gfx`'s asset cache is also still synchronous, which is the one genuinely
fixable item left in §3.8's list.

**Gate:** `cargo build --target wasm32-unknown-unknown` produces a binary, and the
launcher runs in a browser. Both halves are confirmed now, and the second against
the release path rather than a dev artifact:

| artifact | size |
|---|---|
| `target/wasm32-unknown-unknown/release/peacecraft3.wasm` | 74,235,106 |
| `web/pkg/peacecraft3_bg.wasm` | 75,887,489 |
| `web/pkg/peacecraft3.js` | 114,018 |

In node the package instantiates and its API answers: `add_archive` opened the
440 MB `wc3/game/app/war3.mpq` in ~90 ms — 10,681 live block entries of 10,760,
`(listfile)` present — and the 27 MB `War3Patch.mpq` reports 576 of 576 and *no*
`(listfile)`, which is what its tables say. In headless Chromium (SwiftShader,
WebGL2) the launcher window renders off `#launcher-canvas`, the mode axes respond,
and clicking *Stage assets* writes `✕ source rejected: wc3/game/app does not
exist` into the log. The archive itself comes from the page, not from a served
directory: the user picks the file, the page reads it into memory and hands the
bytes over, which is the only shape a browser can offer.

### 3.9 Reforged assets and Reforged fixes

We want two separate things from Reforged, and conflating them would break §6.

**(a) Reforged *presentation* assets** — HD textures, remastered models, new sounds, icons.
**(b) Reforged *fixes*** — the bug fixes and data corrections made between 1.28c and 3.x.

#### Local source

A full Reforged install exists at `/home/maci/Faugus/battle-net/` (a Wine prefix, 35 GB):

```
drive_c/Program Files (x86)/Warcraft III/
  .build.info          product w3, version 3.0.0.24268, build key 3a9d8f26…, CDN tpr/war3, region KR
  Data/data/           CASC storage — 16 × .idx + 34 × .0NN data parts + shmem
  Data/w3/             local (loose) content overrides, CASC-indexed
  _retail_/x86_64/     war3_loader.dll (39.3 MB) + other x64 binaries (largest 52.9 MB)
```

**This is not MPQ.** Reforged ships assets in Blizzard's CASC container, so supporting
Reforged presentation assets means a CASC reader, not an extension of `pcx-mpq`.

*Measured 2026-10-11:* the store opens cleanly — 195 configured archives, 33 data parts, 16
indices, **74,500 entries, 34.96 GB** — and the build config recovers
`build-name 3.0.0.24268-retail`, `build-uid w3`, comment `RETAIL - RC 3.5 LAUNCH BUILD`, plus
the `root` / `encoding` / `vfs-root` EKeys and 110 VFS entries. What is **not** working yet is
the step that matters: reading an EKey returns zero bytes because the BLDR/encoding chain is
unwired, and path→EKey resolution is FDID-oriented in the available implementations while WC3
uses its own VFS layout. Those are RE-Q17 and the gate on this milestone.

#### Asset-mode policy

A first-class runtime setting, defaulting to `vanilla`:

| mode | textures / models / sounds | gameplay data |
|---|---|---|
| `vanilla` (default) | 1.28c MPQ | 1.28c — the fidelity baseline |
| `reforged` | Reforged CASC | 1.28c — *look* changes, *behaviour* does not |
| `reforged-hd` | Reforged HD where available, classic SD elsewhere | 1.28c |
| `reforged-fixes` | Reforged | Reforged — opt-in deviation |

The rule: **`vanilla` is the only mode the §6 fidelity gate is measured against.** Every
other mode is a documented deviation, and `pcx-diff` must report *which* mode produced a
trace so a mismatch is never silently attributed to the sim.

#### Overriding assets without touching either install

Both MPQ and CASC mounts stay read-only. Overrides are a layer above them:

```
assets/
  vanilla/      -> symlink or path to the 1.28c wc3/game/app
  reforged/     -> path to the Reforged install
  override/     -> user drop-in files, our own format
```

Resolution order is `override/` → highest-priority archive → … → lowest. `override/` is a
plain directory tree using our normalized paths, so a user can replace one `.blp` without
touching a 33 GB install. A generated `override/index.json` records which layer won each
path, so a trace can name the asset source.

#### Incorporating the fixes, honestly

Reforged fixed real bugs (stuck units, desyncs, pathing, UI, Classic-HD fallback). We do
**not** silently inherit them, because then "faithful to 1.28c" becomes unmeasurable. Instead:

1. A **fix ledger** `docs/reforged-fixes.md`: one row per fix — subsystem, symptom, the
   1.28c behaviour, the Reforged behaviour, evidence (`ev_…` id, patch note), and a
   disposition: `adopt` / `reject` / `defer`.
2. `reforged-fixes` mode applies only ledger rows marked `adopt`, and the game prints or
   logs which rows were active for a run. A run's trace header carries that list.
3. Rows default to `defer`. Adoption requires the fix to be (i) observable in a scripted
   scenario, (ii) not break any 1.28c golden trace in `vanilla` mode, and (iii) justified in
   writing — so "we took the fix because it was better" never becomes "we don't know why the
   sim differs".

This is the same discipline as the residual-unknown tracker, applied to upstream behaviour
rather than to our own gaps.

### 3.10 Play-test harness layout

```
crates/pcx-sim        deterministic headless sim (already Bevy-free by construction)
crates/pcx-play       scenario script parser, action scheduler, trace recorder, canonical codec
crates/pcx-play/bin   `pcx-diff`: first-divergence report (exit 0 unchanged / exit 1 diverge)
tests/play/           .rpl scenarios per campaign/chapter, with expected outcomes
tests/baselines/      golden traces from the original, each pinned to an ev_… id
docs/reforged-fixes.md  fix ledger: adopt / reject / defer per upstream change
tools/playrun.rs      CLI: run a scenario, emit trace, diff against baseline
```

The trace header records the **asset mode** and the **active fix-ledger rows**, so every
trace is self-describing and a `vanilla` run can never be confused with a `reforged` one.

`pcx-play` and `pcx-diff` stay Bevy-free and run in CI. Only the render-parity tier
screenshots the window, and that stays a separate manual/CI job.

Each milestone ends with a runnable artifact and a fidelity check.

### 3.11 Map compatibility — the real acceptance bar

Required by the [non-negotiable requirement](#non-negotiable-requirement-original-maps-must-work-unmodified)
at the top of this document. This section states the **required surface** so it cannot be
quietly narrowed. It is a requirement list, not a claim of what has been measured — every
line below is *unverified* until a decode lands with its framing invariant asserted, and
`TRACKER.md` is where that status is tracked.

**Measured so far:** the `.w3m`/`.w3n` container is `HM3W` + `0x200` header + embedded
MPQ (+ optional 260-byte `NGIS` trailer); terrain is `war3map.w3e` inside it with
`corner_size == 1` and raw 14-bit corner indices; `war3map.doo` / `.shd` / `.mmp` /
`war3mapMap.blp` are decoded with their own framing identities; the object-edit
containers `war3map.w3u` / `.w3t` are decoded (the other five extensions are refused as
unmeasured); and `war3map.wts` / `.wtg` / `.wct` are decoded as containers with
`pcx-trig`, while `war3map.j` parses under `pcx-script`. The **scripting** half —
trigger desugaring, the VM, native dispatch — is not measured and is M7. See M3, M4, M7.

#### Container and data files a shipped custom map contains

| File | Carries | Needed by |
|---|---|---|
| `.w3m` / `.w3n` | container: header, name, author, player count, embedded MPQ | M3 ✓ measured |
| `war3map.w3e` | terrain: per-corner height indices, ground/slot texture assignment | M3 ✓ measured |
| `war3map.w3i` | map info: forced/suggested player slots, map flags, **declared object tables** | M4/M7 |
| `war3map.wpmp` | allied shared data (campaign-wide table overrides) | M4 |
| `war3map.doo` | doodad instances: type, position, rotation, scale, variation, life | M3 ✓ measured — `W3do` v7, `16 + 42n + 8 + 16m` |
| `war3map.shd` | shadow map (height-derived occlusion) | M3 ✓ measured — `16·w·h`, carries its own grid |
| `war3map.mmp` + `war3mapMap.blp` | minimap markers + minimap image | M3 ✓ measured — the names `war3map.map`/`.ter` do not exist and are refused |
| `war3map.wts` | table strings: all player-visible text, as string handles | M7 ✓ container measured (`pcx-trig`); VM pending |
| `war3map.wtg` | **trigger graph** — the editor-authored program | M7 ✓ container measured (`pcx-trig`); desugaring pending |
| `war3map.wct` | trigger's custom text script | M7 ✓ container measured (`pcx-trig`) |
| `war3map.j` / `.txt` | raw JASS shipped by the map | M7 ✓ parsed (`pcx-script`); VM pending |
| `war3map.ini` | map init file (script source selection, safe mode, forced flags) | M7 |
| `war3map.w3u` | custom **units** | M4 ✓ measured — 91 files |
| `war3map.w3t` | custom **items** | M4 ✓ measured — 1 file |
| `war3map.w3a` | custom **abilities** | M4 — **ships in no map**; refused, layout unmeasured |
| `war3map.w3b` | custom **buffs** | M4 — **ships in no map**; refused, layout unmeasured |
| `war3map.w3d` | custom **doodads** | M4 — **ships in no map**; refused, layout unmeasured |
| `war3map.w3h` | custom **destructables** | M4 — **ships in no map**; refused, layout unmeasured |
| `war3map.w3q` | custom **upgrades** | M4 — **ships in no map**; refused, layout unmeasured |
| `war3map.blj` | build log (not loaded, but ships) | — |

The `W3?` object files share one container family and are the highest-value next decode
after terrain: they unlock any map with custom content, and their rawcode tables are the
join key into the object editor data. **Measured, and only two of the seven extensions
ever ship** — see M4. The five that do not are refused rather than assumed absent: the
absence is a census over every map's embedded archive on both builds, not a guess from
the extension list.

#### Scripting surface required

- Raw JASS execution: lexer, parser, and the VM's value/handle model.
- **Trigger desugaring:** `WTG` → JASS with the original's exact semantics, not an
  approximate transpile.
- Natives with the original's signatures, return conventions and **side-effect ordering**.
- `BJ` (Blizzard.j) and `Blz` library surface, including the `Blz` extensions.
- `WTS` string handles and localization lookups.
- Timers, and the original's timer/event ordering (this is what DotA depends on most).
- Unit/ability/item/upgrade/buff/stage/destructable handles; handle casting and wrapping.
- Pre-1.20 and post-1.24 API shapes coexisting.
- A missing native or unresolved rawcode must fail loudly with the map's own identifier —
  silent zero-returns are how a map "runs" and plays wrong.

#### Gates

- **M3** gates on: container + `w3e` + `doo` + `shd` + minimap. *(All five measured — see
  M3 for the framing identities.)*
- **M4** gates on: stock object data + the `W3?` override families that actually ship
  (`w3u`/`w3t`, measured on 92/92) + `wpmp`. The other five extensions are refused as
  unmeasured and cannot gate anything until a shipped file proves their layout — a gate
  written over data that does not exist is not a gate.
- **M7** gates on: `wtg`/`wct`/`j`/`wts` + the VM, evidenced by **DotA, Athena and
  Footman Frenzy each running unmodified** (see M7 for the exact gate).

Maps used for verification must be obtained separately and are **not committed** — same
asset rule as the game data.

### M0 — RE harness and evidence base · *in progress*
REA wired and verified against both binaries; `tools/rea.sh` + snapshot discipline;
evidence directory laid out; answers to RE questions 1–4 sketched in `docs/re-notes/`.

**Done when:** `tools/rea.sh function Storm.dll <hash fn>` returns a dossier with
`ev_…` id in seconds from cache.

### M1 — MPQ layer (**first shippable milestone**) · *done 2026-10-11*
`pcx-mpq`: header, both tables, name lookup, sector layout, file keys, implode + zlib.
`cargo test -p pcx-mpq` → 18 passing, including an end-to-end `(listfile)` extraction that
decodes 529 298 bytes of PKWARE-imploded sectors into real WC3 paths.

**Done when:** every shipped archive opens, lists and extracts. *(Met — remaining: an
`mpqxtract` CLI and a byte-identical-vs-Storm golden diff; both are nice-to-haves, not
gates.)*

### M1b — Play-test harness skeleton
`pcx-play` (scenario parser + action scheduler + trace recorder) and the `pcx-diff`
binary (trace comparison + first-divergence report), both Bevy-free. One hand-written
scenario per Rosetta campaign chapter proving the loop end to end.

**Landed as a skeleton, deliberately.** `pcx-play` runs synthetic in-memory scenarios
only — a scenario declares its starting state directly and no map container is read —
because map ingestion is M3 and script execution is M7. What it *does* prove is the two
things the skeleton exists to prove:

- **The trace format is canonical and self-describing.** A header carries scenario,
  map, asset-mode (`vanilla` / `hd` / `reforged`), seed, the active fix-ledger rows
  (`adopt|reject|defer <id>`) and the tick count, so a `vanilla` run can never hash or
  read like a `reforged` one and `pcx-diff` names `asset-mode` first. Per-tick rows
  record unit state, RNG draws and timer events. The codec is hand-rolled with **no
  serde** — byte-stability needs fixed field order and sorted rows, and the decoder
  refuses anything that is not already canonical, so `encode(decode(b)) == b`.
- **Determinism is asserted, not assumed.** Two independent runs of the same scenario
  produce byte-identical `encode()` output and equal FNV-1a 64, repeated at 10,000
  ticks in an `#[ignore]`d test. `pcx-diff` reports the first divergence as one line:
  `unchanged` (exit 0) or `diverge tick=<n|header> field=<path> left=<v> right=<v>`
  (exit 1), comparing header identity first, then ticks in order, then the record tail.

**Done when:** `cargo run -p tools/playrun -- tests/play/roc/tutorial.rpl` emits a trace that
is byte-identical across 100 consecutive runs, and `pcx-diff` reports `unchanged` against a
baseline captured from the original.

### M2 — Asset decoders + first Bevy render · *the rendering/material gate is closed*
`pcx-blp`, `pcx-tga`, `pcx-mdl`, `pcx-slq`. Bevy loaders. A test scene draws one Footman
with correct skinning, every geoset bound to the material it names rather than to one
inferred from its position, posed on a terrain tile, all decoded from the shipped MPQs —
and the artefact is a PNG asserted by a pixel test, not something a human eyeballs.

> `pcx-mdl` container is solved: **3,234 / 3,234** models parse on 1.28c and
> **3,249 / 3,249** on 1.29b, all at MDX version 800, with `MODL`, `SEQS`, `TEXS`,
> `GEOS`, `BONE`, `MTLS`, `GEOA`, `PIVT`, the node chunks and `TXAN` / `RIBB` /
> `PREM` / `PRE2` / `CAMS` / `LITE` / `CLID` decoded. The plan's assumption of a chunked
> `.mdl` was wrong — 1.28c ships only `.mdx`, and has no `PINS` chunk.
> **No MDX chunk is left with undecoded framing.** What remains is refused *semantics*,
> not open chunks — `PRE2`'s 171-byte prelude and `CLID`'s float meanings are measured
> in width and deliberately left unnamed. See `docs/re-notes/004-mdx.md`.
**Done when:** a test scene shows one Footman with correct skinning, animation, and a
sprite-lit terrain tile, all decoded from the shipped MPQs. *(Met for skinning, material
binding, terrain **and animation** — the viewer now plays a real `AnimationClip` built
from the model's own keys, on **Bevy 0.20**. The four remaining leftovers are listed
below as open follow-on work, not folded into this gate.)*

> `pcx-blp` is done for 1.28c: **6,273 / 6,273** shipped textures decode, header and
> mip chain asserted, JPEG and direct paths both covered. See
> `docs/re-notes/003-blp.md`. Two things did not survive contact with the bytes — the
> palettised alpha plane is planar rather than interleaved, and the JPEG mips are CMYK
> that subtracts from white rather than multiplies. BLP0/BLP2 do not occur in 1.28c and
> stay open for Reforged.

#### The render is now checked by a pixel test, not by an eye

`crates/pcx-gfx/examples/capture.rs` renders the Footman **offscreen** — no
window, `primary_window: None`, `ExitCondition::DontExit`, camera aimed at a
`RenderTarget::Image` — poses the skeleton at 7,000 ms (the first key of the clip
that actually animates) and reads the framebuffer back. That closes the old "a
human must eyeball the render" item: the artefact is the PNG, and `analyze`
asserts on it, so a blank or monochrome buffer fails the run instead of being
signed off.

Measured, on the 320×320 capture with the Ashenvale dirt/grass tile under the
model, **after** the material binding was applied:

| property | measured |
|---|---|
| PNG | 45,759 bytes |
| opaque coverage | 102,400 / 102,400 (100%) |
| lit pixels (`r+g+b > 24`) | 19,102 — **18.7%** |
| distinct colours (5-bit buckets) | **1,353** |
| mean `rgb` | 21.9 / 22.3 / 23.6 |

18.7% lit and 1,353 distinct colours is the part that matters: a black frame gives
0 and a flat-shaded one gives a handful. The row profile has a wide middle and
narrow top — a standing figure, not a plane — and the figure's upper blocks carry
a clear positive blue bias against a neutral background, which is the
bluish-steel armour reading as steel rather than as grey.

**Re-measured after the Bevy 0.20 upgrade** (same model, same tile, same camera):
**46,006 bytes**, still 100% opaque coverage, still **19,102 lit pixels — 18.7%**,
**1,339** distinct 5-bit colours, mean `rgb` 22.0 / 22.4 / 23.7. The render is
unchanged by the engine migration, which is what makes the migration verifiable
rather than merely compiling: the silhouette still runs head → torso → legs with
the terrain tile under it. (The 14-colour drop is anti-aliasing/rounding in the
newer render path, not a material change — the silhouette and the camera are
identical.)

The capture is also what proves the material fix: binding `textures[material_id]`
instead of cycling by geoset index moved it from **49,953 to 45,759 bytes** and from
**1,588 to 1,353** distinct colours, with **11,704 pixels (11.4%)** changed in the
lower body only — silhouette and camera identical. The Footman's five geosets
carry `material_id` `[0, 1, 0, 0, 2]`; index cycling gave `[0, 1, 2, 0, 1]`, which
held for only 81.9% of the shipped set, i.e. **521 models and 3,121 geosets** drew
with the wrong texture. The blood geoset now draws `gutz.blp` and the empty-material
geoset is flat grey.

#### What is still open in M2, precisely

Nothing material-binding-related remains, and animation is no longer on this list —
see below. These are the genuine leftovers, and they are follow-on work rather than a
partly-closed gate:

1. **The interactive `view` binary draws no terrain tile.** The capture does; the viewer
   does not. Both are one line of scene setup apart and neither is a format gap.
2. **`pcx-gfx` asset-cache plugins and async decode.** Also the one §3.8 structural
   constraint that is unimplemented rather than impossible.
3. **`pcx-blp` BLP0 / BLP2.** Neither occurs in 1.28c or 1.29b; they are Reforged-only, so
   they belong to M1c rather than to a decoder gap here.
4. **Rendering *parity*, as distinct from rendering.** The capture is a smoke test for
   "something drew and it is the right shape", not a differential against the original.
   That tier is §3.3 item 3 and it is unbuilt.

#### Animation now reaches the pixels through the model's own keys

`pcx-gfx`'s viewer builds a Bevy **`AnimationClip` per `SEQS` sequence** straight from
the model's own `KGTR` / `KGRT` / `KGSC` key data — the same bytes the decoder's
`sample_bone_pose` interpolates — and plays it through an `AnimationGraph` on a fixed
tick rather than on the clock. The offscreen capture still poses at 7,000 ms; the viewer
now actually animates.

The test that pins it is deliberately *not* a key-value test. Every animated bone of
the Footman and the Knight is sampled at instants **between** the bone's own keys
(`between_keys` walks off any key frame it lands on), so what is asserted is the
**Hermite interpolation**, not the stored key values. Two carriers are used because the
Footman translates on one bone and rotates on another and carries no bone with both a
`KGTR` and a `KGRT`; the Knight supplies the bones that do. Past a sequence's end the
clip must hold that sequence's last pose rather than slide into the next sequence's
keys, and the test asserts that a sequence exists where that is actually observable, so
it cannot pass vacuously. It is proven non-vacuous by construction: returning
`Quat::IDENTITY` or `Vec3::ZERO` from a curve fails it.

### M3 — World: maps, terrain, camera, doodads
`pcx-map` + `pcx-tera`: build terrain from the map's tilesets with the original's
4-corner blending and cliff rules, place doodads, wire the camera to the original's
limits/pitch behaviour.

The admitted-map ground publisher now uses the selected catalog and sheet bytes,
source corner selectors, isolated variation-array layers and linear mip sampling.
Publication keeps the prior draw and provenance when a replacement fails. A local
Vulkan pixel gate verifies cell-edge isolation and actual minification; it does
not establish native terrain appearance. Explicit budgets and the matching source
reader are required. Gameplay camera/world activation, cliffs, water, lighting,
shadows, fog and original filter equality remain open. See
`docs/re-notes/admitted-ground-rendering.md`.

**Measured, and it overturned the assumed framing.** There is no `TERRAIN` chunk in a
`.w3m` and no chunk table. Both builds write `HM3W` + a fixed `0x200` header + an
**embedded MPQ archive**, with `archive_size == file_len - 0x200` on all 133 shipped maps,
and `game.dll`'s writer stores that literal and then opens an MPQ rather than returning
to a chunk table. Terrain is `war3map.w3e` *inside* the embedded archive. The trailer is
`NGIS` + 256 opaque bytes, present on 89/133 and absent on 44; any other trailer length
or a missing `NGIS` is refused.

Also measured, and **the height ramp does not exist**: the corner table consumes the
terrain file *exactly* on all 132 terrains, leaving zero bytes for the interior grid the
older format documentation implies. `corner_size == 1` — one vertex per tile corner,
heights stored as raw **14-bit indices**, handed out as `height_index` with no world-unit
claim, because the shipped indices only span 5,477..14,336 of 16,384 and cannot supply the
mapping either. Terrain here is *per-corner*, not a sampled interior heightfield; any
tiled/blended terrain assumption has to be re-derived from this rather than assumed.

**The rest of the map's content files are decoded too**, each with its own framing
identity rather than a count — 132/132 shipped maps carry all four, on both builds:

| file | layout | identity |
|---|---|---|
| `war3map.doo` | `W3do` + version 7 + `n` × 42-byte instances + `m` × 16-byte terrain doodads | `16 + 42·n + 8 + 16·m == file_len` |
| `war3map.shd` | `(4·w) × (4·h)` one-byte mask, no header — its grid comes from the terrain, not from itself | `file_len == 16·w·h` |
| `war3map.mmp` | marker table: `8 + 16·n`, 16-byte records of kind/`x`/`y`/`0xAARRGGBB` | `8 + 16·n == file_len` |
| `war3mapMap.blp` | the minimap image — all 132 are BLP1 JPEG, `256×256`, one mip; framed through `pcx-blp`, not re-decoded here | `pcx-blp`'s own two relations |

Two refusals in this set, both deliberate: doodad **version 8+** records are wider by
item-drop fields no shipped file proves the offset of, and a map that really contains
the literal `war3map.map` / `war3map.ter` is refused — those names occur in 0 of 133
shipped maps, 0 of 200 loose community maps, no `(listfile)`, and neither build's name
tables, so there is no layout to read them with.

**Done when:** the Human campaign's first mission loads and pans identically to the original.
**Not yet:** the scripting half of the container — the trigger graph is *decoded* as a
container (§3.11, `pcx-trig`) but the WTG→JASS desugaring, the VM and gameplay are M7
work. `pcx-tera` mesh generation is also still open.

### M4 — Object data
`pcx-objects`: the shipped object editor data plus `game.dll`-derived defaults →
unit/ability/upgrade/item/buff/destructable tables.

**Now also carries** `W3U`/`W3T`/`W3A`/`W3B`/`W3D`/`W3H`/`W3Q` — the per-map object
*edits* — because DotA and Athena both ship custom objects and a map whose rawcodes do
not resolve is unplayable. Object identity is the join key between the scripting layer
and everything else.

**Measured, and the family is smaller than the extension list suggests.** The
`W3?` container is decoded on **92 files** — 47 `.w3u` + 1 `.w3t` on 1.28c, 44 `.w3u`
on 1.29b — carrying **2,179 object records** and **5,975 modification entries**. Only
two of the seven extensions ever ship; the other five (`w3a`/`w3b`/`w3d`/`w3h`/`w3q`)
occur in **no map on either build** and are **refused** (`Error::UnmeasuredExtension`)
rather than run through a w3u-shaped walk, because nothing measures whether ability
records carry the level/variation columns the community notes describe.

```text
offset  size  field
0x000   4     version word — 1 in 91 files, 2 in one (same layout, byte for byte)
0x004   4     B   count of first-table objects (base modifications)
        ...   B object records
        4     C   count of second-table objects (custom rawcodes)
        ...   C object records, then EOF

object record      +0x00 original fourcc · +0x04 modified fourcc · +0x08 M · M entries
modification entry +0x00 field fourcc · +0x04 type word (0 int / 1 real / 2 unreal /
                   3 string) · +0x08 value · last 4 end word
```

The framing holds as arithmetic on **92/92**:
`file_len == 4 + 4 + Σ_base(12 + Σ_mods) + 4 + Σ_custom(12 + Σ_mods)`, with
`mod_len == 8 + value_len + 4` and `value_len == 4` for types 0/1/2 and `strlen + 1`
for type 3 — plus round-trip `canonical_bytes() == raw` on the same 92. Strings are
**NUL-terminated, not length-prefixed** (1,937 of them; a leading length word is
refuted by the bytes). Two further relations hold on 2,179/2,179 records: every
first-table record has `modified id == 0` and every second-table record has
`modified id != 0`.

**Deliberately refused semantics:** per-fourcc int width and signedness (a four-byte
slot holding small values is indistinguishable from a `u8` field, so the raw word is
handed out), what `real` and `unreal` do differently, and the **end word** — kept raw
because 285 of 5,975 carry something other than zero, and 179 of those sit next to
`umdl`/`uico` art paths with what reads like the owning unit's rawcode. Two
hand-checked samples are a hypothesis, not a proof.

**And the base data under them is now decoded.** `crates/pcx-objects/src/editor/`
reads every object-editor file the shipped archives carry, through `pcx-slq`:

* Inventory, measured per build from the archive listfile union — `Units\` **15
  `.slk` + 48 `.txt`** (1.28c) and **17 + 49** (1.29b), `Doodads\` **2 + 0** on both;
  59 and 65 `.slk` in the union overall. **There is no `Abilities\*.slk` and no
  `Misc\` directory**; the ability data is `Units\AbilityData.slk`, the misc
  constants `Units\/`UI\MiscData.txt`, the UI table is spelled
  `Units\unitUI.slk`, and the unit localisation tables are `.txt`, not `.fdf`.
  `Melee_V0`/`Custom_V0`/`Custom_V1` are the World Editor's working copies and are
  deliberately not decoded. **The listfile union itself under-counts**: 1.28c's
  `Units\AbilityBuffData.slk` and `AbilityBuffMetaData.slk` live in
  `War3Patch.mpq` and no listfile on that build names them, yet they decode.
* The fourcc join, measured not remembered: `Units\UnitMetaData.slk` is the
  editor's own schema, and **267/267 rows resolve** to a row-1 column of the shipped
  table or to a `.txt` key. **The editor's declared `type` predicts the modification
  type word of 3 393 of 3 401** 1.28c entries — the one exception, `uhot` (a declared
  `char` on the string wire, 8 times), is applied as text and its `char` reading
  refused.
* Raw codes are matched **case-insensitively**, because `Units\UnitBalance.slk`
  row 664 spells `Nman` where `Maps\Campaign\Orc04Interlude.w3m`'s `war3map.w3u`
  writes `nman` (91 mixed-case raw codes in that column alone).
* Refused, with typed `Refusal`/`Unresolved` errors: per-fourcc int width and
  scale, the 8 `Profile` fourccs (`urq3..urq8`, `urqa`, `urva`) whose key ships in
  no shipped `.txt` section, and the 1 001 field rows this crate's `.txt` scope does
  not cover (724 ability, 192 misc, 55 upgrade-effect, …). Every one is counted and
  reported; no shipped map sets them.

**Done when:** every stock unit's stats match the in-game editor exactly (automated diff),
**and** a map with custom objects resolves every rawcode it declares. The second half
now measures over the whole shipped corpus, on both builds: **1.28c 888 base records
(887 resolved) + 282 custom (282 new)**, **1.29b 757 (756) + 252 (252)**, 0
collisions, 0 unknown fourccs. The single record that resolves to nothing is
`Maps\Campaign\Orc02Interlude.w3m`'s `war3map.w3u` base record for **`Nwar`**, on
both builds — reported by name, in `Refusal`, rather than dropped.

### M5 — Simulation core
`pcx-sim`: entities, stats/attack/armour/resist model, damage types, orders, pathing,
cooldowns, mana/regen, exp/levels, upkeep, projectiles, death/ragdoll.
**Done when:** scripted 1v1 duels produce identical HP curves and identical RNG draws
against the original under `compare-process-captures`.

### M6 — Game-loop parity
Tick rate, RNG parity, timer/event ordering, effect and sound scheduling, camera shake,
save/load of `.w3s`.
**Done when:** a 10-minute scripted match is bit-reproducible across 100 runs.

### M7 — Scripting
JASS / trigger VM faithful to `game.dll`, including natives, the `BJ`/`Blz` library
surface, localization handles, timers, handles, and event ordering.

**Two code paths, one engine.** Community maps reach the VM two different ways and
**both** must work:
- **Raw JASS** — a hand-written `war3map.j` shipped in the map, no trigger graph. DotA
  is this path.
- **Trigger graph** — the editor's `WTG` (+ `WCT` for the trigger's custom text), which
  must desugar to JASS with the original's observable semantics and preserve names
  referenced by generated scripts and globals. Athena is this path.

A VM that only runs raw JASS, or that runs trigger graphs with subtly different ordering,
passes a smoke test and still fails DotA or Athena.

**Era tolerance is required, not optional.** The community spans pre-1.20 and post-1.24
APIs in active use at once, so the native surface must carry the older shapes and
`Blz` extensions together, and a missing native must fail loudly rather than silently
returning zero.

**Current implementation and its limits.** The syntax/container measurements
below establish parsing. The resolver, interpreter and persistent application
session now exist; they do not establish complete native or map behavior:

- **`pcx-script` — raw-JASS syntax and execution.** Lexer, parser and an exact tree. It parses
  **1,326 real scripts** with round-trip proven on every one — 462 install scripts
  (`Scripts\*.j`/`*.ai` out of both installs' MPQs), 191 `war3map.j` from the shipped
  `.w3m` maps, 676 from the community `.w3x` maps in those Maps trees, and 3 from
  `DemoCampaign.w3n`. The whole corpus is 24.1M tokens, 335k functions, 2.29M
  statements. Round-trip proof is re-lex to identical token kinds, re-parse to identical
  canonical bytes, the same FNV-1a 64 hash, and idempotent printing. Every node carries
  a `Span`, and `SourceMap` turns those into the map's own 1-based line/column — which
  is what makes "fail loudly with the map's own identifier" implementable.
  Those corpus measurements are syntax evidence. Current code resolves names,
  evaluates typed values, arrays and functions, and dispatches caller-owned natives
  with source diagnostics. `MapScriptSession` owns one admitted VM and rejects
  callbacks belonging to a replaced session. The application now has a reusable
  source-bound Host factory and a delivery-to-session bridge: caller-qualified
  inputs run globals, `config` and `main` on one candidate before publication.
  Refusals preserve the prior session; stale deliveries cannot consume new inputs.
  The same interpreter can now park an opt-in Host native inside expressions and
  resume its result without repeating completed effects. Locals, source spans,
  call depth and cumulative step work survive. Localized and neutral wrappers
  forward that hook. The application session remains synchronous: a suspending
  replacement refuses and preserves the old session and callback tickets.
  See [cooperative continuations](docs/re-notes/script-continuations.md).
  Automatic preparation of those inputs, native coverage, scheduling and complete
  save state remain open. vJass `method`/`cast`/`delegate`, `.` member
  access, bare `!`, comma local lists and chained `a[i][j]` remain explicit refusals.
- **`pcx-trig` — the trigger-graph containers.** `war3map.wts` (table strings),
  `war3map.wtg` (the editor's trigger graph) and `war3map.wct` (a trigger's custom
  text), each walked byte-exactly on every shipped file — 133/133 wts, 132/132 wct,
  132/132 wtg — with the cross-file relation `wct.blocks == wtg.triggers`. **No
  trigger semantics are interpreted**: an ECA name says which editor function is
  stored, not what it does, and each ECA's parameter count comes from
  `UI\TriggerData.txt`, never from the file. (`X Hero Siege 3.08.w3x` is refused as
  unprovable: it opens `?STRING 1`, a stray byte no other file carries.)

The bounded WCT/custom-script compilation path also preserves ordering and source
diagnostics. Full WTG-to-JASS behavior, generated-name compatibility and all
unsupported profiles remain open. Container and parser tests do not prove those
semantics.

The current campaign Host retains executed player, sound, camera, rectangle,
technology, destructable and source-bound unit state. Human01 globals and `config`
complete in the retained local probe; `main` now retains the executed
`SetHeroLevel` request and `SetUnitState` assignment. Explicit source-bound Defend
admission passes both fresh-unit off requests as false; `main` retains 155 units
and then retains 26 explicit directed alliance assignments and one player-state
request, then creates seven source-qualified `Filter` wrappers and retains
explicit force memberships. The selected script creates 17 forces and 16
memberships. With explicitly supplied, locally captured startup participation,
null-filter force enumeration replaces the all-player force. Explicit startup
speed/fog/mask input passes the three getters. Explicit captured slot-state
input passes `GetPlayerSlotState`. A separate explicit current-controller
view serves later source player rows without overwriting executed setup; the
single-player counting loop completes. Explicit source-backed label-constructor
banks qualify all 11 literal labels and retain 11 executed creations across nine
labels on the same arena. A separately admitted timer-expiry subscription links
the actual existing trigger and live timer through an event on that arena; its
following action is retained without running it. Explicit source-qualified
runtime version input now passes `VersionGet` without allocating or guessing an
edition. Source/domain-bound game-state subscriptions now retain six executed
trigger/state/comparison/threshold associations on that arena. Separately admitted
camera-bound getters now derive all four coordinate extrema from the executed
corners in a caller-qualified unrotated profile. Eight reads match the corrected
original-client observation; both source BJ rectangles are retained. The next
unsupported call was the boolean `GetPlayerTechResearched`. Explicit current
research observations now pass its 24 exact startup queries after the authored
early setter; retained setter levels remain separate and missing answers refuse.
Explicit item and unit slot configuration now stores the integers passed to
`SetAllItemTypeSlots`, `SetAllUnitTypeSlots`, `SetItemTypeSlots` and
`SetUnitTypeSlots`. No BJ slot count is copied into the host. A per-unit
record does not inherit the global integer. Add/remove stock, restock and
shop simulation remain open. In `InitNeutralBuildings` the next unsupported
call after those two global assignments and `CreateTimer` is `TimerStart`.
See `docs/re-notes/script-stock-slots.md`.
Comparison evaluation, timer start, expiry and callback delivery remain unsupported. Fog/mask setters retain
independent booleans;
speed writes remain unsupported because original requests do not always read
back as requested. No participation or settings default is inferred. Wrapping
does not evaluate predicates. See `docs/re-notes/script-startup-settings.md` and
`docs/re-notes/script-player-slots.md` and
`docs/re-notes/script-player-controllers.md` and
`docs/re-notes/script-label-sounds.md` and
`docs/re-notes/script-trigger-admission.md` and
`docs/re-notes/script-camera-input.md`.
These assignments do not enforce alliance gameplay or effective resource balances,
and on/off activation is separate from combat modifiers and animation. This is
execution evidence for that selected source profile, not a playable chapter.
The application Host factory does not yet connect execution to simulation,
terrain and HUD. See
[map admission](docs/re-notes/map-data-admission.md) and
[executed Host state](docs/re-notes/script-player-setup.md).

The application now drives published campaign timers on logical ticks with
retained catch-up work, and direct initialization-trigger callbacks execute in
the owning VM. Loading time is excluded from a newly published session. The
Human01 candidate now reaches its initialization action and refuses
`SetCampaignMenuRace`; it is still not a playable chapter. Automatic startup
input preparation and the live world, rendering and controls remain open. See
[campaign runtime](docs/re-notes/campaign-runtime.md).

Explicit session-local player input now reaches `GetLocalPlayer` and the
executed camera branch using the same arena identity as `Player`. This removes
one required camera-helper gap; automatic first-chapter bootstrap and the last
observed `SetCampaignMenuRace` initialization refusal remain open.

Executed day/night model arguments can now prepare their lighting inputs through
the session's retained source reader, with no chapter-specific model list.
Other automatic Host inputs, world lighting, actors and controls remain open.

Remaining work includes native behavior and handle lifetimes, actual trigger
registration/delivery and VM callbacks, timer/event ordering, the `BJ`/`Blz`
surface, complete localization substitution, and application game-loop wiring.
Implement from the selected declarations and observed behavior; do not introduce
natives merely because a roadmap example names them. The syntax/VM foundation
cannot close the campaign and community-map gates below.

**Done when, and this is the real gate:**
- **DotA** runs: joins, creates units, runs its timers, and completes a match without a
  script error. Tick-ordered observable behaviour matches the original under
  `compare-process_captures`.
- **Athena** runs, proving the `WTG` trigger path desugars correctly and not just the
  script path.
- **Footman Frenzy** runs, proving pre-1.20 API tolerance.
- All stock maps load and run their triggers, and the campaign completes.

A scripting layer that satisfies none of the three named maps has not met this
milestone, regardless of unit-test count.

### M8 — Full campaign + polish parity
All 4 races, campaign AI, cinematics, multiplayer-shaped netcode surface (loopback first).

### M1c — Reforged asset pipeline
`pcx-casc` reads the local Reforged install's CASC storage (`Data/data/*.idx` + `*.0NN`);
`pcx-assets` resolves a path across vanilla MPQ, Reforged CASC and `override/`, recording
which layer won. Asset modes (`vanilla` / `reforged` / `reforged-hd`) selectable at runtime,
`vanilla` by default.

**Done when:** `--assets reforged` loads the Reforged Footman model and its textures through
the same `pcx-gfx` path as the vanilla one, and every resolved asset reports its source
layer in the trace header.

### M10 — Multiplayer

- [x] Local browser WebSocket transport to the existing authoritative movement simulation: two browsers exchange binary commands and matching snapshots; same-tick input merging, spoof rejection and reconnect/resync verified. See [browser sessions](docs/browser-online.md).
- [ ] Connect the session to actual game input/rendering and original lobby flow; complete map initialization, combat and economy. The movement transport does not close the multiplayer-match gate.
- [ ] Public sessions: asynchronous admission, TLS, room isolation and identity; no public service deployed.

- [ ] `pcx-net`: transport traits, command codec, authoritative lockstep server
- [ ] Gate: a real 1.29b LAN client joins a game we host and plays it out
- [ ] Classic Battle.net client against an emulator realm
- [ ] Our own low-latency protocol, benchmarked against the classical ones
- [ ] Own hosted sessions validate state and proposed commands on the host.
      Classic crossplay preserves original action/lockstep/checksum semantics
      with host admission and relay validation; original clients do not consume
      the project's movement snapshots.

### M11 — WASM build

- [x] Skippable original startup cinematic from locally supplied media, followed by the menu. Browser playback, soundtrack pausing/resuming, mute preference and native decoder cleanup are implemented; see [movie evidence](docs/intro-movie.md).
- [x] TFT logo uses parsed authored Birth Bézier and the traced default sprite origin.
- [x] Hanging-panel entrance: TopLeft/TopRight sprite layers play parsed `MainMenu Birth` then `MainMenu Stand` with their authored `SNDX` sounds; see `docs/re-notes/menu-sprite-rendering.md`.
- [ ] Preserve original resolution handling across common classic and widescreen modes, resize and display density. Background fills the display; ordinary UI follows the original wide-window branch and direct classic conversion. Portrait containment is a browser extension. Record native live flags before claiming exact parity at every resolution.

- [ ] **Local ISO import, entirely inside the browser.** The user selects an
      ISO through the file picker. Its contents are never uploaded to a server,
      sent to a remote extraction service, or included in telemetry. The page
      reads it through the File/Blob APIs and decodes it locally in JavaScript
      or WASM, with progress, cancellation, and bounded-memory reads.
      Discover the image's filesystem (ISO 9660/Joliet or UDF as applicable),
      locate Warcraft III MPQs, and validate their headers, edition/build
      evidence and required members before registering the right asset set in
      the browser's existing memory asset source. Preserve archive priority
      and identify ambiguous/mixed installations instead of silently choosing
      one. No installer executable is run. Images carrying an installer or
      repacked payload need a verified local decoder; unsupported payloads
      must name the blocker rather than report successful import.
      **Done when:** a supported original ISO loads the actual menu/game
      assets locally, a browser network capture shows no ISO/archive/asset
      content leaving the browser, and truncated images, missing expansion
      files and ambiguous installations produce actionable errors. A huge ISO
      must not be copied wholesale into WASM memory just to scan it.
- [ ] **Menu feels and sounds like the original.** Use the original model,
      textures, font, animation timing, button states, music and interaction
      sounds from the selected asset set. Verify rendered behavior and audible
      playback; asset counts or a successful compile alone do not close this
      fidelity gate. Browser autoplay requires a first input gesture.
      PeaceCraft-specific asset/build/behaviour settings appear before entering
      the original main menu. This pre-launch configuration is the deliberate
      extra screen; it does not authorize redesigning the game's menus or
      in-game UI, controls, animation or audio. The original Options screen
      must eventually expose the original game options, rather than serving
      as a substitute for PeaceCraft configuration.
      UI uses Bevy 0.20's current widget and layout APIs with relative flow and
      viewport/font-relative sizing. Avoid absolute element positioning.
      Verify resizing at multiple aspect ratios. Engine implementation choices
      must preserve original simulation, input and UI behaviour; modern Bevy
      conventions do not justify changing gameplay or menu semantics.
- [x] `cargo build --target wasm32-unknown-unknown` **links** — the whole
      workspace: `pcx-sim`, `pcx-net`, both decoders, every binary, the launcher
- [x] `pcx-sim` + `pcx-net` build for `wasm32-unknown-unknown`, transport `cfg`'d
      out (a browser has no UDP)
- [x] `pcx-mpq` reader works from an in-memory buffer (`from_bytes`, same parser)
- [x] Browser entry point: `crates/peacecraft3/src/browser.rs` supplies
      `#[wasm_bindgen(start)] fn start()` — it runs the Bevy app and *returns*,
      because a browser has no process to `exit`. `wasm-bindgen` is a
      `cfg(target_arch = "wasm32")`-only dependency.
- [x] In-browser asset path: Bevy's default wasm asset source (an HTTP reader
      rooted at `assets/`) is replaced by Bevy's own `MemoryAssetReader`, and the
      archives are read with `MpqArchive::from_bytes`.
- [x] JS side: `web/index.html` + `web/loader.js` + `web/pkg/` (the
      `wasm-bindgen --target web` output). No trunk, no wasm-pack, no bundler.
      Exports offered to the page: `add_archive`, `add_asset`, `drop_assets`,
      `stored_assets`, `selftest`, `start`.
- [x] **Launcher verified running in Chromium with WebGL** against the local
      served originals: desktop and narrow viewport sizing, Options/Main menu
      navigation, settings scrolling and original button/font loading pass.
      Original music and canvas click playback are also verified. This closes
      the launcher runtime gate; full game playback and menu visual fidelity
      remain separate requirements above.
- [ ] `pcx-gfx` cache is async; no synchronous file I/O on a shared path
      *(the only one of §3.8's structural constraints that is still genuinely
      unimplemented rather than impossible)*
- [x] **Dev artifact size measured: 975,009,181 bytes, of which 913 MB (93.6%) is
      DWARF + `name`**, and only 51,609,943 is `code` plus 8,629,367 `data`.
- [x] **Debuginfo-stripped artifact measured: 119,794,662 bytes**
      (`web/pkg/peacecraft3_bg.wasm`) — no DWARF at all. 63 MB of it is still the
      `name` custom section (symbol names, not code); `code` is 47,289,068 and
      `data` 8,629,367. Same factor-of-eight lesson: measure the release path,
      never the dev artifact.
- [ ] `wasm-strip` / `-C strip=symbols` on the 63 MB `name` section — the last
      size win, unmeasured.
- [ ] Three browser limitations documented rather than fixed: staging (no
      destination to copy into), install-level version detection (a single archive
      can be opened and counted, but not identified as 1.28c or 1.29b), and
      rendering without a canvas.
- [ ] `crates/peacecraft3/README.md` — `web/loader.js` points at it for the build
      recipe and it does not exist yet.

### Desktop build targets

- [ ] Compile and package `peacecraft3` for `x86_64-pc-windows-msvc`,
      `aarch64-pc-windows-msvc`, `x86_64-apple-darwin` and
      `aarch64-apple-darwin`. Each target needs a successful compilation, native
      test run and an artifact whose executable header matches its OS and CPU.
      `.github/workflows/desktop-builds.yml` and `tools/build-desktop.py`
      provide that four-target matrix; configuration alone is not proof of
      compilation. See `docs/build-desktop.md` for commands and current limits.
      No original game assets are bundled. GPU/audio runtime verification on
      Windows/macOS is a separate acceptance check.

### M9 — Campaign playthrough gate (Tier A + B complete)
Run §3.5 Tier A across every chapter of every stock campaign and Tier B to completion,
against baselines pinned to REA evidence ids.

**Done when:** `cargo test --workspace --features play` passes Tier A for all shipped
campaigns and maps, and Tier C reports zero unexplained divergences over the seed corpus.

---

## 5.1 Cross-version cross-check

The 1.29b numbers are the gate; the 1.28c column is insurance that paid for itself once.
Current numbers:

| Decoder | 1.28c | 1.29b |
|---|---|---|
| `pcx-blp` textures | 6,273 / 0 fail | 6,696 / 0 fail |
| `pcx-mdl` models | 3,234 / 0 fail, 8,672 meshes | 3,249 / 0 fail, 8,770 meshes |
| `pcx-mdl` geometry | 968,613 verts / 819,287 tris | 978,194 verts / 826,833 tris |
| `pcx-mdl` skeleton | 35,097 bones, 0 fail | 35,257 bones, 0 fail |
| `pcx-mdl` materials | 6,959, 0 fail | 7,057, 0 fail |
| `pcx-mdl` skinning | verified | verified — `MTGC` names exactly `MATS`'s useful indices on 2,851/2,851 geosets |
| `pcx-mdl` `GEOA` | decoded | 5,405 records, 10,456 tracks, interp-aware key framing |
| `pcx-mdl` `PIVT` + node chunks | 60,169 pivots, 15,923 `HELP`/`ATCH` nodes | 60,434 pivots, 15,999 `HELP`/`ATCH` nodes |
| `pcx-mdl` `TXAN` | 129 records / 146 tracks | 150 / 167 — `len == 4 + Σ tracks` |
| `pcx-mdl` `RIBB` | 361 / 431 | 354 / 420 — `total == 4 + node + 52 + tracks` |
| `pcx-mdl` `PREM` | 64 / 66 | 64 / 66 — `total == 4 + node + 284 + tracks` |
| `pcx-mdl` `CAMS` | 644 / 466 | 656 / 469 — no node header; `size == 120 + Σ tracks`, `far > near` |
| `pcx-mdl` `LITE` | 250 / 245 | 250 / 245 — `total == 4 + node + 44 + tracks`, `kind ∈ {0,1,2}` |
| `pcx-mdl` `PRE2` | **framing decoded** — 3,572 records; `node_size == 96 + Σ node tracks`, `total == 4 + node_size + 171 + Σ sub-chunks`, sub-chunk chain sorted `KP2S < KP2L < KP2E < KP2V < KP2N < KP2W` | **3,628** records; same three relations, 7,200/7,200 across both builds |
| `pcx-mdl` `CLID` | **framing decoded** — 832 records; `header_size == 96 + Σ node-track bytes` (**not** a constant 96), kind `∈ {0,2}` or refused | **846** records; same relation, 96 of them carry a `KGTR` track inside the header |
| `pcx-gfx` loaders | — | 16 tests; Footman → 5 Bevy meshes, all attributes intact |
| `pcx-gfx` geoset→material | **applied** | `to_bevy` binds `textures[material_id]`; read at `MATS+count*4`, proved on 8,770/8,770 geosets of 3,249 models |
| `pcx-gfx` offscreen capture | — | works: **46,006-byte PNG**, 100% opaque, **18.7% lit**, 1,339 distinct colours — re-measured after the Bevy 0.20 upgrade |
| `pcx-gfx` viewer | runs | `view` spawns the Footman (5 geosets, camera/light framed from extents) and **plays a real `AnimationClip`** built from the model's own keys; no terrain tile, unlike the capture |
| `pcx-gfx` `load_image` | fixed | routes JPEG and palettised BLP; was failing on 4,924 of 6,696 textures |
| `pcx-tga` images | 383 / 0 fail | 435 / 0 fail, 0 unread — one pinned footer-split constant is 1.28c-only, see `TRACKER.md` |
| `pcx-slq` tables | 59 / 59, 0 fail, 768,067 cells, 10,373,503 B | 65 / 65, 0 fail, 983,763 cells, 12,971,315 B — 12 vs 13 sound tables |
| `pcx-map` container | 89 maps, `archive_size == file_len - 0x200` on all 89 | 44 maps; same relation, 133/133 across both builds |
| `pcx-map` terrain | 88 `w3e` decoded, framing exact, 0 remainder | 44 `w3e`; same relation |
| `pcx-map` `doo` / `shd` / `mmp` / minimap | 132 present, 132 parsed, 0 fail | 132 present, 132 parsed, 0 fail (both builds share the same 132) |
| `pcx-objects` `w3u`/`w3t` | 48 files (47 `.w3u` + 1 `.w3t`), framing 92/92 across both builds, round-trip exact | same two relations; 44 `.w3u`, one at version 2 |
| `pcx-script` JASS | 1,329 real scripts, 1,326 parse + round-trip, 0 fail; 24,085,861 tokens, 335,112 functions, 2,288,966 statements | same corpus from the Legacy install's MPQs; the corpus spans both trees |
| `pcx-trig` `wts` | 89 files, 5,171 records | 44 files, 4,431 records — 913 files and 511,490 records including the 780 loose community maps on disk |
| `pcx-trig` `wtg`/`wct` | 88 + 88 files, 3,207 variables, 4,689 triggers (7 custom), 57,289 ECAs, 172,287 parameters, 4,689 wct blocks | 44 + 44 files, 3,113 variables, 4,442 triggers (7 custom), 55,036 ECAs, 165,922 parameters, 4,442 wct blocks |
| `pcx-play` trace | byte-identical across two runs, equal FNV-1a 64; stable at 10,000 ticks | same — the harness is synthetic, so there is no build dimension to differ |

The effect-chunk counts are the one place the two builds genuinely move against
each other: `PREM` and `LITE` are identical on both, `TXAN` and `RIBB` trade
places. Every framing equality holds on **both** with zero exceptions, which is
the claim the gate is making — the counts are expected to differ, the layout is
not.

`CAMS` and `LITE` are deliberately *not* counted in the node row: neither can be
walked by the shared `BONE`-shaped reader, which is itself a gate result. See
`docs/re-notes/004-mdx.md` for the measured misread rates.

**"Framing decoded" is a narrower claim than "decoded", and the two `PRE2`/`CLID`
rows mean it precisely.** What is proved for both is the byte arithmetic: every
record's size fields account for its own bytes exactly, and every sub-chunk chain
lands on the next one with nothing skipped or reordered. What is *not* claimed is
the meaning of the remaining words. `PRE2`'s 171-byte prelude is measured in width
on all 7,200 records and kept as raw bytes; `CLID`'s floats are handed back in
stored order, uncaptioned, on the strength of corpus evidence that a box is
componentwise-ordered corners and a sphere is a centre plus a radius. Both are
refusals, both are deliberate, and both are the safer error: a plausible-but-wrong
collision shape lets units walk through walls. So no MDX chunk is left with
undecoded framing, and no chunk's semantics are quietly filled in to make a table
look finished.

The counts differ, which is expected: 1.29b replaces the World Editor UI. The formats must
agree. One case where they did not, and why it matters:

`Editor-Toolbar-MapValidation.blp` is 878 bytes in 1.29b with a **ten-entry** palette. Every
1.28c BLP carries a full 256-entry palette, so the decoder's fixed-size assumption was never
contradicted by 1.28c alone. Sweeping 1.29b caught it, and the palette is now read as
variable length — it runs from `0x9C` to `mip_offsets[0]`. The mip indices reach only 8, so
the short palette is the file's real shape, not corruption.

A third, stronger gate is byte-level: `the_footman_decodes_identically_on_both_builds`
asserts the Footman decodes to the *same structure* on both builds. The two files are
deliberately not byte-identical — 1.29b rewrites parts of them — so the test hashes the
decoded skeleton rather than the file, and asserts `da != db` so that a future "identical"
result cannot make the test silently vacuous.

That is the whole argument for keeping a second build in the gates: a single-install sweep
confirms the code matches *one* sample, and a decoder quietly generalises past it.

---

## 5. Risks

| Risk | Mitigation |
|---|---|
| MPQ format is not the published one | M0 answers it from `Storm.dll` before anything is built; blocker is explicit and first |
| `game.dll` is 71 k procedures — unbounded scope | Ship vertical slices; every subsystem gets its own milestone gate |
| RNG parity is unattainable without exact generator + call order | M6 is scheduled *after* the sim works; residual unknown is tracked from day 1 |
| Disk (125 GB) vs full asset extraction | Mount, don't convert; convert only what a milestone needs |
| Bevy's abstractions fight bit-exactness | `pcx-sim` is Bevy-free by construction; no shortcuts across that line |
| A scripted campaign playthrough passes while the game is still wrong | Tier A/B assert *specific* outcomes (quest state, win condition, final chapter completion), not "did not crash"; Tier C differential fuzzing catches the rest |
| The original cannot be scripted headlessly | REA `capture-process` + `observe-native-calls` on the decisive routines; if input injection fails, fall back to replaying the shipped `.w3m` with a fixed script in both engines |
| Reforged support could quietly make us "not faithful" | asset mode is explicit, `vanilla` is the only mode §6 is measured against, and every adopted fix gets a ledger row with evidence |
| CASC is a second container format | scoped to `pcx-casc`, layered *above* `pcx-mpq` so the vanilla path stays untouched |
| Legal | We ship **no** Blizzard assets in the repo — we read the user's own install at runtime; the repo contains only our code and format documentation we derive |

## 6. Definition of "faithful"

A subsystem is faithful when, for a scripted scenario covering its behaviour space:
1. our observable outputs match the original's (REA `compare-process_captures` = unchanged), and
2. its byte-level formats round-trip against real shipped files, and
3. every remaining divergence is written down in `TRACKER.md` as a known unknown
with the reason, not silently patched.

And the **project** is faithful — as opposed to merely a good renderer — when, in addition:

4. **Original community maps run unmodified.** Not "a map loads": **DotA**, **Athena**
   and **Footman Frenzy** each start, execute their own scripting without a script error,
   and behave correctly, with observable behaviour matching the original under
   `compare-process_captures`. See [§3.11](#311-map-compatibility-the-real-acceptance-bar).

Criterion 4 is deliberately a different *kind* of test from 1–3. It is an end-to-end
integration gate over the community's actual content, not a diff against a synthetic
scenario, because that is the only test that exercises the join between object data,
the scripting VM and the simulation at the scale real maps use them. A green test suite
that has not passed criterion 4 means we have a renderer with a decoder, not Warcraft III.

Campaign scene connection (2026-10-19): the launcher consumes instant executed
camera setups on the published VM and matching admitted terrain. Source clipping,
field identities and session replacement stay explicit. Projection conventions
need gameplay qualification; this is not first-chapter playability. See
[executed camera consumer](docs/re-notes/script-camera-setups.md#application-camera-consumer).
