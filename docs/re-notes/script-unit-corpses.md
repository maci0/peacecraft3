# Canonical corpse creation

`CreateCorpse(player, integer, real, real, real) -> unit` now uses the same selected
catalog, source identity, preflight, handle allocator and accepted placement as
`CreateUnit`. Optional declarations must have the nonconstant ABI and creation
source; foreign or malformed declarations refuse before admission. Null players
return null, while malformed arguments, nonfinite coordinates, wrong families,
budgets, missing liveness/vitals and model failures refuse before publication.

The factory publishes a canonical unit with depleted life and dead combat state.
Maximum health, mana, armor, abilities, model, color, geometry and requested
coordinates remain source-derived. Depleted life is zero, not a copied unit stat;
no stock rawcode, health amount, facing, position or duration was added to code.
The authored Death clip is selected from the loaded model. If a supplied model
lacks that clip, creation refuses rather than substituting a standing actor.

The implementation is a recreation, not the native object/event layout. A fresh
corpse is not a synthetic combat kill: it produces no new kill notification,
reward, inventory drop or guard route. It remains queryable/enumerable through the
same unit handle and is excluded by existing dead-unit food and combat predicates.
Its prepared collision stamp is inactive before publication. Show/hide of a dead
unit cannot republish occupancy; ordinary removal retires the same unit.

## Evidence

Local static evidence uses selected client SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The functional registration at `49d25a` targets `48dc60`. The handler resolves the
player, calls `663ca0`, then `69afe0` with two clear arguments before subsequent
state/action dispatch. `663ca0` forwards through the same `687ca0` unit factory;
its flags differ from ordinary creation. Internal dispatch tokens have not been
renamed as public orders. Those exact PE-mapped, hashed windows remain local under
`.scratch/create-corpse-20261025/`; no original assets are committed.

Synthetic public-native tests use nonstock stats and source model clips. They
verify dead life/maximum, owner/facing, Death animation, no death event (with a real
live-unit kill as positive control), no food use, refused movement, overlapping
live-unit placement, show/hide occupancy, removal and refused-call atomicity.
Training, cache restoration and summoning continue through the same preparation
path with distinct birth intent. Core 1,819 tests and the full application and
integration suites pass; strict native/WASM checks and application builds pass.

The unchanged TFT chapter now constructs its first `enec` corpse with canonical
life zero and its actual source Death clip. Its retained post-control horizon
then fails at `SetUnitBlendTime` (Blizzard.j:3193); it still returns player control
before that failure. RoC passes the full entry/control/horizon diagnostic. These
are recreation runs, not original-client runtime observations. The Docker
reference socket refused read-only access this turn, so no client was launched.
No new corpse GPU/reference comparison was run.

## Earlier fidelity boundary

Automatic corpse decay/resurrection is still absent from the existing dead-unit
lifecycle. `CreatePermanentCorpseLocBJ` next needs real animation blend/playback
controls and decay suspension; successful corpse construction does not establish
that complete helper or a full mission playthrough. Fresh-hero corpse behavior,
rare models without Death and exact native placement/transition timing still need
broader qualification. The global game/scripting objective remains open.

## Decay lifecycle and suspension (2026-10-25)

The earlier missing-decay boundary above is superseded. `unit_decay::Rules` reads
source `UnitData/death` and `deathType` through canonical definitions, typed
MiscData durations (including map overrides), same-image hero classification and
checked constructor mask operands. Selected-image evidence uses the digest above:
`66f1bc` supplies the two unsigned TEST operands, `69f9d0` reads flesh DecayTime,
`69fb50` selects structure/bone duration, and the registered `UnitSuspendDecay`
handler `4af8e0` qualifies dead flesh/bone state before action retirement. The
complete cache-wrapper field lineage and internal action tokens are not claimed.

Creation and ordinary death share a per-unit lifecycle on the existing logical
clock. Authored Death/Flesh/Bone/Dissipate clips fit the source gameplay durations;
explicit playback scale remains separate. A large step drains phase boundaries
using their original timestamps. Raise-only remains persist; ordinary completed
corpses use canonical removal and its cleanup; hero dissipation hides the same
canonical hero. Work exhaustion is checked before phase mutation.

The optional source-owned native validates its full ABI. Living, removed and
ineligible-phase handles do not acquire a decay timer. Eligible suspension saves
remaining phase time, leaves the animation running and is idempotent. Resumption
uses that remaining lifetime at the current clock: this is an explicit recreation
choice, not a measured native timer restart contract. The unchanged TFT helper
also calls SetUnitTimeScale, so script-driven animation freezing stays independent.

Synthetic tests cover nonstock masks/durations, actual parsed model sequences,
normal kill versus corpse creation, structure and hero phases, split/large steps,
suspension independence, removal and refusal atomicity. Core 1,837 and app 675
tests pass; integration suites, explicit selected-image/profile gate, strict native
and WASM checks, both application builds and graphics check pass. Both unchanged
opening chapters pass 300 seconds of idle script execution after player control.
TFT retains actual source corpses in suspended Bone/Decay Bone throughout that
horizon. This establishes the permanent-corpse helper execution path only.

The isolated offscreen RoC run reaches gameplay from the post-intro game main
menu. Its overall renderer gate remains red for sky illumination and sight
occlusion; parser and script checks do not establish visual parity. No new
original-client corpse comparison, resurrection, full objective playthrough or
exceptional native death/effect behavior is qualified here.
