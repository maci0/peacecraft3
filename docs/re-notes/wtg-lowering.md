# war3map.wtg / war3map.wct lowering to JASS

`pcx_trig::lower::lower(&Wtg, &Wct, &Catalog)` writes the JASS for the editor's GUI
triggers of one map: a `globals` block, `InitGlobals`, `InitCustomTriggers`,
`RunInitializationTriggers`, and per trigger an actions function, a conditions
function when conditions exist, an `InitTrig_` function and one wrapper per code or
boolexpr parameter. Custom-text triggers emit their WCT block verbatim. Names come from
`UI\TriggerData.txt` (`ScriptName` alias when authored), preset code text from
`[TriggerParams]`, and variable types from `[TriggerTypes]`.

`pcx_play::editor_triggers` is the explicit editor-source path:
- `compile_editor_triggers` parses, lowers and compiles WTG/WCT member bytes.
- `compile_editor_map_scripts` reads `war3map.wtg` and `war3map.wct` from an archive through
  `Map::extract_checked` with the caller's `ReadBudget`. It never reads `war3map.j`.
Admission does not call either function.

## Script source at runtime (measured in source)

- `pcx_play::map_admission` calls `map_scripts_import::import_map_scripts`.
- `import_map_scripts` requires the `war3map.j` member and fails with
  `MissingScriptMember` when it is absent. It never reads `war3map.wtg` or
  `war3map.wct`.
- Therefore the executed script for every map admitted today is the compiled
  `war3map.j`, which the editor writes beside the trigger graph. The WTG is editor
  source.
- The explicit path is caller-selected. Admission does not call it, so selecting it
  changes no loaded script.
- `pcx_play::map_scripts_import` keeps its `war3map.j` behavior: a map with compiled
  JASS never falls back to the lowered graph.

## Evidence used

Ground truth is the `war3map.j` the editor wrote for each shipped map, carried in the
same archive. These are observations from the local 1.29b install, not committed.

- Human01: `MapInitializationEvent` has no `TriggerRegister` call. The trigger runs
  from `RunInitializationTriggers` via `ConditionalTriggerExecute`.
- Disabled triggers keep a `trigger gg_trg_X` declaration and emit no functions or
  `InitTrig` call. The 2002 script also omits disabled events and conditions:
  Human04 `ArthasEmptyCrates` (disabled condition and event) matches exactly in
  `InitTrig` and `Conditions`.
- Disabled actions are omitted (Human05Interlude Scene1; Human09 and others).
- An initially-off trigger calls `DisableTrigger` right after `CreateTrigger`.
- Statements keep file order within a kind. The file groups ECAs by kind, not necessarily
  events first.
- `IfThenElse` writes `if ( Trig_X_FuncNNNC() ) then`, branch actions inline. `ReturnAction`
  writes `return`.
- `ForLoopA` writes `bj_forLoopAIndex` / `bj_forLoopAIndexEnd` (declared in `Blizzard.j`)
  around `loop` / `exitwhen` / `endloop`.
- A `boolcall` argument is a direct wrapper call. A `boolexpr` argument is
  `Condition(function F)`.
- `TriggerRegisterVariableEvent` takes its variable as a quoted `udg_` name.
- Numeric literals are written raw; four-character object codes quoted; `boolean`
  literals raw.
- WCT custom text: the block holds the editor's full text for the trigger, its
  actions function and `InitTrig_` function, with one NUL inside the length. Human06
  `Malganis Continuously Attacks Buildings` matches exactly.
- Uninitialized `string` globals are `""`. TriggerTypeDefaults has no `string` entry.
  This comes from one map's output (WarCraftIIICredits `CreditMusic`), which matches exactly.
- Trailing whitespace in a trigger name is dropped from the identifier. Leading
  whitespace becomes `_`.
- An initially-off map-initialization trigger is not in `RunInitializationTriggers`
  (NightElf05 `The Dragons`, the only shipped case).
- Initial values are `[TriggerParams]` preset keys (`UnitNull`, `Player03`) or
  literals of the variable's base type.
- `CommentString` is a no-op stub in Blizzard.j (its comment says so). The lowering writes
  each line of its text as a `//` comment, the same form the editor writes for it in
  Human06. Both forms are excluded from the differential.
- Some 2002 scripts end a function with `endfunction` followed by a comment on the same
  line, and some use bare-CR line endings. The differential accepts both.

## String literals

WTG string values are decoded text. Each character the lexer defines an escape for
is written in that escape, using `pcx_script::Esc` (`\" \\ \b \t \r \n \f`). The
output re-lexes to the input: `lower_wtg` checks a quote literal through
`pcx_script::lex`. The 2002 scripts write backslashes doubled, which matches.
`[TriggerParams]` backquoted text is already JASS-escaped and is emitted raw.

## Differential

`crates/pcx-trig/tests/lower_differential.rs` (ignored; needs `PEACECRAFT3_LEGACY_DIR`,
`PEACECRAFT3_TRIGGER_EDITION` and `PEACECRAFT3_TRIGGER_VERSION`) compares each generated
`Trig_*_Actions`, `Trig_*_Conditions`, `InitTrig_*`, `InitGlobals`, `InitCustomTriggers`
and `RunInitializationTriggers` with the editor's function of the same name. Whitespace
is removed and wrapper numbers are normalized.

Result on the 44 shipped maps (11,201 compared bodies):

- **10,622 exact.**
- **567 drift-only.** Matched only after two named normalizations: a TriggerData
  `ScriptName` alias (for example `ForGroupBJ` for `ForGroup`) written under the plain
  name in the 2002 output, and `.blp` textures in TriggerData versus `.tga` in the
  2002 output. Reported separately and never counted as exact.
- **5 residual, all listed:**
  - 4 `InitCustomTriggers` (Human01, Human02, Human07, Undead02): call order only. The cause
    is unestablished. One trigger sits among another category's triggers in the editor
    order (`Destroy TalkThornby`, Human01), which suggests a WTG re-save after compilation.
    Unverified.
  - 1 `InitGlobals` (Human06): an editor loop default-initializes an integer array (`i>1`
    bound). Declared arrays have no size in the v4 WTG, and most maps with uninitialized
    arrays (Human01, Human05, Orc02 and others) match exactly with no loop, so the loop has
    no WTG source.
- Excluded by design: `CommentString` statements, in both forms. Seven Human06 bodies
  differ only in that form (the editor writes `//text`). Blizzard.j defines `CommentString`
  as a no-op stub, so the form is not a behavior difference.

The 2002 script predates the 1.29b TriggerData, so drift is not evidence of a lowering
error. Drift is also not evidence of 1.29 editor behavior.

## Census

`lower_original.rs` (ignored): all 44 `war3map.wtg` in the Legacy install lower, 0 refused.

## Explicit VM comparison

`crates/pcx-play/tests/editor_trigger_vm.rs` runs the lowered output and a hand-written
script with the same behavior in the VM under one recording host. The recording host
logs every native call with code arguments named by their function. The test asserts
the same native trace, the same final `udg_Count` (6), and that the disabled trigger
is absent. It covers globals, actions, events, init order and disabled triggers. It
does not cover timers firing, condition evaluation, or the native game.

## Refused (named, whole map)

- Editor-expanded forms with no source in the shipped corpus: `AndMultiple`,
  `OrMultiple`, `IfThenElseMultiple`, `ForLoopAMultiple`, `ForLoopBMultiple`,
  `ForLoopVarMultiple`, `ForLoopVar`, `ForForceMultiple`, `ForGroupMultiple`.
  Probe counts over the 44 shipped maps are 0 for each.
- `CustomScriptCode`: 0 occurrences in the shipped maps; no evidence for its emission.
- Run-on-initialization triggers: 0 occurrences in the shipped maps.
- Initially-off custom-text triggers (no shipped case; the editor text would decide).
- `MapInitializationEvent` combined with other events.
- Trigger names that collide after sanitizing.
- An initialized array variable (no shipped case).
- A declared name that is not a JASS identifier.
- Nesting deeper than 64 (`NESTING_LIMIT`), which bounds the recursion.

## Not verified

- Athena and community GUI maps: no Athena or community `.w3x` is present in the local
  assets (`find` finds only a system icon). Not verified.
- Native runtime equivalence beyond the recording-host test.
- Init order (see residual).

## Provenance

- 2026-10-20, worker piece 19: lowering module, catalog sections, synthetic and
  differential gates. Requested by the wave 20 coordinator.
- Follow-up, same worker: WCT custom text, JASS string escaping via `pcx_script::Esc`,
  nesting bound, `pcx_play::editor_triggers`, and the VM comparison test. Evidence:
  the Legacy install's `war3map.j` and `war3map.wct` for the cited maps (local only).

## Editor source and runtime differential (added)

- `compile_editor_map_scripts` (archive) and `compile_editor_triggers` (bytes) are tested
  in `crates/pcx-play/tests/editor_trigger_map.rs`. A map whose `war3map.j` is invalid JASS
  compiles through the editor path, which proves it is not read. A map without
  `war3map.wtg` is refused by member name.
- `crates/pcx-play/src/editor_trigger_runtime_tests.rs` runs the lowered graph and a
  hand-written script on the existing object arena, trigger host and timer host, with the
  same clock and firing budget. The graph has an initialized global, an enabled timer
  trigger, a trigger refused by a false condition, and a trigger accepted by a true
  condition. Both runs must deliver the same callbacks, condition results and final globals,
  and the final count must be 16.
- Evidence scope: a synthetic TriggerData, a synthetic Blizzard.j stub for
  `TriggerRegisterTimerEventSingle`, and a hand-written compiled script. This is the trigger
  host contract only, not the native game and not a shipped map.
- Object globals (`gg_unit_*`, `gg_rct_*`, and similar) are not bound by this path. The
  existing placement helpers (`unit_placement`, `pcx_map::Units`) carry no script name, and
  the editor's naming rule (`gg_unit_<type>_<nnnn>`) and creation order are not derived from
  the file. A graph that names an unbound global is refused by name at compile time
  (`an_object_global_the_path_does_not_bind_is_refused_by_name`). Shipped trigger graphs
  reference such globals, so the explicit path cannot compile a stock map yet.
