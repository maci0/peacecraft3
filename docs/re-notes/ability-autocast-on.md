# Ordinary autocast ON orders

Reviewed executable: `Warcraft III.exe` 1.29.2.9231 (SHA-256 `3f2ed012...70eed`), addresses are
virtual addresses at image base `0x400000`. Companion to `ability-autocast-off.md`, which owns the
class discovery (RTTI-style class-name strings, getter, name slot, vtable = name slot - 0x58) and
the OFF handler.

## Class ABI slots used here

Read from the class vtable (byte offsets); `Ainf` is `CAbilityInnerFire`, vtable `0xf676bc`.

| slot | meaning | Inner Fire |
| --- | --- | --- |
| `+0x1c` | class code getter (`mov eax, imm; ret`) | `Ainf` |
| `+0x1b8` | exclusion group (`mov eax, imm; ret`, or `xor eax, eax; ret`) | 1 |
| `+0x310`, `+0x314` | ON order id, ON order flags | `0xd0063`, `0x2006c01` |
| `+0x320`, `+0x324` | OFF order id, OFF order flags | `0xd0064`, `0x2007c01` |
| `+0x328`, `+0x330` | cast order id (tail jump to `+0x350`) | `0xd0062` |
| `+0x338` | ON wrapper (`call [vtable+0x340]; ret 4`) | `0xb81080` |
| `+0x33c` | OFF wrapper (`call [vtable+0x344]; ret 4`) | `0xb2a310` |
| `+0x340`, `+0x344` | ON handler, OFF handler | `0xb99a80`, `0xb99b50` |
| `+0x348`, `+0x34c` | post-toggle hooks | `0xbdb5e0`, `0xbdb5f0` |
| `+0x434`, `+0x438` | activate, deactivate | `0xb9c590`, `0xb9c5c0` |
| `+0x450` | acquire target | `0xb9c8e0` |

## IssueImmediateOrder validation (observed in shipped code)

- The native registers at `0x49e03a` and runs `0x49a610`: order string to id, then `0x6a16b0`
  (unit validation). It scans the unit's abilities (`[unit+0x3e8]` list); each ability checks the
  order through `vtable+0x264` (`0xcb6180`), which calls `vtable+0x30c` (`0xb28850`), then
  `0xb4adf0`, then the toggle check `0xb994b0`.
- `0xb994b0` for the ON id: returns `0xdd` ("not applicable") when state bit `0x80` of `[ability+0x20]`
  is already set, else `0`. For the OFF id: `0xdd` when the bit is clear, else `0`. Exception: an
  OFF id equal to the global at `0xe9ba14` (`0xd0142`) skips the ON-side bit test.
- A result other than `0` or `0xdc` makes the native return false with no order. `0` creates the order
  (`0x698910`), issues it (`0x690590`) and the native returns true. A unit with no ability for
  the order ends the scan at `0xdd`, so the native returns false.

So ON of an already-enabled ability, and ON on a unit without the ability, return false with no
effect. ON of a disabled ability is accepted.

## ON handler `0xb99a80`

Shared by all 331 class vtables that inherit the base toggle (all abilities, `+0x340` is the same
function). Steps, from the bytes:

1. Owner `[this+0x30]` (or `0x7f2610`). If owner `[+0x164]` has `0x100000` or `0x200000`, or
   `[owner+0x15c] > 0`, the handler leaves the state alone and, unless `0x100` is set and
   `[owner+0x3a4] > 0`, reports through `0x698910` and `0x6994a0`. This is the same gate as the OFF
   handler at `0xb99b50` (dead, disabled owner).
2. If bit `0x80` of `[this+0x20]` is set: return, nothing changes.
3. If the OFF id is not `0xd0142`: read the group (`+0x1b8`). If nonzero, call `0x6d9270(owner,
   group)`, which calls the OFF handler (`+0x344`) of every ability of the unit with flag
   `0x20` clear and an equal group. Then `[this+0x20] |= 0x80`, run `0x46e070` (command-card refresh
   through `0x5f5d70`).
4. Tail jump to `vtable+0x348`.

The handler sets no return value; the order's true comes from the validation above.

Group exclusion: enabling an ability switches OFF every other ability of the same unit whose class
has the same nonzero group and is currently ON. The OFF of such a peer runs the plain OFF handler:
state bit cleared, update hook run, no order and no event. Measured over the image: 34 classes
carry a nonzero group (27 with group 1, among them `Ainf`, `Aweb`, `Arai`, `Ahea`, `Aslo`; 7 with
group 2: `ANba`, `AHfa`, `AHca`, `Afak`, `AEpa`, `AHRa`, `ANia`); the other 297 return 0.

## Update hook and activation

`+0x348` (`0xbdb5e0`) jumps to `0xbdb730`. It computes active = `[this+0x3c] <= 0`, flag `0x200`
clear and `flags & 0x82`. Active and bit `0x80000` clear: set `0x80000` and call `+0x434`
(`0xb9c590`). Inactive and `0x80000` set: clear it and call `+0x438` (`0xb9c5c0`).

`0xb9c590`: if flag `0x100000` is clear, register timer event `0xd01c2` on `this+0x128` through
`0x7fbe90` with the delay float at `0x10db05c` (`0x3ca3d70a`, 0.02 s). The event dispatcher
(`vtable+0xc`, `0xb9c5f0`) routes `0xd01c2` to `0xb9d120`.

## Automatic cast tick `0xb9d120`

Runs when the timer fires. Observed conditions, in order: ability flag `0x100000` clear; the
re-arm delay is read through `+0x470` (`0xb9cbb0`, global `0x112d7c8`); `[this+0x20] & 0x20`
clear; `0x80000` set; ready check `+0x3e0` (`0xb4a4a0`) nonzero; owner `[+0x164] & 0x1000000`
clear, `[owner+0x20] & 0x4000000` clear and `[owner+0x3a4] <= 0`; then `+0x450` (`0xb9c8e0`)
returns the target. A nonzero target sets `0x220000` and casts through `0xb4b0f0` or `0xb2b080`
according to `+0x354` bits; then the timer re-arms with `max(+0x3e8, +0x474)`.

Not decoded, so not implemented: the acquisition scan in `0xb9c8e0` (range `+0x44c`, candidate
filters `+0x3f4`, `+0x458`, `+0x45c`, `+0x460`, `+0x468`, ordering `0x9a01c0`); the ready check's
mana and cooldown terms (`0xb4a4a0`, `0xb4a530`); the cast itself and the buff each class applies.

Default-enabled abilities (`UnitAbilities.auto`: `Ainf` on `ntks`, `Aweb` on `ucry` and `nnwl`) run
the same activation when the unit is created in the original. Creation now arms the decoded first
evaluation for admitted ordinary autocast classes; automatic acquisition and casting remain open.

## `CAbilityWeb` (`Aweb`) uses the same ON handler (observed)

The class name string is `0xf802f8`, its getter `0xc1bea0`, its name slot `0xf7def0`, so the
vtable is `0xf7de98`. Its slots equal Inner Fire's: ON wrapper `+0x338` `0xb81080`, OFF wrapper
`+0x33c` `0xb2a310`, handlers `+0x340`/`+0x344` `0xb99a80`/`0xb99b50`, hooks `+0x348`/`+0x34c`
`0xbdb5e0`/`0xbdb5f0`, activate/deactivate `+0x434`/`+0x438` `0xb9c590`/`0xb9c5c0`. The ON id
getter `+0x310` (`0xc1bf10`) returns `0xd00f4` (`webon`) and the group getter `+0x1b8`
(`0xb81160`) returns 1. The shape check in `autocast_rules.rs` therefore binds it
(`selected_original_autocast_classes_bind` asserts `Aweb`). The `Aweb` refusal in the root lane's
`NightElfX07` row predates the ON binding: the probe at `4364677f`, before any change of this
lane, already passes the chapter. There is no different handler shape to decode.

TFT `NightElfX07` (`war3map.j:1520` onward) issues `webon` on `ucry` units; `Aweb` is default-on for
`ucry` (`UnitAbilities.auto`), so the order returns false with no change, and the chapter probe
passes (`executed_units=279`).

## Recreation

- `autocast_rules.rs` decodes per class the ON handler shape (state bit, group call, tail hook), the
  update hook, the activation timer and its delay float, and the group of every class sharing the ON
  handler. A class whose handler, hook or activation differs (`CAbilityManaShield`,
  `CAbilityImmolation`) has no ON decode and its ON refuses before any change.
  `CAbilityBlackArrow` (`ANba`, `blackarrowon` `0xd0262`, group 2) joins the bound classes.
  `selected_original_autocast_classes_bind` checks the real image (34 grouped families).
- `autocast_host.rs`: ON of a disabled ability returns true, runs the same order interruption, events and
  state revision as OFF, restores a default-on ability to its authored state or records it as enabled,
  switches the group peers OFF without events, and arms the first tick at `now + 0.02` (the decoded
  float). Already-enabled, missing-ability, dead and paused units return false without a change.
- The tick is not implemented. When game time reaches an armed tick, `advance_unit_activity` fails
  with `automatic cast of <alias> ... target acquisition and the cast are not implemented`.
  OFF, peer switch-off and unit removal disarm it. A startup script that issues ON and returns before
  game time moves 0.02 s never reaches the tick, so the order is faithful there.

Inferred, not measured: the ON order stops the unit and emits the issued-order events exactly as the
runtime-probed OFF does (same validation, order creation and issue path); no ON runtime probe exists.

## Default activation scheduling (2026-11-13)

The shared unit factory prepares default timers from the selected unit's `auto` list, matched
against each present ability's source `code`, rather than its alias. The existing selected-image
activation decode supplies the delay. Committing creation timestamps the timers against the game
clock, without issuing an ON order, switching group peers or marking default state as edited.
Malformed lists, an undecoded activation for a matching default, invalid delays and a missing clock
refuse before allocating the unit. Hosts without the optional autocast capability retain their
existing admission boundary. Corpse and illusion creation are excluded; their activation and
resurrection behavior remain unqualified.

This is scheduling support, not completed autocast. At the exact first due evaluation the existing
unsupported-acquisition error is now exposed for default-on abilities too. Ordinary OFF, peer
exclusion and unit removal retain their existing timer cleanup. Readiness, pause/death/transfer,
ability addition/removal/learning, restored timer phase, visibility, acquisition ranking,
subsequent retry delays and actual spell effects remain open as
described in `ability-autocast-tick.md`. Maps that previously advanced while silently skipping these
default-on evaluations may now refuse there.

Synthetic `autocast_host_tests` cover a class code different from its alias, the last representable
time before the deadline and the exact deadline, a repeated ON that must not rearm, unchanged
unit/RNG state when the due evaluation refuses and is retried, OFF/removal cleanup, re-enabling,
a missing ability and malformed/undecoded activation before unit allocation. These exercise the
public creation/order paths and host clock; they do not add original-client runtime parity evidence.

## `CAbilityManaBattery` (`Ambt`, Moon Well) uses the same ON handler (observed)

The class name string is `0xf2ffb4` (`CAbilityManaBattery`), its getter `0xb813a0` and its name
slot `0xf2fb48`, so the vtable is `0xf2faf0`. Its slots equal Inner Fire's: ON wrapper `+0x338`
`0xb81080`, OFF wrapper `+0x33c` `0xb2a310`, handlers `0xb99a80`/`0xb99b50`, hooks `0xbdb5e0`/
`0xbdb5f0`, activate/deactivate `0xb9c590`/`0xb9c5c0`, acquire target `0xb9c8e0`. Only the ON id
getter `+0x310` (`0xb81610`), OFF id getter `+0x320` (`0xb81620`) and the group getter `+0x1b8`
(`0xb81160`, group 1) are class specific. The class was only missing from the bound list
(`CLASS_NAMES`); there is no different handler shape.

`Units\NightElfAbilityFunc.txt` (`Ambt`) and `UndeadAbilityFunc.txt` (`Amb2`, same class code)
author only `Order=recharge`, the cast order. No `Orderon`, `Orderoff` or `Unorder` key exists, so
the data cross-check of the toggle keys has nothing to compare and the ON and OFF ids come from the
class getters alone (`rechargeon`, `rechargeoff` are the strings the order registry maps to them).

The same vtable shape holds for `CAbilityReplenishLife` (`Arpl`, vtable `0xfc37bc`) and
`CAbilityReplenishMana` (`Arpm`, `0xfc3c48`), which are not bound (not needed by any probed chapter).

## Neutral Heal default activation (2026-11-13)

After main integrated default scheduling at `5ad20afb`, the unchanged RoC Undead05 horizon
refused `CreateUnit` for `Anh2` at script line 909; local receipt
`/tmp/pc3-ai-horizon-main/Undead05.log`. The selected local Legacy `War3x.mpq` unprefixed
`Units/AbilityData.slk` row resolves that alias to code `Anhe`, rather than `Ahea`.
This is shipped-byte evidence for that archive member, not a claim about every dataset.

Read-only disassembly of the reviewed image above identifies `CAbilityNeutralHeal` (name getter
`0xbed5e0`, code getter `0xbed5f0`, vtable `0xf64efc`) and `CAbilityHeal` (vtable `0xf64a6c`).
Their ON/OFF handlers, update hook, activation and deactivation virtuals are identical:
`0xb99a80`, `0xb99b50`, `0xbdb5e0`, `0xb9c590`, `0xb9c5c0`. The Neutral Heal factory at
`0xbf0260` calls the same base initializer `0xb2ac80`, clears the same timer region as the Heal
factory, and adds no separate class-specific flag write. Both deleting-destructor wrappers call
`0xb82570`. This supports using the existing decoded ordinary default activation lifecycle.
The omitted class name is now included in the decoder; no duration, group or order ID was copied
into the host. The default unit therefore reaches its scheduled evaluation instead of failing
class admission during creation. Acquisition and healing still refuse explicitly at that boundary.

Neutral Heal's three native order getters differ from Heal's, while the inspected expansion
profile `Units/NeutralAbilityFunc.txt` spells the ordinary heal orders for `Anh2`. This mismatch
does not enter default activation, which issues no order. Explicit order/profile reconciliation
remains unqualified; the existing strict toggle cross-check is retained.

The explicit original-image binding gate checks `Anhe` activation against `Ahea` and verifies
their native toggle IDs differ. `selected_original_neutral_heal_activation_reaches_the_default_evaluation`
feeds the decoded `Anhe` class code and delay into a synthetic unit definition with a different
alias, creates it at nonzero game time through `CreateUnit`, and checks the exact first-evaluation
boundary without changing unit state on refusal. This is a native-input/caller regression, not
an original-client or unchanged-mission runtime parity result. Ghidra import timed out before
returning a function dossier; the new native observations here are direct PE/disassembly evidence.

Independent main validation passes the full gameplay package (3,169 passed,
zero failed, 145 explicit ignores) and all 36 autocast tests with both explicit
selected-image gates enabled. Strict native/WASM all-target Clippy, both
libraries, formatting and the application test-binary rebuild pass with source
hashes unchanged. The local image gates bound file reads before allocation.
Receipts are retained in `/tmp/pc3-neutral-heal-main-validation/`.

The unchanged Undead05 replay now publishes 194 units; it no longer refuses
`CreateUnit` for `Anh2`. It then refuses automatic `ACbl` acquisition at
`0.019999999552965164`, so it does not complete the opening or idle horizon.
The exact executable hash and failed command are retained under
`/tmp/pc3-neutral-heal-horizon-main/`. This is recreation frontier evidence,
not an observed `Anhe` cast or original-client parity.

## Spell Steal and Orb of Annihilation default activation (2026-11-13)

The unchanged TFT startup receipts under `/tmp/pc3-thematic-horizon-main`
refuse default activation for `Asps` in HumanX01, HumanX03Interlude and UndeadX04,
and `Afak` in HumanX02. The selected local `War3x.mpq` unprefixed
`Units/AbilityData.slk` binds `Asps` to code `Asps` (Spell Steal), and `Afak` to
code `Afak` (Orb of Annihilation). `ANak` is another authored alias for `Afak`.
The native class name alone must not determine the effect. `UnitAbilities.slk`
gives `hspt` and `nbel` default `auto=Asps`, and `ubsp` default `auto=Afak`.
HumanX02's failing line 1987 creates `ubsp`; the other three scripts create the
corresponding `hspt`/`nbel` units. These are shipped-byte joins, not new mission
horizon observations.

Static disassembly of the same reviewed image identifies `CAbilitySpellSteal`
(vtable `f6b6b8`, code getter `bf8320`) and `CAbilityFlamingAttack` (vtable
`f966c8`, code getter `c4a620`). Their factories `bf8f60` and `c4a810` call
`b2ac80`; their complete post-base embedded timer initialization before the
class-vtable write is byte-identical to Heal's. Neither adds a separate state
flag write. Their ON/OFF wrappers, ON/OFF handlers, update hooks, activation,
deactivation, timer-event dispatch and retry/minimum getters equal Heal's:
`b81080`/`b2a310`, `b99a80`/`b99b50`, `bdb5e0`/`bdb5f0`, `b9c590`,
`b9c5c0`, `b9c5f0`, and `b9cbb0`/`b9cbd0`. The shared activation reads its
first delay through the same image operand at `10db05c`. The existing decoder
continues deriving those rules and exclusion groups from the image.

The selected-image gate exposed an additional discovery boundary: Spell Steal's
name-getter address also occurs as operand bytes in executable code. The PE's
initialized ranges include that code; treating all of them as class data made
the pointer search ambiguous. Discovery now excludes the parsed executable
ranges from its initialized-data scan. Code getter discovery remains in code,
and the existing bounded scans, source hash and true data ambiguity refusals
remain strict. A nonstock regression distinguishes executable pointer
coincidences from duplicate pointers in actual data; the selected-image gates
exercise the corrected PE range selection.

The two class names are now admitted through that existing decoder. The patch
changes no clock, common loop, target acquisition or cast implementation.
Spell Steal's cast/ON/OFF getters join the native registry's `spellsteal`,
`spellstealon` and `spellstealoff`, matching `HumanAbilityFunc.txt`. Orb of
Annihilation's native getters join `flamingattacktarg`, `flamingattack` and
`unflamingattack`; its `UndeadAbilityFunc.txt` row contains no order keys.
No profile order alias is invented, and the existing strict cross-check remains.
Default activation issues no explicit ON/cast order. Their cast flags and ranking
virtuals differ, so common-toggle equality does not qualify either actual spell.

The selected-image class gate now checks both bindings and compares their
activation/tick fields with other decoded classes. Two additional explicit
selected-image caller gates feed the parsed class codes and activation delays
into synthetic unit definitions whose ability alias differs from the code.
They exercise public creation at nonzero time, the last representable host time
before the first boundary, exact-boundary unsupported-work refusal, repeated
refusal with unchanged unit/RNG/handle/event state, and OFF/removal disarming.
Fixture toggle names are synthetic and make no claim about original order
spelling. Existing malformed/unknown-rule tests retain their strict boundary.
These checks qualify native input and canonical callers; they do not measure
original-client deadlines or prove spell effects, startup horizons or tick parity.

Local read-only qualification receipts, source hashes and bounded byte-join
assertions remain at `/tmp/pc3-sol61-08-autocast-qualification.{md,json}`.
REA session discovery failed with a provider cleanup error; these observations
use direct PE/disassembly evidence rather than a new decompiler dossier.
No original asset bytes are committed.

Branch validation passes all 52 focused autocast tests with all four local-image
gates explicitly enabled, including both new public lifecycle gates and
`executable_getter_pointer_coincidence_keeps_true_data_ambiguity_strict`.
Strict native all-target/all-feature Clippy, WASM all-feature Clippy, both
all-feature library builds, and formatting of the three changed Rust files pass.
The driver used two jobs, the lane's isolated range target, and verified private
Xvfb/null audio. Checked Rust hashes remain unchanged. Command/exit receipts,
exact named-gate results and complete logs remain local under
`/tmp/pc3-sol61-08-autocast-{sources.sha256,validation-receipt.json,verified.log}`.
The initial failed image-discovery run is preserved separately as
`autocast-failed-anchor.log` and `autocast-failed-anchor-receipt.json` under the
same local prefix. No application rebuild or unchanged-mission horizon was run
in this branch; native clock alignment, acquisition and spell effects remain open.

## Parasite default activation (2026-11-13)

After the Spell Steal admission, unchanged TFT HumanX03Interlude and UndeadX04
startup replays refused default `ANpa` activation while creating `nnsw` at map
lines 226 and 1692. These are parent recreation receipts under
`/tmp/pc3-resident-lifecycle-horizons/`, not original-client observations.

The exact map headers, selected common version constant and native selection
operands derive `Custom_V1` for both chapters. That namespace's
`UnitAbilities.slk` lists `nnsw` with default `ANpa` and present ability `ANpa`;
`AbilityData.slk` resolves that alias to class code `ANpa`. The authored `ACpa`
alias also uses code `ANpa`, but the two failing callers carry `ANpa` itself.
The earlier V0 witness must not stand in for these expansion units. The new
ignored production source gate in `campaign_inputs.rs` uses the actual
`CampaignRecipe`, selected archive resolver, `defaults` and `MetadataCatalog`
for both unchanged map headers. It checks the default/list/code join and
reconciles selected profile orders against the image-derived order registry,
retaining exact map/table/profile provenance in the local test output. It adds
no alternate archive loader or production data-selection behavior.

Static PE/disassembly of the same reviewed Legacy image identifies
`CAbilityParasite`: vtable `fbc67c`, name getter `c99da0`, code getter `c99db0`,
factory `c9b260`. The factory calls `b2ac80`; its complete post-base embedded
timer initialization before the class-vtable write equals Heal's. Its toggle
wrappers/handlers, update hooks, activate/deactivate, timer-event dispatcher and
retry/minimum getters equal the admitted ordinary path. The first delay and
group continue to come from the existing image decoder. The native cast/ON/OFF
getters `c99e10`/`c99df0`/`c99e00` join registry `parasite`/`parasiteon`/
`parasiteoff`; these match the examined `ANpa` profile. No profile alias, timing,
spell effect or successful no-op is invented.

The only production behavior change adds this named class to the existing
admission list. The selected-image gate now includes all 21 classes and checks
Parasite's activation/timing against decoded peers. A dedicated public caller
case feeds the parsed native code/delay into the existing synthetic definition
with a different ability alias. Shared assertions cover creation at nonzero
time, no issued ON order, just-before/exact-first-boundary behavior, repeated
strict unsupported-work refusal without unit/RNG/handle/event changes, and
OFF/removal disarming. Existing Spell Steal/Orb gate names and coverage remain.

Clock adoption, Script/AI wakes, acquisition, damage, buffs, summoned units and
native deadline alignment are unchanged and unqualified here. Passing these
source/caller gates does not complete a chapter or establish Parasite casting
parity. Parent owns the unchanged-map startup/horizon reruns after intake.
Local static source/image receipts remain at
`/tmp/pc3-sol61-08-anpa-qualification.{md,json}`.

Validation on the released `2bce5832` baseline passes all 54 focused autocast
checks, zero failed or ignored, with all six source-image gates explicitly run.
These include
`selected_original_parasite_default_activation_reaches_boundary_and_disarms`
and
`selected_original_parasite_autocast_production_sources_follow_both_tft_headers`.
The production gate observes `War3x.mpq::Custom_V1/Units/UnitAbilities.slk` and
`Custom_V1/Units/AbilityData.slk` for both maps, with the profile from
`War3x.mpq::Units/CampaignAbilityFunc.txt`. Exact map/table/image SHA256 and
profile locators are in the local output. This closes the earlier read-only
qualification's unresolved production resolver join.

Strict native all-target/all-feature Clippy, WASM all-feature Clippy, both
all-feature library builds, and formatting of the four changed Rust files pass.
The five-stage driver used the isolated range cache, two jobs and verified
private Xvfb/null audio. Checked Rust hashes remained unchanged; receipts,
exact named-gate results and full output remain at
`/tmp/pc3-sol61-08-anpa-{sources.sha256,validation-receipt.json,verified.log}`.
No application rebuild or chapter horizon was run in this branch. Actual cast
and native clock/wake parity remain open.
