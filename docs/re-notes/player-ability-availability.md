# Player ability availability

`SetPlayerAbilityAvailable(player, ability, available)` is admitted against its
loaded declaration. The host stores bounded overrides by current player identity
and the script rawcode integer. Player handles are checked by the existing player
host; there is no second slot roster. Replacing an override does not consume a
new record, and a refused new record leaves existing overrides unchanged.

The implementation treats an unrecorded ability as available. This availability
gate is separate from learned rank, research, target eligibility, mana and
cooldown. Disabling an ability does not erase a learned rank or refund skill
points. Holy Light command discovery and cast preparation consult the caster's
current owner; walking casts recheck before spending mana or applying effects.
Defend orders consult the authored ability alias, independently of its class
code and required research. Ownership changes need no cached override migration.

These are recreation semantics for the declared native. Tests establish the
implementation's player isolation, owner changes, command hiding, cast refusal,
walk cancellation, re-enabling and preservation of state. They do not establish
original-client behavior for disabling an active toggle, timing within a native
simulation tick, or whether every script-issued ability order is treated alike.
Passive effects and other unimplemented ability families are not newly modeled.

The production startup builder admits the same state. A local Human03 gate with
the selected Legacy install and seed 1 currently refuses earlier at
`war3map.j:1097`, creating `nogm`: the placement engine rejects a collision radius
larger than its currently supported cell geometry. Therefore this change does not
claim Human03 startup or campaign acceptance. Preserve that refusal until the
general placement path is extended; do not clamp the radius or omit the unit.

Synthetic checks live in `player_ability_tests.rs`, `combat_tests.rs` and
`unit_host_tests.rs`. Original game assets are not required by those tests.
