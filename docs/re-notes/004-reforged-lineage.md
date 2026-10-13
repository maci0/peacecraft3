# RE-004 — Reforged 2.0.1 is the 1.28 tree continued

Status: **read**. String scan of `wc3/pdbs-2.0.1/Warcraft III.pdb` (285,396,992 bytes).
Its SHA-512 matches the Battle.net hash in RE-002. `World Editor.pdb` matches too,
and was not scanned. The PDBs stay under ignored `wc3/`; this note records paths
and counts only.

## Verdict

The 2.0.1 game is the classic codebase, built on from 1.28. The same translation
units are still compiled. The build root moved, a few directories were nested
one level deeper, and new subsystems were added beside the old ones.

Of the 247 unique filenames in `rea/game-source-paths.txt` (254 paths), 240 are
still path suffixes in this PDB. The root changed from
`e:\Drive1\temp\buildwar3x\` to `D:\Jenkins\workspace\warcraft3-pipeline\`.

| 1.28 path inside `buildwar3x` | 2.0.1 path inside `warcraft3-pipeline` |
|---|---|
| `War3\Source\Unit\CAbilityCriticalStrike.cpp` | `War3\Source\Unit\Ability\CAbilityCriticalStrike.cpp` |
| `War3\Source\Unit\CUnit.cpp` | `War3\Source\Unit\CUnit.cpp` |
| `Engine\Source\Agile\CAgent.h` | `Engine\Source\Agile\CAgent.h` |
| `Engine\Source\Base\CDataStore.h` | `Engine\Source\Base\CDataStore.h` |
| `Engine\Source\Jass2\Compile.h` | `Engine\Source\Scripting\Jass2\Compile.h` |

`Jass2` did not move out of the engine. It now sits under `Scripting`, next to
`Jass3` and `Lua`.

## What the PDB actually contains

An ASCII scan for Windows paths ending in `.cpp`, `.c`, `.h`, `.hpp`, `.inl`,
or `.hlsl` yields **9,148** paths and **6,160** unique filenames. After
case-folding, **5,982** of those sit under `warcraft3-pipeline`. The raw count
is higher because the same path is often stored twice (`War3\Source` and
`war3\source`, `Engine\3rdParty` and `engine\3rdparty`). That is the same
double casing RE-001 already saw for `Glue` / `glue`, and it is one tree.

The 1.28 harvest is 254 assertion strings, so it is not a census of that
compile. The jump from 254 to ~6,000 is the rest of the compile becoming
visible, plus the additions below.

`War3\Source`, case-folded, files per directory:

| directory | files | what it is |
|---|---:|---|
| `Unit` | 571 | `CAbility*`, `CUnit*` — the 1.28 rules tree, with `Ability\` inserted |
| `UI` | 253 | UI framework |
| `WorldEdit` | 199 | editor |
| `Game` | 171 | match / session |
| `Glue` | 166 | glue screens, including the old Battle.net panels |
| `WebUI-Backend` | 121 | new menu backend |
| `Sound` | 40 | |
| `AI` | 28 | |
| `World` | 25 | |
| `Net` | 22 | |

`Engine\Source`, same basis:

| directory | files | what it is |
|---|---:|---|
| `Frame` | 107 | frame graph |
| `Tempest` | 87 | platform layer |
| `ClientSdkController` | 75 | new Battle.net client SDK |
| `FDFile` | 75 | file IO |
| `Services` | 72 | new |
| `Scripting` | 68 | `Jass2`, `Jass3`, `Lua`, and the bridge directories |
| `Base` | 58 | `CDataStore` and neighbours |
| `Agile` | 52 | `CAgent` |
| `MdlFile` | 40 | |
| `Ipse` | 39 | threading |
| `Gx` | 36 | graphics |
| `Net` | 32 | |
| `Anim` | 12 | |

`Engine\3rdParty` is 2,464 files. The large ones are PopcornFX (921), FMOD
(247), FreeType (220), Scintilla (202), Prism (159), FaceFX (144), NVTT (139),
Storm (115), libvpx (100), Aegis (61). Storm is still the archive library; it
lives here rather than as a top-level `Storm\` tree. Aegis is the obfuscator
RE-001 already identified on `war3_loader.dll`.

## Names from the 1.28 harvest that this scan did not find

These six filenames have **zero** hits anywhere in the PDB:

- `CRegion.cpp`
- `CReplayPanel.cpp`, `CReplayPanel.h`
- `CWorldObjectsClippable.cpp`
- `gxvideo.h`
- `JassWar3.cpp`

`SAPIExtend.h` occurs twice as a byte string and is not the suffix of a path
this regex captured. Absence from a string scan means the filename is not in
the PDB's path table. It does not by itself say the behaviour was deleted.

## Method

Same class of evidence as RE-001: a path proves the filename was compiled into
this PDB, not what a given function does. Paths were taken with an ASCII regex
over the raw PDB. UTF-16 produced no additional paths. `llvm-pdbutil` is
available and was not required for the lineage question.

## Why this matters

`pcx-sim` can keep the RE-001 layering. `engine/{base,agile,jass2,frame}` and
`war3/{unit,game,world}` are still the fidelity surface. Reforged adds
`Scripting\{Jass3,Lua}`, `WebUI-Backend`, PopcornFX, FaceFX, and the client
SDK around that surface. Retail `war3_loader.dll` remains Aegis-obfuscated;
this PDB is the unobfuscated compile of the same tree, which is why the names
are readable here and not in the shipped loader.
