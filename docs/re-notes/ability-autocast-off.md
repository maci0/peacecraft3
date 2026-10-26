# Ordinary autocast OFF orders

The selected Legacy client capability is read from the retained executable, with
its input identity and SHA checked at admission. Named Slow, Inner Fire and Heal
class getters lead to their class-code and on/off-order virtuals. The OFF handler
joins a tested state-bit operand to its complementary clear operand and the
already-clear return branch. The reader derives these operands and IDs; it does
not embed native addresses or order IDs. Synthetic relocated/nonstock fixtures
and malformed/ambiguous/budget refusal tests cover this reader.

Unit ability aliases and map overlays remain owned by the existing ability
resolver. `UnitAbilities.auto` supplies the initial enabled aliases, and the
resolved ability profiles' `Orderon`/`Orderoff` must agree with the admitted class
and registry. A canonical unit stores only changes from that source default.
Missing or malformed authored data is an error, not an invented default.

## Original runtime observations

Two command-only probes used the known-loadable Human02 container and source
Sorceress/Priest/Footman definitions. The isolated original clients were stopped
before collection. Local artifacts and raw records remain ignored under
`.scratch/autocast-off-probe/`; no original data is included here.

Arm A retained 81 records and 18 before/after command pairs. A state-changing idle
OFF returned true and emitted an issued-order callback, with the OFF order visible
inside the callback and zero current order after return. Repeated OFF, authored
initially-disabled Inner Fire and the missing-ability control returned false,
without order/spell events or movement interruption.

Arm B retained 77 records and 16 pairs. The first enabled-to-disabled OFF while
actually moving returned true, emitted the issued-order callback, stopped motion
and left current order zero. Subsequent samples remained stationary. Repeated
OFF did not interrupt a later movement order. No tested OFF emitted spell events.
This is bounded ordinary-unit evidence, not a claim about all autocast abilities
or special native ownership modes.

## Canonical implementation and limits

A changed OFF preflights the frame, activity interruption, event delivery and
state capacity before mutation. It uses the canonical stop/order cleanup and
shared player/unit event contexts. The ephemeral OFF current order lasts through
its callbacks; a callback's explicit replacement order survives settlement.
Already-disabled and missing-ability returns do not reserve event frames or add
state overrides. Removed units return false; unknown and wrong-family handles
retain errors. Whole-unit retirement drops the state.

The existing cache format and Mirror Image snapshot do not encode altered
autocast state, so their preflights explicitly refuse it rather than drop it.
There is no new cache version. Per-ability removal is not implemented by this
slice. ON orders explicitly refuse before mutation: automatic Slow, Inner Fire
and Heal effects, their default-enabled automatic operation, and the later
`ravenform` transformation remain required work. No full autocast or interlude
completion is claimed by these parser and synthetic host tests.
