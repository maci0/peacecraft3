# Immediate order behavior

`IssueImmediateOrder` remains unsupported by `ObjectHost`. The retained Human01
VM completes globals and `config`, then refuses the call at map span
`67827..67884`; its selected `common.j` declaration is at `106038..106124`.
No campaign frontier advancement or playable chapter is claimed here.

## Source association

The refused unit already retains its resolved `Definition`, including map unit
overrides. Its ability list is metadata field `uabi`, declared `abilityList`,
from `UnitAbilities/abilList`. Read that executed unit's definition rather than
assuming its base unit's abilities or copying rawcodes into an order table.

`Editor` does not load the ability-function profiles. The existing
`DefaultTables::object(Family::Ability, ...).profile(...)` does, preserving every
assignment and its provenance. For this call, exactly one associated ability's
`Unorder` assignment matches the executed string. Its selected member is
`Units\HumanAbilityFunc.txt` from compatibility `War3Patch.mpq`, FNV-1a64
`c4b72f8dcc1cb49e`, profile line 293. The earlier base `war3.mpq` member hash
`698a0abcb11cc53db9896fe6477848cbd0e6be256ce56ebccc61bb5cd9542931`
identifies a different archive read; it is not the selected member's identity.
This catalog policy does not establish native Legacy archive priority.

## Original-client control

An isolated Wine run of the supplied `1.29.2.9231` executable, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
loaded an instrumented copy of that Human01 map through Custom Game. All 17
retained members were re-read; only `war3map.j` changed. The experiment archive
omits the original attributes member and opaque signature trailer. It makes no
authenticity claim and does not change the source map.

The probe creates two units from the captured rawcode, player and coordinates.
It reads the on/off strings and research requirements from the selected ability
profile. Research level one and a script seed are explicit experiment stimuli,
not inferred defaults. Both units report ability level one. Booleans are encoded
by the probe's explicit true/false branches.

| Sequence | Observed return |
| --- | --- |
| Initial off on each fresh unit, before research stimulus | false |
| On on unit A, after research stimulus | true |
| Repeat on on A | false |
| Off on A | true |
| Repeat off on A | false |
| Deliberately invalid order on A | false |
| On, then off on untouched unit B | true, true |

Two independent runs use actual 3840×2160 and 1280×960 framebuffers/clients at
96 DPI, with fresh private profiles and the identical probe map. Both in-game
marker captures and generated preload files contain these results. The complete
2168-byte output files have the same SHA-256,
`7babfa3325ca938d46ed10e23f912734f78e5be6941d039a1f07231bb1fb1fd0`.
Probe map SHA-256 is
`fa1a7f8bb008073f81a37e72fbd3b63aa39a042ef915d1d5954ea09698c25177`.
Local preparation and source association are retained under
`.scratch/cursor-gameplay-20261016/root-immediate-orders/`; the runtime receipts,
input journal and unchanged captures are under
`.scratch/original-game-vm/orders-control-4k-1/` and
`.scratch/original-game-vm/orders-control-1280-1/`. Both containers were stopped
after capture; their files remain local.

This observes one ability and one sequence under Wine, not general order,
requirement, damage, animation, clock or network behavior. In particular, the
pre-research off results do not isolate the effect of the research requirement.
The output is read as data; the generated preload script is never executed.

## Implementation boundary

Returning true merely because a string was retained would contradict this
control. An order consumer must join the actual unit and ability, apply the
supported ability's state transition, and preserve unsuccessful orders as false.
Unknown behavior must remain an explicit unsupported path. Profile `Order` and
`Unorder` assignments alone do not establish mana, cooldown, research, effects
or the behavior of every ability with such assignments.

The pinned reconstruction source provides a separate implementation reference:
[Warsmash Jass2](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/parsers/jass/Jass2.java)
checks unit abilities for use and no-target admission before dispatching an
immediate order. Its
[active toggle](https://github.com/Retera/WarsmashModEngine/blob/f9e0aeed4be372d6016519d0e97b384aa873f374/core/src/com/etheller/warsmash/viewer5/handlers/w3x/simulation/abilitybuilder/ability/CAbilityAbilityBuilderActiveToggle.java)
also checks active state, mana and cooldown. Those sources are not native-code
evidence or proof that every profile uses that class. Their pinned hashes remain
in the local `root-immediate-orders/warsmash/sources.json` receipt.
