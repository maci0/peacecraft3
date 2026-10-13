# RE-001 — Classic 1.28c symbol recovery (no leak required)

Status: **harvested**. Everything here is extracted from `wc3/game/app/game.dll`, which we
already own. No leaked binaries, no PDB, no third-party material.

## What was recovered

`game.dll` embeds its compile-time breadcrumbs — assertion macros, `calldump.log`,
`WEUTiming.txt`, `TerrainMemory.txt` and similar carry absolute source paths from Blizzard's
build machine. A regex sweep for `[A-Za-z]:\...{path}.{cpp,c,h}` yields:

- **254 source paths**, all rooted at `e:\Drive1\temp\buildwar3x\`
- **36 `C`-prefixed type names** matched at symbol-adjacent sites
- the full engine layering

Artifacts: `rea/game-source-paths.txt`, `rea/game-class-names.txt`.

## Engine layering

| subsystem | paths | what it is |
|---|---:|---|
| `war3/Glue` + `war3/glue` | 101 | UI glue layer — dialogs, lists, frames. Two casings, likely two translation units of one tree. |
| `war3/UI` + `war3/ui` | 69 | the UI framework proper |
| `war3/WorldEdit` + `war3/worldedit` | 21 | World Editor |
| `war3/unit` | 14 | `CAbility*`, `CUnit*` — combat, abilities, unit behaviour |
| `war3/Game` | 12 | match/session logic |
| `war3/world` | 3 | `WorldLoad`, `WorldCampaign` |
| `engine/agile` | 7 | the component/agent system (`CAgent`, `CAgentWar3`, `IntegerListener`, `PositionModifier`) |
| `engine/jass2` | 3 | the JASS VM (`Nodes.h`, `Instance.h`, `Compile.h`) |
| `engine/base` | 3 | `CDataStore.h`, `Status.h` |
| `engine/frame` + `framedef` | 6 | the frame graph |
| `engine/FDFile` | 4 | file IO abstraction |
| `engine/anim` | 1 | `Interp.h` |
| `engine/Gx`, `Ipse`, `Tempest` | 3 | graphics, threading, platform layers |

## Why this matters

1. **Naming.** A procedure sitting in the `.text` range referenced from
   `CAbilityCriticalStrike.cpp`'s assertion string is almost certainly
   `CAbilityCriticalStrike`. REA gets that as a comment per function, cheaply and offline.
2. **Module boundaries for `pcx-sim`.** The tree is effectively the dependency graph we
   want: `engine/base` and `engine/agile` are the substrate, `jass2` the VM above it,
   `war3/unit` the rules, `war3/Glue` the presentation. That maps almost one-to-one onto the
   crate split already in `PLAN.md` §3.1.
3. **Priority.** `war3/unit` + `engine/jass2` + `engine/agile` is where the fidelity gate
   lives; `war3/Glue` is the largest tree and the least important for M5–M7.

## Method notes

- The `[A-Za-z]:\` prefix and `.cpp/.c/.h/.hpp/.idl` suffix are the filter. Two casings of
  the same directory (`Glue`/`glue`) survive; merge them when building the map.
- Type names were matched as `C[A-Z][A-Za-z0-9]{3,40}` followed by a space, `:` or `*`.
  That is conservative on purpose — it avoids inventing names that only look like classes.
- This is a *string* harvest, so a name appearing here proves the type was compiled in, not
  that any particular function implements it. Treat every inference as a hypothesis to
  confirm against the code, exactly like any other RE claim.

## Related: the Reforged side (see `PLAN.md` §2.5 and RE-004)

The user's own Reforged install exposes CodeView records for its non-obfuscated binaries:

| binary | PDB reference | note |
|---|---|---|
| `Warcraft III.exe` | `D:\Jenkins\workspace\warcraft3-pipeline\build\bin\x86_64\Warcraft III.pdb` | plain build |
| `World Editor.exe` | `…\x86_64\World Editor.pdb` | plain build |
| `war3_loader.dll` | *(scrambled)* | **Aegis-obfuscated** |
| `worldedit_loader.dll` | *(scrambled)* | **Aegis-obfuscated** |

The obfuscated pair is significant: the CodeView payload is replaced with the Aegis marker
(`f9812e74-424e-3031-751f-…`, i.e. `BN01`), so `war3_loader.dll` — the actual game code —
carries no usable symbol reference in any retail build, 2.0.1 or current. That is the same
obfuscator whose PDB was in the 2.0.1 drop, and the shipped loader still has to be
deobfuscated before its own bytes can be named.

RE-004 reads `wc3/pdbs-2.0.1/Warcraft III.pdb` and settles the lineage question:
2.0.1 compiles this same tree. 240 of these 247 filenames are still path suffixes
there, the two casings are one tree stored twice, and `Jass2` now lives at
`Engine\Source\Scripting\Jass2` beside `Jass3` and `Lua`.