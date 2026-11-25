# RE-001: Source strings in the classic compatibility executable

This note records a string survey of the local `wc3/game/app/game.dll`.
Later evidence identifies that executable as `1.26.0.6401`. It was labelled
“1.28c” in the earlier experiment.

## What was recovered

`game.dll` embeds its compile-time breadcrumbs — assertion macros, `calldump.log`,
`WEUTiming.txt`, `TerrainMemory.txt` and similar carry absolute source paths from Blizzard's
build machine. The filter described below yielded:

- **254 source paths**, all rooted at `e:\Drive1\temp\buildwar3x\`
- **36 `C`-prefixed type-name matches**

Local artifacts: `rea/game-source-paths.txt` and `rea/game-class-names.txt`.

## Subsystem names in the sample

The grouped rows below account for 247 entries. The full path scan found 254.

| subsystem | paths | what it is |
|---|---:|---|
| `war3/Glue` + `war3/glue` | 101 | UI glue layer for dialogs, lists and frames. Both casings were observed. |
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

## Use of the sample

Source paths suggest names for further investigation. For example, an assertion
reference to `CAbilityCriticalStrike.cpp` supplies a label to check against
other evidence. The subsystem groupings informed the
[crate layout](../../PLAN.md#31-crate-layout). They do not establish dependency
directions within the original engine.

## Method notes

- The `[A-Za-z]:\` prefix and `.cpp/.c/.h/.hpp/.idl` suffix are the filter. Two casings of
  the same directory (`Glue`/`glue`) survive; merge them when building the map.
- Type names were matched as `C[A-Z][A-Za-z0-9]{3,40}` followed by a space, `:` or `*`.
  That is conservative on purpose — it avoids inventing names that only look like classes.
- A matched string identifies a name present in the binary. It does not
  establish the type’s implementation or the enclosing function’s role.

## Reforged comparison

The retail-install CodeView sample is separate from the 2.0.1 PDB scan:

| binary | PDB reference | note |
|---|---|---|
| `Warcraft III.exe` | `D:\Jenkins\workspace\warcraft3-pipeline\build\bin\x86_64\Warcraft III.pdb` | plain build |
| `World Editor.exe` | `…\x86_64\World Editor.pdb` | plain build |
| `war3_loader.dll` | *(scrambled)* | **Aegis-obfuscated** |
| `worldedit_loader.dll` | *(scrambled)* | **Aegis-obfuscated** |

The two examined loaders used the `BN01` marker in their CodeView payload and
carried no usable symbol reference.

The [lineage comparison](004-reforged-lineage.md) found 247 unique basenames
among the 254 classic paths. The inspected Reforged 2.0.1 PDB matched 240 of
those basenames and placed `Jass2` under
`Engine\Source\Scripting\Jass2` beside `Jass3` and `Lua`.
