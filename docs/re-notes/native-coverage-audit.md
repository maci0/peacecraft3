# The native coverage audit against the host's dispatch

`pcx_play::native_coverage_host` reports, for an admitted program, which of its
declared natives the host dispatches and which remain explicit refusals. This
note records a check of that report against the host's own dispatch, what it
found, and what it does not settle.

## The check

The report's supported set is a list in `crates/pcx-play/src/native_coverage_host.rs`.
The host's dispatch is not described in one place: `ObjectHost::native` hands a
name to each host module in turn, and a module claims a name by matching it.
The names are matched in several shapes:

| Shape | Example |
| --- | --- |
| match arm | `"GetUnitX" => …` |
| or-pattern arm | `"GetKillingUnit" \| "GetAttacker" => …` |
| tuple arm | `("PlayThematicMusic", [Value::Str(path)]) => …` |
| comparison guard | `if name == "GetLocalPlayer" {` |
| `matches!` list | `matches!(name, "PlayMusic" \| …)` |
| contract table | `("Preloader", &["string"])` |
| `const` table | `const REGISTER: &str = "TriggerRegisterTimerExpireEvent";` |
| `RealOps` trait | `Sin`, `Cos`, `Atan2`, `SquareRoot` |

The check reads the crate's own source: it takes `ObjectHost`'s dispatch chain
from `object_host.rs`, reads each host module's dispatch function, and collects
the names those functions claim.

## What it found

The list held **73** names before this work. The dispatch claims more, and two
defects in the check were found by measuring it against the source:

- The check read only the modules `ObjectHost::native` names directly. It did
  not follow the chain further. Nineteen modules are reached *through* another
  module — `unit_native` passes the name on to `unit_visibility_native`,
  `unit_look_at_native`, `unit_pause_native`, `unit_revive_native` and
  `unit_sleep_native`, and `player_native` passes it to the two handicap
  modules — so their names were never read.
- The check matched a literal by scanning forward from the opening quote for
  the first character that is not alphanumeric. That stopped at the closing
  quote, so `after` still began with a quote and the plain-arm and tuple-arm
  shapes never matched. Only the `matches!` list and comparison shapes were
  read, which is why the check saw 152 names while the dispatch claims 233.

The check now reads string literals as literals, follows the chain to its end,
and recognises the plain arm, the or-pattern arm, the tuple arm, the
comparison and the `matches!` list. A literal is a claim only when an arm
follows it, so a signature table row — `("Preloader", &["string"])`, which no
`=>` follows — is not counted as a dispatch.

That measurement found **14** natives the host dispatches and the list did not
name, so the audit reported them as unsupported:

`GetKillingUnit`, `IsUnitHidden`, `IsUnitPaused`, `PauseUnit`,
`PlayThematicMusic`, `ResetUnitLookAt`, `ReviveHero`, `ReviveHeroLoc`,
`SetUnitLookAt`, `ShowUnit`, `UnitAddSleep`, `UnitCanSleep`, `UnitIsSleeping`
and `UnitWakeUp`.

Each one dispatches a real behaviour — `ShowUnit` and `IsUnitHidden` through
`unit_visibility_host`, `ReviveHero` and `ReviveHeroLoc` through
`unit_revive_host`, the sleep natives through `unit_sleep_host`, and so on —
so the list now names all 257.

## The gate

`the_audit_lists_every_native_the_host_dispatches` reads the crate's source
and fails when a dispatch name is absent from the list, so the list cannot
silently fall behind again. The check reads the shapes it can see: a name that
only appears in a module's contract table, in a helper's name list
(`is_music_name`) or through the `RealOps` trait is not seen by it, so the
list can still fall behind those shapes. The check also asserts that it read
more than 150 names, so it cannot become vacuous if the source layout changes.

The measurement behind the gate is the crate's own source, not an original:
the 73, 233 and 257 counts are measured values, not thresholds.

## What this is not

- The list is not the game's native list. It is the set this build dispatches;
  the game's 1,383 declarations come from the selected scripts through
  `crates/pcx-play/tests/native_inventory.rs`.
- A name in the list means a host implementation exists. It does not mean
  every case is implemented: an admitted native may still refuse
  contextually, and the plan requires those refusals to stay visible.
- The conversion natives (`Player`, `ConvertRace`, `ConvertPlayerColor`,
  `ConvertGameSpeed`) are not in the list. The host dispatches whichever
  conversion natives the caller admits from the selected program
  (`campaign_inputs::enum_contracts` takes the `Convert*` names the common
  source's constant declarations use), so the set is a property of the
  admitted program rather than of this build. `Player` is not among them,
  which means a lowered trigger's `Player(0)` is refused on the campaign
  path; that is a separate finding and not settled here.
- The check reads source text, so a reformatting that moves a name out of a
  dispatch function could hide it. The check fails only on names it can see.
- No original was observed for this slice, and no campaign script was run.

## Provenance

- 2026-11-02: the check above, the extended list and the gate. The
  measurement is against this crate's own source; the 73, 233 and 257 counts
  are measured values, not thresholds.
