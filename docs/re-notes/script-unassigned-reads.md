# Unassigned global and local reads

Status: current behavior recorded from source. The original's per-type default for an
unassigned read is not established by this repo or its notes. Semantics are unchanged.

## Current behavior (source)

- `crates/pcx-script/src/eval.rs` global admission: a scalar declaration without an
  initializer holds `Null` of its declared kind, and an array declaration holds an
  empty element map. Local declarations follow the same rule (`bind_decl`).
- A read of an unset array element yields `Null` (`eval.rs`, indexed read).
- Consequences: arithmetic and ordered comparison on `Null` raise an operator error
  (`value.rs`, `pair`), and a condition on `Null` raises a logic error (`eval.rs`,
  `truth`). No zero, `false` or `""` is substituted. `Null == Null` is true, and
  `Null` never equals a handle or a string.
- Handle declarations with a `= null` initializer are the form the shipped maps use
  (see below).

## Shipped campaign maps (measured)

Input: the 97 campaign maps of the local 1.29 install, 44 from `War3Local.mpq`
(`Maps\Campaign\`) and 53 from `War3xLocal.mpq` (`Maps\FrozenThrone\Campaign\`).
Each map's `war3map.j` was read from the map MPQ after its 512-byte header. Counts
come from a static scan of the declared `globals` block, the `InitGlobals` function
and every other function body. Extracted bytes stay under the ignored `.scratch/`.

Scalar globals (`boolean`, `string`, `integer`, `real`, handle types) declared without
an initializer: 972 across the maps (boolean 502, string 217, integer 148, real 105).

- 954 are assigned in `InitGlobals`. The World Editor writes the explicit default
  there, zero, `false` or `""`, even when the declaration has none.
- 18 are not: 17 `string` sound-path globals (`gg_snd_*`) assigned in `InitSounds`,
  and 1 `integer` assigned in the trigger action that reads it, before that read in
  the same body.
- No handle-typed scalar is declared without an initializer. Every handle declaration
  carries `= null`.

Handle globals declared `= null` that no function ever assigns: 1,049 across 82 maps
(unit 300, trigger 464, location 93, effect 53, questitem 30, fogmodifier 32, rect 18,
and smaller counts for the rest). For these the value is the author's explicit `null`,
so the native default does not affect them. The null player passed by UndeadX07c
(`udg_AP3_Nerubians`) is one such declaration.

Arrays have no initializer in the language. Arrays with no element write in any
function: 17 (11 integer, 3 rect, 1 unit, 1 player, 1 playercolor). Their textual
reads in function bodies are 0 except one unit array, `udg_DemonsArray` in
NightElf06. Its only reader, `Trig_Tichondrius_Summons_Demons_Actions`, is run nine
times per firing by `TriggerExecute` in a loop inside `Trig_Tichondrius_For_Int_A_Actions`,
which is registered for a player-unit-attacked event. Each read is an unset element
and yields `Null` in the VM, so the native call receives a null unit. The native's
handling of that call is unverified (see below).

## Read-before-write (static, function level)

- The 18 scalars above: no read in `InitGlobals`, `InitSounds`, `config`, `main`, or
  any `RunInitializationTriggers`/`InitCustomTriggers` path by name. The integer is
  written before its only read in the same function.
- Arrays with element writes: not analyzed per element, because the element indices
  are runtime values.

Limits: name-level matching, not a call graph, and no evaluation of loop bounds. This
is a count and a screen, not proof that no startup path reads an unset value.

## Native behavior (not established)

- No note, `PLAN.md`, `TRACKER.md` or `pcx-script` test records the original JASS
  VM's initial storage for an unassigned global or array element, per type.
- The editor writing explicit zero, `false` and `""` is consistent with a zeroed
  native default and with an editor choice. It does not decide between them.
- Warsmash Jass2 is third-party reconstruction, not native evidence, and was not used.

Evidence that would settle the per-type rule:

1. Disassembly of the reviewed 1.29.2 `game.dll` JASS VM: the allocation and reset of
   global storage (symbols under `engine/jass2`, see `001-classic-symbols.md`), showing
   the initial value per type, and the array element default.
2. Runtime observation under the reference harness (`original-game-vm.md`): a script
   with unassigned `integer`, `real`, `boolean`, `string` and handle globals and one
   unset array element per type, each read through a native that reports its value or
   refuses, with the result recorded per type.

Until one of these exists, do not change `Null` to a typed default, and do not treat
a refusal on an unassigned read as proof of native behavior.
