# Heal and Neutral Heal: bounded original-client observations

## Identity and collection

These are controlled runtime observations of the supplied `Warcraft III.exe`
1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
They qualify specific Heal lifecycle behavior, not unchanged-campaign or native
Windows parity. The client ran under Wine in the existing local reference image
`sha256:ab778c3e5be5de42cbe35f7d649cab2c0688630838fbcb1a27e476a0f825ec6e`.

Seventeen diagnostic maps preserve supplied Booty Bay terrain and main setup
(source map SHA-256
`e1c5984189cd046b112ae59b870cf1d585b2b2445368858ea64c7685d4180268`).
Instrumentation disables competing melee AI and victory, discovers the starting
worker type and town-hall location, removes the starting player-zero units, and
places unpaused actors with pathing disabled. Placement offsets come from the
selected ability's parsed range; health and mana fractions, action steps and
sampling limits are explicitly synthetic experiment controls.

Each run uses a fresh private prefix, Xvfb and verified Pulse null sink, with no
host display/audio sockets, devices or network. All seventeen containers were
independently inspected after collection and are stopped. Each trace contains
128 ordered state samples at 0.125 game seconds plus spell-event callbacks.
Collection refuses foreign prefixes, overflow, missing samples and startup
errors; its wall-clock admission is bounded. State sampling gives transition
brackets, not exact deadlines. Event timestamps have the client's printed
precision and clock behavior.

The local builders copied the existing MPQ recipe before adaptation. They update
`(attributes)` with the new script's CRC32, change only the script and attributes
block records, independently extract and compare both, and compare unchanged
terrain. Source `common.j`/`Blizzard.j` syntax resolution passes for every map.
No map, source script, asset table, executable, trace or generated asset copy is
stored in this repository.

## Sources and independent receipts

Local manifests and verified receipts are under:

- `/tmp/pc3-sol09-heal-native-probes-1794512376816455568/`: eight Heal controls.
- `/tmp/pc3-sol09-heal-native-probes-1794513623362117930/`: low mana, EFFECT-time
  Stop/retarget and a discriminating different-maximum-life ranking control.
- `/tmp/pc3-sol09-heal-native-probes-1794514936397147752/`: mark release,
  EFFECT-time ability removal and caster death.
- `/tmp/pc3-sol09-heal-native-probes-1794514998209743837/`: Neutral Heal automatic
  and manual positive controls.

Each directory retains `manifest.json`, `native-status.json`, independent
`container-verification.json` and `runtime-evidence-receipt.json`. These identify
map, staged executable, source inputs, raw and newline-normalized trace digests,
phase events, actions and buff transition brackets. The independent verifier
`/tmp/pc3-sol09-verify-heal.py` passes all 2,176 samples. Behavioral assertions in
`/tmp/pc3-sol09-assert-heal-behavior.py` pass; receipt
`/tmp/pc3-sol09-heal-runtime-behavior-receipt.json`, verifier-script SHA-256
`4924b5485c9882e51ba70170c58d70fa55b19b3d7df963d8ee78be2092bffc5f`.
The assertion readout tolerance is diagnostic precision, not a game-rule value.

Input rows come from the supplied Legacy `War3x.mpq` unprefixed
`Units/AbilityData.slk`, `Units/UnitAbilities.slk` and the relevant ability
profile. Their exact local extracted-member hashes are in each manifest. The
input table alone does not prove active dataset selection. Actual traces check
RoC=true, TFT=false, Human player-zero race, caster type and positive ability
level. Heal selects the lexically first source auto-enabled candidate (`hmpr`)
as a diagnostic policy. Neutral Heal selects the lexically first positive
`InBeta` source candidate carrying alias `Anh2` with default code `Anhe` (`ndth`);
that selection policy is not a reconstructed edition rule.

## Selection and cadence

With a nearer target at higher health percentage and a farther target at lower
percentage, automatic Heal selects the farther target first. A second control
uses different unit maxima: the farther target has *more current life*, but a
lower current/max ratio, and is still selected first. This distinguishes the
observed ratio preference from nearest-target and lowest-current-life rules.
It supports the native comparator join in `ability-autocast-tick.md`; it does
not qualify every reference-metric branch or target filter.

Two fixed equal-health/equidistant controls reverse creation order. The same
west-positioned target is selected first in both, despite its handle changing
from higher to lower than the east target. This rejects creation order or handle
order as an explanation for those controls. Universal enumeration/tie order is
still open; no leftmost rule follows from two placements.

The fixed east-target automatic control produces EFFECT at 0.520020,
2.050049, 3.580078 and subsequent approximately 1.53-second intervals. Manual
Heal produces EFFECT at 0.500000 and approximately 1.51-second intervals.
Equal-distance east/west alternation instead gives approximately 1.71 seconds.
These observations do not support scheduling solely from the authored cooldown,
or equating CHANNEL time with acquisition time. Turn/phase/source-field joins
remain necessary.

A zero-starting-mana control first reports CHANNEL at 9.020020 and EFFECT at
9.520020. Static getter `0xb9cbb0` loads `0x112d7c8`; initializer `0x4015c0`
passes its integer operand to `0x9a1620`, deriving the retry interval. That joins
repeated evaluation rather than a guessed cost-plus-one mana gate. Getter
`0xb9cbd0` loads `0x112d810`, initialized through `0x401460`; this is a separate
post-cast minimum, not a substitute for the full `+0x3e8` duration getter.
The integer operands must be read from the selected image when implemented.

## Effects, payment and cancellation

EFFECT callbacks see the pre-payment mana, pre-heal life and no new `Bhea`.
The first following state sample shows the parsed `Cost1` debit and `DataA1`
healing, after accounting for measured regeneration. Neutral Heal's caster has
its own regeneration rate; the assertion derives that rate from its pre-effect
observations instead of borrowing the Heal caster's rate.

Stop, `UnitRemoveAbility` and `KillUnit` inside the first EFFECT all cause nested
ENDCAST with the original target and old mana. In these three controls the
same-time state sample after callback return shows the cost paid, no heal gain,
no `Bhea`, no cooldown and no FINISH. Subsequent observations preserve that
absence. Payment therefore cannot simply disappear when an active cast job is
cancelled during EFFECT. These controls do not establish every resource clamp,
refund or callback mutation policy.

An accepted retarget issued inside EFFECT does not immediately replace the
active repeated Heal target: every later pulse in that arm still addresses the
original target. Stop at three seconds with autocast disabled prevents later
pulses until a new manual request. With autocast enabled, automatic casting
resumes after the same Stop. Returning true from an order is not proof of an
immediate cast-target transition.

## Caster-owned mark

A second healer's request for the first healer's `Bhea` target returns false;
its request for an unmarked target succeeds. After the first healer stops, its
mark remains visible. In the fixed control, the last first-healer EFFECT is
2.010010; the mark is present at 4.000000 and absent at 4.125000. A request
conditional on this observed release returns true. The second healer nevertheless
continues its existing target in that control, matching the retarget distinction
above. Do not convert the sampling bracket into a hard-coded mark duration or
interpret accepted release as immediate acquisition/effect delivery.

## Neutral Heal and remaining implementation

The Neutral Heal controls use ON/OFF/cast IDs decoded from the selected
`CAbilityNeutralHeal` name anchor, vtable and constant getters, with image/hash
and instruction-shape checks. They do not guess aliases from the expansion
profile. The profile/native order mismatch remains recorded and unresolved for
public host toggles. Actual automatic Neutral Heal produces the same target and
EFFECT sequence as the paired Heal ranking control; manual Neutral Heal repeats
on its requested target. These are positive original-client observations beyond
creation/default activation.

The host still refuses Heal/Neutral Heal acquisition at its due evaluation.
Implementing it requires the canonical target filters and visibility, native
ranking/enumeration, full readiness, turn and post-cast duration bindings,
retained post-EFFECT payment, repeated phases and caster-owned mark lifetime.
No one-shot heal, guessed nearest selector, silent idle success or broad native
parity claim is justified by this qualification.

## Timing bindings and phase writer follow-up

`autocast_rules::Class.tick` now independently binds the two timing virtuals:
`+0x470` for retry and `+0x474` for the minimum after a selected cast. The decoder
checks each getter, the matching bootstrap destination and integer-to-real call,
and reads the bootstrap operand. Unknown shapes leave timing unadmitted without
removing the existing ON/OFF capability. Nonstock operands and relocated getter
and global addresses, destination/call/opcode drift, nonpositive values and class
integration have explicit synthetic regressions. The selected-image gate joins
Heal, Neutral Heal and Slow's decoded timing. This supplies rule inputs; it does
not connect a class's target acquisition or execute a cast.

Further read-only disassembly of `0xbed2f0` identifies an important mutable phase
writer. At `0xbed36f..0xbed3a1`, Heal obtains virtual `+0x2ec`, adds the image
operand referenced at `0x10db058`, and writes FloatMini `+0x94` (value `+0x98`).
At `0xbed3ae..0xbed3cc`, it adds the post-cast minimum global to this updated
value for the `Bhea` creation/refresh duration. The exact virtual-to-source-field
and other phase-writer joins remain open. A source `castbsw` value cannot simply
be kept immutable across this effect.

The generic duration routine `0xb4a600` compares `+0x98` against an image
sentinel read from `0x10db134`, using the tolerance referenced at `0x10db054`.
Mapped image bytes identify that sentinel as positive FLT_MAX, not unity. This
is static native-code evidence, distinct from the sampled runtime brackets.
The local disassemblies are `/tmp/pc3-sol09-heal-effect.asm` and
`/tmp/pc3-sol09-common-postcast-delay.asm`; full retained Heal effect dossier
`ev_20c69a4aeb2e1756d457f245f0d49a12e297778539564799445e7c371b853731`
is in `/tmp/pc3-sol04-slow-native-evidence.json`.

## Mandatory evaluation boundary

The session's existing mandatory-activity wake selection now includes each live
unit's pending automatic evaluation. A large requested clock advance therefore
stops at the first pending evaluation rather than observing its refusal only at
the caller's later horizon. Unsupported acquisition still refuses at that
boundary and retains its deadline. OFF and unit removal stop forcing that wake.
The activity phase order is unchanged; no moving-target or positive-cast phase
ordering is inferred by this hook. Public creation, OFF, removal and clock
regressions cover the boundary and unchanged unit state on refusal.

The printed event times above were sampled with a long-running 2000-second
clock. The independent paired-clock audit in
`/tmp/pc3-sol02-clock-audit/report.txt` demonstrates elapsed-query reconstruction
bias for such a clock while the paired short clock tracks the requested delays
at the diagnostic precision. These printed increments must not become a
universal 8192-based scheduling grid. Timer-query arithmetic and actual spell
queue deadlines remain separate fidelity questions.

## Mana lower-bound binding

The shared rule input now admits `Rules.mana_minimum` from CUnit's mana proxy,
rather than introducing another literal lower bound in spell settlement. CUnit
constructor writer `0x675835` binds proxy `+0x1c0` to the Float vtable. Its
`+0x18` initializer is `0x800cd0`: argument five becomes descriptor `+0x10`,
the minimum. CUnit initialization `0x670a10..0x670a38` supplies the minimum's
address; bootstrap `0x401300` initializes that same global through the native
integer-to-real conversion. The decoder reads the operand and checks these
relationships, including the proxy/helper vtable link and descriptor forwarding.

The resource initializer `0x8c3ee9..0x8c3f05` copies descriptor `+0x10` to
resource `+0x80`; `0x8c47a0` clamps against this bound both before and after its
resource delta. Those writes and clamp readpoints are also checked. This binds
the mana-specific lower operand without asserting an identical implementation
of the original generic property registry. That registry's complete deferred
instantiation path remains unqualified; other resource lower bounds must not be
inferred from mana. Existing synthetic rule fixtures leave the capability absent
unless supplied explicitly, preserving the unsupported boundary for consumers.

The associated retained native dossiers include CUnit initialization
`ev_d5cbe985ea147539e895b6db5229e08a36070e37fd348f420b1cef539bbf2abb`,
CUnit construction
`ev_a8fc81bd2bfacb0e81fa9edc2b6d7f29c71665999a883162990f88ce0ba30a85`,
Float initialization
`ev_f483f8b1108cef7021f9a18959d9ff5a68a571ee82c96f08c5a3c9a0b2737333`,
and resource initialization
`ev_417ddc9988420f0fe5555747f5299a33b3d9d6b6ac12fc3d04253f77b3d2c9b0`
in the local evidence bundle named above. Nonstock negative, zero and positive
initializer operands, relocated proxy/helper/global addresses, changed component,
argument, descriptor, forwarding and clamp roles, and truncated initialization
have explicit regressions. This rule binding alone does not implement Heal
payment or its retained cast lifecycle.
