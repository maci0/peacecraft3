# Unit morphing: evidence and implementation boundary

The Human02 interlude publishes successfully, then reaches Medivh's authored
Raven Form order. Human03 restoration remains unverified. The current work
implements that connected form transition; recognizing an order or changing a
rawcode alone does not establish correct morph behavior.

## Evidence

Shipped asset data associates the unit's owned ability with the normal and
alternate unit definitions. Ranked ability fields supply cast timing, spell
duration, altitude adjustment and landing delay. Source editor metadata supplies
the morph flag meanings. These records must remain runtime inputs, with the
selected install and edition retained; measured probe values are not defaults.

Bounded native-code inspection connects the named Raven Form class to its order
identities and ranked field consumers. Cast timing, spell completion and altitude
work use separate clocks. In particular, altitude work can remain pending after
the spell's finish callbacks. Admission must preserve those relationships rather
than collapse them into one duration.

Two isolated original-client probes exercise the selected ordinary form:

- The first accepts a normal-to-alternate order and the return order on the same
  handle. Repeated orders refuse. Begin callbacks observe the old definition;
  the accepted forward order returns with the alternate definition. During the
  return transition, grounded state precedes the final normal definition.
- The second issues the interlude's authored flight-height request after an
  actual spell-finish event. Its public height changes immediately, then later
  pending morph work replaces that target. Identity, ownership and the control
  unit remain stable throughout both probes.

The native getter reads the requested height target. A separate scalar and
trajectory supply visual interpolation. Getter samples therefore do not measure
instantaneous rendered altitude. The renderer must consume canonical visual
height rather than interpolate the getter's observations or infer flight from
the unit's rawcode.

Raw maps, scripts, native inspection output, original captures and stopped-client
receipts remain local under `.scratch/morph-form-probe/` and
`.scratch/unit-form-order-20261026/`. They are not distributed assets.

## Connected implementation

The core transition retains the same unit identity. It validates both
definitions and their dependent model, movement, combat, ability and placement
data before mutation. Event settlement must preserve callback-visible form and
order state, including reentrant orders. Morphing must not create a second unit
or reenroll food, guards or creation events.

The renderer contract now carries a definition revision and a canonical visual
flight-height override. A revision change refreshes the actor's source bindings;
the visual override is sampled per frame, with the current definition's authored
height as its fallback. Failed model bindings are keyed by revision so a later
form can be evaluated independently. Engine-selected animations also need the
current form's authored animation properties; explicit script animation selection
must retain its own semantics.

The focused campaign actor gate passes ten tests headlessly with null audio.
This checks the renderer field contract, source-height fallback and nonfinite
transform refusal. It does not prove a successful Raven Form cast, a rendered
original comparison or interlude completion. The source-property engine animation selector also passes its focused synthetic
regression, the fifteen model regressions and four rarity regressions. Explicit
script selection keeps its separate policy. The first two canonical lifecycle tests also pass: public string/numeric form
orders preserve identity, begin callbacks see the old form, repeated orders
refuse, completion events occur once, and removal from a callback retires pending
work. Synthetic cast durations on either side of the finish duration exercise
independent clocks. The expanded five-test morph gate, 2,036 core tests, selected-client rule
binding, strict native/WASM checks and both production builds pass. Eighty core
tests remain explicitly ignored.

The next short selected-client interlude run clears the authored Raven Form
order and stops at unsupported `SetUnitFlyHeight`: completed tick 1110, partial
host clock 17.34899997764826, six committed ticks in the failing batch. All pinned
source hashes and the original earned cache remain unchanged. This qualifies
progress through the prior order failure, not interlude completion or original
rendered morph parity.

The subsequent diagnostic identifies the height target as the same living
Medivh handle, still in the normal grounded definition with definition revision
zero. Its owned form ability is present and available, but its current order is
attack. Therefore passing the earlier unsupported-order boundary does not prove
that the actual cast succeeded. The missing transition needs cast-admission
analysis; widening grounded height support would not establish that behavior.
The diagnostic preserves the original earned cache and makes no state changes.

## Unreal height overrides and the next frontier (2026-10-27)

Object-data overrides carry `moveHeight` as an unreal value (the interlude's
`nmdm` record stores `umvh` as type 2). `morph_height` and the actor `scalar`
reader accepted only real overrides for a real-kind field, so the Raven Form
order failed with "unsupported source movement height value" once the
earlier silent refusal was cleared. Both now accept either type for real and
unreal fields. With that, the Medivh Raven Form order and `SetUnitFlyHeight` pass
and the interlude reaches tick 4350.

`AddSpecialEffectLoc` (used by the mage and Jaina teleport triggers) is
implemented as a point effect that copies the location at creation and ends at
`DestroyEffect`; see `effect_host.rs`. Rendering of point effects is not wired.

The interlude then stops at `IssueTargetOrder` with the Sorceress `slow` and
`polymorph` orders (`war3map.j` lines 1584 and 1591; both Sorceress units carry
`Aslo` and `Aply`, and the map researches `Rhst` level 2 for players 0 to 2).
Neither spell is implemented. Polymorph cannot reuse the form path: the sheep
form changes the model, ability list and pools, all of which `prepare_morph_form`
refuses. Slow needs movement and attack speed modifiers that the activity and
combat models do not have.

## Remaining fidelity questions

The probes cover uninterrupted ordinary forms, not manual interruption, pause,
death, damage, every morph flag or external duration modifiers. Native timer
rounding and visual animation/altitude parity remain unverified. Unsupported
source combinations must fail before mutation; they must not borrow values or
success behavior from the selected campaign example.

## Replacement forms

Ordinary forms keep the model, ability association and pool rules, and refuse any
change. Polymorph needs the opposite, so `prepare_form` takes `FormChange::Replacement`:
it skips those three checks and the caller commits with `commit_replacement_form`, which
also swaps the model animation and owned abilities and keeps the unit's life, mana, armor
and defense type. Only Polymorph uses it; see
[slow and polymorph](slow-polymorph-abilities.md) for what it does not model.
