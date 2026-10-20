# war3map.wtg version 4 lowering to JASS

`pcx_trig::lower::lower(&Wtg, &Catalog)` writes the JASS for the GUI triggers of
one map: a `globals` block, `InitGlobals`, `InitCustomTriggers`,
`RunInitializationTriggers`, and per trigger an actions function, a conditions
function when conditions exist, an `InitTrig_` function and one wrapper per code
or boolexpr parameter. Names come from `UI\TriggerData.txt`, preset code text from
`[TriggerParams]`, and variable types from `[TriggerTypes]` (the `Kind::Param`,
`Kind::Type` and `Kind::TypeDefault` sections of `pcx_trig::catalog::Catalog`).

Not wired into the map loader. The output is a fragment: it references `gg_`
object globals declared by the map's own script and `Blizzard.j` helpers.

## Evidence used

Ground truth is the `war3map.j` the World Editor wrote for each shipped map, which
the same map archive carries. These are local-only observations from the 1.29b
install (no assets are committed):

- Human01: `MapInitializationEvent` has no `TriggerRegister` call. The trigger is
  run from `RunInitializationTriggers` through `ConditionalTriggerExecute`.
- Disabled triggers keep a `trigger gg_trg_X` declaration and emit no functions or
  `InitTrig` call.
- Initially-off triggers call `DisableTrigger` right after `CreateTrigger`.
- Statement order inside a trigger follows the file. The file stores ECAs grouped
  by kind, not necessarily events first.
- `IfThenElse` writes `if ( Trig_X_FuncNNNC() ) then` with the branch actions inline,
  and `ReturnAction` writes `return`.
- `ForLoopA` writes `bj_forLoopAIndex` and `bj_forLoopAIndexEnd` (declared in
  `Blizzard.j`) around a `loop` with `exitwhen`, then the body.
- The string parameters of a `boolcall` argument are direct wrapper calls. A
  `boolexpr` argument is `Condition(function F)`.
- `TriggerRegisterVariableEvent` takes its variable as a quoted `udg_` name
  (`VarAsString_*`).
- Numeric literals are stored as string-kind values and written raw. Four-character
  object codes are written quoted. `boolean` literals are written raw. Other strings
  are written in double quotes with backslashes escaped.
- An uninitialized `string` global is set to `""`. TriggerTypeDefaults has no
  `string` entry, so this comes only from the editor output.
- Trailing whitespace in a trigger name is dropped from its identifier; leading
  whitespace becomes `_`.
- An initially-off `MapInitializationEvent` trigger is not in
  `RunInitializationTriggers`. One shipped case (NightElf05 The Dragons).
- Initial values are `[TriggerParams]` preset keys (`UnitNull`, `Player03`) or
  literals of the variable's base type.

## Differential

`crates/pcx-trig/tests/lower_differential.rs` (ignored; needs `PEACECRAFT3_LEGACY_DIR`
and `PEACECRAFT3_TRIGGER_EDITION` / `PEACECRAFT3_TRIGGER_VERSION`) compares every
generated `Trig_*_Actions`, `Trig_*_Conditions`, `InitTrig_*`, `InitGlobals`,
`InitCustomTriggers` and `RunInitializationTriggers` with the editor's functions of
the same name. Whitespace is removed and wrapper numbers are normalized.

Result on the 40 shipped maps that lower: 9,671 of 10,187 generated functions match
exactly. 513 more match after two drift normalizations, which are derived from the
file and never counted as exact:

- a TriggerData `ScriptName` alias (for example `ForGroupBJ`, `SetHeroLevelBJ`) that
  the 2002 editor output writes under the plain name;
- `.blp` textures in TriggerData where the 2002 output has `.tga`.

3 `InitCustomTriggers` functions differ only in the order of their `InitTrig_` calls.
The cause is not established. One trigger (`Destroy TalkThornby`, Human01) sits
among triggers of another category in the editor order, which suggests the WTG was
re-saved after the script was compiled. That is unverified.

The 2002 script predates the 1.29b TriggerData, so the drift classes are not
evidence of a lowering error. They are also not proof of 1.29 editor behavior.

## Census

`lower_original.rs` (ignored) lowers every `war3map.wtg` in the Legacy install:
40 lower, 4 are refused. The 4 are custom-text triggers (WCT JASS), which are
refused, not lowered.

## Refused

Each refusal fails the whole map with `LowerError::Unsupported`, `Malformed`,
`UnknownDefinition`, or `UnknownPreset`. Nothing is skipped silently.

- Editor-expanded forms not lowered: `AndMultiple`, `OrMultiple`,
  `IfThenElseMultiple`, `ForLoopAMultiple`, `ForLoopBMultiple`,
  `ForLoopVarMultiple`, `ForLoopVar`, `ForForceMultiple`, `ForGroupMultiple`,
  `CustomScriptCode`. `ForLoopA`, `ForLoopB`, `IfThenElse`, `ReturnAction` and
  `SetVariable` are lowered.
- Custom-text triggers and run-on-initialization triggers.
- `MapInitializationEvent` combined with other events.
- Trigger names that collide after sanitizing.
- A string literal containing `"` or a line break.
- An initialized array variable.

## Decisions to check

- Disabled ECAs are omitted. Measured on disabled actions in an enabled trigger
  (Human05Interlude Scene1). Conditions and events are assumed the same.
- A bare string literal in a numeric-typed parameter must be a decimal or a
  four-character code. Anything else is refused.
- The BJ `ScriptName` aliases are called, not the TriggerData key. The shipped
  2002 script differs in those names (see drift). Whether the 1.29 editor calls the
  alias is unmeasured.
- `CommentString` is a real call (`call CommentString("...")`), as the editor writes it.
- Generated identifiers follow the editor's pattern (`Trig_<id>_Actions`,
  `Trig_<id>_FuncNNNC/A`). `Func` numbering differs from the editor, and nothing
  depends on it.

## Verification

- `cargo test -p pcx-trig --locked --offline --test lower_wtg`: 14 synthetic tests.
  Exact JASS for a representative trigger, plus refusals and edge cases.
- `cargo test -p pcx-play --locked --offline --test lowered_trigger_compile`: the
  golden output parses and resolves through `pcx_script` against a synthetic
  prelude. A missing native is refused, so the check is not vacuous.
- Ignored gates above. Each runs against the read-only Legacy install with
  explicit environment variables and writes nothing into the install.

Not verified: that the lowered JASS executes with the same behavior. The
differential compares generated text, not runtime effects. The native game was not
run.

## Provenance

- 2026-10-20, worker piece 19: lowering module, catalog sections, synthetic and
  differential gates, this note. Requested by the wave 20 coordinator. Evidence:
  `crates/pcx-trig/tests/lower_differential.rs` output, Human01 and NightElf05
  `war3map.j` from the Legacy install (local scratch only).
